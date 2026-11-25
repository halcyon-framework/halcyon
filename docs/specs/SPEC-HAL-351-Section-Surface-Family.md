# SPEC-HAL-351: Section Surface Family

**Status:** Binding
**Layer:** Visual Primitive / Dense Composition
**Scope:** `SectionBandHeader`, `SectionMetaHeader`, and `DisclosureSectionHeader`
**Depends on:** SPEC-HAL-002, SPEC-HAL-214, SPEC-HAL-215, SPEC-HAL-224, SPEC-HAL-260A, SPEC-HAL-332, SPEC-HAL-349, SPEC-HAL-350
**Integrates with:** SPEC-HAL-352

---

## 1. Purpose

Dense inspector and operator surfaces need more than one section marker. A plain descriptive label, a structural band row, a metadata-bearing band, and an expandable section control do not own the same authority.

This SPEC separates those surfaces so `SectionHeader` remains the descriptive label primitive, while additional section surfaces own only the visual or request authority their use case requires.

---

## 2. Surface Taxonomy

`SectionHeader` remains governed by SPEC-HAL-214 and SPEC-HAL-349. It is a descriptive, background-free section label.

`SectionBandHeader` is a full-width tonal band that labels a structural group. It paints a band background and section text, but owns no content, no expansion state, and no data semantics.

`SectionMetaHeader` is a full-width tonal band with a right-aligned read-only metadata slot. The metadata is display-only status such as count, revision, or resolved state. It does not own or compute that state.

`DisclosureSectionHeader` is an interactive section header that reflects externally provided expanded/collapsed state and requests a toggle through an explicit callback. It does not own expansion state or content visibility.

An edge/focus marker is not a header type. It is a state treatment owned by the focused or selected section surface.

---

## 3. Invariants

- **INV-351-SPLIT-1:** Label, band, metadata, and disclosure are distinct public surfaces. They must not be collapsed into one `SectionHeader` presentation enum.
- **INV-351-HEADER-1:** `SectionHeader` remains unchanged: descriptive, background-free, non-container, and without disclosure or metadata ownership.
- **INV-351-BAND-1:** `SectionBandHeader` paints structure only. It must not own content, child layout, expansion state, or application-domain state.
- **INV-351-META-1:** `SectionMetaHeader` displays caller-provided metadata verbatim. It must not compute counts, revisions, statuses, validation, or policy outcomes.
- **INV-351-DISCLOSURE-1:** `DisclosureSectionHeader` owns no expanded/collapsed state. It reflects a caller-provided boolean and emits only an explicit toggle request.
- **INV-351-DISCLOSURE-2:** `DisclosureSectionHeader` may own transient interaction visual state such as hover, press, or focus affordance. It must not derive, persist, or mutate expanded/collapsed state from that visual state.
- **INV-351-DISCLOSURE-3:** `DisclosureSectionHeader` presents disclosure as compact inset row chrome. The control surface must remain visually distinguishable from the surrounding band in idle state, must not occupy the full band height, and may receive focus geometry as a control surface. Focus geometry must not target the passive indicator mark alone.
- **INV-351-INDICATOR-1:** Disclosure glyph rendering must use the passive DisclosureIndicator primitive from SPEC-HAL-224.
- **INV-351-CONTROLLER-1:** Content visibility remains owned by external controller or application state, such as `CollapsibleSection` per SPEC-HAL-215.
- **INV-351-SCROLL-1:** A caller that needs disclosure-aware viewport preservation must explicitly coordinate the staged state change through SPEC-HAL-352. `DisclosureSectionHeader` does not acquire scroll authority or infer an anchoring transaction.
- **INV-351-THEME-1:** Typography and colors derive from the active theme. Consumers do not provide raw styling.
- **INV-351-NO-CARD-1:** Section surfaces must not introduce cards, shadows, gradients, rounded decorative wells, or consumer-app chrome.

---

## 4. Forbidden Behaviors

- Adding metadata or disclosure behavior to `SectionHeader`.
- Letting `DisclosureSectionHeader` mutate a `CollapsibleSection` directly.
- Treating a section band as a container that measures or owns the content below it.
- Using a disclosure indicator as the state owner or event handler.
- Computing metadata inside a section header.
- Encoding focus/selection as a separate header kind.

---

## 5. Verification

Mechanical tests must prove:

1. all section surfaces report finite layout under unbounded measurement;
2. `SectionBandHeader` and `SectionMetaHeader` are non-hit-testable passive visual surfaces;
3. `DisclosureSectionHeader` is hit-testable and invokes its explicit toggle callback from pointer activation;
4. `DisclosureSectionHeader` reports the externally supplied expanded state without owning a controller;
5. `SectionHeader` remains separate from the new surfaces.
6. the disclosure demo requests SPEC-HAL-352 anchoring before applying externally owned expanded state.
