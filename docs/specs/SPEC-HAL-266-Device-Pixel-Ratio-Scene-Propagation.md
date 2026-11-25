# SPEC-HAL-266: **Device Pixel Ratio Propagation into Scene Build Phase**

### *Completing DPR Availability for Pixel-Accurate Scene Construction*

**Author:** JC
**Status:** Active
**Amended by:** SPEC-HAL-266A (Adopted / Binding)
**Scoped App-density amendment:** Accepted / Binding, 2026-10-16, under JC's
mandate recording technical acceptance of HAL-024 Layer E (`ec307c53`). Only the
App-owned effective-ratio semantics below are added; implementation proof is
pending. No unrelated Draft or proposed 363/364 authority is adopted.
**Depends on:**

* **SPEC-HAL-002** (Constitution & Architectural Invariants)
* **SPEC-HAL-001** (Core Architecture — Phase Model)
* **SPEC-HAL-024** (Coordinate Model)
* **SPEC-HAL-104** (Pixel-Stable Text Metrics)
* **SPEC-HAL-265A** (Icon Visual Rendering Contract §7.2)

---

# 1. Problem Statement

The device pixel ratio (DPR / scale factor) is already present throughout Halcyon:

| Layer | Location | Field |
|-------|----------|-------|
| Platform | `Window` | `scale_factor: f32` |
| Input | `InputFusion` | `scale_factor: f32` |
| Renderer | `MultiTargets` (per-surface) | `scale_factor: Cell<f32>` |
| Renderer | `RenderPassDescriptor` | `scale_factor: f32` |
| Text | `TextQuadBatch` | `scale_factor: f32` |
| Text | `GlyphQuadParams` | `scale_factor: f32` |

However, DPR is **absent** from the scene build phase:

| Layer | Struct | DPR Available? |
|-------|--------|----------------|
| Scene Build | `SceneBuilder` | **NO** |
| Paint Trait | `PaintSink` | **NO** |

This means widgets painting during Phase 5 (Scene Build) cannot perform DPR-aware operations:

* Icon position snapping (SPEC-HAL-265A §7.2)
* Sub-pixel–aware border alignment
* DPR-correct rounded corner radius quantization
* Any future pixel-grid–aligned painting

The DPR is available **before** scene build (from Window/renderer) and **after** scene build (in the renderer). The gap is exactly one phase.

---

# 2. Design

### 2.1 Origin

DPI input `D` originates from the OS via `winit::Window::scale_factor()`, captured
by `platform::Window` at creation and `ScaleFactorChanged` intake. It remains raw
platform metadata, separate from App-local typed UI Scale `S` (default `1.0`).

Per SPEC-HAL-002 Pillar F, the OS value is advisory. HAL-024 Layer E's App owner
admits `S` and composes effective ratio `E = D × S` exactly once. The engine-owned
target density stored in `MultiTargets` is `E`. Existing `DPR`/`scale_factor`
spellings in this propagation/paint contract denote that physical/logical ratio,
not permission to read preferences or multiply by `S` again. Explicit non-App
targets retain their supplied admitted ratio and need no OS query.

### 2.2 Propagation Path

```
Window::scale_factor D + admitted App UiScale S
    ↓ (event intake, Phase 1)
App boundary admission (Phase 2), compose E once
    ↓
MultiTargets::set_scale_factor E (renderer per-surface storage)
    ↓ (read at scene build start, Phase 5)
SceneBuilder::scale_factor (immutable for duration of scene build)
    ↓ (exposed to widgets via PaintSink)
PaintSink::scale_factor() -> f32 (read-only accessor)
```

### 2.3 SceneBuilder Changes

Add `scale_factor: f32` to `SceneBuilder`:

```rust
pub struct SceneBuilder {
    // ... existing fields ...
    /// Device pixel ratio for the target surface.
    /// Immutable during scene build. Set at construction, read by widgets via PaintSink.
    scale_factor: f32,
}
```

Modify `SceneBuilder::new()` to accept `scale_factor: f32`.

### 2.4 PaintSink Changes

Add a read-only accessor to the `PaintSink` trait:

```rust
pub trait PaintSink {
    /// Device pixel ratio for the current render target.
    /// Widgets use this for DPR-aware snapping (icons, borders, etc.).
    fn scale_factor(&self) -> f32;

    // ... existing methods ...
}
```

### 2.5 Construction Site

`SceneBuilder` is constructed in the frame loop before widget painting begins. The call site must pass the current surface's DPR:

```rust
let sf = renderer.scale_factor(surface_id);
let mut builder = SceneBuilder::with_scale_factor(sf);
```

If `renderer.scale_factor()` returns `None` (no surface registered), default to `1.0`.

---

# 3. Consumers

### 3.1 Icon Position Snapping (SPEC-HAL-265A §7.2)

In `SceneBuilder::push_icon`:

```rust
fn push_icon(&mut self, def: &IconDefinition, x: f32, y: f32, size: f32, color: Color, z_index: ZIndex) {
    let sf = self.scale_factor;
    let snapped_x = (x * sf).round() / sf;
    let snapped_y = (y * sf).round() / sf;
    let scale = size / def.viewbox_width;
    // ... layer iteration using snapped_x, snapped_y ...
}
```

### 3.2 Future Consumers

