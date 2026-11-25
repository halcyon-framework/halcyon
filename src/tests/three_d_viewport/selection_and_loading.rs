use super::*;

#[test]
fn hal374_actual_private_programs_n1_n4() {
    let (_renderer, context, _) =
        setup(PolicyLimits3D::conservative()).expect("actual Metal renderer");
    for samples in [1, 4] {
        crate::three_d::validate_outline_programs_for_test(&context, samples).unwrap();
    }
}

fn hal374_cube_geometry(context: &Context3D) -> crate::three_d::PbrGeometry3D {
    use crate::three_d as a;
    let id = a::PbrOriginalIdentity3D {
        source: 374001,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let mut original = Vec::new();
    let mut render = Vec::new();
    let mut triangles = Vec::new();
    for axis in 0..3 {
        let u = (axis + 1) % 3;
        let v = (axis + 2) % 3;
        for sign in [-1.0, 1.0] {
            let mut normal = [0.0; 3];
            normal[axis] = sign;
            let mut tangent = [0.0, 0.0, 0.0, 1.0];
            tangent[u] = 1.0;
            let mut corners = [[0.0; 3]; 4];
            for (corner, (x, y)) in
                corners
                    .iter_mut()
                    .zip([(-0.5, -0.5), (0.5, -0.5), (0.5, 0.5), (-0.5, 0.5)])
            {
                corner[axis] = sign * 0.5;
                corner[u] = x;
                corner[v] = y;
            }
            let order = if sign > 0.0 {
                [0, 1, 2, 0, 2, 3]
            } else {
                [0, 2, 1, 0, 3, 2]
            };
            for i in order {
                original.push(a::PbrOriginalVertex3D {
                    position: corners[i],
                    normal,
                    uv0: None,
                    color: None,
                });
                render.push(a::PbrVertex3D {
                    position: corners[i],
                    normal,
                    uv0: [0.0; 2],
                    color: [255; 4],
                    tangent,
                });
            }
            for _ in 0..2 {
                let triangle = triangles.len();
                triangles.push(a::PbrTriangleCertificate3D {
                    original: id,
                    triangle,
                    certificate_identity: 374010 + triangle as u64,
                    certificate_revision: 1,
                    frame: a::PbrFrameCertificate3D::NormalOnly {
                        separator: normal.map(f64::from),
                        lower: [0.5; 3],
                    },
                });
            }
        }
    }
    let mapping = (0..render.len() as u32).collect();
    context
        .admit_pbr_geometry(
            a::PbrGeometryDescriptor3D {
                original: a::PbrOriginalGeometry3D {
                    identity: id,
                    vertices: original.into(),
                    indices: GeometryIndices::Unindexed,
                    signed_tangents: Arc::from([]),
                    tangent_provenance: a::PbrTangentProvenance3D::Authored,
                },
                render: render.into(),
                render_indices: GeometryIndices::Unindexed,
                render_vertex_sources: mapping,
                triangles: triangles.into(),
                mapped: false,
                maximum_certificate_bytes: 65536,
                maximum_verification_items: 10000,
            },
            None,
        )
        .unwrap()
}

fn hal374_scene(context: &Context3D, n: u32) -> crate::three_d::PbrScene3D {
    hal374_scene_control(context, n, false)
}

fn hal374_scene_control(
    context: &Context3D,
    n: u32,
    emission_control: bool,
) -> crate::three_d::PbrScene3D {
    use crate::three_d as a;
    let geometry = hal374_cube_geometry(context);
    let mut packets = Vec::new();
    for (slot, color) in [
        [0.88, 0.27, 0.055, 1.0],
        [0.045, 0.34, 0.88, 1.0],
        [0.3, 0.32, 0.35, 1.0],
    ]
    .into_iter()
    .enumerate()
    {
        let mut material =
            a::PbrMaterialDescriptor3D::new(374100 + slot as u64, 1, a::PbrSamplingFamily3D::V1);
        material.base_rgba = color;
        material.metallic = 0.15;
        material.roughness = 0.65;
        if emission_control {
            material.emissive_rgb = match slot {
                0 => [0.2, 0.0, 0.0],
                1 => [0.0, 0.4, 0.0],
                _ => [0.0, 0.0, 0.6],
            };
        }
        let material = context.admit_pbr_material(material, None).unwrap();
        packets.push(a::PbrDrawPacket3D {
            geometry: geometry.clone(),
            material,
            raster: RasterMode::SingleSided,
            // Deliberately not occurrence order: selection must NEVER use this.
            stable_ordinal: 900 - slot as u64,
        });
    }
    let snapshot = context
        .admit_pbr_snapshot(StructuralRevision::new(1), packets)
        .unwrap();
    let pitch = 0.19f32;
    let yaw = -0.27f32;
    let rotation = Orientation3::try_from_xyzw(pitch.sin(), 0.0, 0.0, pitch.cos())
        .unwrap()
        .compose_local(Orientation3::try_from_xyzw(0.0, yaw.sin(), 0.0, yaw.cos()).unwrap())
        .unwrap();
    let placement = |p: [f64; 3], scale: [f32; 3]| {
        ModelPlacement3::try_new(
            WorldPoint3::try_new(p[0], p[1], p[2]).unwrap(),
            rotation,
            PositiveScale3::try_new(scale[0], scale[1], scale[2]).unwrap(),
        )
        .unwrap()
    };
    let model = context
        .admit_pbr_model(
            &snapshot,
            ModelRevision::new(1),
            vec![
                placement([-0.70, 0.0, 0.0], [1.1, 1.1, 1.1]),
                placement([0.70, 0.0, 0.0], [1.0, 1.0, 1.0]),
                // Its complete source z interval is in front of both selected cubes,
                // giving a non-ambiguous foreground-depth oracle, not a bias change.
                placement([0.0, 0.0, 2.0], [0.35, 1.6, 0.2]),
            ],
        )
        .unwrap();
    let inv = 14.0f64.sqrt().recip();
    let direct = if emission_control {
        None
    } else {
        Some(
            a::PbrDirectSource3D::capture(
                a::PbrDirectDescriptor3D {
                    identity: 374200,
                    revision: 1,
                    meters_per_scene_unit: 1.0,
                    directional: Some(a::PbrDirectionalLight3D {
                        surface_to_source: [-1.0, 2.0, 3.0].map(|v| v * inv),
                        irradiance: [3.0, 2.8, 2.5],
                        shadow_far_m: 24.0,
                    }),
                    points: Arc::from([a::PbrPointLight3D {
                        position: WorldPoint3::try_new(3.0, 1.0, 4.0).unwrap(),
                        intensity: [6.0, 8.0, 10.0],
                        radius_m: 0.5,
                        range_m: 24.0,
                    }]),
                    spots: Arc::from([]),
                },
                &snapshot,
                &model,
            )
            .unwrap(),
        )
    };
    // Same source bound as the accepted true-box native fixture: r=.65,
    // f*cos<3.21 and incident RGB<=3+10/.5²=43, giving RGB<139<256.
    context
        .admit_pbr_scene(a::PbrSceneDescriptor3D {
            snapshot,
            model,
            hdr: a::PbrHdrSource3D {
                identity: 374300 + u64::from(emission_control),
                revision: 1,
                exposure_ev: 0.0,
                maximum_rgb: if emission_control {
                    [0.2, 0.4, 0.6]
                } else {
                    [256.0; 3]
                },
            },
            samples: SampleCount3D::try_new(n).unwrap(),
            direct,
            indirect: None,
        })
        .unwrap()
}

fn hal374_view() -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 5.0).unwrap(),
            Orientation3::identity(),
        ),
        PerspectiveProjection::try_finite(45.0f32.to_radians(), 0.1, 20.0)
            .unwrap()
            .into(),
    )
}

fn hal374_half_pixels(
    texture: &wgpu::Texture,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
) -> Vec<[u16; 4]> {
    let row = (texture.width() * 8).div_ceil(256) * 256;
    let bytes = u64::from(row) * u64::from(texture.height());
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("HAL374 exact full FP16 proof"),
        size: bytes,
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    encoder.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row),
                rows_per_image: Some(texture.height()),
            },
        },
        wgpu::Extent3d {
            width: texture.width(),
            height: texture.height(),
            depth_or_array_layers: 1,
        },
    );
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let mapped = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let mut result = Vec::new();
    for y in 0..texture.height() {
        for x in 0..texture.width() {
            let at = (y * row + x * 8) as usize;
            result.push(std::array::from_fn(|c| {
                u16::from_le_bytes(mapped[at + c * 2..at + c * 2 + 2].try_into().unwrap())
            }));
        }
    }
    drop(mapped);
    buffer.unmap();
    result
}

fn hal374_float_samples(
    texture: &wgpu::Texture,
    gpu: &crate::renderer::wgpu_renderer::GpuResources,
) -> Vec<[f32; 4]> {
    let n = texture.sample_count();
    let width = texture.width();
    let height = texture.height();
    let ty = if n == 1 {
        "texture_2d<f32>"
    } else {
        "texture_multisampled_2d<f32>"
    };
    let load = if n == 1 {
        "textureLoad(tex,vec2<i32>(id.xy),0)"
    } else {
        "textureLoad(tex,vec2<i32>(id.xy),i32(s))"
    };
    let source = format!(
        r#"
@group(0) @binding(0) var tex:{ty};
@group(0) @binding(1) var<storage,read_write> values:array<vec4<f32>>;
@compute @workgroup_size(8,8) fn main(@builtin(global_invocation_id) id:vec3<u32>) {{
 if id.x>={width}u||id.y>={height}u {{return;}}
 for(var s=0u;s<{n}u;s++){{values[(id.y*{width}u+id.x)*{n}u+s]={load};}}
}}
"#
    );
    let module = gpu
        .device
        .create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("HAL374 read-only actual sample observation"),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        });
    let layout = gpu
        .device
        .create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: None,
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: n == 4,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
    let pipeline_layout = gpu
        .device
        .create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: None,
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
    let pipeline = gpu
        .device
        .create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: None,
            layout: Some(&pipeline_layout),
            module: &module,
            entry_point: Some("main"),
            compilation_options: Default::default(),
            cache: None,
        });
    let size = u64::from(width) * u64::from(height) * u64::from(n) * 16;
    let output = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size,
        usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let view = texture.create_view(&Default::default());
    let binding = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[
            wgpu::BindGroupEntry {
                binding: 0,
                resource: wgpu::BindingResource::TextureView(&view),
            },
            wgpu::BindGroupEntry {
                binding: 1,
                resource: output.as_entire_binding(),
            },
        ],
    });
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_compute_pass(&Default::default());
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &binding, &[]);
        pass.dispatch_workgroups(width.div_ceil(8), height.div_ceil(8), 1);
    }
    encoder.copy_buffer_to_buffer(&output, 0, &readback, 0, size);
    gpu.queue.submit([encoder.finish()]);
    let (tx, rx) = std::sync::mpsc::channel();
    readback
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| tx.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    rx.recv().unwrap().unwrap();
    let data = readback.slice(..).get_mapped_range().expect("mapped readback range");
    let values = data
        .chunks_exact(16)
        .map(|v| {
            std::array::from_fn(|i| f32::from_le_bytes(v[i * 4..i * 4 + 4].try_into().unwrap()))
        })
        .collect();
    drop(data);
    readback.unmap();
    values
}

fn hal374_half(v: u16) -> f32 {
    let e = (v >> 10) & 31;
    let f = v & 1023;
    assert_eq!(v >> 15, 0);
    assert_ne!(e, 31);
    if e == 0 {
        f32::from(f) * 2.0f32.powi(-24)
    } else {
        (1.0 + f32::from(f) / 1024.0) * 2.0f32.powi(i32::from(e) - 15)
    }
}

fn hal374_flat_source(context: &Context3D, n: u32, mask: bool) -> crate::three_d::PbrScene3D {
    flat_pbr_fixture(context, n, mask.then_some((1.0, 0.0, 1.0)), false)
}

fn flat_pbr_fixture(
    context: &Context3D,
    n: u32,
    mask: Option<(f32, f64, f32)>,
    shared_occurrence: bool,
) -> crate::three_d::PbrScene3D {
    use crate::three_d::*;
    let id = PbrOriginalIdentity3D {
        source: 374400,
        revision: 1,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let points = [
        [-1., -1., 0.],
        [1., -1., 0.],
        [-1., 1., 0.],
        [1., -1., 0.],
        [1., 1., 0.],
        [-1., 1., 0.],
    ];
    let geometry = context
        .admit_pbr_geometry(
            PbrGeometryDescriptor3D {
                original: PbrOriginalGeometry3D {
                    identity: id,
                    vertices: points
                        .map(|position| PbrOriginalVertex3D {
                            position,
                            normal: [0., 0., 1.],
                            uv0: None,
                            color: None,
                        })
                        .into(),
                    indices: GeometryIndices::Unindexed,
                    signed_tangents: Arc::from([]),
                    tangent_provenance: PbrTangentProvenance3D::Authored,
                },
                render: points
                    .map(|position| PbrVertex3D {
                        position,
                        normal: [0., 0., 1.],
                        uv0: [0.; 2],
                        color: [255; 4],
                        tangent: [1., 0., 0., 1.],
                    })
                    .into(),
                render_indices: GeometryIndices::Unindexed,
                render_vertex_sources: Arc::from([0, 1, 2, 3, 4, 5]),
                triangles: (0..2)
                    .map(|triangle| PbrTriangleCertificate3D {
                        original: id,
                        triangle,
                        certificate_identity: 374401 + triangle as u64,
                        certificate_revision: 1,
                        frame: PbrFrameCertificate3D::NormalOnly {
                            separator: [0., 0., 1.],
                            lower: [0.5; 3],
                        },
                    })
                    .collect(),
                mapped: false,
                maximum_certificate_bytes: 65536,
                maximum_verification_items: 1000,
            },
            None,
        )
        .unwrap();
    let mut material = PbrMaterialDescriptor3D::new(374410, 1, PbrSamplingFamily3D::V1);
    material.emissive_rgb = [0.125, 0.25, 0.5];
    let material = context.admit_pbr_material(material, None).unwrap();
    let mut packets = vec![PbrDrawPacket3D {
        geometry: geometry.clone(),
        material,
        raster: RasterMode::SingleSided,
        stable_ordinal: 999,
    }];
    let mut placements = vec![
        ModelPlacement3::try_new(
            WorldPoint3::origin(),
            Orientation3::identity(),
            PositiveScale3::one(),
        )
        .unwrap(),
    ];
    if shared_occurrence {
        packets.push(PbrDrawPacket3D {
            geometry: geometry.clone(),
            material: packets[0].material.clone(),
            raster: RasterMode::SingleSided,
            stable_ordinal: 727,
        });
        placements.push(
            ModelPlacement3::try_new(
                WorldPoint3::try_new(0.55, 0.0, 0.125).unwrap(),
                Orientation3::identity(),
                PositiveScale3::try_new(0.25, 0.25, 0.25).unwrap(),
            )
            .unwrap(),
        );
    }
    if let Some((opacity, y, height)) = mask {
        let mut descriptor = PbrMaterialDescriptor3D::new(374411, 1, PbrSamplingFamily3D::V1);
        descriptor.alpha_mode = PbrAlphaMode3D::Mask;
        descriptor.base_rgba[3] = opacity;
        let material = context.admit_pbr_material(descriptor, None).unwrap();
        packets.push(PbrDrawPacket3D {
            geometry,
            material,
            raster: RasterMode::SingleSided,
            stable_ordinal: 123,
        });
        placements.push(
            ModelPlacement3::try_new(
                WorldPoint3::try_new(0., y, 1.).unwrap(),
                Orientation3::identity(),
                PositiveScale3::try_new(height, height, height).unwrap(),
            )
            .unwrap(),
        );
    }
    let snapshot = context
        .admit_pbr_snapshot(StructuralRevision::new(1), packets)
        .unwrap();
    let model = context
        .admit_pbr_model(&snapshot, ModelRevision::new(1), placements)
        .unwrap();
    context
        .admit_pbr_scene(PbrSceneDescriptor3D {
            snapshot,
            model,
            hdr: PbrHdrSource3D {
                identity: 374420,
                revision: 1,
                exposure_ev: 0.,
                maximum_rgb: [0.125, 0.25, 0.5],
            },
            samples: SampleCount3D::try_new(n).unwrap(),
            direct: None,
            indirect: None,
        })
        .unwrap()
}

pub(super) fn hal374_flat_view() -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0., 0., 5.).unwrap(),
            Orientation3::identity(),
        ),
        crate::three_d::OrthographicProjection::try_new(4., 1., 9.)
            .unwrap()
            .into(),
    )
}

