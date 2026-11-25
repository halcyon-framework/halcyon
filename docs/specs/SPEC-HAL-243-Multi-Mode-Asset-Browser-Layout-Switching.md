# SPEC-HAL-243: Multi-Mode Asset Browser Layout Switching

**Status:** Draft
**Layer:** UI Composition / ViewModel Binding / Layout Authority
**Scope:** Switching between grid, list, tree, and custom views in the Asset Browser
**Owner:** Application UX Shell / View Substrate
**Applies To:** Asset Browser, Selection Panels, Picker Overlays

---

## 0. Purpose

Defines the layout switching infrastructure and binding model for Asset Browser views.
Supports mode-switchable layouts:
- Grid (thumbnail-dominant)
- List (linear, info-rich)
- Tree (hierarchical groupings)
- Custom (e.g. tag-grouped, timeline)

---

## 1. Invariants

### INV-243.1: Layout Mode Is ViewModel-Bound

The layout mode is a state property in the ViewModel. Not inferred.

- Explicitly settable
- Stored per-view instance (not global)
- Used for both rendering and behavioral adjustments

### INV-243.2: Layout Mode Affects Composition, Not Data

Switching layout never re-fetches or mutates asset data — it only changes the view projection.

- The asset list remains intact
- No filtering or resorting by default
- Geometry, visuals, and overlays change

### INV-243.3: Mode Switch Is Intent-Driven

UI triggers an `Intent::SwitchAssetViewMode(LayoutMode)` to mutate the ViewModel.

- Ensures consistent source of truth
- Allows analytics or user preference tracking

### INV-243.4: Unsupported Modes Must Gracefully Degrade

View implementations must be resilient to unsupported modes.

- Fallback to closest valid projection
- Degrade icons and overlays where required

---

## 2. Supported Layout Modes

```rust
enum LayoutMode {
    Grid,           // Thumbnail-first, fluid
    List,           // Row-style, label-dominant
    Tree,           // Hierarchical folders/groups
    Timeline,       // (Optional) temporal layout
    Custom(String), // User or app-defined
}
```

---

## 3. UI Responsibilities

- Provide layout switcher UI (icons, dropdown, etc.)
- Forward mode change intents to the ViewModel
- Persist per-context if needed
- Animate layout transitions (optional)

---

## 4. Out of Scope

- Tag-based grouping rules (see HAL-245)
- Asset grouping/folder management
- Saved layout presets
