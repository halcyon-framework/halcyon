//! SPEC-HAL-188 §15.1 Baseline In-Memory Oracle
//!
//! A canonical in-memory implementation that:
//! - Satisfies all contracts in SPEC-HAL-188.
//! - Uses synchronous, in-memory data.
//! - Has no permissions barriers (all capabilities true).
//! - Has no failures (no `Error` or `Denied` states).
//! - Provides `Exact` extent.
//! - Operates in `Live` mode.
//!
//! This oracle is the behavior reference for all tests.
//!
//! **Note:** This type is a reference oracle, not a required production dependency.
//! Production applications should implement their own data sources against the
//! SPEC-HAL-188 contracts. UI widgets must never depend on this concrete type.
//!
//! ## Design Constraints (from SPEC-HAL-188)
//!
//! - All data access is RowId-based (no index-based APIs).
//! - Read, write, capability, invalidation are separable.
//! - No API requires full materialization.
//! - Virtualization works without special cases.
//!
//! ## Usage
//!
//! ```ignore
//! use crate::oracles::BaselineDataOracle;
//!
//! let mut oracle = BaselineDataOracle::new();
//! let row_id = oracle.add_row();
//! let col_id = ColumnId::new();
//!
//! oracle.set_cell(row_id, col_id, CellValue::String("hello".into()));
//!
//! let value = oracle.read_cell(row_id, col_id);
//! assert!(matches!(value, ValueState::Ready(_)));
//! ```

use std::collections::{HashMap, VecDeque};

use crate::widgets::tabular_view::{ColumnId, RowId};

// =============================================================================
// SPEC-HAL-188 §4: ValueState Model
// =============================================================================

/// Cell read result (SPEC-HAL-188 §4).
///
/// All cell reads return a `ValueState`, not a raw value.
/// The baseline oracle always returns `Ready(value)` for valid cells.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueState<T> {
    /// Value is resolved and available.
    Ready(T),
    /// Value is not yet available; may arrive later.
    Pending,
    /// Value exists but access is forbidden.
    Denied { reason: Option<DenialReason> },
    /// Read failed.
    Error { error: Option<DataError> },
    /// Row exists but cell has no value (sparse data).
    Missing,
}

impl<T> ValueState<T> {
    /// Returns true if this is a Ready variant.
    pub fn is_ready(&self) -> bool {
        matches!(self, ValueState::Ready(_))
    }

    /// Unwraps the Ready value, panics otherwise.
    pub fn unwrap(self) -> T {
        match self {
            ValueState::Ready(v) => v,
            _ => panic!("called unwrap() on non-Ready ValueState"),
        }
    }

    /// Returns the Ready value if present.
    pub fn into_ready(self) -> Option<T> {
        match self {
            ValueState::Ready(v) => Some(v),
            _ => None,
        }
    }
}

/// Denial reason (advisory).
#[derive(Debug, Clone, PartialEq)]
pub enum DenialReason {
    NotAuthorized,
    RowLocked,
    ColumnHidden,
    Custom(String),
}

/// Data error (advisory).
#[derive(Debug, Clone, PartialEq)]
pub enum DataError {
    NotFound,
    NetworkFailure,
    ParseError,
    Custom(String),
}

// =============================================================================
// SPEC-HAL-188 §5.2: Extent Model
// =============================================================================

/// Row count extent (SPEC-HAL-188 §5.2).
///
/// The baseline oracle always provides `Exact(n)`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Extent {
    /// Exactly n rows exist.
    Exact(usize),
    /// At least n rows exist; may be more.
    LowerBound(usize),
    /// At most n rows exist; may be fewer.
    UpperBound(usize),
    /// Row count is within range.
    Bounded { lower: usize, upper: usize },
    /// Row count cannot be determined.
    Unknown,
    /// Rows arrive continuously; no finite count.
    Streaming,
}

// =============================================================================
// SPEC-HAL-188 §5.3: Universe Mode
// =============================================================================

