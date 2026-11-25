# SPEC-HAL-234: Surface Lifecycle and Termination Authority

**Status:** Draft (Binding upon adoption)
**Layer:** Navigation / Surface Orchestration Authority
**Owner:** Core Architecture
**Depends On:** SPEC-HAL-150, SPEC-HAL-163, SPEC-HAL-228, SPEC-HAL-231
**Enables:** SPEC-HAL-233, SPEC-HAL-235, SPEC-HAL-236, SPEC-HAL-237

---

## 1. Purpose

This SPEC defines surface lifecycle and termination authority in Halcyon.

It establishes:
- which subsystem may request surface termination
- which subsystem may execute surface termination
- when termination is legal
- when termination is forbidden

This SPEC explicitly prevents lifecycle logic from leaking into the compositor and prevents widgets, focus logic, or overlays from destroying surfaces.

---

## 2. Definitions

### Surface

A compositor-managed rendering unit identified by compositor::SurfaceId, hosting a scene subtree.

### Navigation Surface

A surface created as a result of navigation path resolution.

### Orchestrator

The NavigationSurfaceOrchestrator that translates navigation state changes into compositor mutations.

### Termination

Permanent destruction of a surface via Compositor::destroy_surface.

---

## 3. Authority Model

### 3.1 Exclusive Execution Authority

### INV-234-A1

Only the NavigationSurfaceOrchestrator may execute surface destruction.

Execution is defined as calling:
- Compositor::destroy_surface(...)

No other subsystem may call this for navigation-managed surfaces.

---

### 3.2 Request Authority

The following subsystems may request termination:

| Subsystem | May Request | Notes |
|---|---|---|
| NavigationHost | Yes | Via navigation path change |
| Step Controller (SPEC-233) | Yes | Semantic only |
| Application Layer | Yes | Through navigation intents |
| Focus System | No | Explicitly forbidden |
| Widget Layer | No | Explicitly forbidden |
| Compositor | No | No semantic knowledge |

### INV-234-A2

A request to terminate a surface does not imply execution.

---

## 4. Lifecycle States

Each navigation-managed surface is in exactly one lifecycle state:

```text
Created -> Active
Active <-> Inactive
Created | Active | Inactive -> Terminating -> Destroyed
```

`Created -> Terminating` is legal only for abort of a hidden prepared surface.
`Active -> Terminating` is legal only inside a prepared transition that first
publishes another exact current surface in the same non-fallible batch, or during
prepared whole-root teardown after external detachments. Ordinary retained
ancestors may move `Inactive -> Active` when they become current again.

### State Semantics

- **Created** — Surface exists in compositor but is not visible.
- **Active** — Surface is visible and corresponds to the current navigation node.
- **Inactive** — Surface exists but is hidden (ancestor surfaces).
- **Terminating** — Surface is scheduled for destruction; no interaction allowed.
- **Destroyed** — Surface no longer exists in compositor.

### INV-234-S1

A destroyed surface must never re-enter any other state.

### 4.1 Prepared Root Admission

Multiroot admission under SPEC-HAL-237 uses an orchestrator/registry-owned
`PreparedRegistryRoot` inside a prepared compound admission, not a publicly
resident root:

```text
prepare_create_root(root_id, navigation_host)
    -> Result<PreparedRegistryRoot>

commit_create_root_guarded(
    PreparedRegistryRoot,
    RootAdmissionPublicationCapability,
)
    -> NavigationRootId // non-fallible under the held domain guard

abort_create_root(PreparedRegistryRoot)
    -> () // destroys staged resources
```

Preparation rejects duplicate IDs and reserves the root z-band before creating
its initial navigation surface. Every prepared surface remains `Created`: it is
hidden, non-interactive, absent from public registry lookup/projection, and
cannot be published by compositor visibility inference. The prepared value may
expose only sealed owner/lifecycle capabilities needed by the same root-admission
transaction; it is not a routable root handle.

`RootAdmissionPublicationCapability` is sealed call-scoped proof, supplied by
higher-layer application orchestration, that every external owner/association
precondition—including the initial current-surface/focus-domain association—has
entered its non-fallible commit sequence under the same domain guard. This SPEC
neither defines nor depends on those external authorities.
`commit_create_root_guarded` consumes that proof, inserts the registry root, and
transitions the initial surface from `Created` to `Active` as the final
publication step. Until that final
visibility transition, renderer snapshots may observe at most a hidden
mechanical surface, never a visible half-admitted root. Guard release occurs only
after the higher-layer compound commit has made the root resident, associated,
routable, and visible as one complete state.

