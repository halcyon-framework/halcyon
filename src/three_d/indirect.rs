//! Private R-026 mechanical source/resource owner. Original asset integration
//! and static transport belong to the producer, not to this renderer.
use super::context::{
    ControlledResourceKey3D, ControlledResourceKind3D, ControlledResourceLease3D,
    DynamicStorageLease3D, PreparationUploadBudget3D, TextureGpuResidency,
};
use super::pbr::{MaterialSceneSource3D, PbrError3D};
#[cfg(test)]
pub(crate) use super::raster::indirect_probe_for_test;
use super::snapshot::{CommittedMaterialPacket3D, MaterialModelState3D, MaterialSnapshot3D};
use super::{
    Context3D, Orientation3, SamplerAddressMode, SamplerFilterMode, SamplerHandle, SamplerLease,
};
use bytemuck::{Pod, Zeroable};
use std::sync::{Arc, Mutex, Weak};

pub(crate) const ENVIRONMENT_BYTES: u64 = 6_406_144;
fn error(field: &'static str, cause: &'static str) -> PbrError3D {
    PbrError3D::Indirect { field, cause }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct Presence3D(pub(crate) u8);
impl Presence3D {
    pub(crate) fn direct(self) -> bool {
        self.0 & 1 != 0
    }
    pub(crate) fn environment(self) -> bool {
        self.0 & 2 != 0
    }
    pub(crate) fn gi(self) -> bool {
        self.0 & 4 != 0
    }
    pub(crate) fn indirect(self) -> bool {
        self.0 & 6 != 0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum OriginalHandedness3D {
    Right,
    Left,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceReflectionAxis3D {
    X,
    Y,
    Z,
}
/// Producer-owned derivation records. The renderer checks attribution and exact
/// derived bytes; it does not run either producer algorithm. The private proof
/// fixtures supply actual original fields, integration and transport traces.
pub(crate) enum SourceDerivation3D {
    Environment {
        method: Arc<str>,
        original_field: Arc<[f64]>,
        reference_observations: Arc<[f64]>,
        derived: [Weak<[u16]>; 3],
    },
    StaticTransport {
        method: Arc<str>,
        original_transport_field: Arc<[f64]>,
        first: [[f64; 3]; 6],
        second: [[f64; 3]; 6],
    },
}
/// Original-to-WORLD evidence is retained on the CPU. None of this mapping is
/// uploaded or used to rotate shader queries or relabel a foreign G6 field.
#[derive(Clone)]
pub(crate) struct ProducerEvidence3D {
    pub(crate) original_identity: u64,
    pub(crate) original_revision: u64,
    pub(crate) producer_identity: u64,
    pub(crate) producer_revision: u64,
    pub(crate) evidence_identity: u64,
    pub(crate) world_identity: u64,
    pub(crate) original_handedness: Option<OriginalHandedness3D>,
    pub(crate) handedness_correction: Option<SourceReflectionAxis3D>,
    pub(crate) original_to_world: Option<Orientation3>,
    pub(crate) integration_work: u64,
    pub(crate) absolute_error_bound: f64,
    pub(crate) derivation: Arc<SourceDerivation3D>,
}
impl ProducerEvidence3D {
    fn retained_bytes(&self) -> Result<u64, PbrError3D> {
        let (text, values) = match self.derivation.as_ref() {
            SourceDerivation3D::Environment {
                method,
                original_field,
                reference_observations,
                ..
            } => (
                method.len() as u64,
                (original_field.len() as u64).checked_add(reference_observations.len() as u64),
            ),
            SourceDerivation3D::StaticTransport {
                method,
                original_transport_field,
                ..
            } => (
                method.len() as u64,
                Some(original_transport_field.len() as u64),
            ),
        };
        values
            .and_then(|n| n.checked_mul(8))
            .and_then(|n| n.checked_add(text))
            .and_then(|n| n.checked_add(std::mem::size_of::<Self>() as u64))
            .and_then(|n| n.checked_add(std::mem::size_of::<SourceDerivation3D>() as u64))
            .ok_or(error("producer bytes", "retained derivation size overflow"))
    }
    fn validate(&self) -> Result<(), PbrError3D> {
        if [
            self.original_identity,
            self.original_revision,
            self.producer_identity,
            self.producer_revision,
            self.evidence_identity,
            self.world_identity,
            self.integration_work,
        ]
        .contains(&0)
        {
            return Err(error(
                "producer evidence",
                "missing attributable original/producer/world/integration record",
            ));
        }
        if self.original_to_world.is_none()
            || self.original_handedness.is_none()
            || (self.original_handedness == Some(OriginalHandedness3D::Left))
                != self.handedness_correction.is_some()
        {
            return Err(error(
                "source world frame",
                "unknown or unqualified original mapping/handedness",
            ));
        }
        if !self.absolute_error_bound.is_finite() || self.absolute_error_bound < 0.0 {
            return Err(error(
                "producer evidence",
                "nonfinite or negative source error bound",
            ));
        }
        let (method, original) = match self.derivation.as_ref() {
            SourceDerivation3D::Environment {
                method,
                original_field,
                reference_observations,
                ..
            } => {
                if reference_observations.is_empty()
                    || reference_observations.iter().any(|v| !v.is_finite())
                {
                    return Err(error(
                        "E derivation",
                        "missing finite matched integration observations",
                    ));
                }
                (method, original_field)
            }
            SourceDerivation3D::StaticTransport {
                method,
                original_transport_field,
                first,
                second,
            } => {
                if first
                    .iter()
                    .chain(second)
                    .flatten()
                    .any(|v| !v.is_finite() || *v < 0.0)
                {
                    return Err(error(
                        "G derivation",
                        "invalid first/second transport trace",
                    ));
                }
                (method, original_transport_field)
            }
        };
        if method.is_empty() || original.is_empty() || original.iter().any(|v| !v.is_finite()) {
            return Err(error(
                "producer derivation",
                "original field and method are required; tokens alone are insufficient",
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnvironmentRole3D {
    Diffuse,
    Specular,
    BrdfLut,
}
impl EnvironmentRole3D {
    pub(crate) fn dimensions(self) -> (u32, u32, u32, u32) {
        match self {
            Self::Diffuse => (32, 32, 6, 4),
            Self::Specular => (128, 128, 48, 4),
            Self::BrdfLut => (128, 128, 1, 2),
        }
    }
    fn kind(self) -> ControlledResourceKind3D {
        match self {
            Self::Diffuse => ControlledResourceKind3D::EnvironmentDiffuse,
            Self::Specular => ControlledResourceKind3D::EnvironmentSpecular,
            Self::BrdfLut => ControlledResourceKind3D::EnvironmentLut,
        }
    }
    pub(super) fn format(self) -> wgpu::TextureFormat {
        if self == Self::BrdfLut {
            wgpu::TextureFormat::Rg16Float
        } else {
            wgpu::TextureFormat::Rgba16Float
        }
    }
    pub(super) fn view(self) -> wgpu::TextureViewDimension {
        match self {
            Self::Diffuse => wgpu::TextureViewDimension::Cube,
            Self::Specular => wgpu::TextureViewDimension::CubeArray,
            Self::BrdfLut => wgpu::TextureViewDimension::D2,
        }
    }
}

/// Exact IEEE binary16 decoding, before upload. No sRGB/alpha conversion.
pub(crate) fn decode_half(bits: u16) -> f32 {
    let sign = u32::from(bits & 0x8000) << 16;
    let exponent = (bits >> 10) & 31;
    let fraction = u32::from(bits & 1023);
    if exponent == 0 {
        if fraction == 0 {
            f32::from_bits(sign)
        } else {
            let value = (fraction as f32) * 2.0f32.powi(-24);
            if sign == 0 { value } else { -value }
        }
    } else if exponent == 31 {
        f32::from_bits(sign | 0x7f800000 | (fraction << 13))
    } else {
        f32::from_bits(sign | ((u32::from(exponent) + 112) << 23) | (fraction << 13))
    }
}

#[derive(Clone, Debug)]
pub(crate) struct EnvironmentHandle3D {
    pub(super) key: ControlledResourceKey3D,
    pub(crate) role: EnvironmentRole3D,
}
pub(super) struct EnvironmentData3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) role: EnvironmentRole3D,
    pub(crate) evidence: Arc<ProducerEvidence3D>,
    pub(crate) bits: Arc<[u16]>,
    pub(crate) maximum: [f32; 3],
    gpu: Mutex<Weak<EnvironmentGpu3D>>,
}
#[derive(Clone)]
pub(crate) struct EnvironmentLease3D {
    pub(crate) handle: EnvironmentHandle3D,
    pub(super) data: Arc<EnvironmentData3D>,
    _ownership: ControlledResourceLease3D,
}
pub(super) struct EnvironmentGpu3D {
    pub(super) texture: wgpu::Texture,
    pub(super) view: wgpu::TextureView,
    _source: EnvironmentLease3D,
    _charge: TextureGpuResidency,
    enqueued: std::sync::Once,
    _storage: DynamicStorageLease3D,
}
pub(super) struct DeferredEnvironmentUpload3D {
    pub(super) gpu: Arc<EnvironmentGpu3D>,
    source: EnvironmentLease3D,
}
impl DeferredEnvironmentUpload3D {
    pub(super) fn commit(self, queue: &wgpu::Queue) {
        let (w, h, layers, channels) = self.source.data.role.dimensions();
        self.gpu.enqueued.call_once(|| {
            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.gpu.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d::ZERO,
                    aspect: wgpu::TextureAspect::All,
                },
                bytemuck::cast_slice(&self.source.data.bits),
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(w * channels * 2),
                    rows_per_image: Some(h),
                },
                wgpu::Extent3d {
                    width: w,
                    height: h,
                    depth_or_array_layers: layers,
                },
            )
        });
    }
}

impl Context3D {
    pub(crate) fn register_environment_asset(
        &self,
        identity: u64,
        revision: u64,
        role: EnvironmentRole3D,
        evidence: Arc<ProducerEvidence3D>,
        bits: Arc<[u16]>,
        previous: Option<&EnvironmentHandle3D>,
    ) -> Result<EnvironmentHandle3D, PbrError3D> {
        evidence.validate()?;
        let SourceDerivation3D::Environment { derived, .. } = evidence.derivation.as_ref() else {
            return Err(error(
                "E derivation",
                "not an environment integration record",
            ));
        };
        let index = match role {
            EnvironmentRole3D::Diffuse => 0,
            EnvironmentRole3D::Specular => 1,
            EnvironmentRole3D::BrdfLut => 2,
        };
        if !Weak::ptr_eq(&Arc::downgrade(&bits), &derived[index]) {
            return Err(error(
                "E derivation",
                "asset is not the exact producer-derived immutable byte record",
            ));
        }
        if identity == 0 || revision == 0 {
            return Err(error("E identity", "missing immutable asset generation"));
        }
        let (w, h, layers, channels) = role.dimensions();
        if bits.len() as u64
            != u64::from(w) * u64::from(h) * u64::from(layers) * u64::from(channels)
        {
            return Err(error(
                "E extent",
                "wrong complete one-mip role extent/layer/channel count",
            ));
        }
        let mut maximum = [0.0f32; 3];
        for pixel in bits.chunks_exact(channels as usize) {
            let mut values = [0.0f32; 4];
            for (v, b) in values.iter_mut().zip(pixel) {
                *v = decode_half(*b);
            }
            if values.iter().any(|v| !v.is_finite()) {
                return Err(error("E binary16", "nonfinite packed texel element"));
            }
            if role == EnvironmentRole3D::BrdfLut {
                if values[0] < 0.0
                    || values[1] < 0.0
                    || f64::from(values[0]) + f64::from(values[1]) > 1.0
                {
                    return Err(error(
                        "E LUT binary16",
                        "packed A/B outside nonnegative A+B<=1",
                    ));
                }
            } else if values[..3].iter().any(|v| !(0.0..=4096.0).contains(v)) {
                return Err(error("E RGB binary16", "packed RGB outside0..4096"));
            }
            for c in 0..3 {
                maximum[c] = maximum[c].max(values[c]);
            }
        }
        if let Some(old) = previous {
            let old = self.lease_environment_asset(old)?;
            if old.data.identity != identity
                || revision <= old.data.revision
                || old.data.role != role
            {
                return Err(error(
                    "E successor",
                    "asset identity/role must persist and revision advance",
                ));
            }
        }
        let evidence_bytes = evidence.retained_bytes()?;
        let bytes = (bits.len() as u64)
            .checked_mul(2)
            .and_then(|v| v.checked_add(std::mem::size_of::<EnvironmentData3D>() as u64))
            .and_then(|v| v.checked_add(evidence_bytes))
            .and_then(|v| v.checked_add(std::mem::size_of::<EnvironmentGpu3D>() as u64))
            .ok_or(error("E bytes", "source size overflow"))?;
        let data = Arc::new(EnvironmentData3D {
            identity,
            revision,
            role,
            evidence,
            bits,
            maximum,
            gpu: Mutex::new(Weak::new()),
        });
        Ok(EnvironmentHandle3D {
            key: self.register_controlled_source(
                role.kind(),
                data,
                bytes,
                previous.map(|p| &p.key),
            )?,
            role,
        })
    }
    pub(crate) fn lease_environment_asset(
        &self,
        handle: &EnvironmentHandle3D,
    ) -> Result<EnvironmentLease3D, PbrError3D> {
        let ownership = self.lease_controlled_source(&handle.key, handle.role.kind())?;
        let data = ownership.source()?;
        Ok(EnvironmentLease3D {
            handle: handle.clone(),
            data,
            _ownership: ownership,
        })
    }
    pub(super) fn prepare_environment_asset(
        &self,
        source: &EnvironmentLease3D,
        upload: &mut PreparationUploadBudget3D,
    ) -> Result<(Arc<EnvironmentGpu3D>, Option<DeferredEnvironmentUpload3D>), PbrError3D> {
        if !source._ownership.belongs_to(self) {
            return Err(super::Context3DError::ContextMismatch.into());
        }
        let mut cache = source
            .data
            .gpu
            .lock()
            .map_err(|_| PbrError3D::Lock("E GPU generation"))?;
        if let Some(gpu) = cache.upgrade() {
            let pending = if gpu.enqueued.is_completed() {
                None
            } else {
                upload.consume(source.data.bits.len() as u64 * 2)?;
                Some(DeferredEnvironmentUpload3D {
                    gpu: gpu.clone(),
                    source: source.clone(),
                })
            };
            return Ok((gpu, pending));
        }
        let bytes = source.data.bits.len() as u64 * 2;
        upload.consume(bytes)?;
        let gpu = self.gpu_resources()?;
        let (w, h, layers, _) = source.data.role.dimensions();
        let features = source
            .data
            .role
            .format()
            .guaranteed_format_features(gpu.device.features());
        if !features
            .allowed_usages
            .contains(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST)
            || !features
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
            || w > gpu.device.limits().max_texture_dimension_2d
            || layers > gpu.device.limits().max_texture_array_layers
            || !cfg!(target_endian = "little")
        {
            return Err(error(
                "E Device capability",
                "exact filterable one-mip float role unavailable",
            ));
        }
        let charge = self.reserve_shadow_texture_bytes(bytes)?;
        let staging = self.reserve_dynamic_storage(
            0,
            0,
            0,
            bytes + std::mem::size_of::<EnvironmentGpu3D>() as u64,
        )?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("R026 immutable qualified float asset"),
            size: wgpu::Extent3d {
                width: w,
                height: h,
                depth_or_array_layers: layers,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: source.data.role.format(),
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&wgpu::TextureViewDescriptor {
            dimension: Some(source.data.role.view()),
            ..Default::default()
        });
        #[cfg(test)]
        super::audit::record_gpu_texture();
        let resident = Arc::new(EnvironmentGpu3D {
            texture,
            view,
            _source: source.clone(),
            _charge: charge,
            enqueued: std::sync::Once::new(),
            _storage: staging,
        });
        *cache = Arc::downgrade(&resident);
        Ok((
            resident.clone(),
            Some(DeferredEnvironmentUpload3D {
                gpu: resident,
                source: source.clone(),
            }),
        ))
    }
}

pub(crate) struct EnvironmentSource3D {
    pub(crate) evidence: Arc<ProducerEvidence3D>,
    pub(crate) assets: [EnvironmentLease3D; 3],
    pub(crate) sampler: SamplerLease,
}
impl EnvironmentSource3D {
    pub(crate) fn from_handles(
        context: &Context3D,
        evidence: Arc<ProducerEvidence3D>,
        handles: [Option<EnvironmentHandle3D>; 3],
        sampler: Option<SamplerHandle>,
    ) -> Result<Self, PbrError3D> {
        evidence.validate()?;
        let mut leases = Vec::with_capacity(3);
        for (handle, role) in handles.into_iter().zip([
            EnvironmentRole3D::Diffuse,
            EnvironmentRole3D::Specular,
            EnvironmentRole3D::BrdfLut,
        ]) {
            let handle = handle.ok_or(error("E roles", "missing required matched asset"))?;
            if handle.role != role {
                return Err(error("E roles", "wrong typed asset role"));
            }
            let lease = context.lease_environment_asset(&handle)?;
            if !Arc::ptr_eq(&lease.data.evidence, &evidence) {
                return Err(error(
                    "E provenance",
                    "original/world/prefilter/LUT generations do not match",
                ));
            }
            leases.push(lease);
        }
        let sampler = context
            .lease_sampler(&sampler.ok_or(error("E sampler", "missing explicit sampler"))?)?;
        let descriptor = sampler.descriptor();
        if descriptor.address_mode_u() != SamplerAddressMode::ClampToEdge
            || descriptor.address_mode_v() != SamplerAddressMode::ClampToEdge
            || descriptor.min_filter() != SamplerFilterMode::Linear
            || descriptor.mag_filter() != SamplerFilterMode::Linear
        {
            return Err(error(
                "E sampler",
                "requires explicit linear ClampToEdge one-mip sampler",
            ));
        }
        Ok(Self {
            evidence,
            assets: leases.try_into().ok().expect("three qualified roles"),
            sampler,
        })
    }
}

#[derive(Clone)]
pub(crate) struct GiDependencies3D {
    pub(crate) snapshot: Arc<MaterialSnapshot3D>,
    pub(crate) model: Arc<MaterialModelState3D>,
    pub(crate) direct: Option<Arc<super::direct_shadow::DirectSource3D>>,
    pub(crate) environment: Option<Arc<EnvironmentSource3D>>,
    pub(crate) visibility_revision: u64,
}
pub(crate) struct GiSource3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) packet: u64,
    pub(crate) patch: u64,
    pub(crate) evidence: Arc<ProducerEvidence3D>,
    pub(crate) dependencies: GiDependencies3D,
    /// Attributable original L0 and TWO producer transport observations. These
    /// are proof records, not an engine-owned transport operator/solver.
    pub(crate) transport_source: u64,
    pub(crate) first_transport: u64,
    pub(crate) second_transport: u64,
    pub(crate) lobes: [[f32; 3]; 6],
}
// The registry must not retain Context3D through snapshot -> context -> registry.
// A live source lease retains the full captured dependencies; the registry's
// immutable payload keeps qualified weak identities and a weak lease cache.
struct GiRecord3D {
    identity: u64,
    revision: u64,
    packet: u64,
    patch: u64,
    evidence: Arc<ProducerEvidence3D>,
    snapshot: Weak<MaterialSnapshot3D>,
    model: Weak<MaterialModelState3D>,
    direct: Option<Weak<super::direct_shadow::DirectSource3D>>,
    environment: Option<Weak<EnvironmentSource3D>>,
    visibility_revision: u64,
    trace: [u64; 3],
    lobes: [[f32; 3]; 6],
    leased: Mutex<Weak<GiSource3D>>,
}
impl GiRecord3D {
    fn from_source(s: GiSource3D) -> Self {
        Self {
            identity: s.identity,
            revision: s.revision,
            packet: s.packet,
            patch: s.patch,
            evidence: s.evidence,
            snapshot: Arc::downgrade(&s.dependencies.snapshot),
            model: Arc::downgrade(&s.dependencies.model),
            direct: s.dependencies.direct.as_ref().map(Arc::downgrade),
            environment: s.dependencies.environment.as_ref().map(Arc::downgrade),
            visibility_revision: s.dependencies.visibility_revision,
            trace: [s.transport_source, s.first_transport, s.second_transport],
            lobes: s.lobes,
            leased: Mutex::new(Weak::new()),
        }
    }
    fn lease(&self) -> Result<Arc<GiSource3D>, PbrError3D> {
        let mut cache = self
            .leased
            .lock()
            .map_err(|_| PbrError3D::Lock("G generation lease"))?;
        if let Some(s) = cache.upgrade() {
            return Ok(s);
        }
        let retired = || {
            error(
                "G dependency lifetime",
                "original snapshot/model/light/environment has retired",
            )
        };
        let snapshot = self.snapshot.upgrade().ok_or_else(retired)?;
        let model = self.model.upgrade().ok_or_else(retired)?;
        let direct = self
            .direct
            .as_ref()
            .map(|v| v.upgrade().ok_or_else(retired))
            .transpose()?;
        let environment = self
            .environment
            .as_ref()
            .map(|v| v.upgrade().ok_or_else(retired))
            .transpose()?;
        let source = Arc::new(GiSource3D {
            identity: self.identity,
            revision: self.revision,
            packet: self.packet,
            patch: self.patch,
            evidence: self.evidence.clone(),
            dependencies: GiDependencies3D {
                snapshot,
                model,
                direct,
                environment,
                visibility_revision: self.visibility_revision,
            },
            transport_source: self.trace[0],
            first_transport: self.trace[1],
            second_transport: self.trace[2],
            lobes: self.lobes,
        });
        *cache = Arc::downgrade(&source);
        Ok(source)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct GiHandle3D(ControlledResourceKey3D);
#[derive(Clone)]
pub(crate) struct GiLease3D {
    pub(crate) source: Arc<GiSource3D>,
    _ownership: ControlledResourceLease3D,
}
impl Context3D {
    pub(crate) fn register_gi_source(
        &self,
        source: GiSource3D,
        previous: Option<&GiHandle3D>,
    ) -> Result<GiHandle3D, PbrError3D> {
        source.evidence.validate()?;
        let SourceDerivation3D::StaticTransport { first, second, .. } =
            source.evidence.derivation.as_ref()
        else {
            return Err(error(
                "G derivation",
                "missing original transport field/two applications",
            ));
        };
        if (0..6).any(|l| (0..3).any(|c| source.lobes[l][c] != (first[l][c] + second[l][c]) as f32))
        {
            return Err(error(
                "G derivation",
                "WORLD lobes do not match L1+L2 excluding L0",
            ));
        }
        if [
            source.identity,
            source.revision,
            source.patch,
            source.dependencies.visibility_revision,
            source.transport_source,
            source.first_transport,
            source.second_transport,
        ]
        .contains(&0)
            || source.first_transport == source.transport_source
            || source.second_transport == source.first_transport
            || source
                .lobes
                .iter()
                .flatten()
                .any(|v| !v.is_finite() || !(0.0..=4096.0).contains(v))
        {
            return Err(error(
                "G source",
                "missing two-step transport/patch/currentness record or invalid WORLD lobes",
            ));
        }
        if !self.same_resource_family(&source.dependencies.snapshot.context) {
            return Err(super::Context3DError::ContextMismatch.into());
        }
        if let Some(old) = previous {
            let old = self
                .lease_controlled_source(&old.0, ControlledResourceKind3D::StaticIrradiance)?
                .source::<GiRecord3D>()?;
            if old.identity != source.identity || source.revision <= old.revision {
                return Err(error(
                    "G successor",
                    "immutable source revision must advance",
                ));
            }
        }
        let source_evidence_bytes = source.evidence.retained_bytes()?;
        let bytes = ((std::mem::size_of::<GiSource3D>() + std::mem::size_of::<GiRecord3D>())
            as u64)
            .checked_add(source_evidence_bytes)
            .ok_or(error("G bytes", "retained generation size overflow"))?;
        Ok(GiHandle3D(self.register_controlled_source(
            ControlledResourceKind3D::StaticIrradiance,
            Arc::new(GiRecord3D::from_source(source)),
            bytes,
            previous.map(|p| &p.0),
        )?))
    }
    pub(crate) fn lease_gi_source(&self, handle: &GiHandle3D) -> Result<GiLease3D, PbrError3D> {
        let ownership =
            self.lease_controlled_source(&handle.0, ControlledResourceKind3D::StaticIrradiance)?;
        let source = ownership.source::<GiRecord3D>()?.lease()?;
        Ok(GiLease3D {
            source,
            _ownership: ownership,
        })
    }
    pub(crate) fn retire_gi_source(&self, handle: &GiHandle3D) -> Result<(), PbrError3D> {
        self.retire_controlled_source(&handle.0, ControlledResourceKind3D::StaticIrradiance)?;
        Ok(())
    }
    pub(crate) fn retire_environment_asset(
        &self,
        handle: &EnvironmentHandle3D,
    ) -> Result<(), PbrError3D> {
        self.retire_controlled_source(&handle.key, handle.role.kind())?;
        Ok(())
    }
}

pub(crate) struct IndirectSource3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) world_identity: u64,
    pub(crate) environment: Option<Arc<EnvironmentSource3D>>,
    pub(crate) gi: Option<Arc<[Option<GiLease3D>]>>,
    pub(crate) visibility_revision: u64,
}
impl IndirectSource3D {
    pub(crate) fn presence(&self, d: bool) -> Presence3D {
        Presence3D(
            u8::from(d)
                | u8::from(self.environment.is_some()) * 2
                | u8::from(self.gi.is_some()) * 4,
        )
    }
    pub(crate) fn validate(
        &self,
        scene: &MaterialSceneSource3D,
        context: &Context3D,
    ) -> Result<(), PbrError3D> {
        if self.identity == 0
            || self.revision == 0
            || self.world_identity == 0
            || (!self.environment.is_some() && !self.gi.is_some())
        {
            return Err(error(
                "indirect source",
                "missing complete independent identity/presence/world",
            ));
        }
        if let Some(e) = &self.environment {
            if e.evidence.world_identity != self.world_identity {
                return Err(error("E world", "canonical source/world mismatch"));
            }
            for asset in &e.assets {
                if !asset._ownership.belongs_to(context) {
                    return Err(super::Context3DError::ContextMismatch.into());
                }
            }
        }
        if let Some(gi) = &self.gi {
            if gi.len() != scene.snapshot.packets.len() {
                return Err(error("G patch mapping", "missing per-packet association"));
            }
            for (i, (packet, g)) in scene.snapshot.packets.iter().zip(gi.iter()).enumerate() {
                match packet {
                    CommittedMaterialPacket3D::LegacyAppearance { .. } => {
                        if g.is_some() {
                            return Err(error(
                                "G patch mapping",
                                "legacy packet is not a PBR receiver",
                            ));
                        }
                    }
                    CommittedMaterialPacket3D::PbrMaterial { ordinal, .. } => {
                        let g = g.as_ref().ok_or(error(
                            "G patch mapping",
                            "missing qualified PBR receiver patch",
                        ))?;
                        let s = &g.source;
                        let deps = &s.dependencies;
                        if !g._ownership.belongs_to(context)
                            || s.evidence.world_identity != self.world_identity
                            || s.packet != *ordinal
                            || !Arc::ptr_eq(&deps.snapshot, &scene.snapshot)
                            || !Arc::ptr_eq(&deps.model, &scene.model)
                            || !same_arc_option(&deps.direct, &scene.direct)
                            || !same_arc_option(&deps.environment, &self.environment)
                            || deps.visibility_revision != self.visibility_revision
                        {
                            let _ = i;
                            return Err(error(
                                "G currentness",
                                "stale source/world/geometry/material/model/light/environment/visibility/patch dependency",
                            ));
                        }
                    }
                }
            }
        }
        self.validate_complete_bound(scene)
    }
    /// Bound the complete immutable source, not just D or the indirect ideal
    /// inequality. This deliberately uses each component's full source range;
    /// no readback, pixel enumeration, source clamp or brightness compensation.
    fn validate_complete_bound(&self, scene: &MaterialSceneSource3D) -> Result<(), PbrError3D> {
        // IEEE binary32 round-to-nearest absolute upper enclosure. The minimum
        // subnormal term also covers underflow; no world-space epsilon is used.
        let rounded = |x: f64| {
            if x == 0.0 {
                0.0
            } else {
                x * (1.0 + 2.0f64.powi(-24)) + 2.0f64.powi(-149)
            }
        };
        let mut incident = [0.0f64; 3];
        if let Some(d) = &scene.direct {
            if let Some(l) = &d.directional {
                for c in 0..3 {
                    incident[c] += f64::from(l.irradiance[c]);
                }
            }
            for l in d.points.iter().chain(d.spots.iter().map(|s| &s.point)) {
                for c in 0..3 {
                    incident[c] += f64::from(l.intensity[c]) / l.radius_m.powi(2);
                }
            }
        }
        for (i, packet) in scene.snapshot.packets.iter().enumerate() {
            let CommittedMaterialPacket3D::PbrMaterial { material, .. } = packet else {
                continue;
            };
            let m = &material.data.source;
            let min_g = if m.roles[2].is_some() {
                material.data.textures[2]
                    .descriptor()
                    .texels()
                    .chunks_exact(4)
                    .map(|p| f64::from(p[1]) / 255.0)
                    .fold(1.0, f64::min)
            } else {
                1.0
            };
            let r = (f64::from(m.roughness) * min_g).max(0.045);
            for c in 0..3 {
                let ed = self
                    .environment
                    .as_ref()
                    .map_or(0.0, |e| f64::from(e.assets[0].data.maximum[c]));
                let es = self
                    .environment
                    .as_ref()
                    .map_or(0.0, |e| f64::from(e.assets[1].data.maximum[c]));
                let g = self
                    .gi
                    .as_ref()
                    .and_then(|g| g[i].as_ref())
                    .map_or(0.0, |g| {
                        g.source
                            .lobes
                            .iter()
                            .map(|l| f64::from(l[c]))
                            .fold(0.0, f64::max)
                    });
                // unit_nonzero scales by the largest component before sqrt:
                // each final |N_i|<=1. Three squared lobe weights therefore
                // have upper bound3 without assuming exact f32 unit length.
                let gi = rounded(rounded(rounded(g) + rounded(g)) + rounded(g));
                let diffuse_input = rounded(ed + gi);
                // AO,(1-m),base,(1-Fview)<=1; division by pi only reduces it.
                let mut diffuse = diffuse_input;
                for _ in 0..5 {
                    diffuse = rounded(diffuse);
                }
                // The two roughness samples each have nonnegative weights<=1.
                // A+B<=1 on the actual half texels; f0<=1, so the LUT factor
                // cannot exceed1 before the stated arithmetic enclosure.
                let spec = rounded(rounded(rounded(es) + rounded(es)));
                // Preserve R025's separately qualified direct envelope. This
                // mechanical sum is necessary, not a new producer or a proof
                // of all backends' source/model/filter precision.
                let direct = (1.0 + 1.0 / r.powi(6)) * incident[c];
                let bound = rounded(
                    rounded(rounded(direct + f64::from(m.emissive_rgb[c])) + diffuse) + spec,
                );
                if !bound.is_finite() || bound > f64::from(scene.hdr.maximum_rgb[c]) {
                    return Err(error(
                        "complete radiance certificate",
                        "D+emission+E/G source envelope exceeds captured HDR bound",
                    ));
                }
            }
        }
        Ok(())
    }
    pub(crate) fn block(&self, index: usize) -> IndirectBlock112 {
        let mut block = IndirectBlock112::zeroed();
        block.header = [
            u32::from(self.environment.is_some()),
            u32::from(self.gi.is_some()),
            0,
            0,
        ];
        if let Some(g) = self
            .gi
            .as_ref()
            .and_then(|v| v.get(index))
            .and_then(|g| g.as_ref())
        {
            for (dst, rgb) in block.lobes.iter_mut().zip(g.source.lobes) {
                dst[..3].copy_from_slice(&rgb);
            }
        }
        block
    }
}
fn same_arc_option<T>(a: &Option<Arc<T>>, b: &Option<Arc<T>>) -> bool {
    match (a, b) {
        (None, None) => true,
        (Some(a), Some(b)) => Arc::ptr_eq(a, b),
        _ => false,
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct IndirectBlock112 {
    pub(crate) header: [u32; 4],
    pub(crate) lobes: [[f32; 4]; 6],
}
const _: [(); 112] = [(); std::mem::size_of::<IndirectBlock112>()];

pub(crate) fn shader_source(profile: Presence3D) -> String {
    shader_source_with_material(profile, super::pbr::PBR_WGSL)
}
pub(super) fn shader_source_with_material(profile: Presence3D, material: &str) -> String {
    let mut source = if profile.direct() {
        super::direct_shadow::shader_source_with_material(material)
    } else {
        material
            .split("@fragment fn pbr_fragment")
            .next()
            .expect("fixed R024 prefix")
            .to_string()
    };
    source.push_str("\nstruct Indirect112 {header:vec4<u32>,lobes:array<vec4<f32>,6>}\n@group(3) @binding(6) var<uniform> indirect:Indirect112;\n");
    if profile.environment() {
        source.push_str(
            r#"
@group(3) @binding(7) var environment_diffuse:texture_cube<f32>;
@group(3) @binding(8) var environment_specular:texture_cube_array<f32>;
@group(3) @binding(9) var environment_lut:texture_2d<f32>;
@group(3) @binding(10) var environment_sampler:sampler;
"#,
        );
    }
    source.push_str("fn indirect_color(s:Surface,N:vec3<f32>,Vraw:vec3<f32>)->vec3<f32>{\nlet vm=max(max(abs(Vraw.x),abs(Vraw.y)),abs(Vraw.z));if vm<=0.0{return vec3(0.0);}\nlet V=unit_nonzero(Vraw);let nv=clamp(dot(N,V),0.0,1.0);if nv<=0.0{return vec3(0.0);}\nlet f0=(1.0-s.metallic)*vec3(0.04)+s.metallic*s.base;let t=1.0-nv;let t2=t*t;let Fview=f0+(vec3(1.0)-f0)*(t2*t2*t);\n");
    if profile.gi() {
        source.push_str("let Girr=N.x*N.x*indirect.lobes[select(0u,1u,N.x<0.0)].xyz+N.y*N.y*indirect.lobes[select(2u,3u,N.y<0.0)].xyz+N.z*N.z*indirect.lobes[select(4u,5u,N.z<0.0)].xyz;\n");
    } else {
        source.push_str("let Girr=vec3(0.0);\n");
    }
    if profile.environment() {
        source.push_str(r#"
let Eenv=textureSampleLevel(environment_diffuse,environment_sampler,N,0.0).rgb;
let R=reflect(-V,N);let r=max(s.roughness,0.045);var filtered=vec3(0.0);
if r==1.0{filtered=textureSampleLevel(environment_specular,environment_sampler,R,7,0.0).rgb;}
else{let k=7.0*(r-0.045)/0.955;let lo=i32(floor(k));let u=k-f32(lo);
 filtered=(1.0-u)*textureSampleLevel(environment_specular,environment_sampler,R,lo,0.0).rgb+u*textureSampleLevel(environment_specular,environment_sampler,R,lo+1,0.0).rgb;}
let AB=textureSampleLevel(environment_lut,environment_sampler,vec2(nv,r),0.0).rg;
let Espec=filtered*(f0*AB.x+vec3(AB.y));
"#);
    } else {
        source.push_str("let Eenv=vec3(0.0);let Espec=vec3(0.0);\n");
    }
    source.push_str("return s.ao*(1.0-s.metallic)*s.base*(vec3(1.0)-Fview)*(Eenv+Girr)/3.141592653589793+Espec;\n}\n");
    source.push_str("@fragment fn indirect_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32>{\nlet s=surface(i);if material.role_mode.y==1u&&s.alpha<material.mr_ao_cutoff.w{discard;}\nlet f=frame(i,s.normal_sample,front);\n");
    if profile.direct() {
        source.push_str("let direct_rgb=direct_color(i,s,f.N,geometric_normal(i,front));\n");
    } else {
        source.push_str("let direct_rgb=vec3(0.0);\n");
    }
    source.push_str("return vec4(direct_rgb+s.emission+indirect_color(s,f.N,view.camera_relative-i.position),1.0);\n}\n");
    source
}

#[cfg(test)]
pub(super) const PROBE_WGSL: &str = r#"
struct IndirectProbeInput {base_m:vec4<f32>,rough_ao:vec4<f32>,N:vec4<f32>,V:vec4<f32>}
@group(0) @binding(1) var<storage,read> indirect_probe_input:array<IndirectProbeInput>;
@group(0) @binding(2) var<storage,read_write> indirect_probe_output:array<vec4<f32>>;
@compute @workgroup_size(1) fn indirect_probe(@builtin(global_invocation_id) id:vec3<u32>){
 if id.x>=arrayLength(&indirect_probe_input){return;}
 let p=indirect_probe_input[id.x];
 let s=Surface(p.base_m.xyz,1.0,p.base_m.w,p.rough_ao.x,p.rough_ao.y,vec3(0.0),vec3(0.0,0.0,1.0));
 // Observe the same R024 f32 normal qualification as the real material frame.
 indirect_probe_output[id.x]=vec4(indirect_color(s,unit_nonzero(p.N.xyz),p.V.xyz),1.0);
}
"#;
