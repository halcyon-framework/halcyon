# SPEC-HAL-185: ListView Persistence Scope Law

**Status:** Draft (Binding upon adoption)
**Layer:** State / Temporal Consistency Law
**Scope:** ListView, TreeView projection, any TabularView-backed row-centric component
**Depends on:** SPEC-HAL-179, SPEC-HAL-180, SPEC-HAL-181, SPEC-HAL-182, SPEC-HAL-183, SPEC-HAL-184, SPEC-HAL-158
**Amended by:** SPEC-HAL-353 (requested SortSpec persistence and provider-applied-state authority)
**Non-Goals:** Storage backend, serialization format, user profiles, sync, migration

---

## 1. Purpose

This SPEC defines what ListView is allowed to persist, when persistence may be applied, and what persistence MUST NOT affect.

Persistence is a temporal authority.
If unconstrained, it breaks invariants by mutating structure at illegal phases or restoring state that violates current configuration.

This SPEC exists to prevent:

* startup geometry jumps,
* focus/selection corruption,
* stale state overriding live configuration,
* time-of-application bugs.

---

## 2. Definition

Persistence is the act of restoring prior user state across lifetimes.

Persistence is:

* input to structural systems,
* never an override of authoritative resolution,
* never applied mid-interaction.

---

## 3. Persistence Authority

**INV-PERSIST-AUTH-1 — ListView Boundary Ownership**

Column-presentation persistence MAY be owned by ListView at the component boundary.

* TabularSubstrate MUST NOT persist user state.
* Persistence is externalized or injected into ListView as configuration input.
* When a `ViewConfigStore` is configured, a committed column resize writes the
  owner-validated preferred width at resize completion; live drag geometry is not
  persisted mid-interaction.
* A requested sort preference MAY be retained in app-owned session/view state or injected through the ListView
  configuration boundary.
* The data layer exclusively owns the confirmed applied `SortSpec` and `OrderRevision` under SPEC-HAL-353.
* Restoring a sort preference MUST submit a data-layer request; it MUST NOT restore applied order directly.

Persistence is policy, not substrate mechanics.

---

## 4. Lawful Persisted State

**INV-PERSIST-ALLOW-1 — Allowed Persistence Set**

ListView MAY persist only:

* column order (by ColumnId)
* preferred column widths (not resolved widths)
* column visibility
* requested `SortSpec`, including its provider `BaselineOrderId`, ordered stable sort keys, directions, and serializable
  custom parameters (SPEC-HAL-353)

All persisted values MUST be identity-based.

Comparator/provider mode selection is no longer lawful ListView state. Provider-authoritative fulfillment is the only
mode for a SPEC-HAL-188-backed consumer.

---

**INV-PERSIST-ALLOW-2 — Explicitly Forbidden Persistence**

ListView MUST NOT persist:

* resolved column geometry (x offsets, computed widths)
* row order (unless provider-owned and explicitly external)
* selection state
* focus state
* scroll position
* inline edit state
* widget identity
* virtualization realization state
* confirmed applied sort outcome
* `RowIdentityDomainId`, `OrderUniverseId`, `OrderRevision`, or pending
  `SortRequestId`
* RowId order, order windows, or comparator/callback identity

These states are ephemeral or substrate-governed.

---

## 5. Time-of-Application Law

**INV-PERSIST-TIME-1 — Pre-Layout Application Only**

Persistence MAY be applied only:

* before the first layout pass, or
* during a full structural reconfiguration while no interaction is active

Persistence MUST NOT be applied:

* during pointer capture
* during resize or reorder
* during inline editing
* during scroll dispatch
* mid-frame

For sort preference specifically:

* the requested `SortSpec` MUST be submitted to the data/query controller before ListView consumes its initial ordered
  projection;
* provider validation and execution MAY remain pending after submission;
* later completion changes order only through the normal confirmed order-revision invalidation path;
* persistence code MUST NOT apply a RowId order when the provider response arrives.

Preference writes are driven only by complete provider lane testimony under
SPEC-HAL-353:

* except for an explicit reset-origin request, accepted `Pending` writes the provider-resolved absolute requested spec;
* except for that reset-origin request, accepted `Applied` writes the
  provider-normalized confirmed spec only when its matching order revision is
  adopted (or is already the adopted revision), never its exactness, revision,
  request ID, or outcome;
* except for restored-payload rejection/unsupported testimony or reset-origin
  suppression, immediate rejection/unsupported/stale testimony does not
  overwrite an existing preference;
* except for restored-payload rejection/unsupported testimony or reset-origin
  suppression, later rejection/unsupported resolution or successful cancellation
  of the current pending request writes the current confirmed spec;
* immediate or later rejection/unsupported resolution of a restored payload
  removes it, while stale testimony caused by order-universe replacement preserves
  the preference for a new restore request; an explicit
  persistence reset suppresses every outcome of the baseline request issued by
  that reset from re-writing the preference;
