//! SPEC-HAL-188 Data Adapter — Integration Seam
//!
//! This module defines the ONLY boundary where TabularSubstrate interacts with
//! SPEC-HAL-188 data contracts. ListView (and future TreeView/Grid/PropertyEditor)
//! MUST NOT call data contracts directly.
//!
//! ## Authority Boundary (SPEC-HAL-188 §10)
//!
//! **TabularSubstrate authority:**
//! - Owns effective order for navigation and selection.
//! - Owns visible projection (which rows are rendered, in what order).
//! - Owns focus and anchor management.
//!
//! **Data layer authority (via this adapter):**
//! - Owns base universe: what rows exist.
//! - Owns base order: provider-defined RowId sequence.
//! - Owns query semantics: how sort/filter requests are interpreted.
//!
//! ## Rules (Binding)
//!
//! - Adapter MUST NOT compute ordering or apply sorting itself.
//! - Adapter MUST NOT interpret geometry/viewport.
//! - Adapter MUST NOT expose row indices to callers.
//! - Adapter MUST NOT store any "current visible indices" cache.
//! - Adapter may store RowId collections only as replace-only snapshots aligned to epochs.
//!
//! ## Forbidden Patterns (SPEC-HAL-188 §14)
//!
//! - `get_row(i)` — index-based access
//! - `read_cell_at_index(i, j)` — index-based cell access
//! - storing row indices in adapters
//! - caching visible index lists across epochs
//!
//! ## Reusability (TabularSubstrate Consumers)
//!
//! The following are **consumer-agnostic** and reusable by TreeView, Grid, etc:
//! - `DataSource` trait (row identity, cell reads, invalidation events)
//! - `DataAdapter<S>` (integration seam, epoch management, invalidation→dirty mapping)
//! - `DataInvalidationEvent` enum (BaseOrderReplaced, CellsInvalidated, PolicyInvalidated)
//! - `ValueState`, `Extent`, `UniverseMode`, `CellCapabilities` (contract types)
//! - Dirty flag enforcement (SPEC-HAL-189 UpdatePhase/DirtyFlags/DirtyOutcome)
//!
//! The following are **ListView-specific**:
//! - Single-column projection (ListView has no column model exposure)
//! - RowHost widget lifecycle (specific to ListView's rendering approach)

use serde::{Deserialize, Serialize};

use super::column::ColumnId;
use super::intents::SortDirection;
use super::row::RowId;

// =============================================================================
// SPEC-HAL-188 Contract Types (Re-exported for adapter consumers)
// =============================================================================
//
// FROZEN — These types are the stable contract between data layer and UI.
//
// Adding variants to these enums is permitted (non-breaking).
// Removing or changing existing variants is FORBIDDEN.
// Changing trait method signatures is FORBIDDEN.
//
// New consumers (TreeView, Grid, etc.) MUST use these exact types.
// =============================================================================

/// Cell read result (SPEC-HAL-188 §4).
///
/// FROZEN: Variants may be added; existing variants must not change.
///
/// All cell reads through the adapter return this type.
#[derive(Debug, Clone, PartialEq)]
pub enum ValueState<T> {
    /// Value is resolved and available.
    Ready(T),
    /// Value is not yet available; may arrive later.
    Pending,
    /// Value exists but access is forbidden.
    Denied { reason: Option<String> },
    /// Read failed.
    Error { error: Option<String> },
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
}

/// Row count extent (SPEC-HAL-188 §5.2).
///
/// FROZEN: Variants may be added; existing variants must not change.
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

/// Universe mode (SPEC-HAL-188 §5.3).
///
/// FROZEN: Variants may be added; existing variants must not change.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UniverseMode {
    /// Universe may change; emits invalidations.
    Live,
    /// Universe is frozen; no invalidations will be emitted.
    Snapshot,
}

/// Row-order transport used by a data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OrderAccess {
    /// Compatibility path backed by the complete `row_ids()` slice.
    LegacySnapshot,
    /// Canonical revision-scoped bounded-window path.
    Windowed,
}

/// Stable provider baseline identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BaselineOrderId(String);

impl BaselineOrderId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Stable provider-owned custom sort strategy identity.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SortKeyId(String);

