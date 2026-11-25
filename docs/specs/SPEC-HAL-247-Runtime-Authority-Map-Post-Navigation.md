# SPEC-HAL-247: Runtime Authority Map (Post-Navigation)

**Status:** Binding
**Series:** SPEC-HAL (Governance)
**Layer:** Runtime / Authority Classification
**Scope:** Audit and codification of runtime-resident subsystems after Navigation v2 completion

---

## 1. Runtime Residency Table

| System | Permanent Runtime Resident | Instantiated by User Code | Headless-Capable |
|---|---|---|---|
| NavigationHost | No | Yes | Yes |
| NavigationSurfaceOrchestrator | No | Yes | Yes |
| NavigationRootRegistry | No | Yes | Yes |
| NavigationDomainRouter | No | Yes | Yes |
| FrameProjectionBindingRegistry | Yes | Yes | Yes |
| OverlayDomainCoordinator | No | Yes | Yes |
| FocusResolver (over `InteractionDomainState::focus_manager`) | Yes | Yes | Yes |
| HitTestAuthority | Yes | No | Yes |
| SurfaceStack | Yes | No | Yes |
| ScreenHost | Yes | Yes | Yes |
| Compositor | Yes | Yes | Yes |

**Definitions:**
- *Permanent runtime resident* — lives for the duration of the application window. Destruction is abnormal.
- *Instantiated by user code* — application or test code calls the constructor directly.
- *Headless-capable* — functions without a renderer, window, or event loop attached.

---

## 2. Authority Statements

**NavigationHost** is the sole semantic authority for versioned navigation
candidate resolution and committed history/cursor mechanics. The application
guard owns scenario permission under SPEC-HAL-231; it evaluates candidates
outside the interaction-domain token and yields candidate-bound authorization,
not navigation mutation authority.

**NavigationSurfaceOrchestrator** is the sole semantic authority for translating navigation decisions into compositor surface lifecycle mutations.

**NavigationRootRegistry** is the sole semantic authority for navigation-root
identity, residency, isolation, and prepared public admission/removal within a
shared compositor. It delegates every surface lifecycle preparation and
execution to the root's `NavigationSurfaceOrchestrator` under SPEC-HAL-234/237.

**NavigationDomainRouter** is the optional, application-owned authority for interaction-domain root membership, registered root-to-overlay-owner and root-focus-binding-port routing metadata, globally unique root claims over focus-binding-port identities and complete focus-domain trees, priming, delegation stacks, scoped/unscoped target validation, and sealed overlay-activation eligibility for non-suspended roots in multiroot compositions. A root-focus binding port owns association testimony only; FocusResolver remains the sole focus authority.

**FrameProjectionBindingRegistry** is the application-owned authority for
per-ScreenHost projection-testimony bindings, binding epochs, root/domain reverse
references, and binding-scoped source participation enrollment. It does not
control compositor surface presence, navigation mutation, or visual order.

**OverlayDomainCoordinator** is the application-owned authority for the sole active overlay-stack binding and the cross-authority transition guard in each registered overlay-capable, multiroot-routed, or frame-projected interaction domain. Root-local overlay owners retain their stack contents and surface lifecycle.

**FocusResolver** is the engine-owned authority over the existing `FocusManager` in each live `InteractionDomainState`. It owns active focus-domain/target selection for that `InteractionDomainId`; no application or navigation subsystem owns a parallel focus store.

**HitTestAuthority** is the engine-owned per-interaction-domain authority that resolves uncaptured pointer/touch targets from declared geometry and the coordinator-admitted overlay stack. Pointer capture under SPEC-HAL-151 precedes hit testing.

**SurfaceStack** is the sole mechanical storage authority for z-ordered surfaces
and snapshot production. It owns no navigation lifecycle meaning.

**ScreenHost** is the sole semantic authority for root-level widget tree hosting and layout delegation.

**Compositor** is the mechanical surface store and API executor for creation,
destruction, property mutation, prepared surface batches, and identity
resolution. It owns no navigation lifecycle semantics. For navigation-managed
surfaces, `NavigationSurfaceOrchestrator` alone decides and executes the semantic
lifecycle plan; Compositor performs exactly that prevalidated mechanical plan.

---

## 3. Lifetime Rules

### NavigationHost
- **Created:** By application code, before orchestrator construction.
- **Destroyed:** When its owning orchestrator is destroyed, or when explicitly dropped by application code if standalone.
- **Destroyed by:** NavigationSurfaceOrchestrator (owns it) or application code.

