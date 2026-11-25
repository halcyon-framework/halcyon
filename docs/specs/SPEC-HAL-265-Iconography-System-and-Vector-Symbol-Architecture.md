# SPEC-HAL-265 — Iconography System & Vector Symbol Architecture

### *First-Class Icon Identity, Registry, and Widget Integration for Halcyon*

**Author:** JC
**Status:** Proposed
**Date:** 2026-02-26
**Depends on:** SPEC-HAL-002 (Constitution), SPEC-HAL-264 (Vector Mesh Pipeline & Cache), SPEC-HAL-005 (Scene Graph), SPEC-HAL-027 (Batching), SPEC-HAL-024 (Coordinate Model), SPEC-HAL-096 (Color System), SPEC-HAL-095 (Iconography Rules & Pixel Grid), SPEC-HAL-263 (Image Pipeline), SPEC-PM-004 (Geometry Authoring Space), SPEC-HAL-158 (Layout Authority)
**Supersedes:** SPEC-HAL-095 §11–12 (asset pipeline and implementation plan sections only; §1–10 design rules remain valid)
**Consumed by:** Widget SPECs requiring icon support

---

# 1. Objective

Introduce a **first-class icon subsystem** that gives Halcyon structured, type-safe, cacheable, theme-aware vector icons — replacing ad-hoc path construction with a formal identity and registry model.

This SPEC defines:

* `IconId` — stable identity for every icon
* `IconDefinition` — immutable path topology in a canonical coordinate space
* `IconRegistry` — compile-time static registry with pure lookup
* `IconSpec` — widget-facing request type (id + size + color + stroke)
* Canonical coordinate space and scaling contract
* Layout participation (intrinsic sizing, alignment)
* Widget integration pattern
* Raster icon fallback (SPEC-263 compliant)
* Semantic role taxonomy
* Accessibility metadata contract
* Theming and state color model

This SPEC owns **icon identity and semantics**. It does NOT own tessellation, mesh caching, or GPU rendering — those are SPEC-HAL-264's domain.

---

# 2. Architectural Position

Icons are a **domain subsystem** sitting between the widget layer and the rendering substrate.

```
Widget Layer
    ↓ IconSpec { id, size, color, stroke_width }
  ┌──────────────────────┐
  │  Icon System         │  ← THIS SPEC
  │  (identity, registry,│
  │   scaling, layout)   │
  └──────────────────────┘
    ↓ Path + render parameters
  ┌──────────────────────┐
  │  Vector Mesh Cache   │  ← SPEC-HAL-264
  │  (tessellation,      │
  │   pixel snap, cache) │
  └──────────────────────┘
    ↓ PathMesh
  Batcher → GPU Path Pipeline (SPEC-R-004)
```

Icons are NOT:
- Ad-hoc `PathDraw` instances created inline by widgets
- Hardcoded geometry in widget paint methods
- Bitmaps dropped into scene nodes without contract
- Decorative sugar — they are a structured rendering subsystem

---

# 3. Icon Identity

## 3.1 IconId

Every icon has a unique, stable identifier:

```rust
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub struct IconId(u32);
```

IconId properties:
- Stable across builds (assigned at compile time, not generated)
- Never reused within a registry
- Lightweight (Copy, 4 bytes)
- Used as the sole reference to an icon throughout the system — widgets never store path data

## 3.2 Identity Contract

IconId follows Constitution Pillar C (Identity-Through-ID):
- Widgets reference icons by `IconId`, never by path data
- No widget stores `Path`, `PathMesh`, or `IconDefinition` directly
- Icon lookup is always via registry, never via cached reference

---

# 4. IconDefinition

The immutable specification of an icon's geometry:

```rust
pub struct IconDefinition {
    /// Unique identity.
    pub id: IconId,

    /// Human-readable name for debugging and accessibility fallback.
    pub name: &'static str,

    /// Vector path commands in canonical coordinate space.
    pub path: Path,

    /// Canonical coordinate space dimensions (e.g., 24.0 × 24.0).
    pub viewbox_width: f32,
    pub viewbox_height: f32,

    /// Default fill rule.
    pub fill_rule: FillRule,

    /// Default render mode.
    pub default_mode: IconRenderMode,

    /// Semantic role (for accessibility and classification).
    pub role: IconRole,

    /// Pre-computed content hash of path commands (for mesh cache key).
    pub path_hash: u64,
}
```

### 4.1 Canonical Coordinate Space

All icon paths are defined in a **normalized canonical grid**. The grid size is a property of each `IconDefinition`, not a global constant.

**Decision: No single canonical grid size.** Icons define their own viewbox. The standard Halcyon icon set uses 24×24, but the system supports any viewbox. Scaling from viewbox to render size is a uniform transform applied before tessellation.

Rationale:
- 24×24 aligns with industry standard (Material Symbols, Lucide)
- Allows importing icons from other systems without coordinate remapping
- Viewbox is metadata, not a constraint — the mesh cache (SPEC-264) handles actual render size

### 4.2 Fill Rule

```rust
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum FillRule {
    NonZero,
    EvenOdd,
}
```

Default: `NonZero`. Specified per-icon for complex shapes with holes.

### 4.3 Render Mode

```rust
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum IconRenderMode {
    /// Filled shape.
    Fill,
    /// Stroke-only outline.
    Stroke,
    /// Fill with stroke overlay.
    FillAndStroke,
}
```

Default mode is stored in the definition. Widgets may override at request time.

---

# 5. IconRegistry

## 5.1 Architecture

The registry is a **static, compile-time collection** of `IconDefinition` values. No dynamic mutation at runtime.

```rust
pub struct IconRegistry {
    icons: &'static [IconDefinition],
    index: HashMap<IconId, usize>,
}
```

## 5.2 Registration

Icons are registered via a compile-time inventory pattern:

```rust
impl IconRegistry {
    /// Build registry from a static slice of definitions.
    /// Called once at app initialization.
    pub fn new(icons: &'static [IconDefinition]) -> Self;

    /// Lookup by ID. Pure, infallible for registered icons.
    pub fn get(&self, id: IconId) -> Option<&'static IconDefinition>;

    /// Iterator over all registered icons.
    pub fn iter(&self) -> impl Iterator<Item = &'static IconDefinition>;
}
```

## 5.3 Built-In Icon Set

The initial milestone defines a minimal core set:

| IconId constant | Name | Role | Description |
|----------------|------|------|-------------|
| `ICON_CLOSE` | close | Action | × symbol |
| `ICON_CHEVRON_RIGHT` | chevron-right | Structural | > disclosure indicator |
| `ICON_CHEVRON_DOWN` | chevron-down | Structural | ∨ expanded indicator |
| `ICON_PLUS` | plus | Action | + add symbol |
| `ICON_MINUS` | minus | Action | − remove symbol |
| `ICON_CHECK` | check | Status | ✓ checkmark |
| `ICON_SEARCH` | search | Action | magnifying glass |
| `ICON_ARROW_UP` | arrow-up | Navigation | ↑ |
| `ICON_ARROW_DOWN` | arrow-down | Navigation | ↓ |
| `ICON_WARNING` | warning | Status | ⚠ triangle |

Additional icons are added incrementally. The architecture scales to hundreds without structural change.

## 5.4 Feature Gating

**Decision: No feature gating for the core set.** The core icon set is always available. Future icon packs (extended sets, domain-specific collections) MAY be feature-gated behind cargo features, but the registry API and core icons are unconditional.

Rationale:
- Core widgets (Button, TreeView, ComboBox) require icons unconditionally
- Feature-gating core icons creates conditional compilation complexity in widget code
- Extended packs are a future concern (§18)

## 5.5 Deterministic Ordering

`IconRegistry::new()` sorts entries by `IconId` at construction time. Iteration order is deterministic. No runtime insertion.

