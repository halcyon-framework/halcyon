# SPEC-HAL-353: Provider-Authoritative ListView Sorting and Order Projection

## Pre-Flight

- SPEC: SPEC-HAL-353, read before writing this task: YES
- Policy: Interactive
- Interaction added: YES — header gestures and commands emit sort requests; only the data layer may decide and publish row order
- Forbidden behaviors checked: FORBID-353-LOCAL-ORDER, FORBID-353-DUAL-AUTHORITY, FORBID-353-INDEX-ORDER, FORBID-353-FULL-MATERIALIZATION, FORBID-353-OPTIMISTIC-TRUTH, FORBID-353-PERSISTED-TRUTH, FORBID-353-MANUAL-AS-SORT
- Existing infrastructure: SPEC-HAL-171 sort intents and header projection, SPEC-HAL-187 base-order epoch law, SPEC-HAL-188 DataSource/DataAdapter boundary and invalidation path, SPEC-HAL-261-A1 RowTopologyEngine freeze, ListViewCore base-order replacement, ViewConfigStore, ColumnConfigurationState, AppState/WidgetState doctrine, NavigationHost surface scope
- Reinvention risk: Extends the existing adapter and invalidation boundary; it does not create a second row-order owner, local comparator path, navigation store, or persistence backend

**Status:** Binding
**Layer:** ListView interaction / data-query authority / ordered row projection
**Scope:** ListView and every SPEC-HAL-188-backed tabular consumer that exposes column sorting
**Extends:** SPEC-HAL-164, SPEC-HAL-171, SPEC-HAL-179, SPEC-HAL-185, SPEC-HAL-187, SPEC-HAL-188
**Supersedes:** SPEC-HAL-179A §4.2 comparator-driven fulfillment, SPEC-HAL-261 §§2/4–6, and SPEC-HAL-261-A1 §§2–5 local sort/index-topology requirements for ListView and other SPEC-HAL-188-backed consumers; SPEC-HAL-185 single-key and sort-mode persistence clauses
**Integrates with:** SPEC-HAL-007, SPEC-HAL-174A, SPEC-HAL-188A, SPEC-HAL-196, SPEC-HAL-229, SPEC-HAL-261-A1
**Non-Goals:** Filtering semantics, storage backend implementation, sort-indicator styling, manual row-reorder implementation, tree sibling-order rules, migration compatibility for the current full-RowId-slice DataSource API

---

## 1. Authority Decision

Sorting in a data-backed ListView is a data-layer query operation. It is not a local permutation of rows already held by
the widget or substrate.

The authority chain is:

```text
raw input
    -> ListView semantic sort operation
    -> DataAdapter request boundary
    -> data-layer decision and execution
    -> confirmed applied sort snapshot + order revision
    -> bounded RowId projection windows
    -> substrate navigation/selection over that confirmed projection
    -> ListView header/body presentation
```

- The **data layer** owns baseline order, sort-key meaning, request acceptance, execution, the confirmed applied
  `SortSpec`, and the resulting order revision.
- The **DataAdapter** is the only data-contract client. It forwards requests and translates confirmed provider changes
  into substrate invalidation.
- The **substrate** owns navigation, focus, selection, and virtualization mechanics over the current confirmed RowId
  projection. It does not compute sorted order for a SPEC-HAL-188-backed consumer.
- **ListView** maps raw gestures and commands to semantic sort operations and renders confirmed/pending presentation
  state. It owns no row-order semantics.
- The **header** is an intent and projection surface only.

**INV-353-AUTH-1:** A SPEC-HAL-188-backed ListView has exactly one row-order authority: its data layer.

**INV-353-AUTH-2:** `RowComparator`, locally sorted indices, and substrate-computed row permutations are non-conforming
fulfillment paths for ListView.

**INV-353-AUTH-3:** Provider-driven sorting is required even when the provider happens to be synchronous and in memory.
An in-memory provider may implement the request locally; the widget still must not acquire order authority.

---

## 2. Definitions

### 2.1 Provider baseline

A **provider baseline** is a provider-owned deterministic order identified by a stable `BaselineOrderId`.

Examples include:

- domain default order;
- server query default order;
- user-managed manual rank;
- creation sequence;
- another explicitly named provider order.

The baseline is not a RowId vector captured when ListView starts. It may evolve as rows are added, removed, or manually
ranked by domain operations.

### 2.2 Sort key

A sort term addresses one stable provider-recognized key:

```text
SortKey = Column(ColumnId) | Custom(SortKeyId)
```

