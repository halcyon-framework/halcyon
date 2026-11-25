# SPEC-HAL-188: Data Source & Mutation Contract Layer

**Status:** Draft (Binding upon adoption)
**Layer:** Presentation / Data Boundary
**Scope:** TabularSubstrate consumers (ListView, TreeView, Grid, PropertyEditor)
**Parent:** SPEC-HAL-164, SPEC-HAL-187
**Amended by:** SPEC-HAL-353 (provider-authoritative sorting, order revisions, and bounded RowId windows)
**Non-Goals:** Storage model, transport, caching strategy, backend architecture

---

## 1. Purpose

This SPEC defines a data source abstraction layer for Halcyon.

Halcyon is a presentation system, not a data container.
UI components MUST NOT assume where data comes from, how it is stored, or whether it is resident in memory.

This SPEC establishes orthogonal contracts for:

- reading data
- mutating data
- capabilities (visibility, edit, commit)
- invalidation
- provider query/order control and bounded order projection

All contracts are designed to co-exist with virtualization and RowId-based identity.

---

## 2. Core Principles (Binding)

1. **No data ownership**
   UI components never own domain data.

2. **Identity over position**
   Semantic data access is keyed by RowId (+ ColumnId), never by index. Provider order enumeration may use bounded
   ordinal windows only when tied to one explicit `OrderRevision`; those ordinals are transport coordinates, not
   identity.

3. **Lazy by default**
   Data may be remote, paged, partial, delayed, or denied.

4. **Policy-separable**
   Visibility, readability, editability, and commit-ability are independent concerns.

5. **Virtualization-safe**
   No contract may require contiguous indices, full materialization, or stable row counts.

6. **Backend is authority**
   UI may request; backend decides.

---

## 3. Contract Set Overview

This SPEC defines seven logically independent contracts:

1. **RowUniverse** — what rows exist (Required)
2. **CellReader** — how cell values are resolved (Required)
3. **CellMutator** — how edits are requested (Optional)
4. **CellCapabilities** — what operations are permitted (Optional)
5. **DataInvalidation** — how UI learns data changed (Required)
6. **EditSession** — lifecycle for inline editing (Optional)
7. **OrderControl** — baseline, sort capabilities/requests, and confirmed applied sort (Required for sortable universes)

They MAY be implemented by a single type or split across layers.

---

## 4. ValueState Model (Normative)

All fallible provider reads, including cell reads and bounded order-window reads, return a `ValueState`, not a raw value.

```rust
enum ValueState<T> {
    Ready(T),
    Pending,
    Denied { reason: Option<DenialReason> },
    Error { error: Option<DataError> },
    Missing,
}
```

**Semantic definitions:**

- `Ready(T)` — Value is resolved and available.
- `Pending` — Value is not yet available; may arrive later.
- `Denied` — Value exists but access is forbidden. Reason is advisory.
- `Error` — Read failed. Error is advisory.
- `Missing` — The requested value is absent. For a cell, the row exists but the cell has no value. For an order window, the requested revision/window is no longer available; this is not testimony that the current universe is empty. For `removal_neighbor`, the provider no longer has the former-position testimony required to choose a survivor.

**Rules:**

- UI MUST handle all variants gracefully.
- `Pending` MUST NOT block rendering; UI displays placeholder.
- `Denied` and `Error` are distinct: denied is policy, error is failure.
- `Missing` is distinct from `Ready(None)`: missing means no cell, not null value.

**Transition guarantees:**

- `Pending` may transition to any other state via invalidation.
- `Ready`, `Denied`, `Error`, `Missing` are stable until explicitly invalidated.
- No state may transition without an invalidation event.

---

## 5. Row Universe Contract (Required)

Defines what rows exist, their stable identities, and universe characteristics.

