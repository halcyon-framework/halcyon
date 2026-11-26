use crate::three_d::{
    APPEARANCE_BLOCK_SIZE, Appearance3DAdmissionError, AppearanceAdmissionBudget,
    AppearanceFallbackBindingContract, AppearanceMode, AppearanceSourceDescriptor,
    AppearanceSourceRule, Context3D, Context3DError, DeviceLimits3D, GeometryIndices,
    GeometrySemantics, GeometrySourceDescriptor, LocalPoint3, NAGA_ADAPTER_VERSION,
    OBJECT_BLOCK_SIZE, PolicyBudget3D, PolicyLimits3D, ResourceKind3D, VIEW_BLOCK_SIZE,
    WGPU_ADAPTER_VERSION,
};

#[test]
fn r026_exact_eight_presence_abis_and_unchanged_pbr_qualifiers() {
    use crate::three_d::indirect::{IndirectBlock112, Presence3D, shader_source};
    assert_eq!(std::mem::size_of::<IndirectBlock112>(), 112);
    for bits in 0..8 {
        let p = Presence3D(bits);
        let source = if p.indirect() {
            shader_source(p)
        } else if p.direct() {
            crate::three_d::direct_shadow::shader_source()
        } else {
            crate::three_d::pbr::PBR_WGSL.to_owned()
        };
        let module = naga::front::wgsl::parse_str(&source)
            .unwrap_or_else(|e| panic!("profile{bits}: {}", e.emit_to_string(&source)));
        naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            if p.direct() || p.environment() {
                naga::valid::Capabilities::CUBE_ARRAY_TEXTURES
            } else {
                naga::valid::Capabilities::empty()
            },
        )
        .validate(&module)
        .unwrap_or_else(|e| panic!("profile{bits}: {}", e.emit_to_string(&source)));
        let mut actual: Vec<_> = module
            .global_variables
            .iter()
            .filter_map(|(_, g)| {
                g.binding
                    .as_ref()
                    .filter(|b| b.group == 3)
                    .map(|b| b.binding)
            })
            .collect();
        actual.sort();
        let mut expected = Vec::new();
        if p.direct() {
            expected.extend(0..6);
        }
        if p.indirect() {
            expected.push(6);
        }
        if p.environment() {
            expected.extend(7..11);
        }
        assert_eq!(actual, expected, "profile{bits}");
        let vertex = module
            .entry_points
            .iter()
            .find(|e| e.name == "pbr_vertex")
            .unwrap();
        let fragment = module
            .entry_points
            .iter()
            .find(|e| {
                e.name
                    == if p.indirect() {
                        "indirect_fragment"
                    } else if p.direct() {
                        "direct_fragment"
                    } else {
                        "pbr_fragment"
                    }
            })
            .unwrap();
        let ty = vertex.function.result.as_ref().unwrap().ty;
        assert_eq!(fragment.function.arguments[0].ty, ty);
        let naga::TypeInner::Struct { members, .. } = &module.types[ty].inner else {
            panic!()
        };
        let mut count = 0;
        for m in members {
            if let Some(naga::Binding::Location {
                location,
                interpolation,
                sampling,
                ..
            }) = m.binding
            {
                count += 1;
                if location == 5 {
                    assert!(p.direct());
                    assert_eq!(interpolation, Some(naga::Interpolation::Flat));
                } else {
                    assert_eq!(interpolation, Some(naga::Interpolation::Perspective));
                    assert_eq!(
                        sampling,
                        Some(if location == 2 {
                            naga::Sampling::Center
                        } else {
                            naga::Sampling::Centroid
                        })
                    );
                }
            }
        }
        assert_eq!(count, if p.direct() { 6 } else { 5 });
    }
}

