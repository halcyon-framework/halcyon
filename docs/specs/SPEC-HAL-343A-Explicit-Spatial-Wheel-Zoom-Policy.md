# SPEC-HAL-343A: Explicit Spatial Wheel-Zoom Policy

**Author:** JC
**Status:** Adopted / Binding
**Amends:** SPEC-HAL-343 §5.2.10 and §5.4
**Reconciles:** SPEC-HAL-294 §7.1
**Depends on:** SPEC-HAL-294, SPEC-HAL-296, SPEC-HAL-336

---

# 1. Purpose

`SpatialInteractionPolicy` owns the explicit table of domain-neutral spatial
gestures. Its drag gesture entries can opt out through
`SpatialGestureClass::Disabled`, but wheel zoom is currently represented only by
`zoom_scroll_requires_control: bool`.

That boolean distinguishes Ctrl/Cmd-required zoom from zoom without that
requirement. It cannot express a spatial surface that supports selection while
explicitly disabling wheel zoom. Silent widget-local rejection is not an
equivalent policy declaration under SPEC-HAL-343 INV-343-I2 and INV-343-M3.

This amendment replaces that boolean-only contract with one closed wheel-zoom
policy. It does not redesign gestures, event routing, scrolling, viewport math,
or domain interaction.

---

# 2. Existing Contract and Gap

The live policy field is:

```rust
pub zoom_scroll_requires_control: bool
```

The two live field consumers, `NodeFlowSurface` and the Showcase node-graph
surface, resolve it identically:

```text
true  -> accept a nonzero wheel delta only with Ctrl or Meta
false -> accept a nonzero wheel delta without requiring Ctrl or Meta
```

Neither value disables wheel zoom. `Widget::wants_scroll()` is a routing and
ownership declaration: `false` makes the router skip that widget and seek the
nearest ancestor that wants scroll, or drop the event when none exists. It is
not a `SpatialInteractionPolicy` entry and cannot substitute for an explicit
wheel-zoom opt-out.

---

# 3. Policy Amendment

## 3.1 Closed policy type

