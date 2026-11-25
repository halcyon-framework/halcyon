//! Deterministic flat HAL355/R024/R025 source shared by the floor proofs.

use super::three_d::*;
use glam::{DMat4, DVec3, Vec3};
use std::sync::Arc;

pub(super) const EXTENT: [u32; 2] = [1920, 1080];
pub(super) const OCCURRENCES: usize = 540;
pub(super) const MOVED_SLOT: usize = 28 + 8 * 16 + 11;
pub(super) const HDR_ID: u64 = 355_540;
const BOX_SIZE: [f32; 3] = [2.0, 0.5, 4.0];
const NEAR: f32 = 0.15;
const SHADOW_FAR: f32 = 50.0;
const VERTICAL_FOV: f32 = 55.0_f32.to_radians();

pub(super) struct FloorSource {
    pub(super) snapshot: PbrSnapshot3D,
    pub(super) placements: Vec<ModelPlacement3>,
    pub(super) geometry_generation: (u32, u64),
    pub(super) material_generations: [(u32, u64); 4],
}

fn box_geometry(context: &Context3D) -> PbrSourceResult3D<PbrGeometry3D> {
    let identity = PbrOriginalIdentity3D {
        source: HDR_ID,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let mut vertices = Vec::with_capacity(24);
    let mut indices = Vec::with_capacity(36);
    let mut triangles = Vec::with_capacity(12);
    for (n, t) in [
        (Vec3::X, Vec3::Z),
        (Vec3::NEG_X, Vec3::Z),
        (Vec3::Y, Vec3::X),
        (Vec3::NEG_Y, Vec3::X),
        (Vec3::Z, Vec3::X),
        (Vec3::NEG_Z, Vec3::X),
    ] {
        let b = n.cross(t);
        let first = u16::try_from(vertices.len()).unwrap();
        for uv0 in [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]] {
            let corner = n * 0.5 + t * (uv0[0] - 0.5) + b * (uv0[1] - 0.5);
            vertices.push(PbrVertex3D {
                position: (corner * Vec3::from_array(BOX_SIZE)).to_array(),
                normal: n.to_array(),
                uv0,
                color: [255; 4],
                tangent: t.extend(1.0).to_array(),
            });
        }
        indices.extend([first, first + 1, first + 2, first, first + 2, first + 3]);
        for _ in 0..2 {
            let triangle = triangles.len();
            triangles.push(PbrTriangleCertificate3D {
                original: identity,
                triangle,
                certificate_identity: HDR_ID * 100 + triangle as u64,
                certificate_revision: 1,
                frame: PbrFrameCertificate3D::NormalOnly {
                    separator: n.to_array().map(f64::from),
                    lower: [0.5; 3],
                },
            });
        }
    }
    let original_vertices = vertices
        .iter()
        .map(|vertex| PbrOriginalVertex3D {
            position: vertex.position,
            normal: vertex.normal,
            uv0: Some(vertex.uv0),
            color: Some(vertex.color),
        })
        .collect();
    let signed_tangents = indices
        .iter()
        .map(|&index| vertices[usize::from(index)].tangent)
        .collect();
    let indices = GeometryIndices::U16(indices.into());
    context.admit_pbr_geometry(
        PbrGeometryDescriptor3D {
            original: PbrOriginalGeometry3D {
                identity,
                vertices: original_vertices,
                indices: indices.clone(),
                signed_tangents,
                tangent_provenance: PbrTangentProvenance3D::Authored,
            },
            render_vertex_sources: (0..vertices.len() as u32).collect(),
            render: vertices.into(),
            render_indices: indices,
            triangles: triangles.into(),
            mapped: false,
            maximum_certificate_bytes: 65_536,
            maximum_verification_items: 1_000,
        },
        None,
    )
}

fn placement(center: [f64; 3], dimensions: [f32; 3]) -> ModelPlacement3 {
    let [x, y, z] = center;
    let scale = Vec3::from_array(dimensions) / Vec3::from_array(BOX_SIZE);
    // R024 §7.1: one anisotropic source box keeps every actual TRS ratio <= 64.
    assert!(scale.max_element() / scale.min_element() <= 64.0);
    ModelPlacement3::try_new(
        WorldPoint3::try_new(x, y, z).unwrap(),
        Orientation3::identity(),
        PositiveScale3::try_new(scale.x, scale.y, scale.z).unwrap(),
    )
    .unwrap()
}

