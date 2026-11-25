# SPEC-HAL-264 — Vector Mesh Pipeline & Cache Architecture

### *Centralized Tessellation Cache for GPU-Ready Vector Geometry*

**Author:** JC
**Status:** Proposed
**Date:** 2026-02-26
**Depends on:** SPEC-HAL-002 (Constitution), SPEC-HAL-005 (Scene Graph), SPEC-HAL-027 (Batching), SPEC-R-004 (GPU Path Renderer), SPEC-HAL-024 (Coordinate Model), SPEC-HAL-030 (Presentation Policy), SPEC-HAL-084 (RenderGraph v2), SPEC-PM-004 (Geometry Authoring Space)
**Supersedes:** None (new subsystem)
**Consumed by:** SPEC-HAL-265 (Iconography System, forthcoming)

---

# 1. Objective

Introduce a **centralized, renderer-owned mesh cache** that eliminates per-frame tessellation for vector paths whose geometry is stable across frames.

Today, every `PathNode` without a pre-supplied `PathMesh` is tessellated during `flatten_node()` in the batcher — every frame, unconditionally. This is correct but wasteful. For repeated shapes (icons, disclosure triangles, decorative elements), identical path commands produce identical meshes frame after frame.

This SPEC defines:

* A content-addressed mesh cache owned by the renderer
* Cache key structure incorporating all render-affecting parameters
* Invalidation contract
* Pixel-snapping strategy for cached vector geometry
* Phase placement of tessellation and cache operations
* Stroke scaling policy
* Performance invariants

This pipeline is **general-purpose**. It serves any consumer of vector path geometry — icons, decorations, procedural shapes, custom widgets. It has no knowledge of icon identity, semantics, or theming.

---

# 2. Architectural Position

The Vector Mesh Cache sits between the **Scene Graph** (which produces `PathNode`) and the **Batcher** (which consumes `PathDraw`).

```
Widget Layer
    ↓ push_path() / push_path_with_mesh()
Scene Graph (PathNode)
    ↓ flatten_node()
  ┌──────────────────────┐
  │  Vector Mesh Cache   │  ← NEW (this SPEC)
  │  (renderer-owned)    │
  └──────────────────────┘
    ↓ PathMesh (cached or freshly tessellated)
Batcher (PathDraw)
    ↓
GPU Path Pipeline (SPEC-R-004)
```

The cache is **not** a scene graph feature. It is a **renderer optimization** that is invisible to widgets and the scene builder API.

---

# 3. Cache Ownership Model

## 3.1 Authority

The mesh cache is **renderer-owned**. It lives inside the renderer alongside `image_textures` (SPEC-HAL-263). No external module may store, evict, or invalidate cache entries.

```rust
// Conceptual placement within WgpuRenderer
pub struct WgpuRenderer {
    // ... existing fields ...
    image_textures: HashMap<u64, ImageTextureEntry>,
    mesh_cache: MeshCache,  // NEW
}
```

## 3.2 Lifetime

Cache entries persist across frames until explicitly invalidated or evicted. The cache is **not** ephemeral — this is the entire point. Meshes survive frame boundaries.

This does NOT violate Constitution §3.9 (ephemeral layout structures). Layout structures are frame-local geometry computations (positions, sizes, constraints). Cached meshes are **renderer resources** — pre-computed GPU-ready vertex/index data, analogous to registered textures. They carry no layout semantics and are not consumed by the layout phase.

## 3.3 Device Loss

On device loss: `mesh_cache.clear()`. All entries dropped. Meshes are reconstructed on demand from path commands on subsequent frames. No caller re-registration required (unlike textures, which require explicit re-registration per SPEC-HAL-263).

---

# 4. Cache Key

A cached mesh is addressed by a **MeshCacheKey** that captures every parameter affecting tessellation output:

