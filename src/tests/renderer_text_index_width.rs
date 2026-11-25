use crate::core::math::RectF;
use crate::core::scene::{Color, TextQuad};
use crate::renderer::wgpu_renderer::WgpuRenderer;

#[test]
fn text_indices_do_not_wrap_past_u16_vertex_boundary() {
    const QUAD_COUNT: usize = 16_385;

    let quad = TextQuad {
        pos: RectF::new(0.0, 0.0, 1.0, 1.0),
        uv: RectF::new(0.0, 0.0, 1.0, 1.0),
        color: Color::WHITE,
        clip_rect: None,
        downsample_u: 0.0,
    };
    let text_quads = vec![quad; QUAD_COUNT];

    let (vertex_count, indices) =
        WgpuRenderer::build_text_index_values_for_test(&text_quads, 1.0, 1024, 1024);

    assert_eq!(vertex_count, QUAD_COUNT * 4);
    assert_eq!(indices.len(), QUAD_COUNT * 6);
    assert!(indices.iter().copied().any(|index| index > u16::MAX as u32));

    let first_quad_past_u16 = (u16::MAX as usize + 1) / 4;
    assert_eq!(first_quad_past_u16, 16_384);

    let boundary_offset = first_quad_past_u16 * 6;
    let boundary_base = (first_quad_past_u16 * 4) as u32;
    assert_eq!(boundary_base, 65_536);
    assert_eq!(
        &indices[boundary_offset..boundary_offset + 6],
        &[
            boundary_base,
            boundary_base + 1,
            boundary_base + 2,
            boundary_base,
            boundary_base + 2,
            boundary_base + 3,
        ]
    );

    let mut previous_base = None;
    for quad_index in 0..QUAD_COUNT {
        let offset = quad_index * 6;
        let base = indices[offset];
        if let Some(previous_base) = previous_base {
            assert_eq!(base, previous_base + 4);
        }
        assert_eq!(
            &indices[offset..offset + 6],
            &[base, base + 1, base + 2, base, base + 2, base + 3]
        );
        previous_base = Some(base);
    }
}
