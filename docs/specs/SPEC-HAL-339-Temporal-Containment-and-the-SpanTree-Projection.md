# SPEC-HAL-339: **Temporal Containment and the SpanTree Projection**

### *A Second Projection Family over the Temporal–Topology Substrate Pair*

**Author:** JC
**Status:** Ruled — fork (b), SpanTree-first, adopted (§6). §7 answered by SPEC-HAL-340 (Q1) and SPEC-HAL-341 (Q2–Q8). No code is authorized by this document; implementation is gated by SPEC-HAL-341 tasks.
**Layer:** Architecture (Projection Family)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-334 (Time Model), SPEC-HAL-335 (TemporalSubstrate), SPEC-HAL-336 (TimelineHeader and TimelineCanvas, §8 as amended), SPEC-HAL-337 (Shared Topology Substrate)

---

# 1. Purpose and Position

This document records an architectural realization and frames the fork it opens. It exists so that no future reader — human or model — reduces the question to "a TreeView-Gantt hybrid widget." The widget-shape question exists (§7 Q6), but it is downstream of the projection question, which is the actual content.

# 2. Charter, Restated

Three lines, none of them new — this is the design intent of the 334–337 lineage, restated because conversational terminology ("Gantt") lagged the architecture:

1. The temporal stack is a **general interval projection architecture**, not a scheduling engine. Its vocabulary is domain-free: `Timestamp`, `TimeRange`, `IntervalId`, `IntervalDescriptor`, lanes, styles.
2. Scheduling semantics — tasks, dependencies-as-logic, CPM, leveling — are application-layer by the projection-only law (SPEC-HAL-336 §11).
3. `TimelineCanvas` is therefore **one projection** of the substrate — the row-bound, scheduler-shaped one — not *the* projection. SPEC-HAL-337 §1 already states the general form: widgets, including "any future view," are projections over application-owned substrates.

# 3. The Containment Realization

In span-structured data — process phases, traces, nested operations, lifecycle stages — the hierarchy is itself temporal: a child's range lies within its parent's range. The primary information is not *when does X happen* but *X exists within Y*. Time exposes containment; containment is the structure.

The bound composition (336 §8) encodes one structure twice: the tree pane restates, in indentation and guide lines, ancestry that the bar geometry could carry directly. Each pane is information-poorer than the data it projects.

**Principle (the thread of the source conversation):** prefer projections **isomorphic to the data** — every pixel dimension carries a data dimension. No pane spent restating what another pane already encodes.

# 4. SpanTreeView — The Second Projection Family

One glyph per node. The node's bar *is* its row:

```
frame ────────────────────────────────────────────
  layout ──────────────────
    measure ────────
    arrange ──────
  paint ────────────────
    text_upload ──────
  present ─────
```

**Encoding (drill-in — see SPEC-HAL-341).** One ViewRoot fills the frame; x = instant, width = duration. Only the ViewRoot's *direct children* render, on one level — lanes resolve same-level overlap. Ancestry is the breadcrumb (the root path), not vertical indentation. Descent is an explicit enter (re-root onto a child); ascent is an automatic pop on zoom-out past the root's extent. The vertical flame-stack diagram above illustrates the alternative idiom — all levels stacked, TreeView-style fold — which is **not** adopted for v1.

This projection family is not hypothetical. It is the span/trace/waterfall view — Tracy, perfetto, Jaeger, flame charts — converged upon by an entire tool category because it sits closer to the data, and lived in daily by exactly Halcyon's target audience. Its existence is proven; only its priority and widget shape are open.

**First-party consumer:** Halcyon's own frame instrumentation (`[HAL-PASS]` phase timing). The framework profiling itself is the dogfood loop.

# 5. Structural Claim — Same Substrate, Third Consumer

The SpanTreeView is **not** a new architecture. It is a third consumer of the pair already built and bound:

* **Topology:** `SharedHts` (SPEC-HAL-337) — virtualization, expand/collapse, selection state, intent funnel: paid for.
* **Intervals:** TemporalSubstrate (335) — descriptors, intents, `TimeContext` cross-view sync.
* **Time geometry:** TimeGeometryEngine — `x_of`, `scroll_by_px`, `zoom_at` under the INV-335-G3 anchor law; `TimelineHeader` alignment under INV-336-1.
* **Selection:** the unified row-level ruling (336 §8 as amended) becomes a tautology — the bar *is* the row glyph; no mirror, no second authority, because there is no second widget.

