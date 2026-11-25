# SPEC-HAL-118Q — ListView Intent / Directive Semantic Lock

**Status:** Deprecated
**Series:** SPEC-HAL-118* (ListView)

> **DEPRECATED:** This SPEC assumes List as substrate with columnar behavior layered on top,
> which is architecturally invalid. The entire SPEC-HAL-117/118 series is deprecated.
> A new SPEC series with a correct root abstraction is required.
**Stage:** 11
**Applies to:** ListView column interaction pipeline
**Introduces new behavior:** No
**Locks existing behavior:** Yes

---

## Changelog

- 2026-01 — Marked implemented after ListView v1 completion

---

## 1. Purpose

This SPEC locks the Stage 11 semantics of the ListView column interaction pipeline as immutable ground truth for all SPEC-HAL-118* work.

## 2. Mandatory pipeline

Only legal flow:

Intent → ReorderDirective → StructureDirective → external controller

## 3. Authority separation

* ListView: intent + directive emission only
* Geometry: hit-testing only
* Controller: structure mutation only

## 4. Governing invariants

All enforcement is defined by the invariant block below. Any violation is an architectural error.

---

## PART A — STAGE 11 INVARIANT BLOCK (BINDING LAW)

**Scope:** ListView column interaction → intent → directive pipeline
**Applies to:** All stages ≥ 11
**Status:** Immutable ground truth

### INV-LV-11-INTENT-1 — Intent is descriptive, not prescriptive

* Intent represents observed interaction state only
* Intent contains no commands, no outcomes, no mutation requests
* Intent is derived only from pointer position, interaction phase, and captured origin

Forbidden:

* Intent triggering mutation
* Intent depending on column model changes
* Intent performing inference beyond hit-testing

### INV-LV-11-INTENT-2 — Intent resolution is pure

* `resolve_*_intent(...)` is pure
* Same inputs ⇒ same outputs
* Reads are limited to: column geometry snapshot + interaction parameters

Forbidden:

* Internal mutation
* Caching
* Controller access
* Time-dependent logic

### INV-LV-11-DIRECTIVE-1 — Directive is declarative

* A directive describes a structural wish
* A directive does not execute
* A directive does not validate feasibility beyond structural syntax

Forbidden:

* Applying directives inside ListView
* Inferring new directives
* Combining directives

### INV-LV-11-PIPELINE-1 — Pipeline ordering is fixed

Only legal flow:

Intent → ReorderDirective → StructureDirective

Forbidden:

* Intent → StructureDirective directly
* StructureDirective → Intent
* Controller calls from ListView

### INV-LV-11-STRUCT-OWN-1 — ListView owns zero structure mutation

* ListView never mutates: column order, column width, column visibility
* ListView emits only directives

Forbidden:

* Structural writes in ListView
* Hidden mutation helpers
* Temporary mutation during interaction

### INV-LV-11-BOUNDARY-1 — Model mutation authority is singular

* ColumnStructureController is the only place allowed to mutate structure
* All mutations must be triggered only by ColumnStructureDirective

Forbidden:

* Direct column Vec manipulation elsewhere
* Controller logic duplicated in UI layer

### INV-LV-11-IDEMP-1 — Reapplication safety

* Reapplying the same directive is deterministic
* Invalid or stale directives are ignored, never panicked

---

## Forbidden Shortcuts (Normative)

### FORBID-LV-11-SHORTCUT-1
ListView must not call ColumnStructureController under any condition.
(Enforces: INV-LV-11-PIPELINE-1, INV-LV-11-BOUNDARY-1)

### FORBID-LV-11-SHORTCUT-2
ListView must not mutate column order, visibility, or width directly.
(Enforces: INV-LV-11-STRUCT-OWN-1)

### FORBID-LV-11-SHORTCUT-3
StructureDirective must not be inferred from geometry or layout state.
(Enforces: INV-LV-11-INTENT-2, INV-LV-11-DIRECTIVE-1)

### FORBID-LV-11-SHORTCUT-4
Any flow bypassing Intent → ReorderDirective → StructureDirective is illegal.
(Enforces: INV-LV-11-PIPELINE-1)

---

## 5. Non-goals

This SPEC does not define:

* batching
* execution timing
* controller policies
* UI affordances

## 6. Enforcement

Any code that:

* mutates structure inside ListView
* bypasses the pipeline ordering
* calls controller logic from ListView
* infers structure from geometry

violates this SPEC and must be rejected.
