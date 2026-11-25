# SPEC-HAL-189: Row and Cell Reuse & Dirty-Flag Law

**Status:** Binding
**Layer:** Widget Lifecycle / Identity Management
**Scope:** ListView, TreeView, Grid, PropertyEditor, all TabularSubstrate consumers
**Parent:** SPEC-HAL-187, SPEC-HAL-188
**Depends on:** SPEC-HAL-164 (TabularSubstrate), SPEC-HAL-187 (Lock-In Laws), SPEC-HAL-188 (Data Contracts)
**Non-Goals:** Rendering details, styling, editor behavior, concrete cache-eviction algorithm

---

## 1. Purpose

To formally lock the architectural decision that row and cell semantic identity
is stable, while concrete widget instances form a bounded realization cache:

**A resident row or cell widget is reused by identity and updated via
dirty-flagging. An unpinned offscreen instance may be evicted when its
realization residency ends; eviction never ends or changes RowId/ColumnId
identity.**

This SPEC exists to prevent:

- accidental widget churn within the realized/pinned set
- identity churn
- broken nested widgets
- scroll-induced rebuilds inside the retained realization set
- reorder-induced rebuilds
- lazy "just rebuild" compromises
- widget memory proportional to total provider extent

---

## 2. Foundational Statement

This SPEC is foundational. All future work (TreeView, Grid, PropertyEditor, inline editors, nested widgets) MUST comply.

These are not suggestions. They are architectural invariants. Violation is not a bug — it is an SPEC breach.

---

## 3. Identity Law (Hard Invariants)

### 3.1 Row Identity

**INV-189-ROW-IDENTITY:** Globally, row identity is scoped by
**(RowIdentityDomainId, RowId)**. One RowHost is bound to exactly one current
identity domain, so `RowId` remains its sole per-row key and at most one concrete
resident instance may exist for that RowId. The host-level domain is provider
testimony from SPEC-HAL-188/353, never identity inferred from adapter object
address or RowId reuse.

**INV-189-ROW-STABLE:** RowId is stable for the lifetime of the row.

**INV-189-ROW-REUSE:** While a RowId remains realized or interaction-pinned, its
row widget MUST be reused across ALL of the following:

- scroll
- reorder
- visibility changes
- value updates
- policy updates

### 3.2 Cell Identity

**INV-189-CELL-IDENTITY:** Globally, cell identity is scoped by
**(RowIdentityDomainId, RowId, ColumnId)**. Inside a host already bound to that
domain, `(RowId, ColumnId)` remains the sole per-cell key.

**INV-189-CELL-REUSE:** While its owning row and cell remain realized or
interaction-pinned, a cell widget MUST be reused across ALL of the following:

- scroll
- reorder
- value updates
- policy updates
- geometry changes

### 3.3 Identity Death and Realization Residency

**INV-189-RECREATION:** Recreation of a still-resident or interaction-pinned
widget is permitted only when identity dies. A different concrete instance may
be created after a lawful offscreen, unpinned residency eviction; that is a new
realization of the same semantic identity, not identity death.

Valid identity death triggers (exhaustive list):

- RowId removal
- ColumnId removal (future)
- A provider-confirmed change of `RowIdentityDomainId`

`OrderProjectionReplaced { revision, .. }` and compatibility
`BaseOrderReplaced { revision, .. }` always invalidate revision-scoped order/ordinal
state. When the new revision retains the same `RowIdentityDomainId`, surviving
RowIds that remain realized or pinned retain their row and cell widget instances;
only identities whose absence the data authority confirms die, either through
optional current-universe `RowsRemoved { universe, .. }` detail or subsequent RowId re-resolution. When the new
revision changes `RowIdentityDomainId`, every prior row/cell identity dies as one
authoritative domain replacement even if raw RowId values repeat. A provider or
adapter instance change with the same domain ID is not identity death, while an
instance change without explicit domain testimony is non-conforming.

