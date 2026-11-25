use halcyon::core::appearance::{
    AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
    AppearanceImageSource, AppearanceSelector, AppearanceSurface, ButtonAppearanceConfiguration,
    ButtonAppearanceContract, ButtonAppearanceDefinition,
};
use halcyon::core::math::RectF;
use halcyon::core::paint::{
    Color, ColorStop, GradientDef, GradientKind, InvalidSurfaceNinePatch, NinePatchDestinationInsets,
    NinePatchSourceInsets, PaintSink, SurfaceFill, SurfaceGradient, SurfaceImage, SurfaceNinePatch,
};
use halcyon::core::scene::{ImageNode, ImageTextureSource, SceneBuilder, SceneNode};
use halcyon::core::scene_identity::SceneNodeKey;
use halcyon::core::theme::Theme;
use halcyon::widgets::button::{Button, ButtonAppearanceError};
use std::alloc::{GlobalAlloc, Layout, System};
use std::cell::Cell;

struct CountingAllocator;
thread_local! {
    static COUNT: Cell<Option<usize>> = const { Cell::new(None) };
}

unsafe impl GlobalAlloc for CountingAllocator {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        COUNT.with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        unsafe { System.dealloc(ptr, layout) }
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, size: usize) -> *mut u8 {
        COUNT.with(|count| {
            if let Some(n) = count.get() {
                count.set(Some(n + 1));
            }
        });
        unsafe { System.realloc(ptr, layout, size) }
    }
}

#[global_allocator]
static ALLOCATOR: CountingAllocator = CountingAllocator;

fn gradient() -> GradientDef {
    GradientDef {
        kind: GradientKind::Linear,
        stops: vec![
            ColorStop {
                offset: 0.0,
                color: Color::from_srgb(0.2, 0.4, 0.8, 0.5),
            },
            ColorStop {
                offset: 1.0,
                color: Color::from_srgb(0.8, 0.4, 0.2, 1.0),
            },
        ],
        p0: (0.0, 0.0),
        p1: (1.0, 1.0),
    }
}

#[test]
fn surface_fill_solid_emits_one_existing_rect_without_material_allocation() {
    let rect = RectF::new(2.0, 3.0, 40.0, 20.0);
    let color = Color::from_srgb(0.2, 0.4, 0.8, 0.5);
    let fill = SurfaceFill::Solid(color);
    let mut scene = SceneBuilder::new();
    PaintSink::push_rect(&mut scene, rect, color, 7, None);
    COUNT.with(|count| count.set(Some(0)));
    scene.push_surface_fill(rect, &fill.clone(), 7, None);
    let allocations = COUNT.with(|count| count.replace(None).unwrap());
    assert_eq!(allocations, 0);
    let scene = scene.build();
    assert_eq!(scene.root.len(), 2);
    let (SceneNode::Rect(old), SceneNode::Rect(new)) = (&scene.root[0], &scene.root[1]) else {
        panic!("Solid must use the existing Rect primitive");
    };
    assert_eq!(old.rect, new.rect);
    assert_eq!(old.color, new.color);
    assert_eq!(old.z_index, new.z_index);
    assert_eq!(old.key, new.key);
}

#[test]
fn surface_fill_gradient_is_bounded_immutable_and_uses_existing_definition() {
    let mut authored = gradient();
    let frozen = SurfaceGradient::new(authored.clone()).unwrap();
    authored.stops[0].color = Color::WHITE;
    assert_ne!(frozen.definition(), &authored);
    let clone = frozen.clone();
    assert!(std::ptr::eq(frozen.definition(), clone.definition()));
    let mut scene = SceneBuilder::new();
    scene.push_surface_fill(
        RectF::new(0.0, 0.0, 40.0, 20.0),
        &SurfaceFill::Gradient(frozen.clone()),
        3,
        None,
    );
    let scene = scene.build();
    let SceneNode::GradientRect(node) = &scene.root[0] else {
        panic!("existing GradientRect required")
    };
    assert_eq!(&node.gradient, frozen.definition());
    assert_eq!(node.radius, 0.0);
    let appearance: AppearanceSurface = SurfaceFill::Gradient(frozen);
    assert!(matches!(appearance, SurfaceFill::Gradient(_)));
}