#[test]
fn r024_fixed_material_abi_and_matching_covered_centroid_interface() {
    use crate::three_d::pbr::{MaterialBlock64, PBR_WGSL};
    let module = naga::front::wgsl::parse_str(PBR_WGSL).unwrap();
    naga::valid::Validator::new(
        naga::valid::ValidationFlags::all(),
        naga::valid::Capabilities::empty(),
    )
    .validate(&module)
    .unwrap();
    let vertex = module
        .entry_points
        .iter()
        .find(|e| e.name == "pbr_vertex")
        .unwrap();
    let fragment = module
        .entry_points
        .iter()
        .find(|e| e.name == "pbr_fragment")
        .unwrap();
    let ty = vertex.function.result.as_ref().unwrap().ty;
    assert_eq!(
        fragment.function.arguments[0].ty, ty,
        "same exact VS output and FS input declaration"
    );
    let naga::TypeInner::Struct { members, .. } = &module.types[ty].inner else {
        panic!("interstage struct")
    };
    let mut locations = Vec::new();
    for m in members {
        if let Some(naga::Binding::Location {
            location,
            interpolation,
            sampling,
            ..
        }) = m.binding
        {
            assert_eq!(interpolation, Some(naga::Interpolation::Perspective));
            assert_eq!(
                sampling,
                Some(if location == 2 {
                    naga::Sampling::Center
                } else {
                    naga::Sampling::Centroid
                })
            );
            locations.push(location);
        }
    }
    assert_eq!(locations, [0, 1, 2, 3, 4]);
    let material = module
        .types
        .iter()
        .find(|(_, t)| t.name.as_deref() == Some("Material64"))
        .unwrap()
        .1;
    let naga::TypeInner::Struct { members, span } = &material.inner else {
        panic!("Material64")
    };
    assert_eq!(*span, 64);
    assert_eq!(
        members.iter().map(|m| m.offset).collect::<Vec<_>>(),
        [0, 16, 32, 48]
    );
    assert_eq!(std::mem::size_of::<MaterialBlock64>(), 64);
    assert_eq!(
        module
            .global_variables
            .iter()
            .filter_map(|(_, v)| v.binding.as_ref())
            .filter(|b| b.group == 2)
            .map(|b| b.binding)
            .collect::<Vec<_>>(),
        (0..=10).collect::<Vec<_>>()
    );
    assert!(
        module
            .global_variables
            .iter()
            .filter_map(|(_, v)| v.binding.as_ref())
            .all(|b| b.group < 3)
    );
}

const TEXTURED_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    let sampled = textureSample(appearance_texture, appearance_sampler, uv);
    return sampled.rgb * vertex_color_linear.rgb * color_factor_linear.rgb;
}
"#;

const MASK_SOURCE: &str = r#"
const MASK_CUTOFF: f32 = 0.375;

fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    let sampled = textureSample(appearance_texture, appearance_sampler, uv);
    if sampled.a < MASK_CUTOFF {
        discard;
    }
    return sampled.rgb * color_factor_linear.rgb;
}
"#;

const SANITIZER_SOURCE: &str = r#"
fn halcyon_appearance(
    position_render_relative: vec3<f32>,
    normal_world_interpolated: vec3<f32>,
    view_direction_world: vec3<f32>,
    uv: vec2<f32>,
    vertex_color_linear: vec4<f32>,
    front_facing: bool,
    color_factor_linear: vec4<f32>,
    appearance_texture: texture_2d<f32>,
    appearance_sampler: sampler,
) -> vec3<f32> {
    return vec3<f32>(2.0, -0.25, 0.5);
}
"#;

const TEXTURED_SEMANTICS: GeometrySemantics = GeometrySemantics::new(false, true, true);
const UV_SEMANTICS: GeometrySemantics = GeometrySemantics::new(false, true, false);

fn cpu_context(policy: PolicyLimits3D) -> Context3D {
    Context3D::new_cpu_only_for_test(policy, u64::MAX).unwrap()
}

