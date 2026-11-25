# SPEC-HAL-299: **Plot Price Levels, Order Markers & Tags**

### *Persistent Horizontal Markers, Draggable Order Lines, and Custom Tags for Plot Widgets*

**Author:** JC
**Status:** Implemented
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-296 (Plot — Coordinate-Authority Rendering System)
* SPEC-HAL-297 (Plot Tool System — DrawHorizontalLine produces PriceLevels)

---

# 1. Purpose

This SPEC defines the system for rendering persistent horizontal
price lines, order markers, and arbitrary positioned tags inside
the Plot widget. These are the primary mechanism for displaying
orders, alerts, support/resistance levels, and any consumer-defined
annotations that live at specific price coordinates.

Price levels are rendered inside the offscreen texture — they
pan and zoom with the chart, scroll out of view when the price
moves off-screen, and are hit-testable for interaction.

---

# 2. Problem

Trading applications need to display orders, price alerts, and
reference levels directly on the chart. These are not Series data
(they don't have X extents like candles or continuous Y values
like lines). They are horizontal markers at specific Y coordinates
that span the full viewport width, each with an identifying tag
showing price, metadata, and grouping.

Orders additionally need to be draggable — the user grabs a line
and moves it to a new price, which emits a proposed-move event
that the consumer's backend confirms or rejects.

No existing Plot primitive handles this. Series are for data
visualization. Annotations (SPEC-296 §8) are for world-anchored
Halcyon widgets above the texture. Price levels need to be inside
the texture (GPU-rendered, overlay pass) for correct pan/zoom
behavior and efficient rendering of potentially dozens of levels.

---

# 3. Architecture

```
┌─────────────────────────────────────────────────┐
│  Consumer                                        │
│  - Adds/removes/updates PriceLevels              │
│  - Optionally uses OrderMarker helper            │
│  - Polls PriceLevelEvents                        │
│  - Owns backend confirmation logic               │
├─────────────────────────────────────────────────┤
│  Plot                                            │
│  - Stores Vec<PriceLevel>                        │
│  - Renders lines + tags in overlay pass          │
│  - Handles hover/click/drag interaction          │
│  - Emits PriceLevelEvents                        │
├─────────────────────────────────────────────────┤
│  GPU Overlay Pass                                │
│  - Horizontal lines (full viewport width)        │
│  - Tag rectangles (rounded rect background)      │
│  - Tag text (glyph atlas, same as axis labels)   │
│  - Drawn after series, before crosshair          │
└─────────────────────────────────────────────────┘
```

---

# 4. PriceLevel

The core rendering primitive. A horizontal line at a world Y
coordinate with an attached tag containing consumer-defined content.
Non-drag updates approach their target with a frame-rate-independent exponential
rate of `32/3` per second; active dragging remains immediate. A level nominated
as the optional market-detail reference is also immediate: its Plot display
position and linked presentation use the same authoritative `y` value so a live
price reference does not introduce independent easing behind the current candle.

```rust
pub struct PriceLevel {
    /// Unique identifier for update/removal.
    pub id: u64,

    /// World Y coordinate (price).
    pub y: f64,

    /// Line and tag color.
    pub color: [f32; 4],

    /// Line rendering style.
    pub line_style: PriceLevelLine,

    /// Which side of the viewport the tag anchors to.
    pub tag_side: TagSide,

    /// Tag content — consumer-defined text lines.
    /// Each line renders on a new row inside the tag rectangle.
    pub tag: Vec<TagLine>,

    /// Whether this level can be dragged to a new price.
    pub draggable: bool,

    /// Optional group identifier for batch operations and
    /// visual grouping (e.g., "Grid-A", "Manual", "Alerts").
    pub group: Option<String>,
}

/// Line rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLevelLine {
    Solid,
    Dashed,
    Dotted,
}

/// Tag anchor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagSide {
    /// Tag anchored to right edge of viewport (default).
    Right,
    /// Tag anchored to left edge.
    Left,
}

impl Default for TagSide {
    fn default() -> Self {
        Self::Right
    }
}

impl Default for PriceLevel {
    fn default() -> Self {
        Self {
            id: 0,
            y: 0.0,
            color: [0.5, 0.5, 0.5, 0.8],
            line_style: PriceLevelLine::Solid,
            tag_side: TagSide::Right,
            tag: Vec::new(),
            draggable: false,
            group: None,
        }
    }
}
```

