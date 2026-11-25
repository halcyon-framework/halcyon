//! Image-generation custody for one already-admitted SDF resource owner K.
//! Frame owners must retain guards for actual Image uses; registration protection
//! and producer/target admission are separate integration gates.

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use super::custom_render_pass::{CustomRenderPass, UnadmittedPreparationError};
use super::image_binding::{
    ImageAdmissionError, ImageBinding, ImageBindingFailure, ImageBindingIdentity,
};
use super::wgpu_renderer::{
    GpuResources, SdfReservationError, SdfTextureReservation, WgpuRenderer,
};

#[derive(Debug)]
pub(super) enum SdfImageCustodyError {
    Retiring,
    Retired,
    IdentityMismatch,
    QueueMismatch,
    Unavailable(ImageBindingFailure),
    Admission(ImageAdmissionError),
    CounterOverflow,
    Uncertified,
    PollFailed,
    Reservation(SdfReservationError),
    ProducerPreparation(UnadmittedPreparationError),
}

#[derive(Clone)]
pub(super) struct SdfImageSamplingCompletion {
    completed: Arc<AtomicBool>,
    #[cfg(test)]
    observation_held: Arc<AtomicBool>,
}

impl SdfImageSamplingCompletion {
    pub(super) fn observed(&self) -> bool {
        #[cfg(test)]
        if self.observation_held.load(Ordering::Acquire) {
            return false;
        }
        self.completed.load(Ordering::Acquire)
    }
}

#[derive(Default)]
struct SamplingUses {
    cpu: AtomicUsize,
    submitted: Mutex<Vec<SdfImageSamplingCompletion>>,
    uncertified: AtomicBool,
    #[cfg(test)]
    observation_hold: Mutex<Option<Arc<AtomicBool>>>,
}

pub(super) struct SdfImageGenerationCustody {
    producer: Option<CustomRenderPass>,
    binding: ImageBinding,
    device: Arc<wgpu::Device>,
    queue: Arc<wgpu::Queue>,
    uses: Arc<SamplingUses>,
    retirement_requested: bool,
    reservation: Option<SdfTextureReservation>,
}

impl SdfImageGenerationCustody {
    /// The admitted resource owner supplies its producer, current binding and
    /// same baseline GPU resources. This helper grants no producer admission.
    pub(super) fn new(
        producer: CustomRenderPass,
        binding: ImageBinding,
        gpu: &GpuResources,
    ) -> Self {
        Self {
            producer: Some(producer),
            binding,
            device: gpu.device.clone(),
            queue: gpu.queue.clone(),
            uses: Arc::new(SamplingUses::default()),
            retirement_requested: false,
            reservation: None,
        }
    }

    pub(super) fn new_reserved(
        producer: CustomRenderPass,
        binding: ImageBinding,
        gpu: &GpuResources,
        reservation: SdfTextureReservation,
    ) -> Self {
        let mut generation = Self::new(producer, binding, gpu);
        generation.reservation = Some(reservation);
        generation
    }

    pub(super) fn identity(&self) -> ImageBindingIdentity {
        self.binding.identity()
    }

    fn validate(&self) -> Result<(), SdfImageCustodyError> {
        let producer = self
            .producer
            .as_ref()
            .ok_or(SdfImageCustodyError::Retired)?;
        if producer.texture_id() != self.identity().texture_id() {
            return Err(SdfImageCustodyError::IdentityMismatch);
        }
        if self.uses.uncertified.load(Ordering::Acquire) {
            return Err(SdfImageCustodyError::Uncertified);
        }
        self.binding
            .availability()
            .map_err(SdfImageCustodyError::Unavailable)
    }

    pub(super) fn validate_qualification(
        &self,
        gpu: &GpuResources,
    ) -> Result<(), SdfImageCustodyError> {
        if self.retirement_requested {
            return Err(SdfImageCustodyError::Retiring);
        }
        if !Arc::ptr_eq(&self.device, &gpu.device) || !Arc::ptr_eq(&self.queue, &gpu.queue) {
            return Err(SdfImageCustodyError::QueueMismatch);
        }
        self.validate()
    }

