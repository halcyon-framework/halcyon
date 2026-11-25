//! Private R028 V2 content and sampler owner. Producers supply finished bytes;
//! preparation validates, retains and uploads them, and never generates levels.
use super::context::{
    ControlledResourceKey3D, ControlledResourceKind3D, ControlledResourceLease3D,
    DynamicStorageLease3D, PreparationUploadBudget3D, SamplerGpuLease, SamplerLease,
    TextureGpuLease, TextureGpuResidency, TextureLease,
};
use super::pbr::{PbrAlphaMode3D, PbrError3D, PbrRole3D};
use super::{Context3D, SamplerAddressMode, SamplerFilterMode, TextureFormat3D};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, Once, Weak};

fn error(field: &'static str, cause: &'static str) -> PbrError3D {
    PbrError3D::Sampling { field, cause }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum SamplingFamily3D {
    V1,
    V2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum MipMin3D {
    Nearest,
    Linear,
    NearestMipNearest,
    LinearMipNearest,
    NearestMipLinear,
    LinearMipLinear,
}
impl MipMin3D {
    pub(crate) fn uses_mips(self) -> bool {
        !matches!(self, Self::Nearest | Self::Linear)
    }
    fn within(self) -> wgpu::FilterMode {
        if matches!(
            self,
            Self::Nearest | Self::NearestMipNearest | Self::NearestMipLinear
        ) {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        }
    }
    fn between(self) -> wgpu::MipmapFilterMode {
        if matches!(self, Self::NearestMipLinear | Self::LinearMipLinear) {
            wgpu::MipmapFilterMode::Linear
        } else {
            wgpu::MipmapFilterMode::Nearest
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MipRole3D {
    OpaqueBase,
    MaskBase,
    Normal,
    MetallicRoughness,
    Occlusion,
    Emissive,
    NumericMrAo,
}
impl MipRole3D {
    pub(crate) fn format(self) -> TextureFormat3D {
        if matches!(self, Self::OpaqueBase | Self::MaskBase | Self::Emissive) {
            TextureFormat3D::Rgba8UnormSrgb
        } else {
            TextureFormat3D::Rgba8Unorm
        }
    }
    fn gpu_format(self) -> wgpu::TextureFormat {
        if self.format() == TextureFormat3D::Rgba8UnormSrgb {
            wgpu::TextureFormat::Rgba8UnormSrgb
        } else {
            wgpu::TextureFormat::Rgba8Unorm
        }
    }
    pub(crate) fn matches(self, r: PbrRole3D, a: PbrAlphaMode3D) -> bool {
        match r {
            PbrRole3D::Base => {
                self == if a == PbrAlphaMode3D::Opaque {
                    Self::OpaqueBase
                } else {
                    Self::MaskBase
                }
            }
            PbrRole3D::Normal => self == Self::Normal,
            PbrRole3D::MetallicRoughness => {
                matches!(self, Self::MetallicRoughness | Self::NumericMrAo)
            }
            PbrRole3D::Occlusion => matches!(self, Self::Occlusion | Self::NumericMrAo),
            PbrRole3D::Emissive => self == Self::Emissive,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ImageOrigin3D {
    TopLeft,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChainGenerator3D {
    RecursiveStoredAreaR028V1,
}
#[derive(Debug, Clone)]
pub(crate) struct ChainProvenance3D {
    pub(crate) image_identity: u64,
    pub(crate) content_revision: u64,
    pub(crate) producer_identity: u64,
    pub(crate) producer_revision: u64,
    pub(crate) generator_identity: u64,
    pub(crate) generator_revision: u64,
    pub(crate) generator: Option<ChainGenerator3D>,
    pub(crate) origin: Option<ImageOrigin3D>,
    pub(crate) uv_set: u32,
    pub(crate) work: u64,
    /// Producer's encoded-byte component rounding bound per stored-level step;
    /// this is not a temporal, full-chain or real-asset quality certificate.
    pub(crate) error_bound: f64,
    /// Actual original role bytes, not a validity Boolean or inferred picture.
    pub(crate) original: Arc<[u8]>,
    /// Exact immutable producer output associated with the derivation record.
    pub(crate) derived: Weak<[u8]>,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MipLevel3D {
    pub(crate) width: u32,
    pub(crate) height: u32,
    pub(crate) offset: u64,
    pub(crate) bytes: u64,
}
#[derive(Debug, Clone)]
pub(crate) struct MipChainSource3D {
    pub(crate) role: MipRole3D,
    pub(crate) format: TextureFormat3D,
    pub(crate) levels: Arc<[MipLevel3D]>,
    pub(crate) texels: Arc<[u8]>,
    pub(crate) provenance: Arc<ChainProvenance3D>,
}
impl MipChainSource3D {
    pub(crate) fn complete(&self) -> bool {
        self.levels
            .last()
            .is_some_and(|l| l.width == 1 && l.height == 1)
    }
    pub(crate) fn validate(&self) -> Result<(), PbrError3D> {
        self.validate_structure()?;
        if self.role == MipRole3D::Normal && self.texels.chunks_exact(4).any(|x| x[2] < 128) {
            return Err(error("normal chain", "blue below128 in supplied level"));
        }
        Ok(())
    }
    fn validate_structure(&self) -> Result<(), PbrError3D> {
        self.validate_structure_cpu(&NormalCpuControl3D::synchronous())
            .map_err(NormalCpuFailure3D::synchronous)
    }
    pub(super) fn validate_structure_cpu(
        &self,
        control: &NormalCpuControl3D<'_>,
    ) -> Result<(), NormalCpuFailure3D> {
        control.checkpoint()?;
        let p = &self.provenance;
        if [
            p.image_identity,
            p.content_revision,
            p.producer_identity,
            p.producer_revision,
            p.generator_identity,
            p.generator_revision,
            p.work,
        ]
        .contains(&0)
            || p.generator != Some(ChainGenerator3D::RecursiveStoredAreaR028V1)
            || p.origin != Some(ImageOrigin3D::TopLeft)
            || p.uv_set != 0
            || !p.error_bound.is_finite()
            || p.error_bound < 0.0
            || self.format != self.role.format()
        {
            return Err(error(
                "chain provenance",
                "missing/inconsistent original role/encoding/UV/generator record",
            )
            .into());
        }
        if !Weak::ptr_eq(&p.derived, &Arc::downgrade(&self.texels)) {
            return Err(error(
                "chain provenance",
                "bytes are not the exact captured producer derivation",
            )
            .into());
        }
        let first = self
            .levels
            .first()
            .ok_or(error("chain levels", "missing base level"))?;
        if first.width == 0 || first.height == 0 {
            return Err(error("chain extent", "zero extent").into());
        }
        let (mut w, mut h, mut at) = (first.width, first.height, 0u64);
        for (i, l) in self.levels.iter().enumerate() {
            control.checkpoint()?;
            let bytes = u64::from(w)
                .checked_mul(u64::from(h))
                .and_then(|x| x.checked_mul(4))
                .ok_or(error("chain bytes", "extent overflow"))?;
            if l.width != w || l.height != h || l.offset != at || l.bytes != bytes {
                return Err(error(
                    "chain levels",
                    "wrong dimensions/offset/complete level bytes",
                )
                .into());
            }
            at = at
                .checked_add(bytes)
                .ok_or(error("chain bytes", "sum overflow"))?;
            if i + 1 < self.levels.len() && w == 1 && h == 1 {
                return Err(error("chain levels", "duplicate terminal level").into());
            }
            w = (w / 2).max(1);
            h = (h / 2).max(1);
        }
        if at != self.texels.len() as u64 || (self.levels.len() != 1 && !self.complete()) {
            return Err(error("chain levels", "partial/missing/trailing level content").into());
        }
        let base = self.level_bytes(0)?;
        let mut equal = p.original.len() == base.len();
        if equal {
            for (a, b) in p.original.chunks(4096).zip(base.chunks(4096)) {
                control.checkpoint()?;
                if a != b {
                    equal = false;
                    break;
                }
            }
        }
        if !equal {
            return Err(error(
                "chain original",
                "base bytes disagree with original role source",
            )
            .into());
        }
        Ok(())
    }
    pub(crate) fn level_bytes(&self, i: usize) -> Result<&[u8], PbrError3D> {
        let l = self
            .levels
            .get(i)
            .ok_or(error("chain level", "level out of range"))?;
        let end = l
            .offset
            .checked_add(l.bytes)
            .and_then(|v| usize::try_from(v).ok())
            .ok_or(error("chain bytes", "range overflow"))?;
        self.texels
            .get(l.offset as usize..end)
            .ok_or(error("chain bytes", "level outside source bytes"))
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MipChainHandle3D(pub(super) ControlledResourceKey3D);
struct ChainData3D {
    source: Arc<MipChainSource3D>,
    gpu: Mutex<Weak<MipGpu3D>>,
    negative_normal: bool,
    normal: Option<Arc<VerifiedNormalDomain3D>>,
}
#[derive(Clone)]
pub(crate) struct MipChainLease3D {
    pub(crate) source: Arc<MipChainSource3D>,
    handle: MipChainHandle3D,
    data: Arc<ChainData3D>,
    _ownership: ControlledResourceLease3D,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct MipSamplerSource3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) image_identity: u64,
    pub(crate) image_revision: u64,
    pub(crate) u: SamplerAddressMode,
    pub(crate) v: SamplerAddressMode,
    pub(crate) mag: SamplerFilterMode,
    pub(crate) min: MipMin3D,
    pub(crate) extent: [u32; 2],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct MipSamplerHandle3D(pub(super) ControlledResourceKey3D);
struct SamplerData3D {
    source: MipSamplerSource3D,
    gpu: Mutex<Weak<MipSamplerGpu3D>>,
}
#[derive(Clone)]
pub(crate) struct MipSamplerLease3D {
    pub(crate) source: MipSamplerSource3D,
    handle: MipSamplerHandle3D,
    data: Arc<SamplerData3D>,
    _ownership: ControlledResourceLease3D,
}
pub(super) struct MipGpu3D {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    source: MipChainLease3D,
    uploaded: Once,
    _texture_charge: TextureGpuResidency,
    _metadata: DynamicStorageLease3D,
    // Conservatively retain the complete aligned upload capacity with this
    // actual generation through its Queue-safe last-use custody.
    _upload_capacity: DynamicStorageLease3D,
}
pub(super) struct MipSamplerGpu3D {
    sampler: wgpu::Sampler,
    _source: MipSamplerLease3D,
    _metadata: DynamicStorageLease3D,
}
pub(super) struct MipUpload3D {
    pub(super) gpu: Arc<MipGpu3D>,
}
impl MipUpload3D {
    pub(super) fn commit(self, queue: &wgpu::Queue) {
        self.gpu.uploaded.call_once(|| {
            let source = &self.gpu.source.source;
            for (i, l) in source.levels.iter().enumerate() {
                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.gpu.texture,
                        mip_level: i as u32,
                        origin: wgpu::Origin3d::ZERO,
                        aspect: wgpu::TextureAspect::All,
                    },
                    source.level_bytes(i).expect("qualified full chain"),
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(l.width * 4),
                        rows_per_image: Some(l.height),
                    },
                    wgpu::Extent3d {
                        width: l.width,
                        height: l.height,
                        depth_or_array_layers: 1,
                    },
                );
            }
        });
    }
}
impl Context3D {
    pub(crate) fn register_mip_chain(
        &self,
        source: MipChainSource3D,
        previous: Option<&MipChainHandle3D>,
    ) -> Result<MipChainHandle3D, PbrError3D> {
        source.validate()?;
        self.insert_mip_chain(source, previous, None)
    }
    fn insert_mip_chain(
        &self,
        source: MipChainSource3D,
        previous: Option<&MipChainHandle3D>,
        normal: Option<Arc<VerifiedNormalDomain3D>>,
    ) -> Result<MipChainHandle3D, PbrError3D> {
        self.insert_mip_chain_observed(source, previous, normal, None)
    }
    fn insert_mip_chain_observed(
        &self,
        source: MipChainSource3D,
        previous: Option<&MipChainHandle3D>,
        normal: Option<Arc<VerifiedNormalDomain3D>>,
        checked_negative_normal: Option<bool>,
    ) -> Result<MipChainHandle3D, PbrError3D> {
        let l = source.levels[0];
        if l.width > self.limits().hard.max_texture_dimension_2d
            || l.height > self.limits().hard.max_texture_dimension_2d
        {
            return Err(error(
                "chain Device extent",
                "exceeds actual Device dimensions",
            ));
        }
        super::pbr::budget(
            self,
            super::PolicyBudget3D::TextureDimension2D,
            u64::from(l.width.max(l.height)),
            u64::from(self.limits().policy.max_texture_dimension_2d),
        )?;
        super::pbr::budget(
            self,
            super::PolicyBudget3D::TextureBytes,
            source.texels.len() as u64,
            self.limits().policy.max_texture_bytes,
        )?;
        if let Some(old) = previous {
            let old = self.lease_mip_chain(old)?;
            if old.source.provenance.image_identity != source.provenance.image_identity
                || (normal.is_none()
                    && source.provenance.content_revision <= old.source.provenance.content_revision)
                || source.provenance.content_revision < old.source.provenance.content_revision
                || source.role != old.source.role
            {
                return Err(error(
                    "chain successor",
                    "original image/role must persist and content revision advance",
                ));
            }
            if let Some(next) = &normal {
                if source.provenance.content_revision == old.source.provenance.content_revision
                    && !same_normal_source(&source, &old.source)
                {
                    return Err(error(
                        "normal successor",
                        "changed original/chain behind unchanged content revision",
                    ));
                }
                if let Some(prior) = &old.data.normal {
                    if next.certificate.identity == prior.certificate.identity
                        && next.certificate.revision <= prior.certificate.revision
                    {
                        return Err(error(
                            "normal successor",
                            "certificate revision must advance independently",
                        ));
                    }
                }
            }
        }
        let bytes = (source.texels.len() as u64)
            .checked_add(source.provenance.original.len() as u64)
            .and_then(|b| {
                b.checked_add(
                    (source.levels.len() as u64) * std::mem::size_of::<MipLevel3D>() as u64,
                )
            })
            .and_then(|b| {
                b.checked_add(
                    (std::mem::size_of::<ChainData3D>()
                        + std::mem::size_of::<ChainProvenance3D>()
                        + std::mem::size_of::<MipChainSource3D>()
                        + std::mem::size_of::<MipGpu3D>()) as u64,
                )
            })
            .and_then(|b| b.checked_add(normal.as_ref().map_or(0, |p| p.metadata_bytes)))
            .ok_or(error("chain bytes", "CPU descriptor overflow"))?;
        let negative_normal = checked_negative_normal.unwrap_or_else(|| {
            source.role == MipRole3D::Normal && source.texels.chunks_exact(4).any(|p| p[2] < 128)
        });
        Ok(MipChainHandle3D(self.register_controlled_source(
            ControlledResourceKind3D::MaterialMipChain,
            Arc::new(ChainData3D {
                source: Arc::new(source),
                gpu: Mutex::new(Weak::new()),
                negative_normal,
                normal,
            }),
            bytes,
            previous.map(|p| &p.0),
        )?))
    }
    pub(crate) fn lease_mip_chain(
        &self,
        h: &MipChainHandle3D,
    ) -> Result<MipChainLease3D, PbrError3D> {
        let ownership =
            self.lease_controlled_source(&h.0, ControlledResourceKind3D::MaterialMipChain)?;
        let data = ownership.source::<ChainData3D>()?;
        Ok(MipChainLease3D {
            source: data.source.clone(),
            handle: h.clone(),
            data,
            _ownership: ownership,
        })
    }
    pub(crate) fn retire_mip_chain(&self, h: &MipChainHandle3D) -> Result<(), PbrError3D> {
        self.retire_controlled_source(&h.0, ControlledResourceKind3D::MaterialMipChain)?;
        Ok(())
    }
    pub(crate) fn register_mip_sampler(
        &self,
        source: MipSamplerSource3D,
        previous: Option<&MipSamplerHandle3D>,
    ) -> Result<MipSamplerHandle3D, PbrError3D> {
        if [
            source.identity,
            source.revision,
            source.image_identity,
            source.image_revision,
        ]
        .contains(&0)
            || source.extent.contains(&0)
        {
            return Err(error(
                "sampler provenance",
                "missing authored policy/image generation",
            ));
        }
        if let Some(old) = previous {
            let old = self.lease_mip_sampler(old)?;
            if source.identity != old.source.identity || source.revision <= old.source.revision {
                return Err(error(
                    "sampler successor",
                    "identity must persist and revision advance",
                ));
            }
        }
        Ok(MipSamplerHandle3D(self.register_controlled_source(
            ControlledResourceKind3D::MaterialMipSampler,
            Arc::new(SamplerData3D {
                source,
                gpu: Mutex::new(Weak::new()),
            }),
            std::mem::size_of::<SamplerData3D>() as u64,
            previous.map(|p| &p.0),
        )?))
    }
    pub(crate) fn lease_mip_sampler(
        &self,
        h: &MipSamplerHandle3D,
    ) -> Result<MipSamplerLease3D, PbrError3D> {
        let ownership =
            self.lease_controlled_source(&h.0, ControlledResourceKind3D::MaterialMipSampler)?;
        let data = ownership.source::<SamplerData3D>()?;
        Ok(MipSamplerLease3D {
            source: data.source,
            handle: h.clone(),
            data,
            _ownership: ownership,
        })
    }
    pub(crate) fn retire_mip_sampler(&self, h: &MipSamplerHandle3D) -> Result<(), PbrError3D> {
        self.retire_controlled_source(&h.0, ControlledResourceKind3D::MaterialMipSampler)?;
        Ok(())
    }
    pub(super) fn prepare_mip_chain(
        &self,
        source: &MipChainLease3D,
        upload: &mut PreparationUploadBudget3D,
    ) -> Result<(Arc<MipGpu3D>, Option<MipUpload3D>), PbrError3D> {
        if !source._ownership.belongs_to(self) {
            return Err(super::Context3DError::ContextMismatch.into());
        }
        source.check_normal_capture()?;
        let mut cache = source
            .data
            .gpu
            .lock()
            .map_err(|_| PbrError3D::Lock("mip GPU generation"))?;
        let bytes = source.source.texels.len() as u64;
        let padded = source
            .source
            .levels
            .iter()
            .try_fold(0u64, |sum, l| {
                u64::from(l.width)
                    .checked_mul(4)
                    .and_then(|n| n.checked_add(255))
                    .map(|n| n / 256 * 256)
                    .and_then(|n| n.checked_mul(u64::from(l.height)))
                    .and_then(|n| sum.checked_add(n))
            })
            .ok_or(error("mip upload", "aligned source extent overflow"))?;
        let pending = |gpu: Arc<MipGpu3D>,
                       upload: &mut PreparationUploadBudget3D|
         -> Result<Option<MipUpload3D>, PbrError3D> {
            if gpu.uploaded.is_completed() {
                return Ok(None);
            }
            upload.consume(padded)?;
            Ok(Some(MipUpload3D { gpu }))
        };
        if let Some(gpu) = cache.upgrade() {
            let upload = pending(gpu.clone(), upload)?;
            return Ok((gpu, upload));
        }
        let gpu = self.gpu_resources()?;
        let base = source.source.levels[0];
        let format = source.source.role.gpu_format();
        let features = format.guaranteed_format_features(gpu.device.features());
        if !features
            .allowed_usages
            .contains(wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST)
            || !features
                .flags
                .contains(wgpu::TextureFormatFeatureFlags::FILTERABLE)
        {
            return Err(error(
                "mip Device format",
                "exact filterable material format unsupported",
            ));
        }
        let charge = self.reserve_shadow_texture_bytes(bytes)?;
        let metadata =
            self.reserve_dynamic_storage(0, 0, 0, std::mem::size_of::<MipGpu3D>() as u64)?;
        let upload_capacity = self.reserve_dynamic_storage(0, 0, 0, padded)?;
        let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("R028 complete immutable role mip chain"),
            size: wgpu::Extent3d {
                width: base.width,
                height: base.height,
                depth_or_array_layers: 1,
            },
            mip_level_count: source.source.levels.len() as u32,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        #[cfg(test)]
        super::audit::record_gpu_texture();
        let resident = Arc::new(MipGpu3D {
            texture,
            view,
            source: source.clone(),
            uploaded: Once::new(),
            _texture_charge: charge,
            _metadata: metadata,
            _upload_capacity: upload_capacity,
        });
        let upload = pending(resident.clone(), upload)?;
        *cache = Arc::downgrade(&resident);
        Ok((resident, upload))
    }
    pub(super) fn prepare_mip_sampler(
        &self,
        source: &MipSamplerLease3D,
        chain: &MipChainLease3D,
    ) -> Result<Arc<MipSamplerGpu3D>, PbrError3D> {
        if !source._ownership.belongs_to(self) || !chain._ownership.belongs_to(self) {
            return Err(super::Context3DError::ContextMismatch.into());
        }
        validate_pair(chain, source)?;
        // maxLOD depends on the chain's finite complete extent. The sampler
        // source is image-generation-bound, so all uses have that same extent.
        let mut cache = source
            .data
            .gpu
            .lock()
            .map_err(|_| PbrError3D::Lock("V2 sampler generation"))?;
        if let Some(gpu) = cache.upgrade() {
            return Ok(gpu);
        }
        let s = source.source;
        let address = |x| match x {
            SamplerAddressMode::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            SamplerAddressMode::Repeat => wgpu::AddressMode::Repeat,
            SamplerAddressMode::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        };
        let mag = if s.mag == SamplerFilterMode::Nearest {
            wgpu::FilterMode::Nearest
        } else {
            wgpu::FilterMode::Linear
        };
        let gpu = self.gpu_resources()?;
        let metadata =
            self.reserve_dynamic_storage(0, 0, 0, std::mem::size_of::<MipSamplerGpu3D>() as u64)?;
        let sampler = gpu.device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("R028 independently authored MAG/MIN/U/V"),
            address_mode_u: address(s.u),
            address_mode_v: address(s.v),
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: mag,
            min_filter: s.min.within(),
            mipmap_filter: s.min.between(),
            lod_min_clamp: 0.0,
            lod_max_clamp: if s.min.uses_mips() {
                (chain.source.levels.len() - 1) as f32
            } else {
                // Native LOD clamping precedes min/MAG selection: max=0
                // incorrectly forces MAG even for a minifying footprint.
                // With NEAREST mip selection, every LOD in [0,1/4] selects
                // exactly authored level0; positive LOD still selects MIN.
                // This is the native GL_NEAREST/GL_LINEAR mapping, not an
                // authored bias, changed footprint, mip fallback or epsilon.
                // https://github.khronos.org/Vulkan-Site/spec/latest/chapters/samplers.html
                0.25
            },
            anisotropy_clamp: 1,
            compare: None,
            ..Default::default()
        });
        let resident = Arc::new(MipSamplerGpu3D {
            sampler,
            _source: source.clone(),
            _metadata: metadata,
        });
        *cache = Arc::downgrade(&resident);
        Ok(resident)
    }
}
pub(crate) fn validate_pair(t: &MipChainLease3D, s: &MipSamplerLease3D) -> Result<(), PbrError3D> {
    t.check_normal_capture()?;
    if let Some(proof) = &t.data.normal {
        if proof.sampler.handle != s.handle || proof.sampler.source != s.source {
            return Err(error(
                "normal sampler",
                "certified sampler generation/policy does not match",
            ));
        }
    }
    if t.source.provenance.image_identity != s.source.image_identity
        || t.source.provenance.content_revision != s.source.image_revision
    {
        return Err(error(
            "role source/sampler",
            "authored image/content generation mismatch",
        ));
    }
    if s.source.extent != [t.source.levels[0].width, t.source.levels[0].height] {
        return Err(error(
            "role source/sampler",
            "authored original extent mismatch",
        ));
    }
    if s.source.min.uses_mips() && !t.source.complete() {
        return Err(error("role sampler chain", "mip MIN requires every level"));
    }
    Ok(())
}

impl MipChainLease3D {
    fn check_normal_capture(&self) -> Result<(), PbrError3D> {
        if !Arc::ptr_eq(&self.source, &self.data.source) {
            return Err(error(
                "normal source",
                "lease source is not the captured resource referent",
            ));
        }
        if self.data.negative_normal && self.data.normal.is_none() {
            return Err(error(
                "normal source",
                "negative BLUE has no checked source capture",
            ));
        }
        Ok(())
    }
    pub(crate) fn validate_normal_material(
        &self,
        sampler: &MipSamplerLease3D,
        scale: f32,
    ) -> Result<(), PbrError3D> {
        validate_pair(self, sampler)?;
        if let Some(proof) = &self.data.normal {
            if proof.certificate.normal_scale_bits != scale.to_bits() {
                return Err(error(
                    "normal scale",
                    "material bits differ from the certified original scale",
                ));
            }
        }
        Ok(())
    }
    pub(crate) fn normal_domain_summary(&self) -> Option<NormalDomainSummary3D> {
        self.data.normal.as_ref().map(|p| p.summary)
    }
    #[cfg(test)]
    pub(crate) fn normal_cpu_record_for_test(&self) -> Option<([u8; 32], bool)> {
        self.data
            .normal
            .as_ref()
            .map(|p| (p.certificate.digest, self.data.negative_normal))
    }
}

/// Version1 of a finite correlated-domain SOURCE certificate. These values
/// never become shader inputs. Branch/cell coordinates name the independently
/// enumerated joint partition; nearest point/tie branches have explicit choices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct NormalCell3D {
    pub(crate) mag: bool,
    pub(crate) linear: bool,
    pub(crate) lower: u32,
    pub(crate) upper: u32,
    pub(crate) u: u32,
    pub(crate) v: u32,
    pub(crate) u_choice: u8,
    pub(crate) v_choice: u8,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalWitness3D {
    /// Mathematical unit interpretation of this finite nonzero direction;
    /// actual normalization is enclosed, not asserted exact in binary64.
    pub(crate) direction: [f64; 3],
    pub(crate) lower: f64,
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalBox3D {
    /// Octree Morton prefix, at most20 levels/60 bits, in u,v,fractional LOD.
    pub(crate) path: u64,
    pub(crate) depth: u8,
    pub(crate) witness: NormalWitness3D,
}
#[derive(Debug, Clone)]
pub(crate) struct NormalException3D {
    pub(crate) cell: NormalCell3D,
    /// Sorted, prefix-free complete partition of this cell, not sample points.
    pub(crate) leaves: Arc<[NormalBox3D]>,
}
#[derive(Clone)]
pub(crate) struct NormalDomainCertificate3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) source: Arc<MipChainSource3D>,
    pub(crate) sampler: MipSamplerHandle3D,
    pub(crate) policy: MipSamplerSource3D,
    pub(crate) normal_scale_bits: u32,
    pub(crate) cells: u64,
    pub(crate) maximum_work: u64,
    pub(crate) maximum_metadata_bytes: u64,
    pub(crate) default: NormalWitness3D,
    pub(crate) exceptions: Arc<[NormalException3D]>,
    pub(crate) digest: [u8; 32],
}
#[derive(Debug, Clone, Copy)]
pub(crate) struct NormalDomainSummary3D {
    pub(crate) cells: u64,
    pub(crate) leaves: u64,
    pub(crate) work: u64,
    pub(crate) minimum_projection: f64,
    pub(crate) maximum_interval_width: f64,
}
struct VerifiedNormalDomain3D {
    certificate: Arc<NormalDomainCertificate3D>,
    sampler: MipSamplerLease3D,
    summary: NormalDomainSummary3D,
    metadata_bytes: u64,
}

/// CPU-only cancellation and observation. No Context or accounting lease crosses
/// this boundary. Checkpoints do not debit or reset the original proof work.
pub(super) struct NormalCpuControl3D<'a> {
    cancel: Option<&'a AtomicBool>,
    pub(super) work: NormalCpuWorkObservation3D,
}

// The required cumulative-work observation is not a second work owner: only
// NormalWork3D debits it. Relaxed observation needs no payload synchronization;
// cancellation uses its own release/acquire flag. This can safely be inspected
// by the owning proof tests while the same kernel is executing.
pub(super) struct NormalCpuWorkObservation3D(AtomicU64);
impl NormalCpuWorkObservation3D {
    pub(super) fn get(&self) -> u64 {
        self.0.load(Ordering::Relaxed)
    }
    pub(super) fn set(&self, work: u64) {
        self.0.store(work, Ordering::Relaxed);
    }
}

impl<'a> NormalCpuControl3D<'a> {
    pub(super) fn new(cancel: &'a AtomicBool) -> Self {
        Self {
            cancel: Some(cancel),
            work: NormalCpuWorkObservation3D(AtomicU64::new(0)),
        }
    }
    pub(super) fn synchronous() -> Self {
        Self {
            cancel: None,
            work: NormalCpuWorkObservation3D(AtomicU64::new(0)),
        }
    }
    pub(super) fn checkpoint(&self) -> Result<(), NormalCpuFailure3D> {
        if self.cancel.is_some_and(|c| c.load(Ordering::Acquire)) {
            Err(NormalCpuFailure3D::Cancelled)
        } else {
            Ok(())
        }
    }
}

#[derive(Debug)]
pub(super) enum NormalCpuFailure3D {
    Cancelled,
    Source(PbrError3D),
}
impl From<PbrError3D> for NormalCpuFailure3D {
    fn from(error: PbrError3D) -> Self {
        Self::Source(error)
    }
}
impl From<super::Context3DError> for NormalCpuFailure3D {
    fn from(error: super::Context3DError) -> Self {
        Self::Source(error.into())
    }
}
impl NormalCpuFailure3D {
    pub(super) fn synchronous(self) -> PbrError3D {
        match self {
            Self::Source(error) => error,
            Self::Cancelled => unreachable!("synchronous proof has no cancellation source"),
        }
    }
}

/// The mathematical body with a VALUE sampler binding. The historical
/// handle-bearing certificate remains unchanged, including its size_of debit.
pub(super) struct NormalCpuCertificate3D {
    pub(super) identity: u64,
    pub(super) revision: u64,
    pub(super) source: Arc<MipChainSource3D>,
    pub(super) sampler_slot: u32,
    pub(super) sampler_generation: u64,
    pub(super) policy: MipSamplerSource3D,
    pub(super) normal_scale_bits: u32,
    pub(super) cells: u64,
    pub(super) maximum_work: u64,
    pub(super) maximum_metadata_bytes: u64,
    pub(super) default: NormalWitness3D,
    pub(super) exceptions: Arc<[NormalException3D]>,
    pub(super) digest: [u8; 32],
    // Only the HAL adapter can supply this observation of the SAME immutable
    // captured body. The digest equality and entire mathematical proof still run.
    pub(super) observed_digest: Option<[u8; 32]>,
    observed_metadata: Option<u64>,
}

impl NormalDomainCertificate3D {
    fn cpu_body(&self) -> NormalCpuCertificate3D {
        let (sampler_slot, sampler_generation) = self.sampler.0.slot_generation();
        NormalCpuCertificate3D {
            identity: self.identity,
            revision: self.revision,
            source: self.source.clone(),
            sampler_slot,
            sampler_generation,
            policy: self.policy,
            normal_scale_bits: self.normal_scale_bits,
            cells: self.cells,
            maximum_work: self.maximum_work,
            maximum_metadata_bytes: self.maximum_metadata_bytes,
            default: self.default,
            exceptions: self.exceptions.clone(),
            digest: self.digest,
            observed_digest: None,
            observed_metadata: None,
        }
    }
    pub(crate) fn computed_digest(&self) -> [u8; 32] {
        self.cpu_body()
            .computed_digest(&NormalCpuControl3D::synchronous())
            .expect("uncancelled checksum has no fallible source operation")
    }
    fn metadata_bytes(&self) -> Result<u64, PbrError3D> {
        self.cpu_body()
            .metadata_bytes(&NormalCpuControl3D::synchronous())
            .map_err(NormalCpuFailure3D::synchronous)
    }
}

fn same_normal_source(a: &MipChainSource3D, b: &MipChainSource3D) -> bool {
    a.role == b.role
        && a.format == b.format
        && Arc::ptr_eq(&a.levels, &b.levels)
        && Arc::ptr_eq(&a.texels, &b.texels)
        && Arc::ptr_eq(&a.provenance, &b.provenance)
}

impl NormalCpuCertificate3D {
    /// Deterministic SOURCE-body digest, not an issuer or a proof generator.
    /// Admission separately recomputes every supplied inequality and coverage.
    pub(super) fn computed_digest(
        &self,
        control: &NormalCpuControl3D<'_>,
    ) -> Result<[u8; 32], NormalCpuFailure3D> {
        use sha2::{Digest, Sha256};
        let mut h = Sha256::new();
        h.update(b"R028 common-UV domain corners v1");
        let p = &self.source.provenance;
        let (slot, generation) = (self.sampler_slot, self.sampler_generation);
        for n in [
            self.identity,
            self.revision,
            p.image_identity,
            p.content_revision,
            p.producer_identity,
            p.producer_revision,
            p.generator_identity,
            p.generator_revision,
            p.work,
            p.error_bound.to_bits(),
            u64::from(p.uv_set),
            u64::from(slot),
            generation,
            self.policy.identity,
            self.policy.revision,
            self.policy.image_identity,
            self.policy.image_revision,
            u64::from(self.policy.extent[0]),
            u64::from(self.policy.extent[1]),
            self.policy.u as u64,
            self.policy.v as u64,
            self.policy.mag as u64,
            self.policy.min as u64,
            u64::from(self.normal_scale_bits),
            self.cells,
            self.maximum_work,
            self.maximum_metadata_bytes,
        ] {
            h.update(n.to_le_bytes());
        }
        for level in self.source.levels.iter() {
            control.checkpoint()?;
            for n in [
                u64::from(level.width),
                u64::from(level.height),
                level.offset,
                level.bytes,
            ] {
                h.update(n.to_le_bytes());
            }
        }
        // SHA update chunk boundaries do not alter the canonical byte stream.
        for bytes in [&p.original, &self.source.texels] {
            for chunk in bytes.chunks(4096) {
                control.checkpoint()?;
                h.update(chunk);
            }
        }
        let witness = |h: &mut Sha256, w: NormalWitness3D| {
            for x in w.direction.into_iter().chain([w.lower]) {
                h.update(x.to_bits().to_le_bytes());
            }
        };
        witness(&mut h, self.default);
        h.update((self.exceptions.len() as u64).to_le_bytes());
        for e in self.exceptions.iter() {
            control.checkpoint()?;
            let c = e.cell;
            for x in [
                c.mag as u64,
                c.linear as u64,
                u64::from(c.lower),
                u64::from(c.upper),
                u64::from(c.u),
                u64::from(c.v),
                u64::from(c.u_choice),
                u64::from(c.v_choice),
                e.leaves.len() as u64,
            ] {
                h.update(x.to_le_bytes());
            }
            for leaf in e.leaves.iter() {
                control.checkpoint()?;
                h.update(leaf.path.to_le_bytes());
                h.update([leaf.depth]);
                witness(&mut h, leaf.witness);
            }
        }
        control.checkpoint()?;
        Ok(h.finalize().into())
    }
    pub(super) fn metadata_bytes(
        &self,
        control: &NormalCpuControl3D<'_>,
    ) -> Result<u64, NormalCpuFailure3D> {
        if let Some(bytes) = self.observed_metadata {
            return Ok(bytes);
        }
        let mut bytes = (std::mem::size_of::<NormalDomainCertificate3D>()
            + std::mem::size_of::<VerifiedNormalDomain3D>()
            + std::mem::size_of::<MipChainSource3D>()
            + 12 * std::mem::size_of::<usize>()) as u64;
        bytes = bytes
            .checked_add(
                (self.exceptions.len() as u64)
                    .checked_mul(std::mem::size_of::<NormalException3D>() as u64)
                    .ok_or(error(
                        "normal certificate bytes",
                        "exception storage overflow",
                    ))?,
            )
            .ok_or(error("normal certificate bytes", "storage overflow"))?;
        for e in self.exceptions.iter() {
            control.checkpoint()?;
            bytes = bytes
                .checked_add(
                    (e.leaves.len() as u64)
                        .checked_mul(std::mem::size_of::<NormalBox3D>() as u64)
                        .and_then(|x| x.checked_add((2 * std::mem::size_of::<usize>()) as u64))
                        .ok_or(error("normal certificate bytes", "leaf storage overflow"))?,
                )
                .ok_or(error("normal certificate bytes", "storage overflow"))?;
        }
        if bytes > self.maximum_metadata_bytes {
            return Err(error(
                "normal certificate bytes",
                "declared retained storage bound exceeded",
            )
            .into());
        }
        Ok(bytes)
    }
}

#[derive(Clone, Copy)]
pub(super) struct NormalCpuLimits3D {
    pub(super) hard_dimension: u32,
    pub(super) policy_dimension: u32,
    pub(super) texture_bytes: u64,
}

fn normal_association(
    source: &MipChainSource3D,
    actual_policy: MipSamplerSource3D,
    scale: f32,
    certificate: &NormalCpuCertificate3D,
    same_sampler: bool,
    limits: NormalCpuLimits3D,
) -> Result<(), PbrError3D> {
    if source.role != MipRole3D::Normal
        || source.format != TextureFormat3D::Rgba8Unorm
        || !scale.is_finite()
        || !(-1024.0..=1024.0).contains(&scale)
        || certificate.identity == 0
        || certificate.revision == 0
        || certificate.normal_scale_bits != scale.to_bits()
        || !same_sampler
        || certificate.policy != actual_policy
        || !same_normal_source(source, &certificate.source)
    {
        return Err(error(
            "normal certificate association",
            "wrong role/source/sampler/scale or missing certificate generation",
        ));
    }
    let base = source.levels[0];
    if actual_policy.image_identity != source.provenance.image_identity
        || actual_policy.image_revision != source.provenance.content_revision
        || actual_policy.extent != [base.width, base.height]
        || actual_policy.min.uses_mips() && !source.complete()
    {
        return Err(error(
            "normal sampler",
            "image/extent/complete-chain association mismatch",
        ));
    }
    if base.width > limits.hard_dimension || base.height > limits.hard_dimension {
        return Err(error(
            "normal Device extent",
            "exceeds actual Device dimensions",
        ));
    }
    super::context::validate_policy_budget(
        super::PolicyBudget3D::TextureDimension2D,
        u64::from(base.width.max(base.height)),
        u64::from(limits.policy_dimension),
    )?;
    super::context::validate_policy_budget(
        super::PolicyBudget3D::TextureBytes,
        source.texels.len() as u64,
        limits.texture_bytes,
    )?;
    Ok(())
}

pub(super) fn normal_workspace_bytes(
    width: u32,
    height: u32,
    levels: usize,
) -> Result<u64, PbrError3D> {
    (u64::from(width) + u64::from(height))
        .checked_mul(2)
        .and_then(|x| x.checked_add(8))
        .and_then(|x| x.checked_mul(std::mem::size_of::<NormalFraction3D>() as u64))
        .and_then(|x| {
            (levels as u64)
                .checked_add(2)
                .and_then(|n| n.checked_mul(std::mem::size_of::<NormalBranch3D>() as u64))
                .and_then(|n| x.checked_add(n))
        })
        .ok_or(error("normal verification workspace", "size overflow"))
}

pub(super) struct NormalCpuVerified3D {
    pub(super) certificate: NormalCpuCertificate3D,
    pub(super) summary: NormalDomainSummary3D,
    metadata_bytes: u64,
    negative_normal: bool,
}

pub(super) fn normal_cpu_fixed_storage_bytes() -> u64 {
    (std::mem::size_of::<NormalDomainCertificate3D>()
        + std::mem::size_of::<VerifiedNormalDomain3D>()
        + std::mem::size_of::<ChainData3D>()
        + 2 * std::mem::size_of::<MipChainSource3D>()
        + std::mem::size_of::<ChainProvenance3D>()) as u64
}

impl NormalCpuCertificate3D {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        identity: u64,
        revision: u64,
        source: Arc<MipChainSource3D>,
        sampler: (u32, u64),
        policy: MipSamplerSource3D,
        scale_bits: u32,
        cells: u64,
        maximum_work: u64,
        maximum_metadata_bytes: u64,
        default: NormalWitness3D,
        exceptions: Arc<[NormalException3D]>,
    ) -> Self {
        Self {
            identity,
            revision,
            source,
            sampler_slot: sampler.0,
            sampler_generation: sampler.1,
            policy,
            normal_scale_bits: scale_bits,
            cells,
            maximum_work,
            maximum_metadata_bytes,
            default,
            exceptions,
            digest: [0; 32],
            observed_digest: None,
            observed_metadata: None,
        }
    }
}

pub(super) fn verify_normal_cpu_source(
    mut certificate: NormalCpuCertificate3D,
    actual_policy: MipSamplerSource3D,
    scale: f32,
    limits: NormalCpuLimits3D,
    control: &NormalCpuControl3D<'_>,
) -> Result<NormalCpuVerified3D, NormalCpuFailure3D> {
    // Same canonical adapter checksum, before the original private admission
    // predicates. Its observation stays bound to this immutable HAL body.
    certificate.digest = certificate.computed_digest(control)?;
    certificate.observed_digest = Some(certificate.digest);
    let source = certificate.source.clone();
    source.validate_structure_cpu(control)?;
    normal_association(&source, actual_policy, scale, &certificate, true, limits)?;
    let metadata_bytes = certificate.metadata_bytes(control)?;
    certificate.observed_metadata = Some(metadata_bytes);
    let base = source.levels[0];
    normal_workspace_bytes(base.width, base.height, source.levels.len())?;
    let summary = verify_normal_domain_cpu::<true, true, true, true>(
        &source,
        &actual_policy,
        scale,
        &certificate,
        control,
    )?;
    // This is an observation, not an earlier refusal: insert_mip_chain used
    // exactly this predicate only after successor and byte validation.
    let mut negative_normal = false;
    for chunk in source.texels.chunks(4096) {
        control.checkpoint()?;
        negative_normal |= chunk.chunks_exact(4).any(|p| p[2] < 128);
    }
    control.checkpoint()?;
    Ok(NormalCpuVerified3D {
        certificate,
        summary,
        metadata_bytes,
        negative_normal,
    })
}

impl Context3D {
    /// Borrow the successful CPU witness so an insertion refusal cannot destroy
    /// the last large proof allocation on UI. The returned error keeps its CPU
    /// result with the caller for worker-side disposal.
    pub(super) fn finish_normal_cpu_source(
        &self,
        verified: &NormalCpuVerified3D,
        sampler: &MipSamplerHandle3D,
        previous: Option<&MipChainHandle3D>,
    ) -> Result<MipChainHandle3D, PbrError3D> {
        let live = self.lease_mip_sampler(sampler)?;
        let body = &verified.certificate;
        if live.source != body.policy
            || sampler.0.slot_generation() != (body.sampler_slot, body.sampler_generation)
        {
            return Err(error(
                "normal certificate association",
                "wrong role/source/sampler/scale or missing certificate generation",
            ));
        }
        let certificate = Arc::new(NormalDomainCertificate3D {
            identity: body.identity,
            revision: body.revision,
            source: body.source.clone(),
            sampler: sampler.clone(),
            policy: body.policy,
            normal_scale_bits: body.normal_scale_bits,
            cells: body.cells,
            maximum_work: body.maximum_work,
            maximum_metadata_bytes: body.maximum_metadata_bytes,
            default: body.default,
            exceptions: body.exceptions.clone(),
            digest: body.digest,
        });
        self.insert_mip_chain_observed(
            body.source.as_ref().clone(),
            previous,
            Some(Arc::new(VerifiedNormalDomain3D {
                certificate,
                sampler: live,
                summary: verified.summary,
                metadata_bytes: verified.metadata_bytes,
            })),
            Some(verified.negative_normal),
        )
    }

    pub(crate) fn register_certified_normal_chain(
        &self,
        source: MipChainSource3D,
        sampler: &MipSamplerHandle3D,
        normal_scale: f32,
        certificate: Arc<NormalDomainCertificate3D>,
        previous: Option<&MipChainHandle3D>,
    ) -> Result<MipChainHandle3D, PbrError3D> {
        source.validate_structure()?;
        let s = self.lease_mip_sampler(sampler)?;
        normal_association(
            &source,
            s.source,
            normal_scale,
            &certificate.cpu_body(),
            certificate.sampler == *sampler,
            NormalCpuLimits3D {
                hard_dimension: self.limits().hard.max_texture_dimension_2d,
                policy_dimension: self.limits().policy.max_texture_dimension_2d,
                texture_bytes: self.limits().policy.max_texture_bytes,
            },
        )?;
        let base = source.levels[0];
        let metadata_bytes = certificate.metadata_bytes()?;
        // Bound the two joint-axis vectors and branch vector BEFORE allocating
        // verifier scratch; fixed-size scalar/corner arithmetic stays on stack.
        let workspace = normal_workspace_bytes(base.width, base.height, source.levels.len())?;
        let scratch = self.reserve_dynamic_storage(0, 0, 0, workspace)?;
        let summary = verify_normal_domain::<true, true, true, true>(
            &source,
            &s.source,
            normal_scale,
            &certificate,
        )?;
        drop(scratch);
        self.insert_mip_chain(
            source,
            previous,
            Some(Arc::new(VerifiedNormalDomain3D {
                certificate,
                sampler: s,
                summary,
                metadata_bytes,
            })),
        )
    }
}

struct NormalWork3D<'a> {
    used: u64,
    maximum: u64,
    control: &'a NormalCpuControl3D<'a>,
}
impl NormalWork3D<'_> {
    fn take(&mut self, n: u64) -> Result<(), NormalCpuFailure3D> {
        self.control.checkpoint()?;
        self.used = self
            .used
            .checked_add(n)
            .ok_or(error("normal verification work", "counter overflow"))?;
        self.control.work.set(self.used);
        if self.used > self.maximum {
            return Err(error("normal verification work", "declared finite bound exceeded").into());
        }
        Ok(())
    }
}
fn normal_cell_error(c: NormalCell3D, cause: &'static str) -> PbrError3D {
    PbrError3D::NormalCertificate {
        levels: [c.lower, c.upper],
        cell: [c.u, c.v, u32::from(c.u_choice), u32::from(c.v_choice)],
        mag: c.mag,
        cause,
    }
}

#[cfg(test)]
mod hal373_normal_cpu_math_tests {
    use super::*;

    fn source() -> MipChainSource3D {
        let texels: Arc<[u8]> = vec![255, 128, 127, 255].into();
        MipChainSource3D {
            role: MipRole3D::Normal,
            format: TextureFormat3D::Rgba8Unorm,
            levels: vec![MipLevel3D {
                width: 1,
                height: 1,
                offset: 0,
                bytes: 4,
            }]
            .into(),
            texels: texels.clone(),
            provenance: Arc::new(ChainProvenance3D {
                image_identity: 373100,
                content_revision: 1,
                producer_identity: 1,
                producer_revision: 1,
                generator_identity: 1,
                generator_revision: 1,
                generator: Some(ChainGenerator3D::RecursiveStoredAreaR028V1),
                origin: Some(ImageOrigin3D::TopLeft),
                uv_set: 0,
                work: 1,
                error_bound: 0.0,
                original: texels.clone(),
                derived: Arc::downgrade(&texels),
            }),
        }
    }
    fn policy() -> MipSamplerSource3D {
        MipSamplerSource3D {
            identity: 373101,
            revision: 1,
            image_identity: 373100,
            image_revision: 1,
            u: SamplerAddressMode::ClampToEdge,
            v: SamplerAddressMode::ClampToEdge,
            mag: SamplerFilterMode::Linear,
            min: MipMin3D::Linear,
            extent: [1, 1],
        }
    }
    #[test]
    fn hal373_normal_cpu_grid_exact_order_and_cancellation() {
        let control = NormalCpuControl3D::synchronous();
        let widths = (1..=17).chain([31, 64]).collect::<Vec<_>>();
        for &a in &widths {
            for &b in &widths {
                for linear in [false, true] {
                    let mut old = vec![NormalFraction3D::new(0, 1), NormalFraction3D::new(1, 1)];
                    for w in [a, b] {
                        if linear {
                            for j in 0..w {
                                old.push(NormalFraction3D::new(
                                    2 * u64::from(j) + 1,
                                    2 * u64::from(w),
                                ));
                            }
                        } else {
                            for j in 1..w {
                                old.push(NormalFraction3D::new(u64::from(j), u64::from(w)));
                            }
                        }
                    }
                    old.sort_unstable_by(|x, y| {
                        (u128::from(x.n) * u128::from(y.d))
                            .cmp(&(u128::from(y.n) * u128::from(x.d)))
                    });
                    old.dedup();
                    let new = normal_grid(a, b, linear, &control).unwrap();
                    assert_eq!(
                        new.iter().map(|x| (x.n, x.d)).collect::<Vec<_>>(),
                        old.iter().map(|x| (x.n, x.d)).collect::<Vec<_>>()
                    );
                }
            }
        }
        let flag = AtomicBool::new(true);
        assert!(matches!(
            normal_grid(64, 31, true, &NormalCpuControl3D::new(&flag)),
            Err(NormalCpuFailure3D::Cancelled)
        ));
    }

    #[test]
    fn hal373_normal_cpu_shared_kernel_bits_and_phase_checkpoints() {
        let context = Context3D::new_cpu_only_for_test(
            super::super::PolicyLimits3D::conservative(),
            u64::MAX,
        )
        .unwrap();
        let source = source();
        let policy = policy();
        let sampler = context.register_mip_sampler(policy, None).unwrap();
        let mut original = NormalDomainCertificate3D {
            identity: 373102,
            revision: 1,
            source: Arc::new(source.clone()),
            sampler,
            policy,
            normal_scale_bits: 1.0f32.to_bits(),
            cells: 8,
            maximum_work: 1_000_000,
            maximum_metadata_bytes: 65_536,
            default: NormalWitness3D {
                direction: [1.0, 0.0, 0.0],
                lower: 0.003,
            },
            exceptions: Arc::from([]),
            digest: [0; 32],
        };
        original.digest = original.computed_digest();
        let old =
            verify_normal_domain::<false, false, false, false>(&source, &policy, 1.0, &original)
                .unwrap();
        let control = NormalCpuControl3D::synchronous();
        let verified = verify_normal_cpu_source(
            original.cpu_body(),
            policy,
            1.0,
            NormalCpuLimits3D {
                hard_dimension: 8192,
                policy_dimension: 8192,
                texture_bytes: 1_048_576,
            },
            &control,
        )
        .unwrap();
        assert_eq!(
            (
                old.cells,
                old.leaves,
                old.work,
                old.minimum_projection.to_bits(),
                old.maximum_interval_width.to_bits()
            ),
            (
                verified.summary.cells,
                verified.summary.leaves,
                verified.summary.work,
                verified.summary.minimum_projection.to_bits(),
                verified.summary.maximum_interval_width.to_bits()
            )
        );
        assert_eq!(verified.certificate.digest, original.digest);
        let mut forged = original.cpu_body();
        forged.digest[0] ^= 1;
        let error = verify_normal_domain_cpu::<true, true, true, true>(
            &source, &policy, 1.0, &forged, &control,
        )
        .unwrap_err();
        assert!(matches!(
            error,
            NormalCpuFailure3D::Source(PbrError3D::Sampling {
                field: "normal certificate digest",
                ..
            })
        ));

        let flag = AtomicBool::new(false);
        let control = NormalCpuControl3D::new(&flag);
        let body = original.cpu_body();
        let grid = normal_grid(1, 1, true, &control).unwrap();
        let (axis, _) = normal_axis(&grid, 0, [1, 1], true, policy.u, true);
        let leaf = NormalBox3D {
            path: 0,
            depth: 0,
            witness: original.default,
        };
        let h = normal_witness(original.default).unwrap();
        let mut summary = NormalDomainSummary3D {
            cells: 1,
            leaves: 0,
            work: 0,
            minimum_projection: f64::INFINITY,
            maximum_interval_width: 0.0,
        };
        let mut work = NormalWork3D {
            used: 7,
            maximum: 100,
            control: &control,
        };
        control.work.set(7);
        flag.store(true, Ordering::Release);
        assert!(matches!(
            body.computed_digest(&control),
            Err(NormalCpuFailure3D::Cancelled)
        ));
        let key = NormalCell3D {
            mag: false,
            linear: true,
            lower: 0,
            upper: 0,
            u: 0,
            v: 0,
            u_choice: 0,
            v_choice: 0,
        };
        assert!(matches!(
            check_normal_box::<true, true, true>(
                &source,
                &policy,
                1.0,
                &NormalByteIntervals3D::new(1.0),
                key,
                axis[0],
                axis[0],
                leaf,
                None,
                h,
                &mut summary,
                &mut work
            ),
            Err(NormalCpuFailure3D::Cancelled)
        ));
        assert_eq!(work.used, 7);
        assert_eq!(control.work.get(), 7);
        assert_eq!(summary.leaves, 0, "no partial certified leaf");
    }

    #[test]
    fn hal373_normal_cpu_cancel_after_corner_work_started() {
        let mut source = source();
        let width = 1024u32;
        let texels: Arc<[u8]> = [255, 128, 127, 255].repeat((width * width) as usize).into();
        source.levels = vec![MipLevel3D {
            width,
            height: width,
            offset: 0,
            bytes: texels.len() as u64,
        }]
        .into();
        source.texels = texels.clone();
        let p = Arc::get_mut(&mut source.provenance).unwrap();
        p.original = texels.clone();
        p.derived = Arc::downgrade(&texels);
        let policy = MipSamplerSource3D {
            extent: [width, width],
            ..policy()
        };
        let mut body = NormalCpuCertificate3D::new(
            373103,
            1,
            Arc::new(source.clone()),
            (0, 1),
            policy,
            1.0f32.to_bits(),
            2 * (u64::from(width) + 1).pow(2),
            100_000_000,
            65_536,
            NormalWitness3D {
                direction: [1.0, 0.0, 0.0],
                lower: 0.003,
            },
            Arc::from([]),
        );
        body.digest = body
            .computed_digest(&NormalCpuControl3D::synchronous())
            .unwrap();
        let initial = source.texels.len() as u64
            + source.provenance.original.len() as u64
            + body
                .metadata_bytes(&NormalCpuControl3D::synchronous())
                .unwrap();
        let flag = AtomicBool::new(false);
        let control = NormalCpuControl3D::new(&flag);
        let start = std::sync::Barrier::new(2);
        std::thread::scope(|scope| {
            let worker = scope.spawn(|| {
                start.wait();
                verify_normal_domain_cpu::<true, true, true, true>(
                    &source, &policy, 1.0, &body, &control,
                )
            });
            start.wait();
            while control.work.get() <= initial + 2 {
                assert!(
                    !worker.is_finished(),
                    "source proof exited before its corner-work checkpoint"
                );
                std::thread::yield_now();
            }
            // This is actual cumulative source work from the same kernel,
            // not a synthetic phase ID, timer, sleep or test callback.
            flag.store(true, Ordering::Release);
            assert!(matches!(
                worker.join().unwrap(),
                Err(NormalCpuFailure3D::Cancelled)
            ));
        });
        assert!(control.work.get() > initial + 2);
    }
}

/// Exact rational breakpoints. Address periods reduce to [0,1]; Mirror's other
/// half is its affine reflection, and Clamp tails equal the two endpoints.
#[derive(Clone, Copy, PartialEq, Eq)]
struct NormalFraction3D {
    n: u64,
    d: u64,
}
impl NormalFraction3D {
    fn new(n: u64, d: u64) -> Self {
        let (mut a, mut b) = (n, d);
        while b != 0 {
            let r = a % b;
            a = b;
            b = r;
        }
        Self { n: n / a, d: d / a }
    }
    fn interval(self) -> Interval {
        Interval::point(self.n as f64)
            .div(Interval::point(self.d as f64))
            .expect("positive finite bounded grid denominator")
    }
}
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct NormalBranch3D {
    mag: bool,
    linear: bool,
    lower: u32,
    upper: u32,
}
fn normal_grid(
    a: u32,
    b: u32,
    linear: bool,
    control: &NormalCpuControl3D<'_>,
) -> Result<Vec<NormalFraction3D>, NormalCpuFailure3D> {
    let capacity = usize::try_from(u64::from(a) + u64::from(b) + 4)
        .map_err(|_| error("normal grid", "unaddressable grid"))?;
    let mut v = Vec::new();
    v.try_reserve_exact(capacity)
        .map_err(|_| error("normal grid", "bounded grid allocation unavailable"))?;
    v.extend([NormalFraction3D::new(0, 1), NormalFraction3D::new(1, 1)]);
    for w in [a, b] {
        if linear {
            for j in 0..w {
                control.checkpoint()?;
                v.push(NormalFraction3D::new(
                    2 * u64::from(j) + 1,
                    2 * u64::from(w),
                ));
            }
        } else {
            for j in 1..w {
                control.checkpoint()?;
                v.push(NormalFraction3D::new(u64::from(j), u64::from(w)));
            }
        }
    }
    // Equal fractions have identical reduced representations. In-place sorting
    // therefore preserves the canonical grid without uncharged merge scratch.
    // In-place exact ordering, with a cancellation checkpoint in every sift.
    // Reduced equal fractions are identical, so the canonical sorted/deduped
    // grid and every subsequent interval operation are unchanged.
    let less = |x: NormalFraction3D, y: NormalFraction3D| {
        u128::from(x.n) * u128::from(y.d) < u128::from(y.n) * u128::from(x.d)
    };
    let sift = |v: &mut [NormalFraction3D],
                mut root: usize,
                end: usize|
     -> Result<(), NormalCpuFailure3D> {
        loop {
            control.checkpoint()?;
            let child = 2 * root + 1;
            if child >= end {
                return Ok(());
            }
            let child = if child + 1 < end && less(v[child], v[child + 1]) {
                child + 1
            } else {
                child
            };
            if !less(v[root], v[child]) {
                return Ok(());
            }
            v.swap(root, child);
            root = child;
        }
    };
    let len = v.len();
    for root in (0..len / 2).rev() {
        sift(&mut v, root, len)?;
    }
    for end in (1..len).rev() {
        control.checkpoint()?;
        v.swap(0, end);
        sift(&mut v, 0, end)?;
    }
    let mut written = 0;
    for i in 0..v.len() {
        control.checkpoint()?;
        if written == 0 || v[written - 1] != v[i] {
            v[written] = v[i];
            written += 1;
        }
    }
    v.truncate(written);
    Ok(v)
}
fn normal_address(i: i64, w: u32, mode: SamplerAddressMode) -> i64 {
    let w = i64::from(w);
    match mode {
        SamplerAddressMode::ClampToEdge => i.clamp(0, w - 1),
        SamplerAddressMode::Repeat => i.rem_euclid(w),
        SamplerAddressMode::MirrorRepeat => {
            let j = i.rem_euclid(2 * w);
            if j < w { j } else { 2 * w - 1 - j }
        }
    }
}
#[derive(Clone, Copy)]
struct NormalAxis3D {
    lo: NormalFraction3D,
    hi: NormalFraction3D,
    index: [i64; 2],
}
impl NormalAxis3D {
    fn corner_intervals(self) -> [Interval; 2] {
        let lo = self.lo.interval();
        let delta = self.hi.interval().sub(lo);
        // Retain the original fraction expression at BOTH endpoints. Returning
        // lo/hi directly would change the outward add/sub/mul enclosures.
        [0.0, 1.0].map(|t| lo.add(delta.mul(Interval::point(t))))
    }
}
fn normal_mid_index(lo: NormalFraction3D, hi: NormalFraction3D, w: u32, linear: bool) -> i64 {
    let n =
        (i128::from(lo.n) * i128::from(hi.d) + i128::from(hi.n) * i128::from(lo.d)) * i128::from(w);
    let d = 2 * i128::from(lo.d) * i128::from(hi.d);
    if linear {
        (2 * n - d).div_euclid(2 * d) as i64
    } else {
        n.div_euclid(d) as i64
    }
}
fn normal_axis(
    grid: &[NormalFraction3D],
    state: usize,
    levels: [u32; 2],
    linear: bool,
    mode: SamplerAddressMode,
    same_level: bool,
) -> ([NormalAxis3D; 4], usize) {
    let zero = NormalFraction3D::new(0, 1);
    let mut out = [NormalAxis3D {
        lo: zero,
        hi: zero,
        index: [0; 2],
    }; 4];
    if linear || state % 2 == 1 {
        let i = if linear { state } else { state / 2 };
        let lo = grid[i];
        let hi = grid[i + 1];
        out[0] = NormalAxis3D {
            lo,
            hi,
            index: levels.map(|w| normal_mid_index(lo, hi, w, linear)),
        };
        return (out, 1);
    }
    let f = grid[state / 2];
    let mut choices = [[0i64; 2]; 2];
    let mut counts = [1usize; 2];
    for k in 0..2 {
        let n = u128::from(f.n) * u128::from(levels[k]);
        let d = u128::from(f.d);
        let j = (n / d) as i64;
        choices[k][0] = normal_address(j, levels[k], mode);
        if n % d == 0 {
            let other = normal_address(j - 1, levels[k], mode);
            if other != choices[k][0] {
                choices[k][1] = other;
                counts[k] = 2;
            }
        }
        choices[k][..counts[k]].sort();
    }
    let mut count = 0;
    for a in 0..counts[0] {
        for b in 0..counts[1] {
            if same_level && a != b {
                continue;
            }
            out[count] = NormalAxis3D {
                lo: f,
                hi: f,
                index: [choices[0][a], choices[1][b]],
            };
            count += 1;
        }
    }
    (out, count)
}
struct NormalByteIntervals3D {
    unscaled: [Interval; 256],
    scaled: [Interval; 256],
}

impl NormalByteIntervals3D {
    fn new(scale: f32) -> Self {
        let unscaled = std::array::from_fn(|byte| {
            Interval::point(f64::from(2 * i32::from(byte as u8) - 255))
                .div(Interval::point(255.0))
                .expect("nonzero byte denominator")
        });
        let scaled = unscaled.map(|q| q.mul(Interval::point(f64::from(scale))));
        Self { unscaled, scaled }
    }
}

fn normal_texel<const REUSE_BYTE_INTERVALS: bool>(
    source: &MipChainSource3D,
    level: u32,
    x: i64,
    y: i64,
    policy: &MipSamplerSource3D,
    scale: f32,
    bytes: &NormalByteIntervals3D,
) -> Result<[Interval; 3], PbrError3D> {
    let l = source.levels[level as usize];
    let x = normal_address(x, l.width, policy.u) as usize;
    let y = normal_address(y, l.height, policy.v) as usize;
    let p = &source.level_bytes(level as usize)?[(y * l.width as usize + x) * 4..][..4];
    if REUSE_BYTE_INTERVALS {
        return Ok([
            bytes.scaled[usize::from(p[0])],
            bytes.scaled[usize::from(p[1])],
            bytes.unscaled[usize::from(p[2])],
        ]);
    }
    // Only the cfg(test) reference instantiates the original scalar route.
    let mut q = [Interval::point(0.0); 3];
    for k in 0..3 {
        q[k] = Interval::point(f64::from(2 * i32::from(p[k]) - 255))
            .div(Interval::point(255.0))
            .expect("nonzero byte denominator");
        if k < 2 {
            q[k] = q[k].mul(Interval::point(f64::from(scale)));
        }
    }
    Ok(q)
}
fn normal_level_corner<
    const REUSE_BYTE_INTERVALS: bool,
    const REUSE_LOCAL_QUAD: bool,
    const REUSE_AXIS_WEIGHTS: bool,
>(
    source: &MipChainSource3D,
    branch: NormalBranch3D,
    level_index: usize,
    u: Interval,
    v: Interval,
    x: NormalAxis3D,
    y: NormalAxis3D,
    policy: &MipSamplerSource3D,
    scale: f32,
    bytes: &NormalByteIntervals3D,
    quad_texels: &mut Option<[[Interval; 3]; 4]>,
    axis_weights: &mut [[Option<Interval>; 2]; 2],
    endpoints: [usize; 2],
) -> Result<[Interval; 3], PbrError3D> {
    let level = if level_index == 0 {
        branch.lower
    } else {
        branch.upper
    };
    let l = source.levels[level as usize];
    let bx = x.index[level_index];
    let by = y.index[level_index];
    if !branch.linear {
        return normal_texel::<REUSE_BYTE_INTERVALS>(source, level, bx, by, policy, scale, bytes);
    }
    let cached = if REUSE_AXIS_WEIGHTS {
        [axis_weights[0][endpoints[0]], axis_weights[1][endpoints[1]]]
    } else {
        [None; 2]
    };
    // Preserve the original tx, ty, intersection and pair-check order. A hit
    // is already intersected; copying it must not apply another max/min.
    let tx = cached[0].unwrap_or_else(|| {
        u.mul(Interval::point(f64::from(l.width)))
            .sub(Interval::point(0.5 + bx as f64))
    });
    let ty = cached[1].unwrap_or_else(|| {
        v.mul(Interval::point(f64::from(l.height)))
            .sub(Interval::point(0.5 + by as f64))
    });
    // Intersection with the known ideal cell weight range tightens numerical
    // enclosures; no source coordinate or GPU result is clamped or altered.
    let tx = if cached[0].is_some() {
        tx
    } else {
        Interval {
            lo: tx.lo.max(0.0),
            hi: tx.hi.min(1.0),
        }
    };
    let ty = if cached[1].is_some() {
        ty
    } else {
        Interval {
            lo: ty.lo.max(0.0),
            hi: ty.hi.min(1.0),
        }
    };
    if !tx.finite() || !ty.finite() || tx.lo > tx.hi || ty.lo > ty.hi {
        return Err(error(
            "normal domain corner",
            "joint-cell weights not enclosed",
        ));
    }
    if REUSE_AXIS_WEIGHTS {
        // No new endpoint entry becomes usable before this original pair is
        // valid, even when only the other axis was already cached.
        axis_weights[0][endpoints[0]] = Some(tx);
        axis_weights[1][endpoints[1]] = Some(ty);
    }
    // Weights are checked before a hit or the first a/b/c/d source access.
    // The caller owns this entry for one leaf and one fixed level/index pair.
    let [a, b, c, d] = if let Some(quad) = *quad_texels {
        quad
    } else {
        let quad = [
            normal_texel::<REUSE_BYTE_INTERVALS>(source, level, bx, by, policy, scale, bytes)?,
            normal_texel::<REUSE_BYTE_INTERVALS>(source, level, bx + 1, by, policy, scale, bytes)?,
            normal_texel::<REUSE_BYTE_INTERVALS>(source, level, bx, by + 1, policy, scale, bytes)?,
            normal_texel::<REUSE_BYTE_INTERVALS>(
                source,
                level,
                bx + 1,
                by + 1,
                policy,
                scale,
                bytes,
            )?,
        ];
        if REUSE_LOCAL_QUAD {
            // A partial read or earlier source error never populates the entry.
            *quad_texels = Some(quad);
        }
        quad
    };
    let one = Interval::point(1.0);
    if REUSE_AXIS_WEIGHTS {
        let complement_x = one.sub(tx);
        let complement_y = one.sub(ty);
        return Ok(std::array::from_fn(|k| {
            a[k].mul(complement_x)
                .mul(complement_y)
                .add(b[k].mul(tx).mul(complement_y))
                .add(c[k].mul(complement_x).mul(ty))
                .add(d[k].mul(tx).mul(ty))
        }));
    }
    Ok(std::array::from_fn(|k| {
        a[k].mul(one.sub(tx))
            .mul(one.sub(ty))
            .add(b[k].mul(tx).mul(one.sub(ty)))
            .add(c[k].mul(one.sub(tx)).mul(ty))
            .add(d[k].mul(tx).mul(ty))
    }))
}
fn normal_witness(w: NormalWitness3D) -> Result<[Interval; 3], PbrError3D> {
    if !w.lower.is_finite()
        || w.lower < (1.0f64 / 510.0).next_up()
        || w.direction.iter().any(|x| !x.is_finite())
    {
        return Err(error(
            "normal witness",
            "nonfinite witness or insufficient claimed lower bound",
        ));
    }
    let m = w.direction.into_iter().map(f64::abs).fold(0.0, f64::max);
    if m == 0.0 {
        return Err(error("normal witness", "zero direction"));
    }
    let v = w.direction.map(|x| {
        Interval::point(x)
            .div(Interval::point(m))
            .expect("positive witness magnitude")
    });
    let squared = v
        .into_iter()
        .map(|x| x.mul(x))
        .fold(Interval::point(0.0), |a, b| a.add(b));
    let length = Interval {
        lo: squared.lo.max(0.0).sqrt().next_down(),
        hi: squared.hi.sqrt().next_up(),
    };
    if !length.finite() || length.lo <= 0.0 {
        return Err(error("normal witness", "unit interpretation not enclosed"));
    }
    let h = v.map(|x| x.div(length).expect("positive enclosed unit denominator"));
    if h.iter().any(|x| !x.finite()) {
        return Err(error("normal witness", "nonfinite unit enclosure"));
    }
    Ok(h)
}
fn normal_moment_bytes(
    source: &MipChainSource3D,
    work: &mut NormalWork3D,
) -> Result<(), NormalCpuFailure3D> {
    for level in 1..source.levels.len() {
        let p = source.levels[level - 1];
        let c = source.levels[level];
        let parent = source.level_bytes(level - 1)?;
        let child = source.level_bytes(level)?;
        for y in 0..c.height {
            for x in 0..c.width {
                let (pw, ph, cw, ch) = (
                    u64::from(p.width),
                    u64::from(p.height),
                    u64::from(c.width),
                    u64::from(c.height),
                );
                let x0 = u64::from(x) * pw / cw;
                let x1 = (u64::from(x + 1) * pw).div_ceil(cw);
                let y0 = u64::from(y) * ph / ch;
                let y1 = (u64::from(y + 1) * ph).div_ceil(ch);
                let mut sums = [0u128; 4];
                for j in y0..y1 {
                    for i in x0..x1 {
                        work.take(1)?;
                        let dx = ((i + 1) * cw).min(u64::from(x + 1) * pw)
                            - (i * cw).max(u64::from(x) * pw);
                        let dy = ((j + 1) * ch).min(u64::from(y + 1) * ph)
                            - (j * ch).max(u64::from(y) * ph);
                        let weight = u128::from(dx) * u128::from(dy);
                        let offset = (j as usize * p.width as usize + i as usize) * 4;
                        for k in 0..4 {
                            sums[k] += weight * u128::from(parent[offset + k]);
                        }
                    }
                }
                let denominator = u128::from(pw) * u128::from(ph);
                let offset = (y as usize * c.width as usize + x as usize) * 4;
                for k in 0..4 {
                    let q = sums[k] / denominator;
                    let rem = sums[k] % denominator;
                    let byte = if 2 * rem > denominator || 2 * rem == denominator && q % 2 == 1 {
                        q + 1
                    } else {
                        q
                    };
                    if u128::from(child[offset + k]) != byte {
                        return Err(error(
                            "normal first moments",
                            "stored child is not the exact recursive area/ties-even result",
                        )
                        .into());
                    }
                }
            }
        }
    }
    Ok(())
}
fn normal_box_bounds(leaf: NormalBox3D) -> Result<[[f64; 2]; 3], PbrError3D> {
    if leaf.depth > 20 || leaf.path >> (3 * u32::from(leaf.depth)) != 0 {
        return Err(error("normal subdivision", "invalid bounded Morton prefix"));
    }
    let mut at = [0u32; 3];
    for d in (0..leaf.depth).rev() {
        let octant = (leaf.path >> (3 * u32::from(d))) & 7;
        for k in 0..3 {
            at[k] = 2 * at[k] + ((octant >> k) & 1) as u32;
        }
    }
    let den = f64::from(1u32 << leaf.depth);
    Ok(at.map(|x| [f64::from(x) / den, f64::from(x + 1) / den]))
}
fn check_normal_box<
    const REUSE_BYTE_INTERVALS: bool,
    const REUSE_LOCAL_QUAD: bool,
    const REUSE_AXIS_WEIGHTS: bool,
>(
    source: &MipChainSource3D,
    policy: &MipSamplerSource3D,
    scale: f32,
    bytes: &NormalByteIntervals3D,
    key: NormalCell3D,
    x: NormalAxis3D,
    y: NormalAxis3D,
    leaf: NormalBox3D,
    default_axes: Option<[[Interval; 2]; 2]>,
    h: [Interval; 3],
    summary: &mut NormalDomainSummary3D,
    work: &mut NormalWork3D,
) -> Result<(), NormalCpuFailure3D> {
    let bounds = normal_box_bounds(leaf)?;
    let branch = NormalBranch3D {
        mag: key.mag,
        linear: key.linear,
        lower: key.lower,
        upper: key.upper,
    };
    let fraction = |a: NormalFraction3D, b: NormalFraction3D, t: f64| {
        a.interval()
            .add(b.interval().sub(a.interval()).mul(Interval::point(t)))
    };
    // Only the unrefined DEFAULT leaf receives endpoint facts. Exceptions,
    // including depth0 exceptions, retain the original evaluation below.
    let mut uv_corners: [Option<[[Interval; 3]; 2]>; 4] = [None; 4];
    // Separate lower/upper entries, even for equal levels. Both die with this
    // leaf; no reads occur until the original work and weight checks succeed.
    let mut lower_texels: Option<[[Interval; 3]; 4]> = None;
    let mut upper_texels: Option<[[Interval; 3]; 4]> = None;
    // Per level: x[endpoint0/1], y[endpoint0/1]. Never shared with another leaf.
    let mut lower_weights = [[None; 2]; 2];
    let mut upper_weights = [[None; 2]; 2];
    for i in 0..8 {
        work.take(1)?;
        let (u, v) = if let Some([us, vs]) = default_axes {
            (us[i & 1], vs[(i >> 1) & 1])
        } else {
            (
                fraction(x.lo, x.hi, bounds[0][i & 1]),
                fraction(y.lo, y.hi, bounds[1][(i >> 1) & 1]),
            )
        };
        let rho = Interval::point(bounds[2][(i >> 2) & 1]);
        let [a, b] = if default_axes.is_some() {
            if let Some(corners) = uv_corners[i & 3] {
                corners
            } else {
                // Lazy, in the original corner and lower/upper-level order:
                // no later source error can overtake an earlier projection or
                // work refusal. rho1 reuses the identical immutable UV pair.
                let corners = [
                    normal_level_corner::<
                        REUSE_BYTE_INTERVALS,
                        REUSE_LOCAL_QUAD,
                        REUSE_AXIS_WEIGHTS,
                    >(
                        source,
                        branch,
                        0,
                        u,
                        v,
                        x,
                        y,
                        policy,
                        scale,
                        bytes,
                        &mut lower_texels,
                        &mut lower_weights,
                        [i & 1, (i >> 1) & 1],
                    )?,
                    normal_level_corner::<
                        REUSE_BYTE_INTERVALS,
                        REUSE_LOCAL_QUAD,
                        REUSE_AXIS_WEIGHTS,
                    >(
                        source,
                        branch,
                        1,
                        u,
                        v,
                        x,
                        y,
                        policy,
                        scale,
                        bytes,
                        &mut upper_texels,
                        &mut upper_weights,
                        [i & 1, (i >> 1) & 1],
                    )?,
                ];
                uv_corners[i & 3] = Some(corners);
                corners
            }
        } else {
            [
                normal_level_corner::<REUSE_BYTE_INTERVALS, REUSE_LOCAL_QUAD, REUSE_AXIS_WEIGHTS>(
                    source,
                    branch,
                    0,
                    u,
                    v,
                    x,
                    y,
                    policy,
                    scale,
                    bytes,
                    &mut lower_texels,
                    &mut lower_weights,
                    [i & 1, (i >> 1) & 1],
                )?,
                normal_level_corner::<REUSE_BYTE_INTERVALS, REUSE_LOCAL_QUAD, REUSE_AXIS_WEIGHTS>(
                    source,
                    branch,
                    1,
                    u,
                    v,
                    x,
                    y,
                    policy,
                    scale,
                    bytes,
                    &mut upper_texels,
                    &mut upper_weights,
                    [i & 1, (i >> 1) & 1],
                )?,
            ]
        };
        let q = std::array::from_fn::<_, 3, _>(|k| {
            a[k].mul(Interval::point(1.0).sub(rho)).add(b[k].mul(rho))
        });
        let projection = (0..3).fold(Interval::point(0.0), |sum, k| sum.add(h[k].mul(q[k])));
        if !projection.finite()
            || q.iter().any(|x| !x.finite())
            || projection.lo < leaf.witness.lower
        {
            return Err(normal_cell_error(
                key,
                "real common-UV/LOD corner projection is not certified",
            )
            .into());
        }
        summary.minimum_projection = summary.minimum_projection.min(projection.lo);
        summary.maximum_interval_width = summary
            .maximum_interval_width
            .max(projection.hi - projection.lo);
    }
    summary.leaves = summary
        .leaves
        .checked_add(1)
        .ok_or(error("normal coverage", "leaf count overflow"))?;
    Ok(())
}
fn verify_normal_domain<
    const REUSE_DEFAULT_CORNERS: bool,
    const REUSE_BYTE_INTERVALS: bool,
    const REUSE_LOCAL_QUAD: bool,
    const REUSE_AXIS_WEIGHTS: bool,
>(
    source: &MipChainSource3D,
    policy: &MipSamplerSource3D,
    scale: f32,
    certificate: &NormalDomainCertificate3D,
) -> Result<NormalDomainSummary3D, PbrError3D> {
    let body = certificate.cpu_body();
    let control = NormalCpuControl3D::synchronous();
    verify_normal_domain_cpu::<
        REUSE_DEFAULT_CORNERS,
        REUSE_BYTE_INTERVALS,
        REUSE_LOCAL_QUAD,
        REUSE_AXIS_WEIGHTS,
    >(source, policy, scale, &body, &control)
    .map_err(NormalCpuFailure3D::synchronous)
}

pub(super) fn verify_normal_domain_cpu<
    const REUSE_DEFAULT_CORNERS: bool,
    const REUSE_BYTE_INTERVALS: bool,
    const REUSE_LOCAL_QUAD: bool,
    const REUSE_AXIS_WEIGHTS: bool,
>(
    source: &MipChainSource3D,
    policy: &MipSamplerSource3D,
    scale: f32,
    certificate: &NormalCpuCertificate3D,
    control: &NormalCpuControl3D<'_>,
) -> Result<NormalDomainSummary3D, NormalCpuFailure3D> {
    let mut work = NormalWork3D {
        used: 0,
        maximum: certificate.maximum_work,
        control,
    };
    work.take(source.texels.len() as u64)?;
    work.take(source.provenance.original.len() as u64)?;
    work.take(certificate.metadata_bytes(control)?)?;
    let actual_digest = match certificate.observed_digest {
        Some(digest) => digest,
        None => certificate.computed_digest(control)?,
    };
    if certificate.digest != actual_digest {
        return Err(error("normal certificate digest", "source/body digest mismatch").into());
    }
    normal_moment_bytes(source, &mut work)?;
    let default_h = normal_witness(certificate.default)?;
    for e in certificate.exceptions.windows(2) {
        control.checkpoint()?;
        if e[0].cell >= e[1].cell {
            return Err(error(
                "normal coverage",
                "exception cells are unordered or duplicated",
            )
            .into());
        }
    }
    let mut branches = Vec::new();
    branches
        .try_reserve_exact(source.levels.len() + 2)
        .map_err(|_| error("normal branches", "bounded allocation unavailable"))?;
    let levels = source.levels.len() as u32;
    branches.push(NormalBranch3D {
        mag: true,
        linear: policy.mag == SamplerFilterMode::Linear,
        lower: 0,
        upper: 0,
    });
    let linear = policy.min.within() == wgpu::FilterMode::Linear;
    if !policy.min.uses_mips() {
        branches.push(NormalBranch3D {
            mag: false,
            linear,
            lower: 0,
            upper: 0,
        });
    } else if policy.min.between() == wgpu::MipmapFilterMode::Linear && levels > 1 {
        for l in 0..levels - 1 {
            branches.push(NormalBranch3D {
                mag: false,
                linear,
                lower: l,
                upper: l + 1,
            });
        }
    } else {
        for l in 0..levels {
            branches.push(NormalBranch3D {
                mag: false,
                linear,
                lower: l,
                upper: l,
            });
        }
    }
    branches.sort_unstable();
    let mut exception = 0usize;
    let mut summary = NormalDomainSummary3D {
        cells: 0,
        leaves: 0,
        work: 0,
        minimum_projection: f64::INFINITY,
        maximum_interval_width: 0.0,
    };
    // Exactly two fixed stack tables for this immutable scale and this call.
    // No source access, validation, traversal or work charge is moved here.
    let bytes = NormalByteIntervals3D::new(scale);
    for branch in branches {
        control.checkpoint()?;
        let lo = source.levels[branch.lower as usize];
        let hi = source.levels[branch.upper as usize];
        let us = normal_grid(lo.width, hi.width, branch.linear, control)?;
        let vs = normal_grid(lo.height, hi.height, branch.linear, control)?;
        let nu = if branch.linear {
            us.len() - 1
        } else {
            2 * us.len() - 1
        };
        let nv = if branch.linear {
            vs.len() - 1
        } else {
            2 * vs.len() - 1
        };
        for u in 0..nu {
            control.checkpoint()?;
            let (xs, nx) = normal_axis(
                &us,
                u,
                [lo.width, hi.width],
                branch.linear,
                policy.u,
                branch.lower == branch.upper,
            );
            let mut x_corners = [[Interval::point(0.0); 2]; 4];
            if REUSE_DEFAULT_CORNERS {
                for a in 0..nx {
                    x_corners[a] = xs[a].corner_intervals();
                }
            }
            for v in 0..nv {
                control.checkpoint()?;
                let (ys, ny) = normal_axis(
                    &vs,
                    v,
                    [lo.height, hi.height],
                    branch.linear,
                    policy.v,
                    branch.lower == branch.upper,
                );
                let mut y_corners = [[Interval::point(0.0); 2]; 4];
                if REUSE_DEFAULT_CORNERS {
                    for b in 0..ny {
                        y_corners[b] = ys[b].corner_intervals();
                    }
                }
                for a in 0..nx {
                    for b in 0..ny {
                        work.take(1)?;
                        let key = NormalCell3D {
                            mag: branch.mag,
                            linear: branch.linear,
                            lower: branch.lower,
                            upper: branch.upper,
                            u: u as u32,
                            v: v as u32,
                            u_choice: a as u8,
                            v_choice: b as u8,
                        };
                        summary.cells = summary
                            .cells
                            .checked_add(1)
                            .ok_or(error("normal coverage", "cell count overflow"))?;
                        if summary.cells > certificate.cells {
                            return Err(normal_cell_error(
                                key,
                                "declared complete cell count is too small",
                            )
                            .into());
                        }
                        if certificate
                            .exceptions
                            .get(exception)
                            .is_some_and(|e| e.cell < key)
                        {
                            return Err(normal_cell_error(
                                key,
                                "exception names an unreachable/unexamined cell",
                            )
                            .into());
                        }
                        if let Some(e) = certificate
                            .exceptions
                            .get(exception)
                            .filter(|e| e.cell == key)
                        {
                            let mut at = 0u64;
                            for &leaf in e.leaves.iter() {
                                control.checkpoint()?;
                                normal_box_bounds(leaf)?;
                                let shift = 3 * (20 - u32::from(leaf.depth));
                                let start = leaf.path << shift;
                                let end = start.checked_add(1u64 << shift).ok_or(
                                    normal_cell_error(key, "subdivision coverage overflow"),
                                )?;
                                if start != at {
                                    return Err(normal_cell_error(
                                        key,
                                        "subdivision has a gap, overlap or unordered prefix",
                                    )
                                    .into());
                                }
                                at = end;
                                check_normal_box::<
                                    REUSE_BYTE_INTERVALS,
                                    REUSE_LOCAL_QUAD,
                                    REUSE_AXIS_WEIGHTS,
                                >(
                                    source,
                                    policy,
                                    scale,
                                    &bytes,
                                    key,
                                    xs[a],
                                    ys[b],
                                    leaf,
                                    None,
                                    normal_witness(leaf.witness)?,
                                    &mut summary,
                                    &mut work,
                                )?;
                            }
                            if at != 1u64 << 60 {
                                return Err(
                                    normal_cell_error(key, "subdivision is incomplete").into()
                                );
                            }
                            exception += 1;
                        } else {
                            let leaf = NormalBox3D {
                                path: 0,
                                depth: 0,
                                witness: certificate.default,
                            };
                            check_normal_box::<
                                REUSE_BYTE_INTERVALS,
                                REUSE_LOCAL_QUAD,
                                REUSE_AXIS_WEIGHTS,
                            >(
                                source,
                                policy,
                                scale,
                                &bytes,
                                key,
                                xs[a],
                                ys[b],
                                leaf,
                                if REUSE_DEFAULT_CORNERS {
                                    Some([x_corners[a], y_corners[b]])
                                } else {
                                    None
                                },
                                default_h,
                                &mut summary,
                                &mut work,
                            )?;
                        }
                    }
                }
            }
        }
    }
    if summary.cells != certificate.cells
        || exception != certificate.exceptions.len()
        || !summary.minimum_projection.is_finite()
    {
        return Err(error(
            "normal coverage",
            "incomplete or surplus source domain records",
        )
        .into());
    }
    summary.work = work.used;
    Ok(summary)
}

/// The reference instantiation retains the original arithmetic and traversal;
/// it exists only to compare exact summaries/refusals with production reuse.
#[cfg(test)]
pub(crate) fn verify_normal_domain_for_test<
    const REUSE_DEFAULT_CORNERS: bool,
    const REUSE_BYTE_INTERVALS: bool,
    const REUSE_LOCAL_QUAD: bool,
    const REUSE_AXIS_WEIGHTS: bool,
>(
    source: &MipChainSource3D,
    policy: &MipSamplerSource3D,
    scale: f32,
    certificate: &NormalDomainCertificate3D,
) -> Result<NormalDomainSummary3D, PbrError3D> {
    verify_normal_domain::<
        REUSE_DEFAULT_CORNERS,
        REUSE_BYTE_INTERVALS,
        REUSE_LOCAL_QUAD,
        REUSE_AXIS_WEIGHTS,
    >(source, policy, scale, certificate)
}

#[cfg(test)]
pub(crate) fn check_normal_byte_intervals_for_test(scale: f32) {
    let bytes = NormalByteIntervals3D::new(scale);
    assert_eq!(
        std::mem::size_of::<NormalByteIntervals3D>(),
        2 * 256 * std::mem::size_of::<Interval>()
    );
    for byte in 0..=u8::MAX {
        // Independently retain the exact original point/div/mul order.
        let unscaled = Interval::point(f64::from(2 * i32::from(byte) - 255))
            .div(Interval::point(255.0))
            .expect("nonzero byte denominator");
        let scaled = unscaled.mul(Interval::point(f64::from(scale)));
        assert_eq!(
            bytes.unscaled[usize::from(byte)].bits(),
            unscaled.bits(),
            "unscaled byte {byte}, scale bits {:08x}",
            scale.to_bits()
        );
        assert_eq!(
            bytes.scaled[usize::from(byte)].bits(),
            scaled.bits(),
            "scaled byte {byte}, scale bits {:08x}",
            scale.to_bits()
        );
    }
}

/// Closed material-resource sum: V1 handles never become V2 chain handles.
#[derive(Clone)]
pub(crate) enum MaterialTextureLease3D {
    V1(TextureLease),
    V2(MipChainLease3D),
}
#[derive(Clone)]
pub(crate) enum MaterialSamplerLease3D {
    V1(SamplerLease),
    V2(MipSamplerLease3D),
}
pub(crate) enum MaterialTextureDescriptor3D<'a> {
    V1(&'a super::Texture3D),
    V2(&'a MipChainSource3D),
}
impl MaterialTextureDescriptor3D<'_> {
    pub(crate) fn texels(&self) -> &[u8] {
        match self {
            Self::V1(t) => t.texels(),
            Self::V2(t) => &t.texels,
        }
    }
    pub(crate) fn format(&self) -> TextureFormat3D {
        match self {
            Self::V1(t) => t.format(),
            Self::V2(t) => t.format,
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) struct MaterialResourceKey3D(u32, u64);
impl MaterialResourceKey3D {
    pub(crate) fn slot(self) -> u32 {
        self.0
    }
    pub(crate) fn generation(self) -> u64 {
        self.1
    }
}
impl MaterialTextureLease3D {
    pub(crate) fn descriptor(&self) -> MaterialTextureDescriptor3D<'_> {
        match self {
            Self::V1(t) => MaterialTextureDescriptor3D::V1(t.descriptor()),
            Self::V2(t) => MaterialTextureDescriptor3D::V2(&t.source),
        }
    }
    pub(crate) fn handle(&self) -> MaterialResourceKey3D {
        match self {
            Self::V1(t) => MaterialResourceKey3D(t.handle().slot(), t.handle().generation()),
            Self::V2(t) => {
                let (s, g) = t.handle.0.slot_generation();
                MaterialResourceKey3D(s, g)
            }
        }
    }
}
impl MaterialSamplerLease3D {
    pub(crate) fn handle(&self) -> MaterialResourceKey3D {
        match self {
            Self::V1(t) => MaterialResourceKey3D(t.handle().slot(), t.handle().generation()),
            Self::V2(t) => {
                let (s, g) = t.handle.0.slot_generation();
                MaterialResourceKey3D(s, g)
            }
        }
    }
}
pub(super) enum MaterialTextureGpu3D {
    V1(TextureGpuLease),
    V2(Arc<MipGpu3D>),
}
pub(super) enum MaterialSamplerGpu3D {
    V1(SamplerGpuLease),
    V2(Arc<MipSamplerGpu3D>),
}
impl MaterialTextureGpu3D {
    pub(super) fn view(&self) -> &wgpu::TextureView {
        match self {
            Self::V1(t) => t.view(),
            Self::V2(t) => &t.view,
        }
    }
    pub(super) fn retained_clone(&self) -> Self {
        match self {
            Self::V1(t) => Self::V1(t.retained_clone()),
            Self::V2(t) => Self::V2(t.clone()),
        }
    }
}
impl MaterialSamplerGpu3D {
    pub(super) fn sampler(&self) -> &wgpu::Sampler {
        match self {
            Self::V1(t) => t.sampler(),
            Self::V2(t) => &t.sampler,
        }
    }
    pub(super) fn retained_clone(&self) -> Self {
        match self {
            Self::V1(t) => Self::V1(t.retained_clone()),
            Self::V2(t) => Self::V2(t.clone()),
        }
    }
}

/// Only sampling changes. The original Surface factors, Mask, local-first
/// frame, exact BRDF, interfaces and bindings remain in the fixed V1 program.
pub(crate) fn shader_source() -> String {
    let source = super::pbr::PBR_WGSL;
    let begin = source
        .find("fn surface(i:FragmentIn)->Surface {")
        .expect("fixed surface seam");
    let end = source[begin..].find("fn reflected_brdf(").unwrap() + begin;
    format!("{}{}{}", &source[..begin], SURFACE_V2, &source[end..])
}

// Shared outward arithmetic for immutable source normal certificates and the
// explicitly selected offline GPU model. No modeled GPU admission runs per view.
#[derive(Clone, Copy)]
struct Interval {
    lo: f64,
    hi: f64,
}
impl Interval {
    fn point(x: f64) -> Self {
        Self { lo: x, hi: x }
    }
    fn add(self, b: Self) -> Self {
        Self {
            lo: (self.lo + b.lo).next_down(),
            hi: (self.hi + b.hi).next_up(),
        }
    }
    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
    fn sub(self, b: Self) -> Self {
        self.add(b.neg())
    }
    fn mul(self, b: Self) -> Self {
        let p = [
            self.lo * b.lo,
            self.lo * b.hi,
            self.hi * b.lo,
            self.hi * b.hi,
        ];
        Self {
            lo: p.into_iter().fold(f64::INFINITY, f64::min).next_down(),
            hi: p.into_iter().fold(f64::NEG_INFINITY, f64::max).next_up(),
        }
    }
    fn div(self, b: Self) -> Option<Self> {
        if b.lo <= 0.0 && b.hi >= 0.0 {
            return None;
        }
        Some(self.mul(Self {
            lo: (1.0 / b.hi).next_down(),
            hi: (1.0 / b.lo).next_up(),
        }))
    }
    fn abs(self) -> f64 {
        self.lo.abs().max(self.hi.abs()).next_up()
    }
    fn finite(self) -> bool {
        self.lo.is_finite() && self.hi.is_finite()
    }
    fn expand(self, e: f64) -> Self {
        Self {
            lo: (self.lo - e).next_down(),
            hi: (self.hi + e).next_up(),
        }
    }
    fn bits(self) -> [u64; 2] {
        [self.lo.to_bits(), self.hi.to_bits()]
    }
}
#[cfg(test)]
fn determinant(m: [[Interval; 3]; 3]) -> Interval {
    m[0][0]
        .mul(m[1][1].mul(m[2][2]).sub(m[1][2].mul(m[2][1])))
        .sub(m[0][1].mul(m[1][0].mul(m[2][2]).sub(m[1][2].mul(m[2][0]))))
        .add(m[0][2].mul(m[1][0].mul(m[2][1]).sub(m[1][1].mul(m[2][0]))))
}

#[cfg(test)]
fn screen_determinant(m: [[Interval; 3]; 3], centered: bool) -> Interval {
    if !centered {
        return determinant(m);
    }
    // For EVERY actual matrix enclosed by m, subtracting its first row from
    // rows1/2 leaves the determinant exactly unchanged. Outward differences
    // therefore enclose that same determinant, including independent vertex
    // rounding, without expanding products of large common clip coordinates.
    // This is host proof conditioning, never a GPU/source coordinate change.
    determinant([
        m[0],
        std::array::from_fn(|j| m[1][j].sub(m[0][j])),
        std::array::from_fn(|j| m[2][j].sub(m[0][j])),
    ])
}

#[cfg(test)]
fn plane(
    m: [[Interval; 3]; 3],
    v: [f32; 3],
    d: Interval,
    centered: bool,
) -> Result<[Interval; 3], (usize, Interval)> {
    let mut p = [Interval::point(0.0); 3];
    for j in 0..3 {
        let mut replaced = m;
        for i in 0..3 {
            replaced[i][j] = Interval::point(f64::from(v[i]));
        }
        // The SAME row identity applies to every Cramer numerator, including
        // the constant reciprocal-w and original UV source columns. No assumed
        // relationship between separate interval endpoints is required.
        p[j] = screen_determinant(replaced, centered)
            .div(d)
            .ok_or((j, d))?;
        if !p[j].finite() {
            return Err((j, p[j]));
        }
    }
    Ok(p)
}
#[cfg(test)]
fn packed_possible_flush(a: Interval) -> Interval {
    let normal = f64::from(f32::MIN_POSITIVE);
    if a.lo < normal && a.hi > -normal {
        Interval {
            lo: a.lo.min(0.0),
            hi: a.hi.max(0.0),
        }
    } else {
        a
    }
}

#[cfg(test)]
fn packed_dot(row: [f32; 4], x: [Interval; 4]) -> Result<Interval, Interval> {
    packed_dot_coefficients(
        |j| packed_possible_flush(Interval::point(f64::from(row[j]))),
        x,
    )
}

#[cfg(test)]
fn packed_dot_coefficients(
    coefficient: impl Fn(usize) -> Interval,
    x: [Interval; 4],
) -> Result<Interval, Interval> {
    let mut value = Interval::point(0.0);
    let mut magnitude = Interval::point(0.0);
    for j in 0..4 {
        let term = coefficient(j).mul(packed_possible_flush(x[j]));
        value = value.add(term);
        magnitude = magnitude.add(Interval::point(term.abs()));
    }
    // Seven rounded operations enclose both fused and separate four-term dots.
    // The absolute term also encloses implementations that flush subnormals.
    let u = 2.0_f64.powi(-24);
    let gamma7 = ((7.0 * u) / (1.0 - 7.0 * u)).next_up();
    let roundoff = Interval::point(gamma7)
        .mul(magnitude)
        .add(Interval::point(7.0 * f64::from(f32::MIN_POSITIVE)))
        .hi;
    let result = value.expand(roundoff);
    if result.finite() && result.abs() <= f64::from(f32::MAX) {
        Ok(result)
    } else {
        Err(result)
    }
}

#[cfg(test)]
pub(crate) fn projection_dot_bits_for_test(
    row: [f32; 4],
    input: [[f64; 2]; 4],
) -> [Result<[u64; 2], [u64; 2]>; 3] {
    // Independent prior scalar expression, including its original flush closure.
    let original = |row: [f32; 4], x: [Interval; 4]| {
        let possible_flush = |a: Interval| {
            let normal = f64::from(f32::MIN_POSITIVE);
            if a.lo < normal && a.hi > -normal {
                Interval {
                    lo: a.lo.min(0.0),
                    hi: a.hi.max(0.0),
                }
            } else {
                a
            }
        };
        let mut value = Interval::point(0.0);
        let mut magnitude = Interval::point(0.0);
        for j in 0..4 {
            let term = possible_flush(Interval::point(f64::from(row[j]))).mul(possible_flush(x[j]));
            value = value.add(term);
            magnitude = magnitude.add(Interval::point(term.abs()));
        }
        let u = 2.0_f64.powi(-24);
        let gamma7 = ((7.0 * u) / (1.0 - 7.0 * u)).next_up();
        let roundoff = Interval::point(gamma7)
            .mul(magnitude)
            .add(Interval::point(7.0 * f64::from(f32::MIN_POSITIVE)))
            .hi;
        let result = value.expand(roundoff);
        if result.finite() && result.abs() <= f64::from(f32::MAX) {
            Ok(result)
        } else {
            Err(result)
        }
    };
    let input = input.map(|[lo, hi]| Interval { lo, hi });
    let prepared = row.map(|v| packed_possible_flush(Interval::point(f64::from(v))));
    [
        original(row, input),
        packed_dot(row, input),
        packed_dot_coefficients(|j| prepared[j], input),
    ]
    .map(|v| v.map(Interval::bits).map_err(Interval::bits))
}

#[derive(Debug, Clone, Copy)]
#[cfg(test)]
pub(crate) struct UvDomainContext3D {
    pub packet: usize,
    pub ordinal: u64,
    /// None labels the main model; Some labels a shadow-face model diagnostic.
    pub shadow_face: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct UvDomainSummary3D {
    /// Model-qualified triangles; not runtime admission or certified GPU fragments.
    pub qualified: usize,
    /// Only complete proven homogeneous exclusions.
    pub excluded: usize,
    /// Successful row-difference determinant conditioning, not attempts.
    pub conditioned: usize,
    /// Positive sufficient modeled-host minima for EVERY qualified triangle.
    /// None means none qualified or at least one enclosure was indeterminate.
    /// Binary64 bits preserve exact diagnostic equality without changing source.
    pub minimum_determinant_margin_bits: Option<u64>,
    pub minimum_helper_q_lower_bits: Option<u64>,
}

#[derive(Clone, Copy)]
#[cfg(test)]
struct ObjectRelativeCorner3D {
    source_index: usize,
    relative: [Interval; 4],
}

/// Offline diagnostic capture; production retains no object-interval sidecar.
#[cfg(test)]
pub(crate) struct ObjectRelativePacket3D {
    geometry: super::pbr::PbrGeometryLease3D,
    object_bits: [[u32; 4]; 4],
    packet: usize,
    ordinal: u64,
    corners: Box<[ObjectRelativeCorner3D]>,
    complete: bool,
}

#[cfg(test)]
impl ObjectRelativePacket3D {
    #[cfg(test)]
    pub(crate) fn same_corner_bits_for_test(&self, other: &Self) -> bool {
        self.complete
            && other.complete
            && self.corners.len() == other.corners.len()
            && self.corners.iter().zip(&other.corners).all(|(a, b)| {
                a.source_index == b.source_index
                    && a.relative.map(Interval::bits) == b.relative.map(Interval::bits)
            })
    }

    pub(crate) fn is_complete(&self) -> bool {
        self.complete
    }
    pub(crate) fn payload_bytes(geometry: &super::pbr::PbrGeometryLease3D) -> Option<u64> {
        u64::try_from(geometry.data.source.element_count())
            .ok()?
            .checked_mul(std::mem::size_of::<ObjectRelativeCorner3D>() as u64)
    }

    pub(crate) fn try_new(
        geometry: &super::pbr::PbrGeometryLease3D,
        object: [[f32; 4]; 4],
        context: UvDomainContext3D,
    ) -> Option<Self> {
        let count = geometry.data.source.element_count();
        let mut corners = Vec::new();
        corners.try_reserve_exact(count).ok()?;
        corners.resize(
            count,
            ObjectRelativeCorner3D {
                source_index: usize::MAX,
                relative: [Interval::point(0.0); 4],
            },
        );
        Some(Self {
            geometry: geometry.clone(),
            object_bits: object.map(|c| c.map(f32::to_bits)),
            packet: context.packet,
            ordinal: context.ordinal,
            corners: corners.into_boxed_slice(),
            complete: false,
        })
    }

    fn matches(
        &self,
        geometry: &super::pbr::PbrGeometryLease3D,
        object: [[f32; 4]; 4],
        context: UvDomainContext3D,
    ) -> bool {
        self.geometry.handle == geometry.handle
            && Arc::ptr_eq(&self.geometry.data, &geometry.data)
            && self.object_bits == object.map(|c| c.map(f32::to_bits))
            && self.packet == context.packet
            && self.ordinal == context.ordinal
            && context.shadow_face.is_none()
            && self.corners.len() == geometry.data.source.element_count()
    }
}

#[cfg(test)]
enum ObjectRelativeAccess3D<'a> {
    None,
    Read(&'a [ObjectRelativeCorner3D]),
    Capture(&'a mut [ObjectRelativeCorner3D]),
}

#[cfg(test)]
pub(crate) fn qualify_uv_domain_with_objects(
    geometry: &super::pbr::PbrGeometryLease3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    cached: Option<&ObjectRelativePacket3D>,
    #[cfg(any(test, feature = "test-harness"))] timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    qualify_uv_domain_with_objects_impl::<true, true>(
        geometry,
        object,
        projection,
        extent,
        role_extent,
        context,
        cached,
        #[cfg(any(test, feature = "test-harness"))]
        timing,
    )
}

#[cfg(test)]
pub(crate) fn qualify_uv_projection_control_for_test<const PREPARED_PROJECTION: bool>(
    geometry: &super::pbr::PbrGeometryLease3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    cached: Option<&ObjectRelativePacket3D>,
    timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    qualify_uv_domain_with_objects_impl::<PREPARED_PROJECTION, true>(
        geometry,
        object,
        projection,
        extent,
        role_extent,
        context,
        cached,
        timing,
    )
}

#[cfg(test)]
pub(crate) fn qualify_uv_helper_constants_control_for_test<const HOIST_HELPER_CONSTANTS: bool>(
    geometry: &super::pbr::PbrGeometryLease3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    cached: Option<&ObjectRelativePacket3D>,
    timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    qualify_uv_domain_with_objects_impl::<true, HOIST_HELPER_CONSTANTS>(
        geometry,
        object,
        projection,
        extent,
        role_extent,
        context,
        cached,
        timing,
    )
}

#[cfg(test)]
pub(crate) fn original_helper_constant_bits_for_test(extent: (u32, u32)) -> [u64; 3] {
    let delta = [extent.0, extent.1].map(|n| {
        Interval::point(4.0)
            .div(Interval::point(f64::from(n)))
            .expect("positive extent")
            .hi
    });
    let u = 2.0_f64.powi(-24);
    [
        delta[0].to_bits(),
        delta[1].to_bits(),
        ((7.0 * u) / (1.0 - 7.0 * u)).next_up().to_bits(),
    ]
}

#[cfg(test)]
fn qualify_uv_domain_with_objects_impl<
    const PREPARED_PROJECTION: bool,
    const HOIST_HELPER_CONSTANTS: bool,
>(
    geometry: &super::pbr::PbrGeometryLease3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    cached: Option<&ObjectRelativePacket3D>,
    #[cfg(any(test, feature = "test-harness"))] timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    let access = cached
        .filter(|p| p.complete && p.matches(geometry, object, context))
        .map_or(ObjectRelativeAccess3D::None, |p| {
            ObjectRelativeAccess3D::Read(&p.corners)
        });
    qualify_uv_domain_impl::<PREPARED_PROJECTION, HOIST_HELPER_CONSTANTS>(
        &geometry.data.source,
        object,
        projection,
        extent,
        role_extent,
        context,
        access,
        #[cfg(any(test, feature = "test-harness"))]
        timing,
    )
}

#[cfg(test)]
pub(crate) fn capture_uv_object_intervals(
    geometry: &super::pbr::PbrGeometryLease3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    output: &mut ObjectRelativePacket3D,
    #[cfg(any(test, feature = "test-harness"))] timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    let matches = output.matches(geometry, object, context);
    output.complete = false;
    let access = if matches {
        ObjectRelativeAccess3D::Capture(&mut output.corners)
    } else {
        ObjectRelativeAccess3D::None
    };
    let result = qualify_uv_domain_impl::<true, true>(
        &geometry.data.source,
        object,
        projection,
        extent,
        role_extent,
        context,
        access,
        #[cfg(any(test, feature = "test-harness"))]
        timing,
    );
    output.complete = matches && result.is_ok();
    result
}

#[cfg(any(test, feature = "test-harness"))]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg(test)]
pub(crate) struct UvDomainTiming3D {
    /// Setup, bookkeeping, object, projection, exclusion, determinant, helper,
    /// summary, and measured boundary bookkeeping. All are exclusive CPU wall.
    pub stages: [std::time::Duration; 9],
    /// Triangles, vertices, centered, uncentered, helper attempts, q planes,
    /// positive q minima, U attempts/successes, V attempts/successes, final
    /// helper successes/failures, and exclusion tests.
    pub counts: [usize; 14],
    pub wrapper: std::time::Duration,
    pub complete: bool,
    pub object_dot_calls: usize,
    pub projection_dot_calls: usize,
    pub projection_coefficient_preparations: usize,
    pub helper_delta_divisions: usize,
    pub helper_gamma7_evaluations: usize,
    pub helper_constant_bits: Option<[u64; 3]>,
    pub object_corner_reuses: usize,
}

#[cfg(any(test, feature = "test-harness"))]
#[cfg(test)]
impl UvDomainTiming3D {
    pub fn body(&self) -> std::time::Duration {
        self.stages.iter().copied().sum()
    }

    pub fn enclose(&mut self, elapsed: std::time::Duration) {
        self.wrapper = elapsed
            .checked_sub(self.body())
            .expect("UV internal clock fits its enclosing packet timer");
    }
}

#[cfg(any(test, feature = "test-harness"))]
#[cfg(test)]
struct UvDomainClock3D {
    last: std::time::Instant,
    record: UvDomainTiming3D,
}

#[cfg(any(test, feature = "test-harness"))]
#[cfg(test)]
impl UvDomainClock3D {
    fn new() -> Self {
        let record = UvDomainTiming3D::default();
        Self {
            record,
            last: std::time::Instant::now(),
        }
    }

    fn mark(&mut self, stage: usize) {
        let end = std::time::Instant::now();
        self.record.stages[stage] += end.duration_since(self.last);
        let resume = std::time::Instant::now();
        self.record.stages[8] += resume.duration_since(end);
        // These final stores and counter updates are included in the following
        // segment. Clock retrieval and scheduling remain CPU wall overhead.
        self.last = resume;
    }

    fn finish(mut self, output: &mut UvDomainTiming3D) {
        self.mark(8);
        self.record.complete = true;
        *output = self.record;
    }
}

#[cfg(test)]
pub(crate) fn uv_control_geometry_for_test(
    scene: &super::pbr::MaterialSceneSource3D,
) -> &super::geometry::PbrGeometrySource3D {
    let super::snapshot::CommittedMaterialPacket3D::PbrMaterial { geometry, .. } =
        &scene.snapshot.packets[0]
    else {
        panic!("expected original V2 geometry");
    };
    &geometry.data.source
}

/// Explicit offline packed-arithmetic model diagnostic, never runtime admission.
/// Its clip/helper bounds neither observe nor certify actual GPU execution.
#[cfg(test)]
pub(crate) fn qualify_uv_domain(
    geometry: &super::geometry::PbrGeometrySource3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
) -> Result<UvDomainSummary3D, PbrError3D> {
    qualify_uv_domain_impl::<true, true>(
        geometry,
        object,
        projection,
        extent,
        role_extent,
        context,
        ObjectRelativeAccess3D::None,
        #[cfg(any(test, feature = "test-harness"))]
        None,
    )
}

#[cfg(any(test, feature = "test-harness"))]
#[cfg(test)]
pub(crate) fn qualify_uv_domain_with_timing(
    geometry: &super::geometry::PbrGeometrySource3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    if timing.is_none() {
        return qualify_uv_domain(geometry, object, projection, extent, role_extent, context);
    }
    qualify_uv_domain_impl::<true, true>(
        geometry,
        object,
        projection,
        extent,
        role_extent,
        context,
        ObjectRelativeAccess3D::None,
        timing,
    )
}

#[cfg(test)]
std::thread_local! {
    static UV_MODEL_INVOCATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

#[cfg(test)]
pub(crate) fn uv_model_invocations_for_test() -> usize {
    UV_MODEL_INVOCATIONS.with(std::cell::Cell::get)
}

#[cfg(test)]
fn qualify_uv_domain_impl<const PREPARED_PROJECTION: bool, const HOIST_HELPER_CONSTANTS: bool>(
    geometry: &super::geometry::PbrGeometrySource3D,
    object: [[f32; 4]; 4],
    projection: [[f32; 4]; 4],
    extent: (u32, u32),
    role_extent: [u32; 2],
    context: UvDomainContext3D,
    mut objects: ObjectRelativeAccess3D<'_>,
    #[cfg(any(test, feature = "test-harness"))] timing: Option<&mut UvDomainTiming3D>,
) -> Result<UvDomainSummary3D, PbrError3D> {
    #[cfg(test)]
    UV_MODEL_INVOCATIONS.with(|count| count.set(count.get() + 1));
    #[cfg(any(test, feature = "test-harness"))]
    let mut clock = timing.as_ref().map(|_| UvDomainClock3D::new());
    let template = super::pbr::UvGradientFailure3D {
        packet: context.packet,
        ordinal: context.ordinal,
        original: geometry.certified_input().map(|v| v.original.identity),
        triangle: None,
        render_triangle: None,
        shadow_face: context.shadow_face,
        operation: "",
        corner: None,
        axis: None,
        bounds_bits: None,
        determinant_bits: None,
        helper_q_min_bits: None,
        extent: [extent.0, extent.1],
        role_extent,
        object_bits: object.map(|column| column.map(f32::to_bits)),
        projection_bits: projection.map(|column| column.map(f32::to_bits)),
        position_bits: [None; 3],
        uv_bits: [None; 3],
        clip_bits: [None; 3],
        q_plane_bits: None,
        uv_plane_bits: None,
    };
    let invalid =
        |operation, bounds: Option<Interval>, evidence: &super::pbr::UvGradientFailure3D| {
            let mut failure = evidence.clone();
            failure.operation = operation;
            failure.bounds_bits = bounds.map(Interval::bits);
            PbrError3D::UvGradient(Box::new(failure))
        };
    if extent.0 == 0 || extent.1 == 0 {
        return Err(invalid("physical target extent > 0", None, &template));
    }
    if role_extent.contains(&0) {
        return Err(invalid("selected-role extent > 0", None, &template));
    }
    let projection_coefficients: Option<[[Interval; 4]; 4]> = PREPARED_PROJECTION.then(|| {
        std::array::from_fn(|i| {
            std::array::from_fn(|j| {
                packed_possible_flush(Interval::point(f64::from(projection[j][i])))
            })
        })
    });
    #[cfg(any(test, feature = "test-harness"))]
    if let Some(c) = clock.as_mut() {
        c.record.projection_coefficient_preparations = if PREPARED_PROJECTION { 16 } else { 0 };
    }
    let dot =
        |m: [[f32; 4]; 4],
         x: [Interval; 4],
         coefficients: Option<&[[Interval; 4]; 4]>,
         #[cfg(any(test, feature = "test-harness"))] projection: bool,
         #[cfg(any(test, feature = "test-harness"))] mut timing: Option<&mut UvDomainTiming3D>|
         -> Result<[Interval; 4], (usize, Interval)> {
            let mut out = [Interval::point(0.0); 4];
            for i in 0..4 {
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(t) = timing.as_deref_mut() {
                    if projection {
                        t.projection_dot_calls += 1;
                        if coefficients.is_none() {
                            t.projection_coefficient_preparations += 4;
                        }
                    } else {
                        t.object_dot_calls += 1;
                    }
                }
                out[i] = if let Some(rows) = coefficients {
                    packed_dot_coefficients(|j| rows[i][j], x)
                } else {
                    packed_dot(std::array::from_fn(|j| m[j][i]), x)
                }
                .map_err(|v| (i, v))?;
            }
            Ok(out)
        };
    let u = 2.0_f64.powi(-24);
    let compute_helper_delta = || {
        [extent.0, extent.1].map(|n| {
            Interval::point(4.0)
                .div(Interval::point(f64::from(n)))
                .expect("positive extent")
                .hi
        })
    };
    let compute_helper_gamma7 = || ((7.0 * u) / (1.0 - 7.0 * u)).next_up();
    let helper_delta = HOIST_HELPER_CONSTANTS.then(compute_helper_delta);
    let helper_gamma7 = HOIST_HELPER_CONSTANTS.then(compute_helper_gamma7);
    #[cfg(any(test, feature = "test-harness"))]
    if HOIST_HELPER_CONSTANTS {
        if let Some(c) = clock.as_mut() {
            c.record.helper_delta_divisions = 2;
            c.record.helper_gamma7_evaluations = 1;
            c.record.helper_constant_bits = Some([
                helper_delta.unwrap()[0].to_bits(),
                helper_delta.unwrap()[1].to_bits(),
                helper_gamma7.unwrap().to_bits(),
            ]);
        }
    }
    let rounding_factor = Interval::point(1.0)
        .div(Interval::point(1.0 - u))
        .expect("positive unit-roundoff complement");
    let rounded_upper = |value: Interval| {
        value
            .mul(rounding_factor)
            .add(Interval::point(f64::from(f32::MIN_POSITIVE)))
    };
    let mut summary = UvDomainSummary3D {
        qualified: 0,
        excluded: 0,
        conditioned: 0,
        minimum_determinant_margin_bits: None,
        minimum_helper_q_lower_bits: None,
    };
    #[cfg(any(test, feature = "test-harness"))]
    if let Some(c) = clock.as_mut() {
        c.mark(0);
    }
    for triangle in 0..geometry.element_count() / 3 {
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.record.counts[0] += 1;
        }
        let mut evidence = template.clone();
        evidence.triangle = Some(triangle);
        evidence.render_triangle = Some(triangle);
        if let Some(certificate) = geometry
            .certified_input()
            .and_then(|v| v.triangles.get(triangle))
        {
            evidence.original = Some(certificate.original);
            evidence.triangle = Some(certificate.triangle);
        }
        let mut clip = [[Interval::point(0.0); 4]; 3];
        let mut uv = [[0.0f32; 2]; 3];
        for i in 0..3 {
            evidence.corner = Some(i as u8);
            let e = 3 * triangle + i;
            let index =
                match &geometry.indices {
                    super::GeometryIndices::Unindexed => e,
                    super::GeometryIndices::U16(v) => usize::from(*v.get(e).ok_or_else(|| {
                        invalid("original element/index available", None, &evidence)
                    })?),
                    super::GeometryIndices::U32(v) => {
                        usize::try_from(*v.get(e).ok_or_else(|| {
                            invalid("original element/index available", None, &evidence)
                        })?)
                        .map_err(|_| invalid("original index addressable", None, &evidence))?
                    }
                };
            let vertex = geometry
                .vertices
                .get(index)
                .ok_or_else(|| invalid("original vertex/index available", None, &evidence))?;
            let p = [
                vertex.position.x(),
                vertex.position.y(),
                vertex.position.z(),
                1.0,
            ];
            evidence.position_bits[i] = Some([p[0], p[1], p[2]].map(f32::to_bits));
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.record.counts[1] += 1;
                c.mark(1);
            }
            let cached = match &objects {
                ObjectRelativeAccess3D::Read(corners) => corners
                    .get(e)
                    .filter(|c| c.source_index == index)
                    .map(|c| c.relative),
                _ => None,
            };
            let relative = if let Some(relative) = cached {
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    c.record.object_corner_reuses += 1;
                }
                relative
            } else {
                dot(
                    object,
                    p.map(|v| Interval::point(f64::from(v))),
                    None,
                    #[cfg(any(test, feature = "test-harness"))]
                    false,
                    #[cfg(any(test, feature = "test-harness"))]
                    clock.as_mut().map(|c| &mut c.record),
                )
                .map_err(|(axis, bounds)| {
                    evidence.axis = Some(axis as u8);
                    invalid(
                        "packed object dot finite and within f32",
                        Some(bounds),
                        &evidence,
                    )
                })?
            };
            if let ObjectRelativeAccess3D::Capture(corners) = &mut objects {
                corners[e] = ObjectRelativeCorner3D {
                    source_index: index,
                    relative,
                };
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.mark(2);
            }
            clip[i] = dot(
                projection,
                relative,
                projection_coefficients.as_ref(),
                #[cfg(any(test, feature = "test-harness"))]
                true,
                #[cfg(any(test, feature = "test-harness"))]
                clock.as_mut().map(|c| &mut c.record),
            )
            .map_err(|(axis, bounds)| {
                evidence.axis = Some(axis as u8);
                invalid(
                    "packed clip dot finite and within f32",
                    Some(bounds),
                    &evidence,
                )
            })?;
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.mark(3);
            }
            evidence.clip_bits[i] = Some(clip[i].map(Interval::bits));
            let source_uv = vertex
                .uv0
                .ok_or_else(|| invalid("original UV0 available", None, &evidence))?;
            uv[i] = [source_uv.u(), source_uv.v()];
            evidence.uv_bits[i] = Some(uv[i].map(f32::to_bits));
            if !uv[i].iter().all(|v| v.is_finite()) {
                return Err(invalid("original UV0 finite", None, &evidence));
            }
        }
        evidence.corner = None;
        evidence.axis = None;
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.mark(1);
        }
        // Only a complete, certain homogeneous half-space exclusion can skip a
        // primitive. A clipped/partial primitive still receives the host checks.
        let outside = (0..7).any(|plane| {
            clip.iter().all(|p| match plane {
                0 => p[0].sub(p[3]).lo > 0.0,
                1 => p[0].neg().sub(p[3]).lo > 0.0,
                2 => p[1].sub(p[3]).lo > 0.0,
                3 => p[1].neg().sub(p[3]).lo > 0.0,
                4 => p[2].hi < 0.0,
                5 => p[2].sub(p[3]).lo > 0.0,
                _ => p[3].hi <= 0.0,
            })
        });
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.record.counts[13] += 1;
            c.mark(4);
        }
        if outside {
            summary.excluded += 1;
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.mark(7);
            }
            continue;
        }
        let max_w = clip.iter().map(|p| p[3].hi).fold(0.0, f64::max).next_up();
        if max_w <= 0.0 {
            return Err(invalid(
                "packed clip w upper > 0",
                Some(Interval::point(max_w)),
                &evidence,
            ));
        }
        let m = clip.map(|p| [p[0], p[1], p[3]]);
        let original_d = determinant(m);
        // Preserve both sufficient screen models. Failure to establish a
        // nonzero determinant is backend numeric indeterminacy, not invalid
        // source algebra. No primitive is omitted on that basis.
        let centered = !original_d.finite() || original_d.lo <= 0.0 && original_d.hi >= 0.0;
        let d = if centered {
            screen_determinant(m, true)
        } else {
            original_d
        };
        evidence.determinant_bits = Some(d.bits());
        let determinant_margin = if d.finite() && d.hi < 0.0 {
            Some(-d.hi)
        } else if d.finite() && d.lo > 0.0 {
            Some(d.lo)
        } else {
            None
        };
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.record.counts[if centered { 2 } else { 3 }] += 1;
            c.mark(5);
        }
        let helper_q_lower = (|| -> Option<f64> {
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.record.counts[4] += 1;
            }
            let q = plane(m, [1.0; 3], d, centered).ok()?;
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.record.counts[5] += 1;
            }
            evidence.q_plane_bits = Some(q.map(Interval::bits));
            evidence.axis = None;
            // This is the existing sufficient two-pixel helper model, not a
            // vendor coverage/interpolation precision contract. UV stays center
            // interpolated; an inconclusive enclosure is not a source refusal.
            let delta = helper_delta.unwrap_or_else(compute_helper_delta);
            #[cfg(any(test, feature = "test-harness"))]
            if !HOIST_HELPER_CONSTANTS {
                if let Some(c) = clock.as_mut() {
                    c.record.helper_delta_divisions += 2;
                }
            }
            let q_step = Interval::point(q[0].abs())
                .mul(Interval::point(delta[0]))
                .add(Interval::point(q[1].abs()).mul(Interval::point(delta[1])))
                .hi;
            let q_sum = Interval::point(q[0].abs())
                .mul(Interval::point(1.0).add(Interval::point(delta[0])))
                .add(
                    Interval::point(q[1].abs())
                        .mul(Interval::point(1.0).add(Interval::point(delta[1]))),
                )
                .add(Interval::point(q[2].abs()))
                .hi;
            let gamma7 = helper_gamma7.unwrap_or_else(compute_helper_gamma7);
            #[cfg(any(test, feature = "test-harness"))]
            if !HOIST_HELPER_CONSTANTS {
                if let Some(c) = clock.as_mut() {
                    c.record.helper_gamma7_evaluations += 1;
                    c.record.helper_constant_bits =
                        Some([delta[0].to_bits(), delta[1].to_bits(), gamma7.to_bits()]);
                }
            }
            let q_min_bound = Interval::point(1.0)
                .div(Interval::point(max_w))?
                .sub(Interval::point(q_step))
                .sub(Interval::point(gamma7).mul(Interval::point(q_sum)))
                .sub(Interval::point(7.0 * f64::from(f32::MIN_POSITIVE)));
            evidence.helper_q_min_bits = Some(q_min_bound.bits());
            let q_min = q_min_bound.lo;
            if !q_min.is_finite() || q_min <= 0.0 {
                return None;
            }
            #[cfg(any(test, feature = "test-harness"))]
            if let Some(c) = clock.as_mut() {
                c.record.counts[6] += 1;
            }
            let mut footprint = [0.0; 2];
            for axis in 0..2 {
                evidence.axis = Some(axis as u8);
                evidence.uv_plane_bits = None;
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    c.record.counts[7 + 2 * axis] += 1;
                }
                let p = plane(m, uv.map(|v| v[axis]), d, centered).ok()?;
                #[cfg(any(test, feature = "test-harness"))]
                if let Some(c) = clock.as_mut() {
                    c.record.counts[8 + 2 * axis] += 1;
                }
                evidence.uv_plane_bits = Some(p.map(Interval::bits));
                let max_uv = uv
                    .iter()
                    .map(|v| f64::from(v[axis]).abs())
                    .fold(0.0, f64::max)
                    .next_up();
                let numerator_step = Interval::point(p[0].abs())
                    .add(Interval::point(max_uv).mul(Interval::point(q[0].abs())))
                    .mul(Interval::point(delta[0]))
                    .add(
                        Interval::point(p[1].abs())
                            .add(Interval::point(max_uv).mul(Interval::point(q[1].abs())))
                            .mul(Interval::point(delta[1])),
                    )
                    .hi;
                let numerator_round = Interval::point(p[0].abs())
                    .mul(Interval::point(1.0).add(Interval::point(delta[0])))
                    .add(
                        Interval::point(p[1].abs())
                            .mul(Interval::point(1.0).add(Interval::point(delta[1]))),
                    )
                    .add(Interval::point(p[2].abs()))
                    .mul(Interval::point(gamma7))
                    .add(Interval::point(7.0 * f64::from(f32::MIN_POSITIVE)));
                let uv_bound = Interval::point(max_uv)
                    .add(
                        Interval::point(numerator_step)
                            .add(numerator_round)
                            .add(
                                Interval::point(max_uv)
                                    .mul(Interval::point(gamma7))
                                    .mul(Interval::point(q_sum)),
                            )
                            .div(Interval::point(q_min))?,
                    )
                    .hi;
                let stored_uv = rounded_upper(Interval::point(uv_bound));
                let difference = rounded_upper(Interval::point(2.0).mul(stored_uv));
                footprint[axis] =
                    rounded_upper(difference.mul(Interval::point(f64::from(role_extent[axis])))).hi;
                if !stored_uv.finite() || stored_uv.hi >= f64::from(f32::MAX) {
                    return None;
                }
            }
            let squared_bound = rounded_upper(
                rounded_upper(Interval::point(footprint[0]).mul(Interval::point(footprint[0])))
                    .add(rounded_upper(
                        Interval::point(footprint[1]).mul(Interval::point(footprint[1])),
                    )),
            );
            let squared = squared_bound.hi;
            if !squared.is_finite() || squared >= f64::from(f32::MAX) {
                return None;
            }
            Some(q_min)
        })();
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.record.counts[if helper_q_lower.is_some() { 11 } else { 12 }] += 1;
            c.mark(6);
        }
        summary.conditioned += usize::from(centered && determinant_margin.is_some());
        for (minimum, bound) in [
            (
                &mut summary.minimum_determinant_margin_bits,
                determinant_margin,
            ),
            (&mut summary.minimum_helper_q_lower_bits, helper_q_lower),
        ] {
            *minimum = if summary.qualified == 0 {
                bound.map(f64::to_bits)
            } else {
                minimum
                    .zip(bound)
                    .map(|(bits, bound)| f64::from_bits(bits).min(bound).to_bits())
            };
        }
        summary.qualified += 1;
        #[cfg(any(test, feature = "test-harness"))]
        if let Some(c) = clock.as_mut() {
            c.mark(7);
        }
    }
    #[cfg(any(test, feature = "test-harness"))]
    if let (Some(clock), Some(timing)) = (clock, timing) {
        clock.finish(timing);
    }
    Ok(summary)
}
const SURFACE_V2: &str = r#"
struct SampledSurface {value:Surface,dx:vec2<f32>,dy:vec2<f32>}
fn sample_surface(i:FragmentIn)->SampledSurface {
 let uv_dx=dpdx(i.uv0); let uv_dy=dpdy(i.uv0);
 var b=vec4(1.0);if present(0u){b=textureSampleGrad(base_tex,base_sampler,i.uv0,uv_dx,uv_dy);}
 var q=vec3(0.0,0.0,1.0);if present(1u){q=2.0*textureSampleGrad(normal_tex,normal_sampler,i.uv0,uv_dx,uv_dy).rgb-vec3(1.0);}
 var mr=vec2(1.0);if present(2u){mr=textureSampleGrad(mr_tex,mr_sampler,i.uv0,uv_dx,uv_dy).gb;}
 var a=1.0;if present(3u){a=textureSampleGrad(ao_tex,ao_sampler,i.uv0,uv_dx,uv_dy).r;}
 var e=vec3(1.0);if present(4u){e=textureSampleGrad(emissive_tex,emissive_sampler,i.uv0,uv_dx,uv_dy).rgb;}
 return SampledSurface(Surface(b.rgb*material.base_rgba.rgb*i.color.rgb,b.a*material.base_rgba.a*i.color.a,
  mr.y*material.mr_ao_cutoff.x,mr.x*material.mr_ao_cutoff.y,1.0+material.mr_ao_cutoff.z*(a-1.0),e*material.emissive_normal.xyz,q),uv_dx,uv_dy);
}
fn surface(i:FragmentIn)->Surface {return sample_surface(i).value;}
"#;