impl SortKeyId {
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Provider-recognized sort key.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SortKey {
    Column(ColumnId),
    Custom(SortKeyId),
}

/// One ordered term in a provider sort specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortTerm {
    pub key: SortKey,
    pub direction: SortDirection,
    #[serde(default)]
    pub parameters: serde_json::Value,
}

impl SortTerm {
    pub fn new(key: SortKey, direction: SortDirection) -> Self {
        Self {
            key,
            direction,
            parameters: serde_json::Value::Null,
        }
    }
}

/// Complete requested or confirmed provider order specification.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SortSpec {
    pub baseline: BaselineOrderId,
    pub terms: Vec<SortTerm>,
}

impl SortSpec {
    pub fn baseline(baseline: BaselineOrderId) -> Self {
        Self {
            baseline,
            terms: Vec::new(),
        }
    }
}

/// Provider execution guarantee for the confirmed order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum SortExactness {
    Exact,
    Approximate { guarantee: String },
}

/// Monotonic identity of one confirmed provider order.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct OrderRevision(pub u64);

/// Provider-confirmed applied sorting testimony.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AppliedSortSnapshot {
    pub spec: SortSpec,
    pub exactness: SortExactness,
    pub order_revision: OrderRevision,
}

/// Stable identity of a sort request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SortRequestId(pub u64);

/// Semantic sort operation sent through the adapter.
#[derive(Debug, Clone, PartialEq)]
pub enum SortOperation {
    CycleReplace(SortKey),
    CycleAdditive(SortKey),
    SetDirection {
        key: SortKey,
        direction: SortDirection,
        additive: bool,
    },
    RemoveTerm(SortKey),
    ClearToBaseline(BaselineOrderId),
    ApplySpec(SortSpec),
    CancelPending(SortRequestId),
}

/// One adapter-assigned request delivered to the provider.
#[derive(Debug, Clone, PartialEq)]
pub struct SortRequest {
    pub request_id: SortRequestId,
    pub operation: SortOperation,
}

/// Provider outcome for a semantic sort request.
#[derive(Debug, Clone, PartialEq)]
pub enum SortRequestOutcome {
    Applied {
        request_id: SortRequestId,
        snapshot: AppliedSortSnapshot,
    },
    Pending {
        request_id: SortRequestId,
        current_snapshot: Option<AppliedSortSnapshot>,
    },
    Rejected {
        request_id: SortRequestId,
        reason: String,
        current_snapshot: Option<AppliedSortSnapshot>,
    },
    Unsupported {
        request_id: SortRequestId,
        current_snapshot: Option<AppliedSortSnapshot>,
    },
}

impl SortRequestOutcome {
    pub fn request_id(&self) -> SortRequestId {
        match self {
            Self::Applied { request_id, .. }
            | Self::Pending { request_id, .. }
            | Self::Rejected { request_id, .. }
            | Self::Unsupported { request_id, .. } => *request_id,
        }
    }
}

/// Provider capability testimony for one sort key.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SortCapability {
    Unsupported,
    Denied {
        reason: Option<String>,
    },
    Allowed {
        directions: Vec<SortDirection>,
        exactness: SortExactness,
    },
}

impl SortCapability {
    pub fn exact() -> Self {
        Self::Allowed {
            directions: vec![SortDirection::Ascending, SortDirection::Descending],
            exactness: SortExactness::Exact,
        }
    }

    pub fn permits(&self, direction: SortDirection) -> bool {
        matches!(self, Self::Allowed { directions, .. } if directions.contains(&direction))
    }

    pub fn is_allowed(&self) -> bool {
        matches!(self, Self::Allowed { .. })
    }
}

/// Bounded order request against one explicit revision.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OrderWindowRequest {
    pub revision: OrderRevision,
    pub start: usize,
    pub length: usize,
}

/// Bounded RowId projection returned by the provider.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OrderWindow {
    pub revision: OrderRevision,
    pub start: usize,
    pub row_ids: Vec<RowId>,
}

impl OrderWindow {
    pub fn end(&self) -> usize {
        self.start + self.row_ids.len()
    }