    pub(super) fn acquire(&self) -> Result<SdfImageCpuUse, SdfImageCustodyError> {
        self.validate()?;
        if self.retirement_requested {
            return Err(SdfImageCustodyError::Retiring);
        }
        self.uses
            .cpu
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_add(1)
            })
            .map_err(|_| SdfImageCustodyError::CounterOverflow)?;
        Ok(SdfImageCpuUse {
            binding: self.binding.clone(),
            queue: self.queue.clone(),
            uses: self.uses.clone(),
            terminal: CpuTerminal::Queued,
        })
    }

    pub(super) fn poll_nonblocking(&self) -> Result<(), SdfImageCustodyError> {
        self.validate()?;
        if self.device.poll(wgpu::PollType::Poll).is_err() {
            self.uses.uncertified.store(true, Ordering::Release);
            return Err(SdfImageCustodyError::PollFailed);
        }
        let mut submitted = self.uses.submitted.lock().map_err(|_| {
            self.uses.uncertified.store(true, Ordering::Release);
            SdfImageCustodyError::Uncertified
        })?;
        submitted.retain(|completion| !completion.observed());
        Ok(())
    }

    #[cfg(test)]
    pub(super) fn hold_writer_observation_for_test(&self) -> Option<Arc<AtomicBool>> {
        self.producer
            .as_ref()?
            .hold_preparation_observation_for_test()
    }

    #[cfg(test)]
    pub(super) fn hold_sampling_observation_for_test(&self) -> Arc<AtomicBool> {
        // Future submissions get a new hold; already-issued completion witnesses
        // never become pending again when a later sampling use is held.
        let hold = Arc::new(AtomicBool::new(true));
        *self.uses.observation_hold.lock().unwrap() = Some(hold.clone());
        hold
    }

    pub(super) fn retire_if_ready(
        &mut self,
        renderer: &mut WgpuRenderer,
    ) -> Result<bool, SdfImageCustodyError> {
        self.retirement_requested = true;
        if self.producer.is_none() {
            return Ok(true);
        }
        self.poll_nonblocking()?;
        // Reserved Button generations retain the same writer record moved at
        // admission. Sampling/CPU settlement cannot certify producer writes,
        // including an admitted front that was never sampled.
        if self.reservation.is_some()
            && !self
                .producer
                .as_ref()
                .expect("retained producer")
                .unadmitted_preparation_complete()
                .map_err(SdfImageCustodyError::ProducerPreparation)?
        {
            return Ok(false);
        }
        if self.uses.cpu.load(Ordering::Acquire) != 0 {
            return Ok(false);
        }
        let submitted = self.uses.submitted.lock().map_err(|_| {
            self.uses.uncertified.store(true, Ordering::Release);
            SdfImageCustodyError::Uncertified
        })?;
        if !submitted.is_empty() {
            return Ok(false);
        }
        drop(submitted);
        // destroy() unregisters by texture ID, so never apply it to another
        // generation that an external registrar may have installed meanwhile.
        let current = renderer
            .admit_image(self.identity().texture_id())
            .map_err(SdfImageCustodyError::Admission)?;
        if current.identity() != self.identity() {
            return Err(SdfImageCustodyError::IdentityMismatch);
        }
        if let Some(reservation) = self.reservation.as_ref() {
            let certificate = SdfImageRetirementCertificate {
                identity: self.identity(),
            };
            renderer
                .retire_reserved_sdf_generation(reservation, &certificate, &mut self.producer)
                .map_err(SdfImageCustodyError::Reservation)?;
            self.reservation = None;
            return Ok(true);
        }
        self.producer
            .take()
            .expect("validated retained producer")
            .destroy(renderer);
        Ok(true)
    }
}

