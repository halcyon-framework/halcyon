# **SPEC-HAL-097 — Motion & Micro-Animation Rules**

**Status:** REJECTED / RETIRED / INACTIVE / NOT BINDING
**Owner disposition:** JC, 2026-11-04 — HAL-REJECT-FRAMEWORK-WIDE-AESTHETIC-MOTION-BANS-01

## JC owner disposition — 2026-11-04

JC rejects this proposal's categorical framework-wide aesthetic-motion restrictions.
They were never adopted as universal Halcyon law. In particular, the bans on spring,
bounce, elastic motion, overshoot, scale, rotation, zoom and wobble; the exclusive
duration bands and easing list; the universal displacement limits, fade-only rules
and motion-property/frame-percentage quotas are retired, not pending approval.
Spring, bounce, overshoot, rotation, scale and long transitions are not categorically
prohibited by Halcyon.

Applications own their artistic expression. Halcyon supplies
high-performance implementation mechanisms, not a compulsory austere visual genre.
This disposition creates no replacement restriction, curve whitelist, duration cap,
per-animation permit, motion-approval system, mode exception or Showcase-only exemption.
Products use the framework's actual capabilities; this documentary disposition does
not implement an unavailable capability or authorize runtime work.

[HAL-002](SPEC-HAL-002-Constitution-and-Architectural-Invariants.md),
[HAL-002A](SPEC-HAL-002A-Constitution-Amendment-Guidelines.md), binding
[HAL-021](SPEC-HAL-021-Animations.md), and separately adopted widget-specific
contracts retain their exact scopes. HAL-021A's existing status and pressed-local
scope are unchanged; HAL-363/364 remain Proposed. No aesthetic restriction is
reinstated through a historical dependency, cross-reference, proposed amendment,
implementation resemblance, or the rejected
[HAL-097A](SPEC-HAL-097A-Motion-Authorization-and-Layout-Transition-Law.md) or
[HAL-332](SPEC-HAL-332-Professional-Visual-Charter.md).

## Historical proposal — provenance only

The entire original proposal below, including its imperative wording, examples,
budgets and implementation plan, is historical nonbinding provenance. It supplies
no current requirement, permission gate, adoption condition or implementation task.

**SPEC-HAL-097**
**Motion & Micro-Animation Rules**
**Author:** Halcyon Architect
**Historical proposal status:** PROPOSED; never adopted as universal framework law.
**Version:** 1.0

**Depends On:**

* SPEC-HAL-041 — Unified Timing & Animation Clock
* SPEC-HAL-042 — Frame Loop & Engine Ticker
* SPEC-HAL-070 — Halcyon Timeline Animation System
* SPEC-HAL-072 — Declarative Widget Style System
* SPEC-HAL-074 — Style Runtime Engine
* SPEC-HAL-090 — Logical Grid & Compact Layout System
* SPEC-HAL-093 — Visual Appearance & Radii System

---

## **Implementation Status**

*Not implemented.*

This SPEC defines Halcyon’s canonical motion system:

* timing rules
* easing curves
* micro-interactions
* elevation and opacity choreography
* state transitions
* animation budget
* constraints required for compact engineering UI

Halcyon motion must feel:

* fast
* precise
* utilitarian
* non-decorative
* consistent
* engineered rather than playful

No “bouncy,” “springy,” or “overshoot” animations are allowed.

---

# **1. Motivation**

Motion in Halcyon is not decoration — it is a communication layer.

Its purpose is to:

* clarify changes of state
* guide attention
* smooth visual transitions
* maintain spatial memory
* reduce cognitive friction

Motion should always be:

* subtle
* fast
* functional
* non-distracting

A user should perceive meaning, not animation.

---

# **2. Duration Rules**

Halcyon uses strict duration bands.
These are not suggestions — they are the only allowed ranges.

### **Ultra-Fast**

```
60–90 ms
```

Used for:

* hover feedback
* press (active) feedback
* focus ring changes

### **Fast**

```
100–140 ms
```

Used for:

* dropdown appearance
* tooltips
* context menus
* item selection

### **Medium**

```
150–180 ms
```

Used for:

* accordion sections
* tab transitions
* small panel reveals

### **Slow (rare)**

```
200–240 ms
```

Used only for:

* modal overlays
* dimming backgrounds
* layout transitions where clarity is needed

Anything slower is prohibited.
Anything faster than Ultra-Fast becomes imperceptible and pointless.

---

# **3. Easing Functions**

A very small set of curves is allowed.
Reducing variability increases predictability and visual discipline.

### **FAST OUT**

