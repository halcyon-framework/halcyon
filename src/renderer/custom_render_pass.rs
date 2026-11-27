use crate::renderer::wgpu_renderer::{
    GpuResources, HdrFinalCustody, PendingImageRegistration, PendingImageUnregistration,
    WgpuRenderer,
};
use std::num::NonZeroU64;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

pub use crate::renderer::pixel_coverage::PhysicalRect;

#[cfg(test)]
pub(crate) mod audit {
    use std::cell::{Cell, RefCell};

    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub(crate) enum CustomRenderAuditEvent {
        Submission,
        Registration,
        Unregistration,
    }

    #[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
    pub(crate) struct CustomRenderAuditSnapshot {
        pub custom_render_passes: u64,
        pub custom_textures: u64,
        pub custom_command_encoders: u64,
        pub custom_submissions: u64,
        pub custom_registrations: u64,
        pub custom_unregistrations: u64,
        pub device_lifetime_observations: u64,
        pub device_lost_callbacks: u64,
        pub plot_renderers: u64,
        pub worker_pools: u64,
        pub workers: u64,
    }

    impl CustomRenderAuditSnapshot {
        const ZERO: Self = Self {
            custom_render_passes: 0,
            custom_textures: 0,
            custom_command_encoders: 0,
            custom_submissions: 0,
            custom_registrations: 0,
            custom_unregistrations: 0,
            device_lifetime_observations: 0,
            device_lost_callbacks: 0,
            plot_renderers: 0,
            worker_pools: 0,
            workers: 0,
        };
    }

    thread_local! {
        static SNAPSHOT: Cell<CustomRenderAuditSnapshot> = const {
            Cell::new(CustomRenderAuditSnapshot::ZERO)
        };
        static EVENTS: RefCell<Vec<CustomRenderAuditEvent>> = const { RefCell::new(Vec::new()) };
    }

    fn update(f: impl FnOnce(&mut CustomRenderAuditSnapshot)) {
        SNAPSHOT.with(|snapshot| {
            let mut next = snapshot.get();
            f(&mut next);
            snapshot.set(next);
        });
    }

    pub(crate) fn snapshot() -> CustomRenderAuditSnapshot {
        SNAPSHOT.get()
    }

    pub(crate) fn events() -> Vec<CustomRenderAuditEvent> {
        EVENTS.with(|events| events.borrow().clone())
    }

    pub(crate) fn record_custom_render_pass() {
        update(|snapshot| snapshot.custom_render_passes += 1);
    }

    pub(crate) fn record_custom_texture() {
        update(|snapshot| snapshot.custom_textures += 1);
    }

    pub(crate) fn record_command_encoder() {
        update(|snapshot| snapshot.custom_command_encoders += 1);
    }

    pub(crate) fn record_submission() {
        update(|snapshot| snapshot.custom_submissions += 1);
        EVENTS.with(|events| events.borrow_mut().push(CustomRenderAuditEvent::Submission));
    }

    pub(crate) fn record_registration() {
        update(|snapshot| snapshot.custom_registrations += 1);
        EVENTS.with(|events| {
            events
                .borrow_mut()
                .push(CustomRenderAuditEvent::Registration)
        });
    }

    pub(crate) fn record_unregistration() {
        update(|snapshot| snapshot.custom_unregistrations += 1);
        EVENTS.with(|events| {
            events
                .borrow_mut()
                .push(CustomRenderAuditEvent::Unregistration)
        });
    }

    pub(crate) fn record_device_lifetime_observation() {
        update(|snapshot| snapshot.device_lifetime_observations += 1);
    }

    pub(crate) fn record_device_lost_callback() {
        update(|snapshot| snapshot.device_lost_callbacks += 1);
    }

    pub(crate) fn record_plot_renderer() {
        update(|snapshot| snapshot.plot_renderers += 1);
    }

    pub(crate) fn record_worker_pool(workers: usize) {
        update(|snapshot| {
            snapshot.worker_pools += 1;
            snapshot.workers += workers as u64;
        });
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomRenderPassError {
    ZeroSizedTexture { width: u32, height: u32 },
    Destroyed,
    ContentPinned,
}

impl std::fmt::Display for CustomRenderPassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroSizedTexture { width, height } => write!(
                f,
                "custom render pass texture size must be non-zero, got {}x{}",
                width, height
            ),
            Self::Destroyed => write!(f, "custom render pass has been destroyed"),
            Self::ContentPinned => write!(f, "custom render back texture has a live content lease"),
        }
    }
}