#[test]
fn actual_hdr_tone_and_appearance_modules_share_one_capacity_and_raii_in_both_orders() {
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::three_d::hdr::{HdrError3D, ToneProgram3D};
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R027: actual Device required");
    println!(
        "R027 shared module Device: {:?}",
        renderer.headless_adapter_for_test()
    );
    for tone_first in [false, true] {
        let mut policy = PolicyLimits3D::conservative();
        policy.max_shader_modules = 3;
        let context = Context3D::with_policy(renderer.gpu_resources(), policy).unwrap();
        let tone = tone_first.then(|| ToneProgram3D::new(&context, 1).unwrap());
        assert_eq!(
            context.resource_usage().unwrap().admitted_shader_modules,
            u32::from(tone_first)
        );
        let source = context
            .register_appearance(descriptor(
                TEXTURED_SOURCE,
                TEXTURED_SEMANTICS,
                AppearanceMode::Opaque,
            ))
            .unwrap();
        let appearance = context
            .admit_appearance_for_test(source.handle(), TEXTURED_SEMANTICS)
            .unwrap();
        let actual_modules = appearance
            .materialize_shader_modules(&renderer.gpu_resources().device)
            .unwrap();
        let tone = tone.or_else(|| Some(ToneProgram3D::new(&context, 1).unwrap()));
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 3);
        let error = ToneProgram3D::new(&context, 4)
            .err()
            .expect("distinct N4 module cannot fit");
        assert!(matches!(
            error,
            HdrError3D::Context(Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ShaderModules,
                requested: 4,
                maximum: 3
            })
        ));
        let another = context
            .register_appearance(descriptor(
                SANITIZER_SOURCE,
                GeometrySemantics::NONE,
                AppearanceMode::Opaque,
            ))
            .unwrap();
        assert_eq!(
            context
                .admit_appearance_for_test(another.handle(), GeometrySemantics::NONE)
                .unwrap_err(),
            Context3DError::PolicyBudgetExceeded {
                budget: PolicyBudget3D::ShaderModules,
                requested: 4,
                maximum: 3
            }
        );
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 3);
        drop(tone);
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 2);
        let next = context
            .admit_appearance_for_test(another.handle(), GeometrySemantics::NONE)
            .unwrap();
        let next_actual = next
            .materialize_shader_modules(&renderer.gpu_resources().device)
            .unwrap();
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 3);
        drop((
            next_actual,
            actual_modules,
            next,
            appearance,
            another,
            source,
        ));
    }
    // An N1 and an N4 program are two actual retained module generations, not
    // two pipelines hidden behind one nominal fixture count.
    let context =
        Context3D::with_policy(renderer.gpu_resources(), PolicyLimits3D::conservative()).unwrap();
    let one = ToneProgram3D::new(&context, 1).unwrap();
    let four = ToneProgram3D::new(&context, 4).unwrap();
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 2);
    drop(one);
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 1);
    drop(four);
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 0);
}

#[test]
fn concurrent_actual_tone_admissions_cannot_overbook_shared_shader_capacity() {
    use crate::renderer::wgpu_renderer::WgpuRenderer;
    use crate::three_d::hdr::{HdrError3D, ToneProgram3D};
    use std::sync::{Arc, Barrier, mpsc};
    let renderer = pollster::block_on(WgpuRenderer::new_headless())
        .expect("UNVERIFIED R027: actual Device required");
    let mut policy = PolicyLimits3D::conservative();
    policy.max_shader_modules = 3;
    let context = Arc::new(Context3D::with_policy(renderer.gpu_resources(), policy).unwrap());
    let barrier = Arc::new(Barrier::new(5));
    let (send, receive) = mpsc::channel();
    std::thread::scope(|scope| {
        for _ in 0..4 {
            let (context, barrier, send) = (context.clone(), barrier.clone(), send.clone());
            scope.spawn(move || {
                let program = ToneProgram3D::new(&context, 1);
                send.send(match &program {
                    Ok(_) => true,
                    Err(HdrError3D::Context(Context3DError::PolicyBudgetExceeded {
                        budget: PolicyBudget3D::ShaderModules,
                        requested: 4,
                        maximum: 3,
                    })) => false,
                    Err(error) => panic!("unexpected actual module admission: {error}"),
                })
                .unwrap();
                barrier.wait();
                drop(program);
            });
        }
        let successes = (0..4).filter(|_| receive.recv().unwrap()).count();
        assert_eq!(successes, 3);
        assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 3);
        barrier.wait();
    });
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 0);
}

fn descriptor(
    source: &str,
    required_semantics: GeometrySemantics,
    mode: AppearanceMode,
) -> AppearanceSourceDescriptor {
    AppearanceSourceDescriptor::try_new_with_mode(source, required_semantics, mode).unwrap()
}

