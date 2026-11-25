//! HAL381's private single-texture writer and frame read custody. This is a
//! viewport presentation resource, never a PBR front or a public Image producer.
use super::*;
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

pub(super) struct PresenceImageLayout {
    pub(super) globals: wgpu::BindGroupLayout,
    pub(super) image: wgpu::BindGroupLayout,
    pub(super) format: wgpu::TextureFormat,
}

pub(crate) struct PresenceTerminalPipeline {
    pub(crate) pipeline: wgpu::RenderPipeline,
    _module: wgpu::ShaderModule,
    _charge: Box<dyn Send + Sync>,
}

pub(crate) struct PresenceGeneration {
    users: AtomicUsize,
    pub(super) view: wgpu::TextureView,
    _retained: Arc<dyn Send + Sync>,
    wake: crate::renderer::image_binding::ImageAdmissionMailbox,
}

struct PresenceUseInner(Arc<PresenceGeneration>);
impl Drop for PresenceUseInner {
    fn drop(&mut self) {
        self.0.users.fetch_sub(1, Ordering::AcqRel);
        self.0.wake.notify();
    }
}

/// A compiled/native output retains its exact sampled generation until either
/// cancellation or completion of its actual Queue submission.
#[derive(Clone)]
pub(crate) struct PresenceFrameUse(
    #[allow(
        dead_code,
        reason = "Retains the sampled generation until cancellation or queue completion"
    )]
    Arc<PresenceUseInner>,
);
impl PresenceFrameUse {
    fn new(generation: Arc<PresenceGeneration>) -> Self {
        generation.users.fetch_add(1, Ordering::AcqRel);
        Self(Arc::new(PresenceUseInner(generation)))
    }
}

struct PresenceWrite {
    command: wgpu::CommandBuffer,
    retained: Arc<dyn Send + Sync>,
}

struct PresenceEntry {
    generation: Arc<PresenceGeneration>,
    write: Option<PresenceWrite>,
    retired: Arc<AtomicBool>,
    writable: bool,
    terminal: Option<Arc<PresenceTerminalPipeline>>,
    register_only: bool,
}

#[derive(Default)]
pub(super) struct PresenceIngress {
    entries: std::collections::HashMap<u64, PresenceEntry>,
}

/// UI-owned writer. There is one physical output texture; writes can proceed
/// only after old CPU/GPU readers have relinquished it. No waiting/join occurs.
pub(crate) struct PresenceOutput {
    id: u64,
    gpu: GpuResources,
    generation: Arc<PresenceGeneration>,
    retired: Arc<AtomicBool>,
}
impl PresenceOutput {
    pub(crate) fn id(&self) -> u64 {
        self.id
    }
    pub(crate) fn can_write(&self) -> bool {
        let state = self
            .gpu
            .presence_ingress()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        state.entries.get(&self.id).is_some_and(|e| {
            e.writable
                && e.write.is_none()
                && !e.retired.load(Ordering::Acquire)
                && e.generation.users.load(Ordering::Acquire) == 0
        })
    }
    pub(crate) fn queue(
        &self,
        command: wgpu::CommandBuffer,
        retained: Arc<dyn Send + Sync>,
    ) -> Result<bool, &'static str> {
        if self.retired.load(Ordering::Acquire) || !self.gpu.private_hdr_owner_live() {
            return Err("HAL381 presentation owner retired");
        }
        let mut state = self
            .gpu
            .presence_ingress()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        let entry = state
            .entries
            .get_mut(&self.id)
            .ok_or("HAL381 output retired")?;
        if !entry.writable {
            return Err("HAL381 canonical front alias is read-only");
        }
        if entry.write.is_some() || self.generation.users.load(Ordering::Acquire) != 0 {
            return Ok(false);
        }
        entry.write = Some(PresenceWrite { command, retained });
        self.gpu.image_admission.notify();
        Ok(true)
    }
    pub(crate) fn view(&self) -> &wgpu::TextureView {
        &self.generation.view
    }
}
impl Drop for PresenceOutput {
    fn drop(&mut self) {
        self.retired.store(true, Ordering::Release);
        self.gpu.image_admission.notify();
    }
}

