// =============================================================================
// CellMechanicsSubstrate — Cell mechanics atop a hierarchical substrate
// =============================================================================
//
// Substrate-level engine composing `HierarchicalTabularSubstrate` (HTS) and
// extending it with the cell-bearing-widget mechanics that PE v2 currently
// owns internally: realized-cell content cache, cell-paint dispatch,
// always-mounted-editor reconciliation, edit-session lifecycle,
// per-variant editor construction, cell-mode hit-test, cell-mode focus,
// edit-mode keyboard routing, permission enforcement, validation feedback,
// content-width measurement. Extraction lands incrementally; PE v2 internals
// migrate to read through CMS, then TreeView wires as the second consumer.
//
// Composition over subsumption: CMS owns an HTS field. HTS remains an
// independent engine for any consumer that wants topology + virtualization
// without cell mechanics. CMS is a higher-level substrate that adds the
// cell-mechanics layer on top.
//
// Generic parameter over substrate trait is the right end-state for when
// ListView (flat substrate) and TabularView consume CMS. V1 hard-codes HTS
// — premature generics carry an ergonomic cost. Generalization arrives
// with the third consumer.
//
// Tier note: CMS accepts a `LayoutContext` in measurement / realization
// methods (text shaping during measurement, realized-cell population) and
// a `PaintContext` in paint-dispatch methods (per-variant cell rendering).
// CMS does not define new paint primitives — it dispatches through
// existing scene + text-engine APIs. Layout state mutation is layout-time
// only; paint methods are `&self` read-only.
//
// INV-CMS-001 (amended, SPEC-HAL-337 §2): CMS holds the SharedHts handle to
//              the one topology instance. All HTS access from CMS consumers
//              routes through `hts()` / `hts_mut()` (frame-ephemeral guards,
//              INV-337-A1) or `hts_handle()`.
// INV-CMS-002: No NEW paint primitives, no pointer or gesture handling, and
//              no keyboard authority. CMS dispatches through existing
//              `PaintContext` APIs. Pointer and gesture interpretation, and
//              all keyboard authority — capture, mode transitions, focus, and
//              Escape/Tab release — remain with consumer widget bodies and the
//              framework (SPEC-HAL-328). CMS MAY derive cell-scoped keyboard
//              intent: mapping a key to a cell-edit trigger and reporting the
//              disposition for the consumer to act on. CMS never owns the
//              keyboard, capture, or mode.
// INV-CMS-003: Content-width cache invalidates on cell-source revision
//              change or scale_factor change. Sentinel initial values
//              (`u64::MAX` / `f32::NAN`) force first-pass measurement after
//              cell-source registration.

use std::cell::{Ref, RefCell, RefMut};
use std::collections::{HashMap, HashSet, VecDeque};
use std::rc::Rc;

use crate::core::composite_text_edit::{CompositeEditOutcome, CompositeEditorId};
use crate::core::icon::IconId;
use crate::core::layout::Constraints;
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{LayoutContext, PaintContext, Widget};
use crate::core::widget_tree::{NodeId, WidgetKey};
use crate::engines::cell_source::{
    Cell, CellEditorFactory, CellSource, ComboBoxCellEditorSpec, EditExitReason, PermissionAction,
    PermissionQuery, PermissionResult, PropertyValue, RowArchetype, Severity,
};
use crate::engines::hierarchical::{HierarchicalTabularSubstrate, SharedHts, VisibleRow};
use crate::model::tabular::column::ColumnId;
use crate::utils::id::TreeWidgetId;
use crate::widgets::tabular_view::data_adapter::OrderRevision;

/// One explicitly requested cell choice role, independent of recyclable node slots.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct CompositeChoiceRoleKey {
    pub row_id: u64,
    pub column_id: ColumnId,
}

/// Pure layout output. The engine attaches the publishing body's concrete mount.
#[derive(Debug, Clone)]
pub(crate) struct CompositeChoiceRole {
    pub key: CompositeChoiceRoleKey,
    pub widget_key: WidgetKey,
    pub rect: RectF,
}

#[derive(Debug, Clone)]
pub(crate) struct CompositeChoiceManifest {
    pub provider_revision: OrderRevision,
    pub roles: Vec<CompositeChoiceRole>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositeChoiceReadiness {
    Pending,
    Denied,
    Error,
    Missing,
    PolicyDenied,
    Stale,
    Unavailable,
}

pub(crate) enum CompositeChoiceAdmission {
    Ready(ComboBoxCellEditorSpec),
    NonReady(CompositeChoiceReadiness),
    Retired,
}

/// Current Phase-2 facts for an exact mount in the data owner's bounded set.
pub(crate) struct CompositeChoiceMountFacts {
    pub target: CompositeEditorId,
    pub pinned: bool,
}

/// Phase-2 work is consumed immediately after the data-owner borrow ends.
pub(crate) enum CompositeChoiceWork {
    Mount {
        role: CompositeChoiceRoleKey,
        key: WidgetKey,
        widget: Box<dyn Widget>,
    },
    Sync {
        role: CompositeChoiceRoleKey,
        target: CompositeEditorId,
        spec: ComboBoxCellEditorSpec,
        factory: Rc<dyn CellEditorFactory>,
    },
    Remove {
        role: CompositeChoiceRoleKey,
        node: NodeId,
        owner: TreeWidgetId,
        target: CompositeEditorId,
    },
    NonReady {
        role: CompositeChoiceRoleKey,
        target: Option<CompositeEditorId>,
        reason: CompositeChoiceReadiness,
    },
}

pub(crate) enum CompositeChoiceSettlement {
    Mounted {
        role: CompositeChoiceRoleKey,
        node: NodeId,
        owner: TreeWidgetId,
        target: CompositeEditorId,
    },
    Synced {
        role: CompositeChoiceRoleKey,
        outcome: CompositeEditOutcome,
    },
    Removed {
        role: CompositeChoiceRoleKey,
    },
    Rejected {
        role: CompositeChoiceRoleKey,
        outcome: CompositeEditOutcome,
    },
    NonReady {
        role: CompositeChoiceRoleKey,
        reason: CompositeChoiceReadiness,
    },
}

/// Private data-owner hook; it grants no tree, event, or peer-widget access.
pub(crate) trait CompositeChoiceOwner {
    fn mounted_composite_choice_targets(&self) -> Vec<(CompositeChoiceRoleKey, CompositeEditorId)>;

    fn collect_composite_choice_work(
        &mut self,
        manifest: Option<CompositeChoiceManifest>,
        mounts: &HashMap<CompositeChoiceRoleKey, CompositeChoiceMountFacts>,
    ) -> Vec<CompositeChoiceWork>;

    fn settle_composite_choice_work(&mut self, settlement: CompositeChoiceSettlement);
}

struct MountedCompositeChoice {
    node: NodeId,
    owner: TreeWidgetId,
    target: CompositeEditorId,
    ready: bool,
}

impl MountedCompositeChoice {
    fn remove(&self, role: CompositeChoiceRoleKey) -> CompositeChoiceWork {
        CompositeChoiceWork::Remove {
            role,
            node: self.node,
            owner: self.owner,
            target: self.target.clone(),
        }
    }
}

/// Hit-test slop for column separator detection (pixels, each side).
/// PointerDown within this distance of a separator's centerline bubbles
/// to the consumer's resize-arming path (INV-RESIZE-ARM-FORWARDING).
const SEPARATOR_HIT_HALF_WIDTH: f32 = 3.0;

/// SPEC-HAL-260A §10.1: the cell-mode cursor. A single variant value
/// encoding both the row-axis projection of selection focus and the
/// column-axis position in one closed type.
///
/// - `None`: no cursor.
/// - `Cell { row_id, col_id }`: cell-mode focus on a data row.
///   INV-CELL-FOCUS-WELLFORMED guarantees `col_id` is navigable on the
///   row (consumer-enforced).
/// - `Band { row_id }`: row-level focus (no column). Used by consumers
///   that have whole-row focus targets (e.g. PE's Section / SubSection
///   bands). Consumers without band rows leave this variant unused.
///
/// Constructor seams live in the consumer (e.g. PE's `set_focus_row` and
/// `apply_column_intent`). Direct mutation of `cms.focus` outside the
/// consumer's funnel is a funnel bypass and forbidden by the consumer's
/// invariants (PE: INV-PE-INTENT-FUNNEL).
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum PeFocus {
    None,
    Cell { row_id: u64, col_id: ColumnId },
    Band { row_id: u64 },
}

impl PeFocus {
    /// Column position when focused on a cell. `None` for `Band` and
    /// `PeFocus::None`.
    pub(crate) fn col_id(&self) -> Option<ColumnId> {
        match self {
            PeFocus::Cell { col_id, .. } => Some(*col_id),
            PeFocus::Band { .. } | PeFocus::None => None,
        }
    }

    /// Row identity when focused. `None` only on `PeFocus::None`.
    #[allow(
        dead_code,
        reason = "Retained row-axis identity projection for cell and band focus"
    )]
    pub(crate) fn row_id(&self) -> Option<u64> {
        match self {
            PeFocus::Cell { row_id, .. } | PeFocus::Band { row_id } => Some(*row_id),
            PeFocus::None => None,
        }
    }
}