```rust
#[derive(Clone, Hash, Eq, PartialEq)]
pub struct MeshCacheKey {
    /// Content hash of the path commands (MoveTo, LineTo, etc.)
    pub path_hash: u64,

    /// Logical size the path is rendered at, quantized to 0.5px steps.
    /// This is the bounding dimension of the target rect, not the
    /// canonical coordinate space of the path definition.
    pub render_width_q: u32,
    pub render_height_q: u32,

    /// Stroke width in logical pixels, quantized to 0.25px steps.
    /// Zero for fill-only paths.
    pub stroke_width_q: u32,

    /// Fill mode: true = fill, false = stroke-only.
    pub is_fill: bool,

    /// Device pixel ratio, quantized to 0.125 steps.
    /// Different DPR produces different pixel-snapped meshes.
    pub dpr_q: u32,

    /// Pixel snap mode.
    pub snap_mode: PixelSnapMode,
}
```

### 4.1 Quantization

Floating-point parameters are quantized to prevent cache thrashing from insignificant float differences:

| Parameter | Quantization | Encoding |
|-----------|-------------|----------|
| Render size | 0.5 logical px | `(size * 2.0).round() as u32` |
| Stroke width | 0.25 logical px | `(width * 4.0).round() as u32` |
| Device pixel ratio | 0.125 | `(dpr * 8.0).round() as u32` |

### 4.2 Path Content Hash

The `path_hash` is computed from the path command sequence using a deterministic hash (e.g., `rustc_hash::FxHasher` or `ahash`). Two paths with identical command sequences produce identical hashes. The hash is computed once at path construction time, not per-frame.

---

# 5. Cache Entry

```rust
pub struct MeshCacheEntry {
    /// The tessellated, pixel-snapped mesh.
    pub mesh: PathMesh,

    /// Frame number of last access (for LRU eviction).
    pub last_used_frame: u64,
}
```

The `PathMesh` struct is unchanged from the existing definition:

```rust
pub struct PathMesh {
    pub vertices: Vec<[f32; 2]>,
    pub indices: Vec<u32>,
    pub bounds: RectF,
}
```

---

# 6. Cache Operations

## 6.1 Lookup

```rust
impl MeshCache {
    pub fn get(&mut self, key: &MeshCacheKey, current_frame: u64) -> Option<&PathMesh>;
}
```

On hit: updates `last_used_frame`, returns reference to cached mesh.
On miss: returns `None`. Caller tessellates and inserts.

## 6.2 Insert

```rust
impl MeshCache {
    pub fn insert(&mut self, key: MeshCacheKey, mesh: PathMesh, current_frame: u64);
}
```

Stores the entry. If an entry with the same key exists, it is replaced (idempotent).

## 6.3 Eviction

LRU-based. After each frame, entries not accessed for N frames (configurable, default 120 = ~2 seconds at 60fps) are eligible for eviction.

Eviction runs once per frame, after GPU submission, bounded to a maximum number of evictions per frame to avoid latency spikes.

```rust
impl MeshCache {
    pub fn evict_stale(&mut self, current_frame: u64, max_age_frames: u64, max_evictions: usize);
}
```

## 6.4 Clear

```rust
impl MeshCache {
    pub fn clear(&mut self);
}
```

Drops all entries. Used on device loss and DPI change.

---

# 7. Integration with Batcher

The cache integrates at the existing tessellation decision point in `flatten_node()`.

**Current flow** (batcher.rs ~line 1507):
```rust
let mesh = path_node.mesh.clone().unwrap_or_else(|| path_node.path.to_mesh());
```

**New flow:**
```rust
let mesh = if let Some(mesh) = path_node.mesh.clone() {
    // Widget provided pre-computed mesh — use as-is (existing path)
    mesh
} else {
    // Check cache
    let key = MeshCacheKey::from_path_node(path_node, current_dpr, snap_mode);
    if let Some(cached) = mesh_cache.get(&key, current_frame) {
        cached.clone()
    } else {
        // Tessellate, snap, cache, return
        let mesh = tessellate_and_snap(&path_node.path, &key);
        mesh_cache.insert(key, mesh.clone(), current_frame);
        mesh
    }
};
```

**Critical:** `push_path_with_mesh()` remains the escape hatch. If a widget provides its own mesh, the cache is bypassed entirely. This preserves backward compatibility and allows widgets to own their tessellation when needed.

---

# 8. Pixel Snapping

