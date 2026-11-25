# SPEC-HAL-236: Overlay Layer Composition and Hit-Test Authority

**Status:** Draft (Binding upon adoption)
**Layer:** Overlay Composition / Input Routing / Hit-Test Authority
**Owner:** Core Architecture
**Depends On:** SPEC-HAL-002, SPEC-HAL-150, SPEC-HAL-151, SPEC-HAL-228, SPEC-HAL-231, SPEC-HAL-232, SPEC-HAL-235
**Applies To:** Menus, popups, context menus, dropdowns, tooltips, transient overlays, modal layers

---

## 1. Purpose

This SPEC defines overlay layer composition and hit-test authority in Halcyon.

It establishes:
- how overlays are composed relative to base surfaces,
- who owns hit-test decisions when overlays are present,
- how input routing is constrained by overlay presence,
- and how overlay lifetimes interact with navigation and focus.

The goal is to eliminate implicit z-order hacks, widget-local hit-test logic, and compositor-driven heuristics.

---

## 2. Scope

This SPEC governs:
- overlay stacking and ordering
- hit-test ownership and routing
- overlay activation and dismissal
- interaction between overlays and focus domains

This SPEC does not define:
- overlay visuals or animations
- rendering order within a surface
- gesture recognition semantics

---

## 3. Definitions

### Overlay

A transient UI layer rendered above a base surface.

### Overlay Layer

A logical grouping of overlays with a defined priority and scope.

### Hit-Test Authority

The component responsible for determining which entity receives input at a given point.

### Overlay Owner

The surface/context-local lifecycle owner of one overlay stack, identified by a stable `OverlayOwnerId`. It owns the actual overlay records and surfaces but has no authority to make its stack active across an interaction domain.

### OverlayDomainCoordinator

The application-owned domain-wide authority that admits, suspends, dismisses, and identifies the single active overlay stack for each live `InteractionDomainId`. It owns only active-stack bindings; it does not own overlay surfaces, navigation state, or focus targets.

Conceptual shape:

```text
OverlayDomainCoordinator {
    domains: HashMap<InteractionDomainId, OverlayDomainState>,
    owners: HashMap<OverlayOwnerId, RegisteredOverlayOwner>,
}

OverlayDomainState {
    active: Option<ActiveOverlayStackBinding>,
    transition_guard: DomainTransitionGuardState,
}

ActiveOverlayStackBinding {
    owner_id: OverlayOwnerId,
    stack_id: OverlayStackId,
}

RegisteredOverlayOwner {
    interaction_domain_id: InteractionDomainId,
    lifecycle_port: OverlayOwnerLifecyclePort,
    activation_admission: DomainLocal | RouterEligibilityRequired,
}
```

`OverlayDomainCoordinator` owns exactly one transition guard per registered
interaction domain. The guard has one explicit capability form:

```text
begin_domain_transition(domain_id) -> Result<DomainTransitionGuardToken>

activate_guarded(token, owner_id, stack_id, optional_eligibility) -> Result<()>
deactivate_guarded(token, owner_id, Suspend | Dismiss) -> Result<()>
prepare_deactivate_to_focus_domain_guarded(
    token,
    owner_id,
    Suspend | Dismiss,
    next_focus_domain,
) -> Result<PreparedOverlayExitTransition>
commit_deactivate_to_focus_domain_guarded(token, PreparedOverlayExitTransition) -> ()
prepare_deactivate_for_focus_graph_mutation_guarded(
    token,
    owner_id,
    Suspend | Dismiss,
    parent_before_child_additions,
    complete_subtree_removals,
    optional_replacement_active_domain,
) -> Result<PreparedOverlayFocusGraphTransition>
commit_deactivate_for_focus_graph_mutation_guarded(token, PreparedOverlayFocusGraphTransition) -> ()
prepare_deactivate_for_focus_retirement_guarded(
    token,
    interaction_domains,
    owner_id,
    Dismiss,
    retiring_focus_domains,
    optional_replacement_focus_domain,
) -> Result<PreparedOverlayFocusGraphTransition>
prepare_deactivate_for_focus_retirement_transition_guarded(
    token,
    interaction_domains,
    owner_id,
    Dismiss,
    retiring_focus_domains,
    exact_final_active_domain_or_none,
) -> Result<PreparedOverlayFocusGraphTransition>
commit_deactivate_for_focus_retirement_guarded(token, PreparedOverlayFocusGraphTransition) -> ()
prepare_complete_domain_retirement_guarded(
    token,
    interaction_domains,
    active_owner_id,
    complete_interaction_domain_focus_set,
) -> Result<PreparedOverlayCompleteDomainRetirement>
commit_complete_domain_retirement_guarded(
    token,
    interaction_domains,
    PreparedOverlayCompleteDomainRetirement,
) -> ()
push_frame_guarded(token, owner_id, stack_id, frame_id, optional_eligibility) -> Result<()>
pop_frame_guarded(token, owner_id, stack_id, frame_id) -> Result<()>
register_owner_guarded(token, owner_id, lifecycle_port, activation_admission) -> Result<()>
prepare_register_owner_guarded(token, owner_id, lifecycle_port, activation_admission)
    -> Result<PreparedOwnerRegistration>
commit_register_owner_guarded(token, PreparedOwnerRegistration) -> ()
unregister_owner(owner_id) -> Result<()> // ordinary wrapper acquires one token
prepare_owner_unregistration(token, owner_id) -> Result<PreparedOwnerUnregistration>
prepare_owner_unregistration_after_focus_retirement_guarded(
    token,
    PreparedOverlayFocusGraphTransition,
) -> Result<PreparedOwnerUnregistration>
prepare_owner_unregistration_after_complete_domain_retirement_guarded(
    token,
    PreparedOverlayCompleteDomainRetirement,
) -> Result<PreparedOwnerUnregistration>
commit_owner_unregistration(token, PreparedOwnerUnregistration) -> ()
unregister_domain_guarded(token) -> Result<()> // consumes token and record
prepare_unregister_domain_guarded(
    token,
    complete_owner_unregistration_reservations,
    optional_prepared_complete_domain_retirement,
) -> Result<PreparedDomainUnregistration>
commit_unregister_domain_guarded(token, PreparedDomainUnregistration) -> () // consumes token and record
active_binding_guarded(token) -> Result<Option<ActiveOverlayStackBinding>>
active_projection_guarded(token) -> Result<OverlayProjection>
```

A token is single-owner, non-cloneable, and valid only for the domain and guard
generation that issued it. The guarded operations verify the token and never
acquire the guard again. `register_domain` is the bootstrap operation that
atomically creates the domain record and its initially unlocked guard before the
domain accepts overlay work. Ordinary standalone owner registration,
`activate`, `deactivate`, owner-local frame push/pop, ordinary active/projection reads, and unregister operations acquire one token internally, call the matching guarded
operation, and release it. The convenience `activate` wrapper is available only
to `DomainLocal` owners. A router-managed activation is necessarily
application-orchestrated: acquire one token, obtain eligibility
from the router, call the guarded activation, and release that token. Compound root/domain lifecycle transactions acquire
one token explicitly and retain it across every coordinator, router, registry,
focus, and projection-binding mutation in that transaction. There is no
re-entrant or second guard acquisition path.

`RouterEligibilityRequired` owners are navigation-root owners managed by
SPEC-HAL-237. Their activation must carry a sealed, call-scoped,
non-cloneable `OverlayActivationEligibility` issued by
`NavigationDomainRouter::overlay_activation_eligibility_guarded` under the same
domain token. The capability names exactly one interaction domain, owner, and
router-state generation plus the registered root-focus binding generation and
owned-focus-domain set; it is constructible only by the router. The coordinator
validates its identity binding and requires every prepared overlay focus domain
to belong to that sealed set, but does not inspect or depend on router/port storage.
The router issues it only while that owner maps to a resident, non-terminated,
non-suspended root in the same domain. `DomainLocal` owners require no external
eligibility; coordinator registration/lifecycle state is their complete
admission source. Eligibility is consumed by the activation attempt and is
never stored. While live, it holds an exclusive guard-scoped lease on the named
router and focus-binding generations, so no router or root-focus association
mutation can advance either before coordinator activation consumes or drops the
capability. A router-managed inner-frame push obtains fresh eligibility to prove
that its new focus domain is in the current root-owned set; pop requires no new
eligibility because it removes an already admitted frame.

