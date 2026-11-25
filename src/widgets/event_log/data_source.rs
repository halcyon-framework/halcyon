// SPEC-HAL-312 §4.3: Data Source Adapter

use std::sync::Arc;

use crate::widgets::tabular_view::column::ColumnId;
use crate::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use crate::widgets::tabular_view::row::RowId;

use super::types::{EventLogEntry, EventLogLevel};

/// Column role within EventLogDataSource.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ColumnRole {
    Severity,
    Timestamp,
    Source,
    Message,
}

/// Timestamp formatter — default produces `HH:MM:SS.mmm`.
pub type TimestampFormatter = Arc<dyn Fn(i64) -> String + Send + Sync>;

/// Default timestamp formatter: `HH:MM:SS.mmm` derived from epoch-ms, UTC.
pub fn default_timestamp_formatter() -> TimestampFormatter {
    Arc::new(|ts_ms: i64| {
        let total_seconds = ts_ms.div_euclid(1000);
        let ms = ts_ms.rem_euclid(1000) as u32;
        let seconds = total_seconds.rem_euclid(60) as u32;
        let minutes = total_seconds.div_euclid(60).rem_euclid(60) as u32;
        let hours = total_seconds.div_euclid(3600).rem_euclid(24) as u32;
        format!("{:02}:{:02}:{:02}.{:03}", hours, minutes, seconds, ms)
    })
}

/// EventLog data source.
///
/// Wraps a `Vec<EventLogEntry>` and exposes it via the `DataSource` trait
/// so that ListView can consume it. Maintains parallel `Vec<RowId>` for stable
/// identity and a `ColumnId → ColumnRole` map populated after column creation.
pub struct EventLogDataSource {
    entries: Vec<EventLogEntry>,
    row_ids: Vec<RowId>,
    /// RowId → entries index (kept in sync with entries).
    row_index: std::collections::HashMap<RowId, usize>,
    severity_column: Option<ColumnId>,
    timestamp_column: Option<ColumnId>,
    source_column: Option<ColumnId>,
    message_column: Option<ColumnId>,
    timestamp_formatter: TimestampFormatter,
    pending_events: Vec<DataInvalidationEvent>,
}

impl EventLogDataSource {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
            row_ids: Vec::new(),
            row_index: std::collections::HashMap::new(),
            severity_column: None,
            timestamp_column: None,
            source_column: None,
            message_column: None,
            timestamp_formatter: default_timestamp_formatter(),
            pending_events: Vec::new(),
        }
    }

    pub fn with_timestamp_formatter(mut self, fmt: TimestampFormatter) -> Self {
        self.timestamp_formatter = fmt;
        self
    }

    /// Called by EventLog after creating columns to register the mapping.
    pub(crate) fn set_columns(
        &mut self,
        severity: ColumnId,
        timestamp: ColumnId,
        source: ColumnId,
        message: ColumnId,
    ) {
        self.severity_column = Some(severity);
        self.timestamp_column = Some(timestamp);
        self.source_column = Some(source);
        self.message_column = Some(message);
    }

    /// Append an entry. Emits a `RowsAdded` invalidation.
    pub fn push(&mut self, entry: EventLogEntry) {
        let row_id = RowId::new();
        let idx = self.entries.len();
        self.entries.push(entry);
        self.row_ids.push(row_id);
        self.row_index.insert(row_id, idx);
        self.pending_events
            .push(DataInvalidationEvent::RowsAdded { rows: vec![row_id] });
    }

    /// Clear all entries. Emits `BaseOrderReplaced`.
    pub fn clear(&mut self) {
        self.entries.clear();
        self.row_ids.clear();
        self.row_index.clear();
        self.pending_events
            .push(DataInvalidationEvent::BaseOrderReplaced);
    }

    /// Read-only access to an entry by row id.
    pub fn get(&self, row_id: RowId) -> Option<&EventLogEntry> {
        self.row_index
            .get(&row_id)
            .and_then(|&i| self.entries.get(i))
    }

    /// Access the timestamp formatter (for structured copy).
    pub fn timestamp_formatter(&self) -> &TimestampFormatter {
        &self.timestamp_formatter
    }

    /// Determine which column role a `ColumnId` corresponds to.
    fn column_role(&self, column: ColumnId) -> Option<ColumnRole> {
        if Some(column) == self.severity_column {
            Some(ColumnRole::Severity)
        } else if Some(column) == self.timestamp_column {
            Some(ColumnRole::Timestamp)
        } else if Some(column) == self.source_column {
            Some(ColumnRole::Source)
        } else if Some(column) == self.message_column {
            Some(ColumnRole::Message)
        } else {
            None
        }
    }
}