Failure before commit aborts the prepared root, destroys every hidden staged
surface through the orchestrator, releases the z-band reservation, and leaves no
registry, coordinator, router, new root-attributable focus association,
projection, or visible-rendering testimony. Pre-existing independent focus
domains are not owned or destroyed by this registry cleanup. Rollback of an
already visible root is not a conforming admission mechanism.

Prepared-root abort is an explicit admission cleanup path. Because the root was
never resident and every staged surface is hidden in `Created`, abort may destroy
its initial/root surface and any other staged surface. The ordinary live-path
termination restrictions in Section 5 do not apply to this cleanup.

### 4.2 Prepared Navigation-Surface Transition

An ordinary committed navigation change uses an orchestrator-owned prepared
transition rather than fallible mutation after NavigationHost commit:

```text
prepare_navigation_surface_transition(
    final_resolved_candidate,
    expected_orchestration_generation,
) -> Result<PreparedNavigationSurfaceTransition>

commit_navigation_surface_transition(
    PreparedNavigationSurfaceTransition,
    SurfaceTransitionPublicationCapability,
) -> () // non-fallible

abort_navigation_surface_transition(
    PreparedNavigationSurfaceTransition,
) -> () // destroys only hidden staged resources

prepare_overlay_surface_exit(
    expected_orchestration_generation,
    PreparedOverlaySurfaceExitTestimony,
) -> Result<PreparedNavigationOverlaySurfaceExit>

commit_overlay_surface_exit(
    PreparedNavigationOverlaySurfaceExit,
    SurfaceTransitionPublicationCapability,
) -> () // non-fallible
```

Preparation verifies that the orchestrator's active surface, path-to-surface
mapping, and checked non-reused orchestration generation still match the
candidate's `from` state. It computes the complete retained/created/activated/
inactivated/destroyed surface delta, allocates every new surface hidden in
`Created`, validates every lifecycle precondition, checks the next generation,
and prepares one mechanical compositor mutation batch. A surface that would be
removed while an external owner still targets it is reported as a required
detach precondition unless the same compound transaction supplies sealed
owner-issued testimony from an already-prepared overlay deactivation. In that
case the orchestrator may append only the testimony's exact top-first
`Suspend` visibility or `Dismiss` destruction operations to its one mechanical
batch. It does not infer, initiate, or semantically commit that external
deactivation. When delegation or return changes no navigation path, the separate
prepared overlay-surface exit validates and reserves only that exact owner-issued
top-first `Suspend` or `Dismiss` batch; it does not synthesize a no-op host
transition or change navigation mapping/current-surface testimony.

A successful ordinary transition, overlay-only exit, or whole-root teardown preparation holds one
non-cloneable exclusive lease on the orchestrator lifecycle/mapping generation
and its mechanical batch reservation until commit, explicit abort, or drop.
While that lease is live, a second lifecycle preparation or direct orchestrator
mutation returns `Busy/Retry` without mutation. This lease is what makes the
prepared commit non-fallible after its generation check; the domain guard alone
is not treated as hidden ownership inside the orchestrator. Prepared-root
admission has the same exclusive lane inside its not-yet-public orchestrator.

`SurfaceTransitionPublicationCapability` is sealed call-scoped proof from the
higher-layer transaction that every required overlay/focus/binding association
has entered its matching non-fallible detach/replacement sequence under the same
domain guard. This SPEC defines only the proof requirement, not those external
owners. Commit validates and consumes the proof without calling them.

No live surface visibility, lifecycle state, mapping, or active-surface testimony
changes during preparation. Failure, explicit abort, or drop destroys only the
transition's new hidden surfaces, releases the lease/reservations, and leaves the
pre-transition root exact. After every other
participant has prepared, commit applies the prevalidated orchestrator mapping
and lifecycle result and asks Compositor to execute the prepared mechanical batch
as one non-fallible snapshot-atomic operation. Public projection and
renderer-visible surface snapshots therefore see the complete old visible result
or the complete new result, not a partially applied batch. A low-level diagnostic
existence read may observe a newly allocated hidden `Created` surface during
preparation, but that surface has no mapping, active/visible state, input, or
projection authority. The orchestrator remains the semantic lifecycle and
execution owner; Compositor only allocates storage and executes the exact batch.

---

## 5. Ordinary Live-Path Termination Conditions