`register_owner_guarded` is the ordinary atomic convenience wrapper around
`prepare_register_owner_guarded` plus non-fallible commit. Compound root
admission uses the two-phase forms directly. Preparation reserves only the owner
ID/port/admission record under the held token and does not make the owner
registered or activatable. Dropping the prepared value has zero externally
observable mutation. `commit_register_owner_guarded` is valid only under the
same token and publishes the prevalidated registration without failure.

`unregister_owner` is an ordinary prepare-plus-commit convenience only for a
`DomainLocal` owner. A `RouterEligibilityRequired` owner is application-managed
and rejects direct unregistration; root or domain lifecycle must retire its
focus-binding port, router claims, projection references, and surfaces in the
same prepared transaction. `unregister_domain_guarded` is the corresponding
ordinary empty-domain convenience. Compound lifecycle transactions use the
explicit forms. Owner-unregistration preparation requires no active/prepared
stack, unless it is bound to the exact already-prepared deactivation owned by the
same transaction; in that case commit is lawful only after that deactivation
commits. Domain-unregistration preparation accepts the complete set of current
owner-unregistration reservations, verifies that committing exactly those plans
will empty the owner set and that the active binding is absent or covered by the
bound complete-domain deactivation plan, and reserves record removal without
mutation. Owner commits and final domain commit are then non-fallible under the
unchanged token; the domain commit is last and consumes the token/record.
Dropping any preparation before the first irreversible mutation changes
nothing.

The guard is a non-blocking logical mutation-exclusion capability, not a
`Mutex`, `RwLock`, or permission to block a UI-visible boundary. Acquisition
while another transition owns the domain returns an explicit busy/unavailable
result; the caller retries through normal scheduling. Holding the capability
prevents participating authority entry points from admitting another mutation,
but it never waits on user code or a platform lock.
In particular, a SPEC-HAL-231 navigation guard and its application-owned failure
handling run before this token is acquired. No callback, future, service request,
or other application/user code may execute while the token is held; guarded
navigation work is limited to generation revalidation, authority preparation,
and non-fallible commits.
Registered lifecycle/focus-binding ports used in that interval are sealed,
in-process authority records with bounded non-blocking prepare/commit methods;
they are not user-implemented callback or trait-object dispatch seams. Mechanical
surface preparation likewise may not wait on a platform callback or external
service under the token.

The `Result` on a guarded operation represents validation/preparation before
externally observed mutation. A compound lifecycle operation validates and
reserves every later guarded unregister/finalization through the explicit
preparations above while it still can fail.
After owner preparation commits or another authority is marked non-routable,
the remaining prevalidated guarded operations are non-fallible; no error may be
introduced after the transaction's first irreversible mutation.

`OverlayOwnerLifecyclePort` is a coordinator-held command capability, not owner
storage. It is a sealed in-process authority record, not a user callback or
open implementation trait. It exposes bounded, non-blocking whole-stack and admitted-frame lifecycle phases plus two read
operations:

```text
prepare_activate(stack_id) -> Result<PreparedActivation>
commit_activate(PreparedActivation, bottom_first_focus_tokens) -> ()
prepare_deactivate(stack_id, Suspend | Dismiss) -> Result<PreparedDeactivation>
commit_deactivate(PreparedDeactivation) -> ()
prepare_push_frame(stack_id, frame_id) -> Result<PreparedFramePush>
commit_push_frame(PreparedFramePush, focus_restoration_token) -> ()
prepare_pop_frame(stack_id, frame_id) -> Result<PreparedFramePop>
commit_pop_frame(PreparedFramePop) -> ()
register_inactive_stack(stack_descriptor) -> Result<()>
lifecycle_state() -> Inactive | Prepared(OverlayStackId) | Active(OverlayStackId)
project_active(stack_id) -> OverlayProjection
```

Preparation is fallible and must leave externally observed owner state
unchanged on failure. A successful preparation returns an owner-issued,
single-use token for the exact stack and guarantees that its matching commit is
non-fallible. `OverlayProjection` is a read-only ordered `SurfaceId` projection
of that exact active stack. The port does not transfer ownership of stack
records or surfaces to the coordinator.

