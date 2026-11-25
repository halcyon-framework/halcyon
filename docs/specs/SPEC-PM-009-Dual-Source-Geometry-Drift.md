# SPEC-PM-009: Dual-Source Geometry Drift in Variable-Height Virtualization

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Failure Law
**Owner:** Core Architecture
**Audience:** Engine, Widget, Interaction
**Applies To:** Console, EventLog, ListView, TreeView, Grid, any widget with variable-height rows

---

## 0. Pathology Name

**Dual-Source Geometry Drift**

Also known as:
- "Measurement doesn't match paint"
- "Rows paint in wrong place after wrapping"
- "Hit-test misses the row I clicked"
- "Gaps between entries"
- "Text overlapping other text"
- "Selection offset grows further down the viewport"
- "Works at top, wrong at bottom"

---

## 1. Summary of the Failure

This pathology occurs when **two or more code paths independently compute per-row geometry** for a variable-height virtualized list, and their results drift.

The failure manifests in four forms:

1. **Measurement vs Paint**: Height measurement function computes N visual lines, paint function computes N+1 or N-1 visual lines for the same row → gaps or overlaps
2. **Paint vs Hit-Test**: Paint uses exact accumulated heights, hit-test uses fixed-height approximation → clicking below the first few rows hits the wrong row, error grows with distance from viewport top
3. **Layout-Cache vs Live-Recompute**: Layout caches geometry, paint/hit-test recomputes live (or vice versa) → cache goes stale between phases, or live recompute disagrees with cached values
4. **Width-Source Divergence**: Two paths call different shaping functions (`shape()` vs `measure()`) that return subtly different widths → line-break decisions differ

All four are the same root cause: **no single source of truth for per-row geometry**.

This bug has appeared in Console wrapping implementation across multiple iterations, each "fix" introducing a new variant.

It must never be rediscovered.

---

## 2. Observable Symptoms

Any of the following are red flags:

1. Gaps appear between rows that should be adjacent
2. Text from one row overlaps text from an adjacent row
3. Hit-test accuracy degrades further from viewport top
4. Selection highlight appears at a different row than was clicked
5. Glitches appear only in variable-height mode, never in fixed-height mode
6. Visual artifacts appear after scrolling, not before
7. "Fix" involves adjusting one path to match another
8. Two functions named similarly (`measure_*`, `paint_*`, `hit_test_*`) contain the same algorithm

If any of these occur, **this pathology is active**.

---

## 3. Root Cause (Architectural)

The failure is caused by **independent implementations of the same geometry algorithm**.

Specifically:

- Measurement computes row height by walking spans and applying wrap rules
- Paint independently computes word positions by walking the same spans with the same wrap rules
- Hit-test independently computes row Y positions with its own assumption (often fixed-height)
- Each implementation is individually correct; they drift because nothing forces them to agree

This is **not a math bug**.

The arithmetic in each path is locally correct.
Each function works in isolation.

The failure occurs because **the same computation is performed by separate code**, and separate code drifts.

---

## 4. Critical Clarification

> Calling the same helper function is not enough.

Even if:
- Measurement calls `compute_wrapped_lines()`
- Paint calls `compute_wrapped_lines()`
- Hit-test calls `compute_wrapped_lines()`

The bug still occurs if:
- Each call happens at a different phase with different inputs (different viewport width, different font size, different cache state)
- Results are stored and read across phase boundaries with no invariant preserving them
- One path uses the result, another path re-computes with subtly different inputs

The rule is **one computation, one result, consumed by all**. Not "same function called multiple times".

---

## 5. Why the Bug Reappears

This pathology reappears because:

1. Measurement and paint feel like naturally separate concerns
2. Hit-test feels like a separate concern from rendering
3. Each path individually looks correct in code review
4. Fixed-height mode masks the bug (all three paths trivially agree)
5. Testing at small data sets masks the bug (one-line entries don't wrap)
6. Tests pass because each path produces plausible output
7. Integration reveals drift only when entries actually wrap, and drift grows with count
8. "Fixes" that make one path match another are temporary — the next change reintroduces drift
9. No written invariant forbids independent geometry computation

Without explicit law, the mistake is repeated with every new variable-height widget.

---

## 6. Hard Prevention Rules (Binding)

### PM-009-R1: Single Geometry Source

A variable-height virtualized widget MUST have exactly **one** function that computes per-row geometry (visual line count, line positions, segment positions).

Measurement, paint, and hit-test MUST consume the output of that single function. They MUST NOT implement the algorithm independently, even if the implementations "obviously" match.

---

### PM-009-R2: Single Computation Per Frame

Per-row geometry for visible rows MUST be computed at most once per frame.

If measurement computes geometry during layout, paint MUST consume the result, not re-compute.
If paint computes geometry, measurement MUST consume the same result, not re-compute.
If both compute, they MUST share the same cached output within the frame.

---

### PM-009-R3: Hit-Test Uses Same Source

Hit-test for rows in a variable-height widget MUST use the same geometry source as paint.

If paint uses exact wrapped heights, hit-test MUST walk the same heights — not approximate with `floor(y / fixed_row_h)`. Approximation in hit-test while paint is exact is a **guaranteed** drift source.

Exception: if the widget uses fixed-height mode globally, hit-test may use fixed-height arithmetic.

---

### PM-009-R4: No Independent Width Sources

When line-breaking depends on word widths, all paths that make wrap decisions MUST obtain widths from the same function.

Forbidden: one path uses `text_engine.measure(word, size)`, another uses `text_engine.shape(word, size).width`, even if they "should" return the same value.

---

### PM-009-R5: Geometry Data Survives Phase Transitions

If geometry is cached in layout for consumption by paint, the cache MUST be invalidated when any input changes between the two phases, including:
- Scroll position (if scroll affects visibility window selection)
- Viewport width
- Font size
- Entry content
- View mode (Live/Review/snapshot transitions)

If invalidation is impractical, paint MUST re-compute from live data, not trust the cache.

Stale cache between layout and paint is indistinguishable from dual-source drift.

---

## 7. Affected Systems

This pathology applies to:

- Console (wrap mode)
- EventLog (expandable rows)
- ListView (variable-height rows)
- TreeView (variable-height nodes)
- Grid (variable-height cells)
- PropertyEditor (nested content)
- Any widget where row height is not uniform

If a widget's rows have variable height, **this SPEC applies**.

---

## 8. Forbidden Patterns (Explicit)

The following patterns are forbidden:

**Dual-source geometry:**
- `fn measure_row_height(...) { /* walks spans, computes wraps */ }`
  `fn paint_row(...) { /* walks spans, computes wraps */ }` — two implementations of the same algorithm
- "Paint re-computes wrap positions for rendering"
- "Hit-test re-computes row boundaries to find clicked row"

**Width source divergence:**
- `text_engine.measure(word, size)` in one path and `text_engine.shape(word, size).width` in another
- "Fast path uses approximate width, slow path uses exact width"

**Approximation drift:**
- Paint uses exact heights from `compute_wrapped_lines()`
  Hit-test uses `floor(y / fixed_row_h)` — drift guaranteed
- "Hit-test is close enough, off-by-one is fine"

**Stale cache:**
- Layout populates cache, paint reads cache
  Cache not invalidated when view mode changes / snapshot taken / scroll jumps
- "Cache is only used for paint so invalidation isn't critical"

Any appearance of these patterns is an SPEC-PM-009 violation.

---

## 9. Diagnostic Invariants

To verify correct implementation:

**INV-GEOM-SINGLE**: For any row, the visual line count returned by measurement equals the visual line count rendered by paint equals the visual line count walked by hit-test.

**INV-GEOM-PARITY**: If paint positions row N at Y=y, then hit-test at Y=y returns row N (± floating-point epsilon, not ± row height).

**INV-GEOM-DRIFT-ZERO**: Hit-test accuracy does NOT degrade with distance from the viewport origin. A click at Y=1000 is as accurate as a click at Y=10.

If these invariants fail, dual-source drift is occurring.

---

## 10. Relationship to Existing SPECs

This SPEC constrains and overrides assumptions in:

- SPEC-HAL-118 (Virtualized ListView Contract) — if implementations independently compute row geometry
- SPEC-HAL-164 (TabularSubstrate) — variable-height paths
- SPEC-HAL-305 (Console Widget) — wrap mode geometry
- SPEC-HAL-312 (EventLog Widget) — expandable row heights

If any interpretation of those SPECs permits independent geometry computation across measurement/paint/hit-test, **that interpretation is invalid**.

---

## 11. Enforcement Rule

If a future implementation:
- Has gaps or overlaps between variable-height rows
- Has hit-test that drifts further from the viewport origin
- Has paint that disagrees with measurement on row count or position
- Has two functions with the same wrap algorithm

The response is **not debugging**.

The response is:

> "We violated SPEC-PM-009."

Unify the geometry source. Do not patch symptoms path-by-path.

---

## 12. Closure

This failure already happened.

It consumed multiple debugging cycles, each "fix" revealing a new drift variant.

From this point on, reintroducing independent geometry implementations in variable-height virtualization is not an accident.

It is a process violation.
