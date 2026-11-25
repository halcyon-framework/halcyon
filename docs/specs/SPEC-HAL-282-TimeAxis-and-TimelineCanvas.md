# SPEC-HAL-282: **TimeAxis and TimelineCanvas — Timeline Foundation Primitives**

### *Non-Opinionated Foundation for Gantt Charts, Schedulers, and Time-Series Displays*

**Author:** JC
**Status:** Superseded — never implemented. Superseded by SPEC-HAL-334 (Time Model), SPEC-HAL-335 (TemporalSubstrate Architecture), and SPEC-HAL-336 (TimelineHeader and TimelineCanvas).

> **Supersession notice:** No code was ever built against this SPEC. The temporal foundation is governed by SPEC-HAL-334/335/336, which replace this document's TimeAxis / TimelineCanvas / BarDescriptor design with TimeGeometryEngine, IntervalProjection, LaneLayoutEngine, and the TemporalSubstrate composition. This file is retained for historical reference only — do not implement from it.
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-259 (HierarchicalTabularSubstrate)
* Existing: SplitPane, TreeView, ScrollArea, TabularView

---

# 1. Purpose

Define the non-opinionated foundation primitives for any widget that positions visual elements on a horizontal time axis. This covers Gantt charts, resource schedulers, trading timelines, appointment calendars, and any time-positioned data display.

This SPEC defines models and rendering substrate only. Gantt-specific semantics (dependencies, critical path, resource leveling) are application-layer concerns that consume these primitives.

---

# 2. Architectural Decomposition

The foundation consists of three layers:

```
┌─────────────────────────────────────────────────┐
│  Application Layer                              │
│  (Gantt model, dependency engine, CPM solver)   │
│  Produces: BarDescriptors, ArrowDescriptors     │
└───────────┬─────────────────────────────────────┘
            │ feeds
┌───────────▼─────────────────────────────────────┐
│  TimelineCanvas (widget)                        │
│  Paints bars, arrows, markers at coordinates    │
│  Consumes: TimeAxis + visible_rows + descriptors│
└───────────┬─────────────────────────────────────┘
            │ queries
┌───────────▼─────────────────────────────────────┐
│  TimeAxis (model)                               │
│  Maps DateTime ↔ pixel_x                        │
│  Provides: scale, zoom, headers, working-time   │
└─────────────────────────────────────────────────┘
```

---

# 3. TimeAxis — Temporal Coordinate Model

```rust
pub struct TimeAxis {
    /// Visible time range.
    origin: DateTime,
    /// Pixels per time unit (at current zoom).
    scale: TimeScale,
    /// Viewport width in pixels.
    viewport_width: f32,
    /// Working time configuration.
    working_time: WorkingTimeConfig,
}

// Potentially scaling down to minutes, seconds, milliseconds and even microseconds
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TimeScale {
    Hours,    // 1 hour per N pixels
    Days,     // 1 day per N pixels
    Weeks,    // 1 week per N pixels
    Months,   // 1 month per N pixels
}

pub struct WorkingTimeConfig {
    /// Working days (default: Mon-Fri)
    pub working_days: [bool; 7],
    /// Working hours (default: 00:00-24:00, i.e., full day)
    pub day_start: TimeValue,
    pub day_end: TimeValue,
    /// Holiday dates (non-working)
    pub holidays: Vec<Date>,
}
```

### Core API

**INV-282-TA1: DateTime → Pixel**
```rust
fn date_to_x(&self, date: DateTime) -> f32
```
Pure function. Deterministic. No side effects.

**INV-282-TA2: Pixel → DateTime**
```rust
fn x_to_date(&self, x: f32) -> DateTime
```
Inverse of date_to_x. Used for click hit-testing.

**INV-282-TA3: Working Time Query**
```rust
fn is_working_time(&self, date: DateTime) -> bool
fn working_days_between(&self, start: DateTime, end: DateTime) -> u32
```

**INV-282-TA4: Zoom**
```rust
fn zoom_in(&mut self)   // increase pixels-per-unit
fn zoom_out(&mut self)  // decrease pixels-per-unit
fn set_scale(&mut self, scale: TimeScale, pixels_per_unit: f32)
```

**INV-282-TA5: Scroll**
```rust
fn scroll_to_date(&mut self, date: DateTime)
fn scroll_to_today(&mut self)
fn visible_range(&self) -> (DateTime, DateTime)
```

### Multi-Tier Headers

