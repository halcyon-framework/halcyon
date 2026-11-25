# SPEC-HAL-188A: Presentation Snapshot Ownership and Adapter Boundary Clarification

**Status:** Draft (Binding upon adoption)
**Layer:** Presentation / Data Boundary (Clarification Addendum)
**Parent:** SPEC-HAL-188, SPEC-HAL-002
**Extended by:** SPEC-HAL-353 for revision-scoped bounded RowId order-window snapshots
**Scope:** All widgets and presentation-layer consumers of adapter-provided data (including ComboBox, menus, palettes, pickers, inspectors)
**Non-Goals:** New data contracts, new engine APIs, borrowed-props plumbing, modal/popup infrastructure

---

## 1. Purpose

SPEC-HAL-188 defines the data contract layer and the adapter integration seam for TabularSubstrate consumers.

This addendum clarifies a missing rule that becomes critical in retained-mode UI:

- Widgets and presentation layers MAY own presentation snapshots derived from adapter/data-layer outputs.
- Owning a snapshot is not "owning domain data" and does not violate SPEC-HAL-188 §2.1.

This SPEC exists to prevent false "purity blockers" that incorrectly demand borrowed-per-pass data injection into the Widget API.

---

## 2. Definitions (Normative)

### 2.1 Domain Data

"Domain data" is data that is authoritative to the application's model and semantics (e.g., row universe, cell values, capability policy, sort/filter semantics, mutation outcomes).

Domain data authority belongs to the data layer (SPEC-HAL-188 §2.6).

### 2.2 Presentation Snapshot

A "presentation snapshot" is an owned, inert, render-ready copy of information already decided upstream (data layer / adapter / view-model).

Examples:

- `Vec<(OptionId, String)>` for a ComboBox option list
- `Vec<String>` for a palette's visible results
- Preformatted strings, icons, display labels, derived view text
- Any other UI-ready materials that do not grant authority back to the widget

A snapshot is defined by these properties:

- **Owned**: stored in widget or screen state as owned memory
- **Inert**: no liveness guarantees; may be stale until replaced
- **Replace-only**: updated by wholesale replacement, not incremental mutation by the widget
- **Authority-free**: does not convey semantic authority to the widget

---

## 3. Adapter Boundary (Binding)

### 3.1 Adapter is the Only Data Contract Client for Substrates

Where SPEC-HAL-188 establishes an adapter seam (e.g., `DataAdapter<S>`), that seam remains the only client of the SPEC-HAL-188 data contracts for the relevant substrate.

- TabularSubstrate consumers MUST NOT call data contracts directly (SPEC-HAL-188 §12).
- Widgets that are not substrates MUST NOT "reach around" the adapter to become contract clients.

### 3.2 Widgets Are Snapshot Consumers

Widgets MAY consume adapter-derived outputs only as presentation snapshots.

Widgets MUST treat snapshots as:

- Replace-only inputs
- Render materials
- Event-to-signal surfaces

Widgets MUST NOT treat snapshots as:

- Authoritative universes
- Stable identities beyond the IDs already provided
- Incremental data structures they "maintain"

---

## 4. Snapshot Ownership Rules (Binding)

### 4.1 Snapshot Ownership is Allowed and Expected

Owning a presentation snapshot in a widget is allowed and expected in a retained-mode UI.

This does not violate "No data ownership" (SPEC-HAL-188 §2.1), because that rule applies to domain data authority, not to owned render snapshots.

### 4.2 Replace-Only Update Law

If a widget stores a snapshot:

- The snapshot MUST be updated by replacement (e.g., `clear + extend`, `= new_vec`, or equivalent).
- The widget MUST NOT apply semantic diffs (filtering, ranking, matching, sorting, normalization) to the snapshot unless a governing SPEC for that widget explicitly grants such authority (default: forbidden).

### 4.3 No Snapshot-Derived Authority

A widget MUST NOT use snapshot contents to infer semantic intent or policy.

This includes (non-exhaustive):

- Inferring commits from label comparisons
- Deciding meaning of user input by "matching" strings
- Applying normalization/heuristics to interpret user actions

