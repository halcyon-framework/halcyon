# SPEC-HAL-242: Asset Metadata Overlay and Affordance Contracts

**Status:** Draft
**Layer:** ViewModel Overlay / Visual Affordance Composition
**Scope:** Metadata display, badges, status hints, hover overlays for asset representations
**Owner:** Application Platform UX Layer
**Applies To:** Asset browsers, selectors, inspectors

---

## 0. Purpose

Defines the standard projection system for rendering metadata overlays and visual affordances on assets within grids, lists, or tree views.

Affordances include:
- Lock/busy badges
- Preview thumbnails
- Tag/status markers
- Quick action buttons (e.g., pin, delete, favorite)

---

## 1. Invariants

### INV-242.1: Overlays Are Projection-Only

UI overlays must be derived from the authoritative ViewModel. No direct state.

- Control renders badges/icons per bound state
- Cannot mutate state directly

### INV-242.2: Overlay Layer Is Separate From Item Body

Visual affordances are composited outside of item bounds and do not interfere with layout.

- Does not affect item hitbox or virtualized layout size
- Overlay visuals must clip or float

### INV-242.3: Affordance Contracts Are Explicit

All interaction-capable overlays must dispatch explicit intents.

- Example: `Intent::PinAsset`, `Intent::OpenMetadataEditor`
- No silent behavior

### INV-242.4: Hover Hints Are Delegated

Rich hover overlays (e.g., full metadata cards) must be triggered explicitly by user hover.

- Resolved asynchronously
- Delegated to viewmodel
- Cancelable

---

## 2. Overlay Types

- **Static Icon Badge** — e.g., lock, warning, favorite
- **Inline Metadata Labels** — tags, type info
- **Hover Panel** — on-demand preview + description
- **Interactive Buttons** — quick actions via intent

---

## 3. Control Responsibilities

- Declare overlay slots per item
- Query and bind projection metadata
- Route input to intent system
- Allow opt-in or opt-out per item type

---

## 4. Out of Scope

- Editing asset metadata (separate SPEC)
- Global asset tag system
- Remote metadata fetch
- Drag/drop overlays (see HAL-243)
