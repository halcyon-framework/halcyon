# SPEC-HAL-248: Runtime Navigation Projection Pull Contract

**Status:** Binding
**Series:** SPEC-HAL (Navigation / Runtime)
**Layer:** Runtime Frame Loop / Projection Consumption
**Authority Owner:** Application `FrameProjectionBindingRegistry` plus mechanical runtime glue (`App::run_ui_loop` and `App::run`)
**Depends On:** SPEC-HAL-233, SPEC-HAL-234, SPEC-HAL-235, SPEC-HAL-236, SPEC-HAL-237, SPEC-HAL-246, SPEC-HAL-247

---

## 1. Purpose

This SPEC codifies the live guarded runtime frame-projection pull pipeline.

Navigation state is produced by NavigationHost and translated into compositor surface mutations by one `NavigationSurfaceOrchestrator` per context. Overlay composition and focus are independent authorities. The frame-projection pull pipeline reads every explicitly bound navigation context plus the overlay and focus authorities for one interaction domain, mechanically resolves their surface identifiers through the compositor, and deposits one coherent result in ScreenHost.

The normal pipeline is a read-only testimony-composition boundary apart from replacing/clearing its ScreenHost sink. Its sole defensive lifecycle mutation is removing a proven orphaned ScreenHost binding under the domain guard. It does not choose visible roots, control visual or z-order composition, select an effective navigation root, infer overlay activity, resolve focus, or participate in rendering.

ScreenHost is the sole terminal widget consumer of resolved projection data because:
- It is a permanent runtime resident (SPEC-HAL-247).
- It is navigation-agnostic — it imports zero navigation types.
- It is the root widget of the widget tree and the sole terminal storage point for the resolved frame projection.
- It already exists as the mount point for screen and modal subtrees.

No widget observes `NavigationProjection` or `NavigationFrameProjection`; they exist only inside the bounded runtime-glue/compositor pull operation.

---

## 2. Runtime Role of ScreenHost

ScreenHost is:
- A permanent runtime resident that lives for the duration of the application window.
- The root widget of the widget tree.
- A projection sink: it receives `ProjectedSurfaceView` and stores it for root-host coordination and runtime inspection.
- Navigation-agnostic: it imports only `crate::compositor::ProjectedSurfaceView`. Zero navigation imports.

ScreenHost does not know:
- That NavigationHost exists.
- That NavigationSurfaceOrchestrator exists.
- That NavigationRootRegistry exists.
- What navigation path is active.
- Which surfaces are "active" in navigation terms (it stores per-context active-surface testimony opaquely as received from the compositor).
- Whether overlays are open.
- What focus routing decided (it stores focused-surface testimony opaquely).
- Whether the current frame's projection differs from the previous frame's.

ScreenHost's projection-related API consists of exactly three methods:
- `set_projected_view(view: ProjectedSurfaceView)` — called by runtime glue once per successful frame pull.
- `clear_projected_view()` — called by runtime glue when the current binding epoch changes or its pull fails; removes every previously resolved surface identity.
- `projected_view() -> Option<&ProjectedSurfaceView>` — available to ScreenHost-owned coordination and runtime verification; renderer code must not call it.

### 2.1 Explicit Frame Projection Binding

Application runtime glue owns one `FrameProjectionBindingRegistry` outside the
widget tree. It is the sole storage and lifecycle owner for every ScreenHost
projection binding:

```text
FrameProjectionBindingRegistry {
    bindings: HashMap<ScreenHostProjectionKey, VersionedFrameProjectionBinding>,
    root_consumers: HashMap<NavigationRootId, Set<ScreenHostProjectionKey>>,
    domain_consumers: HashMap<InteractionDomainId, Set<ScreenHostProjectionKey>>,
    next_epoch: FrameProjectionEpoch,
}

FrameProjectionAuthorityAccess<'a> {
    interaction_domains: &'a InteractionDomains,
    navigation_root_registry: Option<&'a NavigationRootRegistry>,
    navigation_domain_router: Option<&'a NavigationDomainRouter>,
    overlay_domain_coordinator: &'a OverlayDomainCoordinator,
    focus_resolver: &'a FocusResolver,
}

FrameProjectionWindowAccess<'a> {
    window_id: WindowId,
    interaction_domain_id: InteractionDomainId,
    compositor_identity: CompositorIdentity,
    widget_tree: &'a mut WidgetTree,
}

FrameProjectionRuntimeAccess<'a> {
    bindings: &'a mut FrameProjectionBindingRegistry,
    authorities: FrameProjectionAuthorityAccess<'a>,
    compositor: &'a Compositor,
    window: FrameProjectionWindowAccess<'a>,
}

ScreenHostProjectionKey {
    window_id: WindowId,
    node_id: NodeId,
}

VersionedFrameProjectionBinding {
    epoch: FrameProjectionEpoch,
    binding: FrameProjectionBinding,
    participants: unique Vec<NavigationSurfaceOrchestratorHandle>,
}

FrameProjectionBinding {
    interaction_domain_id: InteractionDomainId,
    navigation_sources: ordered Vec<NavigationProjectionSource>,
}

NavigationProjectionSource =
    StandaloneOrchestrator {
        bound_domain_id: InteractionDomainId,
        orchestrator: NavigationSurfaceOrchestratorHandle,
    }
    | RegisteredRoot(NavigationRootId)
```

