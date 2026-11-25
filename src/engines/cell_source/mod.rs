// =============================================================================
// Cell Source — Universal Cell Access for Tabular Data
// =============================================================================
//
// Foundation for widgets that present row-keyed cellular data: PropertyEditor,
// future flat data grids, future spreadsheets. Separates STRUCTURE (which is
// widget-specific — tree topology for PE, flat ordering for a grid, infinite
// addressing for a spreadsheet) from CONTENT (which is universal — cells
// addressed by (row_id, column_id) carrying a value and editability flag).
//
// Three-tier model:
//   - Column = schema. Declared at widget setup. Carries ColumnKind, title,
//     default editability. Lives in the consumer widget's core state.
//   - Row = identity + structure. Widget-specific (e.g., PropertyNode for PE
//     carries tree topology, `kind: PropertyNodeKind`, parent linkage — see
//     SPEC-HAL-260A §5.5). Archetype (Data vs Band) is universal and lives
//     on `CellSource::row_archetype` so any cell-addressed widget can
//     distinguish cell-bearing rows from grouping bands.
//   - Cell = value + per-cell state. UNIVERSAL — defined here.
//     `Cell { value, editable }`. Per-cell editable lets a derived /
//     calculated cell be read-only even when its column says "Data, editable".
//
// Per-cell editable is the ONLY override Cell carries. Permission checks,
// validation results, formatting hints, tooltips, and column-kind echoes are
// kept off Cell — they're either queries (with their own lifecycle, lookup
// path, and caching strategy) or column-level schema (set once at registration).
// Cell is the read-hot path: every paint pass calls `cell()` per visible
// row × per visible column. Cell must stay cheap.

use crate::core::paint::Color;
use crate::core::scene::linear_to_srgb;
use crate::core::widget::Widget;
use crate::model::tabular::column::ColumnId;
use crate::runtime::actions::Shortcut;

// ----------------------------------------------------------------------------
// Severity — semantic role for `PropertyValue::Badge`.
// ----------------------------------------------------------------------------

/// Functional semantic role for labeled status indicators.
///
/// Maps to theme tokens at render time:
///   - `Neutral` → no semantic color (renderer uses the default text color
///     for the surrounding context — secondary for read-only cells, primary
///     otherwise).
///   - `Success` → `theme.colors.semantic.success`
///   - `Warning` → `theme.colors.semantic.warning`
///   - `Error` → `theme.colors.semantic.error`
///   - `Info` → `theme.colors.semantic.info`
///
/// SPEC-HAL-332 §3: success / warning / error are FUNCTIONAL roles (not
/// decorative). `Neutral` is the no-op default; `Info` is the informational
/// (non-actionable) signal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Severity {
    Neutral,
    Success,
    Warning,
    Error,
    Info,
}

// ----------------------------------------------------------------------------
// PropertyValue — the typed value carried by a Cell.
// ----------------------------------------------------------------------------

