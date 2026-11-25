# SPEC-HAL-205: Projection Lifetime, Stability & Invalidation Semantics

**Status:** Draft (Foundational)
**Layer:** Data Semantics / Projection Lifecycle
**Audience:** Data providers, Widget authors, Interaction layer
**Applies To:** All widgets and systems consuming projections derived from search intent (SPEC-HAL-204)

---

## 0. Purpose

This SPEC defines how long projections live, when they are valid, and how they must be discarded.

It exists to prevent:

- stale UI state
- hidden caching
- false stability assumptions
- accidental state coupling across intent changes

This SPEC closes the search-projection model introduced in SPEC-HAL-204.

---

## 1. Core Law

Every projection is ephemeral and intent-scoped.

A projection is valid only for the exact intent that produced it and only until invalidated.

No projection is permanent.
No projection is authoritative beyond its scope.

---

## 2. Projection Identity

A projection has no stable identity.

Rules:

- Projections must not be compared by pointer, index, or equality
- Item identity inside a projection may be stable or unstable — the widget must not assume either
- Two projections may contain overlapping items without implying continuity

Widgets must treat each projection as a fresh snapshot.

---

## 3. Invalidation Triggers

A projection is invalidated when any of the following occurs:

1. Search intent changes (text, filters, constraints)
2. Dependency intent changes (chained selectors)
3. Data provider signals invalidation
4. Widget loses relevance (hidden, detached, destroyed)
5. Explicit provider replacement

Invalidation is absolute.
There is no partial reuse.

---

## 4. Widget Obligations on Invalidation

Upon invalidation, widgets must:

- drop all references to the old projection
- clear selection/highlight unless explicitly revalidated
- recompute layout based on the new projection
- avoid visual continuity assumptions

Widgets must not animate or interpolate across projections unless explicitly instructed.

---

## 5. Lifetime Ownership

Projection ownership rules:

- The provider owns creation and invalidation
- The widget owns rendering and disposal
- No projection outlives the widget that renders it

Widgets must never:

- retain projections for later reuse
- store projections in shared state
- leak projections across widget boundaries

---

## 6. Caching Rules (Strict)

Caching is allowed only at the data layer.

Rules:

- Widgets may not cache projections
- Providers may cache internally but must treat cache hits as new projections
- Cached projections must still obey invalidation rules

Caching must be semantically invisible to widgets.

---

## 7. Selection Continuity (Optional, Explicit)

Selection persistence across projections is not implicit.

If continuity is desired:

- it must be implemented by the data/provider layer
- it must be explicit (e.g. re-select by domain ID)
- widgets may accept or reject rehydrated selection

Default behavior: selection resets on projection change.

---

## 8. Empty, Loading, Transitional States

Projections may be:

- empty
- loading
- partial
- superseded before render

Widgets must:

- render empty projections as valid states
- tolerate projection replacement mid-interaction
- never block UI waiting for "final" results

There is no such thing as a "complete" projection.

---

## 9. Forbidden Assumptions

Widgets must not assume:

- monotonic growth or shrinkage
- stable ordering
- stable indices
- stable item identity
- synchronous delivery
- locality of data

Any such assumption re-introduces hidden scale coupling.

---

## 10. Termination Clause (Important)

Together, SPEC-HAL-204 + SPEC-HAL-205 fully define:

- what search is (intent)
- what results are (projections)
- how they are produced
- how they live
- how they die

After this point:

- no more generic "search architecture" SPECs are needed
- all future work is application-specific (ComboBox, Selector, CommandPalette, etc.)
- violations belong in the SPEC-PM (Pathology & Mistake Prevention) series, not new core SPECs

This SPEC ends the conceptual model.

---

## 11. One-Sentence Mental Model (Final)

A widget emits intent, receives a temporary view, renders it, and forgets it.

That's it.