`FrameProjectionAuthorityAccess` is a call-scoped validation bundle. The registry
never stores it or any referenced authority. A registered-root source requires
both optional multiroot authorities; standalone-only bindings may omit them.
`FrameProjectionWindowAccess` is the explicit call-scoped split access for one
window. Its constructor captures the supplied compositor's immutable identity
testimony and retains mutable access to that window's widget tree; the registry
therefore can validate and clear a prepared binding while application lifecycle
code separately holds the same compositor for prepared surface execution.
`FrameProjectionRuntimeAccess` additionally borrows the live compositor for the
bounded read-only frame resolution call, binds those accesses to the one registry
instance for a pull, and is never retained. Neither runtime glue nor the registry
may discover any of these authorities globally.
`CompositorIdentity` is an opaque, stable identity exposed read-only by a
compositor and by every orchestrator/registry source bound to it. It exists only
to reject cross-compositor projection; it carries no surface or navigation
authority.

The registry exposes only explicit lifecycle and guarded snapshot operations:

```text
guard_identity(screen_host_key) -> Option<(InteractionDomainId, FrameProjectionEpoch)>
install_or_replace(authorities, window_access, screen_host_key, binding, domain_guard_token) -> Result<FrameProjectionEpoch>
remove(window_access, screen_host_key, domain_guard_token) -> Result<()>
prepare_remove_or_replace_guarded(
    authorities,
    window_access,
    screen_host_key,
    expected_epoch,
    optional_replacement_binding,
    domain_guard_token,
) -> Result<PreparedFrameProjectionBindingMutation>
commit_remove_or_replace_guarded(
    window_access,
    domain_guard_token,
    PreparedFrameProjectionBindingMutation,
) -> Option<FrameProjectionEpoch> // non-fallible; Some for replacement, None for removal
invalidate_missing_screen_host(screen_host_key, domain_guard_token) -> Result<()>
root_consumers(domain_guard_token, root_id) -> Result<Set<ScreenHostProjectionKey>>
domain_consumers(domain_guard_token, domain_id) -> Result<Set<ScreenHostProjectionKey>>
binding_guarded(domain_guard_token, screen_host_key) -> Result<Option<VersionedFrameProjectionBinding>>
```

`guard_identity` is the sole unguarded bootstrap read. It atomically returns only
the immutable binding domain plus current epoch, never the binding contents or a
live reference. A caller uses that domain to acquire the coordinator guard and
must then call `binding_guarded`; a missing record or domain/epoch mismatch means
the bootstrap became stale and the operation releases the token without reading,
clearing, or publishing anything. It may retry on a later frame.

`window_access.widget_tree` is a call-scoped mutable access capability used only to validate
the named ScreenHost and call its projection API; the registry never stores the
tree or widget reference. `window_id` selects that window's widget tree and
prevents tree-local `NodeId` values from colliding across windows. Installation
and removal require `window_access.window_id == screen_host_key.window_id` and
the matching `window_access.interaction_domain_id`. Installation additionally uses the call-scoped authority bundle to validate that
`InteractionDomains::domain_for_window(window_id)` equals the
binding's `interaction_domain_id` and `window_access.interaction_domain_id`.
Every standalone orchestrator and registered root exposes its owning
`CompositorIdentity` as validation testimony; every source in the binding must
match `window_access.compositor_identity`. A binding's domain is immutable for the life
of its ScreenHost key; moving composition to another window/domain removes the
old record and installs a record for the destination ScreenHost. Installation
and removal require that one domain's already-acquired guard token. Registered
roots are validated through the supplied root registry and domain router,
including the presence of their registered
`NavigationRootFocusBindingPort`; the registry does not discover or retain any
of those authorities. Epoch allocation is checked and never wraps or reuses
a value; exhaustion rejects replacement without mutation. `remove` clears the ScreenHost view and updates
both reverse indexes atomically. A ScreenHost node must be removed from the
registry before that node is destroyed or its `(WindowId, NodeId)` can be reused. Registry
guarded queries return complete owned snapshots under the domain transition guard, never live
iterators that can be invalidated during teardown.