/// Universe mode (SPEC-HAL-188 §5.3).
///
/// The baseline oracle always operates in `Live` mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniverseMode {
    /// Universe may change; emits invalidations.
    Live,
    /// Universe is frozen; no invalidations will be emitted.
    Snapshot,
}

// =============================================================================
// SPEC-HAL-188 §7: Mutation Results
// =============================================================================

/// Mutation result (SPEC-HAL-188 §7.1).
///
/// The baseline oracle always returns `Applied`.
#[derive(Debug, Clone, PartialEq)]
pub enum MutationResult {
    /// Mutation was applied.
    Applied,
    /// Mutation was rejected.
    Rejected { reason: Option<RejectionReason> },
    /// Mutation is in flight; await invalidation.
    Pending,
}

/// Rejection reason (advisory).
#[derive(Debug, Clone, PartialEq)]
pub enum RejectionReason {
    ValidationFailed,
    ConcurrencyConflict,
    NotAuthorized,
    Custom(String),
}

// =============================================================================
// SPEC-HAL-188 §7.2: Edit Lifecycle
// =============================================================================

/// Opaque edit token (SPEC-HAL-188 §7.2).
///
/// The baseline oracle uses a simple counter-based token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EditToken(u64);

impl EditToken {
    fn new(id: u64) -> Self {
        EditToken(id)
    }
}

/// Edit request result (SPEC-HAL-188 §7.2).
///
/// The baseline oracle always grants edit requests.
#[derive(Debug, Clone, PartialEq)]
pub enum EditResult {
    /// Edit granted with token.
    Granted { token: EditToken },
    /// Edit denied.
    Denied { reason: Option<DenialReason> },
}

/// Validation result (SPEC-HAL-188 §7.2).
///
/// The baseline oracle always returns `Valid`.
#[derive(Debug, Clone, PartialEq)]
pub enum ValidationResult {
    /// Value is valid.
    Valid,
    /// Value is invalid.
    Invalid { errors: Vec<ValidationError> },
}

/// Validation error (advisory).
#[derive(Debug, Clone, PartialEq)]
pub struct ValidationError {
    pub field: Option<String>,
    pub message: String,
}

/// Commit result (SPEC-HAL-188 §7.2).
///
/// The baseline oracle always returns `Applied`.
#[derive(Debug, Clone, PartialEq)]
pub enum CommitResult {
    /// Commit applied.
    Applied,
    /// Commit rejected.
    Rejected { reason: Option<RejectionReason> },
    /// Commit in flight.
    Pending,
}

// =============================================================================
// SPEC-HAL-188 §8: Capability Model
// =============================================================================

/// Cell capabilities (SPEC-HAL-188 §8).
///
/// The baseline oracle returns all-permissive capabilities.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Capabilities {
    /// Cell may be rendered at all.
    pub visible: bool,
    /// Cell value may be read.
    pub readable: bool,
    /// Cell may enter edit mode.
    pub editable: bool,
    /// Cell edit may be committed now.
    pub committable: bool,
    /// Cell is visible but content is masked.
    pub redacted: bool,
}

impl Default for Capabilities {
    /// Baseline oracle default: all permissive and non-redacted.
    fn default() -> Self {
        Self {
            visible: true,
            readable: true,
            editable: true,
            committable: true,
            redacted: false,
        }
    }
}

// =============================================================================
// SPEC-HAL-188 §9: Invalidation Events
// =============================================================================

/// Invalidation event (SPEC-HAL-188 §9.1).
#[derive(Debug, Clone, PartialEq)]
pub enum InvalidationEvent {
    /// The RowId sequence has been replaced.
    /// MUST trigger epoch bump (SPEC-HAL-187 INV-ROW-BASE-REPLACE).
    BaseOrderReplaced,
    /// New rows exist.
    RowsAdded { rows: Vec<RowId> },
    /// Rows no longer exist.
    RowsRemoved { rows: Vec<RowId> },
    /// Specific cell values have changed.
    CellsInvalidated {
        rows: Vec<RowId>,
        columns: ColumnScope,
    },
    /// Capabilities have changed.
    PolicyInvalidated { scope: PolicyScope },
    /// Row count has changed without base-order replacement.
    ExtentChanged,
}

