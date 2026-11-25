---
doc-id: halcyon-guide-data
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-007
  - SPEC-HAL-031
  - SPEC-HAL-188
  - SPEC-HAL-354
---

# Data and Update Patterns

How external data becomes pixels without owning the frame. Revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`; two-axis tags per SPEC-HAL-309
§7. The governing chain is SPEC-HAL-354's authority decision: "Data
producers own data publication. Halcyon exclusively owns when a frame is
admitted and which layout, scene, and renderer work that frame performs"
(INV-354-AUTH-1) [authority: binding — SPEC-HAL-354:54-55]. No async
mutation of UI state, ever [authority: binding — SPEC-HAL-002 §3.3].

## 1. The Consumer Contract

```text
data changed
    -> update model/widget/data source     (inside Halcyon's update authority)
    -> classify effect
    -> mark paint-only, or layout+paint
    -> return control to Halcyon
```

[authority: binding — SPEC-HAL-354:283-296]. Never call the renderer,
never request one frame per data event, never use layout invalidation as a
"show new values" signal.

## 2. Where Consumer Update Code Runs

- **`Screen::update(&mut WidgetTree)`** — every frame, rebuild or not;
  data-driven value updates on retained widgets; no structural mutation
  [implementation: implemented — trait doc `src/core/app.rs:4785-4909`,
  call site `:5680-5681`].
- **`Widget::update(dt, cx)` + `needs_update()`** — widget-side polling.
  An attached receiver returns `needs_update() == true`;
  `needs_animation()` stays reserved for actually-unsettled visual effects
  [authority: binding — SPEC-HAL-354:195-200 |
  implementation: implemented — `src/core/widget/mod.rs:2105-2114`,
  `:2345`]. `needs_update` keeps the engine admitting frames at bounded
  cadence (~120 Hz active target) without spinning the loop
  (`runtime-and-frame-lifecycle.md` §3).
- **Invalidation API** — inside `update`, report the narrowest true class:
  `cx.invalidate_paint()`, `cx.invalidate_paint_subtree()`,
  `cx.invalidate_layout()`; the engine drains them per widget and marks
  the tree [implementation: implemented —
  `src/core/widget/mod.rs:1980-2005`, `src/core/app.rs:1261-1276`].
- **`Screen::flush_pending_actions(cx)`** — once per frame, after routing,
  before scene build: poll widget action sinks here so discrete actions
  apply same-frame. Polling sinks from per-event `handle_input` misses
  same-frame writes [implementation: implemented — trait doc,
  `src/core/app.rs:4785-4909`].

Cross-thread ingress goes through synchronized queues consumed during the
update phase — bounded and coalesced; repeated equivalent invalidations
before one admitted frame coalesce idempotently (INV-354-AUTH-3)
[authority: binding — SPEC-HAL-354:195-198, :59].

## 3. Reactive Signals (SPEC-HAL-031)

Small deterministic layer, no global FRP
[authority: draft-intent — SPEC-HAL-031:5 (`Status: Draft`) |
implementation: implemented — `src/model/signal_store.rs:40-123`]:

- `SignalStore::create(initial) -> SignalHandle<T>`; `get`, `set` (equality-
  gated), `watch(handle, node_id)` subscribes a widget node and returns the
  value.
- `set` records subscribed nodes into `pending_dirty`; the frame loop
  drains them and applies `mark_layout_dirty` before layout
  [implementation: implemented — `src/model/signal_store.rs:111`,
  `src/core/app.rs:10728-10732`].
- Node removal cleans subscriptions (`remove_subscribers`; loop call at
  `src/core/app.rs:10668-10672`).

Signals mark *layout* dirty at the loop today — for pure value repaints
prefer the §2 invalidation API's paint-only path. Cross-window signal
*delivery* (SPEC-HAL-161/162) is a separate mechanism: envelopes arrive in
the Signal Delivery Phase via `Screen::handle_signal`, never during
layout/scene/paint (`FrameCriticalSectionGuard`,
`src/core/app.rs:2000`, `:11109-11111`).

## 4. The Tabular Data Boundary (SPEC-HAL-188/188A)

For substrate consumers the data seam is the `DataSource` trait
[authority: SPEC-HAL-188:3 (`Draft (Binding upon adoption)`, amended by
SPEC-HAL-353) | implementation: implemented —
`src/widgets/tabular_view/data_adapter.rs:460-549`]:

- Identity/order: `row_ids()`, `order_access()`, `order_revision()`,
  `order_window(request) -> ValueState<OrderWindow>` (bounded RowId
  windows), `locate_row(revision, row_id)` — provider-authoritative
  ordering with explicit revisions (the SPEC-HAL-353 amendment shape).
- Sorting: `sort_key_for_column`, `sort_capability`, `applied_sort`,
  `request_sort(request) -> SortRequestOutcome`.
- Mutation: `commit_cell(row, column, value) -> bool`.

Presentation snapshots belong to the presentation layer, not the adapter
(SPEC-HAL-188A) [authority: SPEC-HAL-188A:3 (`Draft (Binding upon
adoption)`)]. Row/cell reuse and dirty discipline for all consumers is
SPEC-HAL-189 [authority: binding — SPEC-HAL-189:3 (`Status: Binding`)].

Reference implementations: the `oracles` module provides canonical
SPEC-HAL-188 oracles as ground truth for tests/examples/simple apps —
production applications implement their own sources
[implementation: implemented — `src/oracles/mod.rs:1-15`]. A live
in-tree provider: `EventLogDataSource`
(`src/widgets/event_log/data_source.rs:43-54` — entries + `row_ids` +
`RowId→index` map + per-column bindings + `pending_events`), whose
`DataSource` impl (`:144`) answers `extent()` with `Extent::Exact`,
`mode()` with `UniverseMode::Live`, and `read_cell` through the row
index; a shared-handle variant `SharedEventLogSource` follows at
`:214`/`:229` [implementation: implemented].

## 5. Worked Owner Examples (from the binding spec)

SPEC-HAL-354 §6 names the live owners [authority: binding —
SPEC-HAL-354:298-308]:

- `StatusGrid::set_readout` — height/flex change ⇒ layout+paint; same
  structure ⇒ paint-only.
- `ListView` — visible cell replacement ⇒ paint-only; topology/extent/
  viewport change ⇒ layout+paint.
- Plot / aligned-order-book channels — final coalesced visible state ⇒
  paint-only; idle receivers and identical snapshots invalidate nothing.
- Standalone `OrderBook` — same-topology values invalidate the shared-state
  row paint subtree; row-count/extent changes ⇒ layout+paint.
- Internal GPU texture/render generations do not dirty surrounding layout.

## 6. Anti-Patterns (all constitutionally or spec-forbidden)

- Mutating UI state from another thread or from callbacks directly
  [authority: binding — SPEC-HAL-002 §3.3, §3.7].
- Using `invalidate_layout` as a value-refresh signal
  [authority: binding — SPEC-HAL-026:8, SPEC-HAL-354:34-36].
- Returning `needs_animation() == true` from an idle receiver
  [authority: binding — SPEC-HAL-354:198-200].
- Polling action sinks in `handle_input` instead of
  `flush_pending_actions` (§2).
- Calling platform/renderer APIs from data code
  [authority: binding — SPEC-HAL-354:295-296].

## 7. Status Notes

SPEC-HAL-007 (state model) is Draft — its view-model discipline is intent;
the enforceable machinery is what §1-§6 cite. The SPEC-HAL-354 status
section records the implemented scope at 0.187.1-beta, the 0.189.3-beta
closure of the retained ListView selection dependency, and its named
remaining gap (draw-plan-granular reuse) [authority: binding —
SPEC-HAL-354:364-380]; see `runtime-and-frame-lifecycle.md` §7.
