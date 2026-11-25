//! R-024's private, fixed material family. Source qualification is separate from
//! GPU preparation; illumination is absent here and emission is written once.

use std::sync::{Arc, Mutex};

use bytemuck::{Pod, Zeroable};

use super::context::{
    ControlledResourceError3D, ControlledResourceKey3D, ControlledResourceKind3D,
    ControlledResourceLease3D, SamplerLease, TextureLease,
};
#[cfg(test)]
pub(crate) use super::geometry::{
    PbrCertifiedInput3D, PbrFrameCertificate3D, PbrOriginalGeometry3D, PbrOriginalIdentity3D,
    PbrOriginalVertex3D, PbrSmoothSingularity3D, PbrTangentOrigin3D, PbrTriangleCertificate3D,
    PbrVertex52,
};
#[cfg(test)]
pub(crate) use super::geometry::{PbrGeometrySource3D, PbrUnitDirection3D, PbrVertexSource3D};
use super::mip_sampling::{
    MaterialSamplerLease3D, MaterialTextureLease3D, MipChainHandle3D, MipSamplerHandle3D,
    SamplingFamily3D,
};
#[cfg(test)]
pub(crate) use super::raster::material_brdf_probe_for_test;
#[cfg(test)]
pub(crate) use super::snapshot::MaterialPacketSource3D;
pub(crate) use super::snapshot::{MaterialModelState3D, MaterialSnapshot3D};
use super::{Context3D, Context3DError, SamplerHandle, TextureFormat3D, TextureHandle};

pub(crate) const MATERIAL_BYTES: u64 = 64;
pub(crate) const VERTEX_BYTES: u64 = 52;

/// Complete immutable producer association. Its certificate is supplied by the
/// source owner for the complete extension generation. D/E/G retain independent
/// captured generations; rendered pixels never become source authority.
pub(crate) struct MaterialSceneSource3D {
    pub(crate) snapshot: Arc<MaterialSnapshot3D>,
    pub(crate) model: Arc<MaterialModelState3D>,
    pub(crate) hdr: super::hdr::HdrSource3D,
    pub(crate) samples: u32,
    pub(crate) direct: Option<Arc<super::direct_shadow::DirectSource3D>>,
    pub(crate) indirect: Option<Arc<super::indirect::IndirectSource3D>>,
    #[cfg(test)]
    pub(crate) observe: bool,
}

impl std::fmt::Debug for MaterialSceneSource3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("MaterialSceneSource3D")
            .field("source", &self.hdr)
            .field("structural_revision", &self.snapshot.revision)
            .field("model_revision", &self.model.revision)
            .field("samples", &self.samples)
            .finish()
    }
}

/// Successful original source validation, retained by the requested-source owner.
/// The strong Arc replaces that owner's existing Arc; no proof allocation or
/// additional retained payload is introduced. It proves no view or GPU result.
#[derive(Debug, Clone)]
#[repr(transparent)]
pub(crate) struct ValidatedMaterialSceneSource3D(Arc<MaterialSceneSource3D>);

impl ValidatedMaterialSceneSource3D {
    pub(crate) fn new(
        context: &Context3D,
        source: Arc<MaterialSceneSource3D>,
    ) -> Result<Self, PbrError3D> {
        source.validate(context)?;
        Ok(Self(source))
    }

    pub(crate) fn source(&self) -> &Arc<MaterialSceneSource3D> {
        &self.0
    }

    pub(crate) fn matches(&self, context: &Context3D, source: &MaterialSceneSource3D) -> bool {
        std::ptr::eq(self.0.as_ref(), source)
            && context.same_resource_family(&self.0.snapshot.context)
            && context.device_failure().is_none()
            && match context.gpu_resources() {
                Ok(gpu) => gpu.private_hdr_owner_live(),
                // CPU-only source qualification has no renderer custody. It
                // remains distinct from all subsequent GPU/target admission.
                Err(Context3DError::GpuMaterializationUnavailable) => true,
                Err(_) => false,
            }
    }
}

