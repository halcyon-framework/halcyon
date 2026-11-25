# SPEC-HAL-235: Focus Resolution and Routing Authority

**Status:** Draft (Binding upon adoption)
**Layer:** Input / Focus Authority / Navigation Coordination
**Owner:** Core Architecture
**Depends On:** SPEC-HAL-002, SPEC-HAL-038, SPEC-HAL-150, SPEC-HAL-151, SPEC-HAL-228, SPEC-HAL-231, SPEC-HAL-232
**Applies To:** Keyboard focus, pointer focus, focus traversal, routed input, surface-local focus domains

---

## 1. Purpose

This SPEC defines focus resolution and routing authority within Halcyon.

It establishes:
- who decides where focus goes,
- how focus is scoped,
- how focus moves across surfaces and navigation boundaries,
- and how input is routed once focus is resolved.

The primary goal is to eliminate implicit, widget-driven, or compositor-driven focus behavior and replace it with explicit, authority-based focus resolution.

---

## 2. Scope

This SPEC governs:
- focus ownership
- focus routing
- cross-surface focus transitions
- focus invalidation on navigation or surface termination

This SPEC does not define:
- visual focus indicators
- accessibility semantics
- IME behavior
- text editing rules

---

## 3. Definitions

### Focus Target

A logical endpoint eligible to receive input.

### Focus Domain

A bounded scope, identified by a stable `FocusDomainId`, within which focus may move freely. Every focus domain belongs to exactly one live `InteractionDomainId`.

### Focus Resolver

The authority responsible for selecting the active focus target.

### Focus Route

The path keyboard, text, and explicit focus-directed actions follow after focus resolution. Pointer and touch routing are governed by capture and hit testing, not by the focus route.

---

## 4. Authority Model

### 4.1 Singular Focus Authority

### INV-235-A1

At any moment, there is at most one authoritative focus target per interaction domain. Zero targets is lawful when the domain has no eligible target, loses window focus, or resolution fails.

There is no distributed or shared focus state.

The central Focus Resolver is the authority over the existing `FocusManager` stored in each live `InteractionDomainState`; it must not create a second target store beside that manager. The target `FocusManager` extends its existing focused-target/traversal state with exactly these authority records:

```text
FocusManager {
    focused: Option<TreeWidgetId>,
    focus_domains: HashMap<FocusDomainId, FocusDomainRecord>,
    active_focus_domain: Option<FocusDomainId>,
    restoration_stack: Vec<FocusRestorationFrame>,
    next_restoration_generation: u64,
    ... existing focus-chain, scope, and lifecycle fields
}

FocusDomainRecord {
    interaction_domain_id: InteractionDomainId,
    parent: Option<FocusDomainId>,
    targets: HashMap<TreeWidgetId, FocusTargetRecord>,
}

FocusTargetRecord {
    surface_id: Option<SurfaceId>,
    eligible: bool,
}

FocusRestorationFrame {
    generation: u64,
    activated_domain: FocusDomainId,
    superseded_domain: Option<FocusDomainId>,
    superseded_target: Option<TreeWidgetId>,
}

FocusRestorationToken {
    interaction_domain_id: InteractionDomainId,
    generation: u64,
    activated_domain: FocusDomainId,
    superseded_domain: Option<FocusDomainId>,
}
```

These are fields of the existing per-interaction-domain `FocusManager`, not a
resolver-global or application-owned store. Registration validates that every
record names that manager's `InteractionDomainId`. One `TreeWidgetId` may be
registered in at most one focus domain in that manager at a time, and the
selected target must be an eligible member of `active_focus_domain`. The selected target, active
focus domain, restoration frames, and surface projected from the selected
target therefore come from one authoritative per-domain state.

The resolver exposes one read-only frame projection seam:

```text
focus_projection_guarded(interaction_domain_transition_capability, interaction_domain_id)
    -> Result<FocusProjection>

FocusProjection {
    focused_surface: Option<SurfaceId>,
}
```

Every focus target eligible for a projected UI frame must register its owning
surface. `focused_surface` is `Some` only when the authoritative selected target
is live and surface-backed; zero selected targets yields `None`. The read never
selects, repairs, or falls back to another target and cannot mutate focus state.