#[test]
fn hal375_registered_mask_depth_gates_real_surface_text_n1_n4() {
    use crate::core::math::Point;
    use crate::core::scene::{ClipNode, Color, TextNode};
    use crate::renderer::sdf_frame_custody::SdfFramePlan;
    use crate::three_d::PbrSpatialHudAnchor3D;
    use std::num::NonZeroU64;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum Occurrence {
        Current,
        Missing,
        Duplicate,
        Clipped,
        WrongScale,
        RetiredRegistration,
        SmallAtlas,
    }

    for (samples, scale, mask, expected) in [
        (1, 1.0, (1.0, 0.0, 1.0), [1, 1, 0, 0, 0]),
        (4, 2.0, (1.0, 0.0, 1.0), [1, 1, 0, 0, 0]),
        (1, 1.0, (0.0, 0.0, 1.0), [1, 1, 1, 1, 1]),
        (4, 1.25, (0.0, 0.0, 1.0), [1, 1, 1, 1, 1]),
        // At 600 logical pixels high and a 4-unit vertical view the thin
        // foreground Mask spans y≈297. Anchor y=300, label ends at y=294.
        (1, 1.0, (1.0, 0.02, 0.009), [1, 1, 0, 1, 1]),
        (4, 1.25, (1.0, 0.02, 0.009), [1, 1, 0, 1, 1]),
    ] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        assert!(gpu.device.features().is_empty());
        let scene = flat_pbr_fixture(&context, samples, Some(mask), true);
        let anchor = |id, packet_slot, x, y, z, text| PbrSpatialHudAnchor3D {
            label_id: NonZeroU64::new(id).unwrap(),
            packet_slot,
            world: WorldPoint3::try_new(x, y, z).unwrap(),
            text: Arc::from(text),
        };
        let anchors = [
            anchor(1, 0, -0.55, 0.2, 2.0, "Original"),
            anchor(2, 1, 0.55, 0.2, 2.0, "Shared mesh"),
            anchor(3, 0, 0.0, 0.0, 0.5, "Blocked"),
            anchor(4, 2, 0.55, -0.40, 1.0, "Depth tie"),
            // Anchor and leader are below the full Mask. Only the label box crosses it.
            anchor(5, 1, -0.55, -1.08, 0.5, "Box blocked"),
        ];
        assert!(
            context
                .admit_pbr_spatial_hud(&scene, &[anchors[0].clone(), anchors[0].clone()])
                .is_err()
        );
        assert!(
            context
                .admit_pbr_spatial_hud(&scene, &[anchor(6, 3, 0.0, 0.0, 2.0, "Bad slot")])
                .is_err()
        );
        assert!(
            context
                .admit_pbr_spatial_hud(&scene, &[anchor(6, 0, 0.0, 0.0, 2.0, "Two\nlines")])
                .is_err()
        );
        let hud = context.admit_pbr_spatial_hud(&scene, &anchors).unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            scene.clone(),
            Viewport3DViewRevision::new(1),
            hal374_flat_view(),
        )
        .unwrap();
        assert!(viewport.request_pbr_spatial_hud(Some(hud.clone())).unwrap());
        assert!(!viewport.request_pbr_spatial_hud(Some(hud.clone())).unwrap());
        let rect = RectF::new(0.0, 0.0, 800.0, 600.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        let w = (rect.width * scale) as u32;
        let h = (rect.height * scale) as u32;
        renderer
            .register_headless_render_target(surface, (w, h), scale)
            .unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(viewport), None, 0);
        let mut harness = RetainedSceneTestHarness::new();
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let mut render = |renderer: &mut WgpuRenderer, tree: &mut WidgetTree, occurrence| {
            tree.mark_paint_dirty(root);
            let frame = harness.frame_with_custom_render_target(
                tree,
                root,
                Constraints::tight(rect.size()),
                &theme,
                renderer.text_engine_mut(),
                &icons,
                CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(w, h), scale).unwrap(),
            );
            renderer.drain_pending_image_registrations();
            let mut data = SceneGraph::new();
            data.root = frame.scene_root;
            match occurrence {
                Occurrence::Missing => data.root.clear(),
                Occurrence::Duplicate => data.root.extend(data.root.clone()),
                Occurrence::Clipped => {
                    let children = std::mem::take(&mut data.root);
                    data.root.push(SceneNode::Clip(ClipNode {
                        rect: RectF::new(-20.0, -20.0, 10.0, 10.0),
                        rounded: None,
                        children,
                        z_index: LAYER_CONTENT,
                        key: None,
                    }));
                }
                Occurrence::RetiredRegistration => {
                    let id = tree_viewport(tree, root)
                        .custom_render_admission()
                        .unwrap()
                        .presentation_id();
                    let prepared = harness
                        .custom_render_presentation_bindings()
                        .prepared_companion_for_test(id, surface)
                        .unwrap();
                    // A new ordinary registration of the very same sampled
                    // view still cannot satisfy the old pin's generation.
                    renderer.register_image_from_view(
                        prepared.texture_id,
                        prepared.front.view().clone(),
                    );
                }
                Occurrence::Current | Occurrence::WrongScale | Occurrence::SmallAtlas => {}
            }
            data.root.push(SceneNode::Text(TextNode {
                binding: None,
                position: Point::new(20.0, 24.0),
                text: if occurrence == Occurrence::SmallAtlas {
                    "A"
                } else {
                    "Ordinary text"
                }
                .into(),
                font_size: 13.0,
                color: Color::WHITE,
                z_index: LAYER_CONTENT + 3,
                backgrounds: Vec::new(),
                key: None,
            }));
            renderer.advance_frame();
            let frame_counter = renderer.frame_counter();
            let mut occurrences = harness
                .custom_render_presentation_bindings_mut()
                .begin_materialization_for_test(frame_counter);
            let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
                &std::collections::HashMap::from([(surface, data)]),
                compositor.snapshot_multi(),
                &mut MeshCache::new(),
                &mut occurrences,
                |_| {
                    if occurrence == Occurrence::WrongScale {
                        scale * 0.5
                    } else {
                        scale
                    }
                },
                frame_counter,
            ).expect("scene must satisfy DrawPlan Build admission");
            renderer
                .compile_sdf_frame_with_presentation_bindings(
                    SdfFramePlan::new(plan, Default::default()),
                    None,
                    occurrences.freeze(),
                )
                .unwrap()
                .expect_ready("native Compile must be ready")
        };
        let compiled = render(&mut renderer, &mut tree, Occurrence::Current);
        let data = &compiled.plan().surface_data[&surface];
        assert!(data.text_realization().outcome().is_complete());
        assert!(
            compiled.companion_refusals_for_test().is_empty(),
            "{:?}",
            compiled.companion_refusals_for_test()
        );
        let mut key = data
            .frame_batches
            .batches
            .iter()
            .find(|batch| batch.key.pipeline == PipelineId::Image)
            .unwrap()
            .key
            .clone();
        key.z_layer += 1;
        let leader = key.effective_order();
        key.z_layer += 1;
        assert_eq!(
            compiled.companion_draw_order_for_test(surface),
            [(true, leader), (false, key.effective_order())]
        );
        let decorated = read_surface_pixels(&renderer, w, h, |view| {
            renderer.encode_sdf_target_for_test(&compiled, surface, view)
        });
        let [(work_surface, work)] = compiled
            .companion_work()
            .collect::<Vec<_>>()
            .try_into()
            .unwrap();
        assert_eq!(work_surface, surface);
        assert_eq!(
            work.read_visibility_for_test(&gpu),
            expected,
            "N{samples} at scale {scale}, Mask {mask:?}"
        );
        let canonical =
            hal374_half_pixels(tree_viewport(&tree, root).hdr_front_for_test(surface), &gpu);
        drop(compiled);
        if mask == (1.0, 0.0, 1.0) {
            for (case, expected_vertices, refused) in [
                (Occurrence::Missing, 0, false),
                (Occurrence::Duplicate, 8, false),
                (Occurrence::Clipped, 4, true),
                (Occurrence::WrongScale, 4, true),
            ] {
                let output = render(&mut renderer, &mut tree, case);
                assert!(output.companion_work().next().is_none(), "{case:?}");
                assert!(output.companion_draw_order_for_test(surface).is_empty());
                assert_eq!(
                    output.companion_refusals_for_test().len(),
                    usize::from(refused)
                );
                let data = output.plan().surface_compiled_data(surface).unwrap();
                assert!(data.text_realization().outcome().is_complete(), "{case:?}");
                assert_eq!(
                    data.frame_batches.total_image_vertices, expected_vertices,
                    "{case:?}"
                );
            }
        }
        assert!(
            tree.node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Viewport3D>()
                .unwrap()
                .request_pbr_spatial_hud(None)
                .unwrap()
        );
        tree.mark_paint_dirty(root);
        let plain = render(&mut renderer, &mut tree, Occurrence::Current);
        assert!(
            plain
                .plan()
                .text_realization(surface)
                .unwrap()
                .outcome()
                .is_complete()
        );
        assert!(plain.companion_work().next().is_none());
        assert_eq!(
            hal374_half_pixels(tree_viewport(&tree, root).hdr_front_for_test(surface), &gpu),
            canonical
        );
        let original = read_compiled_surface_pixels(&renderer, plain.plan(), surface, w, h);
        let changed: Vec<_> = decorated
            .iter()
            .zip(&original)
            .enumerate()
            .filter(|(_, (a, b))| a != b)
            .map(|(i, _)| {
                (
                    ((i as u32 % w) as f32 / scale) as u32,
                    ((i as u32 / w) as f32 / scale) as u32,
                )
            })
            .collect();
        assert!(
            changed.len() > 100,
            "both glyph boxes must draw actual surface pixels"
        );
        if mask == (1.0, 0.0, 1.0) {
            assert!(
                changed
                    .iter()
                    .all(|&(x, y)| (260..540).contains(&x) && (205..275).contains(&y))
            );
            for center in [318, 482] {
                assert!(
                    changed
                        .iter()
                        .any(|&(x, y)| x.abs_diff(center) <= 2 && (265..270).contains(&y)),
                    "actual visible leader must reach its own anchor"
                );
            }
            let glyph_colors = decorated
                .iter()
                .zip(&original)
                .enumerate()
                .filter_map(|(i, (a, b))| {
                    let x = (i as u32 % w) as f32 / scale;
                    let y = (i as u32 / w) as f32 / scale;
                    (a != b && (273.0..362.0).contains(&x) && (230.0..263.0).contains(&y))
                        .then_some(a)
                })
                .collect::<std::collections::HashSet<_>>();
            assert!(
                glyph_colors.len() >= 3,
                "glyph ink and antialiasing must contribute actual surface pixels"
            );
        }
        let p = u64::from(w) * u64::from(h);
        assert_eq!(
            context.target_charges_for_test(),
            (p, p * (20 * u64::from(samples) + 16))
        );
        if samples == 1 && mask == (1.0, 0.0, 1.0) {
            use crate::core::{paragraph::TextStyle, text_engine::TextEngine, text_shaper::TextDirection};
            let rtl = "שלום";
            assert_eq!(TextDirection::detect(rtl), TextDirection::RightToLeft);
            let engine = renderer.text_engine_mut();
            let style = engine.resolve_text_style(&TextStyle { font_size: 13.0, ..Default::default() }).unwrap();
            let (_, missing) = engine.shape_bound_with_keys("\u{10ffff}", 13.0, &style.binding).unwrap();
            assert!(missing.iter().any(|key| key.glyph_id == 0));
            let (rtl_run, rtl_keys) = engine.shape_bound_with_keys(rtl, 13.0, &style.binding).unwrap();
            assert!(rtl_run.clusters.iter().all(|cluster| cluster.direction == TextDirection::RightToLeft));
            let rtl_supported = !rtl_keys.is_empty() && rtl_keys.iter().all(|key| key.glyph_id != 0);
            for (label, allowed) in [("m".repeat(65), false), ("\u{10ffff}".into(), false), (rtl.into(), rtl_supported)] {
                let anchor = PbrSpatialHudAnchor3D { text: Arc::from(label), ..anchors[0].clone() };
                let candidate = context.admit_pbr_spatial_hud(&scene, &[anchor]).unwrap();
                assert!(tree.node_mut(root).unwrap().widget.as_any_mut().downcast_mut::<Viewport3D>()
                    .unwrap().request_pbr_spatial_hud(Some(candidate)).unwrap());
                tree.mark_layout_dirty(root);
                let composed = render(&mut renderer, &mut tree, Occurrence::Current);
                let output = &composed.plan().surface_data[&surface];
                assert_eq!(
                    composed.companion_refusals_for_test().len(),
                    usize::from(!allowed)
                );
                assert_eq!(
                    output.text_realization().outcome(),
                    plain.plan().surface_data[&surface]
                        .text_realization()
                        .outcome()
                );
                assert_eq!(
                    read_surface_pixels(&renderer, w, h, |view| {
                        renderer.encode_sdf_target_for_test(&composed, surface, view)
                    }) != original,
                    allowed
                );
            }
            let (original_atlas, _) = renderer.text_atlas_size_for_test();
            // The HAL360 eviction control also uses a fixed 64px atlas.
            renderer.set_text_atlas_limits_for_test(64, 64);
            let candidate = context
                .admit_pbr_spatial_hud(
                    &scene,
                    &[PbrSpatialHudAnchor3D {
                        text: Arc::from("BCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789"),
                        ..anchors[0].clone()
                    }],
                )
                .unwrap();
            let mut plain_pixels = None;
            for request in [None, Some(candidate)] {
                let optional = request.is_some();
                tree.node_mut(root)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<Viewport3D>()
                    .unwrap()
                    .request_pbr_spatial_hud(request)
                    .unwrap();
                tree.mark_layout_dirty(root);
                let output = render(&mut renderer, &mut tree, Occurrence::SmallAtlas);
                let required = output.plan().text_realization(surface).unwrap().outcome();
                assert!(required.is_complete(), "{required:?}");
                assert!(output.companion_work().next().is_none());
                let pixels = read_compiled_surface_pixels(&renderer, output.plan(), surface, w, h);
                if optional {
                    assert!(matches!(
                        output.companion_refusals_for_test(),
                        [(
                            _,
                            crate::three_d::PbrSourceError3D::Source {
                                cause: "optional HUD glyph working set exceeds atlas capacity",
                                ..
                            }
                        )]
                    ));
                    assert_eq!(plain_pixels.as_ref().unwrap(), &pixels);
                } else {
                    plain_pixels = Some(pixels);
                }
            }
            renderer.set_text_atlas_limits_for_test(
                original_atlas,
                gpu.device.limits().max_texture_dimension_2d,
            );
        }
        if mask == (1.0, 0.0, 1.0) {
            let mut changed_anchors = anchors[..2].to_vec();
            changed_anchors.extend([
                anchor(7, 0, 0.35, 0.0, 5.25, "Behind camera"),
                anchor(8, 0, 0.35, 0.0, 4.95, "Before near"),
                anchor(9, 0, 0.35, 0.0, -25.0, "After far"),
                anchor(10, 0, 30.0, 0.0, 1.0, "Offscreen"),
            ]);
            let changed_hud = context
                .admit_pbr_spatial_hud(&scene, &changed_anchors)
                .unwrap();
            let perspective = ViewDescriptor3::new(
                RigidPose3::new(
                    WorldPoint3::try_new(0.35, 0.0, 5.0).unwrap(),
                    Orientation3::identity(),
                ),
                PerspectiveProjection::try_finite(50.0f32.to_radians(), 0.1, 20.0)
                    .unwrap()
                    .into(),
            );
            let vp = tree
                .node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Viewport3D>()
                .unwrap();
            assert!(
                vp.request_pbr_scene(scene.clone(), Viewport3DViewRevision::new(2), perspective)
                    .unwrap()
            );
            assert!(vp.request_pbr_spatial_hud(Some(changed_hud)).unwrap());
            tree.mark_layout_dirty(root);
            let moved = render(&mut renderer, &mut tree, Occurrence::Current);
            let moved_pixels = read_surface_pixels(&renderer, w, h, |view| {
                renderer.encode_sdf_target_for_test(&moved, surface, view)
            });
            assert_ne!(decorated, moved_pixels);
            let visible = moved
                .companion_work()
                .find_map(|(target, work)| {
                    (target == surface).then(|| work.read_visibility_for_test(&gpu))
                })
                .unwrap();
            assert_eq!(
                visible,
                [1, 1, 0, 0, 0, 0],
                "same immutable source with perspective and all rejection planes"
            );
            let stale = render(&mut renderer, &mut tree, Occurrence::RetiredRegistration);
            assert!(stale.companion_work().next().is_none());
            assert_eq!(stale.companion_refusals_for_test().len(), 1);
            let ordinary = stale.plan().surface_compiled_data(surface).unwrap();
            assert!(ordinary.text_realization().outcome().is_complete());
            assert_eq!(ordinary.frame_batches.total_image_vertices, 4);
        }
        eprintln!(
            "HAL375 actual Mask N{samples} DPR {scale} {mask:?}: flags {expected:?}; {} changed surface pixels; canonical FP16 unchanged",
            changed.len()
        );
    }
}

