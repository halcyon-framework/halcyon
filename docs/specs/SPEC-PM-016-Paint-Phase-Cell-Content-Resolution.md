# SPEC-PM-016: Paint-Phase Cell Content Resolution in Virtualized Widgets

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget
**Applies To:** ListView, List, TreeView, Grid, PropertyEditor, EventLog, Console, any widget that realizes per-cell or per-row geometry and renders content within those realized regions

---

## 0. Pathology Name

**Paint-Phase Cell Content Resolution**

Also known as:
- "Paint re-fetches what layout should have cached"
- "Per-frame DataSource thrash"
- "Choppy at small N for no virtualization reason"
- "Cost scales with visible columns, not rows"
- "Disabling the data source makes it faster"
- "Format-per-paint"

---

## 1. Summary of the Failure

This pathology occurs when a virtualized widget's realization phase records per-cell **geometry** (rect, position, identity) but not per-cell **content** (string, value, image, pre-resolved render input), and paint then resolves content per cell per frame.

The per-frame cost takes the form `visible_cells × per_query_cost × paints_per_frame`. Virtualization correctly bounds `visible_cells` to the viewport. The pathology lives in the multiplier: even at small N (10k rows or below), if the content path includes allocation (`format!`, `to_string`), trait dispatch, hashmap lookups keyed by fresh strings, or any non-trivial work, the per-paint cost balloons to 20–30ms in debug and produces visibly choppy scroll. The dataset can be tiny; the symptom appears anyway because the cost is per-paint, not per-row.

Layout has all the inputs needed to resolve content — the visible row set, the column set, the data source, the row identities. The architectural error is choosing to leave content resolution for paint.

This was diagnosed in ListView (May 2026) after the substrate cache fix landed. It pre-existed for the lifetime of the DataSource trait, masked by the demo's small column count and absence of profiling. It must never be rediscovered.

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Scroll is choppy at small N (10k rows or below), unrelated to virtualization correctness.
2. `realized_rows` is viewport-bounded but paint still costs >20ms in debug.
3. Per-paint cost scales with visible columns × visible rows.
4. Profiling shows the data source's `read_cell` (or equivalent) called repeatedly during paint.
5. Allocator pressure (`format!`, `String::new`, `Vec::new`) dominates flame graphs during scroll.
6. A widget with more columns feels measurably worse than a sibling with fewer, despite identical row counts.
7. Replacing the data source with a static stub makes the widget faster.
8. The widget's `realize_with_range` (or equivalent) does not produce per-cell content alongside per-cell geometry.

If any of these occur, this pathology is active.

---

## 3. Root Cause (Architectural)

The failure is caused by **separating realization (geometry) from content resolution (data fetch)**.

A virtualized widget's realization function typically returns `Vec<CellProjection>` where `CellProjection = (RowId, ColumnId, RectF)`. Content is intentionally not included on the projection. Paint, given a `CellProjection`, must call back into the data source to fetch the value to render. Multiplied by `visible_cells × paints_per_frame`, this is the dominant per-frame cost as soon as the data source's per-call cost is non-trivial.

This is an architectural omission, not an algorithmic mistake. The realization function has access to everything it needs to resolve content and chose not to.

The correct shape is the one TreeView has by accident: `VisibleRow` carries `.label` resolved at realization time. Paint reads the pre-resolved label. There is no per-paint data-fetch loop. ListView did not have this shape and paid for it.

---

## 4. Why It Happens (and Is Missed)

Several reinforcing factors:

1. **"Realization" sounds geometric.** The word evokes layout, rects, positions. Content feels like a paint-time concern. The seam between layout-and-content is invisible until the per-paint cost is measured.

2. **Lazy-evaluation intuition.** "Don't resolve until you need to paint" reads as sensible defensive programming. It is correct exactly once per frame, and wrong on every frame after that, because paint re-runs.

3. **Trait separation hides the seam.** When `DataSource` and the substrate's realization live in different layers, neither layer's author sees the meeting point clearly. The natural fix (layout calls DataSource, caches into the realization) requires plumbing both into the same scope, which neither side proposes unilaterally.

