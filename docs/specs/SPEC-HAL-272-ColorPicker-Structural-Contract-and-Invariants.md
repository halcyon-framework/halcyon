# SPEC-HAL-272: **ColorPicker — Structural Contract and Invariants**

**Author:** JC
**Status:** Binding
**Layer:** Widget Primitives / Value Controls
**Scope:** Inline ColorPicker and composite ColorPickerField. OpacityPicker and palette systems are deferred.

**Depends on:**
- SPEC-HAL-002 (Constitution & Architectural Invariants) — §3.11 No Interpretive Intent Resolution; §4.11 Explicit Commit Authority
- SPEC-HAL-001 (Core Architecture)
- SPEC-HAL-207 (Application-Layer Value Binding & UI Boundary Law)
- SPEC-HAL-136 (Transactional Selection and Explicit Commit Semantics)
- SPEC-HAL-093A (Interactive Element States & Micro-Feedback)
- SPEC-HAL-096B (Geometry Tokenization and Theme Authority)
- SPEC-HAL-236 (Overlay Layer Composition and Hit-Test Authority)
- SPEC-HAL-265 (Iconography System and Vector Symbol Architecture)

---

# 1. Purpose

This SPEC defines the **full structural contract** for the ColorPicker widget in Halcyon.

It specifies:

- The canonical color model — `ColorValue`, color spaces, validation
- The widget state machine — preview state, committed state, sub-component interaction
- The interaction model — pointer drag, keyboard navigation, text input
- The commit model — what constitutes a color selection commit
- Inline and popup rendering modes — layer authority, overlay, dismissal
- The gradient rendering contract — SV square, hue strip, alpha strip
- Value binding — FormBinder integration and emission format
- Phase compliance
- All invariants in enumerated, enforceable form

---

# 2. Scope Boundaries

## 2.1 In Scope

- sRGB color selection via HSV manipulation (hue strip + SV square)
- Optional alpha channel (configured at construction; off by default)
- Hex string input field (`#RRGGBB` / `#RRGGBBAA`)
- Swatch preview (current preview vs last committed)
- Inline mode: full picker embedded in parent layout
- Popup mode: picker as an overlay anchored to a `ColorPickerField` trigger
- FormBinder value binding

## 2.2 Explicitly Out of Scope

- Lab, LCH, or wide-gamut color spaces
- Palette / swatch library
- Eyedropper / screen color sampling
- Gradient builder
- Color blindness simulation
- Named color lookup / CSS color names
- Non-sRGB output profiles

---

# 3. Model Layer

## 3.1 `ColorValue`

