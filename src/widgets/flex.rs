// Flex layout widgets (FlexRow / FlexColumn)
//
// Invariants:
// - INV-FLEX-MEASURE-UNBOUNDED: All children measured unbounded;
//   grow distributes free space after measurement.

use crate::core::layout::{
    Alignment, Axis, Borders, Constraints, EdgeInsets, FlexWrap, Justification, LayoutResult,
    Margins, compute_box_model, shrink_constraints,
};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

/// Flex layout container
pub struct Flex {
    id: WidgetInternalId,
    axis: Axis,
    padding: EdgeInsets,
    spacing: f32,
    align_items: Alignment,
    justify_content: Justification,
    background: Option<Color>,
    wrap: FlexWrap,
}

/// FlexRow is a horizontal flex container
pub type FlexRow = Flex;

/// FlexColumn is a vertical flex container
pub type FlexColumn = Flex;

impl Flex {
    /// Create a horizontal flex container
    pub fn row() -> Self {
        Self {
            id: WidgetInternalId::new(),
            axis: Axis::Horizontal,
            padding: EdgeInsets::zero(),
            spacing: 0.0,
            align_items: Alignment::Start,
            justify_content: Justification::Start,
            background: None,
            wrap: FlexWrap::NoWrap,
        }
    }

    /// Create a vertical flex container
    pub fn column() -> Self {
        Self {
            id: WidgetInternalId::new(),
            axis: Axis::Vertical,
            padding: EdgeInsets::zero(),
            spacing: 0.0,
            align_items: Alignment::Start,
            justify_content: Justification::Start,
            background: None,
            wrap: FlexWrap::NoWrap,
        }
    }

    pub fn with_padding(mut self, padding: EdgeInsets) -> Self {
        self.padding = padding;
        self
    }

    pub fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    /// SPEC-HAL-319: Mutate spacing in place (used by ConfigureWidget).
    pub fn set_spacing(&mut self, spacing: f32) {
        self.spacing = spacing;
    }

    /// SPEC-HAL-319: Read main-axis direction (used by compact tree snapshot).
    pub fn axis(&self) -> Axis {
        self.axis
    }

    /// SPEC-HAL-319: Read main-axis spacing (used by compact tree snapshot).
    pub fn spacing(&self) -> f32 {
        self.spacing
    }

    pub fn with_align_items(mut self, align: Alignment) -> Self {
        self.align_items = align;
        self
    }

    pub fn with_justify_content(mut self, justify: Justification) -> Self {
        self.justify_content = justify;
        self
    }

    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    /// Enable wrapping (multi-line flex)
    pub fn with_wrap(mut self) -> Self {
        self.wrap = FlexWrap::Wrap;
        self
    }

    /// Explicitly force single-line behavior
    pub fn without_wrap(mut self) -> Self {
        self.wrap = FlexWrap::NoWrap;
        self
    }
}

// Axis helper functions
fn main_size(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.width,
        Axis::Vertical => size.height,
    }
}

fn cross_size(size: Size, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => size.height,
        Axis::Vertical => size.width,
    }
}

fn main_max(constraints: &Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.max_width,
        Axis::Vertical => constraints.max_height,
    }
}

fn cross_max(constraints: &Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.max_height,
        Axis::Vertical => constraints.max_width,
    }
}

fn cross_min(constraints: &Constraints, axis: Axis) -> f32 {
    match axis {
        Axis::Horizontal => constraints.min_height,
        Axis::Vertical => constraints.min_width,
    }
}

fn size_from_axes(main: f32, cross: f32, axis: Axis) -> Size {
    match axis {
        Axis::Horizontal => Size::new(main, cross),
        Axis::Vertical => Size::new(cross, main),
    }
}

/// Per-child flex item data
struct FlexItem {
    node: NodeId,
    /// Main-axis size used for flex calculation (basis or measured)
    base_main: f32,
    /// Original measured main-axis size (before basis override)
    measured_main: f32,
    cross: f32,
    final_main: f32,
    grow: f32,
    shrink: f32,
}

