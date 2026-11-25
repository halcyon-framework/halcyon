# SPEC-HAL-342: **Temporal Emphasis**

### *Shared Hover/Focus State and Linked-Item Resolution over the Temporal Substrate*

**Author:** JC
**Status:** Design — Binding upon adoption (implementation gated on task authorization; no code authorized by this document)
**Layer:** Substrate (Structural Engine)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-334 (Time Model)
* SPEC-HAL-335 (TemporalSubstrate Architecture) — sibling of OrderedSelectionState (§3, INV-335-S1)
* SPEC-HAL-340 (Temporal Containment) — the Containment relation's source

---

# 1. Purpose

Define a shared emphasis state over identity-keyed intervals: a hover/focus id and a resolved set of *linked* ids under a declared relation, exposed by the substrate for projections to render. It is the transient sibling of selection (SPEC-HAL-335 §3, INV-335-S1) — selection persists, emphasis follows the pointer.

It reinforces the bound tree↔canvas correspondence (Identity) and supplies the SpanTree's structural reveal (Containment). It lives at the substrate so every projection shares one mechanism instead of each growing its own — and so two views over one substrate (overview and main) can highlight together.

This SPEC defines a state and its resolver. No rendering — that is per-projection (SPEC-HAL-336, SPEC-HAL-341).

# 2. Position

Purely additive to SPEC-HAL-335. `begin_frame`, the snapshot, `IntervalDescriptor`, `IntervalSource`, and every existing invariant are unchanged. Emphasis is a substrate query parallel to `selection()`, not a field on the snapshot.

Emphasis is the peer of `OrderedSelectionState`: same id-keyed shape, same substrate placement; transient where selection is persistent. The two compose and neither reads nor writes the other (INV-342-E5).

v1 relations are Identity and Containment. Dependency is reserved, not implemented: its links cross the visible window with no off-screen affordance, and its useful form — the chain — is graph traversal forbidden by the projection-only law (FORBID-342-4).

# 3. EmphasisRelation

Per the mandatory-policy discipline (SPEC-HAL-337 §2):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmphasisRelation {
    /// The focus alone, emphasized wherever it is drawn — across the panels and
    /// views that share the substrate. The linked set is empty; needs no source.
    Identity,
    /// Focus plus its direct parent and direct children — one hop (SPEC-HAL-340
    /// INV-340-C1). Requires a ContainmentSource at resolve time; absent one,
    /// degrades to Identity (INV-342-E7).
    Containment,
    // Dependency — reserved. Not implemented in v1 (FORBID-342-4).
}
```

# 4. State and Resolution

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmphasisRole {
    None,
    Focus,
    Linked,
}

pub struct EmphasisState {
    focus: Option<IntervalId>,
    linked: Vec<IntervalId>, // resolved one-hop neighbors; excludes focus
}

impl EmphasisState {
    pub fn focus(&self) -> Option<IntervalId>;
    /// Emphasis role of an id for the current frame.
    pub fn role(&self, id: IntervalId) -> EmphasisRole;
}
```

On the substrate (additive — `begin_frame` unchanged):

```rust
/// The emphasis state, parallel to `selection()`. Valid after resolve_emphasis.
pub fn emphasis(&self) -> &EmphasisState;

/// Set the hovered focus. Cheap, pointer-derived — the emphasis peer of
/// `set_cursor`. Resolution is deferred (INV-342-E4).
pub fn set_emphasis_focus(&mut self, focus: Option<IntervalId>);

/// Resolve the linked set for the current focus under `relation`, against the
/// current visible range. `containment` supplies the Containment relation's
/// data; `None` (or a flat source) degrades Containment to Identity
/// (INV-342-E7). Recomputes only when focus or relation changed since the last
/// resolve (INV-342-E4); one hop only (INV-342-E3).
pub fn resolve_emphasis(
    &mut self,
    relation: EmphasisRelation,
    containment: Option<&dyn ContainmentSource>,
);
```

