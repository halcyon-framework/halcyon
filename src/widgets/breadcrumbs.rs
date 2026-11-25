// SPEC-HAL-220: Breadcrumbs Navigation Control
// Pure navigation selector. Emits explicit navigation intents.
// No routing, no history, no content ownership, no state.
//
// INV-A1: No routing authority.
// INV-A2: Path is external.
// INV-B1: Linear ordered path.
// INV-B2: Exactly one current crumb.
// INV-B3: Past crumbs are navigable. Crumbs after current must not exist.
// INV-C1: Explicit activation only (pointer + keyboard).
// INV-C2: Current crumb is non-navigational by default.
// INV-C3: Optional dropdown on current crumb (explicit).
// INV-D1: Dropdown follows SPEC-HAL-218 + SPEC-HAL-216.
// INV-D2: No hidden navigation logic.
// INV-D3: Dropdown does not change breadcrumb state.
// INV-E1: Crumbs are visual labels.
// INV-E2: Current crumb is visually distinct.
// INV-E3: Geometry is self-contained.
// INV-F1: Disabled breadcrumbs are absolute.
// INV-F2: Disabled crumb is inert.
// INV-G1: Intent emission only.
// INV-G2: No form binding.
// INV-H1: Deterministic interaction.
// INV-H2: No per-frame allocation.
// INV-H3: Invariant supremacy.

use std::cell::Cell;
use std::rc::Rc;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{LAYER_CONTENT, LAYER_FOCUS};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::breadcrumbs::BreadcrumbsSink;
use crate::model::dropdown_menu::DropdownMenuHandle;
use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::menu_surface::MenuSurfaceOverlay;

/// Opaque crumb identity for external coordination.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CrumbId(pub u32);

/// Separator character rendered between crumbs.
const SEPARATOR: &str = "\u{203A}"; // › (single right-pointing angle quotation mark)

pub struct Breadcrumbs {
    id: WidgetInternalId,
    /// External navigation intent sink. Written on activation, read by caller.
    /// Widget retains zero knowledge of emitted values.
    sink: BreadcrumbsSink,
    /// Crumb definitions: (id, label, enabled). Owned snapshot, replace-only.
    crumbs: Vec<(CrumbId, String, bool)>,
    /// Externally provided current crumb. Breadcrumbs do NOT own this.
    current_crumb: Option<CrumbId>,
    /// Keyboard focus index within the crumb list (roving focus).
    focus_index: usize,
    /// Widget focus state.
    focused: bool,
    /// Global disabled state. When true, all input is rejected.
    disabled: bool,
    /// Cached crumb rects from layout (relative to widget origin).
    /// Recomputed only on layout, not per-frame.
    crumb_rects: Vec<RectF>,
    // --- Optional dropdown on current crumb (INV-C3) ---
    /// Dropdown controller (if configured). Persists in caller's state.
    dropdown_controller: Option<DropdownMenuHandle>,
    /// Dropdown selection sink (if configured). Widget bridges selections to BreadcrumbsSink.
    dropdown_sink: Option<Rc<Cell<Option<MenuItemId>>>>,
    /// Dropdown items (explicitly provided, never inferred — INV-D2).
    dropdown_items: Vec<MenuItem>,
    /// Overlay renderer for dropdown popup (INV-D1: follows SPEC-HAL-218).
    overlay: MenuSurfaceOverlay,
    /// Cached rect of the current crumb (absolute coords) for anchoring dropdown.
    cached_current_rect: Cell<RectF>,
}

impl Breadcrumbs {
    pub fn new(sink: BreadcrumbsSink) -> Self {
        Self {
            id: WidgetInternalId::new(),
            sink,
            crumbs: Vec::new(),
            current_crumb: None,
            focus_index: 0,
            focused: false,
            disabled: false,
            crumb_rects: Vec::new(),
            dropdown_controller: None,
            dropdown_sink: None,
            dropdown_items: Vec::new(),
            overlay: MenuSurfaceOverlay::new(),
            cached_current_rect: Cell::new(RectF::new(0.0, 0.0, 0.0, 0.0)),
        }
    }

    /// Replace crumb definitions. Owned snapshot, replace-only.
    pub fn set_crumbs(&mut self, crumbs: &[(CrumbId, &str, bool)]) {
        self.crumbs.clear();
        for &(id, label, enabled) in crumbs {
            self.crumbs.push((id, label.to_string(), enabled));
        }
        if !self.crumbs.is_empty() && self.focus_index >= self.crumbs.len() {
            self.focus_index = self.crumbs.len() - 1;
        }
    }

    /// Set the externally-owned current crumb.
    pub fn set_current_crumb(&mut self, id: Option<CrumbId>) {
        self.current_crumb = id;
    }