This reinforces SPEC-HAL-002 §3.11 (No Interpretive Intent Resolution).

### 4.4 Revision-Scoped Order Windows

Under SPEC-HAL-353, each bounded RowId order window is a distinct presentation snapshot keyed by
`(OrderRevision, ordinal range)`.

- Inserting or replacing one complete returned window in an adapter cache is lawful snapshot replacement.
- A stale-revision window MUST be discarded.
- The adapter MUST NOT stitch windows into an unbounded complete order and treat that accumulation as authority.
- Widgets consume adapter-projected RowIds for realized rows; they do not manage the window cache or infer missing order.

---

## 5. Widget API Reality Clause (Normative)

Halcyon's Widget trait does not accept arbitrary borrowed external props per traversal call.

Therefore:

- Borrowed-per-pass data injection is not a baseline requirement for widgets.
- Requiring widgets to operate only on borrowed slices would force engine-level API changes and is not implied by SPEC-HAL-188.

This SPEC explicitly endorses the retained-mode snapshot approach as the conforming default.

---

## 6. Explicit Prohibitions (Binding)

The following are architecturally illegal for widgets consuming presentation snapshots:

1. **Semantic transformation of the snapshot** — Filtering, ranking, sorting, fuzzy matching, normalization, case-folding, heuristic merging.
2. **Becoming a data contract client** — Calling SPEC-HAL-188 contracts directly (`RowUniverse`/`CellReader`/etc.) from ordinary widgets.
3. **Index authority leakage** — Treating snapshot indices as stable identities or storing index-based selections that outlive snapshot replacement.
4. **Intent inference from state inspection** — Selecting CommitSource or "user meaning" from incidental state (e.g., "highlight exists so it must be selection intent").

These prohibitions apply regardless of whether the snapshot originated from an adapter, screen, or other upstream presenter.

---

## 7. Failure Modes and Correct Handling (Normative)

### 7.1 Snapshot Staleness

Snapshots may be stale. Widgets MUST tolerate that:

- Rendering may show stale labels until replaced.
- Interactions must use authoritative IDs and external resolvers/policies where required.
- A stale snapshot is not an excuse for widget-owned "smart fixes."

### 7.2 Snapshot Replacement Mid-Interaction

If a snapshot is replaced while a widget is interacting:

- The widget MUST clear or revalidate transient pointers into the snapshot (e.g., highlighted index).
- The widget MUST keep stable state only via stable IDs (e.g., `OptionId`), not indices.

---

## 8. Relationship to ComboBox / Resolver Architecture (Normative)

For resolver-driven selection controls (SPEC-HAL-199 / SPEC-HAL-201):

- Option lists stored in widgets are presentation snapshots.
- The widget may build transient borrowed views (e.g., `&str` from owned `String`) at the moment of forwarding to the resolver.
- The widget MUST NOT interpret labels; labels are render-only.
- Commit meaning remains resolver-owned (SPEC-HAL-199; SPEC-HAL-002 §3.11, §4.11).

This SPEC does not grant ComboBox any semantic authority. It only clarifies that storing render snapshots is allowed.

---

## 9. Non-Goals (Binding)

This SPEC does NOT:

- Add new data contracts
- Change SPEC-HAL-188 semantics
- Require or design borrowed-props injection
- Add typemap/props to EventContext/PaintContext/AppContext
- Introduce popup infrastructure or modal changes
- Mandate any performance strategy

---

## 10. Acceptance Criteria

This SPEC is satisfied when it:

- Defines "presentation snapshot" as owned, inert, replace-only, authority-free data.
- States that widget snapshot ownership is allowed and expected in retained-mode UI.
- Preserves adapter authority and keeps widgets out of data contracts.
- Enumerates explicit prohibitions preventing semantics from creeping into widgets.
- Aligns with SPEC-HAL-002 §3.11 and §4.11 and does not contradict SPEC-HAL-188.

---

## 11. Cross-References

See SPEC-HAL-202 for derived composition analysis documenting how snapshot ownership composes inside editor widgets embedded in tabular consumers.
