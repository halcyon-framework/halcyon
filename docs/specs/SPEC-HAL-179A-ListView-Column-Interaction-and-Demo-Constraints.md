# SPEC-HAL-179A: Column Interaction, Resizing, Sorting, and Demo Constraints

**Status:** Draft (Amendment)
**Layer:** Core Widget / Interaction Component
**Scope:** Column interaction intent mapping and demo constraints
**Amends:** SPEC-HAL-179 (Canonical ListView Component)
**Amended by:** SPEC-HAL-353 (provider-authoritative sorting and ordered projection)
**Parent:** SPEC-HAL-164 (TabularSubstrate — semantic engine authority)
**Non-Goals:** TreeView, editing lifecycle, column grouping, permission/provider

---

## 1. Motivation

SPEC-HAL-179 deliberately established identity and boundaries but left two risks unresolved:

1. Column interactions (resize, sort) could bypass intent mapping and mutate state directly.
2. Demo implementations could drift into partial, misleading, or non-representative behavior.

This amendment locks column interaction intent mapping and demo constraints to prevent repeat failure.

---

## 2. Column Interaction Intent Mapping

### 2.1 Intent Mapping Responsibility

ListView is responsible for mapping raw gestures and input events to column interaction intents.

ListView maps input to:

* column resize intent
* column sort intent
* column reorder intent (future)
* column visibility intent (future)

This is policy and gesture decoding, not semantic authority.

TabularSubstrate (SPEC-HAL-164–178) owns:

* Column Model and resolved horizontal geometry (SPEC-HAL-165, SPEC-HAL-168)
* navigation, focus, selection, and virtualization mechanics over the confirmed provider projection
* non-sort intent execution and deferral (SPEC-HAL-177)

For sorting, ListView emits semantic operations through DataAdapter; the data layer owns meaning, validation, execution,
and confirmed order under SPEC-HAL-353. The substrate consumes that order without recomputing it.

---

## 3. Column Resizing Semantics

### 3.1 Resize Model

* Column resizing is header-originated only
* Resize lifecycle is Begin/Update/Commit/Cancel per SPEC-HAL-169
* Resize interaction must acquire capture and suppress scroll per SPEC-HAL-169
* Resize interaction emits:
    * `ColumnResizeIntent { column_id, delta_px }`
* Resize intent does not mutate column widths directly

### 3.2 Width Resolution

* ListView owns preferred width inputs and persistence
* TabularSubstrate owns resolved column geometry (x_offset, width) via SPEC-HAL-165/168
* Geometry table is authoritative for hit-testing and rendering
* ListView may apply clamp/quantize only when committing a structural width preference update, not during pointer move dispatch, and never by reading/writing live geometry inside the interaction loop

### 3.3 Fixed Guarantees

Column resizing must not:

* reassign row identity
* invalidate selection
* reorder rows
* cause vertical layout shift

---

## 4. Column Sorting Semantics

### 4.1 Sorting Is Intent-Only

* Header interaction may emit semantic sort operations governed by SPEC-HAL-353
* ListView never sorts data
* ListView never mutates row order directly
* ListView renders only provider-confirmed applied sort state

### 4.2 Sorting Flow (Canonical)

Exactly one lawful fulfillment mode exists for ListView and every SPEC-HAL-188-backed consumer:

1. User or host emits a semantic sort operation.
2. ListView forwards it through the DataAdapter request boundary.
3. The data layer accepts, rejects, reports pending, or reports unsupported.
4. On acceptance, the data layer publishes the confirmed applied `SortSpec` and a new order revision.
5. The adapter invalidates prior revision windows and the substrate bumps its base-order epoch exactly once.
6. The substrate consumes bounded RowId projection windows for navigation, selection, and virtualization.
7. ListView renders the provider-confirmed direction and multi-sort priority.

Comparator-driven ListView reorder is forbidden. Existing `RowComparator` machinery is not a ListView fulfillment mode
and must not be wired into a SPEC-HAL-188-backed consumer. Multi-column interaction, baseline reset, cancellation,
capability policy, custom sorting, large-data order windows, and persistence are governed by SPEC-HAL-353.

---

## 5. Horizontal Scrolling Contract

### 5.1 Unified Horizontal Axis

* Header and body must share a single horizontal scroll state
* Horizontal scroll state may be owned at ListView composition boundary
* TabularSubstrate MUST be the single consumer/propagator of the offset across Header/Body domains, as a post-resolution transform (SPEC-HAL-168)

### 5.2 Forbidden Patterns

* Independent header/body horizontal scroll
* Geometry inference inside header widgets
* ListView distributing scroll offset directly to domains (must go through TabularSubstrate)

---

## 6. Demo Objective (Binding Constraint)

This amendment defines a mandatory demo objective for SPEC-HAL-179 compliance.

### 6.1 Demo Must Demonstrate

The demo must include:

* One ListView instance
* Mandatory header
* >= 3 columns
* Fixed row height
* >= 100 rows (virtualized)
* Vertical scrolling
* Horizontal scrolling
* Column resizing via header
* Column sorting intent emission
* Row selection by RowId
* External data source (mock acceptable)

### 6.2 Demo Must NOT Include

* Data mutation inside ListView
* Permission logic
* Conditional widget creation
* Cell-level identity
* Editing lifecycle
* Tree behavior
* Grid semantics

If any of the above appear, the demo is invalid.

---

## 7. Explicit Anti-Regression Rules

The following are explicitly forbidden and constitute regression:

* Treating columns as independent lists
* Treating cells as identity carriers
* ListView applying sorting directly (must emit a semantic request through DataAdapter to the data layer)
* ListView mutating column geometry directly (must provide configuration inputs to TabularSubstrate)
* ListView maintaining parallel column geometry or selection state
* Header implemented as a separate widget tree with independent geometry authority
* Header scrolling vertically with rows

---

## 8. Relationship to Other SPECs

This amendment is subordinate to SPEC-HAL-164 (TabularSubstrate semantic engine authority).

This amendment intentionally leaves space for:

* TreeView projection SPEC
* Editing lifecycle SPEC
* Column grouping SPEC
* Permission/provider SPEC

Intent mapping rules defined here must not contradict TabularSubstrate mechanics authority (SPEC-HAL-164–178) or the
data-layer sort authority established by SPEC-HAL-188 and SPEC-HAL-353.

---

## 9. Amendment Summary (Non-Negotiable)

* Columns are projections, not containers
* ListView maps gestures to intents; the data layer owns sort semantics and the substrate owns projection mechanics
* Sorting and resizing are intent-only from ListView's perspective
* TabularSubstrate owns geometry and mechanics over the confirmed projection; the data layer owns sort execution and applied order
* Demo scope is now locked and enforceable