impl GpuResources {
    /// Host objects retained by one private writer/read alias, including the
    /// actual renderer Image entry and its view/bind-group handles. Driver
    /// allocation sizes remain bounded by the existing object/target limits.
    pub(crate) fn loading_presence_metadata_bytes() -> u64 {
        (std::mem::size_of::<PresenceEntry>()
            + std::mem::size_of::<PresenceGeneration>()
            + std::mem::size_of::<PresenceOutput>()
            + std::mem::size_of::<PresenceUseInner>()
            + std::mem::size_of::<ImageTextureEntry>()
            + 8 * std::mem::size_of::<usize>()) as u64
    }
    fn presence_ingress(&self) -> &std::sync::Mutex<PresenceIngress> {
        self.pending_rgba8_registrations
            .presence
            .get_or_init(Default::default)
    }

    pub(crate) fn loading_presence_output(
        &self,
        id: u64,
        view: wgpu::TextureView,
        retained: Arc<dyn Send + Sync>,
        terminal: Option<Arc<PresenceTerminalPipeline>>,
    ) -> Result<PresenceOutput, &'static str> {
        if !self.private_hdr_owner_live() {
            return Err("renderer retired");
        }
        let mut state = self
            .presence_ingress()
            .lock()
            .unwrap_or_else(|e| e.into_inner());
        if state.entries.contains_key(&id) {
            return Err("duplicate HAL381 presentation");
        }
        state
            .entries
            .try_reserve(1)
            .map_err(|_| "HAL381 presentation custody capacity")?;
        let generation = Arc::new(PresenceGeneration {
            users: AtomicUsize::new(0),
            view,
            _retained: retained,
            wake: self.image_admission.clone(),
        });
        let retired = Arc::new(AtomicBool::new(false));
        let writable = terminal.is_none();
        state.entries.insert(
            id,
            PresenceEntry {
                generation: generation.clone(),
                retired: retired.clone(),
                writable,
                register_only: terminal.is_some(),
                terminal,
                write: None,
            },
        );
        Ok(PresenceOutput {
            id,
            gpu: self.clone(),
            generation,
            retired,
        })
    }

    pub(crate) fn loading_presence_terminal_pipeline(
        &self,
        charge: impl FnOnce() -> Box<dyn Send + Sync>,
    ) -> Result<PresenceTerminalPipeline, &'static str> {
        let layout = self
            .pending_rgba8_registrations
            .presence_layout
            .get()
            .ok_or("missing renderer presentation layout")?;
        let text = format!(
            "{}\n{}",
            include_str!("shaders/image.wgsl"),
            TERMINAL_FRAGMENT
        );
        let module =
            naga::front::wgsl::parse_str(&text).map_err(|_| "HAL381 terminal shader parse")?;
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::empty(),
        )
        .validate(&module)
        .map_err(|_| "HAL381 terminal shader validation")?;
        let validation_scope = self.device.push_error_scope(wgpu::ErrorFilter::Validation);
        let shader = self
            .device
            .create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("HAL381 viewport-only CanonicalOverNeutral"),
                source: wgpu::ShaderSource::Wgsl(text.into()),
            });
        let pipeline_layout = self
            .device
            .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("HAL381 private one-clip terminal"),
                bind_group_layouts: &[Some(&layout.globals), Some(&layout.image)],
                immediate_size: 0,
            });
        let pipeline = self
            .device
            .create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("HAL381 CanonicalOverNeutral"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("vs_main"),
                    buffers: &[Some(ImageVertex::desc())],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some("fs_canonical_over_neutral"),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: layout.format,
                        blend: Some(wgpu::BlendState::PREMULTIPLIED_ALPHA_BLENDING),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            });
        if pollster::block_on(validation_scope.pop()).is_some() {
            return Err("HAL381 terminal pipeline Device refusal");
        }
        Ok(PresenceTerminalPipeline {
            pipeline,
            _module: shader,
            _charge: charge(),
        })
    }
}

pub(crate) const TERMINAL_FRAGMENT: &str = r#"
@fragment fn fs_canonical_over_neutral(input: VertexOutput) -> @location(0) vec4<f32> {
 let f=textureSample(image_tex,image_sampler,input.uv);
 let h=f.rgb+(1.0-f.a)*vec3<f32>(0.025,0.031,0.040);
 let c=compute_rounded_clip_coverage(input.position);
 if(c<=0.0){discard;}
 return vec4<f32>(h,1.0)*c;
}
"#;