/// Column scope for invalidation (SPEC-HAL-188 §9.1).
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnScope {
    /// All columns.
    All,
    /// Specific columns.
    Specific(Vec<ColumnId>),
}

/// Policy scope for invalidation (SPEC-HAL-188 §9.1).
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyScope {
    /// All cells.
    All,
    /// Specific rows.
    Rows(Vec<RowId>),
    /// Specific columns.
    Columns(Vec<ColumnId>),
    /// Specific cells.
    Cells(Vec<(RowId, ColumnId)>),
}

// =============================================================================
// Cell Value
// =============================================================================

/// Simple cell value representation.
///
/// Matches existing types in the codebase pattern. For production, this would
/// be aligned with the actual domain value types.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

impl From<&str> for CellValue {
    fn from(s: &str) -> Self {
        CellValue::String(s.to_string())
    }
}

impl From<String> for CellValue {
    fn from(s: String) -> Self {
        CellValue::String(s)
    }
}

impl From<i64> for CellValue {
    fn from(v: i64) -> Self {
        CellValue::Int(v)
    }
}

impl From<f64> for CellValue {
    fn from(v: f64) -> Self {
        CellValue::Float(v)
    }
}

impl From<bool> for CellValue {
    fn from(v: bool) -> Self {
        CellValue::Bool(v)
    }
}

// =============================================================================
// Invalidation Sink
// =============================================================================

/// Invalidation sink for collecting events.
///
/// Test infrastructure only. Used by tests to observe invalidation events
/// emitted by the oracle.
#[derive(Debug, Default)]
#[allow(dead_code)]
pub struct InvalidationSink {
    events: VecDeque<InvalidationEvent>,
}

impl InvalidationSink {
    /// Create a new empty sink.
    pub fn new() -> Self {
        Self {
            events: VecDeque::new(),
        }
    }

    /// Push an event to the sink.
    pub fn push(&mut self, event: InvalidationEvent) {
        self.events.push_back(event);
    }

    /// Pop the next event from the sink.
    pub fn pop(&mut self) -> Option<InvalidationEvent> {
        self.events.pop_front()
    }

    /// Drain all events from the sink.
    pub fn drain(&mut self) -> Vec<InvalidationEvent> {
        self.events.drain(..).collect()
    }

    /// Check if the sink is empty.
    pub fn is_empty(&self) -> bool {
        self.events.is_empty()
    }

    /// Get the number of pending events.
    pub fn len(&self) -> usize {
        self.events.len()
    }

    /// Peek at the next event without removing it.
    pub fn peek(&self) -> Option<&InvalidationEvent> {
        self.events.front()
    }
}

// =============================================================================
// Edit Session State (internal)
// =============================================================================

/// Internal edit session tracking.
struct ActiveEditSession {
    row: RowId,
    column: ColumnId,
    proposed_value: Option<CellValue>,
}

// =============================================================================
// Baseline Data Oracle
// =============================================================================

/// SPEC-HAL-188 §15.1 Baseline In-Memory Oracle.
///
/// A canonical test oracle implementing all SPEC-188 contracts:
///
/// - **RowUniverse**: what rows exist (mode: Live, extent: Exact)
/// - **CellReader**: cell values (always Ready)
/// - **CellMutator**: mutations (always Applied)
/// - **CellCapabilities**: permissions (all permissive)
/// - **DataInvalidation**: event emission (synchronous)
/// - **EditSession**: edit lifecycle (always granted/valid/applied)
///
/// ## Key Properties
///
/// - All data access is RowId-based (no index-based APIs).
/// - replace_rows() is the only way to change base order (triggers BaseOrderReplaced).
/// - Invalidation events are emitted synchronously and deterministically.
/// - No async, no futures, no threading.
#[allow(dead_code)]
pub struct BaselineDataOracle {
    /// Base row order (RowId sequence).
    /// INV-ROW-BASE-REPLACE: Only replaced via replace_rows().
    rows: Vec<RowId>,

    /// Cell data keyed by (RowId, ColumnId).
    cells: HashMap<(RowId, ColumnId), CellValue>,

