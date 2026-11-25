#![cfg(debug_assertions)]

use std::collections::HashMap;

use crate::compositor::{Compositor, MultiSurfaceContract, SceneKey, SurfaceId};
use crate::core::math::{Point, RectF};
use crate::core::scene::{Color, SceneBuilder};
use crate::renderer::DrawPlanBuilder;
use crate::renderer::compiled_draw_plan::MultiTargetDrawPlan;
use crate::renderer::draw_plan::DrawPlan;
use crate::renderer::mesh_cache::MeshCache;
use crate::renderer::wgpu_renderer::WgpuRenderer;

const WIDTH: u32 = 96;
const HEIGHT: u32 = 64;
const LINEAR_IMAGE: u64 = 263_100;
const NEAREST_IMAGE: u64 = 263_101;

fn image_and_text_plan(renderer: &mut WgpuRenderer) -> (SurfaceId, DrawPlan<MultiSurfaceContract>) {
    let viewport = RectF::new(0.0, 0.0, WIDTH as f32, HEIGHT as f32);
    let mut compositor = Compositor::new(viewport);
    let surface_id = compositor.create_surface(0, viewport, SceneKey::BASE);
    renderer
        .register_headless_render_target(surface_id, (WIDTH, HEIGHT), 1.0)
        .expect("register image lifecycle target");

    let mut builder = SceneBuilder::new();
    builder.push_rect(viewport, Color::BLACK, 0, None);
    builder.push_image(
        RectF::new(4.0, 4.0, 40.0, 40.0),
        LINEAR_IMAGE,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        1,
        None,
    );
    builder.push_image(
        RectF::new(48.0, 4.0, 40.0, 40.0),
        NEAREST_IMAGE,
        RectF::new(0.0, 0.0, 1.0, 1.0),
        Color::WHITE,
        1,
        None,
    );
    builder.push_text_probe(
        Point::new(4.0, 58.0),
        "image lifecycle".to_string(),
        12.0,
        Color::WHITE,
        2,
        None,
    );

    let mut scenes = HashMap::new();
    scenes.insert(surface_id, builder.build());
    let plan = DrawPlanBuilder::new()
        .build_multi(
            &scenes,
            compositor.snapshot_multi(),
            &mut MeshCache::new(),
            |_| 1.0,
            0,
        )
        .expect("scene must satisfy DrawPlan Build admission");
    (surface_id, plan)
}

fn create_external_sampled_texture(renderer: &WgpuRenderer) -> wgpu::Texture {
    let gpu = renderer.gpu_resources();
    let texture = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SPEC-HAL-263B nearest registration"),
        size: wgpu::Extent3d {
            width: 2,
            height: 1,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8UnormSrgb,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        &[255, 0, 0, 255, 0, 0, 255, 255],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(1),
        },
        wgpu::Extent3d {
            width: 2,
            height: 1,
            depth_or_array_layers: 1,
        },
    );
    texture
}

fn encode_surface(renderer: &WgpuRenderer, compiled: &MultiTargetDrawPlan, surface_id: SurfaceId) {
    let gpu = renderer.gpu_resources();
    let descriptor = compiled
        .get_target(surface_id)
        .expect("compiled image lifecycle target");
    let (width, height) = descriptor.physical_size;
    let output = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("SPEC-HAL-263B output"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: gpu.format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let command = renderer
        .encode_compiled_target_for_test(
            compiled,
            surface_id,
            &output.create_view(&wgpu::TextureViewDescriptor::default()),
        )
        .expect("compiled image lifecycle surface");
    renderer.submit_compiled_target_for_test(command);
    let _ = gpu.device.poll(wgpu::PollType::wait_indefinitely());
}

#[test]
fn image_registrations_survive_text_and_scale_pipeline_revalidation() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    renderer.register_image_rgba8_srgb(LINEAR_IMAGE, 2, 1, &[255, 0, 0, 255, 0, 0, 255, 255]);
    let nearest_texture = create_external_sampled_texture(&renderer);
    renderer.register_image_from_view_nearest(
        NEAREST_IMAGE,
        nearest_texture.create_view(&wgpu::TextureViewDescriptor::default()),
    );

    let (surface_id, plan) = image_and_text_plan(&mut renderer);
    let first = renderer
        .compile_multi(&plan)
        .expect("first image/text compile")
        .expect_ready("first image/text target");
    encode_surface(&renderer, &first, surface_id);

    let linear_before = renderer.render_registered_image_pixel_for_test(
        LINEAR_IMAGE,
        [0.49, 0.5],
        [1.0; 4],
        wgpu::Color::BLACK,
    );
    let nearest_before = renderer.render_registered_image_pixel_for_test(
        NEAREST_IMAGE,
        [0.49, 0.5],
        [1.0; 4],
        wgpu::Color::BLACK,
    );
    assert_ne!(linear_before, nearest_before);

    renderer.set_scale_factor(surface_id, 2.0);
    let revalidated = renderer
        .compile_multi(&plan)
        .expect("scale-revalidated image/text compile")
        .expect_ready("scale-revalidated image/text target");
    encode_surface(&renderer, &revalidated, surface_id);
    assert_eq!(
        renderer.render_registered_image_pixel_for_test(
            LINEAR_IMAGE,
            [0.49, 0.5],
            [1.0; 4],
            wgpu::Color::BLACK,
        ),
        linear_before
    );
    assert_eq!(
        renderer.render_registered_image_pixel_for_test(
            NEAREST_IMAGE,
            [0.49, 0.5],
            [1.0; 4],
            wgpu::Color::BLACK,
        ),
        nearest_before
    );

    let retained = renderer
        .compile_multi(&plan)
        .expect("retained image/text compile")
        .expect_ready("retained image/text target");
    encode_surface(&renderer, &retained, surface_id);
    drop(nearest_texture);
}
