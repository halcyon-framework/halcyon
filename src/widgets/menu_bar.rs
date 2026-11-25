// SPEC-HAL-333 Rev 4: MenuBar — in-window menu bar host widget (Class D).
//
// INV-D1: the bar drives one MenuSurface (SPEC-HAL-223) per open title. It
//         implements no menu list layout, hover, or dismissal internally —
//         MenuSurfaceOverlay owns all popup geometry and in-menu interaction.
// INV-D2: the bar owns exactly: title open/close, hover-switch between titles
//         while a menu is open, horizontal Left/Right traversal, and title
//         chrome state (rest / hover / active-open).
// INV-D3: titles are flat — no button fill at rest, no caret, no disclosure.
// INV-D4: square corners; push_rect only.
// INV-D5: all colors theme-resolved at paint.
// INV-E2: activations are buffered (shared mailbox); the host drains.
//
// The concrete InWindowMenuHost bridge (INV-G2) lives at the bottom of this file.
// Data flows host <-> widget through MenuBarMailbox — the App-side
// shared-state pattern of ToastManager/ToastWidget. The WidgetTree remains
// the sole owner of the widget; no tree access crosses the host bridge.

use std::cell::{Cell, RefCell};
use std::rc::Rc;

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::menu_item::{MenuItem, MenuItemId};
use crate::model::menu_model::{MenuModel, find_row, find_row_mut};
use crate::model::menu_surface::MenuSurface;
use crate::platform::menu_host::MenuHost as PlatformMenuHost;
use crate::runtime::actions::ActionRegistry;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::menu_surface::MenuSurfaceOverlay;

// INV-justified literal (widgets/mod.rs geometry rule): minimum popup width,
// mirroring DropdownMenu's POPUP_MIN_WIDTH — narrow titles ("File") must not
// produce unusably narrow popups.
const POPUP_MIN_WIDTH: f32 = 120.0;
// INV-justified literal: 1px structural hairline separating the bar from
// content, per the SPEC-HAL-332 professional charter. Square push_rect.
const HAIRLINE: f32 = 1.0;

/// How a closed menu opens (SPEC-HAL-333 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OpenTrigger {
    ClickToOpen,
    HoverToOpen,
}

/// Conditions that dismiss the open menu (SPEC-HAL-333 §4).
/// Distinct from core/overlay_root.rs DismissPolicy (SPEC-HAL-127);
/// module-scoped, not re-exported.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DismissPolicy {
    pub on_click_outside: bool,
    pub on_escape: bool,
    pub on_activation: bool,
}

impl Default for DismissPolicy {
    fn default() -> Self {
        Self {
            on_click_outside: true,
            on_escape: true,
            on_activation: true,
        }
    }
}

/// In-window bar interaction policy (SPEC-HAL-333 §4). Native hosts are
/// exempt — OS-owned interaction governs native menus.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MenuBarPolicy {
    pub open_trigger: OpenTrigger,
    pub hover_switch_while_open: bool,
    pub horizontal_traversal: bool,
    pub close_on_reactivate_title: bool,
    pub dismiss: DismissPolicy,
}

impl Default for MenuBarPolicy {
    fn default() -> Self {
        Self {
            open_trigger: OpenTrigger::ClickToOpen,
            hover_switch_while_open: true,
            horizontal_traversal: true,
            close_on_reactivate_title: true,
            dismiss: DismissPolicy::default(),
        }
    }
}

/// One top-level title and its item tree. Projection copy — the MenuModel
/// remains the single source of truth (INV-A2); the host refreshes this.
pub struct BarTitle {
    pub label: String,
    pub items: Vec<MenuItem>,
}

/// One staged item mirror: enabled + label (INV-A5 sync payload). Checked is
/// not staged yet — popup check visuals are the named SPEC-HAL-223 gap; the
/// field arrives with that amendment.
pub struct StagedItemSync {
    pub id: MenuItemId,
    pub enabled: bool,
    pub label: String,
}

