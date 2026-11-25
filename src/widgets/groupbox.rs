// GroupBox Widget - Fieldset-style group box with inline caption

use crate::core::layout::{Constraints, LayoutResult, Padding};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT, RoundedCorners, ZIndex};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::ControlChromeCategory;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

const GROUPBOX_LABEL_TEXT_LAYER: ZIndex = LAYER_CONTENT + 4;

#[derive(Default)]
struct LegendLayout {
    font_size: f32,
    width: f32,
    height: f32,
    frame_center_y: f32,
    text_top_y: f32,
    margin_left: f32,
    gap: f32,
    radius: f32,
}

/// GroupBox widget - fieldset-style container with inline caption
///
/// GroupBox wraps a single content child and displays a caption text
/// inline with the top border (like HTML fieldset/groupbox).
pub struct GroupBox {
    id: WidgetInternalId,
    /// Wrapped content node. `None` when constructed standalone via
    /// `GroupBox::empty` for SPEC-HAL-319 composition; the agent adds
    /// children later via `WidgetTree::add_child`.
    pub content: Option<NodeId>,
    pub label: String,
    pub padding: Padding,
    pub border_thickness: f32,
    /// Border color. None = use theme.colors.borders.default (default).
    pub border_color: Option<Color>,
    pub background: Option<Color>,
    pub label_margin_left: f32,
    pub label_gap: f32,
    /// Legend font size in logical units, used directly for measurement and shaping.
    pub label_size: f32,
    /// Label text color. None = use theme.colors.typography.primary (default).
    pub label_color: Option<Color>,
    padding_override: Option<Padding>,
    legend: LegendLayout,
}

impl GroupBox {
    pub fn new(content: NodeId, label: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            content: Some(content),
            label: label.into(),
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme in layout()
            padding: Padding::all(8.0),
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme in layout()
            border_thickness: 1.0,
            border_color: None, // Default: use theme.colors.borders.default
            background: None,
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme in layout()
            label_margin_left: 8.0,
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme in layout()
            label_gap: 6.0,
            // INV-TYPOGRAPHY-LITERAL: Font size is typography, not geometry
            // SPEC-HAL-096B Section 5.1 forbids typography substitution
            label_size: 14.0,
            label_color: None, // Default: use theme.colors.typography.primary
            padding_override: None,
            legend: LegendLayout::default(),
        }
    }

    /// SPEC-HAL-319: Standalone constructor for runtime composition.
    /// No content child — the agent adds children later via the tree.
    pub fn empty(label: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            content: None,
            label: label.into(),
            padding: Padding::all(8.0),
            border_thickness: 1.0,
            border_color: None,
            background: None,
            label_margin_left: 8.0,
            label_gap: 6.0,
            label_size: 14.0,
            label_color: None,
            padding_override: None,
            legend: LegendLayout::default(),
        }
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        let padding = Padding::all(padding);
        self.padding = padding;
        self.padding_override = Some(padding);
        self
    }

    pub fn with_edge_padding(mut self, padding: Padding) -> Self {
        self.padding = padding;
        self.padding_override = Some(padding);
        self
    }

    pub fn with_border(mut self, thickness: f32, color: Color) -> Self {
        self.border_thickness = thickness;
        self.border_color = Some(color);
        self
    }

    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn with_label_color(mut self, color: Color) -> Self {
        self.label_color = Some(color);
        self
    }

    /// Sets the effective legend font size without applying an internal scale factor.
    pub fn with_label_size(mut self, size: f32) -> Self {
        self.label_size = size;
        self
    }

    fn resolved_padding(&self, theme_padding: f32) -> Padding {
        self.padding_override
            .unwrap_or_else(|| Padding::all(theme_padding))
    }

    fn paint_legend(
        &self,
        cx: &mut PaintContext,
        bounds: RectF,
        gap_start: f32,
        gap_end: f32,
        text_x: f32,
        text_y: f32,
        title_fits: bool,
        panel_fill: Option<Color>,
    ) {
        if self.label.is_empty() || gap_end <= gap_start {
            return;
        }
        let one_px = cx.one_px();
        let chip = RectF::new(
            gap_start,
            text_y - 2.0 * one_px,
            gap_end - gap_start,
            self.legend.height + 4.0 * one_px,
        );
        let Some(chip) = chip.intersection(&bounds) else {
            return;
        };
        let corners = RoundedCorners::uniform(self.legend.radius);
        if let Some(panel_fill) = panel_fill {
            cx.scene
                .push_rounded_rect(chip, panel_fill, corners, 3, cx.scene_key);
        }
        cx.scene.push_border_rounded_rect(
            chip,
            self.border_color.unwrap_or(cx.theme.colors.borders.subtle),
            one_px,
            corners,
            5,
            cx.scene_key,
        );
        let clip_right = if title_fits {
            gap_end
        } else {
            gap_end - self.legend.gap
        };
        if clip_right <= text_x {
            return;
        }
        cx.scene.begin_clip(
            RectF::new(gap_start, bounds.y, clip_right - gap_start, bounds.height),
            None,
        );
        let run = cx.text_engine.shape(&self.label, self.legend.font_size);
        cx.text_engine.draw(
            &run,
            text_x,
            text_y,
            self.label_color
                .unwrap_or(cx.theme.colors.typography.primary),
            GROUPBOX_LABEL_TEXT_LAYER,
            cx.scene,
        );
        cx.scene.end_clip();
    }
}

