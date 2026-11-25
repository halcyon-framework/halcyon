// SPEC-HAL-310: Navigator — Virtualized Item Grid
//
// INV-310-0: Uses FROZEN DataSource trait.
// INV-310-1: Virtualized — only visible item rows painted.
// INV-310-6: Does NOT use TabularSubstrate.

use crate::core::event::{Event, EventPhase, KeyCode};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::ordered_selection::{
    OrderedIndex, OrderedSelectionAction, OrderedSelectionState, SelectionInteractionModel,
    SelectionMode, reduce_ordered_selection,
};
use crate::core::paint::LAYER_CONTENT;
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::navigation_host::{NavigationHost, NavigationIntent, NavigationNodeId};
use crate::utils::id::WidgetInternalId;
use crate::widgets::tabular_view::intent_policy::{ActivationMode, RangePolicy};
use crate::widgets::tabular_view::row::RowId;

use super::body::NavigatorBody;
use super::data_source::NavigatorDataSource;
use super::item_painter::ItemPainter;

use std::cell::RefCell;
use std::rc::Rc;

// ─── Config ─────────────────────────────────────────────────────────────────

/// SPEC-HAL-310 §6: Layout configuration for grid-based modes.
/// Contains ONLY layout geometry. No visual chrome.
/// Visual item styling is the ItemPainter's responsibility.
pub struct NavigatorConfig {
    /// Item width in logical pixels (grid-based modes).
    pub item_width: f32,
    /// Item height in logical pixels (grid-based modes).
    pub item_height: f32,
    /// Horizontal gap between items.
    pub gap_x: f32,
    /// Vertical gap between item rows.
    pub gap_y: f32,
    /// Horizontal alignment when items don't fill the row.
    pub alignment: GridAlignment,
    /// Opt-in fluid fill (grid-based modes). When true, item
    /// width stretches to fill the row content width per
    /// SPEC-HAL-310 INV-310-13; supersedes `alignment`. Default false.
    pub fill_width: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridAlignment {
    Left,
    Center,
}

/// SPEC-HAL-310 §4: Available view modes for content projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavigatorViewMode {
    /// Columnar table with headers. Uses ListView internally.
    Details,
    /// Compact icon + name, column-first flow. Uses List internally.
    List,
    /// Small icon centered, name below. Grid flow.
    SmallIcon,
    /// Medium icon centered, name below. Grid flow.
    Icon,
    /// Large icon centered, name below. Grid flow.
    LargeIcon,
    /// Extra large icon/preview, name below. Grid flow.
    ExtraLargeIcon,
    /// Wide item: icon left, name + metadata right. Grid flow.
    Tile,
    /// Full-width row: icon left, metadata stacked right.
    Content,
}

/// SPEC-HAL-310 §5: Composable interaction policy.
/// Each axis is independently configurable.
/// Follows the TabularIntentPolicy pattern (SPEC-HAL-200).
#[derive(Debug, Clone)]
pub struct NavigatorPolicy {
    /// Which view modes are available. Navigator rejects
    /// set_view_mode() calls for modes not in this list.
    /// Single entry = mode switching disabled.
    pub allowed_modes: Vec<NavigatorViewMode>,
    /// Default view mode. Must be in allowed_modes.
    pub default_mode: NavigatorViewMode,
    /// Selection cardinality: None / Single / Multi.
    pub selection_mode: SelectionMode,
    /// Immediate (mutate on input) vs Transactional
    /// (working set, requires explicit commit).
    pub interaction_model: SelectionInteractionModel,
    /// When activation callbacks fire.
    pub activation_mode: ActivationMode,
    /// Whether Shift+click/nav performs range selection.
    pub range_selection: RangePolicy,
    /// Whether hierarchical drill-in navigation is enabled.
    pub navigation_enabled: bool,
}

impl NavigatorPolicy {
    /// Display only. No selection, no keyboard nav.
    pub fn display_only() -> Self {
        Self {
            allowed_modes: vec![NavigatorViewMode::Icon],
            default_mode: NavigatorViewMode::Icon,
            selection_mode: SelectionMode::None,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::ExplicitOnly,
            range_selection: RangePolicy::Disabled,
            navigation_enabled: false,
        }
    }

