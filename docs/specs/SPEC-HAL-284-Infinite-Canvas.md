# SPEC-HAL-284: **Infinite Canvas — Pannable Zoomable Coordinate Space**

### *Foundation Primitive for Node Editors, Whiteboards, Diagram Tools, and Map-Style UIs*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-328 (Keyboard Authority, Focus, and Capture)
* SPEC-HAL-343 (Spatial Subsystem Reconciliation)
* SPEC-PM-006 (Rendering Pre-Flight Checklist)

---

# 1. Purpose

Define a pannable, zoomable, infinite 2D coordinate space widget. This is the base primitive that node graph editors, whiteboards, diagram tools, mind maps, circuit designers, and map-style UIs all sit on top of.

The canvas is NOT a node editor. It is a coordinate substrate. It defines viewport transform behavior, grid/background rendering, and coordinate conversion. It does not own canvas items, selection, graph data, domain objects, editor commands, or hidden viewport authority.

Consumers provide a read-only `CanvasProjection` (SPEC-HAL-285) for paint, hit testing, culling, minimap bounds, and zoom-to-fit. The canvas emits intents; the consumer decides what those intents mean.

Under SPEC-HAL-343, `CanvasProjection` remains the compatibility source for existing canvas consumers. A spatial adapter may losslessly bridge `CanvasProjection`, `CanvasEntityId`, `CanvasHitRegion`, `CanvasPaintDescriptor`, and `CanvasIntent` into domain-neutral `Spatial*` projection records, but that bridge must preserve the public `Canvas*` behavior and must not move selection, graph, timeline, plot, or document mutation authority into the canvas substrate.

---

# 2. Coordinate Model

```rust
/// World-space point (infinite canvas coordinates).
#[derive(Debug, Clone, Copy)]
pub struct WorldPoint {
    pub x: f64,
    pub y: f64,
}

/// View transform: maps world space → screen space.
#[derive(Debug, Clone, Copy)]
pub struct CanvasTransform {
    /// World-space coordinate mapped to the viewport origin.
    pub offset_x: f64,
    pub offset_y: f64,
    /// Zoom factor (1.0 = 100%, 2.0 = 200%, 0.5 = 50%).
    pub zoom: f64,
}

pub struct CanvasViewportState {
    pub transform: CanvasTransform,
}
```

### Invariants

**INV-284-C1: World → Screen**
```
screen_x = (world_x - offset_x) * zoom
screen_y = (world_y - offset_y) * zoom
```

**INV-284-C2: Screen → World**
```
world_x = screen_x / zoom + offset_x
world_y = screen_y / zoom + offset_y
```

**INV-284-C3: f64 Precision**
World coordinates use `f64` for precision at extreme zoom levels. Screen coordinates are `f32` (passed to the renderer as logical coordinates).

**INV-284-C4: Zoom Range**
Zoom is clamped to `[min_zoom, max_zoom]`. Default: `[0.1, 10.0]` (10% to 1000%). Configurable via builder.

**INV-284-C5: Zoom Center**
Zoom always centers on the cursor position (or viewport center for keyboard zoom). The world point under the cursor stays fixed.

---

# 3. Widget Architecture

```rust
pub struct InfiniteCanvas {
    id: WidgetInternalId,
    /// Zoom limits.
    min_zoom: f64,
    max_zoom: f64,
    /// Grid configuration.
    grid: CanvasGrid,
    /// Background style.
    background: CanvasBackground,
    /// Transient pointer mechanics only. No persistent viewport authority.
    gesture: CanvasGestureState,
    focused: bool,
    in_capture: bool,
    /// Viewport rect (screen space, set by layout).
    viewport: RectF,
    /// Outbound intents for the owning consumer.
    intents: Vec<CanvasIntent>,
}
```

**INV-284-A1: Canvas Owns Only Coordinate Substrate**
`InfiniteCanvas` may own configuration, grid/background style, viewport geometry, and transient pointer mechanics required to produce viewport intents. It MUST NOT own domain entities, projected items, selection state, graph state, document state, or hidden persistent viewport state.

**INV-284-A2: Projection Is External**
Every paint, hit-test, culling, minimap, and zoom-to-fit operation that refers to content uses an externally supplied `CanvasProjection`.