#[test]
fn surface_fill_gradient_rejects_unrepresentable_stop_sequences_before_paint() {
    let mut value = gradient();
    value.stops.truncate(1);
    assert!(SurfaceGradient::new(value).is_err());
    let mut value = gradient();
    value.stops = vec![value.stops[0]; 9];
    assert!(SurfaceGradient::new(value).is_err());
    let mut value = gradient();
    value.stops.swap(0, 1);
    assert!(SurfaceGradient::new(value).is_err());
}

#[test]
fn surface_fill_image_preserves_current_binding_uv_and_straight_tint() {
    let tint = Color::from_srgb(0.5, 0.7, 0.9, 0.5);
    let image = SurfaceImage {
        source: 42,
        uv_rect: RectF::new(0.25, 0.0, 0.5, 1.0),
        tint,
    };
    let mut scene = SceneBuilder::new();
    scene.push_surface_fill(
        RectF::new(0.0, 0.0, 40.0, 20.0),
        &SurfaceFill::Image(image),
        3,
        None,
    );
    let scene = scene.build();
    let SceneNode::Image(node) = &scene.root[0] else {
        panic!("existing Image required")
    };
    assert_eq!(node.texture_source, ImageTextureSource::Registered(42));
    assert_eq!(node.uv_rect, image.uv_rect);
    assert_eq!(node.tint, tint.into());
    let request: AppearanceSurface = SurfaceFill::Image(SurfaceImage {
        source: AppearanceImageSource::CurrentBinding { texture_id: 42 },
        uv_rect: image.uv_rect,
        tint,
    });
    assert!(matches!(request, SurfaceFill::Image(_)));
}

fn nine_patch_images(
    material: SurfaceNinePatch,
    destination: RectF,
    key: Option<SceneNodeKey>,
) -> Vec<ImageNode> {
    let mut scene = SceneBuilder::new();
    scene.push_surface_fill(destination, &SurfaceFill::NinePatch(material), 7, key);
    scene
        .build()
        .root
        .into_iter()
        .map(|node| {
            let SceneNode::Image(image) = node else {
                panic!("NinePatch must lower to existing Image primitives");
            };
            image
        })
        .collect()
}

#[test]
fn surface_nine_patch_validation_distinguishes_malformed_classes_without_clamping() {
    let uv = RectF::new(0.0, 0.0, 1.0, 1.0);
    let source = NinePatchSourceInsets::all(0.25);
    let destination = NinePatchDestinationInsets::new(2.0, 3.0, 4.0, 5.0);
    let tint = Color::WHITE;
    let make = |uv, source, destination, tint| {
        SurfaceNinePatch::new(42_u64, uv, source, destination, tint)
    };
    let valid = make(uv, source, destination, tint).unwrap();
    assert_eq!(*valid.source(), 42);
    assert_eq!(valid.uv_rect(), uv);
    assert_eq!(valid.source_splits(), source);
    assert_eq!(valid.destination_insets(), destination);
    assert_eq!(valid.tint(), tint);
    for invalid in [
        RectF::new(-0.25, 0.0, 1.0, 1.0),
        RectF::new(0.0, 1.25, 1.0, 0.25),
        RectF::new(0.75, 0.0, 0.5, 1.0),
        RectF::new(0.0, 0.0, 0.0, 1.0),
        RectF::new(0.0, 0.0, 1.0, -1.0),
        RectF::new(f32::NAN, 0.0, 1.0, 1.0),
        RectF::new(0.0, 0.0, f32::INFINITY, 1.0),
    ] {
        assert_eq!(
            make(invalid, source, destination, tint),
            Err(InvalidSurfaceNinePatch::UvRect)
        );
    }
    for invalid in [
        NinePatchSourceInsets::new(0.0, 0.5, 0.0, 0.5),
        NinePatchSourceInsets::new(0.75, 0.0, 0.5, 0.0),
        NinePatchSourceInsets::all(-0.25),
        NinePatchSourceInsets::all(f32::NAN),
        NinePatchSourceInsets::all(f32::INFINITY),
        NinePatchSourceInsets::all(f32::MAX),
    ] {
        assert_eq!(
            make(uv, invalid, destination, tint),
            Err(InvalidSurfaceNinePatch::SourceSplits)
        );
    }
    for invalid in [
        NinePatchDestinationInsets::all(-1.0),
        NinePatchDestinationInsets::new(f32::NAN, 0.0, 0.0, 0.0),
        NinePatchDestinationInsets::all(f32::INFINITY),
    ] {
        assert_eq!(
            make(uv, source, invalid, tint),
            Err(InvalidSurfaceNinePatch::DestinationInsets)
        );
    }
    for invalid in [
        Color::from_linear(f32::NAN, 1.0, 1.0, 1.0),
        Color::from_linear(1.0, 1.0, 1.0, f32::INFINITY),
        Color::from_linear(1.25, 1.0, 1.0, 1.0),
        Color::from_linear(1.0, -0.25, 1.0, 1.0),
    ] {
        assert_eq!(
            make(uv, source, destination, invalid),
            Err(InvalidSurfaceNinePatch::Tint)
        );
    }
}