---

# 6. IconSpec (Widget Request Type)

Widgets request icon rendering through a specification struct:

```rust
#[derive(Clone, Copy)]
pub struct IconSpec {
    /// Which icon.
    pub id: IconId,

    /// Desired logical size (width and height — icons are square).
    pub size: f32,

    /// Color in linear RGBA.
    pub color: Color,

    /// Stroke width override. None = use definition default.
    pub stroke_width: Option<f32>,

    /// Render mode override. None = use definition default.
    pub mode: Option<IconRenderMode>,
}
```

`IconSpec` is the **only type widgets use** to request icons. Widgets never construct paths, compute meshes, or interact with the mesh cache.

---

# 7. Scaling Contract

## 7.1 Uniform Scaling

Icons scale uniformly. No distortion. The scale factor is:

```
scale = spec.size / definition.viewbox_width
```

Since icons are square (`viewbox_width == viewbox_height` for standard icons), this produces a uniform transform.

For non-square viewboxes (rare), the icon is scaled to fit within `spec.size × spec.size` preserving aspect ratio, centered.

## 7.2 Scale Application

Scaling is applied as a transform on the path commands before tessellation. The mesh cache (SPEC-264) sees the final logical-pixel coordinates, not the canonical viewbox coordinates. The `render_width_q` / `render_height_q` in the cache key reflect the target size.

## 7.3 No Geometry Recalculation Per Frame

Once a mesh is cached for a given icon at a given size/DPR, it is reused. Size changes produce new cache entries, not per-frame recomputation. This is enforced by SPEC-264 INV-VMP-003.

---

# 8. Layout Participation

Icons are layout participants with well-defined intrinsic sizes.

## 8.1 Intrinsic Size

An icon's intrinsic size is its `spec.size` in both dimensions (square). This is a hard constraint — icons do not stretch to fill available space.

## 8.2 Size Tokens

Following SPEC-HAL-095 §3:

| Token | Size | Primary Usage |
|-------|------|--------------|
| `IconSize::XS` | 10.0 | Ultra-compact: dense lists, indicators |
| `IconSize::S` | 12.0 | Primary: tree icons, list icons, small buttons |
| `IconSize::M` | 14.0 | Standard: toolbar buttons, menu items |
| `IconSize::L` | 16.0 | Emphasis: section headers, inspectors |
| `IconSize::XL` | 20.0 | Large: dialogs, empty states |

```rust
#[derive(Clone, Copy)]
pub enum IconSize {
    XS = 10,
    S = 12,
    M = 14,
    L = 16,
    XL = 20,
}

impl IconSize {
    pub fn to_logical_px(self) -> f32 {
        self as u32 as f32
    }
}
```

Widgets SHOULD use `IconSize` tokens. Widgets MAY use arbitrary f32 sizes for special cases.

## 8.3 Alignment

Icons support three alignment modes when composed with text:

| Mode | Behavior |
|------|----------|
| Baseline | `icon_y = text_baseline - (icon_size / 2) + optical_offset` |
| Center | Vertically centered within the containing line height |
| Leading/Trailing | Horizontally positioned at start or end of content area |

`optical_offset` is a per-definition optional correction (typically 0–2px) for icons whose visual weight is not centered in their bounding box. Default: 0.

---

# 9. Scene Integration

## 9.1 Resolution Path

When a widget provides an `IconSpec`, the resolution path is:

1. Widget calls `cx.push_icon(spec)` (or equivalent scene builder method)
2. Scene builder looks up `IconDefinition` from registry
3. Scene builder computes scaled path (viewbox → target size)
4. Scene builder emits `SceneNode::Path` with:
    - `path`: scaled path commands
    - `mesh`: `None` (mesh cache handles tessellation)
    - `color`: `spec.color`
    - `is_stroke`: derived from render mode
5. Batcher's `flatten_node()` → mesh cache lookup (SPEC-264) → PathDraw

