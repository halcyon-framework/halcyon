use crate::core::icon::*;
use crate::core::icon_builder::IconBuilder;
use crate::core::icons::builtin_registry;
use crate::renderer::path::Path;

// ============================================================================
// Test 1: line produces non-empty mesh
// ============================================================================

#[test]
fn test_builder_line() {
    let def = IconBuilder::new(24.0)
        .line(2.0, 2.0, 20.0, 20.0, 1.5)
        .build(
            IconId(100),
            "test-line",
            IconRole::Action,
            IconRenderMode::Stroke,
        );
    let mesh = def.layers[0].path.to_mesh();
    assert!(
        !mesh.vertices.is_empty(),
        "line should produce non-empty mesh"
    );
    assert!(!mesh.indices.is_empty(), "line mesh should have triangles");
}

// ============================================================================
// Test 2: circle bounds
// ============================================================================

#[test]
fn test_builder_circle() {
    let def = IconBuilder::new(24.0).circle(12.0, 12.0, 5.0).build(
        IconId(101),
        "test-circle",
        IconRole::Action,
        IconRenderMode::Fill,
    );
    let bounds = def.layers[0].path.bounds();
    assert!(
        (bounds.x - 7.0).abs() < 0.5,
        "circle min_x should be ~7, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 7.0).abs() < 0.5,
        "circle min_y should be ~7, got {}",
        bounds.y
    );
    assert!(
        (bounds.width - 10.0).abs() < 0.5,
        "circle width should be ~10, got {}",
        bounds.width
    );
    assert!(
        (bounds.height - 10.0).abs() < 0.5,
        "circle height should be ~10, got {}",
        bounds.height
    );
}

// ============================================================================
// Test 3: ring has cubic commands for both circles
// ============================================================================

#[test]
fn test_builder_ring() {
    let def = IconBuilder::new(24.0).ring(12.0, 12.0, 3.0, 6.0).build(
        IconId(102),
        "test-ring",
        IconRole::Action,
        IconRenderMode::Fill,
    );
    let cubic_count = def.layers[0]
        .path
        .commands
        .iter()
        .filter(|c| matches!(c, crate::renderer::path::PathCommand::CubicTo(..)))
        .count();
    // Outer circle: 4 cubics, inner circle: 4 cubics = 8 total
    assert!(
        cubic_count >= 8,
        "ring should have >=8 cubic commands, got {}",
        cubic_count
    );
}

// ============================================================================
// Test 4: polygon bounds
// ============================================================================

#[test]
fn test_builder_polygon() {
    let def = IconBuilder::new(24.0)
        .polygon(&[(5.0, 5.0), (20.0, 5.0), (12.5, 20.0)])
        .build(
            IconId(103),
            "test-polygon",
            IconRole::Status,
            IconRenderMode::Fill,
        );
    let bounds = def.layers[0].path.bounds();
    assert!(
        (bounds.x - 5.0).abs() < 0.1,
        "polygon min_x should be ~5, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 5.0).abs() < 0.1,
        "polygon min_y should be ~5, got {}",
        bounds.y
    );
    assert!(
        (bounds.width - 15.0).abs() < 0.1,
        "polygon width should be ~15, got {}",
        bounds.width
    );
    assert!(
        (bounds.height - 15.0).abs() < 0.1,
        "polygon height should be ~15, got {}",
        bounds.height
    );
}

// ============================================================================
// Test 5: rounded_rect bounds
// ============================================================================

