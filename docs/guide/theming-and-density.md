---
doc-id: halcyon-guide-theming
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-012
---

# Theming and Density

Theme token hierarchy, density, and runtime propagation at revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`. Two-axis tags per SPEC-HAL-309
§7. SPEC-HAL-012 (Theming 2.0) is `Draft` [authority: draft-intent —
SPEC-HAL-012:4]; the live shape below diverges from parts of it and the
divergences are recorded, not smoothed over.

## 1. The Live Theme Structure

`Theme` (`src/core/theme.rs:2006-2033`) [implementation: implemented] is a
flat family of spec structs:

| Field | Type | Line |
|---|---|---|
| `density` | `DensityMode` | :2007 |
| `geometry` | `GeometrySpec` | :2008 |
| `typography` | `TypographySpec` | :2009 |
| `colors` | `ColorSpec` | :2010 |
| `interaction` | `InteractionSpec` | :2011 |
| `styles` | `StyleSpec` | :2013 |
| `spacing` | `SpacingSpec` | :2015 |
| `padding` | `PaddingSpec` | :2017 |
| `borders` | `BorderSpec` | :2019 |
| `radii` | `RadiusSpec` | :2021 |
| `scrollbar_geom` | `ScrollbarGeometrySpec` | :2023 |
| `list_geom` | `ListGeometrySpec` | :2025 |
| `shadows` | `ShadowSpec` | :2027 |
| `focus_ring` | `FocusRingSpec` | :2029 |
| `tooltip` | `TooltipSpec` | :2031 |
| `is_dark` (private) | `bool` | :2033 |

Constructors and switches [implementation: implemented —
`src/core/theme.rs:2038-2112`]: `Theme::dark()`,
`Theme::dark_with_density(density)`, `Theme::light()`,
`Theme::light_with_density(density)`, `is_dark()`, `toggle()`,
`set_density(density)`.

**`Theme::default()` is the light theme** — "The framework default theme is
light" [implementation: implemented — `src/core/theme.rs:2289-2293`].
Applications wanting dark start from `Theme::dark()` explicitly.

Unified control chrome: `control_border(focused, captured, error)`,
`chrome_radius(category, requested, is_round)`, and
`control_chrome(category, state, validation) -> ControlChromeSpec` resolve
per-category/state/validation chrome centrally
[implementation: implemented — `src/core/theme.rs:2136-2181`].

## 2. Density

`DensityMode` [implementation: implemented — `src/core/theme.rs:47-58`]:

- `Compact` (enum default) — "EnterpriseGrid anchor: 24px controls, 13px
  text, 2px radius. Maximum density for power users and data-heavy
  layouts."
- `Regular` — "Balanced desktop: 28px controls, 13px text, 2px radius."
- `Comfortable` — "Relaxed spacing: 32px controls. Accessibility-friendly,
  touch-compatible."

Density is a `Theme` field consumed through the geometry/spacing specs;
switch via the `_with_density` constructors or `set_density` (§1).

## 3. How Theme Reaches Widgets

- Update phase: `UpdateContext::with_theme(...)` carries `&Theme` into
  every widget `update` [implementation: implemented —
  `src/core/app.rs:1246-1248`].
- Layout/paint: `build_scene_reusing` takes `theme: &Theme` and the frame
  passes the loop's current theme [implementation: implemented —
  `src/core/app.rs:1645`, `:11117`].
- Widgets resolve visual values at use time from the theme rather than
  storing resolved visuals — this is SPEC-HAL-012 §7's lookup model
  [authority: draft-intent — SPEC-HAL-012:296-315]; the live mechanism that
  makes stale storage unviable is the runtime switch below.

## 4. Runtime Theme Switching — Live Path

The production loop each frame [implementation: implemented —
`src/core/app.rs:10722-10726`]:

```text
if app_root.context.take_theme_changed() {
    theme = app_root.context.theme.clone();
    widget_tree.mark_subtree_dirty(active_root);
}
```

So a runtime theme change is: flag consumed once → loop's theme replaced by
clone → the active root subtree marked dirty, forcing re-layout and repaint
under the new theme. No screen rebuild is triggered by the switch itself.

Divergence from SPEC-HAL-012 §8 (draft): the SPEC sketches a
`ThemeManager { current: Arc<Theme> }` with "no widget rebuild … pure
re-render" [authority: draft-intent — SPEC-HAL-012:318-340]. Live: no
`ThemeManager`/`Arc` swap at the cited path — an owned clone plus explicit
subtree invalidation (which re-runs layout, not only paint). Classified
draft-intent + implemented-divergent; recorded for audit. The SPEC's
"immutable Theme, zero-cost lookup" performance framing
(SPEC-HAL-012:368-399) is design prose; no measurements exist at this
revision (`unknown`).

## 5. Consumer Rules

- Read tokens from `cx`-provided `&Theme` in update/layout/paint; never
  cache resolved colors/geometry across frames — the runtime switch (§4)
  invalidates, it does not notify widgets individually.
- Respect the spec-struct boundaries: geometry/spacing/padding decisions
  belong to their spec structs, not ad-hoc constants; control chrome goes
  through `control_chrome`/`control_border` (§1) rather than per-widget
  reimplementation.
- Theme mutation is owner-driven through `AppContext` (`take_theme_changed`
  producer side); widgets do not mutate the theme.

## 6. Not Covered / Not Claimed

- Per-component style *tables* exactly as drafted in SPEC-HAL-012 §4
  (ButtonStyle/TextFieldStyle/PanelStyle/ScrollbarStyle) do not exist in
  the live tree. `StyleSpec` was verified 2026-07-24: exactly four
  fields — `focus_button: FocusButtonDefaults`,
  `focus_checkbox: FocusCheckboxDefaults`, `hover: HoverStyleDefaults`,
  `pressed: PressedStyleDefaults` (`src/core/style.rs:423-431`, Default
  at `:469`) — focus/hover/pressed interaction defaults, not per-widget
  style tables [authority: draft-intent — SPEC-HAL-012:120-199 |
  implementation: implemented-divergent; contradiction recorded as C10].
- Theme serialization (SPEC-HAL-012 §11) and theme-transition animation
  (§6): not verified in this unit; `unknown`.
- Color values themselves: read them from `src/core/theme.rs` at your
  revision; copying hex tables into a guide creates instant staleness
  (SPEC-HAL-309 §8).
