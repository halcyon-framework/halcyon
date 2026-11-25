use super::*;
use crate::compositor::{Compositor, SceneKey};
use crate::core::{
    math::Point,
    paint::{AnalyticSurfaceLight, Color, GlassSurfaceOpticsParameters},
    scene::{SceneBuilder, SceneGraph},
};
use crate::renderer::{DrawPlanBuilder, draw_plan_builder::DrawPlanBuildError, mesh_cache::MeshCache};
use std::{collections::HashMap, mem::{offset_of, size_of}};

pub(crate) fn crossing(phase: f32, sigma: f32, mix: f32) -> GlassSurfaceOptics {
    let light = |x, rgb: [f32; 3]| {
        AnalyticSurfaceLight::try_new(
            [0.0, 0.0, 1.0], Color::from_linear(rgb[0], rgb[1], rgb[2], 1.0), 0.40,
            Point::new(x, 0.5), Point::new(0.20, 0.25), Point::new(1.0, 0.0),
        ).unwrap()
    };
    GlassSurfaceOptics::try_new(
        GlassSurfaceOpticsParameters {
            bevel_width: 1.75, bevel_slope: 0.94, roughness: 0.28,
            normal_reflectance: 0.50, reflection_strength: 1.0, rim_strength: 0.0, rim_falloff: 1.5,
        },
        [light(0.2 + 0.6 * phase, [1.0, 0.25, 0.10]), light(0.8 - 0.6 * phase, [0.08, 0.35, 1.0])],
        Color::WHITE, Color::WHITE,
    ).unwrap().with_bloom(Some(SurfaceBloom::try_new(8.0, sigma, mix).unwrap()))
}

fn excess(y: f64) -> f64 {
    (y - 1.0).max((y - 0.75).clamp(0.0, 0.5).powi(2))
}

pub(crate) fn extraction(optics: &GlassSurfaceOptics, uv: [f64; 2], distance: f64, gradient: [f64; 2])
    -> (f64, [f64; 3])
{
    let params = optics.parameters();
    let edge = (1.0 - (-distance).max(0.0) / f64::from(params.bevel_width)).clamp(0.0, 1.0);
    let slope = edge * edge * (3.0 - 2.0 * edge) * f64::from(params.bevel_slope);
    let normal = [slope * gradient[0], slope * gradient[1], (1.0 - slope * slope).sqrt()];
    let mut rgb = [0.0; 3];
    for light in optics.lights() {
        let center = light.center_uv();
        let radius = light.radii_uv();
        let rotation = light.rotation_cos_sin();
        let dx = uv[0] - f64::from(center.x);
        let dy = uv[1] - f64::from(center.y);
        let ex = (f64::from(rotation.x) * dx + f64::from(rotation.y) * dy) / f64::from(radius.x);
        let ey = (-f64::from(rotation.y) * dx + f64::from(rotation.x) * dy) / f64::from(radius.y);
        let [lx, ly, lz] = light.direction().map(f64::from);
        let h = [lx, ly, lz + 1.0];
        let norm = h.into_iter().map(|v| v * v).sum::<f64>().sqrt();
        let dot = normal.into_iter().zip(h).map(|(x, y)| x * y / norm).sum::<f64>().clamp(0.0, 1.0);
        let power = 4.0 + 124.0 * (1.0 - f64::from(params.roughness)).powi(2);
        let energy = (-0.5 * (ex * ex + ey * ey)).exp() * f64::from(light.strength()) * dot.powf(power);
        let color = light.color();
        for (out, input) in rgb.iter_mut().zip([color.r, color.g, color.b]) {
            *out += energy * f64::from(input);
        }
    }
    let f0 = f64::from(params.normal_reflectance);
    let tint = optics.reflection_tint();
    let gain = f64::from(optics.bloom().unwrap().gain() * params.reflection_strength * tint.a)
        * (f0 + (1.0 - f0) * (1.0 - normal[2]).powi(5));
    for (out, tint) in rgb.iter_mut().zip([tint.r, tint.g, tint.b]) {
        *out *= gain * f64::from(tint);
        assert!(out.is_finite() && (0.0..=16.0).contains(out));
    }
    let y = rgb[0] * 0.2126 + rgb[1] * 0.7152 + rgb[2] * 0.0722;
    (y, if y > 0.0 { rgb.map(|r| r * excess(y) / y) } else { [0.0; 3] })
}

