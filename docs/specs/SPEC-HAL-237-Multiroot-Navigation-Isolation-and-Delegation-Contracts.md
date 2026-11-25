# SPEC-HAL-237: Multiroot Navigation Isolation and Delegation Contracts

**Status:** Draft (Binding upon adoption)
**Layer:** Navigation Authority / Application Orchestration
**Owner:** Core Architecture
**Depends On:** SPEC-HAL-002, SPEC-HAL-151, SPEC-HAL-228, SPEC-HAL-229, SPEC-HAL-231, SPEC-HAL-232, SPEC-HAL-235, SPEC-HAL-236, SPEC-HAL-247
**Applies To:** Multi-pane applications, split views, embedded workflows, inspectors, tool panels, nested navigation contexts

---

## 1. Purpose

This SPEC defines multiroot navigation isolation and delegation in Halcyon.

It formalizes how multiple navigation roots can coexist within a single application, how they are isolated from one another, and how (and when) navigation authority may be delegated across roots.

The objective is to prevent:
- accidental cross-root navigation
- implicit coupling between unrelated UI regions
- global back-stack corruption

---

## 2. Scope

This SPEC governs:
- definition of navigation roots
- isolation guarantees between roots
- explicit delegation contracts
- interaction between roots and focus / overlays

This SPEC does not define:
- surface rendering
- layout composition
- widget-level navigation triggers

---

## 3. Definitions

### Navigation Root

An independent navigation context managing its own NavigationHost, committed navigation state, history entries/cursor, guard configuration, root-local overlay owner, and lifecycle. `Navigation root` and `navigation context` name the same authority boundary.

### Root Domain

The scope within which a navigation root is authoritative.

### Delegation

An explicit handoff of navigation authority from one root to another within the same interaction domain.

### Interaction Domain

The existing per-window `InteractionDomainId` authority defined by SPEC-HAL-151, within which focus, capture, and intent routing are isolated. This SPEC does not introduce a second navigation-channel identity.

A modal surface or nested navigation context remains inside its window's interaction domain and may participate in that domain's delegation stack. It may establish a distinct focus domain under SPEC-HAL-235; a focus domain is not an `InteractionDomainId`. Separate windows have separate interaction domains, and navigation delegation never crosses them.

### NavigationDomainRouter

The application-owned, optional multiroot routing authority instantiated alongside `NavigationRootRegistry`. It owns a map keyed by the existing `InteractionDomainId`; each value contains the root identities associated with that domain, zero or one primed root, and an ordered delegation stack. It is not part of engine-owned `InteractionDomainState` and is not owned by NavigationRootRegistry.

Conceptual shape:

```text
NavigationDomainRouter {
    domains: HashMap<InteractionDomainId, InteractionDomainNavigationState>,
}

InteractionDomainNavigationState {
    roots: HashMap<NavigationRootId, NavigationRootBinding>,
    focus_binding_port_claims: HashMap<NavigationRootFocusBindingPortId, NavigationRootId>,
    focus_domain_claims: HashMap<FocusDomainId, NavigationRootId>,
    primed_root: Option<NavigationRootId>,
    delegation_stack: Vec<DelegationFrame>,
    state_generation: NavigationRoutingGeneration, // checked, never reused
    snapshot_phase: Stable | Committing,
}

NavigationRootBinding {
    overlay_owner_id: OverlayOwnerId,
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    focus_binding_port: NavigationRootFocusBindingPort,
}

NavigationRootFocusBindingSnapshot {
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    navigation_surface_id: SurfaceId,
    focus_domain_id: FocusDomainId,
    owned_focus_domains: Set<FocusDomainId>,
    generation: checked non-reused value,
}

NavigationRootFocusBindingCandidateIdentity {
    focus_binding_port_id: NavigationRootFocusBindingPortId,
    generation: checked non-reused value,
}
```

The application explicitly registers and unregisters each router record in synchronization with `InteractionDomains` and the separate `OverlayDomainCoordinator` record required by SPEC-HAL-236. The router record survives focus changes, screen-to-screen navigation, and ordinary root-local navigation while its interaction domain/window lives. It is destroyed with that domain and is not cross-restart persistence.

For a root owner registered with SPEC-HAL-236
`RouterEligibilityRequired`, the router exposes one guarded read-capability seam:

```text
overlay_activation_eligibility_guarded(domain_token, overlay_owner_id)
    -> Result<OverlayActivationEligibility>

prepare_root_association_guarded(
    domain_token,
    prepared_root_id,
    overlay_owner_id,
    prepared_focus_binding_port_admission_testimony,
    prepared_initial_focus_graph_testimony,
) -> Result<PreparedRootAssociation>

commit_root_association_guarded(
    domain_token,
    PreparedRootAssociation,
    newly_committed_focus_binding_port,
) -> ()

prepare_root_focus_membership_change_guarded(
    domain_token,
    root_id,
    expected_router_generation,
    prepared_root_owned_focus_domain_change,
    prepared_focus_domain_graph_mutation_testimony,
) -> Result<PreparedRootFocusMembershipChange>

commit_root_focus_membership_change_guarded(
    domain_token,
    PreparedRootFocusMembershipChange,
) -> () // non-fallible port/claim publication after focus graph commit
```

`OverlayActivationEligibility` is sealed call-scoped testimony naming the domain,
owner, current checked router
`state_generation`, and the root-focus binding port's current checked generation
plus complete owned-focus-domain set. It is issued only when the owner maps to a resident,
non-terminated root that is not suspended anywhere in the active delegation
chain. It is consumed by coordinator activation under the same domain token and
is never stored. The live capability exclusively leases that guarded router
generation and focus-binding generation; no root, focus-domain membership,
current-surface association, priming, delegation, suspension, or membership mutation may
advance either until the capability is consumed or dropped. Every root
admission/membership/current-association change, priming change, delegation
push/pop, suspension change, committed routed navigation, or termination
advances the checked non-reusing router generation exactly once for its complete
transaction; exhaustion rejects preparation before mutation. The coordinator validates the
sealed identity binding and verifies that every prepared overlay focus domain is
inside the capability's root-owned set without reading router or port state,
preserving authority direction.

Root-association preparation validates domain membership, root/owner absence,
the prepared focus-binding port's sealed admission testimony and globally
unclaimed port ID, its exact prepared-root/initial-surface association, the
sealed parent-before-child initial focus-graph testimony, the resulting complete
top-level-tree owned set, absence of every domain claim collision, and the
checked next router generation without publishing membership or routability.
Its commit accepts only the newly committed port produced from that exact sealed
preparation, is non-fallible under the same token, and writes the prepared
association only while the enclosing router phase is `Committing`; the
preparation's reserved next router generation is published once with the
complete compound transaction's final `Stable` phase. Dropping an uncommitted
preparation has zero externally observable mutation.

