---
doc-id: halcyon-guide-plot
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-296
  - SPEC-HAL-303
---

# Plot and Market-Detail Composition

**Provenance:** base commit `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`
with four locally modified, uncommitted files present and cited on owner
order (`examples/showcase/tab_plot.rs`, `src/tests/market_detail.rs`,
`src/widgets/aligned_order_book.rs`, `src/widgets/mod.rs`); re-verified
2026-07-24 at this base (dirty-file anchors re-checked on disk:
`src/tests/market_detail.rs` is 1042 lines, the cited widget and
showcase files present as described); final re-pin to a clean revision
when that unit commits. Two-axis tags per SPEC-HAL-309 §7. Governing SPECs are
Drafts: SPEC-HAL-296 (`:6`), amended by 296A (ScatterSeries) and 296B
(connector primitives), both Draft — intent refined by live code below.

## 1. What Plot Is

"Plot is a coordinate-authority rendering surface. It owns a 2D
coordinate system (X × Y), renders data series through its own GPU
pipeline via an offscreen texture, supports pan/zoom/interaction, and can
share axes with other Plots for synchronized views. This module is a peer
to core/, widgets/, renderer/ — not inside widgets/"
[implementation: implemented — `src/plot/mod.rs:1-8`].

Module map (`src/plot/mod.rs:10-25`): axis, axis_authority, color_scheme,
grid, hit_proxy, market_detail, price_level, probe, render_pool,
renderer (private), router, series, spatial, tool, transform, view_state.

## 2. The Plot Widget

`Plot` implements `Widget` (imports at `src/plot/mod.rs:71-77`); struct at
`src/plot/mod.rs:411+` (first 40 of 253 members verified):

- Coordinates: `x_axis`/`y_axis`, `view_state`, `data_bounds`,
  `initial_x_span`, `manual_y` + `y_padding`.
- Shared-axis sync: `shared_x`/`shared_y: Option<Arc<Mutex<AxisAuthority>>>`
  with cached generations — multi-panel X/Y synchronization
  (`:420-423`); `probe: Option<Arc<Mutex<Probe>>>` for cross-panel
  probing (`:426-427`).
- Market-detail seam: `market_detail_link: Option<MarketDetailLink>` +
  reference level (`:428-429`) — §4.
- Interaction state: focus/capture, drag pan, box zoom, space-held pan,
  series drag, object selection + `pending_events: Vec<PlotSelectionEvent>`,
  per-series visibility (`hidden_series`) (`:438-476`).
- GPU: `gpu: Option<GpuResources>`, `render_pool: Option<Arc<PlotRenderPool>>`,
  `renderer: Option<Arc<Mutex<PlotRenderer>>>`, render/submission
  generations + `last_submission` key (`:479-490`) — the offscreen-texture
  path; internal render generations do not dirty surrounding layout
  [authority: binding — SPEC-HAL-354:308].
- Hit proxies: `hit_entries: Vec<hit_proxy::HitTestEntry>` (`:492`).

## 3. Public Surface (all from `src/plot/mod.rs:31-69`)

- **Axes/state:** `Axis`, `AxisKind`, `ScalePolicy`, `AxisAuthority`,
  `ViewState`, `Transform`, `PriceTransform`, `PlotColorScheme`,
  `PlotRenderPool`, `PlotRouter`, `Probe`.
- **Series:** `Series`/`SeriesData`/`SeriesInteraction` +
  Area (`AreaPoint/Series/Style`), Bar, Candle (`CandleSeries`,
  `RenderCandle`), Circle (296A lineage), Depth
  (`DepthBarSeries`, `BookEvent/Level/Side/SideKind`,
  `LevelClassification`), Histogram, Line.
- **Overlay primitives (296B lineage):** `PlotLine`, `PlotBezier`,
  `PlotCircle`, `PlotPath`, `PlotStyledPath`, `PlotRegion`, `PlotTag`,
  `TagAnchor`, `TagLine`, `PriceLevel`, `PriceLevelLine` + their event
  types.