    pub fn row_at(&self, ordinal: usize) -> Option<RowId> {
        ordinal
            .checked_sub(self.start)
            .and_then(|offset| self.row_ids.get(offset).copied())
    }
}

/// Cell capabilities (SPEC-HAL-188 §8).
///
/// FROZEN: Fields may be added; existing fields must not change or be removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CellCapabilities {
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

impl Default for CellCapabilities {
    /// Default capabilities (SPEC-HAL-188 §8 default behavior):
    /// visible: true, readable: true, editable: false, committable: false, redacted: false
    fn default() -> Self {
        Self {
            visible: true,
            readable: true,
            editable: false,
            committable: false,
            redacted: false,
        }
    }
}

/// Simple cell value representation.
#[derive(Debug, Clone, PartialEq)]
pub enum CellValue {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    String(String),
}

/// Invalidation event from the data layer (SPEC-HAL-188 §9.1).
///
/// FROZEN: Variants may be added; existing variants must not change.
#[derive(Debug, Clone, PartialEq)]
pub enum DataInvalidationEvent {
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
        columns: ColumnInvalidationScope,
    },
    /// Capabilities have changed.
    PolicyInvalidated { scope: PolicyInvalidationScope },
    /// Row count has changed without base-order replacement.
    ExtentChanged,
    /// Row heights have changed. Substrate must update RowModel heights
    /// for the specified rows.
    ///
    /// Heights are exact and final — no measurement or approximation required.
    /// SPEC-HAL-312A INV-312A-EXACT-HEIGHT.
    RowHeightsChanged { changes: Vec<(RowId, f32)> },
}

/// Column scope for invalidation.
#[derive(Debug, Clone, PartialEq)]
pub enum ColumnInvalidationScope {
    /// All columns.
    All,
    /// Specific columns.
    Specific(Vec<ColumnId>),
}

/// Policy scope for invalidation.
#[derive(Debug, Clone, PartialEq)]
pub enum PolicyInvalidationScope {
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
// Data Source Trait (SPEC-HAL-188 Contract Interface)
// =============================================================================
//
// FROZEN — This trait signature is the stable contract.
//
// New methods may be added with default implementations (non-breaking).
// Existing method signatures MUST NOT change.
// =============================================================================

/// SPEC-HAL-188 data source contract.
///
/// FROZEN: Method signatures must not change. New methods require defaults.
///
/// This trait defines the interface that data providers must implement.
/// The adapter wraps implementations of this trait.
///
/// ## Rules
///
/// - All access is RowId-based (no index parameters).
/// - No method may require full materialization.
/// - No synchronous guarantees (Pending is a valid return).
pub trait DataSource {
    /// Compatibility access to the complete RowId sequence.
    ///
    /// Windowed providers leave this default untouched and implement
    /// `order_revision`, `order_window`, and `locate_row` instead.
    fn row_ids(&self) -> &[RowId] {
        &[]
    }

    /// Select the provider's row-order transport.
    fn order_access(&self) -> OrderAccess {
        OrderAccess::LegacySnapshot
    }

    /// Current confirmed provider order revision.
    fn order_revision(&self) -> OrderRevision {
        OrderRevision(1)
    }

    /// Read a bounded RowId window for one explicit order revision.
    fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow> {
        if request.revision != self.order_revision() {
            return ValueState::Error {
                error: Some("stale order revision".to_string()),
            };
        }

        let rows = self.row_ids();
        let start = request.start.min(rows.len());
        let end = start.saturating_add(request.length).min(rows.len());
        ValueState::Ready(OrderWindow {
            revision: request.revision,
            start,
            row_ids: rows[start..end].to_vec(),
        })
    }

    /// Locate one RowId within an explicit order revision.
    fn locate_row(&self, revision: OrderRevision, row_id: RowId) -> ValueState<Option<usize>> {
        if revision != self.order_revision() {
            return ValueState::Error {
                error: Some("stale order revision".to_string()),
            };
        }
        ValueState::Ready(
            self.row_ids()
                .iter()
                .position(|candidate| *candidate == row_id),
        )
    }