/// Constructed only after writer, CPU and sampling custody have independently settled.
pub(super) struct SdfImageRetirementCertificate {
    identity: ImageBindingIdentity,
}

impl SdfImageRetirementCertificate {
    pub(super) fn identity(&self) -> ImageBindingIdentity {
        self.identity
    }
}

impl Drop for SdfImageGenerationCustody {
    fn drop(&mut self) {
        if let Some(producer) = self.producer.take() {
            // No registrar is available in Drop. Retain the actual producer,
            // including its fronts, rather than assert or invent safe shutdown.
            std::mem::forget(producer);
            tracing::error!(
                identity = ?self.identity(),
                "SDF generation left without certified unregister; retaining GPU resources"
            );
        }
    }
}

#[derive(Debug)]
pub(super) enum SdfMaterialProducerRecordError {
    Custody(SdfImageCustodyError),
    Publication(super::sdf_material_status::SdfMaterialRejection),
}

/// One immutable K/publication around an already admitted resource generation.
/// This does not allocate texture IDs, prepare a material, request admission or
/// authenticate a bare admission receipt against producer inputs.
pub(super) struct SdfMaterialProducerRecord {
    generation: SdfImageGenerationCustody,
    publication: super::sdf_material_status::SdfMaterialPublication,
}

impl SdfMaterialProducerRecord {
    /// The resource owner must already have consumed genuine completed admission
    /// evidence for this frozen front. Pending registration is not sufficient.
    /// Rejection returns custody intact; no producer is implicitly dropped.
    pub(super) fn from_admitted_generation(
        current: &super::sdf_material_status::SdfMaterialKey,
        prepared_key: super::sdf_material_status::SdfMaterialKey,
        generation: SdfImageGenerationCustody,
    ) -> Result<Self, (SdfMaterialProducerRecordError, SdfImageGenerationCustody)> {
        use super::sdf_material_status::{SdfMaterialPublication, SdfMaterialStatus};

        if generation
            .reservation
            .as_ref()
            .is_some_and(|reservation| reservation.key() != &prepared_key)
        {
            return Err((
                SdfMaterialProducerRecordError::Publication(
                    super::sdf_material_status::SdfMaterialRejection::KeyMismatch,
                ),
                generation,
            ));
        }
        if generation.retirement_requested {
            return Err((
                SdfMaterialProducerRecordError::Custody(SdfImageCustodyError::Retiring),
                generation,
            ));
        }
        if let Err(error) = generation.validate() {
            return Err((SdfMaterialProducerRecordError::Custody(error), generation));
        }
        let publication = match SdfMaterialPublication::new(
            current,
            prepared_key,
            Some(generation.identity()),
            SdfMaterialStatus::Ready {
                binding: generation.binding.clone(),
            },
        ) {
            Ok(publication) => publication,
            Err(error) => {
                return Err((
                    SdfMaterialProducerRecordError::Publication(error),
                    generation,
                ));
            }
        };
        Ok(Self {
            generation,
            publication,
        })
    }

    /// The engine gate must revalidate this immutable snapshot at its cutoff.
    pub(super) fn publication(&self) -> &super::sdf_material_status::SdfMaterialPublication {
        &self.publication
    }

    /// Acquire only for an actual admitted Image scene/draw-plan use.
    pub(super) fn acquire(&self) -> Result<SdfImageCpuUse, SdfImageCustodyError> {
        self.generation.acquire()
    }

    pub(super) fn retire_if_ready(
        &mut self,
        renderer: &mut WgpuRenderer,
    ) -> Result<bool, SdfImageCustodyError> {
        self.generation.retire_if_ready(renderer)
    }
}
enum CpuTerminal {
    Queued,
    Handoff,
    Submitted,
}

#[must_use = "retain with the actual scene/draw plan through submit or terminal cancellation"]
pub(super) struct SdfImageCpuUse {
    binding: ImageBinding,
    queue: Arc<wgpu::Queue>,
    uses: Arc<SamplingUses>,
    terminal: CpuTerminal,
}

