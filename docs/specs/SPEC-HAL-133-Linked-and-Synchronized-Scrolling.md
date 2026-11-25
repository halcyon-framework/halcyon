# SPEC-HAL-133: Linked and Synchronized Scrolling

**Status:** Design / Binding
**Layer:** Engine coordination (cross-widget, non-render)
**Scope:** Coordinated scrolling across multiple `ScrollArea` surfaces
**Depends on:** SPEC-HAL-079, SPEC-HAL-080, SPEC-HAL-086, SPEC-HAL-110, SPEC-HAL-131, SPEC-HAL-131A, SPEC-HAL-131B
**Constrains:** Engine event pipeline + ScrollArea public API usage (no ScrollState sharing)

---

## SECTION A — Problem Statement

Halcyon scroll + virtualization is now structurally correct per SPEC-HAL-131A/B: realization during `layout_tree()`, anchor resolution post-layout, no realization from scroll writes.

What is missing is **cross-surface coordination**:

* table header ↔ body alignment
* multi-panel dashboards (chart + ladder + tape)
* split panes with shared Y axis
* minimap ↔ main viewport
* synchronized feeds (timeline + filtered view)

These behaviors must be defined as **engine-owned contracts** and must not contaminate scroll mechanics, layout, or virtualization correctness.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Coordination, not coupling**
Linking never merges state. Each surface remains valid independently.

**PRINCIPLE-2: Scroll intent, not widget mutation**
The coordinator issues scroll intents; widgets apply them through existing APIs.

**PRINCIPLE-3: Anchor-first**
Anchor-based sync is the default because it survives heterogeneous item sizes and dynamic insertion/removal.

**PRINCIPLE-4: No surprise work**
Linking must not trigger realization, layout, or anchor resolution synchronously.

**PRINCIPLE-5: Loop-free by construction**
Cycles must be prevented structurally, not “usually avoided”.

---

## SECTION C — Terminology

**Scroll Surface**
A `ScrollArea` that owns a `ScrollState` and defines a viewport over a content node.

**Link Group**
A set of surfaces participating in synchronization on one axis (X or Y).

**Leader Event**
A scroll intent produced by one surface (wheel, drag, programmatic scroll request).

**Follower Application**
A derived scroll intent applied to other surfaces.

**Sync Domain**
The mapping basis: offset, fraction, anchor, identity.

---

## SECTION D — Supported Sync Modes

### D.1 Offset Sync (Pixel / Logical Units)

Follower tracks leader offset (optionally transformed).

**Use cases**

* identical or compatible geometry
* pixel-perfect alignment requirements

**INV-LINK-MODE-OFFSET-1**
Follower offset is clamped to its own scroll bounds.

**INV-LINK-MODE-OFFSET-2**
Offset sync does not assume identical content extents.

---

### D.2 Fraction Sync (Normalized Scroll)

Follower tracks leader as a fraction of [0..1] over scrollable range.

**Use cases**

* two views of different total extents that should “move together”
* overview panes

**INV-LINK-MODE-FRACTION-1**
Fraction is computed from leader’s `(offset / max_scroll)` (well-defined even if max=0).

---

### D.3 Anchor Sync (Default / Robust)

Leader exports a scroll anchor reference; follower sets/updates its own anchor and resolves next frame.

**Use cases**

* virtualized lists
* heterogeneous feeds (Twitter-like, news articles)
* dynamic insertion/removal
* varying item heights
* infinite scrolling

**INV-LINK-MODE-ANCHOR-1**
Anchor sync never copies resolved offsets between surfaces.

**INV-LINK-MODE-ANCHOR-2**
Follower resolves anchor using its own realized geometry only (SPEC-131A/B).

---

### D.4 Identity Sync (Logical Item ID)

Leader scrolls to a logical identity (item key). Follower scrolls to the same key in its own model.

**Use cases**

* shared dataset projections
* multi-view selection tracking

**INV-LINK-MODE-ID-1**
Identity sync requires an explicit resolver mapping `ItemKey -> Anchor/Position`. No implicit search.