impl std::error::Error for CustomRenderPassError {}

pub type CustomRenderPassResult<T> = Result<T, CustomRenderPassError>;

static NEXT_CUSTOM_RENDER_PRESENTATION_ID: AtomicU64 = AtomicU64::new(1);

/// Opaque identity used to resolve one admitted producer's published front for
/// the renderer-owned target currently being materialized.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CustomRenderPresentationId(NonZeroU64);

/// Participant-local publication returned from the authorized preparation
/// callback. The engine supplies the target identity separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CustomRenderPresentationBinding {
    presentation_id: CustomRenderPresentationId,
    texture_id: u64,
    canonical_over_neutral: bool,
}

impl CustomRenderPresentationBinding {
    pub const fn presentation_id(self) -> CustomRenderPresentationId {
        self.presentation_id
    }

    pub const fn texture_id(self) -> u64 {
        self.texture_id
    }

    pub(crate) const fn canonical_over_neutral(self) -> bool {
        self.canonical_over_neutral
    }
}

/// Opaque SPEC-HAL-295A marker for a WidgetTree participant's explicit public
/// opt-in to the custom-render lifecycle. Implementing
/// `CustomRenderParticipant` alone does not admit a node; the widget must store
/// a marker created through this custom-render path.
#[derive(Debug)]
#[must_use = "store this marker on the admitted custom-render widget"]
pub struct CustomRenderAdmission {
    presentation_id: CustomRenderPresentationId,
}

impl CustomRenderAdmission {
    /// Explicitly admit one widget to the SPEC-HAL-295 custom-render path.
    ///
    /// The `GpuResources` argument places the opt-in at Halcyon's public
    /// custom-render resource boundary. This marker stores no resource reference
    /// or identity, grants no GPU access, and does not authenticate a Device or
    /// Queue. The widget's actual resources and publication lifecycle must
    /// independently satisfy SPEC-HAL-295A. Target metrics remain renderer-owned
    /// and are supplied later by SPEC-HAL-266A preparation.
    pub fn from_gpu_resources(_gpu: &GpuResources) -> Self {
        Self::new()
    }

    pub const fn presentation_id(&self) -> CustomRenderPresentationId {
        self.presentation_id
    }

    pub const fn presentation_binding(&self, texture_id: u64) -> CustomRenderPresentationBinding {
        CustomRenderPresentationBinding {
            presentation_id: self.presentation_id,
            texture_id,
            canonical_over_neutral: false,
        }
    }

    pub(crate) fn loading_presence_binding(
        &self,
        texture_id: u64,
        terminal: bool,
    ) -> CustomRenderPresentationBinding {
        CustomRenderPresentationBinding {
            presentation_id: self.presentation_id,
            texture_id,
            canonical_over_neutral: terminal,
        }
    }

    #[cfg(test)]
    pub(crate) fn for_test() -> Self {
        Self::new()
    }

    fn new() -> Self {
        let value = match NEXT_CUSTOM_RENDER_PRESENTATION_ID.try_update(
            Ordering::Relaxed,
            Ordering::Relaxed,
            |current| current.checked_add(1),
        ) {
            Ok(value) => value,
            Err(_) => panic!("custom-render presentation identity space exhausted"),
        };
        let Some(value) = NonZeroU64::new(value) else {
            unreachable!("custom-render presentation identities start at one")
        };
        Self {
            presentation_id: CustomRenderPresentationId(value),
        }
    }
}

/// Low-level custom Image lifecycle. Every front passed to registration or
/// publication must yield premultiplied-linear RGBA when sampled; filtering,
/// swapping, registration, and resize perform no alpha conversion.
pub trait CustomRenderLifecycle {
    fn texture_id(&self) -> u64;
    fn physical_rect(&self) -> PhysicalRect;
    fn queue_front_registration(&self) -> CustomRenderPassResult<()>;
    fn register_front_now(&self, renderer: &mut WgpuRenderer) -> CustomRenderPassResult<()>;
    fn begin_frame(&self, label: Option<&str>) -> CustomRenderPassResult<CustomRenderFrame>;
    fn submit_and_swap(&mut self, frame: CustomRenderFrame) -> CustomRenderPassResult<()>;
    fn resize(&mut self, rect: PhysicalRect) -> CustomRenderPassResult<()>;
    fn resize_registered(
        &mut self,
        renderer: &mut WgpuRenderer,
        rect: PhysicalRect,
    ) -> CustomRenderPassResult<()>;
    fn destroy(self, renderer: &mut WgpuRenderer)
    where
        Self: Sized;
    fn destroy_deferred(self) -> CustomRenderPassResult<()>
    where
        Self: Sized;
}