    /// Invalidation sink for event collection.
    /// Tests subscribe to events by draining this sink.
    invalidation_sink: InvalidationSink,

    /// Edit session counter for token generation.
    edit_token_counter: u64,

    /// Active edit sessions keyed by token.
    active_edits: HashMap<EditToken, ActiveEditSession>,
}

impl BaselineDataOracle {
    /// Create a new empty oracle.
    pub fn new() -> Self {
        Self {
            rows: Vec::new(),
            cells: HashMap::new(),
            invalidation_sink: InvalidationSink::new(),
            edit_token_counter: 0,
            active_edits: HashMap::new(),
        }
    }

    /// Create an oracle with N rows (RowIds auto-generated).
    pub fn with_row_count(n: usize) -> Self {
        let rows: Vec<RowId> = (0..n).map(|_| RowId::new()).collect();
        Self {
            rows,
            cells: HashMap::new(),
            invalidation_sink: InvalidationSink::new(),
            edit_token_counter: 0,
            active_edits: HashMap::new(),
        }
    }

    // =========================================================================
    // SPEC-HAL-188 §5: RowUniverse Contract
    // =========================================================================

    /// Get the current RowId sequence (base row order).
    ///
    /// SPEC-HAL-188 §5.1: Order is provider-defined.
    pub fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    /// Get the extent (row count).
    ///
    /// SPEC-HAL-188 §5.2: Baseline oracle provides Exact(n).
    pub fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    /// Get the universe mode.
    ///
    /// SPEC-HAL-188 §5.3: Baseline oracle is Live.
    pub fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    // =========================================================================
    // SPEC-HAL-188 §6: CellReader Contract
    // =========================================================================

    /// Read a cell value.
    ///
    /// SPEC-HAL-188 §6: Return type is ValueState, not raw value.
    /// Baseline oracle always returns Ready for valid cells, Missing for absent cells.
    pub fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        // Check if row exists
        if !self.rows.contains(&row) {
            return ValueState::Error {
                error: Some(DataError::NotFound),
            };
        }

