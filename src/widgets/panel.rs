// SPEC-HAL-011: Panel (Framed / Bordered Container)

use crate::core::event::{Event, EventPhase};
use crate::core::layout::{
    Borders, Constraints, CornerRadius, LayoutResult, Margins, Padding, compute_box_model,
    shrink_constraints,
};
use crate::core::math::RectF;
use crate::core::paint::{Color, LAYER_CONTENT, ShadowMode, ZIndex};
use crate::core::style::{BorderStyle, DecorationStack, DecorationState};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

const PANEL_TITLE_TEXT_LAYER: ZIndex = LAYER_CONTENT + 3;

/// Framed container with border and background
pub struct Panel {
    id: WidgetInternalId,
    pub child: NodeId,
    pub margin: Margins,
    pub padding: Padding,
    pub border: Borders,
    pub radius: CornerRadius,
    /// Background color. None = `surfaces.card` for Card mode (title is
    /// Some), `surfaces.base0` for full-bleed root Panels, and
    /// `surfaces.content` for ordinary plain Panels.
    pub background: Option<Color>,
    /// Optional title rendered as painted chrome at the top of the content
    /// area, with a 1px separator below. Purely visual — not a child widget.
    pub title: Option<String>,
}

impl Panel {
    pub fn new(child: NodeId) -> Self {
        Self {
            id: WidgetInternalId::new(),
            child,
            margin: Margins::zero(),
            // Instance default; read directly in layout() — NOT theme-synced (INV-GEOM-SYNC-PANEL)
            padding: Padding::all(8.0),
            // Instance default; read directly in layout() — NOT theme-synced (INV-GEOM-SYNC-PANEL)
            border: Borders::new(1.0, Color::TRANSPARENT),
            // INV-CONSTRUCTOR-FALLBACK: Inert default; synced from theme in layout()
            radius: CornerRadius::all(6.0),
            background: None, // Default: resolved dynamically in paint()
            title: None,
        }
    }

    /// Card-style preset: 4px rounded corners, 12px padding, transparent
    /// border (Card paints a visible theme border in `paint()` because
    /// `title` is Some), and an elevated background chosen dynamically.
    pub fn card(child: NodeId, title: &str) -> Self {
        Self {
            id: WidgetInternalId::new(),
            child,
            margin: Margins::zero(),
            padding: Padding::all(12.0),
            border: Borders::new(1.0, Color::TRANSPARENT),
            radius: CornerRadius::all(4.0),
            background: None,
            title: Some(title.to_string()),
        }
    }

    pub fn with_margin(mut self, margin: f32) -> Self {
        self.margin = Margins::all(margin);
        self
    }

    pub fn with_padding(mut self, padding: f32) -> Self {
        self.padding = Padding::all(padding);
        self
    }

    pub fn with_border(mut self, color: Color, width: f32) -> Self {
        self.border = Borders::new(width, color);
        self
    }

    pub fn with_radius(mut self, radius: f32) -> Self {
        self.radius = CornerRadius::all(radius);
        self
    }

    pub fn with_background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.title = Some(title.into());
        self
    }
}

impl WidgetDowncast for Panel {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Panel {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // INV-GEOM-SYNC-PANEL: Panel padding and border are instance-owned —
        // set via with_padding / with_border (constructor defaults below) and
        // read directly here, NOT theme-synced. A consumer must be able to zero
        // the container inset (e.g. full-bleed root). See SPEC-HAL-096B §4.4.
        let padding = self.padding;
        let border = self.border;

        // Title occupies vertical space at the top of the content area when present.
        let title_height = if self.title.is_some() {
            cx.theme.typography.font_size + 8.0
        } else {
            0.0
        };

        // SPEC-HAL-010: Use canonical box model
        let mut child_constraints = shrink_constraints(constraints, self.margin, border, padding);
        // Reduce vertical room available to the child by the title strip.
        child_constraints.max_height = (child_constraints.max_height - title_height).max(0.0);
        child_constraints.min_height = (child_constraints.min_height - title_height).max(0.0);

        // SPEC-HAL-026 §8: descend via cx.layout_child. Engine handles
        // dirty-bit clear, pre_layout, deferred_wiring, current_node,
        // cached_size, last_constraints, layout_epoch. Missing-child
        // structural corruption surfaces as the engine's INV-ZERO-2 panic.
        let child_size = cx.layout_child(self.child, child_constraints);

        // SPEC-HAL-010: Compute box model and position child
        let box_model = compute_box_model(child_size, self.margin, border, padding);
        let new_rect = RectF::new(
            box_model.content_offset.0,
            box_model.content_offset.1 + title_height,
            child_size.width,
            child_size.height,
        );
        cx.set_child_rect(self.child, new_rect);

