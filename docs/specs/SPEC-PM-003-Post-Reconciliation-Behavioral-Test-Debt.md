# SPEC-PM-003: Post-Reconciliation Behavioral Test Debt

**Status:** Active
**Series:** SPEC-PM (Pathology & Mistake Prevention)
**Layer:** Meta-Architecture / Test Integrity
**Owner:** Core Architecture
**Audience:** Engine, Widget, Interaction, Renderer
**Applies To:** All subsystems with failing behavioral tests

---

## 0. Purpose

This SPEC permanently records the 20 behavioral test failures that remain after the test corpus reconciliation (commit 6020105).

These failures are not API drift. They are behavioral ambiguities or bugs in production code, exposed but not caused by reconciliation.

This SPEC exists to prevent these failures from being forgotten, misattributed, or silently normalized.

---

## 1. Non-Scope (Binding)

- This SPEC does not propose fixes
- This SPEC does not reinterpret expected behavior
- This SPEC does not downgrade failures to flakiness

---

## 2. Context

All API-level test failures have been resolved. The test corpus now fully matches current architecture:

- InteractionDomainState ownership of focus, capture, and gesture state
- EventRouter as stateless dispatcher
- LayoutAuthorityGuard enforcement (SPEC-HAL-141/145)
- Current TextEngine and paint APIs
- Current build_multi surface model (SPEC-HAL-163)

Remaining failures are behavioral. They pre-existed reconciliation and were merely exposed by it.

---

## 3. Failure Registry

### 3.1 TextLayoutCache Stale Invariant Violations (2 tests)

Tests that trigger stale-cache invariant assertions in the text layout subsystem. The cache does not correctly invalidate under specific mutation sequences.

### 3.2 Gesture Commit / Capture Behavioral Issues (10 tests)

Tests where gesture recognition commits or capture transitions produce incorrect results. The gesture state machine does not handle all edge cases in commit/cancel/capture sequencing.

### 3.3 TabularView Selection Behavior Tests (3 tests)

Tests where TabularView selection after scroll or hit-test targeting produces incorrect row identification. Behavioral mismatch between expected and actual selection/targeting logic.

### 3.4 Stretch Alignment Assertion (1 test)

A single test where stretch-mode alignment produces an assertion failure. The layout engine does not correctly resolve stretch constraints in the tested configuration.

### 3.5 TextArea Layout Cache Behavior (2 tests)

Tests where TextArea layout cache state becomes inconsistent after specific editing or resize sequences. Related to but distinct from 3.1; these are widget-level cache coordination failures.

### 3.6 Pointer Capture Phase Violation (2 tests)

Tests where pointer capture transitions occur in an incorrect phase or order. The capture state machine permits a sequence that violates expected phase constraints.

---

## 4. Binding Rules

### PM-003-R1: No Deletion

These test failures must not be deleted, `#[ignore]`d, or disabled. Each test asserts expected behavior that is currently incorrect.

### PM-003-R2: Explicit Resolution Required

Each failure category requires either:
- A targeted bugfix with a test-specific explanation, or
- A future SPEC that redefines the expected behavior with justification

No silent resolution. No bulk "cleanup" that removes assertions.

### PM-003-R3: Traceability

Any future fix for a failure listed here must explicitly reference SPEC-PM-003 and state which category (3.1 through 3.6) it resolves.

### PM-003-R4: No Reattribution

These failures must not be attributed to reconciliation. They are pre-existing behavioral issues. Reconciliation exposed them; it did not create them.

---

## 5. Relationship to Other SPECs

This SPEC does not constrain or override other SPECs. It records facts.

Fixes for failures listed here may require new SPEC-HAL entries (for behavioral redefinition) or SPEC-PM entries (if investigation reveals a repeating pathology class).

---

## 6. Closure

This SPEC closes when all 20 failures are resolved, each with explicit traceability per PM-003-R3.

Until then, this document stands as the authoritative registry of known behavioral test debt.
