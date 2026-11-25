use crate::core::math::RectF;
use crate::core::paint::{Color, PaintSink};
use crate::core::scene::{SceneBuilder, SceneNode};
use crate::core::scene_identity::SceneNodeKey;
use crate::renderer::path::{Path, PathCommand, StrokeCap, StrokeJoin, StrokeParams};

#[test]
fn dyn_paint_sink_preserves_closed_stroke_geometry_and_scene_context() {
    let mut closed = Path::new();
    closed.move_to(20.0, 0.0);
    closed.line_to(40.0, 20.0);
    closed.line_to(20.0, 40.0);
    closed.line_to(0.0, 20.0);
    closed.close();

    let mut repeated_start_open = Path::new();
    repeated_start_open.move_to(20.0, 0.0);
    repeated_start_open.line_to(40.0, 20.0);
    repeated_start_open.line_to(20.0, 40.0);
    repeated_start_open.line_to(0.0, 20.0);
    repeated_start_open.line_to(20.0, 0.0);

    let stroke = StrokeParams {
        width: 4.0,
        join: StrokeJoin::Miter,
        cap: StrokeCap::Butt,
        miter_limit: 4.0,
    };
    let expected_closed_mesh = closed
        .build_stroke_mesh(stroke)
        .expect("closed diamond must produce stroke geometry");
    let repeated_start_open_mesh = repeated_start_open
        .build_stroke_mesh(stroke)
        .expect("repeated-start open diamond must produce stroke geometry");
    assert!(
        expected_closed_mesh.vertices != repeated_start_open_mesh.vertices
            || expected_closed_mesh.indices != repeated_start_open_mesh.indices,
        "Path::close must produce a closing join rather than two endpoint caps"
    );

    let clip_rect = RectF::new(0.0, 0.0, 80.0, 80.0);
    let scroll_offset = (3.5, 7.25);
    let key = SceneNodeKey::new(41);
    let mut builder = SceneBuilder::new();
    {
        let sink: &mut dyn PaintSink = &mut builder;
        sink.begin_clip(clip_rect, None);
        sink.begin_scroll(scroll_offset);
        sink.push_stroked_path(closed, stroke, Color::from_hex(0xE8B84A), 37, Some(key));
        sink.end_scroll();
        sink.end_clip();
    }

    let graph = builder.build();
    let [SceneNode::Clip(clip)] = graph.root.as_slice() else {
        panic!("stroked path must remain inside the active clip");
    };
    assert_eq!(clip.rect, clip_rect);

    let [SceneNode::Scroll(scroll)] = clip.children.as_slice() else {
        panic!("stroked path must retain the active scroll transform");
    };
    assert_eq!(scroll.offset, scroll_offset);

    let [SceneNode::Path(path)] = scroll.children.as_slice() else {
        panic!("PaintSink must submit one PathNode");
    };
    assert!(path.is_stroke);
    assert_eq!(path.z_index, 37);
    assert_eq!(path.key, Some(key));
    assert!(matches!(
        path.path.commands.last(),
        Some(PathCommand::Close)
    ));

    let mesh = path
        .mesh
        .as_ref()
        .expect("stroked path must carry its mesh");
    assert_eq!(mesh.vertices, expected_closed_mesh.vertices);
    assert_eq!(mesh.indices, expected_closed_mesh.indices);
    assert_eq!(mesh.bounds, expected_closed_mesh.bounds);
}