#[test]
fn test_builder_rounded_rect() {
    let def = IconBuilder::new(24.0)
        .rounded_rect(4.0, 4.0, 16.0, 12.0, 3.0)
        .build(
            IconId(104),
            "test-rrect",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    let bounds = def.layers[0].path.bounds();
    assert!(
        (bounds.x - 4.0).abs() < 0.5,
        "rrect min_x should be ~4, got {}",
        bounds.x
    );
    assert!(
        (bounds.y - 4.0).abs() < 0.5,
        "rrect min_y should be ~4, got {}",
        bounds.y
    );
    assert!(
        (bounds.width - 16.0).abs() < 0.5,
        "rrect width should be ~16, got {}",
        bounds.width
    );
    assert!(
        (bounds.height - 12.0).abs() < 0.5,
        "rrect height should be ~12, got {}",
        bounds.height
    );
}

// ============================================================================
// Test 6: chaining multiple primitives
// ============================================================================

#[test]
fn test_builder_chaining() {
    let def = IconBuilder::new(24.0)
        .line(2.0, 2.0, 10.0, 10.0, 1.0)
        .circle(18.0, 18.0, 3.0)
        .rect(0.0, 0.0, 5.0, 5.0)
        .build(
            IconId(105),
            "test-chain",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    // line: 5 commands (move, 3 line_to, close)
    // circle: 6 commands (move, 4 cubic_to, close)
    // rect: 5 commands (move, 3 line_to, close)
    // Total: at least 15 commands
    assert!(
        def.layers[0].path.commands.len() >= 15,
        "chained primitives should produce >=15 commands, got {}",
        def.layers[0].path.commands.len()
    );
}

// ============================================================================
// Test 7: rebuilt icons match original path hashes
// ============================================================================

/// Reference implementation of `line_segment` — identical to the original
/// hand-authored helper that was in `icons/mod.rs` before the builder rewrite.
fn line_segment_ref(path: &mut Path, x0: f32, y0: f32, x1: f32, y1: f32) {
    const SW: f32 = 1.5;
    const HSW: f32 = SW / 2.0;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 {
        return;
    }
    let px = -dy / len * HSW;
    let py = dx / len * HSW;
    path.move_to(x0 + px, y0 + py);
    path.line_to(x1 + px, y1 + py);
    path.line_to(x1 - px, y1 - py);
    path.line_to(x0 - px, y0 - py);
    path.close();
}

/// Compute a path_hash using the new multi-layer algorithm for a single Base layer.
fn single_base_hash(path: &Path) -> u64 {
    let layer_hash = path.content_hash();
    0u64.wrapping_mul(31)
        .wrapping_add(layer_hash ^ (LayerRole::Base as u64))
}

/// Build original icon paths inline and return their content hashes.
/// This duplicates the pre-builder hand-authored code for regression testing.
/// Uses the new multi-layer hash algorithm (single Base layer for all originals).
fn original_hashes() -> [(IconId, u64); 13] {
    const KAPPA: f32 = 0.5522847498;
    const SW: f32 = 1.5;
    const HSW: f32 = SW / 2.0;

    // close
    let close_hash = {
        let mut p = Path::new();
        line_segment_ref_w(&mut p, 5.5, 5.5, 18.5, 18.5, 2.2);
        line_segment_ref_w(&mut p, 18.5, 5.5, 5.5, 18.5, 2.2);
        single_base_hash(&p)
    };

    // chevron_right
    let chevron_right_hash = {
        let mut p = Path::new();
        line_segment_ref(&mut p, 9.0, 6.0, 15.0, 12.0);
        line_segment_ref(&mut p, 15.0, 12.0, 9.0, 18.0);
        single_base_hash(&p)
    };

    // chevron_down
    let chevron_down_hash = {
        let mut p = Path::new();
        line_segment_ref(&mut p, 6.0, 9.0, 12.0, 15.0);
        line_segment_ref(&mut p, 12.0, 15.0, 18.0, 9.0);
        single_base_hash(&p)
    };

    // plus (manual rects, NOT line_segment)
    let plus_hash = {
        let mut p = Path::new();
        p.move_to(5.0, 11.0);
        p.line_to(19.0, 11.0);
        p.line_to(19.0, 13.0);
        p.line_to(5.0, 13.0);
        p.close();
        p.move_to(11.0, 5.0);
        p.line_to(13.0, 5.0);
        p.line_to(13.0, 19.0);
        p.line_to(11.0, 19.0);
        p.close();
        single_base_hash(&p)
    };

    // minus (manual rect)
    let minus_hash = {
        let mut p = Path::new();
        p.move_to(5.0, 11.0);
        p.line_to(19.0, 11.0);
        p.line_to(19.0, 13.0);
        p.line_to(5.0, 13.0);
        p.close();
        single_base_hash(&p)
    };

    // check
    let check_hash = {
        let mut p = Path::new();
        line_segment_ref(&mut p, 6.0, 12.0, 10.0, 17.0);
        line_segment_ref(&mut p, 10.0, 17.0, 18.0, 7.0);
        single_base_hash(&p)
    };

    // search (ring + handle)
    let search_hash = {
        let mut p = Path::new();
        let cx = 10.0_f32;
        let cy = 10.0_f32;
        let r_outer = 5.0 + HSW;
        let r_inner = 5.0 - HSW;
        let ko = KAPPA * r_outer;
        p.move_to(cx + r_outer, cy);
        p.cubic_to(
            cx + r_outer,
            cy + ko,
            cx + ko,
            cy + r_outer,
            cx,
            cy + r_outer,
        );
        p.cubic_to(
            cx - ko,
            cy + r_outer,
            cx - r_outer,
            cy + ko,
            cx - r_outer,
            cy,
        );
        p.cubic_to(
            cx - r_outer,
            cy - ko,
            cx - ko,
            cy - r_outer,
            cx,
            cy - r_outer,
        );
        p.cubic_to(
            cx + ko,
            cy - r_outer,
            cx + r_outer,
            cy - ko,
            cx + r_outer,
            cy,
        );
        p.close();
        let ki = KAPPA * r_inner;
        p.move_to(cx + r_inner, cy);
        p.cubic_to(
            cx + r_inner,
            cy - ki,
            cx + ki,
            cy - r_inner,
            cx,
            cy - r_inner,
        );
        p.cubic_to(
            cx - ki,
            cy - r_inner,
            cx - r_inner,
            cy - ki,
            cx - r_inner,
            cy,
        );
        p.cubic_to(
            cx - r_inner,
            cy + ki,
            cx - ki,
            cy + r_inner,
            cx,
            cy + r_inner,
        );
        p.cubic_to(
            cx + ki,
            cy + r_inner,
            cx + r_inner,
            cy + ki,
            cx + r_inner,
            cy,
        );
        p.close();
        line_segment_ref(&mut p, 14.0, 14.0, 19.0, 19.0);
        single_base_hash(&p)
    };

    // arrow_up
    let arrow_up_hash = {
        let mut p = Path::new();
        line_segment_ref(&mut p, 12.0, 19.0, 12.0, 7.0);
        line_segment_ref(&mut p, 12.0, 5.0, 7.0, 10.0);
        line_segment_ref(&mut p, 12.0, 5.0, 17.0, 10.0);
        single_base_hash(&p)
    };

    // arrow_down
    let arrow_down_hash = {
        let mut p = Path::new();
        line_segment_ref(&mut p, 12.0, 5.0, 12.0, 17.0);
        line_segment_ref(&mut p, 12.0, 19.0, 7.0, 14.0);
        line_segment_ref(&mut p, 12.0, 19.0, 17.0, 14.0);
        single_base_hash(&p)
    };

    // warning
    let warning_hash = {
        let mut p = Path::new();
        p.move_to(12.0, 4.0);
        p.line_to(21.0, 20.0);
        p.line_to(3.0, 20.0);
        p.close();
        single_base_hash(&p)
    };

    // folder (3 layers: Shadow, Base, Highlight)
    let folder_hash = {
        let mut h: u64 = 0;
        // Layer 0: Shadow — back panel
        let mut p0 = Path::new();
        p0.move_to(2.0, 4.0);
        p0.line_to(22.0, 4.0);
        p0.line_to(22.0, 10.0);
        p0.line_to(2.0, 10.0);
        p0.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p0.content_hash() ^ (LayerRole::Shadow as u64));
        // Layer 1: Base — front face
        let mut p1 = Path::new();
        p1.move_to(2.0, 8.0);
        p1.line_to(22.0, 8.0);
        p1.line_to(22.0, 21.0);
        p1.line_to(2.0, 21.0);
        p1.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p1.content_hash() ^ (LayerRole::Base as u64));
        // Layer 2: Highlight — tab
        let mut p2 = Path::new();
        p2.move_to(2.0, 3.0);
        p2.line_to(11.0, 3.0);
        p2.line_to(13.0, 7.0);
        p2.line_to(2.0, 7.0);
        p2.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p2.content_hash() ^ (LayerRole::Highlight as u64));
        h
    };

    // file (3 layers: Base, Highlight, Shadow)
    let file_hash = {
        let mut h: u64 = 0;
        // Layer 0: Base — page body (shifted left: x=4 instead of 6)
        let mut p0 = Path::new();
        p0.move_to(4.0, 2.0);
        p0.line_to(16.0, 2.0);
        p0.line_to(20.0, 6.0);
        p0.line_to(20.0, 22.0);
        p0.line_to(4.0, 22.0);
        p0.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p0.content_hash() ^ (LayerRole::Base as u64));
        // Layer 1: Highlight — fold face
        let mut p1 = Path::new();
        p1.move_to(16.0, 2.0);
        p1.line_to(20.0, 6.0);
        p1.line_to(16.0, 6.0);
        p1.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p1.content_hash() ^ (LayerRole::Highlight as u64));
        // Layer 2: Shadow — fold crease
        let mut p2 = Path::new();
        line_segment_ref_w(&mut p2, 16.0, 6.0, 20.0, 6.0, 0.6);
        line_segment_ref_w(&mut p2, 16.0, 2.0, 16.0, 6.0, 0.6);
        h = h
            .wrapping_mul(31)
            .wrapping_add(p2.content_hash() ^ (LayerRole::Shadow as u64));
        h
    };

    // folder_open (3 layers: Shadow, Base, Highlight)
    let folder_open_hash = {
        let mut h: u64 = 0;
        // Layer 0: Shadow — back panel / lid
        let mut p0 = Path::new();
        p0.move_to(2.0, 2.0);
        p0.line_to(22.0, 2.0);
        p0.line_to(22.0, 8.0);
        p0.line_to(2.0, 8.0);
        p0.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p0.content_hash() ^ (LayerRole::Shadow as u64));
        // Layer 1: Base — front face (trapezoid)
        let mut p1 = Path::new();
        p1.move_to(1.0, 10.0);
        p1.line_to(23.0, 10.0);
        p1.line_to(21.0, 21.0);
        p1.line_to(3.0, 21.0);
        p1.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p1.content_hash() ^ (LayerRole::Base as u64));
        // Layer 2: Highlight — lid top edge
        let mut p2 = Path::new();
        p2.move_to(2.0, 2.0);
        p2.line_to(11.0, 2.0);
        p2.line_to(13.0, 5.0);
        p2.line_to(2.0, 5.0);
        p2.close();
        h = h
            .wrapping_mul(31)
            .wrapping_add(p2.content_hash() ^ (LayerRole::Highlight as u64));
        h
    };

    [
        (IconId::CLOSE, close_hash),
        (IconId::CHEVRON_RIGHT, chevron_right_hash),
        (IconId::CHEVRON_DOWN, chevron_down_hash),
        (IconId::PLUS, plus_hash),
        (IconId::MINUS, minus_hash),
        (IconId::CHECK, check_hash),
        (IconId::SEARCH, search_hash),
        (IconId::ARROW_UP, arrow_up_hash),
        (IconId::ARROW_DOWN, arrow_down_hash),
        (IconId::WARNING, warning_hash),
        (IconId::FOLDER, folder_hash),
        (IconId::FILE, file_hash),
        (IconId::FOLDER_OPEN, folder_open_hash),
    ]
}