Membership-change preparation validates that the port preparation and focus
graph testimony describe the same exact before/after set, that every current
claim belongs to the named root, that every new claim is absent, and that the
result remains disjoint and ancestry/subtree closed. It reserves the complete
claim-index and the compound transaction's single next-router-generation update.
Commit is valid only after the bound
focus graph mutation commits; it publishes the port set and claim indexes
non-fallibly as one router operation while phase is `Committing`; it does not
advance the router generation a second time.

`NavigationRootFocusBindingPort` is a registered root-local/application-owned
association capability, not focus authority. It owns only the explicit mapping
between that root's current navigation surface and its corresponding registered
SPEC-HAL-235 focus domain, plus the complete set of focus domains owned by the
root for lifecycle retirement. Every port has an immutable, opaque
`NavigationRootFocusBindingPortId`; capability copies identify the same port and
do not create another claim. The port is a sealed in-process authority record,
not a user callback; every guarded method is bounded and non-blocking. Its
conceptual seam is:

```text
prepare_create_guarded(
    domain_token,
    prepared_root_id,
    focus_binding_port_id,
    initial_navigation_surface_id,
    initial_focus_domain_id,
    prepared_initial_focus_graph_testimony,
) -> Result<PreparedRootFocusBindingPortAdmission>
commit_create_guarded(
    domain_token,
    PreparedRootFocusBindingPortAdmission,
) -> NavigationRootFocusBindingPort // non-fallible
candidate_identity() -> NavigationRootFocusBindingCandidateIdentity | Retry
snapshot_guarded(domain_token) -> Result<NavigationRootFocusBindingSnapshot>
prepare_replace_guarded(domain_token, expected_surface, next_surface, next_focus_domain)
    -> Result<PreparedRootFocusBindingReplacement>
commit_replace_guarded(domain_token, PreparedRootFocusBindingReplacement) -> ()
prepare_owned_domain_change_guarded(
    domain_token,
    expected_generation,
    prepared_focus_domain_graph_mutation_testimony,
    optional_current_binding_replacement,
) -> Result<PreparedRootOwnedFocusDomainChange>
commit_owned_domain_change_guarded(domain_token, PreparedRootOwnedFocusDomainChange) -> ()
prepare_retire_guarded(domain_token) -> Result<PreparedRootFocusBindingRetirement>
commit_retire_guarded(domain_token, PreparedRootFocusBindingRetirement) -> ()
```

The port never selects a target and never stores FocusManager state. Preparation
validates root/domain membership, the expected current surface, the registered
focus domain, the complete owned-domain set, and a checked next binding
generation without publishing a new association. Commit is non-fallible under
the same token. Every valid snapshot's `focus_domain_id` is a member of its
`owned_focus_domains`; an empty set is lawful only for a prepared/retiring port
that is not routable or snapshot-readable. The router stores the stable port
capability as routing metadata and validates its owned snapshot; it does not
infer association from focus, visibility, widget ancestry, or compositor state.

`candidate_identity()` is the port's sole unguarded observation. It is a bounded
atomic read of only the immutable port ID and current checked generation; it
returns neither surface/domain association nor owned-domain membership. A
port-commit-in-progress marker or torn generation observation returns `Retry`.
It exists only so the outside-token routed candidate snapshot can bind later
guarded revalidation to a concrete port generation without performing an
ambient focus association lookup. Full association testimony remains available
only through `snapshot_guarded`.

`prepare_create_guarded` is the only root-admission constructor. It binds an
immutable port ID, the prepared root identity and initial surface, the initial
current domain, and the complete owned-domain set from the same sealed focus
graph testimony. The current domain must occur in that set. It publishes no live
port and exposes only sealed `PreparedRootFocusBindingPortAdmissionTestimony` to
router association preparation. After all compound admission preparations
succeed, `commit_create_guarded` consumes the prepared value and creates the
non-snapshot-readable live port non-fallibly while router phase is `Committing`;
the matching router association commit then makes it routable. Dropping the
prepared value creates no port and changes no association.

`prepare_retire_guarded` similarly exposes sealed
`PreparedRootFocusBindingRetirementTestimony` containing the exact port ID,
generation, current surface/domain, and complete owned-domain set. Router claim
and association-removal preparation accepts only that testimony, so retirement
cannot omit a claimed domain, substitute a port, or invent ancestry. The port
commit consumes the prepared retirement before the matching non-fallible router
claim removal; drop changes neither port nor router state.

`prepare_owned_domain_change_guarded` is the sole mutation seam for the
root-owned set after admission. It accepts only the sealed before/after ancestry
and membership testimony of one SPEC-HAL-235
`PreparedFocusDomainGraphMutation`; it does not accept caller-invented add/remove
sets. The expected generation must match, every added/removed domain must belong
to the same interaction domain, and the computed next set must contain the
binding's post-commit focus domain. Without
`optional_current_binding_replacement`, the old current domain remains the
post-commit domain and therefore cannot be removed. With a replacement, the
preparation validates the expected current surface/domain and next
surface/domain and requires the next domain in the prepared graph and computed
owned set. When the removed old current domain is active, the graph mutation's
explicit replacement-active domain must equal that same next domain.

The port preparation alone is not cross-root ownership admission. Application
orchestration presents it and the same sealed graph testimony to
`NavigationDomainRouter::prepare_root_focus_membership_change_guarded`, which
reserves the router's unique claim-index delta and checked router generation.
After both preparations succeed, focus graph commit occurs first, followed by
the compound non-fallible port/claim commit; the held guard makes neither
midpoint observable. A
root-owned overlay-domain registration commits this port change before the
router can issue activation eligibility; removal occurs only after the overlay
frame/token and focus domain have been lawfully retired. The port advances its
checked generation once for the complete change and never accepts an
unattributed domain ID.

`NavigationDomainRouter` is the unique claim authority. A port ID may be claimed
by exactly one resident root, and a focus domain may occur in exactly one
resident root's owned set. Each root-owned set is a union of complete top-level
focus-domain trees: for every owned domain, its entire parent chain to a
parentless domain and every registered descendant are owned by the same root.
Two root-owned sets are therefore disjoint and cannot have ancestor/descendant
crossings. Admission, graph change, retirement, and domain teardown validate and
reserve the complete claim-map result before mutation; unknown ancestry,
overlapping claims, partial subtrees, reused port identity, and a surviving
unclaimed descendant all reject unchanged.

Every registration or removal attributable to a root, including a root-owned
overlay domain, uses one SPEC-HAL-235 batch graph mutation plus the matching
prepared port/claim update under the same token and commits both non-fallibly.
Therefore `owned_focus_domains` is exact, exclusive, ancestry/subtree-closed
retirement testimony, not a best-effort cache. A root-owned overlay cannot
activate with a focus domain absent from the port snapshot.

### Primed Root

The resident root selected within one interaction domain to receive unscoped navigation commands when no delegated override exists.

### Delegation Stack

An ordered stack, local to one interaction domain, of explicit source-to-target delegation frames. Its top target temporarily supersedes the primed root as the effective recipient of unscoped navigation commands. Delegation does not merge histories or navigation state.