#[test]
fn surface_nine_patch_asymmetric_mapping_preserves_image_tint_z_clip_and_one_key() {
    let tint = Color::from_srgb(0.5, 0.7, 0.9, 0.5);
    let material = SurfaceNinePatch::new(
        42,
        RectF::new(0.125, 0.25, 0.5, 0.5),
        NinePatchSourceInsets::new(0.25, 0.125, 0.5, 0.25),
        NinePatchDestinationInsets::new(6.0, 14.0, 18.0, 10.0),
        tint,
    )
    .unwrap();
    let destination = RectF::new(10.0, 20.0, 80.0, 60.0);
    let key = SceneNodeKey::new(123);
    let clip_rect = RectF::new(12.0, 22.0, 70.0, 50.0);
    let mut builder = SceneBuilder::new();
    PaintSink::begin_clip(&mut builder, clip_rect, None);
    builder.push_surface_fill(destination, &SurfaceFill::NinePatch(material), 7, Some(key));
    PaintSink::end_clip(&mut builder);
    let scene = builder.build();
    assert_eq!(scene.root.len(), 1);
    let SceneNode::Clip(clip) = &scene.root[0] else {
        panic!("caller-owned inherited clip must be preserved");
    };
    assert_eq!(clip.rect, clip_rect);
    let expected = [
        (
            RectF::new(10.0, 20.0, 10.0, 6.0),
            RectF::new(0.125, 0.25, 0.125, 0.125),
        ),
        (
            RectF::new(20.0, 20.0, 56.0, 6.0),
            RectF::new(0.25, 0.25, 0.3125, 0.125),
        ),
        (
            RectF::new(76.0, 20.0, 14.0, 6.0),
            RectF::new(0.5625, 0.25, 0.0625, 0.125),
        ),
        (
            RectF::new(10.0, 26.0, 10.0, 36.0),
            RectF::new(0.125, 0.375, 0.125, 0.125),
        ),
        (
            RectF::new(20.0, 26.0, 56.0, 36.0),
            RectF::new(0.25, 0.375, 0.3125, 0.125),
        ),
        (
            RectF::new(76.0, 26.0, 14.0, 36.0),
            RectF::new(0.5625, 0.375, 0.0625, 0.125),
        ),
        (
            RectF::new(10.0, 62.0, 10.0, 18.0),
            RectF::new(0.125, 0.5, 0.125, 0.25),
        ),
        (
            RectF::new(20.0, 62.0, 56.0, 18.0),
            RectF::new(0.25, 0.5, 0.3125, 0.25),
        ),
        (
            RectF::new(76.0, 62.0, 14.0, 18.0),
            RectF::new(0.5625, 0.5, 0.0625, 0.25),
        ),
    ];
    assert_eq!(clip.children.len(), expected.len());
    for (index, (node, (rect, uv_rect))) in clip.children.iter().zip(expected).enumerate() {
        let SceneNode::Image(image) = node else {
            panic!("ordinary Image cell required");
        };
        assert_eq!(image.rect, rect);
        assert_eq!(image.uv_rect, uv_rect);
        assert_eq!(image.texture_source, ImageTextureSource::Registered(42));
        assert_eq!(image.tint, tint.into());
        assert_eq!(image.z_index, 7);
        assert_eq!(image.key, (index == 0).then_some(key));
    }
}