## 8.1 Problem

Vector paths at small sizes (10–24 logical pixels) suffer from sub-pixel blur when strokes or edges do not align to the physical pixel grid. This is especially visible on 1px strokes at 1× DPI.

## 8.2 Strategy

Pixel snapping is applied **at cache insertion time** — to the tessellated vertices, not during rendering. This means:

1. Path is tessellated in logical coordinates (existing `Path::to_mesh()`)
2. Vertices are transformed to physical pixel space: `v_physical = v_logical × DPR`
3. Vertices are snapped per the active `PixelSnapMode`
4. Vertices are transformed back to logical space: `v_logical = v_physical / DPR`
5. Snapped mesh is stored in cache

This approach:
- Produces crisp results without shader-side snapping
- Works with the existing instanced rendering model (PathInstance transform is additive)
- Is DPR-aware (different DPR = different cache entry = different snap grid)
- Costs nothing at render time

## 8.3 Snap Modes

```rust
#[derive(Clone, Copy, Hash, Eq, PartialEq)]
pub enum PixelSnapMode {
    /// No snapping. Continuous floating-point coordinates.
    None,

    /// Snap vertices to nearest physical pixel center.
    /// Best for filled shapes at small sizes.
    Center,

    /// Snap vertices to nearest physical pixel edge (half-pixel offset).
    /// Best for 1px strokes — centers the stroke on a full pixel.
    Edge,
}
```

Default mode: `Center` for fills, `Edge` for strokes.

## 8.4 Snap Application

Snapping is per-vertex. The algorithm:

```
For PixelSnapMode::Center:
    x_snapped = round(x_physical)
    y_snapped = round(y_physical)

For PixelSnapMode::Edge:
    x_snapped = floor(x_physical) + 0.5
    y_snapped = floor(y_physical) + 0.5
```

Bounds are recomputed after snapping.

---

# 9. Stroke Scaling Policy

## 9.1 Decision

Stroke width is **constant in logical pixels**. It does not scale proportionally with icon or shape size.

A 1.0px stroke remains 1.0px regardless of whether the containing shape is rendered at 12px or 48px logical.

## 9.2 Rationale

- Proportional stroke scaling produces visually inconsistent weight across sizes
- Constant stroke preserves the optical weight established by the designer
- Matches the behavior of professional icon systems (SF Symbols, Material Symbols)
- Aligns with SPEC-HAL-095 stroke weight table (1.0px at 10–12px, 1.25px at 14–16px, 1.5px at 20px)

## 9.3 Implication

The stroke width is part of the cache key. If a path is used at the same size but with different stroke widths, they are separate cache entries. This is intentional and correct.

---

# 10. Color-Space Contract

## 10.1 Decision

Color at the scene boundary is **linear RGBA, non-premultiplied**. This is already the established contract (scene/mod.rs binding law, lines 221–231). This SPEC does not change it.

## 10.2 Mesh Cache Implication

The mesh cache stores **geometry only** — no color. Color is applied per-instance via `PathInstance.color` at render time. This means:

- Same mesh serves any color
- Color changes do not invalidate cache
- Theme switches do not invalidate cache
- Color-space correctness is enforced by the existing path pipeline, not by this cache

---

# 11. Tessellation Precision

## 11.1 Flatten Tolerance

The existing `FLATTEN_TOLERANCE = 0.25` logical pixels (Path::flatten()) is retained. This controls adaptive subdivision of Bézier curves into line segments.

## 11.2 Physical-Pixel Tolerance

At high DPR, 0.25 logical pixels may be multiple physical pixels. The cache key includes DPR, so higher-DPR entries are tessellated at the same logical tolerance but snapped to a finer physical grid. This produces visually sharper results at high DPR without increasing the tolerance parameter.

## 11.3 Non-Goal

Sub-pixel anti-aliasing of vector edges is handled by the GPU MSAA pipeline (SPEC-R-004), not by the tessellator. The mesh cache does not interact with MSAA configuration.

---

# 12. Phase Placement

Tessellation and cache lookup occur during **Scene Build → Batcher Flattening** (Phase 5 in the frame model).

