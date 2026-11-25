# SPEC-HAL-340: **Temporal Containment**

### *Parent/Child Structure over the Temporal Substrate*

**Author:** JC
**Status:** Design — Binding upon adoption (implementation gated on task authorization; no code authorized by this document)
**Layer:** Substrate (Structural Engine)
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-334 (Time Model)
* SPEC-HAL-335 (TemporalSubstrate Architecture)
* SPEC-HAL-339 (Temporal Containment and the SpanTree Projection) — resolves §7 Q1

---

# 1. Purpose

Define containment over the temporal substrate: a parent/child relation on identity-keyed intervals, with an enforced range-nesting discipline, consumed by level-of-detail / drill-in projections (SPEC-HAL-341).

This is substrate-level and reusable. It is the structural peer of `IntervalSource`: where `IntervalSource` answers "what intervals intersect this range," `ContainmentSource` answers "what are the direct children of this node." A future Gantt rollup consumes the same `⊆` relation; it is not SpanTree-specific.

This SPEC defines a relation and its contract. No widget, no navigation, no rendering — those are SPEC-HAL-341.

# 2. Position

Purely additive to SPEC-HAL-335. `IntervalDescriptor`, `IntervalSource`, `TemporalDescriptors`, and every existing invariant are unchanged. Containment is a **separate source surface** — a flat-interval projection (a Gantt body) remains containment-free and pays nothing.

Resolves SPEC-HAL-339 §7 Q1: containment is an **enforced substrate relation**, not emergent topology.

# 3. ContainmentPolicy

Per the mandatory-policy discipline (SPEC-HAL-337 §2: every structural subsystem declares its policy before implementation):

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContainmentPolicy {
    /// child.range ⊆ parent.range is a substrate invariant, validated
    /// (INV-340-C2/C3). The drill-in projection (SPEC-HAL-341) requires this.
    Enforced,
    /// Topology asserts parent/child; ranges are unconstrained. Specified
    /// for a future schedule-shaped consumer; not realized by v1.
    Emergent,
}
```

The implementing source declares its policy via `ContainmentSource::policy()`.

# 4. Data Contract — ContainmentSource

```rust
pub trait ContainmentSource {
    fn policy(&self) -> ContainmentPolicy;

    /// Append the DIRECT children of `parent` (None = top-level roots)
    /// intersecting `range`. Exactly one level — never transitive
    /// descendants. Over-returning within the level is permitted (the
    /// projection culls precisely, per INV-335-D2); returning a deeper
    /// descendant is a contract breach (FORBID-340-2).
    fn children_of(&self, parent: Option<IntervalId>, range: TimeRange,
                   out: &mut TemporalDescriptors);

    /// Parent of `id`, for ancestry / root-path resolution. None at a root.
    fn parent_of(&self, id: IntervalId) -> Option<IntervalId>;

    /// Does `id` have at least one child? Drives the "enterable" affordance
    /// in SPEC-HAL-341. Cheap: one level only.
    fn has_children(&self, id: IntervalId) -> bool;

    /// Full descriptor for `id`, regardless of visibility — the projection
    /// needs the ViewRoot's range to set the frame and ancestors' labels
    /// for the breadcrumb, neither of which the culled snapshot can answer.
    fn interval(&self, id: IntervalId) -> Option<IntervalDescriptor>;

    fn drain_invalidation_events(&mut self) -> Vec<TemporalInvalidationEvent>;
}
```

`ContainmentSource` is **independent of `IntervalSource`** — it does not extend it. A source may implement both, but a containment-bearing source is not forced to implement the flat range+rows `collect`, and a flat source is not forced to implement containment.

# 5. Invariants

**INV-340-C1 (Single-Level Enumeration):** `children_of` returns exactly the direct children of `parent` — never transitive descendants, never the subtree. Bounded realization (SPEC-HAL-341 INV-341-V2) rests entirely on this.

**INV-340-C2 (Enforced Nesting):** under `ContainmentPolicy::Enforced`, every interval whose parent is `P` satisfies `range ⊆ P.range` — `range.start >= P.range.start` and `range.end <= P.range.end`, half-open per SPEC-HAL-334 INV-334-3.

**INV-340-C3 (Validation):** under `Enforced`, the substrate validates C2 against each child returned by `children_of`. A violator is a contract breach: `debug_assert!` in dev, dropped from the frame in release. It is **never silently clamped** — clamping would fabricate data the source did not state (SPEC-PM-009 spirit).

**INV-340-C4 (Source Purity):** `ContainmentSource` holds no pixel knowledge; `TimeRange` and `IntervalId` are the only query vocabulary that crosses the trait (extends INV-335-D1).

**INV-340-C5 (Acyclic Forest):** the parent relation is a forest — no cycles. Every `parent_of` chain terminates at a root (`None`). The substrate may `debug_assert!` termination.

**INV-340-C6 (Identity):** containment is keyed on `IntervalId` (INV-335-ID1). A parent reference to an id absent from the source resolves as no parent / no child — never positionally (SPEC-PM-001).

# 6. Forbidden

**FORBID-340-1:** a containment field on `IntervalDescriptor` or a containment method on `IntervalSource`. The flat interval model stays flat; containment lives only on `ContainmentSource`. This is what keeps Gantt projections containment-free.

**FORBID-340-2:** `children_of` returning transitive descendants. Violates INV-340-C1 and the bounded-realization contract of SPEC-HAL-341.

**FORBID-340-3:** silent range clamping of a child that violates C2 under `Enforced` (INV-340-C3).

**FORBID-340-4:** substrate-side caching of the containment tree that survives a `SetReplaced` invalidation. Frame-driven (SPEC-HAL-002): the source is the truth; the substrate re-queries.

# 7. Relationship to SPEC-HAL-339

339 §7 Q1 is answered: **Enforced**. The `Emergent` branch is specified-but-deferred for a future schedule-shaped consumer whose spans carry topology without strict range nesting. SPEC-HAL-341 binds only `Enforced` sources and rejects `Emergent` at bind time (INV-341-V7).

# 8. Non-Goals

No widget, no navigation, no rendering, no rollup or summarization semantics. A future Gantt rollup consumes C2 but defines its own envelope-rendering rules in its own SPEC. This document defines the relation and nothing that draws it.