* Border alignment snapping
* Rounded corner radius quantization
* Divider/separator pixel alignment
* Any widget requiring physical-pixel-grid awareness during paint

These are not part of this SPEC's scope. This SPEC only establishes the propagation path. Each consumer defines its own snapping policy.

---

# 4. Invariants

**INV-DPR-001: Immutability During Scene Build**

`SceneBuilder::scale_factor` MUST NOT be mutated after construction. It is set once when the builder is created and remains constant for the entire Phase 5 (Scene Build) pass.

*Rationale:* SPEC-HAL-002 Pillar B (Phase Separation). No mid-phase mutation.

**INV-DPR-002: Positive Finite Value**

`scale_factor` MUST be positive (`> 0.0`) and finite (`is_finite() == true`). Constructor panics in debug builds if violated.

*Rationale:* Division by zero in snapping formulas. Consistent with existing `INV-SCISSOR-001` and `INV-CM-004` assertions in the renderer.

**INV-DPR-003: Per-Surface Authority**

Each surface has its own effective ratio. App-owned targets combine that App's
admitted `S` with their own `D`; independent Apps may differ. SceneBuilder's
`scale_factor` reflects the target surface for the current paint pass. Widgets
MUST NOT assume a global density or access App/platform scale from layout.

*Rationale:* SPEC-HAL-082 (Multi-Window Architecture). Monitors may have different DPRs.

**INV-DPR-004: No Semantic Authority**

DPR on PaintSink is a **read-only projection** of renderer state into the scene build phase. It does not grant widgets authority over scaling, resolution, or presentation policy.

*Rationale:* SPEC-HAL-002 Pillar D (Pure Data In, Explicit Actions Out). Widgets read DPR; they do not set it.

**INV-DPR-005: Test Sink Default**

Test implementations of `PaintSink` (e.g., `VoidPaintSink`, `IconTrackingSink`) MUST return `1.0` from `scale_factor()` unless explicitly configured otherwise.

*Rationale:* Test determinism. Existing tests assume 1x DPI.

---

# 5. Prohibitions

**FORBID-DPR-001: No DPR Mutation During Paint**

Widgets MUST NOT mutate DPR through any mechanism during Phase 5. `PaintSink::scale_factor()` is a getter only. No setter exists on the trait.

**FORBID-DPR-002: No Direct OS Read During Paint**

Widgets MUST NOT read DPR from the OS, Window, or renderer directly. The only legal source during paint is `PaintSink::scale_factor()`.

*Rationale:* SPEC-HAL-002 §3.10 (No Hidden Global State). Single source of truth within each phase.

**FORBID-DPR-003: No DPR in Layout Phase**

> **Normative parent clause:** SPEC-HAL-266 §§2, 4, 5, and 8 are read subject to
> SPEC-HAL-266A for an admitted custom-render participant. SPEC-HAL-266A owns the
> sole mutable post-layout consumer of full target metrics before paint.
> FORBID-DPR-003 remains binding, and the existing PaintSink DPR projection
> remains read-only paint-time snapping authority, not custom GPU
> resource-sizing authority.

This SPEC does NOT propagate DPR into the layout phase (Phase 4). Layout operates in logical coordinates. If DPR is ever needed in layout, that requires a separate SPEC with its own invariants.

*Rationale:* Scope containment. Layout purity (SPEC-HAL-002 §4.7).

---

# 6. Relationship to Existing Systems

| SPEC | Relationship |
|-----|-------------|
| SPEC-HAL-002 | Governing constitution. DPR propagation obeys phase separation, no mid-phase mutation, read-only in paint. |
| SPEC-HAL-024 | Coordinate model. DPR converts between logical and physical coordinate spaces. This SPEC does not change the coordinate model — it makes an existing value accessible. |
| SPEC-HAL-104 | Text pixel stability. Text already receives DPR via `GlyphQuadParams`. This SPEC makes the same value available to non-text painting. |
| SPEC-HAL-265A §7.2 | Icon position snapping. Primary immediate consumer. |
| SPEC-HAL-082 | Multi-window. Per-surface DPR (INV-DPR-003). |
| SPEC-HAL-030 | Presentation policy. DPR is presentation metadata, not semantic authority. |

---

# 7. Migration

### 7.1 SceneBuilder Construction

All `SceneBuilder::new()` call sites become `SceneBuilder::with_scale_factor(sf)`. Existing `new()` can remain as `with_scale_factor(1.0)` for backward compatibility and test convenience.

### 7.2 PaintSink Implementations

Three implementations require update:

1. **`SceneBuilder`** — returns `self.scale_factor`
2. **`VoidPaintSink`** (test) — returns `1.0`
3. **`IconTrackingSink`** (test) — returns `1.0`

### 7.3 Frame Loop Wiring

The frame loop (`main.rs`, `app.rs`) must pass `renderer.scale_factor(surface_id)` when constructing SceneBuilder for each paint pass.

---

# 8. Implementation Scope

This SPEC is deliberately narrow:

* Add one field to `SceneBuilder`
* Add one method to `PaintSink`
* Wire one value from renderer → builder at construction
* Apply snapping in `push_icon`
* Update test sinks

It does NOT:

* Change the coordinate model
* Modify layout
* Alter text rendering (already has DPR)
* Introduce new global state
* Require constitutional amendment