- `Column(ColumnId)` requests the provider's declared sort semantics for that column.
- `Custom(SortKeyId)` requests a provider-owned domain strategy that is not represented by direct display-value
  comparison.

### 2.3 Sort term and SortSpec

```text
SortTerm {
    key: SortKey,
    direction: Ascending | Descending,
    parameters: serializable provider-defined values
}

SortSpec {
    baseline: BaselineOrderId,
    terms: ordered list of SortTerm
}
```

The first term has priority 1. Terms that follow are deterministic tie-breakers in list order. A key may appear at most
once.

An empty `terms` list means "use the named provider baseline." It does not mean "restore a cached startup sequence" or
"accept unspecified backend order."

When all explicit terms compare equal, the named baseline is the final stable tie-breaker.

### 2.4 Applied sort snapshot

An **applied sort snapshot** is provider testimony, not widget state inferred from a request:

```text
AppliedSortSnapshot {
    spec: SortSpec,
    exactness: Exact | provider-described Approximate,
    order_revision: OrderRevision
}
```

If the provider normalizes, rejects part of, or approximates a request, the snapshot must describe what is actually
applied. Requested state must never be presented as applied state.

### 2.5 Order revision and order window

Two explicit provider identities scope row and order testimony:

```text
RowIdentityDomainId
OrderUniverseId {
    row_identity_domain_id: RowIdentityDomainId,
    lifetime: checked non-reused provider-local identity,
}

OrderRevision {
    order_universe_id: OrderUniverseId,
    sequence: checked non-reused revision within that universe,
}
```

`RowIdentityDomainId` defines the semantic namespace in which a `RowId` keeps
its meaning. Provider/adapter replacement may retain it only when every surviving
`RowId` still denotes the same semantic row. Changing it is authoritative
identity-domain replacement and kills every prior RowId identity.

`OrderUniverseId` defines one serialized ordered-universe/request-lane lifetime.
It is newly allocated whenever provider replacement, query replacement, or lane
reconstruction can allow an old asynchronous response to coexist with new work.
It is never reused while a response from an earlier universe can reach the same
consumer. Numeric allocation is checked and rejects exhaustion rather than
wrapping. A new order universe may retain the same `RowIdentityDomainId`.

An `OrderRevision` identifies one provider-confirmed ordered RowId projection
inside that explicit order universe. Its sequence is never rebound during the
universe lifetime. Publication and invalidation transport preserve distinct full
revisions in provider publication order. Only repeated delivery of the identical
`(OrderUniverseId, sequence)` may be deduplicated.

An **order window** is a bounded sequence of RowIds for an ordinal range within one explicit revision. Its ordinal range
is a virtualization transport coordinate, not row identity and not a stable public API.

```text
OrderWindowRequest {
    revision: OrderRevision,
    start: ephemeral ordinal,
    length: bounded length
}

OrderWindow {
    revision: OrderRevision,
    start: ephemeral ordinal,
    row_ids: bounded RowId sequence
}

order_window(OrderWindowRequest) -> ValueState<OrderWindow>
locate_row(revision, RowId) -> ValueState<optional ephemeral ordinal>
removal_neighbor(revision, removed RowId) -> ValueState<optional surviving RowId>
```

Windows from different revisions or order universes must never be merged. A
response whose complete `OrderRevision` does not match the consumer's current
revision is stale and is discarded. A numerically equal sequence from another
`OrderUniverseId` is not equal testimony.
`Ready(OrderWindow)` may be shorter than requested or empty at a confirmed finite end. `Pending`, `Denied`, `Error`,
and `Missing` do not testify to an empty order and never authorize retention of rows from another revision.
`LocateRow` exists so identity-sticky focus, reveal, and range mechanics can find the current provider position without a
complete RowId map.
`removal_neighbor` is the provider-authoritative, identity-returning recovery
operation defined by SPEC-HAL-188; it never exposes the removed row's former
ordinal.

---

## 3. Sort Request Contract

### 3.1 Semantic operations

ListView emits semantic operations rather than a locally assembled authoritative order:

```text
CycleReplace(SortKey)
CycleAdditive(SortKey)
SetDirection(SortKey, direction, Replace | Additive)
RemoveTerm(SortKey)
ClearToBaseline(BaselineOrderId)
ApplySpec(SortSpec)
```

Cancellation uses the separate `cancel_sort(SortRequestId)` seam in
SPEC-HAL-188; it is not a second `SortOperation` accepted by `request_sort`.