This is correct because:
- Phase 5 is read-only with respect to widget state and AppState (Constitution §4.7)
- Tessellation is a pure function of path commands + render parameters
- Cache mutation (insert/evict) is internal to the renderer and does not affect widget state
- No layout data is read or written

Cache eviction runs **after Phase 6** (GPU submission), as a post-frame maintenance step.

---

# 13. Invariants

**INV-VMP-001 — Renderer Ownership**

The mesh cache is exclusively owned by the renderer. No widget, scene builder, layout engine, or external module may read, write, evict, or reference cache entries.

* **Enforcement:** Structural — `MeshCache` is a private field of `WgpuRenderer`

**INV-VMP-002 — Content-Addressed Identity**

Two `MeshCacheKey` values are equal if and only if they produce identical tessellation output. The cache MUST NOT produce different meshes for equal keys, and MUST NOT produce equal meshes for semantically different keys (within quantization tolerance).

* **Enforcement:** Hash/Eq derivation on `MeshCacheKey`; quantization functions are pure

**INV-VMP-003 — No Per-Frame Tessellation of Cached Paths**

If a cache entry exists for a given key, `to_mesh()` MUST NOT be called. The cached mesh MUST be used.

* **Enforcement:** Batcher integration point checks cache before tessellating

**INV-VMP-004 — Color Independence**

The mesh cache stores geometry only. No color, tint, opacity, or theme data is part of the cache key or cache entry.

* **Enforcement:** Structural — `MeshCacheEntry` contains only `PathMesh` and metadata

**INV-VMP-005 — Pixel Snap at Insertion**

Pixel snapping is applied once, at cache insertion time. No snap correction occurs during rendering.

* **Enforcement:** `tessellate_and_snap()` is the only code path that produces snapped meshes; renderer does not re-snap

**INV-VMP-006 — DPR Isolation**

Cache entries for different device pixel ratios are distinct. A DPR change does not silently serve meshes snapped for a different grid.

* **Enforcement:** `dpr_q` is part of `MeshCacheKey`

**INV-VMP-007 — No Execute-Phase Tessellation**

No tessellation, cache lookup, or cache insertion may occur during the GPU execute phase. All mesh resolution completes before execute begins.

* **Enforcement:** Phase ordering — tessellation occurs during batcher flattening (compile), not during draw call encoding (execute)

**INV-VMP-008 — Device Loss Recovery**

On device loss, the cache MUST be cleared. Meshes are reconstructed on demand in subsequent frames. No stale geometry may survive device loss.

* **Enforcement:** `device_lost()` handler calls `mesh_cache.clear()`

**INV-VMP-009 — Backward Compatibility**

`push_path_with_mesh()` continues to bypass the cache entirely. Widgets that supply their own mesh are unaffected by this SPEC.

* **Enforcement:** Structural — `path_node.mesh.is_some()` check precedes cache lookup

---

# 14. Scroll Compliance

Cached meshes are authored in **unscrolled logical coordinates**, consistent with PM-004. Scroll offset is applied by the batcher via `PathInstance.transform`, exactly as today. The cache does not interact with scroll state.

---

# 15. Capacity & Performance

## 15.1 Expected Cache Population

A typical Halcyon application with a full widget library uses 20–50 distinct icon shapes × 2–3 size variants × 1–2 DPR values = **40–300 cache entries** at steady state. Each entry is small (a few hundred vertices + indices). Total memory: low single-digit MB.

## 15.2 Eviction Defaults

| Parameter | Default | Rationale |
|-----------|---------|-----------|
| Max age (frames) | 120 | ~2 seconds at 60fps; shapes not visible for 2s are likely off-screen |
| Max evictions per frame | 16 | Prevents latency spike from mass eviction |

## 15.3 Diagnostic

Debug builds should expose:
- Cache hit/miss ratio per frame
- Total entries
- Total vertex/index memory
- Evictions per frame

---

# 16. Files Impacted

**New:**
- `src/renderer/mesh_cache.rs` — `MeshCache`, `MeshCacheKey`, `MeshCacheEntry`, `PixelSnapMode`

