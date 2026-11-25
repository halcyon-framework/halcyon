// =============================================================================
// SPEC-HAL-172/259: Tree Guide Paint Helper
// =============================================================================
//
// Paint-layer helper that maps tree guide data (guide_mask, stem_down, depth)
// to renderable line segments for realized rows.
//
// Engine-driven: accepts any implementation of TreeRenderData trait,
// not a specific TreeProjection type. Guide data is computed by the
// hierarchical engine (SPEC-HAL-259) and provided at paint time.
//
// INVARIANT-TREE-2: No tree logic may mutate column model, geometry, header,
//                   or horizontal resolution.
// INVARIANT-TREE-GUIDE-3: Guide data does not alter any RectF / cell rect /
//                          column width/offset.
//
// This module is PURE computation: it reads tree state + row geometry and
// produces segment rects. It does NOT access PaintSink directly.
//
// Guide rendering is a two-phase process:
// 1. Topology: compute RowGuideTopology from engine data (guide_mask, depth, stem_down).
// 2. Geometry: emit TreeGuideSegment rects from the topology struct.
// No inline inference during drawing. All guide decisions derive from explicit state.
// =============================================================================

use super::row::RowProjection;
use crate::core::math::{RectF, snap_to_physical};
use crate::core::paint::Color;

/// Configuration for tree visual chrome (guide lines, disclosure toggles).
///
/// Controls which tree elements are rendered and interactive.
/// Used by TreeView (full chrome) and PropertyEditor (minimal chrome).
#[derive(Debug, Clone, Copy)]
pub struct TreeChromeConfig {
    /// Render tree guide lines (vertical stems, horizontal connectors).
    /// Default: true.
    pub show_guides: bool,
    /// Render disclosure triangles for expandable nodes.
    /// Default: true.
    pub show_disclosure: bool,
    /// Enable single-click toggle on the disclosure zone.
    /// When false, only programmatic toggle (keyboard, double-click) works.
    /// Default: true. Only meaningful when show_disclosure is true.
    pub disclosure_click_enabled: bool,
}

impl Default for TreeChromeConfig {
    fn default() -> Self {
        Self {
            show_guides: true,
            show_disclosure: true,
            disclosure_click_enabled: true,
        }
    }
}

impl TreeChromeConfig {
    /// PropertyEditor preset: no guides, disclosure triangles shown
    /// but single-click toggle disabled (double-click on row toggles instead).
    pub fn property_editor() -> Self {
        Self {
            show_guides: false,
            show_disclosure: true,
            disclosure_click_enabled: false,
        }
    }

    /// Flat list preset: no guides, no disclosure, no toggle.
    pub fn flat() -> Self {
        Self {
            show_guides: false,
            show_disclosure: false,
            disclosure_click_enabled: false,
        }
    }
}

/// Trait providing tree metadata for paint and hit-test operations.
///
/// Decouples paint helpers from any specific tree state implementation.
/// Implementors include engine-side projection views and test harnesses.
pub trait TreeRenderData {
    /// Tree depth for a row (0 for root nodes).
    fn depth(&self, row_index: usize) -> u32;
    /// Whether a row has children (for tree control zone detection).
    fn has_children(&self, row_index: usize) -> bool;
    /// Guide mask: bit semantics depend on row depth D:
    ///   - Bits 0..D-2: ancestor continuation (set = draw │ at that lane).
    ///   - Bit D-1: parent connector (set = ├ tee, clear = └ elbow).
    fn guide_mask(&self, row_index: usize) -> u64;
    /// Stem-down flag: true when node is expanded and has children.
    fn stem_down(&self, row_index: usize) -> bool;
}

// =============================================================================
// Guide State Model — Canonical per-lane states
// =============================================================================