fn admission_error(
    source: &str,
    required_semantics: GeometrySemantics,
    mode: AppearanceMode,
) -> Context3DError {
    let context = cpu_context(PolicyLimits3D::conservative());
    let appearance = context
        .register_appearance(descriptor(source, required_semantics, mode))
        .unwrap();
    context
        .admit_appearance_for_test(appearance.handle(), required_semantics)
        .unwrap_err()
}

#[test]
fn abi_sizes_dynamic_strides_and_fallback_binding_are_exact() {
    let mut hard = DeviceLimits3D::baseline_for_test();
    hard.min_uniform_buffer_offset_alignment = 64;
    let context =
        Context3D::new_cpu_only_with_hard_for_test(PolicyLimits3D::conservative(), hard, u64::MAX)
            .unwrap();
    let layout = context.abi_layout();

    assert_eq!(VIEW_BLOCK_SIZE, 80);
    assert_eq!(OBJECT_BLOCK_SIZE, 112);
    assert_eq!(APPEARANCE_BLOCK_SIZE, 16);
    assert_eq!(layout.view_block_size(), 80);
    assert_eq!(layout.object_block_size(), 112);
    assert_eq!(layout.appearance_block_size(), 16);
    assert_eq!(layout.view_dynamic_stride(), 128);
    assert_eq!(layout.object_dynamic_stride(), 128);
    assert_eq!(layout.appearance_dynamic_stride(), 64);
    assert_eq!(layout.min_uniform_buffer_offset_alignment(), 64);

    let fallback = AppearanceFallbackBindingContract::OPAQUE_WHITE;
    assert_eq!(fallback.extent(), [1, 1]);
    assert_eq!(fallback.linear_rgba8(), [255, 255, 255, 255]);
    assert!(fallback.is_filterable_float_2d());
    assert!(fallback.uses_filtering_sampler());
}

#[test]
fn valid_adapter_preserves_spans_roundtrips_and_constructs_direct_naga_sources() {
    let context = cpu_context(PolicyLimits3D::conservative());
    let appearance = context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let admitted = context
        .admit_appearance_for_test(appearance.handle(), TEXTURED_SEMANTICS)
        .unwrap();

    assert_eq!(WGPU_ADAPTER_VERSION, "30.0.2");
    assert_eq!(NAGA_ADAPTER_VERSION, "30.0.2");
    let manifest = include_str!("../../Cargo.toml");
    assert!(manifest.contains("wgpu = { version = \"=30.0.2\", features = [\"naga-ir\"] }"));
    assert!(
        manifest
            .contains("naga = { version = \"=30.0.2\", features = [\"wgsl-in\", \"wgsl-out\"] }")
    );
    assert_eq!(admitted.handle(), appearance.handle());
    assert_eq!(admitted.required_semantics(), TEXTURED_SEMANTICS);
    assert_eq!(admitted.mode(), AppearanceMode::Opaque);
    assert!(admitted.source_ir_nodes() > 0);
    assert!(admitted.final_ir_nodes() > admitted.source_ir_nodes());
    assert!(admitted.canonical_vertex_ir_nodes() > 0);
    assert!(admitted.source_spans_preserved());
    assert!(admitted.synthetic_spans_are_undefined());
    assert_eq!(admitted.abi_layout(), context.abi_layout());
    assert_eq!(
        admitted.fallback(),
        AppearanceFallbackBindingContract::OPAQUE_WHITE
    );
    assert!(admitted.constructs_direct_naga_sources());
    admitted.validate_roundtrip().unwrap();
    assert_eq!(context.admitted_shader_module_count_for_test(), 2);
    assert_eq!(context.resource_usage().unwrap().admitted_shader_modules, 2);
}

