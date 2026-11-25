use super::{
    CANONICAL_VERTEX_STRIDE, DEFAULT_LINEAR_VERTEX_COLOR, Geometry3DError, GeometryIndices,
    GeometrySourceDescriptor,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CanonicalIndexFormat {
    Unindexed,
    Uint16,
    Uint32,
}

pub(crate) struct CanonicalGeometryBytes {
    vertex_bytes: Vec<u8>,
    index_bytes: Vec<u8>,
    vertex_count: u32,
    index_count: u32,
    index_format: CanonicalIndexFormat,
    index_buffer_size: u64,
}

impl CanonicalGeometryBytes {
    pub(crate) fn pack(descriptor: &GeometrySourceDescriptor) -> Result<Self, Geometry3DError> {
        let vertex_capacity = usize::try_from(descriptor.canonical_vertex_bytes())
            .map_err(|_| Geometry3DError::SizeOverflow)?;
        let mut vertex_bytes = Vec::new();
        vertex_bytes
            .try_reserve_exact(vertex_capacity)
            .map_err(|_| Geometry3DError::SizeOverflow)?;

        let normals = descriptor.normals();
        let uvs = descriptor.uvs();
        let colors = descriptor.colors();
        for (vertex, position) in descriptor.positions().iter().copied().enumerate() {
            push_f32(&mut vertex_bytes, position.x());
            push_f32(&mut vertex_bytes, position.y());
            push_f32(&mut vertex_bytes, position.z());

            if let Some(normal) = normals
                .map(|normals| {
                    normals
                        .get(vertex)
                        .copied()
                        .ok_or(Geometry3DError::SizeOverflow)
                })
                .transpose()?
            {
                push_f32(&mut vertex_bytes, normal.x());
                push_f32(&mut vertex_bytes, normal.y());
                push_f32(&mut vertex_bytes, normal.z());
            } else {
                push_f32(&mut vertex_bytes, 0.0);
                push_f32(&mut vertex_bytes, 0.0);
                push_f32(&mut vertex_bytes, 1.0);
            }

            if let Some(uv) = uvs
                .map(|uvs| {
                    uvs.get(vertex)
                        .copied()
                        .ok_or(Geometry3DError::SizeOverflow)
                })
                .transpose()?
            {
                push_f32(&mut vertex_bytes, uv.u());
                push_f32(&mut vertex_bytes, uv.v());
            } else {
                push_f32(&mut vertex_bytes, 0.0);
                push_f32(&mut vertex_bytes, 0.0);
            }

            let color = colors
                .map(|colors| {
                    colors
                        .get(vertex)
                        .copied()
                        .ok_or(Geometry3DError::SizeOverflow)
                })
                .transpose()?
                .unwrap_or(DEFAULT_LINEAR_VERTEX_COLOR);
            vertex_bytes.extend_from_slice(&color.to_array());
        }

        let actual_vertex_bytes =
            u64::try_from(vertex_bytes.len()).map_err(|_| Geometry3DError::SizeOverflow)?;
        if actual_vertex_bytes != descriptor.canonical_vertex_bytes()
            || actual_vertex_bytes
                != descriptor
                    .vertex_count()
                    .checked_mul(CANONICAL_VERTEX_STRIDE)
                    .ok_or(Geometry3DError::SizeOverflow)?
        {
            return Err(Geometry3DError::SizeOverflow);
        }

        let index_capacity = usize::try_from(descriptor.canonical_index_bytes())
            .map_err(|_| Geometry3DError::SizeOverflow)?;
        let mut index_bytes = Vec::new();
        index_bytes
            .try_reserve_exact(index_capacity)
            .map_err(|_| Geometry3DError::SizeOverflow)?;
        let index_format = match descriptor.indices() {
            GeometryIndices::Unindexed => CanonicalIndexFormat::Unindexed,
            GeometryIndices::U16(indices) => {
                for index in indices.iter().copied() {
                    index_bytes.extend_from_slice(&index.to_le_bytes());
                }
                CanonicalIndexFormat::Uint16
            }
            GeometryIndices::U32(indices) => {
                for index in indices.iter().copied() {
                    index_bytes.extend_from_slice(&index.to_le_bytes());
                }
                CanonicalIndexFormat::Uint32
            }
        };
        let actual_index_bytes =
            u64::try_from(index_bytes.len()).map_err(|_| Geometry3DError::SizeOverflow)?;
        if actual_index_bytes != descriptor.canonical_index_bytes() {
            return Err(Geometry3DError::SizeOverflow);
        }

        let index_buffer_size = aligned_buffer_size(actual_index_bytes)?;
        Ok(Self {
            vertex_bytes,
            index_bytes,
            vertex_count: u32::try_from(descriptor.vertex_count())
                .map_err(|_| Geometry3DError::SizeOverflow)?,
            index_count: u32::try_from(descriptor.index_count())
                .map_err(|_| Geometry3DError::SizeOverflow)?,
            index_format,
            index_buffer_size,
        })
    }

    pub(crate) fn vertex_bytes(&self) -> &[u8] {
        &self.vertex_bytes
    }

    pub(crate) fn index_bytes(&self) -> &[u8] {
        &self.index_bytes
    }

    pub(crate) const fn vertex_count(&self) -> u32 {
        self.vertex_count
    }

    pub(crate) const fn index_count(&self) -> u32 {
        self.index_count
    }

    pub(crate) const fn index_format(&self) -> CanonicalIndexFormat {
        self.index_format
    }

    pub(crate) const fn index_buffer_size(&self) -> u64 {
        self.index_buffer_size
    }

    pub(crate) fn upload_bytes(&self) -> Result<u64, Geometry3DError> {
        u64::try_from(self.vertex_bytes.len())
            .ok()
            .and_then(|vertex| {
                u64::try_from(self.index_bytes.len())
                    .ok()
                    .and_then(|index| vertex.checked_add(index))
            })
            .ok_or(Geometry3DError::SizeOverflow)
    }

    pub(crate) fn gpu_resident_bytes(&self) -> Result<u64, Geometry3DError> {
        u64::try_from(self.vertex_bytes.len())
            .ok()
            .and_then(|vertex| vertex.checked_add(self.index_buffer_size))
            .ok_or(Geometry3DError::SizeOverflow)
    }
}

fn push_f32(bytes: &mut Vec<u8>, value: f32) {
    bytes.extend_from_slice(&value.to_le_bytes());
}

fn aligned_buffer_size(size: u64) -> Result<u64, Geometry3DError> {
    if size == 0 {
        return Ok(0);
    }
    let alignment = wgpu::COPY_BUFFER_ALIGNMENT;
    size.checked_add(alignment - 1)
        .map(|value| value / alignment * alignment)
        .ok_or(Geometry3DError::SizeOverflow)
}

/// Producer-supplied unit direction. The interval is formed from outward-rounded
/// IEEE operations, rather than an invented unit-length epsilon or normalization
/// of invalid supplied content. GPU packing error is retained for qualification.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PbrUnitDirection3D(pub(super) [f64; 3]);

fn round_down(v: f64) -> f64 {
    if v == 0.0 {
        return -f64::from_bits(1);
    }
    f64::from_bits(if v > 0.0 {
        v.to_bits() - 1
    } else {
        v.to_bits() + 1
    })
}
fn round_up(v: f64) -> f64 {
    if v == 0.0 {
        return f64::from_bits(1);
    }
    f64::from_bits(if v > 0.0 {
        v.to_bits() + 1
    } else {
        v.to_bits() - 1
    })
}

impl PbrUnitDirection3D {
    pub(crate) fn try_new(value: [f64; 3]) -> Result<Self, super::pbr::PbrError3D> {
        let mut lo = 0.0;
        let mut hi = 0.0;
        for v in value {
            if !v.is_finite() || v.abs() > 1.0 {
                return Err(super::pbr::PbrError3D::Source {
                    field: "unit source direction",
                    cause: "nonfinite/outside unit domain",
                });
            }
            let q = v * v;
            lo = round_down(lo + round_down(q));
            hi = round_up(hi + round_up(q));
        }
        if lo > 1.0 || hi < 1.0 {
            return Err(super::pbr::PbrError3D::Source {
                field: "unit source direction",
                cause: "unit length not qualified by outward-rounded source operations",
            });
        }
        Ok(Self(value))
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PbrVertexSource3D {
    pub(crate) position: super::LocalPoint3,
    pub(crate) normal: PbrUnitDirection3D,
    pub(crate) uv0: Option<super::TextureCoordinate2>,
    pub(crate) color: Option<super::LinearVertexColorRgba8>,
    pub(crate) tangent: Option<(PbrUnitDirection3D, f32)>,
}

#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PbrVertex52 {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv0: [f32; 2],
    pub(crate) color: [u8; 4],
    pub(crate) tangent: [f32; 4],
}
const _: [(); 52] = [(); std::mem::size_of::<PbrVertex52>()];

/// R-025's supplemental stream. All three corners of an expanded source
/// triangle carry exactly the same qualified local winding normal.
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub(crate) struct PbrFaceNormal12(pub(crate) [f32; 3]);
const _: [(); 12] = [(); std::mem::size_of::<PbrFaceNormal12>()];

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PbrGeometrySource3D {
    pub(super) vertices: std::sync::Arc<[PbrVertexSource3D]>,
    pub(super) indices: GeometryIndices,
    certified: Option<std::sync::Arc<PbrCertifiedGeometry3D>>,
}

fn pbr_dot(a: [f64; 3], b: [f64; 3]) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}
fn pbr_cross(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}
fn pbr_sub(a: [f64; 3], b: [f64; 3]) -> [f64; 3] {
    std::array::from_fn(|i| a[i] - b[i])
}
fn pbr_unit(v: [f64; 3]) -> Option<[f64; 3]> {
    let m = v.iter().copied().map(f64::abs).fold(0.0, f64::max);
    if !m.is_finite() || m == 0.0 {
        return None;
    }
    let a = v.map(|x| x / m);
    let l = pbr_dot(a, a).sqrt();
    Some(a.map(|x| x / l))
}

impl PbrGeometrySource3D {
    #[cfg(test)]
    pub(crate) fn try_new(
        vertices: Vec<PbrVertexSource3D>,
        indices: GeometryIndices,
    ) -> Result<Self, super::pbr::PbrError3D> {
        let source = Self {
            vertices: vertices.into(),
            indices,
            certified: None,
        };
        if source.vertices.is_empty()
            || source.element_count() == 0
            || source.element_count() % 3 != 0
        {
            return Err(super::pbr::PbrError3D::Source {
                field: "triangle list",
                cause: "empty/incomplete",
            });
        }
        for v in source.vertices.iter() {
            PbrUnitDirection3D::try_new(v.normal.0)?;
            if let Some((t, s)) = v.tangent {
                PbrUnitDirection3D::try_new(t.0)?;
                if s != 1.0 && s != -1.0 {
                    return Err(super::pbr::PbrError3D::Source {
                        field: "source tangent sign",
                        cause: "not exact +/-1",
                    });
                }
            }
        }
        source.validate_triangles(false)?;
        Ok(source)
    }

    pub(crate) fn element_count(&self) -> usize {
        match &self.indices {
            GeometryIndices::Unindexed => self.vertices.len(),
            GeometryIndices::U16(v) => v.len(),
            GeometryIndices::U32(v) => v.len(),
        }
    }

    pub(crate) fn triangle_positions(
        &self,
        triangle: usize,
    ) -> Result<[[f64; 3]; 3], super::pbr::PbrError3D> {
        let mut p = [[0.0; 3]; 3];
        for corner in 0..3 {
            let v = &self.vertices[self.index(3 * triangle + corner)?];
            p[corner] = [
                f64::from(v.position.x()),
                f64::from(v.position.y()),
                f64::from(v.position.z()),
            ];
        }
        Ok(p)
    }

    fn index(&self, i: usize) -> Result<usize, super::pbr::PbrError3D> {
        let index = match &self.indices {
            GeometryIndices::Unindexed => i,
            GeometryIndices::U16(v) => usize::from(v[i]),
            GeometryIndices::U32(v) => {
                usize::try_from(v[i]).map_err(|_| super::pbr::PbrError3D::Source {
                    field: "index",
                    cause: "not addressable",
                })?
            }
        };
        if index >= self.vertices.len() {
            return Err(super::pbr::PbrError3D::Source {
                field: "index",
                cause: "outside vertex source",
            });
        }
        Ok(index)
    }

    pub(crate) fn validate_roles(
        &self,
        material: &super::pbr::PbrMaterialSource3D,
    ) -> Result<(), super::pbr::PbrError3D> {
        if material.roles.iter().any(Option::is_some)
            && self.vertices.iter().any(|v| v.uv0.is_none())
        {
            return Err(super::pbr::PbrError3D::Source {
                field: "UV0",
                cause: "present role requires actual source coordinates",
            });
        }
        let mapped = material.roles[super::pbr::PbrRole3D::Normal as usize].is_some();
        if let Some(capture) = &self.certified {
            if capture.input.mapped != mapped {
                return Err(super::pbr::PbrError3D::Source {
                    field: "certified frame/normal role",
                    cause: "captured mapped/absent source does not match the actual material",
                });
            }
            return Ok(());
        }
        self.validate_triangles(mapped)
    }

    fn validate_triangles(&self, mapped: bool) -> Result<(), super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        for triangle in 0..self.element_count() / 3 {
            let i = [
                self.index(triangle * 3)?,
                self.index(triangle * 3 + 1)?,
                self.index(triangle * 3 + 2)?,
            ];
            let v = i.map(|j| &self.vertices[j]);
            let p = v.map(|v| {
                [
                    f64::from(v.position.x()),
                    f64::from(v.position.y()),
                    f64::from(v.position.z()),
                ]
            });
            let a = pbr_sub(p[1], p[0]);
            let b = pbr_sub(p[2], p[0]);
            let face = pbr_unit(pbr_cross(a, b)).ok_or(PbrError3D::Geometry {
                triangle,
                cause: "degenerate source triangle",
            })?;
            if v.iter().any(|v| pbr_dot(v.normal.0, face) < 0.75) {
                return Err(PbrError3D::Geometry {
                    triangle,
                    cause: "normal cone below0.75",
                });
            }
            if mapped {
                let uv = v.map(|v| v.uv0.map(|u| [f64::from(u.u()), f64::from(u.v())]));
                let [Some(u0), Some(u1), Some(u2)] = uv else {
                    return Err(PbrError3D::Geometry {
                        triangle,
                        cause: "normal role missing UV0",
                    });
                };
                let du1 = u1[0] - u0[0];
                let dv1 = u1[1] - u0[1];
                let du2 = u2[0] - u0[0];
                let dv2 = u2[1] - u0[1];
                let det = du1 * dv2 - du2 * dv1;
                if !det.is_finite() || det == 0.0 {
                    return Err(PbrError3D::Geometry {
                        triangle,
                        cause: "zero/nonfinite UV Jacobian",
                    });
                }
                let raw = std::array::from_fn(|k| (a[k] * dv2 - b[k] * dv1) / det);
                let projected = pbr_sub(raw, face.map(|x| x * pbr_dot(face, raw)));
                let tuv = pbr_unit(projected).ok_or(PbrError3D::Geometry {
                    triangle,
                    cause: "undefined geometric UV tangent",
                })?;
                let mut sign = None;
                for v in v {
                    let (t, s) = v.tangent.ok_or(PbrError3D::Geometry {
                        triangle,
                        cause: "normal role missing source tangent",
                    })?;
                    if pbr_dot(t.0, tuv) < 0.75 {
                        return Err(PbrError3D::Geometry {
                            triangle,
                            cause: "tangent cone below0.75",
                        });
                    }
                    if sign.is_some_and(|prior| prior != s) {
                        return Err(PbrError3D::Geometry {
                            triangle,
                            cause: "tangent sign seam not split",
                        });
                    }
                    sign = Some(s);
                }
            }
        }
        Ok(())
    }

    pub(crate) fn pack(&self) -> Result<Vec<PbrVertex52>, super::pbr::PbrError3D> {
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(self.vertices.len())
            .map_err(|_| super::pbr::PbrError3D::Association("PBR packing storage unavailable"))?;
        vertices.extend(self.vertices.iter().map(|v| {
            let uv = v.uv0.map_or([0.0; 2], |u| [u.u(), u.v()]);
            let tangent = v.tangent.map_or([1.0, 0.0, 0.0, 1.0], |(t, s)| {
                [t.0[0] as f32, t.0[1] as f32, t.0[2] as f32, s]
            });
            PbrVertex52 {
                position: [v.position.x(), v.position.y(), v.position.z()],
                normal: v.normal.0.map(|n| n as f32),
                uv0: uv,
                color: v.color.unwrap_or(DEFAULT_LINEAR_VERTEX_COLOR).to_array(),
                tangent,
            }
        }));
        Ok(vertices)
    }

    pub(crate) fn pack_direct(
        &self,
    ) -> Result<(Vec<PbrVertex52>, Vec<PbrFaceNormal12>), super::pbr::PbrError3D> {
        use super::pbr::PbrError3D;
        let canonical = self.pack()?;
        let count = self.element_count();
        let mut primary = Vec::new();
        let mut faces = Vec::new();
        primary
            .try_reserve_exact(count)
            .map_err(|_| PbrError3D::Association("D primary storage unavailable"))?;
        faces
            .try_reserve_exact(count)
            .map_err(|_| PbrError3D::Association("D face storage unavailable"))?;
        for triangle in 0..count / 3 {
            let i = [
                self.index(3 * triangle)?,
                self.index(3 * triangle + 1)?,
                self.index(3 * triangle + 2)?,
            ];
            let p = i.map(|j| canonical[j].position.map(f64::from));
            let n = pbr_unit(pbr_cross(pbr_sub(p[1], p[0]), pbr_sub(p[2], p[0]))).ok_or(
                PbrError3D::Geometry {
                    triangle,
                    cause: "undefined source winding face",
                },
            )?;
            let n = n.map(|v| v as f32);
            if !n.iter().all(|v| v.is_finite()) || n.iter().all(|v| *v == 0.0) {
                return Err(PbrError3D::Geometry {
                    triangle,
                    cause: "face normal is not usable in f32",
                });
            }
            for index in i {
                primary.push(canonical[index]);
                faces.push(PbrFaceNormal12(n));
            }
        }
        Ok((primary, faces))
    }
}