`prepare_remove_or_replace_guarded` is the compound-lifecycle seam for an
existing binding. It requires the exact current epoch, validates the ScreenHost,
reverse-index membership, and every authority/source in an optional replacement,
reserves source-held participation-count changes, and reserves a checked next
epoch when replacing. It does not clear the view or mutate the binding/indexes.
The prepared value stores no widget reference; the held domain guard makes the
validated node/key/epoch stable until commit. Multiple prepared binding
mutations may reserve changes against the same source; the source computes their
effective checked count and every prepared drop removes only its own reservation.
`commit_remove_or_replace_guarded` consumes that value with the same window
access and token, clears the ScreenHost view, and either removes the binding or
installs the prevalidated replacement plus both reverse-index updates
and commits the reserved source participation changes non-fallibly. Ordinary
`remove` and existing-binding replacement are
prepare-plus-commit conveniences. Root/domain teardown prepares one mutation for
every key in the complete guarded `root_consumers`/`domain_consumers` snapshot
before any focus, routing, or residency authority is irreversibly changed.

`invalidate_missing_screen_host` is a defensive lifecycle repair permitted only
after the caller, under the matching domain token, proves that the registered
node is absent or does not downcast to `ScreenHost`. It removes the orphaned
binding, both reverse indexes, and every source participation claim atomically
and performs no widget call. It is not a substitute for the required
remove-before-destroy lifecycle; its explicit error result records that the
invariant was breached.

The registry key names exactly one live ScreenHost in one window widget tree. A
binding names every navigation context whose authority testimony must be
represented in that ScreenHost's `ProjectedSurfaceView` and declares testimony
order within that view. This order has no rendering or z-order effect;
`SurfaceStack` remains the sole visual-order authority. A multiroot source is
resolved only through the explicitly bound `NavigationRootRegistry`; a
standalone source names its orchestrator directly. The binding does not derive
sources or order from focus, navigation priming, delegation, visibility,
widget ancestry, z-order, hover, or compositor state.

A standalone source's `bound_domain_id` must equal the frame binding's domain.
A registered root must be associated with that domain by
`NavigationDomainRouter`. These memberships are validated before every pull;
they are not inferred from the orchestrator or compositor.

Installing a frame binding enrolls every named source in that domain's
transition protocol. Any operation that can change a bound
`NavigationSurfaceOrchestrator::project()` result first resolves a versioned
SPEC-HAL-228 candidate and executes the complete SPEC-HAL-231 guard/replacement
chain outside the domain guard. Only after one-shot candidate authorization may
it acquire the same domain guard exactly once. Under that token it revalidates
the host/history generation and, for a registered root, routing/binding
generations; prepares all fallible host/orchestrator/surface/overlay/binding/focus
work; then commits only non-fallible prepared operations before releasing the
token. A stale generation or failed preparation publishes nothing and causes the
complete candidate/guard sequence to be retried outside a later token. A
standalone consumer performs that wrapper without router-generation steps. A
multiroot consumer uses the SPEC-HAL-237 routed form. NavigationHost and the
orchestrator never own or discover the coordinator; their host-commit and
surface-publication preparations each accept only the token's opaque identity
testimony for a frame-bound transaction. An unguarded caller may not commit a
host or surface mutation to a frame-bound source, including through the public
NavigationHost compatibility APIs exposed by the orchestrator. This
participation law is what makes the pull's guarded navigation read effective
rather than merely advisory without ever running application code under the
token.

Enrollment is source-held and reference-counted once per unique orchestrator in
each binding. A source may participate in multiple ScreenHost bindings in the
same coordinator/domain; it remains guarded until the final binding releases its
claim. A source with a live or prepared claim rejects a different
coordinator/domain. Binding installation, replacement, removal, orphan
invalidation, and root/domain retirement prepare every count delta before
changing registry state and commit those deltas in the same non-fallible guarded
publication sequence.

`NavigationApplicationCoordinator::standalone_snapshot`,
`authorize_standalone_navigation`, and `prepare_standalone_navigation` are the
standalone executable wrapper. Snapshot and application guard evaluation occur
before the token. Preparation then acquires the declared domain once, rejects an
active coordinator overlay that lacks an explicit exit plan, revalidates
host/orchestrator generations, and prepares host plus surface work. Its prepared
transaction commits history and the matching guard-proven surface publication
non-fallibly. Registered roots use the routed transaction variants, all of which
prepare both the host commit and surface transition with the same domain token.
Direct NavigationHost execution, orchestrator dispatch, surface preparation, and
teardown remain lawful only while the source has no live or prepared frame
participation claim.

Changing the set/order of projected contexts is an explicit registry replacement
between frame pulls. Replacement validates the
ScreenHost key plus every source and domain authority, reserves a fresh epoch,
and updates the binding and both reverse indexes atomically while holding the
bound domain guard. Failed validation
leaves the old registry record and its complete resolved view current. A
successful replacement synchronously calls `clear_projected_view()` for that
ScreenHost before returning; the old epoch's view is never current under the new
binding. Root/domain teardown enumerates `root_consumers`/`domain_consumers` and
must remove or replace every referencing binding, clear each affected
ScreenHost, and update the reverse indexes before a source becomes non-resident.
A committed binding may never intentionally name a terminated source.

### 2.2 Frame Projection Shape