#[must_use = "call destroy(renderer) before dropping a CustomRenderPass"]
pub struct CustomRenderPass {
    texture_id: u64,
    gpu: GpuResources,
    rect: PhysicalRect,
    nearest: bool,
    final_format: wgpu::TextureFormat,
    hdr_custody: Option<Arc<HdrFinalCustody>>,
    front_texture: Option<wgpu::Texture>,
    back_texture: Option<wgpu::Texture>,
    // Lazy HAL381 content guards. Ordinary custom/3D callers allocate none.
    front_content: Option<Arc<HdrContentView>>,
    back_content: Option<Arc<HdrContentView>>,
    swap_count: u64,
    destroyed: bool,
    unadmitted_preparation: Option<Arc<UnadmittedPreparation>>,
}

struct HdrContentView {
    view: wgpu::TextureView,
}

/// Real retained bytes plus write exclusion; registration evidence alone owns
/// neither. Kept private to the existing offscreen resource owner.
pub(crate) struct HdrFrontReadLease {
    content: Arc<HdrContentView>,
    texture: wgpu::Texture,
    custody: Arc<HdrFinalCustody>,
    queue: std::sync::Weak<wgpu::Queue>,
    wake: crate::renderer::image_binding::ImageAdmissionMailbox,
}

impl Drop for HdrFrontReadLease {
    fn drop(&mut self) {
        let retained = (
            self.content.clone(),
            self.texture.clone(),
            self.custody.clone(),
        );
        let wake = self.wake.clone();
        // The last CPU reader ends only after its submitted reads are ordered.
        // Retain the write exclusion through actual same-Queue completion.
        if let Some(queue) = self.queue.upgrade() {
            crate::renderer::on_submitted_work_done(&queue, move || {
                drop(retained);
                wake.notify();
            });
        } else {
            // No live renderer/Context Queue owner remains. Keeping an Arc to
            // that Queue inside its own retirement callback would leak both.
            drop(retained);
            wake.notify();
        }
    }
}

pub(crate) struct HdrFrontPin {
    lease: Arc<HdrFrontReadLease>,
    registration: crate::renderer::image_binding::ImageBindingRequest,
    swap_count: u64,
    _ui_owner: std::marker::PhantomData<std::rc::Rc<()>>,
}

impl HdrFrontPin {
    pub(crate) fn registered(
        &self,
    ) -> Result<
        Option<crate::renderer::image_binding::ImageBinding>,
        crate::renderer::image_binding::ImageAdmissionError,
    > {
        let binding = self.registration.try_resolve()?;
        if let Some(binding) = &binding {
            binding
                .availability()
                .map_err(crate::renderer::image_binding::ImageAdmissionError::Unavailable)?;
        }
        Ok(binding)
    }

    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.lease.content.view
    }

    pub(crate) fn read_lease(&self) -> Arc<HdrFrontReadLease> {
        self.lease.clone()
    }
}

struct UnadmittedPreparation {
    queue: Arc<wgpu::Queue>,
    last_submission: Mutex<Option<Arc<AtomicBool>>>,
    uncertified: AtomicBool,
    #[cfg(test)]
    observation_held: Arc<AtomicBool>,
}

#[derive(Debug)]
pub(super) enum UnadmittedPreparationError {
    NotTracked,
    Uncertified,
    PollFailed,
}

struct UnadmittedSubmission {
    work: [Option<Arc<UnadmittedPreparation>>; 2],
    registered: bool,
}

impl UnadmittedSubmission {
    fn register(&mut self, queue: &Arc<wgpu::Queue>) {
        for (index, work) in self.work.iter().enumerate() {
            let Some(work) = work else { continue };
            if index == 1
                && self.work[0]
                    .as_ref()
                    .is_some_and(|first| Arc::ptr_eq(first, work))
            {
                continue;
            }
            if !Arc::ptr_eq(&work.queue, queue) {
                work.uncertified.store(true, Ordering::Release);
            }
            let completion = Arc::new(AtomicBool::new(false));
            let mut last = work.last_submission.lock().unwrap_or_else(|error| {
                work.uncertified.store(true, Ordering::Release);
                error.into_inner()
            });
            *last = Some(completion.clone());
            // Serialize record/callback registration, so the last record covers
            // every already-enqueued submission even with concurrent producers.
            crate::renderer::on_submitted_work_done(queue, move || {
                completion.store(true, Ordering::Release);
            });
        }
        self.registered = true;
    }
}

