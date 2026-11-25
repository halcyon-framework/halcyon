# SPEC-R-021 — RoundedRect Pipeline Contract

**Author:** Halcyon Architect
**Status:** Active (Binding)
**Version:** 1.0

**Depends On:**

* SPEC-R-001 Clip Stack Rework
* SPEC-R-004 GPU Path Renderer
* SPEC-R-020 GPU Scissoring

---

## 1. Purpose

This SPEC establishes a **binding contract** for RoundedRect primitive rendering to prevent regression of the clip bleed bug (commit `a0e0a1f`, January 2026).

This document is authoritative. Violations require explicit SPEC amendment.

---

## 2. Background: The Bug

### 2.1 What Happened

Commit `a0e0a1f` ("Renderer: implement real RoundedRect via path triangulation") changed `add_rounded_rect()` from:

**Before (rect pipeline):**
```rust
self.primitives.push(FlatPrimitive {
    key: DrawBatchKey::rect(clip_state.hash, z_index),
    data: PrimitiveData::RoundedRect { rect, color, radius },
    clip_state,
    order_key,
});
```

**After (path pipeline):**
```rust
let path = Path::rounded_rect(rect, radius);
let mesh = path.to_mesh();
let path_draw = PathDraw::fill(mesh, color_array);
self.add_path(path_draw, z_index);
```

### 2.2 Why It Broke

| Pipeline | Scissor Behavior | Result |
|----------|------------------|--------|
| **Rect Pipeline** | Per-batch `set_scissor_rect(clip_rect)` | Correct clipping per widget |
| **Path Pipeline** | Full viewport + shader `discard` | Clip bleed across widgets |

The path pipeline sets scissor to full viewport and relies on shader-based fragment discard for clipping. This works for actual vector paths but breaks batch-level clip isolation required for widget hover overlays.

### 2.3 Symptom

Hovering over one button caused ALL buttons to show hover state ("global hover flip"). Hovering over gaps between buttons or resizing windows triggered the effect.

---

## 3. Binding Rules

### Rule 1: RoundedRect Uses Rect Pipeline

**BINDING:** `PrimitiveData::RoundedRect` MUST be processed through the rect pipeline.

The rect pipeline provides per-batch GPU scissor clipping via:
```rust
render_pass.set_scissor_rect(sx, sy, sw, sh);
```

This is the ONLY mechanism that guarantees clip isolation between widgets.

### Rule 2: Path Pipeline Exclusion

**BINDING:** RoundedRect primitives are NOT permitted to use the path pipeline until the path pipeline implements identical per-batch GPU scissor semantics.

Current path pipeline characteristics that make it unsuitable:
- Full-viewport scissor rect
- Shader-based `discard` clipping
- No per-batch clip rect application

### Rule 3: Regression Guard

**BINDING:** The example `examples/rounded_rect_clip_guard.rs` serves as a permanent regression guard.

Acceptance criteria:
1. Hover on Button A: ONLY Button A shows hover state
2. Hover on Button B: ONLY Button B shows hover state
3. Hover on gap between buttons: NO button shows hover state
4. NO visual bleed between adjacent buttons
5. Clipping is strictly per-widget

Any change that causes this example to fail constitutes a regression.

---

## 4. Visual Expectation Contract

### Statement

> **Rounded rects are fill-only; small radii (e.g., 2px) may be visually imperceptible.**

This is a deliberate constraint, not a bug:

- Widget corners at `radius = 2` may appear nearly square
- This is acceptable behavior
- Visual rounding is a function of physical pixels, not logical units
- Anti-aliasing may soften corners slightly but sharp corners are permitted

### What This Means

- DO NOT rely on rounded rect AA to provide edge definition
- DO NOT expect visible rounding at small radii on high-DPI displays
- DO NOT attempt to "fix" imperceptible rounding by changing pipelines

### Future Enhancement (Separate Task)

If perceptible small-radius rounding is required, the solution is:

- Introduce exactly ONE explicit edge signal (border OR shadow OR inner highlight)
- Apply engine-wide, deterministically
- No artifacts, no pipeline side effects

This is out of scope for this SPEC and requires separate approval.

---

## 5. Implementation Checklist

- [x] `add_rounded_rect()` uses `PrimitiveData::RoundedRect` (rect pipeline)
- [x] No `Path::rounded_rect()` routing in batcher
- [x] Regression guard example created
- [x] Contract documented (this SPEC)

---

## 6. Amendment Process

This SPEC may only be amended when:

1. Path pipeline implements per-batch GPU scissor clipping
2. Regression guard passes with path pipeline routing
3. Explicit SPEC amendment is approved

Until then, this contract is BINDING.

---

## 7. References

- Commit `a0e0a1f`: Introduced the bug
- Commit `5c286b7`: Reverted the fix
- `src/renderer/batcher.rs`: Implementation location
- `examples/rounded_rect_clip_guard.rs`: Regression guard