```rust
trait RowUniverse {
    fn identity(&self) -> RowUniverseIdentity;
    fn order_revision(&self) -> OrderRevision;
    fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow>;
    fn locate_row(&self, revision: OrderRevision, row: RowId) -> ValueState<Option<usize>>;
    fn removal_neighbor(
        &self,
        revision: OrderRevision,
        removed_row: RowId,
    ) -> ValueState<Option<RowId>>;
    fn extent(&self) -> Extent;
    fn mode(&self) -> UniverseMode;
}

struct RowUniverseIdentity {
    row_identity_domain_id: RowIdentityDomainId,
    order_universe_id: OrderUniverseId,
}

struct OrderWindowRequest {
    revision: OrderRevision,
    start: usize,
    length: usize,
}

struct OrderWindow {
    revision: OrderRevision,
    start: usize,
    row_ids: Vec<RowId>,
}
```

### 5.1 Row Identity

**Rules:**

- `RowIdentityDomainId` scopes the semantic meaning of every `RowId`.
  Provider/adapter replacement may retain it only when surviving RowIds retain
  exactly the same meaning. A changed identity-domain ID kills every prior RowId.
- `OrderUniverseId` scopes one ordered projection/request-lane lifetime and names
  its `RowIdentityDomainId`. It changes whenever old asynchronous work could
  coexist with a replacement provider, query, or reconstructed lane, even when
  RowId identity remains stable.
- `OrderRevision` contains the current `OrderUniverseId`; all window, locate, and
  removal-neighbor responses are therefore lifetime-scoped without a separate
  ambient provider-generation check.
- `identity().row_identity_domain_id`, the domain embedded in
  `identity().order_universe_id`, and the domain/universe embedded in
  `order_revision()` must match exactly. Any mismatch is invalid provider
  testimony and authorizes no data reuse.
- `RowIdentityDomainId` is a container-lifetime scope, not a field that must be
  duplicated into every selection, focus, edit, cache, or widget key. Inside one
  adapter/substrate instance bound to one current identity domain, `RowId`
  remains the sole per-row key and `(RowId, ColumnId)` remains the sole per-cell
  key as required by the tabular substrate contracts. On identity-domain change,
  the consumer must clear every such RowId-keyed selection, focus, edit, cache,
  and widget state before accepting IDs from the replacement domain. Equal raw
  RowId values across the boundary do not survive by equality.
- Order is provider-defined.
- Replacement of the RowId sequence is a base-order replacement (SPEC-HAL-187).
- No partial reorders.
- `start` and `length` are bounded enumeration coordinates within one revision, not row identity.
- `Ready(OrderWindow)` may contain fewer rows than requested and may be empty at a confirmed finite end. `Pending`, `Denied`, `Error`, and `Missing` must not be interpreted as an empty universe or as permission to reuse rows from another revision.
- `locate_row` resolves stable identity to an ephemeral ordinal in one revision without requiring full order
  materialization; `Pending` is lawful.
- `removal_neighbor` is the sole provider-authoritative focus-recovery seam after
  absence of `removed_row` has been confirmed. Against the supplied current
  revision, it returns the surviving row nearest the removed row's former
  provider position, choosing the successor before the predecessor when both
  are equally near. The provider may retain an internal tombstone/rank to answer;
  no former or current ordinal escapes this operation. `Ready(None)` means no
  survivor exists. `Pending`, `Denied`, `Error`, and `Missing` authorize no local
  fallback.
- A response whose complete revision, including `OrderUniverseId`, no longer
  matches the provider's confirmed revision is stale and MUST be discarded.
  Equal numeric revision sequences in different universes never compare equal.
- Consumers MUST NOT require or accumulate a complete RowId sequence as the canonical path.

**Virtualization note:**
The universe may be infinite, paged, or windowed. Exact extent does not authorize full RowId materialization.

### 5.2 Extent Model (Normative)

```rust
enum Extent {
    Exact(usize),
    LowerBound(usize),
    UpperBound(usize),
    Bounded { lower: usize, upper: usize },
    Unknown,
    Streaming,
}
```

