# SPEC-HAL-349: SectionHeader Emphasis

**Status:** Active / Binding
**Layer:** Visual Primitive / Dense Composition
**Scope:** SectionHeader presentation only
**Depends on:** SPEC-HAL-002, SPEC-HAL-092, SPEC-HAL-096, SPEC-HAL-214, SPEC-HAL-332
**Narrowly amends:** SPEC-HAL-214 invariant classes C, D, and E

---

## 1. Purpose

Dense professional compositions need both primary section labels and lower-emphasis subsection labels. `AuxiliaryText` is supporting copy, not a section-label primitive, while GroupBox and independently painted section wells add container semantics and surface weight that a label does not own.

This SPEC keeps SectionHeader as the single descriptive section-label primitive and admits a lower-emphasis presentation without adding interaction, hierarchy, containment, or background painting.

---

## 2. Presentation Model

SectionHeader exposes one presentation enum:

```rust
pub enum SectionHeaderEmphasis {
    Primary,
    Subtle,
}
```

and may expose builder-time configuration and readback:

```rust
pub fn with_emphasis(self, emphasis: SectionHeaderEmphasis) -> Self;
pub fn emphasis(&self) -> SectionHeaderEmphasis;
```

`Primary` is the default and preserves the existing uppercase, primary-color section label.

`Subtle` preserves the caller's text case, uses the theme's secondary typography color, and uses the same compact theme-relative label size as `Primary`. Its quieter role comes from case and semantic color, not from making a section label read like undersized metadata. It remains a SectionHeader: descriptive, non-container, background-free, and without interaction semantics.

---

## 3. Invariants

- **INV-349-SINGLE-PRIMITIVE-1:** Primary and subtle labels are presentations of SectionHeader, not separate widgets or consumer-painted replicas.
- **INV-349-NO-HIERARCHY-1:** Emphasis is visual only. It does not establish containment, navigation, or structural hierarchy.
- **INV-349-THEME-1:** Size and color derive from the active theme. Consumers do not provide raw styling.
- **INV-349-SURFACE-1:** Neither emphasis mode paints a background, well, card, border, or GroupBox surface.
- **INV-349-TEXT-1:** Primary emphasis may normalize case; subtle emphasis preserves caller-authored case.
- **INV-349-SECONDARY-1:** Optional secondary text retains existing AuxiliaryText behavior in both modes.

---

## 4. Forbidden Behaviors

- Using AuxiliaryText as a semantic section heading.
- Reconstructing SectionHeader paint or layout in consumers.
- Adding a background well or container ownership to subtle emphasis.
- Making emphasis interactive, focus-driven, collapsible, or navigational.
- Treating emphasis as application-domain hierarchy.

---

## 5. Verification

Mechanical tests must prove:

1. existing SectionHeader construction retains primary emphasis;
2. subtle emphasis preserves caller case;
3. subtle emphasis resolves to the same compact label size as primary emphasis under the same theme;
4. both modes remain finite under unbounded measurement;
5. neither mode introduces child ownership or background painting.
