# SPEC-HAL-300: **Depth Bar Series**

### *Horizontal Depth Bars with Level Classification and Interaction Events*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-296 (Plot — Coordinate-Authority Rendering System)
* SPEC-HAL-299 (Plot Price Levels — cross-widget level reflection)

---

# 1. Purpose

This SPEC defines the Series type, data model, and interaction
surface for rendering a live order book inside a Plot widget.
The order book is not a passive visualization — it is an
interactive analytical surface where individual levels carry
identity, metadata, visual classification, and emit interaction
events that drive behavior in other widgets (charts, position
panels, alert systems).

**This Series is projection-only.** It does not own book state,
reconcile depth updates, track level history, or detect patterns.
The consumer's book engine owns all of that and feeds the Series
pre-classified BookSide snapshots. Halcyon renders. The consumer
computes.

---

# 2. Problem

An order book is two independent lists of price levels (bids and
asks), each updating independently at exchange speed. Traders
need to:

- See the depth at each price level as horizontal bars
- Identify significant levels: walls, potential fake walls,
  large resting orders, iceberg patterns
- See metadata per level: number of distinct orders, total
  quantity, time-at-level, iceberg flags
- Hover a level and see it reflected on the price chart as a
  horizontal line (cross-widget)
- See their own orders positioned within the book
- Receive events when levels are hovered, clicked, or when
  significant levels appear/disappear

None of this maps to existing Series types. The order book
requires its own data model with per-level identity and metadata.

---

# 3. Data Model

## 3.1 Separate Sides

Bids and asks are separate data structures. They update at
different cadences, have different visual treatment, and the
consumer manages them independently.

```rust
/// A single order book price level with identity and metadata.
#[derive(Debug, Clone)]
pub struct BookLevel {
    /// Price (Y coordinate). Unique within a side.
    pub price: f64,

    /// Total quantity at this level.
    pub quantity: f64,

    /// Cumulative quantity from best price to this level.
    /// Consumer-computed. Used for the depth curve overlay.
    pub cumulative: f64,

    /// Number of distinct orders resting at this level.
    /// None if exchange doesn't provide this.
    pub order_count: Option<u32>,

    /// Consumer-assigned classification for visual treatment.
    pub classification: LevelClassification,

    /// Consumer-defined metadata lines. Rendered in tooltip
    /// or side panel on hover. Not rendered on the book itself.
    pub metadata: Vec<String>,
}

/// Visual classification of a price level.
/// Drives outline style, color emphasis, and cross-widget reflection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LevelClassification {
    /// Normal level — no special treatment.
    #[default]
    Normal,
    /// Significant wall — large resting quantity.
    /// Rendered with a bright outline and thicker bar.
    Wall,
    /// Suspected fake/spoof wall — large quantity that appears
    /// and disappears. Rendered with a pulsing or dashed outline.
    SuspectedSpoof,
    /// Large level that has persisted for a long time.
    /// Rendered with a solid outline, muted emphasis.
    Persistent,
    /// Iceberg detected — visible quantity is much smaller than
    /// inferred total. Rendered with a special iceberg indicator
    /// (e.g., visible bar + faded extension showing inferred size).
    Iceberg { inferred_total: f64 },
    /// Consumer's own order at this level.
    /// Rendered with a distinct color/marker so the user can
    /// immediately see where their orders sit in the book.
    OwnOrder,
    /// Custom classification with a string tag.
    /// Consumer defines the visual treatment via a style map.
    Custom(&'static str),
}

/// One side of the order book.
#[derive(Debug, Clone)]
pub struct BookSide {
    /// Levels sorted by price. Bids: descending (best first).
    /// Asks: ascending (best first).
    pub levels: Vec<BookLevel>,
}

impl BookSide {
    pub fn new() -> Self {
        Self { levels: Vec::new() }
    }

    pub fn best_price(&self) -> Option<f64> {
        self.levels.first().map(|l| l.price)
    }

    pub fn total_quantity(&self) -> f64 {
        self.levels.iter().map(|l| l.quantity).sum()
    }
}
```

## 3.2 Why Not One Struct?

Bids and asks:
- Update at different times (partial depth updates per side)
- Sort in opposite directions
- Have different visual treatments (color, direction)
- May have different depth (100 bid levels, 50 ask levels)
- Are queried independently ("show me bid walls")

A combined struct forces the consumer to rebuild both sides on
every partial update. Separate sides allow surgical updates.

---

# 4. DepthBarSeries