impl FloorSource {
    pub(super) fn admit(context: &Context3D) -> PbrSourceResult3D<Self> {
        let geometry = box_geometry(context)?;
        let mut materials = Vec::with_capacity(4);
        for (index, (base_rgba, metallic, roughness)) in [
            ([0.46, 0.52, 0.61, 1.0], 0.0, 0.86),
            ([0.81, 0.80, 0.76, 1.0], 0.0, 0.82),
            ([0.31, 0.39, 0.42, 1.0], 0.15, 0.71),
            ([0.78, 0.24, 0.10, 1.0], 0.0, 0.67),
        ]
        .into_iter()
        .enumerate()
        {
            let mut source =
                PbrMaterialDescriptor3D::new(HDR_ID + index as u64, 1, PbrSamplingFamily3D::V1);
            source.base_rgba = base_rgba;
            source.metallic = metallic;
            source.roughness = roughness;
            materials.push(context.admit_pbr_material(source, None)?);
        }
        let mut placements = Vec::with_capacity(OCCURRENCES);
        let mut packets = Vec::with_capacity(OCCURRENCES);
        let mut push = |role: usize, center, dimensions| {
            packets.push(PbrDrawPacket3D {
                geometry: geometry.clone(),
                material: materials[role].clone(),
                raster: RasterMode::SingleSided,
                stable_ordinal: placements.len() as u64,
            });
            placements.push(placement(center, dimensions));
        };
        push(0, [0.0, -0.10, 0.0], [32.0, 0.20, 48.0]);
        push(1, [0.0, 4.075, 0.0], [32.0, 0.15, 48.0]);
        for x in [-15.0, -9.0, -3.0, 3.0, 9.0, 15.0] {
            push(2, [x, 3.875, 0.0], [0.40, 0.25, 48.0]);
        }
        for x in [-13.5, -9.5, -5.5, -1.5, 2.5, 6.5, 10.5, 14.0] {
            push(2, [x, 3.45, 0.0], [0.40, 0.22, 43.0]);
        }
        for x in [-15.0, 15.0] {
            for z in [-20.0, -12.0, -4.0, 4.0, 12.0, 20.0] {
                push(2, [x, 1.875, z], [0.60, 3.75, 0.60]);
            }
        }
        for row in 0..32 {
            for column in 0..16 {
                // Integer decimetres keep the selected grid reproducible.
                let x = f64::from(-120 + 16 * column) / 10.0;
                let z = f64::from(-220 + 14 * row) / 10.0;
                push(3, [x, 0.40, z], [0.60, 0.80, 0.60]);
            }
        }
        assert_eq!(placements.len(), OCCURRENCES);
        let snapshot = context.admit_pbr_snapshot(StructuralRevision::new(1), packets)?;
        Ok(Self {
            snapshot,
            placements,
            geometry_generation: geometry.resource_generation(),
            material_generations: std::array::from_fn(|i| materials[i].resource_generation()),
        })
    }

    pub(super) fn scene(
        &self,
        context: &Context3D,
        samples: SampleCount3D,
        revision: u64,
    ) -> PbrSourceResult3D<PbrScene3D> {
        let model = context.admit_pbr_model(
            &self.snapshot,
            ModelRevision::new(revision),
            self.placements.clone(),
        )?;
        let direct = PbrDirectSource3D::capture(
            PbrDirectDescriptor3D {
                identity: HDR_ID,
                revision,
                meters_per_scene_unit: 1.0,
                directional: Some(PbrDirectionalLight3D {
                    surface_to_source: sun_direction().to_array(),
                    irradiance: [3.0; 3],
                    shadow_far_m: f64::from(SHADOW_FAR),
                }),
                points: Arc::from([]),
                spots: Arc::from([]),
            },
            &self.snapshot,
            &model,
        )?;
        context.admit_pbr_scene(PbrSceneDescriptor3D {
            snapshot: self.snapshot.clone(),
            model,
            hdr: PbrHdrSource3D {
                identity: HDR_ID,
                revision,
                exposure_ev: 0.0,
                // R025's whole-scene conservative bound is 3*(1+1/(6*.67^6)) < 16.
                maximum_rgb: [16.0; 3],
            },
            samples,
            direct: Some(direct),
            indirect: None,
        })
    }

    pub(super) fn move_fixture(&mut self) {
        let original = self.placements[MOVED_SLOT];
        let p = original.translation();
        self.placements[MOVED_SLOT] = ModelPlacement3::try_new(
            WorldPoint3::try_new(p.x() + 0.8, p.y(), p.z()).unwrap(),
            original.orientation(),
            original.scale(),
        )
        .unwrap();
    }
}

pub(super) fn sun_direction() -> DVec3 {
    DVec3::new(0.12, 0.18, -1.0).normalize()
}

pub(super) fn camera(second: bool) -> ViewDescriptor3 {
    let [x, y, z] = if second {
        [8.0, 3.0, -15.0]
    } else {
        [0.0, 2.0, -19.0]
    };
    let eye = DVec3::new(x, y, z);
    let target = DVec3::new(0.0, 1.7, 12.0);
    let (_, rotation, _) = DMat4::look_at_rh(eye, target, DVec3::Y)
        .inverse()
        .to_scale_rotation_translation();
    let [qx, qy, qz, qw] = rotation.to_array().map(|v| v as f32);
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(x, y, z).unwrap(),
            Orientation3::try_from_xyzw(qx, qy, qz, qw).unwrap(),
        ),
        PerspectiveProjection::try_finite(VERTICAL_FOV, NEAR, SHADOW_FAR)
            .unwrap()
            .into(),
    )
}

pub(super) fn bounds(placement: ModelPlacement3) -> (DVec3, DVec3) {
    let point = placement.translation();
    let center = DVec3::new(point.x(), point.y(), point.z());
    let scale = placement.scale();
    let half =
        (Vec3::new(scale.x(), scale.y(), scale.z()) * Vec3::from_array(BOX_SIZE)).as_dvec3() * 0.5;
    (center - half, center + half)
}