/// Shared mailbox between InWindowMenuHost (stages projection data, drains
/// activations) and MenuBar (applies staged data, emits activations). The
/// App-side shared-state pattern of ToastManager/ToastWidget (SPEC-HAL-269):
/// the WidgetTree stays sole owner of the widget; no tree access crosses the
/// MenuHost trait; AppRoot retains the bar's NodeId for mount lifecycle and
/// layout-dirty marking only.
#[derive(Default)]
pub struct MenuBarMailbox {
    /// Full projection staged by install(); applied wholesale.
    staged_titles: Option<Vec<BarTitle>>,
    /// Per-item mirrors staged by sync_item(); applied in order.
    staged_syncs: Vec<StagedItemSync>,
    /// Activations emitted by the bar (INV-E2); drained by the host.
    activations: Vec<MenuItemId>,
}

impl MenuBarMailbox {
    /// Test-only activation injection so the AppRoot dispatch path is tested
    /// end-to-end (SPEC-HAL-129: no stub tests).
    #[cfg(any(test, feature = "test-harness"))]
    pub fn push_activation_for_test(&mut self, id: MenuItemId) {
        self.activations.push(id);
    }
}

pub struct MenuBar {
    id: WidgetInternalId,
    policy: MenuBarPolicy,
    titles: Vec<BarTitle>,
    /// Index of the open title. None = no menu open.
    open_title: Option<usize>,
    /// Title under the pointer, for hover chrome. Visual only.
    hovered_title: Option<usize>,
    focused: bool,
    /// One surface, re-shown per open title (INV-D1).
    surface: MenuSurface,
    /// Selection sink shared with the surface; drained into the mailbox.
    sink: Rc<Cell<Option<MenuItemId>>>,
    overlay: MenuSurfaceOverlay,
    /// Shared with InWindowMenuHost.
    mailbox: Rc<RefCell<MenuBarMailbox>>,
    /// Title cell rects from layout, widget-local.
    title_rects: Vec<RectF>,
    /// Bar strip height from layout (theme button height).
    bar_height: f32,
    /// Absolute widget rect cached at paint, for popup anchoring.
    cached_rect: Cell<RectF>,
}

impl MenuBar {
    pub fn new(mailbox: Rc<RefCell<MenuBarMailbox>>) -> Self {
        let sink: Rc<Cell<Option<MenuItemId>>> = Rc::new(Cell::new(None));
        Self {
            id: WidgetInternalId::new(),
            policy: MenuBarPolicy::default(),
            titles: Vec::new(),
            open_title: None,
            hovered_title: None,
            focused: false,
            surface: MenuSurface::new(Rc::clone(&sink)),
            sink,
            overlay: MenuSurfaceOverlay::new(),
            mailbox,
            title_rects: Vec::new(),
            bar_height: 0.0,
            cached_rect: Cell::new(RectF::new(0.0, 0.0, 0.0, 0.0)),
        }
    }

    pub fn set_policy(&mut self, policy: MenuBarPolicy) {
        self.policy = policy;
    }

    /// Replace the projected titles wholesale (INV-A2: structural change is
    /// replacement). Any open menu closes; hover clears.
    pub fn set_titles(&mut self, titles: Vec<BarTitle>) {
        if self.surface.is_open() {
            self.surface.dismiss();
        }
        self.open_title = None;
        self.hovered_title = None;
        self.titles = titles;
        self.title_rects.clear();
    }

    /// Apply any staged mailbox data (install projection, item syncs).
    /// Returns true when anything was applied. Called by the Class G wiring
    /// immediately after host install/sync staging; the CALLER marks the bar
    /// node layout-dirty when this returns true — dirty authority is external
    /// (SPEC-HAL-158), never the widget's.
    pub fn apply_mailbox(&mut self) -> bool {
        let (staged_titles, syncs) = {
            let mut mb = self.mailbox.borrow_mut();
            (
                mb.staged_titles.take(),
                std::mem::take(&mut mb.staged_syncs),
            )
        };
        let mut applied = staged_titles.is_some();
        if let Some(titles) = staged_titles {
            self.set_titles(titles);
        }
        for sync in &syncs {
            if self.apply_item_sync(sync) {
                applied = true;
            }
        }
        applied
    }

    /// Mirror one staged item sync into the projection copy. If the item
    /// lives in the open title, re-show the surface so the open popup
    /// reflects it (highlight resets per SPEC-HAL-223 INV-C3).
    fn apply_item_sync(&mut self, sync: &StagedItemSync) -> bool {
        let mut found = false;
        for title in &mut self.titles {
            if let Some(MenuItem::ActionRow {
                label, disabled, ..
            }) = find_row_mut(&mut title.items, sync.id)
            {
                *label = sync.label.clone();
                *disabled = !sync.enabled;
                found = true;
                break;
            }
        }
        if !found {
            return false;
        }
        if let Some(idx) = self.open_title {
            if find_row(&self.titles[idx].items, sync.id).is_some() {
                let items = self.titles[idx].items.clone();
                self.surface.show(items);
            }
        }
        true
    }