```rust
pub struct DepthBarStyle {
    /// Bid bar color (default: green).
    pub bid_color: [f32; 4],
    /// Ask bar color (default: red).
    pub ask_color: [f32; 4],
    /// Outline width for classified levels.
    pub outline_width: f32,
    /// Bar height as fraction of price tick spacing.
    pub bar_height_fraction: f32,
    /// Whether to show the cumulative depth curve.
    pub show_cumulative: bool,
    /// Cumulative curve line width.
    pub cumulative_line_width: f32,
    /// Style overrides per classification.
    pub classification_styles: ClassificationStyleMap,
}

/// Visual treatment for each LevelClassification.
pub struct ClassificationStyleMap {
    pub wall: ClassificationStyle,
    pub suspected_spoof: ClassificationStyle,
    pub persistent: ClassificationStyle,
    pub iceberg: ClassificationStyle,
    pub own_order: ClassificationStyle,
    pub custom: HashMap<&'static str, ClassificationStyle>,
}

pub struct ClassificationStyle {
    /// Outline color. None = no outline.
    pub outline_color: Option<[f32; 4]>,
    /// Bar color override. None = use side default.
    pub bar_color: Option<[f32; 4]>,
    /// Bar opacity multiplier.
    pub opacity: f32,
    /// Whether to reflect this level on linked charts
    /// as a PriceLevel (SPEC-299).
    pub reflect_on_chart: bool,
}

pub struct DepthBarSeries {
    bids: BookSide,
    asks: BookSide,
    style: DepthBarStyle,
    z_order: u32,
    // Interaction state
    hovered_level: Option<HoveredBookLevel>,
    // Previous classified levels (for diff events)
    prev_classified_bids: Vec<(f64, LevelClassification)>,
    prev_classified_asks: Vec<(f64, LevelClassification)>,
    // Pending events
    pending_events: Vec<BookEvent>,
    // GPU state
}

struct HoveredBookLevel {
    price: f64,
    side: BookSideKind,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BookSideKind {
    Bid,
    Ask,
}
```

## 4.1 Rendering

Plot coordinate space for order book:
- **Y axis:** price
- **X axis:** quantity (0 at center, bids extend left, asks right)

Each bar:
- Y = level.price
- Height = tick spacing × bar_height_fraction
- Width = level.quantity (in X units)
- Color = bid_color or ask_color, overridden by classification
- Outline = classification-driven (Wall gets bright outline, etc.)

**Classified level outlines:**
Levels with non-Normal classification get an outline rendered as
a 1-2px border around the bar. The outline color comes from the
ClassificationStyleMap. Dashed outlines for SuspectedSpoof
(same dash pattern as PriceLevelLine::Dashed from SPEC-299).

**Iceberg special rendering:**
The visible quantity renders as a solid bar. The inferred total
renders as a faded extension beyond the solid bar, showing the
hidden liquidity estimate. The extension uses the same bar color
at 25% opacity.

**Own order marker:**
Rendered with a distinct color (cyan by default, configurable)
and a small filled triangle marker on the bar's inner edge (toward
center). Always outlined regardless of quantity.

**Cumulative depth curve:**
Optional overlay line connecting the outer edges of cumulative
bars. Bid curve: connects rightmost edges descending in price.
Ask curve: connects leftmost edges ascending in price.
Rendered as LineSeries-style quads above the bars.

---

# 5. Consumer API

## 5.1 Construction

```rust
let book = DepthBarSeries::new()
.with_bid_color([0.2, 0.7, 0.3, 0.6])
.with_ask_color([0.8, 0.2, 0.2, 0.6])
.with_cumulative(true)
.with_classification_style(
LevelClassification::Wall,
ClassificationStyle {
outline_color: Some([1.0, 1.0, 0.0, 0.9]),
bar_color: None,
opacity: 1.0,
reflect_on_chart: true,
},
)
.with_classification_style(
LevelClassification::OwnOrder,
ClassificationStyle {
outline_color: Some([0.0, 0.8, 1.0, 1.0]),
bar_color: Some([0.0, 0.6, 0.8, 0.8]),
opacity: 1.0,
reflect_on_chart: true,
},
);

let plot = Plot::new()
.with_gpu_resources(gpu)
.add_series(Box::new(book));
```

## 5.2 Live Updates

Bids and asks update independently:

```rust
pub enum DataUpdate {
    // ... existing variants ...

    /// Replace the bid side of the order book.
    BookBids {
        series_index: usize,
        bids: BookSide,
    },
    /// Replace the ask side of the order book.
    BookAsks {
        series_index: usize,
        asks: BookSide,
    },
    /// Replace both sides atomically (full snapshot).
    BookSnapshot {
        series_index: usize,
        bids: BookSide,
        asks: BookSide,
    },
}
```

## 5.3 Convenience Builder

```rust
impl Plot {
    /// Add an order book depth chart with default styling.
    pub fn with_order_book(self) -> Self;

    /// Add an order book with custom styling.
    pub fn with_order_book_styled(self, style: DepthBarStyle) -> Self;
}
```

---

# 6. Interaction Events

```rust
/// Events emitted by order book interaction.
#[derive(Debug, Clone)]
pub enum BookEvent {
    /// Pointer entered a price level's hover zone.
    LevelHoverEnter {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        order_count: Option<u32>,
        classification: LevelClassification,
        metadata: Vec<String>,
    },

    /// Pointer left a price level's hover zone.
    LevelHoverLeave,

    /// A price level was clicked.
    LevelClicked {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        classification: LevelClassification,
    },

    /// A classified level appeared (new wall, iceberg detected, etc.).
    ClassifiedLevelAppeared {
        price: f64,
        side: BookSideKind,
        classification: LevelClassification,
    },

    /// A classified level disappeared.
    ClassifiedLevelDisappeared {
        price: f64,
        side: BookSideKind,
        classification: LevelClassification,
    },
}
```

Consumer polls via `plot.take_book_events() -> Vec<BookEvent>`.

## 6.1 Cross-Widget Level Reflection

When a level has `reflect_on_chart: true` in its classification
style, the consumer projects it onto a price chart as a SPEC-299
PriceLevel:

```rust
// Consumer code — reacts to book hover:
match book_event {
BookEvent::LevelHoverEnter { price, side, quantity, .. } => {
chart_plot.add_price_level(PriceLevel {
id: BOOK_HOVER_LEVEL_ID,
y: price,
color: match side {
BookSideKind::Bid => [0.2, 0.7, 0.3, 0.5],
BookSideKind::Ask => [0.8, 0.2, 0.2, 0.5],
},
line_style: PriceLevelLine::Dotted,
tag: vec![
TagLine::new(format!("{:.2} × {:.4}", price, quantity)),
],
draggable: false,
..Default::default()
});
}
BookEvent::LevelHoverLeave => {
chart_plot.remove_price_level(BOOK_HOVER_LEVEL_ID);
}
_ => {}
}
```

Plot doesn't do this automatically. The consumer wires the
events. This keeps Plot generic and gives the consumer full
control over which levels reflect where and how.

## 6.2 Persistent Classified Levels on Chart

For classified levels (walls, icebergs) that persist across
updates, the consumer maintains PriceLevels on the chart:

```rust
// On each book update, sync chart PriceLevels:
chart_plot.remove_price_level_group("book-walls");
for level in &bids.levels {
if level.classification == LevelClassification::Wall {
chart_plot.add_price_level(PriceLevel {
id: hash(level.price, BookSideKind::Bid),
y: level.price,
color: [1.0, 1.0, 0.0, 0.4],
line_style: PriceLevelLine::Dashed,
tag: vec![
TagLine::new(format!("WALL {:.2}", level.price)),
TagLine::new(format!("{:.0} qty", level.quantity)).dimmed(),
],
group: Some("book-walls".into()),
..Default::default()
});
}
}
```

---

# 7. Hit-Test

```rust
fn hit_test(&self, x: f32, y: f32, transform: &Transform) -> Option<Box<dyn Any>> {
    let world_y = transform.px_to_y(y);
    let world_x = transform.px_to_x(x);

    let side = if world_x < 0.0 { BookSideKind::Bid } else { BookSideKind::Ask };
    let levels = match side {
        BookSideKind::Bid => &self.bids.levels,
        BookSideKind::Ask => &self.asks.levels,
    };

    // Find nearest level within threshold (~5px in price units)
    let y_per_px = (transform.y_max - transform.y_min) / transform.viewport_h as f64;
    let threshold = y_per_px * 5.0;

    levels.iter()
        .min_by(|a, b| (a.price - world_y).abs().partial_cmp(&(b.price - world_y).abs()).unwrap())
        .filter(|l| (l.price - world_y).abs() <= threshold)
        .map(|l| -> Box<dyn Any> { Box::new(BookHit {
            price: l.price,
            side,
            quantity: l.quantity,
            order_count: l.order_count,
            classification: l.classification,
            metadata: l.metadata.clone(),
        })})
}
```

