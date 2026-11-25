# SPEC-PM-008: **Projection Matrix Truncation Jitter at Fractional DPI**

### *Integer Truncation of Logical Viewport Dimensions Causes Sub-Pixel Position Drift*

**Author:** JC
**Status:** Post-Mortem + Permanent Rule
**Severity:** P0 — Caused 3+ months of visible jitter in ALL widgets at non-integer DPI
**Applies to:** Renderer projection matrix construction

---

# 1. Problem

At fractional DPI scales (125%, 150%, 175%), ALL rendered
content jitters by ~0.5 physical pixels on every window
resize tick. Guide lines, disclosure toggles, icons,
separators, borders, text — everything shifts independently
between frames.

The bug is invisible at 100% and 200% DPI (integer scales).
It affects every widget, every primitive, every pixel.

---

# 2. Root Cause

The orthographic projection matrix maps logical coordinates
to NDC (Normalized Device Coordinates):

```
NDC_x = logical_x * (2.0 / logical_width) - 1.0
```

`logical_width` was derived by truncating the fractional
logical dimension to an integer:

```rust
let logical_w = surface_desc.bounds.width as u32;  // TRUNCATION
// ...
Globals::new(logical_size.0 as f32, ...)  // 800.0 not 800.666
```

At 150% DPI, physical width changes by 1 pixel per resize:
```
physical 1200 → logical 800.000 → truncated 800
physical 1201 → logical 800.666 → truncated 800  ← WRONG
physical 1202 → logical 801.333 → truncated 801
physical 1203 → logical 802.000 → truncated 802
```

The projection coefficient `2.0 / 800` vs `2.0 / 801`
shifts every screen position:

```
Guide at X=30:
  logical_w=800: screen = 30 × 1201/800 = 45.0375 px
  logical_w=801: screen = 30 × 1202/801 = 45.0187 px
```

Different physical sub-pixel position each frame = jitter.

---

# 3. Why It Was Hard to Find

Every diagnostic pointed upstream:
- All paint-time inputs constant (rect.x=0, slot.x=0, sf=1.5)
- All geometry engine outputs constant
- All scroll offsets zero
- All snap_to_physical results stable

The bug is DOWNSTREAM of paint — in the GPU projection
matrix, a part of the pipeline that paint code never sees.
The widget layer was producing perfect coordinates; the
renderer was projecting them onto a shifting grid.

The diagnostic that cracked it: confirming ALL paint inputs
are constant forced investigation downstream into the
batcher → vertex builder → projection matrix → `as u32`.

---

# 4. The Fix

Derive projection logical dimensions from physical size
and scale factor instead of truncating compositor bounds:

```rust
// BEFORE:
let logical_w = surface_desc.bounds.width as u32;
Globals::new(logical_size.0 as f32, ...)

// AFTER:
let proj_logical_w = physical_size.0 as f32 / scale_factor;
Globals::new(proj_logical_w, ...)
```

This ensures `logical_x * (2.0 / logical_w)` maps to
`logical_x * scale_factor / physical_w`, which produces
exact physical pixel positions regardless of window size.

---

# 5. Invariants

**INV-PM-008-1:** The projection matrix MUST use exact
fractional logical dimensions, never integer-truncated
values. `physical_size / scale_factor` is the canonical
derivation.

**INV-PM-008-2:** Integer truncation of logical dimensions
(`as u32`, `.floor()`, `.round()`) is FORBIDDEN in any
code path that feeds the projection matrix or vertex
shader coordinate transform.

**INV-PM-008-3:** The projection must map logical coordinate
X to physical pixel `X × scale_factor` exactly, independent
of viewport dimensions. Any deviation causes visible jitter
at fractional DPI.

---

# 6. Forbidden Behaviors

**FORBID-PM-008-1:** Using `as u32` or `as i32` on logical
viewport dimensions that feed the projection matrix.

**FORBID-PM-008-2:** Deriving projection dimensions from
compositor bounds without preserving fractional precision.

**FORBID-PM-008-3:** Assuming integer DPI scale factors.
Halcyon must render correctly at 100%, 125%, 150%, 175%,
200%, and any arbitrary fractional scale.

---

# 7. Detection

This class of bug is invisible at integer DPI scales and
impossible to diagnose from paint-time coordinates. The
diagnostic protocol is:

1. Confirm all paint inputs are constant during resize
2. If constant but jitter persists → bug is downstream
3. Check projection matrix construction for integer
   truncation of logical dimensions
4. Check vertex builder for coordinate quantization
5. Check scissor rect computation for truncation artifacts

---

# 8. Related

- **SPEC-PM-007** (Clip-Hash Ordering Hazard): Different bug
  class — draw order instability from clip_hash tiebreaker.
  Fixed by z_index discipline. PM-008 is a coordinate
  precision bug, not an ordering bug.

- **Scale factor threading** (geometry engine): The geometry
  engine's `snap_to_physical` uses exact scale_factor for
  column positions. The projection must use the same exact
  scale_factor for the transform. Mixing truncated and
  exact values across the pipeline causes misalignment.

---

# 9. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode | N/A |
| B — Phase Separation | Preserved |
| C — ID-based Identity | N/A |
| D — Pure Data | **Strengthened.** Projection is pure f(physical, sf). |
| E — GPU-First | **Strengthened.** Eliminates GPU coordinate drift. |
| F — Sovereignty | N/A |