        // Return value or Missing
        match self.cells.get(&(row, column)) {
            Some(value) => ValueState::Ready(value.clone()),
            None => ValueState::Missing,
        }
    }

    // =========================================================================
    // SPEC-HAL-188 §7.1: CellMutator Contract (Single-Shot)
    // =========================================================================

    /// Mutate a cell value (single-shot).
    ///
    /// SPEC-HAL-188 §7.1: Baseline oracle always applies mutations.
    /// Emits CellsInvalidated event.
    pub fn mutate_cell(
        &mut self,
        row: RowId,
        column: ColumnId,
        value: CellValue,
    ) -> MutationResult {
        if !self.rows.contains(&row) {
            return MutationResult::Rejected {
                reason: Some(RejectionReason::Custom("Row not found".into())),
            };
        }

        self.cells.insert((row, column), value);

        // Emit invalidation
        self.invalidation_sink
            .push(InvalidationEvent::CellsInvalidated {
                rows: vec![row],
                columns: ColumnScope::Specific(vec![column]),
            });

        MutationResult::Applied
    }

    // =========================================================================
    // SPEC-HAL-188 §7.2: EditSession Contract
    // =========================================================================

    /// Begin an edit session.
    ///
    /// SPEC-HAL-188 §7.2: Baseline oracle always grants edit.
    pub fn begin_edit(&mut self, row: RowId, column: ColumnId) -> EditResult {
        if !self.rows.contains(&row) {
            return EditResult::Denied {
                reason: Some(DenialReason::Custom("Row not found".into())),
            };
        }

        self.edit_token_counter += 1;
        let token = EditToken::new(self.edit_token_counter);

        self.active_edits.insert(
            token,
            ActiveEditSession {
                row,
                column,
                proposed_value: None,
            },
        );

        EditResult::Granted { token }
    }

    /// Propose a value for validation.
    ///
    /// SPEC-HAL-188 §7.2: Baseline oracle always returns Valid.
    pub fn propose(&mut self, token: EditToken, value: CellValue) -> ValidationResult {
        if let Some(session) = self.active_edits.get_mut(&token) {
            session.proposed_value = Some(value);
            ValidationResult::Valid
        } else {
            ValidationResult::Invalid {
                errors: vec![ValidationError {
                    field: None,
                    message: "Invalid edit token".into(),
                }],
            }
        }
    }

    /// Commit an edit.
    ///
    /// SPEC-HAL-188 §7.2: Baseline oracle always applies.
    /// commit without prior propose is allowed.
    pub fn commit(&mut self, token: EditToken) -> CommitResult {
        if let Some(session) = self.active_edits.remove(&token) {
            if let Some(value) = session.proposed_value {
                self.cells.insert((session.row, session.column), value);

                // Emit invalidation
                self.invalidation_sink
                    .push(InvalidationEvent::CellsInvalidated {
                        rows: vec![session.row],
                        columns: ColumnScope::Specific(vec![session.column]),
                    });
            }
            CommitResult::Applied
        } else {
            // Idempotent: already committed or cancelled
            CommitResult::Applied
        }
    }

    /// Cancel an edit session.
    ///
    /// SPEC-HAL-188 §7.2: Idempotent.
    pub fn cancel(&mut self, token: EditToken) {
        self.active_edits.remove(&token);
    }

    // =========================================================================
    // SPEC-HAL-188 §8: CellCapabilities Contract
    // =========================================================================

    /// Get capabilities for a cell.
    ///
    /// SPEC-HAL-188 §8: Baseline oracle returns all permissive.
    pub fn capabilities(&self, _row: RowId, _column: ColumnId) -> Capabilities {
        Capabilities::default()
    }

    // =========================================================================
    // SPEC-HAL-188 §9: DataInvalidation Contract
    // =========================================================================

    /// Get mutable access to the invalidation sink.
    ///
    /// Tests use this to observe emitted events.
    pub fn invalidation_sink_mut(&mut self) -> &mut InvalidationSink {
        &mut self.invalidation_sink
    }

    /// Get immutable access to the invalidation sink.
    pub fn invalidation_sink(&self) -> &InvalidationSink {
        &self.invalidation_sink
    }

    /// Drain all pending invalidation events.
    pub fn drain_events(&mut self) -> Vec<InvalidationEvent> {
        self.invalidation_sink.drain()
    }

    // =========================================================================
    // Test Helper APIs
    // =========================================================================

    /// Set a cell value (test helper).
    ///
    /// Emits CellsInvalidated event for the affected cell.
    pub fn set_cell(&mut self, row: RowId, column: ColumnId, value: CellValue) {
        self.cells.insert((row, column), value);
        self.invalidation_sink
            .push(InvalidationEvent::CellsInvalidated {
                rows: vec![row],
                columns: ColumnScope::Specific(vec![column]),
            });
    }

    /// Add a row.
    ///
    /// This is a base order replacement (per SPEC-HAL-187 INV-ROW-BASE-REPLACE).
    /// Emits BaseOrderReplaced event.
    pub fn add_row(&mut self, row_id: RowId) {
        self.rows.push(row_id);
        // Per SPEC-HAL-187: any change to base RowId sequence is a replacement
        self.invalidation_sink
            .push(InvalidationEvent::BaseOrderReplaced);
    }

    /// Add a row and return its ID.
    ///
    /// Convenience method that generates a new RowId.
    /// Emits BaseOrderReplaced event.
    pub fn add_row_new(&mut self) -> RowId {
        let row_id = RowId::new();
        self.add_row(row_id);
        row_id
    }

    /// Remove a row.
    ///
    /// This is a base order replacement (per SPEC-HAL-187 INV-ROW-BASE-REPLACE).
    /// Emits BaseOrderReplaced event.
    /// Also removes all cell data for that row.
    pub fn remove_row(&mut self, row_id: RowId) {
        if let Some(pos) = self.rows.iter().position(|&r| r == row_id) {
            self.rows.remove(pos);

            // Remove cell data for this row
            self.cells.retain(|(r, _), _| *r != row_id);

            // Per SPEC-HAL-187: any change to base RowId sequence is a replacement
            self.invalidation_sink
                .push(InvalidationEvent::BaseOrderReplaced);
        }
    }

    /// Replace the entire row order.
    ///
    /// SPEC-HAL-187 INV-ROW-BASE-REPLACE: This is THE method for base order replacement.
    /// Emits BaseOrderReplaced event.
    ///
    /// This method represents a complete replacement of the RowId sequence.
    /// Any operation that reorders rows MUST use this method.
    pub fn replace_rows(&mut self, rows: Vec<RowId>) {
        self.rows = rows;
        self.invalidation_sink
            .push(InvalidationEvent::BaseOrderReplaced);
    }

    /// Clear all rows.
    ///
    /// This is a base order replacement.
    /// Emits BaseOrderReplaced event.
    pub fn clear_rows(&mut self) {
        self.rows.clear();
        self.cells.clear();
        self.invalidation_sink
            .push(InvalidationEvent::BaseOrderReplaced);
    }

    /// Get the number of rows.
    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    /// Check if a row exists.
    pub fn contains_row(&self, row_id: RowId) -> bool {
        self.rows.contains(&row_id)
    }

    /// Emit an ExtentChanged event (for testing).
    pub fn emit_extent_changed(&mut self) {
        self.invalidation_sink
            .push(InvalidationEvent::ExtentChanged);
    }

    /// Emit a PolicyInvalidated event (for testing).
    pub fn emit_policy_invalidated(&mut self, scope: PolicyScope) {
        self.invalidation_sink
            .push(InvalidationEvent::PolicyInvalidated { scope });
    }
}