#[test]
fn bloom_crossing_threshold_and_shader_abi() {
    for (phase, y, expected) in [
        (0.0, 0.383427, 0.0), (0.25, 0.891493, 0.020020), (0.5, 1.181037, 0.185793),
        (0.75, 0.891493, 0.020020), (1.0, 0.383427, 0.0),
    ] {
        let (actual, extraction) = extraction(&crossing(phase, 4.0, 1.0), [0.5, 0.5], -16.0, [0.0, 0.0]);
        let sum = extraction[0] * 0.2126 + extraction[1] * 0.7152 + extraction[2] * 0.0722;
        assert!((actual - y).abs() < 1.0e-6, "{phase}: {actual}");
        assert!((sum - expected).abs() < 1.0e-6, "{phase}: {sum}");
    }
    for (y, expected) in [(0.75, 0.0), (1.0, 0.25), (1.25, 0.5)] {
        let h = 1e-7;
        let derivative = (excess(y + h) - excess(y - h)) / (2.0 * h);
        // At T the soft term is active and its derivative is 0.5.
        let expected = if y == 1.0 { 2.0 * expected } else if y == 1.25 { 1.0 } else { expected };
        assert!((derivative - expected).abs() < 1e-6);
    }
    assert_eq!(offset_of!(BloomSourceUniform, control), 288);
    assert_eq!(offset_of!(BloomFilterUniform, weights), 64);
    assert_eq!(offset_of!(BloomCompositeUniform, control), 128);
    for stage in BloomStage::ALL {
        let shader = pipelines::shader_source(stage);
        let module = naga::front::wgsl::parse_str(&shader)
            .unwrap_or_else(|error| panic!("{}", error.emit_to_string(&shader)));
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), naga::valid::Capabilities::all())
            .validate(&module).unwrap();
        let (name, size, offsets): (&str, usize, &[u32]) = match stage {
            BloomStage::Source => ("BloomSourceUniform", size_of::<BloomSourceUniform>(), &[0, 16, 32, 112, 176, 240, 256, 272, 288]),
            BloomStage::Horizontal | BloomStage::Vertical =>
                ("BloomFilterUniform", size_of::<BloomFilterUniform>(), &[0, 16, 32, 48, 64]),
            BloomStage::Composite => ("BloomCompositeUniform", size_of::<BloomCompositeUniform>(), &[0, 16, 32, 112, 128]),
        };
        let ty = module.types.iter().find(|(_, ty)| ty.name.as_deref() == Some(name)).unwrap().1;
        let naga::TypeInner::Struct { members, span } = &ty.inner else { panic!("uniform record") };
        assert_eq!(*span as usize, size);
        assert_eq!(members.iter().map(|member| member.offset).collect::<Vec<_>>(), offsets);
    }
    assert_eq!((size_of::<BloomSourceUniform>(), size_of::<BloomFilterUniform>(), size_of::<BloomCompositeUniform>()),
        (304, 848, 144));
}