Every focus read or mutation that participates in a cross-authority interaction-domain
transaction uses an already-acquired, non-cloneable
`InteractionDomainTransitionCapability`. This is sealed call-scoped evidence containing
the interaction-domain identity and guard generation; FocusResolver defines and validates
the capability shape but neither issues nor retains it. A higher-layer transaction
coordinator supplies it. SPEC-HAL-236 binds its coordinator-issued
`DomainTransitionGuardToken` to this capability, preserving the dependency direction
from overlay orchestration to FocusResolver rather than making FocusResolver depend on
overlay authority. The guarded focus transaction seam is:

```text
prepare_overlay_focus_activation(
    interaction_domain_transition_capability,
    focus_domains_bottom_first,
) -> Result<PreparedOverlayFocusActivation>

commit_overlay_focus_activation(
    interaction_domain_transition_capability,
    PreparedOverlayFocusActivation,
) -> ordered Vec<FocusRestorationToken> // bottom-first, non-fallible

prepare_overlay_focus_exit_to_domain(
    interaction_domain_transition_capability,
    top_first_restoration_tokens,
    next_focus_domain,
) -> Result<PreparedOverlayFocusExit>

commit_overlay_focus_exit_to_domain(
    interaction_domain_transition_capability,
    PreparedOverlayFocusExit,
) -> () // non-fallible; consumes exact tokens and selects next lawful target or none

prepare_focus_domain_activation(
    interaction_domain_transition_capability,
    focus_domain_id,
) -> Result<PreparedFocusDomainActivation>

commit_focus_domain_activation(
    interaction_domain_transition_capability,
    PreparedFocusDomainActivation,
) -> () // non-fallible; selects one lawful target or none

prepare_focus_domain_graph_mutation(
    interaction_domain_transition_capability,
    parent_before_child_additions,
    complete_subtree_removals,
    top_first_overlay_restoration_tokens,
    optional_replacement_active_domain,
) -> Result<PreparedFocusDomainGraphMutation>

prepare_focus_domain_graph_transition(
    interaction_domain_transition_capability,
    parent_before_child_additions,
    complete_subtree_removals,
    top_first_overlay_restoration_tokens,
    exact_final_active_domain_or_none,
) -> Result<PreparedFocusDomainGraphMutation>

commit_focus_domain_graph_mutation(
    interaction_domain_transition_capability,
    PreparedFocusDomainGraphMutation,
) -> () // non-fallible; publishes the complete graph/membership/focus result

prepare_focus_domain_retirement(
    interaction_domain_transition_capability,
    retiring_focus_domains,
    top_first_overlay_restoration_tokens,
    optional_replacement_focus_domain,
) -> Result<PreparedFocusDomainRetirement>

prepare_focus_domain_retirement_transition(
    interaction_domain_transition_capability,
    retiring_focus_domains,
    top_first_overlay_restoration_tokens,
    exact_final_active_domain_or_none,
) -> Result<PreparedFocusDomainRetirement>

commit_focus_domain_retirement(
    interaction_domain_transition_capability,
    PreparedFocusDomainRetirement,
) -> () // non-fallible; consumes tokens, retires domains, selects replacement or none

prepare_complete_focus_domain_retirement(
    interaction_domain_transition_capability,
    complete_interaction_domain_focus_set,
    complete_top_first_overlay_restoration_tokens,
) -> Result<PreparedCompleteFocusDomainRetirement>

commit_complete_focus_domain_retirement(
    interaction_domain_transition_capability,
    PreparedCompleteFocusDomainRetirement,
) -> () // non-fallible; removes the complete graph and publishes no active focus domain

submit_restoration_tokens_guarded(
    interaction_domain_transition_capability,
    top_first_tokens,
) -> () // non-fallible
```