#[test]
fn emitted_wgsl_preserves_the_controlled_entry_and_binding_abi() {
    let context = cpu_context(PolicyLimits3D::conservative());
    let appearance = context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let admitted = context
        .admit_appearance_for_test(appearance.handle(), TEXTURED_SEMANTICS)
        .unwrap();
    let (vertex, fragment) = admitted.emitted_shader_sources_for_test().unwrap();
    let vertex_abi = [(0, 0, VIEW_BLOCK_SIZE), (1, 0, OBJECT_BLOCK_SIZE)];
    let fragment_abi = [(0, 0, VIEW_BLOCK_SIZE), (2, 0, APPEARANCE_BLOCK_SIZE)];
    for (emitted, stage, argument_count, uniform_abi, resource_count) in [
        (vertex, naga::ShaderStage::Vertex, 4, vertex_abi.as_slice(), 2),
        (
            fragment,
            naga::ShaderStage::Fragment,
            5,
            fragment_abi.as_slice(),
            4,
        ),
    ] {
        let reparsed = naga::front::wgsl::parse_str(&emitted).unwrap();
        let entry = &reparsed.entry_points[0];
        assert_eq!(reparsed.entry_points.len(), 1);
        assert_eq!(entry.stage, stage);
        assert!(!entry.name.starts_with("__"));
        assert_eq!(entry.function.arguments.len(), argument_count);
        assert_eq!(reparsed.global_variables.len(), resource_count);
        let mut layout = naga::proc::Layouter::default();
        layout.update(reparsed.to_ctx()).unwrap();
        for &(group, binding, size) in uniform_abi {
            let uniform = reparsed
                .global_variables
                .iter()
                .map(|(_, global)| global)
                .find(|global| global.binding == Some(naga::ResourceBinding { group, binding }))
                .unwrap();
            assert_eq!(uniform.space, naga::AddressSpace::Uniform);
            assert_eq!(u64::from(layout[uniform.ty].size), size);
        }
        if entry.stage == naga::ShaderStage::Fragment {
            assert!(matches!(
                entry.function.result.as_ref().unwrap().binding,
                Some(naga::Binding::Location { location: 0, .. })
            ));
            assert!(emitted.contains("bitcast<f32>(2139095039u)"));
        }
    }
}

#[test]
fn malformed_source_reports_application_location() {
    let malformed = "fn halcyon_appearance(\n    position_render_relative: vec3<f32>\n";
    let error = admission_error(malformed, GeometrySemantics::NONE, AppearanceMode::Opaque);
    let Context3DError::AppearanceAdmission(Appearance3DAdmissionError::SourceParse { diagnostic }) =
        error
    else {
        panic!("unexpected error: {error:?}");
    };
    let location = diagnostic.primary_location().unwrap();
    assert!(location.line_number() >= 1);
    assert!((location.offset() as usize) <= malformed.len());
    assert!(!diagnostic.message().is_empty());
}

#[test]
fn structural_allowlist_rejects_malicious_source_forms() {
    let cases = vec![
        (
            format!(
                "{TEXTURED_SOURCE}\n@fragment fn hostile() -> @location(0) vec4<f32> {{ return vec4<f32>(0.0); }}"
            ),
            AppearanceSourceRule::EntryPoint,
        ),
        (
            format!("var<private> hostile: f32;\n{TEXTURED_SOURCE}"),
            AppearanceSourceRule::ModuleScopeVariable,
        ),
        (
            format!("override hostile: f32 = 1.0;\n{TEXTURED_SOURCE}"),
            AppearanceSourceRule::Override,
        ),
        (
            format!("fn __halcyon355_hostile() {{}}\n{TEXTURED_SOURCE}"),
            AppearanceSourceRule::ReservedIdentifier,
        ),
        (
            format!(
                "fn hostile(value: texture_storage_2d<rgba8unorm, write>) {{}}\n{TEXTURED_SOURCE}"
            ),
            AppearanceSourceRule::ForbiddenType,
        ),
        (
            format!("fn hostile() {{ workgroupBarrier(); }}\n{TEXTURED_SOURCE}"),
            AppearanceSourceRule::ForbiddenStatement,
        ),
        (
            format!(
                "fn hostile(t: texture_2d<f32>) -> vec4<f32> {{ return textureLoad(t, vec2<i32>(0), 0); }}\n{TEXTURED_SOURCE}"
            ),
            AppearanceSourceRule::ForbiddenExpression,
        ),
        (
            format!("enable f16;\n{TEXTURED_SOURCE}"),
            AppearanceSourceRule::OptionalExtension,
        ),
        (
            format!(
                "enable f16;\nfn hostile(value: f16) -> f16 {{ return value; }}\n{TEXTURED_SOURCE}"
            ),
            AppearanceSourceRule::OptionalExtension,
        ),
    ];

    for (source, expected_rule) in cases {
        let error = admission_error(&source, TEXTURED_SEMANTICS, AppearanceMode::Opaque);
        if expected_rule == AppearanceSourceRule::ReservedIdentifier
            && matches!(
                error,
                Context3DError::AppearanceAdmission(Appearance3DAdmissionError::SourceParse { .. })
            )
        {
            continue;
        }
        assert!(
            matches!(
                error,
                Context3DError::AppearanceAdmission(
                    Appearance3DAdmissionError::SourceRejected { rule, .. }
                ) if rule == expected_rule
            ),
            "unexpected result for {expected_rule:?}: {error:?}"
        );
    }

    let wrong_signature = "fn halcyon_appearance() -> vec3<f32> { return vec3<f32>(0.0); }";
    assert!(matches!(
        admission_error(
            wrong_signature,
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ),
        Context3DError::AppearanceAdmission(Appearance3DAdmissionError::SourceRejected {
            rule: AppearanceSourceRule::RequiredFunctionSignature,
            ..
        })
    ));
}