impl std::ops::Deref for ValidatedMaterialSceneSource3D {
    type Target = Arc<MaterialSceneSource3D>;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// The original checker remains the only miss path. A capture never qualifies
/// another Arc or a foreign/retired Context. The capture covers every admitted
/// presence combination because its constructor validates the complete scene.
pub(crate) fn validate_material_scene_source(
    context: &Context3D,
    source: &MaterialSceneSource3D,
    captured: Option<&ValidatedMaterialSceneSource3D>,
) -> Result<(), PbrError3D> {
    if captured.is_some_and(|proof| proof.matches(context, source)) {
        Ok(())
    } else {
        source.validate(context)
    }
}

#[cfg(test)]
std::thread_local! {
    static FULL_SCENE_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
    static FULL_INDIRECT_VALIDATIONS: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

impl MaterialSceneSource3D {
    pub(crate) fn validate(&self, context: &Context3D) -> Result<(), PbrError3D> {
        #[cfg(test)]
        FULL_SCENE_VALIDATIONS.with(|count| count.set(count.get() + 1));
        if !context.same_resource_family(&self.snapshot.context)
            || !Arc::ptr_eq(&self.model.snapshot, &self.snapshot)
        {
            return Err(PbrError3D::Association(
                "material scene context/model/snapshot mismatch",
            ));
        }
        if !matches!(self.samples, 1 | 4) {
            return Err(PbrError3D::Association("source samples must be1 or4"));
        }
        self.hdr.exposure().map_err(|_| {
            PbrError3D::Association("missing/invalid producer radiance certificate or exposure")
        })?;
        if let Some(direct) = &self.direct {
            direct.validate(self)?;
        }
        if let Some(indirect) = &self.indirect {
            #[cfg(test)]
            FULL_INDIRECT_VALIDATIONS.with(|count| count.set(count.get() + 1));
            indirect.validate(self, context)?;
        }
        let mut bound = [0.0f32; 3];
        for packet in self.snapshot.packets.iter() {
            match packet {
                super::snapshot::CommittedMaterialPacket3D::PbrMaterial { material, .. } => {
                    for c in 0..3 {
                        bound[c] = bound[c].max(material.data.source.emissive_rgb[c]);
                    }
                }
                super::snapshot::CommittedMaterialPacket3D::LegacyAppearance { .. } => {
                    bound = [1.0; 3]
                }
            }
        }
        if (0..3).any(|c| self.hdr.maximum_rgb[c] < bound[c]) {
            return Err(PbrError3D::Association(
                "producer certificate does not bound all emission/legacy wrapper output",
            ));
        }
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn full_validation_count_for_test() -> usize {
        FULL_SCENE_VALIDATIONS.with(std::cell::Cell::get)
    }

    #[cfg(test)]
    pub(crate) fn full_indirect_validation_count_for_test() -> usize {
        FULL_INDIRECT_VALIDATIONS.with(std::cell::Cell::get)
    }

    pub(crate) fn validate_successor(current: &Self, next: &Self) -> Result<(), PbrError3D> {
        MaterialSnapshot3D::validate_successor(&current.snapshot, &next.snapshot)?;
        if next.model.revision < current.model.revision
            || (next.model.revision == current.model.revision
                && !Arc::ptr_eq(&next.model, &current.model))
        {
            return Err(PbrError3D::Association(
                "stale/reused complete immutable model identity",
            ));
        }
        if let (Some(old), Some(new)) = (&current.direct, &next.direct) {
            if !Arc::ptr_eq(old, new)
                && old.identity == new.identity
                && new.revision <= old.revision
            {
                return Err(PbrError3D::Association(
                    "stale/reused independent direct source generation",
                ));
            }
        }
        if let (Some(old), Some(new)) = (&current.indirect, &next.indirect) {
            if !Arc::ptr_eq(old, new)
                && old.identity == new.identity
                && new.revision <= old.revision
            {
                return Err(PbrError3D::Association(
                    "stale/reused independent indirect source generation",
                ));
            }
        }
        Ok(())
    }
}

/// One bounded refusal record. Float inputs and outward endpoints are retained
/// as IEEE bits so cloning/equality never round, erase NaNs or invent a bound.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UvGradientFailure3D {
    pub(crate) packet: usize,
    pub(crate) ordinal: u64,
    pub(crate) original: Option<super::geometry::PbrOriginalIdentity3D>,
    pub(crate) triangle: Option<usize>,
    pub(crate) render_triangle: Option<usize>,
    pub(crate) shadow_face: Option<u32>,
    pub(crate) operation: &'static str,
    pub(crate) corner: Option<u8>,
    pub(crate) axis: Option<u8>,
    pub(crate) bounds_bits: Option<[u64; 2]>,
    pub(crate) determinant_bits: Option<[u64; 2]>,
    pub(crate) helper_q_min_bits: Option<[u64; 2]>,
    pub(crate) extent: [u32; 2],
    pub(crate) role_extent: [u32; 2],
    pub(crate) object_bits: [[u32; 4]; 4],
    pub(crate) projection_bits: [[u32; 4]; 4],
    pub(crate) position_bits: [Option<[u32; 3]>; 3],
    pub(crate) uv_bits: [Option<[u32; 2]>; 3],
    pub(crate) clip_bits: [Option<[[u64; 2]; 4]>; 3],
    pub(crate) q_plane_bits: Option<[[u64; 2]; 3]>,
    pub(crate) uv_plane_bits: Option<[[u64; 2]; 3]>,
}

pub(super) fn uv_bound_text(bits: &Option<[u64; 2]>) -> String {
    match bits {
        Some([lo, hi]) => format!("[{}, {}]", f64::from_bits(*lo), f64::from_bits(*hi)),
        None => "unavailable".into(),
    }
}

impl std::fmt::Display for UvGradientFailure3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "R028 UV/gradient packet {} ordinal {} original {:?} triangle {:?} render {:?} shadow_face {:?}: {}; bounds={} determinant={} helper_q_min={}",
            self.packet,
            self.ordinal,
            self.original,
            self.triangle,
            self.render_triangle,
            self.shadow_face,
            self.operation,
            uv_bound_text(&self.bounds_bits),
            uv_bound_text(&self.determinant_bits),
            uv_bound_text(&self.helper_q_min_bits)
        )
    }
}