---

## SECTION E — Engine-Owned Coordinator

Introduce an engine subsystem:

**`ScrollLinkCoordinator`**

Responsibilities:

* track link groups
* observe leader intents
* generate derived follower intents
* enforce loop prevention + ordering rules

It must not:

* borrow widget internals beyond existing public APIs
* access layout data directly
* call realization
* resolve anchors

---

## SECTION F — Pipeline Ordering

Link processing is a frame phase, integrated with the existing ordering:

1. Input routing / event dispatch (may create leader intents)
2. **ScrollLinkCoordinator applies derived follower intents**
3. `layout_tree()` (realization happens here)
4. `post_layout_resolve_scroll_anchors()`
5. `paint_tree()`

**INV-LINK-ORDER-1**
Link processing occurs before `layout_tree()`.

**INV-LINK-ORDER-2**
Link processing must be pure scroll-state mutation (same class as Stage 16 audit).

**INV-LINK-ORDER-3**
Link processing must not call anchor resolution.

---

## SECTION G — Loop Prevention

Loops are not a “bug”, they are the default if you do symmetric linking naïvely.

**Mechanism requirement:** every applied derived intent carries a **link token**.

* A leader intent has token `T`
* Coordinator derives follower intents with token `T`
* Any scroll surface receiving an intent with token `T` must not re-emit it as a leader event for the same group within the same frame.

**INV-LINK-LOOP-1**
No derived intent may cause re-entry into the same link group during the same frame.

**INV-LINK-LOOP-2**
Symmetric linking is supported only via the token system (never via “ignore small deltas”).

---

## SECTION H — Failure Semantics

Linking must degrade cleanly without forcing work.

### H.1 Missing anchor in follower

Follower clamps to nearest feasible state and marks anchor dirty for next layout (if applicable).

**INV-LINK-FAIL-ANCHOR-1**
No retries inside same frame.

### H.2 Missing identity in follower

Event ignored.

**INV-LINK-FAIL-ID-1**
Identity sync must never scan or realize to “find” the key.

### H.3 Geometry mismatch

Offset/fraction clamps. Anchor sync remains valid.

**INV-LINK-FAIL-GEOM-1**
Mismatch never triggers layout.

---

## SECTION I — API Surface (Minimal)

No new widget APIs required if current `ScrollArea` already supports:

* set offset / scroll_by
* set anchor / mark anchor dirty
* query target offset (for leader intent extraction)

If missing, the engine introduces **intent structs** and uses existing methods.

**FORBID-LINK-SHARED-STATE**
Two scroll surfaces must never share a `ScrollState` instance.

**FORBID-LINK-WIDGET-COORDINATION**
Widgets must not coordinate among themselves. All linking lives in engine.

---

## SECTION J — Relationship to Virtualization

This SPEC does not change virtualization. It constrains linked scrolling to preserve it.

**INV-LINK-VIRT-1**
Linking may mark virtualization dirty only indirectly via scroll offset changes (allowed), but must not realize.

**INV-LINK-VIRT-2**
Anchor sync is the canonical solution for heterogeneous/infinite feeds.

---

## SECTION K — Acceptance Criteria

SPEC-HAL-133 is satisfied when:

* [ ] Two `ScrollArea`s can be linked on Y using offset sync without loops
* [ ] Two virtualized lists can be linked using anchor sync while preserving SPEC-131A/B ordering
* [ ] Symmetric linking is stable under wheel + drag input (no oscillation, no ping-pong)
* [ ] Derived follower intents do not trigger layout immediately
* [ ] No ScrollState sharing exists anywhere
* [ ] Tests exist for: loop prevention, clamp behavior, anchor missing behavior

---

## SECTION L — Authoritative Statements

**AUTH-133-1: “Linked scrolling is coordination of intent, not sharing of state.”**

**AUTH-133-2: “Anchor sync is the default because it survives reality: heterogeneity, virtualization, mutation.”**

**AUTH-133-3: “Loop prevention must be structural, not heuristic.”**

---