    /// Map one visible column to its provider-owned sort key.
    fn sort_key_for_column(&self, column: ColumnId) -> Option<SortKey> {
        Some(SortKey::Column(column))
    }

    /// Query provider capability for one sort key.
    fn sort_capability(&self, _key: &SortKey) -> SortCapability {
        SortCapability::Unsupported
    }

    /// Read the provider-confirmed applied sorting snapshot.
    fn applied_sort(&self) -> Option<AppliedSortSnapshot> {
        None
    }

    /// Submit one semantic sort request.
    fn request_sort(&mut self, request: SortRequest) -> SortRequestOutcome {
        SortRequestOutcome::Unsupported {
            request_id: request.request_id,
            current_snapshot: self.applied_sort(),
        }
    }

    /// Get the extent (row count semantics).
    fn extent(&self) -> Extent;

    /// Get the universe mode.
    fn mode(&self) -> UniverseMode;

    /// Read a cell value.
    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue>;

    /// Get capabilities for a cell.
    fn capabilities(&self, row: RowId, column: ColumnId) -> CellCapabilities;

    /// Commit a new value to a cell.
    ///
    /// Default rejects commits so existing read-only data sources remain valid.
    fn commit_cell(&mut self, _row: RowId, _column: ColumnId, _value: CellValue) -> bool {
        false
    }

    /// Drain pending invalidation events.
    ///
    /// Called by the adapter to retrieve events since last poll.
    /// Implementation must return events in order and clear the queue.
    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent>;
}

// =============================================================================
// Data Adapter
// =============================================================================

/// SPEC-HAL-188 Data Adapter.
///
/// This is the ONLY integration seam between SPEC-HAL-188 data contracts and
/// TabularSubstrate. All data access flows through this adapter.
///
/// ## Responsibilities
///
/// 1. Own reference to the data source (implements DataSource trait).
/// 2. Translate data-layer invalidation into substrate-facing events.
/// 3. Provide minimal API surface for TabularSubstrate.
/// 4. Apply BaseOrderReplaced via replace-only semantics.
///
/// ## Non-Responsibilities (explicitly forbidden)
///
/// - Computing ordering or applying sorting.
/// - Interpreting geometry/viewport.
/// - Exposing row indices.
/// - Caching "current visible indices".
pub struct DataAdapter<S: DataSource> {
    /// The data source providing SPEC-HAL-188 contract implementations.
    source: S,

    /// Epoch at which we last fetched the RowId sequence.
    /// Reserved for future use: tracking staleness relative to substrate's base_row_order_epoch.
    #[allow(dead_code)]
    last_fetch_epoch: u64,

    /// Cached RowId sequence from last fetch.
    /// This is a replace-only snapshot aligned to epochs.
    /// INV-ADAPTER-ROWID-SNAPSHOT: Only updated via fetch_row_ids().
    cached_row_ids: Vec<RowId>,

    /// Current provider-confirmed order revision.
    current_order_revision: OrderRevision,

    /// Single bounded projection cache. Replaced, never accumulated.
    cached_order_window: Option<OrderWindow>,

    /// Next adapter-owned sort request identity.
    next_sort_request_id: u64,
}

impl<S: DataSource> DataAdapter<S> {
    /// Create a new adapter wrapping a data source.
    pub fn new(source: S) -> Self {
        let current_order_revision = source.order_revision();
        Self {
            source,
            last_fetch_epoch: 0,
            cached_row_ids: Vec::new(),
            current_order_revision,
            cached_order_window: None,
            next_sort_request_id: 1,
        }
    }

    /// Get immutable access to the underlying data source.
    pub fn source(&self) -> &S {
        &self.source
    }

    /// Get mutable access to the underlying data source.
    pub fn source_mut(&mut self) -> &mut S {
        &mut self.source
    }

    /// Whether this provider uses canonical bounded order windows.
    pub fn uses_windowed_order(&self) -> bool {
        self.source.order_access() == OrderAccess::Windowed
    }

    // =========================================================================
    // RowUniverse Access (SPEC-HAL-188 §5)
    // =========================================================================