/// Per-lane guide state for a single row.
///
/// Determines the visual form at each depth lane in a tree guide column.
/// This is the canonical intermediate representation: topology engine output
/// (guide_mask, depth, stem_down) maps deterministically to these states,
/// and the painter maps these states to geometric segments.
/// No other inference path exists.
///
/// ## Visual primitives
///
/// | State        | Visual | Segments emitted                       |
/// |--------------|--------|----------------------------------------|
/// | Empty        | (none) | No segments                            |
/// | Continuation | │      | Full-height vertical                   |
/// | Tee          | ├      | Full-height vertical + horizontal stub |
/// | Elbow        | └      | Half-height vertical + horizontal stub |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaneState {
    /// No guide mark at this lane.
    /// The ancestor's sibling chain has ended (ancestor is last child).
    Empty,
    /// Full-height vertical continuation line (│).
    /// The ancestor at depth `d+1` is NOT the last child of the ancestor
    /// at depth `d` — the sibling chain continues past this row.
    Continuation,
    /// Tee junction (├): full-height vertical + horizontal connector.
    /// This row is NOT the last sibling — parent connector with continuation.
    /// Appears only at the parent lane (depth - 1).
    Tee,
    /// L-elbow (└): half-height vertical (top → mid) + horizontal connector.
    /// This row IS the last sibling — parent connector terminates here.
    /// Appears only at the parent lane (depth - 1).
    Elbow,
}

/// Complete guide topology for a single visible row.
///
/// Pure function of tree structural state: (guide_mask, depth, stem_down).
/// No viewport, scroll, or geometry dependence. Deterministic and
/// identical for identical tree structure regardless of scroll position.
///
/// ## Lane mapping (for row at depth D)
///
/// | Lane index | Lane role           | Possible states         |
/// |------------|---------------------|-------------------------|
/// | 0..D-2     | Ancestor continuation | Empty, Continuation   |
/// | D-1        | Parent connector    | Tee, Elbow              |
///
/// ## Exhaustive row forms
///
/// | Row structural state              | Parent lane | Ancestor lanes            | Stem  |
/// |-----------------------------------|-------------|---------------------------|-------|
/// | Root (depth 0)                    | —           | —                         | if expanded w/ children |
/// | Middle sibling                    | Tee         | per-ancestor last-child   | if expanded w/ children |
/// | Last sibling                      | Elbow       | per-ancestor last-child   | if expanded w/ children |
/// | Only child (= last sibling)       | Elbow       | per-ancestor last-child   | if expanded w/ children |
/// | Collapsed parent                  | Tee/Elbow   | per-ancestor last-child   | false |
/// | Expanded parent with children     | Tee/Elbow   | per-ancestor last-child   | true  |
/// | Leaf under non-last ancestor      | Tee/Elbow   | has Continuation(s)       | false |
/// | Leaf under all-last ancestors     | Tee/Elbow   | all Empty                 | false |
/// | Deep nested continuation          | Tee/Elbow   | mix of Continuation/Empty | depends |
pub struct RowGuideTopology {
    /// Lane states for depths 0..depth.
    /// Only indices `0..depth` are valid; remaining entries are Empty.
    /// Lanes 0..depth-2 are ancestor continuation lanes (Empty or Continuation).
    /// Lane depth-1 is the parent connector lane (Tee or Elbow).
    pub lanes: [LaneState; 64],
    /// Row depth (= number of active lanes).
    pub depth: u32,
    /// Whether to draw stem-down at own depth level.
    /// True when node is expanded and has children.
    pub stem_down: bool,
}

impl RowGuideTopology {
    /// Compute guide topology from engine-provided structural data.
    ///
    /// Pure mapping: (guide_mask, depth, stem_down) → topology.
    ///
    /// guide_mask bit semantics (from topology engine):
    /// - Bits 0..depth-2: ancestor continuation (bit set = Continuation).
    /// - Bit depth-1: parent connector (bit set = Tee, clear = Elbow).
    pub fn from_row_data(guide_mask: u64, depth: u32, stem_down: bool) -> Self {
        let mut lanes = [LaneState::Empty; 64];

        if depth > 0 {
            let parent_lane = (depth - 1) as usize;

            // Ancestor lanes: 0..depth-2 — continuation or empty.
            for d in 0..parent_lane {
                if guide_mask & (1u64 << d.min(63)) != 0 {
                    lanes[d] = LaneState::Continuation;
                }
            }

            // Parent lane: depth-1 — tee or elbow.
            if guide_mask & (1u64 << parent_lane.min(63)) != 0 {
                lanes[parent_lane] = LaneState::Tee;
            } else {
                lanes[parent_lane] = LaneState::Elbow;
            }
        }

        Self {
            lanes,
            depth,
            stem_down,
        }
    }
}