**Semantic definitions:**

- `Exact(n)` — Exactly n rows exist.
- `LowerBound(n)` — At least n rows exist; may be more.
- `UpperBound(n)` — At most n rows exist; may be fewer.
- `Bounded` — Row count is within range.
- `Unknown` — Row count cannot be determined.
- `Streaming` — Rows arrive continuously; no finite count.

**Rules:**

- Virtualization MUST operate correctly with `Unknown` and `Streaming`.
- `Exact` count may change via invalidation; it is not a promise of stability.
- UI MUST NOT assume `Exact` means "all rows are materialized".

### 5.3 Universe Mode (Normative)

```rust
enum UniverseMode {
    Live,
    Snapshot,
}
```

**Semantic definitions:**

- `Live` — Universe may change; emits invalidations.
- `Snapshot` — Universe membership and values are frozen; no external data-origin invalidations will be emitted.

**Rules:**

- A `Snapshot` universe MUST NOT emit data-origin invalidation events. It MAY emit one order-replacement event in
  response to an explicit accepted sort/filter request because the frozen universe and its query projection are
  distinct authorities.
- A `Live` universe MUST emit invalidations for any change.
- Mixed behavior (sometimes emits, sometimes doesn't) is forbidden.
- Mode is fixed for the lifetime of the universe instance.

---

## 6. Data Read Contract (Required)

Defines how cell values are resolved.

```rust
trait CellReader {
    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue>;
}
```

**Rules:**

- Return type is `ValueState`, not raw value.
- No synchronous guarantees.
- No index-based access.
- No caching assumptions.
- `Pending` result means UI should re-query after invalidation.

---

## 7. Mutation Contract (Optional)

Defines how edits are requested.

### 7.1 Single-Shot Mutation (Convenience)

```rust
trait CellMutator {
    fn mutate_cell(
        &self,
        row: RowId,
        column: ColumnId,
        value: CellValue,
    ) -> MutationResult;
}

enum MutationResult {
    Applied,
    Rejected { reason: Option<RejectionReason> },
    Pending,
}
```

**Rules:**

- Mutations are requests, not guarantees.
- `Rejected` is a valid, expected outcome.
- `Pending` means mutation is in flight; await invalidation.
- Side effects are external to Halcyon.

### 7.2 Edit Lifecycle (Normative)

For inline editing, UI requires structured edit sessions.

```rust
trait EditSession {
    fn begin_edit(&self, row: RowId, column: ColumnId) -> EditResult;
    fn propose(&self, token: EditToken, value: CellValue) -> ValidationResult;
    fn commit(&self, token: EditToken) -> CommitResult;
    fn cancel(&self, token: EditToken);
}

enum EditResult {
    Granted { token: EditToken },
    Denied { reason: Option<DenialReason> },
}

enum ValidationResult {
    Valid,
    Invalid { errors: Vec<ValidationError> },
}

enum CommitResult {
    Applied,
    Rejected { reason: Option<RejectionReason> },
    Pending,
}
```

**Semantic definitions:**

- `begin_edit` — Request permission to edit a cell. Returns token if granted.
- `propose` — Submit a candidate value for validation without committing.
- `commit` — Finalize the edit. Backend applies or rejects.
- `cancel` — Abandon the edit session.

**Rules:**

- `EditToken` is opaque and backend-defined.
- Multiple `propose` calls with same token are allowed (iterative editing).
- `commit` without prior `propose` is allowed (backend validates on commit).
- `commit` is idempotent: repeated calls with same token return same result.
- `cancel` is idempotent: repeated calls are no-op.
- After `commit` or `cancel`, the token is invalidated.
- Backend is authority; UI owns no transactional semantics.

**Relationship to single-shot:**

- `mutate_cell` is equivalent to `begin_edit` + `commit` with implicit token.
- Edit lifecycle is the canonical path for inline editing.
- Single-shot is convenience for programmatic mutations.

---

## 8. Capability Contract (Optional)

Defines what operations are permitted for a cell.

```rust
trait CellCapabilities {
    fn capabilities(&self, row: RowId, column: ColumnId) -> Capabilities;
}

struct Capabilities {
    visible: bool,
    readable: bool,
    editable: bool,
    committable: bool,
    redacted: bool,
}
```

**Semantic definitions:**

- `visible` — Cell may be rendered at all. If false, UI should not display.
- `readable` — Cell value may be read. If false, read returns `Denied`.
- `editable` — Cell may enter edit mode. If false, edit UI is suppressed.
- `committable` — Cell edit may be committed now. May be false during validation.
- `redacted` — Cell is visible but content is masked (e.g., password field).

**Rules:**

- Capabilities are advisory to UI behavior.
- Backend enforcement may still occur.
- Capabilities MAY change over time; changes emit
  `PolicyInvalidated { universe, scope }` for the exact current universe.
- `!visible` implies `!readable`, `!editable`, `!committable`.
- `redacted` is orthogonal to readable: value may be read but displayed masked.

**Default behavior:**

If no `CellCapabilities` is provided, all cells are assumed:
- `visible: true`
- `readable: true`
- `editable: false`
- `committable: false`
- `redacted: false`

---

## 9. Invalidation & Change Notification Contract (Required)

Defines how the UI learns that data changed.

```rust
trait DataInvalidation {
    fn subscribe(&self, sink: InvalidationSink);
}
```

### 9.1 Invalidation Event Taxonomy (Normative)

```rust
enum InvalidationEvent {
    OrderProjectionReplaced {
        revision: OrderRevision,
        applied_sort: AppliedSortSnapshot,
        origin: OrderTransitionOrigin,
    },
    BaseOrderReplaced {
        revision: OrderRevision,
        applied_sort: AppliedSortSnapshot,
        origin: OrderTransitionOrigin,
    },
    RowsAdded { universe: OrderUniverseId, rows: Vec<RowId> },
    RowsRemoved { universe: OrderUniverseId, rows: Vec<RowId> },
    CellsInvalidated {
        universe: OrderUniverseId,
        rows: Vec<RowId>,
        columns: ColumnScope,
    },
    PolicyInvalidated { universe: OrderUniverseId, scope: PolicyScope },
    ExtentChanged { universe: OrderUniverseId },
}

enum OrderTransitionOrigin {
    Provider,
    SortRequest(SortRequestId),
}

enum ColumnScope {
    All,
    Specific(Vec<ColumnId>),
}

enum PolicyScope {
    All,
    Rows(Vec<RowId>),
    Columns(Vec<ColumnId>),
    Cells(Vec<(RowId, ColumnId)>),
}
```

**Semantic definitions:**

- `OrderProjectionReplaced { revision, applied_sort, origin }` — The provider-confirmed RowId order revision changed. This is the revision-bearing canonical
  event for SPEC-HAL-353 consumers and MUST trigger one epoch bump.
- `BaseOrderReplaced { revision, applied_sort, origin }` — Compatibility naming for a provider-confirmed
  RowId-order replacement. It carries the exact revision observed at emission
  and MUST trigger the same deduplicated epoch transition as
  `OrderProjectionReplaced` (SPEC-HAL-187 INV-ROW-BASE-REPLACE). A revisionless
  unit variant and delayed `order_revision()` re-query are non-conforming because
  multiple queued transitions cannot then be attributed exactly.
- `RowsAdded { universe, .. }` — New rows exist in the named universe. Optional membership detail; if confirmed order changes, an order-replacement event is
  still required.
- `RowsRemoved { universe, .. }` — Rows no longer exist in the named universe. Optional membership detail; if confirmed order changes, an order-replacement
  event is still required.
- `CellsInvalidated { universe, .. }` — Specific cell values in the named universe have changed. UI must re-read.
- `PolicyInvalidated { universe, .. }` — Capabilities in the named universe have changed. UI must re-query.
- `ExtentChanged { universe }` — Extent testimony in the named universe changed without a known membership/order change, such as a bound or estimate
  refinement. An exact membership change still requires order replacement.

**Rules:**

- Consumers MUST treat invalidation as advisory.
- Every invalidation is lifetime-scoped at emission. Order-replacement variants
  carry an `OrderRevision`, the exact `AppliedSortSnapshot` for that revision,
  and whether the transition came from provider activity or one named
  `SortRequestId`; every other variant carries its exact
  `OrderUniverseId`. The adapter compares that full universe identity before
  dirtying, re-reading, clearing, removing, or otherwise reconciling state. An
  event from a retired universe is discarded completely, even when its numeric
  sequence or raw RowIds equal current values. Ambient `identity()` or
  `order_revision()` re-query must not be used to retrofit an untagged event.
- Consumers MUST re-resolve by RowId after any invalidation.
- Any event that changes provider-confirmed order MUST be represented by exactly one of
  `OrderProjectionReplaced { revision, .. }` or compatibility
  `BaseOrderReplaced { revision, .. }`.
- A provider MUST NOT emit both variants for the same order transition.
- Every replacement event must satisfy
  `applied_sort.order_revision == revision`. `SortRequest(request_id)` must name
  the exact `Applied` request outcome that produced the transition and share its
  complete confirmed snapshot; `Provider` means no sort-request outcome is
  required. A mismatch is invalid testimony and commits nothing.
- Invalidation transport MUST preserve distinct order revisions in provider
  publication order and MUST NOT coalesce R1 and R2 into one revisionless signal.
  Repeated delivery of the same revision is deduplicated by revision identity.
- An `OrderRevision` contains the explicit `OrderUniverseId` plus a sequence
  that is never rebound to a different confirmed projection during that
  universe. The universe identity is non-reused while prior callbacks can reach
  the consumer; sequence allocation is checked and non-reusing within it. A
  numeric representation rejects exhaustion rather than wrapping. This makes
  repeated delivery of one full revision distinguishable from both a later
  transition and a replacement lane that restarts its numeric sequence.
- A change of `OrderUniverseId` is itself an order transition and must publish
  one revision-bearing replacement event. A change of its embedded
  `RowIdentityDomainId` additionally kills all prior RowId identity; optional
  `RowsRemoved { universe, .. }` detail is not required to enumerate that
  complete death set.
- Before publishing a replacement identity domain as consumable, the adapter and
  substrate clear every RowId-keyed selection, focus, edit, window, ordinal,
  binding, and realized-widget state from the retired domain. This is one
  container-lifetime reset, not per-row removal recovery, and raw RowId equality
  across domains authorizes no retention.
- Current-universe `RowsAdded` / `RowsRemoved` may accompany, but never replace, the one required order-replacement event when membership
  changes the confirmed ordered projection.
- Current-universe `CellsInvalidated` with empty rows means "all rows" for specified columns.
- `Snapshot` universes MUST NOT emit data-origin events. They MAY emit an order-replacement event only for an explicit
  accepted query request as defined in §5.3.

**Epoch binding:**

- Every new confirmed `OrderRevision` MUST cause TabularSubstrate to bump `base_row_order_epoch` exactly once.
- Both revision-bearing replacement variants carry the revision that produces
  that one-bump transition; neither consumer nor compatibility bridge re-queries
  a later current revision to infer event identity.
- All cached RowId windows and RowId → ordinal mappings from prior revisions MUST be invalidated on epoch bump.

---

## 10. Sort/Filter Authority Boundary (Normative)

Defines who owns ordering semantics. Sorting is fully governed by SPEC-HAL-353; filtering requires separate semantic
authority but follows the same no-local-order and revision-replacement laws.

### 10.1 OrderControl contract

A sortable universe exposes the conceptual capabilities below through the adapter-owned data contract boundary:

```text
OrderControl {
    order_universe() -> OrderUniverseId
    baseline() -> BaselineOrderId
    sort_capability(SortKey) -> SortCapability
    sort_chain_capability() -> SortChainCapability
    validate_sort_spec(SortSpec) -> SortSpecValidation
    applied_sort() -> AppliedSortSnapshot
    sort_control_snapshot() -> SortControlSnapshot
    request_sort(SortOperation) -> SortRequestOutcome
    cancel_sort(SortRequestId) -> CancelOutcome
}
```

Exact signatures and sync/async transport are implementation-defined. The semantic surface is not optional for a
sortable provider. `RowUniverse` supplies the confirmed `OrderRevision` and bounded RowId windows under §5; §9 supplies
revision replacement notification.

### 10.2 Data-layer authority

The data layer owns:

- base universe: what rows exist;
- named provider baseline and its deterministic tie-break order;
- interpretation and validation of standard/custom sort keys;
- current capability and policy testimony;
- request sequencing, acceptance, rejection, pending state, cancellation, and approximation;
- confirmed applied `SortSpec` and `OrderRevision`;
- bounded RowId windows for that revision.

### 10.3 Substrate authority

The substrate owns mechanics over the confirmed provider projection:

- navigation and focus by RowId;
- selection mechanics by RowId;
- realized/visible window consumption;
- base-row-order epoch invalidation;
- geometry and virtualization state.

It does not own sort execution or a second effective order.

### 10.4 Request flow

1. ListView maps raw input or host commands to a semantic sort operation.
2. DataAdapter forwards the operation to `OrderControl`; cancellation calls the
   distinct `cancel_sort` seam and is not encoded as a `SortOperation`.
3. The data layer resolves the operation against its newest provider-owned
   effective pending target, or the confirmed snapshot when no request is
   pending, and reports one complete post-operation `SortControlSnapshot` with
   the outcome. Immediate rejection/unsupported testimony does not supersede an
   already effective pending request. A later terminal rejection of the current
   pending request clears it; superseded requests never resume. Cancellation of
   an already terminal request reports that terminal disposition only after its
   terminal `SortRequestOutcome`, or its first successful `CancelOutcome`, has
   been delivered through the same serialized lane.
4. If a sort request changes confirmed order, the data layer sends its `Applied`
   outcome and exactly one matching `SortRequest(request_id)` revision event
   through the serialized adapter lane. The event contains the exact applied
   snapshot. The adapter stages whichever testimony arrives first and rejects a
   mismatched request ID, revision, universe, or confirmed snapshot. A
   provider-originated event carries `Provider` and is complete without a request
   outcome.
5. Once a request-origin pair is complete, or one valid provider-origin event is
   available, the adapter
   performs one non-interleavable confirmed-order adoption: invalidate old
   windows, bump the substrate epoch once, install the new confirmed sort
   projection, and request needed windows for that revision. Until this commit,
   the prior confirmed header/body projection remains authoritative. After it,
   old-revision rows may not remain presented under the new header; missing new
   windows project pending/empty state rather than old rows.
6. An `Applied` outcome whose confirmed `OrderRevision` equals the already
   adopted revision may update confirmed sort testimony without another
   invalidation or epoch bump. ListView otherwise renders only atomically adopted
   confirmed state plus separately projected pending state.

### 10.5 Rules

- Sort/filter requests are requests, not commands.
- One provider-local serialized request lane owns sort submission order,
  pending-target derivation, supersession, and cancellation under SPEC-HAL-353.
  Rapid relative operations must not be independently resolved against the same
  stale confirmed snapshot.
- `SortRequestId` contains the lane's `OrderUniverseId`. Replacing that universe
  atomically retires pending work and causes every old callback/cancellation to
  fail the full identity check before it reaches current lane or persistence
  state.
- The data layer may execute server-side, client-side, or hybrid without changing UI authority.
- Approximation is lawful only when declared by capability and confirmed applied testimony.
- Reinterpretation or partial application MUST return the actual applied state; requested state is never assumed.
- An invalid persisted multi-column sort request is rejected as a unit under SPEC-HAL-353.
- Any accepted change affecting confirmed order MUST publish a new `OrderRevision` and exactly one replacement event.
- A new confirmed sort snapshot whose revision differs from the adapter's adopted
  revision MUST NOT be exposed to header, persistence, or host projection before
  the matching replacement event commits the confirmed-order adoption above.
  Provider-originated order changes use the event's embedded applied snapshot;
  re-querying a later current snapshot is forbidden because distinct queued
  revisions must remain independently adoptable in publication order.
- Neither ListView nor the substrate may assume a request applied before provider confirmation.

### 10.6 Forbidden

- TabularSubstrate or ListView reordering provider rows through `RowComparator` or another local algorithm.
- UI caching sorted indices across revisions.
- Rebuilding a full RowId universe as the required response to an order revision.
- Dual authority: provider order plus independently computed local order.
- Persisting confirmed order, order revision, or RowId windows as a sort preference.
- Comparing or deduplicating revisions/request IDs by numeric sequence while
  ignoring their `OrderUniverseId`.

---

## 11. Error Containment Rules (Normative)

Defines stability guarantees when failures occur.

### 11.1 Read Failures

When `read_cell` returns `Error`:

- Other cells remain readable.
- Focus remains stable (RowId-based, not index-based).
- Selection remains stable.
- UI displays error state for affected cell only.

### 11.2 Policy Changes Mid-Focus

When capabilities change for focused cell:

- If `!editable` during edit: cancel edit, remain focused.
- If `!visible`: move focus to nearest visible row (by RowId traversal).
- If `!readable`: display denied state, remain focused.

### 11.3 Row Disappears

When any row is confirmed removed:

- An edit session on that row is implicitly cancelled; unrelated edit sessions are unchanged.
- Selection removes the disappeared RowId immediately.
- If it was focused, focus is first cleared, then the consumer calls
  `removal_neighbor(current_revision, removed_row)`. Only
  `Ready(Some(surviving_row_id))` may establish replacement focus, after that
  identity is revalidated against the still-current revision.
- `Ready(None)`, `Pending`, `Denied`, `Error`, `Missing`, or a stale-revision
  response leaves focus empty. Pending recovery may be retried after provider
  invalidation; it must not retain the removed focus target meanwhile.
- UI MUST NOT crash or enter undefined state.

### 11.4 Backend Rejects Commit

When `commit` returns `Rejected`:

- Edit session remains active (user may retry or cancel).
- Cell displays rejection feedback.
- No data is modified.
- Focus remains on cell.

### 11.5 General Rules

- All error handling is RowId-based.
- Index-based recovery is forbidden.
- Local neighbor scans and cached former ordinals are forbidden removal-recovery
  authority.
- UI MUST NOT assume contiguous indices after error.
- Partial failures do not corrupt unaffected state.

---

## 12. ListView / TabularSubstrate Integration Rules

**ListView:**

- consumes projections only
- never touches data contracts directly
- emits semantic sort operations to the adapter boundary and renders confirmed sort testimony

**TabularSubstrate:**

- consumes provider-confirmed order windows
- owns navigation and selection mechanics over that projection
- bridges RowId → visible projections
- handles invalidation events
- manages epoch

**Data contracts:**

- never receive indices as row identity or cell-access keys
- may receive bounded order-window ordinals tied to one explicit revision
- never see geometry
- never see viewport

---

## 13. Virtualization Compatibility (Explicit)

All contracts in this SPEC are virtualization-compatible by design.

**Virtualization:**

- operates on visible RowId windows
- requests bounded windows by `OrderRevision` and ephemeral ordinal range
- resolves a focused/anchored RowId to a revision-scoped ordinal through `locate_row` when neighbor/range access requires
  it
- requests `removal_neighbor` when provider-confirmed identity removal requires
  focus recovery, without materializing or retaining the surrounding order
- never persists RowId ↔ ordinal mappings across revisions
- re-queries data lazily per frame
- handles `Unknown` and `Streaming` extents
- does not require a full RowId universe for exact extent, sorting, or replacement invalidation

Any implementation that violates this is non-conforming.

---

## 14. Forbidden Patterns (Binding)

The following are architecturally illegal:

- `get_row(i)`
- `read_cell_at_index(i, j)`
- `mutate_row_at(i)`
- storing row indices in adapters as row identity or semantic mappings
- caching visible index lists across epochs
- caching or reconstructing a complete ordered RowId universe as the canonical windowing mechanism
- using a window ordinal without its `OrderRevision`
- accepting a window, locate/removal response, sort outcome, or cancellation
  from a retired `OrderUniverseId`
- assuming `Exact` extent means materialized
- mixed Live/Snapshot behavior

`OrderWindowRequest { revision, start, length }` and the ordinal returned by `locate_row(revision, RowId)` are the sole
ordinal exceptions. They enumerate or locate within one order projection; neither identifies a row independently,
authorizes cell access by index, or survives a revision change.

These violations reintroduce PM-001.

---

## 15. Test Oracle Requirement (Normative)

Implementations MUST be testable against a reference oracle.

### 15.1 Baseline Oracle

A canonical in-memory implementation that:

- Satisfies all contracts in this SPEC.
- Uses synchronous, in-memory data.
- Has no permissions (all capabilities true).
- Has no failures (no `Error` or `Denied` states).
- Provides `Exact` extent.
- Operates in `Live` mode.

This oracle is the behavior reference for all tests.

### 15.2 Failure Oracle (Optional)

A secondary oracle that:

- Injects `Pending`, `Error`, `Denied` states on demand.
- Simulates policy changes mid-operation.
- Simulates row removal during edit.
- Simulates commit rejection.

Used for error-path testing.

### 15.3 Rules

- All UI components MUST pass tests using baseline oracle.
- Error-path tests use failure oracle.
- Oracles are not production code; they are test infrastructure.
- Oracle behavior defines correct behavior for ambiguous cases.

---

## 16. Acceptance Criteria

This SPEC is satisfied when it contains explicit normative definitions for:

- ValueState model (§4)
- Invalidation event taxonomy including self-contained revision-bearing
  BaseOrderReplaced → epoch testimony and universe-scoped non-order events (§9)
- OrderControl contract including baseline, per-key/chain capability, whole-spec
  validation, complete lane/cancellation outcomes, and confirmed snapshot (§10), composed with
  explicit row-identity/order-universe lifetime, revision/window,
  identity-location, and removal-recovery access (§5)
- Capability model: visibility, readability, editability, committability, redaction (§8)
- Edit lifecycle: begin_edit, propose, commit, cancel (§7.2)
- Sort/filter authority boundary (§10)
- Extent model: Exact, LowerBound, UpperBound, Unknown, Streaming (§5.2)
- Snapshot vs Live semantics (§5.3)
- Error containment rules (§11)
- Test oracle requirement (§15)

And when:

- All data access is RowId-based
- RowId remains the sole internal row key within one bound
  `RowIdentityDomainId`, while replacement of that domain atomically clears all
  RowId-keyed consumer state before repeated raw IDs can be accepted
- Order enumeration ordinals are bounded and revision-scoped; all semantic row access remains RowId-based
- Retired-universe invalidations cannot dirty or reconcile current state, and a
  confirmed sort/header snapshot cannot precede adoption of its exact body
  revision
- Read, write, capability, invalidation are separable
- No API requires full materialization
- ListView remains a pure consumer
- Virtualization works without special cases

---

## 17. Strategic Note (Non-Normative)

This SPEC exists to prevent data assumptions from leaking into UI code.

Once violated, the system becomes untestable, unscalable, and hostile to real backends.

There will be no retrofit later.