---

## 4. Root Isolation Model

### 4.1 Independent State

Each navigation root owns:
- its own NavigationHost
- its own navigation path
- its own navigation history entries and cursor
- its own guard configuration

### INV-237-A1

Navigation state must not be shared across roots.

---

### 4.2 No Implicit Cross-Root Effects

Navigation events in one root must not:
- modify another root's state
- trigger surface changes in another root
- affect another root's history

### INV-237-A2

A context-local `NavigationIntent` never crosses a root boundary. Changing which root holds effective command authority requires explicit delegation, and delegation never transfers or traverses navigation state/history between roots.

---

## 5. Root Identification

Each navigation root must have:
- a stable root identifier
- a stable `InteractionDomainId`
- a stable `OverlayOwnerId` for its root-local overlay stack
- a uniquely claimed stable `NavigationRootFocusBindingPortId` and port for current-surface association and
  root-owned focus-domain retirement
- a well-defined ownership boundary

Root identity, interaction-domain membership, root-to-overlay-owner binding, and
the root-focus association port must be explicitly registered and durable for
the lifetime of the root. `NavigationDomainRouter` stores those capabilities as
routing metadata only; overlay ownership remains root-local under SPEC-HAL-236
and focus target/domain authority remains in SPEC-HAL-235. A live root belongs to
exactly one per-window interaction domain. Moving a context to another
window/domain creates a new root registration; live roots are not silently
rehomed. Sharing semantic application data across roots does not make those
roots one navigation authority.

In a multiroot composition, application lifecycle orchestration treats registry creation and router association as one root-admission transaction:

1. call `OverlayDomainCoordinator::begin_domain_transition(interaction_domain_id)` exactly once; failure returns without mutation,
2. under that token validate that `InteractionDomains` and the coordinator record are live and that the root ID is absent from both `NavigationDomainRouter` and `NavigationRootRegistry`, then call SPEC-HAL-234 `NavigationRootRegistry::prepare_create_root`; duplicate rejection precedes z-band/surface allocation and any created initial surface remains hidden in `Created`, absent from public registry lookup,
3. using only sealed capabilities from that prepared root, prepare coordinator owner registration with `RouterEligibilityRequired`; prepare one SPEC-HAL-235 focus-domain graph mutation that creates the root's complete initial parent-before-child top-level tree; use its sealed testimony to prepare `NavigationRootFocusBindingPort::prepare_create_guarded` for the prepared initial surface/current domain and to prepare the exclusive router port/domain claims and router association from the port admission testimony. None of those preparations publishes an owner, focus domain, claim, focus association, routable root, projection source, or visible surface,
4. after every fallible preparation succeeds, application orchestration invokes its `commit_root_admission` as one non-fallible sequence under the token: mark router phase `Committing`, commit coordinator registration, commit the prepared focus graph, commit the new focus-binding port, pass that exact port to the prepared router claim/association commit, mark the root resident/routable/projectable, issue SPEC-HAL-234 `RootAdmissionPublicationCapability`, and call `commit_create_root_guarded` to insert the public registry root and transition its initial surface from `Created` to `Active` as the final publication substep; publish the one reserved router generation with phase `Stable`, then release the token,
5. if any preparation fails, abort the prepared root, destroying its hidden surfaces and z-band reservation, discard the uncommitted coordinator/focus-graph/focus-binding/claim/router preparations, and release the token. No rollback of a visible or publicly resident root is permitted.

No public creation path may silently replace an existing root. The root becomes
resident, routable, projectable, and visible only after registry creation,
coordinator owner registration, initial root-focus association, and router
association all succeed. A compositor
snapshot may see a staged surface only as hidden mechanical state; it cannot
render a half-admitted root. Removal follows the single termination transaction
in Section 11; no earlier rule may detach routing references before overlay deactivation succeeds. No command may observe a half-registered or half-removed root.

Multiple roots may be resident and rendered in the same window or screen. Visual containment, overlap, z-order, or layout nesting does not establish navigation ancestry or delegation.

Simultaneous projection testimony is governed by the ScreenHost's explicit record in `FrameProjectionBindingRegistry` under SPEC-HAL-248. That binding names every represented root and its testimony order; runtime glue pulls each named root independently and publishes one atomic `NavigationFrameProjection`. It does not control actual surface visibility or z-order, which remain `SurfaceStack` authority. NavigationRootRegistry does not choose the projection set/order, and repeated per-root writes to ScreenHost are forbidden.

---

## 6. Interaction-Domain Priming and Effective Command Target

### INV-237-P1: NavigationDomainRouter Owns Explicit Navigation Routing State

For multiroot navigation, the application-owned `NavigationDomainRouter` owns exactly one navigation routing record per registered `InteractionDomainId`. The record owns:

- the map of root identities explicitly associated with that domain to their stable root-local `OverlayOwnerId` values,
- each root's uniquely claimed `NavigationRootFocusBindingPortId` and the
  disjoint, ancestry/subtree-closed focus-domain claim indexes,
- zero or one `primed_root`,
- an ordered delegation stack.

This state survives ordinary focus changes, screen navigation, and context-neutral chrome interaction. It is destroyed only with its interaction domain. It must not be inferred from focus, widget ancestry, visibility, hover, z-order, or compositor state, and it must not be inserted into the closed engine-owned `InteractionDomainState` structure.

Application-global commands first resolve to an existing `InteractionDomainId` through SPEC-HAL-151 `InteractionDomains` window/interaction authority. `NavigationDomainRouter` then resolves the root. Commands without a valid domain are rejected before navigation routing.

### INV-237-P2: Many Roots May Be Live; One Receives an Unscoped Command

Many peer roots may be resident, visible, and able to receive explicitly scoped commands simultaneously. At most one root per interaction domain is the effective target of an unscoped Back/Forward/navigation command:

1. the top delegated target, when a delegation frame exists;
2. otherwise the primed root;
3. otherwise no root, producing an explicit routing-unavailable result without calling NavigationRootRegistry.

No unscoped command may be broadcast or applied to more than one root.

### INV-237-P3: Priming Is an Explicit Application Action

Application composition or a context-owned interaction boundary may issue `PrimeNavigationRoot(interaction_domain_id, root_id)`. `NavigationDomainRouter` validates that the root is registered, resident, and associated with that same domain before mutation. Failure is explicit and leaves routing state unchanged.

While a delegated override exists, the domain's base primed root is retained and cannot be changed. The override remains the effective target until explicitly returned. Explicitly scoped commands may still reach non-suspended peer roots, but do not change domain priming while the delegation stack is non-empty.

### INV-237-P4: Routing Produces an Explicit Registry Target

Every application navigation command in a multiroot composition passes through interaction-domain routing validation:

- a scoped command supplies `(interaction_domain_id, root_id, intent)`,
- an unscoped command supplies `(interaction_domain_id, intent)` and resolves through the effective-root rule,
- invalid domain membership and suspended roots are rejected before execution.

