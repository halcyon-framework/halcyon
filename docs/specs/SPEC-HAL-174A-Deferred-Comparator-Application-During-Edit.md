# SPEC-HAL-174A: Deferred Comparator Application During Edit

**Status:** Binding Addendum  
**Scope:** TabularSubstrate sort + edit interaction  
**Extends:** SPEC-HAL-174 (Edit Authority), SPEC-HAL-171 (Sort Semantics)

---

## Purpose

This addendum formalizes the behavior of row comparator changes that occur during an active edit session in
`TabularSubstrate`.

Comparator changes are **sort-affecting configuration operations**. Applying them during edit would violate edit
authority guarantees by mutating row order while a cell owns exclusive interaction authority.

This SPEC defines **deferral semantics** for comparator changes during edit.

---

## Rule Summary

- A comparator change **must not** take effect while an edit session is active.
- Comparator changes requested during edit are **deferred**.
- Deferred comparator changes are **applied immediately when the edit session ends**, regardless of whether the edit is
  committed or canceled.

---

## Invariants

### INV-EDIT-ORDER-1 (Comparator Edit Exclusion)

While an edit session is active:

- The active row order **must remain stable**.
- No comparator change may affect sorting state.
- Sort dirty/version state must not change as a result of comparator changes.

---

### INV-EDIT-ORDER-2 (Deferred Storage)

If `set_comparator` is called during an active edit session:

- The new comparator **must be stored in a private deferred slot**.
- The active comparator **must remain unchanged**.
- No sort intent or sort-state mutation may occur.

This storage is **internal-only** and not observable externally.

---

### INV-EDIT-ORDER-3 (Post-Edit Application)

When an edit session ends (commit or cancel):

- If a deferred comparator exists, it **must be applied immediately**.
- Applying the comparator **must re-dirty sort state** if sorting is active.
- The resulting sort recomputation occurs at the next legal layout phase.

Commit and cancel are treated **identically** for comparator application.

---

## Debug vs Release Semantics

- **Debug builds:**  
  Calling `set_comparator` during an active edit **must panic** to surface misuse.

- **Release builds:**  
  The call **must not panic** and must defer the comparator change silently.

This mirrors existing edit authority enforcement patterns.

---

## Non-Goals

- This SPEC does not change sort intent semantics.
- This SPEC does not require comparator changes to be modeled as intents.
- This SPEC does not introduce new public APIs.
- This SPEC does not alter edit commit or cancel semantics beyond comparator application.

---

## Enforcement Notes

- Comparator deferral must be explicit and test-covered.
- No code path may apply a comparator directly during edit.
- Any future refactor touching comparator handling must preserve these invariants unless this SPEC is explicitly
  superseded.

---

## References

- SPEC-HAL-174: Edit Authority
- SPEC-HAL-171: TabularSubstrate Sort Semantics
- `TabularSubstrate::set_comparator`
- `TabularSubstrate::handle_edit_intent`

---

## Closure

**This addendum is fully implemented and closed.** Implementation: `src/widgets/tabular_view/mod.rs` (`set_comparator`, `apply_pending_comparator`, `pending_comparator` field). All invariants (INV-EDIT-ORDER-1 through INV-EDIT-ORDER-3) enforced by deferred-slot storage during edit and immediate application on edit end. No further work permitted under this addendum.