## 4.1 TagLine

A single line of text inside a tag. Consumer controls content,
color, and emphasis.

```rust
pub struct TagLine {
    /// Text content for this line.
    pub text: String,

    /// Color override. None = inherit from PriceLevel.color.
    /// Use for highlighting specific values (green for profit,
    /// red for loss, etc.)
    pub color: Option<[f32; 4]>,

    /// Dimmed rendering (reduced opacity). Used for metadata,
    /// secondary information, group labels.
    pub dimmed: bool,
}

impl TagLine {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            dimmed: false,
        }
    }

    pub fn color(mut self, color: [f32; 4]) -> Self {
        self.color = Some(color);
        self
    }

    pub fn dimmed(mut self) -> Self {
        self.dimmed = true;
        self
    }
}
```

## 4.2 Visual Specification

**Line:**
- Full viewport width (left edge to right edge of chart area)
- 1px thick at normal DPI, 2px at 2× DPI
- Color from `PriceLevel.color`
- Dashed/dotted patterns rendered as GPU line segments with gaps

**Tag:**
- Text only — no background rectangle behind price level tags
  (PlotTag freestanding labels retain their background)
- Tag text follows cursor X position horizontally when the
  cursor is anywhere on the chart viewport
- When cursor leaves the chart, tag falls back to the configured
  TagSide anchor (Right or Left edge)
- Text lines stack vertically with 2px line spacing
- Font: same pipeline as axis labels (text engine shape + draw)
- Font size: 10px
- Dimmed lines render at 60% opacity

**Hover state (draggable levels only):**
- Line thickens from 1px to 3px (normal DPI) or 2px to 4px (2× DPI)
- Line alpha increases to 1.0 (full opacity)

**Drag state:**
- Line and tag follow cursor Y position
- Ghost line at original Y position: dashed, 30% opacity

---

# 5. Consumer API

## 5.1 Add / Remove / Update

```rust
impl Plot {
    /// Add a price level. If a level with this ID already exists,
    /// it is replaced.
    pub fn add_price_level(&mut self, level: PriceLevel);

    /// Remove a price level by ID. No-op if not found.
    pub fn remove_price_level(&mut self, id: u64);

    /// Update a price level. Calls the closure with a mutable
    /// reference to the level if found.
    pub fn update_price_level(&mut self, id: u64, f: impl FnOnce(&mut PriceLevel));

    /// Remove all price levels in a group.
    pub fn remove_group(&mut self, group: &str);

    /// Remove all price levels.
    pub fn clear_price_levels(&mut self);

    /// Get pending price level events (drains queue).
    pub fn take_price_level_events(&mut self) -> Vec<PriceLevelEvent>;

    /// Builder: add a price level during construction.
    pub fn with_price_level(self, level: PriceLevel) -> Self;

    /// Builder: set snap-to-tick function.
    pub fn with_price_level_snap(
        self,
        f: impl Fn(f64) -> f64 + Send + Sync + 'static,
    ) -> Self;
}
```

## 5.2 Live Updates via DataUpdate Channel

Price levels can also be managed through the existing DataUpdate
channel for thread-safe updates from background threads:

```rust
pub enum DataUpdate {
    // ... existing variants ...

    /// Add or replace a price level.
    AddPriceLevel(PriceLevel),
    /// Remove a price level by ID.
    RemovePriceLevel(u64),
    /// Update a price level's Y coordinate (e.g., order price changed).
    UpdatePriceLevelY { id: u64, y: f64 },
}
```

---

# 6. Events

```rust
/// Events emitted by price level interaction.
pub enum PriceLevelEvent {
    /// A draggable level was moved to a new price.
    /// Consumer should confirm or reject the move.
    Moved {
        id: u64,
        old_y: f64,
        new_y: f64,
    },

    /// A level was clicked (not dragged).
    Clicked {
        id: u64,
    },

    /// Pointer entered a level's hover zone.
    HoverEnter {
        id: u64,
    },

    /// Pointer left a level's hover zone.
    HoverLeave {
        id: u64,
    },
}
```

## 6.1 Drag Confirmation Protocol

After a `PriceLevelEvent::Moved` event, the level remains at the
new Y position. There is no automatic pending indicator or
snap-back timer. The consumer owns the decision entirely:

- Accept: leave the level at the new Y (it's already there)
- Reject: call `update_price_level(id, |l| l.y = original_price)`

The simplicity is deliberate — Plot does not know whether the
consumer needs backend confirmation, a dialog, or immediate
acceptance.

---

# 7. Drag Interaction

## 7.1 Hit Zone

A draggable price level has a hit zone extending 5px above and
below the line. The tag rectangle is also part of the hit zone.

Hit testing priority: price levels are tested BEFORE series
(they float above data). If a click is on a level's hit zone,
the level handles it. Otherwise, fallthrough to series hit-test
or default pan/zoom.

## 7.2 Drag Mechanics

- **Down** on hit zone: begin drag. Record original Y. Line
  thickens to 3px with full alpha. Suppress pan/zoom.
- **Move** during drag: line and tag follow cursor Y. Ghost at
  original position. Tag text updates to show proposed price.
- **Up**: emit `PriceLevelEvent::Moved { id, old_y, new_y }`.
  Level stays at new position until consumer confirms or rejects.
- **Escape during drag**: cancel. Snap back to original Y. No
  event emitted.

## 7.3 Y-Only Constraint

Drag is constrained to vertical movement only. The cursor can
move horizontally without affecting the level's position. This
matches the natural expectation — you're adjusting a price, not
a time.

## 7.4 Snap to Tick

Optional: the consumer can provide a snap function that quantizes
the proposed Y to valid price increments (tick size):

```rust
plot.set_price_level_snap(|y: f64| -> f64 {
    let tick = 0.01;
    (y / tick).round() * tick
});
```

During drag, the level snaps to the nearest valid tick. The
`PriceLevelEvent::Moved` contains the snapped price.

---

# 8. OrderMarker (Consumer-Side Convenience)

`OrderMarker` is NOT part of Plot. It's a domain-side helper that
maps order semantics to PriceLevel visuals. The consumer can use
it or build PriceLevels directly.

```rust
/// Domain helper: converts order semantics to PriceLevel visual.
/// NOT inside the Plot module — this is consumer/app code.
pub struct OrderMarker {
    pub id: u64,
    pub price: f64,
    pub side: Side,
    pub qty: f64,
    pub filled_qty: f64,
    pub order_type: OrderType,
    pub label: Option<String>,
}

pub enum Side { Buy, Sell }
pub enum OrderType { Limit, Stop, StopLimit, Market }

impl From<OrderMarker> for PriceLevel {
    fn from(order: OrderMarker) -> PriceLevel {
        let color = match order.side {
            Side::Buy => [0.2, 0.7, 0.3, 0.85],   // green
            Side::Sell => [0.8, 0.25, 0.2, 0.85],  // red
        };

        let line_style = match order.order_type {
            OrderType::Limit => PriceLevelLine::Solid,
            OrderType::Stop | OrderType::StopLimit => PriceLevelLine::Dashed,
            OrderType::Market => PriceLevelLine::Dotted,
        };

        let side_str = match order.side {
            Side::Buy => "BUY",
            Side::Sell => "SELL",
        };

        let mut tag = vec![
            TagLine::new(format!("{} {:.2}", side_str, order.price)),
        ];

        // Quantity / fill state
        if order.filled_qty > 0.0 {
            tag.push(TagLine::new(
                format!("{:.4}/{:.4}", order.filled_qty, order.qty),
            ));
        } else {
            tag.push(TagLine::new(format!("{:.4}", order.qty)));
        }

        // Custom label (strategy, grid, etc.)
        if let Some(label) = &order.label {
            tag.push(TagLine::new(label.clone()).dimmed());
        }

        PriceLevel {
            id: order.id,
            y: order.price,
            color,
            line_style,
            tag,
            draggable: true,
            group: order.label.clone(),
            ..Default::default()
        }
    }
}
```

---

# 9. PlotTag (Freestanding Labels)

Arbitrary positioned labels at world coordinates. Same rendering
machinery as price level tags but without the horizontal line.
Positioned at an (x, y) world coordinate with an anchor direction.

```rust
pub struct PlotTag {
    /// Unique identifier.
    pub id: u64,

    /// World X coordinate.
    pub x: f64,

    /// World Y coordinate.
    pub y: f64,

    /// Tag content lines.
    pub tag: Vec<TagLine>,

    /// Background color.
    pub color: [f32; 4],

    /// Where the tag sits relative to the anchor point.
    pub anchor: TagAnchor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagAnchor {
    /// Tag above the point, centered horizontally.
    Above,
    /// Tag below the point.
    Below,
    /// Tag to the right of the point.
    Right,
    /// Tag to the left of the point.
    Left,
}

impl Default for TagAnchor {
    fn default() -> Self {
        Self::Above
    }
}

impl Plot {
    /// Add a freestanding tag at world coordinates.
    pub fn add_tag(&mut self, tag: PlotTag);

    /// Remove a tag by ID.
    pub fn remove_tag(&mut self, id: u64);

    /// Update a tag.
    pub fn update_tag(&mut self, id: u64, f: impl FnOnce(&mut PlotTag));

    /// Remove all tags.
    pub fn clear_tags(&mut self);

    /// Builder: add a tag during construction.
    pub fn with_tag(self, tag: PlotTag) -> Self;
}
```

PlotTags are useful for:
- Trade entry/exit markers ("Entry @ 101.50")
- Event labels ("Earnings", "Fed Decision")
- Strategy annotations ("Signal: RSI Divergence")
- Debug markers during development

---

# 10. Rendering

## 10.1 Render Order (within offscreen texture)

```
1. Grid lines                   (z: 0)
2. Bar series (volume)          (z: 50)
3. Line series (EMAs)           (z: 10-30)
4. Candle series                (z: 100)
5. Price level lines            (z: 200)
6. Price level tags             (z: 201)
7. PlotTags                     (z: 202)
8. Selection highlights         (z: 250)
9. Tool overlays                (z: 260)
10. Crosshair                   (z: 300)
```

Price levels render above all data series but below the crosshair
and selection system.

## 10.2 Line Rendering

Lines use the existing overlay pipeline (same as crosshair and
grid). Solid lines are simple quads. Dashed and dotted lines are
sequences of short quads with gaps computed from the line style.

```rust
fn build_level_line_vertices(
    y_px: f32,
    viewport_w: f32,
    color: [f32; 4],
    style: PriceLevelLine,
    scale_factor: f32,
) -> Vec<Vertex> {
    let thickness = if scale_factor > 1.5 { 2.0 } else { 1.0 };
    match style {
        PriceLevelLine::Solid => {
            // Single full-width quad
            vec![/* one rect from x=0 to x=viewport_w */]
        }
        PriceLevelLine::Dashed => {
            // 8px on, 4px off pattern
            let dash = 8.0;
            let gap = 4.0;
            // Generate quads
        }
        PriceLevelLine::Dotted => {
            // 2px on, 3px off pattern
        }
    }
}
```

## 10.3 Tag Rendering

Tag rendering is split across two pipelines:

**Tag backgrounds** (GPU overlay pass — inside offscreen texture):
- PlotTag backgrounds rendered as colored rectangles in
  `build_level_vertices`, included in `RenderParams.level_vertices`
- Price level tags have NO background (removed for cleanliness)

**Tag text** (main-thread paint — above texture):
- All tag text (both PriceLevel and PlotTag) rendered via
  `paint_level_tags()` using `cx.text_engine.shape()` +
  `cx.text_engine.draw()`
- This is the same text pipeline as axis labels
- Text is positioned relative to the Plot's layout rect
- Text appears above the offscreen texture in the scene graph

## 10.4 Threading Considerations

Price levels and tags are stored on the main-thread Plot struct.
They are included in `RenderParams` as pre-computed vertex data.
The worker thread renders them without knowing the domain semantics.

Add to RenderParams:

```rust
pub struct RenderParams {
    // ... existing fields ...

    /// Pre-computed price level line + PlotTag background vertices.
    /// Computed on main thread in build_level_vertices().
    pub level_vertices: Vec<OverlayVertex>,
}
```

The main thread computes level lines and PlotTag backgrounds in
`build_level_vertices()` during `do_render()` and passes them as
vertex data. Tag text is rendered separately on the main thread
via `paint_level_tags()` — it is NOT passed through RenderParams.

---

# 11. Hit Proxy Integration

Price levels need hit-testing on the main thread (same as Series).
Since levels are stored directly on Plot (not inside the renderer),
no HitProxy mirror is needed — the main thread accesses them
directly.

Hit testing prioritizes draggable levels over non-draggable:

1. Draggable price levels (first pass — interactable)
2. Non-draggable price levels (second pass — click events only)
3. Series (via hit_entries)
4. Default handler (pan/zoom/select)

```rust
fn hit_test_price_levels(
    &self,
    screen_y: f32,
    transform: &Transform,
) -> Option<u64> {
    let threshold = 5.0; // 5px hit zone
    for level in &self.price_levels {
        let level_py = transform.y_to_px(level.y);
        if (screen_y - level_py).abs() <= threshold {
            return Some(level.id);
        }
    }
    None
}
```

---

# 12. Invariants

**INV-299-1:** Price levels are stored on Plot (main thread). They
are NOT stored inside PlotRenderer. The renderer receives
pre-computed vertices via RenderParams.

**INV-299-2:** PriceLevelEvent::Moved is a proposal, not a command.
The consumer owns the decision to accept or reject. Plot never
auto-confirms a drag.

**INV-299-3:** Price level hit-testing takes priority over series
hit-testing. A click on a level line is handled by the level
system, not by CandleSeries.

**INV-299-4:** Drag is Y-only constrained. Horizontal cursor
movement during drag does not affect the level's position.

**INV-299-5:** Escape during drag cancels the drag and restores
the original Y. No event is emitted.

**INV-299-6:** OrderMarker is consumer-side code. Plot has no
knowledge of orders, sides, quantities, or order types. Plot
only knows PriceLevel.

**INV-299-7:** Tag content is entirely consumer-defined. Plot
does not interpret, format, or modify TagLine content. The
consumer controls all text, color, and emphasis.

**INV-299-8:** Price levels render inside the offscreen texture
(overlay pass). They are NOT Halcyon widgets above the texture.
They pan and zoom with the chart content.

**INV-299-9:** PlotTags at world coordinates are hidden when
scrolled out of the viewport. No paint, no hit-test.

**INV-299-10:** Price level vertex data is computed on the main
thread and passed to the worker via RenderParams. The worker
does not access PriceLevel structs.

**INV-299-11:** Extending a PriceLevel into an optional linked market-detail
surface preserves Plot ownership. The nominated live reference bypasses ordinary
price-level easing: Plot renders and publishes the selected level's authoritative
`y`, resolved theme-or-explicit color, and `PriceLevelLine` style as pure data so
the Plot line, current candle, and linked line cannot acquire separate animation
timelines. Series mutation and overlay parameters must share one render
generation. The UI-thread presentation latch publishes the level, the matching
Plot front texture, and its rendered Y-domain together before sibling paint. The
linked widget neither owns nor mutates the level.

---

# 13. Acceptance Criteria

1. PriceLevel renders a horizontal line spanning the viewport
2. Tag renders at the specified side with consumer-defined content
3. Multiple TagLines stack vertically inside the tag
4. Dimmed TagLines render at reduced opacity
5. Color-coded TagLines override the level color
6. Draggable levels show grab cursor on hover
7. Drag moves line + tag, ghost shows original position
8. PriceLevelEvent::Moved fires on drag release
9. Escape cancels drag, restores original position
10. Dashed and dotted line styles render correctly
11. Price levels pan/zoom with the chart
12. Levels outside the visible Y range are not rendered
13. Level hit-testing takes priority over series hit-testing
14. add/remove/update/clear API works correctly
15. Live updates via DataUpdate channel work
16. PlotTag renders at world coordinates with correct anchor
17. PlotTag hidden when scrolled out of viewport
18. OrderMarker converts to correct PriceLevel visual
19. Snap-to-tick quantizes drag position
20. Tag text uses glyph atlas (same quality as axis labels)
21. A nominated linked market-detail PriceLevel uses the same Y coordinate,
    resolved color, and line style in the aligned surface after the matching Plot
    frame is presented

---

# 14. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. PriceLevels owned by Plot. No external references. |
| B — Phase Separation | Preserved. Vertex computation in update/do_render. Rendering in paint. |
| C — ID-based Identity | Preserved. Levels identified by u64 id. No references. |
| D — Pure Data | Preserved. PriceLevel, TagLine, PriceLevelEvent are pure data. |
| E — GPU-First | Preserved. Lines and tags are GPU-rendered vertices in overlay pass. |
| F — Sovereignty | Preserved. Level interaction handled by Plot, not OS. |

No permanent invariants weakened.

---

# 15. Relationship to Other SPECs

- **SPEC-296 §7 (Selection):** Level click does not interfere with
  object selection. They are separate interaction domains.

- **SPEC-296 §8 (Annotations):** World-anchored annotations are
  Halcyon widgets above the texture. Price levels are GPU elements
  inside the texture. Different mechanisms for different needs.

- **SPEC-297 (Tool System):** DrawHorizontalLine tool produces a
  `ToolOutput::Annotation(AnnotationData::HorizontalLine { y })`
  which the consumer converts to a PriceLevel. The tool creates;
  PriceLevel renders.

- **SPEC-296 §22 (Threading):** Level vertices are pre-computed on
  the main thread and included in RenderParams. No threading
  changes needed.

---

# 16. Non-Goals

- Vertical price levels (vertical lines at X coordinates — future)
- Price level persistence to disk (consumer responsibility)
- Level-to-level distance measurement (use MeasureTool)
- Animated level transitions (level snaps to new Y immediately)
- Level grouping visual (bracket, brace) — future
- Right-click context menu on levels — future, uses Halcyon popup

> **Note:** PlotRegion (§17) is designed. Implementation pending.

---

# 17. PlotRegion (Highlighted Zones)

General-purpose rectangular region in world coordinates with
optional full-dimension spanning. Third annotation primitive
alongside PriceLevel (horizontal line) and PlotTag (point label).

```rust
pub struct PlotRegion {
    pub id: u64,
    /// World X bounds. None = span full viewport width.
    pub x_min: Option<f64>,
    pub x_max: Option<f64>,
    /// World Y bounds. None = span full viewport height.
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    /// Fill color (typically semi-transparent).
    pub fill_color: [f32; 4],
    /// Optional border color. None = no border.
    pub border_color: Option<[f32; 4]>,
    /// Optional label (rendered as PlotTag at center).
    pub label: Option<String>,
    /// Optional group for batch removal.
    pub group: Option<String>,
}

impl Default for PlotRegion {
    fn default() -> Self {
        Self {
            id: 0,
            x_min: None,
            x_max: None,
            y_min: None,
            y_max: None,
            fill_color: [0.5, 0.5, 0.5, 0.1],
            border_color: None,
            label: None,
            group: None,
        }
    }
}
```

## 17.1 Use Cases

**Horizontal band** — active traded range, resistance zone:
`y_min: Some(99.5), y_max: Some(101.2), x_min: None, x_max: None`

**Vertical band** — session boundary, event window:
`x_min: Some(t1), x_max: Some(t2), y_min: None, y_max: None`

**Specific zone** — all four bounds set. Alert zone, pattern region.

**Full backdrop** — all None = entire viewport tinted. Useful for
mode indicators ("paper trading", "replay mode").

## 17.2 Consumer API

```rust
impl Plot {
    pub fn add_region(&mut self, region: PlotRegion);
    pub fn remove_region(&mut self, id: u64);
    pub fn update_region(&mut self, id: u64, f: impl FnOnce(&mut PlotRegion));
    pub fn remove_region_group(&mut self, group: &str);
    pub fn clear_regions(&mut self);
    pub fn with_region(self, region: PlotRegion) -> Self;
}
```

## 17.3 DataUpdate

```rust
pub enum DataUpdate {
    // ...
    AddRegion(PlotRegion),
    RemoveRegion(u64),
}
```

## 17.4 Rendering

Regions render as filled quads in the overlay pass at z-order 3
(behind everything except the background). None bounds resolve
to the current viewport edges at render time. Border is a 1px
outline quad around the fill.

## 17.5 Invariants

**INV-299-R1:** Regions with all-None bounds fill the entire
viewport. They are NOT clipped to data bounds.

**INV-299-R2:** Regions render behind all Series, price levels,
tags, crosshairs. Z-order 3 (above grid at 0, below data).

**INV-299-R3:** Region labels render as PlotTags at the region
center. If the center is off-screen, the label is hidden.

## 17.6 Acceptance Criteria

21. PlotRegion renders as a filled rectangle
22. None bounds span full viewport dimension
23. Regions pan/zoom with the chart (world coordinates)
24. Border renders as 1px outline when border_color is set
25. Label renders at region center
26. add/remove/update/group/clear API works
27. DataUpdate::AddRegion/RemoveRegion works
28. Regions render behind all data series