The data layer owns one serialized request lane per ordered universe. During
submission, before deciding acceptance, it resolves each relative operation against its provider-owned intent
base: the newest effective pending `requested_spec`, if one exists, otherwise
the latest confirmed applied snapshot. It immediately lowers that operation to
one absolute requested `SortSpec`. ListView never computes or stores this chain.

`ApplySpec` exists for restored preferences, host commands, and programmatic query construction. It does not authorize
ListView to declare the spec applied.

### 3.2 Lane testimony and outcomes

Every request has a stable `SortRequestId` composed from the current
`OrderUniverseId` plus a checked, non-reused submission sequence in that
universe. Its provider-local order is the submission order. Every immediate or
later outcome carries the complete post-operation lane testimony:

```text
PendingSortSnapshot {
    request_id: SortRequestId,
    requested_spec: SortSpec,
}

SortControlSnapshot {
    confirmed: AppliedSortSnapshot,
    effective_pending: Option<PendingSortSnapshot>,
}

SortRequestOutcome {
    request_id: SortRequestId,
    requested_spec: SortSpec,
    disposition: Applied | Pending | Rejected(reason) | Unsupported | Stale,
    state: SortControlSnapshot,
}
```

- `Applied` is accepted, installs the actual provider-normalized confirmed
  snapshot, clears the effective pending slot, and makes every older unconfirmed
  completion stale.
- `Pending` is accepted and installs exactly its `(request_id, requested_spec)`
  as the sole effective pending target while leaving `state.confirmed`
  authoritative.
- An immediate `Rejected` or `Unsupported` request is not accepted and does not
  replace an already effective pending target. Its `state` reports that
  unchanged target.
- A later `Rejected`/`Unsupported` resolution for the current effective pending
  request clears that slot and retains the confirmed snapshot. No superseded
  request resumes.
- A completion for a superseded request has disposition `Stale` and reports the
  unchanged current lane state.
- A pending request may later resolve through a second outcome with the same
  request ID. Any applied order change also follows the revision invalidation
  path.

A newly accepted `Pending` or `Applied` request supersedes every older
unconfirmed request. A rejected or unsupported submission does not. A newly
accepted pending request becomes the sole effective pending target for
presentation and publication.
The provider may continue obsolete backend work, but an older completion is
stale and cannot publish applied state. Because each new relative operation is
resolved against the prior effective pending target, rapid additive input is
deterministic: from the baseline, pending Shift+A followed by Shift+B produces
the absolute pending chain `[A, B]`, regardless of backend completion order. The
newer absolute request is validated as a complete spec and may apply even if the
superseded request fails independently.

Replacing the `OrderUniverseId` atomically retires the old request lane, clears
its effective pending slot, and publishes a confirmed snapshot whose
`OrderRevision` names the new universe. Every old window, locate/removal response,
sort outcome, cancellation, and invalidation remains tagged with the old
universe and is discarded at the adapter boundary; it cannot be reclassified as
current merely because a sequence value is reused. Runtime persistence-origin
records for the retired universe are dropped without a write. If the persisted
preference still applies to the new data/query scope, restoration is a new
`ApplySpec` request with a new `SortRequestId`.

**INV-353-REQUEST-1:** A request is not proof of application.

**INV-353-REQUEST-2:** ListView may display pending feedback, but its direction arrows and priority badges must continue
to represent the confirmed applied snapshot.

**INV-353-REQUEST-3:** Supersession and cancellation are provider-owned request-lane operations. ListView must not fabricate sequencing, a pending target chain, or cancellation by rolling back local state.

**INV-353-REQUEST-4:** ListView projects pending feedback only from
`SortControlSnapshot::effective_pending`. It must not reconstruct lane history
from callbacks or assume that the newest submitted request superseded an older
pending request.

### 3.3 Cancellation

Successful cancellation applies only to the current effective pending
`SortRequestId`; attempts against other issued/current-universe or unknown IDs
return the non-mutating dispositions below.

```text
CancelOutcome {
    request_id: SortRequestId,
    disposition:
        Cancelled
        | NotCurrent
        | AlreadyResolved(Applied | Rejected | Unsupported | Stale | Cancelled)
        | Unsupported
        | Unknown,
    state: SortControlSnapshot,
}
```

- Every cancellation result carries the complete post-operation lane testimony.
- If cancellation succeeds, disposition is `Cancelled`; the provider clears the complete unconfirmed lane,
  marks every older in-flight completion stale, and the last confirmed applied
  snapshot remains active.
- Cancelling an older superseded request returns `NotCurrent` and
  cannot alter the current effective pending target.
