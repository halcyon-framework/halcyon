// SPEC-HAL-312 §4: Data Model

use std::sync::atomic::{AtomicU64, Ordering};

/// Stable opaque identifier for an event log entry.
///
/// SPEC-HAL-312 §4.1: Identity is monotonic and never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventLogEntryId(pub u64);

impl EventLogEntryId {
    /// Generate a new unique ID.
    pub fn new() -> Self {
        static COUNTER: AtomicU64 = AtomicU64::new(1);
        EventLogEntryId(COUNTER.fetch_add(1, Ordering::Relaxed))
    }
}

/// Severity level per SPEC-HAL-312 §4.2.
///
/// Ordering is significant: Trace < Debug < Info < Warning < Error < Critical.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum EventLogLevel {
    Trace,
    Debug,
    Info,
    Warning,
    Error,
    Critical,
}

impl EventLogLevel {
    /// Short uppercase tag for the level (fixed width 4 chars for alignment).
    pub fn tag(self) -> &'static str {
        match self {
            EventLogLevel::Trace => "TRCE",
            EventLogLevel::Debug => "DBUG",
            EventLogLevel::Info => "INFO",
            EventLogLevel::Warning => "WARN",
            EventLogLevel::Error => "ERR ",
            EventLogLevel::Critical => "CRIT",
        }
    }
}

/// Event log entry (SPEC-HAL-312 §4.1).
#[derive(Debug, Clone)]
pub struct EventLogEntry {
    pub id: EventLogEntryId,
    pub level: EventLogLevel,
    /// Epoch milliseconds.
    pub timestamp_ms: i64,
    pub source: String,
    pub message: String,
    /// Optional detail text (Phase 2 feature — present in Phase 1 for API stability,
    /// not rendered).
    pub detail: Option<String>,
    /// Optional category tag (reserved for filter bar in Phase 3).
    pub category: Option<String>,
    /// Reserved for acknowledgement UI in a later phase.
    pub acknowledged: bool,
}

impl EventLogEntry {
    /// Convenience constructor with auto-generated ID.
    pub fn new(level: EventLogLevel, timestamp_ms: i64, source: String, message: String) -> Self {
        Self {
            id: EventLogEntryId::new(),
            level,
            timestamp_ms,
            source,
            message,
            detail: None,
            category: None,
            acknowledged: false,
        }
    }

    pub fn with_detail(mut self, detail: String) -> Self {
        self.detail = Some(detail);
        self
    }

    pub fn with_category(mut self, category: String) -> Self {
        self.category = Some(category);
        self
    }
}
