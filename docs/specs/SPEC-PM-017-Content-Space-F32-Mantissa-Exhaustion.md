# SPEC-PM-017: **Content-Space f32 Mantissa Exhaustion**

### *Large Scroll Extents Quantize All Per-Row Geometry, Paint, and Hit-Testing*

**Author:** JC
**Status:** Post-Mortem + Permanent Rule
**Severity:** P0 — Silent visual and input corruption for all virtualized
content past ~16.7M logical px; progressive degradation from ~4.2M px
**Applies to:** Every scroll authority, every virtualized realizer, the
scene-graph scroll transform, and the input router's local-coordinate math
**Ruling:** JC, 2026-06-12 — remedy architecture (B): f64 scroll core,
viewport-relative conversion at the paint and hit boundaries

---

# 1. Problem

In the SPEC-HAL-259 TreeView stress demo (1,000,000 nodes × 24 px =
24M px content extent), disclosure toggles and guide lines visibly
break partway down the tree. Dotted guide segments fuse or vanish;
disclosure glyph strokes land on wrong pixels and deform. The
transition is abrupt and reproducible at a specific row.

The observed breaking point: between row 699,050 and row 699,051.

```
2^24 = 16,777,216        (f32 integer-precision ceiling)
16,777,216 / 24.0 = 699,050.67
```

The breaking point is not approximate. It is the exact row at which
content-space y crosses 2^24.

# 2. Root Cause

IEEE-754 f32 has a 24-bit significand. Representable-value spacing
(ULP) grows with magnitude:

| Magnitude range | ULP | Consequence at 24 px rows |
|---|---|---|
| [2^22, 2^23) ≈ [4.2M, 8.4M) | 0.5 | half-pixel snapping degrades (fractional DPI) — from row ~174,762 |
| [2^23, 2^24) ≈ [8.4M, 16.8M) | 1.0 | sub-pixel offsets vanish — from row ~349,525 |
| [2^24, 2^25) ≈ [16.8M, 33.6M) | 2.0 | 1px features die; y lands on a 2px lattice — from row ~699,051 |

Three arithmetic forms in the pipeline hit the ceiling:

**Form 1 — the content-space product.**
`(row_index as f32) * row_height` computed in f32
(TreeViewBody `row_y_abs`, body.rs:505; virtualization
`compute_fixed_height` y_offset, virtualization.rs:71). The product
quantizes to the ULP of its magnitude.

**Form 2 — catastrophic cancellation at the scroll subtraction.**
Content-space y (~16.7M, ULP 2.0) minus scroll offset (~16.7M, ULP
2.0) is performed in f32 by the batcher (`apply_scroll_to_rect`,
fed by the scene `Scroll` node) and by the `hts_rows` adapter
(hts_rows.rs:57). The result — the on-screen position — inherits the
2px lattice of its operands even though its own magnitude is small.

**Form 3 — f32 accumulation.**
Variable-height prefix sums (`prefix_sums: Vec<f32>`,
virtualization.rs:117–122) accumulate row heights in f32. Summation
error grows with row count; this form degrades earlier than the
product form and non-deterministically.

The same cancellation poisons the **input path**: the router computes
widget-local coordinates as
`global_pos + cumulative_scroll_offset − abs_rect` in f32
(event_router.rs, all pointer arms). In the deep zone, hit positions
quantize identically — disclosure toggles fail by hit as well as by
paint.

Scroll **state** itself stalls: `scroll_y: f32` at 16.7M cannot
represent increments below 2.0, so smooth-scroll steps under 2 px are
no-ops.

# 3. Why It Was Hard to Find

- Invisible below ~4.2M px of content; demos and tests use small trees.
- The paint code is locally correct — every individual line is
  reasonable f32 arithmetic. The defect is the *magnitude regime*,
  not any single expression.
- Symptoms masquerade as styling bugs (guide dots "look wrong") or
  input flakiness (toggles "miss"), pointing at widgets rather than
  at coordinate representation.