**Modified:**
- `src/renderer/wgpu_renderer.rs` — Add `mesh_cache: MeshCache` field; pass to batcher; clear on device loss
- `src/renderer/batcher.rs` — `flatten_node()` integration point: cache lookup before tessellation
- `src/renderer/path.rs` — Add `content_hash()` method to `Path`; add `tessellate_and_snap()` function

**Not Modified:**
- `src/core/scene/mod.rs` — No scene graph changes
- `src/renderer/pipelines/path_pipeline.rs` — No GPU pipeline changes
- `src/renderer/shaders/path.wgsl` — No shader changes

---

# 17. Testing Requirements

1. **Cache hit test** — Same path + same key → same mesh returned, `to_mesh()` not called
2. **Cache miss test** — Different key → fresh tessellation
3. **DPR isolation test** — Same path at DPR 1.0 vs 2.0 → different cache entries
4. **Pixel snap correctness test** — Verify snapped vertices align to physical pixel grid
5. **Eviction test** — Entry not accessed for N+1 frames → evicted
6. **Device loss test** — `clear()` empties all entries; subsequent access tessellates fresh
7. **Color independence test** — Same path with different colors → same cache entry (color not in key)
8. **push_path_with_mesh bypass test** — Widget-supplied mesh not cached, not evicted
9. **Quantization stability test** — Nearby float values produce same quantized key
10. **PM-004 compliance test** — Cached mesh coordinates are unscrolled

---

# 18. Non-Goals

- Icon identity, semantics, or theming (→ SPEC-HAL-265)
- SVG parsing or import
- Animated path morphing
- GPU-side tessellation (compute shader)
- Texture atlas for rasterized vector output
- SDF/MSDF icon rendering (existing MSAA pipeline is sufficient)
- Mesh sharing across renderers/surfaces (each renderer owns its cache)

---

# 19. Relationship to Existing SPECs

| SPEC | Relationship |
|-----|-------------|
| SPEC-R-004 (GPU Path Renderer) | Unchanged. This SPEC caches the *input* to the path pipeline, not the pipeline itself |
| SPEC-HAL-263 (Image Pipeline) | Parallel pattern. Mesh cache follows same renderer-owned, explicit-lifetime model as `image_textures` |
| SPEC-HAL-095 (Iconography Rules) | 095 assumes SDF/atlas approach. This SPEC provides the actual rendering substrate. 095's pixel grid rules (§2), stroke weights (§4), and size tokens (§3) remain valid design inputs for SPEC-HAL-265 |
| SPEC-PM-004 (Geometry Authoring) | Fully compliant. Cached meshes are unscrolled. Scroll applied by batcher transform |
| SPEC-HAL-002 (Constitution) | §3.9 does not apply — cached meshes are renderer resources, not layout structures. §4.10 satisfied — cache is read-only during paint |

---

# 20. Open Decisions Resolved

| # | Decision | Resolution | Rationale |
|---|----------|-----------|-----------|
| 1 | Stroke scaling policy | Constant in logical pixels | Matches professional icon systems; preserves optical weight across sizes |
| 2 | Pixel snapping strategy | Snap at cache insertion; Center for fills, Edge for strokes | One-time cost; no render-time overhead; DPR-aware via separate cache entries |
| 3 | Color-space at scene boundary | Linear RGBA, non-premultiplied (unchanged) | Already established; premultiplication in shader only |
| 4 | Tessellation precision tolerance | Retain FLATTEN_TOLERANCE = 0.25 logical px | Proven adequate; DPR-specific snapping handles high-density displays |

---

# 21. Result

After implementation of SPEC-HAL-264:

- Repeated vector shapes (icons, decorations, structural glyphs) are tessellated once and cached
- Cache is content-addressed, DPR-aware, and pixel-snapped
- No widget API changes required
- No scene graph changes required
- No shader changes required
- Path pipeline performance improves from O(N×shapes×frames) tessellation to O(N×shapes) amortized
- Foundation is laid for SPEC-HAL-265 (Iconography System) to define icon identity and semantics on top of this cache

This is the rendering substrate for all vector geometry reuse in Halcyon.