Preparation validates domain membership as applicable and checks any complete
candidate restoration-generation range required for new frames without overflow.
When an operation selects or restores focus, preparation also computes the
deterministic target result. It does not change the generation counter, active
domain, target, restoration stack, or domain registry. A prepared focus-transition
value records its unissued candidate range (which may be empty) and is bound to
the same non-cloneable guard capability; it does
not allocate or expose a generation. It exclusively borrows that interaction
domain's focus-generation lane until consumed or dropped, so no second focus
preparation or focus mutation can enter under the capability while it is live.
Dropping it has zero mutation. Commit consumes the prepared value and may
run only under that unchanged capability, atomically advances the counter by
the prepared frame count, issues those checked generations when non-empty, and applies the prepared state as a non-fallible
step. An empty eligible-target set is a lawful prepared result whose commit
selects no target; an unknown/wrong-domain focus domain or checked-counter
exhaustion fails preparation before any authority changes. For overlay
activation, the returned restoration-token sequence corresponds one-for-one with
the requested focus domains and is stored by the overlay owner against the same
bottom-first frame sequence. Each prepared restoration frame also captures the
exact currently selected `TreeWidgetId` from its superseded domain, or `None`;
the opaque token does not expose or duplicate that target. In any interaction domain with a guard-bearing coordinator
record, every externally observable focus mutation requires a valid capability,
including ordinary focus movement, target invalidation, and focus-domain
registration/removal. Application orchestration may wrap a standalone focus
operation by acquiring and releasing one domain token, or incorporate it into an
existing compound transaction; no focus mutation may bypass that admission.

`prepare_focus_domain_graph_mutation` is the sole root-lifecycle graph/membership
mutation seam. Its addition list may create one domain or an arbitrary bounded
parent/child forest in one transaction. Every addition names an absent domain ID
and either an existing surviving same-interaction-domain parent or an earlier
addition; parent cycles, duplicate IDs, wrong-domain parents, and parents removed
by the same mutation are rejected. Added domains begin with empty target maps;
target registration remains a separate guarded focus mutation.

The removal set must contain every descendant of every removed domain in the
post-addition candidate graph. Additions and removals are disjoint. Preparation
validates the complete target invalidation and every restoration frame touching
the removal set. If the active domain is removed, an explicit replacement must
name a surviving or newly added domain outside the removed subtrees; preparation
computes its deterministic target or none. If the active domain survives, an
unrelated replacement is rejected and the selected target is preserved exactly.
Inactive, restoration-insensitive removal supplies no restoration tokens.
Active or restoration-sensitive removal supplies the exact owner-provided
top-first tokens and uses the same no-intermediate-restoration rules as
retirement.

The prepared value records the exact before/after domain-parent graph, addition
and removal sets, token disposition, active-domain result, target result, and
checked generation effects. It exposes sealed, read-only
`FocusDomainGraphMutationTestimony` for the same transaction so SPEC-HAL-237 can
prepare its unique root claim and binding-port membership update without
inferring focus ancestry. That testimony is bound to the prepared value and
unchanged domain capability; it cannot mutate focus or outlive commit/drop.
Commit invalidates removed targets/frames, installs every added empty domain,
publishes only the final active-domain/target result, and advances the prepared
generation lanes as one non-fallible step. Thus parent+child admission, complete
subtree removal, and old-to-new current-domain replacement require one focus
preparation rather than a sequence of partially visible single-domain calls.

`prepare_focus_domain_retirement` is the closed whole-set convenience used by
root/domain teardown. It lowers to the same graph-mutation machinery with no
additions and the supplied complete retirement set/tokens/replacement. There is
no second retirement algorithm.

The exact-transition variants are the compound delegation-unwind seam. Unlike
ordinary graph mutation, their owner-provided top-first tokens explicitly pop
that exact active overlay suffix even when its focus domains survive the graph
change. Preparation then rejects any remaining restoration frame touching a
removed domain and validates the caller's exact final active domain, or exact
`None`, against the post-mutation graph. A surviving final domain receives its
deterministic lawful target; `None` clears the active domain and target. An
unconsumed restoration frame whose activated domain differs from the requested
final domain rejects unchanged. These variants allow one prepared value to
dismiss the effective delegated overlay, retire a different root's complete
focus tree, and publish only the final surviving source/replacement focus result
without sequential return commits or an intermediate target.