- An issued request that has already reached any terminal disposition returns
  `AlreadyResolved(terminal_disposition)`, including immediate/later rejection,
  unsupported, stale, applied, and an earlier successful cancellation. The
  serialized provider lane must deliver/process that request's terminal lane
  result—its terminal `SortRequestOutcome`, or its first successful
  `CancelOutcome`—before exposing `AlreadyResolved`. The repeated cancellation
  result is read-only confirmation and cannot recreate a retired
  persistence-origin record or apply the terminal transition twice. If terminal resolution races
  cancellation, the lane linearizes exactly one first: resolution followed by
  `AlreadyResolved`, or successful `Cancelled`; both cannot win.
- `Unsupported` applies only when the named request is the current effective
  pending request but the provider cannot cancel it; lane state is unchanged.
- An ID not issued by the current `OrderUniverseId`, including any ID from a
  retired universe, returns `Unknown` without mutation. It is not treated as a
  current, superseded, or resolved request in the current lane.
- Returning to a prior applied order requires a new sort request.

### 3.4 Clear and reset

These operations are distinct:

- **Clear sorting** submits `ClearToBaseline` and waits for provider confirmation.
- **Cancel pending** targets one in-flight request and does not change confirmed sorting.
- **Reset persisted preference** removes the saved requested `SortSpec`, then submits `ClearToBaseline`.
- **Reset all ListView presentation preferences** may additionally reset column presentation state under
  SPEC-HAL-185, but it must not bypass the data-layer sort request.

No operation restores a ListView-cached "original order."

---

## 4. Canonical Multi-Column Interaction

### 4.1 Plain activation

Pointer click, tap, and unmodified keyboard activation map to `CycleReplace(key)`.

Against the provider-owned intent base defined in Section 3.1:

1. If the key is absent, request a one-term ascending chain.
2. If the key is ascending, request a one-term descending chain.
3. If the key is descending, request the named baseline with no explicit terms.

Any other active terms are removed by this replace operation.

### 4.2 Additive activation

Shift-modified pointer or keyboard activation maps to `CycleAdditive(key)`:

1. If the key is absent, append it ascending at the lowest priority.
2. If the key is ascending, retain its priority and request descending.
3. If the key is descending, remove it.
4. Removing a term closes the priority gap without changing the relative order of surviving terms.
5. Removing the last term requests the provider baseline.

The host may expose explicit menu commands for ascending, descending, removing one key, and clearing all keys. Those
commands map to the same semantic request boundary.

### 4.3 Priority presentation

- The header may display direction and 1-based priority for every visible active sort key.
- The header must not maintain a private sort chain.
- A hidden active sort key remains in the chain. It has no header indicator while hidden; its indicator returns if the
  column becomes visible again.
- Hiding a column must not emit a sort request.

**INV-353-MULTI-1:** Multi-column priority is the ordered `SortSpec`, not header-local state.

**INV-353-MULTI-2:** The same semantic operations must be available to pointer and keyboard users.

---

## 5. Capability and Policy Contract

### 5.1 Effective sortability

A column is user-sortable only when both conditions hold:

1. the view/host exposes sorting for that column; and
2. the data layer currently advertises a permitted sort capability for its key.

The view flag controls affordance exposure. The provider capability controls semantic support and policy. Neither may
substitute for the other.

### 5.2 Provider capability testimony

Capability testimony has both per-key and complete-chain forms:

```text
SortChainCapability {
    additive_supported: bool,
    max_terms: positive bounded integer,
}

validate_sort_spec(SortSpec) -> Valid | Invalid(reason) | Pending
```

`max_terms` bounds the complete explicit term chain. `additive_supported ==
false` means replace operations may still be lawful, but additive gestures and
commands are unavailable. The whole-spec validation seam is advisory preflight
for programmatic/restored absolute specs; `request_sort` remains the final
authority because policy may change between validation and submission.

For every exposed key, provider capability must declare enough information to determine:

- supported directions;
- exact or approximate execution;
- supported custom strategy ID and parameter shape, when applicable;
- ordering of null, missing, denied, error, and NaN values where those states are possible;
- collation, case, locale, natural-number, and timezone behavior where relevant;
- whether the capability is currently allowed for the active principal/query;
- whether sorting remains meaningful for live or streaming data.

Capabilities may change. A capability invalidation requires re-querying capability testimony and revalidating future
requests. It does not silently rewrite the confirmed applied spec; any forced provider fallback must arrive as a new
confirmed snapshot/order revision.