// =============================================================================
// Guide Segment Types (rendering primitives)
// =============================================================================

/// Kind of tree guide line segment.
///
/// These are the rendering-layer primitives emitted by the guide painter.
/// Each LaneState decomposes into one or more of these:
///
/// | LaneState    | Segments emitted                          |
/// |--------------|-------------------------------------------|
/// | Empty        | (none)                                    |
/// | Continuation | 1× Continuation                          |
/// | Tee          | 1× Continuation + 1× ElbowHorizontal     |
/// | Elbow        | 1× ElbowVertical + 1× ElbowHorizontal    |
/// | (stem_down)  | 1× StemDown                               |
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TreeGuideKind {
    /// Vertical continuation line (full row height at ancestor depth).
    Continuation,
    /// Vertical part of elbow connector (top of row to row center).
    ElbowVertical,
    /// Horizontal part of connector (from lane center to child indent).
    ElbowHorizontal,
    /// Vertical stem extending down from expanded node (center to bottom).
    StemDown,
}

/// A tree guide line segment to be painted.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TreeGuideSegment {
    /// Bounding rect for this segment (in body-relative coordinates).
    pub rect: RectF,
    /// Segment kind (for color/style differentiation if needed).
    pub kind: TreeGuideKind,
}

// =============================================================================
// Guide Emission — topology-driven segment generation
// =============================================================================

