# SPEC-HAL-303: **Order Book Widget**

### *Tabular Bid/Ask Ladder with Classification, Interaction Events, and Cross-Widget Reflection*

**Author:** JC
**Status:** Draft
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-300 (Depth Bar Series — shared data types)
* SPEC-HAL-299 (Plot Price Levels — cross-widget reflection target)

---

# 1. Purpose

This SPEC defines the Order Book Widget — a tabular bid/ask
ladder for real-time order book display. This is a **widget**,
not a Plot series.

**This widget is projection-only.** It does not own book state,
does not reconcile depth updates, does not track level history,
does not detect walls or icebergs, does not classify levels.
The consumer's order book engine owns all of that and feeds the
widget pre-classified `BookSide` snapshots. Halcyon renders.
The consumer computes. Same principle as indicators.

Every feature described here — classification rendering,
interaction events, classified level diffs, iceberg display,
own-order marking, cross-widget reflection — applies equally
to this widget and to the DepthBarSeries (SPEC-300). Same data
model, same event model, same classification system. Two
projection surfaces for the same consumer-owned book state.

---

# 2. Problem

No existing Halcyon widget serves the order book use case:

- **ListView** is designed for uniform rows with selection/focus
  semantics. The standalone order book has two opposed sections, no
  traditional selection, and sub-frame update rates. The aligned projection
  supports one exact-source presentation selection without importing
  ListView's navigation model.
- **Plot + DepthBarSeries** visualizes depth as horizontal bars
  in chart coordinate space. Useful as a sidebar, but not the
  primary tabular view traders use.

---

# 3. Architecture

## 3.1 Widget, Not ListView

The default Order Book Widget is a standalone widget
(`src/widgets/order_book.rs`). Consumers may instead opt into
`AlignedOrderBook` with a `MarketDetailLink`; this adds shared price projection
without changing standalone behavior. The widgets reuse:

- **ColumnModel** for column width/resize geometry
- **Theme tokens** for text sizing, colors, spacing
- **Text engine** for cell text rendering
- **Scrollbar** for vertical overflow

It does NOT reuse ListView's DataSource/DataAdapter (wrong
abstraction), selection/focus model, or body/row_host virtualization
(fixed-height rows with simple O(visible) painting). Standalone clicking
remains an action. `AlignedOrderBook` may additionally retain one exact
`(side, price)` presentation selection while continuing to emit the click.

## 3.2 Data Model

Shared with DepthBarSeries (SPEC-HAL-300). One data model, two views.

```rust
/// Shared types — extracted to model/order_book.rs
pub use crate::model::order_book::{
    BookLevel, BookSide, BookSideKind, LevelClassification,
};
```

```rust
pub struct BookLevel {
    pub price: f64,
    pub quantity: f64,
    pub cumulative: f64,
    pub order_count: Option<u32>,
    pub classification: LevelClassification,
    pub metadata: Vec<String>,
}

pub enum LevelClassification {
    Normal,
    Wall,
    SuspectedSpoof,
    Persistent,
    Iceberg { inferred_total: f64 },
    OwnOrder,
}

pub struct BookSide {
    pub levels: Vec<BookLevel>,
}

pub enum BookSideKind {
    Bid,
    Ask,
}
```

No duplication. Consumer builds BookSide once, feeds it to
both the OrderBook widget and the DepthBarSeries.

## 3.3 Widget Structure

```
┌─────────────────────────────────┐
│ Order Book        ··· ▼ 0.01   │  Header (title + aggregation)
├─────┬──────────┬────────────────┤
│Price│  Amount  │     Total      │  Column headers
├─────┼──────────┼────────────────┤
│ ASK │ levels   │ (red section)  │  Asks (worst → best, top to bottom)
│ ... │  ...     │  ...           │
│BEST │ ASK      │                │  Best ask at bottom of ask section
├─────┴──────────┴────────────────┤
│        66,422.14 ↑              │  Spread indicator
├─────┬──────────┬────────────────┤
│BEST │ BID      │                │  Best bid at top of bid section
│ ... │  ...     │  ...           │
│ BID │ levels   │ (green section)│  Bids (best → worst, top to bottom)
├─────┴──────────┴────────────────┤
│ B 60.58% ████████ 39.41% S     │  Buy/Sell ratio bar
└─────────────────────────────────┘
```

