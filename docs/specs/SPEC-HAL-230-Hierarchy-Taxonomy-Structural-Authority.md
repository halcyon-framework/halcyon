# SPEC-HAL-230: Hierarchy / Taxonomy Structural Authority

**Subtitle:** Logical Structural Model (Non-Interactive, Policy-Free)
**Status:** Draft (Binding upon adoption)
**Layer:** Application Semantics / Structural Authority
**Audience:** Core Architecture, Data Model, Widgets (Consumers Only)
**Depends On:** SPEC-HAL-002 (Constitution), SPEC-HAL-001 (Core Architecture)
**Explicit Non-Dependencies:** NavigationHost, Breadcrumbs, Selection models, Focus, Permissions, UI widgets

---

## 0. Purpose

This SPEC defines a Hierarchy / Taxonomy Authority whose sole responsibility is to own and validate logical structural relationships between entities.

This authority exists to answer exactly one class of questions:

"What is structurally related to what?"

It intentionally does not answer:
- where the user navigated,
- what is selected or focused,
- what the current location is,
- what should happen next,
- who is allowed to do what.

Those concerns belong to other systems.

---

## 1. Conceptual Model (Frozen)

### 1.1 Structure

A Hierarchy is a persistent, directed, acyclic structure describing parent/child or containment relationships between logical entities.

It is:
- atemporal (no "current"),
- non-interactive,
- non-navigational.

### 1.2 Taxonomy

A Taxonomy is a specialization of hierarchy that allows:
- multiple parents per node,
- semantic grouping or classification,
- non-exclusive membership.

Taxonomy does not change authority rules; it only affects allowed topology.

### 1.3 Structural Node

A StructuralNode is:
- an abstract identifier,
- stable across frames,
- not a widget,
- not a navigation target,
- not a UI object.

---

## 2. Authority Boundaries

### INV-HIER-OWNERSHIP-1: Sole Ownership

The Hierarchy Authority is the exclusive owner of all structural relationships.

No other system may:
- invent edges,
- infer structure,
- repair topology,
- reinterpret relationships.

### INV-HIER-NO-INTERACTION: No Interaction Semantics

Hierarchy Authority:
- processes no input events,
- tracks no user state,
- exposes no "active" or "current" node.

There is no location, no cursor, no viewpoint.

---

## 3. Data Model (Conceptual)

`HierarchyNodeId(u64)`

Each node may declare:
- zero or more parents,
- zero or more children,
- optional semantic ordering metadata (purely declarative).

Cycles are forbidden.

---

## 4. Invariants (Binding)

### INV-HIER-NO-NAV: No Navigation

Hierarchy MUST NOT:
- track traversal,
- track paths,
- emit NavigationIntent,
- derive breadcrumbs.

### INV-HIER-NO-HISTORY: No History

Hierarchy MUST NOT record:
- visit order,
- undo/redo,
- temporal state.

### INV-HIER-NO-SELECTION: No Selection or Focus

Hierarchy MUST NOT store:
- selection,
- focus,
- highlight,
- expansion state.

### INV-HIER-NO-INFERENCE: No Implicit Meaning

Hierarchy MUST NOT infer:
- canonical parents,
- preferred paths,
- defaults,
- "main" branches.

All meaning must be encoded explicitly by the application.

---

## 5. Mutation Model (CRUD)

Hierarchy Authority does support mutation, but only with explicit intent and zero policy.

### 5.1 Supported Operations

- add node
- remove node (with explicit strategy)
- add edge (parent → child)
- remove edge
- reorder children (semantic order only)

All operations are:
- explicit,
- atomic,
- validated,
- all-or-nothing.

---

## 6. Deletion Semantics (Critical)

There is no implicit "delete node".

### INV-HIER-DELETE-EXPLICIT: Deletion Strategy Required

All node deletion MUST specify a strategy.

#### Required Deletion Strategies

At minimum:
- **DeleteLeaf** — Fails if node has children.
- **DeleteCascade** — Deletes node and entire descendant subtree.
- **DeleteReparent** — Deletes node and reattaches children to an explicitly provided parent.
- **DeleteDetach** — Deletes node and leaves children orphaned but valid.

If no strategy is specified, the operation fails.

---

## 7. Validation Rules

### INV-HIER-VALIDATION: Structural Integrity

Hierarchy Authority MUST reject mutations that would result in:
- cycles,
- dangling references,
- missing nodes,
- duplicate edges,
- invalid reparenting.

Failures are hard and immediate. No silent repair. No fallback behavior.

---

## 8. Projection Contract

Hierarchy Authority exposes read-only structural snapshots.

Projections:
- consume hierarchy data,
- render or inspect it visually,
- may maintain local UI state (expanded/collapsed).

### 8.1 Projection Output Limits

Projections MAY emit:
- inspection signals,
- UI-local state changes.

Projections MUST NOT emit:
- NavigationIntent,
- SelectionIntent,
- Commit actions.

---

## 9. Relationship to Navigation

Hierarchy and Navigation are orthogonal systems.

### INV-HIER-NAV-ORTHOGONAL: No Coupling

- Navigation MAY reference hierarchy nodes as data.
- Hierarchy MUST NOT react to navigation.
- Breadcrumbs MUST NOT be derived from hierarchy.

Any coupling requires an explicit application-level bridge, never implicit logic.

---

## 10. Permissions (Explicitly Out of Scope)

Hierarchy Authority is policy-free.

### INV-HIER-NO-POLICY: No Access Control

It does not:
- check access rights,
- hide nodes,
- filter structure,
- encode roles.

Permissions belong to:
- application logic, or
- a separate Policy / Access Authority SPEC.

---

## 11. Explicit Non-Goals

This SPEC does NOT define:
- tree widgets,
- locators,
- path bars,
- breadcrumbs,
- wizards,
- editors,
- access control,
- navigation flows.

Each requires a separate SPEC.

---

## 12. Summary (Binding)

Hierarchy Authority is:
- just structure — and nothing else
- authoritative
- persistent
- atemporal
- mutation-safe
- policy-free
- interaction-agnostic

It exists so that structure stops leaking into navigation, UI, and intent.