**INV-282-TA6: Header Tiers**
The TimeAxis produces header tier data for rendering:
```rust
fn header_tiers(&self) -> Vec<HeaderTier>

pub struct HeaderTier {
    pub cells: Vec<HeaderCell>,
}

pub struct HeaderCell {
    pub label: String,
    pub x_start: f32,
    pub x_end: f32,
}
```

At `TimeScale::Days`, tiers are: `[Month row, Day row]`.
At `TimeScale::Weeks`, tiers are: `[Month row, Week row]`.
At `TimeScale::Hours`, tiers are: `[Day row, Hour row]`.

---

# 4. TimelineCanvas — Rendering Widget

A widget that paints time-positioned items in a scrollable horizontal canvas. It does NOT own data — it receives descriptors and a TimeAxis reference.

```rust
pub struct TimelineCanvas {
    time_axis: TimeAxis,
    row_source: RowSource,  // trait — provides visible row positions
    /// Items to paint (set each frame from application).
    bars: Vec<BarDescriptor>,
    arrows: Vec<ArrowDescriptor>,
    markers: Vec<MarkerDescriptor>,
    /// Visual config
    show_today_marker: bool,
    show_non_working_shading: bool,
    show_grid_lines: bool,
    /// Scroll state
    scroll_x: f32,
    scroll_y: f32,  // synchronized with left panel
}
```

### RowSource Trait

```rust
pub trait RowSource {
    /// Number of visible rows.
    fn visible_row_count(&self) -> usize;
    /// Y position of row top edge.
    fn row_y(&self, index: usize) -> f32;
    /// Height of row.
    fn row_height(&self, index: usize) -> f32;
    /// Row identifier (for matching bars to rows).
    fn row_id(&self, index: usize) -> u64;
}
```

The HTS implements this via its `visible_rows()` output. A flat list can implement it trivially.

---

# 5. Paint Descriptors

### BarDescriptor

```rust
pub struct BarDescriptor {
    pub row_id: u64,
    pub start: DateTime,
    pub end: DateTime,
    pub color: Color,
    pub label: Option<String>,
    pub progress: f32,          // 0.0 – 1.0
    pub style: BarStyle,
    pub layer: BarLayer,        // for baseline comparison
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BarStyle {
    Solid,
    Hatched,         // progress vs remaining
    Hollow,          // summary/rollup bar
    Milestone,       // diamond marker, zero duration
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum BarLayer {
    Baseline,        // rendered behind, faded
    #[default]
    Primary,         // normal bar
    Preview,         // ghost bar during what-if drag
}
```

### ArrowDescriptor

```rust
pub struct ArrowDescriptor {
    pub from_row_id: u64,
    pub from_date: DateTime,    // typically end date of source
    pub to_row_id: u64,
    pub to_date: DateTime,      // typically start date of target
    pub kind: DependencyKind,
    pub color: Option<Color>,   // None = theme default
    pub critical: bool,         // render with emphasis if true
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DependencyKind {
    FinishToStart,
    StartToStart,
    FinishToFinish,
    StartToFinish,
}
```

### MarkerDescriptor

```rust
pub struct MarkerDescriptor {
    pub date: DateTime,
    pub color: Color,
    pub label: Option<String>,
    pub style: MarkerStyle,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MarkerStyle {
    VerticalLine,   // today marker, deadline
    Diamond,        // milestone (alternative to BarStyle::Milestone)
    Flag,           // annotation
}
```

---

# 6. Rendering Rules

### Bar Paint

**INV-282-R1: Pipeline Safety**
All bar rendering uses `push_rect` only. No `push_rounded_rect`. Per SPEC-PM-006.

**INV-282-R2: Bar Geometry**
```
x = time_axis.date_to_x(bar.start) - scroll_x
w = time_axis.date_to_x(bar.end) - time_axis.date_to_x(bar.start)
y = row_source.row_y(row_index) + bar_margin
h = row_source.row_height(row_index) - 2 * bar_margin
```

**INV-282-R3: Progress Fill**
For `progress > 0.0`: paint a fill rect from bar left to `x + w * progress` in a darker shade.

**INV-282-R4: Bar Label**
Label text centered vertically in the bar. If text wider than bar, truncate with ellipsis or render to the right of the bar.

**INV-282-R5: Baseline Layer**
`BarLayer::Baseline` bars render at 30% opacity behind `Primary` bars. Same row, same time position — visual gap shows schedule drift.