/// Active edit session — one cell currently being edited via a
/// mount-on-demand editor child. INV-PE-SINGLE-EDIT: at most one session
/// is active across CMS state at any time.
///
/// `child_widget_id` and `editor_node_id` are populated by
/// `reconcile_edit_session` immediately after `cx.tree.add_child` returns.
/// Both are `None` while the session is in the request-pending state
/// (between an `EditIntent::Begin` and the next layout pass that mounts
/// the editor).
pub struct EditSession {
    pub row_id: u64,
    pub column_id: ColumnId,
    /// Editor's widget id, set after the editor is mounted.
    pub child_widget_id: Option<TreeWidgetId>,
    /// Editor's tree node id, set after the editor is mounted. Used by
    /// `reconcile_edit_session_owned` to find and remove the editor child when
    /// the session ends (INV-PE-TREE-DEFERRED).
    pub editor_node_id: Option<NodeId>,
}

/// A recorded editor exit, queued by the substrate-owned sink and drained
/// by the consumer. Pairs the edited cell's identity with how the editor
/// exited and the value it carried. The consumer maps each record to its
/// own intent in its layout pass (see `drain_edit_outcomes`).
pub struct EditOutcome {
    pub row: u64,
    pub col: ColumnId,
    pub reason: EditExitReason,
    pub value: Option<PropertyValue>,
}

/// CellMechanicsSubstrate — composes HTS, adds cell-mechanics state.
pub struct CellMechanicsSubstrate {
    hts: SharedHts,

    /// Universal cell-access handle. Cloned and pushed in by the consumer
    /// widget when it registers a data source. Read by `prepare_layout`
    /// for content-width measurement (SPEC-HAL-180) and by future
    /// realization / paint / edit machinery.
    cell_source: Option<Rc<RefCell<dyn CellSource>>>,

    /// Content-width cache: data revision at last measurement. INV-CMS-003.
    /// Sentinel `u64::MAX` forces a first-pass measurement on first
    /// `prepare_layout` after cell-source registration.
    content_width_cache_revision: u64,

    /// Content-width cache: scale factor at last measurement. INV-CMS-003.
    /// Sentinel `f32::NAN` forces a first-pass measurement on first
    /// `prepare_layout` after cell-source registration. NaN compared by `!=`
    /// is always true, which is the intended behavior.
    content_width_cache_scale: f32,

    /// Per-layout realized cell content for the current visible window.
    ///
    /// Populated by `realize_cell_values()` at layout time; read by
    /// `realized_cell_value()` at paint time. Cleared at the start of every
    /// `realize_cell_values` call (SPEC-PM-016-R4).
    ///
    /// Keyed by stable `(node_id, column_id)` — virtualization shifts the
    /// visible window but identity is invariant (SPEC-PM-016-R3). Paint
    /// reads only this map and never re-queries the cell source
    /// (SPEC-PM-016-R1 / R2).
    realized_cell_values: HashMap<(u64, ColumnId), Option<Cell>>,

    /// Always-mounted Bool editor (Checkbox) NodeIds keyed by
    /// `(node_id, column_id)`. Mounted when the cell is `Bool` AND visible
    /// AND `cell.editable` AND `permission_check(Edit) == Permit`;
    /// unmounted otherwise. Reconciled every layout pass by
    /// `reconcile_always_mounted_editors`.
    ///
    /// SPEC-HAL-260A §6.2 (Bool cells get an always-mounted Checkbox).
    /// SPEC-HAL-260A §7.3 (cell_widget_key bit-packing for child identity).
    always_mounted_editors: HashMap<(u64, ColumnId), NodeId>,

    /// Always-mounted ComboBox editor NodeIds keyed by `(node_id, column_id)`.
    /// Consumer-owned column configuration decides which string cells mount
    /// combo editors; CMS owns tree reconciliation and geometry.
    always_mounted_combo_box_editors: HashMap<CompositeChoiceRoleKey, MountedCompositeChoice>,
    composite_choice_desired: HashMap<CompositeChoiceRoleKey, CompositeChoiceRole>,
    composite_choice_revision: Option<OrderRevision>,
    composite_choice_waiting_manifest: HashSet<CompositeChoiceRoleKey>,
    composite_choice_readiness: HashMap<CompositeChoiceRoleKey, CompositeChoiceReadiness>,

    /// Active edit-session state (SPEC-HAL-260A §8.1). State truth: this
    /// field is set/cleared by the consumer's intent funnel and read by
    /// the substrate's `reconcile_edit_session` to drive the mount /
    /// unmount transition. `Some` while a session is active;
    /// `child_widget_id` and `editor_node_id` populate during the layout
    /// pass that mounts the editor.
    edit_session: Option<EditSession>,

    /// Tree truth: NodeId of the currently-mounted editor child, if any.
    /// Paired with `current_edit_cell` — both are `Some` together or
    /// `None` together. Used by `reconcile_edit_session` to drive the
    /// state machine. Read by `is_edit_cell` for paint-time suppression
    /// of inline cell content on the cell that hosts the editor.
    current_edit_node: Option<NodeId>,

    /// Tree truth: `(row_id, column_id)` of the cell whose editor is
    /// mounted as `current_edit_node`. INV-PE-TREE-DEFERRED — during the
    /// gap between a commit/cancel event (which clears `edit_session`
    /// synchronously) and the next layout pass that removes the editor
    /// child, `edit_session` is `None` but `current_edit_cell` still
    /// names the cell. `is_edit_cell` reads BOTH so consumers correctly
    /// suppress inline rendering across that gap.
    current_edit_cell: Option<(u64, ColumnId)>,

    /// Cell-mode cursor (SPEC-HAL-260A §8.1 / §10.1). The consumer's
    /// intent funnel reads via `focus()` and writes via `set_focus()`.
    /// Direct construction outside the funnel is a bypass and forbidden
    /// by the consumer's invariants (PE: INV-PE-INTENT-FUNNEL).
    focus: PeFocus,

    /// Column-preservation cache (SPEC-HAL-260A §10.2). Tracks the most
    /// recently focused data column. Consulted by the consumer's row-
    /// focus resolution when entering a row whose prior column is not
    /// navigable on the new row; written on every data-column
    /// transition.
    last_data_column: Option<ColumnId>,

    /// Editor factory for the substrate-owned edit path. Registered once by
    /// the consumer via `set_editor_factory`. `None` until registered; the
    /// owned reconcile path cannot mount an editor without it.
    editor_factory: Option<Rc<dyn CellEditorFactory>>,

    /// Edit-outcome queue. The substrate-owned editor sink pushes an
    /// `EditOutcome` here on commit / cancel; the consumer drains it via
    /// `drain_edit_outcomes`. Shared handle (`Rc`): the editor's callback
    /// holds a clone across frames, so it can push without borrowing the
    /// substrate.
    edit_outcomes: Rc<RefCell<VecDeque<EditOutcome>>>,
}

impl CellMechanicsSubstrate {
    /// Construct a new CMS over the given shared topology handle
    /// (SPEC-HAL-337 §2) with an empty content-width cache (sentinel
    /// values force first-pass measurement).
    pub fn new(hts: SharedHts) -> Self {
        Self {
            hts,
            cell_source: None,
            content_width_cache_revision: u64::MAX,
            content_width_cache_scale: f32::NAN,
            realized_cell_values: HashMap::new(),
            always_mounted_editors: HashMap::new(),
            always_mounted_combo_box_editors: HashMap::new(),
            composite_choice_desired: HashMap::new(),
            composite_choice_revision: None,
            composite_choice_waiting_manifest: HashSet::new(),
            composite_choice_readiness: HashMap::new(),
            edit_session: None,
            current_edit_node: None,
            current_edit_cell: None,
            focus: PeFocus::None,
            last_data_column: None,
            editor_factory: None,
            edit_outcomes: Rc::new(RefCell::new(VecDeque::new())),
        }
    }