`register_inactive_stack` is owner-local record admission, not domain binding
publication. It is allowed only while the owner has no active stack or live
lifecycle preparation and validates non-empty stack identity plus unique frame,
focus-domain, and surface identity before recording surfaces created after root
admission. Frame and surface identities are unique across the owner's complete
catalog, so dismissing one stack cannot leave another inactive record referring
to the destroyed surface. It never activates focus, publishes a coordinator
binding, or changes router eligibility.

`PreparedActivation` carries the complete ordered overlay-frame identity and
focus-domain sequence bottom-first. Before any owner or binding publication, the
coordinator verifies required activation eligibility, including membership of
every router-managed focus domain in the sealed root-owned set, and submits that sequence to SPEC-HAL-235
`prepare_overlay_focus_activation` under the same domain token. Focus preparation
checks but does not issue restoration generations. After both preparations succeed, focus commit returns the corresponding opaque tokens
bottom-first and `commit_activate` stores each token against its exact frame while
making the stack externally active. No activation path may publish owner or
binding state before focus commit has issued the checked generations and tokens.

`PreparedDeactivation` also carries the opaque SPEC-HAL-235 focus-restoration
tokens owned by every active overlay frame in that stack, ordered top-first.
It additionally exposes sealed owner-issued surface-exit testimony containing
the exact active surface sequence top-first and the declared disposition. Only
the compound application transaction may pass that testimony to SPEC-HAL-234 so
the navigation delta and owner-requested hide/destroy operations become one
mechanical batch; the testimony neither transfers surface authority nor commits
owner state.
Preparation validates that the sequence is complete. Commit makes the whole
stack externally inactive. `Suspend` retains its owner-local stack/frame records
for possible reactivation with fresh focus generations. `Dismiss` permanently
removes every deactivated frame record from the owner-local catalog; a later
stack with the same identity requires fresh explicit admission and may refer only
to newly live surfaces. While retaining the same domain transition guard,
the coordinator then submits every carried focus token to `FocusResolver` in
order before publishing binding removal. Token submission is deterministic and
non-fallible: each token restores its lawful predecessor or resolves to no
target. Neither `Suspend` nor `Dismiss` may leave an inactive overlay frame's
focus token live. Reactivating suspended owner-local records allocates new focus
restoration frames/generations.

`prepare_deactivate_to_focus_domain_guarded` is the compound form used when
deactivation and an explicit next focus-domain activation must commit together,
including SPEC-HAL-237 delegation and return. It obtains `PreparedDeactivation`,
then passes that exact top-first token sequence plus the caller-supplied next
focus domain to SPEC-HAL-235 `prepare_overlay_focus_exit_to_domain`; neither
owner nor focus state changes. The resulting `PreparedOverlayExitTransition`
owns both single-use preparations. Its non-fallible commit makes the overlay
owner inactive, consumes every restoration token through the prepared focus
exit, publishes the final next-domain target or none, and then removes the
active binding. It must not call ordinary token submission or prepare another
focus plan. Dropping it before commit has zero externally observable mutation.

`prepare_deactivate_for_focus_graph_mutation_guarded` is the generalized compound
form used when the overlay exit and a SPEC-HAL-235 root-owned focus graph change
must be one transaction. It obtains `PreparedDeactivation`, passes that exact
owner's top-first tokens plus the requested additions/removals/replacement to
`prepare_focus_domain_graph_mutation`, and exposes only the resulting sealed
graph testimony needed for the same transaction's SPEC-HAL-237 claim/binding
preparation. Commit makes the owner inactive, commits the prepared focus graph
and final target or none, and removes the active binding without publishing an
intermediate restoration target. The caller then commits the already prepared
root claim/binding result under the unchanged token. Dropping the compound value
changes neither owner nor focus graph.