```text
NavigationFrameProjection {
    compositor_identity: CompositorIdentity,
    navigation_contexts: ordered Vec<NavigationProjectionContext>,
    overlay_surfaces: ordered Vec<SurfaceId>,
    focused_surface: Option<SurfaceId>,
}

NavigationProjectionContext {
    navigation_surfaces: ordered Vec<SurfaceId>,
    active_surface: Option<SurfaceId>,
}

ProjectedNavigationContext {
    navigation_surfaces: ordered Vec<SurfaceDescriptor>,
    active_surface: Option<SurfaceId>,
}

ProjectedSurfaceView {
    navigation_contexts: ordered Vec<ProjectedNavigationContext>,
    overlay_surfaces: ordered Vec<SurfaceId>,
    focused_surface: Option<SurfaceId>,
}
```

Each `NavigationProjection` is the navigation-only authority snapshot from
SPEC-HAL-246 and is mechanically moved into the corresponding compositor-native
`NavigationProjectionContext`. `OverlayProjection` is returned by
`OverlayDomainCoordinator::active_projection_guarded(domain_transition_guard_token)` through
the admitted owner's registered lifecycle port. `FocusProjection` is read from
`FocusResolver::focus_projection_guarded(...)` using the token-sealed focus capability,
that interaction domain's authoritative `FocusManager`, and the exact domain ID.
Runtime glue owns no field authority; it only preserves the navigation, overlay,
and focus snapshot fields in the declared compositor-native frame shape.

`Compositor::apply_frame_projection()` maps only navigation surface IDs to
descriptors, validates every referenced active/overlay/focus ID, and otherwise
preserves that testimony verbatim. The resolved shape retains context grouping
so simultaneous roots cannot be collapsed into a single active navigation
surface.

---

## 3. Projection Pull Sequence (Normative)

The projection pull occurs in the frame loop (`App::run_ui_loop`), at step 7.5, between navigation queue processing (step 7) and view rebuild (step 8). The caller supplies one call-scoped `FrameProjectionRuntimeAccess`, including the exact registry, authority bundle, window compositor, and mutable widget tree; runtime glue discovers none of them globally. The sequence is:

**Step 0.** Runtime glue calls `runtime_access.bindings.guard_identity(screen_host_key)`. No record means there is no bound pull. Otherwise it obtains only `(interaction_domain_id, epoch_hint)`, calls the supplied `OverlayDomainCoordinator::begin_domain_transition(interaction_domain_id)` exactly once, and then obtains an owned `binding_guarded(token, screen_host_key)` snapshot from the supplied registry. Token validation failure returns an error without mutation. A missing snapshot or domain/epoch mismatch proves that the bootstrap read became stale; runtime glue releases the token and returns `RetryNextFrame` without reading sources, clearing a view, or publishing anything. The acquired token is retained through every remaining step and released on every success or failure exit.

**Step 1.** Under that token, runtime glue validates that the supplied window access matches `screen_host_key.window_id`, that its domain equals both `InteractionDomains::domain_for_window(window_id)` and the binding domain, and that its widget tree contains the named ScreenHost. It then resolves every declared navigation source through the supplied `InteractionDomains`, `NavigationRootRegistry`, and `NavigationDomainRouter` capabilities without mutation and validates each source's owning `CompositorIdentity` against the supplied window compositor. An unknown root, torn-down source, wrong-domain source, wrong-compositor source, or missing authority record fails the whole pull; no source may be skipped and no partial frame may be published. If the registered node is absent or is not a ScreenHost, runtime glue calls `invalidate_missing_screen_host(screen_host_key, token)`, releases the token, and returns the lifecycle error. Because no ScreenHost exists, this branch performs no impossible `clear_projected_view()` call.

**Step 2.** In declared order, runtime glue calls `NavigationSurfaceOrchestrator::project() -> Result<NavigationProjection, NavigationProjectionError>` once for each resolved source. Every call is read-only under INV-246-06. Any error fails the whole pull before frame assembly.

For each `RegisteredRoot`, Step 2 also reads that root's guarded
`NavigationRootFocusBindingSnapshot` through the already resolved router binding
and requires
`projection.active_surface == Some(snapshot.navigation_surface_id)`. A
`None`, stale, or missing association fails the whole pull. Runtime glue
does not read or select the focus target; it validates only the explicit
cross-authority association required by SPEC-HAL-237.

**Step 3.** Runtime glue reads `OverlayDomainCoordinator::active_projection_guarded(token)` and passes the token-sealed focus capability, the exact interaction-domain `FocusManager`, and `interaction_domain_id` to `FocusResolver::focus_projection_guarded(...)`. Those reads return `OverlayProjection` and `FocusProjection`; a guarded-read error fails the whole pull, and runtime glue neither verifies testimony by inference nor substitutes local state.

**Step 4.** Runtime glue assembles one `NavigationFrameProjection` by preserving the binding order and the three authority-owned snapshot classes exactly. Assembly is mechanical and has no policy branch.

