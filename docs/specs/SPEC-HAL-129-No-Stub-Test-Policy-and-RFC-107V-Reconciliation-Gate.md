# SPEC-HAL-129: No-Stub Test Policy & SPEC-HAL-107-V Reconciliation Gate

**Status:** Binding (Tests/Process)
**Scope:** `tests/` only (no production code changes required by this SPEC)
**Layer:** Quality Gate / Verification Law
**Owner:** Engine integrity
**Motivation:** Prevent “green bar” from empty `#[test] { /* TODO */ }` bodies and force reconciliation to real, assertion-based tests.

---

## 1. Definitions

**Stub test**: any Rust `#[test]` whose body contains no assertions and performs no observable verification, including but not limited to:
- `// TODO` body
- empty body `{ }`
- `panic!()` / `todo!()` / `unimplemented!()` used as placeholder
- “assertions” that always pass without checking behavior (e.g. `assert!(true)`)

**Assertion-based test**: a test that validates at least one measurable invariant via `assert!`, `assert_eq!`, `matches!`, or equivalent.

**SPEC-HAL-107-V test group**: tests intended to verify invariants defined by SPEC-HAL-107-V (editing semantics).

---

## 2. Non-Negotiable Invariants (Binding)

### INV-129-1 — Zero Stub Tests in `tests/`
No stub test is allowed anywhere in `tests/`.

**Forbidden:**
- Leaving placeholder bodies “for later”
- Checking in stubs “temporarily” while “real work is elsewhere”
- Counting stubs as coverage

### INV-129-2 — Stub Detection Must Be Automated
Repository must contain an automated check that fails CI/test runs if a stub test exists.

Minimum required detection:
- Detect `#[test]` functions whose body contains `// TODO`
- Detect `#[test]` functions with an empty body
- Detect `#[test]` functions containing `todo!()` / `unimplemented!()` / `panic!("TODO")` patterns

### INV-129-3 — SPEC-Tagged Test Files Must Prove Their Tag
Any test file that references an SPEC (e.g., contains `SPEC-HAL-107-V`) must contain assertion-based tests that directly correspond to invariants from that SPEC.

**Forbidden:**
- SPEC label in header while tests do nothing
- “Skeleton suites” that only name tests

### INV-129-4 — Reconciliation Does Not Touch Production Code By Default
Reconciliation work is test-only unless a failing test reveals a real bug.
If a bug is revealed:
- The failing test must land first (or in the same commit)
- The production fix must be minimal and directly justified by the failing test

### INV-129-5 — Reporting Must Be Mechanical
Any reconciliation task must output:
- Exact list of affected files
- Count of tests before/after
- Proof via `cargo test -- --list` that the tests exist
- Evidence of assertions (e.g., grep of `assert_` lines) per file

---

## 3. Targeted Cleanup Object (The Known Offenders)

The following files are designated as **SPEC-129 reconciliation targets** when found to be stubbed:

- `tests/layout_order_invariants.rs`
- `tests/line_newline_invariants.rs`
- `tests/selection_invariants.rs`
- `tests/text_deletion_invariants.rs`
- `tests/text_insertion_invariants.rs`
- `tests/enter_key_invariants.rs`

They must contain real assertion-based tests or be removed from `tests/` entirely.
(“Removed” is allowed only if the SPEC tag is also removed and the tests are replaced by a correctly-located suite elsewhere. No silent deletion.)

---

## 4. Required Mechanism: Stub-Gate Test

A new test must exist (name is binding):

`tests/no_stub_tests_gate.rs`

It must fail if any stub patterns are present in `tests/`.
Implementation strategy is up to the implementer, but it must be deterministic and run on all platforms.

Minimum behavior:
- Scan all `.rs` files in `tests/`
- Fail with a message containing the filename(s) that violate INV-129-1

---

## 5. Staging Plan

### Stage 129.1 — Install the gate
- Add `tests/no_stub_tests_gate.rs`
- Make it fail on the current known stub patterns

**Acceptance:** Gate test fails if stubs exist; passes when none exist.

### Stage 129.2 — SPEC-HAL-107-V reconciliation (tests-only)
- Convert the 6 target files into assertion-based suites grounded in the current editing substrate and widget wiring.
- If an invariant is not testable with current harness APIs, the test must be `#[ignore]` with an exact missing API note.

**Acceptance:** No `// TODO` bodies, no empty tests, suites contain real asserts.

### Stage 129.3 — Long-term maintenance rule
- Any PR adding/altering tests must pass the gate.
- Any new SPEC-tagged test file must prove the tag with asserts.

**Acceptance:** Gate remains enforced; no regressions.

---

## 6. Out of Scope

- Changing SPEC-HAL-107-V content
- Redesigning the editing model
- IME and complex shaping correctness (separate SPEC)

---

## 7. Success Criteria

This SPEC is “Complete” when:
- `tests/no_stub_tests_gate.rs` exists and passes
- Zero stub tests exist in `tests/`
- The 6 target files contain assertion-based tests or have been legitimately replaced with equivalent real suites
- `cargo test` passes
