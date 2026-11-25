# SPEC-HAL-305: **Console Widget**

### *Virtualized Variable-Height Log Display with DataSource Backend*

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-002 (Constitution)

---

# 1. Purpose

A scrolling log console for real-time styled text output.
Variable-height entries, virtualized rendering,
consumer-owned data via DataSource trait.

Used for: application logs, system diagnostics, debug consoles,
strategy output, any streaming text output.

**This widget does NOT depend on SPEC-HAL-160** (Text Log
Substrate). SPEC-160 defines aspirational substrate architecture.
If SPEC-160 materializes later, the Console can adopt its storage
layer without changing its public API.

**Scope boundary:** Console is a **text projection surface**.
The atom of interaction is text — characters, spans, lines.
For structured entry-level interaction (row selection, expandable
detail, metadata actions), see SPEC-HAL-312 (EventLog Widget).

---

# 2. Architecture

`Console (container) → ScrollArea → ConsoleBody`. Same
composition pattern as OrderBook (SPEC-HAL-303).

Console reads from a consumer-provided `ConsoleDataSource`.
The widget never owns the data. A `DefaultConsoleSource`
(backed by `SlidingWindow<ConsoleEntry>`) is provided for
simple use cases.

Variable-height virtualization uses the existing
`VirtualizationEngine` (`src/engines/virtualization/`) and
`RowHeights` (`src/model/tabular/row_heights.rs`).

```
Consumer
  ├── owns data source (impl ConsoleDataSource or DefaultConsoleSource)
  └── marks Console dirty when data changes

Console (container widget)
  ├── reads data source generation in layout
  ├── manages scroll state (tail follow, manual scroll)
  ├── owns RowHeights for height tracking
  ├── uses VirtualizationEngine for visible range
  └── ScrollArea
       └── ConsoleBody (paint widget)
             ├── measures entry heights on demand (lazy)
             ├── stores measured heights in RowHeights
             └── paints only realized entries
```

---

# 3. Data Model

## 3.1 ConsoleEntry

A console entry is a **logical record**, not a line. One entry
renders to a variable number of visual lines depending on
content, viewport width, font size, and collapse state.

```rust
#[derive(Clone, Debug)]
pub struct ConsoleEntry {
  /// Styled content — sequence of spans.
  pub spans: Vec<ConsoleSpan>,
  /// Log severity level. Determines default colors.
  pub level: Option<ConsoleLevel>,
  /// Timestamp (consumer-provided, epoch millis).
  pub timestamp: Option<i64>,
  /// Source module/component (optional).
  pub source: Option<String>,
}

#[derive(Clone, Debug)]
pub struct ConsoleSpan {
  /// Text content.
  pub text: String,
  /// Visual style.
  pub style: ConsoleStyle,
}

#[derive(Clone, Debug, Default)]
pub struct ConsoleStyle {
  /// Foreground color. None = level default or theme primary.
  pub fg: Option<Color>,
  /// Background color. None = transparent.
  pub bg: Option<Color>,
  /// Bold text.
  pub bold: bool,
  /// Italic text.
  pub italic: bool,
  /// Dimmed (reduced opacity).
  pub dim: bool,
  /// Underline.
  pub underline: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConsoleLevel {
  Trace,
  Debug,
  Info,
  Warn,
  Error,
}

impl ConsoleLevel {
  /// Short tag string for display.
  pub fn tag(self) -> &'static str {
    match self {
      ConsoleLevel::Trace => "TRACE",
      ConsoleLevel::Debug => "DEBUG",
      ConsoleLevel::Info  => "INFO",
      ConsoleLevel::Warn  => "WARN",
      ConsoleLevel::Error => "ERROR",
    }
  }
}
```

All types derive `Clone + Debug` — required by
`SlidingWindow<T: Debug + Clone>`.

The enum is `ConsoleLevel`, not `LogLevel` — avoids collision
with `tracing::Level`, `log::Level`, and consumer enums.

## 3.2 Entry vs Line

The data model operates on entries. An entry contains content
that may span multiple visual lines. Virtualization operates
on entries, not lines. Each entry has one measured height
(which may be many visual lines tall).

| Entry Type | Typical Visual Height |
|------------|----------------------|
| Simple info message | 1 line |
| Warning with context | 2-3 lines |
| Stack trace | 10-50 lines |
| JSON / XML dump | variable |
| Wrapped long message | 2-4 lines |
| Collapsed multi-line | 1 line |

