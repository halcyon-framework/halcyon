# SPEC-HAL-287: **Port Model — Connection Points and Type System**

### *Typed Input/Output Ports on Nodes*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)

---

# 1. Purpose

Define the port model: connection points on nodes with types, directions, connection rules, and visual representation. Ports are where wires attach. The type system determines which ports can connect to which.

---

# 2. Data Model

```rust
/// Port definition in a node type template.
#[derive(Debug, Clone)]
pub struct PortDefinition {
    pub port_id: u32,          // unique within the node type
    pub label: String,
    pub direction: PortDirection,
    pub type_id: PortTypeId,
    pub max_connections: PortCardinality,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortDirection {
    Input,
    Output,
}

/// How many connections a port allows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortCardinality {
    /// At most one connection.
    Single,
    /// Unlimited connections.
    Multiple,
}

/// Opaque port type identifier. Application-defined.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortTypeId(pub u64);

/// Disposable port projection produced during node layout.
#[derive(Debug, Clone)]
pub struct PortProjection {
    pub node_id: u64,
    pub definition: PortDefinition,
    /// Derived from NodeGraphDocument connections.
    pub connection_count: u32,
    /// Derived world position set during projection layout.
    pub world_pos: WorldPoint,
}
```

**INV-287-A1: Definitions Are Catalog Authority**
`PortDefinition` belongs to an application-supplied node type catalog. It is not owned by the canvas and is not mutated by port interaction.

**INV-287-A2: PortProjection Is Rebuildable**
`PortProjection.connection_count` and `PortProjection.world_pos` are derived from `NodeGraphDocument` plus layout. They are cacheable projection data, not source-of-truth state.

---

# 3. Port Type Registry

```rust
/// Registered port type with visual and compatibility info.
#[derive(Debug, Clone)]
pub struct PortTypeInfo {
    pub type_id: PortTypeId,
    pub name: String,
    pub color: Color,
    /// Types this port can connect TO (Output→Input).
    pub compatible_with: Vec<PortTypeId>,
}

/// Registry of all known port types.
pub struct PortTypeRegistry {
    types: HashMap<PortTypeId, PortTypeInfo>,
}

impl PortTypeRegistry {
    pub fn register(&mut self, info: PortTypeInfo);
    pub fn get(&self, id: PortTypeId) -> Option<&PortTypeInfo>;
    pub fn are_compatible(&self, output_type: PortTypeId, input_type: PortTypeId) -> bool;
    pub fn color(&self, id: PortTypeId) -> Color;
}
```

### Invariants

**INV-287-T1: Application-Defined Types**
Port types are registered by the application, not the canvas or node widget. The registry is an application/document-side input to validation and projection.

**INV-287-T2: Compatibility Is Directional**
`are_compatible(output_type, input_type)` checks if `output_type.compatible_with` contains `input_type`. Compatibility is NOT symmetric — the output defines what it can connect to.

**INV-287-T3: Universal Type**
A port type with `compatible_with: vec![]` means "not compatible with anything" unless either endpoint is `PortTypeId::ANY`. A special `PortTypeId::ANY` constant (value 0) is compatible with all types in either direction. Application can use this for "generic" ports.

---

# 4. Port Hit Testing

**INV-287-H1: Hit Radius**
Port hit area is a circle with `port_hit_radius` from node geometry/theme. This is larger than the visual port radius for easy targeting.

**INV-287-H2: Port Priority**
When a pointer event hits a port, the port takes priority over the node body. This allows clicking on ports that visually overlap the node border.

**INV-287-H3: Port Position**
Port centers are positioned at the node's left edge (inputs) or right edge (outputs), vertically centered in their row.

```rust
impl PortProjection {
    pub fn hit_test(&self, world_point: WorldPoint) -> bool {
        let dx = world_point.x - self.world_pos.x;
        let dy = world_point.y - self.world_pos.y;
        (dx * dx + dy * dy) <= (hit_radius * hit_radius)
    }
}
```

---

# 5. Port Rendering

**INV-287-R1: Connected vs Disconnected**
- Connected (connection_count > 0): filled circle in port type color
- Disconnected (connection_count == 0): hollow circle (border only) in port type color

**INV-287-R2: Port Hover**
When the pointer is near a port (within hit radius): port circle grows slightly (1.5× visual radius) and brightens.

**INV-287-R3: Drag Hover (Compatibility Highlight)**
During wire drag (SPEC-289), ports highlight to show compatibility:
- Compatible ports: glow with port type color
- Incompatible ports: dim to 30% opacity
- Source port: pulsing ring

**INV-287-R4: Port Labels**
Labels render beside the port circle. Input labels to the right of the circle. Output labels to the left. Labels use `port_font_size` from theme.

---

# 6. Connection Rules

**INV-287-CR1: Direction Rule**
Connections must go from Output to Input. Output→Output and Input→Input connections are forbidden.

**INV-287-CR2: Self-Connection Rule**
A port cannot connect to another port on the same node.

**INV-287-CR3: Cardinality Rule**
Single-cardinality ports: a committed connection transaction must either replace the existing connection in the same compound transaction or be rejected. Multiple-cardinality ports: new connections are added alongside existing ones.

**INV-287-CR4: Type Compatibility Rule**
Connection is allowed only if `registry.are_compatible(output_type, input_type)` returns true.

**INV-287-CR5: Application Veto**
After all framework rules pass, the owning application/document validator receives the proposed connection and may veto it. This allows domain-specific validation (e.g., cycle detection in DAGs).

**INV-287-CR6: Validation Uses Authoritative Graph**
Cardinality and duplicate checks are evaluated against `NodeGraphDocument`, not against projected port counters.

---

# 7. Acceptance Criteria

- Port types register with color and compatibility info
- Ports render as filled (connected) or hollow (disconnected) circles
- Port hit testing with generous hit radius works
- Compatibility check works directionally (output → input)
- Application/document validator runs before a connection transaction commits
- Cardinality enforced: single ports replace old wire and add new wire in one compound transaction
- Port projections expose no mutation API and are rebuildable from graph/catalog state

---

# 8. Non-Goals

- Port value visualization (showing live data flowing through)
- Dynamic port creation (adding ports at runtime — future extension)
- Port reordering (ports are in definition order)
