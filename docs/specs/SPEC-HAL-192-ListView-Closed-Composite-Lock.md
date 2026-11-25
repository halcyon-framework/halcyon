# SPEC-HAL-192: ListView Closed-Composite Lock

**Status:** Binding
**Layer:** Widget Architecture
**Scope:** ListView only
**Depends on:** SPEC-HAL-190 Appendix A

---

## 1. Purpose

This SPEC permanently locks ListView's architectural status as a closed composite.

Any future modification that violates this lock requires an explicit SPEC overturning SPEC-HAL-192.

---

## 2. Invariants

### INV-LISTVIEW-CLOSED-1: Closed Composite

ListView is a **closed composite** per SPEC-HAL-190 Appendix A.

- All internal structure is fixed at implementation time
- No external widgets are plugged in as children
- Header, body, footer are internal organs, not extensibility points

### INV-LISTVIEW-CLOSED-2: Direct Substrate Ownership

ListView owns `TabularSubstrate` directly as a struct field.

```rust
pub struct ListView<S: DataSource> {
    substrate: TabularSubstrate,  // Owned, not projected
    // ...
}
```

### INV-LISTVIEW-CLOSED-3: Direct Substrate Passing

ListView accesses its substrate via direct method calls in all phases:

- **Layout:** `self.substrate.set_body_viewport()`, `self.substrate.prepare_evaluation()`
- **Paint:** `self.substrate.body().realized_rows()`, `self.substrate.geometry_view()`
- **Event:** `self.substrate.handle_intent()`

---

## 3. Forbidden APIs

ListView MUST NOT use:

| API | Reason |
|-----|--------|
| `cx.authority<T>()` | ListView owns its substrate; no ancestor lookup needed |
| `cx.emit_intent()` | ListView processes intents directly via `self.substrate.handle_intent()` |
| `AuthorityOwner` trait | ListView is not an authority provider to external children |

**Note:** Tests may reference SPEC-HAL-190 types for verification purposes. This lock applies to production ListView code only.

---

## 4. Enforcement

Violation of any invariant in §2 or §3 is an **SPEC breach**, not a bug.

To modify ListView in a way that violates this lock:

1. Write a new SPEC explicitly overturning SPEC-HAL-192
2. State the architectural justification
3. Obtain approval before implementation

---

## 5. Rationale

ListView works. It is the canonical example of a closed composite done right.

This lock exists to prevent:

- Cargo-culting SPEC-HAL-190 machinery into a widget that does not need it
- Accidental complexity from indirect authority lookups
- Architectural drift during maintenance

---

## 6. Scope Limitation

This SPEC says nothing about:

- TreeView, Grid, or future composites
- SPEC-HAL-190 applicability elsewhere
- ListView feature additions that preserve the closed-composite pattern

Only the invariants in §2 and §3 are locked.
