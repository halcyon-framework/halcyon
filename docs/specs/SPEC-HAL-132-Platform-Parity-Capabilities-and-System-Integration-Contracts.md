# SPEC-HAL-132

**Platform Parity Capabilities & System Integration Contracts**

**Status:** Design / Binding
**Layer:** Engine infrastructure (cross-cutting, non-widget)
**Scope:** Capabilities required to reach parity with mature native UI frameworks
**Depends on:** SPEC-HAL-001, 002, 018, 042, 044, 079, 080, 131A, 131B
**Explicit goal:** Define *what* Halcyon must support to match native platforms — without importing *how* they do it.

---

## SECTION A — Problem Statement

Halcyon already exceeds most web-based UI stacks in:

* determinism
* text fidelity
* scroll & virtualization correctness
* architectural separation

However, mature native frameworks (Apple, Android, Qt, Flutter) still possess **system-level integration capabilities** that are not widget features and not renderer tricks.

These gaps are **structural**, not cosmetic.

This SPEC defines **only the missing capability surface**, expressed as **engine-level contracts**, without copying APIs, naming, or internal designs from any platform.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Capability, not imitation**
We define *what must be possible*, not *how others implemented it*.

**PRINCIPLE-2: Engine-owned, not widget-owned**
All capabilities here live above widgets and below applications.

**PRINCIPLE-3: Deterministic first**
No capability may introduce nondeterministic state mutation.

**PRINCIPLE-4: Opt-in surface**
Applications explicitly enable capabilities. Nothing is implicit.

---

## SECTION C — Capability Inventory (What Native Frameworks Have)

The following capabilities are *functionally present* in Apple UIKit/AppKit, Android View system, Qt, and Flutter — though implemented differently.

Halcyon currently lacks **formal contracts** for them.

### C.1 System Composition Participation

Native frameworks can:

* coexist with system UI layers
* negotiate z-order with OS surfaces
* host foreign surfaces (IME, accessibility, video, platform views)
* project application-level commands into system-owned chrome where the platform convention requires it

Halcyon currently renders in isolation.

---

### C.1A System Main Menu Projection

Mature desktop platforms can expose application commands through platform-owned main-menu chrome. On macOS, this is not cosmetic: the system menu bar is the expected application command surface.

Halcyon requires:

* a platform-neutral application menu model
* target-gated native projection where platform convention demands it
* framework-rendered fallback where native menus are insufficient or absent
* activation re-entry through Halcyon's Action System
* no OS ownership of command, edit, focus, routing, lifecycle, clipboard, or window/domain semantics

This capability is specified by SPEC-HAL-333 and mediated by SPEC-HAL-139.

---

### C.2 Accessibility as a First-Class Projection

Native systems treat accessibility as:

* a parallel semantic tree
* synchronized with UI state
* queryable and navigable independently of rendering

Halcyon has SPEC-HAL-023 and 125, but lacks **engine-level projection timing guarantees**.

---

### C.3 Platform Text Services Integration

Beyond shaping and IME:

* spell checking
* grammar services
* dictionary lookup
* OS-native text services lifetimes

Halcyon supports shaping and IME, but not **service orchestration**.

---

### C.4 Cross-Process Input & Focus Negotiation

Native frameworks negotiate focus with:

* OS
* other apps
* system overlays

Halcyon currently assumes exclusive ownership once focused.

---

### C.5 Long-Lived Surface Identity

Native platforms maintain:

* stable surface identity across frames
* stable IDs across resize, minimize, hide, show

Halcyon rebuilds scene graphs cleanly but lacks **surface identity contracts**.

---

### C.6 Power / Thermal / Background Awareness

Native frameworks adapt behavior based on:

* foreground / background state
* power constraints
* thermal signals

Halcyon frame loop is deterministic but not **context-aware**.

---

## SECTION D — Formal Capability Contracts

### D.1 SystemSurface Contract

```rust
struct SystemSurface {
    surface_id: SurfaceId,
    role: SurfaceRole,
    visibility: SurfaceVisibility,
    composition_mode: CompositionMode,
}
```

**INV-SURFACE-1**
Surface identity is stable across frames.

**INV-SURFACE-2**
Surface visibility changes do not destroy surface state.