`ColorValue` is the canonical color representation. sRGB encoding. All channels in `[0.0, 1.0]`.

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorValue {
    pub r: f32,  // 0.0–1.0
    pub g: f32,  // 0.0–1.0
    pub b: f32,  // 0.0–1.0
    pub a: f32,  // 0.0–1.0; always 1.0 when alpha is disabled
}
```

### INV-272-CV-1: Channel Clamp

All channels are clamped to `[0.0, 1.0]` at construction. Out-of-range input is clamped silently. NaN is forbidden; the constructor substitutes 0.0 for NaN inputs.

### INV-272-CV-2: Value Type

`ColorValue` is a value type. No in-place mutation is exposed. All operations return new values.

### INV-272-CV-3: sRGB Encoding

`ColorValue` stores sRGB-encoded linear values. No gamma conversion is performed by the widget. The application layer is responsible for any color space conversion before use.

### INV-272-CV-4: Alpha Default

When alpha is disabled at construction, `a` is always `1.0`. The alpha strip is not rendered. Alpha input in hex strings is ignored.

## 3.2 HSV Manipulation Space

The picker manipulates color internally in HSV (Hue, Saturation, Value) space. RGB↔HSV conversion is performed at read/write boundaries only — never during drag.

```rust
struct HsvState {
    h: f32,  // 0.0–360.0 degrees
    s: f32,  // 0.0–1.0
    v: f32,  // 0.0–1.0
    a: f32,  // 0.0–1.0
}
```

### INV-272-HSV-1: Hue Wraparound

Hue is defined on `[0.0, 360.0)`. Values are wrapped, not clamped. `360.0` wraps to `0.0`.

### INV-272-HSV-2: Achromatic Hue Preservation

When saturation is 0.0 (achromatic), the hue value is preserved in `HsvState`. Hue is not discarded or reset to 0. This ensures that reducing saturation to zero and then increasing it again restores the original hue.

### INV-272-HSV-3: Conversion Boundary

RGB↔HSV conversion occurs only when: (a) committing a value, (b) reading `ColorValue` from state, or (c) parsing a hex string input. It never occurs during a pointer drag sequence.

## 3.3 `ColorPickerState`

Owns all mutable runtime state. Lives inside the widget. Not shared.

```rust
pub struct ColorPickerState {
    /// The committed selected color. None until first explicit commit.
    selected: Option<ColorValue>,
    /// The current working preview. Always defined after first interaction.
    preview: HsvState,
    /// Dirty flag: true when selected changed since last take_changed().
    dirty: bool,
    /// Whether alpha channel is enabled.
    alpha_enabled: bool,
    /// Active drag target, if any.
    drag_target: Option<DragTarget>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum DragTarget {
    SvSquare,
    HueStrip,
    AlphaStrip,
}
```

### INV-272-STATE-1: Preview/Selected Independence

`preview` and `selected` are independent. Dragging the SV square or hue strip updates `preview` only. `selected` does not change until an explicit commit. The dirty flag reflects changes to `selected` only.

### INV-272-STATE-2: Widget Ownership

`ColorPickerState` is owned exclusively by the widget struct. It is not shared, not wrapped in `Rc`, not accessible from outside the widget implementation.

### INV-272-STATE-3: Dirty Flag Semantics

`dirty` is set to `true` only when `commit()` changes the value of `selected`. Preview updates, drag events, and hex field edits do not set dirty. `take_changed()` clears dirty and returns the current `selected`. Subsequent calls in the same frame return `None`.

### INV-272-STATE-4: Initial Preview

`preview` is initialized from the `initial` color value passed at construction, converted to HSV. If `initial` is `None`, `preview` defaults to hue=0, s=0, v=1 (white).

---

# 4. Sub-Components and Layout

## 4.1 Fixed Layout Structure

The color picker has a fixed internal layout:

```
┌─────────────────────────────┬───┐
│                             │   │
│         SV Square           │ H │
│                             │ u │
│                             │ e │
├─────────────────────────────┤   │
│      Alpha Strip (opt.)     │   │
├─────────────────────────────┴───┤
│  Prev Swatch │ Cur Swatch │ Hex │
└─────────────────────────────────┘
```

- **SV Square**: 2D gradient; x = saturation (0→1), y = value (1→0). Background is the pure hue color. White overlay gradient left→right, black overlay gradient top→bottom.
- **Hue Strip**: Vertical strip to the right of the SV square. Full hue spectrum 0→360.
- **Alpha Strip**: Horizontal strip below the SV square. Rendered only when `alpha_enabled`. Checkerboard background + color overlay 0→1 alpha.
- **Swatches**: Two fixed-size squares. Left = last committed color (`selected`, or grey if None). Right = current preview.
- **Hex Field**: Single-line text input embedded in the picker. Accepts `#RGB`, `#RRGGBB`, `#RRGGBBAA`.

### INV-272-LAYOUT-1: Fixed Preferred Size

`ColorPickerWidget` reports a fixed preferred size from `layout()` for a given theme configuration. It does not stretch or fill. The internal sub-component rects are computed in `layout()` and cached on the widget.

### INV-272-LAYOUT-2: Geometry in Layout Only

All sub-component rects (SV square rect, hue strip rect, alpha strip rect, swatch rects, hex field rect) are computed in `layout()` and stored in cached fields. `paint()` reads cached rects. No geometry recomputation in `paint()`.

### INV-272-LAYOUT-3: Hue Strip Width

Hue strip width is a fixed geometry token from `cx.theme.geometry`. It does not scale with the overall widget width.

---

# 5. Gradient Rendering Contract

The SV square and hue/alpha strips require gradient rendering that cannot be achieved with the standard `push_vertical_gradient` or `push_linear_gradient` pipeline alone.

### INV-272-GRAD-1: SV Square Requires Shader

The SV square gradient is a 2D bilinear blend: horizontally from white to the pure hue color, vertically from the result to black. This requires either a dedicated quad shader with hue uniform, or compositing two gradient passes. The implementation MUST use the GPU gradient pipeline — no CPU-side pixel blit.

### INV-272-GRAD-2: Hue Strip Is Static Per Frame

The hue strip renders a full 0°→360° spectrum. The strip geometry is constant; only the selection indicator (circular thumb) moves. The strip may be rendered as a multi-stop vertical gradient using the existing `push_vertical_gradient` with a fixed set of hue stops: 0°, 60°, 120°, 180°, 240°, 300°, 360°.

### INV-272-GRAD-3: Alpha Strip Checkerboard

The alpha strip background uses a checkerboard pattern (two `push_rect` calls with alternating colors) rendered before the alpha gradient overlay. The overlay is a horizontal gradient from transparent (current color at a=0) to opaque (current color at a=1).

### INV-272-GRAD-4: SV Cursor and Hue Thumb Are Overlay Geometry

The SV square selection cursor (crosshair ring) and the hue strip thumb are rendered at `LAYER_OVERLAY + 1` (overlay geometry pass), after the gradient content. They are never occluded by gradient fills.

### INV-272-GRAD-5: No CPU Rasterization

The widget must not rasterize gradient data into CPU-side pixel buffers or textures. All gradient content uses the existing GPU gradient pipeline or a dedicated color picker shader.

---

# 6. Interaction Model

## 6.1 SV Square Interaction

- **Pointer down** in SV rect → `drag_target = Some(DragTarget::SvSquare)`; update `preview.s` and `preview.v` from pointer position; no commit
- **Pointer move** while `drag_target == SvSquare` → update `preview.s` and `preview.v`; no commit
- **Pointer up** → clear `drag_target`; no commit

### INV-272-INT-1: No Commit on Drag Release

Releasing the pointer over the SV square does not commit. The preview updates live during drag; the user must explicitly commit via Enter, Space, or the Apply button.

### INV-272-INT-2: Drag Clamping

Pointer coordinates outside the SV rect are clamped to the rect boundary. S and V values stay in `[0.0, 1.0]`.

## 6.2 Hue Strip Interaction

- **Pointer down** in hue strip rect → `drag_target = Some(DragTarget::HueStrip)`; update `preview.h`
- **Pointer move** while `drag_target == HueStrip` → update `preview.h`
- **Pointer up** → clear `drag_target`; no commit

Hue is derived from the y position in the strip: `h = (1.0 - clamp(y/strip_height)) * 360.0`.

## 6.3 Alpha Strip Interaction

Identical pattern to hue strip. Active only when `alpha_enabled`. Updates `preview.a` from x position.

## 6.4 Hex Field Interaction

The hex field is an embedded single-line text input.

- The field accepts free-form typing. The preview is not updated while typing.
- On **Enter** or **Tab-out**: the field content is parsed. If valid, `preview` is updated from the parsed value, and the value is immediately **committed** (dirty set). If invalid, the field reverts to the last valid hex string; no state change.
- On **Escape**: the field reverts to the hex representation of the current `preview` without committing.

### INV-272-HEX-1: Hex Parse Formats

Accepted formats: `#RGB` (expands to `#RRGGBB`), `#RRGGBB`, `#RRGGBBAA` (only when `alpha_enabled`). Leading `#` is optional. Case-insensitive.

### INV-272-HEX-2: Hex Field Commit Is Direct

Hex input is the one exception to the two-commit-source rule: valid hex entry via Enter is a direct commit. This is intentional — the hex field is a text input with explicit activation semantics, not a drag control.

### INV-272-HEX-3: Invalid Input Is a No-Op

Malformed hex strings (wrong length, non-hex characters) on Enter cause the field to revert. No error state is emitted. No partial preview update occurs.

## 6.5 Swatch Interaction

- **Click on previous swatch** (left / last committed): if `selected` is Some, sets `preview` to the committed color. Does not commit. This is a preview restore.
- **Click on current swatch** (right / preview): no-op. The swatch is display-only.

## 6.6 Keyboard Navigation

Keyboard focus applies only to the picker as a whole when not in hex field focus.

| Key | Action |
|---|---|
| Arrow Left / Right | Adjust `preview.s` by ±0.01 (fine saturation step) |
| Arrow Up / Down | Adjust `preview.v` by ±0.01 (fine value step) |
| `[` / `]` | Adjust `preview.h` by ±1° |
| `{` / `}` | Adjust `preview.h` by ±10° |
| Enter / Space | Commit current `preview` as `selected` |
| Escape | Close popup (popup mode); release focus (inline mode). No commit. |
| Tab | Move focus to hex field |

### INV-272-KBD-1: Arrow Keys Are Scoped

Arrow key events are consumed by the picker while it holds focus. They do not propagate to parent focus handlers.

### INV-272-KBD-2: No Implicit Commit

Arrow key navigation, bracket key hue adjustment, swatch click, and drag release are never commits. Per SPEC-HAL-002 §4.11.

---

# 7. Commit Model

Governed by SPEC-HAL-002 §4.11 (Explicit Commit Authority) and SPEC-HAL-136 (Transactional Selection).

### INV-272-COMMIT-1: Commit Sources

Exactly three events constitute a color commit:

1. **Keyboard activation** — Enter or Space while the picker has focus and is not in hex field focus
2. **Hex field confirmation** — Enter in the hex field with a valid color string
3. **Apply button activation** — pointer click on an explicit Apply / OK button (when present; optional construction flag)

No other event constitutes a commit.

### INV-272-COMMIT-2: No Implicit Commit

The following must never constitute a commit:
- Pointer drag in SV square, hue strip, or alpha strip
- Drag release
- Pointer click on previous swatch
- Focus loss or blur
- Popup dismissal via outside-click or Escape
- Programmatic preview update

### INV-272-COMMIT-3: Commit Idempotency

If the user commits the same color already in `selected` (within float epsilon), `dirty` is not set.

### INV-272-COMMIT-4: Commit Is Atomic

`commit()` atomically updates `selected` and sets `dirty`. No intermediate state where `dirty` is true but `selected` has not been updated.

---

# 8. Rendering

## 8.1 Inline Mode

`ColorPickerWidget` is a standard retained-mode widget placed in the widget tree by the parent screen.

### INV-272-INLINE-1: Fixed Preferred Size

Reports a fixed preferred size from `layout()`. Does not fill or stretch.

### INV-272-INLINE-2: No Layout Mutation

The picker is a leaf-sized widget. It does not shift surrounding layout.

## 8.2 Popup Mode — ColorPickerField

Popup mode is implemented by `ColorPickerField`, a composite widget that owns an embedded `ColorPickerWidget` and manages a trigger surface + anchored popup overlay. The pattern is identical to `DatePickerField`.

The trigger surface is a fixed-height control displaying a color swatch of the current `selected` color (or a placeholder swatch when `selected` is None) plus an affordance indicator.

### INV-272-POPUP-1: SPEC-236 Overlay Mechanism

`ColorPickerField` registers two `OverlayHitRegion` entries in `paint_overlay`:
1. Full-window backdrop (registered first, hit-tested last) — outside-click dismissal.
2. Popup picker rect (registered second, hit-tested first) — routes events to the field.

### INV-272-POPUP-2: Placement

Default placement is below the trigger. Auto-placement flips to above if insufficient space below. `PopupPlacement` enum, identical contract to `DatePickerField`.

### INV-272-POPUP-3: Dismissal Without Commit

Outside-click and Escape close the popup without modifying `selected` or setting `dirty`. Per INV-272-COMMIT-2.

### INV-272-POPUP-4: Focus and Capture

`ColorPickerField` calls `cx.request_keyboard_capture()` when the popup opens. `on_capture_leave` closes the popup.

### INV-272-POPUP-5: Embedded Mode

The owned `ColorPickerWidget` is created with `.with_embedded(true)`. In embedded mode the outer background gradient is suppressed; the field provides the background via a blocker.

### INV-272-POPUP-6: Trigger Swatch Paint

The trigger swatch rect uses a checkerboard underlay when `alpha_enabled`, so semi-transparent colors render correctly.

### INV-272-POPUP-7: `i` Key Opens Popup

Per Halcyon global convention, `Event::Text("i")` with no modifiers opens the popup from the trigger, consistent with DatePickerField.

## 8.3 Paint Pipeline Compliance

### INV-272-PAINT-1: Paint Purity

`paint()` and `paint_overlay()` are pure. No `ColorPickerState` mutation occurs in these methods.

### INV-272-PAINT-2: Theme Compliance

No hardcoded color literals. UI chrome colors (borders, focus rings, swatch border, thumb colors) sourced from `cx.theme.colors.*` semantic tokens.

**Exception**: The gradient fill colors (hue spectrum, SV gradients) are mathematically derived from the current `HsvState`. They are not theme tokens. This is correct and intentional.

### INV-272-PAINT-3: Geometry Computed in Layout, Read in Paint

All sub-component rects are computed in `layout()` and cached. `paint()` reads cached rects only.

### INV-272-PAINT-4: Cursor and Thumb Rendering

| Element | Description |
|---|---|
| SV cursor | Ring with 2px white inner + 1px black outer, centered on current (s,v) position in the SV rect |
| Hue thumb | Horizontal line or rounded rect spanning the strip width, centered on current hue y position |
| Alpha thumb | Vertical line or rounded rect spanning the strip height, centered on current alpha x position |

All cursor/thumb elements render at `LAYER_OVERLAY + 1`.

## 8.4 Visual State Tokens

| Element | Token |
|---|---|
| Picker background | `surfaces.level2` |
| Swatch border | `borders.default` |
| Focus ring (SV square, strips) | `borders.focus` |
| Hex field | Standard TextInput visual contract |
| Previous swatch (hover) | `list.row_hover` tint overlay |
| Checker pattern dark | `#808080` (fixed) |
| Checker pattern light | `#C0C0C0` (fixed) |

Checker colors are fixed and are not theme tokens. They serve as a neutral background for alpha visualization.

---

# 9. Phase Compliance

| Phase | ColorPicker behavior |
|---|---|
| Phase 2 — Deferred Mutations | FormBinder value transfer applied |
| Phase 3 — Rebuild | Widget rebuilt from state snapshot |
| Phase 4 — Layout | Sub-component rects computed, cached. Preferred size returned. |
| Phase 5 — Scene Build | Picker painted from cached rects; no state mutation |
| Phase 6 — GPU Render | No tree access |

### INV-272-PHASE-1: No Mutation in Layout or Paint

All state-mutating operations (`update_sv`, `update_hue`, `commit`, etc.) are called only from `handle_event()`. Never from `layout()` or `paint()`.

### INV-272-PHASE-2: Popup Open/Close Is Immediate

Popup open/close is immediate widget state in `handle_event`. It is not a deferred Phase-2 mutation.

---

# 10. Value Binding Contract

## 10.1 FormBinder Integration

`ColorPickerWidget` emits its committed value via a `ChangeWriter` text binding.

Emission format:
- Alpha disabled: `"#RRGGBB"` (6 hex digits, uppercase)
- Alpha enabled: `"#RRGGBBAA"` (8 hex digits, uppercase)

A cleared selection (`selected = None`) is not emitted. The field only emits when `dirty` is true.

```rust
fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
    self.form.bind(cx, |b| {
        b.text("stroke_color").sync(&mut self.model.color_str);
    });
    // Caller parses model.color_str: "#RRGGBB" or "#RRGGBBAA"
}
```

### INV-272-BIND-1: Emit Exactly Once Per Commit

Emitted exactly once per selection commit. Not emitted on preview updates, drag events, or popup state changes.

### INV-272-BIND-2: Unidirectional Binding

FormBinder binding is widget → application. Application initializes `selected` via `ColorPickerState::new(Some(color), ...)` at construction. It does not push values back via FormBinder.

### INV-272-BIND-3: FormBinder Not Required

A calling screen may poll `state.take_changed()` directly in `flush_pending_actions`. Both paths are legal.

---

# 11. Invariant Index

| ID | Statement |
|---|---|
| INV-272-CV-1 | ColorValue channels clamped to [0,1]; NaN replaced with 0 |
| INV-272-CV-2 | ColorValue is a value type; no in-place mutation |
| INV-272-CV-3 | ColorValue stores sRGB-encoded values |
| INV-272-CV-4 | Alpha is 1.0 when alpha mode disabled |
| INV-272-HSV-1 | Hue wraps on [0, 360); never clamped |
| INV-272-HSV-2 | Achromatic hue preserved in HsvState |
| INV-272-HSV-3 | RGB↔HSV conversion only at commit/read/parse boundaries |
| INV-272-STATE-1 | Preview and selected are independent state |
| INV-272-STATE-2 | ColorPickerState is widget-owned exclusively |
| INV-272-STATE-3 | Dirty flag set only by commit; cleared only by take_changed() |
| INV-272-STATE-4 | Preview initialized from initial color at construction |
| INV-272-LAYOUT-1 | ColorPickerWidget has a fixed preferred size |
| INV-272-LAYOUT-2 | Sub-component rects computed in layout(), read in paint() |
| INV-272-LAYOUT-3 | Hue strip width is a fixed geometry token |
| INV-272-GRAD-1 | SV square gradient uses GPU shader; no CPU rasterization |
| INV-272-GRAD-2 | Hue strip is a multi-stop vertical gradient; static per frame |
| INV-272-GRAD-3 | Alpha strip uses checkerboard underlay + gradient overlay |
| INV-272-GRAD-4 | SV cursor and hue thumb render at LAYER_OVERLAY + 1 |
| INV-272-GRAD-5 | No CPU-side pixel buffer or texture rasterization |
| INV-272-INT-1 | Drag release over SV square does not commit |
| INV-272-INT-2 | Pointer coordinates clamped to sub-component rect |
| INV-272-HEX-1 | Accepted hex formats: #RGB, #RRGGBB, #RRGGBBAA |
| INV-272-HEX-2 | Valid hex + Enter is a direct commit |
| INV-272-HEX-3 | Invalid hex on Enter reverts field; no state change |
| INV-272-KBD-1 | Arrow keys scoped to picker; do not propagate |
| INV-272-KBD-2 | Arrow/bracket navigation is never a commit |
| INV-272-COMMIT-1 | Exactly three explicit commit sources |
| INV-272-COMMIT-2 | Drag, release, swatch click, blur, dismissal are not commits |
| INV-272-COMMIT-3 | Committing same color does not set dirty |
| INV-272-COMMIT-4 | Commit is atomic: selected and dirty updated together |
| INV-272-INLINE-1 | Fixed preferred size; no stretch |
| INV-272-INLINE-2 | Inline picker does not shift surrounding layout |
| INV-272-POPUP-1 | SPEC-236 OverlayHitRegion mechanism (not SPEC-199) |
| INV-272-POPUP-2 | PopupPlacement auto-flips on screen overflow |
| INV-272-POPUP-3 | Outside-click and Escape dismiss without commit |
| INV-272-POPUP-4 | ColorPickerField calls request_keyboard_capture on open |
| INV-272-POPUP-5 | Embedded mode suppresses outer background gradient |
| INV-272-POPUP-6 | Trigger swatch uses checkerboard underlay when alpha enabled |
| INV-272-POPUP-7 | `i` key opens popup per Halcyon global convention |
| INV-272-PAINT-1 | paint() and paint_overlay() are pure |
| INV-272-PAINT-2 | UI chrome colors from theme tokens; gradient colors mathematical |
| INV-272-PAINT-3 | Sub-component rects computed in layout(), not recomputed in paint() |
| INV-272-PAINT-4 | SV cursor and hue/alpha thumbs rendered at LAYER_OVERLAY + 1 |
| INV-272-PHASE-1 | No state mutation in layout() or paint() |
| INV-272-PHASE-2 | Popup open/close is immediate widget state, not Phase-2 deferred |
| INV-272-BIND-1 | Value emitted exactly once per commit |
| INV-272-BIND-2 | Binding is widget → application only |
| INV-272-BIND-3 | FormBinder is canonical but not required |

---

# 12. Explicit Prohibitions

The following are permanently forbidden.

- Widget-side color normalization or nearest-valid substitution on invalid hex input — interaction is a no-op (SPEC-HAL-002 §3.11)
- Implicit commit on drag release, blur, focus loss, or popup dismissal (SPEC-HAL-002 §4.11)
- CPU-side gradient rasterization into pixel buffers or textures
- Direct write to `AppState` from widget event handlers (SPEC-HAL-002 §3.7)
- Hardcoded UI chrome color values in `paint()` (INV-272-PAINT-2)
- Geometry recomputation in `paint()` (INV-272-PAINT-3)
- Sharing `ColorPickerState` across widget instances (INV-272-STATE-2)
- Popup mode using nav_open_modal or navigation stack (INV-272-POPUP-1)

---

# 13. Files

| File | Role |
|---|---|
| `src/model/color_value.rs` | `ColorValue` — value type, channel clamp, hex parsing/formatting |
| `src/widgets/color_picker.rs` | `ColorPickerWidget` (inline + embedded) + `ColorPickerState` + `HsvState` |
| `src/widgets/color_picker_field.rs` | `ColorPickerField` — trigger swatch + anchored popup overlay |
| `src/widgets/mod.rs` | `pub mod color_picker`, `pub mod color_picker_field` |
| `examples/color_picker_demo.rs` | Reference demo — inline and popup (field) modes |

---

# 14. Deferred

- **OpacityPicker**: standalone alpha-only control — separate SPEC
- **Palette / swatch library**: selectable named or saved colors — separate SPEC
- **Eyedropper**: screen pixel sampling — separate SPEC; requires OS capability gating (SPEC-HAL-139)
- **Gradient builder**: multi-stop color editors — separate SPEC
- **ColorSpace**: Lab, LCH, wide-gamut — separate SPEC when engine output profiles are defined
