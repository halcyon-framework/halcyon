# SPEC-HAL-325: Fresnel Button — Dynamic Edge Glow

### *GPU-Computed Mouse-Aware Button Effect*

**Author:** JC
**Status:** Draft
**Depends on:** SPEC-HAL-011 (Button), SPEC-HAL-021 (Generic Animation Timeline), SPEC-HAL-093 (Pixel Stability)

---

## 1. Problem Statement

The current button uses a static 4-stop vertical gradient
for a glossy Aero-era look. The gradient doesn't respond
to the mouse — hover changes the gradient stops but not
the spatial distribution. The effect is flat and dated.

Modern high-end UIs (Apple's visionOS, macOS Sonoma glass
buttons, game UI) use dynamic Fresnel edge glow that
responds to cursor position, creating a sense of physical
materiality — buttons that feel like polished glass tiles
you'd want to lick.

## 2. Design Vision

### 2.1 Fresnel Edge Glow

In optics, Fresnel reflectance increases at grazing angles.
For a button, this means: **edges glow brighter than the
center**, and the glow is strongest on the edge nearest
the mouse cursor.

The effect is computed per-pixel in the shader:
```
edge_factor = smoothstep(inner_radius, outer_radius, sdf_distance)
fresnel = pow(edge_factor, fresnel_power)
glow_direction = normalize(pixel_pos - mouse_pos)
directional_fresnel = fresnel * (0.5 + 0.5 * dot(glow_direction, edge_normal))
```

### 2.2 Three Animation States

**Idle → Hover (entry)**:
- `hover_t` animates 0→1 over ~150ms (ease-out)
- Edge glow appears, strongest at the entry edge
- Entry direction captured: mouse enters from left → left
  edge lights first, glow spreads

**Hover (sustained)**:
- Mouse position tracked continuously
- Fresnel glow follows the cursor — nearest edge is brightest
- Subtle, not distracting. The glow is a whisper, not a shout.

**Hover → MouseDown (press)**:
- `press_t` animates 0→1 over ~80ms (fast)
- Button "sinks": rim glow intensifies (the edge pops as
  the surface compresses toward the viewer)
- Slight darkening of the center (the glass absorbs light
  as it depresses)
- Optional: 0.5-1px visual inset (translate y+1)

**MouseDown → MouseUp (release)**:
- `press_t` animates 1→0 over ~200ms with spring dynamics
- The rim glow "bounces" — brief overshoot in brightness
  before settling to hover state
- The button feels physical — it springs back

**Hover → Exit**:
- `hover_t` animates 1→0 over ~200ms
- Glow fades from the exit edge inward

## 3. Shader Design

### 3.1 Pipeline: ButtonFresnel

New pipeline (`PipelineId::ButtonFresnel = 12`). Does NOT
extend the Gradient pipeline — the per-instance data model
is different.

### 3.2 Per-Instance Data

```rust
struct ButtonFresnelInstance {
    /// Button rectangle (x, y, w, h) in physical pixels
    rect: [f32; 4],
    /// Background color (base, before Fresnel)
    color: [f32; 4],
    /// Mouse position relative to button center, normalized
    /// to [-1, 1] range. (0,0) = center, (-1, -1) = top-left.
    mouse_local: [f32; 2],
    /// Hover animation progress [0.0, 1.0]
    hover_t: f32,
    /// Press animation progress [0.0, 1.0]
    press_t: f32,
    /// Corner radius in physical pixels
    corner_radius: f32,
    /// Fresnel power exponent (higher = sharper edge falloff)
    fresnel_power: f32,
    /// Glow color (typically white or light accent)
    glow_color: [f32; 4],
    /// Glow intensity multiplier
    glow_intensity: f32,
}
```

### 3.3 WGSL Shader (sketch)

```wgsl
// SDF for rounded rectangle
fn sdf_rounded_rect(p: vec2<f32>, half_size: vec2<f32>, radius: f32) -> f32 {
    let q = abs(p) - half_size + radius;
    return length(max(q, vec2(0.0))) + min(max(q.x, q.y), 0.0) - radius;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let half = vec2(inst.rect.z, inst.rect.w) * 0.5;
    let center = vec2(inst.rect.x, inst.rect.y) + half;
    let p = in.position.xy - center;

    // SDF distance (negative = inside)
    let d = sdf_rounded_rect(p, half, inst.corner_radius);

    // Anti-aliased mask
    let aa = fwidth(d);
    let mask = 1.0 - smoothstep(-aa, aa, d);

    // Edge factor: 0 at center, 1 at edge
    let inner_margin = min(half.x, half.y) * 0.3;
    let edge = smoothstep(-inner_margin, 0.0, d);

    // Fresnel: pow(edge, exponent)
    let fresnel = pow(edge, inst.fresnel_power);

    // Directional bias: stronger glow toward mouse
    let mouse_dir = normalize(inst.mouse_local);
    let pixel_dir = normalize(p / half);
    let dir_bias = 0.4 + 0.6 * max(dot(pixel_dir, mouse_dir), 0.0);

    // Combine: Fresnel * direction * hover_t
    let glow = fresnel * dir_bias * inst.hover_t * inst.glow_intensity;

    // Press effect: darken center, intensify rim
    let press_darken = mix(1.0, 0.92, inst.press_t);
    let press_rim = mix(1.0, 1.4, inst.press_t);

    // Final color
    let base = inst.color * press_darken;
    let rim_glow = inst.glow_color * glow * press_rim;
    let final_color = base + rim_glow;

    return vec4(final_color.rgb, mask * final_color.a);
}
```