Successful `NavigationDomainRouter` routing produces one explicit `root_id`. NavigationRootRegistry receives only that explicit target and remains an isolation/dispatch authority under SPEC-HAL-247. It does not own priming/delegation state and does not select a target for an unscoped command.

Application orchestration owns the bounded transaction wrapper, with this
conceptual seam:

```text
route_snapshot(command) -> RoutedNavigationSnapshot | RoutingUnavailable | Retry

prepare_routed_navigation_guarded(
    domain_token,
    routed_snapshot,
    navigation_guard_authorization,
) -> Result<PreparedRoutedNavigationTransaction>

commit_routed_navigation_guarded(
    domain_token,
    PreparedRoutedNavigationTransaction,
) -> NavigationOutcome // non-fallible
```

The prepared transaction contains owner-issued prepared values; it is not a new
authority store and does not reinterpret them.

`route_snapshot` is a coherent optimistic read, not an unlocked collection of
map lookups. Every guarded compound writer that can change a registered root's
routing, residency, host mechanics, current surface/focus association, or
suspension atomically marks `snapshot_phase = Committing` before its first
participant commit and retains that phase through the complete already prepared
non-fallible commit. Preparation reserves exactly one checked next
`state_generation` for the whole compound transaction. After its final
participant commit, the router publishes that one generation and
`snapshot_phase = Stable`; no subcommit advances it independently. Domain
removal instead removes the record as its final state. Pure router mutations use
the same phase and one generation advance. Thus root admission,
ordinary navigation, delegation/return, root termination, and domain teardown
cannot expose a stable router snapshot from a compound midpoint.

The optimistic reader returns one owned immutable record only when phase is
stable and the same router generation/phase plus root-focus-binding
`candidate_identity()` are observed before and after copying the relevant
domain, root binding, claim, priming/delegation, registry residency/suspension,
and that root's owned SPEC-HAL-228 mechanics snapshot. Any busy/torn port read or
changed router/port generation returns `Retry`. It holds no live reference and
pins no state. The phase marker is snapshot coordination, not a second routing
authority or a routable intermediate state. Guarded revalidation remains
mandatory even after a coherent snapshot.

Ordinary scoped/unscoped routing uses an optimistic versioned candidate followed
by one guarded prepared transaction. It never executes the application guard
while holding the domain token:

1. application orchestration obtains one owned
   `RoutedNavigationSnapshot` containing the interaction domain, explicit target
   root, checked router `state_generation`, root-focus-binding generation,
   residency, suspension testimony, and the root's coherent owned SPEC-HAL-228
   mechanics snapshot. It resolves the intent from that embedded host snapshot.
   Routing-unavailable, host `Unavailable`, and host `NoOp` return without guard
   execution or mutation;
2. outside every `DomainTransitionGuardToken`, it evaluates the complete
   SPEC-HAL-231 guard/replacement chain against those immutable versioned
   candidates. Guard failure follows application policy and releases no token
   because none has been acquired. `Deny` terminates unchanged. `Allow` yields
   one candidate-bound `NavigationGuardAuthorization`;
3. it then makes one non-blocking
   `OverlayDomainCoordinator::begin_domain_transition` attempt. Busy drops no
   state and retries the complete route/resolve/guard sequence on a later
   scheduling turn; an old authorization is never queued for blind commit;
4. under the token, `prepare_routed_navigation_guarded` revalidates the domain,
   explicit root, router/root-focus-binding generations, residency, suspension,
   NavigationHost mechanics generation, candidate/authorization identity, and
   any SPEC-HAL-231 guard-context generation. A mismatch releases the token,
   drops the authorization, and restarts from step 1 without invoking the guard
   under the token;
5. after revalidation, application orchestration prepares every fallible
   participant: SPEC-HAL-228 `PreparedNavigationCommit`, SPEC-HAL-234
   `PreparedNavigationSurfaceTransition`, any required coordinator overlay exit,
   and the exact root focus result. If the target focus domain is already
   registered and claimed by the root, that result uses
   `NavigationRootFocusBindingPort::prepare_replace_guarded` plus ordinary focus
   activation. If navigation adds/removes a focus subtree or swaps the current
   domain, it instead prepares one SPEC-HAL-235 batch graph mutation and the
   matching `PreparedRootFocusMembershipChange`, including the binding
   replacement and final focus result. An overlay exit accompanying that graph
   change uses SPEC-HAL-236
   `prepare_deactivate_for_focus_graph_mutation_guarded` rather than a second
   focus plan. Every prepared overlay exit also exposes its sealed owner-issued
   top-first surface plan to SPEC-HAL-234; the orchestrator validates that each
   named surface belongs to this root and appends only the declared `Suspend` or
   `Dismiss` operations to the same prepared mechanical batch. New
   surfaces remain hidden in `Created`.
   Any preparation failure aborts only hidden staged resources, drops every
   prepared value, releases the token, and leaves host, history, router,
   orchestrator, compositor-visible surfaces, binding, overlay, focus, and frame
   testimony unchanged;
6. once every preparation succeeds, one
   `PreparedRoutedNavigationTransaction` owns those single-use values. Its commit
   contains no fallible call. It first marks the router snapshot phase
   `Committing`, then commits exactly one prepared focus path. An
   overlay-plus-graph path commits
   `PreparedOverlayFocusGraphTransition` and then its root claim/membership
   result; a graph-only path commits the focus graph and then its root
   claim/membership result; an overlay exit to an existing domain commits only
   `PreparedOverlayExitTransition`; and an existing-domain/no-overlay path commits
   its prepared activation. It next commits the root current-surface binding when
   that replacement was not already included in the membership change. At this
   point no authoritative focus target or binding names a surface scheduled for
   destruction. Application orchestration then issues the SPEC-HAL-234
   `SurfaceTransitionPublicationCapability` from that committed external detach/
   replacement result, commits the NavigationHost history/cursor write, and
   commits the orchestrator/compositor surface batch as the final surface
   publication. No prepared focus value is consumed twice and no destroyed
   surface survives in focus or binding testimony. It advances every non-router
   participant generation exactly as prepared, publishes the compound's single
   reserved next router `state_generation` with snapshot phase `Stable`, and
   exposes the complete result before releasing the token.

The wrapper does not transfer navigation authority to
`OverlayDomainCoordinator`; that coordinator supplies only domain-wide exclusion
and overlay lifecycle preparation. No current surface may be published without
its exact current focus binding, no pruned surface may be destroyed with a live
overlay, and no reader admitted by the domain protocol can observe a prepared
midpoint. A SPEC-HAL-248 frame binding does not change during ordinary
navigation; its guarded pull observes the complete pre-commit or post-commit
authority state. A frame-bound standalone orchestrator uses the same
resolve/guard-outside-token, host/orchestrator-generation revalidation, prepared
commit sequence without router-generation steps.

---

## 7. Delegation Contracts

### 7.1 Explicit Delegation Only

