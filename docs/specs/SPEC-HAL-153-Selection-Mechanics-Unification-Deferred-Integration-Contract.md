# SPEC-HAL-153 — Selection Mechanics Unification: Deferred Integration Contract

**Status:** Draft (Binding upon acceptance)  
**Layer:** Interaction Model / Selection Semantics  
**Scope:** List, ListView, TreeView, and all selection-capable collection widgets  
**Depends on:** SPEC-HAL-118*, SPEC-HAL-038, SPEC-HAL-033, SPEC-HAL-128, SPEC-HAL-140  
**Blocks:** Any attempt to refactor selection mechanics before interaction surfaces are finalized

---

## SECTION A — Problem Statement

Selection mechanics in Halcyon currently exhibit **systemic instability**:

- Different behavior across:
    - non-transactional selection
    - transactional selection
    - ListView-integrated selection
- Shift-range selection breaks or resets state depending on context
- Drag-based selection interacts inconsistently with prior selection state
- Demos “work” only because of ad-hoc configuration

This instability is **not** caused by local bugs.

It is caused by **premature semantic unification** attempted before the interaction surface is complete.

Selection semantics are being implemented while:
- hit-zones are incomplete
- drag vs click arbitration is not finalized
- resize / scroll / select intent boundaries are undefined

This SPEC establishes a **hard sequencing rule** to prevent repeated refactors and cascading breakage.

---

## SECTION B — Root Cause Analysis

Selection currently leaks across layers:

1. **List**  
   Owns selection state and core mechanics

2. **ListView / Consumers**  
   Implicitly assume selection behavior (shift, ctrl, drag)

3. **Interaction Layer**  
   Lacks finalized intent routing (zones, capture, arbitration)

Because interaction surfaces are unfinished, selection logic is forced to guess:
- what a click *means*
- when a drag is a range-select vs scroll vs resize
- when prior selection should be preserved

Any attempt to “fix” selection now will be invalidated once hit-zones and routing are completed.

---

## SECTION C — Architectural Law (Binding)

### LAW-SEL-SEQ-1 — Sequencing Law

> **Selection mechanics MUST NOT be unified or refactored until all interaction surfaces are finalized.**

This includes, but is not limited to:
- column hit-zones
- header interaction routing
- resize vs select arbitration
- horizontal scroll integration

---

### LAW-SEL-OWN-1 — Ownership Law (Reaffirmation)

- **List** is the sole owner of selection state
- **ListView** must not reinterpret or augment selection semantics
- Consumers may configure selection mode, but not behavior

---

### LAW-SEL-BLACKBOX-1 — Black Box Rule

Until unification:
- Selection is treated as a **black box**
- Widgets may:
    - forward events
    - assert configuration invariants (debug-only)
- Widgets must **not**:
    - implement shift/ctrl logic
    - compute selection ranges
    - patch selection behavior for demos

---

## SECTION D — Explicit Deferral Contract

Selection unification is **explicitly deferred** until the following SPECs are fully implemented and frozen:

### Required Preconditions

1. **SPEC-HAL-118C / 118I / 118O**  
   Column properties (alignment, padding, visibility)

2. **SPEC-HAL-118J**  
   Column separators

3. **SPEC-HAL-118D / 118E**  
   Horizontal extent + scroll wiring

4. **SPEC-HAL-118L / 118M**  
   Hit-zones and header interaction routing

Only after these are complete may selection mechanics be revisited.

---

## SECTION E — What Is Allowed Before Unification

Before selection unification, the following are allowed:

- Enforcing **default configuration invariants** (e.g. transactional multi-select)
- Debug assertions validating selection mode
- Demo configuration fixes that **do not alter semantics**
- Documentation of known limitations

---

## SECTION F — What Is Forbidden Before Unification

The following are **architecturally illegal** until this SPEC is satisfied:

- Refactoring selection state machines
- “Fixing” shift-range behavior
- Special-casing ListView selection
- Adding new selection modes
- Making selection behavior depend on incomplete hit-zones

Violations will result in repeated regressions and are explicitly disallowed.

---

## SECTION G — Exit Criteria (When This SPEC Is Satisfied)

This SPEC is considered satisfied when:

- All required SPECs in Section D are implemented and frozen
- Interaction surfaces are stable
- A new SPEC is introduced that:
    - defines unified selection semantics
    - specifies intent → selection mapping
    - replaces legacy non-transactional paths

Only then may selection unification proceed.

---

## SECTION H — Rationale (Non-Normative)

Selection is a **semantic layer** built on top of:
- geometry
- hit-testing
- intent routing

Attempting to stabilize it before its foundations are complete guarantees failure.

This SPEC exists to prevent:
- repeated refactors
- demo-driven hacks
- erosion of architectural trust

---

**End of SPEC-HAL-153**