For a resident instance, all non-death updates MUST use reuse + dirty flags.
Offscreen eviction is lawful only under the bounded residency law in §7.

---

## 4. Forbidden Behaviors (Absolute)

**INV-189-FORBIDDEN:** The following behaviors are explicitly forbidden:

| Forbidden Behavior | Why |
|---|---|
| Recreating a widget that remains inside the realized/overscan set during scroll | Its residency survives |
| Recreating a retained widget on reorder | Reorder does not end residency |
| Recreating a resident widget on policy-driven visibility change | Use reuse or lawful residency eviction |
| Recreating widgets on value change | Use BindingDirty |
| Recreating widgets on policy change | Use PolicyDirty |
| Keying widgets by index | Index is ephemeral |
| Keying widgets by visible position | Position is ephemeral |
| Evicting a focused, captured, editing, IME-owning, or otherwise interaction-pinned widget | Live interaction requires residency |
| Retaining one concrete widget per RowId in the provider universe | Instance memory would scale with total extent |
| Diff-based index recovery logic | SPEC-HAL-187 forbids this |

Any code performing the above is invalid by definition.

---

## 5. Dirty Flag Model (Closed Set)

**INV-189-DIRTY-EXHAUSTIVE:** Dirty flags are mandatory and exhaustive. No additional flags may be introduced without amending this SPEC.

### 5.1 Dirty Flags

#### BindingDirty

- **Meaning:** Underlying data value changed
- **Requires:** Rebind data to existing widget
- **Does NOT:** Affect identity or geometry

#### PolicyDirty

- **Meaning:** Permissions / capabilities / editability changed
- **Requires:** Capability re-evaluation
- **Does NOT:** Recreate widgets

#### GeometryDirty

- **Meaning:** Layout-affecting change (row height, column width)
- **Requires:** Layout recomputation
- **Does NOT:** Recreate widgets

#### PresentationDirty

- **Meaning:** Purely visual change (theme, style, selection highlight)
- **Requires:** Repaint only
- **Does NOT:** Recreate widgets

#### FullDirty

- **Meaning:** Composite dirtiness (equivalent to all above flags)
- **Still does NOT:** Permit recreation

### 5.2 Non-Dirtiness

**INV-189-CLEAN-NOOP:** No dirty flag ⇒ no work. Clean widgets MUST NOT be touched.

This is a performance invariant.

Dirty flags apply to concrete resident instances. An unresident identity has no
widget-local dirty record; a later realization resolves current binding, policy,
geometry, and presentation state from their authoritative owners. RowHost MUST
NOT replace unbounded widget retention with an unbounded offscreen dirty map.

---

## 6. Dirty Propagation Law

### 6.1 Source of Dirtiness

| Component | Role |
|---|---|
| `DataAdapter` | Emits invalidation events |
| `TabularSubstrate` | Interprets invalidation semantically |
| `ListView` | Forwards dirtiness, owns no semantics |
| `RowHost` / CellHost | Marks concrete widgets dirty |

### 6.2 Mapping Rules

**INV-189-CELLS-INVALIDATED:** current-universe `CellsInvalidated { universe, .. }` → `BindingDirty` for affected (RowId, ColumnId).

**INV-189-POLICY-INVALIDATED:** current-universe `PolicyInvalidated { universe, .. }` → `PolicyDirty`.

**INV-189-EXTENT-CHANGED:** current-universe `ExtentChanged { universe }` → NO rebuild. May imply `GeometryDirty`.

**INV-189-BASE-ORDER-REPLACED:** `OrderProjectionReplaced { revision, .. }` or
compatibility `BaseOrderReplaced { revision, .. }` invalidates prior-revision order
windows, ordinal mappings, and geometry derived from them. With an unchanged
`RowIdentityDomainId`, it does not recreate widgets for surviving RowIds and the
event alone does not testify which identities were removed. With a changed
domain ID, it testifies that the complete old identity domain died and requires
teardown of every resident/pinned widget keyed by that domain before any repeated
raw RowId is realized under the new domain.