#[test]
fn hal375_attachment_refusal_keeps_original_and_rejects_foreign_source() {
    use crate::three_d::{PbrSourceError3D, PbrSpatialHudAnchor3D};
    use std::num::NonZeroU64;

    for samples in [1, 4] {
        let bytes = 64 * 64 * (20 * u64::from(samples) + 16);
        let policy = PolicyLimits3D {
            max_single_target_attachment_bytes: bytes,
            ..PolicyLimits3D::conservative()
        };
        let (mut renderer, context, _) = setup(policy).unwrap();
        let foreign = Context3D::with_policy(renderer.gpu_resources(), policy).unwrap();
        let scene = hal374_flat_source(&context, samples, false);
        let other = hal374_scene(&context, samples);
        let anchor = PbrSpatialHudAnchor3D {
            label_id: NonZeroU64::MIN,
            packet_slot: 0,
            world: WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
            text: Arc::from("Exterior"),
        };
        let anchors = std::slice::from_ref(&anchor);
        assert!(foreign.admit_pbr_spatial_hud(&scene, anchors).is_err());
        let wrong = context.admit_pbr_spatial_hud(&other, anchors).unwrap();
        let hud = context.admit_pbr_spatial_hud(&scene, anchors).unwrap();
        let view = hal374_flat_view();
        let mut viewport =
            Viewport3D::new_pbr(context.clone(), scene, Viewport3DViewRevision::new(1), view)
                .unwrap();
        assert!(viewport.request_pbr_spatial_hud(Some(wrong)).is_err());
        viewport.request_pbr_spatial_hud(Some(hud.clone())).unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(viewport), None, 0);
        let surface = SurfaceId::test_id(37501);
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let mut harness = RetainedSceneTestHarness::new();
        harness.frame_with_custom_render_target(
            &mut tree,
            root,
            Constraints::tight(Size::new(64.0, 64.0)),
            &theme,
            renderer.text_engine_mut(),
            &icons,
            CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(64, 64), 1.0).unwrap(),
        );
        renderer.drain_pending_image_registrations();
        let viewport = tree_viewport(&tree, root);
        assert!(matches!(viewport.spatial_hud_error_for_test(surface),
            Some(PbrSourceError3D::Context(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::SingleTargetAttachmentBytes, requested, maximum,
            })) if *requested == bytes + 64 * 64 * 4 * u64::from(samples) && *maximum == bytes));
        assert_eq!(
            viewport.status_for(surface).unwrap().state(),
            Viewport3DLifecycleState::Committed
        );
        assert!(
            hal374_half_pixels(
                viewport.hdr_front_for_test(surface),
                &renderer.gpu_resources()
            )
            .iter()
            .any(|p| p[3] != 0)
        );
        assert_eq!(context.target_charges_for_test(), (64 * 64, bytes));
        assert!(
            harness
                .custom_render_presentation_bindings()
                .prepared_companion_for_test(
                    viewport
                        .custom_render_admission()
                        .unwrap()
                        .presentation_id(),
                    surface,
                )
                .is_none()
        );
        let viewport = tree
            .node_mut(root)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<Viewport3D>()
            .unwrap();
        viewport
            .replace_pbr_scene(other, Viewport3DViewRevision::new(1), view)
            .unwrap();
        assert!(viewport.request_pbr_spatial_hud(Some(hud)).is_err());
    }
}

#[test]
fn hal375_depth_lease_survives_resize_and_device_withdrawal() {
    use crate::three_d::PbrSpatialHudAnchor3D;
    use std::num::NonZeroU64;

    for samples in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let scene = hal374_flat_source(&context, samples, false);
        let hud = context
            .admit_pbr_spatial_hud(
                &scene,
                &[PbrSpatialHudAnchor3D {
                    label_id: NonZeroU64::MIN,
                    packet_slot: 0,
                    world: WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
                    text: Arc::from("Exterior"),
                }],
            )
            .unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            scene,
            Viewport3DViewRevision::new(1),
            hal374_flat_view(),
        )
        .unwrap();
        let id = viewport
            .custom_render_admission()
            .unwrap()
            .presentation_id();
        viewport.request_pbr_spatial_hud(Some(hud)).unwrap();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(viewport), None, 0);
        let surface = SurfaceId::test_id(37502);
        let theme = Theme::default();
        let icons = IconRegistry::new(&[]);
        let mut harness = RetainedSceneTestHarness::new();
        let mut prepare = |renderer: &mut WgpuRenderer,
                           tree: &mut WidgetTree,
                           w: u32,
                           h: u32,
                           scale: f32,
                           active: bool| {
            let size = if active {
                Size::new(w as f32 / scale, h as f32 / scale)
            } else {
                Size::new(0.0, 0.0)
            };
            harness.frame_with_custom_render_target(
                tree,
                root,
                Constraints::tight(size),
                &theme,
                renderer.text_engine_mut(),
                &icons,
                CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(w, h), scale).unwrap(),
            );
            renderer.drain_pending_image_registrations();
            harness
                .custom_render_presentation_bindings()
                .prepared_companion_for_test(id, surface)
                .cloned()
        };
        let old = prepare(&mut renderer, &mut tree, 128, 96, 1.0, true).unwrap();
        assert_eq!(old.depth.extent, (128, 96));
        assert!(old.front.registered().unwrap().is_some());
        let current = prepare(&mut renderer, &mut tree, 1200, 900, 1.5, true).unwrap();
        assert_eq!(current.depth.extent, (1200, 900));
        assert_ne!(current.request, old.request);
        assert!(!Arc::ptr_eq(&old.depth, &current.depth));
        assert!(current.front.registered().unwrap().is_some());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        let p = 1200 * 900;
        let current_bytes = p * (20 * u64::from(samples) + 16);
        let (pixels, resident) = context.target_charges_for_test();
        assert_eq!(pixels, p);
        assert!(
            resident >= current_bytes + 128 * 96 * 4 * u64::from(samples),
            "old depth is still retained and charged"
        );
        drop(old);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (p, current_bytes));
        let program = Arc::downgrade(&current.program);
        drop(current);
        assert!(prepare(&mut renderer, &mut tree, 1200, 900, 1.5, false).is_none());
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(
            program.upgrade().is_none(),
            "inactive target retains no HUD program"
        );
        assert_eq!(context.target_charges_for_test(), (0, 0));
        assert!(prepare(&mut renderer, &mut tree, 1200, 900, 1.5, true).is_some());
        gpu.device.destroy();
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        assert!(prepare(&mut renderer, &mut tree, 1200, 900, 1.5, true).is_none());
        assert_eq!(
            tree_viewport(&tree, root)
                .status_for(surface)
                .unwrap()
                .state(),
            Viewport3DLifecycleState::TerminalDeviceFailure
        );
        assert_eq!(context.target_charges_for_test().0, 0);
    }
}

#[test]
fn hal374_exact_physical_two_pixel_aa_n1_and_n4_sample_stroke() {
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let source = hal374_flat_source(&context, n, false);
        let group = context.admit_pbr_outline_selection(&source, &[0]).unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context,
            source,
            Viewport3DViewRevision::new(1),
            hal374_flat_view(),
        )
        .unwrap();
        viewport.on_mount();
        viewport.request_pbr_outline(Some(group)).unwrap();
        assert!(
            viewport.needs_update(),
            "one queued request must enter ordinary Update"
        );
        let metrics = active_metrics(37403, 64, 64, 1.0);
        let mut timeline = Timeline::new();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
        let (_, _, stroke) = viewport
            .outline_textures_for_test(metrics.surface_id)
            .unwrap();
        let values = hal374_float_samples(stroke, &gpu);
        // Orthographic source edges are EXACTLY x/y16 and48. N1 centres
        // are half-integers: at14.5 d=1.5 -> a=.5; at15.5 d=.5 ->1.
        // All are binary-exact, so this oracle needs no geometric epsilon.
        for y in 24..40 {
            for s in 0..n as usize {
                assert_eq!(values[(y * 64 + 13) * n as usize + s][3], 0.);
                assert_eq!(
                    values[(y * 64 + 16) * n as usize + s][3],
                    0.,
                    "selected interior is untouched"
                );
                if n == 1 {
                    assert_eq!(values[y * 64 + 14][3], 0.5);
                    assert_eq!(values[y * 64 + 15][3], 1.);
                } else {
                    assert!(
                        values[(y * 64 + 14) * 4 + s][3] > 0.
                            && values[(y * 64 + 14) * 4 + s][3] < 1.
                    );
                    assert_eq!(values[(y * 64 + 15) * 4 + s][3], 1.);
                }
            }
        }
        if n == 4 {
            assert_ne!(
                values[(32 * 64 + 14) * 4][3],
                values[(32 * 64 + 14) * 4 + 1][3],
                "true sample-position AA, not one pixel-centre value copied four times"
            );
        }
        assert!(
            !viewport.needs_update(),
            "static Outline must settle, not demand continuous frames"
        );
        assert!(!viewport.needs_animation());
        let dpr2 = active_metrics(37403, 64, 64, 2.0);
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &dpr2, 0.);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &dpr2, 0.);
        let (_, _, stroke2) = viewport.outline_textures_for_test(dpr2.surface_id).unwrap();
        assert!(
            hal374_float_samples(stroke2, &gpu) == values,
            "same physical extent at DPR2 must preserve the exact physical-width/sample AA output"
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

#[test]
fn hal374_membership_refuses_foreign_duplicate_mask_and_source_regression() {
    let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
    let source = hal374_scene_control(&context, 4, true);
    for slots in [&[][..], &[0, 0], &[3], &[u32::MAX]] {
        assert!(context.admit_pbr_outline_selection(&source, slots).is_err());
    }
    let (_foreign_renderer, foreign, _) = setup(PolicyLimits3D::conservative()).unwrap();
    assert!(foreign.admit_pbr_outline_selection(&source, &[0]).is_err());
    let masked = hal374_flat_source(&context, 4, true);
    assert!(
        context.admit_pbr_outline_selection(&masked, &[0]).is_err(),
        "an UNSELECTED masked occluder must refuse the effect"
    );
    let selected = context
        .admit_pbr_outline_selection(&source, &[0, 1])
        .unwrap();
    let other = hal374_flat_source(&context, 4, false);
    let wrong = context.admit_pbr_outline_selection(&other, &[0]).unwrap();
    let mut viewport = Viewport3D::new_pbr(
        context.clone(),
        source.clone(),
        Viewport3DViewRevision::new(1),
        hal374_view(),
    )
    .unwrap();
    viewport.on_mount();
    viewport.request_pbr_outline(Some(selected)).unwrap();
    assert!(viewport.request_pbr_outline(Some(wrong)).is_err());
    let metrics = active_metrics(37404, 64, 64, 1.);
    let mut timeline = Timeline::new();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport
            .outline_textures_for_test(metrics.surface_id)
            .is_some()
    );
    viewport
        .replace_pbr_scene(other, Viewport3DViewRevision::new(1), hal374_view())
        .unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport
            .outline_textures_for_test(metrics.surface_id)
            .is_none()
    );
    viewport
        .replace_pbr_scene(
            source.clone(),
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport
            .outline_textures_for_test(metrics.surface_id)
            .is_none(),
        "A->B->A cannot resurrect retired selection"
    );
    // Settle the preceding source's actual submitted-reader release, then
    // drive the normal image-owner retry. No wait enters production code.
    renderer
        .gpu_resources()
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    renderer.drain_pending_image_registrations();
    renderer
        .gpu_resources()
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport.pbr_failure(metrics.surface_id).is_none(),
        "A retry: {:?}",
        viewport.pbr_failure(metrics.surface_id)
    );
    assert!(Arc::ptr_eq(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .committed_material()
            .unwrap()
            .source(),
        &source.0
    ));
    let before = hal374_half_pixels(
        viewport.hdr_front_for_test(metrics.surface_id),
        &renderer.gpu_resources(),
    );
    let retone = context
        .admit_pbr_scene(crate::three_d::PbrSceneDescriptor3D {
            snapshot: source.snapshot(),
            model: source.model(),
            hdr: crate::three_d::PbrHdrSource3D {
                revision: 2,
                exposure_ev: 1.,
                ..source.hdr()
            },
            samples: SampleCount3D::FOUR,
            direct: None,
            indirect: None,
        })
        .unwrap();
    viewport
        .request_pbr_scene(
            retone.clone(),
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    renderer
        .gpu_resources()
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    renderer.drain_pending_image_registrations();
    renderer
        .gpu_resources()
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport.pbr_failure(metrics.surface_id).is_none(),
        "re-tone: {:?}",
        viewport.pbr_failure(metrics.surface_id)
    );
    assert!(Arc::ptr_eq(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .committed_material()
            .unwrap()
            .source(),
        &retone.0
    ));
    let after = hal374_half_pixels(
        viewport.hdr_front_for_test(metrics.surface_id),
        &renderer.gpu_resources(),
    );
    assert!(
        after != before,
        "real exposure-only re-tone must change actual covered source pixels"
    );
    let selected = context.admit_pbr_outline_selection(&retone, &[1]).unwrap();
    viewport.request_pbr_outline(Some(selected)).unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport
            .outline_textures_for_test(metrics.surface_id)
            .is_some(),
        "new actual source generation needs its own selected association"
    );
    viewport.on_unmount();
}

