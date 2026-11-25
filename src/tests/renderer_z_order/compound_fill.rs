use crate::core::icon::IconId;
use crate::core::icons::builtin_registry;
use crate::core::math::RectF;
use crate::core::paint::{Color, PaintSink};
use crate::renderer::path::{FillRule, Path, StrokeCap, StrokeJoin, StrokeParams};

pub(super) const WIDTH: u32 = 160;
pub(super) const HEIGHT: u32 = 80;
pub(super) const SCALES: [f32; 3] = [1.0, 2.0, 1.25];

const RED: [u8; 4] = [188, 0, 0, 255];
const BLUE: [u8; 4] = [0, 0, 255, 255];
const BLACK: [u8; 4] = [0, 0, 0, 255];

pub(super) const SAMPLES: &[(&str, [f32; 2], [u8; 4])] = &[
    ("CHECK left arm", [24.0, 43.0], RED),
    ("CHECK right arm", [42.0, 36.0], RED),
    ("CHECK overlap", [30.0, 49.0], RED),
    ("CHECK wedge", [30.0, 36.0], BLACK),
    ("NonZero left", [70.0, 10.0], RED),
    ("NonZero overlap", [84.0, 20.0], RED),
    ("NonZero right", [98.0, 28.0], RED),
    ("EvenOdd left", [114.0, 10.0], RED),
    ("EvenOdd overlap", [128.0, 20.0], BLACK),
    ("EvenOdd right", [142.0, 28.0], RED),
    ("NonZero outer", [70.0, 54.0], RED),
    ("NonZero hole", [84.0, 56.0], BLACK),
    ("simple path outside clip", [115.0, 54.0], BLACK),
    ("simple path inside clip", [123.0, 54.0], RED),
    ("higher-Z rectangle", [135.0, 55.0], BLUE),
    ("stroke interior", [16.0, 63.0], RED),
];

fn rectangle(path: &mut Path, rect: RectF, reverse: bool) {
    let mut corners = [
        [rect.x, rect.y],
        [rect.x + rect.width, rect.y],
        [rect.x + rect.width, rect.y + rect.height],
        [rect.x, rect.y + rect.height],
    ];
    if reverse {
        corners.reverse();
    }
    path.move_to(corners[0][0], corners[0][1]);
    for [x, y] in corners.into_iter().skip(1) {
        path.line_to(x, y);
    }
    path.close();
}

pub(super) fn paint_fixture(scene: &mut dyn PaintSink) {
    let tint = Color::from_linear(1.0, 0.0, 0.0, 0.5);
    scene.push_rect(
        RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32),
        Color::BLACK,
        0,
        None,
    );
    let registry = builtin_registry();
    let check = registry.get(IconId::CHECK).expect("registered CHECK");
    scene.push_icon(check, 0.0, 0.0, 72.0, tint, 10);

    for (x, rule) in [(66.0, FillRule::NonZero), (110.0, FillRule::EvenOdd)] {
        let mut path = Path::new();
        rectangle(&mut path, RectF::new(x, 6.0, 24.0, 20.0), false);
        rectangle(&mut path, RectF::new(x + 14.0, 14.0, 24.0, 20.0), false);
        scene
            .push_path_with_fill_rule(path, rule, tint, 10, None)
            .unwrap();
    }

    let mut ring = Path::new();
    rectangle(&mut ring, RectF::new(66.0, 44.0, 38.0, 25.0), false);
    rectangle(&mut ring, RectF::new(76.0, 49.0, 19.0, 14.0), true);
    scene.push_path(ring, tint, 10, None);

    scene.push_rect(
        RectF::new(131.0, 51.0, 10.0, 9.0),
        Color::from_linear(0.0, 0.0, 1.0, 1.0),
        20,
        None,
    );
    scene.begin_clip(RectF::new(119.0, 43.0, 30.0, 25.0), None);
    scene.push_path(
        Path::rounded_rect(112.0, 44.0, 36.0, 22.0, 5.0),
        tint,
        10,
        None,
    );
    scene.end_clip();

    let mut stroke = Path::new();
    stroke.move_to(8.0, 69.0);
    stroke.line_to(23.0, 58.0);
    stroke.line_to(34.0, 67.0);
    scene.push_stroked_path(
        stroke,
        StrokeParams {
            width: 5.0,
            join: StrokeJoin::Bevel,
            cap: StrokeCap::Round,
            ..StrokeParams::default()
        },
        tint,
        10,
        None,
    );
}

#[cfg(target_os = "macos")]
#[test]
fn compound_fill_metal_pixels_use_the_ordinary_renderer() {
    use super::{pixel_at, render_scene_at_scale, setup_with_size_and_scale};
    use crate::core::scene::{SceneBuilder, SceneNode};

    for scale in SCALES {
        let (mut renderer, compositor, surface) =
            setup_with_size_and_scale(WIDTH, HEIGHT, scale).expect("Metal renderer is required");
        let adapter = renderer.gpu_resources().adapter.get_info();
        assert_eq!(adapter.backend, wgpu::Backend::Metal);
        let mut builder = SceneBuilder::with_scale_factor(scale);
        paint_fixture(&mut builder);
        let scene = builder.build();
        let SceneNode::Path(check) = &scene.root[1] else {
            panic!("the registered CHECK must emit an ordinary path");
        };
        assert!(check.mesh.is_none() && !check.is_stroke);
        assert_eq!(check.fill_rule, FillRule::NonZero);

        let pixels = render_scene_at_scale(&mut renderer, &compositor, surface, scene, scale);
        let width = (WIDTH as f32 * scale) as u32;
        for &(name, [x, y], expected) in SAMPLES {
            let actual = pixel_at(&pixels, width, (x * scale) as u32, (y * scale) as u32);
            assert!(
                actual
                    .into_iter()
                    .zip(expected)
                    .all(|(a, b)| a.abs_diff(b) <= 2),
                "{name} at {scale}x: expected {expected:?}, got {actual:?}",
            );
            println!("Metal {scale}x {name}: {actual:?}");
        }
    }
}