impl WidgetDowncast for GroupBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for GroupBox {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // INV-GEOM-SYNC-LEGEND: Theme geometry sync (CANONICAL LOCATION)
        // Theme → local sync MUST happen here in layout(), before paint/event.
        // Paint/event code MUST NOT read theme directly for geometry.
        // Constructor defaults are inert fallbacks only.
        let padding = self.resolved_padding(cx.theme.padding.m);
        let border_thickness = cx.theme.borders.b1;

        // 1. Measure the configured legend size; Paint uses these same metrics.
        let label_font_size = self.label_size;
        let metrics = cx.text_engine.font_metrics(label_font_size);
        let glyph_height = metrics.ascent + metrics.descent;
        let label_height = metrics
            .line_height
            .max(glyph_height + 2.0 * cx.theme.typography.baseline_offset.abs());
        let text_top = compute_control_text_baseline_y(
            label_height,
            metrics.ascent,
            metrics.descent,
            cx.theme.typography.baseline_offset,
        ) - metrics.ascent;
        let frame_center_y = text_top + glyph_height * 0.5;
        self.legend = LegendLayout {
            font_size: label_font_size,
            width: cx.text_engine.measure(&self.label, label_font_size).0,
            height: glyph_height,
            frame_center_y,
            text_top_y: text_top,
            margin_left: padding.left,
            gap: cx.theme.spacing.s,
            radius: if self.border_color.is_some() {
                0.0
            } else {
                cx.theme.chrome_radius(
                    ControlChromeCategory::SectionContainer,
                    cx.theme.radii.xs,
                    false,
                )
            },
        };

        // 2. Content starts below the caption AND the border, plus padding_top.
        // The caption is centered on the border, so its bottom edge sits at
        // `rect.y + label_height`. Earlier we used `extra_above + border +
        // padding.top` (≈ label_height / 2 + half-border + padding), which
        // made the first child crowd the title — content started at roughly
        // the caption baseline. Take the larger of the caption bottom and
        // the border bottom, then add padding.
        let content_y = label_height
            .max(text_top + glyph_height)
            .max(border_thickness)
            + padding.top;

        // 4. Derive inner constraints for content
        // CRITICAL: Always give child INFINITY height so it reports true intrinsic size.
        // GroupBox wraps to content - it should never constrain child height.
        // This fixes width-dependent height changes (FlowGrid rewrap during resize).
        let content_width = (constraints.max_width - padding.left - padding.right).max(0.0);
        // The stretched allocation must reach child Layout before it lays out
        // descendants; set_child_rect alone cannot resize their geometry.
        let inner_constraints = Constraints::new(
            if content_width.is_finite() {
                content_width
            } else {
                0.0
            },
            content_width,
            0.0,
            f32::INFINITY,
        );

        // FIX #4: GroupBox MUST NOT shrink content width
        // Content width = inner_constraints.max_width (full available)
        // Self width = constraints.max_width (parent-assigned)
        let content_max_width = inner_constraints.max_width;