        // SPEC-HAL-010: Final size from box model
        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        let mut size = compute_box_model(child_size, self.margin, border, padding).size;
        // Title strip lives inside the content rect; add it back to the total.
        size.height += title_height;
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Geometry precedence: theme token (if available) → constructor default
        // SPEC-HAL-090 / SPEC-HAL-096C
        let border_width = cx.theme.borders.b1;
        let radius = cx.theme.radii.l;

        // SPEC-HAL-010: Paint uses rect from traversal
        // Apply margin inset to get visual panel rect
        let panel_rect = RectF::new(
            rect.x + self.margin.left,
            rect.y + self.margin.top,
            rect.width - self.margin.horizontal(),
            rect.height - self.margin.vertical(),
        );

        // Plain Panels have two roles:
        // - full-bleed root/chassis panels (zero padding + zero border) use
        //   `surfaces.base0`, so the application window background is visible
        //   and theme-reactive;
        // - ordinary content panels use `surfaces.content`, preserving the
        //   data/editor surface continuity expected by list/property widgets.
        // Card mode uses `surfaces.card` — a dedicated elevation token,
        // calibrated per theme.
        let is_full_bleed_plain_panel = self.title.is_none()
            && self.padding.horizontal() == 0.0
            && self.padding.vertical() == 0.0
            && self.border.width.horizontal() == 0.0
            && self.border.width.vertical() == 0.0;
        let bg_color = self.background.unwrap_or_else(|| {
            if self.title.is_some() {
                cx.theme.colors.surfaces.card
            } else if is_full_bleed_plain_panel {
                cx.theme.colors.surfaces.base0
            } else {
                cx.theme.colors.surfaces.content
            }
        });

        // Card mode: wide soft drop shadow for elevation. blur=10, alpha=10%,
        // no spread — atmospheric, not sharp. Shadow is the sole edge
        // definition (no visible border). z=0 keeps shadow below content.
        if self.title.is_some() {
            cx.scene.push_shadow(
                panel_rect,
                self.radius.top_left,
                ShadowMode::Drop,
                (0.0, 2.0),
                0.0,
                10.0,
                Color::from_linear(0.0, 0.0, 0.0, 0.10),
                0,
                None,
            );
        }

        // Border color: Card mode uses transparent (shadow-only elevation).
        // Plain Panels honor the constructor-set border color.
        let border_color = if self.title.is_some() {
            Color::TRANSPARENT
        } else {
            self.border.color
        };

        // Build decoration stack with background and optional border
        let stack = if border_width > 0.0 {
            DecorationStack::background(bg_color, radius).with_border(BorderStyle::new(
                border_width,
                border_color,
                radius,
            ))
        } else {
            DecorationStack::background(bg_color, radius)
        };

        // No focus, no hover for Panel (pure chrome)
        let state = DecorationState::default();

        // base_z=0 is the background/fill layer. Child content that must appear
        // above it paints at LAYER_CONTENT or higher. SPEC-PM-007 forbids using
        // traversal order as cross-batch layering authority at equal z.
        stack.paint(cx.scene, panel_rect, state, 0);

        // Title chrome: text top-left inside the panel content area, with a
        // 1px subtle separator below it spanning the full content width.
        // Layout reserved `title_height = font_size + 8.0` at the top.
        if let Some(ref title) = self.title {
            let font_size = cx.theme.typography.font_size;
            let title_height = font_size + 8.0;
            let title_x = panel_rect.x + border_width + self.padding.left;
            let title_y = panel_rect.y + border_width + 4.0;
            let title_run = cx.text_engine.shape(title, font_size);
            cx.text_engine.draw(
                &title_run,
                title_x,
                title_y,
                cx.theme.colors.typography.primary,
                PANEL_TITLE_TEXT_LAYER,
                cx.scene,
            );

            let sep_x = panel_rect.x + border_width;
            let sep_y = panel_rect.y + border_width + title_height;
            let sep_w = (panel_rect.width - 2.0 * border_width).max(0.0);
            cx.scene.push_rect(
                RectF::new(sep_x, sep_y, sep_w, 1.0),
                cx.theme.colors.borders.subtle,
                4,
                None,
            );
        }
    }

    fn handle_event(&mut self, _cx: &mut EventContext, _event: &Event, _phase: EventPhase) -> bool {
        // SPEC-HAL-011: Transparent, does not intercept events
        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "Panel"
    }
}
