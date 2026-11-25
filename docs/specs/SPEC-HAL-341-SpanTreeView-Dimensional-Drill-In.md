# SPEC-HAL-341: **SpanTreeView — Dimensional Drill-In**

### *Single-Level, Read-Only Projection of a Contained Temporal Substrate*

**Author:** JC
**Status:** Design — Binding upon adoption (implementation gated on task authorization; no code authorized by this document)
**Layer:** Projection (Widget)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-334 (Time Model)
* SPEC-HAL-335 (TemporalSubstrate Architecture)
* SPEC-HAL-336 (TimelineHeader and TimelineCanvas) — header reuse, interaction-tier model
* SPEC-HAL-339 (Temporal Containment and the SpanTree Projection) — resolves §7 Q2–Q8
* SPEC-HAL-340 (Temporal Containment) — the consumed relation

---

# 1. Purpose

Define SpanTreeView: a read-only, single-level, drill-in projection of a containment-bearing temporal substrate (SPEC-HAL-340). The view shows one **ViewRoot**'s direct children as a flat, time-positioned timeline. Descending re-roots the view onto a child; ascending pops on zoom-out. The recursion is in navigation, not rendering — the projection is flat at every level, pointed at a different range.

Resolves SPEC-HAL-339 §7 Q2–Q8. First consumer: Halcyon's `[HAL-PASS]` frame instrumentation (the dogfood).

# 2. Position

This supersedes the vertical flame-stack visual sketched in SPEC-HAL-339 §4. The two trace idioms — depth-stacked flame graph and dimensional drill-in (focus) — are alternatives. v1 is drill-in: simpler render, clean zoom semantics, context carried by the breadcrumb rather than by vertical depth.

Projection-only: the framework computes no trace or profiling analytics. The substrate and source own data; the projection places and renders. SpanTreeView is a manifestation of the substrate (SPEC-HAL-335 §2), not a new mechanism.

# 3. Vocabulary

**span** — a contained interval (`IntervalDescriptor`) read through this projection. The substrate type is unchanged (SPEC-HAL-335 §6.1); "span" is this projection's reading of it, the way a Gantt reads an interval as a task bar. The domain-free substrate (SPEC-HAL-339 §2) is preserved — no substrate type is renamed.

**ViewRoot** — the span whose subtree is currently projected and whose `range` bounds the viewport. Always defined. At the top, the ViewRoot is the synthetic root (`parent = None`), whose children are the top-level spans.

**root path** — the ancestry chain from the global root to the ViewRoot. The breadcrumb renders it. It is the only context surface and the only up-navigation path.

# 4. Mode

**Free-mode v1.** SpanTreeView binds a `ContainmentSource` directly. It does **not** use a `RowSource` / SharedHts (SPEC-HAL-337): there is no persistent row topology — the rows are the ViewRoot's children, resolved structurally per frame. HTS-bound SpanTreeView (SPEC-HAL-339 §7 Q2, other branch) is deferred; SPEC-HAL-337 is untouched and unused here.

# 5. Projection Model

**Frame.** The `TimeGeometryEngine` frame is fit to `ViewRoot.range` on enter and on pop. Zoom and pan *within* the root are free (INV-335-G3/G4 unchanged); the dimensional transitions occur only at the root's extent boundary (§6).

**Render set.** `children_of(ViewRoot, visible_range)` — the ViewRoot's direct children only (structural, per SPEC-HAL-340 INV-340-C1, then clipped to the visible range). One level. The `LaneLayoutEngine` (SPEC-HAL-335 §8) resolves same-level overlap — parallel spans stack into lanes within the level. Nothing is nested.

**Enterable affordance.** A child for which `has_children(id) == true` carries a visual mark. It is the double-click target. A child with no children carries no mark and is a leaf.

# 6. Navigation

**NavStack.** An ordered stack of `IntervalId` — the root path. The ViewRoot is its top. Push = enter; pop = ascend. NavStack is view state, not data.

**Enter (explicit).** A primary double-click on an enterable child pushes that child onto the NavStack; it becomes the ViewRoot; the frame re-fits to its range. There is no automatic enter on zoom (SPEC-HAL-339 ruling).

