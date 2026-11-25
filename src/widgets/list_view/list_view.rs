// =============================================================================
// SPEC-HAL-187/188/189/192: ListView — Closed Composite with ScrollArea Body
// =============================================================================
//
// ListView is a closed composite with three regions:
// - Header: Painted directly by ListView, fixed at top
// - Body: ScrollArea child containing ListViewBody (handles scrolling)
// - Footer: Painted directly by ListView, fixed at bottom
//
// ScrollArea handles all scroll mechanics for the body:
// - Scroll state and smooth scrolling
// - Event coalescing, frame loop, scheduler, animation
// - Clipping
// - Scrollbar rendering
//
// This avoids reimplementing scroll infrastructure and follows SPEC-PM-002:
// scroll transform applied exactly once by ScrollArea.
// =============================================================================

use std::borrow::Cow;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{Event, EventPhase, KeyCode, KeyEvent, PointerButton, PointerEventKind};
use crate::core::icon::{IconId, IconSize};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{LAYER_OVERLAY, LAYER_SCROLLBAR, RoundedCorners, ZIndex};
use crate::core::scroll_container::HorizontalScrollLink;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::{ControlChromeCategory, ControlChromeState, ControlValidationState};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy};
use crate::utils::id::WidgetInternalId;
use crate::widgets::scroll_area::ScrollArea;
use crate::widgets::tabular_view::column::{ColumnId, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::{
    CellValue, DataSource, SortOperation, ValueState,
};
use crate::widgets::tabular_view::intent_policy::{
    IntentContext, IntentInput, TabularIntentPolicy,
};
use crate::widgets::tabular_view::intents::{ReorderIntent, TabularIntent};
use crate::widgets::tabular_view::row::RowId;
#[cfg(test)]
use crate::widgets::tabular_view::row::RowProjection;

use super::body::ListViewBody;
use super::cell_painter::CellPainter;
use super::column_config_overlay::{ColumnConfigAction, ColumnConfigOverlayPanel};
use super::core::{ListViewCore, ResizeState};
use super::footer::ListViewFooter;

use crate::widgets::tabular_view::column_config::{ColumnConfigurationState, ColumnMetaSnapshot};
use crate::widgets::tabular_view::view_config;

// =============================================================================
// ListView Widget
// =============================================================================

/// ListView: Closed composite with Header + Body (ScrollArea) + Footer.
///
/// ## Architecture
///
/// ```text
/// ListView (composite)
/// ├── Header region (painted directly, fixed)
/// ├── Body region (ScrollArea child)
/// │   └── ListViewBody (row content)
/// └── Footer region (painted directly, fixed)
/// ```
///
/// ScrollArea handles body scrolling. Header/footer are outside ScrollArea
/// so they don't scroll.
/// Separator hit zone half-width (±3px from separator center).
const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;
const DEFAULT_HEADER_HEIGHT: f32 = 24.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ResizeSeparatorHit {
    column_id: ColumnId,
}
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct ListViewColumnProbeEntry {
    pub column_id: ColumnId,
    pub display_index: usize,
    pub visible: bool,
    pub resizable: bool,
    pub reorderable: bool,
    pub scroll_domain: ColumnScrollDomain,
    pub preferred_width: f32,
    pub resolved_x_offset: Option<f32>,
    pub resolved_width: Option<f32>,
    pub min_width: f32,
    pub max_width: Option<f32>,
    pub separator_x_content: Option<f32>,
    pub separator_x_viewport: Option<f32>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListViewRealizedCellProbeEntry {
    pub row_id: RowId,
    pub column_id: ColumnId,
    pub rect: RectF,
    pub content_state: ValueState<CellValue>,
}

#[cfg(test)]
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct ListViewStructuralProbeEntry {
    pub widget_id: u64,
    pub root_rect: RectF,
    pub header_rect: RectF,
    pub body_rect: RectF,
    pub footer_rect: RectF,
    pub horizontal_scroll_offset: f32,
    pub vertical_scroll_offset: f32,
    pub resize_state: &'static str,
    pub active_resize_separator: Option<ColumnId>,
    pub hovered_separator: Option<ColumnId>,
    pub pointer_in_expansion_region: bool,
    pub selected_rows: Vec<u64>,
    pub focused_row: Option<u64>,
}

/// Column drag-reorder state machine.
/// States: Idle → Armed → Active → Idle
enum ColumnDragState {
    Idle,
    Armed {
        column_id: ColumnId,
        start_global_x: f32,
        reorderable: bool,
        additive_sort: bool,
        /// Snapshot of column order at arm time for revert.
        order_snapshot: Vec<ColumnId>,
    },
    Active {
        column_id: ColumnId,
        start_global_x: f32,
        /// Current pointer global X for ghost positioning.
        current_global_x: f32,
        /// Snapshot of column order at arm time for revert.
        order_snapshot: Vec<ColumnId>,
    },
}

struct HeaderCellVisualState {
    hover_t: f32,
    hover_anim: Option<TransitionHandle>,
    pressed_t: f32,
    pressed_anim: Option<TransitionHandle>,
}

impl Default for HeaderCellVisualState {
    fn default() -> Self {
        Self {
            hover_t: 0.0,
            hover_anim: None,
            pressed_t: 0.0,
            pressed_anim: None,
        }
    }
}

const DRAG_GHOST_SHADOW_LAYER: ZIndex = LAYER_OVERLAY;
const DRAG_GHOST_SURFACE_LAYER: ZIndex = LAYER_OVERLAY + 1;
const DRAG_GHOST_ROW_LAYER: ZIndex = LAYER_OVERLAY + 2;
const DRAG_GHOST_CHROME_LAYER: ZIndex = LAYER_OVERLAY + 3;
const DRAG_GHOST_TEXT_LAYER: ZIndex = LAYER_OVERLAY + 4;
const DRAG_GHOST_BORDER_LAYER: ZIndex = LAYER_OVERLAY + 5;
const HEADER_CHROME_LAYER: ZIndex = LAYER_SCROLLBAR + 1;
const HEADER_TEXT_LAYER: ZIndex = HEADER_CHROME_LAYER + 1;
const HEADER_SEPARATOR_LAYER: ZIndex = HEADER_TEXT_LAYER + 1;
const HEADER_INTERACTION_LAYER: ZIndex = HEADER_SEPARATOR_LAYER + 1;

pub struct ListView<S: DataSource> {
    /// Widget internal ID.
    id: WidgetInternalId,

    /// Shared core state (substrate, adapter, column_titles).
    /// Shared with ListViewBody via Rc.
    core: Rc<RefCell<ListViewCore<S>>>,

    /// Header region height.
    header_height: f32,

    /// Footer region height (default 0, structural no-op).
    /// Set via `set_footer_height()` to show footer with column separators.
    footer_height: f32,

    /// Cached body rect for layout.
    body_rect: RectF,

    /// Whether body widget has been taken (for tree construction).
    body_taken: bool,

    /// Whether footer widget has been taken (for tree construction).
    footer_taken: bool,

    /// Cached font_size from theme (for auto-size text measurement).
    font_size: f32,

    /// Cached text_padding from theme (for auto-size calculation).
    text_padding: f32,

    // =========================================================================
    // Column Resize State Machine
    //
    // States: Idle → Armed → Dragging → Idle
    //
    // INV-RESIZE-OWNERSHIP: Only ListView mutates resize state or column widths.
    // INV-RESIZE-CAPTURE: Pointer capture exists only in Dragging state.
    // INV-RESIZE-EXCLUSIVE: While resize ≠ Idle, all row interaction is suppressed.
    // INV-RESIZE-DETERMINISTIC: Width deltas computed solely from pointer movement.
    // INV-RESIZE-CLEAN-EXIT: On finish or cancel, state is fully reset.
    // =========================================================================
    /// Current resize state (Idle, Armed, or Dragging).
    resize_state: ResizeState,

    /// Column drag-reorder state (Idle, Armed, or Active).
    drag_state: ColumnDragState,

    /// A value refresh changed the active lifted column's visible presentation.
    pending_drag_value_paint: bool,

    /// Header cell currently under the pointer.
    header_hovered_column: Option<ColumnId>,

    /// Per-column transient header interaction animation state.
    header_cell_visuals: RefCell<HashMap<ColumnId, HeaderCellVisualState>>,

    /// Per-column drag slide animation handles (animation-system-driven).
    /// Started via cx.start_animation() during PointerMove, read via cx.get_animation() in paint.
    drag_slide_handles: RefCell<HashMap<ColumnId, TransitionHandle>>,

    /// Per-column drag slide cached values (updated from animation system in paint).
    /// Used as `from` value when retargeting animations in event handlers.
    drag_slide_values: RefCell<HashMap<ColumnId, f32>>,

    /// Per-column drag slide target positions (for retarget detection).
    /// Only mutated in event handlers (where &mut self is available).
    drag_slide_targets: HashMap<ColumnId, f32>,

    /// Column configuration overlay panel.
    config_overlay: ColumnConfigOverlayPanel,

    /// Cached column metadata snapshot for overlay display.
    /// Stored on ListView (not core) to avoid RefCell borrow conflicts during paint/event.
    config_snapshot: Vec<ColumnMetaSnapshot>,

    /// SPEC-HAL-176: Whether the widget is in keyboard Capture mode.
    /// Set by on_capture_enter/on_capture_leave.
    in_capture: bool,
}

impl<S: DataSource> ListView<S> {
    /// Create a new ListView with a data source.
    ///
    /// After creation, configure columns via `add_column()`, then call
    /// `take_body_scroll_area()` to get the ScrollArea for the body region.
    /// Add that as a child of ListView in the widget tree.
    pub fn new(source: S) -> Self {
        let core = Rc::new(RefCell::new(ListViewCore::new(source)));

        Self {
            id: WidgetInternalId::new(),
            core,
            header_height: DEFAULT_HEADER_HEIGHT,
            footer_height: 0.0,
            body_rect: RectF::zero(),
            body_taken: false,
            footer_taken: false,
            font_size: 14.0,   // Default; updated from theme in layout
            text_padding: 6.0, // Default; updated from theme in layout
            resize_state: ResizeState::Idle,
            drag_state: ColumnDragState::Idle,
            pending_drag_value_paint: false,
            header_hovered_column: None,
            header_cell_visuals: RefCell::new(HashMap::new()),
            drag_slide_handles: RefCell::new(HashMap::new()),
            drag_slide_values: RefCell::new(HashMap::new()),
            drag_slide_targets: HashMap::new(),
            config_overlay: ColumnConfigOverlayPanel::new(),
            config_snapshot: Vec::new(),
            in_capture: false,
        }
    }

    /// Add a column with title and width. Returns the ColumnId.
    pub fn add_column(&mut self, title: &str, width: f32) -> ColumnId {
        self.core.borrow_mut().add_column(title, width)
    }

    /// Add a pre-built Column with title. Returns the ColumnId.
    pub fn add_column_raw(
        &mut self,
        title: &str,
        column: crate::widgets::tabular_view::column::Column,
    ) -> ColumnId {
        self.core.borrow_mut().add_column_raw(title, column)
    }

    /// Mount inline ComboBox editors for string values in the given column.
    pub fn set_column_combo_options(&mut self, id: ColumnId, options: Vec<(String, String)>) {
        self.core.borrow_mut().set_column_combo_options(id, options);
    }

    fn cell_uses_combo_box_drag_proxy(
        core: &ListViewCore<S>,
        column_id: ColumnId,
        cell_value: &ValueState<CellValue>,
    ) -> bool {
        core.combo_box_options.contains_key(&column_id)
            && matches!(cell_value, ValueState::Ready(CellValue::String(_)))
    }

    fn drag_cell_presentation<'a>(
        core: &ListViewCore<S>,
        column_id: ColumnId,
        cell_value: &'a ValueState<CellValue>,
    ) -> (bool, Cow<'a, str>) {
        let use_combo_box_proxy = Self::cell_uses_combo_box_drag_proxy(core, column_id, cell_value);
        let text = match cell_value {
            ValueState::Ready(CellValue::String(text)) => Cow::Borrowed(text.as_str()),
            ValueState::Ready(CellValue::Int(n)) => Cow::Owned(n.to_string()),
            ValueState::Ready(CellValue::Float(f)) => Cow::Owned(format!("{f:.2}")),
            ValueState::Ready(CellValue::Bool(b)) => Cow::Owned(b.to_string()),
            ValueState::Ready(CellValue::Null) => Cow::Borrowed("\u{2014}"),
            ValueState::Pending => Cow::Borrowed("..."),
            ValueState::Missing => Cow::Borrowed(""),
            ValueState::Denied { .. } => Cow::Borrowed("[denied]"),
            ValueState::Error { .. } => Cow::Borrowed("[error]"),
        };
        (use_combo_box_proxy, text)
    }

    #[cfg(test)]
    pub(crate) fn test_cell_uses_combo_box_drag_proxy(
        &self,
        column_id: ColumnId,
        cell_value: &ValueState<CellValue>,
    ) -> bool {
        let core = self.core.borrow();
        Self::cell_uses_combo_box_drag_proxy(&core, column_id, cell_value)
    }

    /// Set column visibility. Returns true if changed.
    pub fn set_column_visible(&mut self, id: ColumnId, visible: bool) -> bool {
        self.core.borrow_mut().set_column_visible(id, visible)
    }

    /// Toggle column visibility. Returns true if changed.
    pub fn toggle_column_visible(&mut self, id: ColumnId) -> bool {
        self.core.borrow_mut().toggle_column_visible(id)
    }

    /// Query column visibility.
    pub fn is_column_visible(&self, id: ColumnId) -> bool {
        self.core.borrow().is_column_visible(id)
    }

    /// Get mutable access to the data adapter.
    pub fn adapter_mut(
        &mut self,
    ) -> std::cell::RefMut<'_, crate::widgets::tabular_view::data_adapter::DataAdapter<S>> {
        std::cell::RefMut::map(self.core.borrow_mut(), |c| &mut c.adapter)
    }

    /// Provider-confirmed applied sorting testimony.
    pub fn applied_sort(
        &self,
    ) -> Option<crate::widgets::tabular_view::data_adapter::AppliedSortSnapshot> {
        self.core.borrow().applied_sort()
    }

    /// Submit a host-authored semantic sort operation through the adapter.
    pub fn request_sort(
        &mut self,
        operation: SortOperation,
    ) -> crate::widgets::tabular_view::data_adapter::SortRequestOutcome {
        self.core.borrow_mut().request_sort(operation)
    }

    /// Request the provider baseline; no startup RowId vector is restored.
    pub fn clear_sort(
        &mut self,
    ) -> Option<crate::widgets::tabular_view::data_adapter::SortRequestOutcome> {
        let baseline = self.applied_sort()?.spec.baseline;
        Some(
            self.core
                .borrow_mut()
                .request_sort(SortOperation::ClearToBaseline(baseline)),
        )
    }

    /// Remove the durable requested preference and request provider baseline.
    pub fn reset_sort_preference(
        &mut self,
    ) -> Option<crate::widgets::tabular_view::data_adapter::SortRequestOutcome> {
        self.core.borrow_mut().reset_sort_preference()
    }

    /// Refresh realized cell values after the data source reports cell invalidation.
    ///
    /// This keeps high-frequency value changes on the paint path when row order,
    /// column geometry, and row heights are unchanged.
    pub fn refresh_visible_cells(&mut self) {
        let mut core = self.core.borrow_mut();
        core.sync_data_source();
        let lifted_column = {
            let hts = core.cms.hts();
            let Some(layout) = hts.try_column_layout() else {
                return;
            };
            match self.drag_state {
                ColumnDragState::Active { column_id, .. }
                    if !self.pending_drag_value_paint
                        && self.header_visible()
                        && self.body_rect.height > 0.0
                        && layout
                            .header_slots()
                            .any(|slot| slot.column_id == column_id) =>
                {
                    Some(column_id)
                }
                _ => None,
            }
        };
        let viewport_height = core.viewport_height;
        let visible = core.visible_rows_for_viewport(viewport_height);
        let previous_lifted_values = lifted_column.map(|column_id| {
            // The realized window can include overscan. Only the lifted cells
            // overlapping the body viewport contribute visible paint.
            let scroll_y = core.cms.hts().scroll_offset() as f32;
            let viewport_bottom = scroll_y + self.body_rect.height;
            let values: Vec<_> = core
                .project_visible_rows(&visible)
                .into_iter()
                .filter(|row| {
                    row.y_offset + row.height > scroll_y && row.y_offset < viewport_bottom
                })
                .map(|row| {
                    let row_id = row.row_id.raw();
                    (row_id, core.realized_cell_value_state(row_id, column_id))
                })
                .collect();
            (column_id, values)
        });
        core.realize_visible_cells(&visible);
        if let Some((column_id, values)) = previous_lifted_values {
            self.pending_drag_value_paint = values.into_iter().any(|(row_id, previous)| {
                let current = core.realized_cell_value_state(row_id, column_id);
                Self::drag_cell_presentation(&core, column_id, &previous)
                    != Self::drag_cell_presentation(&core, column_id, &current)
            });
        }
    }

    /// Replace the cell painter.
    ///
    /// The default is `DefaultCellPainter` (text rendering). Consumers may install
    /// a custom painter that dispatches on `ColumnId` to render badges, icons,
    /// colored indicators, or any other non-text representation for specific columns.
    ///
    /// A custom painter must still handle all columns — columns without special
    /// handling should fall through to text rendering (typically by delegating to
    /// `DefaultCellPainter` for those columns).
    pub fn set_cell_painter(&mut self, painter: Box<dyn CellPainter>) {
        let mut core = self.core.borrow_mut();
        core.cell_painter = painter;
        core.custom_cell_painter = true;
    }

    /// Configure ListView's single declarative intent policy before interaction.
    ///
    /// The policy's selection mode and interaction model are synchronized into
    /// the existing ordered-selection authority. No shadow policy is created.
    /// Returns `false` if a different policy is requested after the first
    /// routed row interaction locks selection configuration.
    pub fn set_intent_policy(&mut self, policy: TabularIntentPolicy) -> bool {
        self.core.borrow_mut().set_intent_policy(policy)
    }

    /// Read the configured declarative intent policy.
    pub fn intent_policy(&self) -> TabularIntentPolicy {
        self.core.borrow().intent_policy
    }

    /// Read the current identity-authoritative row-axis focus.
    pub fn focused_row(&self) -> Option<RowId> {
        self.core.borrow().focused_row()
    }

    /// Install a row-axis focus callback.
    ///
    /// The callback receives the committed `RowId` focus after pointer or
    /// keyboard navigation changes it. It runs after all internal `RefCell`
    /// borrows are released. This callback observes focus only; it does not
    /// imply selection or activation. Consumers may enqueue the identity into
    /// a pending application-action buffer; they must not mutate WidgetTree,
    /// other widgets, layout, navigation, or Halcyon AppState directly here.
    pub fn set_focused_row_callback<F>(&mut self, callback: F)
    where
        F: Fn(Option<RowId>) + 'static,
    {
        self.core.borrow_mut().focused_row_callback = Some(Rc::new(callback));
    }

    /// Install an activation callback.
    ///
    /// Invoked when an `ActivationIntent::Activate` is produced by the
    /// IntentPolicy — typically in response to Enter keyboard. The callback
    /// runs AFTER all internal borrows are released, so it may re-enter
    /// ListView/substrate state safely.
    ///
    /// SPEC-HAL-312A: Primary use is EventLog expansion toggle.
    pub fn set_activation_callback<F: Fn(RowId) + 'static>(&mut self, callback: F) {
        self.core.borrow_mut().activation_callback = Some(std::rc::Rc::new(callback));
    }

    /// SPEC-HAL-313 §3.5: Set the widget factory for expanded rows.
    ///
    /// The factory is called once per row when expanded. The produced widget
    /// lives until the row is collapsed. The factory is transferred to the
    /// body's ExpandableRowHost on the first layout pass.
    ///
    /// INV-313-FACTORY-FN-1: Factory must be Fn (not FnMut).
    /// INV-313-FACTORY-PURE-1: Called exactly once per Expand per row.
    pub fn set_row_widget_factory(&mut self, f: impl Fn(RowId) -> Box<dyn Widget> + 'static) {
        self.core.borrow_mut().row_widget_factory = Some(Box::new(f));
    }

    /// SPEC-HAL-313 §3.5: Set row expansion policy.
    ///
    /// Configures whether expansion is available and how many rows can be
    /// expanded simultaneously. Also sets `intent_policy.expansion_enabled`
    /// so that keybinds (Enter, Space, h, l, i) and capture mode activate.
    pub fn set_expansion_policy(&mut self, policy: ExpansionPolicy) {
        let mut core = self.core.borrow_mut();
        let enabled = !matches!(policy, ExpansionPolicy::Disabled);
        core.cms.hts_mut().set_expansion_policy(policy);
        core.intent_policy.expansion_enabled = enabled;
    }

    /// SPEC-HAL-313 §3.5: Set expanded height policy.
    ///
    /// Configures how the expanded region height is determined:
    /// - `Fixed(h)`: always `h` pixels
    /// - `Cap(cap)`: `min(child.natural_height, cap)`, ScrollArea wraps overflow
    /// - `Natural`: child's natural height, no bound
    pub fn set_expanded_height_policy(&mut self, policy: ExpandedHeightPolicy) {
        self.core
            .borrow_mut()
            .cms
            .hts_mut()
            .set_expanded_height_policy(policy);
    }

    /// Take the body ScrollArea widget for tree construction.
    ///
    /// Call this once after configuring columns, then add the returned
    /// ScrollArea as a child of ListView in the widget tree.
    ///
    /// # Panics
    /// Panics if called more than once.
    pub fn take_body_scroll_area(&mut self) -> ScrollArea {
        if self.body_taken {
            panic!("ListView::take_body_scroll_area() called more than once");
        }
        self.body_taken = true;

        // Create ScrollArea (unwired - will be wired when added to tree).
        // The body widget will be added as ScrollArea's content child.
        ScrollArea::new_unwired()
    }

    /// Create the body content widget (ListViewBody).
    ///
    /// This is separate from ScrollArea creation so the tree can wire them.
    pub fn create_body_widget(&self) -> ListViewBody<S> {
        ListViewBody::new(self.core.clone())
    }

    /// Create the footer widget (ListViewFooter).
    ///
    /// Footer is a structural completeness widget for resize. Default height is 0.
    /// Add as second child of ListView (after ScrollArea).
    ///
    /// ## Child Order
    ///
    /// - children[0]: ScrollArea (body, required)
    /// - children[1]: ListViewFooter (required for resize completeness)
    ///
    /// # Panics
    /// Panics if called more than once.
    pub fn create_footer_widget(&mut self) -> ListViewFooter<S> {
        if self.footer_taken {
            panic!("ListView::create_footer_widget() called more than once");
        }
        self.footer_taken = true;
        ListViewFooter::new(self.core.clone())
    }

    /// Set footer height.
    ///
    /// Default is 0 (structural no-op, footer invisible).
    /// Set > 0 to show footer region with column separators.
    pub fn set_footer_height(&mut self, height: f32) {
        self.footer_height = height;
    }

    /// Show or omit the owner-rendered header band.
    ///
    /// Configure this before mounting the ListView. A hidden header resolves to
    /// zero height; the body remains inside the owning composite and begins at
    /// the ListView's top edge.
    pub fn set_header_visible(&mut self, visible: bool) {
        self.header_height = if visible { DEFAULT_HEADER_HEIGHT } else { 0.0 };
        if !visible {
            self.core.borrow_mut().config_overlay_open = false;
        }
    }

    /// Read whether the owner-rendered header band is present.
    pub fn header_visible(&self) -> bool {
        self.header_height > 0.0
    }

    /// Set view_id for persistence (builder-style).
    pub fn with_view_id(self, view_id: &str) -> Self {
        self.core.borrow_mut().view_id = Some(view_id.to_string());
        self
    }

    /// Set app namespace for persistence (builder-style).
    pub fn with_app_namespace(self, ns: &str) -> Self {
        self.core.borrow_mut().app_namespace = Some(ns.to_string());
        self
    }

    /// Set persistence store (builder-style).
    pub fn with_config_store(self, store: Box<dyn view_config::ViewConfigStore>) -> Self {
        self.core.borrow_mut().config_store = Some(store);
        self
    }

    /// Set a custom cell painter for rendering cell contents.
    /// Default: text rendering (DefaultCellPainter).
    pub fn with_cell_painter(self, painter: Box<dyn super::cell_painter::CellPainter>) -> Self {
        let mut core = self.core.borrow_mut();
        core.cell_painter = painter;
        core.custom_cell_painter = true;
        drop(core);
        self
    }

    /// Finalize configuration: capture default state and load persisted config.
    ///
    /// Call after all columns are added and before first paint.
    pub fn finalize_config(&mut self) {
        let mut core = self.core.borrow_mut();

        // Capture baseline.
        let default_config = ColumnConfigurationState::default_from_model(&*core.column_model());
        core.default_column_config = Some(default_config);

        // Load persisted config if available.
        // Load raw JSON first, then apply (avoids borrow conflict between store and substrate).
        if let Some(key) = core.view_config_key() {
            let raw_json = core.config_store.as_ref().and_then(|s| s.load(&key));
            if let Some(raw) = raw_json {
                // Guard discipline (SPEC-HAL-337 INV-337-A1): bind the parse
                // result in its own statement so the column-model guard drops
                // here — an if-let scrutinee's temporaries live through the
                // block and would hold `core` immutably across the mutable
                // call below (E0502).
                let config = view_config::column_config_from_raw_json(
                    &key,
                    raw.clone(),
                    &*core.column_model(),
                );
                if let Some(config) = config {
                    core.apply_column_configuration(&config);
                }
                if let Some(spec) = view_config::sort_preference_from_raw_json(&key, &raw) {
                    core.request_sort(SortOperation::ApplySpec(spec));
                }
            }
        }

        // Build initial snapshot.
        drop(core);
        self.config_snapshot = self.core.borrow().build_column_snapshot();
    }

    /// Wire horizontal scroll link and freeze flag for horizontal scrolling.
    ///
    /// INV-PAINT-HSCROLL-1: Header, body, footer use identical h-offset.
    /// INV-RESIZE-HSCROLL-2: While dragging, scroll_x is frozen.
    pub fn set_horizontal_scroll_link(
        &mut self,
        link: HorizontalScrollLink,
        freeze: Rc<Cell<bool>>,
    ) {
        let mut core = self.core.borrow_mut();
        core.horizontal_scroll_link = Some(link);
        core.horizontal_scroll_freeze = Some(freeze);
    }

    /// Get total row count.
    pub fn row_count(&self) -> usize {
        self.core.borrow().row_count()
    }

    /// Returns the unconstrained height required to show every row without
    /// vertical scrolling, including the configured header and footer bands.
    ///
    /// This is an owner-authored geometry query only; ListView still fills the
    /// height allocated by its parent during layout. After data-source
    /// invalidation, call [`Self::refresh_visible_cells`] before reading the
    /// updated height.
    pub fn natural_height(&self) -> f32 {
        self.header_height + self.core.borrow().content_height() + self.footer_height
    }

    #[cfg(test)]
    pub(crate) fn test_core_weak(&self) -> std::rc::Weak<RefCell<ListViewCore<S>>> {
        Rc::downgrade(&self.core)
    }

    #[cfg(test)]
    pub(crate) fn test_column_width(&self, column_id: ColumnId) -> Option<f32> {
        self.get_column_width(column_id)
    }

    #[cfg(test)]
    pub(crate) fn test_column_preferred_width(&self, column_id: ColumnId) -> Option<f32> {
        self.core
            .borrow()
            .column_model()
            .get(column_id)
            .map(|column| column.preferred_width)
    }

    #[cfg(test)]
    pub(crate) fn test_commit_column_resize(
        &mut self,
        column_id: ColumnId,
        width: f32,
    ) -> Option<f32> {
        let applied = self.apply_column_width(column_id, width);
        if applied.is_some() {
            self.commit_resize();
        }
        applied
    }

    #[cfg(test)]
    pub(crate) fn test_column_probe_entries(&self) -> Vec<ListViewColumnProbeEntry> {
        let core = self.core.borrow();
        let columns: Vec<_> = core.column_model().columns().iter().cloned().collect();
        let slots: Vec<_> = core.cms.hts().column_layout().body_slots().collect();
        let horizontal_scroll = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |link| link.get());

        columns
            .into_iter()
            .enumerate()
            .map(|(display_index, column)| {
                let slot = slots
                    .iter()
                    .find(|slot| slot.column_id == column.id)
                    .copied();
                let separator_x_content = slot.map(|slot| slot.x + slot.width - 1.0);
                let separator_x_viewport = slot.map(|slot| {
                    if slot.scroll_domain == ColumnScrollDomain::Scrollable {
                        slot.x + slot.width - horizontal_scroll - 1.0
                    } else {
                        slot.x + slot.width - 1.0
                    }
                });

                ListViewColumnProbeEntry {
                    column_id: column.id,
                    display_index,
                    visible: column.visible,
                    resizable: column.resizable,
                    reorderable: column.reorderable,
                    scroll_domain: column.scroll_domain,
                    preferred_width: column.preferred_width,
                    resolved_x_offset: slot.map(|slot| slot.x),
                    resolved_width: slot.map(|slot| slot.width),
                    min_width: column.min_width,
                    max_width: column.max_width.is_finite().then_some(column.max_width),
                    separator_x_content,
                    separator_x_viewport,
                }
            })
            .collect()
    }

    #[cfg(test)]
    pub(crate) fn test_realized_row_projections(&self) -> Vec<RowProjection> {
        self.core.borrow().visible_row_projections()
    }

    #[cfg(test)]
    pub(crate) fn test_realized_cell_probe_entries(&self) -> Vec<ListViewRealizedCellProbeEntry> {
        let core = self.core.borrow();
        let rows = core.visible_row_projections();
        let slots: Vec<_> = core.cms.hts().column_layout().body_slots().collect();
        let mut entries = Vec::new();

        for row in rows {
            for slot in &slots {
                entries.push(ListViewRealizedCellProbeEntry {
                    row_id: row.row_id,
                    column_id: slot.column_id,
                    rect: RectF::new(slot.x, row.y_offset, slot.width, row.height),
                    content_state: core.realized_cell_value_state(row.row_id.raw(), slot.column_id),
                });
            }
        }

        entries
    }

    #[cfg(test)]
    pub(crate) fn test_structural_probe_entry(&self) -> ListViewStructuralProbeEntry {
        let core = self.core.borrow();
        let horizontal_scroll_offset = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |link| link.get());
        let vertical_scroll_offset = core.cms.hts().scroll_offset() as f32;
        let root_rect = RectF::new(
            0.0,
            0.0,
            self.body_rect.width,
            self.header_height + self.body_rect.height + self.footer_height,
        );
        let header_rect = RectF::new(0.0, 0.0, self.body_rect.width, self.header_height);
        let footer_rect = RectF::new(
            0.0,
            self.header_height + self.body_rect.height,
            self.body_rect.width,
            self.footer_height,
        );
        let mut selected_rows: Vec<u64> = core
            .selection
            .selected_keys
            .iter()
            .map(|row_id| row_id.raw())
            .collect();
        selected_rows.sort_unstable();

        ListViewStructuralProbeEntry {
            widget_id: self.id.0,
            root_rect,
            header_rect,
            body_rect: self.body_rect,
            footer_rect,
            horizontal_scroll_offset,
            vertical_scroll_offset,
            resize_state: match self.resize_state {
                ResizeState::Idle => "idle",
                ResizeState::Armed { .. } => "armed",
                ResizeState::Dragging { .. } => "dragging",
            },
            active_resize_separator: core.active_resize_separator,
            hovered_separator: core.hovered_separator,
            pointer_in_expansion_region: false,
            selected_rows,
            focused_row: core.focused_row().map(|row_id| row_id.raw()),
        }
    }

    // =========================================================================
    // Keyboard Handling
    // =========================================================================

    fn invalidate_body_paint(&self, cx: &mut EventContext) {
        if let Some(body_id) = self.core.borrow().body_widget_id {
            cx.invalidate_paint_by_id(body_id);
        }
    }

    fn handle_keyboard(&mut self, cx: &mut EventContext, event: &KeyEvent) -> bool {
        if !event.is_down {
            return false;
        }

        // =====================================================================
        // INV-RESIZE-CLEAN-EXIT: Escape cancels resize, rollback to initial width.
        // =====================================================================
        if event.key_code == KeyCode::Escape {
            if self.is_resize_active() {
                if matches!(self.resize_state, ResizeState::Dragging { .. }) {
                    cx.release_pointer();
                }
                self.cancel_resize();
                return true;
            }
            if !matches!(self.drag_state, ColumnDragState::Idle) {
                cx.release_pointer();
                self.cancel_drag_reorder(cx);
                return true;
            }
            // No pending resize/drag — clear selection if non-empty.
            // If selection is empty, return false to let the framework handle
            // Escape (focus reset, scope exit, etc.).
            let has_selection = !self.core.borrow().selection.is_selection_empty();
            if has_selection {
                {
                    let mut core = self.core.borrow_mut();
                    core.dispatch_intent(&TabularIntent::Selection(
                        crate::widgets::tabular_view::intents::SelectionIntent::ClearSelection,
                    ));
                }
                self.invalidate_body_paint(cx);
                return true;
            }
            return false;
        }

        // =====================================================================
        // SPEC-HAL-200: Navigation keys — pre-resolve target index, then
        // interpret through policy for selection semantics.
        // =====================================================================
        let target_index = {
            let core = self.core.borrow();
            match event.key_code {
                KeyCode::Up => Some(core.compute_new_focus(-1)),
                KeyCode::Down => Some(core.compute_new_focus(1)),
                KeyCode::Home => Some(0usize),
                KeyCode::End => Some(core.row_count().saturating_sub(1)),
                KeyCode::PageUp => {
                    let page = core.page_size();
                    let current = core
                        .focused_index()
                        .unwrap_or(core.row_count().saturating_sub(1));
                    Some(current.saturating_sub(page))
                }
                KeyCode::PageDown => {
                    let page = core.page_size();
                    let current = core.focused_index().unwrap_or(0);
                    let last = core.row_count().saturating_sub(1);
                    Some((current + page).min(last))
                }
                // SPEC-HAL-313: j/k = intra-list row navigation.
                // Only reachable in Capture mode (in Navigation mode,
                // dispatch_engine_action consumes j/k as FOCUS_DOWN/FOCUS_UP
                // before the widget sees them).
                KeyCode::Character('j') => Some(core.compute_new_focus(1)),
                KeyCode::Character('k') => Some(core.compute_new_focus(-1)),
                _ => None,
            }
        };

        if let Some(target_index) = target_index {
            // Extract policy (Copy) and build context with pre-resolved target.
            let (policy, target_row_id, focused) = {
                let core = self.core.borrow();
                let policy = core.intent_policy;
                let target_row_id = core.index_to_row_id(target_index);
                let focused = core.focused;
                (policy, target_row_id, focused)
            };

            if let Some(target_row_id) = target_row_id {
                // Interpret key through policy with pre-resolved target as focused_row.
                let context = IntentContext {
                    focused,
                    has_anchor: false,
                    is_editing: false,
                    focused_row: Some(target_row_id),
                    focused_row_expanded: false,
                };
                let input = IntentInput::KeyPress {
                    key: event.key_code,
                    modifiers: event.modifiers,
                };
                let intents = policy.interpret(&input, &context);

                let callbacks = self
                    .core
                    .borrow_mut()
                    .dispatch_intents_and_focus(&intents, Some(target_row_id));
                if callbacks.presentation_changed() {
                    self.invalidate_body_paint(cx);
                }
                callbacks.invoke();
            } else {
                // No valid target (empty list). set_focus is a no-op but preserves behavior.
                self.core.borrow_mut().set_focus(target_index);
            }

            return true;
        }

        // =====================================================================
        // SPEC-HAL-313: Non-navigation keys (Enter, Space, h, l, i, etc.)
        //
        // These use the CURRENT focused row (no pre-resolution). IntentPolicy
        // decides what they mean based on expansion_enabled and context.
        //
        // FORBID-313-DIRECT-KEYCODE-1: No direct KeyCode matching here.
        // All interpretation goes through IntentPolicy::interpret.
        //
        // INV-313-FALLTHROUGH-1: If IntentPolicy returns empty Vec, the widget
        // returns false, allowing the Action System to dispatch system-layer
        // bindings (e.g., h → FOCUS_LEFT, i → ENTER_WIDGET).
        // =====================================================================
        let (policy, focused_row, widget_focused, focused_row_expanded) = {
            let core = self.core.borrow();
            let focused_row = core.focused_row();
            let focused_row_expanded = focused_row
                .map(|r| core.cms.hts().expansion().is_expanded(r))
                .unwrap_or(false);
            (
                core.intent_policy,
                focused_row,
                core.focused,
                focused_row_expanded,
            )
        };

        let context = IntentContext {
            focused: widget_focused,
            has_anchor: false,
            is_editing: false,
            focused_row,
            focused_row_expanded,
        };
        let input = IntentInput::KeyPress {
            key: event.key_code,
            modifiers: event.modifiers,
        };
        let intents = policy.interpret(&input, &context);

        // INV-313-FALLTHROUGH-1: Empty intents → not consumed → Action System fallback.
        if intents.is_empty() {
            return false;
        }

        let callbacks = self
            .core
            .borrow_mut()
            .dispatch_intents_and_focus(&intents, None);
        if callbacks.presentation_changed() {
            self.invalidate_body_paint(cx);
        }
        callbacks.invoke();

        // Post-dispatch: check for Activation and Expansion intents.
        let mut needs_invalidation = false;

        if intents
            .iter()
            .any(|intent| matches!(intent, TabularIntent::Activation(_)))
        {
            needs_invalidation = true;
        }

        // Expansion intents require subtree invalidation for body re-layout.
        for intent in &intents {
            if matches!(intent, TabularIntent::Expansion(_)) {
                needs_invalidation = true;
                break;
            }
        }

        if needs_invalidation {
            cx.invalidate_subtree();
        }

        true
    }

    // =========================================================================
    // Separator Hit Testing
    // =========================================================================

    /// Hit-test for separator hover. Returns ColumnId if pointer is over a separator.
    ///
    /// INV-HOVER-NO-CAPTURE: This is detection only, no capture.
    /// INV-HOVER-NO-DRAG: This does not arm any drag state.
    // =========================================================================
    // Column Configuration Action Dispatch
    // =========================================================================

    fn apply_config_action(&mut self, action: ColumnConfigAction, cx: &mut EventContext) {
        match action {
            ColumnConfigAction::ToggleVisibility(id) => {
                let mut core = self.core.borrow_mut();
                // Guard: don't hide last visible column.
                let visible_count = core.column_model().visible_columns().count();
                let currently_visible = core.is_column_visible(id);
                if currently_visible && visible_count <= 1 {
                    return;
                }
                core.set_column_visible(id, !currently_visible);
                // Save if persistence enabled.
                if let Some(key) = core.view_config_key() {
                    if let Some(ref store) = core.config_store {
                        view_config::save_column_config(
                            &*core.column_model(),
                            &key,
                            store.as_ref(),
                        );
                    }
                }
                if let Some(body_id) = core.body_widget_id {
                    drop(core);
                    cx.invalidate_layout_by_id(body_id);
                }
            }
            ColumnConfigAction::MoveUp(id) => {
                let mut core = self.core.borrow_mut();
                let ids: Vec<ColumnId> =
                    core.column_model().columns().iter().map(|c| c.id).collect();
                if let Some(pos) = ids.iter().position(|&cid| cid == id) {
                    if pos > 0 {
                        let mut new_order = ids;
                        new_order.swap(pos, pos - 1);
                        core.reorder_columns(&new_order);
                        if let Some(key) = core.view_config_key() {
                            if let Some(ref store) = core.config_store {
                                view_config::save_column_config(
                                    &*core.column_model(),
                                    &key,
                                    store.as_ref(),
                                );
                            }
                        }
                        if let Some(body_id) = core.body_widget_id {
                            drop(core);
                            cx.invalidate_layout_by_id(body_id);
                        }
                    }
                }
            }
            ColumnConfigAction::MoveDown(id) => {
                let mut core = self.core.borrow_mut();
                let ids: Vec<ColumnId> =
                    core.column_model().columns().iter().map(|c| c.id).collect();
                if let Some(pos) = ids.iter().position(|&cid| cid == id) {
                    if pos < ids.len() - 1 {
                        let mut new_order = ids;
                        new_order.swap(pos, pos + 1);
                        core.reorder_columns(&new_order);
                        if let Some(key) = core.view_config_key() {
                            if let Some(ref store) = core.config_store {
                                view_config::save_column_config(
                                    &*core.column_model(),
                                    &key,
                                    store.as_ref(),
                                );
                            }
                        }
                        if let Some(body_id) = core.body_widget_id {
                            drop(core);
                            cx.invalidate_layout_by_id(body_id);
                        }
                    }
                }
            }
            ColumnConfigAction::PinLeft(id) => {
                let mut core = self.core.borrow_mut();
                core.set_column_scroll_domain(id, ColumnScrollDomain::PinnedLeft);
                if let Some(key) = core.view_config_key() {
                    if let Some(ref store) = core.config_store {
                        view_config::save_column_config(
                            &*core.column_model(),
                            &key,
                            store.as_ref(),
                        );
                    }
                }
                if let Some(body_id) = core.body_widget_id {
                    drop(core);
                    cx.invalidate_layout_by_id(body_id);
                }
            }
            ColumnConfigAction::PinRight(id) => {
                let mut core = self.core.borrow_mut();
                core.set_column_scroll_domain(id, ColumnScrollDomain::PinnedRight);
                if let Some(key) = core.view_config_key() {
                    if let Some(ref store) = core.config_store {
                        view_config::save_column_config(
                            &*core.column_model(),
                            &key,
                            store.as_ref(),
                        );
                    }
                }
                if let Some(body_id) = core.body_widget_id {
                    drop(core);
                    cx.invalidate_layout_by_id(body_id);
                }
            }
            ColumnConfigAction::Unpin(id) => {
                let mut core = self.core.borrow_mut();
                core.set_column_scroll_domain(id, ColumnScrollDomain::Scrollable);
                if let Some(key) = core.view_config_key() {
                    if let Some(ref store) = core.config_store {
                        view_config::save_column_config(
                            &*core.column_model(),
                            &key,
                            store.as_ref(),
                        );
                    }
                }
                if let Some(body_id) = core.body_widget_id {
                    drop(core);
                    cx.invalidate_layout_by_id(body_id);
                }
            }
            ColumnConfigAction::ResetAll => {
                let mut core = self.core.borrow_mut();
                let baseline = core.applied_sort().map(|snapshot| snapshot.spec.baseline);
                if let Some(default_config) = core.default_column_config.clone() {
                    core.apply_column_configuration(&default_config);
                }
                if let Some(key) = core.view_config_key() {
                    if let Some(ref store) = core.config_store {
                        store.clear(&key);
                    }
                }
                if let Some(baseline) = baseline {
                    core.adapter
                        .request_sort(SortOperation::ClearToBaseline(baseline));
                    core.sync_data_source();
                }
                if let Some(body_id) = core.body_widget_id {
                    drop(core);
                    cx.invalidate_layout_by_id(body_id);
                }
            }
            ColumnConfigAction::Close => {
                self.core.borrow_mut().config_overlay_open = false;
            }
        }

        // Rebuild snapshot after any action.
        self.config_snapshot = self.core.borrow().build_column_snapshot();
    }

    fn hit_test_separator(&self, local_x: f32) -> Option<ResizeSeparatorHit> {
        // No Y restriction: separator is active along full column height.
        // Separators visually extend through header and body rows.

        let core = self.core.borrow();

        // INV-RESIZE-HSCROLL-3: Convert viewport-local to content-local by adding scroll_x.
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());

        // INV-SEPARATOR-PLACEMENT: Column resize separators are rendered and hit-tested
        // only between adjacent columns, never after the last column.
        let hts = core.cms.hts();
        let Some(column_layout) = hts.try_column_layout() else {
            return None;
        };
        let all_slots: Vec<_> = column_layout.header_slots().collect();
        for pair in all_slots.windows(2) {
            let left = pair[0];
            let sep_x = left.x + left.width - 1.0;

            // Pinned columns: test in viewport-local space (no scroll adjustment).
            // Scrollable columns: convert to content-local by adding scroll_x.
            let test_x = if left.scroll_domain != ColumnScrollDomain::Scrollable {
                local_x // pinned (left or right): viewport-local
            } else {
                local_x + scroll_x // scrollable: viewport → content
            };

            if (test_x - sep_x).abs() <= SEPARATOR_HIT_HALF_WIDTH {
                return Some(ResizeSeparatorHit {
                    column_id: left.column_id,
                });
            }
        }

        None
    }

    fn separator_target_resizable(&self, hit: ResizeSeparatorHit) -> bool {
        let core = self.core.borrow();
        let model = core.column_model();
        model.get(hit.column_id).map_or(true, |c| c.resizable)
    }

    /// Hit-test which column cell a local X coordinate falls within.
    fn hit_test_column(&self, local_x: f32) -> Option<ColumnId> {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let hts = core.cms.hts();
        let Some(column_layout) = hts.try_column_layout() else {
            return None;
        };
        for slot in column_layout.header_slots() {
            let slot_left = slot.x;
            let slot_right = slot_left + slot.width;
            let test_x = if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                local_x
            } else {
                local_x + scroll_x
            };
            if test_x >= slot_left && test_x < slot_right {
                return Some(slot.column_id);
            }
        }
        None
    }

    /// Check if the platform-specific pin modifier is active (CMD on macOS, CTRL elsewhere).
    fn is_platform_pin_modifier(modifiers: &crate::core::event::Modifiers) -> bool {
        #[cfg(target_os = "macos")]
        {
            modifiers.meta && !modifiers.ctrl && !modifiers.alt && !modifiers.shift
        }
        #[cfg(not(target_os = "macos"))]
        {
            modifiers.ctrl && !modifiers.meta && !modifiers.alt && !modifiers.shift
        }
    }

    // =========================================================================
    // Column Resize Interaction
    // =========================================================================

    /// Get the current width of a column.
    fn get_column_width(&self, column_id: ColumnId) -> Option<f32> {
        let core = self.core.borrow();
        let hts = core.cms.hts();
        hts.try_column_layout()
            .and_then(|layout| layout.get(column_id).map(|slot| slot.width))
    }

    /// Finalize resize and clean up all resize state.
    ///
    /// INV-RESIZE-CLEAN-EXIT: All state cleared unconditionally.
    /// INV-HOVER-CLEAR-ON-FINALIZE: Clear hover to avoid stale visual state
    /// after pointer capture is released outside the hit zone.
    fn finalize_resize(&mut self) {
        self.resize_state = ResizeState::Idle;
        let mut core = self.core.borrow_mut();
        core.active_resize_separator = None;
        core.hovered_separator = None;
        // INV-RESIZE-HSCROLL-2: Unfreeze horizontal scroll when resize ends.
        if let Some(ref freeze) = core.horizontal_scroll_freeze {
            freeze.set(false);
        }
    }

    fn commit_resize(&mut self) {
        self.finalize_resize();
        let core = self.core.borrow();
        let Some(key) = core.view_config_key() else {
            return;
        };
        let Some(store) = core.config_store.as_ref() else {
            return;
        };
        view_config::save_column_config(&*core.column_model(), &key, store.as_ref());
    }

    /// Cancel resize and rollback to original width.
    ///
    /// INV-RESIZE-CLEAN-EXIT: On cancel, state is fully reset.
    /// INV-RESIZE-DETERMINISTIC: Rollback restores initial width exactly.
    fn cancel_resize(&mut self) {
        // Rollback widths if we were dragging.
        if let ResizeState::Dragging {
            column_id,
            rollback_width,
            ..
        } = self.resize_state
        {
            self.apply_column_width(column_id, rollback_width);
        }
        self.finalize_resize();
    }

    /// Check if resize is active (Armed or Dragging).
    ///
    /// INV-RESIZE-EXCLUSIVE: Returns true when row interaction must be suppressed.
    fn is_resize_active(&self) -> bool {
        !matches!(self.resize_state, ResizeState::Idle)
    }

    /// Apply column width change via the substrate resize authority.
    ///
    /// INV-RESIZE-OWNERSHIP: Width changes go through the substrate.
    /// SPEC-HAL-169 INVARIANT-5: No clamping here — substrate decides.
    fn apply_column_width(&mut self, column_id: ColumnId, width: f32) -> Option<f32> {
        let mut core = self.core.borrow_mut();
        core.commit_user_resize(column_id, width)
    }

    fn apply_resize_delta(
        &mut self,
        column_id: ColumnId,
        start_x: f32,
        initial_width: f32,
        pointer_x: f32,
    ) -> (f32, f32) {
        let proposed_width = initial_width + (pointer_x - start_x);
        let Some(applied_width) = self.apply_column_width(column_id, proposed_width) else {
            return (start_x, initial_width);
        };

        if (applied_width - proposed_width).abs() > 0.001 {
            (pointer_x, applied_width)
        } else {
            (start_x, initial_width)
        }
    }

    /// Compute auto-size width for a column based on visible content.
    ///
    /// INV-AUTOSIZE-VISIBLE: Only measures visible realized rows (no full scan).
    /// INV-AUTOSIZE-HEADER: Includes header text width.
    /// Returns optimal width = max(header_width, max_cell_width) + 2*padding.
    fn compute_auto_size_width(
        &self,
        column_id: ColumnId,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
    ) -> f32 {
        let core = self.core.borrow();

        // Start with header text width.
        let mut max_width: f32 = 0.0;
        if let Some(title) = core.column_titles.get(&column_id) {
            let run = text_engine.shape(title, self.font_size);
            max_width = run.width;
        }

        let visible_rows = core.visible_row_projections();
        for row_projection in visible_rows {
            let cell_value = core.adapter.read_cell(row_projection.row_id, column_id);
            let text = match cell_value {
                crate::widgets::tabular_view::data_adapter::ValueState::Ready(
                    crate::widgets::tabular_view::data_adapter::CellValue::String(s),
                ) => s,
                crate::widgets::tabular_view::data_adapter::ValueState::Ready(
                    crate::widgets::tabular_view::data_adapter::CellValue::Int(n),
                ) => format!("{}", n),
                crate::widgets::tabular_view::data_adapter::ValueState::Ready(
                    crate::widgets::tabular_view::data_adapter::CellValue::Float(f),
                ) => format!("{:.2}", f),
                crate::widgets::tabular_view::data_adapter::ValueState::Ready(
                    crate::widgets::tabular_view::data_adapter::CellValue::Bool(b),
                ) => format!("{}", b),
                _ => String::new(),
            };

            if !text.is_empty() {
                let run = text_engine.shape(&text, self.font_size);
                max_width = max_width.max(run.width);
            }
        }

        // Add padding on both sides.
        max_width + self.text_padding * 2.0
    }

    // =========================================================================
    // Column Drag-Reorder Interaction
    // =========================================================================

    /// Compute insertion index from pointer global X.
    ///
    /// Iterates visible header slots (excluding dragged column), computes
    /// viewport midpoints, returns the index where the dragged column
    /// should be inserted.
    fn compute_insertion_index(
        &self,
        dragged_id: ColumnId,
        pointer_global_x: f32,
        rect: &RectF,
    ) -> Option<usize> {
        let core = self.core.borrow();
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());
        let hts = core.cms.hts();
        let Some(column_layout) = hts.try_column_layout() else {
            return None;
        };
        let pinned_width = column_layout.pinned_width();
        let base_x = rect.x.floor();

        let slots: Vec<_> = column_layout
            .header_slots()
            .filter(|s| s.column_id != dragged_id)
            .collect();

        // Compute closed-up positions (remaining columns packed as if dragged removed).
        let mut closed_positions: Vec<f32> = Vec::with_capacity(slots.len());
        let mut p_acc = 0.0f32;
        let mut s_acc = pinned_width;
        for slot in &slots {
            if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                closed_positions.push(p_acc);
                p_acc += slot.width;
            } else {
                closed_positions.push(s_acc);
                s_acc += slot.width;
            }
        }

        // Find insertion point based on closed-up midpoints.
        for (i, slot) in slots.iter().enumerate() {
            let closed_x = closed_positions[i];
            let viewport_x = if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                base_x + closed_x
            } else {
                base_x + closed_x - scroll_x
            };
            let midpoint = viewport_x + slot.width / 2.0;
            if pointer_global_x < midpoint {
                return Some(i);
            }
        }
        Some(slots.len()) // After last column
    }

    /// Commit drag reorder: compute insertion, build new order, call reorder.
    fn commit_drag_reorder(&mut self, cx: &mut EventContext) {
        if let ColumnDragState::Active {
            column_id,
            current_global_x,
            ..
        } = self.drag_state
        {
            let rect = cx.widget_global_rect();
            if let Some(rect) = rect {
                let Some(insertion_idx) =
                    self.compute_insertion_index(column_id, current_global_x, &rect)
                else {
                    self.finalize_drag_and_invalidate_body(cx);
                    return;
                };

                // Build new visible order: all visible except dragged,
                // then insert dragged at computed index.
                let core = self.core.borrow();
                let current_visible: Vec<ColumnId> =
                    if let Some(column_layout) = core.cms.hts().try_column_layout() {
                        column_layout.header_slots().map(|s| s.column_id).collect()
                    } else {
                        drop(core);
                        self.finalize_drag_and_invalidate_body(cx);
                        return;
                    };
                // Full column list (visible + hidden) for reorder API.
                let full_columns: Vec<ColumnId> =
                    core.column_model().columns().iter().map(|c| c.id).collect();
                drop(core);

                let mut new_visible: Vec<ColumnId> = current_visible
                    .iter()
                    .filter(|id| **id != column_id)
                    .copied()
                    .collect();
                let clamped_idx = insertion_idx.min(new_visible.len());
                new_visible.insert(clamped_idx, column_id);

                // Build full order: visible columns in new order, hidden
                // columns appended at end (preserves visible reorder while
                // satisfying ColumnModel::reorder length requirement).
                let mut new_order = new_visible.clone();
                for col_id in &full_columns {
                    if !new_order.contains(col_id) {
                        new_order.push(*col_id);
                    }
                }

                // Check if order actually changed from full column order.
                if new_order != full_columns {
                    let mut core = self.core.borrow_mut();
                    core.dispatch_intent(&TabularIntent::Reorder(ReorderIntent::Apply {
                        new_order,
                    }));
                    drop(core);
                    if let Some(body_id) = self.core.borrow().body_widget_id {
                        cx.invalidate_layout_by_id(body_id);
                    }
                }
                // else: same position → revert is implicit (no change needed)
            }
        }
        // Clean up state
        self.finalize_drag_and_invalidate_body(cx);
    }

    /// Cancel drag reorder: restore order from snapshot.
    fn cancel_drag_reorder(&mut self, cx: &mut EventContext) {
        if let ColumnDragState::Active {
            ref order_snapshot, ..
        }
        | ColumnDragState::Armed {
            ref order_snapshot, ..
        } = self.drag_state
        {
            let snapshot = order_snapshot.clone();
            // Restore original order. Append any columns not in
            // snapshot (hidden since drag started) to satisfy
            // ColumnModel::reorder length requirement.
            let mut core = self.core.borrow_mut();
            let mut restore_order = snapshot;
            for col in core.column_model().columns() {
                if !restore_order.contains(&col.id) {
                    restore_order.push(col.id);
                }
            }
            core.dispatch_intent(&TabularIntent::Reorder(ReorderIntent::Apply {
                new_order: restore_order,
            }));
            drop(core);
            if let Some(body_id) = self.core.borrow().body_widget_id {
                cx.invalidate_layout_by_id(body_id);
            }
        }
        self.finalize_drag_and_invalidate_body(cx);
    }

    /// Clean up drag state, animation handles, and unfreeze scroll.
    fn finalize_drag(&mut self) {
        self.drag_state = ColumnDragState::Idle;
        self.pending_drag_value_paint = false;
        self.drag_slide_handles.get_mut().clear();
        self.drag_slide_values.get_mut().clear();
        self.drag_slide_targets.clear();
        let mut core = self.core.borrow_mut();
        core.dragged_column_id = None;
        core.drag_visual_offsets = None;
        core.ghost_occlude_range = None;
        if let Some(ref freeze) = core.horizontal_scroll_freeze {
            freeze.set(false);
        }
    }

    fn finalize_drag_and_invalidate_body(&mut self, cx: &mut EventContext) {
        if let Some(column_id) = match &self.drag_state {
            ColumnDragState::Armed { column_id, .. }
            | ColumnDragState::Active { column_id, .. } => Some(*column_id),
            ColumnDragState::Idle => None,
        } {
            self.animate_header_pressed(cx, column_id, false);
        }
        let body_id = self.core.borrow().body_widget_id;
        self.finalize_drag();
        if let Some(body_id) = body_id {
            cx.invalidate_layout_by_id(body_id);
        }
    }

    fn animate_header_hovered(&self, cx: &mut EventContext, column_id: ColumnId, hovered: bool) {
        let mut visuals = self.header_cell_visuals.borrow_mut();
        let visual = visuals.entry(column_id).or_default();
        let target = if hovered { 1.0 } else { 0.0 };
        let animation = Animation {
            from: visual.hover_t,
            to: target,
            duration: if hovered { 0.12 } else { 0.08 },
            easing: if hovered {
                Easing::EaseOut
            } else {
                Easing::EaseIn
            },
        };
        if let Some(handle) = cx.start_animation(animation) {
            visual.hover_anim = Some(handle);
        } else {
            visual.hover_t = target;
            visual.hover_anim = None;
        }
    }

    fn animate_header_pressed(&self, cx: &mut EventContext, column_id: ColumnId, pressed: bool) {
        let mut visuals = self.header_cell_visuals.borrow_mut();
        let visual = visuals.entry(column_id).or_default();
        let target = if pressed { 1.0 } else { 0.0 };
        let animation = Animation {
            from: visual.pressed_t,
            to: target,
            duration: if pressed { 0.035 } else { 0.08 },
            easing: if pressed {
                Easing::EaseOut
            } else {
                Easing::EaseIn
            },
        };
        if let Some(handle) = cx.start_animation(animation) {
            visual.pressed_anim = Some(handle);
        } else {
            visual.pressed_t = target;
            visual.pressed_anim = None;
        }
    }

    fn update_header_hover(&mut self, cx: &mut EventContext, global_pos: (f32, f32)) {
        let next = cx.widget_global_rect().and_then(|rect| {
            let in_header = global_pos.1 >= rect.y
                && global_pos.1 < rect.y + self.header_height
                && global_pos.0 >= rect.x
                && global_pos.0 < rect.x + rect.width;
            if !in_header {
                return None;
            }
            let local_x = global_pos.0 - rect.x;
            if self.hit_test_separator(local_x).is_some() {
                return None;
            }
            let config_button_left = rect.width - 28.0;
            if local_x >= config_button_left {
                return None;
            }
            self.hit_test_column(local_x)
        });

        if next == self.header_hovered_column {
            return;
        }
        if let Some(previous) = self.header_hovered_column {
            self.animate_header_hovered(cx, previous, false);
        }
        if let Some(column_id) = next {
            self.animate_header_hovered(cx, column_id, true);
        }
        self.header_hovered_column = next;
    }

    fn clear_header_hover(&mut self, cx: &mut EventContext) {
        if let Some(column_id) = self.header_hovered_column.take() {
            self.animate_header_hovered(cx, column_id, false);
        }
    }

    fn header_visual_snapshot(&self, cx: &mut PaintContext) -> HashMap<ColumnId, (f32, f32)> {
        let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
        let mut visuals = self.header_cell_visuals.borrow_mut();
        for visual in visuals.values_mut() {
            if let Some(handle) = visual.hover_anim {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    visual.hover_t = value;
                } else {
                    visual.hover_anim = None;
                }
            }
            if let Some(handle) = visual.pressed_anim {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    visual.pressed_t = value;
                } else {
                    visual.pressed_anim = None;
                }
            }
        }
        visuals.retain(|_, visual| {
            visual.hover_anim.is_some()
                || visual.pressed_anim.is_some()
                || visual.hover_t > 0.001
                || visual.pressed_t > 0.001
        });
        visuals
            .iter()
            .map(|(column_id, visual)| (*column_id, (visual.hover_t, visual.pressed_t)))
            .collect()
    }

    fn paint_combo_box_drag_proxy(cx: &mut PaintContext, rect: RectF, text: &str) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }

        let t = cx.theme;
        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let bx = snap_to_physical(rect.x, scale);
        let by = snap_to_physical(rect.y, scale);
        let br = snap_to_physical(rect.x + rect.width, scale);
        let bb = snap_to_physical(rect.y + rect.height, scale);
        let bw = (br - bx).max(0.0);
        let bh = (bb - by).max(0.0);
        let snapped = RectF::new(bx, by, bw, bh);
        let radius = t.chrome_radius(
            ControlChromeCategory::RecessedEditable,
            t.geometry.combo_box.radius,
            false,
        );
        let chrome = t.control_chrome(
            ControlChromeCategory::RecessedEditable,
            ControlChromeState::Idle,
            ControlValidationState::Neutral,
        );
        let edge_thick = one_px;
        let affordance_width = t.geometry.combo_box.affordance_width.min(snapped.width);

        cx.scene.push_rounded_rect(
            snapped,
            chrome.border_color,
            RoundedCorners::uniform(radius),
            DRAG_GHOST_CHROME_LAYER,
            None,
        );
        let inset_rect = RectF::new(
            bx + edge_thick,
            by + edge_thick,
            (bw - edge_thick * 2.0).max(0.0),
            (bh - edge_thick * 2.0).max(0.0),
        );
        cx.scene.push_rounded_rect(
            inset_rect,
            chrome.fill.unwrap_or(t.colors.surfaces.content),
            RoundedCorners::uniform((radius - edge_thick).max(0.0)),
            DRAG_GHOST_CHROME_LAYER,
            None,
        );

        let text_width = (snapped.width - affordance_width).max(0.0);
        let padding_x = t.geometry.text_field.padding_x;
        let text_viewport = RectF::new(
            snapped.x + padding_x,
            snapped.y,
            (text_width - padding_x * 2.0).max(0.0),
            snapped.height,
        );
        cx.scene.begin_clip(text_viewport, None);
        if !text.is_empty() {
            let font_size = t.typography.font_size;
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = snapped.y
                + compute_control_text_baseline_y(
                    snapped.height,
                    metrics.ascent,
                    metrics.descent,
                    t.typography.baseline_offset,
                );
            let text_y = baseline_y - metrics.ascent;
            let run = cx.text_engine.shape(text, font_size);
            cx.text_engine.draw(
                &run,
                text_viewport.x,
                text_y,
                t.colors.typography.primary,
                DRAG_GHOST_TEXT_LAYER,
                cx.scene,
            );
        }
        cx.scene.end_clip();

        let sep_x = snap_to_physical(snapped.x + snapped.width - affordance_width, scale);
        let sep_inset = edge_thick + 2.0 * one_px;
        let sep_top = by + sep_inset;
        let sep_bottom = bb - sep_inset;
        cx.scene.push_rounded_rect(
            RectF::new(sep_x, sep_top, one_px, (sep_bottom - sep_top).max(0.0)),
            t.colors.borders.subtle,
            RoundedCorners::uniform(0.5),
            DRAG_GHOST_BORDER_LAYER,
            None,
        );

        let aff_rect = RectF::new(
            snapped.x + snapped.width - affordance_width,
            snapped.y,
            affordance_width,
            snapped.height,
        );
        let bar_width = (affordance_width * 0.45).round();
        let bar_height = 1.5;
        let gap = 3.0;
        let total_h = bar_height * 2.0 + gap;
        let bar_x = aff_rect.x + (aff_rect.width - bar_width) / 2.0;
        let top_y = aff_rect.y + (aff_rect.height - total_h) / 2.0;
        let bar_radius = RoundedCorners::uniform(0.5);
        cx.scene.push_rounded_rect(
            RectF::new(bar_x, top_y, bar_width, bar_height),
            t.colors.typography.primary,
            bar_radius,
            DRAG_GHOST_BORDER_LAYER,
            None,
        );
        cx.scene.push_rounded_rect(
            RectF::new(bar_x, top_y + bar_height + gap, bar_width, bar_height),
            t.colors.typography.primary,
            bar_radius,
            DRAG_GHOST_BORDER_LAYER,
            None,
        );
    }

    /// Compute target visual offsets and start/retarget per-column animations.
    ///
    /// Called from PointerMove during Active drag. Computes where each non-dragged
    /// column should visually be (with gap for dragged column), then starts
    /// Animation<f32> via the engine animation system for columns whose target changed.
    fn update_drag_slide_targets(&mut self, cx: &mut EventContext) {
        let ColumnDragState::Active {
            column_id: dragged_id,
            current_global_x,
            ..
        } = &self.drag_state
        else {
            return;
        };
        let dragged_id = *dragged_id;
        let current_global_x = *current_global_x;

        let Some(rect) = cx.widget_global_rect() else {
            return;
        };
        let base_x = rect.x.floor();

        let target_map = {
            let core = self.core.borrow();
            let scroll_x = core
                .horizontal_scroll_link
                .as_ref()
                .map_or(0.0, |l| l.get());
            let hts = core.cms.hts();
            let Some(column_layout) = hts.try_column_layout() else {
                return;
            };
            let pinned_width = column_layout.pinned_width();

            let all_slots: Vec<_> = column_layout.header_slots().collect();
            let dragged_width = all_slots
                .iter()
                .find(|s| s.column_id == dragged_id)
                .map_or(0.0, |s| s.width);
            let remaining: Vec<_> = all_slots
                .iter()
                .filter(|s| s.column_id != dragged_id)
                .collect();

            // Step 1: Compute closed-up positions (remaining columns packed as if dragged removed).
            let mut closed_positions: Vec<f32> = Vec::with_capacity(remaining.len());
            let mut pinned_acc = 0.0f32;
            let mut scrollable_acc = pinned_width;
            for s in &remaining {
                if s.scroll_domain != ColumnScrollDomain::Scrollable {
                    closed_positions.push(pinned_acc);
                    pinned_acc += s.width;
                } else {
                    closed_positions.push(scrollable_acc);
                    scrollable_acc += s.width;
                }
            }

            // Step 2: Compute insertion index using closed-up viewport midpoints.
            let mut insertion_idx = remaining.len();
            for (i, s) in remaining.iter().enumerate() {
                let closed_x = closed_positions[i];
                let vp_x = if s.scroll_domain != ColumnScrollDomain::Scrollable {
                    base_x + closed_x
                } else {
                    base_x + closed_x - scroll_x
                };
                let midpoint = vp_x + s.width / 2.0;
                if current_global_x < midpoint {
                    insertion_idx = i;
                    break;
                }
            }

            // Step 3: Build target visual positions with gap at insertion_idx.
            let mut target_map = HashMap::with_capacity(remaining.len());
            let mut p_acc = 0.0f32;
            let mut s_acc = pinned_width;
            let mut vi = 0usize;
            for s in &remaining {
                if s.scroll_domain != ColumnScrollDomain::Scrollable {
                    if vi == insertion_idx {
                        p_acc += dragged_width;
                    }
                    target_map.insert(s.column_id, p_acc);
                    p_acc += s.width;
                } else {
                    if vi == insertion_idx {
                        s_acc += dragged_width;
                    }
                    target_map.insert(s.column_id, s_acc);
                    s_acc += s.width;
                }
                vi += 1;
            }
            target_map
        };

        // Retarget animations where target changed materially.
        let epsilon = 0.5;
        let handles = self.drag_slide_handles.get_mut();
        let values = self.drag_slide_values.get_mut();
        for (col_id, new_target) in &target_map {
            let old_target = self.drag_slide_targets.get(col_id).copied();
            if old_target.map_or(true, |old| (old - new_target).abs() > epsilon) {
                // Target changed — start new animation from current to new target.
                let current = values.get(col_id).copied().unwrap_or(*new_target);
                let anim = Animation {
                    from: current,
                    to: *new_target,
                    duration: 0.22,
                    easing: Easing::EaseOut,
                };
                if let Some(handle) = cx.start_animation(anim) {
                    handles.insert(*col_id, handle);
                } else {
                    // No animation capability — snap to target.
                    values.insert(*col_id, *new_target);
                }
            }
        }
        // Remove handles for columns no longer in target map.
        handles.retain(|id, _| target_map.contains_key(id));
        self.drag_slide_targets = target_map;
    }

    fn paint_header_chrome(&self, cx: &mut PaintContext, rect: RectF, core: &ListViewCore<S>) {
        let header_visuals = self.header_visual_snapshot(cx);
        let (pinned_width, all_header_slots, scrollable_slots, pinned_slots) = {
            let hts = core.cms.hts();
            let Some(column_layout) = hts.try_column_layout() else {
                return;
            };
            (
                column_layout.pinned_width(),
                column_layout.header_slots().collect::<Vec<_>>(),
                column_layout.scrollable_slots().collect::<Vec<_>>(),
                column_layout.pinned_slots().collect::<Vec<_>>(),
            )
        };

        let font_size = cx.theme.typography.font_size;
        let text_padding = cx.theme.list_geom.padding_x;
        let header_background = cx.theme.colors.list.header_background;
        let pinned_tint = cx.theme.colors.list.pinned_tint;

        let header_y = rect.y;
        let header_rect = RectF::new(rect.x, header_y, rect.width, self.header_height);
        cx.scene
            .push_rect(header_rect, header_background, HEADER_CHROME_LAYER, None);

        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());

        cx.scene.begin_clip(header_rect, None);

        let sf = cx.scene.scale_factor();
        let phys_px = 1.0 / sf;
        let base_x = snap_to_physical(rect.x, sf);
        let inter_column_slots = &all_header_slots[..all_header_slots.len().saturating_sub(1)];

        let boundary_left_column_id = inter_column_slots
            .iter()
            .rev()
            .find(|s| s.scroll_domain == ColumnScrollDomain::PinnedLeft)
            .map(|s| s.column_id);
        let boundary_right_column_id = inter_column_slots
            .iter()
            .find(|s| s.scroll_domain == ColumnScrollDomain::PinnedRight)
            .and_then(|_| {
                inter_column_slots
                    .iter()
                    .rev()
                    .find(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
                    .map(|s| s.column_id)
            });

        let drag_visual_offsets = &core.drag_visual_offsets;
        let ghost_occlude = core.ghost_occlude_range;

        {
            let scrollable_clip_x = base_x + pinned_width;
            let scrollable_clip_w = (rect.x + rect.width - scrollable_clip_x).max(0.0);
            let scrollable_clip = RectF::new(
                scrollable_clip_x,
                header_y,
                scrollable_clip_w,
                self.header_height,
            );
            cx.scene.begin_clip(scrollable_clip, None);

            for slot in scrollable_slots {
                if let ColumnDragState::Active { column_id, .. } = &self.drag_state {
                    if slot.column_id == *column_id {
                        continue;
                    }
                }
                let visual_x = if let Some(offsets) = drag_visual_offsets {
                    offsets.get(&slot.column_id).copied().unwrap_or(slot.x)
                } else {
                    slot.x
                };
                let cell_x = base_x + visual_x - scroll_x;
                let cell_rect = RectF::new(cell_x, header_y, slot.width, self.header_height);
                self.paint_header_cell_feedback(
                    cx,
                    cell_rect,
                    header_visuals.get(&slot.column_id).copied(),
                    header_background,
                );

                let text_clip = if let Some((gs, ge)) = ghost_occlude {
                    let cs = visual_x;
                    let ce = visual_x + slot.width;
                    if cs >= ge || ce <= gs {
                        Some(cell_rect)
                    } else if cs >= gs && ce <= ge {
                        None
                    } else if cs < gs {
                        Some(RectF::new(cell_x, header_y, gs - cs, self.header_height))
                    } else {
                        Some(RectF::new(
                            cell_x + (ge - cs),
                            header_y,
                            ce - ge,
                            self.header_height,
                        ))
                    }
                } else {
                    Some(cell_rect)
                };
                if let Some(clip) = text_clip {
                    self.paint_header_cell_content(
                        cx,
                        core,
                        slot.column_id,
                        cell_rect,
                        clip,
                        header_visuals.get(&slot.column_id).copied(),
                        font_size,
                        text_padding,
                    );
                }
            }

            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
            {
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if is_active || is_hovered {
                    continue;
                }
                let sep_x = if let Some(offsets) = drag_visual_offsets {
                    let visual_x = offsets.get(&slot.column_id).copied().unwrap_or(slot.x);
                    snap_to_physical(base_x + visual_x + slot.width, sf) - phys_px - scroll_x
                } else {
                    snap_to_physical(base_x + slot.x + slot.width, sf) - phys_px - scroll_x
                };
                let sep_rect = RectF::new(sep_x, rect.y, phys_px, self.header_height);
                cx.scene.push_rect(
                    sep_rect,
                    cx.theme.colors.borders.default,
                    HEADER_SEPARATOR_LAYER,
                    None,
                );
            }

            cx.scene.end_clip();
        }

        {
            if pinned_width > 0.0 {
                let pinned_bg = RectF::new(rect.x, header_y, pinned_width, self.header_height);
                cx.scene
                    .push_rect(pinned_bg, pinned_tint, HEADER_CHROME_LAYER, None);
            }

            for slot in pinned_slots {
                if let ColumnDragState::Active { column_id, .. } = &self.drag_state {
                    if slot.column_id == *column_id {
                        continue;
                    }
                }
                let visual_x = if let Some(offsets) = drag_visual_offsets {
                    offsets.get(&slot.column_id).copied().unwrap_or(slot.x)
                } else {
                    slot.x
                };
                let cell_x = base_x + visual_x;
                let cell_rect = RectF::new(cell_x, header_y, slot.width, self.header_height);
                self.paint_header_cell_feedback(
                    cx,
                    cell_rect,
                    header_visuals.get(&slot.column_id).copied(),
                    pinned_tint,
                );

                let text_clip = if let Some((gs, ge)) = ghost_occlude {
                    let cs = visual_x;
                    let ce = visual_x + slot.width;
                    if cs >= ge || ce <= gs {
                        Some(cell_rect)
                    } else if cs >= gs && ce <= ge {
                        None
                    } else if cs < gs {
                        Some(RectF::new(cell_x, header_y, gs - cs, self.header_height))
                    } else {
                        Some(RectF::new(
                            cell_x + (ge - cs),
                            header_y,
                            ce - ge,
                            self.header_height,
                        ))
                    }
                } else {
                    Some(cell_rect)
                };
                if let Some(clip) = text_clip {
                    self.paint_header_cell_content(
                        cx,
                        core,
                        slot.column_id,
                        cell_rect,
                        clip,
                        header_visuals.get(&slot.column_id).copied(),
                        font_size,
                        text_padding,
                    );
                }
            }

            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain != ColumnScrollDomain::Scrollable)
            {
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if is_active || is_hovered {
                    continue;
                }
                let sep_x = if let Some(offsets) = drag_visual_offsets {
                    let visual_x = offsets.get(&slot.column_id).copied().unwrap_or(slot.x);
                    snap_to_physical(base_x + visual_x + slot.width, sf) - phys_px
                } else {
                    snap_to_physical(base_x + slot.x + slot.width, sf) - phys_px
                };
                let is_boundary = boundary_left_column_id == Some(slot.column_id)
                    || boundary_right_column_id == Some(slot.column_id);
                let sep_color = if is_boundary {
                    cx.theme.colors.borders.strong
                } else {
                    cx.theme.colors.borders.default
                };
                let sep_rect = RectF::new(sep_x, rect.y, phys_px, self.header_height);
                cx.scene
                    .push_rect(sep_rect, sep_color, HEADER_SEPARATOR_LAYER, None);
            }
        }

        {
            let btn_size = 24.0;
            let btn_x = rect.x + rect.width - btn_size - 4.0;
            let btn_y = header_y + (self.header_height - btn_size) / 2.0;
            let btn_rect = RectF::new(btn_x, btn_y, btn_size, btn_size);
            cx.scene.push_rect(
                btn_rect,
                cx.theme.colors.surfaces.level3,
                HEADER_CHROME_LAYER,
                None,
            );
            cx.scene.push_border_rect(
                btn_rect,
                cx.theme.colors.borders.default,
                1.0,
                HEADER_SEPARATOR_LAYER,
                None,
            );
            if let Some(icon) = cx.icon_registry.get(IconId::SETTINGS) {
                let icon_size = IconSize::L.logical_px();
                cx.scene.push_icon(
                    icon,
                    btn_x + (btn_size - icon_size) / 2.0,
                    btn_y + (btn_size - icon_size) / 2.0,
                    icon_size,
                    cx.theme.colors.typography.primary,
                    HEADER_TEXT_LAYER,
                );
            }
        }

        let header_border_rect = RectF::new(
            rect.x,
            snap_to_physical(rect.y + self.header_height - phys_px, sf),
            rect.width,
            phys_px,
        );
        cx.scene.push_rect(
            header_border_rect,
            cx.theme.colors.borders.strong,
            HEADER_SEPARATOR_LAYER,
            None,
        );

        cx.scene.end_clip();
    }

    fn paint_header_cell_feedback(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        visual: Option<(f32, f32)>,
        base: crate::core::paint::Color,
    ) {
        let Some((hover_t, pressed_t)) = visual else {
            return;
        };
        if hover_t <= 0.001 && pressed_t <= 0.001 {
            return;
        }
        let hovered = cx.theme.control_chrome(
            ControlChromeCategory::RaisedClickable,
            ControlChromeState::Hovered,
            ControlValidationState::Neutral,
        );
        let pressed = cx.theme.control_chrome(
            ControlChromeCategory::RaisedClickable,
            ControlChromeState::Pressed,
            ControlValidationState::Neutral,
        );
        let hover_fill = hovered.fill.unwrap_or(base);
        let pressed_fill = pressed.fill.unwrap_or(hover_fill);
        let fill = base
            .lerp(hover_fill, hover_t.clamp(0.0, 1.0) * 0.55)
            .lerp(pressed_fill, pressed_t.clamp(0.0, 1.0) * 0.8);
        cx.scene.push_rect(rect, fill, HEADER_CHROME_LAYER, None);
    }

    #[allow(clippy::too_many_arguments)]
    fn paint_header_cell_content(
        &self,
        cx: &mut PaintContext,
        core: &ListViewCore<S>,
        column_id: ColumnId,
        cell_rect: RectF,
        clip: RectF,
        visual: Option<(f32, f32)>,
        font_size: f32,
        text_padding: f32,
    ) {
        let Some(title) = self.projected_header_title(core, column_id) else {
            return;
        };
        let indicator = self.projected_sort_indicator(core, column_id);
        let indicator_run = indicator
            .as_ref()
            .map(|(text, _)| cx.text_engine.shape(text, font_size * 0.9));
        let indicator_x = indicator_run
            .as_ref()
            .map(|run| cell_rect.x + cell_rect.width - text_padding - run.width);
        let clip_right = clip.x + clip.width;
        let title_right = indicator_x
            .map(|x| x - text_padding)
            .unwrap_or(clip_right)
            .min(clip_right);
        let title_clip = RectF::new(clip.x, clip.y, (title_right - clip.x).max(0.0), clip.height);

        cx.scene.begin_clip(clip, None);
        if title_clip.width > 0.0 {
            cx.scene.begin_clip(title_clip, None);
            let run = cx.text_engine.shape(&title, font_size);
            let text_y = cell_rect.y + (self.header_height - run.height) / 2.0;
            cx.scene.push_text_with_backgrounds(
                &run,
                cell_rect.x + text_padding,
                text_y,
                cx.theme.colors.typography.secondary,
                Vec::new(),
                HEADER_TEXT_LAYER,
                None,
            );
            cx.scene.end_clip();
        }
        if let (Some((_, active)), Some(run), Some(x)) =
            (indicator, indicator_run.as_ref(), indicator_x)
        {
            let (hover_t, pressed_t) = visual.unwrap_or((0.0, 0.0));
            let color = if active {
                cx.theme.colors.typography.primary
            } else {
                let alpha = (0.32 + hover_t * 0.3 + pressed_t * 0.12).min(0.76);
                cx.theme.colors.typography.muted.with_alpha(alpha)
            };
            let text_y = cell_rect.y + (self.header_height - run.height) / 2.0;
            cx.scene.push_text_with_backgrounds(
                run,
                x,
                text_y,
                color,
                Vec::new(),
                HEADER_TEXT_LAYER,
                None,
            );
        }
        cx.scene.end_clip();
    }

    fn projected_header_title(
        &self,
        core: &ListViewCore<S>,
        column_id: ColumnId,
    ) -> Option<String> {
        core.column_titles.get(&column_id).cloned()
    }

    fn projected_sort_indicator(
        &self,
        core: &ListViewCore<S>,
        column_id: ColumnId,
    ) -> Option<(String, bool)> {
        if !core.sort_capability_for_column(column_id).is_allowed() {
            return None;
        }
        let key = core.sort_key_for_column(column_id)?;
        let active = core.applied_sort().and_then(|snapshot| {
            snapshot
                .spec
                .terms
                .iter()
                .enumerate()
                .find(|(_, term)| term.key == key)
                .map(|(priority, term)| (priority, term.direction))
        });
        if let Some((priority, direction)) = active {
            let arrow = match direction {
                crate::widgets::tabular_view::intents::SortDirection::Ascending => '↑',
                crate::widgets::tabular_view::intents::SortDirection::Descending => '↓',
            };
            return Some((format!("{arrow}{}", priority + 1), true));
        }
        Some(("↑↓".to_string(), false))
    }
}

