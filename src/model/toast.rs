// SPEC-HAL-269: Toast Notification System — Model Layer
//
// ToastManager is the sole owner of the toast queue (INV-TM-OWNER).
// All timing uses cx.frame_time_ms() (INV-ANIM-TIMER).
// Queue mutations only in Phase 2 (INV-DEFERRED-MUT).

use std::cell::Cell;
use std::rc::Rc;

// ============================================================================
// Types
// ============================================================================

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ToastId(u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DismissState {
    Pending,
    Visible,
    Exiting,
    Dismissed,
}

pub struct ToastAction {
    pub label: String,
}

pub struct ToastEntry {
    pub id: ToastId,
    pub message: String,
    pub duration_ms: u64,
    pub action: Option<ToastAction>,
    pub show_at_ms: Cell<Option<u64>>,
    pub dismiss_state: Cell<DismissState>,
    pub exit_start_ms: Cell<Option<u64>>,
    pub paused_ms: Cell<u64>,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ToastStats {
    pub total_pushed: u64,
    pub next_id: u64,
    pub queue_len: usize,
    pub pending: usize,
    pub visible: usize,
    pub exiting: usize,
    pub dismissed: usize,
    pub active: usize,
    pub render_tracked: usize,
    pub capacity_evictions: u64,
    pub flushed_dismissed: u64,
}

// ============================================================================
// ToastHandle
// ============================================================================

#[derive(Clone)]
pub struct ToastHandle {
    pub id: ToastId,
    inner: Rc<ToastEntry>,
}

impl ToastHandle {
    pub fn is_dismissed(&self) -> bool {
        self.inner.dismiss_state.get() == DismissState::Dismissed
    }

    pub fn dismiss(&self) {
        if self.inner.dismiss_state.get() == DismissState::Visible {
            self.inner.dismiss_state.set(DismissState::Exiting);
        }
    }
}

// ============================================================================
// ToastManager
// ============================================================================

const MAX_VISIBLE: usize = 5;

pub struct ToastManager {
    queue: Vec<Rc<ToastEntry>>,
    next_id: u64,
    total_pushed: u64,
    capacity_evictions: u64,
    flushed_dismissed: u64,
}

impl ToastManager {
    pub fn new() -> Self {
        Self {
            queue: Vec::new(),
            next_id: 0,
            total_pushed: 0,
            capacity_evictions: 0,
            flushed_dismissed: 0,
        }
    }

    /// Push a new toast. Enforces INV-TM-MAX: if 5 visible entries exist,
    /// transitions the oldest visible entry to Exiting before appending.
    pub fn push(
        &mut self,
        message: impl Into<String>,
        duration_ms: u64,
        action: Option<ToastAction>,
    ) -> ToastHandle {
        self.flush_dismissed();

        // INV-TM-MAX: evict oldest visible if at capacity
        let mut visible_count = self
            .queue
            .iter()
            .filter(|e| {
                matches!(
                    e.dismiss_state.get(),
                    DismissState::Pending | DismissState::Visible
                )
            })
            .count();
        while visible_count >= MAX_VISIBLE {
            // Find oldest visible entry and transition to Exiting
            if let Some(oldest) = self.queue.iter().find(|e| {
                matches!(
                    e.dismiss_state.get(),
                    DismissState::Pending | DismissState::Visible
                )
            }) {
                oldest.dismiss_state.set(DismissState::Exiting);
                self.capacity_evictions += 1;
                visible_count -= 1;
            } else {
                break;
            }
        }

        let id = ToastId(self.next_id);
        self.next_id += 1;
        self.total_pushed += 1;

        let entry = Rc::new(ToastEntry {
            id,
            message: message.into(),
            duration_ms,
            action,
            show_at_ms: Cell::new(None),
            dismiss_state: Cell::new(DismissState::Pending),
            exit_start_ms: Cell::new(None),
            paused_ms: Cell::new(0),
        });

        let handle = ToastHandle {
            id,
            inner: Rc::clone(&entry),
        };
        self.queue.push(entry);
        handle
    }

    pub fn dismiss(&mut self, id: ToastId) {
        for entry in &self.queue {
            if entry.id == id && entry.dismiss_state.get() == DismissState::Visible {
                entry.dismiss_state.set(DismissState::Exiting);
            }
        }
    }

    pub fn dismiss_all(&mut self) {
        for entry in &self.queue {
            if matches!(
                entry.dismiss_state.get(),
                DismissState::Pending | DismissState::Visible
            ) {
                entry.dismiss_state.set(DismissState::Exiting);
            }
        }
    }

    /// Remove all Dismissed entries from the queue.
    /// MUST only be called during Phase 2 (Deferred Mutations).
    pub fn flush_dismissed(&mut self) {
        let before = self.queue.len();
        self.queue
            .retain(|e| e.dismiss_state.get() != DismissState::Dismissed);
        self.flushed_dismissed += before.saturating_sub(self.queue.len()) as u64;
    }

    /// True if any entry is Pending, Visible, or Exiting (drives needs_animation).
    pub fn has_active(&self) -> bool {
        self.queue
            .iter()
            .any(|e| e.dismiss_state.get() != DismissState::Dismissed)
    }

    /// Entries in render order: newest-first (LIFO visual, capped by INV-TM-MAX).
    pub fn visible_entries(&self) -> Vec<Rc<ToastEntry>> {
        self.queue
            .iter()
            .filter(|e| {
                matches!(
                    e.dismiss_state.get(),
                    DismissState::Visible | DismissState::Exiting
                )
            })
            .cloned()
            .rev() // newest first
            .take(MAX_VISIBLE)
            .collect()
    }

    /// All active entries (including Pending) for layout measurement.
    pub fn active_entries(&self) -> Vec<Rc<ToastEntry>> {
        self.queue
            .iter()
            .filter(|e| {
                matches!(
                    e.dismiss_state.get(),
                    DismissState::Pending | DismissState::Visible | DismissState::Exiting
                )
            })
            .cloned()
            .rev() // newest first
            .take(MAX_VISIBLE)
            .collect()
    }

    /// Look up an entry by id.
    pub fn entry_by_id(&self, id: ToastId) -> Option<Rc<ToastEntry>> {
        self.queue.iter().find(|e| e.id == id).cloned()
    }

    pub fn advance_lifecycle(
        &self,
        now_ms: u64,
        paused: bool,
        pause_delta_ms: u64,
        exit_duration_ms: u64,
    ) {
        for entry in &self.queue {
            let state = entry.dismiss_state.get();
            if state == DismissState::Dismissed {
                continue;
            }

            if entry.show_at_ms.get().is_none() {
                entry.show_at_ms.set(Some(now_ms));
                if state == DismissState::Pending {
                    entry.dismiss_state.set(DismissState::Visible);
                }
            }

            let show_at = entry.show_at_ms.get().unwrap_or(now_ms);
            let current_state = entry.dismiss_state.get();

            if current_state == DismissState::Visible && entry.duration_ms > 0 {
                if paused {
                    entry
                        .paused_ms
                        .set(entry.paused_ms.get().saturating_add(pause_delta_ms));
                }

                let effective_elapsed = now_ms
                    .saturating_sub(show_at)
                    .saturating_sub(entry.paused_ms.get());

                if effective_elapsed >= entry.duration_ms {
                    entry.dismiss_state.set(DismissState::Exiting);
                }
            }

            let current_state = entry.dismiss_state.get();

            if current_state == DismissState::Exiting && entry.exit_start_ms.get().is_none() {
                entry.exit_start_ms.set(Some(now_ms));
            }

            if current_state == DismissState::Exiting {
                if let Some(exit_start) = entry.exit_start_ms.get() {
                    if now_ms.saturating_sub(exit_start) >= exit_duration_ms {
                        entry.dismiss_state.set(DismissState::Dismissed);
                    }
                }
            }
        }
    }

    pub fn stats(&self) -> ToastStats {
        let mut stats = ToastStats {
            total_pushed: self.total_pushed,
            next_id: self.next_id,
            queue_len: self.queue.len(),
            capacity_evictions: self.capacity_evictions,
            flushed_dismissed: self.flushed_dismissed,
            ..ToastStats::default()
        };

        for entry in &self.queue {
            match entry.dismiss_state.get() {
                DismissState::Pending => stats.pending += 1,
                DismissState::Visible => stats.visible += 1,
                DismissState::Exiting => stats.exiting += 1,
                DismissState::Dismissed => stats.dismissed += 1,
            }
        }
        stats.active = stats.pending + stats.visible + stats.exiting;
        stats.render_tracked = stats.active.min(MAX_VISIBLE);
        stats
    }
}