#[test]
fn extension_keywords_inside_comments_are_not_directives() {
    let source = format!(
        "// enable f16;\n/* requires readonly_and_readwrite_storage_textures; */\n{TEXTURED_SOURCE}"
    );
    let context = cpu_context(PolicyLimits3D::conservative());
    let appearance = context
        .register_appearance(descriptor(
            &source,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    context
        .admit_appearance_for_test(appearance.handle(), TEXTURED_SEMANTICS)
        .unwrap();
}

#[test]
fn invalid_handles_fail_before_compiler_or_cache_work() {
    let context = cpu_context(PolicyLimits3D::conservative());
    let geometry = context
        .register_geometry(
            GeometrySourceDescriptor::try_new(
                vec![
                    LocalPoint3::try_new(0.0, 0.0, 0.0).unwrap(),
                    LocalPoint3::try_new(1.0, 0.0, 0.0).unwrap(),
                    LocalPoint3::try_new(0.0, 1.0, 0.0).unwrap(),
                ],
                None,
                None,
                None,
                GeometryIndices::Unindexed,
            )
            .unwrap(),
        )
        .unwrap();
    let wrong_kind = Context3D::wrong_kind_appearance_handle_for_test(geometry.handle());
    assert_eq!(
        context
            .admit_appearance_for_test(&wrong_kind, GeometrySemantics::NONE)
            .unwrap_err(),
        Context3DError::WrongResourceKind {
            expected: ResourceKind3D::Appearance,
            actual: ResourceKind3D::Geometry,
        }
    );

    let malformed = context
        .register_appearance(descriptor(
            "not valid WGSL",
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let missing_slot =
        Context3D::appearance_handle_with_slot_for_test(malformed.handle(), u32::MAX);
    assert_eq!(
        context
            .admit_appearance_for_test(&missing_slot, GeometrySemantics::NONE)
            .unwrap_err(),
        Context3DError::SlotOutOfRange { slot: u32::MAX }
    );
    assert_eq!(context.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn declared_and_available_geometry_semantics_are_both_enforced_before_cache_publish() {
    let undeclared = admission_error(
        TEXTURED_SOURCE,
        GeometrySemantics::NONE,
        AppearanceMode::Opaque,
    );
    assert!(matches!(
        undeclared,
        Context3DError::AppearanceAdmission(
            Appearance3DAdmissionError::UndeclaredSemanticRead {
                used,
                declared: GeometrySemantics::NONE,
            }
        ) if used == TEXTURED_SEMANTICS
    ));

    let context = cpu_context(PolicyLimits3D::conservative());
    let malformed = context
        .register_appearance(descriptor(
            "not valid WGSL",
            UV_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    assert_eq!(
        context
            .admit_appearance_for_test(malformed.handle(), GeometrySemantics::NONE)
            .unwrap_err(),
        Context3DError::MissingGeometrySemantics {
            required: UV_SEMANTICS,
            available: GeometrySemantics::NONE,
        }
    );
    assert_eq!(context.admitted_shader_module_count_for_test(), 0);
    assert!(matches!(
        context
            .admit_appearance_for_test(malformed.handle(), UV_SEMANTICS)
            .unwrap_err(),
        Context3DError::AppearanceAdmission(Appearance3DAdmissionError::SourceParse { .. })
    ));
    assert_eq!(context.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn discard_is_alpha_mask_only() {
    assert_eq!(
        admission_error(MASK_SOURCE, UV_SEMANTICS, AppearanceMode::Opaque),
        Context3DError::AppearanceAdmission(Appearance3DAdmissionError::OpaqueModeDiscards)
    );

    let context = cpu_context(PolicyLimits3D::conservative());
    let appearance = context
        .register_appearance(descriptor(
            MASK_SOURCE,
            UV_SEMANTICS,
            AppearanceMode::AlphaMask,
        ))
        .unwrap();
    let admitted = context
        .admit_appearance_for_test(appearance.handle(), UV_SEMANTICS)
        .unwrap();
    assert_eq!(admitted.mode(), AppearanceMode::AlphaMask);
    let (emitted, entry_may_kill) = admitted.fragment_wrapper_proof().unwrap();
    assert!(entry_may_kill);
    assert!(emitted.contains("MASK_CUTOFF"));
    assert!(emitted.contains("0.375"));
    assert!(emitted.contains("discard;"));
    admitted.validate_roundtrip().unwrap();
}

#[test]
fn wrapper_enforces_binding_rgb_sanitization() {
    let context = cpu_context(PolicyLimits3D::conservative());
    let appearance = context
        .register_appearance(descriptor(
            SANITIZER_SOURCE,
            GeometrySemantics::NONE,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let admitted = context
        .admit_appearance_for_test(appearance.handle(), GeometrySemantics::NONE)
        .unwrap();

    let (emitted, entry_may_kill) = admitted.fragment_wrapper_proof().unwrap();
    assert!(!entry_may_kill);
    assert!(emitted.contains("select("));
    assert!(emitted.contains("clamp("));
    admitted.validate_roundtrip().unwrap();
}

#[test]
fn source_function_nesting_ir_and_module_budgets_are_transactional() {
    let mut source_bytes = PolicyLimits3D::conservative();
    source_bytes.max_appearance_source_bytes = (TEXTURED_SOURCE.len() - 1) as u64;
    let source_context = cpu_context(source_bytes);
    assert!(matches!(
        source_context.register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        )),
        Err(Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::AppearanceSourceBytes,
            ..
        })
    ));

    let mut functions = PolicyLimits3D::conservative();
    functions.max_appearance_functions = 1;
    let function_context = cpu_context(functions);
    let appearance = function_context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    assert!(matches!(
        function_context
            .admit_appearance_for_test(appearance.handle(), TEXTURED_SEMANTICS)
            .unwrap_err(),
        Context3DError::AppearanceAdmission(Appearance3DAdmissionError::BudgetExceeded {
            budget: AppearanceAdmissionBudget::Functions,
            requested: 2,
            maximum: 1,
        })
    ));

    let mut nesting = PolicyLimits3D::conservative();
    nesting.max_appearance_nesting = 1;
    let nesting_context = cpu_context(nesting);
    let appearance = nesting_context
        .register_appearance(descriptor(
            MASK_SOURCE,
            UV_SEMANTICS,
            AppearanceMode::AlphaMask,
        ))
        .unwrap();
    assert!(matches!(
        nesting_context
            .admit_appearance_for_test(appearance.handle(), UV_SEMANTICS)
            .unwrap_err(),
        Context3DError::AppearanceAdmission(Appearance3DAdmissionError::BudgetExceeded {
            budget: AppearanceAdmissionBudget::Nesting,
            ..
        })
    ));

    let mut ir = PolicyLimits3D::conservative();
    ir.max_appearance_ir_nodes = 1;
    let ir_context = cpu_context(ir);
    let appearance = ir_context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    assert!(matches!(
        ir_context
            .admit_appearance_for_test(appearance.handle(), TEXTURED_SEMANTICS)
            .unwrap_err(),
        Context3DError::AppearanceAdmission(Appearance3DAdmissionError::BudgetExceeded {
            budget: AppearanceAdmissionBudget::IrNodes,
            ..
        })
    ));

    let mut modules = PolicyLimits3D::conservative();
    modules.max_shader_modules = 1;
    let module_context = cpu_context(modules);
    let appearance = module_context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    assert_eq!(
        module_context
            .admit_appearance_for_test(appearance.handle(), TEXTURED_SEMANTICS)
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::ShaderModules,
            requested: 2,
            maximum: 1,
        }
    );
    assert_eq!(module_context.admitted_shader_module_count_for_test(), 0);
}

#[test]
fn admission_cache_is_generation_and_context_keyed() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_shader_modules = 3;
    let context = cpu_context(policy);
    let first_resource = context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let first_handle = first_resource.handle().clone();
    let first = context
        .admit_appearance_for_test(&first_handle, TEXTURED_SEMANTICS)
        .unwrap();
    let first_again = context
        .admit_appearance_for_test(&first_handle, TEXTURED_SEMANTICS)
        .unwrap();
    assert!(first.same_admitted_module(&first_again));
    assert!(first.same_canonical_vertex_module(&first_again));
    assert_eq!(context.admitted_shader_module_count_for_test(), 2);

    let second_resource = context
        .replace_appearance(
            &first_handle,
            descriptor(TEXTURED_SOURCE, TEXTURED_SEMANTICS, AppearanceMode::Opaque),
        )
        .unwrap();
    let second = context
        .admit_appearance_for_test(second_resource.handle(), TEXTURED_SEMANTICS)
        .unwrap();
    assert!(!first.same_admitted_module(&second));
    assert!(first.same_canonical_vertex_module(&second));
    assert_eq!(first.source_hash(), second.source_hash());
    assert_ne!(first.handle().generation(), second.handle().generation());
    assert_eq!(context.admitted_shader_module_count_for_test(), 3);

    assert!(matches!(
        context
            .admit_appearance_for_test(&first_handle, TEXTURED_SEMANTICS)
            .unwrap_err(),
        Context3DError::StaleGeneration { .. }
    ));
    let sibling = cpu_context(policy);
    assert_eq!(
        sibling
            .admit_appearance_for_test(&first_handle, TEXTURED_SEMANTICS)
            .unwrap_err(),
        Context3DError::ContextMismatch
    );
    assert_eq!(sibling.admitted_shader_module_count_for_test(), 0);

    drop(first);
    drop(first_again);
    drop(first_resource);
    let _ = context.resource_usage().unwrap();
    assert_eq!(context.admitted_shader_module_count_for_test(), 2);
}

#[test]
fn retained_generation_module_budget_failure_publishes_no_cache_and_can_retry() {
    let mut policy = PolicyLimits3D::conservative();
    policy.max_shader_modules = 2;
    let context = cpu_context(policy);
    let first_resource = context
        .register_appearance(descriptor(
            TEXTURED_SOURCE,
            TEXTURED_SEMANTICS,
            AppearanceMode::Opaque,
        ))
        .unwrap();
    let first = context
        .admit_appearance_for_test(first_resource.handle(), TEXTURED_SEMANTICS)
        .unwrap();
    let second_resource = context
        .replace_appearance(
            first_resource.handle(),
            descriptor(MASK_SOURCE, UV_SEMANTICS, AppearanceMode::AlphaMask),
        )
        .unwrap();

    assert_eq!(
        context
            .admit_appearance_for_test(second_resource.handle(), UV_SEMANTICS)
            .unwrap_err(),
        Context3DError::PolicyBudgetExceeded {
            budget: PolicyBudget3D::ShaderModules,
            requested: 3,
            maximum: 2,
        }
    );
    assert_eq!(context.admitted_shader_module_count_for_test(), 2);

    drop(first);
    drop(first_resource);
    assert_eq!(context.admitted_shader_module_count_for_test(), 1);
    let second = context
        .admit_appearance_for_test(second_resource.handle(), UV_SEMANTICS)
        .unwrap();
    assert_eq!(second.mode(), AppearanceMode::AlphaMask);
    assert_eq!(context.admitted_shader_module_count_for_test(), 2);
}
