# SPEC-HAL-229: NavigationHost Context Scoping & Projection Binding

**Status:** Draft (Binding upon adoption)
**Layer:** Application Navigation / Surface Architecture
**Scope:** NavigationHost context scoping, projection binding, multi-context and multi-surface rules
**Applies To:** NavigationHost, Breadcrumbs, TabBar, future navigation controls, overlays
**Ground Truth:** SPEC-HAL-151, SPEC-HAL-201, SPEC-HAL-220, SPEC-HAL-226, SPEC-HAL-227, SPEC-HAL-228

---

## 0. Purpose

SPEC-HAL-228 defines NavigationHost as the single source of truth for navigation state. It does not define how NavigationHost relates to surfaces (windows, modals, overlays) or how controls bind to a host.

Halcyon is multi-surface and overlay-heavy. Without explicit scoping rules:
- implementations will assume a single global navigation state
- overlays will inherit or corrupt parent navigation by accident
- controls will scan widget trees or focus trees to "find" a host
- multi-window configurations will share state that must be independent

This SPEC locks context scoping, command targeting, and explicit projection binding before those assumptions enter code.

---

## 1. Definitions (Binding)

### Surface

A surface is an independent rendering and event root. Surfaces include:
- a window
- a modal surface
- an overlay surface

A surface is not automatically a navigation context. A surface may project zero, one, or multiple explicitly identified navigation contexts in distinct application-owned regions. One navigation context may project a sequence of navigation-owned surfaces through the navigation orchestrator. Rendering containment and navigation authority must not be conflated.

### Navigation Context / Navigation Root

An independent semantic navigation authority with its own NavigationHost, committed current state, history entries/cursor, guard configuration, projections, and lifecycle boundary. `Navigation context` and `navigation root` name the same authority boundary in this SPEC series.

A pane, region, overlay, or embedded workflow becomes a distinct navigation context only when it has independently meaningful navigation state/history. Visual nesting alone does not create a context.

### Projection

A projection is a navigation control (Breadcrumbs, TabBar, etc.) that renders caller-supplied state derived from a NavigationHost. A projection has no navigation authority. It consumes projection data and emits intents.

### Binding

Binding is the caller-owned association of projection data and an intent sink with one specific navigation context. The caller reads that context's NavigationHost and routes emitted intents back to the same context; the control never receives the host itself. Binding is never discovered, inferred, or inherited.

---

## Invariant Class A — Authority & Scope

### INV-229-A1: NavigationHost Is Context-Scoped

A NavigationHost owns navigation state and history for exactly one navigation context. The context may project multiple navigation-owned surfaces over its lifetime, but the host does not own state for another context. A NavigationHost must not outlive its owning context.

---

### INV-229-A2: Multiple NavigationHosts May Exist Simultaneously

Multiple contexts may each own a NavigationHost. They may coexist in the same application, window, screen, split view, inspector, embedded workflow, or overlay. Multiple contexts may be resident and rendered simultaneously. These hosts are independent. No synchronization occurs between them unless explicitly implemented by application code outside the host.

---

### INV-229-A3: No Implicit Cross-Surface Propagation

Navigation state changes in one host must not propagate to another host. There is no broadcast, no event bus, no shared state between hosts unless application code explicitly creates it.

---

## Invariant Class B — Projection Binding

### INV-229-B1: Controls Never Query Ambient Navigation State

No navigation control may:
- search for a NavigationHost in the widget tree
- query a global or singleton host
- infer host identity from context, focus, or ancestry

Controls receive state explicitly or render nothing.

---

### INV-229-B2: Controls Receive Navigation Projection Explicitly

A navigation control must be given its `NavigationControlProjection` data by the caller. The caller is responsible for reading the coherent snapshot from the correct NavigationHost and providing it after initial binding and each committed navigation change.

The control never holds a reference to the host. It holds projected data. Binding occurs through projected data, not through references to NavigationHost itself.

---

### INV-229-B3: No Silent Rebinding

A control instance must not silently change which context it projects. Rebinding is an explicit caller operation that atomically replaces both projected data and the caller-provided intent sink. Supplying projection data from one context while retaining a sink bound to another is forbidden.

Each sink is local to one binding instance and must not be shared across controls or navigation contexts. Rebinding or teardown discards undrained intents from the old sink before replacing or removing the whole binding. The control does not discover or validate host identity; the caller owns the explicit atomic binding operation.

---

### INV-229-B4: Scoped Intent Delivery Is Caller-Bound

A context-local Back, Forward, breadcrumb, tab, route, or content control receives projected navigation data and a caller-provided intent sink bound to one navigation context. The control emits into that sink. It holds neither NavigationHost nor NavigationRootRegistry and never resolves a target through ambient state.

The application drains the sink. In a standalone single-root composition permitted by SPEC-HAL-247, the explicit binding may deliver the intent to that root's host/orchestrator directly. In a multiroot composition, the application submits the bound domain/root identity plus intent through SPEC-HAL-237 interaction-domain routing validation; successful routing produces the explicit target passed to NavigationRootRegistry. If the originating interaction is an application-declared priming boundary, the application issues the separate explicit priming operation before routing the intent. Priming and delegation semantics are owned by SPEC-HAL-237, not duplicated here.

### INV-229-B5: Unscoped Controls Do Not Select a Context