impl SdfImageCpuUse {
    pub(super) fn binding(&self) -> &ImageBinding {
        &self.binding
    }

    /// Duplicate an already-held scene reference for its actual output command.
    /// The original remains queued for retained/cache/plan clones; this terminal
    /// alone transfers to GPU custody or cancels with the unsubmitted output.
    pub(super) fn fork_queued_reference(&self) -> Result<Self, SdfImageCustodyError> {
        self.validate_retained_reference()?;
        self.uses
            .cpu
            .try_update(Ordering::AcqRel, Ordering::Acquire, |count| {
                count.checked_add(1)
            })
            .map_err(|_| SdfImageCustodyError::CounterOverflow)?;
        Ok(Self {
            binding: self.binding.clone(),
            queue: self.queue.clone(),
            uses: self.uses.clone(),
            terminal: CpuTerminal::Queued,
        })
    }

    pub(super) fn validate_retained_reference(&self) -> Result<(), SdfImageCustodyError> {
        if !matches!(self.terminal, CpuTerminal::Queued) {
            return Err(SdfImageCustodyError::Uncertified);
        }
        self.validate_submission(&self.queue)
    }

    pub(super) fn validate_submission(
        &self,
        queue: &Arc<wgpu::Queue>,
    ) -> Result<(), SdfImageCustodyError> {
        if self.uses.uncertified.load(Ordering::Acquire) {
            return Err(SdfImageCustodyError::Uncertified);
        }
        if !Arc::ptr_eq(queue, &self.queue) {
            return Err(SdfImageCustodyError::QueueMismatch);
        }
        self.binding
            .availability()
            .map_err(SdfImageCustodyError::Unavailable)
    }

    pub(super) fn begin_submission(&mut self) {
        self.terminal = CpuTerminal::Handoff;
    }

    /// Called by the renderer only after its ordinary sampling submit returns.
    pub(super) fn register_submitted(
        mut self,
        queue: &Arc<wgpu::Queue>,
    ) -> Result<SdfImageSamplingCompletion, SdfImageCustodyError> {
        if !matches!(self.terminal, CpuTerminal::Handoff) {
            self.uses.uncertified.store(true, Ordering::Release);
            return Err(SdfImageCustodyError::Uncertified);
        }
        if !Arc::ptr_eq(queue, &self.queue) {
            return Err(SdfImageCustodyError::QueueMismatch);
        }
        let completion = SdfImageSamplingCompletion {
            completed: Arc::new(AtomicBool::new(false)),
            #[cfg(test)]
            observation_held: self
                .uses
                .observation_hold
                .lock()
                .unwrap()
                .clone()
                .unwrap_or_else(|| Arc::new(AtomicBool::new(false))),
        };
        self.uses
            .submitted
            .lock()
            .map_err(|_| SdfImageCustodyError::Uncertified)?
            .push(completion.clone());
        let callback = completion.completed.clone();
        crate::renderer::on_submitted_work_done(queue, move || {
            callback.store(true, Ordering::Release);
        });
        self.uses.cpu.fetch_sub(1, Ordering::Release);
        self.terminal = CpuTerminal::Submitted;
        Ok(completion)
    }
}