### NavigationSurfaceOrchestrator
- **Created:** By application code or NavigationRootRegistry. Requires a Compositor reference and a NavigationHost.
- **Destroyed:** When its owning registry calls `destroy_root()`, or when explicitly dropped by application code if standalone.
- **Destroyed by:** NavigationRootRegistry or application code.

### NavigationRootRegistry
- **Created:** By application code when multiroot navigation is needed. Optional — single-root applications use NavigationSurfaceOrchestrator directly.
- **Destroyed:** When the application window closes or application code drops it.
- **Destroyed by:** Application code.

### NavigationDomainRouter
- **Created:** By application code alongside NavigationRootRegistry when interaction-domain multiroot routing is needed.
- **Destroyed:** After its domain records and associated roots are explicitly unwound during application/window teardown.
- **Destroyed by:** Application code.

### FrameProjectionBindingRegistry
- **Created:** By application runtime glue before the first ScreenHost projection binding is installed.
- **Destroyed:** After every ScreenHost binding and reverse reference is removed during application/window teardown.
- **Destroyed by:** Application code.

### OverlayDomainCoordinator
- **Created:** Once by application code before registering the first overlay-capable, multiroot-routed, or frame-projected interaction domain. It owns one guard-bearing record per such live domain and at most one active overlay binding in each record.
- **Destroyed:** After every admitted stack is confirmed inactive and every per-domain record is removed during application/window teardown.
- **Destroyed by:** Application code.

### FocusResolver
- **Created:** As a stateless engine capability by application/runtime or test code alongside `InteractionDomains`; all authoritative focus state remains in each domain's existing `FocusManager`.
- **Destroyed:** After its associated interaction-domain states have invalidated focus targets and domains.
- **Destroyed by:** Application/runtime ownership (drop); dropping it does not destroy or transfer `FocusManager` state.

### HitTestAuthority
- **Created:** With each `InteractionDomainState` before the domain accepts pointer/touch input.
- **Destroyed:** With that interaction-domain state, after pointer capture is released and overlay admission is cleared.
- **Destroyed by:** `InteractionDomains` lifecycle.

### SurfaceStack
- **Created:** Internally by Compositor::new(). Not independently constructible by external code (`pub(crate)`).
- **Destroyed:** When the owning Compositor is destroyed.
- **Destroyed by:** Compositor (owns it).

### ScreenHost
- **Created:** By application code as the root widget of the widget tree. Typically once per window.
- **Destroyed:** When the widget tree is torn down, after its `FrameProjectionBindingRegistry` record has been removed and its projected view cleared.
- **Destroyed by:** Widget tree ownership (drop).

### Compositor
- **Created:** By application framework (`App`) during window initialization, or by application/test code directly.
- **Destroyed:** When the application window closes or application code drops it.
- **Destroyed by:** App framework or application code.

---

## 4. Dependency Direction

```
NavigationHost         → (nothing)
NavigationSurfaceOrchestrator → NavigationHost, Compositor
NavigationRootRegistry → NavigationSurfaceOrchestrator, Compositor
NavigationDomainRouter → NavigationRootRegistry, InteractionDomains, OverlayDomainCoordinator, registered NavigationRootFocusBindingPort capabilities
FrameProjectionBindingRegistry → NavigationRootRegistry, NavigationDomainRouter, InteractionDomains, OverlayDomainCoordinator guard capabilities, FocusResolver, call-scoped window Compositor and WidgetTree/ScreenHost projection access
OverlayDomainCoordinator → InteractionDomains, registered OverlayOwnerLifecyclePort capabilities, FocusResolver guarded activation/restoration/retirement seams
FocusResolver          → InteractionDomains, FocusManager
HitTestAuthority       → InteractionDomains, OverlayDomainCoordinator
SurfaceStack           → (nothing)
ScreenHost             → (nothing)
Compositor             → SurfaceStack
```

DAG verified. No cycles.

**Reading:** "A → B" means A depends on B. B has no knowledge of A.

---

## 5. Forbidden Authority Leaks

### NavigationHost
- Must not create, destroy, or mutate compositor surfaces.
- Must not hold compositor references.
- Must not store focus state.
- Must not know SurfaceStack exists.

### NavigationSurfaceOrchestrator
- Must not reinterpret NavigationHost decisions.
- Must not bypass NavigationHost for path resolution.
- Must not select or persist authoritative focus; it may submit explicit focus-domain activation requests or constraints to the central resolver under SPEC-HAL-235.
- Must not expose compositor internals to consumers.
- Must not deliver signals or events.
- Must not own widget-layer state.
- May own root-local overlay stack contents and lifecycle, but must not publish that stack as active in an interaction domain without OverlayDomainCoordinator admission.

