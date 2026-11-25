//! Checked, opt-in unlinked material-source ingress (HAL-370/R-024–028).
//!
//! Descriptors are producer facts in ORIGINAL mesh-local / HAL WORLD space.
//! Admission freezes them through the existing Context3D resource owners; this
//! module neither imports assets nor creates tangents, mips, lights or shaders.
//! This engineering surface is a candidate for separately qualified S3 use.
//!
//! Admit immutable geometry and each present texture/sampler first, then a
//! material, flat packet snapshot, matching model, and complete certified scene.
//! `Viewport3D::new_pbr` or `request_pbr_scene` consumes that scene through the
//! original unlinked writer. A new complete scene requires an advancing stable
//! HDR identity/revision; D, E, G, structural and model generations remain
//! independently captured. Missing optional sources stay absent.
//!
//! Returned handles are same-context generations, not raw GPU resources. Retiring
//! a handle prevents new source admission from it; snapshots and committed or
//! in-flight fronts retain their original leases until the existing owner releases
//! them. Descriptor mutation after admission cannot change an admitted generation.

use std::sync::Arc;

use super::{
    Context3D, Context3DError, GeometryIndices, Math3DError, ModelPlacement3, ModelRevision,
    Orientation3, RasterMode, SamplerAddressMode, SamplerFilterMode, SamplerHandle,
    StructuralRevision, TextureFormat3D, TextureHandle, WorldPoint3,
};
use super::{
    context, direct_shadow as d, geometry as g, hdr, indirect as i, mip_sampling as m, pbr as p,
    snapshot as s,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrTextureRole3D {
    Base,
    Normal,
    MetallicRoughness,
    Occlusion,
    Emissive,
}
impl From<p::PbrRole3D> for PbrTextureRole3D {
    fn from(v: p::PbrRole3D) -> Self {
        match v {
            p::PbrRole3D::Base => Self::Base,
            p::PbrRole3D::Normal => Self::Normal,
            p::PbrRole3D::MetallicRoughness => Self::MetallicRoughness,
            p::PbrRole3D::Occlusion => Self::Occlusion,
            p::PbrRole3D::Emissive => Self::Emissive,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum PbrSourceError3D {
    #[error(transparent)]
    Context(#[from] Context3DError),
    #[error(transparent)]
    Math(#[from] Math3DError),
    #[error(transparent)]
    Viewport(#[from] super::Viewport3DError),
    #[error("HDR output: {0}")]
    Output(&'static str),
    #[error("required {0:?} source role is absent")]
    RequiredRole(PbrTextureRole3D),
    #[error("{role:?}: {cause}")]
    Role {
        role: PbrTextureRole3D,
        cause: &'static str,
    },
    #[error("source {field}: {cause}")]
    Source {
        field: &'static str,
        cause: &'static str,
    },
    #[error("R028 {field}: {cause}")]
    Sampling {
        field: &'static str,
        cause: &'static str,
    },
    /// Attributable R028 numeric refusal. None for `shadow_face` denotes the main
    /// scene; Some identifies the actual R025 face. Triangle is original when a
    /// certified source exists, otherwise the original legacy-input element
    /// triangle. Missing endpoints mean that stage did not establish a bound.
    /// All *_bits endpoints are exact IEEE binary64; packed inputs are binary32.
    #[error("R028 UV/gradient packet {packet} ordinal {ordinal} original {original:?} triangle {triangle:?} render {render_triangle:?} shadow_face {shadow_face:?}: {operation}; bounds={} determinant={} helper_q_min={}", p::uv_bound_text(.bounds_bits), p::uv_bound_text(.determinant_bits), p::uv_bound_text(.helper_q_min_bits))]
    UvGradient {
        packet: usize,
        ordinal: u64,
        original: Option<PbrOriginalIdentity3D>,
        triangle: Option<usize>,
        render_triangle: Option<usize>,
        shadow_face: Option<u32>,
        operation: &'static str,
        corner: Option<u8>,
        axis: Option<u8>,
        bounds_bits: Option<[u64; 2]>,
        determinant_bits: Option<[u64; 2]>,
        helper_q_min_bits: Option<[u64; 2]>,
        extent: [u32; 2],
        role_extent: [u32; 2],
        object_bits: [[u32; 4]; 4],
        projection_bits: [[u32; 4]; 4],
        position_bits: [Option<[u32; 3]>; 3],
        uv_bits: [Option<[u32; 2]>; 3],
        clip_bits: [Option<[[u64; 2]; 4]>; 3],
        q_plane_bits: Option<[[u64; 2]; 3]>,
        uv_plane_bits: Option<[[u64; 2]; 3]>,
    },
    #[error("R025 {field}: {cause}")]
    Direct {
        field: &'static str,
        cause: &'static str,
    },
    #[error("R026 {field}: {cause}")]
    Indirect {
        field: &'static str,
        cause: &'static str,
    },
    #[error("triangle {triangle}: {cause}")]
    Geometry {
        triangle: usize,
        cause: &'static str,
    },
    #[error("triangle {triangle} N{normal_corner:?}/T{tangent_corner:?}: {cause}")]
    Frame {
        triangle: usize,
        normal_corner: Option<u8>,
        tangent_corner: Option<u8>,
        cause: &'static str,
    },
    #[error("normal domain {levels:?} cell{cell:?} MAG{mag}: {cause}")]
    NormalDomain {
        levels: [u32; 2],
        cell: [u32; 4],
        mag: bool,
        cause: &'static str,
    },
    #[error("source association: {0}")]
    Association(&'static str),
    #[error("resource slot {slot} generation {generation} is stale")]
    Stale { slot: u32, generation: u64 },
    #[error("resource kind/payload does not match the typed source")]
    ResourceKind,
    #[error("controlled GPU: {0}")]
    Gpu(String),
    #[error("owner lock: {0}")]
    Lock(&'static str),
}
impl From<p::PbrError3D> for PbrSourceError3D {
    fn from(v: p::PbrError3D) -> Self {
        match v {
            p::PbrError3D::GeometryCpuCancelled => {
                Self::Association("geometry CPU operation cancelled")
            }
            p::PbrError3D::UvGradient(e) => Self::UvGradient {
                packet: e.packet,
                ordinal: e.ordinal,
                original: e.original.map(|v| PbrOriginalIdentity3D {
                    source: v.source,
                    revision: v.revision,
                    node: v.node,
                    mesh: v.mesh,
                    primitive: v.primitive,
                }),
                triangle: e.triangle,
                render_triangle: e.render_triangle,
                shadow_face: e.shadow_face,
                operation: e.operation,
                corner: e.corner,
                axis: e.axis,
                bounds_bits: e.bounds_bits,
                determinant_bits: e.determinant_bits,
                helper_q_min_bits: e.helper_q_min_bits,
                extent: e.extent,
                role_extent: e.role_extent,
                object_bits: e.object_bits,
                projection_bits: e.projection_bits,
                position_bits: e.position_bits,
                uv_bits: e.uv_bits,
                clip_bits: e.clip_bits,
                q_plane_bits: e.q_plane_bits,
                uv_plane_bits: e.uv_plane_bits,
            },
            p::PbrError3D::Context(e) => Self::Context(e),
            p::PbrError3D::Math(e) => Self::Math(e),
            p::PbrError3D::Resource(e) => match e {
                context::ControlledResourceError3D::Context(e) => Self::Context(e),
                context::ControlledResourceError3D::Stale { slot, generation } => {
                    Self::Stale { slot, generation }
                }
                _ => Self::ResourceKind,
            },
            p::PbrError3D::Sampling { field, cause } => Self::Sampling { field, cause },
            p::PbrError3D::NormalCertificate {
                levels,
                cell,
                mag,
                cause,
            } => Self::NormalDomain {
                levels,
                cell,
                mag,
                cause,
            },
            p::PbrError3D::Indirect { field, cause } => Self::Indirect { field, cause },
            p::PbrError3D::Source { field, cause } => Self::Source { field, cause },
            p::PbrError3D::RequiredRole(r) => Self::RequiredRole(r.into()),
            p::PbrError3D::Role { role, cause } => Self::Role {
                role: role.into(),
                cause,
            },
            p::PbrError3D::Geometry { triangle, cause } => Self::Geometry { triangle, cause },
            p::PbrError3D::FrameCertificate {
                triangle,
                normal_corner,
                tangent_corner,
                cause,
            } => Self::Frame {
                triangle,
                normal_corner,
                tangent_corner,
                cause,
            },
            p::PbrError3D::Association(e) => Self::Association(e),
            p::PbrError3D::Shader(e) => Self::Gpu(e),
            p::PbrError3D::Lock(e) => Self::Lock(e),
            p::PbrError3D::Direct { field, cause } => Self::Direct { field, cause },
        }
    }
}
pub type PbrSourceResult3D<T> = Result<T, PbrSourceError3D>;

fn conversion_bytes(parts: &[(usize, usize)]) -> PbrSourceResult3D<u64> {
    parts.iter().try_fold(256u64, |bytes, &(count, size)| {
        (count as u64)
            .checked_mul(size as u64)
            .and_then(|v| bytes.checked_add(v))
            .ok_or(PbrSourceError3D::Association(
                "source adapter storage size overflow",
            ))
    })
}

fn convert_slice<T, U>(input: &[T], convert: impl FnMut(&T) -> U) -> PbrSourceResult3D<Vec<U>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| PbrSourceError3D::Association("bounded source adapter storage unavailable"))?;
    output.extend(input.iter().map(convert));
    Ok(output)
}

/// Producer-authored render storage. Checked against the original source before
/// registration; this is the existing 52-byte input, not a new GPU ABI.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PbrVertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv0: [f32; 2],
    pub color: [u8; 4],
    pub tangent: [f32; 4],
}
const _: [(); 52] = [(); std::mem::size_of::<PbrVertex3D>()];
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PbrOriginalIdentity3D {
    pub source: u64,
    pub revision: u64,
    pub node: u32,
    pub mesh: u32,
    pub primitive: u32,
}
impl From<PbrOriginalIdentity3D> for g::PbrOriginalIdentity3D {
    fn from(v: PbrOriginalIdentity3D) -> Self {
        Self {
            source: v.source,
            revision: v.revision,
            node: v.node,
            mesh: v.mesh,
            primitive: v.primitive,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PbrOriginalVertex3D {
    pub position: [f32; 3],
    pub normal: [f32; 3],
    pub uv0: Option<[f32; 2]>,
    pub color: Option<[u8; 4]>,
}
#[derive(Debug, Clone)]
pub enum PbrTangentProvenance3D {
    Authored,
    DefaultMikk {
        producer: u64,
        revision: u64,
        version: [u32; 3],
    },
}
#[derive(Debug, Clone)]
pub struct PbrOriginalGeometry3D {
    pub identity: PbrOriginalIdentity3D,
    pub vertices: Arc<[PbrOriginalVertex3D]>,
    pub indices: GeometryIndices,
    /// Exactly one original signed tangent per ORIGINAL element/corner.
    pub signed_tangents: Arc<[[f32; 4]]>,
    pub tangent_provenance: PbrTangentProvenance3D,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrSmoothSingularity3D {
    VertexInCone {
        normal_vertex: bool,
        vertex: u8,
        basis: [u8; 3],
        count: u8,
        antiparallel: bool,
    },
    EdgePlanes {
        normals: [u8; 2],
        tangents: [u8; 2],
        reverse_ray: bool,
        antiparallel: bool,
    },
    /// Indices0..2 select original NORMAL;3..5 select original tangent corners.
    /// Strictly positive exact alternating cofactors prove the mixed dependence.
    MixedCone {
        generators: [u8; 4],
        reverse_coefficients: bool,
        antiparallel: bool,
    },
}
impl From<PbrSmoothSingularity3D> for g::PbrSmoothSingularity3D {
    fn from(v: PbrSmoothSingularity3D) -> Self {
        match v {
            PbrSmoothSingularity3D::VertexInCone {
                normal_vertex,
                vertex,
                basis,
                count,
                antiparallel,
            } => Self::VertexInCone {
                normal_vertex,
                vertex,
                basis,
                count,
                antiparallel,
            },
            PbrSmoothSingularity3D::EdgePlanes {
                normals,
                tangents,
                reverse_ray,
                antiparallel,
            } => Self::EdgePlanes {
                normals,
                tangents,
                reverse_ray,
                antiparallel,
            },
            PbrSmoothSingularity3D::MixedCone {
                generators,
                reverse_coefficients,
                antiparallel,
            } => Self::MixedCone {
                generators,
                reverse_coefficients,
                antiparallel,
            },
        }
    }
}
#[derive(Debug, Clone)]
pub enum PbrFrameCertificate3D {
    OldCone,
    /// Row-major lower[3*i+j] encloses b dot(N_i cross T_j) for the actual
    /// packed render directions; all nine must remain at least1/16. The owner
    /// independently checks original normalization, packing and correspondence.
    AuthoredSmooth {
        separator: [f64; 3],
        lower: [f64; 9],
    },
    SourceGeometric {
        singularity: PbrSmoothSingularity3D,
        separator: [f64; 3],
        lower: [f64; 9],
    },
    NormalOnly {
        separator: [f64; 3],
        lower: [f64; 3],
    },
}
#[derive(Debug, Clone)]
pub struct PbrTriangleCertificate3D {
    pub original: PbrOriginalIdentity3D,
    pub triangle: usize,
    pub certificate_identity: u64,
    pub certificate_revision: u64,
    pub frame: PbrFrameCertificate3D,
}
#[derive(Debug, Clone)]
pub struct PbrGeometryDescriptor3D {
    pub original: PbrOriginalGeometry3D,
    pub render: Arc<[PbrVertex3D]>,
    pub render_indices: GeometryIndices,
    /// ORIGINAL vertex index for each render vertex. Ordered triangle elements
    /// and original positions/UVs must agree, including duplicated seam corners.
    pub render_vertex_sources: Arc<[u32]>,
    pub triangles: Arc<[PbrTriangleCertificate3D]>,
    pub mapped: bool,
    pub maximum_certificate_bytes: u64,
    pub maximum_verification_items: u64,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PbrGeometry3D(pub(crate) p::PbrGeometryHandle3D);
impl PbrGeometry3D {
    pub fn resource_generation(&self) -> (u32, u64) {
        self.0.0.slot_generation()
    }
}
impl Context3D {
    pub fn admit_pbr_geometry(
        &self,
        input: PbrGeometryDescriptor3D,
        previous: Option<&PbrGeometry3D>,
    ) -> PbrSourceResult3D<PbrGeometry3D> {
        p::budget(
            self,
            super::PolicyBudget3D::MeshVertices,
            input.render.len() as u64,
            u64::from(self.limits().policy.max_vertices_per_mesh),
        )?;
        let temporary = conversion_bytes(&[
            (
                input.original.vertices.len(),
                std::mem::size_of::<g::PbrOriginalVertex3D>(),
            ),
            (
                input.triangles.len(),
                std::mem::size_of::<g::PbrTriangleCertificate3D>(),
            ),
            (
                input.render.len(),
                std::mem::size_of::<g::PbrVertex52>() + std::mem::size_of::<g::PbrVertexSource3D>(),
            ),
        ])?;
        let _conversion = self.reserve_dynamic_storage(0, 0, 0, temporary)?;
        let (render, indices, source) = convert_geometry_input(input, &mut || Ok(()))?;
        let source = g::PbrGeometrySource3D::try_new_certified(render, indices, source)?;
        Ok(PbrGeometry3D(
            self.register_pbr_geometry(source, previous.map(|h| &h.0))?,
        ))
    }
    pub fn retire_pbr_geometry_source(&self, handle: &PbrGeometry3D) -> PbrSourceResult3D<()> {
        Ok(self.retire_pbr_geometry(&handle.0)?)
    }
}

// Both public paths use these exact conversions. The synchronous checkpoint is
// a statically inlined no-op; CPU cancellation cannot alter source arithmetic.
fn convert_geometry_slice<T, U, F: FnMut() -> Result<(), p::PbrError3D>>(
    input: &[T],
    mut convert: impl FnMut(&T) -> U,
    check: &mut F,
) -> PbrSourceResult3D<Vec<U>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(input.len())
        .map_err(|_| PbrSourceError3D::Association("bounded source adapter storage unavailable"))?;
    for value in input {
        check()?;
        output.push(convert(value));
    }
    Ok(output)
}
pub(super) fn convert_geometry_input<F: FnMut() -> Result<(), p::PbrError3D>>(
    input: PbrGeometryDescriptor3D,
    check: &mut F,
) -> PbrSourceResult3D<(Vec<g::PbrVertex52>, GeometryIndices, g::PbrCertifiedInput3D)> {
    check()?;
    let original = input.original;
    let source = g::PbrCertifiedInput3D {
        original: g::PbrOriginalGeometry3D {
            identity: original.identity.into(),
            vertices: convert_geometry_slice(
                &original.vertices,
                |v| g::PbrOriginalVertex3D {
                    position: v.position,
                    normal: v.normal,
                    uv0: v.uv0,
                    color: v.color,
                },
                check,
            )?
            .into(),
            indices: original.indices,
            signed_tangents: original.signed_tangents,
            tangent_origin: match original.tangent_provenance {
                PbrTangentProvenance3D::Authored => g::PbrTangentOrigin3D::Authored,
                PbrTangentProvenance3D::DefaultMikk {
                    producer,
                    revision,
                    version,
                } => g::PbrTangentOrigin3D::DefaultMikk {
                    producer,
                    revision,
                    version,
                },
            },
        },
        render_vertex_sources: input.render_vertex_sources,
        triangles: convert_geometry_slice(
            &input.triangles,
            |t| g::PbrTriangleCertificate3D {
                original: t.original.into(),
                triangle: t.triangle,
                certificate_identity: t.certificate_identity,
                certificate_revision: t.certificate_revision,
                frame: match &t.frame {
                    PbrFrameCertificate3D::OldCone => g::PbrFrameCertificate3D::OldCone,
                    PbrFrameCertificate3D::AuthoredSmooth { separator, lower } => {
                        g::PbrFrameCertificate3D::AuthoredSmooth {
                            separator: *separator,
                            lower: *lower,
                        }
                    }
                    PbrFrameCertificate3D::SourceGeometric {
                        singularity,
                        separator,
                        lower,
                    } => g::PbrFrameCertificate3D::SourceGeometric {
                        singularity: (*singularity).into(),
                        separator: *separator,
                        lower: *lower,
                    },
                    PbrFrameCertificate3D::NormalOnly { separator, lower } => {
                        g::PbrFrameCertificate3D::NormalOnly {
                            separator: *separator,
                            lower: *lower,
                        }
                    }
                },
            },
            check,
        )?
        .into(),
        mapped: input.mapped,
        maximum_certificate_bytes: input.maximum_certificate_bytes,
        maximum_verification_items: input.maximum_verification_items,
    };
    let render = convert_geometry_slice(
        &input.render,
        |v| g::PbrVertex52 {
            position: v.position,
            normal: v.normal,
            uv0: v.uv0,
            color: v.color,
            tangent: v.tangent,
        },
        check,
    )?;
    Ok((render, input.render_indices, source))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrSamplingFamily3D {
    V1,
    V2,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrMipMin3D {
    Nearest,
    Linear,
    NearestMipNearest,
    LinearMipNearest,
    NearestMipLinear,
    LinearMipLinear,
}
impl From<PbrMipMin3D> for m::MipMin3D {
    fn from(v: PbrMipMin3D) -> Self {
        match v {
            PbrMipMin3D::Nearest => Self::Nearest,
            PbrMipMin3D::Linear => Self::Linear,
            PbrMipMin3D::NearestMipNearest => Self::NearestMipNearest,
            PbrMipMin3D::LinearMipNearest => Self::LinearMipNearest,
            PbrMipMin3D::NearestMipLinear => Self::NearestMipLinear,
            PbrMipMin3D::LinearMipLinear => Self::LinearMipLinear,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrMipRole3D {
    OpaqueBase,
    MaskBase,
    Normal,
    MetallicRoughness,
    Occlusion,
    Emissive,
    NumericMrAo,
}
impl From<PbrMipRole3D> for m::MipRole3D {
    fn from(v: PbrMipRole3D) -> Self {
        match v {
            PbrMipRole3D::OpaqueBase => Self::OpaqueBase,
            PbrMipRole3D::MaskBase => Self::MaskBase,
            PbrMipRole3D::Normal => Self::Normal,
            PbrMipRole3D::MetallicRoughness => Self::MetallicRoughness,
            PbrMipRole3D::Occlusion => Self::Occlusion,
            PbrMipRole3D::Emissive => Self::Emissive,
            PbrMipRole3D::NumericMrAo => Self::NumericMrAo,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrImageOrigin3D {
    TopLeft,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrMipGenerator3D {
    RecursiveStoredAreaR028V1,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PbrMipLevel3D {
    pub width: u32,
    pub height: u32,
    pub offset: u64,
    pub bytes: u64,
}
#[derive(Debug, Clone)]
pub struct PbrMipProvenance3D {
    pub image_identity: u64,
    pub content_revision: u64,
    pub producer_identity: u64,
    pub producer_revision: u64,
    pub generator_identity: u64,
    pub generator_revision: u64,
    pub generator: PbrMipGenerator3D,
    pub origin: PbrImageOrigin3D,
    pub uv_set: u32,
    pub work: u64,
    pub error_bound: f64,
    pub original: Arc<[u8]>,
    /// The actual finished producer buffer; pointer identity must match texels.
    pub derived: std::sync::Weak<[u8]>,
}
#[derive(Debug, Clone)]
pub struct PbrMipChainDescriptor3D {
    pub role: PbrMipRole3D,
    pub format: TextureFormat3D,
    pub levels: Arc<[PbrMipLevel3D]>,
    pub texels: Arc<[u8]>,
    pub provenance: PbrMipProvenance3D,
}
impl PbrMipChainDescriptor3D {
    fn private_reusing(
        &self,
        prior: Option<&m::MipChainSource3D>,
    ) -> PbrSourceResult3D<m::MipChainSource3D> {
        self.private_reusing_cpu(prior, &m::NormalCpuControl3D::synchronous())
            .map_err(|e| e.synchronous().into())
    }
    pub(super) fn private_reusing_cpu(
        &self,
        prior: Option<&m::MipChainSource3D>,
        control: &m::NormalCpuControl3D<'_>,
    ) -> Result<m::MipChainSource3D, m::NormalCpuFailure3D> {
        control.checkpoint()?;
        if let Some(old) = prior {
            let a = &self.provenance;
            let b = &old.provenance;
            let mut same_levels = old.role == self.role.into()
                && old.format == self.format
                && Arc::ptr_eq(&old.texels, &self.texels)
                && Arc::ptr_eq(&b.original, &a.original)
                && std::sync::Weak::ptr_eq(&b.derived, &a.derived)
                && old.levels.len() == self.levels.len();
            if same_levels {
                for (x, y) in old.levels.iter().zip(self.levels.iter()) {
                    control.checkpoint()?;
                    if (x.width, x.height, x.offset, x.bytes)
                        != (y.width, y.height, y.offset, y.bytes)
                    {
                        same_levels = false;
                        break;
                    }
                }
            }
            if same_levels
                && (
                    a.image_identity,
                    a.content_revision,
                    a.producer_identity,
                    a.producer_revision,
                    a.generator_identity,
                    a.generator_revision,
                    a.uv_set,
                    a.work,
                    a.error_bound.to_bits(),
                ) == (
                    b.image_identity,
                    b.content_revision,
                    b.producer_identity,
                    b.producer_revision,
                    b.generator_identity,
                    b.generator_revision,
                    b.uv_set,
                    b.work,
                    b.error_bound.to_bits(),
                )
                && b.generator == Some(m::ChainGenerator3D::RecursiveStoredAreaR028V1)
                && b.origin == Some(m::ImageOrigin3D::TopLeft)
            {
                // An unchanged ORIGINAL capture may get a new certificate,
                // sampler or scale. Preserve that referent only after exact
                // provenance/buffer/level equality; never relabel changed bytes.
                return Ok(old.clone());
            }
        }
        self.private_cpu(control)
    }
    fn private(&self) -> PbrSourceResult3D<m::MipChainSource3D> {
        self.private_cpu(&m::NormalCpuControl3D::synchronous())
            .map_err(|e| e.synchronous().into())
    }
    fn private_cpu(
        &self,
        control: &m::NormalCpuControl3D<'_>,
    ) -> Result<m::MipChainSource3D, m::NormalCpuFailure3D> {
        let mut levels = Vec::new();
        levels.try_reserve_exact(self.levels.len()).map_err(|_| {
            p::PbrError3D::Association("bounded source adapter storage unavailable")
        })?;
        for l in self.levels.iter() {
            control.checkpoint()?;
            levels.push(m::MipLevel3D {
                width: l.width,
                height: l.height,
                offset: l.offset,
                bytes: l.bytes,
            });
        }
        let p = &self.provenance;
        Ok(m::MipChainSource3D {
            role: self.role.into(),
            format: self.format,
            texels: self.texels.clone(),
            levels: levels.into(),
            provenance: Arc::new(m::ChainProvenance3D {
                image_identity: p.image_identity,
                content_revision: p.content_revision,
                producer_identity: p.producer_identity,
                producer_revision: p.producer_revision,
                generator_identity: p.generator_identity,
                generator_revision: p.generator_revision,
                generator: Some(match p.generator {
                    PbrMipGenerator3D::RecursiveStoredAreaR028V1 => {
                        m::ChainGenerator3D::RecursiveStoredAreaR028V1
                    }
                }),
                origin: Some(match p.origin {
                    PbrImageOrigin3D::TopLeft => m::ImageOrigin3D::TopLeft,
                }),
                uv_set: p.uv_set,
                work: p.work,
                error_bound: p.error_bound,
                original: p.original.clone(),
                derived: p.derived.clone(),
            }),
        })
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PbrMipSamplerDescriptor3D {
    pub identity: u64,
    pub revision: u64,
    pub image_identity: u64,
    pub image_revision: u64,
    pub u: SamplerAddressMode,
    pub v: SamplerAddressMode,
    pub mag: SamplerFilterMode,
    pub min: PbrMipMin3D,
    pub extent: [u32; 2],
}
impl From<PbrMipSamplerDescriptor3D> for m::MipSamplerSource3D {
    fn from(v: PbrMipSamplerDescriptor3D) -> Self {
        Self {
            identity: v.identity,
            revision: v.revision,
            image_identity: v.image_identity,
            image_revision: v.image_revision,
            u: v.u,
            v: v.v,
            mag: v.mag,
            min: v.min.into(),
            extent: v.extent,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PbrMipChain3D(pub(super) m::MipChainHandle3D);
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PbrMipSampler3D(pub(super) m::MipSamplerHandle3D);
impl PbrMipChain3D {
    pub fn resource_generation(&self) -> (u32, u64) {
        self.0.0.slot_generation()
    }
}
impl PbrMipSampler3D {
    pub fn resource_generation(&self) -> (u32, u64) {
        self.0.0.slot_generation()
    }
}
/// Canonical common-UV domain key, ordered lexicographically by its declared
/// fields. A branch is MAG at level0, no-mip MIN at level0, a nearest-mip MIN
/// level, or each adjacent pair for linear mip blending. `linear` is the
/// within-level filter, independent of interlevel blending.
///
/// For each axis, sort/deduplicate 0,1 and both levels' texel-center fractions
/// (2*j+1)/(2*extent) for linear filtering, or internal j/extent for nearest.
/// Linear `u`/`v` index consecutive grid intervals. Nearest indices alternate
/// grid points at even indices and open intervals at odd indices. At a nearest
/// tie, independently enumerate addressed lower/upper-level texel choices in
/// ascending texel-index order; `u_choice`/`v_choice` select those combinations.
/// Equal levels use the same choice. Clamp, Repeat and MirrorRepeat use the
/// captured sampler policy, including seam/tie alternatives. No source texel
/// from a different UV location may be assigned independent blending weights.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PbrNormalCell3D {
    pub mag: bool,
    pub linear: bool,
    pub lower: u32,
    pub upper: u32,
    pub u: u32,
    pub v: u32,
    pub u_choice: u8,
    pub v_choice: u8,
}
#[derive(Debug, Clone, Copy)]
pub struct PbrNormalWitness3D {
    /// Finite nonzero separator; its mathematical unit interpretation is
    /// independently outward-enclosed by the existing source checker.
    pub direction: [f64; 3],
    /// Claimed projection bound on every real response corner, strictly above
    /// binary64 1/510 by at least its next representable value.
    pub lower: f64,
}
#[derive(Debug, Clone, Copy)]
pub struct PbrNormalBox3D {
    /// At most20 octree levels. Each successive3-bit octant uses bit0 for u,
    /// bit1 for v, bit2 for fractional LOD, with the root octant most significant.
    /// The depth0 whole-domain box has path0. Coordinates are dyadic subdivisions
    /// of this SAME cell, not independent per-mip UV coordinates.
    pub path: u64,
    pub depth: u8,
    pub witness: PbrNormalWitness3D,
}
#[derive(Debug, Clone)]
pub struct PbrNormalException3D {
    pub cell: PbrNormalCell3D,
    /// Sorted, prefix-free leaves covering the entire cell. At depth d, each
    /// leaf occupies [path << (60-3*d), (path+1) << (60-3*d)) in Morton order;
    /// these intervals must partition [0,2^60) exactly without gaps/overlap.
    pub leaves: Arc<[PbrNormalBox3D]>,
}
/// Complete SOURCE proof; no Boolean badge, callback or generated witness.
/// Unlisted cells use `default`; exceptions are strictly increasing by cell
/// key. `cells` is the complete reachable count, including nearest ties and MAG,
/// not the exception count. The existing verifier enumerates the entire domain,
/// checks original stored-parent first moments and all real response corners,
/// and refuses exhausted work/metadata bounds. The captured source Arc, sampler
/// generation, policy and exact material scale bits must all agree.
#[derive(Debug, Clone)]
pub struct PbrNormalCertificate3D {
    pub identity: u64,
    pub revision: u64,
    pub source: Arc<PbrMipChainDescriptor3D>,
    pub sampler: PbrMipSampler3D,
    pub policy: PbrMipSamplerDescriptor3D,
    pub normal_scale_bits: u32,
    pub cells: u64,
    pub maximum_work: u64,
    pub maximum_metadata_bytes: u64,
    pub default: PbrNormalWitness3D,
    pub exceptions: Arc<[PbrNormalException3D]>,
}
impl From<PbrNormalWitness3D> for m::NormalWitness3D {
    fn from(v: PbrNormalWitness3D) -> Self {
        Self {
            direction: v.direction,
            lower: v.lower,
        }
    }
}
impl Context3D {
    pub fn admit_pbr_mip_sampler(
        &self,
        input: PbrMipSamplerDescriptor3D,
        previous: Option<&PbrMipSampler3D>,
    ) -> PbrSourceResult3D<PbrMipSampler3D> {
        Ok(PbrMipSampler3D(self.register_mip_sampler(
            input.into(),
            previous.map(|h| &h.0),
        )?))
    }
    pub fn admit_pbr_mip_chain(
        &self,
        input: Arc<PbrMipChainDescriptor3D>,
        previous: Option<&PbrMipChain3D>,
    ) -> PbrSourceResult3D<PbrMipChain3D> {
        let temporary =
            conversion_bytes(&[(input.levels.len(), std::mem::size_of::<m::MipLevel3D>())])?;
        let _conversion = self.reserve_dynamic_storage(0, 0, 0, temporary)?;
        Ok(PbrMipChain3D(self.register_mip_chain(
            input.private()?,
            previous.map(|h| &h.0),
        )?))
    }
    pub fn admit_pbr_certified_normal_chain(
        &self,
        input: Arc<PbrMipChainDescriptor3D>,
        sampler: &PbrMipSampler3D,
        normal_scale: f32,
        proof: PbrNormalCertificate3D,
        previous: Option<&PbrMipChain3D>,
    ) -> PbrSourceResult3D<PbrMipChain3D> {
        if !Arc::ptr_eq(&input, &proof.source) {
            return Err(PbrSourceError3D::Association(
                "normal proof does not capture the supplied source referent",
            ));
        }
        let mut temporary = conversion_bytes(&[
            (input.levels.len(), std::mem::size_of::<m::MipLevel3D>()),
            (
                proof.exceptions.len(),
                std::mem::size_of::<m::NormalException3D>(),
            ),
        ])?;
        for e in proof.exceptions.iter() {
            temporary = temporary
                .checked_add(conversion_bytes(&[(
                    e.leaves.len(),
                    std::mem::size_of::<m::NormalBox3D>(),
                )])?)
                .ok_or(PbrSourceError3D::Association(
                    "normal adapter storage overflow",
                ))?;
        }
        let _conversion = self.reserve_dynamic_storage(0, 0, 0, temporary)?;
        let prior = previous.map(|h| self.lease_mip_chain(&h.0)).transpose()?;
        let source = input.private_reusing(prior.as_ref().map(|p| p.source.as_ref()))?;
        let mut exceptions = Vec::new();
        exceptions
            .try_reserve_exact(proof.exceptions.len())
            .map_err(|_| {
                PbrSourceError3D::Association("bounded normal adapter storage unavailable")
            })?;
        for e in proof.exceptions.iter() {
            exceptions.push(m::NormalException3D {
                cell: m::NormalCell3D {
                    mag: e.cell.mag,
                    linear: e.cell.linear,
                    lower: e.cell.lower,
                    upper: e.cell.upper,
                    u: e.cell.u,
                    v: e.cell.v,
                    u_choice: e.cell.u_choice,
                    v_choice: e.cell.v_choice,
                },
                leaves: convert_slice(&e.leaves, |l| m::NormalBox3D {
                    path: l.path,
                    depth: l.depth,
                    witness: l.witness.into(),
                })?
                .into(),
            });
        }
        let mut certificate = m::NormalDomainCertificate3D {
            identity: proof.identity,
            revision: proof.revision,
            source: Arc::new(source.clone()),
            sampler: proof.sampler.0,
            policy: proof.policy.into(),
            normal_scale_bits: proof.normal_scale_bits,
            cells: proof.cells,
            maximum_work: proof.maximum_work,
            maximum_metadata_bytes: proof.maximum_metadata_bytes,
            default: proof.default.into(),
            exceptions: exceptions.into(),
            digest: [0; 32],
        };
        // This is the private owner's canonical body checksum, not a proof or
        // preintegrator. Every supplied corner inequality is independently checked.
        certificate.digest = certificate.computed_digest();
        Ok(PbrMipChain3D(self.register_certified_normal_chain(
            source,
            &sampler.0,
            normal_scale,
            Arc::new(certificate),
            previous.map(|h| &h.0),
        )?))
    }
    pub fn retire_pbr_mip_chain(&self, h: &PbrMipChain3D) -> PbrSourceResult3D<()> {
        Ok(self.retire_mip_chain(&h.0)?)
    }
    pub fn retire_pbr_mip_sampler(&self, h: &PbrMipSampler3D) -> PbrSourceResult3D<()> {
        Ok(self.retire_mip_sampler(&h.0)?)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrAlphaMode3D {
    Opaque,
    Mask,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PbrRoleBinding3D {
    V1 {
        texture: TextureHandle,
        sampler: SamplerHandle,
    },
    V2 {
        texture: PbrMipChain3D,
        sampler: PbrMipSampler3D,
    },
}
#[derive(Debug, Clone, Default)]
pub struct PbrMaterialRoles3D {
    pub base: Option<PbrRoleBinding3D>,
    pub normal: Option<PbrRoleBinding3D>,
    pub metallic_roughness: Option<PbrRoleBinding3D>,
    pub occlusion: Option<PbrRoleBinding3D>,
    pub emissive: Option<PbrRoleBinding3D>,
}
#[derive(Debug, Clone, Copy, Default)]
pub struct PbrRequiredRoles3D {
    pub base: bool,
    pub normal: bool,
    pub metallic_roughness: bool,
    pub occlusion: bool,
    pub emissive: bool,
}
#[derive(Debug, Clone)]
pub struct PbrMaterialDescriptor3D {
    pub identity: u64,
    pub revision: u64,
    pub base_rgba: [f32; 4],
    pub emissive_rgb: [f32; 3],
    pub normal_scale: f32,
    pub metallic: f32,
    pub roughness: f32,
    pub occlusion_strength: f32,
    pub cutoff: f32,
    pub alpha_mode: PbrAlphaMode3D,
    pub double_sided: bool,
    pub roles: PbrMaterialRoles3D,
    pub required: PbrRequiredRoles3D,
    pub sampling: PbrSamplingFamily3D,
}
impl PbrMaterialDescriptor3D {
    /// R-024's exact genuinely absent-role factors; no light/source is invented.
    pub fn new(identity: u64, revision: u64, sampling: PbrSamplingFamily3D) -> Self {
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
            roles: Default::default(),
            required: Default::default(),
            sampling,
        }
    }
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PbrMaterial3D(p::PbrMaterialHandle3D);
impl PbrMaterial3D {
    pub fn resource_generation(&self) -> (u32, u64) {
        self.0.0.slot_generation()
    }
}
impl Context3D {
    pub fn admit_pbr_material(
        &self,
        input: PbrMaterialDescriptor3D,
        previous: Option<&PbrMaterial3D>,
    ) -> PbrSourceResult3D<PbrMaterial3D> {
        let roles = input.roles;
        let req = input.required;
        let source = p::PbrMaterialSource3D {
            identity: input.identity,
            revision: input.revision,
            base_rgba: input.base_rgba,
            emissive_rgb: input.emissive_rgb,
            normal_scale: input.normal_scale,
            metallic: input.metallic,
            roughness: input.roughness,
            occlusion_strength: input.occlusion_strength,
            cutoff: input.cutoff,
            double_sided: input.double_sided,
            alpha_mode: match input.alpha_mode {
                PbrAlphaMode3D::Opaque => p::PbrAlphaMode3D::Opaque,
                PbrAlphaMode3D::Mask => p::PbrAlphaMode3D::Mask,
            },
            sampling: match input.sampling {
                PbrSamplingFamily3D::V1 => m::SamplingFamily3D::V1,
                PbrSamplingFamily3D::V2 => m::SamplingFamily3D::V2,
            },
            required: [
                req.base,
                req.normal,
                req.metallic_roughness,
                req.occlusion,
                req.emissive,
            ],
            roles: [
                roles.base,
                roles.normal,
                roles.metallic_roughness,
                roles.occlusion,
                roles.emissive,
            ]
            .map(|r| {
                r.map(|r| match r {
                    PbrRoleBinding3D::V1 { texture, sampler } => {
                        p::PbrRoleBinding3D::V1 { texture, sampler }
                    }
                    PbrRoleBinding3D::V2 { texture, sampler } => p::PbrRoleBinding3D::V2 {
                        texture: texture.0,
                        sampler: sampler.0,
                    },
                })
            }),
        };
        Ok(PbrMaterial3D(
            self.register_pbr_material(source, previous.map(|h| &h.0))?,
        ))
    }
    pub fn retire_pbr_material_source(&self, h: &PbrMaterial3D) -> PbrSourceResult3D<()> {
        Ok(self.retire_pbr_material(&h.0)?)
    }
}

#[derive(Debug, Clone)]
pub struct PbrDrawPacket3D {
    pub geometry: PbrGeometry3D,
    pub material: PbrMaterial3D,
    pub raster: RasterMode,
    pub stable_ordinal: u64,
}
#[derive(Clone)]
pub struct PbrSnapshot3D(pub(crate) Arc<s::MaterialSnapshot3D>);
#[derive(Clone)]
pub struct PbrModelState3D(pub(crate) Arc<s::MaterialModelState3D>);
impl std::fmt::Debug for PbrSnapshot3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PbrSnapshot3D")
            .field("revision", &self.0.revision)
            .field("packets", &self.0.packets.len())
            .finish()
    }
}
impl std::fmt::Debug for PbrModelState3D {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PbrModelState3D")
            .field("revision", &self.0.revision)
            .field("snapshot", &self.0.snapshot.revision)
            .finish()
    }
}
impl PbrSnapshot3D {
    pub fn revision(&self) -> StructuralRevision {
        self.0.revision
    }
    pub fn packet_count(&self) -> usize {
        self.0.packets.len()
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl PbrModelState3D {
    pub fn revision(&self) -> ModelRevision {
        self.0.revision
    }
    pub fn matches_snapshot(&self, snapshot: &PbrSnapshot3D) -> bool {
        Arc::ptr_eq(&self.0.snapshot, &snapshot.0)
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Context3D {
    pub fn admit_pbr_snapshot(
        &self,
        revision: StructuralRevision,
        packets: Vec<PbrDrawPacket3D>,
    ) -> PbrSourceResult3D<PbrSnapshot3D> {
        p::budget(
            self,
            super::PolicyBudget3D::DrawPacketsPerSnapshot,
            packets.len() as u64,
            u64::from(self.limits().policy.max_draw_packets_per_snapshot),
        )?;
        let _conversion = self.reserve_dynamic_storage(
            0,
            0,
            0,
            conversion_bytes(&[(
                packets.len(),
                std::mem::size_of::<s::MaterialPacketSource3D>(),
            )])?,
        )?;
        let packets = convert_slice(&packets, |p| s::MaterialPacketSource3D::PbrMaterial {
            geometry: p.geometry.0.clone(),
            material: p.material.0.clone(),
            raster: p.raster,
            stable_ordinal: p.stable_ordinal,
        })?;
        Ok(PbrSnapshot3D(
            self.admit_material_snapshot(revision, packets)?,
        ))
    }
    pub fn admit_pbr_model(
        &self,
        snapshot: &PbrSnapshot3D,
        revision: ModelRevision,
        placements: Vec<ModelPlacement3>,
    ) -> PbrSourceResult3D<PbrModelState3D> {
        Ok(PbrModelState3D(self.admit_material_model(
            snapshot.0.clone(),
            revision,
            placements,
        )?))
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PbrDirectionalLight3D {
    pub surface_to_source: [f64; 3],
    pub irradiance: [f32; 3],
    pub shadow_far_m: f64,
}
#[derive(Debug, Clone, Copy)]
pub struct PbrPointLight3D {
    pub position: WorldPoint3,
    pub intensity: [f32; 3],
    pub radius_m: f64,
    pub range_m: f64,
}
#[derive(Debug, Clone, Copy)]
pub struct PbrSpotLight3D {
    pub point: PbrPointLight3D,
    pub emitter_to_scene: [f64; 3],
    pub inner_radians: f64,
    pub outer_radians: f64,
}
impl From<PbrPointLight3D> for d::PointLight3D {
    fn from(v: PbrPointLight3D) -> Self {
        Self {
            position: v.position,
            intensity: v.intensity,
            radius_m: v.radius_m,
            range_m: v.range_m,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PbrDirectDescriptor3D {
    pub identity: u64,
    pub revision: u64,
    pub meters_per_scene_unit: f64,
    pub directional: Option<PbrDirectionalLight3D>,
    pub points: Arc<[PbrPointLight3D]>,
    pub spots: Arc<[PbrSpotLight3D]>,
}
/// Immutable captured direct input. Complete scene admission performs the R-025
/// validation against the exact captured geometry/model and complete HDR bound.
#[derive(Clone)]
pub struct PbrDirectSource3D(Arc<d::DirectSource3D>);
impl PbrDirectSource3D {
    pub fn capture(
        input: PbrDirectDescriptor3D,
        snapshot: &PbrSnapshot3D,
        model: &PbrModelState3D,
    ) -> PbrSourceResult3D<Self> {
        if input.points.len() > 4 || input.spots.len() > 4 {
            return Err(PbrSourceError3D::Direct {
                field: "source slots",
                cause: "more than four point/spot sources",
            });
        }
        Ok(Self(Arc::new(d::DirectSource3D {
            identity: input.identity,
            revision: input.revision,
            meters_per_scene_unit: input.meters_per_scene_unit,
            directional: input.directional.map(|v| d::Directional3D {
                surface_to_source: v.surface_to_source,
                irradiance: v.irradiance,
                shadow_far_m: v.shadow_far_m,
            }),
            points: convert_slice(&input.points, |v| (*v).into())?.into(),
            spots: convert_slice(&input.spots, |v| d::SpotLight3D {
                point: v.point.into(),
                emitter_to_scene: v.emitter_to_scene,
                inner_radians: v.inner_radians,
                outer_radians: v.outer_radians,
            })?
            .into(),
            snapshot: snapshot.0.clone(),
            model: model.0.clone(),
        })))
    }
    pub fn identity(&self) -> (u64, u64) {
        (self.0.identity, self.0.revision)
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct PbrHdrSource3D {
    pub identity: u64,
    pub revision: u64,
    pub exposure_ev: f32,
    /// Producer's complete material+direct+emission+E/G nonnegative RGB envelope.
    pub maximum_rgb: [f32; 3],
}
#[derive(Clone)]
pub struct PbrSceneDescriptor3D {
    pub snapshot: PbrSnapshot3D,
    pub model: PbrModelState3D,
    pub hdr: PbrHdrSource3D,
    pub samples: super::SampleCount3D,
    pub direct: Option<PbrDirectSource3D>,
    pub indirect: Option<PbrIndirectSource3D>,
}
#[derive(Debug, Clone)]
pub struct PbrScene3D(pub(crate) Arc<p::MaterialSceneSource3D>);
impl PbrScene3D {
    pub fn hdr(&self) -> PbrHdrSource3D {
        PbrHdrSource3D {
            identity: self.0.hdr.identity,
            revision: self.0.hdr.revision,
            exposure_ev: self.0.hdr.exposure_ev,
            maximum_rgb: self.0.hdr.maximum_rgb,
        }
    }
    pub fn direct(&self) -> Option<PbrDirectSource3D> {
        self.0.direct.as_ref().map(|s| PbrDirectSource3D(s.clone()))
    }
    pub fn indirect(&self) -> Option<PbrIndirectSource3D> {
        self.0
            .indirect
            .as_ref()
            .map(|s| PbrIndirectSource3D(s.clone()))
    }
    pub fn identity(&self) -> (u64, u64) {
        (self.0.hdr.identity, self.0.hdr.revision)
    }
    pub fn snapshot(&self) -> PbrSnapshot3D {
        PbrSnapshot3D(self.0.snapshot.clone())
    }
    pub fn model(&self) -> PbrModelState3D {
        PbrModelState3D(self.0.model.clone())
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
    pub fn presence(&self) -> (bool, bool, bool) {
        (
            self.0.direct.is_some(),
            self.0
                .indirect
                .as_ref()
                .is_some_and(|i| i.environment.is_some()),
            self.0.indirect.as_ref().is_some_and(|i| i.gi.is_some()),
        )
    }
    pub fn samples(&self) -> u32 {
        self.0.samples
    }
}
impl Context3D {
    pub fn admit_pbr_scene(&self, input: PbrSceneDescriptor3D) -> PbrSourceResult3D<PbrScene3D> {
        let source = p::MaterialSceneSource3D {
            snapshot: input.snapshot.0,
            model: input.model.0,
            hdr: hdr::HdrSource3D {
                identity: input.hdr.identity,
                revision: input.hdr.revision,
                exposure_ev: input.hdr.exposure_ev,
                maximum_rgb: input.hdr.maximum_rgb,
            },
            samples: input.samples.value(),
            direct: input.direct.map(|v| v.0),
            indirect: input.indirect.map(|v| v.0),
            #[cfg(test)]
            observe: false,
        };
        source.validate(self)?;
        Ok(PbrScene3D(Arc::new(source)))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrOriginalHandedness3D {
    Right,
    Left,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrSourceReflectionAxis3D {
    X,
    Y,
    Z,
}
#[derive(Debug, Clone)]
pub struct PbrProducerEvidence3D {
    pub original_identity: u64,
    pub original_revision: u64,
    pub producer_identity: u64,
    pub producer_revision: u64,
    pub evidence_identity: u64,
    pub world_identity: u64,
    pub original_handedness: Option<PbrOriginalHandedness3D>,
    pub handedness_correction: Option<PbrSourceReflectionAxis3D>,
    pub original_to_world: Option<Orientation3>,
    pub integration_work: u64,
    pub absolute_error_bound: f64,
}
impl PbrProducerEvidence3D {
    fn private(self, derivation: i::SourceDerivation3D) -> Arc<i::ProducerEvidence3D> {
        Arc::new(i::ProducerEvidence3D {
            original_identity: self.original_identity,
            original_revision: self.original_revision,
            producer_identity: self.producer_identity,
            producer_revision: self.producer_revision,
            evidence_identity: self.evidence_identity,
            world_identity: self.world_identity,
            original_handedness: self.original_handedness.map(|h| match h {
                PbrOriginalHandedness3D::Right => i::OriginalHandedness3D::Right,
                PbrOriginalHandedness3D::Left => i::OriginalHandedness3D::Left,
            }),
            handedness_correction: self.handedness_correction.map(|h| match h {
                PbrSourceReflectionAxis3D::X => i::SourceReflectionAxis3D::X,
                PbrSourceReflectionAxis3D::Y => i::SourceReflectionAxis3D::Y,
                PbrSourceReflectionAxis3D::Z => i::SourceReflectionAxis3D::Z,
            }),
            original_to_world: self.original_to_world,
            integration_work: self.integration_work,
            absolute_error_bound: self.absolute_error_bound,
            derivation: Arc::new(derivation),
        })
    }
}
/// Unverified producer capture. Qualification occurs only at asset/scene
/// admission. All three derived buffers retain their actual common attribution.
#[derive(Clone)]
pub struct PbrEnvironmentInput3D {
    evidence: Arc<i::ProducerEvidence3D>,
    bits: [Arc<[u16]>; 3],
}
impl PbrEnvironmentInput3D {
    pub fn capture(
        evidence: PbrProducerEvidence3D,
        method: Arc<str>,
        original_field: Arc<[f64]>,
        reference_observations: Arc<[f64]>,
        derived: [Arc<[u16]>; 3],
    ) -> Self {
        let evidence = evidence.private(i::SourceDerivation3D::Environment {
            method,
            original_field,
            reference_observations,
            derived: derived.each_ref().map(Arc::downgrade),
        });
        Self {
            evidence,
            bits: derived,
        }
    }
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PbrEnvironmentRole3D {
    Diffuse,
    Specular,
    BrdfLut,
}
impl From<PbrEnvironmentRole3D> for i::EnvironmentRole3D {
    fn from(v: PbrEnvironmentRole3D) -> Self {
        match v {
            PbrEnvironmentRole3D::Diffuse => Self::Diffuse,
            PbrEnvironmentRole3D::Specular => Self::Specular,
            PbrEnvironmentRole3D::BrdfLut => Self::BrdfLut,
        }
    }
}
#[derive(Debug, Clone)]
pub struct PbrEnvironmentAsset3D(i::EnvironmentHandle3D);
#[derive(Clone)]
pub struct PbrEnvironment3D(Arc<i::EnvironmentSource3D>);
impl PbrEnvironment3D {
    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Context3D {
    pub fn admit_pbr_environment_asset(
        &self,
        input: &PbrEnvironmentInput3D,
        identity: u64,
        revision: u64,
        role: PbrEnvironmentRole3D,
        previous: Option<&PbrEnvironmentAsset3D>,
    ) -> PbrSourceResult3D<PbrEnvironmentAsset3D> {
        let index = match role {
            PbrEnvironmentRole3D::Diffuse => 0,
            PbrEnvironmentRole3D::Specular => 1,
            PbrEnvironmentRole3D::BrdfLut => 2,
        };
        Ok(PbrEnvironmentAsset3D(self.register_environment_asset(
            identity,
            revision,
            role.into(),
            input.evidence.clone(),
            input.bits[index].clone(),
            previous.map(|h| &h.0),
        )?))
    }
    pub fn admit_pbr_environment(
        &self,
        input: &PbrEnvironmentInput3D,
        assets: [Option<PbrEnvironmentAsset3D>; 3],
        sampler: Option<SamplerHandle>,
    ) -> PbrSourceResult3D<PbrEnvironment3D> {
        Ok(PbrEnvironment3D(Arc::new(
            i::EnvironmentSource3D::from_handles(
                self,
                input.evidence.clone(),
                assets.map(|v| v.map(|v| v.0)),
                sampler,
            )?,
        )))
    }
    pub fn retire_pbr_environment_asset(&self, h: &PbrEnvironmentAsset3D) -> PbrSourceResult3D<()> {
        Ok(self.retire_environment_asset(&h.0)?)
    }
}
/// Original static transport field and exactly two producer-owned applications;
/// the library never bakes, rotates, or relabels the WORLD lobes.
#[derive(Clone)]
pub struct PbrTransportInput3D(Arc<i::ProducerEvidence3D>);
impl PbrTransportInput3D {
    pub fn capture(
        evidence: PbrProducerEvidence3D,
        method: Arc<str>,
        original_transport_field: Arc<[f64]>,
        first: [[f64; 3]; 6],
        second: [[f64; 3]; 6],
    ) -> Self {
        Self(evidence.private(i::SourceDerivation3D::StaticTransport {
            method,
            original_transport_field,
            first,
            second,
        }))
    }
}
#[derive(Clone)]
pub struct PbrGiDependencies3D {
    pub snapshot: PbrSnapshot3D,
    pub model: PbrModelState3D,
    pub direct: Option<PbrDirectSource3D>,
    pub environment: Option<PbrEnvironment3D>,
    pub visibility_revision: u64,
}
#[derive(Clone)]
pub struct PbrGiDescriptor3D {
    pub identity: u64,
    pub revision: u64,
    pub packet: u64,
    pub patch: u64,
    pub evidence: PbrTransportInput3D,
    pub dependencies: PbrGiDependencies3D,
    pub transport_source: u64,
    pub first_transport: u64,
    pub second_transport: u64,
    /// WORLD +X,-X,+Y,-Y,+Z,-Z RGB, excluding L0.
    pub lobes: [[f32; 3]; 6],
}
#[derive(Debug, Clone)]
pub struct PbrGi3D(i::GiHandle3D);
#[derive(Clone)]
pub struct PbrIndirectSource3D(Arc<i::IndirectSource3D>);
#[derive(Clone)]
pub struct PbrIndirectDescriptor3D {
    pub identity: u64,
    pub revision: u64,
    pub world_identity: u64,
    pub environment: Option<PbrEnvironment3D>,
    pub gi: Option<Arc<[Option<PbrGi3D>]>>,
    pub visibility_revision: u64,
}
impl PbrIndirectSource3D {
    pub fn world_identity(&self) -> u64 {
        self.0.world_identity
    }
    pub fn visibility_revision(&self) -> u64 {
        self.0.visibility_revision
    }
    pub fn environment(&self) -> Option<PbrEnvironment3D> {
        self.0
            .environment
            .as_ref()
            .map(|s| PbrEnvironment3D(s.clone()))
    }
    pub fn identity(&self) -> (u64, u64) {
        (self.0.identity, self.0.revision)
    }
    pub fn same_identity(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Context3D {
    pub fn admit_pbr_gi(
        &self,
        input: PbrGiDescriptor3D,
        previous: Option<&PbrGi3D>,
    ) -> PbrSourceResult3D<PbrGi3D> {
        let deps = input.dependencies;
        let source = i::GiSource3D {
            identity: input.identity,
            revision: input.revision,
            packet: input.packet,
            patch: input.patch,
            evidence: input.evidence.0,
            dependencies: i::GiDependencies3D {
                snapshot: deps.snapshot.0,
                model: deps.model.0,
                direct: deps.direct.map(|v| v.0),
                environment: deps.environment.map(|v| v.0),
                visibility_revision: deps.visibility_revision,
            },
            transport_source: input.transport_source,
            first_transport: input.first_transport,
            second_transport: input.second_transport,
            lobes: input.lobes,
        };
        Ok(PbrGi3D(
            self.register_gi_source(source, previous.map(|h| &h.0))?,
        ))
    }
    /// Captures concrete leases; complete scene admission checks all independent
    /// geometry/material/model/light/E/visibility dependencies and the full bound.
    pub fn capture_pbr_indirect(
        &self,
        input: PbrIndirectDescriptor3D,
    ) -> PbrSourceResult3D<PbrIndirectSource3D> {
        let gi = input
            .gi
            .map(|items| {
                p::budget(
                    self,
                    super::PolicyBudget3D::DrawPacketsPerSnapshot,
                    items.len() as u64,
                    u64::from(self.limits().policy.max_draw_packets_per_snapshot),
                )?;
                let _conversion = self.reserve_dynamic_storage(
                    0,
                    0,
                    0,
                    conversion_bytes(&[(
                        items.len(),
                        std::mem::size_of::<Option<i::GiLease3D>>(),
                    )])?,
                )?;
                let mut leases = Vec::new();
                leases.try_reserve_exact(items.len()).map_err(|_| {
                    PbrSourceError3D::Association("bounded indirect adapter storage unavailable")
                })?;
                for item in items.iter() {
                    leases.push(
                        item.as_ref()
                            .map(|v| self.lease_gi_source(&v.0))
                            .transpose()?,
                    );
                }
                Ok::<Arc<[_]>, PbrSourceError3D>(leases.into())
            })
            .transpose()?;
        Ok(PbrIndirectSource3D(Arc::new(i::IndirectSource3D {
            identity: input.identity,
            revision: input.revision,
            world_identity: input.world_identity,
            environment: input.environment.map(|v| v.0),
            gi,
            visibility_revision: input.visibility_revision,
        })))
    }
    pub fn retire_pbr_gi(&self, h: &PbrGi3D) -> PbrSourceResult3D<()> {
        Ok(self.retire_gi_source(&h.0)?)
    }
}

/// Read-only truthful source-family projection of an actual retained target.
#[derive(Clone, Copy)]
pub struct PbrViewportTuple3D<'a>(pub(crate) &'a super::viewport::MaterialViewport3DTuple);
impl PbrViewportTuple3D<'_> {
    pub fn source(&self) -> PbrScene3D {
        PbrScene3D(self.0.source().clone())
    }
    pub fn request_serial(&self) -> super::Viewport3DRequestSerial {
        self.0.request_serial()
    }
    pub fn target_key(&self) -> super::Viewport3DTargetCompatibilityKey {
        self.0.target_key()
    }
    pub fn view_revision(&self) -> super::Viewport3DViewRevision {
        self.0.view_revision()
    }
    pub fn view(&self) -> super::ViewDescriptor3 {
        self.0.view()
    }
    pub fn structural_revision(&self) -> StructuralRevision {
        self.0.structural_revision()
    }
    pub fn model_revision(&self) -> ModelRevision {
        self.0.model_revision()
    }
}
impl super::Viewport3DStatus {
    pub fn pbr_requested(&self) -> Option<PbrViewportTuple3D<'_>> {
        self.requested_material().map(PbrViewportTuple3D)
    }
    pub fn pbr_committed(&self) -> Option<PbrViewportTuple3D<'_>> {
        self.committed_material().map(PbrViewportTuple3D)
    }
}