impl Default for EventLogDataSource {
    fn default() -> Self {
        Self::new()
    }
}

impl DataSource for EventLogDataSource {
    fn row_ids(&self) -> &[RowId] {
        &self.row_ids
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.row_ids.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some(&idx) = self.row_index.get(&row) else {
            return ValueState::Missing;
        };
        let Some(entry) = self.entries.get(idx) else {
            return ValueState::Missing;
        };
        match self.column_role(column) {
            Some(ColumnRole::Severity) => {
                ValueState::Ready(CellValue::String(entry.level.tag().to_string()))
            }
            Some(ColumnRole::Timestamp) => ValueState::Ready(CellValue::String((self
                .timestamp_formatter)(
                entry.timestamp_ms,
            ))),
            Some(ColumnRole::Source) => ValueState::Ready(CellValue::String(entry.source.clone())),
            Some(ColumnRole::Message) => {
                ValueState::Ready(CellValue::String(entry.message.clone()))
            }
            None => ValueState::Missing,
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Look up the severity level for a row id, used by `SeverityPainter`.
pub trait SeverityLookup {
    fn level_for_row(&self, row_id: RowId) -> Option<EventLogLevel>;
}

impl SeverityLookup for EventLogDataSource {
    fn level_for_row(&self, row_id: RowId) -> Option<EventLogLevel> {
        self.get(row_id).map(|e| e.level)
    }
}

// ─── SharedEventLogSource ────────────────────────────────
//
// Pattern 1 newtype: wraps Rc<RefCell<EventLogDataSource>>, forwards DataSource.
// This lets ListView own the wrapper while EventLog and SeverityPainter hold
// clones of the same Rc for push/clear and severity lookup respectively.

use std::cell::RefCell;
use std::rc::Rc;

/// Shared wrapper around `EventLogDataSource` implementing `DataSource`.
///
/// Holds a cached copy of `row_ids` (refreshed on `drain_invalidation_events`)
/// so that `row_ids(&self) -> &[RowId]` can return a borrow from self rather
/// than from the inner RefCell.
pub struct SharedEventLogSource {
    pub inner: Rc<RefCell<EventLogDataSource>>,
    cached_row_ids: Vec<RowId>,
}

impl SharedEventLogSource {
    pub fn new(inner: Rc<RefCell<EventLogDataSource>>) -> Self {
        let cached_row_ids = inner.borrow().row_ids().to_vec();
        Self {
            inner,
            cached_row_ids,
        }
    }
}

impl DataSource for SharedEventLogSource {
    fn row_ids(&self) -> &[RowId] {
        &self.cached_row_ids
    }

    fn extent(&self) -> Extent {
        self.inner.borrow().extent()
    }

    fn mode(&self) -> UniverseMode {
        self.inner.borrow().mode()
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        self.inner.borrow().read_cell(row, column)
    }

    fn capabilities(&self, row: RowId, column: ColumnId) -> CellCapabilities {
        self.inner.borrow().capabilities(row, column)
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        let events = self.inner.borrow_mut().drain_invalidation_events();
        // Refresh cached row_ids after draining events (push/clear may have changed them).
        self.cached_row_ids = self.inner.borrow().row_ids().to_vec();
        events
    }
}