---

# 4. Data Source

## 4.1 ConsoleDataSource Trait

The consumer owns the data. The widget reads from a trait:

```rust
pub trait ConsoleDataSource {
  /// Total entry count.
  fn entry_count(&self) -> usize;

  /// Entry by index (0 = oldest visible).
  fn entry(&self, index: usize) -> &ConsoleEntry;

  /// Monotonic generation counter.
  /// Incremented on any data change (push, evict, clear, filter).
  /// Widget compares against cached value to detect updates.
  fn generation(&self) -> u64;

  /// Request a full snapshot for Review mode.
  /// Clones the ENTIRE current window into an independent buffer.
  /// The snapshot remains valid regardless of subsequent evictions
  /// or mutations in the live window.
  ///
  /// The widget calls this once when the user scrolls away from
  /// the tail. Only one snapshot is active at a time — requesting
  /// a new snapshot implicitly releases any previous one.
  ///
  /// Default: returns None (Review mode unsupported; user sees
  /// live data with potential eviction-under-feet).
  fn request_snapshot(&mut self) -> Option<SnapshotHandle> {
    None
  }

  /// Release the active snapshot.
  /// Called when the user returns to Live mode. The data source
  /// drops the cloned data.
  fn release_snapshot(&mut self, _handle: SnapshotHandle) {}

  /// Read from a snapshot. Returns the entry at the given index
  /// within the snapshot, or None if out of bounds.
  /// Index 0 = oldest entry in the snapshot.
  fn snapshot_entry(&self, _handle: &SnapshotHandle, _index: usize) -> Option<&ConsoleEntry> {
    None
  }

  /// Entry count within a snapshot.
  fn snapshot_entry_count(&self, _handle: &SnapshotHandle) -> usize {
    0
  }

  /// Number of new entries that arrived in the live window
  /// since the snapshot was taken. Used by the Review mode
  /// indicator ("↓ 342 new").
  ///
  /// Default: 0 (no tracking).
  fn entries_since_snapshot(&self, _handle: &SnapshotHandle) -> usize {
    0
  }
}

/// Opaque handle to a snapshot held by the data source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SnapshotHandle(pub u64);
```

## 4.2 DefaultConsoleSource

For simple use cases, a SlidingWindow-backed default:

```rust
pub struct DefaultConsoleSource {
    entries: SlidingWindow<ConsoleEntry>,
    generation: u64,
    /// Active snapshot (if any). Full clone of the entire window
    /// at the moment the snapshot was requested.
    snapshot: Option<SnapshotData>,
    /// Next snapshot handle ID.
    next_snapshot_id: u64,
}

struct SnapshotData {
    handle: SnapshotHandle,
    /// Full clone of all entries at snapshot time.
    entries: Vec<ConsoleEntry>,
    /// Live generation at snapshot time.
    /// Used to compute entries_since_snapshot.
    generation_at_snapshot: u64,
}

impl DefaultConsoleSource {
    pub fn new(capacity: usize) -> Self;
    pub fn push(&mut self, entry: ConsoleEntry);
    pub fn clear(&mut self);
}

impl ConsoleDataSource for DefaultConsoleSource { ... }
```

When `request_snapshot` is called, `DefaultConsoleSource` clones
the **entire** SlidingWindow contents into `snapshot.entries`.
The rolling window continues independently. New entries arrive,
old entries evict — none of that affects the snapshot.

The full clone is cheap: 10,000 entries × ~200 bytes = ~2MB.
One-time allocation on scroll-up. Dropped on return to live.

Only one snapshot is active at a time. Requesting a new
snapshot implicitly releases the previous one.

`entries_since_snapshot` returns `self.generation - snapshot.generation_at_snapshot`.

For advanced cases (100k+ entries, database backing, filtered
views, merged streams), the consumer implements
`ConsoleDataSource` directly.

## 4.3 Update Flow

```
Consumer pushes entry to their data source
  → source.generation increments
  → consumer marks Console widget dirty (WidgetDirty::Layout)

Next frame:
  → Console.layout() reads source.generation()
  → if changed: sync RowHeights count, trigger VirtualizationEngine
  → ConsoleBody.paint() measures + paints visible entries only
```