---

# 8. Own Order Display

The consumer marks their own orders by setting
`classification: LevelClassification::OwnOrder` on the relevant
BookLevel. If the consumer's order is at a price that doesn't
exist in the book (limit order between levels), they add it as
a synthetic level with their quantity and OwnOrder classification.

Multiple own orders at different prices are supported — each
gets its own BookLevel with OwnOrder classification.

---

# 9. Classified Level Diff

On every BookBids/BookAsks/BookSnapshot update, DepthBarSeries
diffs the current classified levels against the previous set:

- New classified level not in previous → emit ClassifiedLevelAppeared
- Previous classified level not in current → emit ClassifiedLevelDisappeared
- Price exists in both but classification changed → emit both
  Disappeared (old) and Appeared (new)

Normal levels (unclassified) are excluded from diffing — only
non-Normal classifications participate.

---

# 10. Invariants

**INV-300-1:** Bids and asks are separate BookSide structs. They
update independently via separate DataUpdate variants.

**INV-300-2:** LevelClassification is consumer-assigned. The
Series does not auto-detect walls, icebergs, or spoofs.

**INV-300-3:** Cross-widget level reflection is consumer-wired,
not automatic. The Series emits events; the consumer maps them.

**INV-300-4:** ClassifiedLevel diff events are computed by
comparing previous and current classified sets on each update.

**INV-300-5:** Iceberg rendering shows visible quantity (solid)
and inferred total (faded extension).

**INV-300-6:** OwnOrder levels are regular BookLevels with a
classification. The Series doesn't know about order management.

**INV-300-7:** Hit-test determines side (bid/ask) from cursor X
(left of center = bid, right = ask).

**INV-300-8:** BookSide.levels are sorted by the consumer.
Bids: descending. Asks: ascending. The Series does not sort.

**INV-300-9:** Grouped-price bucketing in an aligned market-detail projection is
presentation-only level-of-detail. `AutoPixel` derives its exact interval from
the active Plot price transform and configured pixel floor and does not enlarge
it for round-number aesthetics. Explicit `DecimalPlaces` and validated
`PriceStep` policies use globally anchored semantic price bins; when necessary,
they merge only the minimum integer number of adjacent bins that satisfies the
pixel floor. A linked reference anchors the automatic grid or splits the one
intersecting semantic bin so it remains an exact boundary. Each row displays
one grouped axis-midpoint price while its aggregation bounds remain internal
projection evidence. Exact source levels inside the group contribute quantity
but are never rewritten, reconciled, interpolated, or replaced.

**INV-300-10:** Visible-depth scaling is horizontal authority only. It must not
change the linked price domain and uses a robust smoothed target rather than a
single instantaneous maximum.

---

# 11. Acceptance Criteria

1. DepthBarSeries renders horizontal bars at each price level
2. Bids extend left from center, asks extend right
3. Bids and asks update independently
4. BookSnapshot updates both sides atomically
5. Wall classification renders with bright outline
6. SuspectedSpoof renders with dashed outline
7. Iceberg renders visible + faded inferred bars
8. OwnOrder renders with distinct color and marker
9. Cumulative depth curve renders as overlay line
10. Hover emits LevelHoverEnter with full metadata
11. Leave emits LevelHoverLeave
12. Click emits LevelClicked
13. ClassifiedLevelAppeared fires on new classified level
14. ClassifiedLevelDisappeared fires on removed classified level
15. Hit-test determines bid/ask from cursor X
16. Auto-scale Y adjusts to visible price range
17. Pan/zoom work correctly
18. Consumer can wire hover to chart PriceLevels (SPEC-299)
19. Dense aligned depth is bounded by visible pixel buckets
20. Exact source price/quantity remains available for hover and click
21. A transient large level does not instantly collapse all other visible bars
22. Sparse near-mid levels in a wide aligned price domain do not render as overlapping rows
23. Aligned grouped rows display one price, remain stable across source quantity
    churn, and terminate at an opted-in shared reference instead of straddling it
24. Automatic grouping is limited to exact pixel-collision protection, while
    explicit decimal/price-step grouping merges only the minimum semantic bins
    required for pixel safety

---

# 12. Non-Goals

- Automatic wall/iceberg/spoof detection (consumer analytics)
- Order placement from the book (consumer action system)
- Semantic aggregation by tick size (consumer pre-processes)
- Historical depth replay (consumer manages snapshots)
- DOM-style tabular display (use ListView, not Plot)