**INV-SURFACE-3**
SystemSurface may host foreign sub-surfaces (IME, video, accessibility).

**FORBID-SURFACE-IMPLICIT**
Widgets must not assume they are top-level.

---

### D.2 Accessibility Projection Layer

```rust
struct AccessibilityNode {
    semantic_id: SemanticId,
    role: AccessibilityRole,
    bounds: LogicalRect,
    state: AccessibilityState,
}
```

**INV-A11Y-1**
Accessibility tree is derived, never authored manually.

**INV-A11Y-2**
Accessibility updates are frame-synchronous but render-independent.

**INV-A11Y-3**
Focus and selection changes propagate to accessibility before paint.

**FORBID-A11Y-WIDGET-QUERY**
Widgets may not query accessibility state.

---

### D.3 Platform Text Services Contract

```rust
trait TextService {
    fn spellcheck(&self, range: TextRange);
    fn grammar_hint(&self, range: TextRange);
    fn lookup(&self, position: TextPosition);
}
```

**INV-TEXT-SERVICE-1**
Services are advisory. They never mutate text.

**INV-TEXT-SERVICE-2**
Service results are versioned against text epoch.

**INV-TEXT-SERVICE-3**
IME composition has priority over all services.

---

### D.4 Focus & Input Negotiation

**INV-FOCUS-EXT-1**
Focus acquisition may be denied by the platform.

**INV-FOCUS-EXT-2**
Loss of focus does not destroy widget state.

**INV-FOCUS-EXT-3**
Focus transitions are explicit engine events.

---

### D.5 Surface Lifecycle Awareness

```rust
enum AppVisibility {
    Foreground,
    Background,
    Suspended,
}
```

**INV-LIFECYCLE-1**
Visibility changes are delivered before frame scheduling.

**INV-LIFECYCLE-2**
Backgrounded surfaces must not schedule animations.

**INV-LIFECYCLE-3**
Suspended surfaces must release transient GPU resources.

---

### D.6 Power & Performance Hints

```rust
struct PerformanceContext {
    power_mode: PowerMode,
    thermal_state: ThermalState,
    refresh_budget: FrameBudget,
}
```

**INV-PERF-CTX-1**
Hints are advisory, never mandatory.

**INV-PERF-CTX-2**
Deterministic output is preserved regardless of hints.

---

## SECTION E — What This SPEC Explicitly Does NOT Do

* ❌ No widget APIs
* ❌ No OS-specific bindings
* ❌ No imitation of UIKit / Android / Qt internals
* ❌ No policy decisions

This SPEC defines **capability surfaces only**.

---

## SECTION F — Relationship to Existing SPECs

* Complements **SPEC-HAL-018 (App Framework)**
* Extends **SPEC-HAL-023 / 125 (Accessibility)**
* Integrates with **SPEC-HAL-042 / 044 (Frame Loop, UI Thread)**
* Builds on **SPEC-HAL-131A/B (Scroll & Virtualization)**

No conflicts.

---

## SECTION G — Competitive Positioning (Reality Check)

After SPEC-HAL-132:

Halcyon will still **not** have:

* decades of platform bugs fixed
* vendor-specific optimizations
* proprietary OS shortcuts

But it **will** have:

* cleaner separation than UIKit
* more deterministic behavior than Android
* stronger text pipeline than Flutter
* stricter invariants than Qt

This SPEC closes the **structural gap**, not the historical one.

---

## SECTION H — Acceptance Criteria

SPEC-HAL-132 is complete when:

* [ ] Engine can host foreign system surfaces
* [ ] Accessibility tree updates without render dependency
* [ ] Focus loss / denial is handled explicitly
* [ ] Text services integrate without mutating text
* [ ] Frame loop reacts to visibility changes
* [ ] Power hints do not affect determinism
* [ ] No widget queries platform state directly

---

## SECTION I — Authoritative Statements

**AUTH-132-1: “System integration is projection, not control”**
The OS observes Halcyon. It does not drive it.

**AUTH-132-2: “Capabilities precede policy”**
Policy belongs to applications, not the engine.

**AUTH-132-3: “Parity is not imitation”**
Independent convergence is not theft.