The widget paints at frame rate, not at entry rate. Whether 0
or 10,000 entries arrived since last frame, one paint of ~40
visible entries. The per-entry cost is the data source mutation
(consumer side), not the widget.

---

# 5. Consumer API

```rust
// ── Construction ──

// Simple: DefaultConsoleSource with SlidingWindow backing
let source = Rc::new(RefCell::new(DefaultConsoleSource::new(10_000)));
let console = Console::new(Rc::clone(&source));

// With config
let console = Console::with_config(Rc::clone(&source), ConsoleConfig {
    estimated_row_height: 16.0,
    font_size: 12.0,
    ..Default::default()
});

// ── Direct push (same-thread) ──

source.borrow_mut().push(entry);
source.borrow_mut().push(ConsoleEntry {
    spans: vec![ConsoleSpan {
        text: "Hello world".into(),
        style: ConsoleStyle::default(),
    }],
    level: Some(ConsoleLevel::Info),
    timestamp: Some(1713000000000),
    source: Some("main".into()),
});
source.borrow_mut().clear();

// After push, mark widget dirty:
tree.mark_layout_dirty(console_node_id);

// ── Convenience helpers on DefaultConsoleSource ──

source.borrow_mut().push_text("message", Some(ConsoleLevel::Info));
source.borrow_mut().push_ansi(
    "\x1b[31mERROR\x1b[0m: connection failed",
    Some(ConsoleLevel::Error),
); // convenience: parse_ansi + push

// ── Channel-based (for cross-thread) ──

let (source, tx) = DefaultConsoleSource::with_channel(10_000);
let console = Console::new(Rc::new(RefCell::new(source)));

// From any thread:
tx.send(ConsoleUpdate::Entry(entry));
tx.send(ConsoleUpdate::Clear);

// Console.update() drains the channel and marks itself dirty.
```

---

# 6. ANSI Support

ANSI parsing is a **utility function**, not baked into the
widget. The widget accepts `ConsoleEntry` spans. A standalone
parsing function converts raw ANSI strings to spans:

```rust
/// Parse ANSI escape sequences into ConsoleSpan sequences.
/// Stateless per call — no cross-entry SGR state.
pub fn parse_ansi(input: &str) -> Vec<ConsoleSpan> { ... }
```