/// Emit tree guide segments for all realized rows.
///
/// Two-phase process per row:
/// 1. Compute `RowGuideTopology` from engine data (guide_mask, depth, stem_down).
/// 2. Emit `TreeGuideSegment` rects strictly from the topology struct.
///
/// No inline inference during drawing. All guide decisions derive from
/// the explicit LaneState values in RowGuideTopology.
///
/// # Arguments
/// * `realized_rows` - Currently realized row projections.
/// * `tree_data` - Engine-provided tree render metadata.
/// * `first_col_x` - X position of the first visible column (before indentation).
/// * `indent_step` - Pixels per depth level (TREE_INDENT_STEP).
/// * `line_width` - Width of guide lines in pixels.
/// * `viewport_y` - Y offset of body viewport (header height).
/// * `emit` - Callback receiving each computed segment.
pub fn emit_tree_guide_segments<F>(
    realized_rows: &[RowProjection],
    tree_data: &dyn TreeRenderData,
    first_col_x: f32,
    indent_step: f32,
    line_width: f32,
    viewport_y: f32,
    mut emit: F,
) where
    F: FnMut(TreeGuideSegment),
{
    const GUIDE_TO_NODE_GAP: f32 = 0.0;

    let half_lw = line_width * 0.5;

    for row in realized_rows {
        let row_index = row.row_index;

        // Phase 1: Compute topology from engine data.
        let topo = RowGuideTopology::from_row_data(
            tree_data.guide_mask(row_index),
            tree_data.depth(row_index),
            tree_data.stem_down(row_index),
        );

        // Phase 2: Emit segments strictly from topology.
        let row_top = row.y_offset + viewport_y;
        let row_h = row.height;
        let mid_y = row_top + row_h * 0.5;

        // Per-row termination and toggle clipping bounds.
        // Disclosure box geometry (same formula as DisclosureToggle).
        let has_toggle = tree_data.has_children(row_index);
        let raw = (row_h * 0.5).floor().clamp(9.0, 13.0);
        let box_size = if raw as i32 % 2 == 0 { raw - 1.0 } else { raw };
        let box_half = box_size * 0.5;
        let disclosure_center = center_x(first_col_x, indent_step, topo.depth);

        let termination_x = if has_toggle {
            disclosure_center - box_half - GUIDE_TO_NODE_GAP
        } else {
            // Leaf: icon left edge (same formula as body.rs tree column painting).
            let icon_x = first_col_x + (topo.depth as f32 + 1.0) * indent_step;
            icon_x - GUIDE_TO_NODE_GAP
        };

        // Toggle box Y bounds for geometric clipping of vertical segments.
        let toggle_top = row_top + (row_h - box_size) * 0.5;
        let toggle_bottom = toggle_top + box_size;
        let toggle_left = disclosure_center - box_half;
        let toggle_right = disclosure_center + box_half;
        let row_bottom = row_top + row_h;

        // Returns true if a vertical line at `vx` intersects the toggle box horizontally.
        let at_toggle = |vx: f32| -> bool { has_toggle && vx >= toggle_left && vx <= toggle_right };

        for d in 0..topo.depth {
            let du = d as usize;
            match topo.lanes[du] {
                LaneState::Empty => {}
                LaneState::Continuation => {
                    let cx = center_x(first_col_x, indent_step, d);
                    if at_toggle(cx) {
                        // Y-clip: split into top and bottom segments around toggle box.
                        let h_top = (toggle_top - row_top).max(0.0);
                        if h_top > 0.0 {
                            emit(TreeGuideSegment {
                                rect: RectF::new(cx - half_lw, row_top, line_width, h_top),
                                kind: TreeGuideKind::Continuation,
                            });
                        }
                        let h_bot = (row_bottom - toggle_bottom).max(0.0);
                        if h_bot > 0.0 {
                            emit(TreeGuideSegment {
                                rect: RectF::new(cx - half_lw, toggle_bottom, line_width, h_bot),
                                kind: TreeGuideKind::Continuation,
                            });
                        }
                    } else {
                        emit(TreeGuideSegment {
                            rect: RectF::new(cx - half_lw, row_top, line_width, row_h),
                            kind: TreeGuideKind::Continuation,
                        });
                    }
                }
                LaneState::Tee => {
                    let cx = center_x(first_col_x, indent_step, d);
                    // Tee vertical: full-height continuation at parent lane.
                    if at_toggle(cx) {
                        let h_top = (toggle_top - row_top).max(0.0);
                        if h_top > 0.0 {
                            emit(TreeGuideSegment {
                                rect: RectF::new(cx - half_lw, row_top, line_width, h_top),
                                kind: TreeGuideKind::Continuation,
                            });
                        }
                        let h_bot = (row_bottom - toggle_bottom).max(0.0);
                        if h_bot > 0.0 {
                            emit(TreeGuideSegment {
                                rect: RectF::new(cx - half_lw, toggle_bottom, line_width, h_bot),
                                kind: TreeGuideKind::Continuation,
                            });
                        }
                    } else {
                        emit(TreeGuideSegment {
                            rect: RectF::new(cx - half_lw, row_top, line_width, row_h),
                            kind: TreeGuideKind::Continuation,
                        });
                    }
                    // Tee horizontal: connector from lane center to node edge.
                    let horiz_width = (termination_x - cx).max(0.0);
                    if horiz_width > 0.0 {
                        emit(TreeGuideSegment {
                            rect: RectF::new(cx, mid_y - half_lw, horiz_width, line_width),
                            kind: TreeGuideKind::ElbowHorizontal,
                        });
                    }
                }
                LaneState::Elbow => {
                    let cx = center_x(first_col_x, indent_step, d);
                    // Elbow vertical: row_top → mid_y, Y-clipped at toggle_top.
                    if at_toggle(cx) {
                        let h = (toggle_top.min(mid_y) - row_top).max(0.0);
                        if h > 0.0 {
                            emit(TreeGuideSegment {
                                rect: RectF::new(cx - half_lw, row_top, line_width, h),
                                kind: TreeGuideKind::ElbowVertical,
                            });
                        }
                    } else {
                        emit(TreeGuideSegment {
                            rect: RectF::new(cx - half_lw, row_top, line_width, row_h * 0.5),
                            kind: TreeGuideKind::ElbowVertical,
                        });
                    }
                    // Elbow horizontal: connector from lane center to node edge.
                    let horiz_width = (termination_x - cx).max(0.0);
                    if horiz_width > 0.0 {
                        emit(TreeGuideSegment {
                            rect: RectF::new(cx, mid_y - half_lw, horiz_width, line_width),
                            kind: TreeGuideKind::ElbowHorizontal,
                        });
                    }
                }
            }
        }

        // Stem down at own depth level (expanded parent with children).
        // Y-clipped to start below toggle box when toggle is present.
        if topo.stem_down {
            let cx = center_x(first_col_x, indent_step, topo.depth);
            if at_toggle(cx) {
                let start_y = mid_y.max(toggle_bottom);
                let h = row_bottom - start_y;
                if h > 0.0 {
                    emit(TreeGuideSegment {
                        rect: RectF::new(cx - half_lw, start_y, line_width, h),
                        kind: TreeGuideKind::StemDown,
                    });
                }
            } else {
                emit(TreeGuideSegment {
                    rect: RectF::new(cx - half_lw, mid_y, line_width, row_h * 0.5),
                    kind: TreeGuideKind::StemDown,
                });
            }
        }
    }
}