The shared spatial API adds exactly one domain-neutral enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SpatialWheelZoomPolicy {
    Disabled,
    Unmodified,
    ControlOrMeta,
}
```

`SpatialInteractionPolicy` replaces the boolean field with:

```rust
pub wheel_zoom: SpatialWheelZoomPolicy
```

The legacy boolean is removed. It must not remain as a deprecated parallel
field because two independently writable entries could disagree about one
gesture.

## 3.2 Resolution

`SpatialInteractionPolicy` is the single resolver:

```rust
pub fn wheel_zoom_requested(&self, modifiers: Modifiers) -> bool
```

The result is:

| Policy | Resolution |
|---|---|
| `Disabled` | Always `false`. |
| `Unmodified` | Always `true`; no Ctrl/Meta modifier is required. Other modifier bits do not suppress zoom. |
| `ControlOrMeta` | `true` exactly when `modifiers.ctrl || modifiers.meta`. |

`Unmodified` means that wheel zoom has no modifier requirement. It does not
mean that a modifier is forbidden. This preserves the complete legacy `false`
behavior, including a wheel event that also carries Ctrl or Meta.

The resolver classifies modifier policy only. Consumers retain their existing
zero-delta checks, zoom factors, anchors, viewport authority, intent routing,
and invalid-state behavior.

## 3.3 Default and legacy mapping

`SpatialInteractionPolicy::default().wheel_zoom` is `ControlOrMeta`, preserving
the current default.

The source migration is exact:

| Legacy configuration | Replacement |
|---|---|
| `zoom_scroll_requires_control: true` | `wheel_zoom: SpatialWheelZoomPolicy::ControlOrMeta` |
| `zoom_scroll_requires_control: false` | `wheel_zoom: SpatialWheelZoomPolicy::Unmodified` |
| no field override / `SpatialInteractionPolicy::default()` | `ControlOrMeta` |
| no legacy representation | `SpatialWheelZoomPolicy::Disabled` |

---

# 4. Invariants

**INV-343A-1: Closed Wheel-Zoom Policy**
Every shared-policy wheel-zoom decision is one of `Disabled`, `Unmodified`, or
`ControlOrMeta`. Consumers do not add private modifier booleans beside it.

**INV-343A-2: Disabled Is Complete**
`Disabled` never produces a wheel-zoom viewport mutation or viewport intent,
regardless of modifiers. It does not disable selection, hover, or separately
declared drag gestures.

**INV-343A-3: Default Compatibility**
The default continues to require Ctrl or Meta. Existing default-backed node-flow
surfaces accept and reject the same wheel events as before this amendment.

**INV-343A-4: Routing Remains Separate**
This policy does not change `Widget::wants_scroll()`, EventRouter target
selection, bubbling, capture, input fusion, or wheel-delta coalescing. A widget
that owns wheel routing still declares that through the existing Widget
contract.

**INV-343A-5: Domain Authority Does Not Move**
The policy resolves whether wheel zoom is requested. Domain sessions and
viewport engines continue to own the resulting state change. Plot axes,
Timeline time geometry, InfiniteCanvas compatibility state, and NodeEditor
session state remain with their existing owners.

---

# 5. Consumer Migration and Reduction Audit

The following table records the at-adoption consumer baseline and its use of
§3.3's migration map. It is non-normative source-audit evidence, not an ongoing
application policy or a claim that migration has completed. Normative wheel
policy, legacy mapping and preserved behavior remain defined by §§3–4.

| Surface | Live behavior | Baseline mapped policy/migration | Behavior change |
|---|---|---|---|
| `NodeFlowSurface` | Ctrl/Meta+wheel zoom; plain wheel rejected | Keep default `ControlOrMeta`; replace the private boolean branch with `wheel_zoom_requested` | None |
| Showcase node graph | Ctrl/Meta+wheel zoom; plain wheel rejected | Keep default `ControlOrMeta`; use the shared resolver | None |
| Standalone node-graph example | Wheel zoom without a modifier requirement | Declare `Unmodified`; use the shared resolver | None |
| Spatial viewport example | Wheel zoom without a modifier requirement | Declare `Unmodified`; use the shared resolver; preserve its existing animation, factors, and anchor | None |
| `TimelineCanvas` | Plain vertical wheel zoom; Shift+wheel pans time | Its exposed policy declares `Unmodified`; Shift+wheel keeps its existing domain-owned precedence | None |
| `Plot` | Wheel zoom; Space+wheel pans | No migration. Plot does not consume `SpatialInteractionPolicy`, and SPEC-HAL-343 §6.3 keeps Plot coordinate interaction domain-owned | None |

SPEC-HAL-294 §7.1's binding row becomes, on adoption:

| Policy entry | Node flow value | Authority |
|---|---|---|
| `wheel_zoom` | `ControlOrMeta` | Wheel zoom requires Ctrl/Cmd and resolves through viewport/session intent. |

No other SPEC-HAL-294 interaction row changes. SPEC-HAL-336's plain-wheel zoom
and Shift+wheel pan semantics remain unchanged.

---

# 6. Public API and Persistence Impact

`SpatialInteractionPolicy` is a public struct, so replacing a public field is a
source-breaking API change for external field readers and struct literals. The
implementation therefore requires the project's normal pre-1.0 minor version
bump and changelog entry. The mapping in §3.3 is complete; no compatibility
boolean or second source of truth is permitted.

The live type derives `Debug`, `Clone`, `PartialEq`, and `Eq`. It has no Serde
derive, custom serialization implementation, persistence schema, configuration
file format, or wire representation. This amendment changes no stored data and
requires no data migration.

---

# 7. Required Verification

Implementation is complete only when focused evidence proves:

1. the default is `ControlOrMeta`;
2. `Disabled` rejects plain, Ctrl, Meta, and combined-modifier wheel zoom;
3. `Unmodified` accepts the same modifier combinations previously accepted by
   legacy `false`;
4. `ControlOrMeta` rejects plain wheel and accepts Ctrl or Meta exactly as the
   legacy default did;
5. `NodeFlowSurface` and the Showcase node graph preserve their current event
   results and viewport-intent behavior;
6. the standalone node graph, spatial viewport, and TimelineCanvas preserve
   their current unmodified-wheel behavior, including Timeline Shift+wheel pan;
7. Plot's existing wheel tests remain unchanged and pass without Plot importing
   the shared policy;
8. every `SpatialInteractionPolicy` construction and field read has been
   migrated, and no `zoom_scroll_requires_control` reference remains; and
9. examples and the affected package tests compile and pass.

Named examples in the proof above are controlled compatibility fixtures. They
do not prescribe application configurations.

The reduction scan must find no generic gesture enum redesign, event-router
change, new scroll-routing mode, viewport-math change, animation change, Plot
migration, serialization layer, compatibility boolean, or domain-specific
variant in `SpatialWheelZoomPolicy`.

---

# 8. Out of Scope and Review Gate

This amendment does not authorize implementation while its status is `Draft`.
After adoption, implementation is limited to the type replacement, migration
through §3.3's exact legacy mapping, tests, public-version/changelog work, and
dependent SPEC row reconciliation named above.

Touch, trackpad gesture taxonomy, axis-specific wheel policy, scroll-container
behavior, wheel acceleration, zoom animation, pan policy, a generic gesture
framework, and changes to Locus are out of scope.