## 9.2 No New SceneNode Variant

Icons resolve to `SceneNode::Path`. The scene graph does not need to know icons exist. This keeps the scene graph lean and avoids a proliferation of node types.

## 9.3 Scene Builder Extension

A convenience method is added to `SceneBuilder`:

```rust
impl SceneBuilder {
    pub fn push_icon(
        &mut self,
        spec: IconSpec,
        position: [f32; 2],
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );
}
```

This method:
1. Resolves `IconDefinition` from registry
2. Scales path to `spec.size`
3. Translates to `position`
4. Delegates to existing `push_path()` with appropriate color and mode

No new scene node type. No new batcher logic. No new GPU pipeline.

---

# 10. Theming & State Colors

## 10.1 Principle

Icons are **stateless at render time**. They do not store theme color internally. Color is injected by the widget layer through `IconSpec.color`.

## 10.2 Semantic Color Tokens

Following SPEC-HAL-096 and SPEC-HAL-095 §8:

| Token | Usage |
|-------|-------|
| `theme.icon.color.primary` | Default icon color |
| `theme.icon.color.secondary` | De-emphasized icons |
| `theme.icon.color.muted` | Disabled state |
| `theme.icon.color.accent` | Active/selected state |
| `theme.icon.color.success` | Checkmarks, success indicators |
| `theme.icon.color.warning` | Caution indicators |
| `theme.icon.color.error` | Error, delete, close indicators |

## 10.3 State Color Resolution

Widgets resolve icon color from interaction state before constructing `IconSpec`:

| State | Color Source |
|-------|-------------|
| Default | `theme.icon.color.primary` |
| Hover | Primary + luminance adjustment per 095 §9 |
| Active/Pressed | Primary + luminance adjustment per 095 §9 |
| Selected | `theme.icon.color.accent` |
| Disabled | `theme.icon.color.muted` |

The icon system does not manage state. The widget owns interaction state and translates it to a color before requesting the icon.

## 10.4 Cache Implication

Color is not part of the mesh cache key (SPEC-264 INV-VMP-004). A single cached mesh serves all color variants. Theme switches do not cause retessellation.

---

# 11. Semantic Role Taxonomy

```rust
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub enum IconRole {
    /// Navigation: back, forward, close, home
    Navigation,
    /// Action: add, remove, edit, save, delete
    Action,
    /// Status: warning, error, success, info
    Status,
    /// Structural: chevron, caret, disclosure, separator
    Structural,
    /// Entity: file, folder, document, image
    Entity,
    /// Indicator: busy, loading, dot, badge
    Indicator,
}
```

Roles are metadata carried by `IconDefinition`. They serve:
- Accessibility: screen readers can infer purpose
- Filtering: tooling can list icons by role
- Documentation: self-classifying icon catalog

Roles do not affect rendering.

---

# 12. Accessibility

## 12.1 Semantic Label

Every `IconDefinition` carries a `name` field (`&'static str`) that serves as the accessible label. When an icon is the sole content of a control (icon-only button), the widget MUST expose this name to the accessibility layer.

## 12.2 Scene Graph Metadata

**Decision: Deferred.** Current `PathDraw`/`PathNode` do not carry semantic metadata. Adding accessibility annotations to scene nodes is a cross-cutting concern that affects all node types, not just icons.

This SPEC defines the **data source** (IconDefinition.name, IconDefinition.role). The delivery mechanism to the accessibility tree is deferred to the accessibility SPEC (SPEC-HAL-125) when scene graph semantic annotations are designed.

## 12.3 Widget Responsibility

Until scene-level accessibility exists, widgets using icons are responsible for:
- Setting accessible name on the containing control
- Not relying on icon visibility for conveying state to assistive technology

---

# 13. Raster Icon Fallback

## 13.1 Scope

Raster icons are a fallback for cases where vector definitions are unavailable (imported bitmap icons, platform-specific glyphs).

## 13.2 SPEC-263 Compliance