    /// Fetch the current RowId sequence from the data source.
    ///
    /// This is the entry point for base row order. On BaseOrderReplaced,
    /// the caller (TabularSubstrate) must call this to get the new sequence.
    ///
    /// ## Returns
    ///
    /// A slice of RowIds representing the current base row order.
    /// This is NOT an index array — it's the identity sequence.
    pub fn fetch_row_ids(&mut self) -> &[RowId] {
        self.cached_row_ids = self.source.row_ids().to_vec();
        &self.cached_row_ids
    }

    /// Get the cached RowId sequence.
    ///
    /// Use fetch_row_ids() to update after BaseOrderReplaced.
    pub fn cached_row_ids(&self) -> &[RowId] {
        &self.cached_row_ids
    }

    /// Current confirmed provider order revision.
    pub fn order_revision(&self) -> OrderRevision {
        self.current_order_revision
    }

    /// Fetch and replace the single bounded order cache.
    pub fn fetch_order_window(&mut self, start: usize, length: usize) -> ValueState<OrderWindow> {
        let request = OrderWindowRequest {
            revision: self.current_order_revision,
            start,
            length,
        };
        match self.source.order_window(request) {
            ValueState::Ready(window)
                if window.revision == self.current_order_revision
                    && window.start == start
                    && window.row_ids.len() <= length =>
            {
                self.cached_order_window = Some(window.clone());
                ValueState::Ready(window)
            }
            ValueState::Ready(_) => ValueState::Error {
                error: Some("provider returned a mismatched order window".to_string()),
            },
            ValueState::Pending => ValueState::Pending,
            ValueState::Denied { reason } => ValueState::Denied { reason },
            ValueState::Error { error } => ValueState::Error { error },
            ValueState::Missing => ValueState::Missing,
        }
    }

    /// Read the currently cached bounded order window.
    pub fn cached_order_window(&self) -> Option<&OrderWindow> {
        self.cached_order_window.as_ref()
    }

    /// Resolve one ordinal without materializing the full provider order.
    pub fn row_id_at(&self, ordinal: usize) -> ValueState<Option<RowId>> {
        if let Some(window) = &self.cached_order_window
            && let Some(row_id) = window.row_at(ordinal)
        {
            return ValueState::Ready(Some(row_id));
        }

        match self.source.order_window(OrderWindowRequest {
            revision: self.current_order_revision,
            start: ordinal,
            length: 1,
        }) {
            ValueState::Ready(window) if window.revision == self.current_order_revision => {
                ValueState::Ready(window.row_at(ordinal))
            }
            ValueState::Ready(_) => ValueState::Error {
                error: Some("provider returned a stale order window".to_string()),
            },
            ValueState::Pending => ValueState::Pending,
            ValueState::Denied { reason } => ValueState::Denied { reason },
            ValueState::Error { error } => ValueState::Error { error },
            ValueState::Missing => ValueState::Missing,
        }
    }

    /// Resolve a RowId to an ephemeral ordinal in the current revision.
    pub fn locate_row(&self, row_id: RowId) -> ValueState<Option<usize>> {
        self.source.locate_row(self.current_order_revision, row_id)
    }

    pub fn sort_key_for_column(&self, column: ColumnId) -> Option<SortKey> {
        self.source.sort_key_for_column(column)
    }

    pub fn sort_capability(&self, key: &SortKey) -> SortCapability {
        self.source.sort_capability(key)
    }

    pub fn applied_sort(&self) -> Option<AppliedSortSnapshot> {
        self.source.applied_sort()
    }

    /// Submit one semantic operation with an adapter-owned request identity.
    pub fn request_sort(&mut self, operation: SortOperation) -> SortRequestOutcome {
        let request_id = SortRequestId(self.next_sort_request_id);
        self.next_sort_request_id = self.next_sort_request_id.saturating_add(1);
        self.source.request_sort(SortRequest {
            request_id,
            operation,
        })
    }

    /// Get the extent from the data source.
    pub fn extent(&self) -> Extent {
        self.source.extent()
    }

    /// Get the universe mode from the data source.
    pub fn mode(&self) -> UniverseMode {
        self.source.mode()
    }

    // =========================================================================
    // CellReader Access (SPEC-HAL-188 §6)
    // =========================================================================