Supports: SGR codes (colors, bold, italic, dim, underline,
reset). Standard 16-color, 256-color (ESC[38;5;Nm), and
24-bit (ESC[38;2;R;G;Bm).

The consumer calls `parse_ansi` and feeds the result to the
data source, or constructs spans directly:

```rust
// Via ANSI utility:
source.borrow_mut().push(ConsoleEntry {
    spans: parse_ansi("\x1b[31mERROR\x1b[0m: connection failed"),
    level: Some(ConsoleLevel::Error),
    ..Default::default()
});

// Via direct spans:
source.borrow_mut().push(ConsoleEntry {
    spans: vec![
        ConsoleSpan {
            text: "ERROR".into(),
            style: ConsoleStyle {
                fg: Some(Color::from_hex(0xFF0000)),
                bold: true,
                ..Default::default()
            },
        },
        ConsoleSpan {
            text: ": connection failed".into(),
            style: ConsoleStyle::default(),
        },
    ],
    level: Some(ConsoleLevel::Error),
    ..Default::default()
});
```

---

# 7. Configuration

```rust
pub struct ConsoleConfig {
    /// Default row height estimate for unmeasured entries.
    /// Used by VirtualizationEngine for scroll extent
    /// approximation. Default: 16.0.
    pub estimated_row_height: f32,
    /// Font size for console text. Default: 12.0.
    pub font_size: f32,
    /// Left padding in logical pixels. Default: 8.0.
    pub padding_left: f32,
    /// If true, widget signals needs_animation() for
    /// continuous frames. Default: true.
    pub live: bool,
    /// Timestamp formatter. When set, entries with
    /// `timestamp: Some(ms)` render a fixed-width timestamp
    /// column on the left (dimmed, right-aligned).
    /// Column width measured from first timestamped entry.
    /// Default: None (no timestamp column).
    pub timestamp_formatter: Option<Arc<dyn Fn(i64) -> String>>,
    /// Policy governing interaction level.
    /// Default: OutputOnly.
    pub policy: ConsolePolicy,
}
```

---

# 8. Variable-Height Virtualization

## 8.1 Existing Infrastructure

Halcyon already has the complete variable-height virtualization
stack:

- **VirtualizationEngine** (`src/engines/virtualization/mod.rs`):
  O(1) fixed-height, O(log N) variable-height via prefix sums.
  Prefix sums rebuilt O(N) only when height_version changes.

- **RowHeights** (`src/model/tabular/row_heights.rs`):
  Engine-grade height oracle. Supports fixed and variable modes.
  Provides `height_at(index)`, `set_height(index, h)`,
  `push_with_height(h)`. Monotonic `height_version` for
  staleness detection.

Console uses these directly. No reinvention.

## 8.2 Height Measurement Strategy

**Measurement can only happen in layout() or paint().**
`UpdateContext` has no text engine (confirmed: only contains
`animation: Option<AnimationContext>`). The consumer push site
has no text engine. There is no way to measure at push time.

Strategy: **lazy measurement in paint for visible entries only.**

- New entries get estimated height (`config.estimated_row_height`)
  pushed to RowHeights.
- When an entry enters the visible range during paint,
  ConsoleBody measures its actual rendered height by shaping
  the text at the current viewport width.
- The measured height replaces the estimate in RowHeights via
  `set_height(index, measured)`.
- This bumps `height_version`, causing VirtualizationEngine
  to rebuild prefix sums on next frame.
- ~40 entries measured per frame. Microseconds total.

**Unmeasured entries** (not yet scrolled into view) keep their
estimated height. Scrollbar position is approximate for
unmeasured regions. Minor scroll position correction occurs
when scrolling through regions where estimates were wrong.
This is standard virtual scroll behavior.

**Resize invalidation:** when viewport width changes, text
wrapping changes. All cached heights become stale. RowHeights
is cleared and re-estimated. Visible entries re-measured on
next paint.

## 8.3 Visible Range Computation

```
Fixed-height (all entries same height):
  first = floor(scroll_y / row_height)
  last  = ceil((scroll_y + viewport_height) / row_height)
  → O(1)

Variable-height (after lazy measurement):
  prefix_sums = cumulative heights of all entries
  first = binary_search(prefix_sums, scroll_y)
  last  = binary_search(prefix_sums, scroll_y + viewport_height)
  → O(log N), prefix rebuild O(N) only on height_version change
```

Only entries in [first..last] are measured, styled, and painted.

---

# 9. Rendering

## 9.1 ConsoleBody Paint

For each realized (visible) entry:

```
x = padding_left
For each span in entry.spans:
    Resolve style → font_size, color, bold/italic
    Shape + measure text
    If x + span_width > viewport_width: wrap to next visual line
    Draw text at (x, y) with resolved color
    x += span_width
```

Entry height = number of wrapped visual lines × line_height.
If measured height differs from stored height, update RowHeights.

Bold: use the font's bold variant if available, otherwise
lighten the color by 20%.

Dim: multiply alpha by 0.5.

Background spans: push a rect behind the text at the span's
bg color.

## 9.2 Level Coloring

Resolved **at paint time** from current theme — dark mode safe:

| Level | Color Source |
|-------|-------------|
| Trace | `cx.theme.colors.typography.muted` |
| Debug | `cx.theme.colors.typography.secondary` |
| Info  | `cx.theme.colors.typography.primary` |
| Warn  | `Color::from_hex(0xF39C12)` (amber) |
| Error | `Color::from_hex(0xE74C3C)` (red) |

Consumer-set span colors (`style.fg = Some(...)`) override
level defaults.

No `default_color()` method on ConsoleLevel. Paint resolves
from theme.

## 9.3 Timestamp Column

If `ConsoleConfig::timestamp_formatter` is set and an entry has
`timestamp: Some(ms)`, render a fixed-width column on the left,
dimmed (`cx.theme.colors.typography.muted`), right-aligned.
Column width measured from first timestamped entry, cached.

## 9.4 Visual Elements

- Severity indicator: color bar (2-3px) on left edge per entry
- Monospace font for log content alignment
- Zebra striping: alternating entry backgrounds
- Entry separators (subtle, optional)

---

# 10. Entry Wrapping and Overflow

Entries wrap by default. Console is a variable-height viewer.
Each entry occupies as many visual lines as needed based on
content and viewport width.

Wrapping changes entry heights. When viewport width changes,
all height measurements are invalidated and re-estimated.

Horizontal scroll (no-wrap mode) is a future option. In that
mode, entries have fixed height per visual line and excess
content scrolls horizontally.

---

# 11. Scroll Behavior

## 11.1 Tail Follow (Auto-Scroll)

When new entries arrive and the viewport is at the bottom,
the view follows the newest entry. This is the default mode.

Mechanism: Console checks scroll position vs content extent
in layout(). If at bottom and new entries detected (generation
changed), scroll to bottom.

## 11.2 Manual Scroll Detection

When the user scrolls up (wheel, keyboard, scrollbar drag),
auto-scroll pauses. New entries accumulate but the view stays
put.

Detection: in layout(), before auto-scroll, Console reads
`ScrollArea.visual_scroll_y()`. If not within one
estimated_row_height of max scroll extent, set
`manual_scroll = true`.

```rust
// In Console::layout, before auto-scroll:
if let Some(sa_id) = self.scroll_area_id {
    if let Some(node) = _cx.tree.node(sa_id) {
        if let Some(sa) = node.widget.as_any().downcast_ref::<ScrollArea>() {
            let visual_y = sa.visual_scroll_y();
            let state = self.state.borrow();
            let content_h = state.row_heights.total_height();
            let max_scroll = (content_h - h).max(0.0);
            let at_bottom = (visual_y - max_scroll).abs()
                < state.config.estimated_row_height;
            if !at_bottom {
                self.manual_scroll = true;
            }
        }
    }
}
```

`manual_scroll` lives on Console (container), NOT shared state.

## 11.3 Resume

Space or End key: `manual_scroll = false`,
`pending_scroll_to_bottom = true`.

## 11.4 Scroll Stability on Eviction

When oldest entries evict (ring buffer wraps), the visible
region should not jump. The data source's generation increment
triggers RowHeights resync. If entries were removed from the
front, scroll offset adjusts by the evicted height to keep
current view stable.

## 11.5 Dual-Mode Viewing Model

Console operates in one of two viewing modes:

```rust
enum ConsoleViewMode {
    /// Auto-follow tail. DataSource provides live window.
    /// This is the default.
    Live,
    /// Frozen snapshot. User scrolls freely within the full
    /// cloned history. Entered when user scrolls away from tail.
    Review {
        snapshot: SnapshotHandle,
    },
}
```

**Live mode:** Console reads entries from the data source via
`entry(index)`. Auto-scroll follows the tail. The rolling window
pushes and evicts normally. This is the default and most common
state.

**Review mode:** Entered when the user scrolls up (manual_scroll
becomes true). Console requests a full snapshot from the data
source via `request_snapshot()`. The data source clones the
**entire** current window into an independent `Vec`. The rolling
window continues independently — new entries arrive, old entries
evict, none of that affects the snapshot.

The full-window clone eliminates:
- Partial buffer sizing (how much above/below?)
- Snapshot extension logic (user hits edge, can we grow?)
- Edge detection (is adjacent data still in live window?)
- Buffer zone management
- The entire concept of "soft edge vs hard edge"

The user gets everything that existed when they scrolled up.
Stable, complete, no anxiety about content disappearing.

**Memory cost:** 10,000 entries × ~200 bytes average = ~2MB.
At 100,000 entries, ~20MB. One-time allocation, dropped on
return to live. Negligible on any machine running a GPU UI.

**Clone cost:** SlidingWindow is contiguous. Cloning N entries
is a memcpy + String clones. ~2-5ms for 10K entries. One-shot,
user-initiated. Imperceptible.

The Console reads entries from the snapshot via
`snapshot_entry(handle, index)` instead of `entry(index)`.
The user can scroll freely within the entire snapshot — no
edges, no boundaries, no limitations.

**Returning to Live:** The user returns to live via:
- Space or End key
- Clicking the Review mode indicator pill (§11.8)

The snapshot handle is released via `release_snapshot(handle)`.
The data source drops the cloned data. Auto-scroll re-engages.

**INV-305-10:** The snapshot is the data source's concern, not
the widget's. Console asks for entries; the data source decides
whether to serve from the live window or the snapshot.

**INV-305-11:** Snapshot creation does not block or stall the
rolling window. The live window and the snapshot are independent
memory regions. The full clone is a one-shot copy, not a shared
view.

## 11.6 DefaultConsoleSource Snapshot Implementation

`DefaultConsoleSource` implements snapshots by cloning the
entire SlidingWindow:

```
request_snapshot():
  snapshot.entries = entries.iter().cloned().collect::<Vec<_>>()
  snapshot.generation_at_snapshot = self.generation
  return SnapshotHandle(next_id)
```

The snapshot is a plain `Vec<ConsoleEntry>`, indexed 0..N.
No ring buffer arithmetic, no index translation. Entry 0 in
snapshot = oldest entry at snapshot time.

The snapshot survives evictions from the SlidingWindow because
it is an independent `Vec`. When released, the `Vec` is dropped.

Only one snapshot is active at a time in `DefaultConsoleSource`.
If a second is requested, the first is implicitly released.

`entries_since_snapshot` returns the difference between the
current generation and the generation at snapshot time,
providing the count for the Review mode indicator.

The consumer creates a `DefaultConsoleSource`, wraps it in
`Rc<RefCell<...>>`, passes a clone to Console, and pushes
entries directly. After pushing, the consumer marks the
Console widget dirty via the tree.

For advanced cases (database backing, 100K+ entries, external
storage), the consumer implements `ConsoleDataSource` directly
and decides their own snapshot strategy — full clone, partial
cache, or query-on-demand.

## 11.7 Smooth Auto-Scroll

Auto-scroll to the tail must be visually smooth, not choppy.

**Problem:** If `scroll_to_y(target)` is called every frame with
an ever-increasing target (as new entries arrive), the visual
position permanently "catches up" — creating a jerky effect where
the scroll lerp never settles.

**Solution:** Set the scroll target only when new content arrives
(generation changed), not on every frame. ScrollArea's internal
smooth scroll interpolation handles the visual animation. Between
content updates, the scroll position settles at its target. When
the next batch of entries arrives, a new target is set and the
smooth animation begins again.

```rust
// In Console::layout, auto-scroll section:
if self.pending_scroll_to_bottom && !self.manual_scroll {
    self.pending_scroll_to_bottom = false; // consume the flag — one shot
    let target = (content_h - viewport_h).max(0.0);
    sa.scroll_to_y(target);
}
```

`pending_scroll_to_bottom` is set to `true` only when new
content is detected (in update() or layout()). It is consumed
immediately. It is NOT re-set every frame.

## 11.8 Review Mode Indicator

When in Review mode, the Console displays a floating pill
indicator anchored to the bottom edge of the viewport:

```
┌─────────────────────────────────┐
│  ...log entries...              │
│                                 │
│          ↓ 342 new              │
└─────────────────────────────────┘
```

The pill shows:
- A down arrow (↓) indicating newer content exists below
- The count of entries that arrived since the snapshot was
  taken (from `entries_since_snapshot`)

The count updates in real-time — the live window keeps
pushing, and the data source tracks the delta.

**Interaction:** Clicking the pill returns to Live mode
(equivalent to Space/End). The snapshot is released, auto-
scroll re-engages, and the view jumps to the tail.

**Visual style:** Semi-transparent background, rounded
corners, centered horizontally, 8px above bottom edge.
Accent color matching the theme. Subtle pulse animation
when count increases (optional).

**Visibility:** The pill is ONLY visible in Review mode.
In Live mode it is hidden. This is the user's primary cue
that they are viewing frozen data while the live stream
continues.

---

# 12. Policy

```rust
pub enum ConsolePolicy {
    /// Output only. No text selection, no search.
    OutputOnly,
    /// Selectable text. Copy to clipboard via Ctrl+C.
    Selectable,
    /// Full interactive: selectable + search (Ctrl+F).
    Interactive,
}
```

Default: `OutputOnly`. Changes to `Selectable` when Phase 4
ships.

---

# 13. Interaction (Phase-Gated)

## 13.1 OutputOnly Mode

Scroll. Space/End resume tail follow.
Collapse/expand toggle (future).

## 13.2 Selectable Mode — Text Selection Model

Console uses **free-form character-level text selection** that
spans across entry boundaries. This is distinct from row/item
selection (which is the domain of EventLog, SPEC-HAL-312).

### Selection Representation

```rust
struct ConsoleTextSelection {
    /// Anchor: where the drag started.
    anchor: ConsoleTextPosition,
    /// Head: where the drag currently is (or ended).
    head: ConsoleTextPosition,
}

struct ConsoleTextPosition {
    /// Entry index within the data source (or snapshot).
    entry_index: usize,
    /// Byte offset within the entry's concatenated span text.
    byte_offset: usize,
}
```

Selection uses indices (entry index + byte offset), not
references — conforming to SPEC-HAL-002 §3.9 (INV-305-3).

### Interaction

- **Click:** Place caret (selection anchor = head).
- **Click + Drag:** Select text from anchor to current position.
  Selection spans across entry boundaries — dragging from one
  entry into the next selects all text between the two positions.
- **Shift + Click:** Extend selection from anchor to click point.
- **Ctrl+A:** Select all visible text.
- **Ctrl+C:** Copy selected text to clipboard as plain text.
  Spans resolved to their text content. Entry boundaries
  inserted as newlines. Style information (colors, bold) is
  NOT preserved in copy — plain text only.
- **Escape:** Clear selection.

### Selection Rendering

Selected text is rendered with a highlight background
(`cx.theme.colors.accents.selection` or similar). The
highlight spans across entries, painting a continuous selection
rect behind the text.

### Selection Lifetime

- Selection persists in Review mode.
- Selection is **cleared** on return to Live mode (since the
  underlying data may have changed).
- Selection is cleared on content update in Live mode (entries
  may have shifted, invalidating indices).
- Selection is purely visual — it does not affect entries.

## 13.3 Interactive Mode (Future)

Selectable + Ctrl+F search bar. Match highlighting.
Level/source filter. Keyboard entry navigation. Context menu.

---

# 14. Filtering (View Layer)

Filters create a view over the data source — not data deletion:

- Level filter (show only Error+Warn)
- Text search (match against span content)
- Source filter (show only specific module)

The filtered set is a subset of indices passed to
VirtualizationEngine (same pattern as TabularSubstrate's
`sorted_indices`). The data source is unchanged.

---

# 15. Invariants

**INV-305-0:** Bounded memory. Data source capacity enforced.
DefaultConsoleSource uses SlidingWindow which auto-evicts.

**INV-305-1:** ANSI parsing is stateless per entry. No
cross-entry SGR state.

**INV-305-2:** Auto-scroll disabled by manual scroll. Re-enabled
by Space/End.

**INV-305-3:** Text selection uses indices (entry index + char
offset), not references (SPEC-HAL-002 §3.9).

**INV-305-4:** Span styles resolved at paint time. Level-default
colors from theme (dark mode safe).

**INV-305-5:** ConsoleBody paints only realized (visible) entries.
VirtualizationEngine computes visible range. O(visible) per frame.

**INV-305-6:** Entry heights measured lazily in paint. Estimated
heights for unmeasured entries. No eager measurement (no text
engine at push time or in update()).

**INV-305-7:** `manual_scroll` lives on Console (container), not
shared state.

**INV-305-8:** Consumer owns data via ConsoleDataSource. Widget
never owns entry storage. DefaultConsoleSource is a convenience.

**INV-305-9:** Dirty marking is consumer responsibility. Consumer
pushes data, increments generation, marks widget dirty.

**INV-305-10:** The snapshot is the data source's concern, not
the widget's. Console asks for entries; the data source decides
whether to serve from the live window or the snapshot. The widget
does not manage snapshot storage. `request_snapshot()` clones
the full window — no range parameters, no partial views.

**INV-305-11:** Snapshot creation does not block the rolling
window. Live window and snapshot are independent memory regions.
The full clone is a one-shot copy (~2MB for 10K entries), not
a shared view. New entries continue to arrive while a snapshot
is held.

**INV-305-12:** Auto-scroll target is set once per content update
(when generation changes), not every frame. ScrollArea's smooth
interpolation handles the visual animation.

**INV-305-13:** Text selection is character-level, cross-entry.
Selection indices are invalidated on content change in Live mode
(entries may shift). Selection persists in Review mode (snapshot
data is stable).

**INV-305-14:** Review mode indicator is the sole visual cue
that the user is viewing frozen data. It must be visible whenever
a snapshot is active. The entry count reflects real-time delta
between live window and snapshot (via `entries_since_snapshot`).
Clicking the indicator is equivalent to Space/End.

---

# 16. Forbidden Behaviors

**FORBID-305-0:** Console must NOT own unbounded storage.
Data source capacity is consumer's responsibility.

**FORBID-305-1:** Console must NOT parse ANSI internally. ANSI
parsing is a utility function.

**FORBID-305-2:** Console must NOT create offscreen textures or
GPU pipelines. Text engine rendering only.

**FORBID-305-3:** Console must NOT perform search/filter in
OutputOnly or Selectable modes.

**FORBID-305-4:** ConsoleLevel must NOT have a `default_color()`
method. Level colors resolved from theme at paint time.

**FORBID-305-5:** Console must NOT use VecDeque for entry storage.
DefaultConsoleSource uses SlidingWindow.

**FORBID-305-6:** Console must NOT measure entries eagerly at push
time or in update(). No text engine available. Lazy measurement
in paint only.

**FORBID-305-7:** Console must NOT implement row-level selection
or item-level interaction (expand, mark, action buttons). That
is EventLog territory (SPEC-HAL-312).

**FORBID-305-8:** Console must NOT re-set auto-scroll target on
every frame. Target is set once when new content is detected.

---

# 17. Acceptance Criteria

1. Consumer-provided ConsoleDataSource drives rendering
2. DefaultConsoleSource wraps SlidingWindow for simple use
3. Variable-height entries render correctly (wrapping)
4. Virtualized: only visible entries measured and painted
5. VirtualizationEngine computes visible range (O(1) or O(log N))
6. Lazy height measurement in paint, estimated for unmeasured
7. Styled spans with correct fg/bg colors
8. Bold, italic, dim, underline render correctly
9. Level-default colors theme-aware (dark mode correct)
10. Auto-scroll to bottom on new entries (smooth animation)
11. Manual scroll pauses auto-scroll
12. Space/End resumes auto-scroll
13. Scroll stability on eviction
14. Channel-based updates from background thread
15. Timestamp column when formatter present
16. `parse_ansi` correctly handles 16/256/24-bit color codes
17. Text selection works in Selectable mode (cross-entry)
18. Ctrl+C copies selection as plain text
19. Demo proves 10k+ entries with O(visible) paint
20. Review mode: scroll up clones full window into snapshot
21. User scrolls freely within entire snapshot — no edges
22. Return to Live releases snapshot, resumes tail follow
23. Review mode indicator shows entry count since snapshot
24. Clicking indicator returns to Live mode

---

# 18. Implementation Phases

**Phase 1:** OutputOnly. ConsoleDataSource trait +
DefaultConsoleSource. Variable-height virtualization via
VirtualizationEngine + RowHeights. Styled spans (fg color).
Level-default colors (theme-aware). Auto-scroll. Lazy height
measurement. Demo with 10k+ entries proving O(visible) paint.

**Phase 2:** Full ConsoleStyle (bg, bold, italic, dim,
underline). Timestamp column. Manual scroll detection.
Severity indicator bar. Collapse/expand.

**Phase 3:** ANSI utility (`parse_ansi`). Convenience
`push_ansi` on DefaultConsoleSource.

**Phase 4:** Selectable mode. Text selection (character-level,
cross-entry) + Ctrl+C copy. Default policy changes to
Selectable.

**Phase 5:** Dual-mode viewing. Review mode with full-window
snapshot (clone entire SlidingWindow on scroll-up).
DefaultConsoleSource snapshot implementation. Review mode
indicator pill (↓ N new, clickable). Smooth auto-scroll.

**Phase 6 (future):** Interactive mode. Search. Level/source
filter. Keyboard entry navigation. Context menu.
Horizontal scroll for wide lines.

---

# 19. Existing Infrastructure Used

| Component | Location | Used For |
|-----------|----------|----------|
| VirtualizationEngine | `src/engines/virtualization/` | Visible range O(1)/O(log N) |
| RowHeights | `src/model/tabular/row_heights.rs` | Height oracle + version tracking |
| SlidingWindow | `src/core/sliding_window.rs` | DefaultConsoleSource backing |
| ScrollArea | `src/widgets/scroll_area.rs` | Scroll mechanics + clip |
| TextEngine | `src/core/widget/mod.rs` | Text shaping + measurement |
| Theme | `src/theme.rs` | Level-default colors |

---

# 20. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Widget in tree. |
| B — Phase Separation | Preserved. Paint is pure. Dirty marking triggers repaint. |
| C — ID-based Identity | Preserved. WidgetId. |
| D — Pure Data | Preserved. ConsoleEntry is pure data. Consumer owns source. |
| E — GPU-First | Preserved. Text engine renders into scene graph. |
| F — Sovereignty | Preserved. No OS interaction. |
