# SPEC-HAL-312: **EventLog Widget**

### *Structured Entry-Level Log Viewer with Row Selection and Detail Expansion*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-118 (Virtualized ListView Contract)
* SPEC-HAL-164 (TabularSubstrate Root Abstraction)
* SPEC-HAL-188 (DataSource and Mutation Contracts)

---

# 1. Purpose

A structured log viewer where the atom of interaction is the
**entry** — a record with severity, timestamp, message, source,
and expandable detail. Supports row selection, keyboard
navigation, filtering, and structured copy.

Used for: application event logs, audit trails, alert histories,
system event viewers, structured diagnostic output.

**Scope boundary:** EventLog operates on **entries as items**.
For free-form text output where the atom is characters and spans
(terminal output, `tail -f`, debug console), see SPEC-HAL-305
(Console Widget).

---

# 2. Console vs EventLog — Separation of Concerns

These are distinct widgets solving different problems. The
distinction is architectural, not cosmetic.

| Property | Console (SPEC-HAL-305) | EventLog (SPEC-HAL-312) |
|----------|----------------------|----------------------|
| Atom | Text (characters, spans) | Entry (structured record) |
| Selection | Character-level, cross-entry | Row-level (click, shift-click, ctrl-click) |
| Copy | Plain text, span content | Structured (timestamp + level + message) |
| Detail | None — text is flat | Expandable detail panel per entry |
| Metadata | Inline spans only | Columns: severity, timestamp, source, message |
| Actions | None | Per-entry actions (acknowledge, dismiss, link) |
| Navigation | Scroll only | Keyboard row navigation (↑↓, Home, End, PgUp/PgDn) |
| Filtering | View-layer (future) | First-class (severity, source, text search) |
| Backing | ConsoleDataSource (simple trait) | DataSource (TabularSubstrate-grade) |
| Virtualization | VirtualizationEngine (standalone) | TabularSubstrate / HierarchicalTabularSubstrate |

Both may display timestamped text with severity levels. The
difference is whether the user interacts with the text or with
the entries.

---

# 3. Architecture

EventLog is a **specialized ListView** with log-entry semantics.
It reuses the TabularSubstrate infrastructure rather than
building a parallel system.

```
Consumer
  ├── owns EventLogDataSource (impl DataSource)
  └── pushes EventLogEntry records

EventLog (composite widget)
  ├── ListView with fixed columns:
  │     ├── Severity icon/badge (pinned left, narrow)
  │     ├── Timestamp (pinned left, fixed width)
  │     ├── Source (optional, fixed width)
  │     └── Message (elastic, fills remaining)
  ├── Expansion: click row → detail panel below row
  ├── Row selection: single, multi, range (via OrderedSelectionState)
  └── Filter bar (optional, above list)
```

EventLog does NOT reimplement ListView internals. It configures
a ListView with the correct columns, cell painter, and
interaction policy, then adds the log-specific behaviors
(severity rendering, expansion, filtering).

---

# 4. Data Model

## 4.1 EventLogEntry

```rust
pub struct EventLogEntry {
  /// Unique entry identifier (monotonic).
  pub id: EventLogEntryId,
  /// Severity level.
  pub level: EventLogLevel,
  /// Timestamp (epoch millis, consumer-provided).
  pub timestamp: i64,
  /// Source module/component.
  pub source: String,
  /// Primary message (single line, displayed in list).
  pub message: String,
  /// Detail text (multi-line, shown on expand).
  /// None = entry is not expandable.
  pub detail: Option<String>,
  /// Consumer-defined category for filtering.
  pub category: Option<String>,
  /// Whether this entry has been acknowledged/read.
  pub acknowledged: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct EventLogEntryId(pub u64);

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum EventLogLevel {
  Trace,
  Debug,
  Info,
  Warning,
  Error,
  Critical,
}
```

## 4.2 Data Source

EventLog uses the existing `DataSource` trait from SPEC-HAL-188.
The consumer implements `DataSource` with columns mapping to
EventLogEntry fields.

Alternatively, a convenience `EventLogDataSource` wrapper is
provided that adapts `Vec<EventLogEntry>` or
`SlidingWindow<EventLogEntry>` into the `DataSource` trait
with pre-defined column semantics.