For a path shrink or replacement inside a still-resident navigation root,
termination is legal only if all conditions hold:
1. Surface is not the root surface
2. Surface is not present in the new navigation path
3. Surface is not hosting an active overlay
4. Surface is not the current active surface

### INV-234-T1

If any condition fails, ordinary live-path termination is forbidden. This rule
does not govern prepared-root abort or explicit whole-root teardown, which have
their own closed preconditions below.

For a prepared ordinary transition, conditions 2–4 are evaluated against the
complete planned post-transition authority state. A surface that was current in
the `from` state may be removed only when the same non-fallible batch first makes
the exact surviving/new surface authoritative and transitions the former current
through `Inactive -> Terminating`; it is never destroyed while it remains the
authoritative current surface. This does not permit removal of the live root
surface.

### 5.1 Prepared Whole-Root Teardown

Whole-root teardown is the only live-root path allowed to destroy the root
surface and the formerly current surface. Before the first irreversible external
authority mutation, the registry/orchestrator prepares one complete
`PreparedRootSurfaceTeardown` covering every surface in the root, deepest first
and root last, plus z-band release. Preparation is fallible, changes no live
surface, and reserves every mechanical operation needed for non-fallible commit.

```text
prepare_root_surface_teardown(root_id, expected_orchestration_generation)
    -> Result<PreparedRootSurfaceTeardown>

prepare_root_surface_teardown_with_overlay_exits(
    root_id,
    expected_orchestration_generation,
    optional_retiring_owner_surface_exit_testimony,
    optional_current_effective_peer_root_generation_and_surface_exit_testimony,
) -> Result<PreparedRootSurfaceTeardown>

prepare_domain_surface_teardown(
    complete_root_generation_set,
    optional_active_owner_surface_exit_testimony,
    domain_transition_token,
) -> Result<PreparedDomainSurfaceTeardown>

commit_root_surface_teardown(
    PreparedRootSurfaceTeardown,
    RootSurfaceTeardownPublicationCapability,
) -> () // non-fallible

commit_domain_surface_teardown(
    PreparedDomainSurfaceTeardown,
    domain_transition_token,
) -> () // non-fallible
```

Commit is lawful only after the same higher-layer transaction has:

1. dismissed the root-owned active overlay stack or proved none is admitted;
2. retired every root-owned focus domain and target;
3. removed or replaced every frame-projection reference and cleared its sink;
4. marked the root non-routable and removed/replaced priming and delegation
   references; and
5. retired the root focus-binding and overlay-owner registrations.

After those steps, higher-layer orchestration issues the sealed
`RootSurfaceTeardownPublicationCapability` bound to the prepared root identity,
surface set, and domain-guard generation. The orchestrator consumes it without
querying or invoking any external owner.

After those external detachments, the root's surfaces are non-interactive and no
authority can lawfully publish or target them. The orchestrator then executes the
prepared whole-root teardown, atomically hides/removes the complete surface set,
destroys deepest-to-root, and releases the z-band. The registry then consumes its
prepared root-removal record. Both are non-fallible finalization steps, but only
the orchestrator calls the mechanical destruction API. Ordinary path-shrink conditions 1 and 4 are
intentionally inapplicable; no root/current surface survives successful
whole-root teardown. Any attempted whole-root destruction before the listed
detachments is forbidden.

Delegation-linked retirement may require dismissal of an active overlay owned
by the surviving current effective peer before dependent frames can unwind. The
registry therefore exposes the second prepared form above. It validates the
peer root identity and orchestration generation, requires owner-issued `Dismiss`
testimony, verifies every peer overlay surface against that peer root's
navigation surfaces, and acquires exclusive lifecycle leases for both the peer
orchestrator and retiring orchestrator. The retiring owner's testimony may
similarly authorize externally owned overlay surfaces attached to the retiring
root. One compositor batch destroys the exact testified overlay surfaces and the
complete retiring-root surface set; no second prepared compositor batch may
coexist behind the same mutable store. Commit advances the surviving peer's
orchestration generation, clears the retiring mechanics, and removes the root.
Abort releases both leases and changes no surface, mapping, or generation.

Interaction-domain retirement uses the batch form. It accepts the router's
complete root/generation set, validates every root against one compositor,
acquires every root mechanics lease, verifies the active owner's exact
top-first `Dismiss` testimony against its owning root, and prepares one
mechanical batch for every external overlay plus every root-local overlay and
navigation surface. Commit is legal only after the domain transaction has
retired the complete focus graph, focus-binding ports, projection consumers,
router claims/routing, and owner registrations. It then destroys all surfaces,
clears every root mechanics record, releases every z-band, and removes every
registry root without a fallible operation. Abort releases every lease and
preserves all roots and surfaces.