impl Drop for SdfImageCpuUse {
    fn drop(&mut self) {
        match self.terminal {
            CpuTerminal::Queued => {
                self.uses.cpu.fetch_sub(1, Ordering::Release);
            }
            CpuTerminal::Handoff => {
                // Submission may have happened without an installed certificate.
                // Keep CPU custody outstanding and prohibit retirement.
                self.uses.uncertified.store(true, Ordering::Release);
            }
            CpuTerminal::Submitted => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    use crate::compositor::{Compositor, SceneKey};
    use crate::core::math::RectF;
    use crate::core::scene::{Color, SceneBuilder};
    use crate::renderer::custom_render_pass::PhysicalRect;
    use crate::renderer::mesh_cache::MeshCache;
    use crate::renderer::{DrawPlanBuilder, MultiTargetDrawPlan};

    #[test]
    fn sdf_material_producer_record_requires_completed_admission() {
        use crate::core::widget_tree::WidgetTree;
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::renderer::sdf_material_status::{
            SdfMaterialKey, SdfMaterialRejection, SdfMaterialRevision, SdfMaterialStatus,
        };
        use crate::widgets::label::Label;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("resource owner fixture")), None, 0);
        let owner = tree.node(root).unwrap().id;
        let rect = RectF::new(0.0, 0.0, 120.0, 30.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        let metrics = CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(120, 30), 1.0)
            .unwrap()
            .metrics_for(rect, rect)
            .unwrap();
        let revision = SdfMaterialRevision::initial();
        let key = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            owner,
            surface,
            revision,
            metrics,
        )
        .unwrap();
        let wrong_revision = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            owner,
            surface,
            revision.checked_increment().unwrap(),
            metrics,
        )
        .unwrap();

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        // Explicit test fixture ID; no production per-material allocator exists.
        let texture_id = 4_202_610_304;
        let mut producer = CustomRenderPass::new_unpublished(
            texture_id,
            gpu.clone(),
            PhysicalRect::new(0, 0, 120, 30),
        )
        .unwrap();
        let mut frame = producer.begin_frame(Some("staged record source")).unwrap();
        {
            let (encoder, view) = frame.encoder_and_view();
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("initialize staged record source"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        producer
            .submit_and_swap_deferred_registration(frame)
            .unwrap();
        producer.queue_front_registration().unwrap();
        let request = gpu.image_admission.request(texture_id);
        assert_eq!(request.try_resolve().unwrap(), None);
        assert!(matches!(
            renderer.admit_image(texture_id),
            Err(ImageAdmissionError::MissingRegistration(id)) if id == texture_id
        ));

        // Real registrar drain and mailbox receipt, not register_front_now.
        renderer.drain_pending_image_registrations();
        let binding = request
            .try_resolve()
            .unwrap()
            .expect("completed real admission");
        assert_eq!(binding.availability(), Ok(()));
        let identity = binding.identity();
        let generation = SdfImageGenerationCustody::new(producer, binding.clone(), &gpu);
        let generation = match SdfMaterialProducerRecord::from_admitted_generation(
            &key,
            wrong_revision,
            generation,
        ) {
            Err((
                SdfMaterialProducerRecordError::Publication(SdfMaterialRejection::KeyMismatch),
                generation,
            )) => generation,
            Err(_) => panic!("unexpected record rejection"),
            Ok(_) => panic!("obsolete preparation revision admitted"),
        };
        assert_eq!(binding.availability(), Ok(()));
        let mut record = match SdfMaterialProducerRecord::from_admitted_generation(
            &key,
            key.clone(),
            generation,
        ) {
            Ok(record) => record,
            Err(_) => panic!("current admitted generation rejected"),
        };
        assert_eq!(record.publication().key(), &key);
        assert!(matches!(record.publication().status(),
            SdfMaterialStatus::Ready { binding } if binding.identity() == identity));

        let usage = record.acquire().unwrap();
        let mut scene = SceneBuilder::new();
        scene.push_admitted_image(
            rect,
            usage.binding(),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            0,
            None,
        );
        let scene = scene.build();
        assert!(!record.retire_if_ready(&mut renderer).unwrap());
        assert_eq!(binding.availability(), Ok(()));
        drop(scene);
        drop(usage); // Terminal no-submit cancellation of this actual scene use.
        assert!(record.retire_if_ready(&mut renderer).unwrap());
        assert_eq!(
            binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
    }

    #[test]
    fn sdf_image_generation_custody_uses_renderer_submission() {
        fn generation(
            renderer: &mut WgpuRenderer,
            gpu: &GpuResources,
            texture_id: u64,
        ) -> SdfImageGenerationCustody {
            let mut producer =
                CustomRenderPass::new(texture_id, gpu.clone(), PhysicalRect::new(0, 0, 1, 1))
                    .unwrap();
            let mut frame = producer.begin_frame(Some("custody source")).unwrap();
            {
                let (encoder, view) = frame.encoder_and_view();
                let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("initialize custody source"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        depth_slice: None,
                        view,
                        resolve_target: None,
                        ops: wgpu::Operations {
                            load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                            store: wgpu::StoreOp::Store,
                        },
                    })],
                    depth_stencil_attachment: None,
                    multiview_mask: None,
                    occlusion_query_set: None,
                    timestamp_writes: None,
                });
            }
            producer.submit_and_swap(frame).unwrap();
            producer.register_front_now(renderer).unwrap();
            let binding = renderer.admit_image(texture_id).unwrap();
            SdfImageGenerationCustody::new(producer, binding, gpu)
        }

        struct SamplingFrame {
            compiled: MultiTargetDrawPlan,
            cpu: Vec<SdfImageCpuUse>,
        }

        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).expect("headless GPU");
        let gpu = renderer.gpu_resources();
        let mut first = generation(&mut renderer, &gpu, 4_202_610_300);
        let mut second = generation(&mut renderer, &gpu, 4_202_610_301);
        let first_binding = first.binding.clone();
        let second_binding = second.binding.clone();
        assert_ne!(first.identity(), second.identity());
        let cpu = first.acquire().unwrap();
        let another_cpu = first.acquire().unwrap();
        let cancelled = second.acquire().unwrap();
        let rect = RectF::new(0.0, 0.0, 16.0, 16.0);
        let mut cancelled_scene = SceneBuilder::new();
        cancelled_scene.push_admitted_image(
            rect,
            cancelled.binding(),
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            0,
            None,
        );
        let cancelled_scene = cancelled_scene.build();
        assert!(!second.retire_if_ready(&mut renderer).unwrap());
        assert!(matches!(
            second.acquire(),
            Err(SdfImageCustodyError::Retiring)
        ));
        drop(cancelled_scene);
        drop(cancelled);
        assert!(second.retire_if_ready(&mut renderer).unwrap());
        assert_eq!(second.uses.cpu.load(Ordering::Acquire), 0);
        assert!(second.uses.submitted.lock().unwrap().is_empty());
        assert_eq!(
            second_binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(first_binding.availability(), Ok(()));

        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (16, 16), 1.0)
            .unwrap();
        let mut scene = SceneBuilder::new();
        for (member, usage) in [&cpu, &another_cpu].into_iter().enumerate() {
            scene.push_admitted_image(
                RectF::new(member as f32 * 8.0, 0.0, 8.0, 16.0),
                usage.binding(),
                RectF::new(0.0, 0.0, 1.0, 1.0),
                Color::WHITE,
                0,
                None,
            );
        }
        let scenes = HashMap::from([(surface, scene.build())]);
        let plan = DrawPlanBuilder::new()
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let compiled = renderer
            .compile_multi(&plan)
            .unwrap()
            .expect_ready("native Compile must be ready");
        drop(plan);
        drop(scenes);
        let frame = SamplingFrame {
            compiled,
            cpu: vec![cpu, another_cpu],
        };
        assert!(!first.retire_if_ready(&mut renderer).unwrap());
        assert_eq!(first.uses.cpu.load(Ordering::Acquire), 2);
        assert_eq!(first_binding.availability(), Ok(()));
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("custody sampling target"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let SamplingFrame { compiled, cpu } = frame;
        let command = renderer
            .encode_compiled_target_for_test(
                &compiled,
                surface,
                &output.create_view(&Default::default()),
            )
            .expect("real admitted Image encodes");
        drop(compiled);
        let completions = renderer
            .submit_frame_command_with_sdf_custody(command, cpu)
            .unwrap();
        assert_eq!(first.uses.cpu.load(Ordering::Acquire), 0);
        assert_eq!(first.uses.submitted.lock().unwrap().len(), 2);
        assert_eq!(completions.len(), 2);
        assert_eq!(first_binding.availability(), Ok(()));
        let deadline = Instant::now() + Duration::from_secs(10);
        while !first.retire_if_ready(&mut renderer).unwrap() {
            assert!(first.producer.is_some());
            assert_eq!(first_binding.availability(), Ok(()));
            assert!(Instant::now() < deadline, "sampling completion timed out");
            std::thread::yield_now();
        }
        assert!(completions.iter().all(SdfImageSamplingCompletion::observed));
        assert!(first.producer.is_none());
        assert_eq!(
            first_binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert!(second.producer.is_none());
    }
}

