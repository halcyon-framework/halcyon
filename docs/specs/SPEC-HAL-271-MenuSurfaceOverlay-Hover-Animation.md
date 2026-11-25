# SPEC-HAL-271: **MenuSurfaceOverlay Hover Animation**

### *Frame-Driven Highlight Interpolation for Menu Item Rows*

**Author:** JC
**Status:** Binding
**Layer:** Overlay Rendering
**Scope:** `MenuSurfaceOverlay` only
**Depends on:**
- SPEC-HAL-002 (Constitution & Architectural Invariants)
- SPEC-HAL-223 (MenuSurface Contract Invariants)
- SPEC-HAL-222 (MenuItem Primitive Invariants)

---

# 1. Purpose

This SPEC defines the **hover animation contract** for `MenuSurfaceOverlay`.

It specifies:

* Why animation is applied at the overlay layer, not the model layer
* The exact animation model and timing parameters
* The invariants that ensure animation remains strictly cosmetic
* Constitutional compliance

---

# 2. Motivation

SPEC-HAL-223 Section 2 (Non-Goals) explicitly defers visual styling and animation curves to other SPECs. This is that SPEC for highlight animation.

Without animation, `highlighted_index` changes propagate as instant background color swaps — perceptually jarring as the pointer moves between items. A short fade-in smooths the highlight without introducing any semantic ambiguity.

---

# 3. Architectural Placement

## 3.1 Model vs. Overlay

`MenuSurface` (model) operates on integer indices only. It has no knowledge of time, pixel coordinates, or color values. This is correct and must not change.

`MenuSurfaceOverlay` owns all geometry and rendering decisions. It is the correct and only place to introduce animation state.

**INV-HAL271-PLACEMENT**: Hover animation state MUST reside in `MenuSurfaceOverlay`. `MenuSurface` MUST remain time-agnostic and index-only.

## 3.2 Shared Surface

`MenuSurfaceOverlay` is embedded by `ContextMenu`, `PopupMenu`, and `DropdownMenu`. Animation is therefore available uniformly across all three without any per-widget changes.

---

# 4. Animation Model

## 4.1 Trigger

Animation begins when `highlighted_index` changes value — detected by comparing the current value from the model against a cached value in `MenuSurfaceOverlay`.

**Change detection runs in `paint()`** — the only place both `cx.frame_time_ms()` and the model's current `highlighted_index` are simultaneously available.

## 4.2 Direction

- **Fade in only.** When a new item becomes highlighted, its background row color interpolates from `row_background` → `row_hover` over `HOVER_ANIM_MS`.
- **Snap out.** When an item loses highlight (pointer leaves, or moves to another item), it reverts to `row_background` immediately — no fade-out.

Rationale: fade-in provides the perceptual smoothing benefit. Fade-out adds no benefit and introduces ghost trails as the pointer moves rapidly between items.

## 4.3 Parameters

```
HOVER_ANIM_MS = 80.0
```

80ms is fast enough to feel responsive, slow enough for the eye to register as smooth.

## 4.4 Interpolation

```
bg_color = row_background.lerp(row_hover, hover_t)
```

where:

```
elapsed = now_ms - hover_anim_start_ms
hover_t = (elapsed / HOVER_ANIM_MS).clamp(0.0, 1.0)
```

Linear interpolation. No easing curve — at 80ms the difference is imperceptible.

## 4.5 Non-highlighted items

All items that are not currently highlighted always render `row_background` with no interpolation. No per-item animation state.

---

# 5. State

Two fields added to `MenuSurfaceOverlay`:

| Field | Type | Description |
|---|---|---|
| `hover_anim_index` | `Cell<Option<usize>>` | Index that is currently animating in |
| `hover_anim_start_ms` | `Cell<u64>` | Frame timestamp when animation started |

Both are `Cell<_>` because `paint()` takes `&self`.

No allocation. No heap. No per-item state. O(1) storage regardless of item count.

---

# 6. Invariants

**INV-HAL271-PLACEMENT** — Animation state in overlay only. Model is time-agnostic.

**INV-HAL271-COSMETIC** — Animation affects rendered color only. It MUST NOT affect:
- hit testing
- index resolution
- activation timing
- dismissal logic
- keyboard navigation

Complies with SPEC-HAL-223 INV-G1.

**INV-HAL271-NO-TIMER** — Animation MUST NOT use timers, deferred events, or callbacks to advance state. It advances solely by reading `cx.frame_time_ms()` in `paint()`. Complies with SPEC-HAL-223 INV-G2.

**INV-HAL271-NO-ANIM-HANDLE** — Animation MUST NOT use the engine's `AnimationHandle` / `start_animation` system. Frame-time sampling in `paint()` is sufficient and avoids cross-phase state.

**INV-HAL271-FADE-IN-ONLY** — Only fade-in is animated. Fade-out is an instant snap. This is the specified behavior, not a limitation.

**INV-HAL271-ZERO-ALLOC** — No per-frame allocation. Animation state is two `Cell` fields. Per-item lerp is computed inline.

**INV-HAL271-CHANGE-DETECT** — Change detection is performed by comparing `highlighted_index` (from model, current frame) against `hover_anim_index` (cached in overlay). On mismatch: update cache, reset `hover_anim_start_ms` to `now_ms`.

---

# 7. Constitutional Compliance

**Pillar B — Phase Separation**: All animation state mutation occurs inside `paint()` via `Cell`. No mutation during layout or event routing.

**Pillar E — GPU-First Rendering**: `hover_t` is computed from frame time and baked into the color value passed to the scene graph. The scene graph receives a static color — no animated handles, no mid-frame mutation.

**SPEC-HAL-223 INV-G1**: Animation affects appearance only. Routing, activation, and dismissal are unaffected.

**SPEC-HAL-223 INV-G2**: No timers. Time is read from `cx.frame_time_ms()` at paint time only.

---

# 8. Files

| File | Change |
|---|---|
| `src/widgets/menu_surface.rs` | Add `HOVER_ANIM_MS`, two `Cell` fields to `MenuSurfaceOverlay`, change detection + lerp in `paint()` |

No other files change.