impl<S: DataSource + 'static> WidgetDowncast for ListView<S> {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl<S: DataSource + 'static> Widget for ListView<S> {
    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if std::mem::take(&mut self.pending_drag_value_paint) {
            cx.invalidate_paint();
        }
    }

    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let available_width = constraints.max_width;
        let available_height = constraints.max_height;

        // Cache theme values for event handling (auto-size uses these).
        self.font_size = cx.theme.typography.font_size;
        self.text_padding = cx.theme.list_geom.padding_x;

        // INV-LIST-VIEWPORT-1: A self-virtualizing widget (we declare
        // needs_viewport_constraint = true at line 1299) MUST NOT operate with
        // an unbounded viewport. When our parent measures us with unbounded
        // constraints (typical Flex measurement pass for a flexible child),
        // the body_height arithmetic below would yield INFINITY, and writing
        // INFINITY into substrate.body_viewport corrupts every downstream
        // visible_rows() projection — paint, hit-test, and child layouts all
        // walk the entire dataset, O(N) per frame.
        //
        // Skip the substrate update, but still lay out structural children with
        // finite degenerate constraints. Otherwise the layout cleanup pass sees
        // dirty ScrollArea/Footer children and may try to resolve them with the
        // original unbounded measurement constraints.
        if !available_height.is_finite() {
            let fallback_width = if available_width.is_finite() {
                available_width
            } else {
                0.0
            };
            if !children.is_empty() {
                let body_constraints = Constraints::new(fallback_width, fallback_width, 0.0, 0.0);
                cx.layout_child(children[0], body_constraints);
                cx.set_child_rect(
                    children[0],
                    RectF::new(0.0, self.header_height, fallback_width, 0.0),
                );
            }
            if children.len() > 1 {
                let footer_constraints = Constraints::new(
                    fallback_width,
                    fallback_width,
                    self.footer_height,
                    self.footer_height,
                );
                cx.layout_child(children[1], footer_constraints);
                cx.set_child_rect(
                    children[1],
                    RectF::new(0.0, self.header_height, fallback_width, self.footer_height),
                );
            }
            let size = Size::new(available_width, available_height);
            return LayoutResult::new(size, Size::new(available_width, 0.0));
        }

        // Body height: remaining space after header and footer.
        let body_height = (available_height - self.header_height - self.footer_height).max(0.0);
        self.body_rect = RectF::new(0.0, self.header_height, available_width, body_height);

        // Cache viewport height for keyboard page navigation.
        {
            let mut core = self.core.borrow_mut();
            core.viewport_height = body_height;
            core.cms.hts_mut().set_viewport_height(body_height);
        }

        // Layout ScrollArea child (body) with body constraints.
        // Position it at the body region (below header).
        //
        // INV-FINALITY-PROPAGATION: a tight height is the Stage-2.8 "final
        // pass" signal for the ScrollArea below — the only pass allowed to
        // commit scroll state. ListView must not fabricate it. When our own
        // incoming height is loose (Flex measurement bound, SPEC-PM-005 §7),
        // the forwarded height stays loose so the ScrollArea skips its
        // commit; the Flex allocation pass arrives height-tight and commits
        // normally. Fabricating tightness from the measurement committed a
        // second, wrong max_scroll every frame — last writer won, clamping
        // input 34px short and oscillating at the end stop (Workbench
        // pipeline table, 2026-06-12; PM-009 class). Width stays tight as
        // before: finality requires both axes, so the honest height alone
        // gates it (Flex allocation is main-axis tight only, flex.rs).
        //
        // Child order:
        // - children[0]: ScrollArea (body, required)
        // - children[1]: ListViewFooter (optional)
        if !children.is_empty() {
            let height_is_tight = (constraints.min_height - constraints.max_height).abs() < 0.01;
            let body_min_height = if height_is_tight { body_height } else { 0.0 };
            let body_constraints = Constraints::new(
                available_width,
                available_width,
                body_min_height,
                body_height,
            );
            cx.layout_child(children[0], body_constraints);

            // Position child at body region (y = header_height).
            let child_rect = RectF::new(0.0, self.header_height, available_width, body_height);
            cx.set_child_rect(children[0], child_rect);
        }

        // Layout footer child (if present).
        // Footer is positioned below body, fixed at bottom.
        if children.len() > 1 {
            let footer_y = self.header_height + body_height;
            let footer_constraints = Constraints::new(
                available_width,
                available_width,
                self.footer_height,
                self.footer_height,
            );
            cx.layout_child(children[1], footer_constraints);

            let footer_rect = RectF::new(0.0, footer_y, available_width, self.footer_height);
            cx.set_child_rect(children[1], footer_rect);
        }

        // ListView fills allocated space.
        let size = Size::new(available_width, available_height);
        LayoutResult::new(size, size)
    }

    fn paint(&self, cx: &mut PaintContext, _rect: RectF, children: &[NodeId]) {
        // Read animation-system-driven visual offsets and write to core for body to read.
        // Mirrors Button pattern: read cx.get_animation() in paint, cache values.
        // Must happen before the main immutable borrow of core.
        {
            if let ColumnDragState::Active {
                column_id: dragged_id,
                start_global_x,
                current_global_x,
                ..
            } = &self.drag_state
            {
                let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
                let handles = self.drag_slide_handles.borrow();
                let mut values = self.drag_slide_values.borrow_mut();
                let mut offsets = HashMap::with_capacity(self.drag_slide_targets.len());

                for (col_id, target) in &self.drag_slide_targets {
                    let visual_x = if let Some(handle) = handles.get(col_id) {
                        if let Some(value) = cx.get_animation::<f32>(*handle) {
                            value
                        } else {
                            *target // Animation finished
                        }
                    } else {
                        *target // No animation handle (snapped)
                    };
                    values.insert(*col_id, visual_x);
                    offsets.insert(*col_id, visual_x);
                }

                drop(handles);
                drop(values);

                // Compute ghost occlusion range in slot/content-space coordinates.
                // Used by header and body to suppress cell text under the ghost.
                let delta = current_global_x - start_global_x;
                let ghost_range = {
                    let core = self.core.borrow();
                    core.cms.hts().try_column_layout().and_then(|layout| {
                        layout
                            .header_slots()
                            .find(|s| s.column_id == *dragged_id)
                            .map(|slot| (slot.x + delta, slot.x + delta + slot.width))
                    })
                };

                let mut core = self.core.borrow_mut();
                core.drag_visual_offsets = Some(offsets);
                core.ghost_occlude_range = ghost_range;
            }
        }

        // Cache scale factor for body layout's geometry resolution.
        self.core.borrow_mut().scale_factor = cx.scene.scale_factor();

        // Children are painted by tree traversal. The fixed header is visual
        // chrome and is painted in paint_overlay() after the ScrollArea body.
        let _ = children;
    }

    fn paint_overlay(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _tree_widget_id: crate::utils::id::TreeWidgetId,
    ) {
        if !self.header_visible() {
            return;
        }

        let sf = cx.scene.scale_factor();
        let phys_px = 1.0 / sf;
        let config_overlay_open = {
            let core = self.core.borrow();
            self.paint_header_chrome(cx, rect, &core);
            core.config_overlay_open
        };

        // Column config overlay (if open).
        if config_overlay_open && !self.config_snapshot.is_empty() {
            // Anchor below config button at right edge of header.
            let anchor_x = (rect.x + rect.width - 180.0).max(rect.x);
            let anchor_y = rect.y + self.header_height;
            self.config_overlay.paint(
                cx,
                &self.config_snapshot,
                (anchor_x, anchor_y),
                _tree_widget_id,
                rect,
            );
        }

        // Drag-reorder: full-height ghost column (elevated, opaque, shadowed)
        if let ColumnDragState::Active {
            column_id,
            start_global_x,
            current_global_x,
            ..
        } = &self.drag_state
        {
            let core = self.core.borrow();
            let scroll_x = core
                .horizontal_scroll_link
                .as_ref()
                .map_or(0.0, |l| l.get());
            // SPEC-PM-017: f32 shim — rung 2.
            let scroll_y = core.cms.hts().scroll_offset() as f32;

            let base_x = rect.x.floor();
            let font_size = cx.theme.typography.font_size;
            let text_padding = cx.theme.list_geom.padding_x;

            // Find dragged column slot for width and original position.
            let dragged_slot = {
                let hts = core.cms.hts();
                let Some(column_layout) = hts.try_column_layout() else {
                    return;
                };
                column_layout
                    .header_slots()
                    .find(|s| s.column_id == *column_id)
                    .map(|slot| (slot.width, slot.scroll_domain, slot.x))
            };

            if let Some((ghost_width, scroll_domain, slot_x)) = dragged_slot {
                // Ghost X: original painted position + drag delta.
                let original_painted_x = if scroll_domain != ColumnScrollDomain::Scrollable {
                    base_x + slot_x
                } else {
                    base_x + slot_x - scroll_x
                };
                let delta = current_global_x - start_global_x;
                let ghost_x = original_painted_x + delta;

                // Full height: header + body + footer.
                let body_height = self.body_rect.height;
                let total_height = self.header_height + body_height + self.footer_height;
                let ghost_y = rect.y;
                let ghost_full_rect = RectF::new(ghost_x, ghost_y, ghost_width, total_height);

                // Drop shadow: offset dark rect behind the ghost.
                // Main pass, below ghost surface/background layers.
                let shadow_offset_y = 3.0;
                let shadow_spread = 4.0;
                let shadow_color = cx.theme.colors.surfaces.base0.with_alpha(0.35);
                let shadow_rect = RectF::new(
                    ghost_x - shadow_spread * 0.5,
                    ghost_y + shadow_offset_y,
                    ghost_width + shadow_spread,
                    total_height + shadow_spread * 0.5,
                );
                cx.scene
                    .push_rect(shadow_rect, shadow_color, DRAG_GHOST_SHADOW_LAYER, None);

                // Ghost background: fully opaque elevated overlay surface.
                let ghost_bg_color = cx.theme.colors.surfaces.level3;
                cx.scene.push_rect(
                    ghost_full_rect,
                    ghost_bg_color,
                    DRAG_GHOST_SURFACE_LAYER,
                    None,
                );

                // Ghost header region: slightly brighter tint.
                let ghost_header_rect =
                    RectF::new(ghost_x, ghost_y, ghost_width, self.header_height);
                cx.scene.push_rect(
                    ghost_header_rect,
                    cx.theme.colors.surfaces.level2,
                    DRAG_GHOST_ROW_LAYER,
                    None,
                );

                // Ghost header text.
                if let Some(title) = core.column_titles.get(column_id) {
                    cx.scene.begin_clip(ghost_header_rect, None);
                    let run = cx.text_engine.shape(title, font_size);
                    let text_y = ghost_y + (self.header_height - run.height) / 2.0;
                    cx.text_engine.draw(
                        &run,
                        ghost_x + text_padding,
                        text_y,
                        cx.theme.colors.typography.primary,
                        DRAG_GHOST_TEXT_LAYER,
                        cx.scene,
                    );
                    cx.scene.end_clip();
                }

                // Ghost body: row backgrounds + cell text (same pipeline as ListViewBody).
                let body_global_y = rect.y + self.header_height;
                let body_clip = RectF::new(ghost_x, body_global_y, ghost_width, body_height);
                cx.scene.begin_clip(body_clip, None);

                let realized_rows = core.visible_row_projections();
                let list_colors = &cx.theme.colors.list;
                let list_focused = core.focused;
                for row_projection in &realized_rows {
                    let row_viewport_y = body_global_y + row_projection.y_offset - scroll_y;
                    let row_height = row_projection.height;

                    // Skip rows outside body viewport.
                    if row_viewport_y + row_height < body_global_y
                        || row_viewport_y > body_global_y + body_height
                    {
                        continue;
                    }

                    let is_selected = core.is_selected(row_projection.row_id);
                    let is_focused_row = core.is_row_focused(row_projection.row_id);

                    // Row background: selection > zebra > normal (mirrors ListViewBody).
                    let bg_color = if is_selected {
                        if list_focused {
                            list_colors.row_selected
                        } else {
                            list_colors.row_selected_inactive
                        }
                    } else if row_projection.row_index % 2 == 1 {
                        list_colors.row_background_alt
                    } else {
                        list_colors.row_background
                    };
                    let row_rect = RectF::new(ghost_x, row_viewport_y, ghost_width, row_height);
                    cx.scene
                        .push_rect(row_rect, bg_color, DRAG_GHOST_ROW_LAYER, None);

                    // Focus border on focused row (overlay pass, above text).
                    if is_focused_row {
                        cx.scene.push_border_rect(
                            row_rect,
                            cx.theme.colors.borders.focus,
                            1.0,
                            DRAG_GHOST_BORDER_LAYER,
                            None,
                        );
                    }

                    let text_color = if is_selected {
                        list_colors.text_selected
                    } else {
                        list_colors.text
                    };

                    let cell_value = core.adapter.read_cell(row_projection.row_id, *column_id);
                    let (use_combo_box_proxy, text) =
                        Self::drag_cell_presentation(&core, *column_id, &cell_value);

                    if use_combo_box_proxy {
                        let control_rect = RectF::new(
                            ghost_x + text_padding,
                            row_viewport_y,
                            (ghost_width - text_padding * 2.0).max(0.0),
                            row_height,
                        );
                        Self::paint_combo_box_drag_proxy(cx, control_rect, &text);
                    } else if !text.is_empty() {
                        let cell_clip =
                            RectF::new(ghost_x, row_viewport_y, ghost_width, row_height);
                        cx.scene.begin_clip(cell_clip, None);
                        let run = cx.text_engine.shape(&text, font_size);
                        let text_y = row_viewport_y + (row_height - run.height) / 2.0;
                        cx.text_engine.draw(
                            &run,
                            ghost_x + text_padding,
                            text_y,
                            text_color,
                            DRAG_GHOST_TEXT_LAYER,
                            cx.scene,
                        );
                        cx.scene.end_clip();
                    }
                }

                cx.scene.end_clip(); // End body clip

                // Ghost border: strong accent (overlay pass, above text).
                cx.scene.push_border_rect(
                    ghost_full_rect,
                    cx.theme.colors.accents.primary,
                    2.0,
                    DRAG_GHOST_BORDER_LAYER,
                    None,
                );
            }
            drop(core);
        }

        let core = self.core.borrow();

        // INV-PAINT-HSCROLL-1: Read scroll_x for overlay separator alignment.
        let scroll_x = core
            .horizontal_scroll_link
            .as_ref()
            .map_or(0.0, |l| l.get());

        // PIXEL-SNAP: Floor the column base X so geometry-snapped column positions
        // land on pixel boundaries. Prevents sub-pixel jitter during window resize.
        let base_x = snap_to_physical(rect.x, sf);

        // =====================================================================
        // INV-SEP-Z-PRIORITY: Phase 3 - Hovered/Active separators (above border)
        //
        // Normal separators and border are painted by paint_header_chrome().
        // This overlay paints only hovered/active separators on top of everything.
        // =====================================================================

        let (pinned_width, sep_slots) = {
            let hts = core.cms.hts();
            let Some(column_layout) = hts.try_column_layout() else {
                return;
            };
            (
                column_layout.pinned_width(),
                column_layout.header_slots().collect::<Vec<_>>(),
            )
        };

        // INV-PAINT-HSCROLL-2: Clip header region in overlay to prevent overflow.
        let header_rect = RectF::new(rect.x, rect.y, rect.width, self.header_height);
        cx.scene.begin_clip(header_rect, None);

        // INV-SEPARATOR-PLACEMENT: Only between adjacent columns, not after last.
        let inter_column_slots = &sep_slots[..sep_slots.len().saturating_sub(1)];

        // Helper: paint a hovered/active separator.
        let paint_overlay_sep =
            |cx: &mut PaintContext, slot: &crate::widgets::tabular_view::geometry::ColumnSlot| {
                let is_active = core.active_resize_separator == Some(slot.column_id);
                let is_hovered = core.hovered_separator == Some(slot.column_id);
                if !is_active && !is_hovered {
                    return;
                }
                let sep_x = if slot.scroll_domain != ColumnScrollDomain::Scrollable {
                    snap_to_physical(base_x + slot.x + slot.width, sf) - phys_px
                } else {
                    snap_to_physical(base_x + slot.x + slot.width, sf) - phys_px - scroll_x
                };
                let (sep_width, sep_color) = if is_active {
                    (2.0 * phys_px, cx.theme.colors.borders.focus)
                } else {
                    (2.0 * phys_px, cx.theme.colors.borders.default)
                };
                let sep_rect = RectF::new(sep_x, rect.y, sep_width, self.header_height);
                cx.scene
                    .push_rect(sep_rect, sep_color, HEADER_INTERACTION_LAYER, None);
            };

        // Scrollable overlay separators: clip to scrollable area.
        {
            let scrollable_clip_x = base_x + pinned_width;
            let scrollable_clip_w = (rect.x + rect.width - scrollable_clip_x).max(0.0);
            let scrollable_clip = RectF::new(
                scrollable_clip_x,
                rect.y,
                scrollable_clip_w,
                self.header_height,
            );
            cx.scene.begin_clip(scrollable_clip, None);
            for slot in inter_column_slots
                .iter()
                .filter(|s| s.scroll_domain == ColumnScrollDomain::Scrollable)
            {
                paint_overlay_sep(cx, slot);
            }
            cx.scene.end_clip();
        }

        // Pinned overlay separators: paint in pinned area.
        for slot in inter_column_slots
            .iter()
            .filter(|s| s.scroll_domain != ColumnScrollDomain::Scrollable)
        {
            paint_overlay_sep(cx, slot);
        }

        cx.scene.end_clip();
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Preserve logical capture/bubble routing for descendant overlays without
        // treating their coordinates as ListView header or separator interaction.
        // ListView-owned overlay chrome still arrives here in Target phase.
        if let Event::Pointer(pe) = event {
            if cx.pointer_route_is_overlay() && phase != EventPhase::Target {
                if matches!(pe.kind, PointerEventKind::Move) {
                    self.clear_header_hover(cx);
                    if self.core.borrow().hovered_separator.is_some() {
                        self.core.borrow_mut().hovered_separator = None;
                        cx.invalidate_paint();
                    }
                }
                return false;
            }
        }

        // Route events to config overlay if open.
        if self.core.borrow().config_overlay_open {
            if let Some(action) = self
                .config_overlay
                .handle_event(event, &self.config_snapshot)
            {
                self.apply_config_action(action, cx);
                return true;
            }
        }

        match event {
            Event::Key(ke) => {
                // Keyboard only in Target phase.
                if phase != EventPhase::Target {
                    return false;
                }
                // Keyboard navigation handled here; scroll reveal happens in post_layout phase
                // via ListViewBody's take_requested_scroll_y() being checked in
                // post_layout_resolve_scroll_anchors().
                self.handle_keyboard(cx, ke)
            }
            Event::Pointer(pe) => {
                if matches!(pe.kind, PointerEventKind::Move)
                    && matches!(self.drag_state, ColumnDragState::Idle)
                    && matches!(self.resize_state, ResizeState::Idle)
                {
                    self.update_header_hover(cx, pe.global_pos);
                }
                // =========================================================================
                // Column Resize State Machine
                //
                // States: Idle → Armed → Dragging → Idle
                //
                // INV-RESIZE-OWNERSHIP: Only ListView mutates resize state or column widths.
                // INV-RESIZE-CAPTURE: Pointer capture exists only in Dragging state.
                // INV-RESIZE-EXCLUSIVE: While resize ≠ Idle, all row interaction is suppressed.
                // INV-RESIZE-DETERMINISTIC: Width deltas computed solely from pointer movement.
                // INV-RESIZE-CLEAN-EXIT: On finish or cancel, state is fully reset.
                // =========================================================================
                match pe.kind {
                    // =================================================================
                    // Move/DragStart: Drag-reorder state machine, then resize state machine
                    // =================================================================
                    PointerEventKind::Move | PointerEventKind::DragStart => {
                        // =============================================================
                        // Column Drag-Reorder: Armed → Active threshold, Active update.
                        //
                        // Checked before resize state machine. Drag and resize are
                        // mutually exclusive (drag arms only when resize is Idle).
                        // =============================================================
                        match &self.drag_state {
                            ColumnDragState::Active {
                                column_id,
                                start_global_x,
                                order_snapshot,
                                ..
                            } => {
                                // Update current_global_x
                                self.drag_state = ColumnDragState::Active {
                                    column_id: *column_id,
                                    start_global_x: *start_global_x,
                                    current_global_x: pe.global_pos.0,
                                    order_snapshot: order_snapshot.clone(),
                                };
                                // Retarget per-column slide animations via animation system.
                                self.update_drag_slide_targets(cx);
                                cx.request_cursor(crate::core::event::CursorIcon::Move);
                                return true;
                            }
                            ColumnDragState::Armed {
                                column_id,
                                start_global_x,
                                reorderable,
                                order_snapshot,
                                ..
                            } => {
                                let delta = (pe.global_pos.0 - start_global_x).abs();
                                if delta >= 4.0 {
                                    if !*reorderable {
                                        self.animate_header_pressed(cx, *column_id, false);
                                        cx.release_pointer();
                                        self.drag_state = ColumnDragState::Idle;
                                        return true;
                                    }
                                    let col_id = *column_id;
                                    self.drag_state = ColumnDragState::Active {
                                        column_id: col_id,
                                        start_global_x: *start_global_x,
                                        current_global_x: pe.global_pos.0,
                                        order_snapshot: order_snapshot.clone(),
                                    };
                                    self.animate_header_pressed(cx, col_id, false);
                                    // Set shared dragged column so body suppresses it during paint.
                                    {
                                        let mut core = self.core.borrow_mut();
                                        core.dragged_column_id = Some(col_id);
                                        // Freeze horizontal scroll during drag
                                        if let Some(ref freeze) = core.horizontal_scroll_freeze {
                                            freeze.set(true);
                                        }
                                    }
                                    if let Some(body_id) = self.core.borrow().body_widget_id {
                                        cx.invalidate_layout_by_id(body_id);
                                    }
                                    // Compute initial slide targets and start animations.
                                    self.update_drag_slide_targets(cx);
                                    cx.request_cursor(crate::core::event::CursorIcon::Move);
                                    return true;
                                }
                                // Below threshold: consume to maintain gesture ownership.
                                // Prevents child widgets from intercepting the drag gesture.
                                return true;
                            }
                            ColumnDragState::Idle => {} // Fall through to resize state machine
                        }

                        // =============================================================
                        // Column Resize State Machine
                        // =============================================================
                        match self.resize_state {
                            // --- DRAGGING: Update column width ---
                            // INV-RESIZE-CAPTURE-ROUTING: Captured events are routed directly
                            // to the capturing widget in TARGET phase (not Capture phase).
                            // See event_router.rs route_to_widget() which dispatches with Target.
                            ResizeState::Dragging {
                                column_id,
                                start_x,
                                initial_width,
                                rollback_width,
                            } => {
                                if phase == EventPhase::Capture || phase == EventPhase::Target {
                                    let (start_x, initial_width) = self.apply_resize_delta(
                                        column_id,
                                        start_x,
                                        initial_width,
                                        pe.global_pos.0,
                                    );
                                    self.resize_state = ResizeState::Dragging {
                                        column_id,
                                        start_x,
                                        initial_width,
                                        rollback_width,
                                    };
                                    cx.invalidate_layout();

                                    // Mark Body dirty so it re-resolves geometry with new column width.
                                    // INV-RESIZE-LAYOUT: Body.layout() calls resolve_layout_geometry
                                    // which re-resolves column geometry from preferred_width.
                                    if let Some(body_id) = self.core.borrow().body_widget_id {
                                        cx.invalidate_layout_by_id(body_id);
                                    }

                                    // INV-42G-3: Re-request continuous frames each frame while dragging.
                                    // Framework resets flag at frame start; widgets must re-request.
                                    cx.request_continuous_frames();

                                    // Maintain cursor during drag.
                                    cx.request_cursor(crate::core::event::CursorIcon::ColResize);
                                    return true; // Consume during resize.
                                }
                                false
                            }

                            // --- ARMED → DRAGGING: DragStart after arm ---
                            // INV-LV-RESIZE-ACTIVATION: Resize activation MUST happen via
                            // DragStart, not raw PointerMove. Move remains accepted here for
                            // direct primitive tests that bypass InputFusion.
                            // INV-RESIZE-TRANSITION: First DragStart/Move after arm MUST start
                            // dragging, regardless of EventPhase. In header, DragStart may
                            // arrive in Capture phase; gating on Target/Bubble would block it.
                            ResizeState::Armed {
                                column_id,
                                start_x,
                                initial_width,
                                rollback_width,
                            } => {
                                // Transition to Dragging and capture pointer.
                                // INV-RESIZE-CAPTURE: Capture only in Dragging state.
                                cx.capture_pointer(PointerButton::Primary);

                                // Apply initial delta (may be non-zero if mouse moved before capture).
                                let (start_x, initial_width) = self.apply_resize_delta(
                                    column_id,
                                    start_x,
                                    initial_width,
                                    pe.global_pos.0,
                                );
                                self.resize_state = ResizeState::Dragging {
                                    column_id,
                                    start_x,
                                    initial_width,
                                    rollback_width,
                                };
                                cx.invalidate_layout();

                                // Mark Body dirty so it re-resolves geometry with new column width.
                                if let Some(body_id) = self.core.borrow().body_widget_id {
                                    cx.invalidate_layout_by_id(body_id);
                                }

                                // INV-42I-SMOOTH-1: Request continuous frames for smooth drag.
                                cx.request_continuous_frames();

                                cx.request_cursor(crate::core::event::CursorIcon::ColResize);
                                true
                            }

                            // --- IDLE: Separator hover detection ---
                            ResizeState::Idle => {
                                if matches!(pe.kind, PointerEventKind::DragStart) {
                                    return false;
                                }

                                if phase != EventPhase::Target && phase != EventPhase::Bubble {
                                    return false;
                                }

                                // INV-HOVER-NO-CAPTURE: Skip hover during any capture.
                                if cx.has_pointer_capture() {
                                    return false;
                                }

                                // Suppress separator hover in expansion regions.
                                if self.core.borrow().pointer_in_expansion_region {
                                    self.core.borrow_mut().hovered_separator = None;
                                    return false;
                                }

                                let Some(rect) = cx.widget_global_rect() else {
                                    return false;
                                };

                                // Compute local X for hit-testing.
                                let local_x = pe.global_pos.0 - rect.x;

                                // Hit-test separator.
                                let hit_sep = self.hit_test_separator(local_x);

                                // Filter out non-resizable columns from hover.
                                let hit_sep =
                                    hit_sep.filter(|hit| self.separator_target_resizable(*hit));

                                // Update hover state in shared core.
                                self.core.borrow_mut().hovered_separator =
                                    hit_sep.map(|hit| hit.column_id);

                                // Request cursor if hovering separator.
                                if hit_sep.is_some() {
                                    cx.request_cursor(crate::core::event::CursorIcon::ColResize);
                                }

                                false // Don't consume - allow other handlers.
                            }
                        }
                    }

                    // =================================================================
                    // Down: Idle → Armed (Target + Bubble phases)
                    //
                    // INV-RESIZE-ARM-FORWARDING: PointerDown from body/footer bubbles
                    //   from ScrollArea. Handle in Bubble phase to enable resize arming
                    //   from anywhere along the column separator.
                    // =================================================================
                    PointerEventKind::Down if pe.button == Some(PointerButton::Primary) => {
                        // Only arm from Idle state.
                        if !matches!(self.resize_state, ResizeState::Idle) {
                            return false;
                        }

                        // Target + Bubble phases (mirrors hover detection pattern).
                        if phase != EventPhase::Target && phase != EventPhase::Bubble {
                            return false;
                        }

                        // INV-RESIZE-NO-ARM-DURING-CAPTURE: Cannot arm while capture exists.
                        if cx.has_pointer_capture() {
                            return false;
                        }

                        let Some(rect) = cx.widget_global_rect() else {
                            return false;
                        };

                        // Pin toggle: CMD+Click (macOS) / CTRL+Click on header/footer cell.
                        if Self::is_platform_pin_modifier(&pe.modifiers) {
                            let global_y = pe.global_pos.1;
                            let header_bottom = rect.y + self.header_height;
                            let footer_top = rect.y + rect.height - self.footer_height;
                            let in_header = global_y >= rect.y && global_y < header_bottom;
                            let in_footer = self.footer_height > 0.0 && global_y >= footer_top;

                            if in_header || in_footer {
                                let local_x = pe.global_pos.0 - rect.x;
                                if let Some(column_id) = self.hit_test_column(local_x) {
                                    let action = super::actions::ListViewAction::ToggleColumnPin {
                                        column_id,
                                    };
                                    let body_id = {
                                        let mut core = self.core.borrow_mut();
                                        super::actions::resolve(&action, &mut core);
                                        core.body_widget_id
                                    };
                                    if let Some(body_id) = body_id {
                                        cx.invalidate_layout_by_id(body_id);
                                    }
                                    return true;
                                }
                            }
                        }

                        // Config button hit test (right edge of header, 24x24).
                        {
                            let global_y = pe.global_pos.1;
                            let header_bottom = rect.y + self.header_height;
                            let in_header = global_y >= rect.y && global_y < header_bottom;
                            if in_header {
                                let btn_size = 24.0;
                                let btn_x = rect.x + rect.width - btn_size - 4.0;
                                let btn_y = rect.y + (self.header_height - btn_size) / 2.0;
                                if pe.global_pos.0 >= btn_x
                                    && pe.global_pos.0 <= btn_x + btn_size
                                    && pe.global_pos.1 >= btn_y
                                    && pe.global_pos.1 <= btn_y + btn_size
                                {
                                    let mut core = self.core.borrow_mut();
                                    core.config_overlay_open = !core.config_overlay_open;
                                    drop(core);
                                    self.config_snapshot =
                                        self.core.borrow().build_column_snapshot();
                                    return true;
                                }
                            }
                        }

                        if !self.core.borrow().pointer_in_expansion_region {
                            let local_x = pe.global_pos.0 - rect.x;
                            let hit_sep = self.hit_test_separator(local_x);

                            if let Some(hit) = hit_sep {
                                if self.separator_target_resizable(hit) {
                                    // Get resolved initial width for delta calculation.
                                    let Some(initial_width) = self.get_column_width(hit.column_id)
                                    else {
                                        return false;
                                    };

                                    // Transition to Armed state.
                                    // INV-RESIZE-CAPTURE: No capture yet - only in Dragging.
                                    self.resize_state = ResizeState::Armed {
                                        column_id: hit.column_id,
                                        start_x: pe.global_pos.0,
                                        initial_width,
                                        rollback_width: initial_width,
                                    };

                                    // Set shared visual state for separator highlight.
                                    // INV-RESIZE-HSCROLL-2: Freeze horizontal scroll during resize.
                                    {
                                        let mut core = self.core.borrow_mut();
                                        core.active_resize_separator = Some(hit.column_id);
                                        if let Some(ref freeze) = core.horizontal_scroll_freeze {
                                            freeze.set(true);
                                        }
                                    }

                                    // Maintain cursor.
                                    cx.request_cursor(crate::core::event::CursorIcon::ColResize);

                                    return true; // Consume - prevent row selection.
                                }
                            }
                        }

                        // Header cell action dispatch via input mapping layer.
                        // Modifier → action mapping is centralized in actions::map_header_click().
                        {
                            let global_y = pe.global_pos.1;
                            let header_bottom = rect.y + self.header_height;
                            let in_header = global_y >= rect.y && global_y < header_bottom;

                            if in_header {
                                let local_x = pe.global_pos.0 - rect.x;
                                if let Some(column_id) = self.hit_test_column(local_x) {
                                    if let Some(action) =
                                        super::actions::map_header_click(&pe.modifiers, column_id)
                                    {
                                        match action {
                                            super::actions::ListViewAction::BeginColumnReorder {
                                                column_id,
                                            } => {
                                                let (is_reorderable, is_sortable) = self
                                                    .core
                                                    .borrow()
                                                    .column_model()
                                                    .get(column_id)
                                                    .map_or((true, true), |column| {
                                                        (column.reorderable, column.sortable)
                                                    });
                                                let provider_sortable = self
                                                    .core
                                                    .borrow()
                                                    .sort_capability_for_column(column_id)
                                                    .is_allowed();
                                                if !is_reorderable
                                                    && !(is_sortable && provider_sortable)
                                                {
                                                    return false;
                                                }
                                                let order_snapshot: Vec<ColumnId> = self
                                                    .core
                                                    .borrow()
                                                    .column_model()
                                                    .columns()
                                                    .iter()
                                                    .map(|c| c.id)
                                                    .collect();
                                                self.drag_state = ColumnDragState::Armed {
                                                    column_id,
                                                    start_global_x: pe.global_pos.0,
                                                    reorderable: is_reorderable,
                                                    additive_sort: pe.modifiers.shift,
                                                    order_snapshot,
                                                };
                                                self.animate_header_pressed(cx, column_id, true);
                                                // Capture pointer immediately to prevent
                                                // platform gesture interception (e.g. macOS
                                                // title bar drag) from stealing the gesture.
                                                cx.capture_pointer(PointerButton::Primary);
                                                return true;
                                            }
                                            _ => {}
                                        }
                                    }
                                }
                            }
                        }

                        false
                    }

                    // =================================================================
                    // DoubleClick: Auto-size column to content
                    //
                    // INV-AUTOSIZE-VISIBLE: Only measures visible realized rows.
                    // INV-AUTOSIZE-SAME-PATH: Uses same apply_column_width path as drag.
                    // =================================================================
                    PointerEventKind::DoubleClick if pe.button == Some(PointerButton::Primary) => {
                        // Only auto-size from Idle or Armed state (not during drag).
                        if matches!(self.resize_state, ResizeState::Dragging { .. }) {
                            return false;
                        }

                        // Target + Bubble phases (same as Down handler).
                        if phase != EventPhase::Target && phase != EventPhase::Bubble {
                            return false;
                        }

                        let Some(rect) = cx.widget_global_rect() else {
                            return false;
                        };

                        let local_x = pe.global_pos.0 - rect.x;
                        let hit_sep = self.hit_test_separator(local_x);

                        if let Some(hit) = hit_sep {
                            let column_id = hit.column_id;
                            // Per-column resizable guard.
                            let is_resizable = self
                                .core
                                .borrow()
                                .column_model()
                                .get(column_id)
                                .map_or(true, |c| c.resizable);
                            if !is_resizable {
                                return false;
                            }

                            // Compute auto-size width from visible content.
                            let auto_width =
                                self.compute_auto_size_width(column_id, cx.text_engine());

                            // Apply via same path as drag resize.
                            self.apply_column_width(column_id, auto_width);
                            cx.invalidate_layout();

                            // Clear any armed state (double-click may arrive after single-click armed).
                            self.commit_resize();

                            // Mark Body dirty for layout update.
                            if let Some(body_id) = self.core.borrow().body_widget_id {
                                cx.invalidate_layout_by_id(body_id);
                            }

                            return true; // Consume.
                        }

                        false
                    }

                    // =================================================================
                    // Up: Commit resize and return to Idle
                    // =================================================================
                    PointerEventKind::Up if pe.button == Some(PointerButton::Primary) => {
                        // Check drag-reorder state first
                        if let ColumnDragState::Active { .. } = &self.drag_state {
                            if phase == EventPhase::Capture || phase == EventPhase::Target {
                                cx.release_pointer();
                                self.commit_drag_reorder(cx);
                                return true;
                            }
                        }
                        if let ColumnDragState::Armed {
                            column_id,
                            additive_sort,
                            ..
                        } = &self.drag_state
                        {
                            let column_id = *column_id;
                            let additive_sort = *additive_sort;
                            self.animate_header_pressed(cx, column_id, false);
                            cx.release_pointer();
                            self.drag_state = ColumnDragState::Idle;
                            let body_id = {
                                let mut core = self.core.borrow_mut();
                                core.request_sort_for_column(column_id, additive_sort);
                                core.body_widget_id
                            };
                            if let Some(body_id) = body_id {
                                cx.invalidate_layout_by_id(body_id);
                            }
                            cx.invalidate_layout();
                            return true;
                        }

                        match self.resize_state {
                            ResizeState::Dragging { .. } => {
                                // INV-RESIZE-CAPTURE-ROUTING: Captured events arrive in Target phase.
                                if phase == EventPhase::Capture || phase == EventPhase::Target {
                                    // Release capture and commit (width already applied).
                                    cx.release_pointer();
                                    self.commit_resize();
                                    return true;
                                }
                                false
                            }
                            ResizeState::Armed { .. } => {
                                // Up before any move: no-op click on separator.
                                // No capture to release, just return to Idle.
                                if phase == EventPhase::Target || phase == EventPhase::Bubble {
                                    self.finalize_resize();
                                    return true;
                                }
                                false
                            }
                            ResizeState::Idle => false,
                        }
                    }

                    _ => {
                        // Other pointer events: only Target phase, no special handling.
                        if phase != EventPhase::Target {
                            return false;
                        }
                        false
                    }
                }
            }
            Event::Text(_) => {
                // SPEC-HAL-176: TextInput is reserved for composed text destined for
                // focused input controls (TextField, TextArea). ListView is not an
                // input control. Every character keypress (in both Navigation and
                // Capture modes) is delivered to widgets via the paired Event::Key
                // dispatched from route_input_event's KeyDown branch — see
                // platform/window.rs Step 3 (lines 423-437) which emits the Key
                // event for any non-clipboard character keypress regardless of
                // mode, and core/app.rs:2606-2614 which restates SPEC-HAL-176.
                //
                // The previous synthesize-KeyEvent-from-Text path in this arm
                // caused double-dispatch: each printable keypress reached
                // handle_keyboard once via the real Key event AND once via the
                // synthetic Key reconstructed from the paired TextInput. With
                // the navigation policy treating j/k as row navigation, that
                // produced a two-row skip per physical press; with the policy
                // not treating j/k as navigation, it produced selection drift
                // (focus moved twice, selection only once). Both symptoms are
                // resolved by ignoring Event::Text here.
                false
            }
            _ => false,
        }
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        let changed = {
            let mut core = self.core.borrow_mut();
            let changed = !core.focused;
            core.focused = true;
            changed
        };
        if changed {
            self.invalidate_body_paint(cx);
        }
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        let changed = {
            let mut core = self.core.borrow_mut();
            let changed = core.focused;
            core.focused = false;
            changed
        };
        if changed {
            self.invalidate_body_paint(cx);
        }

        // INV-RESIZE-CLEAN-EXIT: Focus loss cancels resize.
        if self.is_resize_active() {
            if matches!(self.resize_state, ResizeState::Dragging { .. }) {
                cx.release_pointer();
            }
            self.cancel_resize();
        }

        // Cancel drag-reorder on focus loss.
        // Both Armed and Active hold pointer capture.
        if !matches!(self.drag_state, ColumnDragState::Idle) {
            cx.release_pointer();
            self.cancel_drag_reorder(cx);
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.clear_header_hover(cx);
    }

    fn capture_eligible(&self) -> bool {
        self.core.borrow().intent_policy.expansion_enabled
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        self.in_capture = true;
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        self.in_capture = false;
        // Clear selection when leaving capture mode (Escape or Tab away).
        // Return to clean navigation state.
        let mut core = self.core.borrow_mut();
        core.dispatch_intent(&TabularIntent::Selection(
            crate::widgets::tabular_view::intents::SelectionIntent::ClearSelection,
        ));
    }

    fn debug_type_name(&self) -> &'static str {
        "ListView"
    }
}