---

# 4. Classification System

The classification system is identical to SPEC-300. The consumer
assigns `LevelClassification` on each BookLevel. The widget
renders each classification with distinct visual treatment.
The widget does NOT auto-detect walls, icebergs, or spoofs.

## 4.1 ClassificationStyleMap

```rust
/// Visual treatment for each LevelClassification.
/// Configurable per-widget instance.
pub struct OrderBookClassificationStyles {
    pub wall: RowClassificationStyle,
    pub suspected_spoof: RowClassificationStyle,
    pub persistent: RowClassificationStyle,
    pub iceberg: RowClassificationStyle,
    pub own_order: RowClassificationStyle,
}

/// Visual overrides for a classified row.
pub struct RowClassificationStyle {
    /// Row background color override. None = use side default.
    pub row_color: Option<[f32; 4]>,
    /// Text color override. None = use side default.
    pub text_color: Option<[f32; 4]>,
    /// Row border color. None = no border.
    pub border_color: Option<[f32; 4]>,
    /// Whether to reflect this classification on linked charts
    /// as a PriceLevel (SPEC-299).
    pub reflect_on_chart: bool,
}
```

Defaults:

| Classification | Row Color | Text Color | Border | Reflect |
|---------------|-----------|------------|--------|---------|
| Wall | brighter side color | bold white | yellow 1px | true |
| SuspectedSpoof | dimmed side color | muted | orange dashed | true |
| Persistent | subtle side color | normal | muted purple | false |
| Iceberg | side color | normal | cyan | true |
| OwnOrder | cyan fill | white | cyan 1px | true |

## 4.2 Wall Rendering

Wall levels (large resting quantity) render with:
- Brighter row background than normal levels
- Bold/bright text color
- Thin yellow left-edge border as a visual flag
- Quantity bar fill extends further (proportional to quantity)

## 4.3 SuspectedSpoof Rendering

Suspected fake walls render with:
- Dimmed row background (lower opacity)
- Muted text color
- Orange dashed left-edge indicator
- Visually de-emphasized compared to legitimate walls

## 4.4 Persistent Level Rendering

Large levels that have persisted for a long time:
- Subtle row background (slightly brighter than normal)
- Normal text color
- Muted purple left-edge indicator

## 4.5 Iceberg Rendering

Iceberg levels (visible quantity much smaller than inferred total):
- Amount cell: shows visible quantity
- Total cell: shows `visible (inferred)` — e.g., `0.13 (4.20)`
- Thin cyan left-edge indicator
- The inferred total portion uses dimmed text

## 4.6 OwnOrder Rendering

The consumer's own orders at this price level:
- Entire row background: distinct cyan fill
- White text for all columns
- Small filled marker on the left edge (▶ triangle or ● dot)
- Always stands out regardless of quantity

If the consumer's order is at a price that doesn't exist in the
book (limit order between levels), they add it as a synthetic
BookLevel with their quantity and OwnOrder classification.
Multiple own orders at different prices are supported.

---

# 5. Visual Design

## 5.1 Row Backgrounds — Quantity Bars

Each row has a colored background fill whose width is proportional
to the level's quantity relative to the maximum visible quantity.

```
│  66,434.64   0.72316   48.04K │████████████████████░░░░░│
│  66,434.07   0.31304   20.79K │████████░░░░░░░░░░░░░░░░│
│  66,433.60   0.13000    8.63K │███░░░░░░░░░░░░░░░░░░░░░│
```