#[test]
fn hal374_combined_original_private_upload_refuses_effect_only_and_safe_retry() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_upload_bytes_per_frame = 10_000;
    let (mut renderer, context, _) = setup(policy).unwrap();
    let source = hal374_scene_control(&context, 4, true);
    let group = context.admit_pbr_outline_selection(&source, &[0]).unwrap();
    let mut viewport = Viewport3D::new_pbr(
        context,
        source,
        Viewport3DViewRevision::new(1),
        hal374_view(),
    )
    .unwrap();
    viewport.on_mount();
    viewport.request_pbr_outline(Some(group.clone())).unwrap();
    let metrics = active_metrics(37405, 64, 64, 1.);
    let mut timeline = Timeline::new();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(viewport.pbr_failure(metrics.surface_id).is_none());
    assert!(matches!(
        viewport.pbr_outline_error(metrics.surface_id),
        Some(crate::three_d::PbrSourceError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::UploadBytesPerFrame,
                ..
            }
        ))
    ));
    assert!(
        viewport
            .outline_textures_for_test(metrics.surface_id)
            .is_none()
    );
    assert!(
        viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .committed_material()
            .is_some(),
        "valid original PBR is not refused with Outline"
    );
    // Source geometry/material uploads are now resident; an explicit same-group
    // retry must use their actual warm receipt rather than re-debit cold bytes.
    viewport.request_pbr_outline(Some(group)).unwrap();
    hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
    assert!(
        viewport.pbr_outline_error(metrics.surface_id).is_none(),
        "{:?}",
        viewport.pbr_outline_error(metrics.surface_id)
    );
    assert!(
        viewport
            .outline_textures_for_test(metrics.surface_id)
            .is_some()
    );
    viewport.on_unmount();
}

#[test]
fn hal374_union_hidden_orbit_resize_inactive_and_device_lifetimes_n1_n4() {
    use crate::three_d::*;
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let source = hal374_scene_control(&context, n, true);
        let group = context
            .admit_pbr_outline_selection(&source, &[0, 1])
            .unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
        viewport.on_mount();
        viewport.request_pbr_outline(Some(group)).unwrap();
        let small = active_metrics(37406, 96, 80, 1.);
        let mut timeline = Timeline::new();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &small, 0.);
        let (_, tags, stroke) = viewport
            .outline_textures_for_test(small.surface_id)
            .unwrap();
        for (t, s) in hal374_float_samples(tags, &gpu)
            .iter()
            .zip(hal374_float_samples(stroke, &gpu))
        {
            if t[0] == 1. || t[0] == 2. {
                assert_eq!(s[3], 0., "union interiors and seams must stay unchanged");
            }
        }
        let orbit = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.6, 0., 5.4).unwrap(),
                Orientation3::try_from_xyzw(0., 0.055f32.sin(), 0., 0.055f32.cos()).unwrap(),
            ),
            PerspectiveProjection::try_finite(42.0f32.to_radians(), 0.1, 20.)
                .unwrap()
                .into(),
        );
        viewport
            .request_pbr_scene(source.clone(), Viewport3DViewRevision::new(2), orbit)
            .unwrap();
        let large = active_metrics(37406, 800, 600, 1.);
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &large, 0.);
        assert!(
            viewport.pbr_outline_error(large.surface_id).is_none(),
            "{:?}",
            viewport.pbr_outline_error(large.surface_id)
        );
        let (output, _, _) = viewport
            .outline_textures_for_test(large.surface_id)
            .unwrap();
        assert_eq!((output.width(), output.height()), (800, 600));
        assert!(!viewport.needs_update());
        viewport.prepare_custom_render(&inactive_metrics(37406, 800, 600));
        assert!(
            viewport
                .outline_textures_for_test(large.surface_id)
                .is_none()
        );
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &large, 0.);
        assert!(
            viewport
                .outline_textures_for_test(large.surface_id)
                .is_some()
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(
            viewport
                .outline_textures_for_test(large.surface_id)
                .is_none()
        );
        // Same real original meshes; selected source is wholly behind a wider
        // independently placed foreground. No selected winner may leak a halo.
        let q = Orientation3::try_from_xyzw(0.19f32.sin(), 0., 0., 0.19f32.cos())
            .unwrap()
            .compose_local(
                Orientation3::try_from_xyzw(0., (-0.27f32).sin(), 0., (-0.27f32).cos()).unwrap(),
            )
            .unwrap();
        let place = |x: f64, z: f64, s: [f32; 3]| {
            ModelPlacement3::try_new(
                WorldPoint3::try_new(x, 0., z).unwrap(),
                q,
                PositiveScale3::try_new(s[0], s[1], s[2]).unwrap(),
            )
            .unwrap()
        };
        let model = context
            .admit_pbr_model(
                &source.snapshot(),
                ModelRevision::new(2),
                vec![
                    place(-0.3, 0., [0.2; 3]),
                    place(0.3, 0., [0.2; 3]),
                    place(0., 2., [3., 3., 0.2]),
                ],
            )
            .unwrap();
        let hidden = context
            .admit_pbr_scene(PbrSceneDescriptor3D {
                snapshot: source.snapshot(),
                model,
                hdr: PbrHdrSource3D {
                    identity: 374430,
                    revision: 1,
                    ..source.hdr()
                },
                samples: SampleCount3D::try_new(n).unwrap(),
                direct: None,
                indirect: None,
            })
            .unwrap();
        let group = context
            .admit_pbr_outline_selection(&hidden, &[0, 1])
            .unwrap();
        let mut hidden_view = Viewport3D::new_pbr(
            context.clone(),
            hidden,
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
        hidden_view.on_mount();
        hidden_view.request_pbr_outline(Some(group)).unwrap();
        hal381_resolve_front(&mut hidden_view, &mut renderer, &mut timeline, &small, 0.);
        let (output, tags, stroke) = hidden_view
            .outline_textures_for_test(small.surface_id)
            .unwrap();
        assert!(
            hal374_float_samples(tags, &gpu)
                .iter()
                .all(|v| v[0] != 1. && v[0] != 2.)
        );
        assert!(
            hal374_float_samples(stroke, &gpu)
                .iter()
                .all(|v| v[3] == 0.)
        );
        assert_eq!(
            hal374_half_pixels(output, &gpu),
            hal374_half_pixels(hidden_view.hdr_front_for_test(small.surface_id), &gpu)
        );
        let snapshot = source.snapshot();
        let MaterialPacketSource3D::PbrMaterial { geometry, .. } = &snapshot.0.source_packets()[0]
        else {
            unreachable!()
        };
        context.retire_pbr_geometry(geometry).unwrap();
        assert!(
            context.admit_pbr_outline_selection(&source, &[0]).is_err(),
            "retired source generation cannot admit a new selected association"
        );
        gpu.device.destroy();
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        hidden_view.prepare_custom_render(&small);
        assert!(
            hidden_view
                .outline_textures_for_test(small.surface_id)
                .is_none()
        );
        assert_eq!(
            hidden_view.status_for(small.surface_id).unwrap().state(),
            Viewport3DLifecycleState::TerminalDeviceFailure
        );
        assert!(
            context.admit_pbr_outline_selection(&source, &[0]).is_err(),
            "failed Device cannot admit selection"
        );
        hidden_view.on_unmount();
    }
}