#[test]
fn bloom_checked_appearance_and_final_source_admission() {
    use crate::core::paint::{AnalyticSurfaceError, FrostedGlassAppearance, FrostedGlassError, SurfaceBloomError};
    use crate::core::scene::{Color as SceneColor, RoundedCorners as SceneCorners};
    let base = crossing(0.5, 4.0, 0.0);
    let active = crossing(0.5, 4.0, 1.0);
    for values in [[0.0, 4.0, 1.0], [8.0, 0.0, 1.0], [8.0, 4.0, 0.0]] {
        assert_eq!(active.with_bloom(Some(SurfaceBloom::try_new(values[0], values[1], values[2]).unwrap())), base);
    }
    assert_eq!(SurfaceBloom::try_new(f32::NAN, 4.0, 1.0), Err(SurfaceBloomError::NonFiniteInput));
    for [gain, sigma, mix] in [[8.1, 1.0, 1.0], [8.0, 0.5, 1.0], [8.0, 16.0, -1.0]] {
        assert_eq!(SurfaceBloom::try_new(gain, sigma, mix), Err(SurfaceBloomError::InvalidParameters));
    }
    let source = RectF::new(20.0, 20.0, 32.0, 32.0);
    let rounded = RoundedCorners::uniform(6.0);
    let mut s = SceneBuilder::new();
    let zero = crate::core::paint::analytic_surface_tests::fixture(0.0, 0.0, 0.6)
        .with_glass_optics(Some(active));
    assert_eq!(s.push_analytic_surface(source, rounded, &zero, 4, None), Err(AnalyticSurfaceError::UnsupportedBloomOpacity));
    assert_eq!(FrostedGlassAppearance::try_new(2.0, Color::WHITE, 0.9).unwrap().try_with_optics(Some(active), 0.0, 0.0),
        Err(FrostedGlassError::UnsupportedBloomOpacity));
    let viewport = RectF::new(0.0, 0.0, 160.0, 120.0);
    let mut compositor = Compositor::new(viewport);
    let id = compositor.create_surface(0, viewport, SceneKey::BASE);
    let build = |s: SceneGraph| {
        DrawPlanBuilder::new().build_multi(&HashMap::from([(id, s)]),
            compositor.snapshot_multi(), &mut MeshCache::new(), |_| 1.0, 0)
    };
    let mat = crate::core::paint::analytic_surface_tests::fixture(1.0, 0.0, 0.6)
        .with_glass_optics(Some(active));
    let key = Some(SceneNodeKey::new(395));
    let mut s = SceneBuilder::new();
    s.push_analytic_surface(source, rounded, &mat, 8, key).unwrap();
    s.push_rect(RectF::new(100.0, 20.0, 20.0, 20.0), SceneColor::WHITE, 8, None);
    let plan = build(s.build()).unwrap();
    let admitted = plan.surface_data[&id].bloom.as_ref().unwrap();
    assert_eq!((admitted.surface_id, admitted.node_key, admitted.ordinal, admitted.rect), (id, key, 1, source));
    let mut s = SceneBuilder::new();
    s.push_analytic_surface(source, rounded, &mat, 8, key).unwrap();
    s.push_rect(RectF::new(52.0, 24.0, 10.0, 10.0), SceneColor::WHITE, 8, None);
    assert!(matches!(build(s.build()), Err(DrawPlanBuildError::Bloom {
        surface_id, node_key, reason: BloomBuildRefusal::AmbiguousOrder,
    }) if surface_id == id && node_key == key));
    let mut s = SceneBuilder::new();
    s.push_analytic_surface(source, rounded, &mat, 8, key).unwrap();
    s.push_analytic_surface(source, rounded, &mat, 9, None).unwrap();
    assert!(matches!(build(s.build()), Err(DrawPlanBuildError::Bloom {
        reason: BloomBuildRefusal::OccurrenceLimit { limit: 1, requested: 2 }, ..
    })));
    let mut s = SceneBuilder::new();
    s.with_clip(viewport, Some(SceneCorners::uniform(6.0)), |s| {
        s.with_clip(viewport, Some(SceneCorners::uniform(6.0)), |s| {
            s.push_analytic_surface(source, rounded, &mat, 8, key).unwrap();
        });
    });
    assert!(matches!(build(s.build()), Err(DrawPlanBuildError::Bloom {
        reason: BloomBuildRefusal::UnsupportedClip { depth: 2 }, ..
    })));
}