### NavigationRootRegistry
- Must not dispatch to a root other than the one specified.
- Must not share state between roots.
- Must reject duplicate root creation before z-band allocation or surface mutation; implicit root replacement is forbidden.
- Must keep every prepared root surface hidden and absent from public lookup/projection until coordinator registration, initial root-focus association, router association, registry insertion, routability, and final visibility can commit non-fallibly under one domain guard.
- Must not impose cross-root z-ordering policy beyond band allocation.
- Must not intercept or modify navigation decisions.

### NavigationDomainRouter
- Must not own NavigationHost state, guards, surfaces, overlays, focus, or registry storage.
- May retain one registered `NavigationRootFocusBindingPort` per root and validate
  its current surface/domain snapshot, but must not select a target, mutate
  FocusManager state, or infer an association.
- Must not infer priming or delegation from focus, visibility, hover, ancestry, z-order, or compositor state.
- Must not dispatch an unscoped command until it has resolved one explicit registered root in the supplied live interaction domain.
- May coordinate delegation preconditions through OverlayDomainCoordinator, but must not mutate a root-local overlay stack directly.
- Is the sole issuer of sealed overlay-activation eligibility for router-managed
  owners and must deny suspended, terminated, wrong-domain, or non-resident
  roots before coordinator activation.
- Must maintain one unique claim per registered root focus-binding-port identity
  and per root-owned focus domain, rejecting overlap, reused ports, partial
  subtrees, and ancestor/descendant crossings across roots.

### FrameProjectionBindingRegistry
- Must store only per-ScreenHost bindings, their unique resolved source
  participant handles, monotonically assigned epochs, root/domain
  reverse-reference indexes, and transient prepared binding mutations required
  for atomic lifecycle publication.
- Must not store `NavigationProjection`, `NavigationFrameProjection`, `ProjectedSurfaceView`, compositor descriptors, focus state, or overlay-stack contents.
- Must not infer a binding from focus, priming, delegation, visibility, z-order, hover, ancestry, or recent mutation.
- Must not show, hide, reorder, or otherwise mutate compositor surfaces; `SurfaceStack` remains the sole rendered presence/order authority.
- Must clear an affected ScreenHost view when a binding epoch changes or a
  current-epoch pull fails. Compound root/domain teardown must prepare the
  complete removal/replacement set under the domain guard before its first
  irreversible authority change, then clear views and remove/replace every
  reverse reference through non-fallible commits before the source becomes
  non-resident.

### OverlayDomainCoordinator
- Must not own overlay surfaces or stack contents.
- Must not own navigation state, delegation, focus, hit-test geometry, or root registry storage.
- May retain registered `OverlayOwnerLifecyclePort` command capabilities and owner-to-domain identity bindings; these are lifecycle seams, not root or overlay storage.
- May carry prepared focus activation plans and opaque restoration tokens only for the duration of one guarded whole-stack or nested-frame transaction, install/submit them in the order declared by SPEC-HAL-235/236, and then discard them; it must not inspect, persist, reorder, or reinterpret them.
- Must not infer an active stack from z-order, visibility, focus, hover, or compositor state.
- Must retain at most one explicitly admitted active-stack binding per live interaction domain.
- May consume a sealed router-issued activation eligibility capability for a
  `RouterEligibilityRequired` owner, but must not query or retain router state.
- Must prepare any owner/domain unregistration required by compound teardown
  before focus retirement, including exact binding to a prepared owner
  deactivation; commits after retirement are non-fallible and domain
  unregistration is last.
- Complete-domain teardown must snapshot every registered owner under the held
  token, reserve every inactive owner including domain-local owners, bind the
  active router-root owner to its exact complete-domain deactivation, and reject
  an active non-root owner unchanged until owner-correct deactivation.

### FocusResolver
- Must use the authoritative `FocusManager` belonging to the supplied live `InteractionDomainId`; it must not create global or application-owned duplicate target state.
- May validate a sealed `InteractionDomainTransitionCapability` supplied by higher-layer orchestration, but must not issue, retain, or use that capability to acquire overlay or navigation authority.
- Must not infer a focus domain from navigation priming, visibility, hover, z-order, or compositor state.
- Owns exact previous-target restoration and guarded focus-domain retirement;
  root teardown may supply explicit association/retirement testimony but cannot
  remove or retarget focus itself.
