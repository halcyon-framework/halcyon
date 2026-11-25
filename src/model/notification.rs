// SPEC-HAL-270: Notification Log — Model Layer
//
// NotificationManager owns a fixed-capacity ring buffer of NotificationEntry
// values backed by crate::core::series::Series<NotificationEntry>.
// Oldest entries are evicted automatically when the buffer is full.
// No auto-dismiss. Entries live until explicitly removed or evicted.

use crate::core::series::Series;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NotificationId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationLevel {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone)]
pub struct NotificationAction {
    pub label: String,
}

#[derive(Debug, Clone)]
pub struct NotificationEntry {
    pub id: NotificationId,
    pub level: NotificationLevel,
    pub title: Option<String>,
    pub message: String,
    pub timestamp_ms: u64,
    pub action: Option<NotificationAction>,
}

// ============================================================================
// NotificationHandle
// ============================================================================

#[derive(Clone)]
pub struct NotificationHandle {
    pub id: NotificationId,
}

// ============================================================================
// NotificationManager
// ============================================================================

pub const DEFAULT_CAPACITY: usize = 256;

pub struct NotificationManager {
    log: Series<NotificationEntry>,
    next_id: u64,
}

impl NotificationManager {
    pub fn new() -> Self {
        Self::with_capacity(DEFAULT_CAPACITY)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            log: Series::new(capacity),
            next_id: 0,
        }
    }

    /// Push a new notification. Returns a handle with the assigned id.
    /// If the log is at capacity, the oldest entry is silently evicted.
    pub fn push(
        &mut self,
        level: NotificationLevel,
        message: impl Into<String>,
        title: Option<impl Into<String>>,
        action: Option<NotificationAction>,
        timestamp_ms: u64,
    ) -> NotificationHandle {
        let id = NotificationId(self.next_id);
        self.next_id += 1;

        let entry = NotificationEntry {
            id,
            level,
            title: title.map(|t| t.into()),
            message: message.into(),
            timestamp_ms,
            action,
        };

        self.log.push(entry);
        NotificationHandle { id }
    }

    /// Remove a single entry by id. No-op if not found.
    pub fn remove(&mut self, id: NotificationId) {
        // Series does not support arbitrary removal; rebuild without the entry.
        // This is an infrequent operation — allocation cost is acceptable.
        let mut retained: Vec<NotificationEntry> =
            self.log.iter().filter(|e| e.id != id).cloned().collect();
        // iter() is newest-first; re-push in oldest-first order to preserve ring semantics.
        retained.reverse();
        let cap = self.log.capacity();
        self.log = Series::new(cap);
        for entry in retained {
            self.log.push(entry);
        }
    }

    /// Remove all entries.
    pub fn clear(&mut self) {
        self.log = Series::new(self.log.capacity());
    }

    /// Iterate entries newest-first (Series::iter() contract).
    pub fn iter(&self) -> impl Iterator<Item = &NotificationEntry> {
        self.log.iter()
    }

    pub fn len(&self) -> usize {
        self.log.len()
    }

    pub fn is_empty(&self) -> bool {
        self.log.is_empty()
    }

    pub fn capacity(&self) -> usize {
        self.log.capacity()
    }
}

impl Default for NotificationManager {
    fn default() -> Self {
        Self::new()
    }
}