    /// Single-select browser.
    pub fn single_select() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::Icon,
                NavigatorViewMode::Tile,
            ],
            default_mode: NavigatorViewMode::Details,
            selection_mode: SelectionMode::Single,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::Disabled,
            navigation_enabled: false,
        }
    }

    /// Multi-select browser.
    pub fn multi_select() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::List,
                NavigatorViewMode::SmallIcon,
                NavigatorViewMode::Icon,
                NavigatorViewMode::LargeIcon,
                NavigatorViewMode::ExtraLargeIcon,
                NavigatorViewMode::Tile,
                NavigatorViewMode::Content,
            ],
            default_mode: NavigatorViewMode::Details,
            selection_mode: SelectionMode::Multi,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::ShiftRange,
            navigation_enabled: false,
        }
    }

    /// Full file browser. All modes, navigation, multi-select.
    pub fn file_browser() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::List,
                NavigatorViewMode::SmallIcon,
                NavigatorViewMode::Icon,
                NavigatorViewMode::LargeIcon,
                NavigatorViewMode::ExtraLargeIcon,
                NavigatorViewMode::Tile,
                NavigatorViewMode::Content,
            ],
            default_mode: NavigatorViewMode::Details,
            selection_mode: SelectionMode::Multi,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::ShiftRange,
            navigation_enabled: true,
        }
    }

    /// Asset picker with transactional selection.
    pub fn asset_picker() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::Icon,
                NavigatorViewMode::Tile,
            ],
            default_mode: NavigatorViewMode::Icon,
            selection_mode: SelectionMode::Multi,
            interaction_model: SelectionInteractionModel::Transactional,
            activation_mode: ActivationMode::ExplicitOnly,
            range_selection: RangePolicy::ShiftRange,
            navigation_enabled: true,
        }
    }
}

impl Default for NavigatorPolicy {
    fn default() -> Self {
        Self::single_select()
    }
}