#[test]
fn surface_nine_patch_small_destination_collapses_independent_axes_and_preserves_equality() {
    let material = SurfaceNinePatch::new(
        42,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        NinePatchSourceInsets::all(0.25),
        NinePatchDestinationInsets::new(12.0, 10.0, 24.0, 30.0),
        Color::WHITE,
    )
    .unwrap();
    let destination = RectF::new(10.0, 20.0, 18.0, 12.0);
    let images = nine_patch_images(material, destination, None);
    let expected = [
        RectF::new(10.0, 20.0, 13.5, 4.0),
        RectF::new(23.5, 20.0, 4.5, 4.0),
        RectF::new(10.0, 24.0, 13.5, 8.0),
        RectF::new(23.5, 24.0, 4.5, 8.0),
    ];
    assert_eq!(images.len(), expected.len());
    for (index, (image, rect)) in images.iter().zip(expected).enumerate() {
        assert_eq!(image.rect, rect);
        assert!(rect.width > 0.0 && rect.height > 0.0);
        assert!(rect.x >= destination.x && rect.y >= destination.y);
        assert!(rect.right() <= destination.right() && rect.bottom() <= destination.bottom());
        for other in &images[index + 1..] {
            assert!(
                rect.right() <= other.rect.x
                    || other.rect.right() <= rect.x
                    || rect.bottom() <= other.rect.y
                    || other.rect.bottom() <= rect.y
            );
        }
    }
    let equality = nine_patch_images(material, RectF::new(0.0, 0.0, 40.0, 36.0), None);
    assert_eq!(equality.len(), 4);
    assert_eq!(equality[0].rect, RectF::new(0.0, 0.0, 30.0, 12.0));
    assert_eq!(equality[3].rect, RectF::new(30.0, 12.0, 10.0, 24.0));
    let zero_insets = SurfaceNinePatch::new(
        *material.source(),
        material.uv_rect(),
        material.source_splits(),
        NinePatchDestinationInsets::all(0.0),
        material.tint(),
    )
    .unwrap();
    let center = nine_patch_images(zero_insets, destination, None);
    assert_eq!(center.len(), 1);
    assert_eq!(center[0].rect, destination);
    assert_eq!(center[0].uv_rect, RectF::new(0.25, 0.25, 0.5, 0.5));
}

#[test]
fn surface_nine_patch_large_finite_insets_compress_without_overflow() {
    let material = SurfaceNinePatch::new(
        42,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        NinePatchSourceInsets::all(0.25),
        NinePatchDestinationInsets::new(f32::MAX, f32::MAX, f32::MAX, f32::MAX * 0.5),
        Color::WHITE,
    )
    .unwrap();
    let images = nine_patch_images(material, RectF::new(0.0, 0.0, 12.0, 20.0), None);
    let expected = [
        RectF::new(0.0, 0.0, 4.0, 10.0),
        RectF::new(4.0, 0.0, 8.0, 10.0),
        RectF::new(0.0, 10.0, 4.0, 10.0),
        RectF::new(4.0, 10.0, 8.0, 10.0),
    ];
    assert_eq!(images.len(), 4);
    for (image, rect) in images.iter().zip(expected) {
        assert_eq!(image.rect, rect);
    }
}