/// Fixed test-only source-stage sampling observation. It uses actual admitted
/// chain uploads and the authored sampler. Production gradient evidence instead
/// comes from the geometry-driven `sample_surface` fragment, never this probe.
#[cfg(test)]
pub(crate) fn sample_probe_for_test(
    context: &Context3D,
    chain: &MipChainLease3D,
    sampler: &MipSamplerLease3D,
    queries: &[[[f32; 4]; 2]],
) -> Result<Vec<[f32; 4]>, PbrError3D> {
    validate_pair(chain, sampler)?;
    const SHADER: &str = r#"
    struct Query {uv_lod_mode:vec4<f32>,gradients:vec4<f32>}
    @group(0) @binding(0) var image:texture_2d<f32>;
    @group(0) @binding(1) var authored:sampler;
    @group(0) @binding(2) var<storage,read> queries:array<Query>;
    @group(0) @binding(3) var<storage,read_write> output:array<vec4<f32>>;
    @compute @workgroup_size(1) fn observe(@builtin(global_invocation_id) id:vec3<u32>) {
      let q=queries[id.x];
      if q.uv_lod_mode.w==0.0 {
        output[id.x]=textureSampleLevel(image,authored,q.uv_lod_mode.xy,q.uv_lod_mode.z);
      } else {
        output[id.x]=textureSampleGrad(image,authored,q.uv_lod_mode.xy,q.gradients.xy,q.gradients.zw);
      }
    }
    "#;
    let gpu = context.gpu_resources()?;
    let n = queries.len() as u64;
    let input_bytes = bytemuck::cast_slice(queries);
    let out_size = n
        .checked_mul(16)
        .ok_or(error("sampling probe", "size overflow"))?;
    if n == 0
        || n > u64::from(gpu.device.limits().max_compute_workgroups_per_dimension)
        || input_bytes.len() as u64 > u64::from(gpu.device.limits().max_storage_buffer_binding_size)
        || out_size > u64::from(gpu.device.limits().max_storage_buffer_binding_size)
        || !queries.iter().flatten().flatten().all(|x| x.is_finite())
    {
        return Err(error(
            "sampling probe",
            "unqualified bounded numeric operands",
        ));
    }
    let storage = context.reserve_dynamic_storage(
        input_bytes.len() as u64 + 2 * out_size,
        0,
        1,
        input_bytes.len() as u64 + out_size,
    )?;
    let mut upload = context.begin_preparation_upload_budget();
    upload.consume(input_bytes.len() as u64)?;
    let (texture, pending) = context.prepare_mip_chain(chain, &mut upload)?;
    let sampler = context.prepare_mip_sampler(sampler, chain)?;
    let reservation = context.reserve_controlled_shader_module()?;
    let parsed = naga::front::wgsl::parse_str(SHADER)
        .map_err(|e| PbrError3D::Shader(e.emit_to_string(SHADER)))?;
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&parsed)
    .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    let mut pipeline_charge = context
        .raster_gpu()?
        .reserve_controlled_pipeline(context.limits().policy.max_pipeline_variants)?;
    let validation_scope = gpu.device.push_error_scope(wgpu::ErrorFilter::Validation);
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("R028 fixed test-only explicit-level/gradient source observer"),
            source: wgpu::ShaderSource::Wgsl(SHADER.into()),
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("R028 actual authored sampler source-stage observation"),
            layout: None,
            module: &module,
            entry_point: Some("observe"),
            compilation_options: Default::default(),
            cache: None,
        });
    let buffer = |label, size, usage| {
        gpu.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some(label),
            size,
            usage,
            mapped_at_creation: false,
        })
    };
    let input = buffer(
        "R028 qualified sampling operands",
        input_bytes.len() as u64,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
    );
    let output = buffer(
        "R028 actual source-stage samples",
        out_size,
        wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
    );
    let readback = buffer(
        "R028 numeric source-stage readback",
        out_size,
        wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
    );
    let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: Some("R028 held actual chain/sampler generations"),
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&texture.view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: wgpu::BindingResource::Sampler(&sampler.sampler),
            },
            wgpu::BindGroupEntry {
                binding: 2,
                resource: input.as_entire_binding(),
            },
            wgpu::BindGroupEntry {
                binding: 3,
                resource: output.as_entire_binding(),
            },
        ],
    });
    if let Some(e) = pollster::block_on(validation_scope.pop()) {
        return Err(PbrError3D::Shader(e.to_string()));
    }
    let module_charge = reservation.commit_one();
    pipeline_charge.commit();
    if let Some(pending) = pending {
        pending.commit(&gpu.queue);
    }
    gpu.queue.write_buffer(&input, 0, input_bytes);
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &binding, &[]);
        pass.dispatch_workgroups(n as u32, 1, 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, out_size);
    gpu.queue.submit([encoder.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |v| send.send(v).unwrap());
    gpu.device
        .poll(wgpu::PollType::wait_indefinitely())
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    receive
        .recv()
        .map_err(|e| PbrError3D::Shader(e.to_string()))?
        .map_err(|e| PbrError3D::Shader(e.to_string()))?;
    let data = readback.slice(..).get_mapped_range()
        .map_err(|error| PbrError3D::Shader(error.to_string()))?;
    let result = data
        .chunks_exact(16)
        .map(bytemuck::pod_read_unaligned)
        .collect();
    drop(data);
    readback.unmap();
    // Retire the actual objects before releasing their shared reservations.
    // Completion has been observed above, so none of these handles is in flight.
    drop(binding);
    drop(pipeline);
    drop(module);
    drop(input);
    drop(output);
    drop(readback);
    drop(texture);
    drop(sampler);
    drop((storage, module_charge, pipeline_charge));
    Ok(result)
}