/// Value types supported by cell-addressed widgets.
///
/// PE-tailored set for now (text, integer, float, bool, choice, color,
/// keybinding, read-only wrapper). A future consumer that wants typed
/// numeric cells without going through this enum would require a generic
/// `Cell<V>` — explicitly deferred (no consumer yet; generics cascade into
/// every cell handler, editor, paint path).
///
/// Band-archetype rows (Section / SubSection in PE) are NOT a value variant.
/// They are a row-structural concern: `CellSource::row_archetype(row_id)`
/// returns `RowArchetype::Band`, and `cell()` returns `None` at every
/// column. PE's finer-grained classification (`PropertyNodeKind { Data,
/// Section, SubSection }`) lives in `PropertyNode.kind` and projects to
/// the core `RowArchetype` at the `PropertyDataSource` trait boundary —
/// see SPEC-HAL-260A §5.5.
#[derive(Debug, Clone)]
pub enum PropertyValue {
    /// Free-form text. Mounts TextField (SPEC-HAL-013, 014).
    String(String),
    /// Integer value. Mounts TextField (SPEC-HAL-013, 014) with integer
    /// input filter.
    Integer(i64),
    /// Floating point value. Mounts TextField (SPEC-HAL-013, 014) with
    /// numeric input filter.
    Float(f64),
    /// Boolean toggle. Always-mounted Checkbox in PE (§6.2); other widgets
    /// may render their own affordance.
    Bool(bool),
    /// Choice from fixed options. Mounts ComboBox (SPEC-HAL-201–206).
    Choice {
        selected: usize,
        options: Vec<String>,
    },
    /// Color value. Mounts ColorPickerField (SPEC-HAL-272).
    Color(Color),
    /// Key binding. Mounts KeybindEditor (SPEC-HAL-329). Carries the
    /// canonical Shortcut from src/runtime/actions.rs (SPEC-HAL-020A),
    /// not a free-form string. Display uses `Shortcut::to_display_string()`
    /// for `Some` and an empty string for `None` (unbound).
    ///
    /// `Option<Shortcut>` enables the steal-binding UX (JC directive
    /// 2026-05-21): when a shortcut is taken from one cell and assigned
    /// to another, the source cell transitions to `KeyBinding(None)` so
    /// the display honestly reflects "no binding."
    KeyBinding(Option<Shortcut>),
    /// Semantic-colored short label. Renders the contained text in the
    /// theme color corresponding to its severity. Non-editable — no editor
    /// is mounted for Badge cells regardless of `Cell.editable`.
    ///
    /// Use for status indicators, log levels, priority tags, build states —
    /// any short label whose color carries functional meaning.
    /// SPEC-HAL-332 §3 (semantic palette is functional). SPEC-HAL-332 §6
    /// (badges/pills/tags family). Current renderer in TreeView draws Badge
    /// as colored text; pill-background rendering is a future renderer
    /// upgrade that does not require a variant change.
    Badge(String, Severity),
    /// Read-only wrapper. Renders the inner value's display_text() but is
    /// non-editable. Boxed to keep the enum size bounded.
    ReadOnly(Box<PropertyValue>),
}

impl PropertyValue {
    /// Display string for the cell.
    pub fn display_text(&self) -> String {
        match self {
            PropertyValue::String(s) => s.clone(),
            PropertyValue::Integer(n) => n.to_string(),
            PropertyValue::Float(f) => format!("{:.2}", f),
            PropertyValue::Bool(b) => {
                if *b {
                    "Yes".to_string()
                } else {
                    "No".to_string()
                }
            }
            PropertyValue::Choice { selected, options } => {
                options.get(*selected).cloned().unwrap_or_default()
            }
            PropertyValue::Color(c) => {
                // Color stores linear RGB internally; convert to sRGB for hex.
                let r = linear_to_srgb(c.r);
                let g = linear_to_srgb(c.g);
                let b = linear_to_srgb(c.b);
                format!(
                    "#{:02X}{:02X}{:02X}",
                    (r * 255.0).round().clamp(0.0, 255.0) as u8,
                    (g * 255.0).round().clamp(0.0, 255.0) as u8,
                    (b * 255.0).round().clamp(0.0, 255.0) as u8,
                )
            }
            PropertyValue::KeyBinding(opt) => match opt {
                Some(s) => s.to_display_string(),
                None => String::new(),
            },
            PropertyValue::ReadOnly(inner) => inner.display_text(),
            PropertyValue::Badge(label, _) => label.clone(),
        }
    }

    /// Whether the variant supports edit. `ReadOnly` is the only non-editable
    /// value-side variant; per-cell editability is layered on top via the
    /// `Cell.editable` flag, and consumer permission gating layers on top
    /// of that (e.g., PE's §13 permission_check).
    pub fn is_editable(&self) -> bool {
        !matches!(
            self,
            PropertyValue::ReadOnly(_) | PropertyValue::Badge(_, _)
        )
    }
}

// ----------------------------------------------------------------------------
// Cell — value + per-cell state.
// ----------------------------------------------------------------------------

/// A single addressable cell in a row-keyed tabular widget.
///
/// Returned by `CellSource::cell(row_id, column_id)`. `None` from `cell()`
/// signals "no cell exists at this address" — for example, a
/// band-archetype row (`row_archetype` returns `RowArchetype::Band`) that
/// has no value for any column, or a sparse spreadsheet cell that hasn't
/// been written.
#[derive(Debug, Clone)]
pub struct Cell {
    /// Cell value.
    pub value: PropertyValue,
    /// Per-cell editability. The CONSUMER computes this — typically the
    /// column's default editability AND'd with any row-level or cell-level
    /// overrides (e.g., calculated cells are non-editable; locked rows are
    /// non-editable). Consumer-side permission gating (e.g., PE's §13
    /// permission_check) layers on top of this flag.
    pub editable: bool,
}

