# SPEC-HAL-204: Search Intent & Projection Derivation Contract

**Status:** Draft (Foundational)
**Layer:** Data Semantics / Projection / Widget-Data Boundary
**Audience:** Widget authors, Data providers, Interaction layer
**Applies To:** ComboBox, Selector widgets, Search-driven widgets, Command palettes, Property editors with lookup fields, Any widget consuming search-based or lookup-based projections

---

## 0. Purpose

This SPEC formalizes search as a projection derivation problem, not a widget feature.

It defines a strict contract between:

- widgets that emit search intent, and
- data systems that produce projections in response.

The goal is to support large, dynamic, remote, or computed datasets without leaking scale, filtering logic, or data ownership into UI widgets.

---

## 1. Core Principle

Search-driven widgets never own, filter, or index data.

They operate exclusively on projections supplied by an external authority.

---

## 2. Definitions

### 2.1 Search Intent

A Search Intent is a semantic signal emitted by a widget, containing:

- current input text (possibly empty)
- optional cursor / selection context
- optional hint metadata (case sensitivity, prefix vs fuzzy, etc.)
- optional dimension constraints (max items, viewport intent)

A search intent never contains data and never implies filtering mechanics.

---

### 2.2 Projection

A Projection is a read-only, bounded view over a larger domain.

Properties:

- finite
- ordered
- indexable
- renderable
- discardable

A projection may represent:

- 10 items from a list of 1 million
- cached local results
- remote query results
- synthetic / computed suggestions
- an empty set

The widget does not know which.

---

## 3. Projection Derivation Law

Only the data layer may derive projections.

Widgets may request projections via search intent but must not:

- filter existing projections
- accumulate results
- assume stability
- assume completeness
- assume locality

Every projection is authoritative only for the current intent.

---

## 4. Dimensional Constraints

Widgets may express dimensional intent, never dimensional enforcement.

Allowed signals:

- preferred max visible items
- preferred min visible items
- viewport resize hints (e.g. "shrink to fit fewer matches")

The projection provider decides:

- how many items to return
- whether fewer than max is acceptable
- whether zero items is meaningful

Widgets must render exactly what they receive, nothing more.

---

## 5. Dynamic Resizing Semantics

When projections shrink or grow in response to search intent:

- widgets must adapt layout
- widgets must not preserve stale height
- widgets must not fabricate empty rows

Projection size drives overlay size — always.

---

## 6. Autocompletion (Optional, Non-Authoritative)

Autocompletion is a presentation hint, not a data operation.

Rules:

- widgets may display suggested completions
- widgets must not commit completions implicitly
- widgets must not assume completion correctness

Autocompletion never mutates intent, only UI state.

---

## 7. Dependency & Chaining Semantics

Widgets may be semantically dependent without direct coupling.

Example:

- Selector B depends on selection in Selector A

Rule:

Dependency is expressed via shared or derived search intent, not widget references.

No widget:

- observes another widget directly
- pulls state imperatively
- mutates another widget's projection

All coupling occurs through the data / intent layer.

---

## 8. Empty & Invalid States

A projection may legitimately be:

- empty
- loading
- partial
- stale

Widgets must:

- render empty projections without visual artifacts
- avoid forced expansion
- avoid placeholder rows unless explicitly provided

An empty projection is data, not an error.

---

## 9. Forbidden Patterns

Widgets must not:

- cache domain data
- implement filtering logic
- perform local search over projections
- infer domain size
- assume identity stability across projections
- reuse projections across intents

Violations of these rules reintroduce hidden scale coupling.

---

## 10. Relationship to Other SPECs

- SPEC-HAL-203: Defines Selector / ComboBox behavior as a concrete application
- SPEC-PM series: Will record historical failures prevented by this contract
- TabularSubstrate SPECs: Projection rendering substrate, not derivation

---

## 11. Summary (Non-Negotiable)

- Search is intent, not logic
- Results are projections, not datasets
- Widgets render, never decide
- Scale is externalized permanently

This SPEC establishes a single mental model for all future search-driven UI in Halcyon.