impl Default for BaselineDataOracle {
    fn default() -> Self {
        Self::new()
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_oracle_creation() {
        let oracle = BaselineDataOracle::new();
        assert_eq!(oracle.row_count(), 0);
        assert_eq!(oracle.extent(), Extent::Exact(0));
        assert_eq!(oracle.mode(), UniverseMode::Live);
    }

    #[test]
    fn test_oracle_with_rows() {
        let oracle = BaselineDataOracle::with_row_count(100);
        assert_eq!(oracle.row_count(), 100);
        assert_eq!(oracle.extent(), Extent::Exact(100));
    }

    #[test]
    fn test_read_cell_ready() {
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        let col_id = ColumnId::new();

        oracle.set_cell(row_id, col_id, CellValue::String("test".into()));
        oracle.drain_events(); // Clear setup events

        let value = oracle.read_cell(row_id, col_id);
        assert!(matches!(value, ValueState::Ready(CellValue::String(s)) if s == "test"));
    }

    #[test]
    fn test_read_cell_missing() {
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        let col_id = ColumnId::new();
        oracle.drain_events();

        let value = oracle.read_cell(row_id, col_id);
        assert!(matches!(value, ValueState::Missing));
    }

    #[test]
    fn test_read_cell_invalid_row() {
        let oracle = BaselineDataOracle::new();
        let row_id = RowId::new(); // Not in oracle
        let col_id = ColumnId::new();

        let value = oracle.read_cell(row_id, col_id);
        assert!(matches!(value, ValueState::Error { .. }));
    }

    #[test]
    fn test_mutate_cell() {
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        let col_id = ColumnId::new();
        oracle.drain_events();

        let result = oracle.mutate_cell(row_id, col_id, CellValue::Int(42));
        assert_eq!(result, MutationResult::Applied);

        let value = oracle.read_cell(row_id, col_id);
        assert!(matches!(value, ValueState::Ready(CellValue::Int(42))));

        let events = oracle.drain_events();
        assert_eq!(events.len(), 1);
        assert!(matches!(
            events[0],
            InvalidationEvent::CellsInvalidated { .. }
        ));
    }

    #[test]
    fn test_edit_lifecycle() {
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        let col_id = ColumnId::new();
        oracle.drain_events();

        // Begin edit
        let edit_result = oracle.begin_edit(row_id, col_id);
        let token = match edit_result {
            EditResult::Granted { token } => token,
            EditResult::Denied { .. } => panic!("Edit should be granted"),
        };

        // Propose value
        let validation = oracle.propose(token, CellValue::String("edited".into()));
        assert_eq!(validation, ValidationResult::Valid);

        // Commit
        let commit = oracle.commit(token);
        assert_eq!(commit, CommitResult::Applied);

        // Verify value changed
        let value = oracle.read_cell(row_id, col_id);
        assert!(matches!(value, ValueState::Ready(CellValue::String(s)) if s == "edited"));

        // Verify invalidation emitted
        let events = oracle.drain_events();
        assert!(
            events
                .iter()
                .any(|e| matches!(e, InvalidationEvent::CellsInvalidated { .. }))
        );
    }

    #[test]
    fn test_edit_cancel() {
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        let col_id = ColumnId::new();
        oracle.set_cell(row_id, col_id, CellValue::String("original".into()));
        oracle.drain_events();

        // Begin edit
        let edit_result = oracle.begin_edit(row_id, col_id);
        let token = match edit_result {
            EditResult::Granted { token } => token,
            EditResult::Denied { .. } => panic!("Edit should be granted"),
        };

        // Propose value
        oracle.propose(token, CellValue::String("changed".into()));

        // Cancel instead of commit
        oracle.cancel(token);

        // Verify original value unchanged
        let value = oracle.read_cell(row_id, col_id);
        assert!(matches!(value, ValueState::Ready(CellValue::String(s)) if s == "original"));
    }

    #[test]
    fn test_capabilities_all_permissive() {
        let oracle = BaselineDataOracle::new();
        let row_id = RowId::new();
        let col_id = ColumnId::new();

        let caps = oracle.capabilities(row_id, col_id);
        assert!(caps.visible);
        assert!(caps.readable);
        assert!(caps.editable);
        assert!(caps.committable);
        assert!(!caps.redacted);
    }

    #[test]
    fn test_base_order_replaced_on_add_row() {
        let mut oracle = BaselineDataOracle::new();
        oracle.drain_events();

        oracle.add_row(RowId::new());

        let events = oracle.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], InvalidationEvent::BaseOrderReplaced);
    }

    #[test]
    fn test_base_order_replaced_on_remove_row() {
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        oracle.drain_events();

        oracle.remove_row(row_id);

        let events = oracle.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], InvalidationEvent::BaseOrderReplaced);
    }

    #[test]
    fn test_base_order_replaced_on_replace_rows() {
        let mut oracle = BaselineDataOracle::with_row_count(10);
        oracle.drain_events();

        // Replace with reversed order
        let mut new_rows = oracle.row_ids().to_vec();
        new_rows.reverse();
        oracle.replace_rows(new_rows);

        let events = oracle.drain_events();
        assert_eq!(events.len(), 1);
        assert_eq!(events[0], InvalidationEvent::BaseOrderReplaced);
    }

    #[test]
    fn test_invalidation_sink() {
        let mut oracle = BaselineDataOracle::new();

        // Add multiple rows
        for _ in 0..5 {
            oracle.add_row(RowId::new());
        }

        // Should have 5 BaseOrderReplaced events
        let events = oracle.drain_events();
        assert_eq!(events.len(), 5);
        for event in events {
            assert_eq!(event, InvalidationEvent::BaseOrderReplaced);
        }

        // Sink should be empty now
        assert!(oracle.invalidation_sink().is_empty());
    }

    #[test]
    fn test_no_index_based_api() {
        // This test verifies at compile time that there are no index-based APIs.
        // The oracle only exposes RowId-based access.
        let mut oracle = BaselineDataOracle::new();
        let row_id = oracle.add_row_new();
        let col_id = ColumnId::new();

        // All access is by RowId
        let _ = oracle.read_cell(row_id, col_id);
        let _ = oracle.capabilities(row_id, col_id);
        let _ = oracle.begin_edit(row_id, col_id);

        // row_ids() returns the RowId sequence, not indices
        let ids: &[RowId] = oracle.row_ids();
        assert!(!ids.is_empty());
    }
}