Raster icons MUST follow the SPEC-263 per-texture model:

- Each raster icon variant is a separately registered texture via `register_image_rgba8_srgb()`
- No atlas (INV-IMG-007)
- Explicit lifetime: register → use → unregister
- SceneGraph stores `texture_id: u64`, never GPU references (INV-IMG-002)

## 13.3 Raster IconDefinition

```rust
pub struct RasterIconSource {
    /// DPR this raster was designed for.
    pub source_dpr: f32,

    /// Registered texture ID.
    pub texture_id: u64,

    /// Pixel dimensions of the source image.
    pub width: u32,
    pub height: u32,
}
```

A raster icon may have multiple `RasterIconSource` entries for different DPR values (1×, 2×, 3×). The system selects the closest match and scales down, never up.

## 13.4 Rendering Path

Raster icons emit `SceneNode::Image` instead of `SceneNode::Path`. The mesh cache (SPEC-264) is not involved. Color tinting of raster icons is applied via the image pipeline's `tint` field.

## 13.5 Non-Goal

Raster icons are a compatibility escape hatch, not the primary rendering path. Halcyon is vector-first. No raster-specific optimization (atlas, sprite sheets) is part of this SPEC.

---

# 14. Static vs Procedural Icons

## 14.1 Static Icons (Primary)

Static icons have path commands defined at compile time. They are the default and preferred type.

## 14.2 Procedural Icons

Procedural icons generate path commands at runtime based on parameters (e.g., a progress ring at N% fill, a dynamically sized bracket).

**Phase constraint:** Procedural icon path generation MUST complete during **Phase 3 (Rebuild)** or earlier. By the time scene build (Phase 5) executes, the `Path` must be fully resolved. No procedural generation during scene build or later phases.

This ensures:
- Constitution §4.7 compliance (layout and paint are pure)
- Mesh cache can operate on stable path data
- No per-frame path construction in the rendering hot path

## 14.3 Procedural Registration

Procedural icons are NOT registered in the static `IconRegistry`. They are widget-local and pushed via standard `push_path()`. They benefit from the mesh cache (SPEC-264) if their path commands are stable across frames.

---

# 15. Widget Integration Pattern

## 15.1 Contract

Widgets MUST NOT:
- Construct icon path geometry manually
- Store `Path` or `PathMesh` for icon shapes
- Duplicate icon geometry across widgets
- Apply icon-level scroll transforms

Widgets MUST:
- Reference icons by `IconId`
- Provide size, color, and optional overrides via `IconSpec`
- Use `cx.push_icon()` or equivalent to emit icon scene nodes
- Resolve theme color before constructing `IconSpec`

## 15.2 Affected Widgets

| Widget | Icon Usage |
|--------|-----------|
| Button | Leading/trailing icon, icon-only variant |
| MenuItem | Leading icon, shortcut indicator |
| ComboBox | Dropdown arrow |
| TreeView | Disclosure chevron, node type icon |
| Breadcrumb | Separator chevron |
| TabHeader | Tab icon, close button |
| Toolbar | Tool icons |
| SearchField | Search icon, clear button |
| StatusBar | Status indicators |
| Checkbox | Check mark |
| SpinBox | Increment/decrement arrows |

## 15.3 Migration

Since the audit confirmed zero hardcoded path geometry in widget code today, migration is additive. Widgets that currently lack icons gain them. No existing path construction needs removal.

---

# 16. Scroll Compliance

Icons enter the scene graph in **unscrolled logical coordinates** per PM-004. The icon system does not interact with scroll state. Scroll offset is applied by the batcher (same as all other scene nodes).

No icon-level scroll logic is permitted.

---

# 17. Invariants

**INV-ICO-001 — Identity by IconId**

Widgets reference icons exclusively by `IconId`. No widget may store `Path`, `PathMesh`, or `IconDefinition` references across frames.

* **Enforcement:** Structural — `IconSpec` contains only `IconId`, not path data

