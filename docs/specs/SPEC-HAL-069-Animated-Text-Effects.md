# **SPEC-HAL-069 — Animated Text Effects (Shadows, Outlines, Glows, Pulses)**

**SPEC-HAL-069**
**Animated Text Effects**
**Author:** Halcyon Architect
**Status:** Design – Not Implemented
**Version:** 1.0

**Depends On:**

* SPEC-HAL-024 Absolute Logical Space
* SPEC-HAL-048 Text Layout Engine
* SPEC-HAL-066 Unified Glyph Raster Pipeline
* SPEC-HAL-067 Subpixel & SDF Text Rendering v2
* SPEC-HAL-068 Text Compositing Effects
* SPEC-HAL-104 Pixel-Stable Text Metrics & Glyph Quad Normalization

---

## Implementation Status

**Not implemented**

Animated text effects are not implemented.

---

## **SPEC-HAL-104 Stability Invariant**

All animated text effects defined in this SPEC operate on top of SPEC-HAL-104-compliant glyph quads. Base text geometry MUST already satisfy SPEC-HAL-104's pixel stability rules before any effects are applied.

### Invariant Requirements

1. Base glyph quad positions (prior to applying FX) are **integer physical pixels** as per SPEC-HAL-104 Rule 5.
2. Effect systems are allowed to apply **fractional offsets** to the rendered vertices for visual animation.
3. These fractional offsets are considered purely visual and MUST NOT feed back into any cached metrics, layout, or hit-testing.

### No Feedback Loop Rule

Animated offsets MUST NOT be used as input to any metric, layout, or baseline computation. Any sampling of text geometry for layout or measurement MUST use the unmodified SPEC-HAL-104 base positions.

This ensures that animated effects remain purely cosmetic and do not compromise the stability guarantees established by SPEC-HAL-104.

### Phase Isolation Rule

All animated text effects operate strictly in Phase 5 (Scene Build) and Phase 6 (Renderer). They MUST NOT mutate WidgetState, AppState, text buffers, or layout metrics during any frame phase.

---

**Implements:**
Time-based and state-based animated text effects, fully GPU-driven:

1. Pulsating glow
2. Breathing outline
3. Animated shadow offset
4. Gradient color animation
5. Transition curves
6. Event-driven effect triggers
7. Deterministic per-frame evaluation

This SPEC introduces **no new glyph rasterization** — all animation happens at the GPU compositor layer.

---

## **1. Motivation**

Static effects (SPEC-068) are useful but limited.

UI needs animated effects for:

* hover highlights
* focus rings
* pulsing notifications
* selection glows
* attention indicators
* warning states
* interactive menus
* chat cursors / text indicators

All animations must:

* be **smooth**
* be **deterministic**
* run **purely in GPU** once parameters are uploaded
* support both subpixel and SDF renderers

069 defines the complete animation framework for text effects.

---

## **2. Goals**

### **2.1 Primary Goals**

* Unified animation curve system
* Per-effect animation parameters
* Per-text-node animation state
* GPU shader-controlled transitions
* Full compatibility with SDF/MSDF + RGB paths

### **2.2 Secondary Goals**

* Zero overhead when no animations are active
* Support for “time uniform” or “per-instance time”
* Trigger hooks for widget system

---

## **3. Architecture Overview**

```
 User Event / UI State
                ↓
      Widget AnimationState
                ↓
      TextEffectAnimation { params }
                ↓
 SceneGraph (instance buffer update)
                ↓
      GPU Shader Curve Evaluation
                ↓
                Frame
```

---

## **4. CPU Data Model**

### **4.1 Animation State Per Text Node**

```
pub struct TextEffectAnimState {
    pub time: f32,         // Local time  
    pub active: bool,      // Animation active?
    pub speed: f32,        // Playback rate multiplier
    pub phase: f32,        // Phase offset
}
```

Each effect may use its own animation state or share a common time.

### **4.2 Animated Effect**

```
pub struct AnimatedTextEffect {
    pub base: TextEffect,            // from SPEC-068
    pub animation: TextEffectAnim,   // new
}
```

### **4.3 Animation Types**

```
pub enum TextEffectAnim {
    None,

    Pulse {               // Glow in/out
        amplitude: f32,
        frequency: f32,
    },

    BreathingOutline {    // Thickness expands & contracts
        min_thickness: f32,
        max_thickness: f32,
        frequency: f32,
    },

    DriftShadow {         // Shadow offset oscillates
        dx: f32,
        dy: f32,
        frequency: f32,
    },

    ColorShift {          // Color Lerp or Gradient
        to: Color,
        frequency: f32,
    },
}
```