ListView may expose additive affordance only when the provider's chain
capability permits it. At maximum term count, additive activation of an absent
key is unavailable, while an existing term remains available for its lawful
direction toggle or removal because those operations do not increase the chain.
Cross-key compatibility is never inferred from per-key flags: the provider
validates each complete absolute candidate and may still reject it atomically.

### 5.3 Sensitive and display-only values

- A denied or redacted value is not sortable by default. The provider may expose a non-leaking sort capability only
  when application policy explicitly permits the resulting ordering disclosure.
- A computed or display-only column is sortable only if the provider exposes a real `Column` or `Custom` sort key for
  it.
- ListView must never read visible cell text and compare it as a substitute for missing provider capability.

### 5.4 Approximate sorting

A provider may apply approximate sorting for streaming, distributed, or bounded-window data only when:

- the capability declares that exact sorting is unavailable or optional;
- the applied snapshot reports `Approximate` and identifies the guarantee in provider-defined serializable testimony;
- ListView can distinguish approximate from exact applied state without inferring it from data.

Silently presenting approximate order as exact is forbidden.

---

## 6. Custom Sorting

Custom sorting is provider-owned strategy selection, not a callback into ListView or the substrate.

- `SortKeyId` must be stable within the persistence scope in which it is saved.
- Parameters must be serializable and provider-validated.
- The provider maps the ID and parameters to server queries, database collation, domain ranking, or another
  owner-correct implementation.
- Unsupported IDs or parameters reject the whole requested `SortSpec`; the UI must not partially reconstruct an
  alternative chain.
- Closures, function pointers, trait objects, and process-local comparator identities must not be persisted.

**INV-353-CUSTOM-1:** A custom sort has the same request/outcome/order-revision contract as a column sort.

**INV-353-CUSTOM-2:** A custom strategy may not inspect widget geometry, viewport, realization, focus, or selection.

---

## 7. Order Projection and Large-Data Law

Provider-side execution alone is insufficient if every order change requires full RowId materialization.

### 7.1 Revision replacement

When accepted sorting changes the provider-confirmed order:

1. the provider sends the new `OrderRevision`, applied sort snapshot, and exactly
   one self-contained matching replacement invalidation through the serialized
   adapter lane. A request-origin event names its `SortRequestId`; a
   provider-origin event explicitly names no request;
2. the adapter stages incomplete testimony and atomically adopts the transition
   only when the applied snapshot and invalidation name the same current full
   revision;
3. that adoption invalidates every cached prior-revision window, bumps
   `base_row_order_epoch` exactly once, and exposes the new confirmed header/body
   projection as one state change;
4. virtualization requests only the bounded RowId windows it needs and projects
   pending/empty state rather than prior-revision rows until they arrive;
5. within an unchanged `RowIdentityDomainId`, focus and selection remain
   identity-based and are reconciled by `RowId`.

The adapter must not expose a new confirmed sort snapshot to ListView, host
projection, or persistence while its `order_revision` differs from the adopted
body revision. Whichever half arrives first remains staged and invisible. A
mismatched request ID/revision/snapshot pair is invalid provider testimony. A
provider-origin event is complete by itself because it embeds the exact applied
snapshot for that revision; the adapter must not re-query a later current
snapshot. An `Applied` outcome that retains
the already adopted revision may change normalized/exactness testimony without a
replacement invalidation or epoch bump because the ordered RowId projection did
not change.

`RowIdentityDomainId` is the adapter/substrate container lifetime; it is not
duplicated into every focus or selection entry. If an order replacement changes
that domain, every RowId-keyed focus, selection, edit, cache, binding, and
realized-widget state from the retired domain is cleared before the replacement
domain becomes consumable. Equal raw RowId values across domains do not survive
that boundary.

`BaseOrderReplaced { revision, .. }` remains the revision-bearing compatibility
meaning for "the provider order revision changed." It does not authorize or
require a full replacement vector at the widget boundary, and a revisionless
compatibility event is not lawful for this contract.

### 7.2 Window constraints

- Window size must be bounded independently of total extent.
- Exact extent does not imply full materialization.
- Unknown and streaming extents remain lawful.
- An ordinal is usable only as bounded window address/cache metadata under one explicit revision; it is never semantic
  row state.
- Cell reads and mutations continue to use `RowId`; an ordinal must never reach those contracts.
- Identity-based navigation/selection may resolve a RowId to an ordinal through `LocateRow`, then enumerate only the
  bounded neighboring/range windows it needs.
- Adapters may cache bounded windows by revision and range. They must not accumulate an unbounded complete order as the
  default mechanism.

### 7.3 Current incompatible path

