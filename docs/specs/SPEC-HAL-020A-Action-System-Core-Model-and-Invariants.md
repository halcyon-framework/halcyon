# SPEC-HAL-020A: Action System Core Model and Invariants

**Status:** Design / Binding
**Layer:** Core interaction model (engine + widgets)
**Scope:** Action identity, definition, exposure, core state, invariants (NO dispatch, NO bindings, NO OS)
**Depends on:** SPEC-HAL-038 (focus/input routing baseline), SPEC-HAL-127/137 (overlay boundaries, where relevant)
**Constrains:** `src/core/*` action registry/resolution types, widget action exposure API surface

---

## SECTION A — Problem Statement

Halcyon needs a single, uniform “action” abstraction to represent intent that can be:

* exposed by any widget/screen/app
* queried without side effects
* resolved and executed by higher layers
* bound to inputs later (keyboard/mouse/etc.)
* used by command palette / menus / contextual hotkeys / accessibility tooling

This SPEC defines what an action *is*, how it is identified, and what invariants the entire system must obey.

This SPEC does **not** define:

* contextual resolution order
* dispatch routing
* key bindings / input mapping
* execution lifecycle / async / cancellation
* OS integration

Those belong to SPEC-HAL-020B/C/D.

---

## SECTION B — Definitions

### B.1 Action

An **Action** is a named intent with stable identity and optional parameters, exposed by a scope, resolvable to a concrete target, and executable under explicit rules.

### B.2 ActionId

A stable identifier for an action definition.

### B.3 ActionInstance

A concrete “invocation form” of an action with parameters (if any).

### B.4 ActionScope

A node in the exposure graph that can expose actions (Widget, Overlay, Screen, App).

### B.5 ActionProvider

A source that can enumerate actions for a scope without performing work.

### B.6 ActionState

A pure classification of whether an action is currently available (enabled/disabled/hidden) and why.

---

## SECTION C — Core Data Model

### C.1 Action Identity

**INV-ACT-ID-1**
Every action MUST have a stable `ActionId` that does not depend on runtime allocation, widget instance addresses, or ephemeral widget tree NodeIds.

**INV-ACT-ID-2**
`ActionId` MUST be stable across frames.

**INV-ACT-ID-3**
Two different semantic actions MUST NOT share the same `ActionId`.

**INV-ACT-ID-4**
The same semantic action exposed by multiple scopes MUST still use the same `ActionId` (identity is semantic, not per-instance).

Recommended representation (binding, but implementation-specific):

```rust
pub struct ActionId(pub u64); // stable hashed identifier of a canonical string
```

### C.2 Action Definition

A definition is static metadata:

* canonical name (string)
* optional human label (string)
* optional group/category (string)
* parameter schema (optional)
* optional default chord suggestions (non-binding hint only)

**INV-ACT-DEF-1**
Action definitions MUST be side-effect free and cheap to copy/clone.

**INV-ACT-DEF-2**
Action definitions MUST NOT capture closures to arbitrary runtime objects (no hidden allocations).

### C.3 ActionInstance (Parameterization)

**INV-ACT-PARAM-1**
If an action supports parameters, the parameter payload MUST be explicitly represented and serializable (no hidden closure capture).

**INV-ACT-PARAM-2**
Two ActionInstances with same `ActionId` and same parameter payload are semantically identical.

### C.4 Action State

Action state is pure availability:

```rust
pub enum ActionAvailability {
    Enabled,
    Disabled { reason: Option<String> },
    Hidden,
}
```

**INV-ACT-STATE-1**
Action availability MUST be computable without triggering UI tree rebuild, layout, paint, or side effects.

**INV-ACT-STATE-2**
If an action is `Hidden`, it MUST behave as nonexistent for discovery surfaces (palette/menus), unless a debugging surface explicitly requests hidden actions.

**INV-ACT-STATE-3**
If an action is `Disabled`, it MAY be listed, but execution MUST be rejected.