---

## **5. CPU → GPU Transfer**

### **5.1 Extended Instance Data**

Each glyph instance gets:

```
effect_mode: u32,
effect_params: vec2<f32>,
anim_kind: u32,
anim_params: vec4<f32>,
time: f32,
```

Only filled if animation is active.

### **5.2 Time Management**

Renderer exposes:

```
pub fn set_time(&mut self, t: f32);
```

Time may be:

* system time
* animation clock
* user-provided timeline

---

## **6. Shader Model**

### **6.1 Curve Functions**

All animated effects use a small set of analytic functions in WGSL:

```
fn curve_sin(t: f32, freq: f32) -> f32 { 
    return sin(t * freq * TAU);
}

fn curve_pulse(t: f32, freq: f32) -> f32 {
    return 0.5 + 0.5 * sin(t * freq * TAU);
}

fn curve_breathe(t, freq) -> f32 {
    return 0.5 + 0.5 * sin(t * freq * TAU + PI);
}
```

### **6.2 Shader Behaviors**

#### **Pulse Glow**

```
scale = base_radius * (1.0 + amplitude * curve_pulse(time+phase, frequency));
```

### Purely Visual Offset Rule

Fractional offsets applied for animation are visual only. They MUST NOT be incorporated into layout, hit-testing, caret placement, or measurement. All of those use the unmodified SPEC-HAL-104 integer-aligned quads.

#### **Breathing Outline**

```
thickness = lerp(min_thickness, max_thickness,
                 curve_breathe(time+phase, frequency));
```

#### **Drifting Shadow**

```
offset = vec2(dx, dy) * curve_sin(time+phase, frequency);
quad_position += offset;  // Fractional offset applied at render time only
```

Note: This offset is purely visual. The base `quad_position` remains the SPEC-HAL-104-compliant integer pixel position. The animated offset is applied only during vertex emission to the GPU and does not affect layout, hit-testing, or cached geometry.

#### **Color Shift**

```
base_color = mix(base_color, target_color,
                 curve_pulse(time+phase, frequency));
```

---

## **7. Integration With Renderer**

### **7.1 Scene Graph**

Each text node:

* stores AnimatedTextEffect list
* updates its instance buffer every frame (only effects)

### **7.2 Pipelines (same as SPEC-068)**

* text_effect_pass (renders animated FX)
* text_main_pass (same as before, static)

Animations affect only FX pipeline unless `ColorShift` is used.

---

## **8. Event & Widget Integration**

### **8.1 Trigger Points**

Common triggers:

* hover enter / leave
* focus gained / lost
* selection changed
* validation error
* attention state change
* button pressed

Widgets update their `TextEffectAnimState.active` based on events.

### **8.2 Example API**

```
text_block
    .with_effect(Glow::default())
    .animate(TextEffectAnim::Pulse { amplitude: 0.5, frequency: 1.2 });
```

### **8.3 Implicit Effects**

Widgets may define defaults:

* Button hover → glow pulse
* Focus field → outline breathing
* Error label → red glow pulse

---

## **9. Testing Matrix**

### **9.1 Temporal Tests**

* Deterministic at any framerate
* No visible jitter
* Smooth curve at 60/120/144/240 Hz
* Stable at low FPS

### **9.2 Spatial Tests**

* No drift
* No misalignment
* Outline thickness stable

### **9.3 Stress Tests**

* 2000 animated glyphs
* 10 text nodes with multiple effects
* Stable GPU memory

---

## **10. Implementation Plan**

### **Phase 1 — Data Layer**

* Add TextEffectAnim
* Add TextEffectAnimState
* Extend text nodes

### **Phase 2 — Renderer Extensions**

* Add anim fields to instance buffer
* GPU pipeline additions

### **Phase 3 — Shader Implementation**

* Add curve functions
* Add per-effect animation logic

### **Phase 4 — Widget API Integration**

* DSL builder for effects
* Event-driven triggers

### **Phase 5 — Final Integration**

* Sorting
* Fallbacks
* Consistent reset semantics

---

## **11. Implementation Notes**

### **11.1 Performance**

* Animation cost = 0 when inactive
* GPU cost = O(n glyphs)
* CPU cost = extremely low (update instance params)

### **11.2 Future Work**

SPEC-HAL-070 will introduce:

* Multi-stage keyframe systems
* Timeline-based UI animations
* Multi-property curves (position + color + size + glow)
* Declarative animation graphs