- Owns guarded prepared parent/child addition, subtree removal, restoration
  disposition, and active-domain replacement through one batch graph mutation;
  root-owned membership changes pair
  its sealed ancestry testimony with the exact root-focus binding-port/router
  claim change under one domain guard.
- Complete interaction-domain retirement uses a borrow-free prepared candidate
  over the exact full focus graph and restoration stack, with sealed
  complete-removal testimony and a non-fallible final state of no active focus
  domain or target.
- Must not route pointer/touch input around SPEC-HAL-151 capture or SPEC-HAL-236 hit-test authority.

### HitTestAuthority
- Must not own focus, navigation, overlay lifecycle, or pointer-capture state.
- Must consult only the coordinator-admitted overlay projection plus declared hit-test geometry for the supplied interaction domain.
- Must not broadcast, fall through a consuming overlay, or infer targets from rendering order.

### SurfaceStack
- Must not import from navigation, widget, event, layout, or style modules.
- Must not infer navigation intent.
- Must not retain focus state.
- Must not initiate surface creation or destruction (responds to Compositor commands only).

### ScreenHost
- Must not hold compositor references.
- Must not hold navigation references.
- Must not manage surface lifecycle.
- Must not interpret navigation state.
- Must not own focus routing logic.

### Compositor
- Must not import from navigation, widget, event, layout, or style modules.
- Must not interpret surface semantics (navigation, overlay, focus).
- Must not initiate navigation decisions.
- Must not decide navigation-surface lifecycle eligibility or ordering; it may
  execute only the exact orchestrator-prepared mechanical mutation batch.
- Must not know NavigationHost, NavigationSurfaceOrchestrator, or NavigationRootRegistry exist.

### Renderer
- Must not import from navigation, orchestrator, or projection modules.
- Must not reference `NavigationProjection`, `ProjectedSurfaceView`, or any projection type.
- Must not reference `ScreenHost` or `projected_view`.
- Must not infer navigation semantics from surface properties (z-order, visibility, count, SceneKey patterns).
- Must not reorder, filter, or prioritize surfaces based on meaning.
- Consumes compositor-native `SurfaceStackSnapshot` only. Projection does not enter the renderer domain.

---

## 6. Pipeline Stage Definitions

The frame-projection and rendering paths are separate. Every semantic input has one authority owner; runtime composition and compositor resolution are mechanical stages with zero authority to reinterpret those inputs.

| Stage | Authority Owner | Scope |
|---|---|---|
| Navigation mechanics | NavigationHost | Versioned candidate resolution plus prepared/non-fallible history and cursor commit |
| Navigation permission | Application guard (SPEC-HAL-231) | Scenario-owned allow/deny/replacement evaluation outside the domain transition token; candidate-bound authorization only |
| Navigation orchestration | NavigationSurfaceOrchestrator per context | Semantic ownership and execution of prepared navigation-surface lifecycle plans plus navigation-only surface testimony |
| Projection-testimony binding/epoch | Application `FrameProjectionBindingRegistry` | Explicitly naming every context represented in one ScreenHost view, maintaining root/domain reverse references, and clearing stale epochs; no inferred root selection or visual-order authority |
| Overlay projection | OverlayDomainCoordinator plus admitted owner port | Projecting only the coordinator-admitted stack for the bound interaction domain |
| Focus projection | FocusResolver / authoritative FocusManager | Projecting the selected surface-backed focus target or none for the bound interaction domain |
| Frame composition | Runtime glue | Mechanically assembling `NavigationFrameProjection`; no selection, inference, fallback, or retained cache |
| Surface batch execution / resolution | Compositor | Mechanically executing orchestrator-prepared non-fallible surface batches; mapping navigation-surface `SurfaceId` values to `SurfaceDescriptor`; existence-validating active/overlay/focus IDs; retaining testimony IDs. No lifecycle decision or semantic inference. |
| Storage | ScreenHost | Storing one current resolved `ProjectedSurfaceView`, or no view after epoch change/failure. No interpretation, comparison, accumulation, or projection history. |
| Rendering | Renderer | Consuming compositor-native `SurfaceStackSnapshot` via `Compositor::snapshot()`. No access to projection types. No navigation semantics. Execution-only. |

**Resolution** is defined as mapping/validation-only: the compositor resolves
navigation-surface `SurfaceId` values to `SurfaceDescriptor` records and
existence-validates active-navigation, overlay, and focus IDs while retaining
those fields as IDs. It preserves context grouping plus within-context, context,
and overlay order received from the frame projection. Resolution does not infer,
reorder, filter, silently skip, reinterpret, or convert overlay/focus testimony
to descriptors. It is a mechanical identity lookup and validation boundary.

