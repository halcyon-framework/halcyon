//! External-crate proof: this client has no pub(crate), cfg(test) library seam,
//! source loader or programmable shader. All material input uses three_d exports.
use halcyon::three_d::*;
use std::sync::Arc;

fn geometry(mode: u8, revision: u64) -> PbrGeometryDescriptor3D {
    let identity = PbrOriginalIdentity3D {
        source: 100 + u64::from(mode),
        revision,
        node: 0,
        mesh: 0,
        primitive: 0,
    };
    let positions = [[-1.0, -1.0, 0.0], [1.0, -1.0, 0.0], [0.0, 1.0, 0.0]];
    let uv = [[0.0, 0.0], [1.0, 0.0], [0.5, 1.0]];
    let (normals, tangents, render_n, render_t, frame) = match mode {
        0 => (
            [[0.0, 0.0, 1.0]; 3],
            [[1.0, 0.0, 0.0, 1.0]; 3],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            PbrFrameCertificate3D::OldCone,
        ),
        1 => (
            [[0.8, 0.0, 0.6]; 3],
            [[-0.6, 0.0, 0.8, 1.0]; 3],
            [0.8, 0.0, 0.6],
            [-0.6, 0.0, 0.8, 1.0],
            PbrFrameCertificate3D::AuthoredSmooth {
                separator: [0.0, -1.0, 0.0],
                lower: [0.9; 9],
            },
        ),
        2 => (
            [[1.0, 0.0, 1.0], [-1.0, 0.0, 1.0], [0.0, 0.0, 1.0]],
            [
                [1.0, 2.0, -1.0, 1.0],
                [-1.0, -2.0, -1.0, 1.0],
                [1.0, 0.0, 0.0, 1.0],
            ],
            [0.0, 0.0, 1.0],
            [1.0, 0.0, 0.0, 1.0],
            PbrFrameCertificate3D::SourceGeometric {
                singularity: PbrSmoothSingularity3D::MixedCone {
                    generators: [0, 1, 3, 4],
                    reverse_coefficients: false,
                    antiparallel: true,
                },
                separator: [0.0, 1.0, 0.0],
                lower: [0.9; 9],
            },
        ),
        _ => panic!("test source mode"),
    };
    PbrGeometryDescriptor3D {
        original: PbrOriginalGeometry3D {
            identity,
            vertices: (0..3)
                .map(|j| PbrOriginalVertex3D {
                    position: positions[j],
                    normal: normals[j],
                    uv0: Some(uv[j]),
                    color: Some([255; 4]),
                })
                .collect(),
            indices: GeometryIndices::Unindexed,
            signed_tangents: tangents.into(),
            tangent_provenance: PbrTangentProvenance3D::Authored,
        },
        render: (0..3)
            .map(|j| PbrVertex3D {
                position: positions[j],
                normal: render_n,
                uv0: uv[j],
                color: [255; 4],
                tangent: render_t,
            })
            .collect(),
        render_indices: GeometryIndices::Unindexed,
        render_vertex_sources: Arc::from([0, 1, 2]),
        triangles: Arc::from([PbrTriangleCertificate3D {
            original: identity,
            triangle: 0,
            certificate_identity: 400 + u64::from(mode),
            certificate_revision: revision,
            frame,
        }]),
        mapped: true,
        maximum_certificate_bytes: 65536,
        maximum_verification_items: 1000,
    }
}