#[test]
fn hal374_same_cap_preflight_denies_only_outline_and_charges_real_overlap() {
    for aggregate in [false, true] {
        let mut policy = PolicyLimits3D::conservative();
        // 64² N4 canonical=393216; private=294912. Each alone fits,
        // combined doesn't. No target-size downgrade or hidden separate cap.
        if aggregate {
            policy.max_total_attachment_bytes = 600_000;
        } else {
            policy.max_single_target_attachment_bytes = 600_000;
        }
        let (mut renderer, context, _) = setup(policy).unwrap();
        let gpu = renderer.gpu_resources();
        let source = hal374_scene_control(&context, 4, true);
        let selection = context.admit_pbr_outline_selection(&source, &[0]).unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source,
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
        viewport.on_mount();
        viewport.request_pbr_outline(Some(selection)).unwrap();
        let metrics = active_metrics(37407, 64, 64, 1.);
        let mut timeline = Timeline::new();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
        assert!(
            viewport.pbr_failure(metrics.surface_id).is_none(),
            "aggregate={aggregate}: {:?}",
            viewport.pbr_failure(metrics.surface_id)
        );
        assert!(
            matches!(viewport.pbr_outline_error(metrics.surface_id),Some(crate::three_d::PbrSourceError3D::Context(
            Context3DError::PolicyBudgetExceeded{budget,..})) if *budget==if aggregate {PolicyBudget3D::TotalAttachmentBytes}else{PolicyBudget3D::SingleTargetAttachmentBytes})
        );
        assert_eq!(context.target_charges_for_test(), (4096, 393216));
        viewport.on_unmount();
        drop(viewport);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn hal374_actual_same_sample_canonical_winner_occlusion_max_and_copy_n1_n4() {
    for n in [1, 4] {
        let (mut renderer, context, _) =
            setup(PolicyLimits3D::conservative()).expect("actual Metal renderer");
        let gpu = renderer.gpu_resources();
        let source = hal374_scene_control(&context, n, true);
        let group = context.admit_pbr_outline_selection(&source, &[0]).unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source,
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
        viewport.on_mount();
        viewport.request_pbr_outline(Some(group)).unwrap();
        let metrics = active_metrics(37402, 128, 112, 1.0);
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        assert!(
            viewport.pbr_outline_error(metrics.surface_id).is_none(),
            "{:?}",
            viewport.pbr_outline_error(metrics.surface_id)
        );
        let (output, winners, stroke) = viewport
            .outline_textures_for_test(metrics.surface_id)
            .expect("registered F and actually queued composition");
        let tags = hal374_float_samples(winners, &gpu);
        let strokes = hal374_float_samples(stroke, &gpu);
        let planes = viewport.hdr_scene_planes_for_test(metrics.surface_id);
        let scene: Vec<_> = planes
            .iter()
            .map(|p| hal374_float_samples(p, &gpu))
            .collect();
        let original = hal374_half_pixels(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        let composed = hal374_half_pixels(output, &gpu);
        let mut populations = [0usize; 4];
        let mut feather = 0;
        let mut opaque = 0;
        let mut partial_pixels = 0;
        for (sample, tag) in tags.iter().enumerate() {
            let id = tag[0] as usize;
            assert!(id <= 3);
            assert_eq!(tag[0], id as f32);
            populations[id] += 1;
            let expected: [f32; 3] = match id {
                0 => [0.0, 0.0, 0.0],
                1 => [0.2, 0.0, 0.0],
                2 => [0.0, 0.4, 0.0],
                3 => [0.0, 0.0, 0.6],
                _ => unreachable!(),
            };
            // These are ACTUAL canonical MRT4 values from the winning source
            // fragment, not a second-pass observation overwritten by losers.
            for c in 0..3 {
                assert_eq!(
                    scene[c][sample][0].to_bits(),
                    expected[c].to_bits(),
                    "N{n} canonical source winner sample{sample} tag{id}"
                );
            }
            assert_eq!(scene[3][sample][0], f32::from(id != 0));
            let a = strokes[sample][3];
            assert!(a.is_finite() && (0.0..=1.0).contains(&a));
            if id == 1 || id == 3 {
                assert_eq!(
                    a, 0.0,
                    "selected destination or proved nearer foreground must suppress stroke"
                );
            }
            if a > 0.0 && a < 1.0 {
                feather += 1;
            }
            if a == 1.0 {
                opaque += 1;
            }
            // Finite fixture envelope: one half-storage error in alpha and one
            // in RGB, each<=2^-11 on [0,1], give <=2^-10 premult comparison.
            for c in 0..3 {
                assert!((strokes[sample][c] - [0.24, 0.62, 0.84][c] * a).abs() <= 2.0f32.powi(-10));
            }
        }
        assert!(
            populations.iter().all(|&n| n > 0),
            "all original members and uncovered samples must exist: {populations:?}"
        );
        assert!(
            feather > 0 && opaque > 0,
            "real analytic AA must contain fractional AND opaque stroke: {feather}/{opaque}"
        );
        for p in 0..original.len() {
            let s = &strokes[p * n as usize..(p + 1) * n as usize];
            let covered = tags[p * n as usize..(p + 1) * n as usize]
                .iter()
                .filter(|v| v[0] != 0.0)
                .count();
            assert_eq!(
                hal374_half(original[p][3]),
                covered as f32 / n as f32,
                "N{n} exact source alpha k/N"
            );
            if covered > 0 && covered < n as usize {
                partial_pixels += 1;
            }
            if s.iter().all(|v| v[3] == 0.0) {
                assert_eq!(
                    composed[p], original[p],
                    "outside stroke must copy original half bits"
                );
            }
        }
        if n == 4 {
            assert!(partial_pixels > 0);
        }
        eprintln!(
            "HAL374 N{n}: actual canonical/replay winners={populations:?}; opaque/AA stroke={opaque}/{feather}; mixed source pixels={partial_pixels}"
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

#[test]
fn hal374_original_multi_pbr_selection_ab_none_n1_n4() {
    for samples in [1, 4] {
        let (mut renderer, context, _) =
            setup(PolicyLimits3D::conservative()).expect("actual Metal renderer");
        let gpu = renderer.gpu_resources();
        let scene = hal374_scene(&context, samples);
        let selected_a = context.admit_pbr_outline_selection(&scene, &[0]).unwrap();
        let selected_b = context.admit_pbr_outline_selection(&scene, &[1]).unwrap();
        assert_eq!(selected_a.packet_slots(), &[0]);
        assert_eq!(selected_b.packet_slots(), &[1]);
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            scene.clone(),
            Viewport3DViewRevision::new(1),
            hal374_view(),
        )
        .unwrap();
        viewport.on_mount();
        let metrics = active_metrics(37401, 96, 80, 1.0);
        let mut timeline = Timeline::new();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
        let original = hal374_half_pixels(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert!(
            original.iter().any(|p| p[3] != 0),
            "original 3D source must really cover samples"
        );
        assert!(
            viewport
                .outline_textures_for_test(metrics.surface_id)
                .is_none()
        );
        let mut results = Vec::new();
        for selected in [selected_a.clone(), selected_b.clone()] {
            viewport.request_pbr_outline(Some(selected)).unwrap();
            hal372_update(&mut viewport, &mut timeline, 0.0);
            hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
            viewport.prepare_custom_render(&metrics);
            renderer.drain_pending_image_registrations();
            assert!(
                viewport.pbr_failure(metrics.surface_id).is_none(),
                "{:?}",
                viewport.pbr_failure(metrics.surface_id)
            );
            assert!(
                viewport.pbr_outline_error(metrics.surface_id).is_none(),
                "{:?}",
                viewport.pbr_outline_error(metrics.surface_id)
            );
            let (output, _, _) = viewport
                .outline_textures_for_test(metrics.surface_id)
                .expect("real private source frame");
            let pixels = hal374_half_pixels(output, &gpu);
            assert_eq!(
                hal374_half_pixels(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
                original,
                "Outline must not change canonical F"
            );
            let changed = pixels.iter().zip(&original).filter(|(a, b)| a != b).count();
            assert!(
                changed > 0,
                "N{samples} must actually draw the selected 3D Outline"
            );
            eprintln!(
                "HAL374 actual original multi-PBR N{samples}: changed={changed}, canonical FP16 unchanged"
            );
            results.push(pixels);
        }
        assert_ne!(
            results[0], results[1],
            "actual occurrence A->B must move the outline"
        );
        let front = viewport.target_texture_id_for_test(metrics.surface_id);
        let serial = viewport
            .status_for(metrics.surface_id)
            .unwrap()
            .committed_material()
            .unwrap()
            .request_serial();
        viewport.request_pbr_outline(None).unwrap();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        assert!(
            viewport
                .outline_textures_for_test(metrics.surface_id)
                .is_none()
        );
        assert_eq!(
            viewport.target_texture_id_for_test(metrics.surface_id),
            front
        );
        assert_eq!(
            viewport
                .status_for(metrics.surface_id)
                .unwrap()
                .committed_material()
                .unwrap()
                .request_serial(),
            serial
        );
        assert_eq!(
            hal374_half_pixels(viewport.hdr_front_for_test(metrics.surface_id), &gpu),
            original
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

#[test]
fn hal374_exclusive_output_one_rounded_clip_and_real_compiled_retirement_n1_n4() {
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let source = hal374_flat_source(&context, n, false);
        let selection = context.admit_pbr_outline_selection(&source, &[0]).unwrap();
        let angle = std::f32::consts::FRAC_PI_8;
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0., 0., 5.).unwrap(),
                Orientation3::try_from_xyzw(0., 0., angle.sin(), angle.cos()).unwrap(),
            ),
            crate::three_d::OrthographicProjection::try_new(1.75, 1., 9.)
                .unwrap()
                .into(),
        );
        let rect = RectF::new(0., 0., 64., 64.);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (64, 64), 1.)
            .unwrap();
        let metrics = active_metrics_for_surface(surface, 64, 64, 1.);
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source,
            Viewport3DViewRevision::new(1),
            view,
        )
        .unwrap();
        viewport.on_mount();
        viewport.request_pbr_outline(Some(selection)).unwrap();
        let mut timeline = Timeline::new();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.);
        let (output, _, _) = viewport.outline_textures_for_test(surface).unwrap();
        let c = hal374_half_pixels(output, &gpu);
        let composed =
            hal381_composed_pixels(&mut renderer, &mut viewport, &metrics, &compositor, false);
        let at = 5 * 64 + 5;
        assert!(
            hal374_half(c[at][3]) > 0.,
            "real stroke must meet the fractional rounded clip"
        );
        let h = ((2.0f32 * 14.5 * 14.5).sqrt() - 20.).clamp(0., 1.);
        let clip = 1. - h * h * (3. - 2. * h);
        assert!(clip > 0. && clip < 1.);
        let encode = |v: f32| {
            if gpu.format.is_srgb() {
                if v <= 0.0031308 {
                    12.92 * v
                } else {
                    1.055 * v.powf(1. / 2.4) - 0.055
                }
            } else {
                v
            }
        };
        let alpha = hal374_half(c[at][3]);
        for channel in 0..3 {
            let rgb = hal374_half(c[at][channel]);
            let expected = clip * rgb + (1. - clip * alpha) * [0.40, 0.05, 0.20][channel];
            let byte = (encode(expected).clamp(0., 1.) * 255.).round() as u8;
            // Same scoped two-code-value UI envelope as the HAL381 endpoint:
            // measured half input plus native8 storage/blending conversion.
            assert!(
                composed[at][channel].abs_diff(byte) <= 2,
                "N{n} channel{channel}: actual{:?} expected{byte}; clip applied once",
                composed[at]
            );
        }
        assert_eq!(composed[at][3], 255);
        let retained =
            hal381_compile_presence(&mut renderer, &mut viewport, &metrics, &compositor, false);
        let original = hal374_half_pixels(viewport.hdr_front_for_test(surface), &gpu);
        viewport.request_pbr_outline(None).unwrap();
        hal372_update(&mut viewport, &mut timeline, 0.);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        assert!(viewport.outline_textures_for_test(surface).is_none());
        assert_eq!(
            hal374_half_pixels(viewport.hdr_front_for_test(surface), &gpu),
            original
        );
        viewport.on_unmount();
        drop(viewport);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert!(
            context.target_charges_for_test().1 >= 64 * 64 * (16 * n as u64 + 8),
            "compiled old private output still owns its actual allocations"
        );
        drop(retained);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

fn hal381_proxy_fixture(
    context: &Context3D,
) -> (
    crate::three_d::LoadingPresenceProxy3D,
    crate::three_d::LoadingPresenceAssociation3D,
) {
    use crate::three_d::*;
    use sha2::Digest;
    // Same original triangles as consumer_pbr_geometry below, before its mapped
    // shading conversion. This exact CPU derivative is HAL-owned test provenance.
    let positions = [
        [-1.0, -1.0, 0.0],
        [1.0, -1.0, 0.0],
        [-1.0, 1.0, 0.0],
        [1.0, -1.0, 0.0],
        [1.0, 1.0, 0.0],
        [-1.0, 1.0, 0.0],
    ];
    let source = Arc::new(
        GeometrySourceDescriptor::try_new(
            positions
                .iter()
                .map(|p| LocalPoint3::try_new(p[0], p[1], p[2]).unwrap())
                .collect(),
            None,
            None,
            None,
            GeometryIndices::Unindexed,
        )
        .unwrap(),
    );
    let mut digest = sha2::Sha256::new();
    digest.update(97000u64.to_le_bytes());
    digest.update(1u64.to_le_bytes());
    for p in positions {
        for c in p {
            digest.update(c.to_bits().to_le_bytes());
        }
    }
    let original: [u8; 32] = digest.finalize().into();
    let placement = ModelPlacement3::try_new(
        WorldPoint3::try_new(0.0, 0.0, 0.0).unwrap(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap();
    let claim = LoadingPresenceSource3D {
        original_binding: original,
        source_revision: std::num::NonZeroU64::new(1).unwrap(),
        source_generation: std::num::NonZeroU64::new(1).unwrap(),
        original_root: "HAL381 owned R024 fixture97000".into(),
        original_resource: "original vertices and triangles of consumer_pbr_geometry".into(),
        proxy_hash_algorithm: "SHA256 fixture source identity + LE position bits".into(),
        proxy_byte_hash: Arc::from(original),
        qualification_receipt:
            "exact identity derivative of the HAL-owned original triangle array; not HAC".into(),
        model_frame_origin: placement.translation(),
        source_to_proxy: placement,
    };
    let prepared = std::thread::spawn(move || {
        LoadingPresenceProxyPrepared3D::prepare(
            source,
            claim,
            placement,
            ([-1.0, -1.0, 0.0], [1.0, 1.0, 0.0]),
            1 << 20,
            Arc::new(AtomicBool::new(false)),
        )
    })
    .join()
    .unwrap()
    .unwrap();
    let association = prepared.association();
    (
        context.admit_loading_presence_proxy(&prepared).unwrap(),
        association,
    )
}

#[test]
fn hal381_ready_full_before_first_preview_publishes_only_canonical_n1_n4() {
    use crate::three_d::PresentationMotionPolicy;
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let view = hal374_view();
        let (proxy, association) = hal381_proxy_fixture(&context);
        let revision = Viewport3DViewRevision::new(1);
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            revision,
            view,
            SampleCount3D::try_new(n).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: true,
            },
        )
        .unwrap();
        viewport.on_mount();
        let mut time = Timeline::new();
        hal372_update(&mut viewport, &mut time, 0.0);
        let request = viewport.loading_presence_request(&association).unwrap();
        let full = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), n, false, 1);
        viewport
            .request_loading_presence_pbr(full.clone(), &request, revision, view)
            .unwrap();
        let metrics = active_metrics(38100, 64, 64, 1.0);
        let surface = metrics.surface_id;
        hal372_update(&mut viewport, &mut time, 0.030);
        let outcome = viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let status = viewport.status_for(surface).unwrap();
        assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
        let committed = status.committed_material().unwrap();
        assert!(Arc::ptr_eq(committed.source(), &full.0));
        assert_eq!(committed.view_revision(), revision);
        assert_eq!(
            committed.request_serial(),
            status.requested_material().unwrap().request_serial()
        );
        let binding = outcome.presentation_binding().unwrap();
        assert!(!binding.canonical_over_neutral());
        assert_eq!(
            Some(binding.texture_id()),
            viewport.target_texture_id_for_test(surface)
        );
        let pixel = renderer.render_registered_image_pixel_for_test(
            binding.texture_id(),
            [0.5, 0.5],
            [1.0; 4],
            wgpu::Color::TRANSPARENT,
        );
        assert_eq!(pixel[3], 255, "actual registered PBR source at 30ms");
        assert!(
            viewport
                .loading_presence_texture_for_test(surface)
                .is_none()
        );
        assert!(
            viewport
                .loading_presence_target_progress_for_test(surface)
                .is_none()
        );
        assert!(viewport.loading_presence_error().is_none());
        hal372_update(&mut viewport, &mut time, 0.030);
        assert!(
            !viewport.needs_update(),
            "no compulsory preview dwell or Timeline"
        );
    }
}

#[test]
fn hal381_actual_early_proxy_then_registered_full_front_n1_n4() {
    use crate::three_d::{OrthographicProjection, PresentationMotionPolicy};
    for samples in [1, 4] {
        let (mut renderer, context, _) =
            setup(PolicyLimits3D::conservative()).expect("actual Metal renderer");
        let gpu = renderer.gpu_resources();
        assert!(gpu.device.features().is_empty());
        let (proxy, association) = hal381_proxy_fixture(&context);
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(3.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::try_new(samples).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: true,
            },
        )
        .unwrap();
        viewport.on_mount();
        let request = viewport.loading_presence_request(&association).unwrap();
        let metrics = active_metrics(38101, 64, 64, 1.0);
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        assert!(
            viewport.loading_presence_error().is_none(),
            "{:?}",
            viewport.loading_presence_error()
        );
        assert!(
            viewport
                .status_for(metrics.surface_id)
                .unwrap()
                .committed_material()
                .is_none(),
            "ghost is not PBR Committed"
        );
        let g = read_r027_fp16(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .unwrap(),
            &gpu,
        );
        assert_eq!(g[3], 1.0);
        for c in 0..3 {
            // The centered front-facing surface has e=0 and no contour.
            // Its scalar noise may only vary the bounded interior term.
            let b = [0.025, 0.031, 0.040][c];
            let base = [0.10, 0.22, 0.29][c];
            let low = 0.0825 * 0.94 * base + (1.0 - 0.0825) * b;
            let high = 0.1175 * 1.06 * base + (1.0 - 0.1175) * b;
            assert!(
                (low - 0.0001..=high + 0.0001).contains(&g[c]),
                "N{samples}: {g:?}"
            );
        }
        // Unknown wait, including a complete period, has no fabricated progress.
        for time in [1.6, 3.2, 32.0] {
            hal372_update(&mut viewport, &mut timeline, time);
            viewport.prepare_custom_render(&metrics);
            renderer.drain_pending_image_registrations();
            assert_eq!(viewport.loading_presence_weight_for_test(), Some(0.0));
            assert!(
                viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .committed_material()
                    .is_none()
            );
        }
        let geometry = consumer_pbr_geometry(&context);
        let full = consumer_pbr_source(&context, &geometry, samples, false, 1);
        viewport
            .request_loading_presence_pbr(
                full.clone(),
                &request,
                Viewport3DViewRevision::new(1),
                view,
            )
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        assert_eq!(
            viewport.loading_presence_weight_for_test(),
            Some(0.0),
            "CPU/source commit is not registration"
        );
        assert!(
            viewport
                .status_for(metrics.surface_id)
                .unwrap()
                .committed_material()
                .is_some(),
            "real source refusal: {:?}",
            viewport.pbr_failure(metrics.surface_id)
        );
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut timeline, 32.0);
        assert_eq!(
            viewport.loading_presence_weight_for_test(),
            Some(0.0),
            "new immutable-view registration is pending"
        );
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut timeline, 32.0);
        assert!(
            viewport.loading_presence_error().is_none(),
            "{:?}",
            viewport.loading_presence_error()
        );
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let ready_g = read_r027_fp16(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .unwrap(),
            &gpu,
        );
        let f = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        hal372_update(&mut viewport, &mut timeline, 32.080);
        assert!(
            (viewport.loading_presence_weight_for_test().unwrap() - 0.5).abs() < 0.0001,
            "weight={:?}, PBR={:?}, presence={:?}",
            viewport.loading_presence_weight_for_test(),
            viewport.pbr_failure(metrics.surface_id),
            viewport.loading_presence_error()
        );
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let t = read_r027_fp16(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .unwrap(),
            &gpu,
        );
        assert_eq!(t[3], 1.0);
        assert_ne!(t, g);
        for c in 0..3 {
            let h = f[c] + (1.0 - f[3]) * [0.025, 0.031, 0.040][c];
            assert!((t[c] - (ready_g[c] + h) * 0.5).abs() <= 2.0_f32.powi(-11));
        }
        hal372_update(&mut viewport, &mut timeline, 32.1601);
        assert!(viewport.loading_presence_terminal_for_test(metrics.surface_id));
        assert_eq!(viewport.loading_presence_weight_for_test(), Some(1.0));
        assert!(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .is_none(),
            "temporary targets retire at terminal"
        );
        assert!(
            viewport
                .status_for(metrics.surface_id)
                .unwrap()
                .committed_material()
                .is_some()
        );
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

fn hal381_compile_presence(
    renderer: &mut WgpuRenderer,
    viewport: &mut Viewport3D,
    metrics: &CustomRenderTargetMetrics,
    compositor: &Compositor,
    terminal: bool,
) -> crate::renderer::compiled_draw_plan::MultiTargetDrawPlan {
    let outcome = viewport.prepare_custom_render(metrics);
    renderer.drain_pending_image_registrations();
    let admission = viewport.custom_render_admission().unwrap();
    let mut bindings = CustomRenderPresentationBindings::default();
    bindings
        .publish_for_target(
            metrics.surface_id,
            admission.presentation_id(),
            outcome.presentation_binding().unwrap(),
        )
        .unwrap();
    let mut image = SceneBuilder::new();
    image.push_target_bound_image(
        metrics.final_logical_rect,
        admission.presentation_id(),
        RectF::new(0.0, 0.0, 1.0, 1.0),
        crate::core::scene::Color::WHITE,
        LAYER_CONTENT,
        None,
    );
    let mut background = SceneBuilder::new();
    background.push_rect(
        metrics.final_logical_rect,
        crate::core::scene::Color::from_linear(0.40, 0.05, 0.20, 1.0),
        0,
        None,
    );
    let mut scene = background.build();
    scene
        .root
        .push(SceneNode::Clip(crate::core::scene::ClipNode {
            rect: metrics.final_logical_rect,
            rounded: Some(crate::core::scene::RoundedCorners::uniform(20.0)),
            children: image.build().root,
            z_index: LAYER_CONTENT,
            key: None,
        }));
    let scenes = std::collections::HashMap::from([(metrics.surface_id, scene)]);
    let plan = DrawPlanBuilder::new().build_multi_with_presentation_bindings(
        &scenes,
        compositor.snapshot_multi(),
        &mut MeshCache::new(),
        &mut bindings.begin_materialization_for_test(0),
        |_| 1.0,
        0,
    ).expect("scene must satisfy DrawPlan Build admission");
    let image_batches = plan
        .surface_scene(metrics.surface_id)
        .unwrap()
        .batches
        .iter()
        .filter(|b| {
            matches!(
                b.key.pipeline,
                PipelineId::Image | PipelineId::CanonicalOverNeutral
            )
        })
        .collect::<Vec<_>>();
    assert_eq!(
        image_batches.len(),
        1,
        "exactly one visible viewport primitive"
    );
    assert_eq!(
        image_batches[0].key.pipeline,
        if terminal {
            PipelineId::CanonicalOverNeutral
        } else {
            PipelineId::Image
        }
    );
    renderer.advance_frame();
    let compiled = renderer
        .compile_multi(&plan)
        .unwrap()
        .expect_ready("native Compile must be ready");
    assert_eq!(
        compiled.presence.get(&metrics.surface_id).unwrap().len(),
        1,
        "actual generation is retained by Compile"
    );
    compiled
}

fn hal381_composed_pixels(
    renderer: &mut WgpuRenderer,
    viewport: &mut Viewport3D,
    metrics: &CustomRenderTargetMetrics,
    compositor: &Compositor,
    terminal: bool,
) -> Vec<[u8; 4]> {
    let compiled = hal381_compile_presence(renderer, viewport, metrics, compositor, terminal);
    read_compiled_surface_pixels(
        renderer,
        &compiled,
        metrics.surface_id,
        metrics.target_physical_extent.width,
        metrics.target_physical_extent.height,
    )
}

pub(super) fn read_compiled_surface_pixels(
    renderer: &WgpuRenderer,
    compiled: &crate::renderer::compiled_draw_plan::MultiTargetDrawPlan,
    surface: SurfaceId,
    w: u32,
    h: u32,
) -> Vec<[u8; 4]> {
    read_surface_pixels(renderer, w, h, |view| {
        renderer.encode_compiled_target_for_test(compiled, surface, view)
    })
}

fn read_surface_pixels(
    renderer: &WgpuRenderer,
    w: u32,
    h: u32,
    encode: impl FnOnce(&wgpu::TextureView) -> Option<crate::renderer::frosted_glass::EncodedTargetCommands>,
) -> Vec<[u8; 4]> {
    let gpu = renderer.gpu_resources();
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("HAL381 production UI endpoint readback"),
        size: wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
        view_formats: &[],
    });
    let render = encode(&output.create_view(&Default::default())).unwrap();
    let pitch = (w * 4).div_ceil(256) * 256;
    let buffer = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("HAL381 rounded-edge output bytes"),
        size: u64::from(pitch * h),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
        mapped_at_creation: false,
    });
    let mut copy = gpu.device.create_command_encoder(&Default::default());
    copy.copy_texture_to_buffer(
        wgpu::TexelCopyTextureInfo {
            texture: &output,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyBufferInfo {
            buffer: &buffer,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(pitch),
                rows_per_image: Some(h),
            },
        },
        wgpu::Extent3d {
            width: w,
            height: h,
            depth_or_array_layers: 1,
        },
    );
    renderer.submit_compiled_target_for_test(render);
    gpu.queue.submit([copy.finish()]);
    let (send, receive) = std::sync::mpsc::channel();
    buffer
        .slice(..)
        .map_async(wgpu::MapMode::Read, move |r| send.send(r).unwrap());
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    receive.recv().unwrap().unwrap();
    let bytes = buffer.slice(..).get_mapped_range().expect("mapped readback range");
    let mut pixels = Vec::new();
    for y in 0..h {
        for x in 0..w {
            let i = (y * pitch + x * 4) as usize;
            let mut p: [u8; 4] = bytes[i..i + 4].try_into().unwrap();
            if matches!(
                gpu.format,
                wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
            ) {
                p.swap(0, 2);
            }
            pixels.push(p);
        }
    }
    drop(bytes);
    buffer.unmap();
    pixels
}