    pub fn is_open(&self) -> bool {
        self.open_title.is_some()
    }

    pub fn open_title_index(&self) -> Option<usize> {
        self.open_title
    }

    fn open_title_at(&mut self, index: usize) {
        if index >= self.titles.len() {
            return;
        }
        let items = self.titles[index].items.clone();
        self.open_title = Some(index);
        self.surface.show(items);
    }

    fn close_menu(&mut self) {
        if self.surface.is_open() {
            self.surface.dismiss();
        }
        self.open_title = None;
    }

    /// Reconcile bar state after the overlay handled an event: collect any
    /// emitted activation into the mailbox (INV-E2) and apply the dismiss
    /// policy; detect overlay-driven dismissal (Escape / click outside).
    fn reconcile_after_overlay(&mut self) {
        if let Some(id) = self.sink.take() {
            self.mailbox.borrow_mut().activations.push(id);
            if self.policy.dismiss.on_activation {
                self.open_title = None;
            } else if let Some(idx) = self.open_title {
                // Menu stays open after activation: re-show the same title.
                let items = self.titles[idx].items.clone();
                self.surface.show(items);
            }
            return;
        }
        if !self.surface.is_open() {
            self.open_title = None;
        }
    }

    fn traverse(&mut self, dir: i32) {
        let n = self.titles.len();
        if n == 0 {
            return;
        }
        if let Some(open) = self.open_title {
            let next = (open as i32 + dir).rem_euclid(n as i32) as usize;
            if next != open {
                self.open_title_at(next);
            }
        }
    }

    fn title_index_at(&self, x: f32, y: f32) -> Option<usize> {
        self.title_rects
            .iter()
            .position(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
    }

    /// True when (widget-local) point lies inside the strip band.
    fn in_strip(&self, y: f32) -> bool {
        y >= 0.0 && y < self.bar_height
    }

    /// True when (widget-local) point lies inside the open popup. Popup origin
    /// is the anchor this widget paints with: (open cell x, bar height).
    fn point_in_open_popup(&self, x: f32, y: f32) -> bool {
        let Some(idx) = self.open_title else {
            return false;
        };
        let Some(cell) = self.title_rects.get(idx) else {
            return false;
        };
        let px = cell.x;
        let py = self.bar_height;
        let w = self.overlay.cached_popup_width();
        let h = self.overlay.cached_popup_height();
        x >= px && x < px + w && y >= py && y < py + h
    }

    /// Pointer hover bookkeeping. Returns true when the hovered title changed.
    fn update_hover(&mut self, x: f32, y: f32) -> bool {
        let new = if self.in_strip(y) {
            self.title_index_at(x, y)
        } else {
            None
        };
        if new != self.hovered_title {
            self.hovered_title = new;
            true
        } else {
            false
        }
    }
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(MenuBarMailbox::default())))
    }
}

impl WidgetDowncast for MenuBar {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for MenuBar {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // SPEC-HAL-333: an empty projection occupies nothing. Until an
        // application supplies a MenuModel (INV-G1), the bar collapses to
        // zero height and ScreenHost allocates no strip.
        if self.titles.is_empty() {
            self.title_rects.clear();
            self.bar_height = 0.0;
            let width = if constraints.max_width.is_finite() {
                constraints.max_width
            } else {
                0.0
            };
            return LayoutResult::fit(Size::new(width, 0.0));
        }

        let font_size = cx.theme.typography.font_size;
        let padding_x = cx.theme.geometry.button.padding_x;
        let height = cx.theme.geometry.button.height;
        self.bar_height = height;

        self.title_rects.clear();
        let mut cursor_x = 0.0f32;
        for title in &self.titles {
            let run = cx.text_engine.shape(&title.label, font_size);
            let cell_width = run.width + padding_x * 2.0;
            self.title_rects
                .push(RectF::new(cursor_x, 0.0, cell_width, height));
            cursor_x += cell_width;
        }