`prepare_overlay_focus_exit_to_domain` is the compound deactivation/delegation
seam. It validates the complete top-first restoration-token sequence and the
explicit next focus domain. Using `FocusDomainRecord::parent`, it rejects the
next domain if it is equal to or a descendant of any exiting overlay focus
domain, and it rejects any computed target outside the exact lawful next domain.
It then computes the final target or none and holds the same exclusive
focus-generation lane without consuming a token or changing focus.
Its non-fallible commit consumes every restoration token exactly once and
publishes only the final next-domain focus result; an intermediate restoration
target is never externally observable. Ordinary overlay dismissal with no
explicit next-domain transition continues to use guarded top-first token
submission.

`prepare_focus_domain_retirement` is the owner-neutral compound seam used when a
higher-layer lifecycle transaction permanently removes a declared set of focus
domains, including navigation-root termination. It validates that the complete
retirement set belongs to this interaction domain and contains every descendant
whose parent is retired; validates and absorbs any active overlay
restoration-token sequence owned by those domains; rejects a replacement domain
equal to or below the retiring set; and computes the final replacement target or
none. If the authoritative active domain and selected target are wholly outside
the retiring set, preparation preserves them exactly and rejects an unrelated
replacement request; retirement never steals focus from a surviving peer root.
Every restoration frame whose activated or superseded domain intersects the
retiring set must be covered by the supplied top-first token sequence; an
uncovered surviving-owner frame rejects preparation instead of silently
invalidating that owner's token.
Only the overlay owner that holds a frame's opaque token may contribute it. If a
live peer-owner frame has `superseded_domain` or `activated_domain` in the
retiring set, root retirement cannot consume, rewrite, or invalidate that frame;
preparation fails unchanged until the peer owner explicitly deactivates through
its own SPEC-HAL-236 transaction, after which retirement may be retried.
Commit consumes the supplied tokens, invalidates affected restoration
frames and target registrations, removes the complete domain set, and publishes
only the final replacement result. No intermediate restoration into a retiring
domain is observable. Dropping the prepared value changes nothing.

Whole interaction-domain teardown uses the complete-domain form. Its prepared
candidate does not retain a mutable sub-borrow of `FocusManager`, because the
outer application transaction must retain exclusive mutable ownership of
`InteractionDomains` through final domain removal. Preparation still validates
the exact current graph: the removal set must equal every registered focus
domain, the owner-provided top-first token sequence must consume the complete
restoration stack, and the only lawful final active domain/target is none. It
captures the complete before-state and exposes the same sealed graph testimony
needed by every root focus-binding port and router claim retirement. Commit
revalidates that captured state and removes the complete graph, target map,
restoration stack, and active selection without a fallible operation. This is a
borrow-free prepared representation of the same retirement law, not a second
focus authority.

---

### 4.2 Resolution Authority

Focus resolution is owned by a central focus resolver, not by widgets.

Widgets may:
- declare focus eligibility
- declare traversal relationships

Widgets must not:
- assign focus
- steal focus
- persist focus state independently

### INV-235-A2

Widgets are declarative participants, not focus authorities.

---

### 4.3 Navigation Interaction

Navigation events may:
- invalidate focus
- relocate focus
- restrict focus scope

Focus preparation may compute a deterministic target only after the final
guard-authorized navigation candidate exists. Focus publication occurs inside
the same non-fallible compound commit as navigation/binding publication, never
from an unguarded or replaced candidate.

### INV-235-A3

Navigation state constrains focus, not the other way around.

---

## 5. Focus Domains

### 5.1 Domain Definition

A focus domain is typically associated with:
- a navigation surface
- an overlay
- a modal context

Domains form an acyclic parent forest: each domain has at most one parent and one
interaction-domain membership. Domains are non-overlapping. A complete subtree
is defined by the authoritative parent edges in `FocusManager`, never by widget,
surface, or navigation inference.

---

### 5.2 Domain Isolation

Focus must not cross domain boundaries implicitly.

### INV-235-D1

Cross-domain focus transitions require explicit authorization.

---

### 5.3 Active Domain

Only one focus domain is active at a time per `InteractionDomainId`. `Interaction channel` is not a second identity or storage scope.

Inactive domains:
- retain declarative structure
- do not receive routed input

