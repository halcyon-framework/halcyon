# SPEC-HAL-240: Asset Browser ViewModel and Projection Authority

**Status:** Draft
**Layer:** Application ViewModel / Projection Substrate
**Scope:** Application asset browsing, selection, preview, hierarchy projection
**Owner:** Application Platform Architecture
**Applies To:** Embedded asset explorers, editors, complex tool UIs

---

## 0. Purpose

This SPEC defines the architectural contract for implementing a structured, reactive Asset Browser substrate, including its ViewModel, projection boundaries, and interaction model.

This is not a file picker or system dialog. It is an internal application-level component that visualizes, navigates, and allows interaction with domain-specific assets (e.g., textures, scenes, animations, prefabs, scripts).

---

## 1. Invariants

### INV-240.1: Authority Resides in the ViewModel

The AssetBrowser control renders projections of an authoritative ViewModel, not its own data.

- ViewModel contains tree, folder, item, preview state
- Control performs no filesystem I/O or model resolution itself

### INV-240.2: Projection-Only Control

AssetBrowser UI is purely a renderer + interaction surface. No data ownership.

- Projections must be derived from externally-owned data
- Interaction updates must dispatch clearly defined intents

### INV-240.3: Embedded or Surface-Spawned

AssetBrowser may be embedded inline or invoked as an overlay surface.

- Popup or modal surface must bind through NavigationHost
- Inline embedding must respect layout container semantics

### INV-240.4: Supports Hierarchy and Flat Views

Control must support folder-tree projection or flat filter-based lists.

- ViewModel determines current mode (hierarchy, search, tag view)
- Tree projection is strictly read-only

### INV-240.5: Preview and Metadata Boundaries

Previewing and metadata resolution are asynchronous and projected.

- UI triggers preview intent
- ViewModel populates state when ready
- No blocking or eager load

---

## 2. Structure

### ViewModel Responsibilities

- Asset tree structure
- Filtering and sorting state
- Selected item state
- Preview URL or object
- Async fetch control / loading states

### Control Composition

- Search bar (optional)
- Folder tree (left)
- Item grid or list (right)
- Optional preview panel
- Navigation bar / breadcrumbs

---

## 3. Input and Output Events

- `Intent::SelectAsset(AssetId)`
- `Intent::OpenAsset(AssetId)`
- `Intent::PreviewAsset(AssetId)`
- `Intent::RefreshAssetList`
- Output: AssetId / URI / metadata

All must be routed through app intent layer. AssetBrowser must not emit raw data.

---

## 4. Rendering Considerations

- Virtualized scroll for item grid
- Optional thumbnail resolution delegate
- Must support keyboard and mouse interaction
- Context menus for actions (rename, delete, open)

---

## 5. Out of Scope

- OS file access or sandbox bridge (see HAL-239)
- Asset editing
- Import/export pipelines
- Drag and drop (see HAL-243+)