**INV-ICO-002 — Registry Immutability**

The `IconRegistry` is immutable after construction. No dynamic insertion, removal, or mutation of icon definitions at runtime.

* **Enforcement:** `&'static [IconDefinition]` backing store; no `&mut` API

**INV-ICO-003 — Pure Lookup**

`IconRegistry::get()` is a pure function. It performs no allocation, no tessellation, no GPU operations, and no side effects.

* **Enforcement:** Structural — returns `Option<&'static IconDefinition>`

**INV-ICO-004 — Color Injection, Not Storage**

Icons do not store theme color. Color is provided by the widget at request time via `IconSpec.color`. Theme changes do not mutate icon definitions.

* **Enforcement:** Structural — `IconDefinition` contains no color field

**INV-ICO-005 — No Widget-Level Path Construction**

No widget may construct icon path commands (`MoveTo`, `LineTo`, etc.) directly. All icon geometry originates from `IconDefinition.path` in the registry.

* **Enforcement:** Code review; debug assertions in `push_icon()` verifying IconId resolves to registered definition

**INV-ICO-006 — Uniform Scaling Only**

Icons scale uniformly. No axis-independent stretching. Aspect ratio is preserved.

* **Enforcement:** `push_icon()` computes uniform scale factor from viewbox

**INV-ICO-007 — Procedural Phase Constraint**

Procedural icon path generation completes before scene build (Phase 5). No path construction during Phase 5 or Phase 6.

* **Enforcement:** Procedural icons are resolved during widget rebuild (Phase 3); by Phase 5 all paths are immutable

**INV-ICO-008 — Raster SPEC-263 Compliance**

Raster icon textures follow SPEC-263: per-texture allocation, explicit lifetime, no atlas. `texture_id` is the only GPU reference in the scene graph.

* **Enforcement:** Raster icons use existing `register_image_rgba8_srgb()` / `unregister_image()` API

**INV-ICO-009 — Scroll Authority**

Icons enter the scene graph in unscrolled logical coordinates. No icon-level scroll transform is permitted.

* **Enforcement:** PM-004 debug assertion in batcher `add_path()` (already exists)

---

# 18. Non-Goals (Future Expansion)

