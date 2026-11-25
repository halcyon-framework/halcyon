# SPEC-HAL-232: Navigation and UI Governance Constitution

**Status:** Foundational / Binding
**Layer:** System Constitution / Authority Model
**Owner:** Core Architecture
**Depends On:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-228 through SPEC-HAL-231
**Applies To:** All surface-hosting controls, navigation systems, and projected UI

---

## 0. Purpose

This SPEC seals the NavigationHost system (HAL-228–231) as the sole lawful authority for semantic navigation resolution and navigation-owned projection. It forbids manual navigation-state pushing, ambient host discovery, and ad-hoc route resolution. Surface and overlay lifecycle that does not represent navigation is governed by its explicitly declared surface/overlay authority rather than being forced through NavigationHost.

This SPEC elevates HAL-228–231 from implementation to constitutional law.

---

## 1. Motivation

The introduction of NavigationHost, ApplicationGuard, and the structural override model (HAL-230/231) provided a coherent, gateable architecture for runtime UI projection. However, without formal constitutional elevation, these mechanisms remain optional conventions, susceptible to circumvention by:
- Direct calls to NavigationHost::resolve() from controls
- Manual widget instantiation from imperative logic
- Inlined view switching logic within stateful widgets
- Navigation-owned surface changes bypassing committed navigation resolution
- Surface or overlay creation bypassing its declared lifecycle authority

Such patterns fatally compromise hierarchy consistency, lifecycle safety, and resolution auditability.

This SPEC closes all such escape hatches.

---

## 2. Authority Law and Binding Invariants

### INV-232-A: Navigation-Owned Surfaces Follow NavigationHost

A surface whose lifecycle represents a navigation destination may be instantiated, composed, or activated only from a committed NavigationHost decision through NavigationSurfaceOrchestrator. Independent overlays, tooltips, menus, and other non-navigation surfaces follow their owning lifecycle SPEC and must not acquire a NavigationHost merely to exist.

### INV-232-B: Navigation Projection Uses Declared, Explicit Binding

Navigation controls and navigation-owned surfaces are bound via data-driven projection inputs. Runtime surfaces do not hold, request, or infer their own NavigationHost.

### INV-232-C: Navigation Execution Always Uses the Guard Seam

Every executable navigation candidate passes through the NavigationGuard seam. An installed application guard may allow, deny, or replace it. When no application guard is installed, the seam deterministically supplies `Allow` as required by SPEC-HAL-231; absence of application policy is not absence of guard semantics. Non-navigation surface lifecycle is not guard-gated unless it explicitly requests a `NavigationIntent`.

### INV-232-D: All authority over navigation resolution is held by NavigationHost

No control, widget, helper, or view may perform resolution logic outside the host pipeline. Lookup, override, and fallback are host-internal only.

### INV-232-E: Application overrides are first-class

Guard-based navigation overrides are lawful. Imperative route-resolution hacks are not. Any dynamic navigation resolution must pass through guard policy evaluation.

### INV-232-F: Surface identity implies no authority

Surface IDs, types, or labels must not be used to imply resolution or projection rights. Identity != authority.

### INV-232-G: All Authority Paths Must Be Auditable

Navigation projection chains must be reconstructable from NavigationHost. Independent surface and overlay origins must be reconstructable from their declared lifecycle authority. No surface origin may be hidden or inferred from rendering state.

---

## 3. Forbidden Patterns (Effective Immediately)

- Constructing or activating a navigation-destination view inside a runtime widget path in a way that bypasses a committed NavigationHost decision and NavigationSurfaceOrchestrator. Ordinary construction or rebuild of non-navigation child widgets remains lawful under the owning widget lifecycle contract.
- `.with_navigation_host(host.clone())` on a surface
- Manually calling `host.resolve("/some/path")` in a UI control
- Inlined match arms with local resolution logic
- Widget-owned navigation resolution or navigation-owned surface construction bypassing NavigationHost
- Surface or overlay lifecycle mutation bypassing its declared authority
- Control-dependent route switching not routed through guard+projection

---

## 4. Downstream Enforcement and Dependencies

All future SPECs involving:
- Wizard, Stepper, Onboarding sequences
- Overlay, Tooltip, Coachmark, Dialog surfaces
- Multiroot or NestedHost structures

...must explicitly conform to HAL-232 authority law. Navigation resolution, navigation projection, or navigation-owned UI advancement is not valid without:
1. Declared NavigationPath
2. Host-governed resolution
3. Execution through the guard seam

An overlay, tooltip, coachmark, dialog, or other surface with no semantic navigation state instead follows its owning surface lifecycle, overlay, focus, and hit-test contracts. Such a surface must not create a synthetic NavigationHost or NavigationPath.

---

## 5. Constitutional Elevation

Upon adoption, HAL-232 becomes a constitutional override over any prior SPECs that suggest or permit imperative navigation resolution, authority inference, or widget-owned navigation destination instantiation.

All existing components must be audited for compliance.

---

## 6. Transition and Legacy Components

A one-time audit exception may be granted for legacy widgets authored prior to HAL-228, provided they:
- Do not control navigation outside their domain
- Do not instantiate multi-surface hierarchies
- Are stateless and layout-only

Any such components must be locked from future projection behavior.

---

## 7. Sealed Boundary: Navigation Semantics vs Rendering

NavigationHost owns path resolution and guard evaluation. NavigationSurfaceOrchestrator owns translation of committed navigation decisions into surface lifecycle changes. The central Focus Resolver under SPEC-HAL-235 exclusively selects focus domains and focus targets; navigation code may submit explicit activation requests or constraints but must not select or retain authoritative focus. Overlay composition and hit-test authority remain governed by SPEC-HAL-236. No rendering code participates in, observes, or infers any of these semantics.

Rendering is execution-only. The renderer consumes compositor-native surface snapshots (`SurfaceStackSnapshot`) produced by `Compositor::snapshot()`. It iterates surfaces in compositor-provided order, delegates to GPU pipelines, and terminates. The renderer does not import, reference, or receive `NavigationProjection`, `ProjectedSurfaceView`, or any type from the navigation or orchestrator modules.

Projection types (`NavigationProjection`, `NavigationFrameProjection`, `ProjectedSurfaceView`) exist on a separate data path that terminates at ScreenHost (SPEC-HAL-248). They are not part of the renderer domain. The renderer has no knowledge that projection exists.

This boundary is sealed. Any change to it — introducing navigation awareness into the renderer, exposing projection types to rendering code, or allowing the renderer to infer navigation state from surface properties — requires an explicit new SPEC that states the new boundary. Incidental edits to existing SPECs do not constitute boundary changes.

Guard tests in `src/tests/navigation_projection_boundary_guard.rs` enforce this boundary at the code level.

Cross-references: SPEC-HAL-246 §7 (terminology note), SPEC-HAL-247 §6–§7 (pipeline stage definitions, sealed boundary), SPEC-HAL-248 §10 (sealed boundary: projection vs rendering).

---

## 8. Enforcement Status

Binding. Non-optional. Foundational.

All systems layered on top of Halcyon must assume HAL-232 as law.
No future component, control, or subsystem may override this without formal constitutional amendment.