**Step 5.** Runtime glue calls `runtime_access.compositor.apply_frame_projection(&NavigationFrameProjection) -> Result<ProjectedSurfaceView, FrameProjectionError>`. This read-only operation first requires the live compositor identity to equal `runtime_access.window.compositor_identity`, resolves every navigation `SurfaceId` to its `SurfaceDescriptor`, validates that active/overlay/focus IDs still exist, and preserves navigation-context grouping, within-context order, context order, overlay order, and focus testimony. No compositor state is mutated. Any identity mismatch or unresolved ID fails the whole operation; no missing ID is silently skipped.

**Step 6.** Runtime glue revalidates through the supplied registry's `binding_guarded` that it still contains the same `ScreenHostProjectionKey`, domain, and epoch. A mismatch discards the resolved value, releases the token, and returns `RetryNextFrame`. Otherwise it accesses `runtime_access.window.widget_tree.node_mut(screen_host_key.node_id)`, revalidates the ScreenHost, and writes that complete `ProjectedSurfaceView` via `set_projected_view()`, atomically replacing the previously stored view while the guard is still held.

**Step 7.** Any failure in steps 1–6 for the still-current epoch and a still-live validated ScreenHost calls `clear_projected_view()` before releasing the token and returning the error. The missing/mismatched-ScreenHost branch is handled exclusively by Step 1 orphan invalidation. No other widget or storage point receives `NavigationFrameProjection` or `ProjectedSurfaceView`. ScreenHost is the terminal sink. Renderer input remains the separate `Compositor::snapshot() -> SurfaceStackSnapshot` path.

**Step 8.** After successful ScreenHost replacement, runtime glue releases the one domain token and returns `FrameProjectionPullOutcome::Published { epoch }`. No navigation, overlay, focus, projection-binding, or relevant widget-tree mutation may enter between Steps 0 and 8.

The live runtime glue function is
`pull_navigation_frame_projection(runtime_access, screen_host_key)`. It accepts
the already selected window/compositor/widget-tree split access and rejects a
window-key mismatch; it does not select or discover a widget tree or compositor.

---

## 4. Authority Ownership Table

| Concern | Sole Authority Owner | ScreenHost Role |
|---|---|---|
| Navigation path resolution | NavigationHost | None |
| Surface lifecycle (create/destroy) | NavigationSurfaceOrchestrator | None |
| Per-ScreenHost projection-testimony set/order and binding epoch | Application `FrameProjectionBindingRegistry` | None |
| Visual surface presence/order | Compositor `SurfaceStack` | None |
| Focus-domain/target routing | Focus Resolver over `InteractionDomainState::focus_manager` (SPEC-HAL-235) | None |
| Overlay active-stack admission and projection | OverlayDomainCoordinator plus admitted owner's lifecycle port (SPEC-HAL-236) | None |
| Surface identity resolution | Compositor (SurfaceStack) | None |
| Per-context navigation projection shape | NavigationProjection (SPEC-HAL-246) | None |
| Frame projection assembly | Runtime glue (mechanical, zero semantic authority) | None |
| Projection consumption | ScreenHost | Sole consumer |
| Rendering | Renderer reading `SurfaceStackSnapshot` only | None |

No authority is shared. No authority is delegated to ScreenHost beyond storage and retrieval.

---

## 5. Binding Invariants

**INV-248-01: Pull-Only Consumption.**
ScreenHost receives projection data exclusively through `set_projected_view()` called by runtime glue. ScreenHost never initiates a pull. ScreenHost never queries the orchestrator, compositor, or any navigation system.

**INV-248-02: Zero Authority.**
The `ProjectedSurfaceView` stored in ScreenHost has zero authority. Reading it causes no state change in any system. Writing it causes no state change outside ScreenHost's own `projected_view` field.

**INV-248-03: No Navigation Inference.**
ScreenHost does not infer navigation state from projection data. It does not compare per-context active surfaces across frames, derive focus from surface ordering, or interpret overlay surfaces as modal state. It stores and returns exactly what it receives.

**INV-248-04: Sole Consumer.**
ScreenHost is the only widget that stores `ProjectedSurfaceView`. No other widget, layout node, event handler, or rendering subsystem holds or receives `ProjectedSurfaceView`. `FrameProjectionBindingRegistry` stores only bindings, epochs, and reverse-reference indexes, never resolved views. Runtime glue composes every context named by one binding and writes the single result to exactly one ScreenHost instance per successful pull.

**INV-248-05: Per-Frame Replacement.**
Each successful projection pull replaces the previous `ProjectedSurfaceView` entirely. No accumulation occurs. No diff is computed. No history is retained. A binding-epoch change or current-epoch failed pull publishes no partial result and clears the prior view; authority testimony from an older epoch is never exposed as current. A stale Step 0 bootstrap is not a current-epoch pull and performs no second clear: the remove/replacement that changed the epoch already cleared the prior view under its guard.