#[test]
fn hal381_one_clip_terminal_matches_true_front_at_fractional_rounded_edge_n1_n4() {
    use crate::three_d::{
        LoadingPresenceTransitionDuration3D as Duration, OrthographicProjection,
        PresentationMotionPolicy,
    };
    for (n, millis) in [(1, 160), (4, 160), (1, 3_000), (4, 3_000)] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let (proxy, association) = hal381_proxy_fixture(&context);
        let rect = RectF::new(0.0, 0.0, 64.0, 64.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (64, 64), 1.0)
            .unwrap();
        let metrics = active_metrics_for_surface(surface, 64, 64, 1.0);
        let angle = std::f32::consts::FRAC_PI_8;
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::try_from_xyzw(0.0, 0.0, angle.sin(), angle.cos()).unwrap(),
            ),
            OrthographicProjection::try_new(1.7, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::try_new(n).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: true,
            },
        )
        .unwrap();
        viewport.on_mount();
        let duration = Duration::try_from_millis(millis).unwrap();
        if duration != Duration::DEFAULT {
            assert!(
                viewport
                    .set_loading_presence_transition_duration(duration)
                    .unwrap()
            );
        }
        let request = viewport.loading_presence_request(&association).unwrap();
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let geometry = consumer_pbr_geometry(&context);
        let full = consumer_pbr_source(&context, &geometry, n, false, 1);
        viewport
            .request_loading_presence_pbr(full, &request, Viewport3DViewRevision::new(1), view)
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        let seconds = f64::from(millis) / 1_000.0;
        // The last transition frame can round its smoothstep weight to one
        // before reaching the actual elapsed-time threshold.
        hal372_update(&mut viewport, &mut timeline, seconds - 0.0000001);
        assert!(!viewport.loading_presence_terminal_for_test(surface));
        let transition =
            hal381_composed_pixels(&mut renderer, &mut viewport, &metrics, &compositor, false);
        hal372_update(&mut viewport, &mut timeline, seconds + 0.0001);
        assert!(viewport.loading_presence_terminal_for_test(surface));
        assert!(
            !viewport.needs_update(),
            "no perpetual terminal frame demand"
        );
        let terminal =
            hal381_composed_pixels(&mut renderer, &mut viewport, &metrics, &compositor, true);
        // Fixture-specific 8-bit native-format endpoint envelope, not a claim
        // about universal FP16 rounding or physical display scanout.
        let maximum = transition
            .iter()
            .zip(&terminal)
            .flat_map(|(a, b)| (0..4).map(move |c| a[c].abs_diff(b[c])))
            .max()
            .unwrap();
        assert!(maximum <= 2, "N{n} endpoint channel difference {maximum}");
        let decode = |v: u8| {
            let v = f32::from(v) / 255.0;
            if renderer.gpu_resources().format.is_srgb() {
                if v <= 0.04045 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            } else {
                v
            }
        };
        let edge = terminal[5 * 64 + 5];
        let full = read_r027_fp16_at(
            viewport.hdr_front_for_test(surface),
            &renderer.gpu_resources(),
            5,
            5,
        );
        if n == 4 {
            assert!(
                full[3] > 0.0 && full[3] < 1.0,
                "actual fractional source coverage at rounded edge: {full:?}"
            );
        }
        let h = (((2.0_f32 * 14.5 * 14.5).sqrt() - 20.0) / 1.0).clamp(0.0, 1.0);
        let coverage = 1.0 - h * h * (3.0 - 2.0 * h);
        assert!(coverage > 0.0 && coverage < 1.0);
        for c in 0..3 {
            let h = full[c] + (1.0 - full[3]) * [0.025, 0.031, 0.040][c];
            let expected = coverage * h + (1.0 - coverage) * [0.40, 0.05, 0.20][c];
            assert!(
                (decode(edge[c]) - expected).abs() < 0.006,
                "one clip, fractional source coverage, U!=B: N{n} edge={edge:?} c={coverage} expected={expected}"
            );
        }
        assert_eq!(edge[3], 255);
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        renderer
            .gpu_resources()
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
}

#[test]
fn hal381_scoped_source_completion_refuses_old_selection_mount_view_samples_and_context() {
    use crate::three_d::PresentationMotionPolicy;
    let (_renderer, context, view) = setup(PolicyLimits3D::conservative()).unwrap();
    let (a, association) = hal381_proxy_fixture(&context);
    let (b, _) = hal381_proxy_fixture(&context);
    let motion = PresentationMotionPolicy {
        allow_nonessential_motion: false,
    };
    let mut viewport = Viewport3D::new_loading_presence(
        context.clone(),
        a.clone(),
        Viewport3DViewRevision::new(1),
        view,
        SampleCount3D::ONE,
        motion,
    )
    .unwrap();
    assert!(
        viewport.loading_presence_request(&association).is_err(),
        "no fabricated mount"
    );
    viewport.on_mount();
    let first = viewport.loading_presence_request(&association).unwrap();
    let full = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), 1, false, 1);
    viewport
        .request_loading_presence(
            b,
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::ONE,
            motion,
        )
        .unwrap();
    viewport
        .request_loading_presence(
            a.clone(),
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::ONE,
            motion,
        )
        .unwrap();
    assert!(
        viewport
            .request_loading_presence_pbr(
                full.clone(),
                &first,
                Viewport3DViewRevision::new(1),
                view
            )
            .is_err(),
        "A/B/A cannot revive the old A ticket"
    );
    let old_view = viewport.loading_presence_request(&association).unwrap();
    viewport
        .request_loading_presence(
            a.clone(),
            Viewport3DViewRevision::new(2),
            view,
            SampleCount3D::ONE,
            motion,
        )
        .unwrap();
    assert!(
        viewport
            .request_loading_presence_pbr(
                full.clone(),
                &old_view,
                Viewport3DViewRevision::new(2),
                view
            )
            .is_err()
    );
    let old_samples = viewport.loading_presence_request(&association).unwrap();
    viewport
        .request_loading_presence(
            a.clone(),
            Viewport3DViewRevision::new(2),
            view,
            SampleCount3D::FOUR,
            motion,
        )
        .unwrap();
    let full4 = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), 4, false, 1);
    assert!(
        viewport
            .request_loading_presence_pbr(
                full4.clone(),
                &old_samples,
                Viewport3DViewRevision::new(2),
                view
            )
            .is_err()
    );
    let old_mount = viewport.loading_presence_request(&association).unwrap();
    viewport.on_unmount();
    viewport.on_mount();
    viewport
        .request_loading_presence(
            a,
            Viewport3DViewRevision::new(2),
            view,
            SampleCount3D::FOUR,
            motion,
        )
        .unwrap();
    assert!(
        viewport
            .request_loading_presence_pbr(full4, &old_mount, Viewport3DViewRevision::new(2), view)
            .is_err()
    );
    let current = viewport.loading_presence_request(&association).unwrap();
    let (_other_renderer, other_context, _) = setup(PolicyLimits3D::conservative()).unwrap();
    let foreign = consumer_pbr_source(
        &other_context,
        &consumer_pbr_geometry(&other_context),
        4,
        false,
        1,
    );
    assert!(
        viewport
            .request_loading_presence_pbr(foreign, &current, Viewport3DViewRevision::new(2), view)
            .is_err()
    );
    assert!(
        viewport.loading_presence_request(&association).is_ok(),
        "refusal preserves the current qualified proxy request"
    );
    assert!(
        viewport.current_prepared_target().is_none(),
        "no rejected source acquired a front"
    );
}