    /// Set global disabled state.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    /// Enable optional dropdown on current crumb (INV-C3).
    /// Caller creates handle + sink via dropdown_menu_handle().
    /// Breadcrumbs bridges dropdown selections to its own BreadcrumbsSink.
    pub fn enable_dropdown(
        &mut self,
        controller: DropdownMenuHandle,
        sink: Rc<Cell<Option<MenuItemId>>>,
        items: Vec<MenuItem>,
    ) {
        self.dropdown_controller = Some(controller);
        self.dropdown_sink = Some(sink);
        self.dropdown_items = items;
    }

    /// Is the given crumb the current crumb?
    fn is_current(&self, id: CrumbId) -> bool {
        self.current_crumb == Some(id)
    }

    /// Does the current crumb have a dropdown configured?
    fn has_dropdown(&self) -> bool {
        self.dropdown_controller.is_some() && !self.dropdown_items.is_empty()
    }

    /// Attempt to activate the crumb at the given index.
    /// Past crumbs: emit navigation intent.
    /// Current crumb + dropdown: open dropdown.
    /// Current crumb without dropdown: no-op (INV-C2).
    /// No-op if: disabled, crumb disabled (INV-F1, INV-F2).
    /// Returns whether dropdown presentation was opened or reset.
    fn activate_crumb(&mut self, index: usize) -> bool {
        if self.disabled {
            return false;
        }
        if let Some((id, _, enabled)) = self.crumbs.get(index) {
            if !enabled {
                return false; // INV-F2: disabled crumb is inert
            }
            if self.is_current(*id) {
                // INV-C2: current crumb is non-navigational by default
                // INV-C3: unless dropdown is configured
                if self.has_dropdown() {
                    self.open_dropdown();
                    return true;
                }
                return false;
            }
            // Past crumb: emit navigation intent (INV-G1)
            self.sink.set(Some(*id));
        }
        false
    }

    /// Open dropdown on current crumb.
    fn open_dropdown(&mut self) {
        if let Some(ref controller) = self.dropdown_controller {
            let trigger_rect = self.cached_current_rect.get();
            let items = self.dropdown_items.clone();
            controller.borrow_mut().show(trigger_rect, items);
        }
    }

    /// Bridge dropdown selection to BreadcrumbsSink.
    /// Maps MenuItemId -> CrumbId. INV-D3: dropdown does not change breadcrumb state.
    fn bridge_dropdown_selection(&self) {
        if let Some(ref sink) = self.dropdown_sink {
            if let Some(menu_item_id) = sink.take() {
                self.sink.set(Some(CrumbId(menu_item_id)));
            }
        }
    }

    /// Move focus_index by delta, skipping disabled crumbs.
    /// Clamps at boundaries (does not wrap).
    fn move_focus(&mut self, delta: isize) {
        if self.crumbs.is_empty() {
            return;
        }
        let len = self.crumbs.len();
        let mut idx = self.focus_index as isize;
        for _ in 0..len {
            idx += delta;
            if idx < 0 {
                idx = 0;
                break;
            }
            if idx >= len as isize {
                idx = (len - 1) as isize;
                break;
            }
            if self.crumbs[idx as usize].2 {
                break;
            }
        }
        self.focus_index = idx.clamp(0, (len - 1) as isize) as usize;
    }

    /// Is the dropdown currently open?
    fn is_dropdown_open(&self) -> bool {
        self.dropdown_controller
            .as_ref()
            .is_some_and(|c| c.borrow().is_open())
    }
}

impl WidgetDowncast for Breadcrumbs {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Breadcrumbs {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let font_size = cx.theme.typography.font_size;
        let padding_x = cx.theme.geometry.button.padding_x;
        let height = cx.theme.geometry.button.height;

        self.crumb_rects.clear();
        let mut cursor_x = 0.0f32;

        let separator_run = cx.text_engine.shape(SEPARATOR, font_size);
        let separator_gap = separator_run.width + padding_x;

        for (i, (_, label, _)) in self.crumbs.iter().enumerate() {
            // Separator gap before each crumb except the first
            if i > 0 {
                cursor_x += separator_gap;
            }

            let run = cx.text_engine.shape(label, font_size);
            let crumb_width = run.width + padding_x * 2.0;
            self.crumb_rects
                .push(RectF::new(cursor_x, 0.0, crumb_width, height));
            cursor_x += crumb_width;
        }

        // INV-E3: Geometry is self-contained.
        let width = if constraints.max_width.is_finite() {
            cursor_x.min(constraints.max_width)
        } else {
            cursor_x
        };

        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if self.crumbs.is_empty() {
            return;
        }

        let font_size = cx.theme.typography.font_size;
        let controls = &cx.theme.colors.controls;

        // INV-E3: Clip to widget bounds (overflow = clip).
        cx.scene.begin_clip(rect, None);

        let separator_run = cx.text_engine.shape(SEPARATOR, font_size);