/// Compute guide color as a linear blend of row background and text.
///
/// Produces a subtle but readable guide color at 55% text / 45% background.
#[inline]
pub fn compute_guide_color(bg: Color, fg: Color) -> Color {
    let t = 0.55_f32;
    Color {
        r: bg.r + (fg.r - bg.r) * t,
        g: bg.g + (fg.g - bg.g) * t,
        b: bg.b + (fg.b - bg.b) * t,
        a: 1.0,
    }
}

/// Collect tree guide rects for all realized rows into a Vec.
///
/// Identical topology to `emit_tree_guide_segments` but converts each segment
/// into tight dashed rects (1px ON, 2px OFF). Used by single-mesh guide
/// rendering (push_rect_mesh).
///
/// # Arguments
/// * `realized_rows` - Currently realized row projections.
/// * `tree_data` - Engine-provided tree render metadata.
/// * `first_col_x` - X position of the first visible column (before indentation).
/// * `indent_step` - Pixels per depth level (TREE_INDENT_STEP).
/// * `line_width` - Width of guide lines in pixels.
/// * `viewport_y` - Y offset of body viewport (header height).
/// `content_phase_parity`: parity of the floored vertical scroll offset
/// (0 or 1). SPEC-PM-017 R1b: callers now pass viewport-relative
/// `y_offset`s, so the dot phase — anchored to content-local Y for scroll
/// stability — must add the scroll parity back to stay content-anchored.
pub fn build_tree_guide_rects(
    realized_rows: &[RowProjection],
    tree_data: &dyn TreeRenderData,
    first_col_x: f32,
    indent_step: f32,
    line_width: f32,
    viewport_y: f32,
    scale_factor: f32,
    content_phase_parity: i32,
) -> Vec<RectF> {
    let mut rects = Vec::new();
    emit_tree_guide_segments(
        realized_rows,
        tree_data,
        first_col_x,
        indent_step,
        line_width,
        viewport_y,
        |seg| {
            let r = seg.rect;
            // Dot pattern: 1px ON, 1px OFF (period 2). Windows 2000/7 style.
            // Each dot is a single pixel rect, pixel-snapped on both axes.
            match seg.kind {
                TreeGuideKind::Continuation
                | TreeGuideKind::ElbowVertical
                | TreeGuideKind::StemDown => {
                    // Vertical dots.
                    // Phase anchored to content-local Y for scroll stability.
                    let x = snap_to_physical(r.x, scale_factor);
                    let phase_base = (r.y - viewport_y).floor() as i32 + content_phase_parity;
                    let total = r.height.floor() as i32;
                    for dy in 0..total {
                        if (phase_base + dy).rem_euclid(2) == 0 {
                            rects.push(RectF::new(
                                x,
                                snap_to_physical(r.y + dy as f32, scale_factor),
                                line_width,
                                1.0,
                            ));
                        }
                    }
                }
                TreeGuideKind::ElbowHorizontal => {
                    // Horizontal dots.
                    let y = snap_to_physical(r.y, scale_factor);
                    let phase_base = r.x.floor() as i32;
                    let total = r.width.floor() as i32;
                    for dx in 0..total {
                        if (phase_base + dx).rem_euclid(2) == 0 {
                            rects.push(RectF::new(
                                snap_to_physical(r.x + dx as f32, scale_factor),
                                y,
                                1.0,
                                line_width,
                            ));
                        }
                    }
                }
            }
        },
    );
    rects
}

/// Compute center x position for a given depth level.
///
/// center_x(d) = first_col_x + d * indent_step + indent_step / 2
#[inline]
fn center_x(first_col_x: f32, indent_step: f32, depth: u32) -> f32 {
    first_col_x + depth as f32 * indent_step + indent_step * 0.5
}
