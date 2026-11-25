# SPEC-HAL-187: Pre-ListView Lock-In Laws

**Status:** Binding
**Layer:** TabularSubstrate / ListView Authority Boundary
**Scope:** ListView, TreeView, adapters, demos, tests
**Parent:** SPEC-HAL-164 (TabularSubstrate — semantic engine authority)
**Depends on:** SPEC-HAL-164, SPEC-HAL-179, SPEC-HAL-180, SPEC-PM-001
**Non-Goals:** Implementation details, rendering, animation, UX polish

**Amended by:** SPEC-HAL-347 for current ListView selection/focus delegation. ListView hosts one shared-engine `OrderedSelectionState<RowId>` in `ListViewCore` and delegates semantics to `reduce_ordered_selection`; data-authoritative `RowId` order supplies the reducer oracle. This is the SPEC-HAL-262 architecture, not duplicated widget semantics.
**Amended by:** SPEC-HAL-353 for provider order revisions and bounded revision-scoped RowId windows. A base-order
replacement invalidates the prior revision; it does not require a complete RowId vector at the presentation boundary.

---

## 1. Purpose

This SPEC freezes four structural laws before ListView-from-scratch begins.

These are not suggestions. They are architectural invariants derived from historical failures (PM-001). Violation is not a bug — it is an SPEC breach.

---

## 2. INV-ROW-BASE-REPLACE — Base Order Replacement Semantics

Any operation that reorders or replaces the provider-supplied RowId sequence MUST be treated as a base order replacement and MUST bump `base_row_order_epoch` exactly once.

Partial, in-place, or incremental reorder of the base RowId sequence is forbidden.

For a windowed provider under SPEC-HAL-353, the provider-supplied sequence is represented by an `OrderRevision` plus
bounded RowId windows. Publishing a new revision is the replacement operation. The adapter invalidates all prior-revision
windows and maps that revision transition to exactly one epoch bump; it does not patch windows into a synthetic stable
sequence.

**Rationale:**
Prevents "fast reorder", "swap two rows", "adjust indices" shortcuts that bypass epoch invalidation.

**Applies to:**
- Provider-driven data refresh
- Bulk row replacement
- Any operation that changes the base RowId sequence

---

## 3. INV-ROW-INDEX-EPHEMERAL — Index Ephemerality Law

Row indices (`usize`, render index, visible index) are frame-local artifacts.

They MUST NOT be stored, cached, or persisted across frames, epochs, or intents.

Any persisted reference to a row MUST use `RowId`.

A bounded order-window request or identity-to-ordinal lookup may use an ordinal only as an ephemeral transport coordinate
tied to one explicit `OrderRevision`. That ordinal is not identity, must not escape adapter/virtualization mechanics, and
becomes invalid when the revision changes. An adapter may retain a window range as bounded cache metadata across frames
only while that revision remains current; it must not expose the ordinal as semantic state or grow the cache into a
complete order.

**Explicitly applies to:**
- ListView
- TreeView
- Adapters
- Demos
- Tests
- "Temporary helpers"

**Rationale:**
Prevents the silent reintroduction of PM-001 via convenience code.

---

## 4. ListView Authority Contract

### 4.1 ListView MUST NOT:

- Own row identity
- Own row order
- Own selection semantics
- Own focus semantics
- Reinterpret geometry
- Cache row indices
- Derive semantics from visibility

### 4.2 ListView MUST:

- Delegate data-backed ordering requests to the data layer through DataAdapter
- Delegate navigation, selection, focus, and virtualization mechanics over the confirmed order projection to the substrate
- Render rows based on substrate projections
- Forward intents only
- Host row widgets only

**Translation:**
ListView is a host + renderer, not a brain.

---

## 5. Forbidden Index-Based APIs

The following API patterns are architecturally illegal:

### 5.1 Forbidden (examples):

- `selected_index()`
- `focused_row_index()`
- `scroll_to_index(i)`
- `row_at_y(y) -> usize`
- `visible_row_indices()`

### 5.2 Allowed equivalents:

- `focused_row_id()`
- `scroll_to_row(RowId)`
- `row_at_y(y) -> Option<RowId>`

**Rule:**
If an API forces the caller to reason in indices, it is forbidden.

Revision-scoped provider window enumeration is not a public index-based row API. Cell reads, mutations, focus,
selection, scrolling targets, and host callbacks remain RowId-based.

**Rationale:**
This single ban eliminates ~70% of historical ListView bugs.

---

## 6. Acceptance Criteria

SPEC-HAL-187 is satisfied when:

- Base order replacement bumps epoch exactly once (INV-ROW-BASE-REPLACE)
- No code persists row indices across frames (INV-ROW-INDEX-EPHEMERAL)
- Any order-window ordinal is bounded, revision-scoped, adapter-local, and discarded on revision change
- ListView delegates order semantics to the data layer and row mechanics to the substrate (§4; SPEC-HAL-353)
- No index-based public APIs exist (§5)

---

## 7. Strategic Note (Non-Normative)

This SPEC exists because ListView failed twice when these rules were implicit.

If these are not frozen now, ListView will grow "just one helper" and rot again.

There will not be a third attempt without this contract.

---

## 8. Cross-References

See SPEC-HAL-202 for derived composition analysis verifying virtualization and destabilization behavior for editor widgets inside tabular consumers.
