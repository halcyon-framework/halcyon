# SPEC-HAL-083A — SceneCache Dispatch Mandatory Wiring

## Status

**Binding**

## Scope

This SPEC is a **hard enforcement addendum** to SPEC-HAL-083.
It exists solely to prevent omission of SceneCache dispatch wiring (Option B) after Phase-15.

## Motivation

Phase-15 intentionally introduces a **non-functional cache skeleton**.
Without an explicit enforcement artifact, the cache dispatch wiring can be silently skipped in later phases, resulting in an architecturally invalid but seemingly “working” system.

This SPEC converts that risk into a **compile-time and process-level impossibility**.

## Definitions

* **Cache Dispatch (B)**
  Invocation of `dispatch_cache_operations(...)` from the frame loop, after SceneGraph diffing and before render preparation.

* **Phase-16**
  Any phase that introduces *actual* cache storage, reuse, eviction, GPU resources, or cache lifetime.

## Invariants

### INV-083A-1 — Mandatory Dispatch Presence

Any phase ≥ Phase-16 **MUST** wire `dispatch_cache_operations(...)` into the frame loop.

Absence of this wiring is a **hard violation**.

### INV-083A-2 — Compile-Time Enforcement

Phase-16 or later **MUST NOT compile** unless cache dispatch wiring is present.

The enforcement mechanism **must be mechanical**, not procedural.

Allowed mechanisms:

* `compile_error!`
* feature-gated build failure
* unconditional panic in debug builds

Comments, TODOs, or documentation alone are **explicitly insufficient**.

### INV-083A-3 — Ordering Constraint

Cache dispatch **MUST** occur:

```
SceneGraph build
→ SceneGraph diff (SPEC-HAL-083)
→ SceneCache dispatch (this SPEC)
→ Render preparation
```

Reordering is forbidden.

## Required Artifacts (Non-Optional)

All of the following **MUST exist** before Phase-16 begins:

1. **Compile-time or debug-time guard**
   Prevents enabling cache functionality without dispatch wiring.

2. **Explicit call site anchor**
   A named, frozen location in the frame loop where cache dispatch is performed.

3. **Invariant marker**
   A visible invariant comment near the call site stating ordering and necessity.

## Forbidden Behaviors

* Introducing cache storage without dispatch wiring
* “Temporary” caching bypasses
* Lazy or deferred wiring “to be done later”
* Relying on memory, TODOs, or oral tradition

Any of the above constitutes an **architectural violation**.

## Relationship to SPEC-HAL-083

* SPEC-HAL-083 defines *what* the cache is and how it evolves.
* SPEC-HAL-083A defines *that the cache pipeline must exist at all*.

SPEC-HAL-083A **overrides convenience** in favor of structural correctness.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-364 §12 is the explicit new follow-up required by frozen
SPEC-HAL-083 §§15–16. Its presented-value path still performs ordinary diff,
mandatory dispatch and render preparation in that order. Frozen SPEC-HAL-083
is not edited; this relationship adds neither another cache nor a dispatch
exception.

## Exit Criteria

This SPEC is considered satisfied when:

* Cache dispatch is wired
* Enforcement is active
* Phase-16 proceeds without guards firing