**INV-284-A2A: Spatial Bridge Is Compatibility Only**
When a `CanvasProjection` is bridged into a `SpatialProjection`, the bridge is read-only and lossless over canvas identity, paint descriptors, hit regions, draw order, viewport state, and intent translation. The bridge may expose domain-neutral spatial records for shared paint, hit-test, route, guide, overlay, and policy helpers. It MUST NOT replace `CanvasProjection` as the canvas compatibility API, delete `Canvas*` concepts, or acquire domain mutation authority.

**INV-284-A2B: Spatial Viewport Conversion Is Rect-Aware**
`CanvasTransform` remains an origin-anchored, y-down, uniform-scale
compatibility transform. Its spatial orientation is explicitly
`IncreasingPixels` for both X and Y. Given a finite positive `zoom` and the
current finite nondegenerate canvas viewport rectangle, the equivalent
center-anchored spatial camera is:

```text
world_center.x        = offset_x + viewport.width  / (2 * zoom)
world_center.y        = offset_y + viewport.height / (2 * zoom)
pixels_per_world_unit = zoom
```

The materialized spatial transform includes the viewport rectangle origin, so
its global pixel result is the rectangle origin plus the existing local Canvas
formula in INV-284-C1. For the same rectangle and the
`IncreasingPixels`/`IncreasingPixels` orientation, the inverse is:

```text
offset_x = world_center.x - viewport.width  / (2 * pixels_per_world_unit)
offset_y = world_center.y - viewport.height / (2 * pixels_per_world_unit)
zoom     = pixels_per_world_unit
```

A compatibility bridge MUST either retain `CanvasViewportState` unchanged
until the caller-owned rectangle is available or receive that rectangle when
it materializes the spatial camera and transform. It MUST NOT claim a lossless
unconditional representation conversion before layout geometry is known.
Canvas zoom clamping remains the configurable InfiniteCanvas policy in
INV-284-C4; it does not become a global `SpatialViewportState` limit.

**INV-284-A3: Viewport State Is Explicit**
`CanvasViewportState` is passed in by the owner. In node-editor usage, it belongs to `NodeEditorSession`. A standalone canvas adapter may store it, but the substrate protocol still treats it as explicit owner state.

### Grid

```rust
#[derive(Debug, Clone)]
pub struct CanvasGrid {
    pub enabled: bool,
    pub base_spacing: f64,       // world-space grid spacing at zoom=1.0
    pub subdivisions: u32,       // minor grid lines per major line (default 5)
    pub snap_enabled: bool,      // exposes snap helper; owner decides whether to apply
    pub color_major: Color,
    pub color_minor: Color,
}
```

**INV-284-G1: Adaptive Grid**
Grid spacing adapts to zoom level. At high zoom, fine grid is visible. At low zoom, only major grid lines show. The grid never becomes too dense or too sparse.

**INV-284-G2: Grid Is Cosmetic**
Grid lines are rendered but NOT part of the scene graph. They are painted directly in the canvas's `paint()` pass at the lowest z-order.

**INV-284-G3: Snap Is Advisory**
Grid snapping produces snapped coordinates or deltas for the consumer. The canvas MUST NOT apply snap by mutating projected content.