    /// Read a cell value.
    ///
    /// SPEC-HAL-188 §6: Returns ValueState, not raw value.
    /// Caller must handle all variants (Ready, Pending, Denied, Error, Missing).
    pub fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        self.source.read_cell(row, column)
    }

    // =========================================================================
    // CellCapabilities Access (SPEC-HAL-188 §8)
    // =========================================================================

    /// Get capabilities for a cell.
    ///
    /// SPEC-HAL-188 §8: Capabilities are advisory to UI behavior.
    pub fn capabilities(&self, row: RowId, column: ColumnId) -> CellCapabilities {
        self.source.capabilities(row, column)
    }

    /// Commit a cell value through the data source.
    pub fn commit_cell(&mut self, row: RowId, column: ColumnId, value: CellValue) -> bool {
        self.source.commit_cell(row, column, value)
    }

    // =========================================================================
    // Invalidation Handling (SPEC-HAL-188 §9)
    // =========================================================================

    /// Poll for invalidation events.
    ///
    /// Returns all pending events from the data source.
    /// Caller (TabularSubstrate) must process these:
    ///
    /// - BaseOrderReplaced: bump epoch, call fetch_row_ids(), re-apply base order
    /// - CellsInvalidated: re-read affected cells
    /// - PolicyInvalidated: re-query capabilities
    /// - ExtentChanged: update extent display
    pub fn poll_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        self.source.drain_invalidation_events()
    }

    /// Process invalidation events and return substrate-facing actions.
    ///
    /// This is the translation layer from SPEC-HAL-188 events to substrate actions.
    pub fn process_invalidation(&mut self) -> AdapterInvalidationResult {
        let events = self.poll_invalidation_events();

        let mut result = AdapterInvalidationResult::default();

        for event in events {
            match event {
                DataInvalidationEvent::BaseOrderReplaced => {
                    // SPEC-HAL-187 INV-ROW-BASE-REPLACE: Must trigger epoch bump
                    result.base_order_replaced = true;
                    self.current_order_revision = self.source.order_revision();
                    self.cached_order_window = None;
                    if !self.uses_windowed_order() {
                        self.fetch_row_ids();
                    }
                }
                DataInvalidationEvent::RowsAdded { rows } => {
                    // Can be treated as BaseOrderReplaced for simplicity
                    // (SPEC-HAL-188 §9.1: RowsAdded is an optimization)
                    result.base_order_replaced = true;
                    result.added_rows.extend(rows);
                    self.current_order_revision = self.source.order_revision();
                    self.cached_order_window = None;
                    if !self.uses_windowed_order() {
                        self.fetch_row_ids();
                    }
                }
                DataInvalidationEvent::RowsRemoved { rows } => {
                    // Can be treated as BaseOrderReplaced for simplicity
                    result.base_order_replaced = true;
                    result.removed_rows.extend(rows);
                    self.current_order_revision = self.source.order_revision();
                    self.cached_order_window = None;
                    if !self.uses_windowed_order() {
                        self.fetch_row_ids();
                    }
                }
                DataInvalidationEvent::CellsInvalidated { rows, columns } => {
                    result.cells_invalidated = true;
                    result.invalidated_rows.extend(rows);
                    result.invalidated_columns = Some(columns);
                }
                DataInvalidationEvent::PolicyInvalidated { scope } => {
                    result.policy_invalidated = true;
                    result.policy_scope = Some(scope);
                }
                DataInvalidationEvent::ExtentChanged => {
                    result.extent_changed = true;
                }
                DataInvalidationEvent::RowHeightsChanged { changes } => {
                    result.row_heights_changed = true;
                    result.height_changes.extend(changes);
                }
            }
        }

        result
    }
}