**INV-282-R6: Preview Layer**
`BarLayer::Preview` bars render at 50% opacity with dashed outline. Used during what-if drag simulation.

### Arrow Paint

**INV-282-R7: Arrow Routing**
Arrows are routed as right-angle polylines:
- Exit from source bar's right edge (FS) or left edge (SS)
- Horizontal segment
- Vertical segment to target row
- Horizontal segment to target bar's left edge (FS) or right edge (FF)

Arrowhead at the endpoint.

**INV-282-R8: Critical Arrows**
`critical: true` arrows render thicker (2px vs 1px) in `accents.error` color.

### Grid & Shading

**INV-282-R9: Grid Lines**
Thin vertical lines at time unit boundaries (day boundaries when zoomed to days, etc.).

**INV-282-R10: Non-Working Shading**
Columns where `time_axis.is_working_time() == false` render with a muted overlay (`surfaces.content` at 50% opacity over the background).

**INV-282-R11: Today Marker**
Vertical line at `time_axis.date_to_x(now)` in `accents.primary` color, 2px width, spanning all visible rows.

---

# 7. Interaction

**INV-282-I1: Horizontal Scroll**
Scroll wheel (horizontal or Shift+vertical) scrolls the timeline. Ctrl+Scroll zooms.

**INV-282-I2: Bar Click**
Click on a bar → selection callback with `(row_id, bar_index)`. Application decides what to do.

**INV-282-I3: Bar Drag (Tier 3)**
Pointer down on bar + drag → move bar (change start/end dates). Application receives `on_bar_drag(row_id, new_start, new_end)` and can accept/reject. Preview bars show during drag.

**INV-282-I4: Bar Resize (Tier 3)**
Pointer down on bar edge + drag → resize (change end date only). Same callback pattern.

**INV-282-I5: Empty Click**
Click on empty canvas space → deselect. Right-click → context menu callback.

---

# 8. Composition with Existing Widgets

### Gantt Chart Composition

```
SplitPane
├── Left: TreeView
│   └── HTS-backed
│   └── Columns: Task, Start, Duration, Assignee
│   └── scroll_y synchronized with TimelineCanvas
│
└── Right: TimelineCanvas
    └── TimeAxis for x-positioning
    └── HTS.visible_rows() as RowSource
    └── Application feeds BarDescriptors from task model
    └── Application feeds ArrowDescriptors from dependency model
```

### Trading Timeline Composition

```
SplitPane (or standalone)
├── Left: Watchlist (TabularView)
└── Right: TimelineCanvas
    └── TimeAxis at minute/hour scale
    └── BarDescriptors = trade executions, order durations
    └── MarkerDescriptors = market events, earnings
```

### Appointment Scheduler Composition

```
Vertical layout
├── TimelineCanvas (horizontal, hours scale)
│   └── Rows = rooms/providers
│   └── BarDescriptors = appointments
│   └── Non-working shading = lunch break, off-hours
```

---

# 9. HTS Bridge

**INV-282-HTS1: Single Source of Truth**
The HTS's `visible_rows()` is the canonical row list. Both the TreeView (left panel) and TimelineCanvas (right panel) consume it. Expand/collapse in the tree immediately affects which bars are visible.

**INV-282-HTS2: Scroll Sync**
`scroll_y` is shared between left and right panels. Scrolling either panel scrolls both. The HTS owns the scroll offset.

**INV-282-HTS3: Row Height Consistency**
Both panels use the same row height from the HTS. Variable row heights (summary rows taller) work automatically.

---

# 10. Acceptance Criteria (Tier 1 Foundation)

- TimeAxis maps DateTime ↔ pixel_x correctly at all scales
- TimelineCanvas renders solid bars at correct positions
- Bar labels render inside or beside bars
- Multi-tier time headers render correctly
- Today marker renders at correct position
- Non-working time shading renders for weekends
- HTS integration: expand/collapse updates both panels
- Scroll synchronization between left and right panels
- Horizontal scroll on timeline works independently
- Zoom in/out changes time scale

---

# 11. Non-Goals (This SPEC)

- Dependency engine / CPM solver (application layer)
- Resource leveling algorithm (application layer)
- Earned value computation (application layer)
- Drag-to-move / drag-to-resize (Tier 3, future)
- What-if simulation (Tier 4, future)
- Print/export pagination (Tier 4, future)
- Undo/redo system (framework-level SPEC, not timeline-specific)
