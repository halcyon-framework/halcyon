# SPEC-HAL-142A: Visual vs Interaction State — Enforcement and Invariants

**Status:** Binding
**Layer:** Interaction Engine / State Model
**Scope:** Mechanical enforcement of visual vs interaction state separation
**Depends on:** SPEC-HAL-142, SPEC-HAL-038, SPEC-HAL-079, SPEC-HAL-100
**Constrains:** Engine-level invariants only (no widget policy invention)

---

## SECTION A — Purpose

SPEC-HAL-142 defines the **principle**:
visual state and interaction state are conceptually distinct.

This SPEC defines the **enforcement**:
how that separation is made **mechanically unavoidable** inside the engine.

This document introduces **hard invariants, forbidden couplings, and validation rules**.
Any implementation that violates these rules is **architecturally illegal**, even if behavior “appears correct”.

---

## SECTION B — Definitions

**Visual State**
State used exclusively for rendering and appearance.

Examples:

* hovered
* pressed (visual)
* selected (visual highlight)
* focused ring
* disabled tint

**Interaction State**
State that governs authority, routing, and mutation.

Examples:

* focus target
* navigation cursor
* capture ownership
* active drag
* active transaction
* selection anchor

---

## SECTION C — Core Enforcement Rule

**INV-VIS-INT-SEP-1**
Visual state MUST NOT be read by interaction logic.

**INV-VIS-INT-SEP-2**
Interaction state MUST NOT be inferred from visual state.

No engine subsystem may branch, route, capture, mutate, or commit based on visual flags.

---

## SECTION D — One-Way Projection Law

Visual state is a **projection** of interaction state, never the inverse.

**INV-PROJECTION-ONLY**
Visual state may depend on interaction state.
Interaction state MUST NOT depend on visual state.

This is a strict one-way dependency graph:

```
Interaction State ───▶ Visual State
        ▲                  ✖
        └──────────────────┘  FORBIDDEN
```

---

## SECTION E — Structural Separation Requirements

### E.1 Storage Separation

**INV-STATE-STORAGE-1**
Visual state and interaction state MUST NOT be stored in the same struct.

**INV-STATE-STORAGE-2**
Visual-only flags MUST NOT appear in reducers or interaction pipelines.

### E.2 Update Separation

**INV-STATE-UPDATE-1**
Interaction reducers MUST NOT mutate visual state.

**INV-STATE-UPDATE-2**
Visual recomputation MUST be pure and side-effect free.

---

## SECTION F — Forbidden Couplings (Hard Errors)

The following patterns are categorically forbidden:

* using `hovered` to decide click, scroll, or drag ownership
* using `pressed` to infer activation authority
* using visual `selected` to infer selection membership
* using focus ring visibility to infer keyboard focus
* mutating interaction state inside paint or layout passes

Any occurrence is a spec violation.

---

## SECTION G — Selection-Specific Enforcement

**INV-SELECTION-VISUAL-DECOUPLE-1**
Visual selection highlight MUST NOT be the source of truth for selection membership.

**INV-SELECTION-INTERACT-ONLY**
Selection membership MUST be derived exclusively from interaction state.

---

## SECTION H — Focus and Hover Enforcement

**INV-HOVER-NO-AUTHORITY**
Hover state MUST NOT grant interaction authority.

**INV-FOCUS-VISUAL-INDEPENDENT**
Focus visualization MUST NOT imply focus ownership.

---

## SECTION I — Debug and Validation Requirements

Engines MUST provide debug-time validation for:

* visual state read inside reducers
* interaction mutation inside render paths
* mixed visual/interaction storage

Violations SHOULD panic or assert in debug builds.

---

## SECTION J — Interaction with Existing SPECs

* SPEC-HAL-142 defines the conceptual separation
* SPEC-HAL-038 defines focus routing
* SPEC-HAL-079 defines unified event flow
* SPEC-HAL-100 defines interaction state machines

SPEC-HAL-142A introduces **no new behavior**, only enforcement.

---

## SECTION K — Acceptance Criteria

SPEC-HAL-142A is satisfied when:

* [ ] Visual state cannot influence reducers
* [ ] Interaction state never depends on visuals
* [ ] Storage is physically separated
* [ ] Debug validation detects violations
* [ ] Removing all visuals does not break interaction logic

---

## SECTION L — Authoritative Statements

**AUTH-142A-1:** “Visuals are a projection, not a source of truth.”
**AUTH-142A-2:** “Interaction state must survive headless execution.”
**AUTH-142A-3:** “If visuals disappear, behavior must remain correct.”