---

## 6. Focus Resolution Process

Focus resolution occurs in phases:
1. Determine active focus domain
2. Enumerate eligible focus targets
3. Apply resolution rules (order, priority, constraints)
4. Select one focus target or none
5. Route focus-directed input only when a target was selected

This process is deterministic.

### INV-235-R1

Given identical state, focus resolution must yield identical results.

---

## 7. Input Routing

Once focus is resolved:
- keyboard, text, IME-adjacent commands within this SPEC's scope, and explicit focus-directed actions are routed exclusively to the active focus target,
- bubbling of those events must remain within the selected focus domain and its owning `InteractionDomainId`,
- pointer and touch events first follow SPEC-HAL-151 pointer capture and otherwise SPEC-HAL-236 centralized hit-test resolution; focus must not redirect the event to a different target,
- a pointer/touch target may submit a subsequent explicit focus request, but that request affects later focus-directed input only.

When the interaction domain has a SPEC-HAL-236 transition guard, input routing
uses that domain's one non-blocking admission attempt. A busy result defers the
entire original event unchanged to the normal next scheduling turn. The busy
attempt must not resolve a fallback target, dispatch any phase, mutate focus,
or duplicate the event. On retry, capture/focus/hit-test authority is resolved
fresh against the then-current state. If the event is no longer retryable under
the input lifecycle, it terminates as no-target/ignored rather than falling
through to another domain or target.

Deferred events retain their existing order in that interaction domain's input
queue. Repeated busy results keep the same event record pending; they do not
append another retry record or allow later same-domain input to overtake it.

There is no "broadcast" input.

### INV-235-R2

Focus-directed input routing follows focus resolution strictly. Pointer/touch routing follows capture or hit-test authority strictly.

---

## 8. Surface Interaction

### 8.1 Surface Activation

When a surface becomes active:
- its focus domain may become active
- previous domain may be suspended

Focus transfer must be explicit.

---

### 8.2 Surface Termination

Before a planned surface is destroyed, the guarded compound transaction must
invalidate every target within it and select the explicit lawful replacement or
none. Surface destruction is not a trigger for later best-effort focus repair.
If an out-of-contract destruction is detected, the resolver's next guarded
admission fail-closes by invalidating the dead target before further input and
reports the lifecycle violation; it does not legitimize that ordering or bypass
the domain capability requirement.

### INV-235-S1

No focus target may survive surface termination.

---

## 9. Overlay Interaction

Overlays (menus, dialogs, popups):
- establish their own focus domains
- may temporarily supersede underlying domains

Activating one or more overlay focus domains occurs only through the guarded
prepare/commit seam in Section 4.1. Preparation checks the complete next
generation range for every frame bottom-first but does not advance the counter
or issue a generation. Commit atomically advances the counter and pushes the matching
`FocusRestorationFrame` sequence, makes the final frame's `activated_domain`
active, and returns the matching opaque tokens bottom-first. Nested overlay
activation pushes exactly one additional prepared frame; it does not rewrite or
invalidate any older frame. Candidate-range arithmetic is checked; exhaustion rejects
the complete preparation without mutation rather than wrapping, reusing a
generation, or admitting an overlay without its token.

A restoration token is current only when it exactly matches the top frame and
that frame's `activated_domain` is still the active focus domain. Dismissing the
top overlay consumes that token once and pops exactly one frame. If its
`superseded_target` is still registered, eligible, and a member of the surviving
`superseded_domain`, the resolver restores that exact target as required by
SPEC-HAL-137. Otherwise it performs the deterministic constrained fallback for
that superseded domain, including nearest lawful ancestor when available, or
selects none. After an inner frame is popped, the
unchanged outer frame becomes top/current again, so nested dismissal cannot
invalidate the outer token. An out-of-order, replayed, wrong-domain, or
generation-mismatched token performs no restoration. The resolver then selects
another explicitly authorized target or none only according to the current
domain constraints; it never treats the rejected token as fallback authority.

