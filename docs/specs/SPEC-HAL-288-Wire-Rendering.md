# SPEC-HAL-288: **Wire Rendering — Connection Visualization Between Ports**

### *Visual Lines and Curves Connecting Node Ports*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-285 (Canvas Projection Protocol)
* SPEC-HAL-287 (Port Model)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define how connections between ports are rendered as visual wires. Wires are the primary visual element conveying data flow or logical relationships between nodes.

---

# 2. Critical Implementation Note

**Halcyon's path pipeline already supports cubic bezier curves.** The `PathCommand::CubicTo` variant, `Path::flatten()` for tessellation, and `Path::build_stroke_mesh()` with `StrokeParams` (width, join, cap) are all implemented in `src/renderer/path.rs`.

**No new shader or GPU pipeline is needed.** Wire rendering uses the existing path pipeline with cubic bezier paths + stroke tessellation. This is a rendering consumer, not a rendering primitive.

---

# 3. Wire Projection Model

```rust
/// Disposable projected view of a GraphConnection.
#[derive(Debug, Clone)]
pub struct WireProjection {
    pub id: u64,
    pub from_node: u64,
    pub from_port: u32,
    pub to_node: u64,
    pub to_port: u32,
    pub color: Option<Color>,     // None = derive from port type
    pub style: WireStyle,
    pub state: WireVisualState,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireStyle {
    #[default]
    Bezier,       // smooth cubic bezier curve
    Straight,     // direct straight line
    RightAngle,   // right-angle polyline (Manhattan routing)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum WireVisualState {
    #[default]
    Default,
    Hovered,
    Selected,
    Critical,    // critical path emphasis
    Preview,     // temporary wire during drag-to-connect
    Dimmed,      // de-emphasized (when highlighting other connections)
}
```

**INV-288-A1: WireProjection Is Not Authority**
`WireProjection` is derived from `GraphConnection`, `PortProjection`, `NodeEditorSession`, and `NodeFlowConfig`. It MUST NOT be the source of truth for the connection.

**INV-288-A2: Connection Identity Lives in Graph**
Permanent wire identity and endpoints live in `GraphConnection` (SPEC-293). Wire visual state is derived session/application overlay state.

---

# 4. Wire Geometry

### Bezier Curve (Default)

**INV-288-G1: Cubic Bezier**
Wires render as cubic bezier curves. The control points create a natural horizontal-exit, horizontal-entry curve:

```rust
fn wire_bezier(from: WorldPoint, to: WorldPoint) -> Path {
    let dx = (to.x - from.x).abs().max(50.0) * 0.5;
    let mut path = Path::new();
    path.move_to(from.x, from.y);
    path.cubic_to(
        from.x + dx, from.y,    // control point 1: horizontal exit
        to.x - dx, to.y,        // control point 2: horizontal entry
        to.x, to.y,             // endpoint
    );
    path
}
```

**INV-288-G2: Backward Connection**
When the target is to the LEFT of the source (wire goes backward), the curve loops down/up to avoid overlapping the nodes:

```rust
if to.x < from.x {
    // Route below/above with extra vertical offset
    let mid_y = (from.y + to.y) * 0.5 + 100.0; // offset down
    // Use two bezier segments or a larger control point spread
}
```

### Straight Line

**INV-288-G3: Direct Line**
`WireStyle::Straight`: single line segment from output port to input port.

### Right-Angle

**INV-288-G4: Manhattan Routing**
`WireStyle::RightAngle`: horizontal from source → vertical transition → horizontal to target. Produces an axis-aligned polyline with one or two bends.

---

# 5. Wire Rendering

**INV-288-R1: Path Pipeline**
Wires use the existing path pipeline. Each wire is a `Path` with `build_stroke_mesh()` applied.

```rust
fn paint_wire(cx: &mut CanvasPaintContext, wire: &WireProjection, from: WorldPoint, to: WorldPoint) {
    let path = wire_bezier(from, to);
    let stroke = StrokeParams {
        width: wire_width(wire.state),
        join: StrokeJoin::Round,
        cap: StrokeCap::Round,
        ..Default::default()
    };
    let mesh = path.build_stroke_mesh(&stroke);
    let color = wire_color(wire, cx.theme);
    // Submit mesh to path pipeline via PathDraw
}
```

**INV-288-R2: Wire Width**
- Default: 2.0 world units
- Hovered: 3.0
- Selected: 3.0
- Critical: 3.0
- Preview: 2.0 (dashed if possible, else lower opacity)
- Dimmed: 1.0

**INV-288-R3: Wire Color**
- Default: port type color (from `PortTypeRegistry`)
- If `wire.color` is set: use it directly
- Hovered: brightened (lerp 0.2 toward white)
- Selected: `accents.primary`
- Critical: `accents.error`
- Preview: port type color at 60% opacity
- Dimmed: 30% opacity

**INV-288-R4: Z-Order**
Wires render BEHIND nodes. Default wire z_order = 50 (nodes are at z=100). Selected/hovered wires: z=90 (just below nodes but above other wires).

**INV-288-R5: Arrowhead**
Optional arrowhead at the input end. Small triangle pointing toward the input port. Helps indicate data flow direction. Configurable: `show_arrowheads: bool`.

---

# 6. Wire Hit Testing

**INV-288-H1: Distance-Based**
Wire hit testing checks the minimum distance from the pointer to the bezier curve. If distance < `wire_hit_tolerance` (default 5.0 world units), the wire is hit.

**INV-288-H2: Bezier Distance**
For cubic bezier: flatten to line segments (using `Path::flatten()`), then compute point-to-segment distance for each segment. Return the minimum.

**INV-288-H3: Hit Priority**
Wires have lower hit priority than nodes and ports. If a pointer event hits both a wire and a node, the node wins.

**INV-288-H4: Hit Test Emits Entity ID**
Wire hit testing returns `CanvasEntityId::Wire(connection_id)` through the canvas projection. It does not select or delete the wire directly.

---

# 7. Temporary Wire (Drag Preview)

**INV-288-T1: During Connection Drag**
When the user drags from a port (SPEC-289), `NodeEditorSession` stores the active drag endpoint. Projection emits a temporary preview wire from the source port to the cursor position. It uses `WireVisualState::Preview`.

**INV-288-T2: Snap Preview**
When the cursor nears a compatible port, the preview wire snaps to that port's position (instead of following the cursor exactly).

**INV-288-T3: Preview Is Not Graph State**
A preview wire is not inserted into `NodeGraphDocument`. A graph connection appears only after a successful committed transaction.

---

# 8. Wire Animation (Future Extension)

**INV-288-A1: Flow Dots**
Animated dots moving along the wire to indicate data flow direction. Computed in the shader using `time_seconds` uniform. Deferred — not required for v1.

**INV-288-A2: Connection Animation**
New wire "grows" from source to target with a short animation. Deferred.

---

# 9. Acceptance Criteria

- Bezier wires render smoothly between output and input ports
- Wire color derived from port type
- Wire hit testing works (click to select a wire)
- Temporary preview wire follows cursor during drag
- Temporary preview wire is session/projection state, not graph state
- Backward connections route without overlapping nodes
- Wire visual states (default, hovered, selected, critical, preview, dimmed)
- Wires render behind nodes (z-order)
- Straight line and right-angle wire styles work

---

# 10. Non-Goals

- Wire bundling (grouping parallel wires — future)
- Wire labels (text on wires — future)
- Animated flow dots (future extension, §8)
- Self-routing to avoid ALL node intersections (basic avoidance only)