#[test]
fn surface_nine_patch_zero_source_strips_omit_cells_and_key_the_first_remaining_cell() {
    let material = SurfaceNinePatch::new(
        42,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        NinePatchSourceInsets::new(0.0, 0.25, 0.25, 0.0),
        NinePatchDestinationInsets::all(8.0),
        Color::WHITE,
    )
    .unwrap();
    let key = SceneNodeKey::new(123);
    let images = nine_patch_images(material, RectF::new(10.0, 20.0, 40.0, 32.0), Some(key));
    assert_eq!(images.len(), 4);
    assert_eq!(images[0].rect, RectF::new(18.0, 28.0, 24.0, 16.0));
    assert_eq!(images[0].uv_rect, RectF::new(0.0, 0.0, 0.75, 0.75));
    assert_eq!(images[0].key, Some(key));
    assert!(images[1..].iter().all(|image| image.key.is_none()));
    assert!(images.iter().all(|image| {
        image.rect.width > 0.0
            && image.rect.height > 0.0
            && image.uv_rect.width > 0.0
            && image.uv_rect.height > 0.0
    }));
}

#[test]
fn surface_nine_patch_empty_destination_emits_no_primitive_or_key() {
    let material = SurfaceNinePatch::new(
        42,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        NinePatchSourceInsets::all(0.25),
        NinePatchDestinationInsets::all(8.0),
        Color::WHITE,
    )
    .unwrap();
    assert!(
        nine_patch_images(
            material,
            RectF::new(10.0, 20.0, 0.0, 32.0),
            Some(SceneNodeKey::new(123)),
        )
        .is_empty()
    );
}

#[test]
fn surface_nine_patch_lowering_does_not_allocate_cell_storage() {
    let material = SurfaceNinePatch::new(
        42,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        NinePatchSourceInsets::all(0.25),
        NinePatchDestinationInsets::all(8.0),
        Color::WHITE,
    )
    .unwrap();
    let fill = SurfaceFill::NinePatch(material);
    let destination = RectF::new(0.0, 0.0, 40.0, 32.0);
    let mut scene = SceneBuilder::new();
    for _ in 0..2 {
        scene.push_surface_fill(destination, &fill, 7, None);
    }
    COUNT.with(|count| count.set(Some(0)));
    scene.push_surface_fill(destination, &fill, 7, None);
    let allocations = COUNT.with(|count| count.replace(None).unwrap());
    assert_eq!(allocations, 0);
    assert_eq!(scene.build().root.len(), 27);
}

#[test]
fn surface_nine_patch_appearance_value_is_valid_but_unadmitted_button_source_remains_pending() {
    let surface: AppearanceSurface = SurfaceFill::NinePatch(
        SurfaceNinePatch::new(
            AppearanceImageSource::CurrentBinding { texture_id: 42 },
            RectF::new(0.0, 0.0, 1.0, 1.0),
            NinePatchSourceInsets::all(0.25),
            NinePatchDestinationInsets::all(8.0),
            Color::WHITE,
        )
        .unwrap(),
    );
    let contract = ButtonAppearanceContract::standard();
    let selector = AppearanceSelector::button();
    let mut values = Theme::dark().button_appearance_values(&contract);
    values.resting.as_mut().unwrap().surface = surface.clone();
    let configuration = ButtonAppearanceConfiguration::new(
        AppearanceConfigurationId {
            owner: AppearanceConfigurationOwner::Framework,
            generation: 123,
        },
        contract,
        vec![ButtonAppearanceDefinition {
            selector: selector.clone(),
            values,
        }],
        vec![],
    )
    .unwrap();
    let style = configuration
        .prepare_style(selector, AppearanceFallbackPolicy::Reject)
        .unwrap();
    let mut button = Button::with_text("unadmitted NinePatch pending");
    match button.set_appearance(style, None).unwrap_err() {
        ButtonAppearanceError::PendingCapabilityAdmission { surface: rejected } => {
            assert_eq!(rejected, surface);
        }
        error => panic!("expected pending unadmitted NinePatch capability, got {error:?}"),
    }
}