        // INV-G2: the bar claims the full top strip width.
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            cursor_x
        };
        LayoutResult::fit(Size::new(width, height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.cached_rect.set(rect);

        // SPEC-HAL-333: empty projection — no chrome, no hairline.
        if self.titles.is_empty() {
            return;
        }

        let controls = &cx.theme.colors.controls;
        let font_size = cx.theme.typography.font_size;

        // Bar background across the full strip (INV-D5: theme-resolved).
        cx.scene
            .push_rect(rect, controls.background, LAYER_CONTENT, None);
        // Bottom hairline separating bar from content (SPEC-HAL-332).
        let hairline = RectF::new(
            rect.x,
            rect.y + rect.height - HAIRLINE,
            rect.width,
            HAIRLINE,
        );
        cx.scene.push_rect(
            hairline,
            cx.theme.colors.borders.default,
            LAYER_CONTENT + 1,
            None,
        );

        for (i, (title, cell)) in self.titles.iter().zip(self.title_rects.iter()).enumerate() {
            let abs = RectF::new(rect.x + cell.x, rect.y + cell.y, cell.width, cell.height);

            // INV-D3: flat titles. Rest paints no cell fill; hover and
            // active-open are visually distinct.
            let is_open = self.open_title == Some(i);
            let is_hovered = self.hovered_title == Some(i);
            if is_open {
                // SPEC-HAL-333 emphasis: the open title paints the popover
                // surface so title and popup read as one connected surface.
                cx.scene.push_rect(
                    abs,
                    cx.theme.colors.surfaces.level3,
                    LAYER_CONTENT + 1,
                    None,
                );
            } else if is_hovered {
                cx.scene
                    .push_rect(abs, controls.background_hover, LAYER_CONTENT + 1, None);
            }

            let run = cx.text_engine.shape(&title.label, font_size);
            let text_x = abs.x + (abs.width - run.width) * 0.5;
            let text_y = abs.y + (abs.height - run.height) * 0.5;
            cx.scene.push_text_with_backgrounds(
                &run,
                text_x,
                text_y,
                controls.text,
                vec![],
                LAYER_CONTENT + 2,
                None,
            );
        }
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        if !self.surface.is_open() {
            return;
        }
        let Some(idx) = self.open_title else {
            return;
        };
        let Some(cell) = self.title_rects.get(idx) else {
            return;
        };

        // Popup anchored below the open title cell. Owner region = the whole
        // bar, so pointer events anywhere route back here in bar-local coords
        // (title clicks and hover-switch keep working while open).
        let anchor = (rect.x + cell.x, rect.y + rect.height);
        let min_width = cell.width.max(POPUP_MIN_WIDTH);
        self.overlay.paint(
            cx,
            &self.surface,
            anchor,
            min_width,
            tree_widget_id,
            Some(rect),
            rect,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        let is_open = self.open_title.is_some();

        if is_open {
            // ---- Bar-strip pointer events first (INV-D2: bar owns titles) ----
            if let Event::Pointer(pe) = event {
                let (lx, ly) = pe.local_pos;
                if self.in_strip(ly) {
                    match pe.kind {
                        PointerEventKind::Move => {
                            let changed = self.update_hover(lx, ly);
                            if self.policy.hover_switch_while_open {
                                if let Some(i) = self.hovered_title {
                                    if Some(i) != self.open_title {
                                        self.open_title_at(i);
                                        cx.request_continuous_frames();
                                    }
                                }
                            }
                            if changed {
                                cx.request_continuous_frames();
                            }
                            // Not consumed: hover lifecycle still applies.
                            return false;
                        }
                        PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                            match self.title_index_at(lx, ly) {
                                Some(i) if Some(i) == self.open_title => {
                                    if self.policy.close_on_reactivate_title {
                                        self.close_menu();
                                        cx.request_continuous_frames();
                                    }
                                }
                                Some(i) => {
                                    self.open_title_at(i);
                                    cx.request_continuous_frames();
                                }
                                None => {
                                    // Strip gap: outside any title and popup.
                                    if self.policy.dismiss.on_click_outside {
                                        self.close_menu();
                                        cx.request_continuous_frames();
                                    }
                                }
                            }
                            return true;
                        }
                        PointerEventKind::Up => {
                            // Own the press sequence that began in the strip.
                            return true;
                        }
                        _ => {}
                    }
                } else if pe.kind == PointerEventKind::Down
                    && !self.point_in_open_popup(lx, ly)
                    && !self.policy.dismiss.on_click_outside
                {
                    // Policy keeps the menu open on outside clicks: do not
                    // delegate, or the overlay would dismiss.
                    return false;
                }
            }

            // ---- Keyboard gates per policy ----
            if let Event::Key(ke) = event {
                if ke.is_down {
                    if ke.key_code == KeyCode::Escape && !self.policy.dismiss.on_escape {
                        return false;
                    }
                    if self.policy.horizontal_traversal {
                        match ke.key_code {
                            KeyCode::Left => {
                                self.traverse(-1);
                                cx.request_continuous_frames();
                                return true;
                            }
                            KeyCode::Right => {
                                self.traverse(1);
                                cx.request_continuous_frames();
                                return true;
                            }
                            _ => {}
                        }
                    }
                }
            }

            // ---- Delegate everything else to the overlay (INV-D1) ----
            if let Some(consumed) = self.overlay.handle_event(&mut self.surface, event, true) {
                self.reconcile_after_overlay();
                if consumed {
                    cx.invalidate_layout();
                }
                return consumed;
            }
            return false;
        }

        // ---- Closed ----
        match event {
            Event::Pointer(pe) if pe.kind == PointerEventKind::Move => {
                let (lx, ly) = pe.local_pos;
                let changed = self.update_hover(lx, ly);
                if self.policy.open_trigger == OpenTrigger::HoverToOpen {
                    if let Some(i) = self.hovered_title {
                        self.open_title_at(i);
                        cx.request_continuous_frames();
                        return true;
                    }
                }
                if changed {
                    cx.request_continuous_frames();
                }
                false
            }
            Event::Pointer(pe)
                if pe.kind == PointerEventKind::Down
                    && pe.button == Some(PointerButton::Primary) =>
            {
                let (lx, ly) = pe.local_pos;
                if self.in_strip(ly) {
                    if let Some(i) = self.title_index_at(lx, ly) {
                        self.open_title_at(i);
                        cx.request_continuous_frames();
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        if self.open_title.is_some() {
            self.close_menu();
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.hovered_title.is_some() {
            self.hovered_title = None;
            cx.request_continuous_frames();
        }
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "MenuBar"
    }
}

// ============================================================================
// InWindowMenuHost — concrete bridge (SPEC-HAL-333 Rev 4 INV-G2)
// ============================================================================

/// The in-window menu host: the concrete bridge between AppRoot and the MenuBar widget.
/// Holds no tree
/// access and no authoritative state: projection data and activations flow
/// through the shared mailbox; AppRoot retains the bar's NodeId for mount
/// lifecycle and layout-dirty marking (SPEC-HAL-158).
pub struct InWindowMenuHost {
    mailbox: Rc<RefCell<MenuBarMailbox>>,
}

impl InWindowMenuHost {
    pub fn new(mailbox: Rc<RefCell<MenuBarMailbox>>) -> Self {
        Self { mailbox }
    }
}

impl PlatformMenuHost for InWindowMenuHost {
    /// Realize the full model into the mailbox (INV-G3 step 1).
    fn install(&mut self, model: &MenuModel, _registry: &ActionRegistry) {
        let titles: Vec<BarTitle> = model
            .titles()
            .iter()
            .map(|t| BarTitle {
                label: t.title.clone(),
                items: project_items(&t.items, model),
            })
            .collect();
        let mut mb = self.mailbox.borrow_mut();
        mb.staged_titles = Some(titles);
        // Stale per-item syncs target the replaced projection: drop them.
        mb.staged_syncs.clear();
    }

    /// Mirror one item's current model state into the mailbox (INV-A5).
    fn sync_item(&mut self, model: &MenuModel, id: MenuItemId) {
        let Some(state) = model.item_state(id) else {
            return;
        };
        let Some(label) = model.item_label(id) else {
            return;
        };
        // Checked is not consumed yet — named SPEC-HAL-223 gap (popup check
        // visuals). Enabled + label mirror per INV-A5.
        self.mailbox.borrow_mut().staged_syncs.push(StagedItemSync {
            id,
            enabled: state.enabled,
            label: label.to_string(),
        });
    }

    /// Drain activations buffered since the last call (INV-E2).
    fn drain_activations(&mut self) -> Vec<MenuItemId> {
        std::mem::take(&mut self.mailbox.borrow_mut().activations)
    }

    fn occupies_in_window_bar(&self) -> bool {
        true
    }
}

/// Deep-copy a MenuItem tree applying live model state: enabled overrides the
/// copy's disabled flag (the overlay renders from MenuItem data).
fn project_items(items: &[MenuItem], model: &MenuModel) -> Vec<MenuItem> {
    items
        .iter()
        .map(|item| match item {
            MenuItem::Separator => MenuItem::Separator,
            MenuItem::ActionRow {
                id,
                label,
                icon,
                shortcut,
                disabled,
                submenu,
            } => {
                let enabled = model
                    .item_state(*id)
                    .map(|s| s.enabled)
                    .unwrap_or(!*disabled);
                MenuItem::ActionRow {
                    id: *id,
                    label: label.clone(),
                    icon: *icon,
                    shortcut: shortcut.clone(),
                    disabled: !enabled,
                    submenu: submenu.as_ref().map(|c| project_items(c, model)),
                }
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(id: MenuItemId, label: &str) -> MenuItem {
        MenuItem::ActionRow {
            id,
            label: label.to_string(),
            icon: None,
            shortcut: None,
            disabled: false,
            submenu: None,
        }
    }

    fn rig() -> (MenuBar, InWindowMenuHost) {
        let mailbox = Rc::new(RefCell::new(MenuBarMailbox::default()));
        (
            MenuBar::new(Rc::clone(&mailbox)),
            InWindowMenuHost::new(mailbox),
        )
    }

    fn action_registry() -> ActionRegistry {
        ActionRegistry::new()
    }

    fn two_title_model() -> MenuModel {
        let mut model = MenuModel::new();
        model.add_title("File", vec![row(1, "Open"), row(2, "Save")]);
        model.add_title("Edit", vec![row(10, "Undo")]);
        model
    }

    fn bar_with_two_titles() -> MenuBar {
        let mut bar = MenuBar::default();
        bar.set_titles(vec![
            BarTitle {
                label: "File".to_string(),
                items: vec![row(1, "Open"), row(2, "Save")],
            },
            BarTitle {
                label: "Edit".to_string(),
                items: vec![row(10, "Undo")],
            },
        ]);
        bar
    }

    #[test]
    fn policy_canonical_defaults() {
        let p = MenuBarPolicy::default();
        assert_eq!(p.open_trigger, OpenTrigger::ClickToOpen);
        assert!(p.hover_switch_while_open);
        assert!(p.horizontal_traversal);
        assert!(p.close_on_reactivate_title);
        assert!(p.dismiss.on_click_outside);
        assert!(p.dismiss.on_escape);
        assert!(p.dismiss.on_activation);
    }

    #[test]
    fn open_then_activation_flows_through_mailbox_and_closes_by_default() {
        let (mut bar, mut host) = rig();
        host.install(&two_title_model(), &action_registry());
        assert!(bar.apply_mailbox());

        bar.open_title_at(0);
        assert!(bar.is_open());

        // As the overlay would on click/Enter:
        bar.surface.activate_index(1); // id 2
        bar.reconcile_after_overlay();

        assert!(!bar.is_open());
        assert!(!bar.surface.is_open());
        assert_eq!(host.drain_activations(), vec![2]);
        assert!(host.drain_activations().is_empty());
    }

    #[test]
    fn activation_with_on_activation_false_keeps_menu_open() {
        let (mut bar, mut host) = rig();
        host.install(&two_title_model(), &action_registry());
        bar.apply_mailbox();
        let mut policy = MenuBarPolicy::default();
        policy.dismiss.on_activation = false;
        bar.set_policy(policy);

        bar.open_title_at(0);
        bar.surface.activate_index(0); // id 1
        bar.reconcile_after_overlay();

        assert!(bar.is_open());
        assert_eq!(bar.open_title_index(), Some(0));
        assert!(bar.surface.is_open());
        assert_eq!(host.drain_activations(), vec![1]);
    }

    #[test]
    fn overlay_dismissal_without_activation_closes_bar_state() {
        let mut bar = bar_with_two_titles();
        bar.open_title_at(1);

        // As the overlay would on Escape / click outside:
        bar.surface.dismiss();
        bar.reconcile_after_overlay();

        assert!(!bar.is_open());
        assert!(bar.mailbox.borrow().activations.is_empty());
    }

    #[test]
    fn traverse_wraps_and_switches_surface_items() {
        let mut bar = bar_with_two_titles();
        bar.open_title_at(0);

        bar.traverse(-1);
        assert_eq!(bar.open_title_index(), Some(1));
        match bar.surface.items().first() {
            Some(MenuItem::ActionRow { id, .. }) => assert_eq!(*id, 10),
            other => panic!("expected Edit items, got {:?}", other.is_some()),
        }

        bar.traverse(1);
        assert_eq!(bar.open_title_index(), Some(0));
    }

    #[test]
    fn set_titles_resets_open_and_hover_state() {
        let mut bar = bar_with_two_titles();
        bar.open_title_at(0);
        bar.hovered_title = Some(1);

        bar.set_titles(vec![BarTitle {
            label: "Only".to_string(),
            items: vec![row(99, "Item")],
        }]);

        assert!(!bar.is_open());
        assert!(!bar.surface.is_open());
        assert_eq!(bar.hovered_title, None);
    }

    #[test]
    fn out_of_range_open_is_no_op() {
        let mut bar = bar_with_two_titles();
        bar.open_title_at(5);
        assert!(!bar.is_open());
        assert!(!bar.surface.is_open());
    }

    #[test]
    fn install_then_apply_replaces_titles_once() {
        let (mut bar, mut host) = rig();
        host.install(&two_title_model(), &action_registry());

        assert!(bar.apply_mailbox());
        assert_eq!(bar.titles.len(), 2);
        assert_eq!(bar.titles[0].label, "File");
        assert_eq!(bar.titles[1].label, "Edit");

        // Nothing staged: second apply is a no-op.
        assert!(!bar.apply_mailbox());
    }

    #[test]
    fn install_projects_enabled_state_including_submenus() {
        let (mut bar, mut host) = rig();
        let mut model = MenuModel::new();
        model.add_title(
            "File",
            vec![MenuItem::ActionRow {
                id: 1,
                label: "Export".to_string(),
                icon: None,
                shortcut: None,
                disabled: false,
                submenu: Some(vec![row(11, "As PDF")]),
            }],
        );
        model.set_enabled(11, false);
        host.install(&model, &action_registry());
        bar.apply_mailbox();

        match find_row(&bar.titles[0].items, 11) {
            Some(MenuItem::ActionRow { disabled, .. }) => assert!(*disabled),
            other => panic!("expected nested row, got {:?}", other.is_some()),
        }
    }

    #[test]
    fn sync_item_patches_enabled_and_label() {
        let (mut bar, mut host) = rig();
        let mut model = two_title_model();
        host.install(&model, &action_registry());
        bar.apply_mailbox();

        model.set_label(1, "Renamed".to_string());
        model.set_enabled(1, false);
        host.sync_item(&model, 1);
        assert!(bar.apply_mailbox());

        match find_row(&bar.titles[0].items, 1) {
            Some(MenuItem::ActionRow {
                label, disabled, ..
            }) => {
                assert_eq!(label, "Renamed");
                assert!(*disabled);
            }
            other => panic!("expected row, got {:?}", other.is_some()),
        }
    }

    #[test]
    fn sync_on_open_title_reshows_surface() {
        let (mut bar, mut host) = rig();
        let mut model = two_title_model();
        host.install(&model, &action_registry());
        bar.apply_mailbox();
        bar.open_title_at(0);

        model.set_label(1, "Renamed".to_string());
        host.sync_item(&model, 1);
        assert!(bar.apply_mailbox());

        assert!(bar.is_open());
        assert!(bar.surface.is_open());
        match bar.surface.items().first() {
            Some(MenuItem::ActionRow { label, .. }) => assert_eq!(label, "Renamed"),
            other => panic!("expected row, got {:?}", other.is_some()),
        }
    }

    #[test]
    fn sync_unknown_id_stages_nothing() {
        let (mut bar, mut host) = rig();
        let model = two_title_model();
        host.install(&model, &action_registry());
        bar.apply_mailbox();

        host.sync_item(&model, 999);
        assert!(!bar.apply_mailbox());
    }

    #[test]
    fn install_drops_stale_staged_syncs() {
        let (mut bar, mut host) = rig();
        let mut model = two_title_model();
        model.set_enabled(1, false);
        host.sync_item(&model, 1);

        host.install(&model, &action_registry());
        assert!(bar.mailbox.borrow().staged_syncs.is_empty());
        assert!(bar.apply_mailbox());
    }
}
