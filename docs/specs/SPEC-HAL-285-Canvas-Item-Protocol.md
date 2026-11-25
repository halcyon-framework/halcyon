# SPEC-HAL-285: **Canvas Projection Protocol**

### *Read-Only Projected Elements on an Infinite Canvas*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)

---

# 1. Purpose

Define the read-only projection protocol for anything displayed on an
InfiniteCanvas.

The canvas does not own items. It does not own selection. It does not move,
delete, duplicate, serialize, or mutate projected content.

Every projected item is derived from an external authority:

* document/domain authority, such as `NodeGraphDocument`
* editor/session authority, such as `NodeEditorSession`
* application-owned non-node models for whiteboards or diagrams

The projection is disposable view data for paint, hit testing, culling, and
zoom-to-fit.

---

# 2. Authority Boundary

**INV-285-A1: Projection Is Not Authority**
`CanvasProjection` is never the source of truth. It is rebuilt from external
authorities and may be discarded at any frame boundary.

**INV-285-A2: No Mutable Item API**
Projected items MUST NOT expose `set_position`, `set_selected`,
`set_z_order`, `delete`, `duplicate`, or any equivalent mutator.

**INV-285-A3: No Selection Storage**
Selection belongs to the consumer's editor/session authority. The projection
may expose whether an entity is selected as already-derived visual state, but
it cannot change that state.

**INV-285-A4: No Domain Object Storage**
The canvas and projection MUST NOT store boxed domain objects such as
`Box<dyn CanvasItem>`. They store plain projection records only.

---

# 3. Entity Identity

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CanvasEntityId {
    Node(u64),
    Port { node_id: u64, port_id: u32, direction: PortDirection },
    Wire(u64),
    Frame(u64),
    Annotation(u64),
    Custom(u64),
}
```

**INV-285-ID1: Stable Semantic ID**
Projected entity IDs are stable semantic IDs from the owning authority, not
widget IDs and not insertion indices.

**INV-285-ID2: Unique Per Projection**
Each `CanvasEntityId` appears at most once in a single `CanvasProjection`.

---

# 4. Geometry

```rust
#[derive(Debug, Clone, Copy)]
pub struct WorldRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

impl WorldRect {
    pub fn contains(&self, p: WorldPoint) -> bool {
        p.x >= self.x && p.x <= self.x + self.width
            && p.y >= self.y && p.y <= self.y + self.height
    }

    pub fn intersects(&self, other: &WorldRect) -> bool {
        self.x < other.x + other.width && self.x + self.width > other.x
            && self.y < other.y + other.height && self.y + self.height > other.y
    }
}
```

```rust
#[derive(Debug, Clone)]
pub struct ProjectedCanvasItem {
    pub id: CanvasEntityId,
    pub bounds: WorldRect,
    pub z_order: i32,
    pub hit_regions: Vec<CanvasHitRegion>,
    pub paint: CanvasPaintDescriptor,
}

#[derive(Debug, Clone)]
pub enum CanvasHitRegion {
    Rect { id: CanvasEntityId, rect: WorldRect, priority: HitPriority },
    Circle { id: CanvasEntityId, center: WorldPoint, radius: f64, priority: HitPriority },
    Path { id: CanvasEntityId, path: Path, tolerance: f64, priority: HitPriority },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum HitPriority {
    Background,
    Frame,
    Wire,
    Node,
    Port,
    Overlay,
}
```

**INV-285-G1: World-Space Geometry**
All projection geometry is expressed in world coordinates. Screen-space
conversion is performed only by the canvas transform.

**INV-285-G2: Derived Bounds**
`bounds` is derived from the authoritative model plus editor/session state.
Moving an item requires an intent or transaction against that authority.

---

# 5. Paint Context

```rust
pub struct CanvasPaintContext<'a> {
    pub scene: &'a mut SceneGraph,
    pub text_engine: &'a TextEngine,
    pub theme: &'a Theme,
    pub zoom: f64,
    pub viewport: WorldRect,
}
```

**INV-285-PC1: LOD Access**
Projected paint code may read `cx.zoom` to choose level of detail.

**INV-285-PC2: Viewport Awareness**
Projected paint code may read `cx.viewport` for internal culling.

**INV-285-PC3: Paint Is Pure Projection**
Painting a projected item MUST NOT mutate document state, session state, the
projection, or the widget tree.

---

# 6. CanvasProjection

```rust
pub struct CanvasProjection {
    revision: ProjectionRevision,
    items: Vec<ProjectedCanvasItem>,
}