- The breaking point depends on row height: changing density moves
  the cliff, defeating naive bisection.

The diagnostic that cracked it: a user-observed abrupt visual
transition at a *specific row index*, and the recognition that
`row × row_height` at that index equals 2^24 exactly.

# 4. The Fix (Ruling B — f64 scroll core, f32 view space)

64-bit document space, 32-bit view space — the standard professional
answer. Precision-critical arithmetic (content-space products, sums,
and the content→viewport subtraction) happens in f64, where 24M px
has exact integer representation with 2^29 to spare. Only the small
viewport-relative result — bounded by viewport size — is cast to f32
and enters the scene graph, the batcher, and the GPU pipeline. The
GPU-facing f32 pipeline is untouched (GPU-First preserved).

Concretely:

1. **Scroll authorities store f64.** `HierarchicalTabularSubstrate::scroll_y`,
   ScrollArea's scroll core (target, visual, anchor resolution), and
   any future scroll owner.
2. **Content-space y is computed in f64.** `index as f64 * height as f64`;
   prefix sums accumulate f64.
3. **One subtraction, in f64, at each boundary.** Paint: virtualized
   bodies compute `(content_y_f64 − scroll_f64) as f32` and submit
   viewport-relative coordinates. Hit: local-coordinate derivation
   performs the same single f64 subtraction before comparing against
   small geometry.
4. **The scene-graph scroll transform carries no content-magnitude
   offsets for f64-authority subtrees.** Bodies that pre-subtract
   paint viewport-relative; the residual scene `Scroll` offset for
   those subtrees is zero. (Small-extent uses of the scene scroll
   transform remain legal — the forbidden thing is content-magnitude
   f32 offsets riding the scene.)

# 5. Invariants

**INV-PM-017-1 (f64 Scroll Authority):** Every scroll authority
stores its offset(s) in f64. Public scroll surfaces (`scroll_to`,
`scroll_offset`) speak f64.

**INV-PM-017-2 (f64 Content Arithmetic):** Content-space positions
and extents — index×height products, prefix-sum accumulation,
content totals — are computed and stored in f64 wherever the extent
is unbounded or may exceed 2^22 logical px.

**INV-PM-017-3 (Single-Subtraction Boundary):** Content→viewport
conversion is one f64 subtraction at the paint or hit boundary. Only
the viewport-relative result (magnitude ≤ viewport + one row) is
cast to f32. No f32 value of content magnitude crosses into the
scene graph, the batcher, or local-coordinate event math.

**INV-PM-017-4 (View Space Stays f32):** The scene graph, batcher,
vertex pipeline, and theme geometry remain f32. This spec adds f64
to document space only; widening the GPU-facing pipeline is
forbidden scope creep.

# 6. Forbidden Behaviors

**FORBID-PM-017-1:** Computing a content-space product or sum in f32
where the result may exceed 2^22, including via `as f32` on a row
index before multiplication.

**FORBID-PM-017-2:** f32 accumulation of row heights or any
content-extent prefix sum.

**FORBID-PM-017-3:** Relying on the batcher's scroll subtraction
(`apply_scroll_to_rect`) to viewport-relativize content-magnitude
coordinates. The subtraction must already have happened, in f64, at
the producer.

**FORBID-PM-017-4:** Declaring a content-extent ceiling as the
remedy. A documented limit is not a fix (ruling: option C rejected,
2026-06-12).

# 7. Detection

1. Symptom signatures: 1px constructs (dots, hairline strokes,
   glyph details) degenerate *only past a content depth*; hit zones
   feel coarse or off-by-a-pixel in the same region; smooth scroll
   stutters or stalls at depth; row positions pop between frames.
2. Compute the suspect's cliff: `2^24 / row_height` (1px death),
   `2^23 / row_height` (sub-pixel death), `2^22 / row_height`
   (fractional-DPI snapping death). A reproducible transition at one
   of these indices is this pathology, proven.