impl Default for NavigatorConfig {
    fn default() -> Self {
        Self {
            item_width: 100.0,
            item_height: 96.0,
            gap_x: 12.0,
            gap_y: 12.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
}

impl NavigatorConfig {
    pub fn details() -> Self {
        Self {
            item_width: 0.0,
            item_height: 0.0,
            gap_x: 0.0,
            gap_y: 0.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
    pub fn list() -> Self {
        Self {
            item_width: 0.0,
            item_height: 0.0,
            gap_x: 0.0,
            gap_y: 0.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
    pub fn small_icon() -> Self {
        Self {
            item_width: 80.0,
            item_height: 80.0,
            gap_x: 8.0,
            gap_y: 8.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
    pub fn icon() -> Self {
        Self::default()
    }
    pub fn large_icon() -> Self {
        Self {
            item_width: 128.0,
            item_height: 120.0,
            gap_x: 12.0,
            gap_y: 12.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
    pub fn extra_large_icon() -> Self {
        Self {
            item_width: 256.0,
            item_height: 220.0,
            gap_x: 16.0,
            gap_y: 16.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
    pub fn tile() -> Self {
        Self {
            item_width: 240.0,
            item_height: 60.0,
            gap_x: 8.0,
            gap_y: 4.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }
    pub fn content() -> Self {
        Self {
            item_width: 0.0,
            item_height: 64.0,
            gap_x: 0.0,
            gap_y: 2.0,
            alignment: GridAlignment::Left,
            fill_width: false,
        }
    }

    /// Return the config preset for a given view mode.
    /// Details and List return icon() — they will be handled
    /// by widget swapping in a future task.
    pub fn for_mode(mode: NavigatorViewMode) -> Self {
        match mode {
            NavigatorViewMode::SmallIcon => Self::small_icon(),
            NavigatorViewMode::Icon => Self::icon(),
            NavigatorViewMode::LargeIcon => Self::large_icon(),
            NavigatorViewMode::ExtraLargeIcon => Self::extra_large_icon(),
            NavigatorViewMode::Tile => Self::tile(),
            NavigatorViewMode::Content => Self::content(),
            // Details/List use different internal widgets — config
            // is irrelevant. Fall back to icon for now.
            NavigatorViewMode::Details | NavigatorViewMode::List => Self::icon(),
        }
    }
}

// ─── Events ─────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub enum NavigatorEvent {
    /// Single click on an item.
    ItemClicked(RowId),
    /// Double click on an item.
    ItemDoubleClicked(RowId),
    /// Item activated (per activation_mode policy).
    ItemActivated(RowId),
    /// Selection set changed.
    SelectionChanged,
    /// View mode changed.
    ViewModeChanged(NavigatorViewMode),
    /// User drilled into a navigable item.
    /// Only emitted when policy.navigation_enabled == true.
    Navigated(NavigationNodeId),
}

// ─── OrderedIndex oracle for row_ids ────────────────────────────────────────

pub(crate) struct RowIdOracle<'a> {
    pub row_ids: &'a [RowId],
}

impl<'a> OrderedIndex<RowId> for RowIdOracle<'a> {
    fn len(&self) -> usize {
        self.row_ids.len()
    }

    fn key_at(&self, index: usize) -> RowId {
        self.row_ids[index]
    }

    fn index_of(&self, key: RowId) -> Option<usize> {
        self.row_ids.iter().position(|r| *r == key)
    }
}

// ─── Drag State (Phase 5/6) ─────────────────────────────────────────────────

pub(crate) struct DragState {
    /// ALL items being dragged (1 for single, N for multi).
    pub dragged_items: Vec<RowId>,
    /// Source indices (sorted ascending).
    pub source_indices: Vec<usize>,
    /// Current pointer position (relative to body).
    pub pointer: (f32, f32),
    /// Computed insertion index.
    pub insert_index: usize,
    /// Pointer offset from item top-left at drag start.
    pub grab_offset: (f32, f32),
}

// ─── Navigation ────────────────────────────────────────────────────────────

/// Optional navigation state. When present, Navigator
/// participates on the NavigationHost bus.
pub(crate) struct NavigatorNavigation {
    /// Reference to the shared NavigationHost.
    pub host: Rc<RefCell<NavigationHost>>,
    /// Last-seen location. When host.current() differs,
    /// Navigator re-queries DataSource.
    pub observed_location: NavigationNodeId,
}

// ─── State ──────────────────────────────────────────────────────────────────

pub(crate) struct NavigatorState {
    pub config: NavigatorConfig,
    pub policy: NavigatorPolicy,
    /// Current view mode.
    pub view_mode: NavigatorViewMode,
    /// Cached row IDs from DataSource.
    pub row_ids: Vec<RowId>,
    /// Computed column count for current width.
    pub columns: usize,
    /// Current hovered item index. None = no hover.
    pub hovered_index: Option<usize>,
    /// Available width (set during layout).
    pub available_width: f32,
    /// Viewport height (set during layout).
    pub viewport_height: f32,
    /// Current scroll Y position (read from ScrollArea in layout).
    pub scroll_y: f32,
    /// Shared painter delegate.
    pub painter: Rc<dyn ItemPainter>,
    /// Selection state (SPEC-HAL-310 §7).
    pub selection: OrderedSelectionState<RowId>,
    /// Pending events for consumer polling.
    pub pending_events: Vec<NavigatorEvent>,
    /// Whether the container has keyboard focus (Phase 3).
    pub focused: bool,
    /// Last click for double-click detection (Phase 4).
    pub last_click: Option<(usize, web_time::Instant)>,
    /// Active drag state (Phase 5/6).
    pub drag: Option<DragState>,
    /// Armed drag: index + down position (not yet past threshold).
    pub drag_arm: Option<(usize, (f32, f32))>,
    /// Per-item animated shift offset.
    pub shift_offsets: Vec<f32>,
    /// Pending scroll-to-Y for auto-reveal. Set by keyboard nav,
    /// consumed by body's take_requested_scroll_y().
    pub pending_scroll_to: Option<f32>,
    /// Committed scroll target — survives take_requested_scroll_y.
    /// Used by navigate_to_index for visibility checks during
    /// rapid keypresses when smooth scroll hasn't converged.
    pub scroll_target: Option<f32>,
    /// Optional navigation integration (SPEC-HAL-310 §8).
    pub navigation: Option<NavigatorNavigation>,
}

// ─── Navigator ───────────────────────────────────────────────────────────────

pub struct Navigator<S: NavigatorDataSource> {
    id: WidgetInternalId,
    state: Rc<RefCell<NavigatorState>>,
    source: Rc<RefCell<S>>,
    /// Last layout viewport height (for page size computation).
    last_body_h: f32,
}

impl<S: NavigatorDataSource> Navigator<S> {
    pub fn new(source: S, painter: Rc<dyn ItemPainter>) -> Self {
        let policy = NavigatorPolicy::default();
        Self {
            id: WidgetInternalId::new(),
            state: Rc::new(RefCell::new(NavigatorState {
                config: NavigatorConfig::default(),
                policy: policy.clone(),
                view_mode: policy.default_mode,
                row_ids: Vec::new(),
                columns: 1,
                hovered_index: None,
                available_width: 0.0,
                viewport_height: 0.0,
                scroll_y: 0.0,
                painter,
                selection: OrderedSelectionState::new(policy.selection_mode),
                pending_events: Vec::new(),
                focused: false,
                last_click: None,
                drag: None,
                drag_arm: None,
                shift_offsets: Vec::new(),
                pending_scroll_to: None,
                scroll_target: None,
                navigation: None,
            })),
            source: Rc::new(RefCell::new(source)),
            last_body_h: 0.0,
        }
    }

    pub fn with_config(self, config: NavigatorConfig) -> Self {
        self.state.borrow_mut().config = config;
        self
    }

    pub fn with_policy(self, policy: NavigatorPolicy) -> Self {
        let mut state = self.state.borrow_mut();
        state.selection.selection_mode = policy.selection_mode;
        state.selection.interaction_model = policy.interaction_model;
        state.view_mode = policy.default_mode;
        state.config = NavigatorConfig::for_mode(policy.default_mode);
        state.policy = policy;
        drop(state);
        self
    }

    /// Switch view mode at runtime. Rejects modes not in
    /// policy.allowed_modes (INV-310-12). Applies the
    /// corresponding config preset and resets scroll.
    pub fn set_view_mode(&self, mode: NavigatorViewMode) {
        let mut state = self.state.borrow_mut();
        if !state.policy.allowed_modes.contains(&mode) {
            return; // INV-310-12: mode not allowed
        }
        if state.view_mode == mode {
            return; // no-op
        }
        state.view_mode = mode;
        state.config = NavigatorConfig::for_mode(mode);
        // Reset scroll position (INV-310-9)
        state.scroll_y = 0.0;
        state.pending_scroll_to = Some(0.0);
        // Emit event
        state
            .pending_events
            .push(NavigatorEvent::ViewModeChanged(mode));
    }

    /// Enable hierarchical navigation via NavigationHost.
    /// Only effective when policy.navigation_enabled == true.
    pub fn with_navigation(self, host: Rc<RefCell<NavigationHost>>) -> Self {
        let current = host.borrow().current();
        self.state.borrow_mut().navigation = Some(NavigatorNavigation {
            host,
            observed_location: current,
        });
        self
    }

    /// Share selection state with another widget (e.g., ListView).
    pub fn with_selection(self, selection: &OrderedSelectionState<RowId>) -> Self {
        self.state.borrow_mut().selection = selection.clone();
        self
    }

    /// Create the body widget. Shares state and data source with this container via Rc.
    pub fn create_body(&self) -> NavigatorBody<S> {
        NavigatorBody::new(self.state.clone(), self.source.clone())
    }

    /// Take pending events for consumer polling.
    pub fn take_events(&self) -> Vec<NavigatorEvent> {
        std::mem::take(&mut self.state.borrow_mut().pending_events)
    }
}

/// Navigate focus to a new index, update selection, return scroll target if off-screen.
/// Free function to avoid borrow conflicts with RefCell<NavigatorState>.
fn navigate_to_index(state: &mut NavigatorState, new_index: usize, extend: bool) {
    let count = state.row_ids.len();
    let cols = state.columns.max(1);
    if new_index >= count {
        return;
    }

    // Auto-reveal: if target item is not fully visible, scroll to show it.
    let cfg = &state.config;
    let item_row = new_index / cols;
    let row_stride = cfg.item_height + cfg.gap_y;
    let item_top = item_row as f32 * row_stride;
    let item_bottom = item_top + cfg.item_height;
    let reveal_margin = cfg.gap_y;
    let scroll_y = state.scroll_target.unwrap_or(state.scroll_y);
    let viewport_h = state.viewport_height;

    if item_bottom > scroll_y + viewport_h {
        let target = (item_bottom + reveal_margin - viewport_h).max(0.0);
        state.pending_scroll_to = Some(target);
        state.scroll_target = Some(target);
    } else if item_top < scroll_y {
        let target = (item_top - reveal_margin).max(0.0);
        state.pending_scroll_to = Some(target);
        state.scroll_target = Some(target);
    }

    // Selection update.
    let row_id = state.row_ids[new_index];
    let row_ids_snapshot = state.row_ids.clone();
    let oracle = RowIdOracle {
        row_ids: &row_ids_snapshot,
    };

    let action = if extend {
        OrderedSelectionAction::ExtendTo { key: row_id }
    } else {
        OrderedSelectionAction::Select { key: row_id }
    };

    let result = reduce_ordered_selection(&mut state.selection, action, &oracle);

    if result.selection_changed {
        state.pending_events.push(NavigatorEvent::SelectionChanged);
    }
}

impl<S: NavigatorDataSource + 'static> WidgetDowncast for Navigator<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S: NavigatorDataSource + 'static> Widget for Navigator<S> {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let w = constraints.max_width;
        let h = constraints.max_height;

        self.last_body_h = h;

        {
            let mut state = self.state.borrow_mut();
            state.available_width = w;
            state.viewport_height = h;

            // Compute column count (INV-310-3)
            let cfg = &state.config;
            let cols = if cfg.item_width > 0.0 {
                ((w + cfg.gap_x) / (cfg.item_width + cfg.gap_x))
                    .floor()
                    .max(1.0) as usize
            } else {
                1 // Full-width mode (Content, Details, List)
            };
            state.columns = cols;

            // Refresh row IDs from data source (skip during drag to avoid flicker)
            if state.drag.is_none() {
                let source = self.source.borrow();
                state.row_ids = source.row_ids().to_vec();
            }
        }

        // ScrollArea gets full area
        if !children.is_empty() {
            _cx.layout_child(children[0], Constraints::new(w, w, h, h));
            _cx.set_child_rect(children[0], RectF::new(0.0, 0.0, w, h));
        }

        LayoutResult::fit(Size::new(w, h))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene
            .push_rect(rect, cx.theme.colors.surfaces.content, LAYER_CONTENT, None);
    }

    fn update(&mut self, dt: f32, _cx: &mut UpdateContext) {
        // SPEC-HAL-310 §8.5: Detect navigation location changes.
        // Re-query DataSource when NavigationHost's current node changes.
        {
            let mut state = self.state.borrow_mut();
            let location_changed = if let Some(ref mut nav) = state.navigation {
                let host = nav.host.borrow();
                if host.current() != nav.observed_location {
                    nav.observed_location = host.current();
                    true
                } else {
                    false
                }
            } else {
                false
            };

            if location_changed {
                let location = state.navigation.as_ref().unwrap().observed_location;
                let source = self.source.borrow();
                state.row_ids = source.items_at(location);
                // Clear selection on location change (acceptance criterion 23)
                state.selection.selected_keys.clear();
                state.selection.focus_key = None;
                state.selection.anchor_key = None;
                // Reset scroll
                state.pending_scroll_to = Some(0.0);
            }
        }

        // Drain invalidation events from DataSource
        let events = self.source.borrow_mut().drain_invalidation_events();
        if !events.is_empty() {
            let source = self.source.borrow();
            let mut state = self.state.borrow_mut();
            state.row_ids = source.row_ids().to_vec();
        }

        // Animate shift offsets during drag (Phase 5/6)
        let mut state = self.state.borrow_mut();
        if let Some(ref drag) = state.drag {
            let insert = drag.insert_index;
            let group_size = drag.dragged_items.len();
            let source_indices = drag.source_indices.clone();
            let py = drag.pointer.1;

            let alpha = 1.0 - (-12.0 * dt).exp();
            let len = state.shift_offsets.len();
            for i in 0..len {
                let target = if source_indices.contains(&i) {
                    0.0
                } else if i >= insert {
                    group_size as f32
                } else {
                    0.0
                };
                state.shift_offsets[i] += (target - state.shift_offsets[i]) * alpha;
            }

            // Auto-scroll near edges during drag
            let margin = 40.0;
            let speed = 200.0;
            let viewport_h = self.last_body_h;
            let scroll_y = state.scroll_y;
            let drag_scroll = if py < margin && py > 0.0 {
                let intensity = 1.0 - py / margin;
                let delta = -speed * intensity * dt;
                Some((scroll_y + delta).max(0.0))
            } else if py > viewport_h - margin && viewport_h > 0.0 {
                let intensity = 1.0 - (viewport_h - py) / margin;
                let delta = speed * intensity * dt;
                Some(scroll_y + delta)
            } else {
                None
            };
            drop(state);
            if let Some(y) = drag_scroll {
                self.state.borrow_mut().pending_scroll_to = Some(y);
            }
        } // end if drag
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        let mut state = self.state.borrow_mut();
        state.focused = true;
        if state.selection.focus_key.is_none() && !state.row_ids.is_empty() {
            let first = state.row_ids[0];
            state.selection.focus_key = Some(first);
            state.selection.anchor_key = Some(first);
        }
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.state.borrow_mut().focused = false;
    }

    fn handle_event(&mut self, _cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Key(ke) if ke.is_down => {
                // Escape cancels drag
                if ke.key_code == KeyCode::Escape {
                    let mut state = self.state.borrow_mut();
                    // Cancel drag first
                    if state.drag.is_some() {
                        state.drag = None;
                        state.drag_arm = None;
                        state.shift_offsets.clear();
                        drop(state);
                        _cx.release_pointer();
                        return true;
                    }

                    // Clear selection if non-empty (keep focus_key for arrow continuity)
                    if !state.selection.selected_keys.is_empty() {
                        state.selection.selected_keys.clear();
                        state.pending_events.push(NavigatorEvent::SelectionChanged);
                        return true;
                    }

                    // Navigate back if selection already empty and nav enabled
                    if state.policy.navigation_enabled {
                        if let Some(ref nav) = state.navigation {
                            let host_rc = nav.host.clone();
                            let path_len = host_rc.borrow().path().len();
                            if path_len > 1 {
                                drop(state);
                                host_rc.borrow_mut().apply(NavigationIntent::NavigateUp);
                                return true;
                            }
                        }
                    }

                    // Nothing to do — let framework handle Escape (e.g. clear focus).
                    return false;
                }

                let mut state = self.state.borrow_mut();
                if state.policy.selection_mode == SelectionMode::None {
                    return false;
                }
                let cols = state.columns.max(1);
                let count = state.row_ids.len();
                if count == 0 {
                    return false;
                }

                let current = state
                    .selection
                    .focus_key
                    .and_then(|k| state.row_ids.iter().position(|r| *r == k))
                    .unwrap_or(0);

                let page_rows = {
                    let cfg = &state.config;
                    let row_stride = cfg.item_height + cfg.gap_y;
                    (self.last_body_h / row_stride).floor().max(1.0) as usize
                };

                // Early-return actions (control keys only — Event::Key path)
                match ke.key_code {
                    KeyCode::Enter => {
                        if current < count {
                            let row_id = state.row_ids[current];

                            // Check if item is navigable and navigation enabled
                            if state.policy.navigation_enabled {
                                if let Some(ref nav) = state.navigation {
                                    let host_rc = nav.host.clone();
                                    let source = self.source.borrow();
                                    if let Some(node_id) = source.navigation_node(row_id) {
                                        drop(source);
                                        drop(state);
                                        host_rc
                                            .borrow_mut()
                                            .apply(NavigationIntent::NavigateTo(node_id.into()));
                                        let mut state = self.state.borrow_mut();
                                        state
                                            .pending_events
                                            .push(NavigatorEvent::Navigated(node_id));
                                        return true;
                                    }
                                }
                            }

                            // Non-navigable or navigation disabled: activate
                            state
                                .pending_events
                                .push(NavigatorEvent::ItemActivated(row_id));
                        }
                        return true;
                    }
                    KeyCode::Space => {
                        let row_ids_snapshot = state.row_ids.clone();
                        let oracle = RowIdOracle {
                            row_ids: &row_ids_snapshot,
                        };
                        let result = reduce_ordered_selection(
                            &mut state.selection,
                            OrderedSelectionAction::ToggleFocused,
                            &oracle,
                        );
                        if result.selection_changed {
                            state.pending_events.push(NavigatorEvent::SelectionChanged);
                        }
                        return true;
                    }
                    _ => {}
                }

                // Arrow/Home/End/Page navigation (control keys — Event::Key path)
                let new_index = match ke.key_code {
                    KeyCode::Left => current.saturating_sub(1),
                    KeyCode::Right => (current + 1).min(count.saturating_sub(1)),
                    KeyCode::Up => current.saturating_sub(cols),
                    KeyCode::Down => (current + cols).min(count.saturating_sub(1)),
                    KeyCode::Home => 0,
                    KeyCode::End => count.saturating_sub(1),
                    KeyCode::PageUp => current.saturating_sub(cols * page_rows),
                    KeyCode::PageDown => (current + cols * page_rows).min(count.saturating_sub(1)),
                    _ => return false,
                };
                let new_index = new_index.min(count.saturating_sub(1));

                if new_index != current {
                    navigate_to_index(&mut state, new_index, ke.modifiers.shift);
                }
                true
            }

            // Character keys arrive via Event::Text, not Event::Key.
            // Platform sends letter keys as TextInput, not KeyDown.
            Event::Text(te) => {
                let mut state = self.state.borrow_mut();
                if state.policy.selection_mode == SelectionMode::None {
                    return false;
                }
                let cols = state.columns.max(1);
                let count = state.row_ids.len();
                if count == 0 {
                    return false;
                }

                let shortcut_mod = te.modifiers.ctrl || te.modifiers.meta;

                // Ctrl+A / Cmd+A: select all
                if shortcut_mod && (te.text == "a" || te.text == "A") {
                    if state.policy.selection_mode == SelectionMode::Multi {
                        let row_ids_snapshot = state.row_ids.clone();
                        let oracle = RowIdOracle {
                            row_ids: &row_ids_snapshot,
                        };
                        reduce_ordered_selection(
                            &mut state.selection,
                            OrderedSelectionAction::SelectAll,
                            &oracle,
                        );
                        state.pending_events.push(NavigatorEvent::SelectionChanged);
                    }
                    return true;
                }

                // Don't handle character keys when modifiers are held
                if shortcut_mod || te.modifiers.alt {
                    return false;
                }

                // `i` = interact: activate focused item (same as Enter)
                if te.text == "i" || te.text == "I" {
                    let current = state
                        .selection
                        .focus_key
                        .and_then(|k| state.row_ids.iter().position(|r| *r == k))
                        .unwrap_or(0);
                    let count = state.row_ids.len();
                    if current < count {
                        let row_id = state.row_ids[current];

                        if state.policy.navigation_enabled {
                            if let Some(ref nav) = state.navigation {
                                let host_rc = nav.host.clone();
                                let source = self.source.borrow();
                                if let Some(node_id) = source.navigation_node(row_id) {
                                    drop(source);
                                    drop(state);
                                    host_rc
                                        .borrow_mut()
                                        .apply(NavigationIntent::NavigateTo(node_id.into()));
                                    let mut state = self.state.borrow_mut();
                                    state
                                        .pending_events
                                        .push(NavigatorEvent::Navigated(node_id));
                                    return true;
                                }
                            }
                        }

                        state
                            .pending_events
                            .push(NavigatorEvent::ItemActivated(row_id));
                    }
                    return true;
                }

                let current = state
                    .selection
                    .focus_key
                    .and_then(|k| state.row_ids.iter().position(|r| *r == k))
                    .unwrap_or(0);

                // hjkl vim navigation
                let new_index = match te.text.as_str() {
                    "h" | "H" => current.saturating_sub(1),
                    "l" | "L" => (current + 1).min(count.saturating_sub(1)),
                    "k" | "K" => current.saturating_sub(cols),
                    "j" | "J" => (current + cols).min(count.saturating_sub(1)),
                    _ => return false,
                };
                let new_index = new_index.min(count.saturating_sub(1));

                if new_index != current {
                    navigate_to_index(&mut state, new_index, te.modifiers.shift);
                }
                true
            }

            _ => false,
        }
    }

    // Fix 1: hit_testable
    fn hit_testable(&self) -> bool {
        true
    }

    // Fix 3: needs_animation
    fn needs_animation(&self) -> bool {
        let state = self.state.borrow();
        state.drag.is_some() || state.shift_offsets.iter().any(|s| s.abs() > 0.01)
    }

    // Fix 4: wants_scroll
    fn wants_scroll(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "Navigator"
    }
}