**Rendering** is defined as execution-only: the renderer reads `SurfaceStackSnapshot` from the compositor, iterates surfaces in compositor-provided order, and delegates to GPU pipelines. The renderer has no knowledge that projection, navigation, or orchestration exist.

The projection data path (authority projections → NavigationFrameProjection → ProjectedSurfaceView → ScreenHost) and the rendering data path (SurfaceStackSnapshot → Renderer) are separate. They share no projection types, state, or communication channel.

---

## 7. Sealed Boundary: Navigation vs Rendering

The boundary between navigation authority and rendering execution is sealed. Navigation semantics do not cross into the renderer. Projection types do not enter the renderer domain. The renderer does not infer navigation state from any source.

This boundary is enforced by:
- Type-level isolation: the renderer imports zero navigation or projection types.
- Guard tests in `src/tests/navigation_projection_boundary_guard.rs` (SPEC-HAL-246, SPEC-HAL-248).
- The governance constitution (SPEC-HAL-232 §7).

Any change to this boundary requires an explicit new SPEC. No incidental edit, refactor, or "cleanup" may introduce navigation awareness into the renderer.

---

## 8. Runtime Implementation Note (0.176.0-beta)

The live `NavigationHost` provides coherent versioned candidates,
candidate-bound guard authorization, and exclusive prepared non-fallible
history/cursor commit. `NavigationSurfaceOrchestrator` owns generation-checked
prepared ordinary transitions, overlay-only exits, hidden staging, exact ordered
termination, prepared abort, and whole-root teardown over compositor-native
mechanical batches. `NavigationRootRegistry` owns duplicate-safe hidden root
admission and resident-root removal; both authorities reject a compositor other
than the store to which they are bound.

The live `NavigationDomainRouter`, registered
`NavigationRootFocusBindingPort` capabilities, central `FocusResolver`, and
`OverlayDomainCoordinator` implement unique port/focus-tree claims, explicit
priming, delegation stacks, guarded root association, owner admission,
single-active-overlay arbitration, exact restoration tokens, batch focus-graph
mutation, and peer-safe retirement. `NavigationApplicationCoordinator` composes
those authorities into non-fallible prepared root admission, ordinary routed
navigation, delegation/return, and root retirement after all fallible work has
succeeded. Delegation-linked retirement reserves and unwinds the exact dependent
frame suffix, explicitly dismisses any active current-effective owner, and folds
its owner-issued surface exit into the same mechanical batch as whole-root
teardown.

The live `FrameProjectionBindingRegistry` owns versioned per-ScreenHost bindings,
epochs, root/domain reverse references, and guarded prepared removal/replacement
with stale-view clearing. The SPEC-HAL-248 runtime frame pull now consumes those
bindings under exactly one coordinator guard, projects every declared navigation
context without partial results, reads overlay and focus testimony from their
independent authorities, validates compositor identity and every surface, and
atomically replaces or clears the bound ScreenHost view. `App::run_ui_loop` and
`App::run` invoke that operation at the declared frame boundary. Each versioned
binding now retains unique source participation claims, and each source's
NavigationHost gate holds their checked same-domain reference count plus prepared
deltas. Every
routed navigation variant and whole-root retirement prepares surface publication
with matching coordinator/domain testimony; the standalone coordinator wrapper
provides the equivalent host/surface transaction without router generations.
Direct NavigationHost execution plus orchestrator dispatch, surface preparation,
and teardown compatibility entry points remain available only for roots that
were never application-admitted. Hidden root admission installs a permanent
same-domain participation claim before registry publication, so every direct
path fails closed for an application root even when it has no ScreenHost
binding.

Full interaction-domain retirement is live as one application-owned prepared
transaction. It acquires one domain token, proves the router's complete
root/port/focus-claim set, enumerates and prepares removal of every immutable
domain projection consumer, reserves complete focus/owner/router/domain
retirement and one all-root compositor batch, and then commits only non-fallible
focus, port, claim, projection, routing, owner, association, surface/root, router,
coordinator, and `InteractionDomains` removals. Inactive domain-local overlay
owners are part of the exact owner set, and lawful domain-local focus trees are
removed by focus authority without becoming router claims. An active owner
outside the resident router-root set rejects unchanged. The orchestrator's local
`focused_surface` field remains compatibility testimony rather than focus
authority.

Cross-references: SPEC-HAL-232 §7, SPEC-HAL-246 §8, SPEC-HAL-248 §9.