/// Like line_segment_ref but with configurable width.
fn line_segment_ref_w(path: &mut Path, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) {
    let hsw = width / 2.0;
    let dx = x1 - x0;
    let dy = y1 - y0;
    let len = (dx * dx + dy * dy).sqrt();
    if len < 0.001 {
        return;
    }
    let px = -dy / len * hsw;
    let py = dx / len * hsw;
    path.move_to(x0 + px, y0 + py);
    path.line_to(x1 + px, y1 + py);
    path.line_to(x1 - px, y1 - py);
    path.line_to(x0 - px, y0 - py);
    path.close();
}

#[test]
fn test_rebuilt_icons_match() {
    let registry = builtin_registry();
    let originals = original_hashes();

    for (id, expected_hash) in &originals {
        let def = registry
            .get(*id)
            .unwrap_or_else(|| panic!("missing icon {:?}", id));
        assert_eq!(
            def.path_hash, *expected_hash,
            "icon {:?} ({}) hash mismatch: builder={:#018x}, original={:#018x}",
            id, def.name, def.path_hash, expected_hash
        );
    }
}

// ============================================================================
// Test 8: layer switching produces multiple layers
// ============================================================================

#[test]
fn test_layer_switching() {
    let icon = IconBuilder::new(24.0)
        .layer(LayerRole::Shadow)
        .rect(0.0, 0.0, 10.0, 10.0)
        .layer(LayerRole::Base)
        .rect(2.0, 2.0, 6.0, 6.0)
        .build(
            IconId(200),
            "test-layers",
            IconRole::Action,
            IconRenderMode::Fill,
        );
    assert_eq!(icon.layers.len(), 2);
    assert_eq!(icon.layers[0].role, LayerRole::Shadow);
    assert_eq!(icon.layers[1].role, LayerRole::Base);
}