fn normal(correlated: bool) -> Arc<PbrMipChainDescriptor3D> {
    let original: Arc<[u8]> = if correlated {
        vec![
            255, 128, 127, 255, 128, 255, 255, 255, 128, 255, 255, 255, 0, 127, 128, 255,
        ]
        .into()
    } else {
        [255, 128, 127, 255].repeat(4).into()
    };
    // Actual stored-parent first moment; constant stays constant. The nonconstant
    // parent averages independently to [127.75,191.25,191.25,255].
    let child = if correlated {
        [128, 191, 191, 255]
    } else {
        [255, 128, 127, 255]
    };
    let mut bytes = original.to_vec();
    bytes.extend(child);
    let texels: Arc<[u8]> = bytes.into();
    Arc::new(PbrMipChainDescriptor3D {
        role: PbrMipRole3D::Normal,
        format: TextureFormat3D::Rgba8Unorm,
        levels: Arc::from([
            PbrMipLevel3D {
                width: 2,
                height: 2,
                offset: 0,
                bytes: 16,
            },
            PbrMipLevel3D {
                width: 1,
                height: 1,
                offset: 16,
                bytes: 4,
            },
        ]),
        provenance: PbrMipProvenance3D {
            image_identity: 1000 + u64::from(correlated),
            content_revision: 1,
            producer_identity: 1010,
            producer_revision: 1,
            generator_identity: 1011,
            generator_revision: 1,
            generator: PbrMipGenerator3D::RecursiveStoredAreaR028V1,
            origin: PbrImageOrigin3D::TopLeft,
            uv_set: 0,
            work: 4,
            error_bound: 0.5,
            original,
            derived: Arc::downgrade(&texels),
        },
        texels,
    })
}
fn sampler(input: &PbrMipChainDescriptor3D) -> PbrMipSamplerDescriptor3D {
    PbrMipSamplerDescriptor3D {
        identity: 1020,
        revision: 1,
        image_identity: input.provenance.image_identity,
        image_revision: 1,
        u: SamplerAddressMode::ClampToEdge,
        v: SamplerAddressMode::ClampToEdge,
        mag: SamplerFilterMode::Linear,
        min: PbrMipMin3D::LinearMipLinear,
        extent: [2, 2],
    }
}
fn proof(
    input: Arc<PbrMipChainDescriptor3D>,
    registered: PbrMipSampler3D,
    policy: PbrMipSamplerDescriptor3D,
    correlated: bool,
) -> PbrNormalCertificate3D {
    let mut exceptions = Vec::new();
    if correlated {
        for (mag, grid) in [
            (false, vec![0.0f64, 0.25, 0.5, 0.75, 1.0]),
            (true, vec![0.0f64, 0.25, 0.75, 1.0]),
        ] {
            for u in 0..grid.len() - 1 {
                for v in 0..grid.len() - 1 {
                    let depth = if mag && u == 1 && v == 1 { 1 } else { 0 };
                    let mut leaves = Vec::new();
                    for path in 0..if depth == 0 { 1 } else { 8 } {
                        let cu = (grid[u] + grid[u + 1]) * 0.5
                            + if depth == 0 {
                                0.0
                            } else if path & 1 == 0 {
                                -0.125
                            } else {
                                0.125
                            };
                        let cv = (grid[v] + grid[v + 1]) * 0.5
                            + if depth == 0 {
                                0.0
                            } else if path & 2 == 0 {
                                -0.125
                            } else {
                                0.125
                            };
                        let x = ((cu - 0.25) / 0.5).clamp(0.0, 1.0);
                        let y = ((cv - 0.25) / 0.5).clamp(0.0, 1.0);
                        let sign = if (1.0 - x) * (1.0 - y) >= x * y {
                            1.0
                        } else {
                            -1.0
                        };
                        leaves.push(PbrNormalBox3D {
                            path,
                            depth,
                            witness: PbrNormalWitness3D {
                                direction: [sign, 1.0, 1.0],
                                lower: 0.1,
                            },
                        });
                    }
                    exceptions.push(PbrNormalException3D {
                        cell: PbrNormalCell3D {
                            mag,
                            linear: true,
                            lower: 0,
                            upper: if mag { 0 } else { 1 },
                            u: u as u32,
                            v: v as u32,
                            u_choice: 0,
                            v_choice: 0,
                        },
                        leaves: leaves.into(),
                    });
                }
            }
        }
    }
    PbrNormalCertificate3D {
        identity: 1030,
        revision: 1,
        source: input,
        sampler: registered,
        policy,
        normal_scale_bits: 1.0f32.to_bits(),
        cells: 25,
        maximum_work: 1_000_000,
        maximum_metadata_bytes: 65536,
        default: PbrNormalWitness3D {
            direction: if correlated {
                [0.0, 1.0, 1.0]
            } else {
                [1.0, 0.0, 0.0]
            },
            lower: if correlated { 0.1 } else { 0.9 },
        },
        exceptions: exceptions.into(),
    }
}
fn view() -> ViewDescriptor3 {
    ViewDescriptor3::new(
        RigidPose3::new(
            WorldPoint3::try_new(0.0, 0.0, 2.0).unwrap(),
            Orientation3::identity(),
        ),
        OrthographicProjection::try_new(2.0, 0.1, 10.0)
            .unwrap()
            .into(),
    )
}
fn identity() -> ModelPlacement3 {
    ModelPlacement3::try_new(
        WorldPoint3::try_new(0.0, 0.0, 0.0).unwrap(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap()
}

#[test]
fn pbr_unlinked_material_source_external_abc_and_normal_boundary() {
    let renderer =
        pollster::block_on(halcyon::renderer::wgpu_renderer::WgpuRenderer::new_headless())
            .expect("external source proof requires the real Device");
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    for mode in 0..3 {
        context.admit_pbr_geometry(geometry(mode, 1), None).unwrap();
    }
    let mut absent = geometry(0, 2);
    absent.mapped = false;
    for vertex in Arc::make_mut(&mut absent.original.vertices) {
        vertex.uv0 = None;
    }
    absent.original.signed_tangents = Arc::from([]);
    for vertex in Arc::make_mut(&mut absent.render) {
        vertex.uv0 = [0.0; 2];
    }
    Arc::make_mut(&mut absent.triangles)[0].frame = PbrFrameCertificate3D::NormalOnly {
        separator: [0.0, 0.0, 1.0],
        lower: [0.9; 3],
    };
    context.admit_pbr_geometry(absent, None).unwrap();
    let before = context.resource_usage().unwrap();
    let mut forged = geometry(2, 1);
    if let PbrFrameCertificate3D::SourceGeometric { singularity, .. } =
        &mut Arc::make_mut(&mut forged.triangles)[0].frame
    {
        *singularity = PbrSmoothSingularity3D::MixedCone {
            generators: [0, 1, 3, 4],
            reverse_coefficients: true,
            antiparallel: true,
        };
    }
    assert!(matches!(
        context.admit_pbr_geometry(forged, None),
        Err(PbrSourceError3D::Frame { .. })
    ));
    let mut forged = geometry(1, 1);
    if let PbrFrameCertificate3D::AuthoredSmooth { lower, .. } =
        &mut Arc::make_mut(&mut forged.triangles)[0].frame
    {
        *lower = [2.0; 9];
    }
    assert!(context.admit_pbr_geometry(forged, None).is_err());
    assert_eq!(context.resource_usage().unwrap(), before);
    for correlated in [false, true] {
        let input = normal(correlated);
        let policy = sampler(&input);
        let registered = context.admit_pbr_mip_sampler(policy, None).unwrap();
        let cert = proof(input.clone(), registered.clone(), policy, correlated);
        assert!(context.admit_pbr_mip_chain(input.clone(), None).is_err());
        let chain = context
            .admit_pbr_certified_normal_chain(input.clone(), &registered, 1.0, cert.clone(), None)
            .unwrap();
        let mut material =
            PbrMaterialDescriptor3D::new(1050 + u64::from(correlated), 1, PbrSamplingFamily3D::V2);
        material.roles.normal = Some(PbrRoleBinding3D::V2 {
            texture: chain.clone(),
            sampler: registered.clone(),
        });
        context.admit_pbr_material(material.clone(), None).unwrap();
        material.normal_scale = -0.0;
        assert!(context.admit_pbr_material(material, None).is_err());
        let wrong = context.admit_pbr_mip_sampler(policy, None).unwrap();
        let before = context.resource_usage().unwrap();
        assert!(
            context
                .admit_pbr_certified_normal_chain(input.clone(), &wrong, 1.0, cert.clone(), None)
                .is_err()
        );
        let mut partial = cert.clone();
        partial.cells -= 1;
        assert!(
            context
                .admit_pbr_certified_normal_chain(input.clone(), &registered, 1.0, partial, None)
                .is_err()
        );
        let mut forged = cert;
        forged.default.lower = 2.0;
        // The constant uses the default; the correlated source has explicit cells.
        if !correlated {
            assert!(
                context
                    .admit_pbr_certified_normal_chain(input.clone(), &registered, 1.0, forged, None)
                    .is_err()
            );
        }
        assert_eq!(context.resource_usage().unwrap(), before);
        let mut next = proof(input.clone(), registered.clone(), policy, correlated);
        next.revision = 2;
        let successor = context
            .admit_pbr_certified_normal_chain(input.clone(), &registered, 1.0, next, Some(&chain))
            .unwrap();
        assert_ne!(successor.resource_generation(), chain.resource_generation());
        let stale = proof(input.clone(), registered.clone(), policy, correlated);
        assert!(
            context
                .admit_pbr_certified_normal_chain(
                    input.clone(),
                    &registered,
                    1.0,
                    stale,
                    Some(&successor),
                )
                .is_err()
        );
    }
    let mut required = PbrMaterialDescriptor3D::new(1060, 1, PbrSamplingFamily3D::V2);
    required.required.normal = true;
    assert!(matches!(
        context.admit_pbr_material(required, None),
        Err(PbrSourceError3D::RequiredRole(PbrTextureRole3D::Normal))
    ));
    assert_eq!(context.resource_usage().unwrap().resident_geometry_bytes, 0);
}

#[test]
fn pbr_unlinked_material_source_external_five_roles_v1_v2_mask_and_opaque() {
    let renderer =
        pollster::block_on(halcyon::renderer::wgpu_renderer::WgpuRenderer::new_headless())
            .expect("five-role external source proof requires the actual Device");
    let context = Context3D::new(renderer.gpu_resources()).unwrap();
    let sampler = context.register_sampler(Sampler3D::LINEAR_CLAMP).unwrap();
    for (family_index, family) in [PbrSamplingFamily3D::V1, PbrSamplingFamily3D::V2]
        .into_iter()
        .enumerate()
    {
        for (alpha_index, alpha) in [PbrAlphaMode3D::Opaque, PbrAlphaMode3D::Mask]
            .into_iter()
            .enumerate()
        {
            let mut textures = Vec::new();
            let mut bind = |role, format, pixel: [u8; 4], ordinal| {
                let id = 3000 + (family_index * 100 + alpha_index * 10 + ordinal) as u64;
                if family == PbrSamplingFamily3D::V1 {
                    let lease = context
                        .register_texture(Texture3D::try_new(1, 1, format, pixel.to_vec()).unwrap())
                        .unwrap();
                    let binding = PbrRoleBinding3D::V1 {
                        texture: lease.handle().clone(),
                        sampler: sampler.handle().clone(),
                    };
                    textures.push(lease);
                    binding
                } else {
                    // A genuine authored1x1 source is its complete chain. No
                    // production mip generator or guessed derived byte is used.
                    let original: Arc<[u8]> = pixel.into();
                    let chain = Arc::new(PbrMipChainDescriptor3D {
                        role,
                        format,
                        levels: Arc::from([PbrMipLevel3D {
                            width: 1,
                            height: 1,
                            offset: 0,
                            bytes: 4,
                        }]),
                        texels: original.clone(),
                        provenance: PbrMipProvenance3D {
                            image_identity: id,
                            content_revision: 1,
                            producer_identity: 3001,
                            producer_revision: 1,
                            generator_identity: 3002,
                            generator_revision: 1,
                            generator: PbrMipGenerator3D::RecursiveStoredAreaR028V1,
                            origin: PbrImageOrigin3D::TopLeft,
                            uv_set: 0,
                            work: 4,
                            error_bound: 0.0,
                            derived: Arc::downgrade(&original),
                            original,
                        },
                    });
                    let sampler = context
                        .admit_pbr_mip_sampler(
                            PbrMipSamplerDescriptor3D {
                                identity: id,
                                revision: 1,
                                image_identity: id,
                                image_revision: 1,
                                u: SamplerAddressMode::Repeat,
                                v: SamplerAddressMode::MirrorRepeat,
                                min: PbrMipMin3D::Linear,
                                mag: SamplerFilterMode::Nearest,
                                extent: [1, 1],
                            },
                            None,
                        )
                        .unwrap();
                    let texture = context.admit_pbr_mip_chain(chain, None).unwrap();
                    PbrRoleBinding3D::V2 { texture, sampler }
                }
            };
            let mut material = PbrMaterialDescriptor3D::new(
                3500 + (family_index * 10 + alpha_index) as u64,
                1,
                family,
            );
            material.alpha_mode = alpha;
            material.roles = PbrMaterialRoles3D {
                base: Some(bind(
                    if alpha == PbrAlphaMode3D::Mask {
                        PbrMipRole3D::MaskBase
                    } else {
                        PbrMipRole3D::OpaqueBase
                    },
                    TextureFormat3D::Rgba8UnormSrgb,
                    [100, 160, 200, 192],
                    0,
                )),
                normal: Some(bind(
                    PbrMipRole3D::Normal,
                    TextureFormat3D::Rgba8Unorm,
                    [128, 128, 255, 255],
                    1,
                )),
                metallic_roughness: Some(bind(
                    PbrMipRole3D::MetallicRoughness,
                    TextureFormat3D::Rgba8Unorm,
                    [33, 144, 96, 255],
                    2,
                )),
                occlusion: Some(bind(
                    PbrMipRole3D::Occlusion,
                    TextureFormat3D::Rgba8Unorm,
                    [220, 10, 20, 255],
                    3,
                )),
                emissive: Some(bind(
                    PbrMipRole3D::Emissive,
                    TextureFormat3D::Rgba8UnormSrgb,
                    [10, 20, 30, 255],
                    4,
                )),
            };
            material.required = PbrRequiredRoles3D {
                base: true,
                normal: true,
                metallic_roughness: true,
                occlusion: true,
                emissive: true,
            };
            context.admit_pbr_material(material, None).unwrap();
        }
    }
    assert_eq!(context.resource_usage().unwrap().resident_geometry_bytes, 0);
}

#[test]
fn pbr_unlinked_material_source_external_nontest_writer_n1_n4() {
    use halcyon::compositor::{Compositor, SceneKey};
    use halcyon::core::math::RectF;
    use halcyon::renderer::{
        CustomRenderActivity, CustomRenderParticipant, CustomRenderTargetMetrics, PhysicalExtent,
        PhysicalRect,
    };
    let renderer =
        pollster::block_on(halcyon::renderer::wgpu_renderer::WgpuRenderer::new_headless())
            .expect("real Device required");
    let context = Arc::new(Context3D::new(renderer.gpu_resources()).unwrap());
    let input = normal(false);
    let policy = sampler(&input);
    let registered = context.admit_pbr_mip_sampler(policy, None).unwrap();
    let normal = context
        .admit_pbr_certified_normal_chain(
            input.clone(),
            &registered,
            1.0,
            proof(input, registered.clone(), policy, false),
            None,
        )
        .unwrap();
    let mut material = PbrMaterialDescriptor3D::new(2000, 1, PbrSamplingFamily3D::V2);
    material.base_rgba = [0.5, 0.5, 0.5, 1.0];
    material.metallic = 0.0;
    material.roughness = 1.0;
    material.roles.normal = Some(PbrRoleBinding3D::V2 {
        texture: normal.clone(),
        sampler: registered.clone(),
    });
    let material = context.admit_pbr_material(material, None).unwrap();
    let mesh = context.admit_pbr_geometry(geometry(1, 1), None).unwrap();
    let snapshot = context
        .admit_pbr_snapshot(
            StructuralRevision::new(1),
            vec![PbrDrawPacket3D {
                geometry: mesh.clone(),
                material: material.clone(),
                raster: RasterMode::SingleSided,
                stable_ordinal: 0,
            }],
        )
        .unwrap();
    let model = context
        .admit_pbr_model(&snapshot, ModelRevision::new(1), vec![identity()])
        .unwrap();
    let direct = PbrDirectSource3D::capture(
        PbrDirectDescriptor3D {
            identity: 2100,
            revision: 1,
            meters_per_scene_unit: 1.0,
            directional: Some(PbrDirectionalLight3D {
                surface_to_source: [0.0, 0.0, 1.0],
                irradiance: [1.0; 3],
                shadow_far_m: 8.0,
            }),
            points: Arc::from([]),
            spots: Arc::from([]),
        },
        &snapshot,
        &model,
    )
    .unwrap();
    for samples in [SampleCount3D::ONE, SampleCount3D::FOUR] {
        let source = context
            .admit_pbr_scene(PbrSceneDescriptor3D {
                snapshot: snapshot.clone(),
                model: model.clone(),
                hdr: PbrHdrSource3D {
                    identity: 2200,
                    revision: 1,
                    exposure_ev: 0.0,
                    maximum_rgb: [4.0; 3],
                },
                samples,
                direct: Some(direct.clone()),
                indirect: None,
            })
            .unwrap();
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            source.clone(),
            Viewport3DViewRevision::new(1),
            view(),
        )
        .unwrap();
        let rect = RectF::new(0.0, 0.0, 32.0, 32.0);
        let mut compositor = Compositor::new(rect);
        let surface_id = compositor.create_surface(0, rect, SceneKey::BASE);
        let metrics = CustomRenderTargetMetrics {
            surface_id,
            app_frame_attempt: None,
            final_logical_rect: rect,
            final_logical_viewport: rect,
            target_logical_extent: rect.size(),
            scale_factor: 1.0,
            target_physical_extent: PhysicalExtent::new(32, 32),
            activity: CustomRenderActivity::Active {
                physical_render_extent: PhysicalExtent::new(32, 32),
                physical_composition_coverage: PhysicalRect::new(0, 0, 32, 32),
            },
        };
        viewport.prepare_custom_render(&metrics);
        let status = viewport.status_for(metrics.surface_id).unwrap();
        assert_eq!(status.state(), Viewport3DLifecycleState::Committed);
        assert!(status.committed().is_none());
        assert!(
            status
                .pbr_committed()
                .unwrap()
                .source()
                .same_identity(&source)
        );
        assert_eq!(
            status.pbr_committed().unwrap().target_key().sample_count(),
            samples
        );
        let usage = context.resource_usage().unwrap();
        assert!(
            !viewport
                .request_pbr_scene(source.clone(), Viewport3DViewRevision::new(1), view())
                .unwrap()
        );
        assert!(
            !viewport
                .prepare_custom_render(&metrics)
                .front_publication_changed
        );
        assert_eq!(context.resource_usage().unwrap(), usage);
        assert!(viewport.pbr_failure(metrics.surface_id).is_none());
    }
    context.retire_pbr_geometry_source(&mesh).unwrap();
    context.retire_pbr_material_source(&material).unwrap();
    context.retire_pbr_mip_chain(&normal).unwrap();
    context.retire_pbr_mip_sampler(&registered).unwrap();
    assert!(
        context
            .admit_pbr_snapshot(
                StructuralRevision::new(2),
                vec![PbrDrawPacket3D {
                    geometry: mesh,
                    material,
                    raster: RasterMode::SingleSided,
                    stable_ordinal: 0
                }]
            )
            .is_err()
    );
}

#[cfg(feature = "test-harness")]
#[test]
fn pbr_unlinked_external_harness_actual_metal_n1_n4() {
    use halcyon::compositor::{Compositor, SceneKey};
    use halcyon::core::math::RectF;
    use halcyon::core::widget::Widget;
    use halcyon::renderer::wgpu_renderer::{TestHarnessImageProofError, WgpuRenderer};
    use halcyon::renderer::{
        CustomRenderActivity, CustomRenderParticipant, CustomRenderTargetMetrics, PhysicalExtent,
        PhysicalRect,
    };

    let mut renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("external observation requires the real headless Device");
    let adapter = renderer.headless_adapter_info_for_test_harness().unwrap();
    assert_eq!(adapter.name, "Apple M4 Max");
    assert_eq!(adapter.backend, wgpu::Backend::Metal);
    let decode = |b: u8| {
        let x = f64::from(b) / 255.0;
        if x <= 0.04045 {
            x / 12.92
        } else {
            ((x + 0.055) / 1.055).powf(2.4)
        }
    };
    // One encoded-byte interval encloses normalized-target quantization and
    // its actual sRGB encode. These are numeric Image equations, not look goldens.
    let image_oracle = |actual: [u8; 4], front: [f32; 4], tint: [f32; 4]| {
        for k in 0..3 {
            let expected = f64::from(front[k]) * f64::from(tint[k]) * f64::from(tint[3]) + 1.0
                - f64::from(front[3]) * f64::from(tint[3]);
            assert!(
                (decode(actual[k].saturating_sub(1))..=decode(actual[k].saturating_add(1)))
                    .contains(&expected)
            );
        }
        assert_eq!(actual[3], 255);
    };
    for samples in [SampleCount3D::ONE, SampleCount3D::FOUR] {
        let context = Arc::new(Context3D::new(renderer.gpu_resources()).unwrap());
        let negative = normal(false);
        let mut positive = (*negative).clone();
        let original: Arc<[u8]> = [128, 128, 255, 255].repeat(4).into();
        let mut bytes = original.to_vec();
        bytes.extend([128, 128, 255, 255]);
        positive.texels = bytes.into();
        positive.provenance.image_identity = 6100;
        positive.provenance.original = original;
        positive.provenance.derived = Arc::downgrade(&positive.texels);
        let materials: Vec<_> = [Arc::new(positive), negative]
            .into_iter()
            .enumerate()
            .map(|(index, input)| {
                let mut policy = sampler(&input);
                policy.identity = 6200 + index as u64;
                let registered = context.admit_pbr_mip_sampler(policy, None).unwrap();
                let chain = if index == 0 {
                    context.admit_pbr_mip_chain(input, None).unwrap()
                } else {
                    context
                        .admit_pbr_certified_normal_chain(
                            input.clone(),
                            &registered,
                            1.0,
                            proof(input, registered.clone(), policy, false),
                            None,
                        )
                        .unwrap()
                };
                let mut material =
                    PbrMaterialDescriptor3D::new(6300 + index as u64, 1, PbrSamplingFamily3D::V2);
                material.base_rgba = [0.5, 0.5, 0.5, 1.0];
                material.metallic = 0.0;
                material.roughness = 1.0;
                material.roles.normal = Some(PbrRoleBinding3D::V2 {
                    texture: chain,
                    sampler: registered,
                });
                context.admit_pbr_material(material, None).unwrap()
            })
            .collect();
        let mesh = context.admit_pbr_geometry(geometry(1, 1), None).unwrap();
        let pairs: Vec<_> = materials
            .into_iter()
            .enumerate()
            .map(|(index, material)| {
                let snapshot = context
                    .admit_pbr_snapshot(
                        StructuralRevision::new(1 + index as u64),
                        vec![PbrDrawPacket3D {
                            geometry: mesh.clone(),
                            material,
                            raster: RasterMode::SingleSided,
                            stable_ordinal: 0,
                        }],
                    )
                    .unwrap();
                let model = context
                    .admit_pbr_model(
                        &snapshot,
                        ModelRevision::new(1 + index as u64),
                        vec![identity()],
                    )
                    .unwrap();
                (snapshot, model)
            })
            .collect();
        let scene = |index: usize, revision: u64, irradiance: f32| {
            let (snapshot, model) = &pairs[index];
            let direct = PbrDirectSource3D::capture(
                PbrDirectDescriptor3D {
                    identity: 6400,
                    revision,
                    meters_per_scene_unit: 1.0,
                    directional: Some(PbrDirectionalLight3D {
                        surface_to_source: [0.0, 0.0, 1.0],
                        irradiance: [irradiance; 3],
                        shadow_far_m: 8.0,
                    }),
                    points: Arc::from([]),
                    spots: Arc::from([]),
                },
                snapshot,
                model,
            )
            .unwrap();
            context
                .admit_pbr_scene(PbrSceneDescriptor3D {
                    snapshot: snapshot.clone(),
                    model: model.clone(),
                    hdr: PbrHdrSource3D {
                        identity: 6500,
                        revision,
                        exposure_ev: 0.0,
                        maximum_rgb: [8.0; 3],
                    },
                    samples,
                    direct: Some(direct),
                    indirect: None,
                })
                .unwrap()
        };
        // Identical material/geometry/view with E1/E2, then an independently
        // certified BLUE127 normal at E1. All are real immutable source revisions.
        let sources = [scene(0, 1, 1.0), scene(0, 2, 2.0), scene(1, 3, 1.0)];
        let mut viewport = Viewport3D::new_pbr(
            context.clone(),
            sources[0].clone(),
            Viewport3DViewRevision::new(1),
            view(),
        )
        .unwrap();
        let rect = RectF::new(0.0, 0.0, 32.0, 32.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        let foreign = compositor.create_surface(1, rect, SceneKey::BASE);
        let metrics = CustomRenderTargetMetrics {
            surface_id: surface,
            app_frame_attempt: None,
            final_logical_rect: rect,
            final_logical_viewport: rect,
            target_logical_extent: rect.size(),
            scale_factor: 1.0,
            target_physical_extent: PhysicalExtent::new(32, 32),
            activity: CustomRenderActivity::Active {
                physical_render_extent: PhysicalExtent::new(32, 32),
                physical_composition_coverage: PhysicalRect::new(0, 0, 32, 32),
            },
        };
        assert!(matches!(
            viewport.pbr_front_pixel_for_test_harness(surface, [16, 16]),
            Err(PbrFrontProofError3D::MissingTarget(_))
        ));
        let mut values = Vec::new();
        let mut last_image = None;
        for (index, source) in sources.iter().enumerate() {
            if index != 0 {
                viewport
                    .request_pbr_scene(source.clone(), Viewport3DViewRevision::new(1), view())
                    .unwrap();
                assert!(matches!(
                    viewport.pbr_front_pixel_for_test_harness(surface, [16, 16]),
                    Err(PbrFrontProofError3D::NotCurrent)
                ));
            }
            viewport.prepare_custom_render(&metrics);
            assert!(matches!(
                viewport.pbr_front_pixel_for_test_harness(surface, [16, 16]),
                Err(PbrFrontProofError3D::Image(
                    TestHarnessImageProofError::RegistrationPending(_)
                ))
            ));
            if let Some(id) = last_image {
                assert!(matches!(
                    renderer.registered_image_pixel_for_test_harness(
                        id,
                        [0.515625; 2],
                        [1.0; 4],
                        [1.0; 4]
                    ),
                    Err(TestHarnessImageProofError::RegistrationPending(_))
                ));
            }
            renderer.drain_pending_image_registrations();
            let usage = context.resource_usage().unwrap();
            let front = viewport
                .pbr_front_pixel_for_test_harness(surface, [16, 16])
                .unwrap();
            assert!(front.source.same_identity(source));
            assert_eq!(
                front.target_key.color_format(),
                wgpu::TextureFormat::Rgba16Float
            );
            assert_eq!(front.target_key.sample_count(), samples);
            assert_eq!(front.extent, [32, 32]);
            assert_eq!(front.pixel, [16, 16]);
            assert_eq!(front.view, view());
            assert_eq!(front.rgba[3], 1.0);
            for value in &front.rgba[..3] {
                assert!(value.is_finite() && *value > 0.0 && *value <= 8.0 / 9.0);
            }
            let white = renderer
                .registered_image_pixel_for_test_harness(
                    front.image_id,
                    [0.515625; 2],
                    [1.0; 4],
                    [1.0; 4],
                )
                .unwrap();
            let tint = [0.8, 0.6, 0.4, 0.5];
            let tinted = renderer
                .registered_image_pixel_for_test_harness(
                    front.image_id,
                    [0.515625; 2],
                    tint,
                    [1.0; 4],
                )
                .unwrap();
            image_oracle(white, front.rgba, [1.0; 4]);
            image_oracle(tinted, front.rgba, tint);
            let again = viewport
                .pbr_front_pixel_for_test_harness(surface, [16, 16])
                .unwrap();
            assert_eq!(again.rgba, front.rgba);
            assert_eq!(again.request_serial, front.request_serial);
            assert_eq!(again.target_key, front.target_key);
            assert!(again.source.same_identity(source));
            // Polling the actual Device can complete already-submitted retirement
            // of an old R025 shadow pool. Observation must add no retained charge;
            // completing that existing Queue obligation is not a source mutation.
            let mut after = context.resource_usage().unwrap();
            assert!(after.resident_texture_bytes <= usage.resident_texture_bytes);
            after.resident_texture_bytes = usage.resident_texture_bytes;
            assert_eq!(after, usage);
            assert!(
                !viewport
                    .request_pbr_scene(source.clone(), Viewport3DViewRevision::new(1), view())
                    .unwrap()
            );
            assert!(
                !viewport
                    .prepare_custom_render(&metrics)
                    .front_publication_changed
            );
            assert_eq!(
                viewport
                    .status_for(surface)
                    .unwrap()
                    .pbr_committed()
                    .unwrap()
                    .request_serial(),
                front.request_serial
            );
            std::io::Write::write_fmt(&mut std::io::stderr(), format_args!("EXTERNAL HARNESS {adapter:?} N{} source{:?} serial{} image{}: FP16={:?} WHITE Image={white:?} tinted={tinted:?}; current full key, immutable observation, warm stable\n", samples.value(), source.identity(), front.request_serial.value(), front.image_id, front.rgba)).unwrap();
            last_image = Some(front.image_id);
            values.push(front.rgba);
        }
        // E doubles before FP16 scene storage. Binary16 rounding commutes with
        // power-of-two scaling in this normal range; final tone obeys
        // T(2L)=2*T(L)/(1+T(L)). Enclose final half-ULPs and eight f32 operations.
        let gamma8 = 8.0 * 2.0_f64.powi(-24) / (1.0 - 8.0 * 2.0_f64.powi(-24));
        for k in 0..3 {
            let a = f64::from(values[0][k]);
            let b = f64::from(values[1][k]);
            let half_ulp = |v: f64| 2.0_f64.powi(v.log2().floor() as i32 - 11);
            let low = a - half_ulp(a) - gamma8;
            let high = a + half_ulp(a) + gamma8;
            let margin = half_ulp(b) + 4.0 * gamma8;
            assert!(
                (2.0 * low / (1.0 + low) - margin..=2.0 * high / (1.0 + high) + margin)
                    .contains(&b)
            );
            // The sourced frame maps BLUE127's +X toward +Z. For this r1,
            // m0, c.5 source, its positive Lambert term exceeds the old normal's.
            assert!(values[2][k] > values[0][k]);
        }
        assert!(matches!(
            viewport.pbr_front_pixel_for_test_harness(foreign, [16, 16]),
            Err(PbrFrontProofError3D::MissingTarget(_))
        ));
        assert!(matches!(
            viewport.pbr_front_pixel_for_test_harness(surface, [32, 16]),
            Err(PbrFrontProofError3D::PixelOutOfBounds { .. })
        ));
        assert!(matches!(
            renderer.registered_image_pixel_for_test_harness(
                u64::MAX,
                [0.5; 2],
                [1.0; 4],
                [1.0; 4]
            ),
            Err(TestHarnessImageProofError::MissingImage(_))
        ));
        assert!(matches!(
            renderer.registered_image_pixel_for_test_harness(
                last_image.unwrap(),
                [f32::NAN, 0.5],
                [1.0; 4],
                [1.0; 4]
            ),
            Err(TestHarnessImageProofError::NonfiniteInput)
        ));
        let mut inactive = metrics;
        inactive.activity = CustomRenderActivity::Inactive;
        viewport.prepare_custom_render(&inactive);
        assert!(
            viewport
                .pbr_front_pixel_for_test_harness(surface, [16, 16])
                .is_err()
        );
        viewport.on_unmount();
        renderer.drain_pending_image_registrations();
        assert_eq!(
            viewport.status_for(surface).unwrap().state(),
            Viewport3DLifecycleState::Retired
        );
        assert!(matches!(
            viewport.pbr_front_pixel_for_test_harness(surface, [16, 16]),
            Err(PbrFrontProofError3D::NotCurrent)
        ));
        assert!(
            renderer
                .registered_image_pixel_for_test_harness(
                    last_image.unwrap(),
                    [0.5; 2],
                    [1.0; 4],
                    [1.0; 4]
                )
                .is_err()
        );
    }
}