Delegation must be:
- explicitly declared
- unidirectional
- bounded in scope

### INV-237-D1

Implicit delegation is forbidden.

---

### 7.2 Delegation Trigger

Delegation may occur only through an explicit `NavigationDomainRouter` operation. Context-local `NavigationIntent`, NavigationHost guards, and NavigationRootRegistry cannot create, pop, or alter delegation frames.

Widgets must not directly delegate navigation authority.

---

### 7.3 Delegation Semantics

Delegation is one compound interaction-domain transaction, not a sequence of
ordinary coordinator calls. Application orchestration performs it in this exact
order:

1. acquire exactly one `DomainTransitionGuardToken` from
   `OverlayDomainCoordinator`; acquisition failure returns without mutation,
2. under that token validate the interaction domain, source, target, residency,
   suspension, active-chain constraints, both roots' registered
   `OverlayOwnerId` values, the target root's guarded
   `NavigationRootFocusBindingSnapshot`, its exact current-surface match, and the
   complete router push; query overlay state only through
   `active_binding_guarded(token)`,
3. if no overlay binding is active, prepare the target root focus-domain
   activation through SPEC-HAL-235; if the binding is source-owned, call
   SPEC-HAL-236 `prepare_deactivate_to_focus_domain_guarded` with that owner,
   disposition, and target focus domain so overlay exit, exact restoration-token
   consumption, and final target focus are one mutation-free preparation; if a
   target-owned overlay binding is already active, preserve its overlay focus
   domain and prepare no root-focus replacement; any other owner rejects before
   mutation,
4. an invalid/wrong-domain/stale-surface focus binding, a binding whose focus
   domain is equal to or below an exiting source-overlay domain, or failed
   owner/focus preparation
   releases the token without mutation, while a lawful prepared result that
   selects no target remains valid,
5. after every fallible preparation succeeds, non-fallibly commit either the
   ordinary target-focus plan or the compound source-overlay-exit plan, perform
   the reserved router push, and retain any target-owned overlay focus authority,
6. publish the complete overlay/router/focus result and release the one token.

Validation, focus preparation, or coordinator preparation failure releases the
token and leaves the coordinator binding, delegation stack, focus, and both
roots' navigation state unchanged. A coordinator deactivation may dismiss or
suspend root-local stack contents, but it never transfers them to the target.
The compound exit consumes source-overlay restoration tokens without publishing
an intermediate restoration target. No ordinary `active` or `deactivate` call
may reacquire the guard or run beside a prepared focus plan inside this
operation.

The reserved `NavigationDomainRouter` commit applies the already-validated delegation:
- an explicit `(source_root, target_root)` frame is pushed in one interaction domain
- `source_root` is the domain's current effective root
- `target_root` is a registered resident root in the same domain and is neither `source_root` nor any root already present in that domain's active delegation chain
- the source root is suspended from navigation execution for the lifetime of that frame
- the target root becomes the effective recipient of unscoped commands
- no state merging occurs

The executable router seam is:

```text
prepare_delegation_guarded(token, source_root, target_root)
    -> Result<PreparedDelegationPush>
begin_delegation_commit_guarded(token, &mut PreparedDelegationPush)
finish_delegation_commit_guarded(token, PreparedDelegationPush) // non-fallible

prepare_delegation_return_guarded(token)
    -> Result<PreparedDelegationReturn>
begin_delegation_return_commit_guarded(token, &mut PreparedDelegationReturn)
finish_delegation_return_commit_guarded(token, PreparedDelegationReturn) // non-fallible
```

Each preparation reserves the exact checked next router generation and embeds
both owner identities plus the destination root's guarded focus-binding
testimony. Begin marks the router snapshot phase `Committing`; finish alone
pushes or pops the exact frame, publishes the reserved generation, and restores
`Stable` after the already-prepared focus/overlay/surface work commits.

Delegation is not inheritance.

Nested delegation is represented by additional frames, not by merging hosts or
constructing an implicit navigation tree. Return is the symmetric one-token
transaction: acquire one guard; validate and reserve the top-frame pop, source
and target owner bindings, obtain the restored source root's current guarded
focus-binding snapshot and exact active-surface match, and reserve its focus-domain activation; query
`active_binding_guarded(token)`; with no binding, prepare ordinary source focus; with a
target-owned binding, prepare
`prepare_deactivate_to_focus_domain_guarded(..., source_focus_domain)`; reject
any other binding or any source focus domain equal to/below an exiting target
overlay domain before mutation; then non-fallibly commit the selected focus
or compound-exit plan, pop exactly one frame, publish the complete result, and
release the token. No binding is also a valid precondition. The domain's base primed root is
retained beneath the stack. Any source overlay restoration is a separate
later operation that obtains fresh router eligibility and calls guarded
coordinator activation after the return; it is never implied by the pop. Failure before the first irreversible step leaves overlay,
router, and focus state unchanged.

### INV-237-D2: Return From Context Is Explicit

Leaving a delegated context occurs through an explicit `ReturnFromContext`/delegation-return operation. It is not represented by `HistoryBack`, `HistoryForward`, `NavigateUp`, overlay dismissal, or history-boundary fallthrough.

### INV-237-D3: Scoped Peer Dispatch and Suspended Sources Differ

An unprimed peer root may receive a command explicitly scoped to its root identity. `NavigationDomainRouter` rejects execution targeting a source root suspended by an active delegation frame until that frame is returned. Priming alone does not suspend peers; delegation does.

---

## 8. Structural and History Navigation Rules

`NavigateUp`, `HistoryBack`, and `HistoryForward` are root-local.

### INV-237-B1

A scoped action affects only its explicitly identified non-suspended root. An unscoped action affects only the effective root selected by the delegation stack/primed-root rule.

Cross-root history traversal is forbidden. Delegation changes which independent root receives later commands; it never makes one root's Back/Forward traverse another root's history.

### INV-237-B2: No Boundary Fallthrough

When the effective root cannot move farther backward/forward or upward, the action is unavailable and causes no mutation. It must not automatically:

- dispatch to a parent/source/previously primed root,
- close or dismiss the current context,
- pop a delegation frame,
- search another visible context for available history.

Returning from a delegated context is a separate explicit operation.

---

## 9. Focus and Input Interaction

Navigation routing constrains focus; it does not own focus. Every projected
navigation context has exactly one current guarded association supplied by its
registered `NavigationRootFocusBindingPort`. The snapshot's navigation surface
must equal `Some(snapshot.navigation_surface_id)` in the root projection's
optional active-surface field, and its focus domain must be a live member of the
same interaction domain. Binding replacement is prepared and
committed with the corresponding navigation/focus transaction; no ambient or
post-hoc lookup is lawful. Focus, visibility, or widget ancestry must never be
used to discover either the association or the primed/effective root.

When delegation occurs:

1. application orchestration acquires the one domain transition guard,
2. router and overlay-owner validation plus either ordinary target-root focus
   preparation or compound source-overlay-exit-to-target-focus preparation
   completes without mutation; a retained target-owned overlay preserves overlay focus,