impl Drop for UnadmittedSubmission {
    fn drop(&mut self) {
        if !self.registered {
            for work in self.work.iter().flatten() {
                work.uncertified.store(true, Ordering::Release);
            }
        }
    }
}

impl CustomRenderPass {
    /// Creates a nearest-filtered low-level Image producer. Every rendered front
    /// must yield premultiplied-linear RGBA when sampled.
    pub fn new(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
    ) -> CustomRenderPassResult<Self> {
        Self::new_with_filter(texture_id, gpu, rect, true)
    }

    pub(crate) fn new_unpublished(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
    ) -> CustomRenderPassResult<Self> {
        Self::new_with_filter_and_publication(texture_id, gpu, rect, true, false)
    }

    pub(crate) fn new_unpublished_with_filter(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
        nearest: bool,
    ) -> CustomRenderPassResult<Self> {
        Self::new_with_filter_and_publication(texture_id, gpu, rect, nearest, false)
    }

    /// Creates a low-level Image producer. `nearest` changes filtering only;
    /// every rendered front must yield premultiplied-linear RGBA when sampled.
    pub fn new_with_filter(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
        nearest: bool,
    ) -> CustomRenderPassResult<Self> {
        Self::new_with_filter_and_publication(texture_id, gpu, rect, nearest, true)
    }

    fn new_with_filter_and_publication(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
        nearest: bool,
        publish_initial_front: bool,
    ) -> CustomRenderPassResult<Self> {
        let format = gpu.format;
        Self::new_with_format(
            texture_id,
            gpu,
            rect,
            nearest,
            publish_initial_front,
            format,
        )
    }