There is no direct focus-domain destruction path. A successful prepared graph
mutation invalidates the removed domains' targets and only the restoration frames
covered by its exact owner-provided token sequence. Any uncovered peer frame
rejects preparation as defined in Section 4.1. If removal affects the active
domain, the same commit publishes its precomputed lawful replacement or none
before further focus-directed input. Committed/issued generations are never
reused during the lifetime of the owning `InteractionDomainState`. Candidate
values dropped before commit were never generations and carry no token identity.

Implicit focus leakage is forbidden.

---

## 10. Failure and Denial

If focus resolution fails:
- no focus is assigned
- focus-directed input is ignored or deferred

Silent fallback to arbitrary targets is forbidden.

### INV-235-F1

Focus failure must be explicit, not masked.

---

## 11. Forbidden Patterns

### INV-235-F2

No widget-driven focus stealing.

### INV-235-F3

No compositor-driven focus heuristics.

### INV-235-F4

No implicit focus inheritance across navigation.

### INV-235-F5

No persistent focus IDs across destroyed surfaces.

---

## 12. Runtime Status (0.176.0-beta)

The live per-interaction-domain `FocusManager` now owns `FocusDomainId` graph
records, globally unique target membership, the active focus domain, exact
target-to-surface testimony, the checked restoration-generation lane, and the
LIFO restoration stack. `FocusResolver` exposes capability-bound prepared and
non-fallible commit seams for target mutation/selection, domain activation,
bottom-first overlay activation, top-first restoration, explicit overlay exit,
batch parent-before-child graph mutation, complete subtree swap/removal, and the
closed retirement convenience. Exact-final graph/retirement transition variants
now consume an explicitly deactivated owner's complete top restoration suffix
while retiring an independent root tree, enabling delegation-linked root
retirement without a fallible focus operation after unwind begins. Prepared graph testimony is borrow-bound to the
exact plan. Retirement rejects peer-owned frames that reference a retiring
domain instead of consuming or rewriting them. `InteractionDomainState`
constructs its manager with the actual allocated `InteractionDomainId`.
Complete interaction-domain retirement uses a borrow-free prepared focus
candidate that proves the exact full graph and restoration stack, publishes
sealed complete-removal testimony to every port/router participant, and commits
no active domain or target while the application retains outer ownership of
`InteractionDomains`.

The `InteractionDomainTransitionCapability` issuer remains crate-sealed for the
SPEC-HAL-236 coordinator. SPEC-HAL-237 admission, ordinary routed transactions,
delegation/return and root retirement, including top-down active-chain unwind,
consume these prepared seams together with exact root binding/router testimony. Legacy direct `FocusManager` and
`NavigationSurfaceOrchestrator::focused_surface` paths remain compatibility
observations only and must not be treated as the compound navigation authority.

---

## 13. Non-Goals

This SPEC does not cover:
- accessibility APIs
- screen readers
- focus visuals
- platform-specific shortcuts

---

## 14. Summary (Normative)

- Focus is centralized
- Resolver state, target-to-surface metadata, active-domain selection, and the LIFO restoration stack are fields of the live interaction domain's existing `FocusManager`
- Cross-authority focus reads and mutations use one already-acquired domain transition guard and prepared/non-fallible commit seams
- One guarded focus-domain graph mutation prepares complete parent-before-child
  additions, subtree removals, restoration disposition, and active-domain swap
  before one non-fallible commit and pairs its sealed ancestry testimony with the
  root claim/binding change under the same domain guard
- Focus preparation holds exclusive generation-lane access without advancing the
  counter; consuming commit issues checked non-reused generations
- Overlay restoration frames retain resolver-owned previous-target testimony and
  restore it exactly when still lawful before deterministic fallback
- Compound exit and retirement reject replacement focus inside an exiting or
  retiring focus-domain subtree
- Ordinary retirement never consumes or rewrites a surviving peer overlay
  owner's restoration frame; delegation-linked exact transition may consume
  only the complete top suffix contributed by the explicitly deactivated
  current-effective owner
- Resolution is deterministic
- Domains are explicit
- Navigation constrains focus
- Keyboard/text input follows focus; pointer/touch follows capture or hit testing
- Busy guarded input is deferred in-order as one unchanged event
- Overlay restoration tokens are validated and may resolve to no target
- Widgets declare, never decide