#[cfg(test)]
mod drag_proxy_tests {
    use super::*;
    use crate::widgets::tabular_view::data_adapter::{
        CellCapabilities, DataInvalidationEvent, Extent, UniverseMode,
    };

    struct ComboDragProxySource {
        rows: Vec<RowId>,
    }

    impl ComboDragProxySource {
        fn new() -> Self {
            Self {
                rows: vec![RowId::new()],
            }
        }
    }

    impl DataSource for ComboDragProxySource {
        fn row_ids(&self) -> &[RowId] {
            &self.rows
        }

        fn extent(&self) -> Extent {
            Extent::Exact(self.rows.len())
        }

        fn mode(&self) -> UniverseMode {
            UniverseMode::Live
        }

        fn read_cell(&self, _row: RowId, _column: ColumnId) -> ValueState<CellValue> {
            ValueState::Ready(CellValue::String("Alpha".to_string()))
        }

        fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
            CellCapabilities::default()
        }

        fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
            Vec::new()
        }
    }

    #[test]
    fn drag_ghost_internal_layers_are_strictly_ordered() {
        assert!(DRAG_GHOST_SHADOW_LAYER < DRAG_GHOST_SURFACE_LAYER);
        assert!(DRAG_GHOST_SURFACE_LAYER < DRAG_GHOST_ROW_LAYER);
        assert!(DRAG_GHOST_ROW_LAYER < DRAG_GHOST_CHROME_LAYER);
        assert!(DRAG_GHOST_CHROME_LAYER < DRAG_GHOST_TEXT_LAYER);
        assert!(DRAG_GHOST_TEXT_LAYER < DRAG_GHOST_BORDER_LAYER);
        assert_eq!(DRAG_GHOST_SHADOW_LAYER, LAYER_OVERLAY);
    }

    #[test]
    fn combo_backed_string_cells_use_combo_box_drag_proxy() {
        let mut list_view = ListView::new(ComboDragProxySource::new());
        let choice_col = list_view.add_column("Choice", 120.0);
        let plain_col = list_view.add_column("Plain", 120.0);
        list_view.set_column_combo_options(
            choice_col,
            vec![
                ("Alpha".to_string(), "Alpha".to_string()),
                ("Beta".to_string(), "Beta".to_string()),
            ],
        );

        let string_value = ValueState::Ready(CellValue::String("Alpha".to_string()));
        assert!(list_view.test_cell_uses_combo_box_drag_proxy(choice_col, &string_value));
        assert!(!list_view.test_cell_uses_combo_box_drag_proxy(plain_col, &string_value));

        let bool_value = ValueState::Ready(CellValue::Bool(true));
        assert!(!list_view.test_cell_uses_combo_box_drag_proxy(choice_col, &bool_value));
    }
}