    pub(crate) fn new_private_hdr_final(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
        charge: Box<crate::renderer::gpu_custody::GpuCharge>,
    ) -> Result<Self, &'static str> {
        if !gpu.private_hdr_owner_live() {
            return Err("renderer retired");
        }
        let mut pass = Self::new_with_format(
            texture_id,
            gpu,
            rect,
            false,
            false,
            wgpu::TextureFormat::Rgba16Float,
        )
        .map_err(|_| "invalid private HDR final extent")?;
        let textures = [
            pass.front_texture.as_ref().unwrap().clone(),
            pass.back_texture.as_ref().unwrap().clone(),
        ];
        match pass
            .gpu
            .reserve_hdr_final_custody(texture_id, charge, textures)
        {
            Ok(custody) => {
                pass.hdr_custody = Some(custody);
                Ok(pass)
            }
            Err(error) => {
                pass.release_textures();
                Err(error)
            }
        }
    }

    fn new_with_format(
        texture_id: u64,
        gpu: GpuResources,
        rect: PhysicalRect,
        nearest: bool,
        publish_initial_front: bool,
        format: wgpu::TextureFormat,
    ) -> CustomRenderPassResult<Self> {
        Self::validate_rect(rect)?;
        if !gpu.renderer_device_matches() {
            return Err(CustomRenderPassError::Destroyed);
        }

        let front_texture = Some(Self::create_offscreen_texture(
            &gpu,
            rect,
            format,
            "CustomRenderPass Front Texture",
        ));
        let back_texture = Some(Self::create_offscreen_texture(
            &gpu,
            rect,
            format,
            "CustomRenderPass Back Texture",
        ));

        let unadmitted_preparation = (!publish_initial_front).then(|| {
            Arc::new(UnadmittedPreparation {
                queue: gpu.queue.clone(),
                last_submission: Mutex::new(None),
                uncertified: AtomicBool::new(false),
                #[cfg(test)]
                observation_held: Arc::new(AtomicBool::new(false)),
            })
        });
        let pass = Self {
            texture_id,
            gpu,
            rect,
            nearest,
            final_format: format,
            hdr_custody: None,
            front_texture,
            back_texture,
            front_content: None,
            back_content: None,
            swap_count: 0,
            destroyed: false,
            unadmitted_preparation,
        };
        #[cfg(test)]
        audit::record_custom_render_pass();
        if publish_initial_front {
            pass.queue_front_registration()?;
        }
        Ok(pass)
    }

    pub fn texture_id(&self) -> u64 {
        self.texture_id
    }

    pub fn physical_rect(&self) -> PhysicalRect {
        self.rect
    }

    pub fn physical_size(&self) -> (u32, u32) {
        self.rect.size()
    }

    pub fn texture_format(&self) -> wgpu::TextureFormat {
        self.final_format
    }

    #[cfg(any(test, feature = "test-harness"))]
    pub(crate) fn front_texture_for_test(&self) -> CustomRenderPassResult<&wgpu::Texture> {
        self.front_texture
            .as_ref()
            .ok_or(CustomRenderPassError::Destroyed)
    }

    pub fn swap_count(&self) -> u64 {
        self.swap_count
    }

    /// Queues the current premultiplied-linear front without converting content.
    pub fn queue_front_registration(&self) -> CustomRenderPassResult<()> {
        let registration = self.front_registration()?;
        let mut pending = self
            .gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        pending.push(registration);
        #[cfg(test)]
        audit::record_registration();
        Ok(())
    }

    pub fn queue_front_unregistration(&self) -> CustomRenderPassResult<()> {
        self.queue_unregistration(Vec::new())
    }

    pub(crate) fn request_plot_front_admission(
        &self,
        current: Arc<AtomicBool>,
    ) -> CustomRenderPassResult<super::image_binding::ImageBindingRequest> {
        let registration = self.front_registration()?;
        #[cfg(test)]
        audit::record_registration();
        Ok(self
            .gpu
            .image_admission
            .request_plot_front(registration, current))
    }

    fn queue_unregistration(
        &self,
        retained_textures: Vec<wgpu::Texture>,
    ) -> CustomRenderPassResult<()> {
        if self.destroyed {
            return Err(CustomRenderPassError::Destroyed);
        }
        self.discard_pending_registrations();
        let mut pending = self
            .gpu
            .pending_unregistrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if let Some(custody) = &self.hdr_custody {
            self.gpu
                .queue_hdr_final_removal(&mut pending, custody, retained_textures);
        } else {
            pending.push(PendingImageUnregistration {
                texture_id: self.texture_id,
                retained_textures,
            });
        }
        #[cfg(test)]
        audit::record_unregistration();
        Ok(())
    }

    /// Registers the current premultiplied-linear front without converting content.
    pub fn register_front_now(&self, renderer: &mut WgpuRenderer) -> CustomRenderPassResult<()> {
        self.discard_pending_registrations();
        let registration = self.front_registration()?;
        if registration.nearest {
            renderer.register_image_from_view_nearest(registration.texture_id, registration.view);
        } else {
            renderer.register_image_from_view(registration.texture_id, registration.view);
        }
        #[cfg(test)]
        audit::record_registration();
        Ok(())
    }

    pub fn begin_frame(&self, label: Option<&str>) -> CustomRenderPassResult<CustomRenderFrame> {
        self.ensure_live()?;
        if self
            .back_content
            .as_ref()
            .is_some_and(|content| Arc::strong_count(content) != 1)
        {
            return Err(CustomRenderPassError::ContentPinned);
        }
        let back_texture = self
            .back_texture
            .as_ref()
            .ok_or(CustomRenderPassError::Destroyed)?;
        let back_view = back_texture.create_view(&wgpu::TextureViewDescriptor::default());
        let encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label });
        #[cfg(test)]
        audit::record_command_encoder();

        Ok(CustomRenderFrame {
            encoder: Some(encoder),
            back_view,
            unadmitted_preparation: self.unadmitted_preparation.clone(),
        })
    }

    /// Publishes a frame whose rendered content is already premultiplied-linear.
    pub fn submit_and_swap(&mut self, frame: CustomRenderFrame) -> CustomRenderPassResult<()> {
        self.submit_and_swap_deferred_registration(frame)?;
        self.queue_front_registration()
    }

    pub(crate) fn submit_and_swap_deferred_registration(
        &mut self,
        frame: CustomRenderFrame,
    ) -> CustomRenderPassResult<()> {
        self.ensure_live()?;
        let mut preparation = UnadmittedSubmission {
            work: [
                self.unadmitted_preparation.clone(),
                frame.unadmitted_preparation.clone(),
            ],
            registered: false,
        };
        self.gpu.queue.submit(std::iter::once(frame.finish()));
        preparation.register(&self.gpu.queue);
        #[cfg(test)]
        audit::record_submission();
        std::mem::swap(&mut self.front_texture, &mut self.back_texture);
        std::mem::swap(&mut self.front_content, &mut self.back_content);
        self.swap_count += 1;
        Ok(())
    }

    pub fn resize(&mut self, rect: PhysicalRect) -> CustomRenderPassResult<()> {
        self.ensure_live()?;
        Self::validate_rect(rect)?;
        let retained_textures = self.take_textures();
        self.queue_unregistration(retained_textures)?;
        self.recreate_textures(rect);
        self.queue_front_registration()
    }

    pub(crate) fn resize_unpublished(&mut self, rect: PhysicalRect) -> CustomRenderPassResult<()> {
        self.ensure_live()?;
        Self::validate_rect(rect)?;
        let retained_textures = self.take_textures();
        self.queue_unregistration(retained_textures)?;
        self.recreate_textures(rect);
        Ok(())
    }

    pub fn resize_registered(
        &mut self,
        renderer: &mut WgpuRenderer,
        rect: PhysicalRect,
    ) -> CustomRenderPassResult<()> {
        self.ensure_live()?;
        Self::validate_rect(rect)?;
        self.discard_pending_registrations();
        renderer.unregister_image(self.texture_id);
        self.recreate_textures(rect);
        self.register_front_now(renderer)
    }

    pub fn destroy(mut self, renderer: &mut WgpuRenderer) {
        self.discard_pending_registrations();
        renderer.unregister_image(self.texture_id);
        #[cfg(test)]
        audit::record_unregistration();
        self.release_textures();
    }

    pub fn destroy_deferred(mut self) -> CustomRenderPassResult<()> {
        if self.destroyed {
            return Err(CustomRenderPassError::Destroyed);
        }
        let retained_textures = self.take_textures();
        self.queue_unregistration(retained_textures)?;
        self.destroyed = true;
        Ok(())
    }

    fn ensure_live(&self) -> CustomRenderPassResult<()> {
        if self.destroyed
            || self
                .hdr_custody
                .as_ref()
                .is_some_and(|custody| !self.gpu.hdr_final_is_live(custody))
        {
            Err(CustomRenderPassError::Destroyed)
        } else {
            Ok(())
        }
    }

    fn validate_rect(rect: PhysicalRect) -> CustomRenderPassResult<()> {
        if rect.is_empty() {
            Err(CustomRenderPassError::ZeroSizedTexture {
                width: rect.width,
                height: rect.height,
            })
        } else {
            Ok(())
        }
    }

    fn create_offscreen_texture(
        gpu: &GpuResources,
        rect: PhysicalRect,
        format: wgpu::TextureFormat,
        label: &'static str,
    ) -> wgpu::Texture {
        #[cfg(test)]
        audit::record_custom_texture();
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: rect.width,
                height: rect.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    fn recreate_textures(&mut self, rect: PhysicalRect) {
        self.front_content = None;
        self.back_content = None;
        self.front_texture = Some(Self::create_offscreen_texture(
            &self.gpu,
            rect,
            self.final_format,
            "CustomRenderPass Front Texture",
        ));
        self.back_texture = Some(Self::create_offscreen_texture(
            &self.gpu,
            rect,
            self.final_format,
            "CustomRenderPass Back Texture",
        ));
        self.rect = rect;
        self.swap_count = 0;
    }

    fn release_textures(&mut self) {
        self.take_textures();
        self.destroyed = true;
    }

    fn take_textures(&mut self) -> Vec<wgpu::Texture> {
        self.front_content = None;
        self.back_content = None;
        let mut textures = Vec::with_capacity(2);
        if let Some(texture) = self.front_texture.take() {
            textures.push(texture);
        }
        if let Some(texture) = self.back_texture.take() {
            textures.push(texture);
        }
        textures
    }

    fn discard_pending_registrations(&self) {
        let mut pending = self
            .gpu
            .pending_registrations
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        pending.retain(|entry| entry.texture_id != self.texture_id);
    }

    pub(super) fn uses_baseline_resources(&self, gpu: &GpuResources) -> bool {
        std::sync::Arc::ptr_eq(&self.gpu.device, &gpu.device)
            && std::sync::Arc::ptr_eq(&self.gpu.queue, &gpu.queue)
    }

    /// Frozen submitted work may publish before completion, never before every
    /// outstanding producer frame is terminal. This performs no device poll.
    pub(super) fn submitted_preparation_ready(&self) -> Result<bool, UnadmittedPreparationError> {
        let work = self
            .unadmitted_preparation
            .as_ref()
            .ok_or(UnadmittedPreparationError::NotTracked)?;
        if work.uncertified.load(Ordering::Acquire)
            || self.destroyed
            || !Arc::ptr_eq(&work.queue, &self.gpu.queue)
        {
            return Err(UnadmittedPreparationError::Uncertified);
        }
        if Arc::strong_count(work) != 1 {
            return Ok(false);
        }
        let last = work.last_submission.lock().map_err(|_| {
            work.uncertified.store(true, Ordering::Release);
            UnadmittedPreparationError::Uncertified
        })?;
        Ok(self.swap_count > 0 && last.is_some())
    }

    #[cfg(test)]
    pub(super) fn hold_preparation_observation_for_test(&self) -> Option<Arc<AtomicBool>> {
        let hold = &self.unadmitted_preparation.as_ref()?.observation_held;
        hold.store(true, Ordering::Release);
        Some(hold.clone())
    }

    /// The same tracked writer completion survives admission; sampling custody is independent.
    pub(super) fn unadmitted_preparation_complete(
        &self,
    ) -> Result<bool, UnadmittedPreparationError> {
        let work = self
            .unadmitted_preparation
            .as_ref()
            .ok_or(UnadmittedPreparationError::NotTracked)?;
        if work.uncertified.load(Ordering::Acquire) || self.destroyed {
            return Err(UnadmittedPreparationError::Uncertified);
        }
        // Each outstanding frame owns a clone until submission or cancellation.
        if Arc::strong_count(work) != 1 {
            return Ok(false);
        }
        if self.gpu.device.poll(wgpu::PollType::Poll).is_err() {
            work.uncertified.store(true, Ordering::Release);
            return Err(UnadmittedPreparationError::PollFailed);
        }
        #[cfg(test)]
        if work.observation_held.load(Ordering::Acquire) {
            return Ok(false);
        }
        let last = work.last_submission.lock().map_err(|_| {
            work.uncertified.store(true, Ordering::Release);
            UnadmittedPreparationError::Uncertified
        })?;
        Ok(last
            .as_ref()
            .is_none_or(|completion| completion.load(Ordering::Acquire)))
    }

    #[cfg(test)]
    pub(super) fn unadmitted_completion_for_test(&self) -> Option<Arc<AtomicBool>> {
        self.unadmitted_preparation
            .as_ref()?
            .last_submission
            .lock()
            .ok()?
            .clone()
    }

    pub(super) fn front_registration(&self) -> CustomRenderPassResult<PendingImageRegistration> {
        self.ensure_live()?;
        let front_texture = self
            .front_texture
            .as_ref()
            .ok_or(CustomRenderPassError::Destroyed)?;
        let view = self
            .front_content
            .as_ref()
            .map(|content| content.view.clone())
            .unwrap_or_else(|| front_texture.create_view(&wgpu::TextureViewDescriptor::default()));
        Ok(PendingImageRegistration {
            texture_id: self.texture_id,
            view,
            nearest: self.nearest,
        })
    }

    pub(crate) fn pin_hdr_front(&mut self) -> CustomRenderPassResult<HdrFrontPin> {
        self.ensure_live()?;
        if self
            .unadmitted_preparation
            .as_ref()
            .is_some_and(|work| Arc::strong_count(work) != 1)
        {
            // An encoder already handed to a producer may still submit later.
            // Registration metadata cannot revoke that writer capability.
            return Err(CustomRenderPassError::ContentPinned);
        }
        let custody = self
            .hdr_custody
            .as_ref()
            .filter(|c| self.gpu.hdr_final_is_live(c))
            .ok_or(CustomRenderPassError::Destroyed)?
            .clone();
        let texture = self
            .front_texture
            .as_ref()
            .ok_or(CustomRenderPassError::Destroyed)?
            .clone();
        if self.front_content.is_none() {
            self.front_content = Some(Arc::new(HdrContentView {
                view: texture.create_view(&wgpu::TextureViewDescriptor::default()),
            }));
        }
        if self.back_content.is_none() {
            self.back_content = Some(Arc::new(HdrContentView {
                view: self
                    .back_texture
                    .as_ref()
                    .ok_or(CustomRenderPassError::Destroyed)?
                    .create_view(&wgpu::TextureViewDescriptor::default()),
            }));
        }
        let content = self.front_content.as_ref().unwrap().clone();
        let lease = Arc::new(HdrFrontReadLease {
            content,
            texture,
            custody,
            queue: Arc::downgrade(&self.gpu.queue),
            wake: self.gpu.image_admission.clone(),
        });
        // This exact cached view must actually survive renderer registration;
        // merely finding an entry with the same texture_id is not sufficient.
        self.queue_front_registration()?;
        let registration = self.gpu.image_admission.request_exact_view(
            self.texture_id,
            Some(lease.content.view.clone()),
            Some(lease.clone()),
        );
        Ok(HdrFrontPin {
            lease,
            registration,
            swap_count: self.swap_count,
            _ui_owner: std::marker::PhantomData,
        })
    }

    pub(crate) fn hdr_back_writable(&self) -> bool {
        !self.destroyed
            && self
                .back_content
                .as_ref()
                .is_none_or(|c| Arc::strong_count(c) == 1)
    }

    pub(crate) fn matches_hdr_pin(&self, pin: &HdrFrontPin) -> bool {
        !self.destroyed
            && self.swap_count == pin.swap_count
            && self
                .front_content
                .as_ref()
                .is_some_and(|c| Arc::ptr_eq(c, &pin.lease.content))
            && self
                .hdr_custody
                .as_ref()
                .is_some_and(|c| Arc::ptr_eq(c, &pin.lease.custody))
            && self.gpu.hdr_final_is_live(&pin.lease.custody)
    }
}