impl Cell {
    pub fn new(value: PropertyValue, editable: bool) -> Self {
        Self { value, editable }
    }

    /// Convenience constructor: a read-only cell.
    pub fn read_only(value: PropertyValue) -> Self {
        Self {
            value,
            editable: false,
        }
    }
}

// ----------------------------------------------------------------------------
// RowArchetype — universal row-kind classification.
// ----------------------------------------------------------------------------

/// Row archetype. The structural distinction between rows that host cells
/// (`Data`) and rows that don't (`Band`). Universal across cell-addressed
/// widgets — any widget consuming a `CellSource` branches paint, hit-test,
/// focus, and navigation on archetype.
///
/// Two variants at the core. Widgets that need finer-grained archetype
/// distinctions for their own paint or UX (e.g., PropertyEditor's
/// `PropertyNodeKind { Data, Section, SubSection }`) project their richer
/// variants to this binary core enum at the trait boundary in their
/// `CellSource::row_archetype` impl.
#[derive(Copy, Clone, Debug, PartialEq, Eq)]
pub enum RowArchetype {
    /// The row hosts cells at its data columns. `CellSource::cell()`
    /// returns `Some` per Data column when the consumer populates.
    /// Cell-mode focus, selection, and edit apply on Data rows.
    Data,
    /// The row is a structural grouping band. `CellSource::cell()`
    /// returns `None` at every column. Cell-mode operations
    /// (per-column focus, selection, edit) do not apply; the row is
    /// hit-tested and focused as a whole. Bands typically host a
    /// disclosure toggle when they have children; widgets paint them
    /// with a distinct full-width background.
    Band,
}

// ----------------------------------------------------------------------------
// Permission gate types — query / action / result.
// ----------------------------------------------------------------------------

/// Query passed to `CellSource::permission_check`. Identifies the cell or
/// row being acted on and the action being attempted.
pub struct PermissionQuery {
    pub row_id: u64,
    /// `None` when the action is row-wide (Focus, Select, Tree).
    /// `Some(idx)` when the action is cell-specific (Edit, Activate),
    /// where `idx` is the column's index in the consumer widget's
    /// registered display order.
    pub column_index: Option<usize>,
    pub action: PermissionAction,
}

/// Action being checked. `#[non_exhaustive]` so the action set can widen
/// over time without breaking trait implementors.
#[non_exhaustive]
#[derive(Copy, Clone, Debug)]
pub enum PermissionAction {
    /// Move focus to the cell / row.
    Focus,
    /// Select the row.
    Select,
    /// Begin an edit session.
    Edit,
    /// Activate (e.g., toggle Bool, expand Section).
    Activate,
    /// Extend selection via drag.
    DragExtend,
    /// Tree-structural action (expand / collapse).
    Tree,
}

/// Permission gate result.
#[derive(Copy, Clone, Debug)]
pub enum PermissionResult {
    Permit,
    Deny,
}

// ----------------------------------------------------------------------------
// CellSource trait — the universal cell-access seam.
// ----------------------------------------------------------------------------

/// Widget-agnostic cell access. Any widget that presents row-keyed cells
/// in (row_id, column_id) addressing space consumes a `CellSource`.
///
/// Consumer widgets layer their own structural traits on top:
///   - PE's `PropertyDataSource: CellSource` adds `nodes()` for tree
///     topology and `permission_check()` for the §13 gate.
///   - A future flat data grid would consume `CellSource` directly.
///   - A future spreadsheet would consume `CellSource` directly with
///     sparse storage.
pub trait CellSource {
    /// Read the cell at (row_id, column_id). Returns `None` when no cell
    /// exists at this address.
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<Cell>;

    /// Row archetype for the given `row_id`. Default returns
    /// `RowArchetype::Data` so existing `CellSource` impls preserve
    /// today's behavior (every row hosts cells). Widgets that
    /// introduce band-archetype rows override this method; their
    /// `cell()` impl returns `None` at every column for Band rows.
    ///
    /// This is the seam by which downstream consumers (paint, hit-test,
    /// focus, navigation in any cell-addressed widget) know whether a
    /// row hosts cells without per-column probing.
    fn row_archetype(&self, _row_id: u64) -> RowArchetype {
        RowArchetype::Data
    }

    /// Commit a new value to the cell at (row_id, column_id). Returns
    /// `true` to accept, `false` to reject. On rejection, the consumer
    /// widget displays the `validate` error (if any) inline against the
    /// editing cell.
    fn commit_cell(&mut self, row_id: u64, column_id: ColumnId, new_value: PropertyValue) -> bool;