4. **Sibling widgets accidentally diverge.** TreeView's data is index-resolved at set-tree-data time and stored on `VisibleRow.label`. ListView's data comes through a more abstract `DataSource` trait and ended up resolved per paint. The asymmetry is invisible to readers of either widget alone.

5. **Small N masks it.** At 30 visible rows × 1 column, the cost is invisible. At 30 × 5, it is felt. At 30 × 10, it freezes. Demos with few columns hide the problem indefinitely.

6. **The fix appears to be "optimize read_cell".** First-instinct optimization targets the data source's per-call cost (e.g., O(N) → O(1) lookup). That helps but does not eliminate the architectural error: paint is still calling the data source per cell per frame, just with a faster source. The full fix moves resolution to layout, not just speeds up the call.

---

## 5. Hard Prevention Rules (Binding)

### PM-016-R1: Realize Geometry AND Content

A virtualized widget's realization phase MUST produce both per-cell geometry and per-cell content for every visible cell. If realization produces only geometry, the widget is incomplete.

Content is whatever paint needs in order to render the cell — typically the cell's resolved value (`ValueState<CellValue>`, a `String`, a pre-shaped `GlyphRun`, an image handle, etc.). Paint MUST consume content from realization, not re-resolve it.

---

### PM-016-R2: Single Resolution Per Frame

Per-visible-cell content MUST be resolved at most once per frame.

If layout resolves content, paint MUST read the cached result, not re-resolve. If paint resolves content (because no realization-time cache exists), the widget violates R1 and MUST be brought into conformance.

---

### PM-016-R3: Cache Keyed By Stable Identity

Cached content MUST be keyed by stable cell identity (`(RowId, ColumnId)` or equivalent), never by position, visible index, or anything that virtualization can shift. Identity-keyed caches survive virtualization correctly; position-keyed caches go stale silently (SPEC-PM-010 territory).

---

### PM-016-R4: Cache Cleared On Realization

Cached content for visible cells MUST be cleared at the start of every realization phase, then repopulated for the new visible set. This is the same boundary at which `realized_rows` and `realized_cells` are cleared and rebuilt.

A cache that survives realization boundaries without explicit invalidation re-introduces SPEC-PM-010 (Steady-State Invalidation Blindness) by another door.

---

### PM-016-R5: Data Source Implicit Performance Contract

Any `DataSource`, `CellReader`, callback, or equivalent that the framework invokes during realization MUST be O(1) or near-O(1) per call. If a particular implementation cannot meet this, it MUST cache internally.

The framework calls the trait once per realized cell per layout. Layout fires once per frame during scroll animation. An O(N) read freezes the UI.

This contract is implicit in the SPEC-HAL-188 §13 design ("re-queries data lazily per frame"). Implementations that fail it produce the choppy-at-small-N symptom even after PM-016-R1 conformance, because layout itself becomes the slow phase. PM-016-R5 closes that escape hatch.

---

### PM-016-R6: Paint-Time Resolution Is Permitted Only For Paint-Time Inputs

Content that depends on paint-time-only inputs (current hover state, animation phase, transient highlight, focus-during-paint flag) MAY be resolved at paint. Content that depends only on layout-time inputs (the data source, row identity, column identity, theme, font size) MUST be resolved at layout.

The line is mechanical: if the content can be computed without information that only paint has, layout has no excuse to defer it.

---

## 6. Affected Systems

- **ListView** (multi-column, DataSource-backed) — corrected May 2026 via SPEC-HAL-167 substrate cell-values cache. Conformant.
- **TreeView** — conformant by design (`VisibleRow.label` populated at layout from HTS topology). Cited as the reference shape.
- **List** (single-column, inline `format!`) — low-cost violation. Inline `format!("Item {row_index}")` at paint is the same architectural shape as the ListView pre-fix. Cost is negligible (~34 cells × ~1µs ≈ 35µs/paint), not on the critical path. Conformance: lift the `format!` into a `RowProjection`-time label field populated at realization. To be done opportunistically.
- **Grid, PropertyEditor, EventLog, Console** — must conform when any of them realizes per-cell content via a non-trivial source.
- **Any future widget** that realizes per-cell geometry and renders content within those regions.