3. the selected focus/compound-exit commit and router push execute as
   non-fallible finalization under that token,
4. the complete new effective-root, overlay, and focus state is published before the guard is released.

An eligible-target search that resolves to none commits the delegation with no
focused target; it is not a post-commit failure and never falls back to source
focus or another root. Invalid focus-domain authority fails preparation before
the delegation commits. Return follows the symmetric prepared source-focus rule.

Peer roots may remain visible with retained declarative focus structure, but only the focus domain selected by the focus resolver receives routed keyboard input. Context-local pointer actions still carry explicit root identity and, when the interaction domain has no delegated override, may explicitly prime a peer before routing subsequent unscoped navigation commands.

This must align with SPEC-HAL-235.

---

## 10. Overlay Interaction

Overlays are scoped to their owning root-local `OverlayOwnerId`, while the application-owned `OverlayDomainCoordinator` under SPEC-HAL-236 publishes at most one active overlay-stack binding across the entire `InteractionDomainId`.

### INV-237-O1

An overlay must not outlive its navigation root. Root-local ownership does not permit peer roots in one interaction domain to expose active overlay stacks concurrently.

Delegation never rebinds an overlay across roots. Source-root overlays remain source-owned and must be explicitly dismissed or suspended through `OverlayDomainCoordinator::deactivate_guarded` inside the one-token delegation transaction before the source is suspended. Target-root overlays remain target-owned and may become active only after the source stack is inactive, the router issues `OverlayActivationEligibility` for that non-suspended target under the held domain token, and `OverlayDomainCoordinator::activate_guarded` consumes it. Retained inactive overlay state receives no hit-tests or focus. A suspended source cannot obtain eligibility, so direct, ordinary, or stale-callback activation fails before owner/focus/binding mutation. Returning from delegation deactivates a target-owned active binding through the symmetric guarded transaction and does not implicitly recreate or reactivate a dismissed source overlay. A returned source may reactivate later only through a new eligibility check after the delegation pop commits.

---

## 11. Lifecycle and Termination

When a navigation root is terminated:
- all associated navigation state is discarded
- all associated history entries/cursor are discarded
- all associated overlays are invalidated
- no delegation may occur afterward

Root termination is final.

Delegation-linked retirement has one exact router-owned unwind plan. For a
contiguous active stack `F0 .. Fn`, the router finds the lowest frame `Fk` that
names the retiring root as source or target and reserves removal of the complete
suffix `Fn .. Fk` in top-first order. A non-top member therefore cannot strand a
dependent frame. If the retiring root is the base primed root, `k = 0` and the
whole stack is unwound; its optional explicit priming replacement becomes the
post-commit base. Otherwise priming is unchanged. The final effective root is
the target of the last retained frame, or the surviving primed/replacement root,
or none. A replacement is accepted only when the retiring root is primed; it may
name a currently suspended surviving chain member because the same prepared
unwind removes that suspension before publication.

```text
root_delegation_retirement_plan_guarded(
    token,
    root_id,
    optional_replacement_primed_root,
) -> Result<NavigationRootDelegationRetirementPlan>

prepare_root_retirement_guarded(
    token,
    root_id,
    expected_router_generation,
    prepared_focus_binding_retirement_testimony,
    prepared_focus_graph_testimony,
    NavigationRootDelegationRetirementPlan,
) -> Result<PreparedRootRetirement>
```

The plan binds the guard, checked router generation, complete before-stack,
primed root, exact top-first removed suffix, and final effective root. It is a
versioned candidate, not mutation. Retirement preparation revalidates it and
reserves the router's one next generation. Commit truncates exactly that suffix
inside the already-open `Committing` phase; it never executes a sequence of
ordinary return operations and never exposes an intermediate effective root.

Root termination is one application lifecycle transaction with this exact order:

1. acquire exactly one `DomainTransitionGuardToken` from `OverlayDomainCoordinator`; acquisition failure returns without mutation,
2. under that token obtain and validate the exact router unwind plan above, the
   root's registry residency, router association, owner-port registration, and
   priming replacement if required. Read the complete reverse
   reference set from SPEC-HAL-248
   `FrameProjectionBindingRegistry::root_consumers` and obtain one
   `PreparedFrameProjectionBindingMutation` for every reference, naming its exact
   removal or replacement. Obtain `prepare_retire_guarded` from the root's
   focus-binding port, including the complete root-owned focus-domain set and
   current surface/domain testimony and prepare the router's removal of that exact
   port claim and every domain claim.
   If the operation explicitly names a
   surviving effective replacement root, validate that root's current guarded
   focus binding as the optional replacement; no state changes yet,
3. query `active_binding_guarded(token)`. When the unwind plan removes no frame,
   preserve the existing owner-correct rule: a retiring-owner binding uses
   SPEC-HAL-236 `prepare_deactivate_for_focus_retirement_guarded`; no binding uses
   direct SPEC-HAL-235 retirement; a surviving peer binding is preserved only
   when no peer restoration frame references the retiring set, otherwise
   preparation rejects unchanged. When the plan removes any frame, the current
   effective root is the only lawful active overlay owner. If it has an active
   binding, call SPEC-HAL-236
   `prepare_deactivate_for_focus_retirement_transition_guarded` with `Dismiss`,
   the complete retiring set, and the plan's exact final effective root focus
   domain or none; if it has no binding, call the matching SPEC-HAL-235 exact
   retirement-transition preparation directly. Any different active owner
   rejects unchanged. The exact transition consumes only the explicitly
   deactivated owner's tokens, removes the retiring graph, and publishes no
   intermediate return target. Obtain owner unregistration-after-retirement only
   when the retiring owner supplied that deactivation; a deactivated surviving
   peer remains registered and the inactive retiring owner uses ordinary
   prepared unregistration. Pass retiring-owner and surviving-peer surface-exit
   testimony to SPEC-HAL-234 and prepare one
   `PreparedRootSurfaceTeardown` so one mechanical batch contains every required
   overlay dismissal plus the complete deepest-to-root surface teardown. Any owner/focus/binding/claim/
   projection/surface-teardown preparation failure releases the token and leaves
   the root resident, routable, fully associated, focused exactly as before, and
   non-terminated,
4. after every fallible preparation succeeds, commit the compound overlay/focus retirement or direct focus retirement, then commit the root-focus binding retirement and router port/domain-claim removal. These steps consume every restoration frame affected by the retiring set plus the explicitly deactivated current owner's exact top suffix, remove all root-owned focus targets/domains, and publish only the exact post-unwind target, an unchanged lawful peer target when no unwind was required, or none; focus may not name the retiring root after this point,
5. commit every prepared frame-projection mutation (advancing replacement epochs
   and clearing affected ScreenHost views), mark the root non-routable, remove the
   prepared delegation suffix top-first as one router publication, remove or
   replace its priming reference, call
   `commit_owner_unregistration`, and remove its router association. With
   overlay, focus, binding, claim, projection, routing, and owner references now
   detached, issue SPEC-HAL-234 `RootSurfaceTeardownPublicationCapability` and
   commit whole-root teardown to hide/destroy every
   navigation surface deepest-to-root, release the z-band, destroy root-local
   overlay storage, and remove the registry root as reserved non-fallible
   finalization steps,