`prepare_deactivate_for_focus_retirement_guarded` is the permanent-lifecycle
compound form used by navigation-root termination. It obtains an owner
`PreparedDeactivation` with `Dismiss`, then passes its exact top-first token
sequence, the caller's complete root-owned focus-domain set, and the optional
explicit replacement domain to SPEC-HAL-235
`prepare_focus_domain_retirement`. The focus resolver rejects any incomplete
domain subtree or replacement equal to/below the retiring set. Commit dismisses
the owner, consumes the overlay tokens, retires the root focus domains/targets,
publishes only the replacement target or none, and removes the active binding as
one non-fallible sequence. It does not perform ordinary restoration first and
cannot expose focus in the root being destroyed.
It is the closed no-additions, complete-retirement convenience over the same
generalized focus-graph compound machinery, not a second algorithm.
This compound form may carry only restoration tokens owned by that exact
deactivated owner. It cannot consume or rewrite a live peer owner's frame. When
another owner is active, root retirement uses the direct SPEC-HAL-235 preparation
with no peer tokens; if a peer frame references the retiring root, preparation
fails unchanged until that peer owner explicitly deactivates in its own
transaction.

Delegation-linked root retirement is the explicit owner-correct exception to
the direct-peer path: when retirement must unwind one or more delegation frames,
the current effective root's active owner is itself a required participant in
that same lifecycle transaction. Application orchestration calls
`prepare_deactivate_for_focus_retirement_transition_guarded` on that exact owner
with `Dismiss`, the retiring root's complete focus-domain set, and the exact
post-unwind active domain or none. The coordinator contributes only that owner's
tokens and sealed surface-exit testimony; SPEC-HAL-235 validates the exact final
focus result. If the active binding is not owned by the router's current
effective root, preparation rejects unchanged. This is explicit peer
deactivation, never silent token consumption or frame rewrite.

Complete interaction-domain retirement is the closed all-authority form. Under
one domain token, the coordinator snapshots every registered owner in the
domain, prepares the active router-root owner's exact `Dismiss` plus complete
focus-graph retirement when such an owner exists, reserves unregistration for
that owner against the same deactivation, reserves every other inactive owner,
then reserves domain removal against that exact complete owner set. An active
owner that is not associated with a resident router root cannot be silently
reinterpreted as navigation-owned; preparation rejects unchanged until its
owner deactivates through its own lifecycle path. Commit dismisses only the
explicit active owner, retires focus, unregisters every prepared owner, and
removes the coordinator domain record last. Inactive `DomainLocal` owners are
included in the complete owner set rather than blocking otherwise lawful domain
retirement.

`PreparedFramePush` names exactly one not-yet-active inner frame and its focus
domain. For a `RouterEligibilityRequired` owner, `push_frame_guarded` first
consumes fresh same-token eligibility and validates that domain against its
sealed root-owned set. It then prepares both owner and focus state, commits one new
focus restoration frame, passes its token to the non-fallible owner commit, and
publishes the expanded active projection before releasing the token.
`PreparedFramePop` names exactly the current top inner frame and carries its exact
focus-restoration token. `pop_frame_guarded` commits owner removal, submits that
token to `FocusResolver`, and publishes the contracted active projection under
the same token. Preparation failure leaves the stack, focus, and projection
unchanged. Push/pop of a non-top, wrong-stack, or non-admitted frame is rejected
before mutation. These operations mutate the already-admitted stack; they never
create a second coordinator binding.

`Inactive` means externally inactive and may contain owner-local suspended
records; those records have no coordinator binding, projection, hit tests, or
focus. `project_active` is valid only for the exact stack reported as `Active`.

---

## 4. Authority Model

### 4.1 Centralized Hit-Test Authority

### INV-236-A1

Hit-testing is owned by a centralized authority, not by widgets or surfaces.

Widgets may declare:
- hit-test geometry
- interaction eligibility

Widgets must not:
- short-circuit hit-testing
- override routing decisions

---

### 4.2 Domain-Wide Overlay Lifecycle Authority

### INV-236-A3

Every interaction domain that can host overlays, participates in multiroot
delegation, or owns a SPEC-HAL-248 frame-projection binding has exactly one
`OverlayDomainCoordinator` record. A record may have zero overlay owners and no
active binding; in that shape it still owns the domain transition guard used for
cross-authority serialization. A root-local overlay owner must obtain coordinator
admission before exposing its first active overlay. Nested overlays owned by the
already-admitted stack do not create another binding.

Coordinator operations are explicit and fallible. Deactivation carries one
declared disposition, `Suspend` or `Dismiss`:

- `register_domain(domain_id)` succeeds only for a live `InteractionDomainId` with no existing coordinator record,
- `register_owner(domain_id, owner_id, lifecycle_port, activation_admission)` succeeds only when the domain record is live, the owner ID is absent, and the port confirms no prepared or externally active stack; a SPEC-HAL-237 root owner must use `RouterEligibilityRequired`,
- `activate(domain_id, owner_id, stack_id, optional_eligibility)` succeeds only when the owner is registered in that domain, any required sealed router eligibility matches that owner/domain under the same token, and no different stack is active; under its one acquired token it obtains `PreparedActivation`, prepares the complete bottom-first focus activation, stages the binding, commits focus and obtains the exact restoration-token sequence, commits owner activation with those tokens, and publishes owner/binding/focus state atomically,
- `deactivate(domain_id, owner_id, disposition)` succeeds only for the admitted owner; under its one acquired token it obtains `PreparedDeactivation`, stages binding removal, commits the requested owner disposition, submits the prepared focus-restoration sequence top-first, and publishes the inactive owner/binding/focus result atomically; a caller that also needs an explicit next focus domain must use the prepared compound form instead,
- guarded navigation that changes the focus-domain graph uses `prepare_deactivate_for_focus_graph_mutation_guarded`, so owner deactivation, exact token consumption, batch graph mutation, and final focus are one preparation/commit path whose sealed testimony can be paired with SPEC-HAL-237 claims,
- guarded root teardown uses `prepare_deactivate_for_focus_retirement_guarded` rather than ordinary deactivation, so overlay dismissal and complete root-owned focus retirement share one focus plan and publish no doomed restoration target,
- `push_frame(domain_id, owner_id, stack_id, frame_id, optional_eligibility)` and `pop_frame(...)` succeed only for the admitted stack and perform the guarded prepared/non-fallible frame transaction defined above; router-managed pushes require fresh eligibility,
- `active(domain_id)` returns the sole admitted binding or none for a registered domain and rejects an unknown domain,
- `active_projection(domain_id)` returns the admitted owner's non-fallible `project_active(stack_id)` result or an empty projection when no binding exists,
- ordinary `unregister_owner(domain_id, owner_id)` succeeds only for a
  `DomainLocal` owner with no admitted, prepared, or active stack;
  `RouterEligibilityRequired` owners require application lifecycle, and compound
  teardown prepares owner unregistration against its exact prepared
  deactivation before committing it only after that deactivation,
- ordinary `unregister_domain(domain_id)` succeeds only after the admitted
  binding and registered-owner set are empty and the corresponding
  `InteractionDomains` domain has entered teardown; compound domain teardown
  prepares against the complete owner-unregistration reservation set and commits
  the domain record last.

The guarded forms `active_binding_guarded` and `active_projection_guarded` are mandatory
when the caller already owns a domain token; they validate that token and never
reacquire the guard. The ordinary read wrappers acquire and release one token
internally.

Repeated activation of the already admitted `(owner_id, stack_id)` is an
idempotent success only while the registered owner port confirms the stack
active. It cannot be used to re-admit a suspended/inactive router-managed owner;
that path requires a fresh eligibility capability. All coordinator transitions execute under exactly one acquired domain
transition guard token; guarded operations must not reacquire it.
Hit-test, focus, and frame-projection readers observe either the complete old
state or the complete new state, never the staged midpoint. Preparation failure
leaves the previous binding and owner state authoritative. Once preparation
succeeds, both owner commit and in-memory publication are non-fallible. A
different owner must not activate until the prior owner is confirmed inactive.
Per-owner overlay storage remains local; the coordinator is the cross-owner
arbitration point consumed by hit-test, focus, and multiroot delegation
orchestration.

Prepared owner state is not externally active: it receives no hit-tests, focus,
or visible active-surface publication. It is reachable only through the
single-use token held by the coordinator operation. Activation and deactivation
are atomic to event routing and frame projection.

---

### 4.3 Overlay Supremacy

When an overlay is active, it has exclusive hit-test priority over underlying content within its bounds.

### INV-236-A2

Underlying surfaces must not receive hit-test queries through an active overlay.

There is no "click-through" unless explicitly declared.

---

