# SPEC-HAL-091A — Intermediate30 Experimental Density Extension

**Author:** JC  
**Status:** Binding / Experimental — JC-authorized scoped fourth-mode trial 2026-10-25  
**Authority:** JC's 2026-10-25 approval; subordinate to SPEC-HAL-002/002A, adopted SPEC-HAL-012 and binding SPEC-HAL-092A  
**Scope:** One fourth density recipe and its framework default selection; existing presets and ownership remain unchanged

## 1. Decision and boundaries

### 2026-11-04 JC OWNER-SELECTED DEFAULT-TRIAL SUPERSESSION

JC selects the existing Regular28 preset (`DensityMode::Regular`) as the current
framework default. `DensityMode::default()`, `Theme::light()` and `Theme::dark()`
select Regular28; `Theme::default()` still delegates to `Theme::light()`. This
supersedes **only the 2026-10-25 Intermediate30 default-selection clause in §1**,
retained as the historical trial record below. All four geometry, spacing and
list recipes, typography, UI scale, phase/invalidation ownership and invariants
remain unchanged. Intermediate30 remains independently selectable at nominal
30 logical pixels; explicit product density selections retain precedence. This
supersession does not adopt Proposed SPEC-HAL-090/091/092/093/094 en bloc.

### 2026-10-25 trial selection (default selection superseded above)

`DensityMode::Intermediate30` is an independently selectable fourth mode. It was
the default returned by `DensityMode::default()`, `Theme::light()` and
`Theme::dark()`. `Theme::default()` continues to delegate to `Theme::light()`.
This selection superseded the previous Comfortable32 default trial, not the
existence or recipes of Compact24, Regular28 or Comfortable32.

**INV-091A-PRESET-PRESERVATION:** The existing three explicit preset branches
remain byte-identical. Their geometry, spacing and list recipes are not tuned
to create this mode. The frozen fourth recipe uses the exact arithmetic midpoints of
the existing Regular and Comfortable numeric fields, recorded below, without
additional rounding. It is implemented as explicit match branches, not a
runtime interpolator or density service.

SPEC-HAL-090/091/092/093/094 remain Proposed; this scoped approval does not adopt
them en bloc or make their unrelated requirements binding. No other Draft or
Proposed contract is adopted by this extension.

## 2. Closed GeometrySpec recipe

All dimensions are logical pixels. `padding_y = 5` and
`control_height = ceil(typography.line_height + 2 * padding_y)`. The unchanged
current line height of 20 therefore yields a nominal control height of 30.
TextField height/minimum hit size, Button height and SpinBox height use that
existing derived control-height mechanism. Other entries below retain their
declared numeric meaning.

| Geometry owner | Intermediate30 fields |
| --- | --- |
| text_field | height=30, padding_x=7, padding_y=5, radius=2, icon_size=17, min_hit_size=30 |
| button | height=30, padding_x=11, padding_y=5, radius=0 |
| checkbox | size=17, radius=1, border_thickness=1, focus_ring_thickness=2, focus_ring_padding=2, checkmark_inset=4.25, checkmark_thickness=2.5, checkmark_leg_ratio=0.5, label_gap=8 |
| icon | size_small=15, size_regular=17, size_large=22 |
| palette | width=600, height=400 |
| text_area | min_height=100 |
| combo_box | affordance_width=26, radius=2 |
| switch | track_width=41, track_height=20, thumb_diameter=16, thumb_inset=2, border_thickness=1, label_gap=8 |
| slider | track_length=165, track_thickness=5, thumb_diameter=16, border_thickness=1, label_gap=8 |
| progress_bar | track_length=165, track_thickness=14, border_thickness=1, radius=2.5, label_gap=8 |
| busy_indicator | track_length=165, track_thickness=5, border_thickness=1, radius=2.5, indicator_ratio=0.3, label_gap=8 |
| spin_box | height=30, value_width=52, button_width=26, border_thickness=1, radius=2, label_gap=8 |
| divider | thickness=1 |
| color_picker | sv_size=210, hue_strip_height=15, strip_gap=9, hex_field_width=92, swatch_height=26, swatch_width=38, radius=2.5, swatch_radius=2.5, popup_padding=11 |
| segmented_control | height=30, min_segment_width=60, padding_x=11, radius=2.5, separator_width=1 |
| time_picker | column_width=30, column_height=30, ampm_width=30, separator_width=9, radius=2.5, field_height=30, field_padding_x=9, popup_padding=8 |
| tag_input | chip_height=23, chip_padding_x=7, chip_gap=4.5, chip_radius=2.5, chip_close_size=13, checkbox_size=15, checkbox_gap=7, popup_max_visible_rows=9, radius=2.5 |
| track_bar | bar_height=22, handle_hit_width=11, handle_visual_width=3.5, radius=2.5 |
| root_inset | 11, with its existing ScrollArea-only content-padding ownership |

This is the complete new GeometrySpec recipe. It adds no field or alternate
geometry authority, does not modify a widget's explicit instance precedence,
and does not change how a consuming layout owner interprets a field.

## 3. Spacing and list recipes

`SpacingSpec::for_density(Intermediate30)` supplies:

| xxs | xs | s | m | l | xl | x2l |
| --- | --- | --- | --- | --- | --- | --- |
| 2 | 4.5 | 7 | 9 | 11.5 | 14 | 16 |

`ListGeometrySpec::for_density(Intermediate30)` supplies
`row_height_min=26`, `row_height=30`, `padding_x=10`.
The standalone `ListGeometrySpec::default()` continues to select Compact.
All original SpacingSpec and ListGeometrySpec branches remain byte-identical.

## 4. Existing owners remain authoritative

**INV-091A-PHASE-OWNERSHIP:** Density supplies configured values under
SPEC-HAL-012 §§5/8/13/16. The existing authorized application/update path,
SPEC-HAL-026 layout and SPEC-HAL-354 dependency invalidation continue to own
next-frame consumption. Changed metrics require their existing layout/text
and paint invalidation before use; Paint cannot substitute new metrics for a
completed layout. This trial adds no invalidation path, scheduler or layout
algorithm, and retains no cross-phase references.

**INV-091A-TYPOGRAPHY-SCALE:** Font size 13, line height 20 and baseline offset
-1 remain unchanged. Existing font/shaping and SPEC-HAL-092A optical-baseline
ownership remain unchanged. Density is not UI scale: the existing default UI
scale remains 1.0, and device conversion remains with its current owner.

The existing binding glyph-containment rule in SPEC-HAL-094 §3A remains in
force: control bounds contain rendered ascent/descent around the authoritative
baseline. The configured nominal height does not replace that text-owner
obligation or establish rendered-glyph proof. Referencing this binding
subsection does not adopt the remainder of Proposed SPEC-HAL-094.

Colors, interaction timing, focus/capture policy, input routing, animation,
renderer behavior and the three original recipes are not changed. There is no
appearance work, disabled/read-only capability or transition-authority
amendment in this extension. An outside-Theme density match that requires a
semantic or interaction-policy choice is a blocker for the owning authority;
this document authorizes only the specified numeric recipe and selection.

## 5. Framework conformance proof

The existing `theme_constructors_apply_density_spacing` and
`spacing_spec_resolves_density_tokens` tests own framework default selection,
exact four-mode geometry/spacing/list values, typography and preservation of
density through theme changes. Source/configuration proof does not by itself
establish rendered-glyph containment, native presentation, every embedding or
whole-framework conformance.