6. publish termination only after every finalization step completes, then release the one token.

No command may enter after the token is acquired and before step 6. Implementations must validate and reserve every resource needed for non-fallible finalization in steps 2–3; they must not expose a resident-but-detached or dead-root-focused failure state, run ordinary overlay restoration before root-focus retirement, or reacquire the domain guard inside a coordinator operation.

Destroying an interaction domain requires one explicit application lifecycle
operation. It first calls
`OverlayDomainCoordinator::begin_domain_transition` exactly once; acquisition
failure returns without mutation. Under that token it validates and reserves the
complete unwind without mutation, including every binding returned by
`FrameProjectionBindingRegistry::domain_consumers`, every root-focus binding-port
retirement, every router port/domain-claim removal, every prepared whole-root
surface teardown, and one complete set containing every focus domain in the interaction
domain. It obtains one prepared frame-projection removal for every consumer;
binding interaction-domain identity is immutable, so no retired-domain binding
may survive through replacement. It prepares one SPEC-HAL-235 focus-domain retirement with no replacement;
when an overlay is active, SPEC-HAL-236 combines its owner deactivation/tokens
with that same retirement plan. It also prepares every owner unregistration,
binding the active owner's plan to that deactivation, then prepares domain
unregistration against that complete reservation set. Preparation failure releases the token and leaves
the domain routable, focused, and unchanged. Non-fallible finalization commits
the compound overlay/focus retirement, commits every root-focus binding
retirement and clears every port/domain claim, marks the router record non-routable, unwinds the delegation stack in
top-down order, commits all prepared domain projection mutations and clears their
ScreenHost views, commits every prepared owner unregistration, removes the router
associations, executes each prepared whole-root surface teardown, destroys every
associated root's remaining registry/overlay storage, removes the router
record, enters `InteractionDomains` teardown, and calls
`commit_unregister_domain_guarded(token, prepared_domain_unregistration)` to consume the token and
coordinator record before finally unregistering the domain from
`InteractionDomains`. No coordinator operation in this sequence reacquires the
guard. No command may enter after the token is acquired, and no focus target or
domain, focus-domain/port claim, root association, priming identity, delegation
frame, frame-projection reference, owner port, navigation surface, or active
overlay binding may survive successful teardown.

The complete focus graph may also contain domain-local or other non-navigation
focus trees that are not router claims. Router preparation requires the exact
complete set of its own port/domain claims to be covered by the root testimonies
and the full graph-removal testimony; it does not invent claims for or reject
additional focus-authority-owned domains. The complete SPEC-HAL-235 preparation
still removes every registered focus domain.

Destroying a domain's primed root clears priming unless the same application operation explicitly names a replacement. After the overlay preparation required by Section 7.3 succeeds, returning from the top delegated target pops its explicit delegation frame and restores that frame's source. Destroying a root named by the active delegation chain is valid only as an explicit compound lifecycle operation that first performs the required owner-correct overlay preparation and unwinds the exact dependent suffix defined above; termination alone must not silently pop delegation or strand a dependent frame.

---

## 12. Failure Handling

If a navigation command carries an unknown `InteractionDomainId`, associates a root with the wrong domain, or targets an unregistered, suspended, or terminated root:
- the intent must be rejected
- no fallback root may be chosen
- NavigationRootRegistry must not be invoked

### INV-237-F1

Navigation failure must be explicit.

---

## 13. Forbidden Patterns

### INV-237-F2

No global navigation stack.

### INV-237-F3

No shared guards across roots.

### INV-237-F4

No widget-driven root switching.

### INV-237-F5

No implicit root activation.

### INV-237-F6

No focus-, visibility-, z-order-, hover-, or ancestry-inferred priming.

### INV-237-F7

No unscoped navigation broadcast and no history-boundary fallthrough to another root.

### INV-237-F8

No NavigationRootRegistry-owned priming, delegation, effective-root selection, or unscoped dispatch. The registry receives an explicit root identity only.

### INV-237-F9

No duplicate root-ID replacement, half-admitted root, or routing record without a live `InteractionDomainId`.

### INV-237-F10

No ambient, inferred, or post-navigation root-to-focus-domain lookup. Every
current surface association is guarded testimony from the registered
`NavigationRootFocusBindingPort`.

---

## 14. Runtime Migration Note (0.176.0-beta)

The live `InteractionDomainId` and `InteractionDomains` authority provides
per-window/domain identity and lifecycle without adding navigation members to the
closed engine-owned `InteractionDomainState`. The application-owned
`NavigationDomainRouter` now provides registered per-domain routing records,
exclusive root/owner/focus-port claims, priming, stable/retry snapshot phases,
router-issued overlay eligibility, and guarded routed commits.

`NavigationApplicationCoordinator` now publishes root admission atomically across
the hidden prepared registry root, overlay-owner registration, complete initial
focus graph, root focus-binding port, router claims/association, and initial
surface visibility. Ordinary routed navigation evaluates the application guard
before acquiring the domain token, revalidates router/host/binding/orchestrator
generations, and owns non-fallible prepared commit variants for existing-domain,
focus-graph, overlay-exit, and overlay-plus-focus-graph transitions. Hidden staged
surfaces and every authority preparation abort without public mutation.

The live per-interaction-domain `FocusManager` and central `FocusResolver`
implement focus-domain graphs, unique target membership, prepared activation and
restoration, batch graph mutation, and peer-safe retirement. The live
`OverlayDomainCoordinator` arbitrates one active stack per domain, binds admitted
root owners, seals router eligibility, and supplies exact owner-issued focus and
surface exit testimony. Routed `Suspend` and `Dismiss` operations now commit the
owner/focus/binding result before the same SPEC-HAL-234 compositor batch hides or
destroys those overlay surfaces.

The live `FrameProjectionBindingRegistry` now supplies versioned ScreenHost
bindings, exact root/domain consumer enumeration, and guarded per-consumer
prepared removal/replacement with synchronous stale-view clearing. Every
versioned binding retains its unique resolved orchestrator participants; each
source holds checked per-binding claims and prepared deltas for that exact
coordinator/domain. All existing-domain, focus-graph, overlay-exit, and
overlay-plus-focus-graph routed navigation variants prepare and publish host and
surface changes with the same token, while direct NavigationHost execution and
orchestrator dispatch reject a frame-bound source. Multiple ScreenHost consumers
remain guarded until the last claim is committed away.
`NavigationApplicationCoordinator` now owns application-level root retirement,
including roots referenced by the live delegation stack. The router prepares the
exact top-first dependent-frame suffix and final effective root; focus authority
prepares one exact-final graph retirement; an active current-effective peer owner
is explicitly dismissed with its own tokens; and SPEC-HAL-234 folds that peer
surface exit into the same mechanical batch as whole-root teardown. Projection,
focus-port, claim, routing, priming, owner, surface, and registry finalization
remain one non-fallible commit after complete preparation. Abort leaves every
authority unchanged, while a peer restoration dependency outside an explicit
delegation unwind still rejects without consuming or rewriting peer state.