#[derive(Debug, thiserror::Error, Clone, PartialEq, Eq)]
pub(crate) enum PbrError3D {
    #[error("geometry CPU operation cancelled")]
    GeometryCpuCancelled,
    #[error("{0}")]
    UvGradient(Box<UvGradientFailure3D>),
    #[error("R028 {field}: {cause}")]
    Sampling {
        field: &'static str,
        cause: &'static str,
    },
    #[error("R028 normal domain {levels:?} cell{cell:?} MAG{mag}: {cause}")]
    NormalCertificate {
        levels: [u32; 2],
        cell: [u32; 4],
        mag: bool,
        cause: &'static str,
    },
    #[error("R026 {field}: {cause}")]
    Indirect {
        field: &'static str,
        cause: &'static str,
    },
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Math(#[from] super::Math3DError),
    #[error(transparent)]
    Resource(#[from] ControlledResourceError3D),
    #[error("R024 source {field}: {cause}")]
    Source {
        field: &'static str,
        cause: &'static str,
    },
    #[error("R024 required role {0:?} is missing")]
    RequiredRole(PbrRole3D),
    #[error("R024 role {role:?}: {cause}")]
    Role {
        role: PbrRole3D,
        cause: &'static str,
    },
    #[error("R024 geometry triangle {triangle}: {cause}")]
    Geometry {
        triangle: usize,
        cause: &'static str,
    },
    #[error("R024 certified triangle {triangle}, N{normal_corner:?}/T{tangent_corner:?}: {cause}")]
    FrameCertificate {
        triangle: usize,
        normal_corner: Option<u8>,
        tangent_corner: Option<u8>,
        cause: &'static str,
    },
    #[error("R024 source/model/view association: {0}")]
    Association(&'static str),
    #[error("R024 controlled shader/device: {0}")]
    Shader(String),
    #[error("R024 private owner lock unavailable: {0}")]
    Lock(&'static str),
    #[error("R025 source {field}: {cause}")]
    Direct {
        field: &'static str,
        cause: &'static str,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub(crate) enum PbrRole3D {
    Base = 0,
    Normal = 1,
    MetallicRoughness = 2,
    Occlusion = 3,
    Emissive = 4,
}

pub(crate) const ROLES: [PbrRole3D; 5] = [
    PbrRole3D::Base,
    PbrRole3D::Normal,
    PbrRole3D::MetallicRoughness,
    PbrRole3D::Occlusion,
    PbrRole3D::Emissive,
];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PbrRoleBinding3D {
    V1 {
        texture: TextureHandle,
        sampler: SamplerHandle,
    },
    V2 {
        texture: MipChainHandle3D,
        sampler: MipSamplerHandle3D,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PbrAlphaMode3D {
    Opaque,
    Mask,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PbrMaterialSource3D {
    pub(crate) identity: u64,
    pub(crate) revision: u64,
    pub(crate) base_rgba: [f32; 4],
    pub(crate) emissive_rgb: [f32; 3],
    pub(crate) normal_scale: f32,
    pub(crate) metallic: f32,
    pub(crate) roughness: f32,
    pub(crate) occlusion_strength: f32,
    pub(crate) cutoff: f32,
    pub(crate) alpha_mode: PbrAlphaMode3D,
    pub(crate) double_sided: bool,
    pub(crate) roles: [Option<PbrRoleBinding3D>; 5],
    pub(crate) required: [bool; 5],
    pub(crate) sampling: SamplingFamily3D,
}

impl PbrMaterialSource3D {
    pub(crate) fn defaults(identity: u64, revision: u64) -> Self {
        Self {
            identity,
            revision,
            base_rgba: [1.0; 4],
            emissive_rgb: [0.0; 3],
            normal_scale: 1.0,
            metallic: 1.0,
            roughness: 1.0,
            occlusion_strength: 1.0,
            cutoff: 0.5,
            alpha_mode: PbrAlphaMode3D::Opaque,
            double_sided: false,
            roles: std::array::from_fn(|_| None),
            required: [false; 5],
            sampling: SamplingFamily3D::V1,
        }
    }

    pub(crate) fn validate(&self) -> Result<(), PbrError3D> {
        if self.identity == 0 || self.revision == 0 {
            return Err(PbrError3D::Source {
                field: "material identity/revision",
                cause: "missing",
            });
        }
        for (field, values) in [
            ("base RGBA", self.base_rgba.as_slice()),
            ("emissive RGB", self.emissive_rgb.as_slice()),
            (
                "metallic/roughness/AO strength",
                &[self.metallic, self.roughness, self.occlusion_strength][..],
            ),
        ] {
            if !values
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
            {
                return Err(PbrError3D::Source {
                    field,
                    cause: "must be finite in [0,1]",
                });
            }
        }
        if !self.normal_scale.is_finite() || !(-1024.0..=1024.0).contains(&self.normal_scale) {
            return Err(PbrError3D::Source {
                field: "normal scale",
                cause: "must be finite in [-1024,1024]",
            });
        }
        if !self.cutoff.is_finite() || self.cutoff < 0.0 {
            return Err(PbrError3D::Source {
                field: "alpha cutoff",
                cause: "must be finite and nonnegative",
            });
        }
        for role in ROLES {
            if self.required[role as usize] && self.roles[role as usize].is_none() {
                return Err(PbrError3D::RequiredRole(role));
            }
            if let Some(binding) = &self.roles[role as usize] {
                if !matches!(
                    (self.sampling, binding),
                    (SamplingFamily3D::V1, PbrRoleBinding3D::V1 { .. })
                        | (SamplingFamily3D::V2, PbrRoleBinding3D::V2 { .. })
                ) {
                    return Err(PbrError3D::Association(
                        "role handle family disagrees with explicit material sampling profile",
                    ));
                }
            }
        }
        Ok(())
    }

    pub(crate) fn block(&self) -> MaterialBlock64 {
        let bits = self
            .roles
            .iter()
            .enumerate()
            .fold(0u32, |b, (i, r)| b | (u32::from(r.is_some()) << i));
        MaterialBlock64 {
            base_rgba: self.base_rgba,
            emissive_normal: [
                self.emissive_rgb[0],
                self.emissive_rgb[1],
                self.emissive_rgb[2],
                self.normal_scale,
            ],
            mr_ao_cutoff: [
                self.metallic,
                self.roughness,
                self.occlusion_strength,
                self.cutoff,
            ],
            role_mode: [
                bits,
                u32::from(self.alpha_mode == PbrAlphaMode3D::Mask),
                0,
                0,
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct MaterialBlock64 {
    pub(crate) base_rgba: [f32; 4],
    pub(crate) emissive_normal: [f32; 4],
    pub(crate) mr_ao_cutoff: [f32; 4],
    pub(crate) role_mode: [u32; 4],
}
const _: [(); 64] = [(); std::mem::size_of::<MaterialBlock64>()];

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PbrMaterialHandle3D(pub(super) ControlledResourceKey3D);

pub(crate) struct PbrMaterialData3D {
    pub(crate) source: PbrMaterialSource3D,
    pub(crate) textures: [MaterialTextureLease3D; 5],
    pub(crate) samplers: [MaterialSamplerLease3D; 5],
    _family: Arc<PbrState3D>,
}

#[derive(Clone)]
pub(crate) struct PbrMaterialLease3D {
    pub(crate) handle: PbrMaterialHandle3D,
    pub(crate) data: Arc<PbrMaterialData3D>,
    pub(super) ownership: ControlledResourceLease3D,
}

impl PbrMaterialLease3D {
    pub(crate) fn belongs_to(&self, context: &Context3D) -> bool {
        self.ownership.belongs_to(context)
    }
}

pub(super) struct PbrDefaults3D {
    textures: [TextureLease; 5],
    samplers: [SamplerLease; 5],
}

pub(super) struct PbrState3D {
    defaults: PbrDefaults3D,
    pub(super) program: Mutex<Option<Arc<super::raster::PbrProgram3D>>>,
}

impl PbrState3D {
    pub(super) fn default_handles(&self) -> Vec<(TextureHandle, SamplerHandle)> {
        (0..5)
            .map(|i| {
                (
                    self.defaults.textures[i].handle().clone(),
                    self.defaults.samplers[i].handle().clone(),
                )
            })
            .collect()
    }
    pub(super) fn new(context: &Context3D) -> Result<Self, PbrError3D> {
        let mut textures = Vec::new();
        let mut samplers = Vec::new();
        let result = (|| {
            for role in ROLES {
                let color = matches!(role, PbrRole3D::Base | PbrRole3D::Emissive);
                let texels = match role {
                    PbrRole3D::Normal => [128, 128, 255, 255],
                    PbrRole3D::MetallicRoughness => [0, 255, 255, 255],
                    PbrRole3D::Occlusion => [255, 0, 0, 255],
                    _ => [255; 4],
                };
                let texture = super::Texture3D::try_new(
                    1,
                    1,
                    if color {
                        TextureFormat3D::Rgba8UnormSrgb
                    } else {
                        TextureFormat3D::Rgba8Unorm
                    },
                    texels.to_vec(),
                )
                .map_err(|_| PbrError3D::Source {
                    field: "default texture",
                    cause: "invalid constant",
                })?;
                let texture = context.register_texture(texture)?;
                textures.push(texture);
                samplers.push(context.register_sampler(super::Sampler3D::default())?);
            }
            let texture_leases = textures.clone();
            let sampler_leases = samplers.clone();
            Ok(Self {
                defaults: PbrDefaults3D {
                    textures: texture_leases
                        .try_into()
                        .map_err(|_| PbrError3D::Association("five default textures"))?,
                    samplers: sampler_leases
                        .try_into()
                        .map_err(|_| PbrError3D::Association("five default samplers"))?,
                },
                program: Mutex::new(None),
            })
        })();
        if result.is_err() {
            for handle in textures {
                let _ = context.retire_texture(handle.handle());
            }
            for handle in samplers {
                let _ = context.retire_sampler(handle.handle());
            }
        }
        result
    }
}

impl Context3D {
    pub(crate) fn register_pbr_material(
        &self,
        source: PbrMaterialSource3D,
        previous: Option<&PbrMaterialHandle3D>,
    ) -> Result<PbrMaterialHandle3D, PbrError3D> {
        source.validate()?;
        // Validate every present role BEFORE allocating defaults, including a
        // provided role whose factor happens to be zero.
        let mut present = Vec::with_capacity(5);
        for role in ROLES {
            let leases = if let Some(binding) = &source.roles[role as usize] {
                let (texture, sampler) = match binding {
                    PbrRoleBinding3D::V1 { texture, sampler } => (
                        MaterialTextureLease3D::V1(self.lease_texture(texture)?),
                        MaterialSamplerLease3D::V1(self.lease_sampler(sampler)?),
                    ),
                    PbrRoleBinding3D::V2 { texture, sampler } => {
                        let texture = self.lease_mip_chain(texture)?;
                        let sampler = self.lease_mip_sampler(sampler)?;
                        super::mip_sampling::validate_pair(&texture, &sampler)?;
                        if !texture.source.role.matches(role, source.alpha_mode) {
                            return Err(PbrError3D::Role {
                                role,
                                cause: "wrong V2 source role/Mask generation semantics",
                            });
                        }
                        if role == PbrRole3D::Normal {
                            texture.validate_normal_material(&sampler, source.normal_scale)?;
                        }
                        (
                            MaterialTextureLease3D::V2(texture),
                            MaterialSamplerLease3D::V2(sampler),
                        )
                    }
                };
                let required_format = if matches!(role, PbrRole3D::Base | PbrRole3D::Emissive) {
                    TextureFormat3D::Rgba8UnormSrgb
                } else {
                    TextureFormat3D::Rgba8Unorm
                };
                if texture.descriptor().format() != required_format {
                    return Err(PbrError3D::Role {
                        role,
                        cause: "wrong source role encoding/format",
                    });
                }
                if role == PbrRole3D::Normal
                    && matches!(texture, MaterialTextureLease3D::V1(_))
                    && texture
                        .descriptor()
                        .texels()
                        .chunks_exact(4)
                        .any(|p| p[2] < 128)
                {
                    return Err(PbrError3D::Role {
                        role,
                        cause: "source normal blue below128",
                    });
                }
                Some((texture, sampler))
            } else {
                None
            };
            present.push(leases);
        }
        if let Some(old) = previous {
            let old = self.lease_pbr_material(old)?;
            if source.identity != old.data.source.identity
                || source.revision <= old.data.source.revision
            {
                return Err(PbrError3D::Association(
                    "material successor must retain source identity and advance revision",
                ));
            }
        }
        let state = self.pbr_state()?;
        let textures = std::array::from_fn(|i| {
            present[i].as_ref().map_or_else(
                || MaterialTextureLease3D::V1(state.defaults.textures[i].clone()),
                |p| p.0.clone(),
            )
        });
        let samplers = std::array::from_fn(|i| {
            present[i].as_ref().map_or_else(
                || MaterialSamplerLease3D::V1(state.defaults.samplers[i].clone()),
                |p| p.1.clone(),
            )
        });
        let bytes = std::mem::size_of::<PbrMaterialData3D>() as u64;
        let data = Arc::new(PbrMaterialData3D {
            source,
            textures,
            samplers,
            _family: state,
        });
        Ok(PbrMaterialHandle3D(self.register_controlled_source(
            ControlledResourceKind3D::PbrMaterial,
            data,
            bytes,
            previous.map(|p| &p.0),
        )?))
    }

    pub(crate) fn lease_pbr_material(
        &self,
        handle: &PbrMaterialHandle3D,
    ) -> Result<PbrMaterialLease3D, PbrError3D> {
        let ownership =
            self.lease_controlled_source(&handle.0, ControlledResourceKind3D::PbrMaterial)?;
        let data = ownership.source()?;
        Ok(PbrMaterialLease3D {
            handle: handle.clone(),
            data,
            ownership,
        })
    }

    pub(crate) fn retire_pbr_material(
        &self,
        handle: &PbrMaterialHandle3D,
    ) -> Result<(), PbrError3D> {
        self.retire_controlled_source(&handle.0, ControlledResourceKind3D::PbrMaterial)?;
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PbrGeometryHandle3D(pub(super) ControlledResourceKey3D);

pub(crate) struct PbrGeometryData3D {
    pub(crate) source: super::geometry::PbrGeometrySource3D,
    pub(super) gpu: Mutex<Option<Arc<super::raster::PbrGeometryGpu3D>>>,
    pub(super) direct_gpu: Mutex<Option<std::sync::Weak<super::raster::PbrGeometryGpu3D>>>,
}

#[derive(Clone)]
pub(crate) struct PbrGeometryLease3D {
    pub(crate) handle: PbrGeometryHandle3D,
    pub(crate) data: Arc<PbrGeometryData3D>,
    pub(super) ownership: ControlledResourceLease3D,
}

impl PbrGeometryLease3D {
    pub(crate) fn belongs_to(&self, context: &Context3D) -> bool {
        self.ownership.belongs_to(context)
    }
}

pub(super) fn budget(
    context: &Context3D,
    kind: super::PolicyBudget3D,
    bytes: u64,
    maximum: u64,
) -> Result<(), PbrError3D> {
    let _ = context;
    if bytes > maximum {
        return Err(Context3DError::PolicyBudgetExceeded {
            budget: kind,
            requested: bytes,
            maximum,
        }
        .into());
    }
    Ok(())
}

impl Context3D {
    pub(crate) fn register_pbr_geometry(
        &self,
        source: super::geometry::PbrGeometrySource3D,
        previous: Option<&PbrGeometryHandle3D>,
    ) -> Result<PbrGeometryHandle3D, PbrError3D> {
        source.validate_certified_registration()?;
        if source.certified_input().is_some() {
            if let Some(previous) = previous {
                let prior = self.lease_pbr_geometry(previous)?;
                source.validate_certified_successor(&prior.data.source)?;
            }
        }
        self.insert_pbr_geometry_source(source, previous)
    }

    pub(super) fn register_cpu_geometry(
        &self,
        source: super::geometry::AdmissibleCpuGeometry3D,
        previous: Option<&PbrGeometryHandle3D>,
    ) -> Result<PbrGeometryHandle3D, PbrError3D> {
        self.insert_pbr_geometry_source(source.into_source(), previous)
    }

    pub(super) fn check_pbr_geometry_limits(
        &self,
        source: &super::geometry::PbrGeometrySource3D,
    ) -> Result<u64, PbrError3D> {
        let vertices = u64::try_from(source.vertices.len())
            .map_err(|_| PbrError3D::Association("vertex count"))?;
        let elements = u64::try_from(source.element_count())
            .map_err(|_| PbrError3D::Association("element count"))?;
        let index_bytes = match &source.indices {
            super::GeometryIndices::Unindexed => Some(0),
            super::GeometryIndices::U16(v) => (v.len() as u64).checked_mul(2),
            super::GeometryIndices::U32(v) => (v.len() as u64).checked_mul(4),
        }
        .ok_or(Context3DError::ArithmeticOverflow {
            operation: "sizing PBR indices",
        })?;
        let vertex_bytes =
            vertices
                .checked_mul(VERTEX_BYTES)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "sizing PBR vertices",
                })?;
        let total =
            vertex_bytes
                .checked_add(index_bytes)
                .ok_or(Context3DError::ArithmeticOverflow {
                    operation: "sizing PBR geometry",
                })?;
        let p = self.limits().policy;
        budget(
            self,
            super::PolicyBudget3D::MeshVertices,
            vertices,
            u64::from(p.max_vertices_per_mesh),
        )?;
        budget(
            self,
            super::PolicyBudget3D::MeshIndices,
            elements,
            u64::from(p.max_indices_per_mesh),
        )?;
        budget(
            self,
            super::PolicyBudget3D::MeshBytes,
            total,
            p.max_mesh_bytes,
        )?;
        if vertex_bytes > self.limits().hard.max_buffer_size
            || index_bytes > self.limits().hard.max_buffer_size
        {
            return Err(Context3DError::HardDeviceLimitExceeded {
                limit: super::HardLimit3D::BufferSize,
                requested: vertex_bytes.max(index_bytes),
                maximum: self.limits().hard.max_buffer_size,
            }
            .into());
        }
        let metadata = vertices
            .checked_mul(std::mem::size_of::<super::geometry::PbrVertexSource3D>() as u64)
            .and_then(|v| v.checked_add(index_bytes))
            .and_then(|v| v.checked_add(std::mem::size_of::<PbrGeometryData3D>() as u64))
            .and_then(|v| v.checked_add(source.certificate_metadata_bytes()))
            .ok_or(Context3DError::ArithmeticOverflow {
                operation: "sizing PBR source metadata",
            })?;
        Ok(metadata)
    }

    fn insert_pbr_geometry_source(
        &self,
        source: super::geometry::PbrGeometrySource3D,
        previous: Option<&PbrGeometryHandle3D>,
    ) -> Result<PbrGeometryHandle3D, PbrError3D> {
        let metadata = self.check_pbr_geometry_limits(&source)?;
        let data = Arc::new(PbrGeometryData3D {
            source,
            gpu: Mutex::new(None),
            direct_gpu: Mutex::new(None),
        });
        Ok(PbrGeometryHandle3D(self.register_controlled_source(
            ControlledResourceKind3D::PbrGeometry,
            data,
            metadata,
            previous.map(|p| &p.0),
        )?))
    }

    pub(crate) fn lease_pbr_geometry(
        &self,
        handle: &PbrGeometryHandle3D,
    ) -> Result<PbrGeometryLease3D, PbrError3D> {
        let ownership =
            self.lease_controlled_source(&handle.0, ControlledResourceKind3D::PbrGeometry)?;
        let data = ownership.source()?;
        Ok(PbrGeometryLease3D {
            handle: handle.clone(),
            data,
            ownership,
        })
    }

    pub(crate) fn retire_pbr_geometry(
        &self,
        handle: &PbrGeometryHandle3D,
    ) -> Result<(), PbrError3D> {
        self.retire_controlled_source(&handle.0, ControlledResourceKind3D::PbrGeometry)?;
        Ok(())
    }
}

/// The exact fixed evaluator shared by the material fragment and separately
/// authorized test observations. No light/environment globals are declared.
pub(crate) const PBR_WGSL: &str = r#"
struct View80 { view_projection:mat4x4<f32>, camera_relative:vec3<f32>, pad:f32 }
struct Object112 { model_relative:mat4x4<f32>, normal_to_world:mat3x3<f32> }
struct Material64 { base_rgba:vec4<f32>, emissive_normal:vec4<f32>, mr_ao_cutoff:vec4<f32>, role_mode:vec4<u32> }
@group(0) @binding(0) var<uniform> view:View80;
@group(1) @binding(0) var<uniform> object:Object112;
@group(2) @binding(0) var<uniform> material:Material64;
@group(2) @binding(1) var base_tex:texture_2d<f32>;
@group(2) @binding(2) var normal_tex:texture_2d<f32>;
@group(2) @binding(3) var mr_tex:texture_2d<f32>;
@group(2) @binding(4) var ao_tex:texture_2d<f32>;
@group(2) @binding(5) var emissive_tex:texture_2d<f32>;
@group(2) @binding(6) var base_sampler:sampler;
@group(2) @binding(7) var normal_sampler:sampler;
@group(2) @binding(8) var mr_sampler:sampler;
@group(2) @binding(9) var ao_sampler:sampler;
@group(2) @binding(10) var emissive_sampler:sampler;
struct VertexIn {
 @location(0) position:vec3<f32>, @location(1) normal:vec3<f32>,
 @location(2) uv0:vec2<f32>, @location(3) color:vec4<f32>, @location(4) tangent:vec4<f32>
}
struct FragmentIn {
 @builtin(position) clip:vec4<f32>,
 @location(0) @interpolate(perspective,centroid) position:vec3<f32>,
 @location(1) @interpolate(perspective,centroid) normal:vec3<f32>,
 @location(2) @interpolate(perspective,center) uv0:vec2<f32>,
 @location(3) @interpolate(perspective,centroid) color:vec4<f32>,
 @location(4) @interpolate(perspective,centroid) tangent:vec4<f32>
}
@vertex fn pbr_vertex(v:VertexIn)->FragmentIn {
 var o:FragmentIn; let p=object.model_relative*vec4(v.position,1.0);
 o.clip=view.view_projection*p; o.position=p.xyz; o.normal=v.normal;
 o.uv0=v.uv0; o.color=v.color; o.tangent=v.tangent; return o;
}
fn present(bit:u32)->bool { return (material.role_mode.x & (1u<<bit))!=0u; }
// For N/T/decoded Q this nonzero premise is certified by the covered-centroid
// source cones/positive-Z rule before dispatch; no replacement frame is used.
fn unit_nonzero(v:vec3<f32>)->vec3<f32> {
 let m=max(max(abs(v.x),abs(v.y)),abs(v.z)); let a=v/m;
 return a*inverseSqrt(dot(a,a));
}
struct Frame { N:vec3<f32>, T:vec3<f32>, g:vec3<f32>, local_n:vec3<f32> }
fn frame(i:FragmentIn,q_sample:vec3<f32>,front:bool)->Frame {
 let n=i.normal; var N=unit_nonzero(object.normal_to_world*n);
 var T=vec3(0.0); var g=vec3(0.0);
 if present(1u) {
  let t=i.tangent.xyz; g=t-n*(dot(n,t)/dot(n,n));
  let linear=mat3x3<f32>(object.model_relative[0].xyz,object.model_relative[1].xyz,object.model_relative[2].xyz);
  T=unit_nonzero(linear*g);
  // Only numerical orthogonality correction; the local GS remains decisive.
  T=unit_nonzero(T-N*dot(N,T));
  let B=i.tangent.w*cross(N,T);
  let q=unit_nonzero(vec3(q_sample.xy*material.emissive_normal.w,q_sample.z));
  N=unit_nonzero(T*q.x+B*q.y+N*q.z);
 }
 if !front {N=-N;}
 return Frame(N,T,g,n);
}
struct Surface {
 base:vec3<f32>, alpha:f32, metallic:f32, roughness:f32, ao:f32,
 emission:vec3<f32>, normal_sample:vec3<f32>
}
fn surface(i:FragmentIn)->Surface {
 // Material presence is draw-uniform. All sampling precedes nonuniform Mask
 // discard/frame/direction branches; numeric roles never undergo color decode.
 var b=vec4(1.0); if present(0u) {b=textureSample(base_tex,base_sampler,i.uv0);}
 var q=vec3(0.0,0.0,1.0); if present(1u) {q=2.0*textureSample(normal_tex,normal_sampler,i.uv0).rgb-vec3(1.0);}
 var mr=vec2(1.0); if present(2u) {mr=textureSample(mr_tex,mr_sampler,i.uv0).gb;}
 var a=1.0; if present(3u) {a=textureSample(ao_tex,ao_sampler,i.uv0).r;}
 var e=vec3(1.0); if present(4u) {e=textureSample(emissive_tex,emissive_sampler,i.uv0).rgb;}
 return Surface(b.rgb*material.base_rgba.rgb*i.color.rgb,b.a*material.base_rgba.a*i.color.a,
  mr.y*material.mr_ao_cutoff.x,mr.x*material.mr_ao_cutoff.y,
  1.0+material.mr_ao_cutoff.z*(a-1.0),e*material.emissive_normal.xyz,q);
}
fn reflected_brdf(c:vec3<f32>,m:f32,r:f32,N:vec3<f32>,V_in:vec3<f32>,L_in:vec3<f32>)->vec3<f32> {
 let vm=max(max(abs(V_in.x),abs(V_in.y)),abs(V_in.z));
 let lm=max(max(abs(L_in.x),abs(L_in.y)),abs(L_in.z));
 if vm==0.0 || lm==0.0 {return vec3(0.0);}
 let V=unit_nonzero(V_in); let L=unit_nonzero(L_in);
 let nv=clamp(dot(N,V),-1.0,1.0); let nl=clamp(dot(N,L),-1.0,1.0);
 if nv<=0.0 || nl<=0.0 {return vec3(0.0);}
 let h=V+L; let hm=max(max(abs(h.x),abs(h.y)),abs(h.z));
 if hm==0.0 {return vec3(0.0);}
 let H=unit_nonzero(h); let nh=clamp(dot(N,H),0.0,1.0); let vh=clamp(dot(V,H),0.0,1.0);
 let re=max(r,0.045); let alpha=re*re; let a2=alpha*alpha;
 let f0=(1.0-m)*vec3(0.04)+m*c;
 let s=1.0-vh; let F=f0+(vec3(1.0)-f0)*(s*s*s*s*s);
 let nh2=nh*nh; let d=(1.0-nh2)+a2*nh2;
 let D=a2/(3.141592653589793*d*d);
 let vl=nl+sqrt(a2+(1.0-a2)*nl*nl);
 let vv=nv+sqrt(a2+(1.0-a2)*nv*nv);
 return (vec3(1.0)-F)*(1.0-m)*c/3.141592653589793 + F*D/(vl*vv);
}
@fragment fn pbr_fragment(i:FragmentIn,@builtin(front_facing) front:bool)->@location(0) vec4<f32> {
 let s=surface(i);
 if material.role_mode.y==1u && s.alpha<material.mr_ao_cutoff.w {discard;}
 let f=frame(i,s.normal_sample,front);
 // R025 direct and R026 indirect are absent. No implicit source is invented;
 // AO and base color cannot attenuate this independent emission.
 return vec4(s.emission,1.0);
}
"#;