// Layout helper implementations
impl Flex {
    fn layout_no_wrap(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Box model setup
        let margin = Margins::zero();
        let border = Borders::none();
        let padding = self.padding;

        let content_constraints = shrink_constraints(constraints, margin, border, padding);

        let main_max_space = main_max(&content_constraints, self.axis);
        let cross_max_space = cross_max(&content_constraints, self.axis);

        let mut items: Vec<FlexItem> = Vec::with_capacity(children.len());

        for &child_id in children {
            let flex = cx.tree.get_flex(child_id);

            // INV-FLEX-MEASURE-UNBOUNDED: ALL children are measured with
            // unbounded main-axis constraints regardless of grow value.
            // Grow distribution operates on natural sizes, not bounded
            // measurements. Bounded measurement for grow>0 children causes
            // false overflow → shrink crushes grow=0 siblings.
            // See: Port/Timeout/Protocol row bug (2026-05-10).
            //
            // SPEC-PM-005 §7 EXCEPTION: fill-style widgets that declare
            // `needs_viewport_constraint = true` (SplitPane, ScrollArea,
            // virtualizing widgets) have no intrinsic main-axis size.
            // Measuring them unbounded returns 0 (their `min_*` fallback),
            // which then propagates as `Constraints::tight(W, 0)` from the
            // pass-2 relayout below and corrupts every descendant rect.
            // For these children only, bound the main axis at measurement.
            // INV-FLEX-MEASURE-UNBOUNDED continues to apply unchanged to
            // every grow-distributable widget (Label, Button, nested Flex,
            // etc.) where unbounded measurement yields natural sizes.
            let needs_bounded_main = cx
                .tree
                .node(child_id)
                .map(|n| n.widget.needs_viewport_constraint())
                .unwrap_or(false);
            let child_constraints = match self.axis {
                Axis::Horizontal => {
                    let main_max = if needs_bounded_main {
                        content_constraints.max_width
                    } else {
                        f32::INFINITY
                    };
                    Constraints::new(0.0, main_max, 0.0, content_constraints.max_height)
                }
                Axis::Vertical => {
                    let main_max = if needs_bounded_main {
                        content_constraints.max_height
                    } else {
                        f32::INFINITY
                    };
                    Constraints::new(0.0, content_constraints.max_width, 0.0, main_max)
                }
            };

            let measured_size = cx.layout_child(child_id, child_constraints);
            let measured_main = main_size(measured_size, self.axis);
            let measured_cross = cross_size(measured_size, self.axis);

            // Clamp infinity (safety net for nested layouts)
            let measured_main = if measured_main.is_infinite() {
                0.0
            } else {
                measured_main
            };
            let measured_cross = if measured_cross.is_infinite() {
                cross_max_space
            } else {
                measured_cross
            };

            // Invariant 4.1.1-2: Measurements must be valid (non-negative, non-NaN)
            debug_assert!(
                measured_main >= 0.0 && !measured_main.is_nan(),
                "Flex: child measured_main must be >= 0 and not NaN, got {}",
                measured_main
            );
            debug_assert!(
                measured_cross >= 0.0 && !measured_cross.is_nan(),
                "Flex: child measured_cross must be >= 0 and not NaN, got {}",
                measured_cross
            );

            let base_main = if flex.basis > 0.0 {
                flex.basis
            } else {
                measured_main
            };

            items.push(FlexItem {
                node: child_id,
                base_main,
                measured_main,
                cross: measured_cross,
                final_main: base_main,
                grow: flex.grow,
                shrink: flex.shrink,
            });
        }

        // Calculate total spacing
        let total_spacing = if items.len() > 1 {
            self.spacing * (items.len() as f32 - 1.0)
        } else {
            0.0
        };

        // Calculate total base size
        let total_base: f32 = items.iter().map(|it| it.base_main).sum::<f32>() + total_spacing;

        // Distribute space
        if total_base <= main_max_space {
            // Free space - distribute to growing items
            let free = (main_max_space - total_base).max(0.0);
            let total_grow: f32 = items.iter().map(|it| it.grow).sum();

            // Guard: if free space is infinite (parent gave unbounded
            // constraints, e.g., ScrollArea), don't distribute.
            // Children stay at their base_main.
            if total_grow > 0.0 && free.is_finite() {
                for it in &mut items {
                    let share = free * (it.grow / total_grow);
                    it.final_main = it.base_main + share;
                }
            }
        } else {
            // Overflow - shrink items
            let deficit = total_base - main_max_space;
            let total_shrink_weight: f32 = items.iter().map(|it| it.base_main * it.shrink).sum();

            if total_shrink_weight > 0.0 {
                for it in &mut items {
                    let weight = it.base_main * it.shrink;
                    let shrink_amount = deficit * (weight / total_shrink_weight);
                    it.final_main = (it.base_main - shrink_amount).max(0.0);
                }
            }
        }

        // Calculate total final main size
        let total_final_main: f32 =
            items.iter().map(|it| it.final_main).sum::<f32>() + total_spacing;

        // Invariant 4.2.1-2: Distribution results must be valid
        for it in &items {
            debug_assert!(
                it.final_main >= 0.0,
                "Flex: final_main must be >= 0, got {}",
                it.final_main
            );
        }
        // Note: total_final_main may exceed main_max_space (content overflow).
        // This is valid in scroll contexts where ScrollArea passes bounded viewport
        // constraints (SPEC-HAL-086 Stage 8) and non-shrinkable children exceed
        // available space. Overflow is reported via LayoutResult.content_size;
        // returned size is clamped to constraints below.

        // Re-layout pass: give children tight main-axis constraints so their
        // internal layout uses the correct allocated size (CSS Flexbox behavior).
        // This fixes children that measured with loose constraints but now have
        // a different final size after grow/shrink distribution.
        for it in &mut items {
            // Skip children whose size didn't change
            if (it.final_main - it.base_main).abs() < 0.5 {
                continue;
            }
            let tight = match self.axis {
                Axis::Horizontal => {
                    Constraints::new(it.final_main, it.final_main, 0.0, cross_max_space)
                }
                Axis::Vertical => {
                    Constraints::new(0.0, cross_max_space, it.final_main, it.final_main)
                }
            };
            let re_measured = cx.layout_child(it.node, tight);
            it.cross = cross_size(re_measured, self.axis);
        }

        // Cross axis sizing - respect min constraint for tight layouts
        let max_child_cross = items.iter().map(|it| it.cross).fold(0.0, f32::max);
        let cross_min_space = cross_min(&content_constraints, self.axis);
        let cross_size_final =
            if self.align_items == Alignment::Stretch && cross_max_space.is_finite() {
                cross_max_space
            } else {
                max_child_cross.max(cross_min_space).min(cross_max_space)
            };

        // Invariant 4.3.2: Cross size respects constraints
        debug_assert!(
            cross_size_final >= 0.0,
            "Flex: cross_size_final must be >= 0, got {}",
            cross_size_final
        );
        debug_assert!(
            cross_size_final >= cross_min_space - 0.1 && cross_size_final <= cross_max_space + 0.1,
            "Flex: cross_size_final ({}) must be within [{}, {}]",
            cross_size_final,
            cross_min_space,
            cross_max_space
        );

        // Main axis positioning based on justification
        let space_left = (main_max_space - total_final_main).max(0.0);

        let (mut cursor, gap) = match self.justify_content {
            Justification::Start => (0.0, self.spacing),
            Justification::End => (space_left, self.spacing),
            Justification::Center => (space_left / 2.0, self.spacing),
            Justification::SpaceBetween => {
                let gap = if items.len() > 1 {
                    space_left / (items.len() as f32 - 1.0)
                } else {
                    0.0
                };
                (0.0, gap)
            }
            Justification::SpaceAround => {
                let gap = if !items.is_empty() {
                    space_left / items.len() as f32
                } else {
                    0.0
                };
                (gap / 2.0, gap)
            }
            Justification::SpaceEvenly => {
                let gap = if !items.is_empty() {
                    space_left / (items.len() as f32 + 1.0)
                } else {
                    0.0
                };
                (gap, gap)
            }
        };

        // Compute content offset from box model
        let bm = compute_box_model(Size::zero(), margin, border, padding);
        let (content_offset_x, content_offset_y) = bm.content_offset;

        // Position each child
        for it in &items {
            // FIX #3: ScrollArea MUST NEVER have its cross-axis shrunk
            // If child is ScrollArea, force item_cross = cross_max_space
            let is_scroll_area = cx.tree.wants_scroll(it.node);

            // Cross axis alignment
            let item_cross = if is_scroll_area {
                // FIX #3: ScrollArea always gets full cross-axis space
                cross_max_space
            } else if self.align_items == Alignment::Stretch {
                cross_size_final
            } else {
                it.cross
            };

            let origin_cross = match self.align_items {
                Alignment::Start => 0.0,
                Alignment::End => (cross_size_final - item_cross).max(0.0),
                Alignment::Center => ((cross_size_final - item_cross) / 2.0).max(0.0),
                Alignment::Stretch => 0.0,
            };

            // Convert to (x, y, w, h) based on axis
            let rect = match self.axis {
                Axis::Horizontal => RectF::new(
                    content_offset_x + cursor,
                    content_offset_y + origin_cross,
                    it.final_main,
                    item_cross,
                ),
                Axis::Vertical => RectF::new(
                    content_offset_x + origin_cross,
                    content_offset_y + cursor,
                    item_cross,
                    it.final_main,
                ),
            };

            cx.set_child_rect(it.node, rect);
            cursor += it.final_main + gap;
        }

        // Second layout pass: re-layout children with TIGHT constraints based on computed rects.
        // This ensures children know their assigned size (critical for ScrollArea viewport).
        // For Vertical (column): width tight (cross), height tight (main)
        // For Horizontal (row): width tight (main), height tight (cross)
        //
        // SPEC-PM-005: keep natural main-axis measurement unbounded. Fill
        // children can have sentinel base_main=0, which must not become a
        // tight main allocation. A finite Stretch cross allocation still
        // needs to reach descendants, independently of main-axis sizing.
        let bounded_main = main_max_space.is_finite();
        // Stretch still propagates its finite cross allocation during natural
        // main-axis measurement; never tighten a sentinel main extent to zero.
        if bounded_main || (self.align_items == Alignment::Stretch && cross_size_final.is_finite())
        {
            for it in &items {
                if let Some(rect) = cx.tree.get_rect(it.node) {
                    if bounded_main {
                        // HAL-026: equal dimensions do not make loose measurement
                        // equivalent to final allocation for nested viewport owners.
                        let size = cx.layout_child(
                            it.node,
                            Constraints::new(rect.width, rect.width, rect.height, rect.height),
                        );
                        cx.set_child_rect(
                            it.node,
                            RectF::new(rect.x, rect.y, size.width, size.height),
                        );
                        continue;
                    }

                    // Natural main-axis probes retain the existing Stretch path.
                    let allocated_cross = match self.axis {
                        Axis::Horizontal => rect.height,
                        Axis::Vertical => rect.width,
                    };
                    let basis_neutral = (it.base_main - it.measured_main).abs() < 0.5;
                    let main_unchanged = (it.final_main - it.measured_main).abs() < 0.5;
                    let cross_unchanged = (allocated_cross - it.cross).abs() < 0.5;
                    let needs_viewport = cx
                        .tree
                        .node(it.node)
                        .map(|n| n.widget.needs_viewport_constraint())
                        .unwrap_or(false);
                    if basis_neutral && main_unchanged && cross_unchanged && !needs_viewport {
                        continue;
                    }
                    if cross_unchanged {
                        continue;
                    }
                    let constraints = match self.axis {
                        Axis::Horizontal => {
                            Constraints::new(0.0, f32::INFINITY, rect.height, rect.height)
                        }
                        Axis::Vertical => {
                            Constraints::new(rect.width, rect.width, 0.0, f32::INFINITY)
                        }
                    };
                    cx.relayout_child_with_constraints(it.node, constraints);
                }
            }
        }

        // INV-FLEX-NOWRAP-REPOSITION: Reposition children after second pass.
        // Child heights/widths may have changed during relayout (e.g., FlowGrid rewrap).
        // Recompute positions using actual rect sizes, not measured values.
        let mut reposition_cursor = 0.0;
        for it in &items {
            if let Some(old_rect) = cx.tree.get_rect(it.node) {
                // Get actual main-axis size from the rect (updated by relayout)
                let actual_main = main_size(Size::new(old_rect.width, old_rect.height), self.axis);

                // Recompute cross-axis alignment with actual cross size
                let actual_cross =
                    cross_size(Size::new(old_rect.width, old_rect.height), self.axis);
                let is_scroll_area = cx.tree.wants_scroll(it.node);

                let item_cross = if is_scroll_area {
                    cross_max_space
                } else if self.align_items == Alignment::Stretch {
                    cross_size_final
                } else {
                    actual_cross
                };

                let origin_cross = match self.align_items {
                    Alignment::Start => 0.0,
                    Alignment::End => (cross_size_final - item_cross).max(0.0),
                    Alignment::Center => ((cross_size_final - item_cross) / 2.0).max(0.0),
                    Alignment::Stretch => 0.0,
                };

                // Build new rect with updated position, preserving size from relayout
                let new_rect = match self.axis {
                    Axis::Horizontal => RectF::new(
                        content_offset_x + reposition_cursor,
                        content_offset_y + origin_cross,
                        old_rect.width,
                        item_cross,
                    ),
                    Axis::Vertical => RectF::new(
                        content_offset_x + origin_cross,
                        content_offset_y + reposition_cursor,
                        item_cross,
                        old_rect.height,
                    ),
                };

                cx.set_child_rect(it.node, new_rect);
                reposition_cursor += actual_main + gap;
            }
        }

        // INV-FLEX-NOWRAP-SHRINKWRAP: Return intrinsic content size, not constraint.
        // Flex must shrink-wrap to content unless children have grow > 0.
        // max_height/max_width is an upper bound, not a sizing directive.
        //
        // Use reposition_cursor (minus trailing gap) as the true content size.
        // This reflects actual heights AFTER the second pass relayout, not measured heights.
        let actual_content_main = if !items.is_empty() {
            reposition_cursor - gap // remove trailing gap
        } else {
            0.0
        };

        let content = size_from_axes(actual_content_main, cross_size_final, self.axis);

        // Add box model back
        let bm = compute_box_model(content, margin, border, padding);

        // Constrained size (clamped to constraints) - what we render as
        let constrained_size = constraints.constrain(bm.size);

        // Full content size (unclamped) - for scroll extent calculation
        // Aggregate children's content_sizes to get true scroll extent
        let mut total_content_main = 0.0;
        let mut max_content_cross = 0.0f32;
        for (idx, it) in items.iter().enumerate() {
            if let Some(child_content_size) = cx.tree.get_content_size(it.node) {
                let child_main = main_size(child_content_size, self.axis);
                let child_cross = cross_size(child_content_size, self.axis);
                total_content_main += child_main;
                max_content_cross = max_content_cross.max(child_cross);
                if idx + 1 < items.len() {
                    total_content_main += gap;
                }
            }
        }
        // Use the larger of actual size and aggregated content_size for scroll extent
        // PM-005-R1: Report pre-shrink natural size as content_size.
        // total_base is the measurement-pass total (natural child sizes + spacing).
        // When shrink compresses children, actual_content_main and total_content_main
        // reflect compressed sizes. ScrollArea needs the natural extent to detect
        // overflow and enable scrolling.
        let full_content_main = total_content_main.max(actual_content_main).max(total_base);
        let full_content_cross = max_content_cross.max(cross_size_final);
        let full_content = size_from_axes(full_content_main, full_content_cross, self.axis);
        let full_bm = compute_box_model(full_content, margin, border, padding);
        let full_content_size = full_bm.size;

        LayoutResult::new(constrained_size, full_content_size)
    }