Fill color:
- Asks: red, base alpha 0.15, scales to 0.40 for max quantity
- Bids: green, base alpha 0.15, scales to 0.40 for max quantity

Maximum quantity computed once per frame across ALL visible
levels on both sides. Bid and ask bar widths are directly
comparable.

Classification overrides apply ON TOP of the quantity bar —
a Wall row with large quantity gets both the wide bar AND the
classification highlight.

## 5.2 Spread Indicator

Full-width structural row between asks and bids:

```
│         66,422.14 ↑            │
```

- Large text (1.5× row font size)
- Last trade price + direction arrow (↑ green / ↓ red)
- Background: distinct from ask/bid rows
- NOT a BookLevel — structural element set via `set_last_price()`

## 5.3 Buy/Sell Ratio Bar

Bottom bar showing total bid vs ask quantity:

```
│ B 60.58% ████████████ 39.41% S │
```

Computed from ALL levels (not just visible). Updated on every
book change.

## 5.4 Hover Highlight

Hovered row: the active theme's `list.row_hover` overlay on the entire row.
Aligned mode applies the overlay only to the bid/ask half containing the exact
source level under the pointer. Empty projected price intervals remain
unhighlighted.

---

# 6. Interaction Events

## 6.1 Event Types

```rust
#[derive(Debug, Clone)]
pub enum OrderBookEvent {
    /// Pointer entered a price level row.
    LevelHoverEnter {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        order_count: Option<u32>,
        classification: LevelClassification,
        metadata: Vec<String>,
    },

    /// Pointer left a price level row.
    LevelHoverLeave,

    /// A price level row was clicked.
    LevelClicked {
        price: f64,
        side: BookSideKind,
        quantity: f64,
        classification: LevelClassification,
    },

    /// The spread indicator was clicked.
    SpreadClicked { price: f64 },

    /// A classified level appeared (new wall, iceberg detected).
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

Consumer polls via `order_book.take_events()`.

## 6.2 Classified Level Diff

On every book update (set_bids / set_asks / channel update),
the widget diffs classified levels against the previous set:

- New classified level not in previous → emit ClassifiedLevelAppeared
- Previous classified level not in current → emit ClassifiedLevelDisappeared
- Price exists in both but classification changed → emit both
  Disappeared (old) and Appeared (new)
- Normal levels are excluded from diffing

This enables the consumer to react to market structure changes
(wall appeared, iceberg detected, spoof disappeared) without
polling and comparing the full book.

## 6.3 Cross-Widget Level Reflection

When a level has `reflect_on_chart: true` in its classification
style, the consumer projects it onto a price chart as a SPEC-299
PriceLevel. The widget emits events; the consumer wires them.

```rust
match event {
    OrderBookEvent::LevelHoverEnter { price, side, quantity, .. } => {
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
    OrderBookEvent::LevelHoverLeave => {
        chart_plot.remove_price_level(BOOK_HOVER_LEVEL_ID);
    }
    _ => {}
}
```

## 6.4 Persistent Classified Level Sync

For classified levels that persist across updates:

```rust
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

Widget does NOT do this automatically. Consumer wires events.

## 6.5 Click Semantics

Standalone click = emit `LevelClicked`; the consumer decides meaning.
Aligned click also records the exact source `(side, price)` as its current
presentation selection and highlights the corresponding projected row.
Application meaning remains consumer-owned through `LevelClicked`.

---

# 7. Scroll and Auto-Center

Default: auto-center around the spread. As the spread moves,
display scrolls to keep it centered vertically.

Manual scroll disables auto-center until:
- Double-click spread indicator, OR
- Consumer calls `order_book.re_center()`

Keyboard when focused:
- **Up/Down**: scroll one row
- **PageUp/PageDown**: scroll one page
- **Home**: jump to best ask
- **End**: jump to best bid
- **Escape**: re-center on spread

---

# 8. Column Model

Three mandatory columns:

| Column | Content | Alignment | Default Width |
|--------|---------|-----------|---------------|
| Price | `BookLevel.price` | Right | 90px |
| Amount | `BookLevel.quantity` | Right | 80px |
| Total | Running sum from best | Right | 80px |

"Total" is a cumulative sum from best price toward the edge.
Display computation, not an indicator. Consumer does NOT
compute it.

Column widths use `ColumnModel` from `model::tabular::column`.
Columns resizable via header drag.

## 8.1 Number Formatting

```rust
pub struct OrderBookConfig {
    pub price_formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
    pub quantity_formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
    pub total_formatter: Option<Arc<dyn Fn(f64) -> String + Send + Sync>>,
}
```

---

# 9. Update Model

## 9.1 Direct Updates

```rust
order_book.set_bids(bid_side);
order_book.set_asks(ask_side);
order_book.set_last_price(66_422.14, PriceDirection::Up);
```

## 9.2 Channel Updates

```rust
pub enum OrderBookUpdate {
    Bids(BookSide),
    Asks(BookSide),
    Snapshot { bids: BookSide, asks: BookSide },
    LastPrice { price: f64, direction: PriceDirection },
}

pub enum PriceDirection { Up, Down, Unchanged }
```

Channel drained in `update()`. Same pattern as Plot's DataUpdate.

---

# 10. Rendering

## 10.1 Paint Order

1. Widget background
2. Row background fills (quantity bars)
3. Classification highlights
4. Hover highlight
5. Column separator lines
6. Cell text (price, amount, total)
7. Iceberg inferred-total text (dimmed, in parentheses)
8. OwnOrder markers (▶ on left edge)
9. Spread indicator
10. Buy/sell ratio bar
11. Header (column titles)
12. Scrollbar

All via `cx.text_engine` and `cx.scene.push_rect`. No custom shaders.

## 10.2 Virtualization

Only visible rows painted. Fixed row height → O(1) range
computation, O(visible) painting.

---

# 11. Data Type Extraction (Prerequisite)

Extract BookLevel, BookSide, BookSideKind, LevelClassification
from `src/plot/series/depth.rs` to `src/model/order_book.rs`.
Both DepthBarSeries and OrderBook import from shared location.

---

# 12. Invariants

**INV-303-0:** This widget is **projection-only**. It does not
own book state, reconcile updates, track history, or compute
analytics. The consumer's book engine owns the data; the widget
renders snapshots. No book logic in Halcyon.

**INV-303-1:** Standalone OrderBook does NOT aggregate levels. AlignedOrderBook
applies presentation-only deterministic grouping according to public
`MarketDetailGrouping` intent. Default `AutoPixel` groups only collisions at
the exact configured pixel floor and performs no nice-number enlargement.
Explicit decimal-place and fixed-price-step policies use global semantic price
bins, merging only the minimum adjacent bins required by that floor. A linked
reference is always an exact boundary. A projected row labels one formatted
grouped price and its already-aggregated quantity without exposing range
notation or source-level count as a quantity multiplier. Internal aggregation
bounds and the exact source snapshot remain available for projection evidence
and interaction.

**INV-303-2:** Widget does NOT interpret clicks. Emits events.

**INV-303-3:** BookLevel/BookSide shared with DepthBarSeries.

**INV-303-4:** "Total" column is a display computation.

**INV-303-5:** Auto-center disabled by manual scroll.

**INV-303-6:** Row background width ∝ `quantity / max_visible_quantity`.

**INV-303-7:** Spread indicator is NOT a BookLevel.

**INV-303-8:** Classification reuses LevelClassification from SPEC-300.

**INV-303-9:** Only visible rows painted.

**INV-303-10:** Channel drained in `update()`.

**INV-303-11:** Classification is consumer-assigned. No auto-detect.

**INV-303-12:** ClassifiedLevel diff events fire on every update.

**INV-303-13:** Cross-widget reflection is consumer-wired.

**INV-303-14:** OwnOrder levels are regular BookLevels.

**INV-303-15:** Aligned mode is explicit and optional. It reads the Plot-owned
price authority and never directly pans, zooms, auto-centers, or writes that
authority. Its optional wheel gesture emits navigation intent for Plot to
apply as sole ViewState owner.

**INV-303-16:** Aligned mode clips levels outside the visible domain and exposes
bounded above/below quantity/count summaries.

**INV-303-17:** Aligned projection work per viewport is bounded by source-index
construction on snapshot replacement plus visible pixel buckets on price-domain
or linked-reference changes. Repeated unchanged authority generations do no
projection work.
When sibling traversal lets `AlignedOrderBook::update` precede Plot's completed
frame publication, paint performs at most one bounded projection from the existing
source index for the newly presented axis generation. This prevents a stale cached
row transform from surviving into the scene; the following update populates the
ordinary cache.

**INV-303-18:** An optionally nominated Plot PriceLevel reference is painted
through the aligned ladder at the shared Plot-owned Y coordinate, with the
Plot-resolved color and `PriceLevelLine` style. AlignedOrderBook cannot publish,
select, smooth, or mutate that reference. Plot publishes the nominated level's
authoritative target price without independent easing so the Plot line and
aligned continuation share one animation timeline. Publication follows the
successful worker render, but the Plot front texture, rendered Y-domain, and
reference become visible only through one UI-thread presentation latch before
sibling paint. The ladder therefore paints neither ahead of nor behind the
visible Plot frame.

**INV-303-19:** Aligned row Y geometry, clipping summaries, hover projection, and
the optional reference line consume the same presented Plot price transform.
Widget-tree sibling update order cannot make cached rows paint from the prior
axis generation while the reference paints from the completed generation. This
does not synchronize or interpolate authoritative book-snapshot prices with
candles, and it does not change horizontal quantity-scale animation.

**INV-303-20:** An aligned projected row's grouped price is the
axis-coordinate midpoint of its internal aggregation interval and maps through
the same `PriceTransform` as Plot prices. `AutoPixel` uses the exact interval
corresponding to `MarketDetailConfig::min_bucket_height`, defaulting to a
label-bearing row of ten logical pixels, without aesthetic rounding.
`DecimalPlaces` and validated
`PriceStep` use global price-grid bins and the smallest integer merge factor
that satisfies the pixel floor. Without a reference, automatic interval
boundaries use global axis multiples. With an opted-in reference, the automatic
grid is reference-anchored and an intersecting semantic bin is split, so
adjacent rows meet the exact reference rather than straddling it. Source
quantity and ordering cannot move the group while its interval and anchor are
unchanged. A new reference generation, automatic interval, or semantic merge
factor is an intentional bounded regrouping boundary.

**INV-303-21:** Aligned hover is local and event-producing. It paints the
active-theme row-hover affordance only for a projected level row under the
pointer; empty intervals do not resolve to a distant source level. It neither
paints a horizontal guide nor automatically writes `MarketDetailLink`'s shared
Probe. Cross-widget hover reflection remains an explicit consumer composition.

**INV-303-22:** Enabled wheel input over `AlignedOrderBook` writes only a
bounded, coalesced price-pan intent to `MarketDetailLink`. Plot consumes that
intent and remains the sole Y-domain writer. Disabling wheel price pan makes
the ladder decline wheel ownership.

**INV-303-23:** Aligned selection identity is the exact source `(side, price)`
resolved by an explicit primary click or supplied by the consumer. Selection
may visually identify its projected group, but it never creates a price,
infers a tick grid, or reattaches to a neighboring level when the selected
source disappears.

**INV-303-24:** Price intervals containing no authoritative source liquidity
remain visually empty. Aligned mode does not stretch neighboring rows or
synthesize fills to remove real market gaps.

---

# 13. Acceptance Criteria

1. Ask levels render above spread, bids below
2. Row backgrounds proportional to quantity
3. Spread indicator shows last price + direction
4. Buy/sell ratio bar at bottom
5. Click emits LevelClicked
6. Hover emits LevelHoverEnter with full metadata
7. Leave emits LevelHoverLeave
8. ClassifiedLevelAppeared fires on new classified level
9. ClassifiedLevelDisappeared fires on removed classified level
10. Wall: bright background + yellow indicator
11. SuspectedSpoof: dimmed + orange indicator
12. Iceberg: visible qty + inferred total in parens
13. OwnOrder: cyan highlight + marker
14. Auto-center on spread
15. Manual scroll disables auto-center
16. Double-click spread re-centers
17. Column widths resizable
18. Consumer formatters work
19. Only visible rows painted
20. Channel updates at exchange speed
21. BookLevel/BookSide shared with DepthBarSeries
22. Consumer can wire hover → chart PriceLevels
23. Consumer can wire classified levels → chart PriceLevels
24. Standalone construction and scrolling remain unchanged
25. Opt-in aligned mode shares exact Plot price coordinates
26. Dense aligned levels are pixel-bucketed while hover/click reports exact source data
27. Liquidity above and below the viewport is summarized explicitly
28. Rapid snapshots plus repeated Plot zoom/pan do not form an invalidation/layout loop
29. Sparse aligned levels that collide in screen space are bucketed into non-overlapping rows
30. Projected quantity labels omit source-level multiplicity, and an opt-in Plot
    reference level continues through the aligned ladder with matching Y, color,
    line style, and same-application-frame current-price timing
31. Projected rows show one formatted grouped price, never an aggregation range;
    with an opted-in reference, adjacent rows meet that exact shared line while
    exact source levels remain available for interaction
32. Ordinary aligned hover paints the active-theme projected-row affordance and
    emits exact events without highlighting empty gaps, drawing a horizontal
    guide, or writing the shared Probe
33. Enabled aligned wheel input pans the Plot-owned Y domain through bounded coalesced intent
34. Primary click persists exact-source aligned selection without price-grid inference
35. Empty source-price intervals remain visible gaps rather than stretched rows
36. Default automatic grouping performs no round-number enlargement beyond the
    exact collision floor
37. Explicit decimal-place and fixed-price-step grouping preserves a stable
    semantic grid and uses only minimum integer merging for pixel safety
38. Default automatic rows remain label-bearing at every Plot zoom instead of
    collapsing permanently into unlabeled one-pixel hairlines

---

# 14. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Widget in tree. |
| B — Phase Separation | Preserved. Channel drained in update(). |
| C — ID-based Identity | Preserved. WidgetId. |
| D — Pure Data | Preserved. BookSide + events are pure data. |
| E — GPU-First | N/A — scene graph rendering. |
| F — Sovereignty | Preserved. No OS interaction. |

---

# 15. Implementation Phases

**Phase 1:** Types + basic rendering. Static BookSide data.
Asks/bids sections, row text, quantity bars, spread indicator.

**Phase 2:** Interaction. Click events, hover highlight, hover
events, scroll, auto-center.

**Phase 3:** Live updates. Channel, continuous repaint, ratio
bar, classified level diff events.

**Phase 4:** Classification rendering. Wall/Iceberg/OwnOrder/
Spoof styling. Cross-widget reflection examples.

---

# 16. Non-Goals

- Book state ownership (consumer's book engine owns the data)
- Depth update reconciliation (consumer reconciles, widget renders)
- Level history tracking (consumer tracks, widget renders current)
- Wall/iceberg/spoof detection (consumer analytics, not Halcyon)
- Semantic aggregation logic (consumer pre-aggregates by tick size)
- Order placement (consumer action system)
- Market trades feed (separate widget — SPEC-HAL-304)
- Options chain (separate widget — future SPEC)