        // 5. Layout content
        // SPEC-HAL-086 Stage 2: Extract size and content_size from LayoutResult
        let (child_size, child_content_size) = if let Some(content_id) = self.content {
            // SPEC-HAL-026 §8: descend via cx.layout_child_full. The full
            // variant returns LayoutResult so content_size propagates to
            // ScrollArea for scroll-extent calculation. Engine handles
            // dirty-bit clear, pre_layout, deferred_wiring, current_node,
            // cached_size, last_constraints, layout_epoch. Missing-content
            // structural corruption surfaces as the engine's INV-ZERO-2
            // panic (replaces the prior silent Size::zero return, which
            // violated SPEC-HAL-158 INV-ZERO-2).
            let result = cx.layout_child_full(content_id, inner_constraints);
            let size = result.size;
            let content_size = result.content_size;

            // FIX #4: Position content with FULL available width, not content's natural width
            let content_rect = RectF::new(padding.left, content_y, content_max_width, size.height);
            cx.set_child_rect(content_id, content_rect);

            (size, content_size)
        } else if !children.is_empty() {
            // SPEC-HAL-319: Standalone GroupBox composed at runtime — no
            // single content child. Stack the live tree's children
            // vertically inside the padded content area with theme spacing.
            let spacing = cx.theme.spacing.s;
            let mut total = 0.0_f32;
            for (i, &child_id) in children.iter().enumerate() {
                if i > 0 {
                    total += spacing;
                }
                let child_size = cx.layout_child(child_id, inner_constraints);
                cx.set_child_rect(
                    child_id,
                    RectF::new(
                        padding.left,
                        content_y + total,
                        content_max_width,
                        child_size.height,
                    ),
                );
                total += child_size.height;
            }
            let stacked = Size::new(content_max_width, total);
            (stacked, stacked)
        } else {
            // SPEC-HAL-319: Standalone GroupBox with no children yet.
            (Size::zero(), Size::zero())
        };

        // 7. Compute final size
        let height = content_y + child_size.height + padding.bottom;

        // Width: fill parent (constraints.max_width)
        // Height: actual content height (NOT constrained - parent needs true size for positioning)
        let size = Size::new(constraints.max_width, height);

        // Propagate child's content_size for scroll extent calculation
        let full_content_height = content_y + child_content_size.height + padding.bottom;
        let full_content_size = Size::new(constraints.max_width, full_content_height);

        LayoutResult::new(size, full_content_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Keep the neutral section rule thin; an explicit border retains its thickness.
        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let rule_height = if self.border_color.is_some() {
            (self.border_thickness * one_px).max(one_px)
        } else {
            one_px
        };

        let is_dark = cx.theme.is_dark();
        let border_color = self.border_color.unwrap_or(cx.theme.colors.borders.default);
        let border_y = rect.y + self.legend.frame_center_y - rule_height * 0.5;

        // Snap the frame only; legend placement remains owned by Layout.
        let bx = crate::core::math::snap_to_physical(rect.x, scale);
        let by = crate::core::math::snap_to_physical(border_y, scale);
        let br = crate::core::math::snap_to_physical(rect.x + rect.width, scale);
        let bb = crate::core::math::snap_to_physical(rect.y + rect.height, scale);
        let bw = br - bx;
        let bh = bb - by;

        let panel_fill = self
            .background
            .or_else(|| (!is_dark).then_some(cx.theme.colors.surfaces.level1));

        let frame = RectF::new(bx, by, bw, bh);
        let corners = RoundedCorners::uniform(self.legend.radius);
        if let Some(bg_color) = panel_fill {
            cx.scene
                .push_rounded_rect(frame, bg_color, corners, 1, cx.scene_key);
        }

        // Text keeps its Layout-owned position; only the thin rule snaps to pixels.
        let label_y = rect.y + self.legend.text_top_y;

        // Gap around label on top border (snapped to physical pixel grid)
        let right_limit = (br - rule_height).max(bx);
        let gap_start =
            crate::core::math::snap_to_physical(rect.x + self.legend.margin_left, scale)
                .clamp(bx, right_limit);
        let full_gap_end = crate::core::math::snap_to_physical(
            rect.x + self.legend.margin_left + self.legend.width + 2.0 * self.legend.gap,
            scale,
        );
        let gap_end = full_gap_end.clamp(gap_start, right_limit);
        let title_fits = full_gap_end <= right_limit;
        let label_x = if title_fits {
            (gap_start + gap_end - self.legend.width) * 0.5
        } else {
            gap_start + self.legend.gap
        };

        // Retain the caption opening without painting an eraser over the frame.
        for clip in [
            RectF::new(bx, by, gap_start - bx, bh),
            RectF::new(gap_end, by, br - gap_end, bh),
            RectF::new(
                gap_start,
                by + rule_height,
                gap_end - gap_start,
                (bh - rule_height).max(0.0),
            ),
        ] {
            if clip.width > 0.0 && clip.height > 0.0 {
                cx.scene.begin_clip(clip, None);
                cx.scene.push_border_rounded_rect(
                    frame,
                    border_color,
                    rule_height,
                    corners,
                    4,
                    cx.scene_key,
                );
                cx.scene.end_clip();
            }
        }
        self.paint_legend(
            cx, rect, gap_start, gap_end, label_x, label_y, title_fits, panel_fill,
        );

        // Children are painted by tree traversal
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "GroupBox"
    }
}