The current `DataSource::row_ids() -> &[RowId]`, adapter-owned complete `Vec<RowId>`, ListView-owned complete
`Vec<RowId>`, and whole-topology rebuild path are not conforming implementations of this section for a large, remote,
unknown, or streaming universe. Implementation work governed by this SPEC must replace that path with revision-scoped
bounded order access; it must not preserve full materialization as the canonical API for compatibility convenience. The
live revisionless `BaseOrderReplaced` unit event is likewise compatibility testimony only and must migrate to one of the
revision-bearing replacement variants in SPEC-HAL-188 before claiming exact per-revision invalidation.

**INV-353-SCALE-1:** Sort memory and invalidation cost at the presentation boundary must be bounded by active cached
windows and realized state, not total row count.

**INV-353-SCALE-2:** A million-row provider must not require a million-RowId widget snapshot to apply or display a
sort.

Concrete row/cell widget residency is likewise bounded by the visible window,
declared overscan, interaction pins, and an explicit finite realization cache
under SPEC-HAL-189. Stable RowId identity does not authorize one retained widget
instance per provider row.

---

## 8. Editing, Selection, Focus, and Live Changes

### 8.1 Active editing

User-originated sort requests must not begin while inline editing is active. The interaction may be unavailable or
explicitly rejected until the edit commits or cancels; ListView must not maintain a hidden deferred-sort queue.

Provider order changes caused externally remain data invalidations and, while
`RowIdentityDomainId` is unchanged, are reconciled by `RowId`. They must not
commit, cancel, or retarget an edit by index. Existing edit-session authority
decides whether an externally removed or denied cell cancels the edit. A changed
identity domain retires all old-domain RowId-keyed state under §7.1 rather than
running per-row removal recovery.

### 8.2 Selection and focus

- Sorting must not clear selection implicitly.
- Within one unchanged identity domain, focus and selection membership remain
  attached to surviving RowIds.
- No cached index, anchor ordinal, or realized slot may survive an order revision as semantic state.
- If a focused/selected RowId is no longer in the provider universe, existing removal reconciliation applies.
- Removal reconciliation is provider-authoritative under SPEC-HAL-188:
  selection drops the confirmed-missing identity immediately; focus clears the
  missing identity and may move only to a `Ready(Some(RowId))` result from
  `removal_neighbor` for the still-current revision. No cached ordinal or local
  neighbor scan is a fallback.
- If `LocateRow` or a required order window is pending, a navigation or range-selection intent is not accepted yet:
  focus/selection and their versions remain unchanged, and the intent may be retried after provider invalidation.

### 8.3 Live providers

A live provider may publish later revisions because values, capabilities, membership, or baseline rank changed. Every
revision follows the same invalidation and window-requery law. ListView must not patch local indices to imitate the
provider's new order.

---

## 9. Session State and Persistence

### 9.1 Ownership split

Sorting has three distinct state forms:

1. **Confirmed runtime state** — the data layer's `AppliedSortSnapshot`; authoritative.
2. **Session preference** — an app-owned requested `SortSpec` keyed to a stable view/data scope; survives screen
   destruction and recreation while the app session lives.
3. **Durable preference** — the same serializable requested `SortSpec` stored through an app-provided durable backend;
   may survive process restart.

Only the second and third forms are persistence inputs. Neither becomes applied truth until accepted by the data layer.

### 9.2 Existing state mechanisms

- Retained `WidgetState` survives rebuilds only while widget identity remains alive. It is not the cross-screen or
  restart owner for sorting.
- `NavigationHost` owns navigation state for one surface. It must not be used as a general view-state registry.
- App-owned session/view-model state is the correct owner for preferences that must survive navigation across destroyed
  screens.
- `ViewConfigStore` may be used as the storage seam for a serializable sort preference when the host supplies a stable
  key and backend. The trait itself does not guarantee durable storage.

### 9.3 Restore timing and validation

- A restored `SortSpec` must be supplied to the query/data-layer controller before ListView consumes its initial ordered
  projection.
- If the request is pending, the consumer may show a pending/empty projection or the last confirmed provider snapshot,
  but it must not label requested sorting as applied.
- The provider validates the entire baseline, key chain, directions, custom IDs, parameters, and current policy.
- If any part is invalid or unsupported, the restored spec is rejected as a unit and the provider baseline remains
  authoritative.
- A later provider response changes order only through the normal confirmed revision path; persistence code must not
  mutate live row order directly.

### 9.4 Preference write policy

The persisted value is the next requested preference, never a record of applied
outcome. Session and durable stores follow the same deterministic transitions:

- except for the reset-origin suppression below, an accepted `Pending` outcome writes its provider-resolved absolute
  `state.effective_pending.requested_spec`;
- except for that suppression, an accepted `Applied` outcome writes only after
  its matching order revision is adopted (or when that revision was already
  adopted), and writes only the serializable normalized `state.confirmed.spec`,
  excluding exactness, revision, request ID, and outcome;
- except for restore-origin `Rejected`/`Unsupported` and reset-origin suppression,
  an immediate `Rejected`, `Unsupported`, or `Stale` outcome does not overwrite
  the existing preference or the preference for an older still-effective pending
  request;
- except for restore-origin `Rejected`/`Unsupported` or reset-origin suppression,
  later rejection/unsupported resolution of the current effective pending
  request, or successful cancellation of it, writes the current confirmed
  snapshot's serializable `spec`, because no unconfirmed request remains capable
  of applying;
- a provider-originated fallback or order change that was not caused by a user,
  host, restore, or reset request does not overwrite user preference;
- immediate or later `Rejected` or `Unsupported` resolution of a restored
  persisted spec removes that stored payload so startup does not retry a request
  the current persistence scope cannot fulfill. A `Stale` restore caused by
  order-universe replacement does not delete the preference; its retired origin
  record is dropped and restoration may be resubmitted in the new universe;
- ordinary `ClearToBaseline` is a user preference and, once accepted, stores the
  resulting empty-term baseline spec;
- `Reset persisted preference` deletes the stored payload before submitting
  `ClearToBaseline` with write suppression keyed to that request ID. The
  suppression covers its immediate and later Pending, Applied, Rejected,
  Unsupported, Stale, and cancellation outcomes. Runtime lane state still
  changes normally, but no outcome recreates the preference that was explicitly
  reset.

Persistence write failure is isolated from provider authority: it reports a
preference-storage error but does not cancel, reinterpret, or roll back the sort
lane. The preference controller keys all later outcomes by `SortRequestId` and
retains an ephemeral `Normal | RestoreOrigin | ResetSuppressed` write mode for
each request that can produce a later outcome. That mode and request ID are
removed after the terminal persistence transition and are never durable state.
An `Applied` outcome naming a not-yet-adopted revision retains its origin record
while the adapter stages it; persistence processing and record removal occur only
when the matching revision event commits confirmed-order adoption. The controller uses
only that origin mode plus the complete `SortControlSnapshot`; it does not infer
acceptance from submission order. `AlreadyResolved` performs no preference write
because the terminal sort outcome or first successful cancellation is causally
delivered first; it also cannot consume an origin record retained for staged
confirmed-order adoption. Order-universe
replacement retires every old-universe origin record without a write; only a new
restore request may establish origin state in the new lane.

### 9.5 Persistence key

The host-supplied key must identify the application namespace, widget/view identity, and data/query scope strongly enough
that an unrelated data universe cannot accidentally consume the preference. The `BaselineOrderId` and stable sort keys
inside the payload provide additional validation but do not repair an incorrectly scoped key.

### 9.6 Forbidden persisted state

The following must not be persisted as sorting truth:

- applied `OrderRevision`;
- `RowIdentityDomainId` or `OrderUniverseId` runtime identity;
- pending `SortRequestId`;
- RowId order or order windows;
- indices or priority derived from header widgets;
- provider acceptance/rejection outcomes;
- process-local comparator/callback identity.

Resetting durable preference must not reset provider domain data or a user-managed manual baseline.

---

## 10. Manual Row Reordering Boundary

Drag-and-drop row reordering is explicitly deferred and is not a sorting operation.

A future manual-order contract must satisfy all of:

- provider-advertised mutation capability;
- identity-based requests such as moving RowIds before or after another RowId;
- no swap-by-index API;
- operation only against a provider baseline that declares itself manually reorderable;
- no ambiguous mutation while explicit computed sort terms are active;
- provider acceptance/rejection and a resulting order revision;
- defined behavior for filtering, paging, concurrent mutation, and rank allocation.

This SPEC reserves no drag gesture and introduces no row-reorder mutation API. Sorting implementation must not encode
assumptions that prevent a later manual baseline, but it must not implement manual ordering indirectly.

---

## 11. Forbidden Behaviors

### FORBID-353-LOCAL-ORDER

ListView, header, adapter, or substrate sorting/copying a provider row set into a new semantic order.

### FORBID-353-DUAL-AUTHORITY

Keeping both provider-applied order and local comparator order as selectable fulfillment modes in one data-backed
ListView architecture.

### FORBID-353-INDEX-ORDER