### Background

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CanvasBackground {
    Solid,        // flat color from theme
    Dots,         // dot pattern at grid intersections
    Lines,        // grid lines
    CrossDots,    // dots with crosshair at grid intersections
}
```

---

# 4. Interaction

### Pan

**INV-284-P1: Middle-Click Pan**
Middle mouse button drag pans the canvas. `PointerDown(Middle)` -> record transient pan anchor -> `PointerMove` -> emit viewport-change intent with updated offset -> `PointerUp(Middle)` -> clear transient pan anchor.

**INV-284-P2: Space+Drag Pan**
Space key held + left-click drag pans. Same mechanics as middle-click.

**INV-284-P3: Two-Finger Pan (Future)**
Touch: two-finger drag pans. Deferred to touch SPEC.

### Zoom

**INV-284-Z1: Ctrl+Scroll Zoom**
`Ctrl + ScrollWheel` zooms in/out centered on cursor. Zoom factor changes multiplicatively (e.g., ×1.1 per scroll tick).

**INV-284-Z2: Pinch Zoom (Future)**
Touch: pinch gesture zooms. Deferred to touch SPEC.

**INV-284-Z3: Keyboard Zoom**
`+` / `=` zooms in, `-` zooms out (centered on viewport center). `0` or `Home` resets to zoom=1.0 and offset=(0,0).

**INV-284-Z4: Zoom-to-Fit**
`F` key or API call `zoom_to_fit(&CanvasProjection)`: calculates the projection's world bounds, then sets zoom and offset to fit those bounds in the viewport with padding.

**INV-284-Z5: Smooth Zoom Animation**
Zoom changes animate with a short ease-out (50-100ms). Not instant jump.

### Selection Rectangle

**INV-284-S1: Rubber Band**
Left-click drag on empty canvas space draws a selection rectangle. The canvas reports the live and committed rectangle in world coordinates. The consumer decides what entities intersect and how selection changes.

### Pointer Passthrough

**INV-284-PT1: Item Hit Testing**
Pointer events are transformed to world coordinates and tested against the supplied `CanvasProjection` (SPEC-285). The canvas emits a `CanvasIntent::Hit` for the topmost projected entity. If no entity is hit, the canvas handles substrate interactions or emits empty-space intents.

---

# 5. Minimap

**INV-284-MM1: Optional Minimap**
A small inset rectangle (configurable corner, default bottom-right) showing the full world extent with a highlighted viewport indicator.

**INV-284-MM2: Minimap Interaction**
Click/drag on the minimap repositions the viewport.

**INV-284-MM3: Minimap Rendering**
The minimap renders simplified projection bounds (filled rectangles, no text). It does NOT re-render the full scene and does not require domain objects.

---

# 6. Rendering

**INV-284-R1: Transform Integration**
The canvas modifies the `view_proj` matrix (or applies a pre-transform) to convert world-space coordinates to screen-space before passing to the existing render pipeline. All existing paint primitives (`push_rect`, `push_text_with_backgrounds`, path pipeline) work in world-space coordinates once the transform is applied.

**INV-284-R2: Viewport Culling**
Only projected items whose world-space bounds intersect the visible viewport are painted. Projected items fully outside the viewport are skipped entirely. This is critical for performance with thousands of items.

**INV-284-R3: Level-of-Detail**
At low zoom levels (zoomed out far), items below a screen-space size threshold render as simplified representations (colored rectangles, no text). This prevents rendering thousands of unreadable text labels.

**INV-284-R4: Pipeline Safety**
The canvas background and grid use `push_rect` only. Projected content paints through `CanvasPaintDescriptor` / projection paint routines without mutating authoritative state. Per SPEC-PM-006.

---

# 7. API

### Builders

```rust
InfiniteCanvas::new()
    .with_grid(CanvasGrid { enabled: true, base_spacing: 20.0, .. })
    .with_background(CanvasBackground::Dots)
    .with_zoom_range(0.1, 10.0)
```

### Programmatic Control

```rust
canvas.pan_to(&mut viewport, WorldPoint::new(100.0, 200.0));
canvas.zoom_to(&mut viewport, 2.0);  // 200%
canvas.zoom_to_fit(&mut viewport, &projection);
canvas.reset_view(&mut viewport);   // zoom=1.0, offset=(0,0)
canvas.world_to_screen(&viewport, world_point) -> (f32, f32);
canvas.screen_to_world(&viewport, screen_x, screen_y) -> WorldPoint;
canvas.paint_projection(&viewport, &projection, cx);
canvas.hit_test(&viewport, &projection, world_point) -> Option<CanvasHit>;
canvas.take_intents() -> Vec<CanvasIntent>;
```

---

# 8. Acceptance Criteria

- Middle-click drag pans the canvas smoothly
- Ctrl+Scroll zooms centered on cursor
- Zoom stays within configured range
- Grid adapts to zoom level (fine at high zoom, coarse at low zoom)
- `zoom_to_fit(&CanvasProjection)` frames projected content in the viewport
- Rubber band emits world-space rectangle intents
- Pointer hit testing returns projected entity IDs in world coordinates
- Viewport culling: projected items outside viewport are not painted
- Minimap shows viewport position (optional)
- Smooth zoom animation (not instant jump)
- No API exists for canvas-owned content, selection, deletion, duplication, or domain mutation

---

# 9. Non-Goals

- 3D perspective (this is 2D only)
- Rotation (pan + zoom only, no canvas rotation)
- Tile-based rendering (future optimization for very large canvases)
- Multi-canvas synchronization (application concern)
- Touch gestures (deferred to touch SPEC)
- Domain model ownership
- Selection ownership
- Undo/redo