---

## 6. Termination Triggers

### 6.1 Navigation Path Shrink

When navigation path transitions from:

`[A, B, C, D] -> [A, B]`

Surfaces corresponding to C and D become eligible for termination.

Execution:
- performed only by the orchestrator
- ordered from deepest to shallowest

---

### 6.2 Redirected Navigation

When a redirect replaces a target node:

`NavigateTo(X) -> RedirectTo(Y)`

SPEC-HAL-231 completes the replacement chain before the domain token and
surface-transition preparation, so no surface is normally allocated for replaced
candidate X. If a standalone integration had already staged a hidden `Created`
surface for X, dropping/aborting that preparation must destroy it before Y can
commit. No visible or mapped surface for X may survive.

### INV-234-T2

No "ghost" surfaces may exist for redirected nodes.

---

## 7. Explicitly Forbidden Behaviors

### Hard Prohibitions

### INV-234-F1

Widgets must never destroy surfaces.

### INV-234-F2

Focus routing must never destroy surfaces.

### INV-234-F3

Overlays must never destroy parent surfaces.

### INV-234-F4

Compositor must never infer lifecycle decisions.

### INV-234-F5

NavigationHost must not call compositor APIs.

---

## 8. Compositor Boundary Guarantee

This SPEC does not add:
- lifecycle state to compositor
- semantic flags to compositor
- navigation knowledge to compositor

The compositor remains mechanical only.

All lifecycle authority lives above it.

### INV-234-C1

Violation of compositor semantic-freedom is a hard architectural failure.

---

## 9. Observability (Read-Only)

Allowed read-only queries:
- active surface id
- surface existence
- surface visibility

No subsystem may infer lifecycle transitions from compositor state alone.

---

## 10. Runtime Status (0.176.0-beta)

The live compositor provides exact prevalidated mechanical surface batches with
exclusive prepare, non-fallible commit, and zero-mutation abort. The live
`NavigationSurfaceOrchestrator` owns complete-entry lifecycle records and lowers
ordinary navigation through hidden surface staging, generation revalidation,
prepared termination ordering, and guarded publication. It now also consumes a
sealed SPEC-HAL-236 surface-exit plan from the exact prepared owner deactivation,
validates every named overlay against this root, and folds the requested
top-first hide/destroy sequence into the same batch as the navigation-surface
delta. `NavigationRootRegistry` exposes duplicate-safe hidden prepared admission
and the separate explicit prepared whole-root teardown path. Each orchestrator
and registry rejects a compositor other than the mechanical store to which it is
bound.

SPEC-HAL-237 root admission, ordinary routed navigation, delegation/return, and
application-owned root retirement now use the compound prepared publication boundary. Root
retirement prepares all projection, focus, binding, router, owner, and mechanical
teardown work before focus retirement, then issues the sealed whole-root teardown
capability only after every external authority reference has been detached.
Delegation-linked retirement can fold the current effective peer owner's exact
`Dismiss` surface plan into the same compositor batch as whole-root teardown
while holding both orchestrator lifecycle leases.
Delegation and return use a separate prepared overlay-only surface batch when the
exiting owner has an active stack but the navigation path is unchanged.
Full interaction-domain retirement now uses one prepared domain surface batch
covering the complete router root set and the active owner's exact external
overlay exit. All mechanical validation and leases are acquired before focus
retirement; commit destroys every associated surface and removes every
registry root only after projection, focus, routing, and owner authorities are
detached. Application-admitted roots hold a permanent same-domain
participation claim, so direct host/orchestrator dispatch and compatibility
whole-root teardown fail closed even when no ScreenHost projection binding is
installed. Compatibility standalone entry points remain available only for
roots that were never application-admitted.

---

## 11. Non-Goals

This SPEC does not define:
- animation
- visual transitions
- timing
- resource cleanup policies
- OS-level windowing behavior

---

## 12. Summary (Normative)

- NavigationSurfaceOrchestrator is the sole execution authority
- NavigationHost is semantic only
- Ordinary path termination, prepared-root abort, and whole-root teardown are
  distinct explicit paths with distinct preconditions
- Prepared ordinary transitions and whole-root teardown complete through
  non-fallible compositor batches after all fallible work succeeds
- Compositor remains purely mechanical
- Lifecycle leaks are structurally forbidden