**Pop (automatic).** Zooming out until the visible range exceeds the ViewRoot's extent pops the NavStack to the parent and re-fits the frame to the parent's range. Hysteretic (INV-341-V3). The global root does not pop.

**Transition.** Enter and pop animate a frame-fit zoom. A hard cut is forbidden (FORBID-341-4).

# 7. Interaction Tier

Read-only (SPEC-HAL-339 §7 Q8). The projection emits, from the SPEC-HAL-335 intent vocabulary, only:

```
Select { id, modifiers }
Activate { id }
ContextMenu { id, ts }
```

plus the cursor-instant readout. `DragMove` and `DragResize` are excluded; the projection emits no edit intent and mutates no source.

# 8. Invariants

**INV-341-V1 (ViewRoot Frame):** the viewport extent equals `ViewRoot.range` at rest. The frame is a function of the NavStack and is never independent of it.

**INV-341-V2 (Single-Level Render):** the projection realizes exactly the ViewRoot's direct children (SPEC-HAL-340 INV-340-C1) — never descendants, never the ViewRoot's siblings, never the whole tree. This bounded-realization guarantee is **sound only because containment is Enforced** (SPEC-HAL-340 INV-340-C2): a parent off the extent, or below the descent threshold, is never entered, so its entire subtree is never realized — one range test prunes it.

**INV-341-V3 (Transition Hysteresis):** the pop boundary is hysteretic. A span that became the ViewRoot at a given scale does not pop until the visible range exceeds its extent by a fixed margin. There is no per-frame flip-flop at the boundary. The projection runs every frame (SPEC-HAL-002 frame-driven law), so hysteresis is mandatory, not cosmetic.

**INV-341-V4 (Explicit Descent):** the ViewRoot deepens only by an explicit enter. Zoom-in never auto-descends (SPEC-HAL-339 ruling).

**INV-341-V5 (Read-Only):** the projection emits only Select / Activate / ContextMenu and never mutates the source. NavStack is view state, not data.

**INV-341-V6 (Structural Identity):** ViewRoot, NavStack entries, enter targets, and selection are keyed on `IntervalId` (INV-335-ID1). A NavStack entry whose id leaves the source (`SetReplaced`) is pruned to the nearest surviving ancestor — the analog of selection pruning (INV-335-S2).

**INV-341-V7 (Enforced-Only):** SpanTreeView binds only a `ContainmentSource` whose `policy()` is `Enforced`. An `Emergent` source is rejected at bind time.

# 9. Forbidden

**FORBID-341-1:** nesting — rendering a child's children inside its bar, or any depth band, in this projection. One level only. The SPEC-HAL-339 §4 flame-stack is the other idiom, not this one.

**FORBID-341-2:** a parent-envelope bar behind the children. The ViewRoot *is* the viewport extent and is never drawn as a bar. This is the structural reason the parent-background occlusion class (SPEC-PM-007) cannot arise in this projection — there is no parent rect to be over-ordered against its children.

**FORBID-341-3:** automatic enter on zoom-in (INV-341-V4).

**FORBID-341-4:** a hard-cut dimensional transition. Enter and pop animate the frame-fit.

**FORBID-341-5:** edit intents (`DragMove` / `DragResize`) or any source mutation (INV-341-V5).

**FORBID-341-6:** a persistent multi-level row topology or `RowSource` in free-mode v1. Rows are the ViewRoot's children, resolved per frame (§4).

# 10. Reuse — Unchanged

`TimelineHeader` applies unmodified (SPEC-HAL-339 §7 Q7) — it projects the geometry engine's tiers and is projection-agnostic.

Selection is the row tautology (SPEC-HAL-339 §5): the bar *is* the row. `OrderedSelectionState<IntervalId>` is reused; there is no selection mirror.

`TimeGeometryEngine`, `LaneLayoutEngine`, and `IntentQueue<TemporalIntent>` are reused as-is.

The breadcrumb (root path) and the enterable mark are **mandatory** in v1. Without them a single-level view is unnavigable — they are not polish, they are the navigation surface.

# 11. Non-Goals

No implementation, no widget API surface, no HTS-bound mode, no flame-stack mode. Those are separate artifacts. No code is authorized until tasks are dispatched.