`NavigationApplicationCoordinator` now owns delegation and return as the
Section 7 one-token prepared transactions. The router reserves an exact
cycle-safe push or top-frame pop and keeps optimistic readers in `Retry` from the
first focus commit through final frame publication. No-overlay transitions
activate the destination root focus domain; source-owned delegation overlays and
target-owned return overlays use the coordinator's compound exit-to-domain plan
plus an orchestrator-owned no-host-change surface batch; a target-owned overlay
is retained during delegation without root-focus theft. Unrelated peer overlays
reject unchanged, and suspended sources cannot route or obtain overlay
activation eligibility.

The SPEC-HAL-248 frame pull and the source-participation seam are live.
`NavigationApplicationCoordinator` also provides the standalone
snapshot/authorize/prepare transaction required for a frame-bound non-router
source. Compatibility standalone orchestrator and teardown entry points
self-issue crate-sealed publication proof only for roots that were never
application-admitted. Every application-admitted root now retains a permanent
same-domain participation claim from hidden admission through root destruction,
so direct host/orchestrator dispatch and compatibility teardown fail closed even
when no ScreenHost binding exists.

Full interaction-domain retirement is now a live prepared application
transaction. Under one token it snapshots the router's complete root, port,
focus-claim, priming, and delegation state; enumerates every immutable-domain
projection consumer and required window; prepares complete borrow-free focus
retirement, every root focus-port/claim retirement, all registered overlay-owner
unregistrations including inactive domain-local owners, exact active-root overlay
dismissal when present, one complete compositor/root batch, both domain records,
and the `InteractionDomains` window/domain removal. Commit publishes router
`Committing`, removes focus/ports/claims/projections/routing/owners/associations,
destroys the complete surface/root set, and removes router, coordinator, and
interaction-domain records through non-fallible finalization. Drop or any
preparation failure releases every reservation with all authorities unchanged.

Delegation-linked retirement proof covers top-target unwind to its source,
top-first removal through a non-top chain member, primed-source replacement by a
surviving formerly suspended target, explicit current-peer overlay dismissal in
the same surface batch, and complete prepared-drop abort with the original chain,
focus, owners, roots, and surfaces unchanged.

Migration proof must exercise delegation and return as one-token compound
transactions: preparation failure with no authority mutation, source-owned overlay
deactivation, target-owned overlay retention without root-focus replacement,
target-overlay deactivation on return, empty eligible focus resolution, and
rejection of any ordinary coordinator call that would reacquire the held guard.
It must prove that a suspended source cannot obtain overlay activation
eligibility and that the denied activation changes no owner, focus, binding, or
projection state. Root-admission proof must show hidden prepared-surface cleanup
on every fallible branch, batch creation of the initial focus tree, unique
port/domain claims, an initial current-surface/focus-domain association before
routability, and no visible/public root before final commit.
It must also prove that an ordinary routed navigation command evaluates every
guard/replacement and guard failure outside the domain token; rejects stale
router, host/history, binding, and guard-context generations without mutation;
prepares every fallible host/orchestrator/surface/overlay/binding/focus operation
under one acquired token; and then commits only non-fallible operations, with
focus/binding detach or replacement complete before surface publication.
Root/domain teardown proof must cover retirement
of every root-owned focus domain and restoration frame, preservation of a
surviving peer's active focus, explicit replacement focus or none for a retiring
active root, rejection while a peer-owned restoration frame references the
retiring root, and zero focus testimony naming a destroyed root. Focus-domain
membership proof must pair every prepared batch graph mutation with the
exact prepared `NavigationRootFocusBindingPort` and router claim change under one token,
prove globally unique port/domain claims plus ancestry/subtree closure,
including removal of the old current domain only when the same transaction
installs a lawful replacement current domain and, if that old domain is active,
selects that same replacement through focus retirement. It must cover atomic
parent+child admission and old-tree/new-tree swap. Root/domain teardown
proof must prepare every SPEC-HAL-248 projection removal/replacement, every
required overlay deactivation, every claim removal, every whole-root surface
teardown, every owner unregistration, and domain
unregistration before focus retirement. Failure injection at each preparation
point must leave every authority unchanged; after focus retirement begins, the
remaining prepared commits contain zero fallible calls.
Complete-domain proof must additionally cover exact immutable-domain consumer
removal and ScreenHost clearing, an active delegated owner and its external
overlay surface, every router root/port/focus claim, an inactive domain-local
owner, a lawful unclaimed domain-local focus tree, the empty-domain case,
missing projection-window rejection, permanent
application participation blocking direct registry/orchestrator execution, and
final absence of router, coordinator, `InteractionDomains`, surface, root,
owner, projection, focus, priming, and delegation state.

---

## 15. Non-Goals

This SPEC does not address:
- window management
- cross-process navigation
- persistence or restoration of roots

---

## 16. Summary (Normative)

- Navigation roots are isolated
- Multiple peer roots may be resident and visible in one interaction domain, and multiple domains may each have an effective root
- SPEC-HAL-248 frame bindings explicitly compose all simultaneously projected roots into one ScreenHost view
- Authority does not bleed across roots
- The application-owned NavigationDomainRouter owns priming and delegation routing state keyed by live interaction domains
- NavigationRootRegistry receives explicit root identities only
- Root admission is duplicate-safe and atomic across hidden surface preparation,
  registry insertion, coordinator owner registration, initial root-focus
  association, router association, routability, and final visibility publication
- Priming explicitly selects the base target of unscoped commands
- Delegation is explicit and constrained
- A delegation stack temporarily overrides and later restores priming
- OverlayDomainCoordinator establishes domain-wide overlay exclusivity before delegation or return commits
- NavigationDomainRouter is the sole issuer of same-token overlay activation
  eligibility for resident, non-suspended root owners
- NavigationDomainRouter uniquely claims every root focus-binding port and focus
  domain; root-owned sets are disjoint and ancestry/subtree closed
- The central focus resolver, not navigation routing or surface orchestration, owns focus selection
- A registered root-focus binding port supplies guarded current-surface/domain
  association and complete root-owned focus retirement without owning focus
- Structural/history navigation is root-local and never falls through across roots
- Ordinary navigation executes guards outside the domain token, revalidates
  checked generations, prepares every fallible authority, and commits
  non-fallibly under one token
- Leaving a delegated context is separate from navigation-history traversal
- Lifecycle boundaries are strict
- Root/domain teardown deactivates owner-correct overlays before atomically
  removing focus claims, projection/routing references, and every surface in
  prepared non-fallible finalization