- **Tools:** `PlotTool`, `MeasureResult`, `GesturePhase/Result`,
  `AnnotationData`, `ToolOutput`.
- **Spatial adapter:** `PlotSpatialAdapter`, projection
  context/intent/key/session/snapshot types.
- **Market detail:** §4.

Live-data invalidation contract is binding: Plot channel updates are
paint-only for the final coalesced visible state; idle receivers do not
animate [authority: binding — SPEC-HAL-354:304-305].

## 4. Market-Detail Composition (SPEC-HAL-296 §15.2 / SPEC-HAL-303)

The mechanism that keeps a price chart and an order-book ladder on one
price axis:

- **`MarketDetailLink`** — "Pure-data opt-in link between a price-owning
  Plot and aligned projections": shared `AxisAuthority` (price axis),
  shared `Probe`, presentation state (reference line + generations), and
  bounded navigation intent (`pending_price_pan_viewports`, clamped to 8
  viewports) [implementation: implemented —
  `src/plot/market_detail.rs:1-45`].
- **Projection vocabulary** — `MarketDetailConfig`, `MarketDetailGrouping`,
  `MarketDetailPriceStep`, `MarketDetailProjection`,
  `MarketDetailProjector`, `ProjectedBookLevel`, `ProjectedPriceRange`,
  `VisibleDepthScale`, `LiquiditySummary`
  [implementation: implemented — exports `src/plot/mod.rs:34-38`].
- **`AlignedOrderBook`** (widget) — "Optional price-aligned order-book
  ladder" consuming the link + projector + the standalone order-book's
  config/event/update types over a bounded crossbeam channel
  (`UPDATE_CHANNEL_CAPACITY = 4`, `EVENT_QUEUE_CAPACITY = 128`).
  `AlignedOrderBookRowLayout`: `PriceAligned` (exact Plot-owned Y
  geometry, default) vs `ReadableAnchored` (exact price anchors, rows
  packed to readable height; labels keep the 10-logical-px minimum,
  INV-303-20) [implementation: implemented —
  `src/widgets/aligned_order_book.rs:1-45`].
- **Showcase composition** — `examples/showcase/tab_plot.rs:1-28` wires
  candles + volume bars + EMA line, overlay primitives, `ViewState`,
  `MarketDetailLink`, and `AlignedOrderBook` in one tab (series slots
  vol/candle/ema; live ticks at 16 ms; 200-candle history).

## 5. Executable Evidence Pointers

`src/tests/market_detail.rs` (1042 lines) — the projection/alignment proof
set at this tree state; exact names include:
`plot_and_market_detail_use_the_same_price_transform` (:57),
`dense_projection_is_pixel_bounded_and_retains_exact_hover_source` (:81),
`grouped_price_ranges_keep_stable_identity_across_quantity_churn` (:112),
`globally_anchored_automatic_price_ranges_survive_equal_span_pan` (:158),
`readable_anchored_layout_decouples_clickable_rows_from_price_collision_grouping`
(:220), `semantic_grouping_merges_only_the_minimum_bins_needed_for_pixel_safety`
(:335), `explicit_price_step_is_validated_and_projects_on_its_global_grid`
(:356), `logarithmic_price_ranges_use_stable_axis_coordinate_midpoints`
(:383), among 12+ total. Execution status is recorded in
`audits/implementation-status.md` §3. At the clean base revision the
committed `market_detail.rs` suite runs 28 passed / 2 failed
(deterministic; names and panic sites in that §3 ledger); the failing
pair sits in the area the uncommitted market-detail unit is changing.

## 6. Boundaries

- Plot owns its coordinate system and GPU pipeline; it does not leak
  render generations into layout (§2; SPEC-HAL-354:308).
- Market-detail alignment is opt-in and pure-data (`MarketDetailLink`);
  the ladder never recomputes price geometry — it consumes Plot-owned
  projections (§4).
- Recent view-state behavior (live-edge follow, `git log`: `9aafdac2`,
  `c168707c`) is engine-side history; behavior claims beyond the cited
  members were not traced this unit (`unknown`).