impl CustomRenderLifecycle for CustomRenderPass {
    fn texture_id(&self) -> u64 {
        self.texture_id()
    }

    fn physical_rect(&self) -> PhysicalRect {
        self.physical_rect()
    }

    fn queue_front_registration(&self) -> CustomRenderPassResult<()> {
        self.queue_front_registration()
    }

    fn register_front_now(&self, renderer: &mut WgpuRenderer) -> CustomRenderPassResult<()> {
        self.register_front_now(renderer)
    }

    fn begin_frame(&self, label: Option<&str>) -> CustomRenderPassResult<CustomRenderFrame> {
        self.begin_frame(label)
    }

    fn submit_and_swap(&mut self, frame: CustomRenderFrame) -> CustomRenderPassResult<()> {
        self.submit_and_swap(frame)
    }

    fn resize(&mut self, rect: PhysicalRect) -> CustomRenderPassResult<()> {
        self.resize(rect)
    }

    fn resize_registered(
        &mut self,
        renderer: &mut WgpuRenderer,
        rect: PhysicalRect,
    ) -> CustomRenderPassResult<()> {
        self.resize_registered(renderer, rect)
    }

    fn destroy(self, renderer: &mut WgpuRenderer) {
        self.destroy(renderer);
    }

    fn destroy_deferred(self) -> CustomRenderPassResult<()> {
        self.destroy_deferred()
    }
}