3. Inventory every f32 in the chain: scroll state → content-y
   production → scene transform → batcher subtraction → hit-test
   local math. Any content-magnitude f32 is a violation site.

# 8. Afflicted-Site Inventory (verified 2026-06-12)

| Site | Form | Status |
|---|---|---|
| `engines/hierarchical/mod.rs:112` `scroll_y: f32` (+ `scroll_to`/`scroll_offset`/`viewport_bottom`) | state, Form 2 | remediation rung 1 |
| `engines/hierarchical/virtualization.rs:71` fixed-height `y_offset` product | Form 1 | rung 1 |
| `engines/hierarchical/virtualization.rs:117–122` `prefix_sums: Vec<f32>` | Form 3 | rung 1 |
| `widgets/tree_view/body.rs:505` `row_y_abs` content product (supersedes INV-TV-PAINT-CONTENT-COORDS for y) | Forms 1+2 | rung 1 |
| `widgets/list_view/body.rs:551` `rect.y + y_offset` + realization y production | Forms 1+2 | rung 2 |
| `widgets/scroll_area.rs` scroll core (`target_scroll_y`, `visual_scroll_y`, smoothing, anchors, `scroll_offset()` trait surface) | state + Form 2 | rung 3 |
| Scene `Scroll` node `(f32, f32)` + `renderer/batcher.rs` `push_scroll`/`apply_scroll_to_rect` | Form 2 boundary | rung 3 |
| `engines/temporal/hts_rows.rs:57` `(first_y + i×h) − scroll` | Forms 1+2 | rung 4 |
| `core/event_router.rs` local-pos math (`pos + cumulative_scroll_offset − abs_rect`, all pointer arms) | Form 2, input | rung 3 |
| PropertyEditor body, EventLog, remaining virtualized realizers | suspected, unverified | rung 5 audit |

# 9. Remediation Ladder

One rung per task; each rung complete for its scope; gates green at
every rung. Order: **R1** HTS f64 scroll core + virtualization f64 +
TreeView paint/hit boundary (kills the reproduced bug end-to-end).
**R2** ListView path. **R3** ScrollArea f64 scroll core + scene/sync
seams + router local-pos f64 subtraction. **R4** `hts_rows` adapter.
**R5** audit remaining realizers + 1M-row stress acceptance at full
depth (paint, hit, and scroll all clean past row 700,000).

**Supersession (SPEC-HAL-002A discipline):**
- `INV-TV-PAINT-CONTENT-COORDS` (tree_view/body.rs:503) is
  superseded for the y axis: canonical per-row y becomes the
  f64-subtracted viewport-relative value. The "one canonical Y for
  every per-row primitive, no per-pass drift" clause survives
  unchanged — only the coordinate space and width change.
- SPEC-HAL-259 §5.5 (scroll state) is amended by INV-PM-017-1: the
  substrate's scroll state is f64.

# 10. Related

- **SPEC-PM-008** (Projection Truncation Jitter): also a precision
  bug, different layer — integer truncation of *viewport*
  dimensions in the projection matrix. PM-017 is mantissa
  exhaustion of *content* coordinates. Both forbid silent precision
  loss; neither subsumes the other.
- **SPEC-PM-002** (Coordinate-Space Confusion): PM-017's boundary
  discipline (who subtracts scroll, exactly once) is PM-002's
  contract sharpened with a numeric-representation law.
- **SPEC-HAL-337 §5 / FORBID-337-2** (`hts_rows` adapter): the
  adapter's documented coordinate contract gains the f64
  subtraction in rung 4.

# 11. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode | N/A |
| B — Phase Separation | Preserved. Boundary subtraction sits at existing phase seams. |
| C — ID-based Identity | N/A |
| D — Pure Data | **Strengthened.** Document space becomes exactly representable. |
| E — GPU-First | **Preserved.** View space and GPU pipeline stay f32 (INV-PM-017-4). |
| F — Sovereignty | **Strengthened.** Scroll authorities own offsets at full precision; no consumer re-derives. |