- Animated icons (rotation, morphing, pulsing)
- Icon fonts / glyph-based icons
- Multi-color glyph icons
- Icon pack loading (dynamic registration from files)
- SVG import pipeline
- SDF/MSDF rendering (MSAA pipeline via SPEC-R-004 is sufficient)
- State morphing (chevron rotation is a transform, not an icon system concern)
- Density-mode automatic size switching (widgets select size tokens; icon system renders what's asked)

---

# 19. Relationship to Existing SPECs

| SPEC | Relationship |
|-----|-------------|
| SPEC-HAL-264 (Vector Mesh Pipeline) | Substrate. This SPEC produces paths; 264 caches and tessellates them |
| SPEC-HAL-095 (Iconography Rules) | Design source. Size tokens (§3), stroke weights (§4), alignment rules (§6), color model (§8) adopted. Asset pipeline (§11–12) superseded by this SPEC + 264 |
| SPEC-HAL-263 (Image Pipeline) | Raster fallback uses 263's per-texture model |
| SPEC-HAL-096 (Color System) | Semantic color tokens for icon states |
| SPEC-HAL-002 (Constitution) | Pillar C (ID-based identity), §3.4/§3.7 (no widget-owned geometry), §4.7 (pure layout/paint) |
| SPEC-R-004 (GPU Path Renderer) | Terminal renderer. Icons become PathDraw instances consumed by the path pipeline |
| SPEC-PM-004 (Geometry Authoring) | Scroll compliance — icons are unscrolled at scene entry |

---

# 20. Open Decisions Resolved

| # | Decision | Resolution | Rationale |
|---|----------|-----------|-----------|
| 5 | Canonical icon grid size | Per-definition viewbox; standard set uses 24×24 | Industry alignment; flexible for imports; viewbox is metadata not constraint |
| 6 | Static vs procedural definitions | Both supported; static is primary, procedural is widget-local with Phase 3 constraint | Covers standard icons and dynamic shapes (progress rings, badges) |
| 7 | SVG import support | Deferred (Non-Goal §18) | Build-time SVG→Path conversion is a tooling concern, not a runtime architecture concern |
| 8 | Registry static or feature-gated | Core set unconditional; extended packs may be feature-gated | Core widgets need icons unconditionally; conditional compilation in widget code is unacceptable complexity |

---

# 21. Files Impacted

**New:**
- `src/core/icon.rs` — `IconId`, `IconDefinition`, `IconRegistry`, `IconSpec`, `IconSize`, `IconRole`, `IconRenderMode`, `FillRule`, `RasterIconSource`
- `src/core/icons/` — Module directory for built-in icon definitions (one file per category or as a single catalog)

**Modified:**
- `src/core/scene/mod.rs` — Add `push_icon()` convenience method to `SceneBuilder`
- `src/core/mod.rs` — Export `icon` module

**Not Modified:**
- `src/renderer/` — No renderer changes (264 handles mesh cache; existing path pipeline handles rendering)
- `src/renderer/batcher.rs` — No batcher changes (icons resolve to standard `PathNode`)
- Widget files — Modified only when icons are integrated into individual widgets (incremental, per-widget)

---

# 22. Testing Requirements

1. **Registry lookup test** — `get(ICON_CLOSE)` returns correct definition; `get(invalid)` returns None
2. **Registry immutability test** — No mutation API exists (compile-time enforcement)
3. **IconSpec resolution test** — `push_icon()` produces correct `SceneNode::Path` with scaled coordinates
4. **Uniform scaling test** — Icon at 12px vs 24px produces proportionally scaled path, no distortion
5. **Color independence test** — Same icon with different colors produces same path hash (different scene node color)
6. **Viewbox normalization test** — Icon with 24×24 viewbox at 12px size → all coordinates halved
7. **Raster fallback test** — Raster icon emits `SceneNode::Image` with correct texture_id
8. **Scroll compliance test** — `push_icon()` output coordinates are unscrolled (PM-004)
9. **Path hash stability test** — Same icon definition produces same `path_hash` across calls
10. **Role metadata test** — Every built-in icon has a non-default `IconRole` assigned

---

# 23. Implementation Milestones

### Milestone 1 — Core Types & Registry
- Define `IconId`, `IconDefinition`, `IconRegistry`, `IconSpec`, `IconSize`, `IconRole`
- Implement registry construction and lookup
- Add `push_icon()` to SceneBuilder
- Tests 1–6, 8–9

### Milestone 2 — Built-In Icon Set
- Define 10 core icons (§5.3) as static `IconDefinition` values
- Path commands authored in 24×24 canonical space
- Test 10

### Milestone 3 — First Widget Integration
- Integrate into one widget (TreeView disclosure chevron is the natural first target)
- Validate end-to-end: IconSpec → push_icon → PathNode → mesh cache → GPU
- Visual verification at multiple DPR values

### Milestone 4 — Expand
- Integrate into remaining widgets (§15.2) incrementally
- Add icons as needed per widget requirements
- Raster fallback (test 7) if needed

---

# 24. Result

After implementation of SPEC-HAL-265:

- Every icon in Halcyon has a stable `IconId` and immutable `IconDefinition`
- Widgets request icons through `IconSpec` — never construct geometry
- Icon registry is static, pure, and compile-time
- Icons are vector-first, rendered through the existing path pipeline with mesh caching (SPEC-264)
- Color is injected at render time — theme switches are free
- Raster fallback follows SPEC-263 per-texture model
- Semantic roles enable future accessibility integration
- No new scene node types, no new GPU pipelines, no new shaders
- Architecture scales to hundreds of icons without structural change

This is the identity and semantic layer for all iconography in Halcyon.