---

# 5. Column Layout

EventLog uses a fixed column set. Consumers do not define
columns — they are built-in:

| Column | Width | Scroll Domain | Content |
|--------|-------|---------------|---------|
| Severity | 24px | Pinned left | Icon or colored badge |
| Timestamp | 80-120px | Pinned left | Formatted timestamp |
| Source | 80-150px | Scrollable | Module/component name |
| Message | Elastic | Scrollable | Primary message text |

Column visibility is configurable (source column can be hidden).
Column widths are resizable. Column order is fixed.

---

# 6. Row Interaction

## 6.1 Selection

EventLog uses `OrderedSelectionState` from the existing
selection infrastructure:

- **Click:** Select single row.
- **Ctrl+Click:** Toggle row selection (multi-select).
- **Shift+Click:** Range selection.
- **Ctrl+A:** Select all visible rows.
- **Escape:** Clear selection.

## 6.2 Keyboard Navigation

- **↑/↓:** Move focus between rows.
- **Home/End:** Jump to first/last row.
- **PgUp/PgDn:** Page navigation.
- **Enter:** Expand/collapse detail (if detail exists).
- **Space:** Toggle selection on focused row.

## 6.3 Copy

When the row owner handles a copy command, it passes selected row IDs
to `EventLog::format_rows_for_copy`. The formatter emits
`[timestamp] [LEVEL] [source] message` for each row in display order.
The current formatter is available to callers and installs no shortcut.
When the mounted detail TextBlock holds focus, its selection copy returns
the exact selected text under HAL315 §14.

## 6.4 Expansion

HAL313 governs expansion and mounts the detail panel below the row.
EventLog's default detail is a selectable read-only TextBlock containing
the detail text when present and the message otherwise.

Expansion changes the row's visual height. This is handled by
the variable-height virtualization infrastructure.

Only one row expanded at a time (accordion), or multiple
(configurable via policy).

---

# 7. Filtering

Filtering is first-class, not a future phase:

```rust
pub struct EventLogFilter {
  /// Minimum severity level to show.
  pub min_level: Option<EventLogLevel>,
  /// Source substring match (case-insensitive).
  pub source_filter: Option<String>,
  /// Message text search (case-insensitive substring).
  pub text_search: Option<String>,
  /// Category filter.
  pub category_filter: Option<String>,
  /// Show only unacknowledged entries.
  pub unacknowledged_only: bool,
}
```

Filters produce a `sorted_indices` subset for the
VirtualizationEngine. The underlying data is unchanged.

An optional filter bar widget renders above the list with
level toggles, a search field, and a source dropdown.

---

# 8. Visual Rendering

## 8.1 Severity Badge

Each row has a severity indicator in the leftmost column:

| Level | Visual |
|-------|--------|
| Trace | Muted dot or no indicator |
| Debug | Gray dot |
| Info | Blue dot |
| Warning | Amber triangle icon |
| Error | Red circle icon |
| Critical | Red filled circle, bold row |

## 8.2 Row Styling

- Unacknowledged entries: bold message, subtle left accent bar.
- Acknowledged entries: normal weight, no accent.
- Selected rows: selection highlight background.
- Focused row: focus ring or subtle border.
- Alternating row backgrounds (zebra striping).

## 8.3 Timestamp Formatting

Configurable via `EventLogConfig::timestamp_formatter`.
Default: `HH:MM:SS.mmm` (24-hour with milliseconds).

---

# 9. Policy

```rust
/// Interaction policy governing EventLog behavior.
pub enum EventLogPolicy {
    /// View-only. Entries visible, scrollable. No selection,
    /// no keyboard navigation, no actions.
    DisplayOnly,
    /// Row selection via click/shift-click/ctrl-click.
    /// Structured formatting for caller-owned copy. Keyboard row navigation
    /// (↑↓, Home, End, PgUp/PgDn).
    Selectable,
    /// Full interactive: selection + expand/collapse +
    /// filter bar + context actions + acknowledge.
    Interactive,
}
```

Default: `Selectable`. The policy is the first field on
`EventLogConfig` and is checked in `handle_event` before
allowing any interaction.