No data/policy/order invalidation path may recreate a retained resident instance.
Lawful offscreen residency eviction remains governed only by §7.

---

## 7. RowHost Law

**INV-189-ROWHOST:** RowHost MUST satisfy ALL of the following:

| Requirement | Reason |
|---|---|
| Host bound to one `RowIdentityDomainId`; entries keyed strictly by `RowId` | Identity stability without duplicating the domain on every entry |
| No index storage | Index is ephemeral |
| Bounded by visible rows, declared overscan, interaction pins, and an explicit finite cache budget | Instance memory must not scale with total extent |
| No eviction while a widget is visible, in overscan, or interaction-pinned | Live realization and interaction survive |
| Deterministic eviction of offscreen, unpinned instances is permitted | Semantic identity is not instance residency |
| No reorder-based eviction of retained instances | Reorder does not end residency |
| Immediate removal on identity death | Dead identities cannot remain interactive |

**RowHost is a bounded realization cache keyed by identity, not an identity
owner.** Selection, focus, provider membership, edit-session authority, and any
state required to survive eviction live in their current-domain-scoped RowId-keyed owning systems, not
inside the concrete row widget. Widget instance handles or generations must
never be treated as data identity.

An interaction pin exists only while an authority requires continuity of that
concrete instance: active focus, pointer capture, edit/IME session, or another
declared in-flight interaction with equivalent continuity requirements. Pins are
released when that authority ends. Selection alone, historical interaction, or
mere RowId existence does not pin residency.

---

## 8. Cell Hosting Law

**INV-189-CELLHOST:** Cell widgets are owned by a dedicated host bound to the
current `RowIdentityDomainId` and keyed internally by **(RowId, ColumnId)**.

Cell widgets:

- Inherit row identity stability
- Follow the same dirty-flag discipline
- MUST survive reorder and partial invalidation while resident
- MAY be evicted only with a lawfully evicted owning row, or independently under
  an explicit bounded cell-realization policy when unpinned and offscreen

Ownership must be explicit and centralized.

---

## 9. Nested Widget Guarantee

**INV-189-NESTED:** Any widget nested inside a resident row or cell MUST satisfy:

| Guarantee | Meaning |
|---|---|
| Preserve its instance across scroll while resident/pinned | No churn inside the realization set |
| Preserve its instance across reorder while resident/pinned | Reorder does not end residency |
| NOT be recreated due to dirtiness | Use dirty flags instead |
| Externalize state that must survive lawful eviction | Re-realization cannot erase semantic state |
| Never evict during active nested interaction | Capture/edit/IME and equivalent state pin residency |

This guarantee is required for:

- TreeView
- Property editors
- Inline editors
- Composite cells

---

## 10. Test Enforcement Requirement

**INV-189-TESTS:** Implementations MUST include tests proving:

- Row widgets survive reorder
- Row widgets inside the realized/overscan or interaction-pinned set survive scroll
- Resident cell widgets survive invalidation
- Nested widgets survive reorder + scroll while resident/pinned
- Row and cell widgets for resident/pinned surviving RowIds survive order-revision replacement
- An `OrderUniverseId` replacement with the same `RowIdentityDomainId` preserves
  resident/pinned surviving widgets, while a domain-ID replacement kills all old
  widget identities even when raw RowIds repeat
- Offscreen unpinned eviction and re-realization preserve current-domain-scoped
  RowId-keyed semantic state
- Traversal across a million-row synthetic universe keeps concrete row/cell
  instance count within the declared realization/cache bound
- Resident/pinned widgets are recreated only on identity death

Tests are not optional documentation. **They are enforcement.**

---

## 11. Finality Clause

This SPEC is binding.

Future work must build on it. Shortcuts, unbounded identity caches, and
"temporary" resident-widget rebuilds are violations.

If a future feature requires breaking this law, a new SPEC must:

- Explicitly supersede this one
- List exact breakage
- Justify the architectural cost

Until then, this law stands.