// Revision02 certificates are CPU source facts. They never enter a shader block.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct PbrOriginalIdentity3D {
    pub(crate) source: u64,
    pub(crate) revision: u64,
    pub(crate) node: u32,
    pub(crate) mesh: u32,
    pub(crate) primitive: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum PbrTangentOrigin3D {
    Authored,
    DefaultMikk {
        producer: u64,
        revision: u64,
        version: [u32; 3],
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PbrOriginalVertex3D {
    pub(crate) position: [f32; 3],
    pub(crate) normal: [f32; 3],
    pub(crate) uv0: Option<[f32; 2]>,
    pub(crate) color: Option<[u8; 4]>,
}

impl PartialEq for PbrOriginalVertex3D {
    fn eq(&self, other: &Self) -> bool {
        self.position.map(f32::to_bits) == other.position.map(f32::to_bits)
            && self.normal.map(f32::to_bits) == other.normal.map(f32::to_bits)
            && self.uv0.map(|v| v.map(f32::to_bits)) == other.uv0.map(|v| v.map(f32::to_bits))
            && self.color == other.color
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PbrOriginalGeometry3D {
    pub(crate) identity: PbrOriginalIdentity3D,
    pub(crate) vertices: std::sync::Arc<[PbrOriginalVertex3D]>,
    pub(crate) indices: GeometryIndices,
    /// Original signed tangent attribute or every finished default-Mikk corner,
    /// in ORIGINAL element order, including corners later rendered through C.
    pub(crate) signed_tangents: std::sync::Arc<[[f32; 4]]>,
    pub(crate) tangent_origin: PbrTangentOrigin3D,
}

/// A finite, source-chosen cone-intersection proof. No witness search runs in
/// admission. VertexInCone covers vertex/edge/interior intersections; EdgePlanes
/// covers a ray in the strict interior of two source edges. Coefficients are
/// derived and enclosed from the exact packed original candidate, not trusted
/// approximate barycentric numbers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PbrSmoothSingularity3D {
    /// Four source-selected rays, indices0..2 NORMAL and3..5 TANGENT. Exact
    /// signed cofactors prove a positive dependence with both source families.
    MixedCone {
        generators: [u8; 4],
        reverse_coefficients: bool,
        antiparallel: bool,
    },
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
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum PbrFrameCertificate3D {
    OldCone,
    AuthoredSmooth {
        separator: [f64; 3],
        lower: [f64; 9],
    },
    SourceGeometric {
        singularity: PbrSmoothSingularity3D,
        separator: [f64; 3],
        lower: [f64; 9],
    },
    /// Genuine normal-role absence, not a fourth mapped frame mode.
    NormalOnly {
        separator: [f64; 3],
        lower: [f64; 3],
    },
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PbrTriangleCertificate3D {
    pub(crate) original: PbrOriginalIdentity3D,
    pub(crate) triangle: usize,
    pub(crate) certificate_identity: u64,
    pub(crate) certificate_revision: u64,
    pub(crate) frame: PbrFrameCertificate3D,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PbrCertifiedInput3D {
    pub(crate) original: PbrOriginalGeometry3D,
    /// Each render vertex identifies its ORIGINAL source vertex. Element/triangle
    /// order is preserved; seam expansion may duplicate a vertex, never a face.
    pub(crate) render_vertex_sources: std::sync::Arc<[u32]>,
    pub(crate) triangles: std::sync::Arc<[PbrTriangleCertificate3D]>,
    pub(crate) mapped: bool,
    pub(crate) maximum_certificate_bytes: u64,
    /// Original vertices, original/render elements, render vertices and triangle
    /// records. Each item has fixed finite arithmetic; there is no optimizer.
    pub(crate) maximum_verification_items: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PbrCheckedFrame3D {
    pub(crate) normal_lower: f64,
    pub(crate) tangent_lower: f64,
    pub(crate) cross_lower: f64,
    pub(crate) local_g_lower: f64,
    pub(crate) maximum_direction_pack_error: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct PbrCertifiedGeometry3D {
    input: PbrCertifiedInput3D,
    frames: std::sync::Arc<[PbrCheckedFrame3D]>,
    metadata_bytes: u64,
    verification_items: u64,
}

type CertificateResult<T> = Result<T, super::pbr::PbrError3D>;

/// Constructible only after this module's original complete source verifier.
pub(super) struct VerifiedCpuGeometry3D(PbrGeometrySource3D);
/// Constructible only after the same original successor verifier as registration.
pub(super) struct AdmissibleCpuGeometry3D(PbrGeometrySource3D);

impl VerifiedCpuGeometry3D {
    pub(super) fn source(&self) -> &PbrGeometrySource3D {
        &self.0
    }
    pub(super) fn successor<F: FnMut() -> CertificateResult<()>>(
        self,
        previous: Option<&PbrGeometrySource3D>,
        check: &mut F,
    ) -> CertificateResult<AdmissibleCpuGeometry3D> {
        check()?;
        if let Some(previous) = previous {
            self.0
                .validate_certified_successor_controlled(previous, check)?;
        }
        check()?;
        Ok(AdmissibleCpuGeometry3D(self.0))
    }
}
impl AdmissibleCpuGeometry3D {
    pub(super) fn into_source(self) -> PbrGeometrySource3D {
        self.0
    }
}

#[cfg(test)]
std::thread_local! { static CPU_GEOMETRY_VISITS: std::cell::Cell<[u64;3]> = const { std::cell::Cell::new([0;3]) }; }
#[cfg(test)]
pub(crate) fn cpu_geometry_visits_for_test() -> [u64; 3] {
    CPU_GEOMETRY_VISITS.with(|v| v.get())
}
#[cfg(test)]
fn cpu_geometry_visit(which: usize) {
    CPU_GEOMETRY_VISITS.with(|v| {
        let mut n = v.get();
        n[which] += 1;
        v.set(n);
    });
}

fn certificate_error(triangle: usize, cause: &'static str) -> super::pbr::PbrError3D {
    super::pbr::PbrError3D::FrameCertificate {
        triangle,
        normal_corner: None,
        tangent_corner: None,
        cause,
    }
}

fn cpu_equal<T: PartialEq, F: FnMut() -> CertificateResult<()>>(
    a: &[T],
    b: &[T],
    check: &mut F,
) -> CertificateResult<bool> {
    if a.len() != b.len() {
        return Ok(false);
    }
    for (a, b) in a.iter().zip(b) {
        check()?;
        if a != b {
            return Ok(false);
        }
    }
    Ok(true)
}
fn cpu_indices_equal<F: FnMut() -> CertificateResult<()>>(
    a: &GeometryIndices,
    b: &GeometryIndices,
    check: &mut F,
) -> CertificateResult<bool> {
    match (a, b) {
        (GeometryIndices::Unindexed, GeometryIndices::Unindexed) => Ok(true),
        (GeometryIndices::U16(a), GeometryIndices::U16(b)) => cpu_equal(a, b, check),
        (GeometryIndices::U32(a), GeometryIndices::U32(b)) => cpu_equal(a, b, check),
        _ => Ok(false),
    }
}

impl PbrGeometrySource3D {
    // Fixed nine allocations, irrespective of the source's vertex/triangle count.
    pub(super) fn cpu_allocations(&self) -> [(usize, usize); 9] {
        fn arc<T>(value: &std::sync::Arc<[T]>) -> (usize, usize) {
            (
                std::sync::Arc::as_ptr(value) as *const () as usize,
                std::mem::size_of_val(value.as_ref()),
            )
        }
        fn indices(value: &GeometryIndices) -> (usize, usize) {
            match value {
                GeometryIndices::Unindexed => (0, 0),
                GeometryIndices::U16(v) => arc(v),
                GeometryIndices::U32(v) => arc(v),
            }
        }
        let mut values = [(0, 0); 9];
        values[0] = arc(&self.vertices);
        values[1] = indices(&self.indices);
        if let Some(c) = &self.certified {
            values[2] = (
                std::sync::Arc::as_ptr(c) as usize,
                std::mem::size_of::<PbrCertifiedGeometry3D>(),
            );
            values[3] = arc(&c.input.original.vertices);
            values[4] = indices(&c.input.original.indices);
            values[5] = arc(&c.input.original.signed_tangents);
            values[6] = arc(&c.input.render_vertex_sources);
            values[7] = arc(&c.input.triangles);
            values[8] = arc(&c.frames);
        }
        values
    }
}

/// Closed directed binary64 enclosure, including coefficient/conversion error.
/// All real inputs are exact f32 values or exact witness f64 values. Overflow or
/// an unproved denominator refuses; no magnitude epsilon is introduced.
#[derive(Clone, Copy, Debug)]
struct FrameInterval {
    lo: f64,
    hi: f64,
}

impl FrameInterval {
    fn point(x: f64) -> Self {
        Self { lo: x, hi: x }
    }
    fn checked(lo: f64, hi: f64) -> CertificateResult<Self> {
        if !lo.is_finite() || !hi.is_finite() || lo > hi {
            return Err(certificate_error(
                0,
                "nonfinite/unbounded certificate arithmetic",
            ));
        }
        Ok(Self { lo, hi })
    }
    fn add(self, b: Self) -> CertificateResult<Self> {
        let (lo, hi) = (self.lo + b.lo, self.hi + b.hi);
        if !lo.is_finite() || !hi.is_finite() {
            return Err(certificate_error(0, "certificate addition overflow"));
        }
        if self.lo == self.hi && b.lo == b.hi {
            let remainder = lo - self.lo;
            let error = (self.lo - (lo - remainder)) + (b.lo - remainder);
            if error == 0.0 {
                return Ok(Self::point(lo));
            }
        }
        Self::checked(round_down(lo), round_up(hi))
    }
    fn neg(self) -> Self {
        Self {
            lo: -self.hi,
            hi: -self.lo,
        }
    }
    fn sub(self, b: Self) -> CertificateResult<Self> {
        self.add(b.neg())
    }
    fn mul(self, b: Self) -> CertificateResult<Self> {
        if self.lo == 0.0 && self.hi == 0.0 || b.lo == 0.0 && b.hi == 0.0 {
            return Ok(Self::point(0.0));
        }
        let p = [
            self.lo * b.lo,
            self.lo * b.hi,
            self.hi * b.lo,
            self.hi * b.hi,
        ];
        if p.iter().any(|v| !v.is_finite()) {
            return Err(certificate_error(0, "certificate multiplication overflow"));
        }
        // Two binary64 significands have at most106 product bits. This exponent
        // guard ensures an FMA residual cannot underflow, so zero is an EXACT
        // product witness, not a guessed magnitude tolerance.
        if self.lo == self.hi
            && b.lo == b.hi
            && p[0].abs() >= 2.0f64.powi(f64::MIN_EXP + 106)
            && self.lo.mul_add(b.lo, -p[0]) == 0.0
        {
            return Ok(Self::point(p[0]));
        }
        Self::checked(
            round_down(p.into_iter().fold(f64::INFINITY, f64::min)),
            round_up(p.into_iter().fold(f64::NEG_INFINITY, f64::max)),
        )
    }
    fn div(self, b: Self) -> CertificateResult<Self> {
        if b.lo <= 0.0 && b.hi >= 0.0 {
            return Err(certificate_error(
                0,
                "certificate denominator not separated from zero",
            ));
        }
        if self.lo == 0.0 && self.hi == 0.0 {
            return Ok(Self::point(0.0));
        }
        let r = [
            self.lo / b.lo,
            self.lo / b.hi,
            self.hi / b.lo,
            self.hi / b.hi,
        ];
        if r.iter().any(|v| !v.is_finite()) {
            return Err(certificate_error(0, "certificate division overflow"));
        }
        if self.lo == self.hi
            && b.lo == b.hi
            && self.lo.abs() >= 2.0f64.powi(f64::MIN_EXP + 106)
            && r[0].mul_add(b.lo, -self.lo) == 0.0
        {
            return Ok(Self::point(r[0]));
        }
        Self::checked(
            round_down(r.into_iter().fold(f64::INFINITY, f64::min)),
            round_up(r.into_iter().fold(f64::NEG_INFINITY, f64::max)),
        )
    }
    fn square(self) -> CertificateResult<Self> {
        if self.lo == self.hi {
            return self.mul(self);
        }
        let hi = self.lo.abs().max(self.hi.abs()).powi(2);
        let lo = if self.lo <= 0.0 && self.hi >= 0.0 {
            0.0
        } else {
            self.lo.abs().min(self.hi.abs()).powi(2)
        };
        if !hi.is_finite() {
            return Err(certificate_error(
                0,
                "certificate squared magnitude overflow",
            ));
        }
        Self::checked(if lo == 0.0 { 0.0 } else { round_down(lo) }, round_up(hi))
    }
    fn sqrt(self) -> CertificateResult<Self> {
        if self.lo < 0.0 {
            return Err(certificate_error(
                0,
                "certificate square root outside domain",
            ));
        }
        let root = self.lo.sqrt();
        if self.lo == self.hi
            && (self.lo == 0.0
                || self.lo >= 2.0f64.powi(f64::MIN_EXP + 106)
                    && root.mul_add(root, -self.lo) == 0.0)
        {
            return Ok(Self::point(root));
        }
        Self::checked(
            if self.lo == 0.0 {
                0.0
            } else {
                round_down(self.lo.sqrt())
            },
            round_up(self.hi.sqrt()),
        )
    }
    fn maximum_abs(self) -> f64 {
        self.lo.abs().max(self.hi.abs())
    }
}

type FrameVector = [FrameInterval; 3];
fn frame_points(v: [f32; 3]) -> CertificateResult<FrameVector> {
    if v.iter().any(|v| !v.is_finite()) {
        return Err(certificate_error(0, "nonfinite original/packed direction"));
    }
    Ok(v.map(|v| FrameInterval::point(f64::from(v))))
}
fn frame_sub(a: FrameVector, b: FrameVector) -> CertificateResult<FrameVector> {
    Ok([a[0].sub(b[0])?, a[1].sub(b[1])?, a[2].sub(b[2])?])
}
fn frame_scale(a: FrameVector, b: FrameInterval) -> CertificateResult<FrameVector> {
    Ok([a[0].mul(b)?, a[1].mul(b)?, a[2].mul(b)?])
}
fn frame_cross(a: FrameVector, b: FrameVector) -> CertificateResult<FrameVector> {
    Ok([
        a[1].mul(b[2])?.sub(a[2].mul(b[1])?)?,
        a[2].mul(b[0])?.sub(a[0].mul(b[2])?)?,
        a[0].mul(b[1])?.sub(a[1].mul(b[0])?)?,
    ])
}
fn frame_dot(a: FrameVector, b: FrameVector) -> CertificateResult<FrameInterval> {
    a[0].mul(b[0])?.add(a[1].mul(b[1])?)?.add(a[2].mul(b[2])?)
}
fn frame_length(a: FrameVector) -> CertificateResult<FrameInterval> {
    let sum = a[0].square()?.add(a[1].square()?)?.add(a[2].square()?)?;
    // Intersect with the proved real sum-of-squares domain. This tightens an
    // outward enclosure around zero; it never clamps source or shader data.
    FrameInterval::checked(sum.lo.max(0.0), sum.hi)?.sqrt()
}
fn frame_unit(a: FrameVector) -> CertificateResult<FrameVector> {
    let m = a.iter().map(|v| v.maximum_abs()).fold(0.0, f64::max);
    if !m.is_finite() || m == 0.0 {
        return Err(certificate_error(0, "zero/unbounded original direction"));
    }
    let scaled = [
        a[0].div(FrameInterval::point(m))?,
        a[1].div(FrameInterval::point(m))?,
        a[2].div(FrameInterval::point(m))?,
    ];
    let length = frame_length(scaled)?;
    if length.lo <= 0.0 {
        return Err(certificate_error(
            0,
            "direction nonzero length not certified",
        ));
    }
    Ok([
        scaled[0].div(length)?,
        scaled[1].div(length)?,
        scaled[2].div(length)?,
    ])
}
fn frame_witness(v: [f64; 3]) -> CertificateResult<FrameVector> {
    if v.iter().any(|v| !v.is_finite()) {
        return Err(certificate_error(0, "nonfinite separator representation"));
    }
    frame_unit(v.map(FrameInterval::point))
}
fn packed_direction(expected: FrameVector, supplied: [f32; 3]) -> CertificateResult<f64> {
    let packed = frame_points(supplied)?;
    if supplied.iter().all(|v| *v == 0.0) {
        return Err(certificate_error(0, "zero packed direction"));
    }
    for k in 0..3 {
        // Both bounds must round to the supplied f32. Equality treats signed
        // zero as the same real direction; ORIGINAL signed-zero bits are retained.
        if expected[k].lo as f32 != supplied[k] || expected[k].hi as f32 != supplied[k] {
            return Err(certificate_error(
                0,
                "original/derived-to-packed direction not certified",
            ));
        }
    }
    Ok(frame_length(frame_sub(packed, expected)?)?.hi)
}

fn certified_element_count(indices: &GeometryIndices, vertices: usize) -> usize {
    match indices {
        GeometryIndices::Unindexed => vertices,
        GeometryIndices::U16(v) => v.len(),
        GeometryIndices::U32(v) => v.len(),
    }
}
fn certified_index(
    indices: &GeometryIndices,
    element: usize,
    vertices: usize,
) -> CertificateResult<usize> {
    let index =
        match indices {
            GeometryIndices::Unindexed => element,
            GeometryIndices::U16(v) => usize::from(*v.get(element).ok_or_else(|| {
                certificate_error(element / 3, "incomplete original/render indices")
            })?),
            GeometryIndices::U32(v) => usize::try_from(*v.get(element).ok_or_else(|| {
                certificate_error(element / 3, "incomplete original/render indices")
            })?)
            .map_err(|_| certificate_error(element / 3, "unaddressable source index"))?,
        };
    if index >= vertices {
        return Err(certificate_error(
            element / 3,
            "original/render index outside source",
        ));
    }
    Ok(index)
}
fn certified_index_bytes(indices: &GeometryIndices) -> CertificateResult<u64> {
    let value = match indices {
        GeometryIndices::Unindexed => Some(0),
        GeometryIndices::U16(v) => (v.len() as u64).checked_mul(2),
        GeometryIndices::U32(v) => (v.len() as u64).checked_mul(4),
    };
    value.ok_or_else(|| certificate_error(0, "original index byte count overflow"))
}

fn certified_capture_bounds(
    render_vertices: usize,
    render_elements: usize,
    input: &PbrCertifiedInput3D,
) -> CertificateResult<(u64, u64)> {
    let original = &input.original;
    let count = certified_element_count(&original.indices, original.vertices.len());
    if original.identity.source == 0
        || original.identity.revision == 0
        || count == 0
        || count % 3 != 0
        || count != render_elements
        || input.triangles.len() != count / 3
        || input.render_vertex_sources.len() != render_vertices
    {
        return Err(certificate_error(
            0,
            "incomplete identity/triangle/corner association",
        ));
    }
    if input.mapped && original.signed_tangents.len() != count
        || !original.signed_tangents.is_empty() && original.signed_tangents.len() != count
    {
        return Err(certificate_error(
            0,
            "missing/incomplete original signed tangent corners",
        ));
    }
    let add = |a: u64, b: u64| {
        a.checked_add(b)
            .ok_or_else(|| certificate_error(0, "certified source size/work overflow"))
    };
    let mul = |a: usize, b: usize| {
        (a as u64)
            .checked_mul(b as u64)
            .ok_or_else(|| certificate_error(0, "certified source size overflow"))
    };
    let mut bytes = std::mem::size_of::<PbrCertifiedGeometry3D>() as u64;
    for v in [
        mul(
            original.vertices.len(),
            std::mem::size_of::<PbrOriginalVertex3D>(),
        )?,
        certified_index_bytes(&original.indices)?,
        mul(original.signed_tangents.len(), 16)?,
        mul(input.render_vertex_sources.len(), 4)?,
        mul(
            input.triangles.len(),
            std::mem::size_of::<PbrTriangleCertificate3D>(),
        )?,
        mul(
            input.triangles.len(),
            std::mem::size_of::<PbrCheckedFrame3D>(),
        )?,
        14 * std::mem::size_of::<usize>() as u64,
    ] {
        bytes = add(bytes, v)?;
    }
    let mut items = 0;
    for v in [
        original.vertices.len(),
        count,
        render_vertices,
        render_elements,
        input.triangles.len(),
    ] {
        items = add(items, v as u64)?;
    }
    if bytes > input.maximum_certificate_bytes || items > input.maximum_verification_items {
        return Err(certificate_error(
            0,
            "producer certificate byte/work bound exceeded",
        ));
    }
    Ok((bytes, items))
}

// Exact determinant of three finite f32 vectors: each two-factor product is
// exact in f64 (48 bits). FMA retains the third-factor product residual (72 bits).
// Products lie in exponents [-447,384], so neither product nor residual overflows
// or underflows f64. TwoSum growth preserves the exact sum of the twelve terms.
fn exact_source_determinant(
    a: [f32; 3],
    b: [f32; 3],
    c: [f32; 3],
) -> CertificateResult<FrameInterval> {
    let mut expansion = [0.0f64; 16];
    let mut used = 0usize;
    for (i, j, k, sign) in [
        (0, 1, 2, 1.0),
        (1, 2, 0, 1.0),
        (2, 0, 1, 1.0),
        (0, 2, 1, -1.0),
        (1, 0, 2, -1.0),
        (2, 1, 0, -1.0),
    ] {
        let ab = f64::from(a[i]) * f64::from(b[j]);
        let p = ab * f64::from(c[k]);
        let error = ab.mul_add(f64::from(c[k]), -p);
        for value in [sign * error, sign * p] {
            let mut q = value;
            let mut next = [0.0; 16];
            let mut n = 0;
            for &e in &expansion[..used] {
                let sum = q + e;
                let bb = sum - q;
                let residual = (q - (sum - bb)) + (e - bb);
                if residual != 0.0 {
                    next[n] = residual;
                    n += 1;
                }
                q = sum;
            }
            if q != 0.0 {
                next[n] = q;
                n += 1;
            }
            expansion = next;
            used = n;
        }
    }
    if used == 0 {
        return Ok(FrameInterval::point(0.0));
    }
    let mut result = FrameInterval::point(0.0);
    for &value in &expansion[..used] {
        result = result.add(FrameInterval::point(value))?;
    }
    Ok(result)
}

fn source_det2(a: [f32; 3], b: [f32; 3], x: usize, y: usize) -> CertificateResult<FrameInterval> {
    let p = f64::from(a[x]) * f64::from(b[y]);
    let q = f64::from(a[y]) * f64::from(b[x]);
    if p == q {
        return Ok(FrameInterval::point(0.0));
    }
    FrameInterval::point(p).sub(FrameInterval::point(q))
}

fn source_ray_in_cone(q: [f32; 3], basis: &[[f32; 3]]) -> CertificateResult<bool> {
    let v = frame_points(q)?;
    match basis.len() {
        1 => {
            let b = basis[0];
            if [(0, 1), (0, 2), (1, 2)].into_iter().any(|(i, j)| {
                f64::from(q[i]) * f64::from(b[j]) != f64::from(q[j]) * f64::from(b[i])
            }) {
                return Ok(false);
            }
            Ok(frame_dot(v, frame_points(b)?)?.lo > 0.0)
        }
        2 => {
            let determinant = exact_source_determinant(q, basis[0], basis[1])?;
            if determinant.lo != 0.0 || determinant.hi != 0.0 {
                return Ok(false);
            }
            for (x, y) in [(0, 1), (0, 2), (1, 2)] {
                let den = source_det2(basis[0], basis[1], x, y)?;
                if den.lo <= 0.0 && den.hi >= 0.0 {
                    continue;
                }
                let a = source_det2(q, basis[1], x, y)?.div(den)?;
                let b = source_det2(basis[0], q, x, y)?.div(den)?;
                return Ok(a.lo >= 0.0 && b.lo >= 0.0 && a.add(b)?.lo > 0.0);
            }
            Ok(false)
        }
        3 => {
            let den = exact_source_determinant(basis[0], basis[1], basis[2])?;
            if den.lo <= 0.0 && den.hi >= 0.0 {
                return Ok(false);
            }
            let coefficients = [
                exact_source_determinant(q, basis[1], basis[2])?,
                exact_source_determinant(basis[0], q, basis[2])?,
                exact_source_determinant(basis[0], basis[1], q)?,
            ];
            let mut sum = FrameInterval::point(0.0);
            for numerator in coefficients {
                let c = if numerator.lo == 0.0 && numerator.hi == 0.0 {
                    numerator
                } else {
                    numerator.div(den)?
                };
                if c.lo < 0.0 {
                    return Ok(false);
                }
                sum = sum.add(c)?;
            }
            Ok(sum.lo > 0.0)
        }
        _ => Ok(false),
    }
}

fn positive_edge_coordinates(q: FrameVector, a: [f32; 3], b: [f32; 3]) -> CertificateResult<bool> {
    for (x, y) in [(0, 1), (0, 2), (1, 2)] {
        let den = source_det2(a, b, x, y)?;
        if den.lo <= 0.0 && den.hi >= 0.0 {
            continue;
        }
        let ca = q[x]
            .mul(FrameInterval::point(f64::from(b[y])))?
            .sub(q[y].mul(FrameInterval::point(f64::from(b[x])))?)?
            .div(den)?;
        let cb = FrameInterval::point(f64::from(a[x]))
            .mul(q[y])?
            .sub(FrameInterval::point(f64::from(a[y])).mul(q[x])?)?
            .div(den)?;
        return Ok(ca.lo > 0.0 && cb.lo > 0.0);
    }
    Ok(false)
}

fn verify_singularity(
    w: PbrSmoothSingularity3D,
    normals: [[f32; 3]; 3],
    tangents: [[f32; 3]; 3],
) -> CertificateResult<()> {
    let get = |values: &[[f32; 3]; 3], index: u8| {
        values.get(usize::from(index)).copied().ok_or_else(|| {
            certificate_error(0, "singularity witness corner outside original triangle")
        })
    };
    let passed = match w {
        PbrSmoothSingularity3D::MixedCone {
            generators,
            reverse_coefficients,
            antiparallel,
        } => {
            if generators
                .iter()
                .enumerate()
                .any(|(i, x)| *x >= 6 || generators[..i].contains(x))
                || !generators.iter().any(|x| *x < 3)
                || !generators.iter().any(|x| *x >= 3)
            {
                return Err(certificate_error(
                    0,
                    "invalid mixed-cone original generators",
                ));
            }
            let rays = generators.map(|j| {
                if j < 3 {
                    normals[usize::from(j)]
                } else {
                    tangents[usize::from(j - 3)].map(|x| if antiparallel { x } else { -x })
                }
            });
            let mut normal_ray = [FrameInterval::point(0.0); 3];
            let mut tangent_ray = [FrameInterval::point(0.0); 3];
            for i in 0..4 {
                let others: [usize; 3] = match i {
                    0 => [1, 2, 3],
                    1 => [0, 2, 3],
                    2 => [0, 1, 3],
                    _ => [0, 1, 2],
                };
                let mut coefficient =
                    exact_source_determinant(rays[others[0]], rays[others[1]], rays[others[2]])?;
                if (i % 2 == 1) != reverse_coefficients {
                    coefficient = coefficient.neg();
                }
                if coefficient.lo <= 0.0 {
                    return Err(certificate_error(
                        0,
                        "mixed-cone cofactor is not strictly positive",
                    ));
                }
                let value = frame_scale(frame_points(rays[i])?, coefficient)?;
                let target = if generators[i] < 3 {
                    &mut normal_ray
                } else {
                    &mut tangent_ray
                };
                for k in 0..3 {
                    target[k] = target[k].add(value[k])?;
                }
            }
            // The alternating-cofactor identity is EXACT for the original f32
            // columns. Separately certify each positive source sum is nonzero;
            // normalization then gives independent convex weights and the same
            // parallel/antiparallel ray, not a rounded residual-as-zero test.
            normal_ray.iter().any(|v| v.lo > 0.0 || v.hi < 0.0)
                && tangent_ray.iter().any(|v| v.lo > 0.0 || v.hi < 0.0)
        }
        PbrSmoothSingularity3D::VertexInCone {
            normal_vertex,
            vertex,
            basis,
            count,
            antiparallel,
        } => {
            if !(1..=3).contains(&count)
                || basis[..usize::from(count)]
                    .iter()
                    .enumerate()
                    .any(|(i, v)| basis[..i].contains(v))
            {
                return Err(certificate_error(0, "invalid singularity cone basis"));
            }
            let (from, to) = if normal_vertex {
                (&normals, &tangents)
            } else {
                (&tangents, &normals)
            };
            let q = get(from, vertex)?;
            let mut b = [[0.0; 3]; 3];
            for i in 0..usize::from(count) {
                b[i] = get(to, basis[i])?.map(|v| if antiparallel { -v } else { v });
            }
            source_ray_in_cone(q, &b[..usize::from(count)])?
        }
        PbrSmoothSingularity3D::EdgePlanes {
            normals: ni,
            tangents: ti,
            reverse_ray,
            antiparallel,
        } => {
            if ni[0] == ni[1] || ti[0] == ti[1] {
                return Err(certificate_error(0, "repeated singularity edge corner"));
            }
            let na = get(&normals, ni[0])?;
            let nb = get(&normals, ni[1])?;
            let ta = get(&tangents, ti[0])?.map(|v| if antiparallel { -v } else { v });
            let tb = get(&tangents, ti[1])?.map(|v| if antiparallel { -v } else { v });
            let mut q = frame_cross(
                frame_cross(frame_points(na)?, frame_points(nb)?)?,
                frame_cross(frame_points(ta)?, frame_points(tb)?)?,
            )?;
            if reverse_ray {
                q = q.map(FrameInterval::neg);
            }
            // Exact ray is constructed as the intersection of the two planes.
            // Nonzero plus strictly positive cone coordinates supplies positive
            // finite sums; dividing each sum yields independent simplex weights.
            q.iter().any(|v| v.lo > 0.0 || v.hi < 0.0)
                && positive_edge_coordinates(q, na, nb)?
                && positive_edge_coordinates(q, ta, tb)?
        }
    };
    if !passed {
        return Err(certificate_error(
            0,
            "original smooth singularity was not proved",
        ));
    }
    Ok(())
}

fn derived_source_frame(
    p: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
) -> CertificateResult<(FrameVector, FrameVector, f32)> {
    let a = frame_sub(frame_points(p[1])?, frame_points(p[0])?)?;
    let b = frame_sub(frame_points(p[2])?, frame_points(p[0])?)?;
    let face = frame_unit(frame_cross(a, b)?)?;
    let du1 =
        FrameInterval::point(f64::from(uv[1][0])).sub(FrameInterval::point(f64::from(uv[0][0])))?;
    let dv1 =
        FrameInterval::point(f64::from(uv[1][1])).sub(FrameInterval::point(f64::from(uv[0][1])))?;
    let du2 =
        FrameInterval::point(f64::from(uv[2][0])).sub(FrameInterval::point(f64::from(uv[0][0])))?;
    let dv2 =
        FrameInterval::point(f64::from(uv[2][1])).sub(FrameInterval::point(f64::from(uv[0][1])))?;
    let det = du1.mul(dv2)?.sub(du2.mul(dv1)?)?;
    let h = if det.lo > 0.0 {
        1.0
    } else if det.hi < 0.0 {
        -1.0
    } else {
        return Err(certificate_error(
            0,
            "source UV determinant sign not certified",
        ));
    };
    let raw = frame_scale(
        frame_sub(frame_scale(a, dv2)?, frame_scale(b, dv1)?)?,
        FrameInterval::point(1.0).div(det)?,
    )?;
    let dv = frame_scale(
        frame_sub(frame_scale(b, du1)?, frame_scale(a, du2)?)?,
        FrameInterval::point(1.0).div(det)?,
    )?;
    let projected = frame_sub(raw, frame_scale(face, frame_dot(face, raw)?)?)?;
    let tangent = frame_unit(projected)?;
    let hand = frame_dot(
        frame_scale(
            frame_cross(face, tangent)?,
            FrameInterval::point(f64::from(h)),
        )?,
        dv,
    )?;
    if hand.lo <= 0.0 {
        return Err(certificate_error(
            0,
            "geometric bitangent orientation not certified",
        ));
    }
    Ok((face, tangent, h))
}

fn mode_c_dyadic_difference(p: [f32; 3], v: [f32; 3]) -> CertificateResult<FrameInterval> {
    let p = p.map(f64::from);
    let v = v.map(f64::from);
    // Exact expansion of (p1-p0)*(v2-v0)-(p2-p0)*(v1-v0).
    // Each original binary32 product is exact in binary64 (<=48 bits),
    // between exponents -298 and +256. Six such terms cannot overflow;
    // their error-free sum/residuals cannot underflow binary64 either.
    let products = [
        p[1] * v[2],
        -(p[1] * v[0]),
        -(p[0] * v[2]),
        -(p[2] * v[1]),
        p[2] * v[0],
        p[0] * v[1],
    ];
    if products.iter().any(|x| !x.is_finite()) {
        return Err(certificate_error(0, "nonfinite mode-C dyadic product"));
    }
    let mut expansion = [0.0f64; 6];
    let mut length = 0;
    for product in products {
        let mut carry = product;
        let mut next = [0.0f64; 6];
        let mut count = 0;
        for &component in &expansion[..length] {
            // General two_sum: sum+error is the EXACT carry+component.
            // Retain every nonzero residual; no rounded cancellation is lost.
            let sum = carry + component;
            let virtual_component = sum - carry;
            let error = (carry - (sum - virtual_component)) + (component - virtual_component);
            if error != 0.0 {
                next[count] = error;
                count += 1;
            }
            carry = sum;
        }
        next[count] = carry;
        expansion = next;
        length = count + 1;
    }
    // Enclose the completed expansion, not the cancelling original products.
    // Existing directed operators include any final binary64 rounding.
    let mut bound = FrameInterval::point(0.0);
    for component in &expansion[..length] {
        bound = bound.add(FrameInterval::point(*component))?;
    }
    Ok(bound)
}

fn derived_source_frame_c(
    p: [[f32; 3]; 3],
    uv: [[f32; 2]; 3],
) -> CertificateResult<(FrameVector, FrameVector, f32)> {
    // Keep the existing explicit area, determinant, nonzero projection and
    // bitangent-orientation qualifications. A/B retain their original path.
    let (face, _, h) = derived_source_frame(p, uv)?;
    let v = uv.map(|uv| uv[1]);
    let a = [
        mode_c_dyadic_difference(p.map(|p| p[0]), v)?,
        mode_c_dyadic_difference(p.map(|p| p[1]), v)?,
        mode_c_dyadic_difference(p.map(|p| p[2]), v)?,
    ];
    // For these SAME exact source values, C=e1×e2 is orthogonal to both
    // edges, hence C·A=0 for A=e1*dv2-e2*dv1. The explicitly qualified
    // projection G is therefore exactly A/det, and U=sign(det)*A/|A|.
    // This removes interval dependency inflation, not a source projection.
    let oriented = if h < 0.0 {
        a.map(FrameInterval::neg)
    } else {
        a
    };
    Ok((face, frame_unit(oriented)?, h))
}

impl PbrGeometrySource3D {
    /// The supplied render buffer is already producer-authored. This operation
    /// verifies its meaning; it never selects C or repairs an old constructor.
    pub(crate) fn try_new_certified(
        render: Vec<PbrVertex52>,
        indices: GeometryIndices,
        input: PbrCertifiedInput3D,
    ) -> CertificateResult<Self> {
        Self::try_new_certified_controlled(render, indices, input, &mut || Ok(()))
    }

    pub(super) fn prepare_cpu_certified<F: FnMut() -> CertificateResult<()>>(
        render: Vec<PbrVertex52>,
        indices: GeometryIndices,
        input: PbrCertifiedInput3D,
        check: &mut F,
    ) -> CertificateResult<VerifiedCpuGeometry3D> {
        Self::try_new_certified_controlled(render, indices, input, check).map(VerifiedCpuGeometry3D)
    }

    fn try_new_certified_controlled<F: FnMut() -> CertificateResult<()>>(
        render: Vec<PbrVertex52>,
        indices: GeometryIndices,
        input: PbrCertifiedInput3D,
        check: &mut F,
    ) -> CertificateResult<Self> {
        check()?;
        #[cfg(test)]
        cpu_geometry_visit(0);
        certified_capture_bounds(
            render.len(),
            certified_element_count(&indices, render.len()),
            &input,
        )?;
        let mut vertices = Vec::new();
        vertices
            .try_reserve_exact(render.len())
            .map_err(|_| certificate_error(0, "certified render storage unavailable"))?;
        for (index, v) in render.into_iter().enumerate() {
            check()?;
            if v.position
                .iter()
                .chain(v.normal.iter())
                .chain(v.uv0.iter())
                .chain(v.tangent.iter())
                .any(|v| !v.is_finite())
            {
                return Err(certificate_error(0, "nonfinite supplied render vertex"));
            }
            if v.tangent[3] != 1.0 && v.tangent[3] != -1.0 {
                return Err(certificate_error(0, "render sign not exact +/-1"));
            }
            let original = input
                .original
                .vertices
                .get(input.render_vertex_sources[index] as usize)
                .ok_or_else(|| certificate_error(0, "render mapping outside original source"))?;
            let uv0 = if original.uv0.is_some() {
                Some(
                    super::TextureCoordinate2::try_new(v.uv0[0], v.uv0[1])
                        .map_err(|_| certificate_error(0, "invalid render UV0"))?,
                )
            } else {
                if v.uv0.map(f32::to_bits) != [0.0f32; 2].map(f32::to_bits) {
                    return Err(certificate_error(
                        0,
                        "absent UV0 fetch is not canonical zero",
                    ));
                }
                None
            };
            vertices.push(PbrVertexSource3D {
                position: super::LocalPoint3::try_new(v.position[0], v.position[1], v.position[2])?,
                // Storage of the CHECKED actual packed triple; no claim of exact
                // UNIT binary storage and no call to the old unit constructor.
                normal: PbrUnitDirection3D(v.normal.map(f64::from)),
                uv0,
                color: Some(super::LinearVertexColorRgba8::from_linear_unorm8(
                    v.color[0], v.color[1], v.color[2], v.color[3],
                )),
                tangent: Some((
                    PbrUnitDirection3D([
                        f64::from(v.tangent[0]),
                        f64::from(v.tangent[1]),
                        f64::from(v.tangent[2]),
                    ]),
                    v.tangent[3],
                )),
            });
        }
        let mut source = Self {
            vertices: vertices.into(),
            indices,
            certified: None,
        };
        let (frames, metadata_bytes, verification_items) =
            source.check_certified_input(&input, check)?;
        source.certified = Some(std::sync::Arc::new(PbrCertifiedGeometry3D {
            input,
            frames: frames.into(),
            metadata_bytes,
            verification_items,
        }));
        Ok(source)
    }

    pub(crate) fn certified_input(&self) -> Option<&PbrCertifiedInput3D> {
        self.certified.as_ref().map(|c| &c.input)
    }
    #[cfg(test)]
    pub(crate) fn certified_frames(&self) -> Option<&[PbrCheckedFrame3D]> {
        self.certified.as_ref().map(|c| c.frames.as_ref())
    }
    pub(super) fn certificate_metadata_bytes(&self) -> u64 {
        self.certified.as_ref().map_or(0, |c| c.metadata_bytes)
    }
    pub(super) fn validate_certified_registration(&self) -> CertificateResult<()> {
        if let Some(c) = &self.certified {
            let (frames, bytes, items) = self.check_certified_input(&c.input, &mut || Ok(()))?;
            if frames.as_slice() != c.frames.as_ref()
                || bytes != c.metadata_bytes
                || items != c.verification_items
            {
                return Err(certificate_error(
                    0,
                    "changed captured certificate/packed association",
                ));
            }
        }
        Ok(())
    }
    pub(super) fn validate_certified_successor(&self, previous: &Self) -> CertificateResult<()> {
        self.validate_certified_successor_controlled(previous, &mut || Ok(()))
    }

    fn validate_certified_successor_controlled<F: FnMut() -> CertificateResult<()>>(
        &self,
        previous: &Self,
        check: &mut F,
    ) -> CertificateResult<()> {
        check()?;
        #[cfg(test)]
        cpu_geometry_visit(2);
        let (Some(next), Some(old)) = (&self.certified, &previous.certified) else {
            return Ok(());
        };
        let a = &next.input.original;
        let b = &old.input.original;
        let same_asset = a.identity.source == b.identity.source
            && a.identity.node == b.identity.node
            && a.identity.mesh == b.identity.mesh
            && a.identity.primitive == b.identity.primitive;
        if same_asset && a.identity.revision < b.identity.revision {
            return Err(certificate_error(0, "stale original source revision"));
        }
        if same_asset && a.identity.revision == b.identity.revision {
            let mut tangent_equal = a.signed_tangents.len() == b.signed_tangents.len();
            if tangent_equal {
                for (a, b) in a.signed_tangents.iter().zip(b.signed_tangents.iter()) {
                    check()?;
                    if a.map(f32::to_bits) != b.map(f32::to_bits) {
                        tangent_equal = false;
                        break;
                    }
                }
            }
            if !cpu_equal(&a.vertices, &b.vertices, check)?
                || !cpu_indices_equal(&a.indices, &b.indices, check)?
                || (a.tangent_origin == b.tangent_origin && !tangent_equal)
            {
                return Err(certificate_error(
                    0,
                    "changed original bytes behind unchanged source generation",
                ));
            }
        }
        // Certificate generations belong to their own immutable records. An
        // original-source successor does not permit reusing a changed record
        // under the old certificate revision.
        if same_asset {
            for (a, b) in next.input.triangles.iter().zip(old.input.triangles.iter()) {
                check()?;
                if a.certificate_identity == b.certificate_identity
                    && (a.certificate_revision < b.certificate_revision
                        || (a.certificate_revision == b.certificate_revision && a != b))
                {
                    return Err(certificate_error(
                        a.triangle,
                        "changed/stale certificate behind unchanged certificate generation",
                    ));
                }
            }
        }
        Ok(())
    }

    fn check_certified_input<F: FnMut() -> CertificateResult<()>>(
        &self,
        input: &PbrCertifiedInput3D,
        check: &mut F,
    ) -> CertificateResult<(Vec<PbrCheckedFrame3D>, u64, u64)> {
        check()?;
        #[cfg(test)]
        cpu_geometry_visit(1);
        let original = &input.original;
        let count = certified_element_count(&original.indices, original.vertices.len());
        let (bytes, items) =
            certified_capture_bounds(self.vertices.len(), self.element_count(), input)?;
        if let PbrTangentOrigin3D::DefaultMikk {
            producer,
            revision,
            version,
        } = original.tangent_origin
        {
            if producer == 0 || revision == 0 || version == [0; 3] {
                return Err(certificate_error(
                    0,
                    "missing default-Mikk producer/revision/version provenance",
                ));
            }
        }
        for v in original.vertices.iter() {
            check()?;
            frame_points(v.position)?;
            frame_unit(frame_points(v.normal)?)?;
            if v.uv0.is_some_and(|uv| uv.iter().any(|v| !v.is_finite())) {
                return Err(certificate_error(0, "nonfinite original UV0"));
            }
        }
        for t in original.signed_tangents.iter() {
            check()?;
            frame_unit(frame_points([t[0], t[1], t[2]])?)?;
            if t[3] != 1.0 && t[3] != -1.0 {
                return Err(certificate_error(0, "invalid original tangent sign"));
            }
        }
        for (index, &src) in input.render_vertex_sources.iter().enumerate() {
            check()?;
            let v = &self.vertices[index];
            let o = original
                .vertices
                .get(src as usize)
                .ok_or_else(|| certificate_error(0, "render mapping outside original source"))?;
            if [v.position.x(), v.position.y(), v.position.z()].map(f32::to_bits)
                != o.position.map(f32::to_bits)
                || v.uv0
                    .map(|u| [u.u(), u.v()])
                    .unwrap_or([0.0; 2])
                    .map(f32::to_bits)
                    != o.uv0.unwrap_or([0.0; 2]).map(f32::to_bits)
                || v.color.unwrap_or(DEFAULT_LINEAR_VERTEX_COLOR).to_array()
                    != o.color.unwrap_or([255; 4])
            {
                return Err(certificate_error(
                    0,
                    "render position/UV/color differs from original source",
                ));
            }
        }
        let mut frames = Vec::new();
        frames
            .try_reserve_exact(count / 3)
            .map_err(|_| certificate_error(0, "checked frame storage unavailable"))?;
        for triangle in 0..count / 3 {
            check()?;
            let result = (|| {
                let certificate = &input.triangles[triangle];
                if certificate.original != original.identity
                    || certificate.triangle != triangle
                    || certificate.certificate_identity == 0
                    || certificate.certificate_revision == 0
                {
                    return Err(certificate_error(
                        triangle,
                        "certificate source/triangle identity mismatch",
                    ));
                }
                let mut oi = [0; 3];
                let mut ri = [0; 3];
                for k in 0..3 {
                    oi[k] = certified_index(
                        &original.indices,
                        triangle * 3 + k,
                        original.vertices.len(),
                    )?;
                    ri[k] = self.index(triangle * 3 + k)?;
                    if input.render_vertex_sources[ri[k]] as usize != oi[k] {
                        return Err(certificate_error(
                            triangle,
                            "render corner reordered or changed original topology",
                        ));
                    }
                }
                let p = oi.map(|i| original.vertices[i].position);
                let face = frame_unit(frame_cross(
                    frame_sub(frame_points(p[1])?, frame_points(p[0])?)?,
                    frame_sub(frame_points(p[2])?, frame_points(p[0])?)?,
                )?)?;
                let actual_n = ri.map(|i| self.vertices[i].normal.0.map(|v| v as f32));
                let mut smooth_n = [[0.0; 3]; 3];
                let mut smooth_t = [[0.0; 3]; 3];
                let mut maximum_pack_error = 0.0f64;
                for k in 0..3 {
                    let n = frame_unit(frame_points(original.vertices[oi[k]].normal)?)?;
                    // Qualify original smooth PACKED candidate independently of
                    // the producer's geometric render values for C singularity.
                    smooth_n[k] = n.map(|v| ((v.lo + v.hi) * 0.5) as f32);
                    packed_direction(n, smooth_n[k])?;
                    if input.mapped {
                        let t = original.signed_tangents[triangle * 3 + k];
                        let t = frame_unit(frame_points([t[0], t[1], t[2]])?)?;
                        smooth_t[k] = t.map(|v| ((v.lo + v.hi) * 0.5) as f32);
                        packed_direction(t, smooth_t[k])?;
                    }
                }
                let mut actual_t = [[0.0; 3]; 3];
                let mut uv = [[0.0; 2]; 3];
                if input.mapped {
                    for k in 0..3 {
                        uv[k] = original.vertices[oi[k]].uv0.ok_or_else(|| {
                            certificate_error(triangle, "mapped original source missing UV0")
                        })?;
                        let (t, s) = self.vertices[ri[k]].tangent.ok_or_else(|| {
                            certificate_error(triangle, "mapped render tangent missing")
                        })?;
                        actual_t[k] = t.0.map(|v| v as f32);
                        if s != 1.0 && s != -1.0
                            || k > 0 && self.vertices[ri[0]].tangent.unwrap().1 != s
                        {
                            return Err(certificate_error(
                                triangle,
                                "render tangent sign seam not split",
                            ));
                        }
                    }
                }
                let separator;
                let mut supplied_lower = None;
                match &certificate.frame {
                    PbrFrameCertificate3D::SourceGeometric {
                        singularity,
                        separator: b,
                        lower,
                    } => {
                        if !input.mapped {
                            return Err(certificate_error(
                                triangle,
                                "C requires actual mapped source",
                            ));
                        }
                        let original_n = oi.map(|i| original.vertices[i].normal);
                        let original_t = std::array::from_fn(|k| {
                            let t = original.signed_tangents[triangle * 3 + k];
                            [t[0], t[1], t[2]]
                        });
                        // Positive normalization scalars preserve the original
                        // cones. Require existence in both the exact raw source
                        // and its actual normalized binary32 candidate.
                        verify_singularity(*singularity, original_n, original_t)?;
                        verify_singularity(*singularity, smooth_n, smooth_t)?;
                        let (f, u, h) = derived_source_frame_c(p, uv)?;
                        for k in 0..3 {
                            maximum_pack_error = maximum_pack_error
                                .max(packed_direction(f, actual_n[k])?)
                                .max(packed_direction(u, actual_t[k])?);
                            if self.vertices[ri[k]].tangent.unwrap().1 != h
                                || actual_n[k].map(f32::to_bits) != actual_n[0].map(f32::to_bits)
                                || actual_t[k].map(f32::to_bits) != actual_t[0].map(f32::to_bits)
                            {
                                return Err(certificate_error(
                                    triangle,
                                    "C packed frame/sign is not the identical source-derived triangle frame",
                                ));
                            }
                        }
                        separator = Some(frame_witness(*b)?);
                        supplied_lower = Some(*lower);
                    }
                    other => {
                        for k in 0..3 {
                            let unit = frame_unit(frame_points(original.vertices[oi[k]].normal)?)?;
                            maximum_pack_error =
                                maximum_pack_error.max(packed_direction(unit, actual_n[k])?);
                            if input.mapped {
                                let raw = original.signed_tangents[triangle * 3 + k];
                                let unit = frame_unit(frame_points([raw[0], raw[1], raw[2]])?)?;
                                maximum_pack_error =
                                    maximum_pack_error.max(packed_direction(unit, actual_t[k])?);
                                if self.vertices[ri[k]].tangent.unwrap().1 != raw[3] {
                                    return Err(certificate_error(
                                        triangle,
                                        "render sign differs from original signed source tangent",
                                    ));
                                }
                            }
                        }
                        match other {
                            PbrFrameCertificate3D::OldCone => {
                                for &i in &oi {
                                    if frame_dot(
                                        frame_unit(frame_points(original.vertices[i].normal)?)?,
                                        face,
                                    )?
                                    .lo < 0.75
                                    {
                                        return Err(certificate_error(
                                            triangle,
                                            "A original normal cone below0.75",
                                        ));
                                    }
                                }
                                if input.mapped {
                                    let (_, u, _) = derived_source_frame(p, uv)?;
                                    for k in 0..3 {
                                        let raw = original.signed_tangents[triangle * 3 + k];
                                        if frame_dot(
                                            frame_unit(frame_points([raw[0], raw[1], raw[2]])?)?,
                                            u,
                                        )?
                                        .lo < 0.75
                                        {
                                            return Err(certificate_error(
                                                triangle,
                                                "A original tangent cone below0.75",
                                            ));
                                        }
                                    }
                                    separator = Some(frame_unit(frame_cross(face, u)?)?);
                                } else {
                                    separator = Some(face);
                                }
                            }
                            PbrFrameCertificate3D::AuthoredSmooth {
                                separator: b,
                                lower,
                            } => {
                                if !input.mapped {
                                    return Err(certificate_error(
                                        triangle,
                                        "B requires actual mapped source",
                                    ));
                                }
                                // Also qualifies face, UV determinant and projection.
                                derived_source_frame(p, uv)?;
                                separator = Some(frame_witness(*b)?);
                                supplied_lower = Some(*lower);
                            }
                            PbrFrameCertificate3D::NormalOnly { separator: b, .. } => {
                                if input.mapped {
                                    return Err(certificate_error(
                                        triangle,
                                        "normal-only certificate cannot erase a present normal role",
                                    ));
                                }
                                separator = Some(frame_witness(*b)?);
                            }
                            PbrFrameCertificate3D::SourceGeometric { .. } => unreachable!(),
                        }
                    }
                }
                let witness = separator
                    .ok_or_else(|| certificate_error(triangle, "missing frame witness"))?;
                let mut normal_max = 0.0f64;
                for n in actual_n {
                    normal_max = normal_max.max(frame_length(frame_points(n)?)?.hi);
                }
                if input.mapped {
                    let mut tangent_max = 0.0f64;
                    for t in actual_t {
                        tangent_max = tangent_max.max(frame_length(frame_points(t)?)?.hi);
                    }
                    let mut cross_min = f64::INFINITY;
                    for i in 0..3 {
                        for j in 0..3 {
                            let bound = frame_dot(
                                witness,
                                frame_cross(
                                    frame_points(actual_n[i])?,
                                    frame_points(actual_t[j])?,
                                )?,
                            )?
                            .lo;
                            let claimed = supplied_lower.map_or(1.0 / 16.0, |v| v[i * 3 + j]);
                            if !claimed.is_finite() || claimed < 1.0 / 16.0 || bound < claimed {
                                return Err(super::pbr::PbrError3D::FrameCertificate {
                                    triangle,
                                    normal_corner: Some(i as u8),
                                    tangent_corner: Some(j as u8),
                                    cause: "nine-pair packed cross lower bound not certified",
                                });
                            }
                            cross_min = cross_min.min(bound);
                        }
                    }
                    Ok(PbrCheckedFrame3D {
                        normal_lower: round_down(cross_min / tangent_max),
                        tangent_lower: round_down(cross_min / normal_max),
                        cross_lower: cross_min,
                        local_g_lower: round_down(cross_min / normal_max),
                        maximum_direction_pack_error: maximum_pack_error,
                    })
                } else {
                    let mut minimum = f64::INFINITY;
                    for i in 0..3 {
                        let bound = frame_dot(witness, frame_points(actual_n[i])?)?.lo;
                        let claimed = match certificate.frame {
                            PbrFrameCertificate3D::NormalOnly { lower, .. } => lower[i],
                            _ => 1.0 / 16.0,
                        };
                        if !claimed.is_finite() || claimed < 1.0 / 16.0 || bound < claimed {
                            return Err(certificate_error(
                                triangle,
                                "normal-only packed lower bound not certified",
                            ));
                        }
                        minimum = minimum.min(bound);
                    }
                    Ok(PbrCheckedFrame3D {
                        normal_lower: minimum,
                        tangent_lower: 0.0,
                        cross_lower: 0.0,
                        local_g_lower: 0.0,
                        maximum_direction_pack_error: maximum_pack_error,
                    })
                }
            })();
            frames.push(result.map_err(|e| match e {
                super::pbr::PbrError3D::FrameCertificate {
                    normal_corner,
                    tangent_corner,
                    cause,
                    ..
                } => super::pbr::PbrError3D::FrameCertificate {
                    triangle,
                    normal_corner,
                    tangent_corner,
                    cause,
                },
                other => other,
            })?);
        }
        Ok((frames, bytes, items))
    }
}