fn hal381_resolve_front(
    viewport: &mut Viewport3D,
    renderer: &mut WgpuRenderer,
    timeline: &mut Timeline,
    metrics: &CustomRenderTargetMetrics,
    time: f64,
) {
    viewport.prepare_custom_render(metrics);
    renderer.drain_pending_image_registrations();
    hal372_update(viewport, timeline, time);
    renderer.drain_pending_image_registrations();
    hal372_update(viewport, timeline, time);
    renderer.drain_pending_image_registrations();
}

#[test]
fn hal381_transition_duration_checks_integer_domain() {
    use crate::three_d::{
        LoadingPresenceError3D as Error, LoadingPresenceTransitionDuration3D as Duration,
    };
    assert_eq!(Duration::DEFAULT.as_millis(), 160);
    for millis in [80, 160, 3_000] {
        assert_eq!(
            Duration::try_from_millis(millis).unwrap().as_millis(),
            millis
        );
    }
    for millis in [79, 3_001] {
        assert!(matches!(
            Duration::try_from_millis(millis),
            Err(Error::Source(_))
        ));
    }
}

#[test]
fn hal381_transition_duration_uses_registered_front_and_freezes_per_occurrence_n1_n4() {
    use crate::three_d::{
        LoadingPresenceError3D as Error, LoadingPresenceTransitionDuration3D as Duration,
        PresentationMotionPolicy,
    };
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let (a, association) = hal381_proxy_fixture(&context);
        let (b, _) = hal381_proxy_fixture(&context);
        let full = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), n, false, 1);
        let view = hal374_view();
        let samples = SampleCount3D::try_new(n).unwrap();
        let motion = PresentationMotionPolicy {
            allow_nonessential_motion: true,
        };
        for millis in [80, 160, 3_000] {
            let duration = Duration::try_from_millis(millis).unwrap();
            let other =
                Duration::try_from_millis(if millis == 3_000 { 80 } else { 3_000 }).unwrap();
            let revision = Viewport3DViewRevision::new(1);
            let mut viewport = Viewport3D::new_loading_presence(
                context.clone(),
                a.clone(),
                revision,
                view,
                samples,
                motion,
            )
            .unwrap();
            assert!(matches!(
                viewport.set_loading_presence_transition_duration(duration),
                Err(Error::Association(_))
            ));
            viewport.on_mount();
            let metrics = active_metrics(38140, 64, 64, 1.0);
            let surface = metrics.surface_id;
            let mut time = Timeline::new();
            hal372_update(&mut viewport, &mut time, 0.0);
            let preview = viewport.prepare_custom_render(&metrics);
            renderer.drain_pending_image_registrations();
            let request = viewport.loading_presence_request(&association).unwrap();
            assert_eq!(
                viewport
                    .set_loading_presence_transition_duration(duration)
                    .unwrap(),
                duration != Duration::DEFAULT
            );
            assert!(
                !viewport
                    .set_loading_presence_transition_duration(duration)
                    .unwrap()
            );
            assert!(
                !viewport
                    .request_loading_presence(a.clone(), revision, view, samples, motion)
                    .unwrap()
            );
            let unchanged = viewport.prepare_custom_render(&metrics);
            assert!(!unchanged.front_publication_changed);
            assert_eq!(
                unchanged.presentation_binding(),
                preview.presentation_binding()
            );

            viewport
                .request_loading_presence_pbr(full.clone(), &request, revision, view)
                .unwrap();
            viewport.prepare_custom_render(&metrics);
            let texture = viewport.target_texture_id_for_test(surface).unwrap();
            let swaps = viewport.hdr_swaps_for_test(surface);
            let serial = viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .request_serial();
            // A committed CPU tuple is insufficient to freeze the setting.
            assert!(
                viewport
                    .set_loading_presence_transition_duration(other)
                    .unwrap()
            );
            assert!(
                viewport
                    .set_loading_presence_transition_duration(duration)
                    .unwrap()
            );
            hal381_resolve_front(&mut viewport, &mut renderer, &mut time, &metrics, 0.0);
            assert_eq!(
                viewport.loading_presence_target_progress_for_test(surface),
                Some((0.0, false))
            );
            assert!(matches!(
                viewport.set_loading_presence_transition_duration(other),
                Err(Error::Association(_))
            ));
            assert!(
                !viewport
                    .set_loading_presence_transition_duration(duration)
                    .unwrap()
            );

            let seconds = f64::from(millis) / 1_000.0;
            for (fraction, weight) in [(0.25, 0.15625), (0.5, 0.5)] {
                hal372_update(&mut viewport, &mut time, seconds * fraction);
                assert_eq!(
                    viewport.loading_presence_target_progress_for_test(surface),
                    Some((weight, false))
                );
            }
            hal372_update(&mut viewport, &mut time, seconds - 0.0001);
            assert!(!viewport.loading_presence_terminal_for_test(surface));
            hal372_update(&mut viewport, &mut time, seconds);
            assert!(viewport.loading_presence_terminal_for_test(surface));
            assert!(!viewport.needs_update());
            assert_eq!(viewport.target_texture_id_for_test(surface), Some(texture));
            assert_eq!(viewport.hdr_swaps_for_test(surface), swaps);
            let committed = viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap();
            assert_eq!(committed.request_serial(), serial);
            assert!(Arc::ptr_eq(committed.source(), &full.0));

            if millis == 3_000 {
                let revision = Viewport3DViewRevision::new(2);
                assert!(
                    viewport
                        .request_loading_presence(a.clone(), revision, view, samples, motion)
                        .unwrap()
                );
                let resized = active_metrics_for_surface(surface, 80, 72, 1.0);
                viewport.prepare_custom_render(&resized);
                renderer.drain_pending_image_registrations();
                assert!(
                    !viewport
                        .set_loading_presence_transition_duration(duration)
                        .unwrap()
                );
                assert!(matches!(
                    viewport.set_loading_presence_transition_duration(other),
                    Err(Error::Association(_))
                ));
                for proxy in [&b, &a] {
                    assert!(
                        viewport
                            .request_loading_presence(
                                proxy.clone(),
                                revision,
                                view,
                                samples,
                                motion
                            )
                            .unwrap()
                    );
                    assert!(
                        !viewport
                            .set_loading_presence_transition_duration(Duration::DEFAULT)
                            .unwrap()
                    );
                    assert!(
                        viewport
                            .set_loading_presence_transition_duration(duration)
                            .unwrap()
                    );
                }
                assert!(viewport.cancel_loading_presence());
                assert!(matches!(
                    viewport.set_loading_presence_transition_duration(duration),
                    Err(Error::Association(_))
                ));
            }
            viewport.on_unmount();
            renderer.drain_pending_image_registrations();
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        }
    }
}

#[test]
fn hal381_two_targets_have_independent_ready_times_on_one_timeline_n1_n4() {
    use crate::three_d::{
        LoadingPresenceError3D, LoadingPresenceTransitionDuration3D, PresentationMotionPolicy,
    };
    for n in [1, 4] {
        let (mut renderer, context, view) = setup(PolicyLimits3D::conservative()).unwrap();
        let (proxy, association) = hal381_proxy_fixture(&context);
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::try_new(n).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: true,
            },
        )
        .unwrap();
        viewport.on_mount();
        let request = viewport.loading_presence_request(&association).unwrap();
        let a = active_metrics(38131, 64, 64, 1.0);
        let b = active_metrics(38132, 80, 60, 1.0);
        let mut time = Timeline::new();
        hal372_update(&mut viewport, &mut time, 0.0);
        viewport.prepare_custom_render(&a);
        viewport.prepare_custom_render(&b);
        renderer.drain_pending_image_registrations();
        let full = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), n, false, 1);
        viewport
            .request_loading_presence_pbr(full, &request, Viewport3DViewRevision::new(1), view)
            .unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut time, &a, 1.0);
        assert!(matches!(
            viewport.set_loading_presence_transition_duration(
                LoadingPresenceTransitionDuration3D::try_from_millis(80).unwrap()
            ),
            Err(LoadingPresenceError3D::Association(_))
        ));
        hal381_resolve_front(&mut viewport, &mut renderer, &mut time, &b, 1.04);
        hal372_update(&mut viewport, &mut time, 1.08);
        assert!(
            (viewport
                .loading_presence_target_progress_for_test(a.surface_id)
                .unwrap()
                .0
                - 0.5)
                .abs()
                < 0.0001
        );
        assert!(
            (viewport
                .loading_presence_target_progress_for_test(b.surface_id)
                .unwrap()
                .0
                - 0.15625)
                .abs()
                < 0.0001
        );
        hal372_update(&mut viewport, &mut time, 1.1601);
        renderer.drain_pending_image_registrations();
        assert!(viewport.loading_presence_terminal_for_test(a.surface_id));
        assert!(!viewport.loading_presence_terminal_for_test(b.surface_id));
        assert!(viewport.needs_update());
        hal372_update(&mut viewport, &mut time, 1.2001);
        renderer.drain_pending_image_registrations();
        assert!(viewport.loading_presence_terminal_for_test(b.surface_id));
        assert!(!viewport.needs_update());
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        renderer
            .gpu_resources()
            .device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
    }
}

#[test]
fn hal381_static_wait_cancel_retone_and_target_retirement_n1_n4() {
    use crate::three_d::{
        LoadingPresenceTransitionDuration3D, OrthographicProjection, PresentationMotionPolicy,
    };
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(3.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let (proxy, association) = hal381_proxy_fixture(&context);
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy.clone(),
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::try_new(n).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: false,
            },
        )
        .unwrap();
        viewport.on_mount();
        assert!(
            viewport
                .set_loading_presence_transition_duration(
                    LoadingPresenceTransitionDuration3D::try_from_millis(3_000).unwrap()
                )
                .unwrap()
        );
        let request = viewport.loading_presence_request(&association).unwrap();
        let metrics = active_metrics(38141, 64, 64, 1.0);
        let mut time = Timeline::new();
        hal372_update(&mut viewport, &mut time, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut time, 100.0);
        assert!(!viewport.needs_update(), "static waiting is not polling");
        assert_eq!(viewport.loading_presence_weight_for_test(), Some(0.0));
        let full = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), n, false, 1);
        viewport
            .request_loading_presence_pbr(
                full.clone(),
                &request,
                Viewport3DViewRevision::new(1),
                view,
            )
            .unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut time, &metrics, 100.0);
        assert!(viewport.loading_presence_terminal_for_test(metrics.surface_id));
        assert!(!viewport.needs_update());
        let original = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        let mut changed = hal372_copy_scene_members(&full);
        changed.hdr.revision = 2;
        changed.hdr.exposure_ev = 1.0;
        let changed = crate::three_d::PbrScene3D(Arc::new(changed));
        viewport
            .request_loading_presence_pbr(
                changed.clone(),
                &request,
                Viewport3DViewRevision::new(1),
                view,
            )
            .unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut time, &metrics, 100.0);
        assert!(
            viewport.loading_presence_error().is_none(),
            "{:?}",
            viewport.loading_presence_error()
        );
        assert!(viewport.loading_presence_terminal_for_test(metrics.surface_id));
        let exposed = read_r027_fp16(viewport.hdr_front_for_test(metrics.surface_id), &gpu);
        assert!(exposed[0] > original[0]);
        assert_eq!(exposed[3], original[3]);
        assert!(viewport.cancel_loading_presence());
        let ordinary = viewport.prepare_custom_render(&metrics);
        assert!(ordinary.presentation_binding().is_some());
        assert!(
            Arc::ptr_eq(
                viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .source(),
                &changed.0
            ),
            "cancel after full admission retains the genuine new source"
        );
        viewport
            .request_loading_presence(
                proxy.clone(),
                Viewport3DViewRevision::new(1),
                view,
                SampleCount3D::try_new(n).unwrap(),
                PresentationMotionPolicy {
                    allow_nonessential_motion: false,
                },
            )
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        assert!(viewport.cancel_loading_presence());
        viewport.prepare_custom_render(&metrics);
        assert!(
            Arc::ptr_eq(
                viewport
                    .status_for(metrics.surface_id)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .source(),
                &changed.0
            ),
            "cancel before new full admission restores the true backup"
        );
        viewport
            .request_loading_presence(
                proxy,
                Viewport3DViewRevision::new(1),
                view,
                SampleCount3D::try_new(n).unwrap(),
                PresentationMotionPolicy {
                    allow_nonessential_motion: false,
                },
            )
            .unwrap();
        let resized = active_metrics_for_surface(metrics.surface_id, 800, 600, 1.0);
        viewport.prepare_custom_render(&resized);
        renderer.drain_pending_image_registrations();
        assert!(viewport.loading_presence_error().is_none());
        assert_eq!(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .unwrap()
                .size()
                .width,
            800
        );
        assert_eq!(viewport.loading_presence_weight_for_test(), Some(0.0));
        gpu.device.destroy();
        let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
        let failed = viewport.prepare_custom_render(&resized);
        assert!(
            failed.presentation_binding().is_none(),
            "lost Device cannot publish a proxy or old front"
        );
        viewport.on_unmount();
    }
}