**INV-248-06: Frame Reconstructibility.**
The projection pull sequence can be executed at any time, any number of times per frame, with identical results given an identical `FrameProjectionBinding` and identical navigation, overlay, focus, and compositor authority state. No frame counter or prior projection is required.

**INV-248-07: Zero Navigation Imports.**
ScreenHost's source file imports zero types from `crate::model::navigation_host`, `crate::core::navigation_orchestrator`, or any navigation module. Its only projection-related import is `crate::compositor::ProjectedSurfaceView`.

**INV-248-08: Headless Operation.**
The projection pull pipeline functions without a renderer, window, or event loop. Navigation, overlay, and focus reads, `Compositor::apply_frame_projection()`, and `ScreenHost::set_projected_view()` are headless-capable.

**INV-248-09: Explicit Multiroot Composition.**
Zero, one, or many navigation contexts may be present in one frame binding. Every declared source is projected exactly once in declared testimony order. Runtime glue must not reduce the set to the primed, delegated, focused, topmost, or most recently changed root. This ordering does not reorder, filter, show, or hide compositor surfaces.

**INV-248-10: Atomic Cross-Authority Read.**
Runtime glue bootstraps only a domain/epoch identity, acquires that domain's
transition guard exactly once, revalidates an owned binding snapshot, and retains
the token through ScreenHost set/clear or orphan invalidation. Navigation,
overlay, focus, binding, and relevant widget-tree mutation for the bound
interaction domain must not interleave with that guarded interval. Readers
observe one complete old state or one complete new state. Any failure before
ScreenHost replacement publishes no frame.

**INV-248-11: No Authority Collapse.**
`NavigationFrameProjection` groups testimony from independent authorities without transferring ownership to runtime glue, compositor, or ScreenHost. Each field class must be sourced only from its owner named in Section 4.

---

## 6. Forbidden Patterns

**FP-248-01: Push-Based Projection.**
Projection must not be delivered to ScreenHost through callbacks, events, observers, subscriptions, channels, or any mechanism other than direct method call from runtime glue. ScreenHost must not register for projection updates.

**FP-248-02: Event or Observer Models.**
No system may emit events when projection changes. No system may subscribe to projection changes. The pull model is the only model.

**FP-248-03: Projection Caching.**
Runtime glue must not cache context projections, `NavigationFrameProjection`, or `ProjectedSurfaceView` across frames. Each frame pulls fresh. ScreenHost must not compare the incoming view with the previous view. No dirty-checking is permitted.

**FP-248-04: Widget-Level Navigation Reads.**
No widget other than ScreenHost may store resolved frame projection data. Widgets must not hold references to `NavigationProjection`, `NavigationFrameProjection`, `ProjectedSurfaceView`, orchestrators, authority projection ports, or compositor projection methods. Renderer code does not read ScreenHost.

**FP-248-05: ScreenHost Inferring Focus or Active Surface.**
ScreenHost must not determine focus from navigation-surface ordering, determine a context's active surface from overlay presence, or collapse multiple context-active surfaces into one selected root. All such testimony remains opaque.

**FP-248-06: Authority Projection Exposure Outside the Bounded Pull.**
`NavigationProjection` and `NavigationFrameProjection` must not be passed to widgets, stored in the widget tree, or exposed through any API accessible to renderer or layout code. Runtime glue handles `NavigationProjection`; `NavigationFrameProjection` crosses only the call-scoped mechanical compositor-resolution boundary. ScreenHost sees only `ProjectedSurfaceView`.

**FP-248-07: Retained Surface Identity.**
ScreenHost must not store `SurfaceId` values outside its one current complete `ProjectedSurfaceView`. Successful pulls replace that view wholesale; a binding-epoch change or failed pull clears it. ScreenHost must not create a second snapshot, history, identity tracker, comparison, or animation state.

**FP-248-08: Multiple Projection Consumers.**
No second consumer of `ProjectedSurfaceView` may be introduced. The pipeline terminates at ScreenHost. Adding a second storage point for projection data is a violation of this SPEC.

**FP-248-09: Inferred Context Selection.**
Runtime glue must not choose projected contexts from focus, priming, delegation, visibility, z-order, hover, ancestry, or recent mutation. The current registry binding is the sole projection-testimony context-set/order source; it is not visual-order authority.

**FP-248-10: Authority Substitution.**
Runtime glue must not read focus or overlay fields retained by `NavigationSurfaceOrchestrator`, derive either from compositor state, or replace missing authority testimony with defaults. A missing bound authority fails the pull.

**FP-248-11: Partial Publication.**
Runtime glue must not skip an unresolved bound root or surface, publish only the sources that succeeded, or combine a current authority snapshot with a cached snapshot from another frame.

---

## 7. Headless and Determinism Guarantees

The projection pull pipeline is deterministic:
- Given an identical `FrameProjectionBinding`, identical bound orchestrator states yield the same ordered context projections.
- Identical coordinator/owner-port and Focus Resolver states yield identical overlay and focus projections.
- Given an identical `NavigationFrameProjection` and compositor state, `apply_frame_projection()` returns an identical `ProjectedSurfaceView`.
- Given identical `ProjectedSurfaceView` input, `set_projected_view()` stores identical state.