A window toolbar, application menu, keyboard shortcut, or platform Back/Forward control that is intentionally unscoped carries the existing `InteractionDomainId` defined by SPEC-HAL-151 and no navigation-root identity. The control does not select a visible, focused, hovered, or highest-z context.

The interaction-domain navigation-routing contract in SPEC-HAL-237 resolves the command to an explicit root identity before NavigationRootRegistry dispatch.

### INV-229-B6: History Availability Is Projected Per Context

`can_history_back`, `can_history_forward`, and `can_navigate_up` are projections of one explicitly identified context. Controls must not combine availability across contexts.

---

## Invariant Class C — Overlays

### INV-229-C1: Overlays Are Surfaces

An overlay surface is not thereby a navigation root. It may:
- own its own NavigationHost (independent navigation within the overlay)
- project a parent context's NavigationHost (if explicitly bound by the caller)
- own no NavigationHost (no navigation state within the overlay)

All three are valid. The choice is made by the caller, never by the overlay.

---

### INV-229-C2: Overlay Closure Must Not Mutate Parent Host Implicitly

Closing, dismissing, or destroying an overlay must not:
- mutate the parent context's NavigationHost state
- emit navigation intents to the parent host
- trigger `NavigateUp`, history traversal, or any other navigation intent

If an overlay must affect parent navigation on close, the application code must emit that intent explicitly after overlay dismissal.

---

### INV-229-C3: Tooltip/Menu Overlays Must Not Introduce Navigation Authority

Overlays governed by SPEC-HAL-226 (TooltipSurface) and SPEC-HAL-227 (Tutorial) are informational and policy surfaces. They must not:
- own a NavigationHost
- project navigation state
- emit navigation intents

Menu overlays (DropdownMenu, PopupMenu) may emit selection intents that the caller maps to navigation intents. The menu itself has no navigation authority. Menu overlays must not directly emit NavigationIntent.

---

## Invariant Class F — Forbidden Behaviors

### INV-229-F1: No Singleton/Global NavigationHost

There must be no globally accessible NavigationHost instance. No static, no thread-local, no service locator. Hosts are owned by their navigation context or the application code that creates that context.

---

### INV-229-F2: No Widget-Tree Scanning

No code may traverse the widget tree to locate a NavigationHost. Host identity is never structural.

---

### INV-229-F3: No Focus-Tree Inference

No code may use focus state, focus ancestry, or keyboard focus to determine which NavigationHost a control should project. Focus and navigation are independent concerns.

---

## 2. Canonical Scenarios

### Two Windows, Two Hosts

Window A owns HostA. Window B owns HostB. Each window contains Breadcrumbs. BreadcrumbsA projects HostA state. BreadcrumbsB projects HostB state. Navigating in Window A has no effect on Window B.

### Modal Wizard With Its Own Host

A modal overlay inside the parent's per-window `InteractionDomainId` owns HostModal for wizard step navigation. The parent window owns HostParent. Breadcrumbs inside the modal project HostModal. The application closes the modal by explicitly unwinding any delegation to HostModal and then destroying HostModal. HostParent is unchanged. The modal may establish a distinct focus domain under SPEC-HAL-235, but that does not create another interaction domain or navigation history.

While the modal is delegated, HostModal is the effective target of unscoped navigation commands. Returning from the modal explicitly unwinds the delegation and restores the previously primed parent context. An application-controlled close must perform that unwind explicitly before destroying HostModal. Reaching the modal history boundary does not implicitly close it or fall through into parent history.

### Split Workbench With Peer Contexts

A main workspace and inspector are simultaneously visible, independently navigable contexts. An interaction explicitly primes the inspector. Inspector-local controls emit through sinks bound to the inspector root; interaction-domain routing dispatches those intents to HostInspector. Unscoped window Back/Forward commands also target HostInspector until the workspace is explicitly primed or a delegated override supersedes it. Neither history is merged or broadcast.

### Overlay Breadcrumbs Bound to Parent Host

An overlay contains Breadcrumbs supplied with HostParent projection data and an intent sink bound by the caller to the parent domain/root. The overlay does not own a host. Clicking a crumb emits through that sink; the caller routes the bound identity and intent through SPEC-HAL-237 validation, then NavigationRootRegistry dispatches the resulting explicit parent-root target. This is valid because the binding is explicit.

---

## 3. Acceptance Criteria

- Any navigation control instantiated without projection data renders nothing (zero visual, zero layout, zero hit regions) — consistent with SPEC-HAL-220 absent-authority behavior
- No spec language in SPEC-HAL-228 or SPEC-HAL-229 implies a single global navigation state
- Multiple NavigationHost instances can coexist without interference
- Multiple navigation contexts may be simultaneously resident and visible in one window
- Exactly zero or one context per interaction domain is the effective target of an unscoped navigation command
- Context-local control intents are delivered through explicit context binding even when another context is primed
- Priming is explicit retained state and is never inferred from focus, widget ancestry, or visibility
- Delegated overrides supersede and later restore priming without merging navigation state/history
- Overlay creation and destruction does not mutate any NavigationHost unless application code explicitly emits an intent
- No navigation control discovers its host — all binding is caller-provided
- Removing caller-provided navigation projection/intent binding from a surface must not degrade into implicit or cached navigation behavior