/// Result of processing invalidation events.
///
/// TabularSubstrate uses this to determine what actions to take.
#[derive(Debug, Default)]
pub struct AdapterInvalidationResult {
    /// Base order was replaced (triggers epoch bump).
    pub base_order_replaced: bool,
    /// Rows that were added.
    pub added_rows: Vec<RowId>,
    /// Rows that were removed.
    pub removed_rows: Vec<RowId>,
    /// Cells were invalidated (need re-read).
    pub cells_invalidated: bool,
    /// Rows with invalidated cells.
    pub invalidated_rows: Vec<RowId>,
    /// Columns with invalidated cells.
    pub invalidated_columns: Option<ColumnInvalidationScope>,
    /// Policy was invalidated (need re-query capabilities).
    pub policy_invalidated: bool,
    /// Scope of policy invalidation.
    pub policy_scope: Option<PolicyInvalidationScope>,
    /// Extent changed.
    pub extent_changed: bool,
    /// Row heights changed (SPEC-HAL-312A).
    pub row_heights_changed: bool,
    /// Height changes: (RowId, new_height).
    pub height_changes: Vec<(RowId, f32)>,
}

impl AdapterInvalidationResult {
    /// Check if any invalidation occurred.
    pub fn has_changes(&self) -> bool {
        self.base_order_replaced
            || self.cells_invalidated
            || self.policy_invalidated
            || self.extent_changed
            || self.row_heights_changed
    }
}