## 5. Overlay Composition Model

### 5.1 Layer Ordering

Overlay layers are ordered strictly above base surfaces.

Within overlays:
- ordering is explicit
- no implicit z-index inference is allowed

### INV-236-C1

Overlay order must be deterministic and explicitly declared.

---

### 5.2 Single Active Overlay Stack

At any moment:
- there is at most one active overlay stack per interaction domain

Nested overlays are allowed but must belong to the same stack.

The active stack is exactly the binding published by `OverlayDomainCoordinator`; per-owner stack contents or z-order cannot independently declare activity.

---

## 6. Hit-Test Resolution Process

Input routing first makes one non-blocking
`begin_domain_transition(interaction_domain_id)` admission attempt. During a
successful guarded attempt, SPEC-HAL-151 pointer capture has precedence. Only an
uncaptured pointer/touch event then enters centralized hit testing. The token is
held through target resolution and dispatch admission, then released before
invoking user/widget code; synchronous event-loop dispatch preserves that
admitted target for the event while allowing the handler to request a later
navigation/focus/overlay transaction without re-entrant guard ownership.

A busy acquisition defers the entire original event unchanged to the normal
next scheduling turn. No capture target, overlay, base target, focus target,
dispatch phase, or focus mutation is partially accepted; there is no
fallthrough and no duplicate delivery. Retry performs fresh authority
resolution against current state. If the input lifecycle declares the event no
longer retryable, it terminates as no-target/ignored. This same disposition
applies to captured, uncaptured, and focus-directed input in a guard-bearing
domain; capture precedence does not authorize an unguarded dispatch.

The deferred event retains its existing same-domain input-queue order. Another
busy result keeps that one record pending; it does not enqueue a duplicate or
allow later same-domain input to overtake it.

Hit-test resolution proceeds in this order:
1. The `OverlayDomainCoordinator`-admitted active stack (top-down)
2. Active surface (if no overlay consumes the event)
3. No target (event ignored)

### INV-236-H1

Hit-testing must stop at the first consuming overlay.

---

## 7. Overlay and Focus Interaction

Overlays establish their own focus domains.

When an overlay becomes active:
- owner preparation supplies the complete bottom-first frame/focus-domain sequence,
- the central focus resolver checks the complete candidate-generation range before any owner or binding publication without advancing its counter,
- under the same domain token, focus commit atomically issues the exact restoration-token sequence, owner commit stores those tokens, and the coordinator publishes the active binding atomically,
- underlying focus domains are suspended as part of that same non-fallible commit

When an overlay is dismissed:
- the coordinator clears the binding only after the stack is inactive,
- the prepared deactivation submits every dismissed frame's exact restoration token top-first according to SPEC-HAL-235 rules

Nested overlays remain one overlay stack but create nested focus-restoration
frames. Their tokens are consumed strictly top-first. Dismissing an inner frame
must not invalidate the still-live outer frame's token. Deactivating the whole
stack consumes the complete top-first token sequence before the guard is
released. Inner-frame push/pop uses only the guarded admitted-frame protocol in
Section 3; direct owner-local mutation of an active stack is forbidden.

---

## 8. Input Routing Guarantees

Once hit-testing resolves a target:
- all input for that event is routed exclusively to that target
- no secondary routing or bubbling across layers

### INV-236-R1

Overlay hit-test resolution fully determines input routing.

---

## 9. Navigation Interaction

### 9.1 Overlay During Navigation

Navigation events may:
- request coordinator-owned overlay dismissal/suspension
- invalidate overlay targets
- reassign hit-test authority

Overlay survival across navigation must be explicit.

### INV-236-N1

Overlays must not implicitly persist across navigation transitions.

---

### 9.2 Back Navigation

Back navigation:
- may explicitly request dismissal of the coordinator-admitted active overlay stack before affecting navigation state

Ordering must be deterministic and declared.

---

## 10. Surface Interaction

Overlays are surface-scoped.

### INV-236-S1

An overlay must not receive hit-tests if its owning surface is inactive or terminated.

Planned surface destruction requires the owning overlay stack/frame to be
deactivated or retired through the appropriate prepared coordinator transaction
before the surface batch commits. Surface destruction is not an authority to
consume restoration tokens or repair owner state afterward. Detection of an
unexpected destroyed owner surface fail-closes hit testing and reports a
lifecycle violation, but does not make that ordering conforming.