Consequence: nothing in the 337 binding, the 336 §8 amendment, or the intent work is invalidated by this lane under any ruling. The only genuinely new construction is the projection layer itself — one widget body that places each visible row's bar at its time position instead of in a fixed column — plus whatever §7 Q1 decides about containment law.

# 6. Relationship to the Gantt Lane

The two projections are siblings over identical engines, shaped by their data:

* **Gantt** (TimelineCanvas bound mode; possibly a merged widget): independent per-row ranges, lanes, dependency arrows, drag-editing. Scheduler-shaped.
* **SpanTree**: nested ranges, structure-first reading, typically read-mostly. Containment-shaped.

The earlier TreeView/Timeline merge question is a **Gantt-lane widget-shape question**; it neither requires nor excludes the SpanTree, and the canvas-core extraction it implies would serve both (Q6).

**Sequencing fork (for ruling — not ruled here):**
(a) Gantt lane first — finish stage-6 validation, then the merge decision;
(b) SpanTree lane first — RFC answering §7, then build;
(c) substrate-validation first — finish the current ladder as the cheapest end-to-end proof of the engine pair, then choose with a working binding in hand.

`TemporalIntent::DeselectAll` (ladder task 2) is intent law and proceeds under every branch.

**Ruling (2026-06-13).** Fork (b) — the SpanTree lane — adopted, in the observational / read-only, zoom-threshold (dimensional drill-in) form. SPEC-HAL-340 (containment relation) and SPEC-HAL-341 (SpanTreeView projection) realize it. The flame-stack and the HTS-bound mode remain available as later, separately-specified variants.

# 7. Open Questions — Gate Implementation

**Q1 — Containment law.** Is `child.range ⊆ parent.range` an enforced substrate relation (new invariant + validation) or emergent (topology asserts parent/child; ranges free)? Trace data satisfies it natively; schedule data does not. Per the mandatory-policy law (337 §2 precedent), the implementing RFC must define a `ContainmentPolicy` before code.

**Q2 — Free mode.** Does SpanTree exist without an HTS (its own span source), mirroring TimelineCanvas free mode, or is it HTS-bound only?

**Q3 — Collapse semantics.** A collapsed parent's bar shows its own range, or the union extent of its descendants? These coincide only under enforced containment (Q1).

**Q4 — Lanes.** Do per-row lanes (335) apply inside a span row, or is depth the only vertical structure in this projection?

**Q5 — Name gutter.** Pure isomorphism (labels on/beside bars only) versus a frozen name column — Tracy and perfetto keep one. Density and deep-zoom usability trade.

**Q6 — Widget shape.** Standalone `SpanTreeView` widget, a TreeView projection mode, or a generalization of a merged-Gantt body. Interacts with the canvas-core extraction question; whichever lane goes first should extract once.

**Q7 — Header reuse.** `TimelineHeader` is expected to apply unmodified (INV-336-1 is projection-agnostic) — confirm no row-bound assumptions exist.

**Q8 — Interaction tier.** Which `TemporalIntent`s apply: `Select`/`Activate`/`ContextMenu` clearly; `DragMove`/`DragResize` presumably excluded in a read-mostly v1. The implementing RFC defines the policy tiers per the 336 §7 model.

## §7 Resolution

Q1 → SPEC-HAL-340: Enforced containment (`child.range ⊆ parent.range`), validated; never clamped.
Q2 → SPEC-HAL-341 §4: free-mode v1 (own `ContainmentSource`, no HTS). HTS-bound deferred.
Q3 → dissolves: there is no in-place collapse — descent re-roots (drill-in), ascent pops. No subtree-fold semantics in v1.
Q4 → SPEC-HAL-341 §5: lanes resolve overlap *within* the rendered level; depth is navigated (enter/pop), not stacked.
Q5 → SPEC-HAL-341 §10: breadcrumb renders the root path. A frozen name gutter is deferred.
Q6 → standalone SpanTreeView widget for v1. Extraction of a shared canvas core is deferred until a second contained-projection exists.
Q7 → confirmed: TimelineHeader reused unmodified.
Q8 → SPEC-HAL-341 §7: read-only (Select / Activate / ContextMenu + cursor readout). No edit intents.

# 8. Non-Goals

No implementation order, no widget API, no spec amendments. The next artifact after ruling is the RFC/SPEC for the chosen lane, answering §7 with invariants.