---

## SECTION D — Action Exposure Model

### D.1 Exposure Surfaces

Supported scopes (core model only; resolution rules in 020B):

* Widget scope (a widget instance)
* Overlay scope (top overlay or overlay stack root)
* Screen scope (current screen)
* App scope (global)

**INV-ACT-EXP-1**
Any scope MAY expose actions.

**INV-ACT-EXP-2**
Exposing actions MUST be observational only: enumeration MUST NOT mutate state.

**INV-ACT-EXP-3**
Action exposure MUST be incremental and cheap (no scanning entire widget tree unless explicitly requested by a higher layer).

### D.2 Provider Interface

**INV-ACT-PROV-1**
Providers MUST support enumerating action definitions and querying availability separately from execution.

**INV-ACT-PROV-2**
Providers MUST be usable without heap allocations in the hot path (zero-cost goal). SmallVec/stack buffers allowed; avoid `Vec` churn.

---

## SECTION E — Execution Separation

Execution exists, but is not defined here; only separation invariants are.

**INV-ACT-SEP-1**
Action enumeration MUST NOT execute actions.

**INV-ACT-SEP-2**
Action availability queries MUST NOT execute actions.

**INV-ACT-SEP-3**
Execution MUST be an explicit operation initiated by a dispatcher (defined in 020B/020D).

---

## SECTION F — Zero-Cost / Performance Invariants

**INV-ACT-PERF-1**
Enumerating actions for the currently focused scope MUST be O(k) where k is number of actions exposed by that scope, not the number of widgets in the tree.

**INV-ACT-PERF-2**
Action availability checks for focused scope MUST NOT trigger layout, paint, or widget tree rebuild.

**INV-ACT-PERF-3**
No allocation per keypress is allowed in the steady-state path of “resolve action availability” for the focused scope (small fixed scratch buffers allowed).

---

## SECTION G — Forbidden Behaviors

**FORBID-ACT-CAPTURE-CLOSURE**
Actions MUST NOT be represented as arbitrary closures capturing widget internals.

**FORBID-ACT-IMPLICIT-EXEC**
No action may execute as a side effect of enumeration or availability queries.

**FORBID-ACT-EPHEMERAL-ID**
Action identity MUST NOT be derived from pointer addresses, NodeIds, or frame-local counters.

---

## SECTION H — Minimal API Surface

Core types (binding; exact module placement may vary):

```rust
pub struct ActionId(pub u64);

pub struct ActionDef {
    pub id: ActionId,
    pub canonical_name: &'static str,
    pub label: Option<&'static str>,
    pub group: Option<&'static str>,
    // optional parameter schema omitted here (020C/020D may refine)
}

pub enum ActionAvailability {
    Enabled,
    Disabled { reason: Option<String> },
    Hidden,
}

pub struct ActionInstance<'a> {
    pub id: ActionId,
    pub params: Option<&'a dyn std::any::Any>, // placeholder; must become serializable if used (020C/020D)
}
```

**INV-ACT-API-1**
A widget MUST be able to expose its actions without requiring global registries.

**INV-ACT-API-2**
Global registries MAY exist for indexing/search, but MUST NOT be required for correctness of local behavior.

---

## SECTION I — Acceptance Criteria

SPEC-HAL-020A is satisfied when:

* [ ] Action identity is stable, semantic, and non-ephemeral.
* [ ] Action enumeration and availability are observational and side-effect free.
* [ ] Execution is cleanly separated (no accidental run during query).
* [ ] The model supports exposure from widget/overlay/screen/app scopes.
* [ ] Performance invariants are enforceable in hot paths.

---

## SECTION J — Out of Scope

Explicitly out of scope for 020A:

* contextual resolution ordering and priority (020B)
* key/chord mapping and binding tables (020C)
* execution, lifecycle, async, cancellations, scope teardown (020D)
* OS accelerator integration, windowing hooks (later SPEC)