| Capability | DisplayOnly | Selectable | Interactive |
|------------|:-----------:|:----------:|:-----------:|
| Scroll | ✓ | ✓ | ✓ |
| Row selection | | ✓ | ✓ |
| Keyboard navigation | | ✓ | ✓ |
| Structured row formatting for copy | | ✓ | ✓ |
| Expand/collapse | | | ✓ |
| Filter bar | | | ✓ |
| Context actions | | | ✓ |
| Acknowledge | | | ✓ |

**INV-312-7:** Policy is checked before any interaction in
`handle_event`. DisplayOnly returns false for all input
except scroll.

**FORBID-312-5:** EventLog must NOT allow selection or
keyboard navigation in DisplayOnly mode.

---

# 10. Configuration

```rust
pub struct EventLogConfig {
    /// Interaction policy. Default: Selectable.
    pub policy: EventLogPolicy,
    /// Row height for unexpanded entries. Default: 24.0.
    pub row_height: f32,
    /// Font size. Default: 13.0.
    pub font_size: f32,
    /// Whether source column is visible. Default: true.
    pub show_source_column: bool,
    /// Whether filter bar is shown. Default: false.
    pub show_filter_bar: bool,
    /// Timestamp formatter. Default: HH:MM:SS.mmm.
    pub timestamp_formatter: Option<Arc<dyn Fn(i64) -> String>>,
    /// Expansion policy.
    pub expansion_policy: ExpansionPolicy,
    /// Maximum entries before oldest eviction.
    pub capacity: usize,
}

pub enum ExpansionPolicy {
    /// Only one row expanded at a time (accordion).
    SingleExpand,
    /// Multiple rows can be expanded simultaneously.
    MultiExpand,
    /// No expansion (detail column hidden).
    NoExpand,
}
```

---

# 11. Events

```rust
pub enum EventLogEvent {
    /// Row selected/deselected.
    SelectionChanged(Vec<EventLogEntryId>),
    /// Row double-clicked or Enter pressed.
    EntryActivated(EventLogEntryId),
    /// Row expanded/collapsed.
    EntryExpanded(EventLogEntryId, bool),
    /// Entry acknowledged via context action.
    EntryAcknowledged(EventLogEntryId),
    /// Filter changed.
    FilterChanged(EventLogFilter),
}
```

---

# 12. Invariants

**INV-312-1:** EventLog is a ListView composition, not a
parallel implementation. It configures ListView with log-entry
columns and a custom cell painter.

**INV-312-2:** EventLog row selection remains row-level via
OrderedSelectionState. The separately mounted detail TextBlock owns
its character selection.

**INV-312-3:** Filtering produces a `sorted_indices` view. The
underlying data source is not modified by filters.

**INV-312-4:** Expansion changes row height. Handled by
variable-height virtualization (RowHeights).

**INV-312-5:** Row copy uses the structured format in §6.3.
The detail TextBlock's copy is independent.

**INV-312-6:** Data source capacity is consumer responsibility.
EventLog does not own unbounded storage.

---

# 13. Forbidden Behaviors

**FORBID-312-1:** EventLog must not implement character-level
selection on behalf of the mounted detail TextBlock.

**FORBID-312-2:** EventLog must NOT reimplement ListView column
mechanics. Use TabularSubstrate.

**FORBID-312-3:** EventLog must NOT parse ANSI escape sequences.
Entries are structured data, not raw terminal output.

**FORBID-312-4:** EventLog must NOT own the data. Consumer
provides a DataSource implementation.

---

# 14. Implementation Phases

**Phase 1:** Basic EventLog over ListView. Fixed columns
(severity, timestamp, source, message). Row selection.
Keyboard navigation. Structured row formatting for copy.

**Phase 2:** Expansion (detail panel). Variable-height rows.
Acknowledge action.

**Phase 3:** Filter bar. Level toggles. Text search.
Source filter.

**Phase 4:** Context menu. Per-entry actions.
Export selected entries.

---

# 15. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. ListView composition. |
| B — Phase Separation | Preserved. DataSource + cell painter. |
| C — ID-based Identity | Preserved. EventLogEntryId. |
| D — Pure Data | Preserved. EventLogEntry is pure data. |
| E — GPU-First | Preserved. Standard ListView rendering. |
| F — Sovereignty | Preserved. No OS interaction. |