// ============================================================================
// Test 9: default Base layer when no layer() calls
// ============================================================================

#[test]
fn test_default_base_layer() {
    let icon = IconBuilder::new(24.0).rect(0.0, 0.0, 10.0, 10.0).build(
        IconId(201),
        "test-default",
        IconRole::Action,
        IconRenderMode::Fill,
    );
    assert_eq!(icon.layers.len(), 1);
    assert_eq!(icon.layers[0].role, LayerRole::Base);
}

// ============================================================================
// Test 10: LayerRole::resolve color math
// ============================================================================

#[test]
fn test_layer_role_resolve() {
    // Base: pass-through.
    let (r, g, b, a) = LayerRole::Base.resolve(0.5, 0.5, 0.5, 1.0);
    assert!((r - 0.5).abs() < 0.001);
    assert!((g - 0.5).abs() < 0.001);
    assert!((b - 0.5).abs() < 0.001);
    assert!((a - 1.0).abs() < 0.001);

    // Shadow: darkened.
    let (r, _, _, _) = LayerRole::Shadow.resolve(1.0, 1.0, 1.0, 1.0);
    assert!((r - 0.65).abs() < 0.001);

    // Highlight: lightened.
    let (r, _, _, _) = LayerRole::Highlight.resolve(0.0, 0.0, 0.0, 1.0);
    assert!((r - 0.35).abs() < 0.001);

    // Outline: strongly darkened.
    let (r, _, _, _) = LayerRole::Outline.resolve(1.0, 1.0, 1.0, 1.0);
    assert!((r - 0.45).abs() < 0.001);
}
