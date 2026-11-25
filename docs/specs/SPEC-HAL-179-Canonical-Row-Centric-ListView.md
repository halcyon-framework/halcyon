# SPEC-HAL-179: Canonical Row-Centric ListView

**Status:** Draft (Foundational)
**Layer:** Core Widget / Interaction Component
**Scope:** ListView identity, responsibility, and boundaries
**Supersedes:** None (first lawful definition of ListView in Halcyon)
**Parent:** SPEC-HAL-164 (TabularSubstrate — semantic engine authority)
**Related:** SPEC-HAL-164–178 (TabularSubstrate series — all semantic authorities)
**Non-Goals:** Mechanics, APIs, rendering contracts, TreeView, Grid, editing lifecycle

**Amended by:** SPEC-HAL-348, which admits an absent presentation header for redundant single-column and dense surfaces while preserving the closed composite and all owner-authored body geometry.
**Amended by:** SPEC-HAL-353, which assigns sort meaning, execution, applied state, and provider order to the data layer
while retaining substrate authority over mechanics on the confirmed projection.

---

## 1. Purpose

This SPEC defines ListView as a complete, canonical interaction component in Halcyon.

ListView is not:

* a thin wrapper,
* a partial view,
* a demo scaffold,
* a compatibility layer,
* or a substrate validation tool.

ListView is a first-class component intended to serve as the foundation for:

* tabular lists,
* multi-column lists,
* tree views,
* property editors,
* data inspection surfaces,
* and other row-oriented interaction structures.

This SPEC establishes identity, responsibility, and boundaries only.
Mechanics, APIs, and rendering contracts are defined in subsequent SPECs or amendments.

---

## 2. Component Identity

### 2.1 What ListView Is

ListView is a row-centric interaction surface that presents an ordered set of rows, each row being projected into one or more columns.

ListView:

* consumes provider-authoritative bounded RowId projections through DataAdapter and TabularSubstrate (the data layer owns row-universe and order semantics; the substrate owns mechanics over the confirmed projection per SPEC-HAL-188/353)
* provides column configuration inputs (min/max/preferred/flex/visibility) to TabularSubstrate (substrate owns Column Model and resolved geometry per SPEC-HAL-165/168)
* exposes selection at component boundary (selection semantics are owned by TabularSubstrate per SPEC-HAL-178)
* maps gestures and input events to semantic intents; sort operations cross DataAdapter to the data layer under SPEC-HAL-353
* coordinates horizontal and vertical scrolling
* defines visual presentation and styling

ListView is the unit of composition over TabularSubstrate, not a parallel authority.

---

### 2.2 What ListView Is Not

ListView does not:

* own business data
* own permission logic
* evaluate access policy
* own storage, caching, or persistence
* infer or derive domain meaning
* mutate external state directly

ListView is a composition layer over TabularSubstrate, not a parallel semantic authority.

---

## 3. Mandatory Structural Characteristics

### 3.1 Row-Centric Model (Non-Negotiable)

* Rows are the primary unit of identity.
* Every interaction targets a RowId, never an index.
* Columns are projections over rows, not independent lists.
* Cells do not own identity.

There is no cell-centric ListView variant.

---

### 3.2 Column Projection Model

* Columns define how a row is projected, not what a row is.
* Column count is independent of row count.
* Columns may be reordered, resized, hidden, or sorted without changing row identity.
* Column interaction emits intent, never performs mutation.

---

### 3.3 Header and Footer Semantics

* Header is present by default and may be configured absent under SPEC-HAL-348.
* Footer is optional.
* Header is part of ListView's canonical structure.
* Header does not scroll vertically with the body.
* Header participates in horizontal scrolling alignment.

Header interaction (sorting, resizing, affordances) is implemented via intent mapping in ListView. TabularSubstrate owns
column/interaction mechanics; the data layer owns sort meaning, execution, and confirmed order under SPEC-HAL-188/353.

---

## 4. Ownership Boundaries

### 4.1 ListView Responsibilities (Non-Authority)

ListView is responsible for:

* Structural composition (header + body [+ footer]) over TabularSubstrate
* Wiring DataAdapter-projected RowIds and column configuration into the substrate without taking data/order authority
* Mapping raw input (gestures, keyboard) to semantic intents and routing sort operations through DataAdapter
* Exposing selection state at component boundary (substrate-owned, ListView-exposed)
* Scroll coordination (horizontal + vertical)
* Visual presentation and styling

---

### 4.2 ListView Does Not Own

* Data fetching
* Data mutation
* Permission evaluation
* Access policy
* Validation logic
* Business rules

Those are provided externally via a presentation / provider layer.

---

### 4.3 Semantic Authority Boundary

ListView MUST NOT duplicate or shadow TabularSubstrate mechanics authority or data-layer semantic authority.

ListView MUST NOT maintain parallel state for any substrate-owned mechanic or data-layer-owned semantic.

TabularSubstrate (SPEC-HAL-164–178) is the authoritative mechanics engine over the confirmed provider projection. It owns:

* Row navigation, focus, selection, virtualization, and projection mechanics by RowId
* Column Model and resolved horizontal geometry (SPEC-HAL-165, SPEC-HAL-168)
* Selection semantics (SPEC-HAL-178)
* Focus and navigation semantics (SPEC-HAL-175)
* Edit arbitration (SPEC-HAL-174)
* Non-sort intent execution (SPEC-HAL-177)

For every SPEC-HAL-188-backed consumer, the data layer owns row-universe identity testimony, baseline and applied order,
sort capabilities, sort execution, and order revision (SPEC-HAL-353). ListView and TabularSubstrate MUST NOT duplicate or
shadow that authority.

ListView provides policy, intent mapping, and composition above the substrate. These are responsibilities, not authorities.

---

## 5. Permission and Access Model (Foundational Constraint)

* ListView never evaluates permissions.
* Rows that are not accessible do not exist from ListView's perspective.
* Rows that are visible but restricted are marked as such by the model.
* Capability gating is preferred over structural removal when layout stability matters.
* Permission affects forking of the presentation model, not live mutation.

This is a hard boundary.

---

## 6. Stability Guarantees

ListView guarantees:

* stable layout under permission changes
* stable identity under sorting, resizing, and virtualization
* no hide-and-seek UI behavior
* no late structural mutation

---

## 7. Non-Goals (Explicit)

This SPEC does not define:

* TreeView behavior
* Grid / spreadsheet semantics
* Editing lifecycle
* Column grouping
* Performance optimizations
* Substrate APIs

Those are explicitly deferred.

---

## 8. Minimal Demo Objective (Constraint Anchor)

A minimal lawful demo for this SPEC MUST demonstrate:

* A ListView with:
    * default visible header
    * multiple columns
    * fixed row height
    * stable row identity
* Row selection via RowId
* Column resizing via header interaction
* Column sorting intent emission
* Horizontal + vertical scrolling
* No permission logic in UI
* No data mutation in ListView

Anything outside this scope is forbidden in the demo.