```
cubic-bezier(0.18, 0.9, 0.32, 1.0)
```

Default for appearing elements.

### **FAST IN**

```
cubic-bezier(0.5, 0.0, 0.7, 0.0)
```

Default for disappearing elements.

### **SMOOTH STEP**

```
cubic-bezier(0.33, 0.0, 0.67, 1.0)
```

Used for opacity and subtle fades.

### **LINEAR**

Used only for:

* progress bars
* shimmer
* counters

No spring curves.
No elastic curves.
No overshoot.
Motion must stop precisely at its target.

---

# **4. Micro-Interaction Rules**

Micro-motion provides immediate feedback without altering layout stability.

## **4.1 Hover**

* opacity shift: +5–8%
* border tone shift: +6%
* optional shadow expansion: +1 px

No scaling.
No positional shifts.
No wobble.

## **4.2 Press (Active)**

* surface darken: 8–12%
* optional optical offset: 1–2 px
* duration: 60–80 ms

## **4.3 Selection**

* background fade: 120–160 ms
* icon/text tint → Accent.Primary

## **4.4 Focus**

* border-color transition (120 ms)
* optional 1px inner glow at low opacity

---

# **5. Layout Transitions**

Allowed transitions:

* height collapse/expand
* opacity fade-in/out
* small transform-based slides (≤ 4 px vertical)

Forbidden:

* zoom
* scale
* bounce
* rotation
* physics-based transitions

The layout must remain spatially stable.
Motion may not introduce jitter or drift.

---

# **6. Elevation & Shadow Motion**

Elevation changes must be perceptible but subtle.

### Elevation-in:

* shadow expands softly
* opacity increases by 5–8%
* surface may rise optically by 1–2 px

### Elevation-out:

* reverse the above steps

Duration: 120–160 ms.

Popover / contextual menu appearance:

```
opacity: 0 → 1  
offset-y: +3 px → 0 px  
```

Modals:

* fade only
* optional 3–4 px rise-in
* no scale

---

# **7. Tooltip Motion**

Tooltip appearance:

```
opacity: 0 → 1 over 100 ms
offset-y: +2 px → 0 px
easing: FAST OUT
```

Tooltip disappearance:

```
opacity: 1 → 0 over 100 ms
easing: FAST IN
```

Tooltips must **never** animate scale.

---

# **8. Scroll & Drag Interpolation**

Scroll and drag interactions must feel deterministic and tightly bound to input:

* high-frequency deltas
* velocity smoothing
* no spring or elasticity
* no bounce at edges
* interpolation in physical pixels

Dragging must feel “directly attached” to the cursor.

---

# **9. Animation Budget**

To preserve a high-performance feel, Halcyon enforces strict motion budgets.

Rules:

* no widget may animate more than **3 properties** simultaneously
* opacity transitions do not count against the budget
* layout-affecting animations are heavily restricted
* total animation load must not exceed **10% of frame time**

Motion must never cause dropped frames or layout thrashing.

---

# **10. Performance Requirements**

Motion must:

* run at 144–240 Hz where possible
* adapt smoothly to lower refresh rates
* be GPU-driven
* avoid CPU-bound layout changes
* follow delta-time rules
* obey frame pacing from SPEC-HAL-043

Animations must not perform any heavy recomputation.
They must be isolated to visual layers.

---

# **11. Declarative API Mapping**

The declarative style system exposes concise motion tokens:

```
motion.hover.opacity = 0.07
motion.press.offset_y = 2px
motion.expand.duration = 140ms
motion.focus.border_color = accent.primary
motion.tooltip.offset_y = 2px
motion.menu.appear = fast_out_120ms
```

These compile into timeline operations defined by SPEC-HAL-070.

---

# **12. Implementation Plan**

### Phase 1 — Timing & Easing Library

Define canonical curves and durations.

### Phase 2 — Micro-Interaction Engine

Hover, press, focus, selection effects.

### Phase 3 — Layout Transitions

Height, opacity, minimal sliding transitions.

### Phase 4 — Shadow & Elevation

Smooth elevation changes and popover motion.

### Phase 5 — Widget Integration

Buttons, lists, trees, dropdowns, text fields, menus, toolbars, popovers.

### Phase 6 — Cross-Platform QA

* 60/120/144/240 Hz displays
* light/dark themes
* dense vs comfortable density modes
* A/B visual QA for jitter, drift, flicker

---

## Disposition confirmation

The historical proposal remains REJECTED / RETIRED / INACTIVE / NOT BINDING.
Its retained rules and implementation plan cannot reinstate the rejected bans or
authorize runtime changes.