Using, storing, persisting, or mutating row indices as identity or order authority. Revision-scoped order-window
ordinals are transport coordinates only.

### FORBID-353-FULL-MATERIALIZATION

Requiring the complete ordered RowId universe at the presentation boundary for sorting, invalidation, navigation, or
virtualization.

### FORBID-353-OPTIMISTIC-TRUTH

Displaying requested direction, priority, baseline, or exactness as applied before provider confirmation.

### FORBID-353-PERSISTED-TRUTH

Restoring a saved `SortSpec`, RowId order, or order revision directly into live applied state without a data-layer
request and validation.

### FORBID-353-MANUAL-AS-SORT

Implementing manual drag ordering as a comparator, special sort key, index swap, or ListView-owned row permutation.

---

## 12. Verification Requirements

Implementation governed by this SPEC must verify at least:

1. Plain activation produces the canonical replace cycle.
2. Shift activation appends, toggles, and removes terms without corrupting priority, including multiple inputs while an earlier request is pending.
3. Clear requests the named baseline and does not restore a cached startup vector.
4. Pending, rejected, unsupported, stale, cancelled, and applied outcomes carry
   complete lane testimony and preserve confirmed-state truth, including A
   pending → B immediate reject, A pending → B pending → B later reject, and
   cancellation of current, superseded, every terminally resolved disposition,
   repeated cancellation, unsupported-current, foreign-universe, and unknown
   IDs. Resolve-versus-cancel races linearize once and deliver terminal testimony
   before `AlreadyResolved`.
5. Hidden active sort keys remain active and recover their indicators when shown.
6. Provider and view capability intersection suppresses unsupported/denied
   affordances. Additive affordance also honors chain-level support and maximum
   term count; complete cross-key incompatibility rejects atomically.
7. Custom sort IDs and parameters are provider-validated and serializable.
8. Sorting does not clear RowId-based selection or retarget focus by index.
9. Every new order revision causes exactly one base-order epoch transition, and
   delayed distinct revision events cannot collapse into a revisionless re-query.
   A new confirmed header/persistence snapshot is not exposed until its matching
   revision event atomically adopts the body projection; mismatched or incomplete
   request-origin pairs expose neither half. Provider-origin events independently
   embed and adopt their exact revision snapshot, including queued R1 then R2.
10. Stale windows are discarded and windows from different revisions are never
    merged. Reused numeric sequences from a retired `OrderUniverseId` cannot
    collide with the current lane. Changing `RowIdentityDomainId` kills every
    prior RowId and clears all RowId-keyed selection, focus, edit, cache, binding,
    and realized-widget state before equal raw IDs from the new domain are
    accepted. Every non-order invalidation from a retired universe is discarded
    before dirtying or reconciliation, including events containing raw IDs reused
    by the current universe.
11. Identity-to-ordinal lookup supports focus/navigation after sorting without a complete RowId map.
12. A large synthetic provider can sort and navigate without a complete RowId
    snapshot in adapter, substrate, or ListView memory and can traverse the
    universe without concrete widget count growing with total extent.
13. Session restoration resubmits the requested spec after screen recreation.
14. Durable restoration resubmits and validates the requested spec after process restart.
15. Invalid persisted specs fail as a unit and leave the provider baseline
    authoritative. Rejected or unsupported restores are removed, accepted
    pending and revision-adopted applied requests update only requested
    preference, cancellation/later
    rejection/unsupported resolution returns persistence to confirmed spec, and
    reset leaves no saved preference.
16. Active editing blocks user-originated sort requests without a local deferred-sort queue.
17. No ListView path installs or invokes `RowComparator` to fulfill a data-layer sort request.

---

## 13. Acceptance Criteria

This SPEC is implemented only when:

- the data contract exposes baseline identity, per-key and chain sort
  capabilities, complete lane outcomes/cancellation testimony, confirmed applied
  state, explicit row-identity/order-universe lifetimes, non-reused compound
  order revision, bounded RowId windows,
  identity-to-ordinal lookup, and identity-returning removal recovery;
- ListView routes pointer, keyboard, menu, clear, cancel, and restored-preference operations through the adapter;
- headers project confirmed multi-column state and distinct pending feedback;
- provider order replacement invalidates bounded windows and bumps substrate epoch exactly once;
- the full-RowId-slice/ListView-complete-vector path is no longer the canonical or required implementation;
- comparator-driven ListView fulfillment is removed;
- session and durable preference ownership follow §9;
- all verification requirements in §12 pass.

Documentation, intent emission alone, or a header arrow without data-layer execution does not satisfy this SPEC.