impl CanvasProjection {
    pub fn revision(&self) -> ProjectionRevision;
    pub fn iter(&self) -> impl Iterator<Item = &ProjectedCanvasItem>;
    pub fn in_draw_order(&self) -> Vec<&ProjectedCanvasItem>;
    pub fn in_region(&self, region: &WorldRect) -> Vec<&ProjectedCanvasItem>;
    pub fn hit_test(&self, point: WorldPoint) -> Option<CanvasHit>;
    pub fn world_bounds(&self) -> Option<WorldRect>;
}
```

**INV-285-P1: Read-Only Projection**
All `CanvasProjection` APIs are read-only. Mutation APIs are forbidden.

**INV-285-P2: Draw Order**
Items are drawn in ascending `z_order`. Items with equal `z_order` are drawn in
projection order.

**INV-285-P3: Hit Test Order**
Hit testing sorts by `HitPriority`, then reverse draw order. Ports beat nodes;
nodes beat wires; wires beat frames; overlays beat all content.

**INV-285-P4: Rebuild on Authority Revision**
A projection is rebuilt when its source document revision or session revision
changes. Incremental caches are allowed only if the output is identical to a
fresh rebuild.

**INV-285-P5: Spatial Compatibility Bridge**
Under SPEC-HAL-343, `CanvasProjection` may be translated into `SpatialProjection`
through a read-only compatibility bridge. The bridge is lossless over
`CanvasEntityId`, projected item bounds, label bounds, paint descriptors, hit
regions, hit priority, draw order, revision, viewport state, and `CanvasIntent`
translation. The bridge MUST NOT rename, delete, flatten, or reinterpret
existing `Canvas*` identities and MUST NOT introduce canvas-owned selection,
movement, deletion, duplication, wire creation, or domain mutation.

Viewport-state losslessness means the bridge obeys the rect-aware conversion in
SPEC-HAL-284 INV-284-A2B. It retains the origin-anchored
`CanvasViewportState` until the caller-owned viewport rectangle is available,
or receives that rectangle when it materializes the center-anchored
`SpatialViewportState` and ephemeral `SpatialTransform`. Losslessness does not
require identical Canvas and Spatial representations, but it forbids deriving
canonical spatial camera state before the rectangle needed for the conversion
is known.

---

# 7. Intent Boundary

Canvas interaction produces intents, not effects.

```rust
pub enum CanvasIntent {
    Hit(CanvasHit),
    EmptyClick(WorldPoint),
    EmptyContextMenu(WorldPoint),
    RubberBandChanged(WorldRect),
    RubberBandCommitted(WorldRect),
    ViewportChanged(CanvasTransform),
}
```

**INV-285-I1: Intent Only**
The canvas emits `CanvasIntent` values. The consumer decides whether an intent
mutates document authority, session authority, or nothing.

**INV-285-I2: No Callbacks That Mutate Projection**
Callbacks may consume intents, but they MUST NOT receive mutable access to
projection items.

---

# 8. Acceptance Criteria

- Projection can represent nodes, ports, wires, frames, annotations, and custom
  items without owning their models.
- Hit testing returns the topmost projected entity at a world point.
- Draw order respects `z_order`.
- Viewport culling uses projection bounds.
- Zoom-to-fit calculates bounds from projection records.
- No API exists for canvas-owned selection, movement, deletion, duplication, or
  domain mutation.
- SPEC-HAL-343 spatial bridging preserves existing `Canvas*` projection, hit
  testing, bounds, draw-order, viewport, and intent behavior through the
  rect-aware compatibility rule in SPEC-HAL-284.

---

# 9. Non-Goals

- Document storage.
- Editor/session storage.
- Undo/redo.
- Serialization.
- Collision resolution between entities.