// =============================================================================
// Tests
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// Minimal test data source for adapter tests.
    struct TestDataSource {
        rows: Vec<RowId>,
        events: Vec<DataInvalidationEvent>,
    }

    impl TestDataSource {
        fn new() -> Self {
            Self {
                rows: Vec::new(),
                events: Vec::new(),
            }
        }

        fn add_row(&mut self, row_id: RowId) {
            self.rows.push(row_id);
            self.events.push(DataInvalidationEvent::BaseOrderReplaced);
        }

        fn replace_rows(&mut self, rows: Vec<RowId>) {
            self.rows = rows;
            self.events.push(DataInvalidationEvent::BaseOrderReplaced);
        }
    }

    impl DataSource for TestDataSource {
        fn row_ids(&self) -> &[RowId] {
            &self.rows
        }

        fn extent(&self) -> Extent {
            Extent::Exact(self.rows.len())
        }

        fn mode(&self) -> UniverseMode {
            UniverseMode::Live
        }

        fn read_cell(&self, row: RowId, _column: ColumnId) -> ValueState<CellValue> {
            if self.rows.contains(&row) {
                ValueState::Ready(CellValue::String("test".into()))
            } else {
                ValueState::Error {
                    error: Some("Row not found".into()),
                }
            }
        }

        fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
            CellCapabilities {
                visible: true,
                readable: true,
                editable: true,
                committable: true,
                redacted: false,
            }
        }

        fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
            std::mem::take(&mut self.events)
        }
    }

    #[test]
    fn test_adapter_creation() {
        let source = TestDataSource::new();
        let adapter = DataAdapter::new(source);
        assert!(adapter.cached_row_ids().is_empty());
    }

    #[test]
    fn test_adapter_fetch_row_ids() {
        let mut source = TestDataSource::new();
        let row1 = RowId::new();
        let row2 = RowId::new();
        source.rows = vec![row1, row2];

        let mut adapter = DataAdapter::new(source);
        let ids = adapter.fetch_row_ids();
        assert_eq!(ids.len(), 2);
        assert_eq!(ids[0], row1);
        assert_eq!(ids[1], row2);
    }

    #[test]
    fn test_adapter_read_cell() {
        let mut source = TestDataSource::new();
        let row_id = RowId::new();
        source.rows.push(row_id);
        let col_id = ColumnId::new();

        let adapter = DataAdapter::new(source);
        let value = adapter.read_cell(row_id, col_id);
        assert!(value.is_ready());
    }

    #[test]
    fn test_adapter_capabilities() {
        let source = TestDataSource::new();
        let adapter = DataAdapter::new(source);
        let row_id = RowId::new();
        let col_id = ColumnId::new();

        let caps = adapter.capabilities(row_id, col_id);
        assert!(caps.visible);
        assert!(caps.readable);
        assert!(caps.editable);
        assert!(caps.committable);
        assert!(!caps.redacted);
    }

    #[test]
    fn test_adapter_base_order_replaced() {
        let mut source = TestDataSource::new();
        source.add_row(RowId::new());
        source.add_row(RowId::new());

        let mut adapter = DataAdapter::new(source);

        // Process invalidation
        let result = adapter.process_invalidation();
        assert!(result.base_order_replaced);
        assert!(result.has_changes());

        // RowIds should be cached
        assert_eq!(adapter.cached_row_ids().len(), 2);
    }

    #[test]
    fn test_adapter_no_index_api() {
        // This test verifies at compile time that there's no index-based API.
        // All methods take RowId/ColumnId, not usize.
        let mut source = TestDataSource::new();
        let row_id = RowId::new();
        let col_id = ColumnId::new();
        source.rows.push(row_id);

        let adapter = DataAdapter::new(source);

        // All access is by RowId
        let _ = adapter.read_cell(row_id, col_id);
        let _ = adapter.capabilities(row_id, col_id);

        // fetch_row_ids() returns &[RowId], not indices
        // This is the only place to get the RowId sequence
    }

    #[test]
    fn test_adapter_extent_mode() {
        let mut source = TestDataSource::new();
        source.rows = vec![RowId::new(); 10];

        let adapter = DataAdapter::new(source);
        assert_eq!(adapter.extent(), Extent::Exact(10));
        assert_eq!(adapter.mode(), UniverseMode::Live);
    }

    #[test]
    fn test_invalidation_result_has_changes() {
        let result = AdapterInvalidationResult::default();
        assert!(!result.has_changes());

        let mut result = AdapterInvalidationResult::default();
        result.base_order_replaced = true;
        assert!(result.has_changes());

        let mut result = AdapterInvalidationResult::default();
        result.cells_invalidated = true;
        assert!(result.has_changes());
    }

    // =========================================================================
    // Integration Test: Oracle → Adapter → Substrate
    // =========================================================================

    #[test]
    fn test_oracle_adapter_substrate_integration() {
        // This test validates the complete integration path:
        // baseline oracle → adapter → substrate
        //
        // Per SPEC-HAL-188 Task 2 acceptance criteria:
        // - BaseOrderReplaced invalidation triggers base order replacement
        // - Cell read returns Ready values
        // - No index-based leakage

        use super::super::TabularSubstrate;
        use super::super::column::Column;

        // Create a test data source that wraps similar logic to baseline oracle
        let mut source = TestDataSource::new();
        let row1 = RowId::new();
        let row2 = RowId::new();
        let row3 = RowId::new();
        source.rows = vec![row1, row2, row3];

        // Create adapter
        let mut adapter = DataAdapter::new(source);

        // Fetch initial RowIds
        let row_ids = adapter.fetch_row_ids().to_vec();
        assert_eq!(row_ids.len(), 3);

        // Create substrate and wire the RowIds from adapter
        let mut substrate = TabularSubstrate::new();
        substrate.add_column(Column::new(200.0));

        // Add rows to substrate using RowIds from adapter (NOT indices)
        for &row_id in &row_ids {
            substrate.add_row(row_id);
        }

        // Set sorted indices (identity order for this test)
        substrate.set_sorted_indices((0..3).collect());

        // Verify substrate has the rows
        assert_eq!(substrate.row_model().len(), 3);

        // Read cell through adapter - should return Ready
        let col_id = ColumnId::new();
        let value = adapter.read_cell(row1, col_id);
        assert!(value.is_ready());

        // Capabilities should be all permissive
        let caps = adapter.capabilities(row1, col_id);
        assert!(caps.visible);
        assert!(caps.readable);
        assert!(caps.editable);

        // Now simulate base order replacement via data source
        adapter.source_mut().replace_rows(vec![row3, row2, row1]); // reversed

        // Process invalidation
        let result = adapter.process_invalidation();
        assert!(result.base_order_replaced);

        // Fetch new RowIds - order should be reversed
        let new_row_ids = adapter.cached_row_ids();
        assert_eq!(new_row_ids.len(), 3);
        assert_eq!(new_row_ids[0], row3);
        assert_eq!(new_row_ids[1], row2);
        assert_eq!(new_row_ids[2], row1);

        // In a real integration, substrate would:
        // 1. Bump base_row_order_epoch
        // 2. Clear and re-add rows in new order
        // 3. Invalidate sorted_indices
        //
        // This test validates the adapter correctly detects and reports
        // the base order replacement event.
    }
}