The pipeline has no timing dependencies:
- It does not consult frame counters, wall clocks, or animation timers.
- It does not require a previous frame's projection to compute the current one.
- It does not require a minimum or maximum call frequency.

The pipeline is headless:
- No renderer is required. No window is required. No event loop is required.
- All authority reads, frame assembly, compositor resolution, and ScreenHost replacement operate on in-memory data structures.
- Headless proof covers standalone, zero-root, multiroot, active-overlay,
  focused-target, unresolved-source, binding-replacement, stale-bootstrap,
  missing/wrong-type ScreenHost, immediate busy-guard exclusion, registered-root
  focus-association mismatch, and compositor-failure cases without windowing.
  The ordinary-navigation proof separately races candidate/guard evaluation with
  host-history and router-generation changes, proves stale authorization is
  dropped and the guard is never invoked while a domain token is held, and
  injects preparation failure to prove no authority or visible surface changes
  before the non-fallible commit sequence. Root/domain teardown proof prepares
  the complete guarded consumer
  snapshot as removal/replacement mutations before any irreversible authority
  change. Failure at any consumer preparation leaves every ScreenHost view,
  binding, reverse index, focus, routing, and residency record unchanged; after
  focus retirement begins, every prepared projection commit is non-fallible and
  completes before SPEC-HAL-234 whole-root surface destruction.

---

## 8. Relationship to SPEC-HAL-246

SPEC-HAL-246 defines:
- The context-local, navigation-only `NavigationProjection` struct and its field sourcing.
- The `project()` access API on `NavigationSurfaceOrchestrator`.
- Invariants governing projection data shape, authority, and idempotency.
- The prohibition on folding focus or overlay authority back into an orchestrator projection.

SPEC-HAL-248 defines:
- The application-owned `FrameProjectionBindingRegistry`, its reverse-reference lifecycle, and the explicit binding that names all context testimony stored for one ScreenHost.
- The runtime pull sequence that composes context, overlay, and focus projections into one frame.
- ScreenHost's role as the sole runtime consumer of resolved projection data (`ProjectedSurfaceView`).
- The frame-loop integration point (step 7.5).
- Prohibitions on alternative consumption patterns.

The boundary is:
- SPEC-HAL-246 governs what one navigation-context projection contains and how it is produced.
- SPEC-HAL-235/236 govern focus and overlay projection testimony.
- SPEC-HAL-248 governs explicit frame source binding, mechanical composition, flow, and termination.
- `Compositor::apply_frame_projection()` mechanically reads `SurfaceStack` to
  resolve navigation-surface `SurfaceId` values to `SurfaceDescriptor` and
  existence-validate retained active/overlay/focus IDs without selecting or
  flattening authority. `SurfaceStack` remains the mechanical store.
- ScreenHost stores the resolved `ProjectedSurfaceView` for root-host coordination; renderer input remains separate.
- Neither `NavigationProjection` nor `NavigationFrameProjection` reaches ScreenHost. `ProjectedSurfaceView` never reaches an authority source.

---

## 9. Stability Declaration

This SPEC defines a closed authority boundary. Explicit source binding, pull-based mechanical composition, ScreenHost's sole-consumer role, and the renderer separation are binding.

Extension requires an explicit amendment or successor SPEC. No change may:
- Add a second consumer of `ProjectedSurfaceView`.
- Introduce push-based, event-based, or observer-based projection delivery.
- Allow any widget other than ScreenHost to store or read projection data.
- Expose authority projections outside runtime glue.
- Cache projection data across frames.
- Reassign focus or overlay authority to `NavigationSurfaceOrchestrator`.
- Infer the projected context set/order instead of using the current registry binding.

Guard tests in `src/tests/navigation_projection_boundary_guard.rs` enforce the renderer/module boundary and sole runtime call sites. The headless frame-projection suite covers explicit multiroot source composition, independent guarded focus/overlay sourcing, stale-bootstrap retry without a second clear, orphan-binding invalidation, guarded busy exclusion, registered-root focus-association mismatch, compositor failure, and no partial publication. Root-retirement tests cover all-or-nothing prepared consumer removal or replacement.

---

## 10. Sealed Boundary: Projection vs Rendering

The projection pipeline terminates at ScreenHost. The renderer operates on a separate data path.

Runtime glue pulls every explicitly bound context projection plus overlay and focus projections from their separate authorities, assembles `NavigationFrameProjection`, passes it to `Compositor::apply_frame_projection()` for mapping/validation-only resolution, and stores the resulting `ProjectedSurfaceView` in ScreenHost. This is the projection data path.

The renderer consumes `SurfaceStackSnapshot` produced by `Compositor::snapshot()`. This is the rendering data path. The renderer iterates surfaces in compositor-provided order and delegates to GPU pipelines. The renderer does not import, reference, or receive `NavigationProjection`, `ProjectedSurfaceView`, or any type from the navigation or orchestrator modules. The renderer has no knowledge that projection exists.