---

## 7. Forbidden Patterns (Explicit)

The following are SPEC-PM-016 violations:

**Paint queries the data source:**
- `paint_cell { let value = adapter.read_cell(row, col); ... }` — fetching content per cell at paint, when the data source is non-trivial.

**Realization is geometry-only:**
- `realize_with_range { for cell in visible { realized_cells.push((row, col, rect)); } }` — without an accompanying content-resolution pass.

**Inline content generation at paint:**
- `paint_cell { let text = format!("Item {}", idx); shape(&text); ... }` — even when the `format!` is deterministic from layout-time inputs, doing it at paint moves the cost to the wrong phase.

**Lazy-cache without explicit realization invalidation:**
- "Content is resolved on first paint and cached after" — without a layout-phase clear, the cache holds entries for cells that are no longer realized, and paint-phase population doesn't repopulate evicted entries.

**Position-keyed content cache:**
- `Vec<ValueState>` indexed by visible-row index — virtualization shifts indices; cache survives stale.

**"Optimize read_cell" as a substitute for the architectural fix:**
- Replacing an O(N) data-source read with O(1) without also moving the read out of paint. The faster call is a partial fix; the architectural error is paint calling the source at all.

Any appearance of these patterns is a SPEC-PM-016 violation.

---

## 8. Diagnostic Invariants

**INV-PM-016-CONTENT-AT-LAYOUT.** For any visible cell at paint time, the cached realized content equals what the data source would return at the time of the most recent realization. Paint NEVER calls the data source for content.

**INV-PM-016-COUNT-INDEPENDENCE.** For a virtualized widget with fixed column count, per-paint cost is independent of total row count. Confirmed by identical paint timings across orders of magnitude of N (e.g., 10k, 100k, 1M).

**INV-PM-016-CACHE-EPOCH.** After every realization, the content cache contains entries for exactly the cells in the current realized set. No stale entries from prior realizations remain.

**INV-PM-016-FETCH-COUNT.** The data source's read function is invoked exactly `realized_cells.len()` times per layout, and zero times per paint within that layout's frame.

If any of these fail, the pathology is active.

---

## 9. Relationship to Existing SPECs

- **SPEC-HAL-167** (Layout owns realization) — this PM extends the realization principle to content, not just geometry.
- **SPEC-HAL-188** (Data Source contracts) — §6 implicit performance contract on `read_cell`; §13 "re-queries data lazily per frame". This PM names the consequence of misinterpreting "lazily per frame" as "per paint per cell".
- **SPEC-PM-009** (Dual-Source Geometry Drift) — sibling PM about geometry. PM-016 is about content. Both flow from the same principle: single source of truth, resolved once per frame, consumed by all subsequent phases.
- **SPEC-PM-010** (Incremental Cache Fragility) — content caches keyed by stable identity (PM-016-R3) and cleared at realization (PM-016-R4) are immune to PM-010 by construction.

---

## 10. Enforcement Rule

If a future virtualized widget exhibits:
- Per-paint cost scaling with visible cells × per-query cost
- Choppy scroll at small N that virtualization should have eliminated
- Profiling shows data-source queries during paint
- A `realize_with_range` (or equivalent) that produces geometry without content

The response is **not** point-optimization of the data source.

The response is:

> "We violated SPEC-PM-016."

Move content resolution to the realization phase. Cache by `(RowId, ColumnId)`. Clear at realization boundary. Paint reads from cache, full stop.

---

## 11. Closure

This failure was diagnosed and corrected in ListView in May 2026 after weeks of confusion in which the symptom (choppy scroll at 10k) was repeatedly misattributed to virtualization bugs, O(N) data-source reads, and text-shape cache pressure. Each of those was a contributing factor; none was the architectural cause. The architectural cause was that ListView realized geometry without content.

From this point on, building a virtualized widget that realizes geometry without content is not an oversight. It is a process violation. The TreeView `VisibleRow.label` pattern is the reference shape; every virtualized widget realizing per-cell content must follow it.