---

## 11. Failure Handling

If hit-test resolution yields no valid target:
- the input event is ignored
- no fallback target is selected

Silent rerouting is forbidden.

### INV-236-F1

Hit-test failure must be explicit and terminal.

Implementation proof for the guarded lifecycle contract must cover:
- whole-stack activation failure during owner preparation and focus
  candidate-range checking, with owner, binding, focus, and projection state unchanged;
- focus preparation holding exclusive generation-lane access, so a second
  preparation/mutation cannot obtain the same unissued candidate range;
- compound overlay exit plus next-domain focus activation consuming the exact
  top-first tokens, rejecting every descendant of the exiting domains, and
  exposing no intermediate restoration target or second focus plan;
- compound overlay exit plus batch focus-graph mutation exposing sealed claim
  testimony before non-fallible owner/focus/claim commit, including atomic
  parent+child addition and current-domain swap;
- routed `Suspend` and `Dismiss` passing the exact owner-issued top-first surface
  plan into one SPEC-HAL-234 batch, with abort preserving every live overlay
  surface and token;
- compound overlay dismissal plus root-focus retirement consuming the same token
  sequence and removing every root-owned focus domain before root destruction,
  with only an explicit surviving replacement target or none;
- delegation-linked retirement explicitly dismissing the current effective
  peer owner, consuming only its exact top-first tokens, retiring a different
  root tree, and publishing the final post-unwind focus domain without an
  intermediate restoration target;
- a peer-owned active restoration frame referencing the retiring root rejecting
  retirement unchanged until owner-correct peer deactivation, with no token
  consumption or frame rewrite;
- owner/domain teardown preparing the complete owner-deactivation and owner/domain
  unregistration set before the first irreversible focus or lifecycle mutation,
  with each injected preparation failure dropping every reservation and leaving
  owner, binding, focus, projection, and coordinator state unchanged;
- owner/domain teardown commits remaining non-fallible after focus retirement,
  including exact prepared-owner binding to any required deactivation and domain
  unregistration only after every prepared owner unregistration has committed;
- nested frame push/pop plus whole-stack deactivation, proving exact bottom-first token installation and top-first restoration without invalidating a still-live outer frame;
- busy guard acquisition as an immediate non-mutating result, with no blocking or guarded-operation re-acquisition;
- busy input admission deferring one complete event, with no partial dispatch,
  fallback, duplication, reordering, or focus mutation before each fresh retry;
- one compound caller retaining the same token across coordinator, focus, and publication work so no reader can observe a prepared midpoint.

---

## 12. Forbidden Patterns

### INV-236-F2

No widget-local hit-test overrides.

### INV-236-F3

No compositor-driven hit-test heuristics.

### INV-236-F4

No implicit click-through overlays.

### INV-236-F5

No overlay persistence without declared ownership.

### INV-236-F6

No root-local owner may activate or publish an overlay stack without `OverlayDomainCoordinator` admission, and no coordinator may own the stack's surfaces or infer activity from z-order.

---

## 13. Non-Goals

This SPEC does not cover:
- accessibility hit-testing
- gesture arbitration
- pointer capture semantics
- drag-and-drop (see SPEC-HAL-241)

---

## 14. Summary (Normative)

- Overlays have explicit authority
- OverlayDomainCoordinator owns domain-wide active-stack admission
- One coordinator-owned domain transition guard token serializes standalone and compound lifecycle operations without re-acquisition
- Overlay owners retain root/surface-local stack lifecycle and storage
- Registered lifecycle ports provide prepared/non-fallible atomic whole-stack and nested-frame activation/deactivation with focus-token installation
- Router-managed root owners require same-token sealed activation/frame-push
  eligibility carrying current root-owned focus-domain testimony; suspended
  sources and unregistered overlay focus domains cannot activate
- Root teardown uses one compound overlay-dismissal/focus-retirement plan
- Hit-testing is centralized
- Busy guarded input is deferred in-order as one unchanged event
- Overlays supersede surfaces
- Focus and input are strictly scoped
- Implicit behavior is forbidden