These two data paths share no projection types, state, or communication channel. `ProjectedSurfaceView` never enters the renderer. `SurfaceStackSnapshot` is not derived from projection. Resolution (SPEC-HAL-246, SPEC-HAL-247 §6) is mapping/validation-only and does not introduce navigation, overlay, or focus semantics into compositor-native renderer input.

This boundary is sealed. It is enforced by guard tests in `src/tests/navigation_projection_boundary_guard.rs` and by the governance constitution (SPEC-HAL-232 §7, SPEC-HAL-247 §7).

---

## 11. Projection Flow (Normative)

```text
FrameProjectionBindingRegistry[WindowId + ScreenHost NodeId] --> (epoch, FrameProjectionBinding)
     |
     +--> NavigationSurfaceOrchestrator[0..N] --> NavigationProjection[0..N]
     |
     +--> OverlayDomainCoordinator + admitted owner port --> OverlayProjection
     |
     +--> FocusResolver / FocusManager --------------------> FocusProjection
                                      |
                                      v
                         NavigationFrameProjection
                                      |
                                      v
                    Compositor::apply_frame_projection
                                      |
                                      v
                         ProjectedSurfaceView
                                      |
                                      v
                    ScreenHost (sole terminal sink)

Compositor::snapshot() --> SurfaceStackSnapshot --> Renderer
```

The projection and rendering flows remain separate. No feedback loop or inferred source branch is lawful. Authority does not move into runtime glue, compositor, ScreenHost, or renderer.

---

## 12. Runtime Implementation Note (0.176.0-beta)

The application-owned `FrameProjectionBindingRegistry` is now live with checked
epochs, exact root/domain reverse indexes, coordinator-bound guarded access,
registered-root and standalone source validation, synchronous ScreenHost clearing,
orphan invalidation, and per-consumer prepared removal/replacement. Multiple root
consumers can be prepared together without changing any binding, index, or stored
view, providing the projection-lifecycle seam required by root retirement.
`NavigationApplicationCoordinator::prepare_root_retirement` now consumes that
complete guarded consumer set, prepares every removal/replacement, and commits
each epoch advance and ScreenHost clear non-fallibly before whole-root surface
destruction. `FrameProjectionWindowAccess` retains only captured compositor
identity plus mutable widget-tree access, so the same transaction can hold the
live compositor exclusively for its prepared teardown without weakening
cross-compositor validation.

Every resolved source is now retained as a unique source participant in the
versioned binding. Source mechanics own checked per-binding participation counts
plus independently abortable prepared deltas. Installation, replacement,
removal, orphan invalidation, and multiconsumer retirement update those claims
with their binding/index commit. Bound direct NavigationHost execution and
orchestrator dispatch fail closed; every registered routed variant publishes
host and surfaces with matching coordinator/domain proof; the standalone
snapshot/authorize/prepare transaction provides the same executable
guard-outside-token sequence without router generations. Whole-root retirement
prepares under that guard and releases every projection claim before destruction.

Complete interaction-domain retirement enumerates the exact
`domain_consumers` reverse index under the same held domain token. Because a
binding's interaction-domain identity is immutable, it requires a supplied
`FrameProjectionWindowAccess` for every distinct consumer window and prepares
removal, never replacement, of every binding in that domain. Commit advances
each binding epoch, clears each ScreenHost view, removes every reverse reference,
and releases every source participation claim before any root surface or
registry record is destroyed. A missing window, stale epoch, reverse-index
mismatch, or participant reservation failure aborts the complete domain
transaction without changing any binding or stored view.

The live frame loop now invokes `pull_navigation_frame_projection` with the exact
registry, authority bundle, compositor, and widget-tree access. The operation
acquires one non-blocking coordinator guard, rejects stale bootstrap epochs
without clearing, validates every standalone or registered-root source, pulls
navigation-only context projections in binding order, reads independent overlay
and focus testimony, and resolves one compositor-identity-bound
`NavigationFrameProjection`. A current-epoch failure clears the prior complete
view, an orphaned ScreenHost invalidates its binding under the guard, and success
revalidates the epoch before replacing the ScreenHost view. Headless runtime
proof covers zero/one/multiple contexts, independent overlay/focus authority,
busy and stale retries, orphan invalidation, registered-root focus-association
rejection, and compositor-resolution failure without partial publication. The
renderer continues to consume only `SurfaceStackSnapshot`.

---

## 13. Non-Normative Summary

Every navigation context represented in ScreenHost projection testimony is named explicitly. Navigation, overlay, and focus authorities each provide their own immutable testimony. Runtime glue combines those snapshots mechanically, the compositor resolves identities, and ScreenHost stores one complete frame view for coordination/inspection only. SurfaceStack independently owns actual rendering presence and order. No event is emitted and no projection cache persists across frames.