        for (i, ((id, label, enabled), crumb_rect)) in
            self.crumbs.iter().zip(self.crumb_rects.iter()).enumerate()
        {
            let abs_rect = RectF::new(
                rect.x + crumb_rect.x,
                rect.y + crumb_rect.y,
                crumb_rect.width,
                crumb_rect.height,
            );

            // Separator between crumbs (painted in the gap before this crumb)
            if i > 0 {
                let prev = &self.crumb_rects[i - 1];
                let gap_start = rect.x + prev.x + prev.width;
                let gap_end = abs_rect.x;
                let sep_x = gap_start + (gap_end - gap_start - separator_run.width) / 2.0;
                let sep_y = abs_rect.y + (abs_rect.height - separator_run.height) / 2.0;
                cx.scene.push_text_with_backgrounds(
                    &separator_run,
                    sep_x,
                    sep_y,
                    cx.theme.colors.typography.muted,
                    vec![],
                    LAYER_CONTENT + 1,
                    None,
                );
            }

            let is_current = self.is_current(*id);

            // Background for current crumb (INV-E2: visually distinct)
            if is_current {
                cx.scene
                    .push_rect(abs_rect, controls.background_pressed, LAYER_CONTENT, None);
            }

            // Cache current crumb rect for dropdown anchoring
            if is_current {
                self.cached_current_rect.set(abs_rect);
            }

            // Text label
            let text_color = if !*enabled {
                cx.theme.colors.typography.muted // INV-F2: disabled = muted
            } else {
                controls.text
            };
            let run = cx.text_engine.shape(label, font_size);
            let text_x = abs_rect.x + (abs_rect.width - run.width) * 0.5;
            let text_y = abs_rect.y + (abs_rect.height - run.height) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &run,
                text_x,
                text_y,
                text_color,
                vec![],
                LAYER_CONTENT + 1,
                None,
            );

            // Roving keyboard focus: the current crumb already has selected
            // background, so only non-current focused crumbs get an outline.
            if self.focused && i == self.focus_index && !is_current {
                cx.scene.push_border_rect(
                    abs_rect,
                    cx.theme.colors.borders.strong.with_alpha(0.35),
                    cx.one_px(),
                    LAYER_FOCUS,
                    None,
                );
            }
        }

        cx.scene.end_clip();
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        if let Some(ref controller) = self.dropdown_controller {
            let ctrl = controller.borrow();
            if !ctrl.is_open() {
                return;
            }

            // Anchor dropdown below current crumb
            let current_rect = self.cached_current_rect.get();
            let min_width = current_rect.width.max(120.0);
            self.overlay.paint(
                cx,
                ctrl.surface(),
                (current_rect.x, current_rect.y + current_rect.height),
                min_width,
                tree_widget_id,
                Some(rect),
                rect,
            );
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        // INV-F1: Disabled breadcrumbs are absolute.
        if self.disabled {
            return false;
        }

        let dropdown_open = self.is_dropdown_open();

        // If dropdown is open, delegate to overlay first (INV-D1: follows SPEC-HAL-218).
        if dropdown_open {
            if let Some(ref controller) = self.dropdown_controller {
                let mut ctrl = controller.borrow_mut();
                if let Some(consumed) = self.overlay.handle_event(ctrl.surface_mut(), event, true) {
                    drop(ctrl);
                    // Bridge dropdown selection to BreadcrumbsSink (INV-D3: no breadcrumb state mutation)
                    self.bridge_dropdown_selection();
                    return consumed;
                }
            }
        }

        match event {
            Event::Pointer(pe) => {
                if pe.kind == PointerEventKind::Down && pe.button == Some(PointerButton::Primary) {
                    let (lx, ly) = pe.local_pos;
                    for (i, crumb_rect) in self.crumb_rects.iter().enumerate() {
                        if lx >= crumb_rect.x
                            && lx < crumb_rect.x + crumb_rect.width
                            && ly >= crumb_rect.y
                            && ly < crumb_rect.y + crumb_rect.height
                        {
                            self.focus_index = i;
                            self.activate_crumb(i);
                            cx.request_continuous_frames();
                            return true;
                        }
                    }
                }
                false
            }
            Event::Key(ke) if ke.is_down => match ke.key_code {
                KeyCode::Left => {
                    self.move_focus(-1);
                    cx.request_continuous_frames();
                    true
                }
                KeyCode::Right => {
                    self.move_focus(1);
                    cx.request_continuous_frames();
                    true
                }
                KeyCode::Enter | KeyCode::Space => {
                    self.activate_crumb(self.focus_index);
                    cx.request_continuous_frames();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
        // Initialize focus to first enabled crumb
        if let Some(idx) = self.crumbs.iter().position(|(_, _, enabled)| *enabled) {
            self.focus_index = idx;
        }
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        // Close dropdown if open (INV-D3: no state mutation, just dismiss)
        if self.is_dropdown_open() {
            if let Some(ref controller) = self.dropdown_controller {
                controller.borrow_mut().dismiss();
            }
        }
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.disabled
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if self.disabled {
            return;
        }
        if self.activate_crumb(self.focus_index) {
            cx.invalidate_paint();
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }
}