#[test]
fn hal381_preview_budget_refuses_before_allocating_single_and_aggregate() {
    use crate::three_d::PresentationMotionPolicy;
    let (mut renderer, context, view) = setup(PolicyLimits3D::conservative()).unwrap();
    let gpu = renderer.gpu_resources();
    let (proxy, _) = hal381_proxy_fixture(&context);
    let before = context.target_charges_for_test();
    let mut viewport = Viewport3D::new_loading_presence(
        context.clone(),
        proxy.clone(),
        Viewport3DViewRevision::new(1),
        view,
        SampleCount3D::FOUR,
        PresentationMotionPolicy {
            allow_nonessential_motion: false,
        },
    )
    .unwrap();
    viewport.on_mount();
    let oversized = active_metrics(38151, 4097, 2048, 1.0);
    viewport.prepare_custom_render(&oversized);
    assert!(matches!(
        viewport.loading_presence_error(),
        Some(crate::three_d::LoadingPresenceError3D::Capacity)
    ));
    assert_eq!(
        context.target_charges_for_test(),
        before,
        "single-cap refusal precedes GPU target construction"
    );
    drop(viewport);
    let held = context
        .split_hdr_charges_for_test(
            0,
            context.limits().policy.max_total_attachment_bytes - 100,
            0,
        )
        .unwrap();
    let mut viewport = Viewport3D::new_loading_presence(
        context.clone(),
        proxy,
        Viewport3DViewRevision::new(1),
        view,
        SampleCount3D::FOUR,
        PresentationMotionPolicy {
            allow_nonessential_motion: false,
        },
    )
    .unwrap();
    viewport.on_mount();
    let metrics = active_metrics(38152, 64, 64, 1.0);
    viewport.prepare_custom_render(&metrics);
    assert!(matches!(
        viewport.loading_presence_error(),
        Some(crate::three_d::LoadingPresenceError3D::Context(
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::TotalAttachmentBytes,
                ..
            }
        ))
    ));
    assert_eq!(
        context.target_charges_for_test().1,
        context.limits().policy.max_total_attachment_bytes - 100
    );
    drop(held);
    drop(viewport);
    renderer.drain_pending_image_registrations();
    gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    assert_eq!(context.target_charges_for_test(), before);
}

#[test]
fn hal381_combined_single_cap_refusal_preserves_current_pbr_n1_n4() {
    use crate::three_d::{
        LoadingPresenceError3D, PbrScene3D, PresentationMotionPolicy, direct_shadow::POOL_BYTES,
    };

    for n in [1, 4] {
        let pixels = 64 * 64;
        let canonical_bytes = pixels * (20 * u64::from(n) + 16) + POOL_BYTES;
        let preview_bytes = pixels * (12 * u64::from(n) + 8);
        let combined = canonical_bytes + preview_bytes;
        let mut policy = PolicyLimits3D::conservative();
        policy.max_active_target_pixels = pixels;
        policy.max_single_target_attachment_bytes = combined - 1;
        let (mut renderer, context, _) = setup(policy).unwrap();
        let gpu = renderer.gpu_resources();
        let (proxy, association) = hal381_proxy_fixture(&context);
        let full = PbrScene3D(r025_source(&context, n, 0, 1.0));
        let revision = Viewport3DViewRevision::new(1);
        let view = r025_view();
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            revision,
            view,
            SampleCount3D::try_new(n).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: true,
            },
        )
        .unwrap();
        viewport.on_mount();
        let metrics = active_metrics(38153, 64, 64, 1.0);
        let surface = metrics.surface_id;
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        let preview = viewport.prepare_custom_render(&metrics);
        assert!(
            preview.presentation_binding().is_some(),
            "N{n}: {:?}",
            viewport.loading_presence_error()
        );
        renderer.drain_pending_image_registrations();
        assert_eq!(context.target_charges_for_test(), (pixels, preview_bytes));
        hal372_update(&mut viewport, &mut timeline, 0.02);
        assert!(viewport.needs_update());

        let request = viewport.loading_presence_request(&association).unwrap();
        viewport
            .request_loading_presence_pbr(full.clone(), &request, revision, view)
            .unwrap();
        let canonical = viewport.prepare_custom_render(&metrics);
        assert!(
            viewport.pbr_failure(surface).is_none(),
            "N{n}: {:?}",
            viewport.pbr_failure(surface)
        );
        assert!(matches!(
            viewport.loading_presence_error(),
            Some(LoadingPresenceError3D::Context(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::SingleTargetAttachmentBytes,
                requested,
                maximum,
            })) if *requested == combined && *maximum == combined - 1
        ));
        let status = viewport.status_for(surface).unwrap();
        assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
        assert!(status.pbr_committed().unwrap().source().same_identity(&full));
        assert!(canonical.presentation_binding().is_some());
        assert_ne!(
            canonical.presentation_binding(),
            preview.presentation_binding()
        );
        assert_eq!(context.target_charges_for_test(), (pixels, combined));

        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (pixels, canonical_bytes));
        assert_eq!(
            read_r027_fp16(viewport.hdr_front_for_test(surface), &gpu)[3],
            1.0
        );
        hal372_update(&mut viewport, &mut timeline, 0.1);
        assert!(!viewport.needs_update());
        let quiet = viewport.prepare_custom_render(&metrics);
        assert!(!quiet.front_publication_changed);
        assert_eq!(quiet.presentation_binding(), canonical.presentation_binding());
        viewport.on_unmount();
        drop(viewport);
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn hal381_compiled_reader_pins_real_old_front_across_retone_and_release_n1_n4() {
    use crate::three_d::{OrthographicProjection, PresentationMotionPolicy};
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let rect = RectF::new(0.0, 0.0, 64.0, 64.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        renderer
            .register_headless_render_target(surface, (64, 64), 1.0)
            .unwrap();
        let metrics = active_metrics_for_surface(surface, 64, 64, 1.0);
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(3.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let (proxy, association) = hal381_proxy_fixture(&context);
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::try_new(n).unwrap(),
            PresentationMotionPolicy {
                allow_nonessential_motion: false,
            },
        )
        .unwrap();
        viewport.on_mount();
        let request = viewport.loading_presence_request(&association).unwrap();
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let a = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), n, false, 1);
        viewport
            .request_loading_presence_pbr(a.clone(), &request, Viewport3DViewRevision::new(1), view)
            .unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
        let held =
            hal381_compile_presence(&mut renderer, &mut viewport, &metrics, &compositor, true);
        let mut b = hal372_copy_scene_members(&a);
        b.hdr.revision = 2;
        b.hdr.exposure_ev = 1.0;
        let b = crate::three_d::PbrScene3D(Arc::new(b));
        viewport
            .request_loading_presence_pbr(b.clone(), &request, Viewport3DViewRevision::new(1), view)
            .unwrap();
        #[cfg(feature = "test-harness")]
        assert!(
            !viewport
                .loading_presence_progress_for_test_harness(surface)
                .unwrap()
                .1,
            "the registered A pin must not be reported as a terminal B generation"
        );
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
        assert!(Arc::ptr_eq(
            viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .source(),
            &b.0
        ));
        let mut c = hal372_copy_scene_members(&b);
        c.hdr.revision = 3;
        c.hdr.exposure_ev = 2.0;
        let c = crate::three_d::PbrScene3D(Arc::new(c));
        viewport
            .request_loading_presence_pbr(c.clone(), &request, Viewport3DViewRevision::new(1), view)
            .unwrap();
        viewport.prepare_custom_render(&metrics);
        assert!(
            viewport.pbr_failure(surface).is_some(),
            "old A is now the backbuffer and remains physically pinned"
        );
        assert!(
            Arc::ptr_eq(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .committed_material()
                    .unwrap()
                    .source(),
                &b.0
            ),
            "B remains attributable last-good, never C"
        );
        drop(held);
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
        assert!(
            viewport.pbr_failure(surface).is_none(),
            "{:?}",
            viewport.pbr_failure(surface)
        );
        assert!(Arc::ptr_eq(
            viewport
                .status_for(surface)
                .unwrap()
                .committed_material()
                .unwrap()
                .source(),
            &c.0
        ));
        assert!(viewport.loading_presence_terminal_for_test(surface));
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
    }
}

#[test]
fn hal381_owner_entry_time_survives_view_resize_and_inactive_targets_n1_n4() {
    use crate::three_d::{OrthographicProjection, PresentationMotionPolicy};
    for n in [1, 4] {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let (proxy, _) = hal381_proxy_fixture(&context);
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(3.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let motion = PresentationMotionPolicy {
            allow_nonessential_motion: true,
        };
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy.clone(),
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::try_new(n).unwrap(),
            motion,
        )
        .unwrap();
        viewport.on_mount();
        let mut timeline = Timeline::new();
        let metrics = active_metrics(38161, 64, 64, 1.0);
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let base = read_r027_fp16(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .unwrap(),
            &gpu,
        );
        hal372_update(&mut viewport, &mut timeline, 0.8);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let advected = read_r027_fp16(
            viewport
                .loading_presence_texture_for_test(metrics.surface_id)
                .unwrap(),
            &gpu,
        );
        assert_ne!(advected, base);
        let geometry_bytes = context.resource_usage().unwrap().resident_geometry_bytes;
        viewport
            .request_loading_presence(
                proxy,
                Viewport3DViewRevision::new(2),
                view,
                SampleCount3D::try_new(n).unwrap(),
                motion,
            )
            .unwrap();
        hal372_update(&mut viewport, &mut timeline, 0.8);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        assert_eq!(
            read_r027_fp16(
                viewport
                    .loading_presence_texture_for_test(metrics.surface_id)
                    .unwrap(),
                &gpu
            ),
            advected
        );
        assert_eq!(
            context.resource_usage().unwrap().resident_geometry_bytes,
            geometry_bytes,
            "view refresh does not reupload geometry"
        );
        let resized = active_metrics_for_surface(metrics.surface_id, 192, 192, 1.0);
        viewport.prepare_custom_render(&resized);
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut timeline, 0.8);
        viewport.prepare_custom_render(&resized);
        renderer.drain_pending_image_registrations();
        // Match the original (32, 32) center with an odd integer resize ratio.
        assert_eq!(
            read_r027_fp16_at(
                viewport
                    .loading_presence_texture_for_test(metrics.surface_id)
                    .unwrap(),
                &gpu,
                97,
                97,
            ),
            advected,
            "target refresh cannot reset source entry time"
        );
        let mut inactive = resized;
        inactive.activity = CustomRenderActivity::Inactive;
        viewport.prepare_custom_render(&inactive);
        hal372_update(&mut viewport, &mut timeline, 0.9);
        assert!(
            !viewport.needs_update(),
            "unavailable target owns no infinite frame demand"
        );
        hal372_update(&mut viewport, &mut timeline, 3.2);
        viewport.prepare_custom_render(&resized);
        renderer.drain_pending_image_registrations();
        hal372_update(&mut viewport, &mut timeline, 3.2);
        viewport.prepare_custom_render(&resized);
        renderer.drain_pending_image_registrations();
        assert_eq!(
            read_r027_fp16_at(
                viewport
                    .loading_presence_texture_for_test(metrics.surface_id)
                    .unwrap(),
                &gpu,
                97,
                97,
            ),
            base,
            "restore uses the same occurrence's actual owner time"
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
        assert_eq!(context.target_charges_for_test(), (0, 0));
    }
}

#[test]
fn hal381_timeline_refusal_retires_private_targets_without_another_update() {
    use crate::three_d::PresentationMotionPolicy;
    let (mut renderer, context, view) = setup(PolicyLimits3D::conservative()).unwrap();
    let (proxy, _) = hal381_proxy_fixture(&context);
    let mut viewport = Viewport3D::new_loading_presence(
        context.clone(),
        proxy,
        Viewport3DViewRevision::new(1),
        view,
        SampleCount3D::ONE,
        PresentationMotionPolicy {
            allow_nonessential_motion: true,
        },
    )
    .unwrap();
    viewport.on_mount();
    let mut time = Timeline::with_capacity(0);
    let metrics = active_metrics(38171, 64, 64, 1.0);
    hal372_update(&mut viewport, &mut time, 0.0);
    viewport.prepare_custom_render(&metrics);
    renderer.drain_pending_image_registrations();
    assert!(context.target_charges_for_test().1 > 0);
    hal372_update(&mut viewport, &mut time, 0.1);
    assert!(viewport.loading_presence_error().is_some());
    assert!(!viewport.needs_update());
    assert!(
        viewport
            .prepare_custom_render(&metrics)
            .presentation_binding()
            .is_none()
    );
    renderer.drain_pending_image_registrations();
    renderer
        .gpu_resources()
        .device
        .poll(wgpu::PollType::wait_indefinitely())
        .unwrap();
    assert_eq!(context.target_charges_for_test(), (0, 0));
}
#[test]
fn hal381_shutdown_retention_does_not_form_a_queue_owner_cycle() {
    use crate::three_d::{OrthographicProjection, PresentationMotionPolicy};
    let queue = {
        let (mut renderer, context, _) = setup(PolicyLimits3D::conservative()).unwrap();
        let gpu = renderer.gpu_resources();
        let weak = Arc::downgrade(&gpu.queue);
        let (proxy, association) = hal381_proxy_fixture(&context);
        let view = ViewDescriptor3::new(
            RigidPose3::new(
                WorldPoint3::try_new(0.0, 0.0, 3.0).unwrap(),
                Orientation3::identity(),
            ),
            OrthographicProjection::try_new(3.0, 0.1, 10.0)
                .unwrap()
                .into(),
        );
        let mut viewport = Viewport3D::new_loading_presence(
            context.clone(),
            proxy,
            Viewport3DViewRevision::new(1),
            view,
            SampleCount3D::FOUR,
            PresentationMotionPolicy {
                allow_nonessential_motion: false,
            },
        )
        .unwrap();
        viewport.on_mount();
        let request = viewport.loading_presence_request(&association).unwrap();
        let metrics = active_metrics(38181, 64, 64, 1.0);
        let mut timeline = Timeline::new();
        hal372_update(&mut viewport, &mut timeline, 0.0);
        viewport.prepare_custom_render(&metrics);
        renderer.drain_pending_image_registrations();
        let full = consumer_pbr_source(&context, &consumer_pbr_geometry(&context), 4, false, 1);
        viewport
            .request_loading_presence_pbr(full, &request, Viewport3DViewRevision::new(1), view)
            .unwrap();
        hal381_resolve_front(&mut viewport, &mut renderer, &mut timeline, &metrics, 0.0);
        assert!(viewport.loading_presence_terminal_for_test(metrics.surface_id));
        // Deliberately close without a final Device::poll/extra submit. This
        // exercises the actual renderer registry retirement and queued guards.
        drop(viewport);
        drop(context);
        drop(gpu);
        drop(renderer);
        weak
    };
    assert!(
        queue.upgrade().is_none(),
        "a completed/retiring frame cannot own its Queue through its callback"
    );
}