#[cfg(test)]
mod active_replacement_writer_tests {
    use super::*;
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::math::RectF;
    use crate::core::scene::{Color, SceneBuilder};
    use crate::core::widget_tree::WidgetTree;
    use crate::renderer::DrawPlanBuilder;
    use crate::renderer::custom_render_pass::PhysicalRect;
    use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
    use crate::renderer::mesh_cache::MeshCache;
    use crate::renderer::sdf_frame_custody::{SdfFramePlan, SdfImageReference, SdfSceneCustody};
    use crate::renderer::sdf_material_status::{SdfMaterialKey, SdfMaterialRevision};
    use crate::widgets::label::Label;
    use std::collections::HashMap;
    use std::time::{Duration, Instant};

    #[test]
    fn sdf_replacement_writer_cpu_and_late_sampling_retire_independently() {
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let gpu = renderer.gpu_resources();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("custody owner")), None, 0);
        let rect = RectF::new(0.0, 0.0, 16.0, 16.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (16, 16), 1.0)
            .unwrap();
        let metrics = CustomRenderTargetSnapshot::new(
            surface,
            crate::renderer::pixel_coverage::PhysicalExtent::new(16, 16),
            1.0,
        )
        .unwrap()
        .metrics_for(rect, rect)
        .unwrap();
        let key = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            tree.node(root).unwrap().id,
            surface,
            SdfMaterialRevision::initial(),
            metrics,
        )
        .unwrap();
        let reservation = renderer.reserve_sdf_texture(key.clone()).unwrap();
        let mut producer = Some(
            CustomRenderPass::new_unpublished(
                reservation.texture_id(),
                gpu.clone(),
                PhysicalRect::new(0, 0, 16, 16),
            )
            .unwrap(),
        );
        let mut writer = producer
            .as_ref()
            .unwrap()
            .begin_frame(Some("actual replacement write"))
            .unwrap();
        {
            let (encoder, view) = writer.encoder_and_view();
            let _pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("actual replacement write"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    depth_slice: None,
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::WHITE),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                multiview_mask: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
        }
        producer
            .as_mut()
            .unwrap()
            .submit_and_swap_deferred_registration(writer)
            .unwrap();
        let writer_hold = producer
            .as_ref()
            .unwrap()
            .hold_preparation_observation_for_test()
            .unwrap();
        let writer_completion = producer
            .as_ref()
            .unwrap()
            .unadmitted_completion_for_test()
            .unwrap();
        let admission = renderer
            .register_reserved_sdf_replacement_front(&reservation, &key, &key, &mut producer)
            .unwrap()
            .expect("ordered submitted writer may genuinely admit without completion observation");
        assert!(
            producer.is_none(),
            "the original producer and writer record transfer together"
        );
        let binding = admission.binding().clone();
        let unregistrations =
            crate::renderer::custom_render_pass::audit::snapshot().custom_unregistrations;
        let mut record = match admission.into_material_record(&key) {
            Ok(record) => record,
            Err(_) => panic!("actual current admitted generation rejected"),
        };
        let first_cpu = record.acquire().unwrap();
        let late_cpu = record.acquire().unwrap();
        let mut scene = SceneBuilder::new();
        scene.push_admitted_image(
            rect,
            &binding,
            RectF::new(0.0, 0.0, 1.0, 1.0),
            Color::WHITE,
            0,
            None,
        );
        let scenes = HashMap::from([(surface, scene.build())]);
        let draw = DrawPlanBuilder::new()
            .build_multi(
                &scenes,
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                |_| 1.0,
                0,
            )
            .expect("scene must satisfy DrawPlan Build admission");
        let mut first_custody = SdfSceneCustody::default();
        first_custody.retain_reference(SdfImageReference::new(key.clone(), first_cpu));
        let mut late_custody = SdfSceneCustody::default();
        late_custody.retain_reference(SdfImageReference::new(key.clone(), late_cpu));
        let first_plan = SdfFramePlan::new(draw.clone(), first_custody);
        let late_plan = SdfFramePlan::new(draw, late_custody);
        let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("real replacement sampling output"),
            size: wgpu::Extent3d {
                width: 16,
                height: 16,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let first_sampling_hold = record.generation.hold_sampling_observation_for_test();
        let first = renderer
            .compile_sdf_frame(first_plan, None)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let first_done = first
            .submit_headless_for_test(&renderer, surface, &output.create_view(&Default::default()))
            .unwrap();
        let first_witness = record.generation.uses.submitted.lock().unwrap()[0].clone();
        assert!(!first_done());
        assert!(!record.retire_if_ready(&mut renderer).unwrap());
        assert_eq!(binding.availability(), Ok(()));
        writer_hold.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !record
            .generation
            .producer
            .as_ref()
            .unwrap()
            .unadmitted_preparation_complete()
            .unwrap()
        {
            assert!(
                Instant::now() < deadline,
                "actual writer completion was not observed"
            );
            std::thread::yield_now();
        }
        assert!(writer_completion.load(Ordering::Acquire));
        assert!(
            !record.retire_if_ready(&mut renderer).unwrap(),
            "writer completion cannot release CPU or held sampling custody"
        );
        first_sampling_hold.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !first_witness.observed() {
            assert!(
                Instant::now() < deadline,
                "actual first sampling completion was not observed"
            );
            record.generation.poll_nonblocking().unwrap();
            std::thread::yield_now();
        }
        assert!(
            !record.retire_if_ready(&mut renderer).unwrap(),
            "an actual CPU plan still able to submit independently prevents retirement"
        );
        let late_sampling_hold = record.generation.hold_sampling_observation_for_test();
        assert!(
            first_witness.observed(),
            "a later hold cannot replay or reset an old completion"
        );
        let late = renderer
            .compile_sdf_frame(late_plan, None)
            .unwrap()
            .expect_ready("native Compile must be ready");
        let late_done = late
            .submit_headless_for_test(&renderer, surface, &output.create_view(&Default::default()))
            .unwrap();
        assert!(!late_done());
        assert!(
            !record.retire_if_ready(&mut renderer).unwrap(),
            "earlier writer and sampling witnesses cannot settle a later sampling submission"
        );
        assert_eq!(binding.availability(), Ok(()));
        late_sampling_hold.store(false, Ordering::Release);
        let deadline = Instant::now() + Duration::from_secs(10);
        while !record.retire_if_ready(&mut renderer).unwrap() {
            assert!(
                Instant::now() < deadline,
                "actual late sampling completion was not observed"
            );
            std::thread::yield_now();
        }
        assert!(late_done());
        assert!(first_witness.observed());
        assert_eq!(
            binding.availability(),
            Err(ImageBindingFailure::Unregistered)
        );
        assert_eq!(
            crate::renderer::custom_render_pass::audit::snapshot().custom_unregistrations,
            unregistrations + 1
        );
    }
}