    /// Read access to the composed HTS. Frame-ephemeral guard (INV-337-A1).
    pub fn hts(&self) -> Ref<'_, HierarchicalTabularSubstrate> {
        self.hts.borrow()
    }

    /// Mutable access to the composed HTS. Frame-ephemeral guard
    /// (INV-337-A1). Consumer widgets that need to mutate HTS (set columns,
    /// set tree data, set viewport height, etc.) route through this accessor.
    pub fn hts_mut(&self) -> RefMut<'_, HierarchicalTabularSubstrate> {
        self.hts.borrow_mut()
    }

    /// Clone of the shared topology handle (SPEC-HAL-337 §2).
    pub fn hts_handle(&self) -> SharedHts {
        Rc::clone(&self.hts)
    }

    /// Register the cell source. Setting `None` clears the source; any
    /// previously-measured elastic content widths remain in HTS until
    /// the next `prepare_layout` runs.
    pub fn set_cell_source(&mut self, source: Option<Rc<RefCell<dyn CellSource>>>) {
        self.cell_source = source;
    }

    /// Register the editor factory used by the substrate-owned edit path.
    /// The consumer sets this once at setup.
    pub fn set_editor_factory(&mut self, factory: Box<dyn CellEditorFactory>) {
        self.editor_factory = Some(Rc::from(factory));
    }

    /// Drain all queued edit outcomes, clearing the queue. The consumer
    /// calls this in its layout pass and maps each outcome to its own
    /// intent.
    pub fn drain_edit_outcomes(&mut self) -> Vec<EditOutcome> {
        self.edit_outcomes.borrow_mut().drain(..).collect()
    }

    /// Layout-phase preparation. Runs content-width measurement when the
    /// cache is stale (revision change or scale change), then resolves HTS
    /// column geometry. Consumer bodies call this once per layout pass at
    /// the composite level (PE Outer / future TreeView Outer), BEFORE any
    /// child layout that depends on `column_layout()` / `total_width()`.
    ///
    /// SPEC-HAL-180 (content-aware elastic columns): measurement writes
    /// per-column max widths to `hts.set_column_content_width`; the
    /// resolver uses them as the elastic floor.
    /// SPEC-PM-016-R5: `CellSource::cell()` is O(1) per call.
    pub fn prepare_layout(
        &mut self,
        cx: &mut LayoutContext<'_>,
        available_width: f32,
        scale_factor: f32,
    ) {
        let needs_measurement = {
            let current_rev = self
                .cell_source
                .as_ref()
                .map(|cs| cs.borrow().revision())
                .unwrap_or(0);
            current_rev != self.content_width_cache_revision
                || scale_factor != self.content_width_cache_scale
        };
        if needs_measurement {
            self.measure_elastic_content_widths(cx, scale_factor);
        }
        self.hts
            .borrow_mut()
            .resolve_geometry(available_width, scale_factor);
    }

    /// Measure max content width per elastic column by shaping every
    /// cell's `display_text` at the consumer's font/scale, then write
    /// results to `hts.set_column_content_width`.
    ///
    /// Visual-domain measurement (font, padding, swatch geometry) lives
    /// here in the substrate, not in the data source — Constitution
    /// Pillar D (data sources are pure data).
    ///
    /// Iteration shape: O(rows × elastic_columns). Acceptable because
    /// measurement is gated by the cache and runs only on data-revision
    /// or scale changes — not on every layout pass.
    fn measure_elastic_content_widths(&mut self, cx: &mut LayoutContext<'_>, scale_factor: f32) {
        let font_size = cx.theme.typography.font_size;
        let data_text_padding = cx.theme.geometry.text_field.padding_x;

        let elastic_columns: Vec<ColumnId> = self
            .hts
            .borrow()
            .column_model()
            .columns()
            .iter()
            .filter(|c| c.flex.is_some() && c.visible)
            .map(|c| c.id)
            .collect();

        let current_revision = self
            .cell_source
            .as_ref()
            .map(|cs| cs.borrow().revision())
            .unwrap_or(0);

        if elastic_columns.is_empty() {
            // No elastic columns — clear cache markers so a future
            // elastic-column registration triggers measurement on the
            // first frame after registration.
            self.content_width_cache_revision = u64::MAX;
            self.content_width_cache_scale = f32::NAN;
            return;
        }

        let Some(source_rc) = self.cell_source.clone() else {
            // No cell source — nothing to measure. Clear elastic content
            // widths (revert to flex_share behavior).
            for col_id in &elastic_columns {
                self.hts
                    .borrow_mut()
                    .set_column_content_width(*col_id, None);
            }
            return;
        };

        let row_height = self.hts.borrow().row_height();
        // Color-cell swatch + gap. Matches per-variant paint geometry
        // (see TreeView dispatch and future shared paint).
        let swatch_overhead = (row_height - 6.0).max(0.0) + 4.0;

        let mut max_per_column: std::collections::HashMap<ColumnId, f32> =
            elastic_columns.iter().map(|id| (*id, 0.0_f32)).collect();

        // Borrow the cell source once; iterate node_ids via topology.
        // CMS does not require `PropertyDataSource::nodes()` — it reads row
        // identity from the substrate it already owns. This keeps the
        // `CellSource` contract universal across non-PE consumers.
        {
            let source = source_rc.borrow();
            let hts = self.hts.borrow();
            let topology = hts.topology();
            let node_count = topology.node_count();
            for i in 0..node_count {
                let node_id = topology.node_id(i);
                for col_id in &elastic_columns {
                    let cell_opt = source.cell(node_id, *col_id);
                    if let Some(cell) = cell_opt {
                        let display = cell.value.display_text();
                        if display.is_empty() {
                            continue;
                        }
                        let run = cx.text_engine.shape(&display, font_size);
                        let text_overhead = 2.0 * data_text_padding;
                        let cell_swatch = if matches!(cell.value, PropertyValue::Color(_)) {
                            swatch_overhead
                        } else {
                            0.0
                        };
                        let total = run.width + text_overhead + cell_swatch;
                        if let Some(slot) = max_per_column.get_mut(col_id) {
                            if total > *slot {
                                *slot = total;
                            }
                        }
                    }
                }
            }
        }

        // Write measurements + update cache markers.
        for (col_id, max_width) in &max_per_column {
            let value = if *max_width > 0.0 {
                Some(*max_width)
            } else {
                None
            };
            self.hts
                .borrow_mut()
                .set_column_content_width(*col_id, value);
        }
        self.content_width_cache_revision = current_revision;
        self.content_width_cache_scale = scale_factor;
    }

    /// Realize per-cell content for the current visible window.
    ///
    /// Iterates `visible_rows × body_slots()` and stores each cell's
    /// `Option<Cell>` keyed by `(node_id, column_id)`. Paint reads via
    /// `realized_cell_value()` and never re-queries the source.
    ///
    /// SPEC-PM-016-R1: realization produces geometry AND content.
    /// SPEC-PM-016-R3: keyed by stable `(node_id, column_id)`.
    /// SPEC-PM-016-R4: cache cleared at the start of the call.
    /// SPEC-PM-016-R5: `cell_source.cell()` is O(1) per call (consumer
    ///                 responsibility).
    ///
    /// Iterates every column slot — including tree-kind / NonData columns.
    /// `CellSource::cell()` returns `None` for non-data columns by contract,
    /// so the cache holds `None` entries for those; consumers reading
    /// cell values for tree columns get `None` and fall back to their own
    /// non-cell content (e.g. `VisibleRow.label`).
    pub fn realize_cell_values(&mut self, visible_rows: &[VisibleRow]) {
        self.realized_cell_values.clear();

        let Some(source_rc) = self.cell_source.clone() else {
            return;
        };

        let column_ids: Vec<ColumnId> = self
            .hts
            .borrow()
            .column_layout()
            .body_slots()
            .map(|s| s.column_id)
            .collect();

        let source = source_rc.borrow();
        for vr in visible_rows {
            for col_id in &column_ids {
                let cell_opt = source.cell(vr.node_id, *col_id);
                self.realized_cell_values
                    .insert((vr.node_id, *col_id), cell_opt);
            }
        }
    }

    /// Realize per-cell content using a consumer-provided resolver.
    ///
    /// Same cache semantics as `realize_cell_values`, but lets consumers whose
    /// source contract is not `CellSource` feed CMS without duplicating paint
    /// mechanics.
    pub fn realize_cell_values_with<F>(&mut self, visible_rows: &[VisibleRow], mut resolve: F)
    where
        F: FnMut(u64, ColumnId) -> Option<Cell>,
    {
        self.realized_cell_values.clear();

        let column_ids: Vec<ColumnId> = self
            .hts
            .borrow()
            .column_layout()
            .body_slots()
            .map(|s| s.column_id)
            .collect();

        for vr in visible_rows {
            for col_id in &column_ids {
                let cell_opt = resolve(vr.node_id, *col_id);
                self.realized_cell_values
                    .insert((vr.node_id, *col_id), cell_opt);
            }
        }
    }

    /// Read a realized cell for paint-time consumption. Returns `None` if
    /// the cell is outside the current visible window OR the source
    /// returned `None` at realization time (e.g. tree-kind / NonData
    /// columns). Paint MUST NOT call the cell source directly.
    pub fn realized_cell_value(&self, node_id: u64, column_id: ColumnId) -> Option<&Cell> {
        self.realized_cell_values
            .get(&(node_id, column_id))
            .and_then(|opt| opt.as_ref())
    }

    /// Render one data cell's content inside its rect. Per-variant
    /// dispatch on `cell.value`:
    ///
    ///   - `Color(c)`: small swatch + `display_text` (hex string).
    ///   - `Bool(b)`: rect outline + check icon when `true`. NO text —
    ///     consumers that mount a Checkbox child for Bool cells gate
    ///     this call upstream and never reach this arm.
    ///   - `Badge(label, severity)`: severity-colored text. No swatch,
    ///     no pill background at this stage; the variant signature
    ///     supports a future pill-background renderer upgrade without
    ///     a signature change.
    ///   - `String / Integer / Float / Choice / KeyBinding / ReadOnly`:
    ///     `display_text` rendered with the cell's value color.
    ///
    /// Color resolution (universal):
    ///   - `cell.editable == false` OR `PropertyValue::ReadOnly(_)`:
    ///     `theme.colors.typography.secondary`.
    ///   - `is_selected == true` (and not read-only):
    ///     `theme.colors.list.text_selected`.
    ///   - Otherwise: `theme.colors.list.text`.
    ///
    /// `Badge` severity overrides the universal color for the four
    /// semantic roles (Success / Warning / Error / Info); `Severity::Neutral`
    /// falls back to the universal color.
    ///
    /// `cell_rect` is the full cell rectangle in widget-local coordinates.
    /// Padding inset uses `theme.geometry.text_field.padding_x` so data
    /// cells align horizontally with editor children (TextField / ComboBox
    /// inner / etc.) — consumers SHOULD register data columns with this
    /// inset assumption.
    pub fn paint_one_cell(
        &self,
        cx: &mut PaintContext<'_>,
        cell: &Cell,
        cell_rect: RectF,
        is_selected: bool,
    ) {
        let theme = cx.theme;
        let font_size = theme.typography.font_size;
        let data_text_padding = theme.geometry.text_field.padding_x;
        let row_height = cell_rect.height;
        let cell_x = cell_rect.x;
        let row_y = cell_rect.y;

        let is_readonly = !cell.editable || matches!(cell.value, PropertyValue::ReadOnly(_));
        let value_color = if is_readonly {
            theme.colors.typography.secondary
        } else if is_selected {
            theme.colors.list.text_selected
        } else {
            theme.colors.list.text
        };

        cx.scene.begin_clip(cell_rect, None);

        let cell_inner_x = cell_x + data_text_padding;
        let mut content_x = cell_inner_x;
        let mut render_text = true;

        match &cell.value {
            PropertyValue::Color(c) => {
                let swatch_size = (row_height - 6.0).max(0.0);
                let swatch_y = row_y + (row_height - swatch_size) / 2.0;
                cx.scene.push_rect(
                    RectF::new(cell_inner_x, swatch_y, swatch_size, swatch_size),
                    *c,
                    LAYER_CONTENT,
                    None,
                );
                cx.scene.push_border_rect(
                    RectF::new(cell_inner_x, swatch_y, swatch_size, swatch_size),
                    theme.colors.borders.subtle,
                    1.0,
                    LAYER_CONTENT + 1,
                    None,
                );
                content_x = cell_inner_x + swatch_size + 4.0;
            }
            PropertyValue::Bool(b) => {
                let box_size = (row_height - 8.0).max(0.0);
                let box_x = cell_inner_x;
                let box_y = row_y + (row_height - box_size) / 2.0;
                cx.scene.push_border_rect(
                    RectF::new(box_x, box_y, box_size, box_size),
                    theme.colors.borders.default,
                    1.0,
                    LAYER_CONTENT,
                    None,
                );
                if *b {
                    if let Some(def) = cx.icon_registry.get(IconId::CHECK) {
                        cx.scene.push_icon(
                            def,
                            box_x,
                            box_y,
                            box_size,
                            value_color,
                            LAYER_CONTENT + 1,
                        );
                    }
                }
                render_text = false;
            }
            PropertyValue::Badge(label, severity) => {
                let badge_color = match severity {
                    Severity::Neutral => value_color,
                    Severity::Success => theme.colors.semantic.success,
                    Severity::Warning => theme.colors.semantic.warning,
                    Severity::Error => theme.colors.semantic.error,
                    Severity::Info => theme.colors.semantic.info,
                };
                let run = cx.text_engine.shape(label, font_size);
                let metrics = cx.text_engine.font_metrics(font_size);
                let baseline_y = row_y
                    + compute_control_text_baseline_y(
                        row_height,
                        metrics.ascent,
                        metrics.descent,
                        theme.typography.baseline_offset,
                    );
                let text_y = baseline_y - metrics.ascent;
                cx.text_engine.draw(
                    &run,
                    cell_inner_x,
                    text_y,
                    badge_color,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
                render_text = false;
            }
            PropertyValue::String(_)
            | PropertyValue::Integer(_)
            | PropertyValue::Float(_)
            | PropertyValue::Choice { .. }
            | PropertyValue::KeyBinding(_)
            | PropertyValue::ReadOnly(_) => {
                // Fall through to display_text rendering below.
            }
        }

        if render_text {
            let display = cell.value.display_text();
            if !display.is_empty() {
                let run = cx.text_engine.shape(&display, font_size);
                let metrics = cx.text_engine.font_metrics(font_size);
                let baseline_y = row_y
                    + compute_control_text_baseline_y(
                        row_height,
                        metrics.ascent,
                        metrics.descent,
                        theme.typography.baseline_offset,
                    );
                let text_y = baseline_y - metrics.ascent;
                cx.text_engine.draw(
                    &run,
                    content_x,
                    text_y,
                    value_color,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }
        }

        cx.scene.end_clip();
    }

    /// Cell-mode hit-test, row axis: viewport-local Y → visible-row index.
    ///
    /// Adds `scroll_offset` to convert `local_y` to content Y, maps to the
    /// sorted row index via `row_height`, then offsets by `first_sorted` to
    /// index into the consumer's visible window (`visible_count` rows).
    /// Returns `None` when `local_y` is outside the viewport or the
    /// resolved index falls outside the window.
    ///
    /// `row_height` / `scroll_offset` come from the owned HTS; the window
    /// (`viewport_height`, `first_sorted`, `visible_count`) is supplied by
    /// the consumer body, which caches it at layout time.
    pub fn hit_test_row(
        &self,
        local_y: f32,
        viewport_height: f32,
        first_sorted: usize,
        visible_count: usize,
    ) -> Option<usize> {
        let row_height = self.hts.borrow().row_height();
        // SPEC-PM-017: f32 shim — CMS converts at the rung-5 audit.
        let scroll_offset = self.hts.borrow().scroll_offset() as f32;

        if row_height <= 0.0 || local_y < 0.0 || local_y >= viewport_height {
            return None;
        }

        let content_y = local_y + scroll_offset;
        let sorted_index = (content_y / row_height).floor() as usize;

        if sorted_index < first_sorted {
            return None;
        }
        let vi = sorted_index - first_sorted;
        if vi < visible_count { Some(vi) } else { None }
    }

    /// Cell-mode hit-test, cell resolution: viewport-local (X, Y) →
    /// `(node_id, column_id)`.
    ///
    /// Combines `hit_test_row` for the row axis with a body-slot scan for
    /// the column axis. Horizontal scroll is folded in via `scroll_x`.
    ///
    /// Band rows are hit-tested as whole-row units, never as cells:
    /// returns `None` over any row whose `CellSource::row_archetype` is
    /// `RowArchetype::Band` (SPEC-HAL-260A §5.5.1 / §10.5). This is the
    /// universal seam — consumers project their richer row kinds (e.g.
    /// PE's `PropertyNodeKind`) to the binary archetype in their
    /// `CellSource` impl. When no cell source is registered the row is
    /// treated as data (no band-skip), matching the no-data outcome.
    ///
    /// SPEC-HAL-168 INVARIANT-6: column geometry is read through the
    /// non-panicking gate (`try_column_layout`). A resize Move arriving
    /// before the re-layout pass sees `resolved == false`; returning `None`
    /// matches the no-hit outcome for any X outside a column slot.
    pub fn hit_test_cell(
        &self,
        local_x: f32,
        local_y: f32,
        viewport_height: f32,
        first_sorted: usize,
        visible_rows: &[VisibleRow],
        scroll_x: f32,
    ) -> Option<(u64, ColumnId)> {
        let vi = self.hit_test_row(local_y, viewport_height, first_sorted, visible_rows.len())?;
        let node_id = visible_rows.get(vi)?.node_id;

        if let Some(source_rc) = self.cell_source.as_ref() {
            if source_rc.borrow().row_archetype(node_id) == RowArchetype::Band {
                return None;
            }
        }

        let slots: Vec<_> = self
            .hts
            .borrow()
            .try_column_layout()?
            .body_slots()
            .collect();

        let geometry_x = local_x + scroll_x;
        let col_id = slots
            .iter()
            .find(|s| geometry_x >= s.x && geometry_x < s.x + s.width)
            .map(|s| s.column_id)?;
        Some((node_id, col_id))
    }

    /// Cell-mode hit-test, separator axis: whether viewport-local X falls
    /// on a column separator.
    ///
    /// Slop is symmetric (`SEPARATOR_HIT_HALF_WIDTH`) around each
    /// separator's centerline (`slot.x + slot.width - 1.0`). The last
    /// column has no trailing separator. Horizontal scroll is folded in
    /// via `scroll_x`.
    ///
    /// SPEC-HAL-168 INVARIANT-6: geometry read through the non-panicking
    /// gate; `false` when geometry is unresolved mid-resize.
    pub fn is_on_separator(&self, local_x: f32, scroll_x: f32) -> bool {
        let slots: Vec<_> = match self.hts.borrow().try_column_layout() {
            Some(view) => view.body_slots().collect(),
            None => return false,
        };

        if slots.len() <= 1 {
            return false;
        }

        let geometry_x = local_x + scroll_x;
        slots[..slots.len() - 1].iter().any(|slot| {
            (geometry_x - (slot.x + slot.width - 1.0)).abs() <= SEPARATOR_HIT_HALF_WIDTH
        })
    }

    /// Reconcile always-mounted Bool editor children (Checkboxes) for
    /// every editable, permitted Bool cell in the current visible window.
    ///
    /// Eligibility: a `(node_id, column_id)` cell qualifies iff
    /// `realized_cell_value(node_id, column_id)` is `Some(cell)` AND
    /// `cell.value` is `PropertyValue::Bool` AND `cell.editable` AND
    /// `cell_source.permission_check(query) == Permit` where `query.action
    /// = PermissionAction::Edit`. Non-eligible mounts are unmounted.
    ///
    /// The toggle callback is built per cell: when the user toggles a
    /// mounted Checkbox, the closure calls `on_bool_toggle(node_id,
    /// column_id, new_value)`. Consumers route this through whatever
    /// commit machinery they own (PE: `dispatch_intent`; future direct-
    /// commit consumers: `cell_source.commit_cell`).
    ///
    /// SPEC-HAL-260A §6.2. SPEC-HAL-260A §7.3 (cell_widget_key encoding).
    /// Reconcile is layout-phase; child-widget mounts happen via `cx.tree`.
    pub fn reconcile_always_mounted_editors<F>(
        &mut self,
        cx: &mut LayoutContext<'_>,
        self_node_id: NodeId,
        visible_rows: &[VisibleRow],
        on_bool_toggle: F,
    ) where
        F: Fn(u64, ColumnId, bool) + Clone + 'static,
    {
        let source_rc = self.cell_source.clone();
        self.reconcile_always_mounted_editors_with(
            cx,
            self_node_id,
            visible_rows,
            move |node_id, _col_id, slot_idx, cell| {
                if !cell.editable {
                    return false;
                }
                let Some(source_rc) = source_rc.as_ref() else {
                    return false;
                };
                let perm = source_rc.borrow().permission_check(PermissionQuery {
                    row_id: node_id,
                    column_index: Some(slot_idx),
                    action: PermissionAction::Edit,
                });
                matches!(perm, PermissionResult::Permit)
            },
            on_bool_toggle,
        );
    }

    pub fn reconcile_always_mounted_editors_with<F, P>(
        &mut self,
        cx: &mut LayoutContext<'_>,
        self_node_id: NodeId,
        visible_rows: &[VisibleRow],
        mut is_bool_editor_enabled: P,
        on_bool_toggle: F,
    ) where
        F: Fn(u64, ColumnId, bool) + Clone + 'static,
        P: FnMut(u64, ColumnId, usize, &Cell) -> bool,
    {
        // An empty visible window means this is a transient/measure layout
        // pass (e.g. content-aware elastic column resolution invokes layout
        // with a zero viewport). Reconciling here would compute an empty
        // eligibility set and tear down the entire always-mounted pool via
        // stale-removal, then remount it next pass with fresh NodeIds. That
        // churn destroys the pointer capture a Checkbox holds between Down
        // and Up, dropping the release so the toggle never fires. An empty
        // window carries no eligibility signal — skip the pass entirely and
        // preserve the pool; the next non-empty pass re-syncs it.
        if visible_rows.is_empty() {
            return;
        }
        let Some(editor_factory) = self.editor_factory.as_ref() else {
            return;
        };

        let row_height = self.hts.borrow().row_height();
        // SPEC-PM-017: f32 shim + PM-009 dual-source derivation — CMS
        // adopts the engine-published first_sorted at the rung-5 audit.
        let scroll_offset = self.hts.borrow().scroll_offset() as f32;
        let first_sorted = if row_height > 0.0 {
            (scroll_offset / row_height).floor() as usize
        } else {
            0
        };

        // Body slot order is the canonical "column display order"
        // CMS exposes. Per-cell `column_index` reported to
        // `permission_check` matches body-slot ordinal — consumers that
        // override the permission gate consume column_index in this
        // sense.
        let body_slot_ids: Vec<ColumnId> = self
            .hts
            .borrow()
            .column_layout()
            .body_slots()
            .map(|s| s.column_id)
            .collect();

        // Mounted list: (visible_row_index, node_id, column_id,
        // body_slot_index, current_bool, enabled). Bool cells always use
        // real Checkbox widgets; editability/permission only controls disabled
        // state, never fallback painting.
        let mut mounted: Vec<(usize, u64, ColumnId, usize, bool, bool)> = Vec::new();
        for (vi, vr) in visible_rows.iter().enumerate() {
            for (slot_idx, col_id) in body_slot_ids.iter().enumerate() {
                let cell = match self
                    .realized_cell_values
                    .get(&(vr.node_id, *col_id))
                    .and_then(|opt| opt.as_ref())
                {
                    Some(c) => c,
                    None => continue,
                };
                let current_bool = match cell.value {
                    PropertyValue::Bool(b) => b,
                    _ => continue,
                };
                let enabled = is_bool_editor_enabled(vr.node_id, *col_id, slot_idx, cell);
                mounted.push((vi, vr.node_id, *col_id, slot_idx, current_bool, enabled));
            }
        }

        let mounted_keys: HashSet<(u64, ColumnId)> =
            mounted.iter().map(|(_, n, c, _, _, _)| (*n, *c)).collect();

        // Stale removal: cells in the pool that are no longer mounted bool cells.
        let stale: Vec<(u64, ColumnId)> = self
            .always_mounted_editors
            .keys()
            .filter(|key| !mounted_keys.contains(key))
            .copied()
            .collect();
        for key in stale {
            if let Some(child_node) = self.always_mounted_editors.remove(&key) {
                cx.tree.remove_node(child_node);
            }
        }

        // Mount / sync each bool cell.
        let checkbox_size = cx.theme.geometry.checkbox.size;
        let cell_text_padding = cx.theme.list_geom.padding_x;
        let checkbox_constraints = Constraints::tight(Size::new(checkbox_size, row_height));

        // Body-slot rect lookup (re-borrow column_layout in a fresh scope
        // since the earlier borrow released).
        let slot_geom: HashMap<ColumnId, (f32, f32)> = self
            .hts
            .borrow()
            .column_layout()
            .body_slots()
            .map(|s| (s.column_id, (s.x, s.width)))
            .collect();

        for (vi, node_id, col_id, slot_idx, current_bool, enabled) in mounted {
            let key_pair = (node_id, col_id);
            let child_node = if let Some(&existing) = self.always_mounted_editors.get(&key_pair) {
                // Sync state from the cell value. `set_checked` does NOT
                // fire `on_toggle_changed` (CHECKBOX-EXT) — avoids the
                // commit re-entrancy loop on revision-driven mirror.
                if let Some(n) = cx.tree.node_mut(existing) {
                    editor_factory.sync_bool_editor(n.widget.as_mut(), current_bool, !enabled);
                }
                existing
            } else {
                let toggle_handler = on_bool_toggle.clone();
                let captured_col_id = col_id;
                let editor = editor_factory.create_bool_editor(
                    current_bool,
                    !enabled,
                    (node_id, captured_col_id),
                    Box::new(move |new_val| {
                        toggle_handler(node_id, captured_col_id, new_val);
                    }),
                );
                // SPEC §7.3 cell_widget_key: tag bit 0 = 1; node_id high
                // 48 bits in bits 16-63; column_index (body-slot ordinal)
                // 15 bits in bits 1-15. Disjoint from disclosure
                // (`node_id << 1`, tag bit 0 = 0).
                let key_u64 = ((node_id & 0x0000_FFFF_FFFF_FFFF) << 16)
                    | ((slot_idx as u64 & 0x7FFF) << 1)
                    | 1;
                let key = Some(WidgetKey::U64(key_u64));
                let child = cx.tree.add_child(self_node_id, editor, key, 0);
                if let Err(refusal) = cx.tree.qualify_cell_editor(self_node_id, child, node_id, col_id) {
                    tracing::warn!(?refusal, node_id, ?col_id, "HAL391 cell editor remains standalone");
                }
                self.always_mounted_editors.insert(key_pair, child);
                child
            };

            cx.layout_child(child_node, checkbox_constraints);

            if let Some((slot_x, _slot_width)) = slot_geom.get(&col_id) {
                let cb_x = slot_x + cell_text_padding;
                let cy_offset = (row_height - checkbox_size) / 2.0;
                let by = (first_sorted + vi) as f32 * row_height + cy_offset;
                cx.set_child_rect(
                    child_node,
                    RectF::new(cb_x, by, checkbox_size, checkbox_size),
                );
            }
        }
    }

    pub(crate) fn has_composite_choice_roles(&self) -> bool {
        !self.always_mounted_combo_box_editors.is_empty()
            || !self.composite_choice_desired.is_empty()
    }

    pub(crate) fn mounted_composite_choice_targets(
        &self,
    ) -> Vec<(CompositeChoiceRoleKey, CompositeEditorId)> {
        self.always_mounted_combo_box_editors
            .iter()
            .map(|(role, mounted)| (*role, mounted.target.clone()))
            .collect()
    }

    pub(crate) fn collect_composite_choices<F>(
        &mut self,
        manifest: Option<CompositeChoiceManifest>,
        provider_revision: OrderRevision,
        mounts: &HashMap<CompositeChoiceRoleKey, CompositeChoiceMountFacts>,
        mut read_current: F,
        on_commit: Rc<dyn Fn(CompositeChoiceRoleKey, String)>,
    ) -> Vec<CompositeChoiceWork>
    where
        F: FnMut(CompositeChoiceRoleKey, bool) -> CompositeChoiceAdmission,
    {
        let mut work = Vec::new();
        if let Some(manifest) = manifest {
            if manifest.provider_revision == provider_revision {
                self.composite_choice_revision = Some(provider_revision);
                self.composite_choice_desired = manifest
                    .roles
                    .into_iter()
                    .map(|role| (role.key, role))
                    .collect();
                self.composite_choice_waiting_manifest.retain(|key| {
                    self.composite_choice_desired.contains_key(key)
                        || self.always_mounted_combo_box_editors.contains_key(key)
                });
                for key in self.composite_choice_desired.keys() {
                    self.composite_choice_waiting_manifest.remove(key);
                }
                self.composite_choice_readiness.retain(|key, _| {
                    self.composite_choice_desired.contains_key(key)
                        || self.always_mounted_combo_box_editors.contains_key(key)
                });
            } else {
                // A rejected structural description does not disturb independent live roles.
                for role in manifest.roles {
                    if !self
                        .always_mounted_combo_box_editors
                        .contains_key(&role.key)
                    {
                        work.push(CompositeChoiceWork::NonReady {
                            role: role.key,
                            target: None,
                            reason: CompositeChoiceReadiness::Stale,
                        });
                    }
                }
            }
        }

        let mut keys: Vec<_> = self
            .always_mounted_combo_box_editors
            .keys()
            .chain(self.composite_choice_desired.keys())
            .copied()
            .collect();
        keys.sort_by_key(|key| (key.row_id, key.column_id.raw()));
        keys.dedup();
        let current_manifest = self.composite_choice_revision == Some(provider_revision);
        for role in keys {
            let mounted = self.always_mounted_combo_box_editors.get(&role);
            let realized = current_manifest && self.composite_choice_desired.contains_key(&role);
            if let Some(mounted) = mounted {
                let Some(facts) = mounts.get(&role).filter(|facts| facts.target == mounted.target)
                else {
                    work.push(mounted.remove(role));
                    continue;
                };
                if current_manifest && !realized && !facts.pinned {
                    // A current viewport can end residency without making
                    // any claim about the underlying row's identity.
                    work.push(mounted.remove(role));
                    continue;
                }
            } else if !realized {
                work.push(CompositeChoiceWork::NonReady {
                    role,
                    target: None,
                    reason: CompositeChoiceReadiness::Stale,
                });
                continue;
            }
            if self.composite_choice_waiting_manifest.contains(&role) {
                continue;
            }
            let admission = read_current(role, realized);
            let spec = match admission {
                CompositeChoiceAdmission::Ready(spec) if current_manifest => spec,
                CompositeChoiceAdmission::Retired => {
                    if let Some(mounted) = mounted {
                        work.push(mounted.remove(role));
                    }
                    continue;
                }
                CompositeChoiceAdmission::Ready(_) | CompositeChoiceAdmission::NonReady(_) => {
                    let reason = match admission {
                        CompositeChoiceAdmission::NonReady(reason) => reason,
                        _ => CompositeChoiceReadiness::Stale,
                    };
                    work.push(CompositeChoiceWork::NonReady {
                        role,
                        target: mounted.map(|mounted| mounted.target.clone()),
                        reason,
                    });
                    continue;
                }
            };
            let Some(factory) = self.editor_factory.as_ref() else {
                work.push(CompositeChoiceWork::NonReady {
                    role,
                    target: mounted.map(|mounted| mounted.target.clone()),
                    reason: CompositeChoiceReadiness::Unavailable,
                });
                continue;
            };
            if let Some(mounted) = mounted {
                work.push(CompositeChoiceWork::Sync {
                    role,
                    target: mounted.target.clone(),
                    spec,
                    factory: factory.clone(),
                });
            } else {
                let callback = on_commit.clone();
                let widget = factory.create_combo_box_editor(
                    &spec,
                    (role.row_id, role.column_id),
                    Box::new(move |value| callback(role, value)),
                );
                work.push(CompositeChoiceWork::Mount {
                    role,
                    key: self.composite_choice_desired[&role].widget_key.clone(),
                    widget,
                });
            }
        }
        work
    }

    pub(crate) fn settle_composite_choice(&mut self, settlement: CompositeChoiceSettlement) {
        match settlement {
            CompositeChoiceSettlement::Mounted {
                role,
                node,
                owner,
                target,
            } => {
                self.always_mounted_combo_box_editors.insert(
                    role,
                    MountedCompositeChoice {
                        node,
                        owner,
                        target,
                        ready: true,
                    },
                );
                self.composite_choice_readiness.remove(&role);
            }
            CompositeChoiceSettlement::Synced { role, outcome } => {
                let ready = matches!(
                    outcome,
                    CompositeEditOutcome::Applied | CompositeEditOutcome::Unchanged
                );
                if let Some(mounted) = self.always_mounted_combo_box_editors.get_mut(&role) {
                    mounted.ready = ready;
                }
                if ready {
                    self.composite_choice_readiness.remove(&role);
                } else {
                    self.composite_choice_readiness
                        .insert(role, CompositeChoiceReadiness::Unavailable);
                    if matches!(
                        outcome,
                        CompositeEditOutcome::Stale | CompositeEditOutcome::Retired
                    ) {
                        self.always_mounted_combo_box_editors.remove(&role);
                        self.composite_choice_waiting_manifest.insert(role);
                    }
                }
            }
            CompositeChoiceSettlement::Removed { role } => {
                self.always_mounted_combo_box_editors.remove(&role);
                self.composite_choice_readiness.remove(&role);
                self.composite_choice_waiting_manifest.remove(&role);
            }
            CompositeChoiceSettlement::Rejected { role, outcome } => {
                if matches!(
                    outcome,
                    CompositeEditOutcome::Stale | CompositeEditOutcome::Retired
                ) {
                    self.always_mounted_combo_box_editors.remove(&role);
                    if !self.composite_choice_desired.contains_key(&role) {
                        self.composite_choice_waiting_manifest.remove(&role);
                        self.composite_choice_readiness.remove(&role);
                        return;
                    }
                }
                if let Some(mounted) = self.always_mounted_combo_box_editors.get_mut(&role) {
                    mounted.ready = false;
                }
                self.composite_choice_waiting_manifest.insert(role);
                self.composite_choice_readiness
                    .insert(role, CompositeChoiceReadiness::Unavailable);
            }
            CompositeChoiceSettlement::NonReady { role, reason } => {
                if let Some(mounted) = self.always_mounted_combo_box_editors.get_mut(&role) {
                    mounted.ready = false;
                }
                if self.composite_choice_desired.contains_key(&role)
                    || self.always_mounted_combo_box_editors.contains_key(&role)
                {
                    self.composite_choice_readiness.insert(role, reason);
                }
            }
        }
    }

    /// Layout consumes only established readiness and geometry, never factory props.
    pub(crate) fn layout_composite_choices(
        &self,
        cx: &mut LayoutContext<'_>,
        roles: &[CompositeChoiceRole],
    ) {
        for (key, mounted) in &self.always_mounted_combo_box_editors {
            if cx.tree.widget_id_for_node(mounted.node) != Some(mounted.owner)
                || cx.tree.validate_composite_editor(&mounted.target).is_err()
            {
                continue;
            }
            let rect = roles
                .iter()
                .find(|role| {
                    role.key == *key
                        && mounted.ready
                        && cx.tree.composite_editor_ready(&mounted.target)
                })
                .map(|role| role.rect)
                .unwrap_or_else(RectF::zero);
            cx.layout_child(
                mounted.node,
                Constraints::tight(Size::new(rect.width, rect.height)),
            );
            cx.set_child_rect(mounted.node, rect);
        }
    }

    #[cfg(test)]
    pub(crate) fn composite_choice_ready(&self, key: CompositeChoiceRoleKey) -> bool {
        self.always_mounted_combo_box_editors
            .get(&key)
            .is_some_and(|mounted| mounted.ready)
    }

    #[cfg(test)]
    pub(crate) fn test_composite_choice_non_ready(
        &self,
        key: CompositeChoiceRoleKey,
    ) -> Option<CompositeChoiceReadiness> {
        self.composite_choice_readiness.get(&key).copied()
    }

    pub(crate) fn has_always_mounted_editor(&self, node_id: u64, column_id: ColumnId) -> bool {
        let key = (node_id, column_id);
        self.always_mounted_editors.contains_key(&key)
            || self
                .always_mounted_combo_box_editors
                .contains_key(&CompositeChoiceRoleKey {
                    row_id: node_id,
                    column_id,
                })
    }

    pub(crate) fn park_always_mounted_editors_for_column(
        &mut self,
        cx: &mut LayoutContext<'_>,
        column_id: ColumnId,
    ) {
        for ((_, editor_column_id), child_node) in self.always_mounted_editors.iter() {
            if *editor_column_id == column_id {
                cx.set_child_rect(*child_node, RectF::zero());
            }
        }
        for (key, mounted) in &self.always_mounted_combo_box_editors {
            if key.column_id == column_id {
                cx.set_child_rect(mounted.node, RectF::zero());
            }
        }
    }

    /// Read-only access to the active edit session, if any.
    pub fn edit_session(&self) -> Option<&EditSession> {
        self.edit_session.as_ref()
    }

    /// Set or clear the active edit session. The consumer's intent funnel
    /// calls this from `apply_edit_intent` arms — Begin sets `Some(...)`;
    /// Cancel / accepted-Commit clear with `None`. The reconcile pass on
    /// the next layout transitions the editor child mount accordingly.
    pub fn set_edit_session(&mut self, session: Option<EditSession>) {
        self.edit_session = session;
    }

    /// Attempt to begin an edit session on a cell, applying the shared
    /// on-demand-editor eligibility rule. Returns `true` when a session
    /// was entered.
    ///
    /// Declines (returns `false`, no state change) when:
    /// - the cell is not `editable`;
    /// - `value` is `Bool` (always-mounted affordance), `ReadOnly`, or
    ///   `Badge` — none mount an on-demand editor (`PropertyValue`
    ///   contract);
    /// - a session is already active (INV-PE-SINGLE-EDIT).
    ///
    /// The consumer runs its own permission gate before calling this and
    /// keeps its own non-editor activation paths (e.g. `Bool` toggle).
    /// The substrate owns the eligibility rule and the single-edit guard,
    /// not the consumer's policy. On success, `reconcile_edit_session_owned`
    /// mounts the editor on the next layout pass. CMS sets only the edit
    /// session here — no keyboard, capture, or mode authority (INV-CMS-002).
    pub fn try_enter_edit(
        &mut self,
        row_id: u64,
        column_id: ColumnId,
        value: &PropertyValue,
        editable: bool,
    ) -> bool {
        if !editable {
            return false;
        }
        if matches!(
            value,
            PropertyValue::Bool(_) | PropertyValue::ReadOnly(_) | PropertyValue::Badge(_, _)
        ) {
            return false;
        }
        if self.edit_session.is_some() {
            return false;
        }
        self.set_edit_session(Some(EditSession {
            row_id,
            column_id,
            child_widget_id: None,
            editor_node_id: None,
        }));
        true
    }

    /// Tree truth: cell whose editor is currently mounted, if any.
    pub fn current_edit_cell(&self) -> Option<(u64, ColumnId)> {
        self.current_edit_cell
    }

    /// Convenience predicate for paint-time suppression: returns `true`
    /// when the cell is the active edit cell (state truth) OR the
    /// currently-mounted-editor cell (tree truth across the
    /// INV-PE-TREE-DEFERRED gap).
    pub fn is_edit_cell(&self, row_id: u64, column_id: ColumnId) -> bool {
        self.edit_session.as_ref().map(|s| (s.row_id, s.column_id)) == Some((row_id, column_id))
            || self.current_edit_cell == Some((row_id, column_id))
    }

    /// Read the cell-mode cursor (Copy, so by-value).
    pub fn focus(&self) -> PeFocus {
        self.focus
    }

    /// Write the cell-mode cursor. The consumer's intent funnel is the
    /// only authorized caller (PE: `set_focus_row`, `apply_column_intent`).
    pub fn set_focus(&mut self, focus: PeFocus) {
        self.focus = focus;
    }

    /// Read the column-preservation cache (SPEC-HAL-260A §10.2).
    pub fn last_data_column(&self) -> Option<ColumnId> {
        self.last_data_column
    }

    /// Write the column-preservation cache. The consumer's intent funnel
    /// is the only authorized caller.
    pub fn set_last_data_column(&mut self, col: Option<ColumnId>) {
        self.last_data_column = col;
    }

    /// Reconcile the active edit-session editor child using the substrate's
    /// own editor factory and outcome sink (mount-on-demand).
    ///
    /// Same mount / unmount / reposition state machine as
    /// `reconcile_edit_session`, but the editor is constructed from the
    /// registered `editor_factory` rather than a consumer-supplied closure,
    /// and its commit / cancel callbacks are wired to a substrate-owned sink
    /// that runs the validation gate and pushes an `EditOutcome` onto the
    /// queue the consumer drains. Without a registered factory the session
    /// cannot mount and `current_edit_node` stays `None`.
    ///
    /// SPEC §7.3 cell_widget_key encoding and SPEC-HAL-328 §3.1
    /// deferred-focus are identical to `reconcile_edit_session`.
    pub fn reconcile_edit_session_owned(
        &mut self,
        cx: &mut LayoutContext<'_>,
        self_node_id: NodeId,
        visible_rows: &[VisibleRow],
    ) {
        let session_snapshot: Option<(u64, ColumnId)> =
            self.edit_session.as_ref().map(|s| (s.row_id, s.column_id));

        let row_height = self.hts.borrow().row_height();
        // SPEC-PM-017: f32 shim + PM-009 dual-source derivation — CMS
        // adopts the engine-published first_sorted at the rung-5 audit.
        let scroll_offset = self.hts.borrow().scroll_offset() as f32;
        let first_sorted = if row_height > 0.0 {
            (scroll_offset / row_height).floor() as usize
        } else {
            0
        };

        let body_slot_ids: Vec<ColumnId> = self
            .hts
            .borrow()
            .column_layout()
            .body_slots()
            .map(|s| s.column_id)
            .collect();

        // Step 6a: mount / unmount transition.
        if session_snapshot.is_none() {
            if let Some(node) = self.current_edit_node.take() {
                cx.tree.remove_node(node);
                self.current_edit_cell = None;
            }
        } else if self.current_edit_node.is_none() {
            let (row_id, column_id) = session_snapshot.unwrap();

            // Read initial value from the realized-cell cache (CMS-3);
            // CellSource is never queried at this phase.
            let initial_value: Option<PropertyValue> = self
                .realized_cell_value(row_id, column_id)
                .map(|c| c.value.clone());

            if let Some(initial_value) = initial_value {
                // Substrate-owned editor sink: validation gate (currently an
                // unconditional pass) then push an EditOutcome onto the queue
                // the consumer drains. The closure holds a clone of the queue
                // handle, so it can push across frames without borrowing the
                // substrate.
                let queue = self.edit_outcomes.clone();
                let cell = (row_id, column_id);
                let on_exit: Box<dyn Fn(EditExitReason, Option<PropertyValue>)> =
                    Box::new(move |reason, value| {
                        if validate_or_pass(reason, &value) {
                            queue.borrow_mut().push_back(EditOutcome {
                                row: cell.0,
                                col: cell.1,
                                reason,
                                value,
                            });
                        }
                    });

                // Construct via the registered factory. Without a factory the
                // session cannot mount; current_edit_node stays None.
                let widget: Option<Box<dyn Widget>> = self
                    .editor_factory
                    .as_ref()
                    .map(|factory| factory.create_editor(&initial_value, cell, on_exit));

                if let Some(widget) = widget {
                    // SPEC §7.3 cell_widget_key: tag bit 0 = 1; node_id high
                    // 48 bits in bits 16-63; column_index (body-slot ordinal)
                    // 15 bits in bits 1-15. Same encoding as
                    // `reconcile_always_mounted_editors`.
                    let slot_idx = body_slot_ids
                        .iter()
                        .position(|&id| id == column_id)
                        .unwrap_or(0);
                    let key_u64 = ((row_id & 0x0000_FFFF_FFFF_FFFF) << 16)
                        | ((slot_idx as u64 & 0x7FFF) << 1)
                        | 1;
                    let key = Some(WidgetKey::U64(key_u64));
                    let child = cx.tree.add_child(self_node_id, widget, key, 0);
                    if let Err(refusal) = cx.tree.qualify_cell_editor(self_node_id, child, row_id, column_id) {
                        tracing::warn!(?refusal, row_id, ?column_id, "HAL391 cell editor remains standalone");
                    }
                    self.current_edit_node = Some(child);
                    self.current_edit_cell = Some((row_id, column_id));

                    // Populate session's post-mount identity fields.
                    let widget_id = cx.tree.widget_id_for_node(child);
                    if let Some(session) = self.edit_session.as_mut() {
                        session.editor_node_id = Some(child);
                        session.child_widget_id = widget_id;
                    }

                    // SPEC-HAL-328 §3.1 deferred-focus: request focus +
                    // capture for the editor post-layout.
                    cx.tree.deferred_focus = Some(child);
                }
            }
        }

        // Step 6b: position the active editor (whether just-mounted or
        // persisted from the previous pass). Editor occupies the full cell
        // rect — slot.width × row_height at (slot.x, row top). Active session
        // is NOT cancelled by scroll (SPEC §11.2).
        let slot_geom: HashMap<ColumnId, (f32, f32)> = self
            .hts
            .borrow()
            .column_layout()
            .body_slots()
            .map(|s| (s.column_id, (s.x, s.width)))
            .collect();

        if let Some(editor_node) = self.current_edit_node {
            if let Some((row_id, column_id)) = session_snapshot {
                let vi = visible_rows.iter().position(|vr| vr.node_id == row_id);
                let slot_data = slot_geom.get(&column_id);
                if let (Some(vi), Some((slot_x, slot_width))) = (vi, slot_data) {
                    let editor_constraints = Constraints::tight(Size::new(*slot_width, row_height));
                    cx.layout_child(editor_node, editor_constraints);
                    let by = (first_sorted + vi) as f32 * row_height;
                    cx.set_child_rect(
                        editor_node,
                        RectF::new(*slot_x, by, *slot_width, row_height),
                    );
                }
            }
        }
    }
}

impl Default for CellMechanicsSubstrate {
    fn default() -> Self {
        Self::new(Rc::new(RefCell::new(HierarchicalTabularSubstrate::new())))
    }
}

/// Validation gate for an editor exit. Returns whether the outcome should
/// be queued (and thus committed downstream by the consumer). Currently an
/// unconditional pass — no validator is consulted. The conditional (reject
/// on validation failure, keeping the editor mounted with an error) lands
/// when the validation subsystem is built; only this function body changes.
fn validate_or_pass(_reason: EditExitReason, _value: &Option<PropertyValue>) -> bool {
    true
}

#[cfg(test)]
mod try_enter_edit_tests {
    use super::CellMechanicsSubstrate;
    use crate::engines::cell_source::{PropertyValue, Severity};
    use crate::model::tabular::column::ColumnId;

    #[test]
    fn declines_when_not_editable() {
        let mut cms = CellMechanicsSubstrate::default();
        let col = ColumnId::new();
        assert!(!cms.try_enter_edit(1, col, &PropertyValue::Integer(5), false));
        assert!(cms.edit_session().is_none());
    }

    #[test]
    fn declines_non_editor_value_types() {
        let col = ColumnId::new();

        let mut cms = CellMechanicsSubstrate::default();
        assert!(!cms.try_enter_edit(1, col, &PropertyValue::Bool(true), true));
        assert!(cms.edit_session().is_none());

        let mut cms = CellMechanicsSubstrate::default();
        let ro = PropertyValue::ReadOnly(Box::new(PropertyValue::Integer(5)));
        assert!(!cms.try_enter_edit(1, col, &ro, true));
        assert!(cms.edit_session().is_none());

        let mut cms = CellMechanicsSubstrate::default();
        let badge = PropertyValue::Badge("status".to_string(), Severity::Info);
        assert!(!cms.try_enter_edit(1, col, &badge, true));
        assert!(cms.edit_session().is_none());
    }

    #[test]
    fn enters_for_editable_editor_value() {
        let mut cms = CellMechanicsSubstrate::default();
        let col = ColumnId::new();
        assert!(cms.try_enter_edit(7, col, &PropertyValue::Integer(5), true));
        assert!(cms.edit_session().map(|s| (s.row_id, s.column_id)) == Some((7, col)));
    }

    #[test]
    fn rejects_second_session_while_active() {
        let mut cms = CellMechanicsSubstrate::default();
        let col = ColumnId::new();
        let col2 = ColumnId::new();
        assert!(cms.try_enter_edit(7, col, &PropertyValue::Integer(5), true));
        // Second attempt while a session is active is rejected; the
        // existing session is untouched (INV-PE-SINGLE-EDIT).
        assert!(!cms.try_enter_edit(9, col2, &PropertyValue::String("x".to_string()), true));
        assert!(cms.edit_session().map(|s| (s.row_id, s.column_id)) == Some((7, col)));
    }
}

#[cfg(test)]
mod drag_reorder_editor_parking_tests {
    use super::{
        CellMechanicsSubstrate, CompositeChoiceAdmission, CompositeChoiceManifest,
        CompositeChoiceMountFacts, CompositeChoiceReadiness, CompositeChoiceRole,
        CompositeChoiceRoleKey, CompositeChoiceSettlement, CompositeChoiceWork,
    };
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::{RectF, Size};
    use crate::core::paint::{Color, PaintSink, TextBackgroundSpan};
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::text_engine::{GlyphRun, TextEngine, TextMetrics};
    use crate::core::theme::Theme;
    use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
    use crate::core::widget_tree::{NodeId, WidgetKey, WidgetTree};
    use crate::engines::cell_source::{Cell, ComboBoxCellEditorSpec, PropertyValue};
    use crate::model::tabular::column::ColumnId;
    use crate::utils::id::WidgetInternalId;
    use crate::widgets::tabular_view::data_adapter::OrderRevision;
    use std::collections::HashMap;
    use std::rc::Rc;

    struct NullTextEngine;

    impl TextEngine for NullTextEngine {
        fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32) {
            (text.len() as f32 * font_size * 0.5, font_size)
        }

        fn metrics(&mut self, _text: &str, font_size: f32) -> TextMetrics {
            TextMetrics {
                ascent: font_size * 0.8,
                descent: font_size * 0.2,
                line_gap: 0.0,
                line_height: font_size,
            }
        }

        fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun {
            GlyphRun {
                binding: None,
                clusters: Vec::new(),
                glyphs: Vec::new(),
                width: text.len() as f32 * font_size * 0.5,
                height: font_size,
                text: text.to_string(),
                font_size,
                ascent: font_size * 0.8,
                descent: font_size * 0.2,
            }
        }

        fn draw(
            &mut self,
            _run: &GlyphRun,
            _x: f32,
            _y: f32,
            _color: Color,
            _z_index: crate::core::paint::ZIndex,
            _sink: &mut dyn PaintSink,
        ) {
        }

        fn draw_with_backgrounds(
            &mut self,
            run: &GlyphRun,
            x: f32,
            y: f32,
            color: Color,
            _backgrounds: Vec<TextBackgroundSpan>,
            z_index: crate::core::paint::ZIndex,
            sink: &mut dyn PaintSink,
        ) {
            self.draw(run, x, y, color, z_index, sink);
        }
    }

    struct FixedWidget {
        id: WidgetInternalId,
    }

    impl FixedWidget {
        fn new() -> Self {
            Self {
                id: WidgetInternalId::new(),
            }
        }
    }

    impl WidgetDowncast for FixedWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }

        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for FixedWidget {
        fn id(&self) -> WidgetInternalId {
            self.id
        }

        fn layout(
            &mut self,
            _cx: &mut LayoutContext,
            _constraints: Constraints,
            _children: &[NodeId],
        ) -> LayoutResult {
            LayoutResult::fit(Size::new(10.0, 10.0))
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}
    }

    #[test]
    fn parks_only_editors_in_dragged_column() {
        let mut cms = CellMechanicsSubstrate::default();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(FixedWidget::new()), None, 0);
        let dragged_col = ColumnId::new();
        let stable_col = ColumnId::new();
        let make_choice = || {
            crate::widgets::combo_box::ComboBox::simple_with_values(vec![("A".into(), "A".into())])
        };
        let dragged_child = tree.add_child(root, Box::new(make_choice()), None, 0);
        let stable_child = tree.add_child(root, Box::new(make_choice()), None, 0);
        for (column_id, node) in [(dragged_col, dragged_child), (stable_col, stable_child)] {
            let owner = tree.widget_id_for_node(node).unwrap();
            cms.settle_composite_choice(CompositeChoiceSettlement::Mounted {
                role: CompositeChoiceRoleKey {
                    row_id: 1,
                    column_id,
                },
                node,
                owner,
                target: tree.composite_editor_id(owner).unwrap(),
            });
        }

        let theme = Theme::default();
        let mut text_engine = NullTextEngine;
        let _guard = LayoutAuthorityGuard::enter();
        {
            let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
            cx.set_child_rect(dragged_child, RectF::new(12.0, 8.0, 80.0, 24.0));
            cx.set_child_rect(stable_child, RectF::new(92.0, 8.0, 80.0, 24.0));
            cms.park_always_mounted_editors_for_column(&mut cx, dragged_col);
        }

        assert_eq!(tree.get_rect(dragged_child), Some(RectF::zero()));
        assert_eq!(
            tree.get_rect(stable_child),
            Some(RectF::new(92.0, 8.0, 80.0, 24.0))
        );
    }

    fn choice_role(row_id: u64, column_id: ColumnId) -> CompositeChoiceRole {
        CompositeChoiceRole {
            key: CompositeChoiceRoleKey { row_id, column_id },
            widget_key: WidgetKey::U64(row_id),
            rect: RectF::new(0.0, 0.0, 80.0, 24.0),
        }
    }

    fn choice_spec(value: &str) -> ComboBoxCellEditorSpec {
        ComboBoxCellEditorSpec {
            current_value: value.into(),
            options: vec![("Current".into(), "current".into())],
        }
    }

    #[test]
    fn composite_editor_choice_current_value_and_pending_role_are_independent() {
        let mut cms = CellMechanicsSubstrate::default();
        cms.set_editor_factory(Box::new(
            crate::widgets::cell_editor_factory::DefaultCellEditorFactory::new(),
        ));
        let column = ColumnId::new();
        let ready = choice_role(1, column);
        let pending = choice_role(2, column);
        cms.realized_cell_values.insert(
            (1, column),
            Some(Cell::new(PropertyValue::String("old-cache".into()), true)),
        );
        let work = cms.collect_composite_choices(
            Some(CompositeChoiceManifest {
                provider_revision: OrderRevision(1),
                roles: vec![ready.clone(), pending.clone()],
            }),
            OrderRevision(1),
            &HashMap::new(),
            |role, _| {
                if role == ready.key {
                    CompositeChoiceAdmission::Ready(choice_spec("current"))
                } else {
                    CompositeChoiceAdmission::NonReady(CompositeChoiceReadiness::Pending)
                }
            },
            Rc::new(|_, _| {}),
        );
        assert_eq!(work.len(), 2);
        let mut saw_ready = false;
        let mut saw_pending = false;
        for operation in work {
            match operation {
                CompositeChoiceWork::Mount { role, widget, .. } => {
                    assert_eq!(role, ready.key);
                    let choice = widget
                        .as_any()
                        .downcast_ref::<crate::widgets::combo_box::ComboBox>()
                        .unwrap();
                    assert_eq!(choice.input_text(), "current");
                    saw_ready = true;
                }
                CompositeChoiceWork::NonReady {
                    role,
                    target: None,
                    reason: CompositeChoiceReadiness::Pending,
                } => {
                    assert_eq!(role, pending.key);
                    saw_pending = true;
                }
                _ => panic!("ready and pending roles must settle independently"),
            }
        }
        assert!(saw_ready && saw_pending);
    }

    #[test]
    fn composite_editor_choice_stale_manifest_cannot_rebind_or_stop_live_sync() {
        let mut cms = CellMechanicsSubstrate::default();
        cms.set_editor_factory(Box::new(
            crate::widgets::cell_editor_factory::DefaultCellEditorFactory::new(),
        ));
        let column = ColumnId::new();
        let live = choice_role(1, column);
        let stale = choice_role(2, column);
        let work = cms.collect_composite_choices(
            Some(CompositeChoiceManifest {
                provider_revision: OrderRevision(2),
                roles: vec![live.clone()],
            }),
            OrderRevision(2),
            &HashMap::new(),
            |_, _| CompositeChoiceAdmission::Ready(choice_spec("current")),
            Rc::new(|_, _| {}),
        );
        let CompositeChoiceWork::Mount { widget, key, .. } = work.into_iter().next().unwrap()
        else {
            panic!("fresh requested role must be installed by the tree owner");
        };
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(FixedWidget::new()), None, 0);
        let node = tree.add_child(root, widget, Some(key), 0);
        let owner = tree.widget_id_for_node(node).unwrap();
        let target = tree.composite_editor_id(owner).unwrap();
        cms.settle_composite_choice(CompositeChoiceSettlement::Mounted {
            role: live.key,
            node,
            owner,
            target: target.clone(),
        });
        let work = cms.collect_composite_choices(
            Some(CompositeChoiceManifest {
                provider_revision: OrderRevision(1),
                roles: vec![stale.clone()],
            }),
            OrderRevision(2),
            &HashMap::from([(
                live.key,
                CompositeChoiceMountFacts {
                    target,
                    pinned: false,
                },
            )]),
            |_, _| CompositeChoiceAdmission::Ready(choice_spec("new-current")),
            Rc::new(|_, _| {}),
        );
        assert!(work.iter().any(|operation| matches!(operation,
            CompositeChoiceWork::Sync { role, spec, .. } if *role == live.key && spec.current_value == "new-current")));
        assert!(work.iter().any(|operation| matches!(operation,
            CompositeChoiceWork::NonReady { role, target: None, reason: CompositeChoiceReadiness::Stale } if *role == stale.key)));
        assert!(!work.iter().any(|operation| matches!(
            operation,
            CompositeChoiceWork::Mount { .. } | CompositeChoiceWork::Remove { .. }
        )));
        assert!(cms.composite_choice_ready(live.key));
    }
}