    fn layout_wrap(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // HAL_PROFILE_RESIZE: Count layout_wrap calls
        crate::core::app::profiler_inc_layout_wrap_call();

        // Same box model setup as in layout_no_wrap
        let margin = Margins::zero();
        let border = Borders::none();
        let padding = self.padding;

        let content_constraints = shrink_constraints(constraints, margin, border, padding);

        let main_max_space = main_max(&content_constraints, self.axis);
        let cross_max_space = cross_max(&content_constraints, self.axis);

        // ---------- 1) MEASURE ALL CHILDREN ----------
        // Use loose constraints so children report their natural size
        // (needed for flex distribution and alignment to work properly)
        let measure_constraints = Constraints::new(
            0.0,
            content_constraints.max_width,
            0.0,
            content_constraints.max_height,
        );
        let mut measured_items: Vec<FlexItem> = Vec::with_capacity(children.len());

        for &child_id in children {
            let flex = cx.tree.get_flex(child_id);
            let measured_size = cx.layout_child(child_id, measure_constraints);

            let measured_main = main_size(measured_size, self.axis);
            let measured_cross = cross_size(measured_size, self.axis);

            // Invariant 4.1.1-2: Measurements must be valid (non-negative, non-NaN)
            debug_assert!(
                measured_main >= 0.0 && !measured_main.is_nan(),
                "Flex wrap: child measured_main must be >= 0 and not NaN, got {}",
                measured_main
            );
            debug_assert!(
                measured_cross >= 0.0 && !measured_cross.is_nan(),
                "Flex wrap: child measured_cross must be >= 0 and not NaN, got {}",
                measured_cross
            );

            let base_main = if flex.basis > 0.0 {
                flex.basis
            } else {
                measured_main
            };

            measured_items.push(FlexItem {
                node: child_id,
                base_main,
                measured_main,
                cross: measured_cross,
                final_main: base_main,
                grow: flex.grow,
                shrink: flex.shrink,
            });
        }

        // Early-out: no children
        if measured_items.is_empty() {
            let bm = compute_box_model(Size::zero(), margin, border, padding);
            return LayoutResult::fit(constraints.constrain(bm.size));
        }

        // ---------- 2) GROUP INTO LINES ----------
        struct FlexLine {
            items: Vec<FlexItem>,
            base_main_sum: f32,
            cross_max: f32,
        }

        let mut lines: Vec<FlexLine> = Vec::new();
        let mut current = FlexLine {
            items: Vec::new(),
            base_main_sum: 0.0,
            cross_max: 0.0,
        };

        for item in measured_items.into_iter() {
            let item_main = item.base_main;
            let item_cross = item.cross;

            // main size if we add this item to current line
            let predicted = if current.items.is_empty() {
                item_main
            } else {
                current.base_main_sum + self.spacing + item_main
            };

            let should_wrap = !current.items.is_empty()
                && main_max_space.is_finite()
                && predicted > main_max_space;

            if should_wrap {
                // close current line
                lines.push(current);
                current = FlexLine {
                    items: Vec::new(),
                    base_main_sum: 0.0,
                    cross_max: 0.0,
                };
            }

            if !current.items.is_empty() {
                current.base_main_sum += self.spacing;
            }
            current.base_main_sum += item_main;
            current.cross_max = current.cross_max.max(item_cross);
            current.items.push(item);
        }

        if !current.items.is_empty() {
            lines.push(current);
        }

        // ---------- 3) FLEX DISTRIBUTION PER LINE ----------
        fn distribute_line(line: &mut FlexLine, main_limit: f32, spacing: f32) -> f32 {
            let count = line.items.len();
            if count == 0 {
                return 0.0;
            }

            let total_spacing = if count > 1 {
                spacing * (count as f32 - 1.0)
            } else {
                0.0
            };

            let base_sum: f32 = line.items.iter().map(|it| it.base_main).sum::<f32>();
            let total_base = base_sum + total_spacing;

            if !main_limit.is_finite() {
                // unbounded main axis: keep base sizes
                for it in &mut line.items {
                    it.final_main = it.base_main;
                }
                return total_base;
            }

            if total_base <= main_limit {
                // GROW
                let free = (main_limit - total_base).max(0.0);
                let total_grow: f32 = line.items.iter().map(|it| it.grow).sum();

                if total_grow > 0.0 {
                    for it in &mut line.items {
                        let share = free * (it.grow / total_grow);
                        it.final_main = it.base_main + share;
                    }
                } else {
                    for it in &mut line.items {
                        it.final_main = it.base_main;
                    }
                }
            } else {
                // SHRINK
                let deficit = total_base - main_limit;
                let total_shrink_weight: f32 =
                    line.items.iter().map(|it| it.base_main * it.shrink).sum();

                if total_shrink_weight > 0.0 {
                    for it in &mut line.items {
                        let weight = it.base_main * it.shrink;
                        let shrink_amount = deficit * (weight / total_shrink_weight);
                        it.final_main = (it.base_main - shrink_amount).max(0.0);
                    }
                } else {
                    for it in &mut line.items {
                        it.final_main = it.base_main;
                    }
                }
            }

            // Invariant 4.2.1: Distribution results must be valid
            for it in &line.items {
                debug_assert!(
                    it.final_main >= 0.0,
                    "Flex wrap: final_main must be >= 0, got {}",
                    it.final_main
                );
            }

            // return final total main for this line
            line.items.iter().map(|it| it.final_main).sum::<f32>() + total_spacing
        }

        let num_lines = lines.len();
        let mut line_final_mains: Vec<f32> = Vec::with_capacity(num_lines);

        for line in lines.iter_mut() {
            let line_main = distribute_line(line, main_max_space, self.spacing);
            line_final_mains.push(line_main);
        }

        // Note: cross_size_final will be computed after second pass with updated heights

        // ---------- 4) POSITION CHILDREN ----------
        let bm = compute_box_model(Size::zero(), margin, border, padding);
        let (content_offset_x, content_offset_y) = bm.content_offset;

        let mut cross_cursor = 0.0;

        for (line_index, line) in lines.iter().enumerate() {
            let line_main_total = line_final_mains[line_index];
            let line_cross = line.cross_max;

            let space_left = if main_max_space.is_finite() {
                (main_max_space - line_main_total).max(0.0)
            } else {
                0.0
            };

            let (mut main_cursor, gap) = match self.justify_content {
                Justification::Start => (0.0, self.spacing),
                Justification::End => (space_left, self.spacing),
                Justification::Center => (space_left / 2.0, self.spacing),
                Justification::SpaceBetween => {
                    let gap = if line.items.len() > 1 {
                        space_left / (line.items.len() as f32 - 1.0)
                    } else {
                        0.0
                    };
                    (0.0, gap)
                }
                Justification::SpaceAround => {
                    let gap = if !line.items.is_empty() {
                        space_left / line.items.len() as f32
                    } else {
                        0.0
                    };
                    (gap / 2.0, gap)
                }
                Justification::SpaceEvenly => {
                    let gap = if !line.items.is_empty() {
                        space_left / (line.items.len() as f32 + 1.0)
                    } else {
                        0.0
                    };
                    (gap, gap)
                }
            };

            for (idx, it) in line.items.iter().enumerate() {
                // FIX #3: ScrollArea MUST NEVER have its cross-axis shrunk
                let is_scroll_area = cx.tree.wants_scroll(it.node);

                let item_cross = if is_scroll_area {
                    // FIX #3: ScrollArea always gets full cross-axis space
                    cross_max_space
                } else if self.align_items == Alignment::Stretch {
                    line_cross
                } else {
                    it.cross
                };

                let origin_cross = match self.align_items {
                    Alignment::Start => 0.0,
                    Alignment::End => (line_cross - item_cross).max(0.0),
                    Alignment::Center => ((line_cross - item_cross) / 2.0).max(0.0),
                    Alignment::Stretch => 0.0,
                };

                let rect = match self.axis {
                    Axis::Horizontal => RectF::new(
                        content_offset_x + main_cursor,
                        content_offset_y + cross_cursor + origin_cross,
                        it.final_main,
                        item_cross,
                    ),
                    Axis::Vertical => RectF::new(
                        content_offset_x + cross_cursor + origin_cross,
                        content_offset_y + main_cursor,
                        item_cross,
                        it.final_main,
                    ),
                };

                cx.set_child_rect(it.node, rect);

                main_cursor += it.final_main;
                if idx + 1 < line.items.len() {
                    main_cursor += gap;
                }
            }

            cross_cursor += line_cross;
            if line_index + 1 < lines.len() {
                cross_cursor += self.spacing;
            }
        }

        // Second layout pass: re-layout children with tight width, loose height.
        // This allows children with fixed height (e.g., Container.with_height(50)) to compute their own height.
        //
        // OPTIMIZATION: Skip relayout when we can prove the result would be identical.
        //
        // Pass 1 constraints: (0, max_width, 0, max_height) - loose
        // Pass 2 constraints: (rect.width, rect.width, 0, INFINITY) - tight width, unbounded height
        //
        // Skip condition: Width unchanged AND height unconstrained (provably safe):
        //   measured_main == final_main AND cross_max_space was INFINITY
        //   → Both passes have identical effective constraints
        //
        // NOTE: We do NOT skip based on height_stable() because that only guarantees
        // the node's own height won't change - children still need constraint propagation.
        let cross_unconstrained = cross_max_space.is_infinite();

        for line in &lines {
            for it in &line.items {
                let width_unchanged = (it.measured_main - it.final_main).abs() < 0.001;

                // Only safe to skip when width is truly unchanged AND cross is unconstrained.
                // height_stable() only guarantees the node's own height won't change - it does
                // NOT mean children are width-insensitive. Children must receive new constraints
                // when width changes, even if parent height is stable.
                if cross_unconstrained && width_unchanged {
                    continue;
                }

                // Width changed: must relayout to propagate constraints to children
                if let Some(rect) = cx.tree.get_rect(it.node) {
                    let constraints = Constraints::new(rect.width, rect.width, 0.0, f32::INFINITY);
                    cx.relayout_child_with_constraints(it.node, constraints);
                }
            }
        }

        // ---------- 4.5) UPDATE LINE HEIGHTS AFTER SECOND PASS ----------
        // Children may have changed height after relayout, so recompute line cross_max
        // and update child positions accordingly
        let mut updated_cross_total = 0.0;
        let mut line_cross_heights: Vec<f32> = Vec::with_capacity(lines.len());

        for line in &lines {
            let mut line_cross_max = 0.0f32;
            for it in &line.items {
                if let Some(rect) = cx.tree.get_rect(it.node) {
                    let child_cross = cross_size(Size::new(rect.width, rect.height), self.axis);
                    line_cross_max = line_cross_max.max(child_cross);
                }
            }
            line_cross_heights.push(line_cross_max);
            updated_cross_total += line_cross_max;
        }

        // Add spacing between lines
        if lines.len() > 1 {
            updated_cross_total += self.spacing * (lines.len() as f32 - 1.0);
        }

        // INV-FLEX-CONTAINMENT: Cross size must contain all children.
        // Don't clamp below child extent, but respect min constraint.
        let cross_min_space = cross_min(&content_constraints, self.axis);
        let cross_size_final = updated_cross_total.max(cross_min_space);

        // Re-position children with updated line heights
        let mut cross_cursor = 0.0;
        for (line_index, line) in lines.iter().enumerate() {
            let line_cross = line_cross_heights[line_index];
            let line_main_total = line_final_mains[line_index];

            let space_left = if main_max_space.is_finite() {
                (main_max_space - line_main_total).max(0.0)
            } else {
                0.0
            };

            let (mut main_cursor, gap) = match self.justify_content {
                Justification::Start => (0.0, self.spacing),
                Justification::End => (space_left, self.spacing),
                Justification::Center => (space_left / 2.0, self.spacing),
                Justification::SpaceBetween => {
                    let gap = if line.items.len() > 1 {
                        space_left / (line.items.len() as f32 - 1.0)
                    } else {
                        0.0
                    };
                    (0.0, gap)
                }
                Justification::SpaceAround => {
                    let gap = if !line.items.is_empty() {
                        space_left / line.items.len() as f32
                    } else {
                        0.0
                    };
                    (gap / 2.0, gap)
                }
                Justification::SpaceEvenly => {
                    let gap = if !line.items.is_empty() {
                        space_left / (line.items.len() as f32 + 1.0)
                    } else {
                        0.0
                    };
                    (gap, gap)
                }
            };

            for (idx, it) in line.items.iter().enumerate() {
                if let Some(old_rect) = cx.tree.get_rect(it.node) {
                    // FIX #3: ScrollArea MUST NEVER have its cross-axis shrunk
                    let is_scroll_area = cx.tree.wants_scroll(it.node);

                    let item_cross = if is_scroll_area {
                        // FIX #3: ScrollArea always gets full cross-axis space
                        cross_max_space
                    } else if self.align_items == Alignment::Stretch {
                        line_cross
                    } else {
                        cross_size(Size::new(old_rect.width, old_rect.height), self.axis)
                    };

                    let origin_cross = match self.align_items {
                        Alignment::Start => 0.0,
                        Alignment::End => (line_cross - item_cross).max(0.0),
                        Alignment::Center => ((line_cross - item_cross) / 2.0).max(0.0),
                        Alignment::Stretch => 0.0,
                    };

                    let rect = match self.axis {
                        Axis::Horizontal => RectF::new(
                            content_offset_x + main_cursor,
                            content_offset_y + cross_cursor + origin_cross,
                            old_rect.width,
                            item_cross,
                        ),
                        Axis::Vertical => RectF::new(
                            content_offset_x + cross_cursor + origin_cross,
                            content_offset_y + main_cursor,
                            item_cross,
                            old_rect.height,
                        ),
                    };

                    cx.set_child_rect(it.node, rect);

                    main_cursor += match self.axis {
                        Axis::Horizontal => old_rect.width,
                        Axis::Vertical => old_rect.height,
                    };
                    if idx + 1 < line.items.len() {
                        main_cursor += gap;
                    }
                }
            }

            cross_cursor += line_cross;
            if line_index + 1 < lines.len() {
                cross_cursor += self.spacing;
            }
        }

        // ---------- 5) FINAL CONTAINER SIZE ----------
        let final_main = if main_max_space.is_finite() {
            main_max_space
        } else {
            line_final_mains.into_iter().fold(0.0, f32::max) // widest line
        };

        // INV-FLEX-CONTAINMENT: Size must fully contain all children.
        // Don't clamp cross axis below child extent even if it exceeds constraints.
        let content = size_from_axes(final_main, cross_size_final, self.axis);
        let bm = compute_box_model(content, margin, border, padding);
        let constrained_size = bm.size;

        // Full content size uses unclamped cross dimension
        let full_content = size_from_axes(final_main, updated_cross_total, self.axis);
        let full_bm = compute_box_model(full_content, margin, border, padding);
        let full_content_size = full_bm.size;

        LayoutResult::new(constrained_size, full_content_size)
    }
}

impl WidgetDowncast for Flex {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Flex {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        match self.wrap {
            FlexWrap::NoWrap => {
                // Non-wrapped: returns LayoutResult with proper content_size for scroll
                self.layout_no_wrap(cx, constraints, children)
            }
            FlexWrap::Wrap => {
                // Wrapped: may overflow, returns LayoutResult with proper content_size
                self.layout_wrap(cx, constraints, children)
            }
        }
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Paint background if set
        if let Some(color) = self.background {
            cx.scene.push_rect(rect, color, 0, cx.scene_key);
        }
        // Children are painted by traversal
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "Flex"
    }

    fn layout_axis(&self) -> Option<crate::core::layout::Axis> {
        Some(self.axis)
    }
}