The consumer sets the focus on hover-change and calls `resolve_emphasis` each frame; the call early-returns when nothing changed. A SpanTree passes its source's containment handle (`RootedContainmentSource::containment`); a flat Gantt passes `None` and gets Identity.

# 5. Invariants

**INV-342-E1 (Read-Only):** emphasis is view state derived from a focus id; it never mutates the source or the selection.

**INV-342-E2 (Identity-Keyed):** focus and linked set are `IntervalId`s (INV-335-ID1); never positional indices (SPEC-PM-001).

**INV-342-E3 (One-Hop):** the linked set is bounded to one hop — for Containment, the direct parent plus direct children, nothing transitive, ever. This is both the performance bound and the analytics boundary (projection-only law).

**INV-342-E4 (Resolve-on-Change):** the linked set is recomputed only when focus or relation changed since the last resolve, and cached against them. Per-frame recomputation is forbidden (FORBID-342-2).

**INV-342-E5 (Independent Composition):** emphasis and selection are independent — an id may be selected, emphasized, both, or neither. Emphasis neither reads nor writes selection.

**INV-342-E6 (Off-Screen Tolerance):** the linked set may contain ids absent from the frame snapshot — a containment child one level down, a parent that is the frame. Consumers render only present ones and never fabricate geometry for absent ones; the projection's existing structural affordances (breadcrumb, enterable mark) convey absent links.

**INV-342-E7 (Relation Degradation):** Containment requires a bound `ContainmentSource`; absent one, resolution degrades to Identity (empty linked set), never an error.

**INV-342-E8 (Frame-Driven):** setting focus or resolving never forces a frame (SPEC-HAL-002 data-flow law, INV-335-D3).

# 6. Forbidden

**FORBID-342-1:** transitive or multi-hop linked sets — a subtree, a dependency chain. The performance killer and the analytics boundary (INV-342-E3).

**FORBID-342-2:** per-frame recomputation of the linked set; it is cached and recomputed only on focus/relation change (INV-342-E4).

**FORBID-342-3:** emphasis mutating selection or the source (INV-342-E1/E5).

**FORBID-342-4:** the Dependency relation in v1. Reserved; it requires an off-screen-link affordance and an explicit one-hop boundary designed as their own unit first.

**FORBID-342-5:** emphasis rendering rules in this SPEC. How a Focus or Linked bar looks is per-projection (SPEC-HAL-335 owns no rendering).

**FORBID-342-6:** fabricating geometry for an off-screen linked id (INV-342-E6).

**FORBID-342-7:** a containment field on the substrate or on `IntervalDescriptor` to serve emphasis — Containment data arrives through the `ContainmentSource` parameter, preserving FORBID-340-1 and INV-340-C4.

# 7. Relationship

Parallels `OrderedSelectionState` (SPEC-HAL-335 §3, INV-335-S1): same id-keyed shape, same substrate placement, a peer accessor; transient where selection is persistent, and pruned implicitly because it is recomputed each resolve rather than retained across `SetReplaced` (unlike selection, which is pruned by INV-335-S2).

Bound tree↔canvas — the originating case — is Identity. The composition sets the focus from either panel's hover and reflects it in both, through the same row↔interval correspondence bound-mode selection already relies on. That wiring is a consumer concern, not part of this state.

SpanTree (SPEC-HAL-341) is Containment. The on-screen payload is projection-dependent: rich in the multilevel Gantt, where a span's parent summary bar and its child task bars are co-rendered; thin in the single-level SpanTree, where the parent is the frame (the breadcrumb) and the children are one level down (the enterable mark). Identity carries the cross-view case there.

Dependency is a later unit — the same resolver plus the off-screen edge indicator and the explicit one-hop boundary.

Multi-view sync (an overview or minimap highlighting with the main canvas) is enabled by the shared placement — both views query `emphasis()` against one substrate — but the overview widget itself is a separate feature.

# 8. Non-Goals

No rendering, no Dependency relation, no transitive analytics, no new widget, no snapshot change, no multi-view widget. The state and its resolver only. No code authorized until tasks are dispatched.