* provider-originated fallback outside a preference request does not overwrite
  the user's stored preference;
* replacement of `OrderUniverseId` drops only old runtime request-origin records;
  it does not write a preference. Reapplying a still-scoped saved preference is a
  new restore-origin request in the new universe;
* `AlreadyResolved` performs no preference write: the serialized provider lane
  delivers and records the request's terminal sort outcome or first successful
  cancellation first, and persistence policy applies that recorded terminal
  result exactly once. Applying it from `AlreadyResolved` would duplicate the
  transition. A resolution/cancellation race linearizes as either that
  terminal outcome followed by `AlreadyResolved`, or one successful
  cancellation, never both. An `Applied` outcome waiting for matching
  order-revision adoption retains its ephemeral origin record; `AlreadyResolved`
  cannot consume it before that adoption applies the one persistence transition.

Storage failure is advisory and MUST NOT alter provider runtime state.

An asynchronous first provider projection is data arrival, not permission to apply persistence mid-interaction.

---

**INV-PERSIST-TIME-2 — Persistence Is Input, Not Mutation**

Persisted values MUST be applied as configuration inputs, not as direct mutation of live state.

Examples:

* preferred widths → input to column model
* column order → input permutation before resolution
* requested `SortSpec` → input to the provider query controller before initial order projection

Direct writes to resolved geometry are forbidden.

On reconstruction, ListView reconciles saved preferred widths by stable
`ColumnId`, rejects non-finite or non-positive values, applies current column
constraints, and supplies the result to the column model before first layout.

---

## 6. Constraint Enforcement

**INV-PERSIST-CONSTR-1 — Persistence Must Respect Invariants**

Persisted values MUST be clamped or rejected if they violate:

* minimum column width invariants (SPEC-HAL-180)
* fixed row height model (SPEC-HAL-183)
* column count or identity changes
* forbidden reorder modes
* unavailable `BaselineOrderId`, sort keys, directions, custom strategy IDs, parameters, or current provider policy

Invalid persisted state MUST be ignored, not "best-effort applied".

A persisted multi-column `SortSpec` is validated as one unit. If any term is invalid, the whole requested spec is
rejected and the provider baseline remains authoritative.

---

## 7. Interaction with Layout and Geometry

**INV-PERSIST-GEOM-1 — No Geometry Side Effects**

Persistence MUST NOT:

* trigger additional layout passes beyond the initial resolution
* cause visible geometry jumps after first paint
* override geometry computed by SPEC-HAL-168

If persistence causes visible movement post-layout, the implementation is wrong.

This law concerns direct application of persisted presentation state. A provider-confirmed order revision arriving
after an asynchronous request is ordinary data invalidation, but the restored request still must have been submitted
before the initial ordered projection was consumed.

---

## 8. Interaction with Focus and Selection

**INV-PERSIST-FOCUS-1 — Focus Is Not Restored**

Focus MUST NOT be restored from persistence.

* Focus enters via explicit interaction only (SPEC-HAL-184).
* Persisted focus creates invisible, invalid anchors.

**INV-PERSIST-SEL-1 — Selection Is Not Restored**

Selection MUST NOT be restored automatically.

* Selection is contextual and data-dependent.
* Persisting selection violates SPEC-HAL-178 and PM-001.

---

## 9. Failure Isolation

**INV-PERSIST-FAIL-1 — Persistence Is Optional and Failable**

Failure to load or apply persistence MUST:

* NOT block ListView initialization
* NOT corrupt structural state
* NOT partially apply illegal values

Persistence is advisory input, not a dependency.

---

## 10. Forbidden Patterns (Hard Law)

The following are architecturally illegal:

* restoring scroll position
* restoring focus or selection
* writing persisted values into resolved geometry
* applying persistence mid-interaction
* persisting by index instead of identity
* silently coercing illegal persisted values
* persisting comparator/provider mode selection
* treating requested sorting as confirmed applied sorting
* restoring an applied RowId sequence, runtime row/order-universe identity, or
  order revision
* using `NavigationHost` as a general ListView preference store

Any occurrence is an SPEC violation.

---

## 11. Acceptance Criteria

SPEC-HAL-185 is satisfied when:

* only the allowed state set is persisted
* persistence is applied before first layout or during the explicitly lawful full structural reconfiguration boundary
* geometry resolution remains authoritative
* focus/selection are never restored
* invalid persisted data is safely ignored
* a committed user resize survives ListView reconstruction when the consumer
  supplies the same stable column identities, view identity, and configuration store
* restored sorting is submitted as one provider-validated `SortSpec` request before the initial ordered projection
* preference writes and reset/rejection/cancellation transitions follow the
  complete-lane policy in §5 and SPEC-HAL-353 §9.4
* no applied sort outcome, row/order-universe runtime identity, order revision,
  RowId order, or comparator identity is persisted

---

## 12. Strategic Note (Non-Normative)

Persistence is memory.
Memory must not overrule law.