impl WgpuRenderer {
    pub(super) fn close_loading_presence(&mut self) {
        if let Some(state) = self
            .pending_rgba8_registrations
            .get()
            .and_then(|p| p.presence.get())
        {
            let entries =
                std::mem::take(&mut state.lock().unwrap_or_else(|e| e.into_inner()).entries);
            for entry in entries.values() {
                entry.retired.store(true, Ordering::Release);
            }
            crate::renderer::on_submitted_work_done(&self.queue, move || drop(entries));
        }
    }

    pub(super) fn loading_presence_terminal_pipelines(
        &self,
    ) -> HashMap<u64, Arc<PresenceTerminalPipeline>> {
        self.pending_rgba8_registrations
            .get()
            .and_then(|p| p.presence.get())
            .map(|s| {
                s.lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .entries
                    .iter()
                    .filter_map(|(id, e)| {
                        (!e.retired.load(Ordering::Acquire))
                            .then(|| e.terminal.as_ref().map(|p| (*id, p.clone())))
                            .flatten()
                    })
                    .collect()
            })
            .unwrap_or_default()
    }
    /// Runs at the existing drain boundary, before Compile. A registered sampled
    /// view is removed BEFORE its one physical texture is written, then the
    /// completed same-Queue producer is registered as a new Image generation.
    pub(super) fn drain_loading_presence(&mut self) {
        let Some(shared) = self.pending_rgba8_registrations.get().cloned() else {
            return;
        };
        let Some(ingress) = shared.presence.get() else {
            return;
        };
        let mut state = ingress.lock().unwrap_or_else(|e| e.into_inner());
        state.entries.retain(|id, entry| {
            if entry.retired.load(Ordering::Acquire) {
                self.unregister_image(*id);
                // Outstanding frame uses retain the complete physical allocation
                // and its original charge independently of this registry slot.
                return false;
            }
            if entry.generation.users.load(Ordering::Acquire) == 0 {
                if entry.register_only {
                    // This private read name aliases the actual immutable F;
                    // no texture/copy/PBR front is created or rewritten.
                    self.register_image_from_view(*id, entry.generation.view.clone());
                    entry.register_only = false;
                }
                if let Some(write) = entry.write.take() {
                    self.unregister_image(*id);
                    self.queue.submit([write.command]);
                    let retained = write.retained;
                    crate::renderer::on_submitted_work_done(&self.queue, move || drop(retained));
                    self.register_image_from_view(*id, entry.generation.view.clone());
                }
            }
            true
        });
    }

    pub(super) fn loading_presence_frame_uses(
        &self,
        data: &HashMap<SurfaceId, crate::renderer::compiled_draw_plan::SurfaceCompiledData>,
    ) -> HashMap<SurfaceId, Vec<PresenceFrameUse>> {
        let Some(ingress) = self
            .pending_rgba8_registrations
            .get()
            .and_then(|p| p.presence.get())
        else {
            return HashMap::new();
        };
        let state = ingress.lock().unwrap_or_else(|e| e.into_inner());
        data.iter()
            .filter_map(|(surface, data)| {
                let mut uses = Vec::new();
                let mut seen = std::collections::HashSet::new();
                for batch in &data.frame_batches.batches {
                    if let Some(id) = batch.key.texture_id {
                        if let Some(entry) = state.entries.get(&id).filter(|e| {
                            !e.retired.load(Ordering::Acquire)
                                && self
                                    .image_textures
                                    .get(&id)
                                    .is_some_and(|registered| registered.view == e.generation.view)
                        }) {
                            if seen.insert(id) {
                                uses.push(PresenceFrameUse::new(entry.generation.clone()));
                            }
                        }
                    }
                }
                (!uses.is_empty()).then_some((*surface, uses))
            })
            .collect()
    }

    pub(super) fn loading_presence_view_matches(&self, id: u64, view: &wgpu::TextureView) -> bool {
        self.pending_rgba8_registrations
            .get()
            .and_then(|p| p.presence.get())
            .is_none_or(|state| {
                state
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .entries
                    .get(&id)
                    .is_none_or(|e| {
                        !e.retired.load(Ordering::Acquire) && e.generation.view == *view
                    })
            })
    }
}