    /// Validate a candidate value before commit. Returns `Ok(())` if valid,
    /// `Err(msg)` if invalid. Default: always valid.
    fn validate(
        &self,
        _row_id: u64,
        _column_id: ColumnId,
        _candidate: &PropertyValue,
    ) -> Result<(), String> {
        Ok(())
    }

    /// Monotonic revision counter. Consumer widgets re-read cells when
    /// revision changes.
    fn revision(&self) -> u64;

    /// Per-cell access policy gate. Default: always permit (preserves
    /// behavior for consumers that have not opted into a non-trivial
    /// access policy).
    ///
    /// Cell-bearing widgets that surface interactive operations
    /// (selection, edit, activation, drag-extend, tree expansion) route
    /// those operations through `permission_check` before applying them.
    /// A consumer-provided source returns `PermissionResult::Deny` to
    /// reject; the widget treats `Deny` as a silent no-op (no error,
    /// no validation feedback — the action simply does not occur).
    fn permission_check(&self, _query: PermissionQuery) -> PermissionResult {
        PermissionResult::Permit
    }
}

// ----------------------------------------------------------------------------
// Editor-exit vocabulary — how an active edit session terminated.
// ----------------------------------------------------------------------------

/// Why an active edit session terminated. Carried in the
/// `on_exit(EditExitReason, Option<PropertyValue>)` callback signaled by
/// the editor (or by the host widget in the ForcedCancel path).
#[derive(Copy, Clone, Debug)]
pub enum EditExitReason {
    /// Editor signaled value should be committed.
    Commit,
    /// Editor signaled value should be discarded (Escape path).
    Cancel,
    /// Editor signaled commit and request cell-mode advance.
    CommitAndAdvance { direction: AdvanceDirection },
    /// Editor was forcibly unmounted by the host (e.g., blur policy =
    /// Cancel, data revision changed mid-edit).
    ForcedCancel,
}

/// Direction of cell-mode advance after CommitAndAdvance.
#[derive(Copy, Clone, Debug)]
pub enum AdvanceDirection {
    Next,
    Previous,
}

#[derive(Debug, Clone)]
pub struct ComboBoxCellEditorSpec {
    pub current_value: String,
    pub options: Vec<(String, String)>,
}

// ----------------------------------------------------------------------------
// CellEditorFactory — constructs the editor widget for a cell entering edit.
// ----------------------------------------------------------------------------

/// Maps a `PropertyValue` variant to its editor widget, wiring the editor's
/// commit/cancel callbacks to `on_exit`. Defined at the engine tier so the
/// cell-mechanics substrate can hold a `Box<dyn CellEditorFactory>` without
/// depending on widget-tier types; implemented at the widget tier
/// (`DefaultCellEditorFactory`, `src/widgets/cell_editor_factory.rs`), where
/// the editor widgets live. Same engine-trait / widget-impl split as
/// `CellSource`.
///
/// `cell_key` is `(row_id, column_id)`. `on_exit(reason, value)` is the sink
/// the editor's callbacks fire into; the host decides what happens on exit.
pub trait CellEditorFactory {
    fn create_bool_editor(
        &self,
        checked: bool,
        disabled: bool,
        cell_key: (u64, ColumnId),
        on_toggle: Box<dyn Fn(bool)>,
    ) -> Box<dyn Widget>;

    fn sync_bool_editor(&self, widget: &mut dyn Widget, checked: bool, disabled: bool);

    fn create_combo_box_editor(
        &self,
        spec: &ComboBoxCellEditorSpec,
        cell_key: (u64, ColumnId),
        on_commit: Box<dyn Fn(String)>,
    ) -> Box<dyn Widget>;

    fn sync_combo_box_editor(
        &self,
        owner: &mut crate::core::composite_text_edit::CompositeEditorOwnerContext<'_>,
        target: &crate::core::composite_text_edit::CompositeEditorId,
        spec: ComboBoxCellEditorSpec,
    ) -> crate::core::composite_text_edit::CompositeEditReply;

    fn create_editor(
        &self,
        value: &PropertyValue,
        cell_key: (u64, ColumnId),
        on_exit: Box<dyn Fn(EditExitReason, Option<PropertyValue>)>,
    ) -> Box<dyn Widget>;
}