#[cfg(test)]
mod text_z_order_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::scene::{SceneNode, TextNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;

    struct PaintBounds {
        rect: RectF,
        z_index: ZIndex,
        is_outline: bool,
    }

    fn collect<'a>(
        nodes: &'a [SceneNode],
        texts: &mut Vec<&'a TextNode>,
        rects: &mut Vec<PaintBounds>,
    ) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => texts.push(text),
                SceneNode::Rect(rect) => rects.push(PaintBounds {
                    rect: rect.rect,
                    z_index: rect.z_index,
                    is_outline: false,
                }),
                SceneNode::RoundedRect(rect) => rects.push(PaintBounds {
                    rect: rect.rect,
                    z_index: rect.z_index,
                    is_outline: false,
                }),
                SceneNode::BorderRect(rect) => rects.push(PaintBounds {
                    rect: rect.rect,
                    z_index: rect.z_index,
                    is_outline: true,
                }),
                SceneNode::RetainedFragment(fragment) => collect(&fragment.children, texts, rects),
                SceneNode::Clip(clip) => collect(&clip.children, texts, rects),
                SceneNode::Layer(layer) => collect(&layer.children, texts, rects),
                SceneNode::Scroll(scroll) => collect(&scroll.children, texts, rects),
                _ => {}
            }
        }
    }

    #[test]
    fn group_box_caption_is_above_its_opaque_chip() {
        let groupbox = GroupBox::empty("Visible group caption")
            .with_border(1.0, Color::WHITE)
            .with_background(Color::BLACK)
            .with_label_size(18.0);
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(groupbox), None, 0);
        let mut harness = RetainedSceneTestHarness::new();
        let mut text_engine = FontManagerTextEngine::new();
        let frame = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(320.0, 96.0)),
            &Theme::dark(),
            &mut text_engine,
            &IconRegistry::new(&[]),
        );

        let mut texts = Vec::new();
        let mut rects = Vec::new();
        collect(&frame.scene_root, &mut texts, &mut rects);
        let caption = texts
            .iter()
            .find(|text| text.text == "Visible group caption")
            .expect("groupbox must paint its caption");
        assert_eq!(
            caption.font_size, 18.0,
            "configured legend size must be effective"
        );
        assert_eq!(caption.z_index, GROUPBOX_LABEL_TEXT_LAYER);
        let (fill, outline) = rects
            .iter()
            .filter(|rect| !rect.is_outline)
            .filter_map(|fill| {
                rects
                    .iter()
                    .find(|outline| outline.is_outline && outline.rect == fill.rect)
                    .map(|outline| (fill, outline))
            })
            .max_by_key(|(fill, _)| fill.z_index)
            .expect("legend must have an overlapping fill and outline");
        assert!(
            fill.z_index < outline.z_index && outline.z_index < caption.z_index,
            "legend fill, outline and caption require strict back-to-front order"
        );
        assert!(
            rects.iter().all(|rect| rect.z_index < caption.z_index),
            "every groupbox fill and border must remain below its caption"
        );
    }

    #[test]
    fn group_box_legend_centers_on_top_rule_and_bounds_long_titles() {
        use crate::compositor::SurfaceId;
        use crate::core::text_engine::TextEngine;
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::widgets::label::Label;
        use std::sync::Arc;

        for theme in [Theme::light(), Theme::dark()] {
            for scale in [1.0, 1.5] {
                for flat in [false, true] {
                    for (title, width, padding) in [
                        ("Calibration", 320.0, None),
                        ("Long acquisition configuration legend", 96.0, None),
                        ("Calibration", 320.0, Some(0.0)),
                        ("Long acquisition configuration legend", 96.0, Some(0.0)),
                    ] {
                        let mut group = GroupBox::empty(title);
                        if let Some(padding) = padding {
                            group = group.with_padding(padding);
                        }
                        if flat {
                            group = group.with_border(3.0, Color::WHITE);
                        }
                        let mut tree = WidgetTree::new();
                        let root = tree.create_root(Box::new(group), None, 0);
                        let child = tree.add_child(root, Box::new(Label::new("Body")), None, 0);
                        let owner = tree.widget_id_for_node(root).unwrap();
                        let child_owner = tree.widget_id_for_node(child).unwrap();
                        let mut harness = RetainedSceneTestHarness::new();
                        let mut text_engine = FontManagerTextEngine::new();
                        let constraints = Constraints::tight(Size::new(width, 96.0));
                        let target = CustomRenderTargetSnapshot::new(
                            SurfaceId::test_id(1),
                            PhysicalExtent {
                                width: (width * scale) as u32,
                                height: (96.0 * scale) as u32,
                            },
                            scale,
                        )
                        .unwrap();
                        let icons = IconRegistry::new(&[]);
                        let frame = harness.frame_with_custom_render_target(
                            &mut tree,
                            root,
                            constraints,
                            &theme,
                            &mut text_engine,
                            &icons,
                            target,
                        );
                        assert!(frame.scene_built);
                        let local = harness.local_fragment(owner).unwrap();
                        let child_local = harness.local_fragment(child_owner).unwrap();
                        let mut texts = Vec::new();
                        let mut rects = Vec::new();
                        collect(&local, &mut texts, &mut rects);
                        let caption = texts.iter().find(|text| text.text == title).unwrap();
                        let chip = rects
                            .iter()
                            .find(|rect| rect.z_index == 5)
                            .map(|rect| rect.rect);
                        let top = rects
                            .iter()
                            .filter(|rect| rect.z_index == 4)
                            .map(|rect| rect.rect.y)
                            .min_by(f32::total_cmp)
                            .unwrap();
                        let rule_height = if flat { 3.0 / scale } else { 1.0 / scale };
                        let metrics = text_engine.font_metrics(caption.font_size);
                        let glyph_center =
                            caption.position.y + (metrics.ascent + metrics.descent) * 0.5;
                        assert!(
                            (glyph_center - top - rule_height * 0.5).abs() <= 0.5 / scale + 0.001
                        );
                        if let Some(chip) = chip {
                            assert!(chip.y <= glyph_center && chip.bottom() >= glyph_center);
                        }
                        let bounds = tree.get_absolute_paint_rect(root).unwrap();
                        assert!(caption.position.y >= bounds.y - 0.001);
                        if let Some(chip) = chip {
                            assert!(chip.x >= bounds.x && chip.right() <= bounds.right());
                            assert!(chip.y >= bounds.y && chip.bottom() <= bounds.bottom());
                        }
                        let clip = local
                            .iter()
                            .find_map(|node| {
                                let SceneNode::Clip(clip) = node else {
                                    return None;
                                };
                                clip.children
                                    .iter()
                                    .any(|node| {
                                        matches!(node, SceneNode::Text(text) if text.text == title)
                                    })
                                    .then_some(clip.rect)
                            })
                            .expect("caption has a local clip");
                        assert!(clip.x >= bounds.x && clip.right() < bounds.right());
                        let text_width = text_engine.measure(title, caption.font_size).0;
                        if title == "Calibration" {
                            assert!(caption.position.x + text_width <= clip.right() + 0.001);
                            let left_gap = caption.position.x - clip.x;
                            let right_gap = clip.right() - caption.position.x - text_width;
                            assert!((left_gap - right_gap).abs() < 0.001);
                            assert!(left_gap > 0.0);
                        } else {
                            assert!(text_width > clip.width);
                            if let Some(chip) = chip {
                                assert!(clip.right() < chip.right());
                                assert!(caption.position.x >= chip.x);
                            }
                        }
                        for border in rects.iter().filter(|rect| rect.z_index == 4) {
                            assert!(border.rect.width >= 0.0 && border.rect.height >= 0.0);
                            assert!(
                                border.rect.x >= bounds.x
                                    && border.rect.right() <= bounds.right() + 0.001
                            );
                            assert!(
                                (border.rect.y * scale - (border.rect.y * scale).round()).abs()
                                    < 0.001
                            );
                        }
                        let body = tree.get_absolute_paint_rect(child).unwrap();
                        assert!(body.y >= caption.position.y + metrics.ascent + metrics.descent);
                        let node = tree.node(root).unwrap();
                        assert!(!node.widget.hit_testable() && !node.widget.focusable());
                        let laid_out = node.widget.as_any().downcast_ref::<GroupBox>().unwrap();
                        assert!(
                            (caption.position.y - bounds.y - laid_out.legend.text_top_y).abs()
                                < 0.001
                        );
                        assert!(
                            (glyph_center - bounds.y - laid_out.legend.frame_center_y).abs()
                                < 0.001
                        );
                        let unchanged = harness.frame_with_custom_render_target(
                            &mut tree,
                            root,
                            constraints,
                            &theme,
                            &mut text_engine,
                            &icons,
                            target,
                        );
                        assert!(!unchanged.scene_built);
                        assert!(Arc::ptr_eq(&local, &harness.local_fragment(owner).unwrap()));
                        assert!(Arc::ptr_eq(
                            &child_local,
                            &harness.local_fragment(child_owner).unwrap()
                        ));
                        assert_eq!(tree.widget_id_for_node(root), Some(owner));
                    }
                }
            }
        }
    }

    #[test]
    fn group_box_title_change_refreshes_legend_and_reuses_unchanged_content() {
        use crate::widgets::label::Label;
        use std::sync::Arc;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(GroupBox::empty("Before")), None, 0);
        let child = tree.add_child(root, Box::new(Label::new("Unchanged content")), None, 0);
        let owner = tree.widget_id_for_node(root).unwrap();
        let child_owner = tree.widget_id_for_node(child).unwrap();
        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let theme = Theme::light();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(320.0, 96.0));
        harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let first = harness.local_fragment(owner).unwrap();
        let child_first = harness.local_fragment(child_owner).unwrap();
        let child_rect = tree.get_absolute_paint_rect(child).unwrap();
        tree.node_mut(root)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<GroupBox>()
            .unwrap()
            .label = "After a longer title".into();
        tree.mark_layout_dirty(root);
        let changed = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(changed.scene_built);
        let successor = harness.local_fragment(owner).unwrap();
        assert!(!Arc::ptr_eq(&first, &successor));
        let mut texts = Vec::new();
        let mut rects = Vec::new();
        collect(&successor, &mut texts, &mut rects);
        assert!(texts.iter().any(|text| text.text == "After a longer title"));
        assert!(!texts.iter().any(|text| text.text == "Before"));
        assert!(Arc::ptr_eq(
            &child_first,
            &harness.local_fragment(child_owner).unwrap()
        ));
        assert_eq!(tree.get_absolute_paint_rect(child), Some(child_rect));
        let unchanged = harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!unchanged.scene_built);
        assert!(Arc::ptr_eq(
            &successor,
            &harness.local_fragment(owner).unwrap()
        ));
        assert_eq!(tree.widget_id_for_node(root), Some(owner));
        assert_eq!(tree.widget_id_for_node(child), Some(child_owner));
    }

    #[test]
    fn group_box_flow_grid_keeps_fixed_item_height_when_wrapping() {
        use crate::core::event::{InteractionDomains, WindowId};
        use crate::widgets::{button::Button, container::Container, flow_grid::FlowGrid, label::Label};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(GroupBox::empty("Grid A")), None, 0);
        let grid = tree.add_child(
            root,
            Box::new(FlowGrid::new(80.0).with_spacing(4.0)),
            None,
            0,
        );
        let mut items = Vec::new();
        for index in 1..=6 {
            let item = tree.add_child(grid, Box::new(Container::empty()), None, 0);
            let button = tree.add_child(item, Box::new(Label::new("placeholder")), None, 0);
            let label = tree.add_child(
                button,
                Box::new(Label::new(format!("A{index}")).with_size(11.0)),
                None,
                0,
            );
            tree.replace_widget_only(button, Box::new(Button::new(label).with_padding(6.0)));
            tree.replace_widget_only(item, Box::new(Container::new(button, 0.0).with_height(32.0)));
            items.push(item);
        }

        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let theme = Theme::dark();
        let icons = IconRegistry::new(&[]);
        let domains = InteractionDomains::new();
        for (width, wraps) in [(600.0, false), (220.0, true)] {
            harness.frame_with_appearance(
                &mut tree,
                root,
                Constraints::loose(Size::new(width, 320.0)),
                &theme,
                &mut text,
                &icons,
                &domains,
                WindowId::PRIMARY,
            );
            for &item in &items {
                assert!((tree.get_rect(item).unwrap().height - 32.0).abs() <= 0.1);
            }
            let first = tree.get_rect(items[0]).unwrap();
            let last = tree.get_rect(items[items.len() - 1]).unwrap();
            assert_eq!(last.y > first.y, wraps, "grid width {width}");
        }
    }
}