impl Drop for CustomRenderPass {
    fn drop(&mut self) {
        debug_assert!(
            self.destroyed,
            "INV-295-6: CustomRenderPass must be destroyed before widget-owned textures are dropped"
        );
    }
}

pub struct CustomRenderFrame {
    encoder: Option<wgpu::CommandEncoder>,
    back_view: wgpu::TextureView,
    unadmitted_preparation: Option<Arc<UnadmittedPreparation>>,
}

impl CustomRenderFrame {
    pub fn encoder_and_view(&mut self) -> (&mut wgpu::CommandEncoder, &wgpu::TextureView) {
        let encoder = self
            .encoder
            .as_mut()
            .expect("CustomRenderFrame encoder already finished");
        (encoder, &self.back_view)
    }

    fn finish(mut self) -> wgpu::CommandBuffer {
        self.encoder
            .take()
            .expect("CustomRenderFrame encoder already finished")
            .finish()
    }
}

#[cfg(test)]
mod replacement_writer_tests {
    use super::*;

    #[test]
    fn sdf_replacement_uncertified_writer_cannot_publish_or_complete() {
        let renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let producer = CustomRenderPass::new_unpublished(
            4_202_610_901,
            renderer.gpu_resources(),
            PhysicalRect::new(0, 0, 16, 16),
        )
        .unwrap();
        let frame = producer
            .begin_frame(Some("uncertified writer handoff"))
            .unwrap();
        // This is the same guard used across Queue::submit and certificate
        // installation. Its nonterminal drop cannot claim no-submit cancellation.
        let handoff = UnadmittedSubmission {
            work: [
                producer.unadmitted_preparation.clone(),
                frame.unadmitted_preparation.clone(),
            ],
            registered: false,
        };
        drop(handoff);
        assert!(matches!(
            producer.submitted_preparation_ready(),
            Err(UnadmittedPreparationError::Uncertified)
        ));
        drop(frame);
        assert!(matches!(
            producer.unadmitted_preparation_complete(),
            Err(UnadmittedPreparationError::Uncertified)
        ));
        assert!(!producer.destroyed);
        // There is deliberately no completion certificate for uncertain work.
        // Preserve the actual owner/resources rather than invent safe destruction.
        std::mem::forget(producer);
    }
}