### 3.4 Batching

All ButtonFresnel instances batch into ONE draw call.
Per-instance data (mouse_local, hover_t, press_t) varies
per button but is stored in the instance buffer — same
pattern as RadialGauge instances.

50 buttons = 1 draw call, 50 instances.

## 4. Widget Changes

### 4.1 New Fields on Button

```rust
/// Mouse position in button-local coordinates, normalized [-1, 1].
mouse_local: Cell<(f32, f32)>,
/// Direction of hover entry (radians).
entry_dir: Cell<f32>,
```

### 4.2 Event Handling

**PointerMove**: update `mouse_local` on every move while
hovered. Compute normalized position:
```rust
let nx = ((px - rect.x) / rect.width) * 2.0 - 1.0;
let ny = ((py - rect.y) / rect.height) * 2.0 - 1.0;
self.mouse_local.set((nx, ny));
```

**PointerEnter**: compute entry direction from the entry
point position relative to button center:
```rust
let dx = px - (rect.x + rect.width / 2.0);
let dy = py - (rect.y + rect.height / 2.0);
self.entry_dir.set(dy.atan2(dx));
```

### 4.3 Paint

Replace the `push_gradient_rect` + 1px sculpted borders
with a single `push_fresnel_button`:
```rust
cx.scene.push_fresnel_button(
    snapped_rect,
    bg_color,
    self.mouse_local.get(),
    self.hover_t.get(),
    self.pressed_t.get(),
    radius,
    fresnel_power,
    glow_color,
    glow_intensity,
);
```

## 5. Scene Primitive

```rust
pub fn push_fresnel_button(
    &mut self,
    rect: RectF,
    color: Color,
    mouse_local: (f32, f32),
    hover_t: f32,
    press_t: f32,
    corner_radius: f32,
    fresnel_power: f32,
    glow_color: Color,
    glow_intensity: f32,
)
```

New `SceneNode::FresnelButton(FresnelButtonNode { ... })`.

## 6. Theme Integration

```rust
pub struct ButtonGeometry {
    pub radius: f32,          // existing: corner radius
    pub fresnel_power: f32,   // NEW: edge falloff exponent (default: 3.0)
    pub glow_intensity: f32,  // NEW: glow strength (default: 0.3)
    pub glow_color: Color,    // NEW: glow tint (default: Color::WHITE)
}
```

## 7. Invariants

**INV-HAL-325-1**: Fresnel glow is PURELY VISUAL — it does
not affect layout, hit-testing, or any semantic state.

**INV-HAL-325-2**: All animation parameters (hover_t,
press_t, mouse_local) are Cell<> interior-mutable values
consumed during paint only. No mutation during layout.

**INV-HAL-325-3**: ButtonFresnel instances batch into a
single draw call via per-instance data. No per-button
draw call overhead.

**INV-HAL-325-4**: When hover_t = 0 (no hover), the shader
degrades to a flat rounded rect with no glow — visually
equivalent to the idle state. No wasted GPU computation
for non-hovered buttons.

## 8. Forbidden Behaviors

**FORBID-HAL-325-1**: Fresnel glow must not change button
dimensions, padding, or hit area.

**FORBID-HAL-325-2**: Mouse tracking must not fire during
layout or event routing phases — only during event handling
(PointerMove).

**FORBID-HAL-325-3**: Glow must not exceed the button's
rounded rect boundary — no visual bleed outside the corner
radius.

## 9. Performance

- **Shader cost**: one `pow()` + one `dot()` per pixel per
  button. Negligible on any GPU from the last decade.
- **Batching**: per-instance buffer, same pattern as gauges.
  Zero additional draw calls.
- **CPU cost**: one `atan2` on hover entry, one normalize +
  2 divisions on every PointerMove. Sub-microsecond.

## 10. Migration Path

Phase 1: Implement ButtonFresnel pipeline + shader. Button
uses it with default theme values (fresnel_power=3,
glow_intensity=0.3, glow_color=WHITE).

Phase 2: Remove the old 4-stop gradient code and 1px sculpted
borders. The Fresnel shader handles all visual states.

Phase 3: Extend to other interactive controls (SegmentedControl
segments, Tab bar items, Toggle switches) where the same
glass-tile effect would look good.
