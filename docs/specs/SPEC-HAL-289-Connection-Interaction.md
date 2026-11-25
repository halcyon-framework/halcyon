# SPEC-HAL-289: **Connection Interaction — Drag-to-Connect Protocol**

### *Creating and Modifying Wire Connections Between Ports*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-287 (Port Model)
* SPEC-HAL-288 (Wire Rendering)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define the interaction model for creating, modifying, and removing connections between node ports. This is the core "feel" of a node editor — drag-to-connect must feel instant, forgiving, and predictable.

Connection interaction owns no graph data. It reads `NodeGraphDocument`, `NodeTypeCatalog`, `PortTypeRegistry`, and `NodeEditorSession`; it emits intents that the owner converts into graph transactions.

---

# 2. Connection Creation

### Drag-to-Connect Flow

**INV-289-C1: Initiate from Output Port**
`PointerDown` on an output port -> update `NodeEditorSession` to begin connection drag. A preview wire renders from the output port to the cursor.

**INV-289-C2: Initiate from Input Port**
`PointerDown` on a disconnected input port -> update `NodeEditorSession` to begin connection drag. The preview wire extends from the cursor toward potential output ports. Direction is reversed visually.

**INV-289-C3: Initiate from Connected Input (Re-route)**
`PointerDown` on a connected input port -> begin a re-route session that references the existing connection and renders it as provisional. The existing graph connection remains authoritative until the session commits. Drop on a new compatible input commits a replace-connection transaction. Drop on empty commits a remove-connection transaction. Escape discards the session and leaves the original connection unchanged.

**INV-289-C4: Compatibility Feedback**
During drag, all ports on all visible nodes update their visual state:
- Compatible ports: glow / highlight (per SPEC-287 §5 INV-287-R3)
- Incompatible ports: dim to 30% opacity
- The source port: pulsing ring

**INV-289-C5: Snap to Port**
When the cursor enters a compatible port's hit radius, the preview wire snaps to that port's center position. Visual: the port circle grows and brightens.

**INV-289-C6: Complete Connection**
`PointerUp` on a compatible port -> emit a connection-commit intent. The owner validates and applies one graph transaction. The preview wire disappears because the next projection contains the permanent `GraphConnection`.

**INV-289-C7: Cancel Connection**
`PointerUp` on empty space or an incompatible port -> cancel unless auto-create is enabled. Preview wire disappears. No connection is created.

**INV-289-C8: Drop on Empty + Auto-Create (Optional)**
`PointerUp` on empty canvas space MAY trigger a node search popup (SPEC-291) at the drop position. The search is pre-filtered to node types that have a compatible port. Selecting a node type emits one compound graph transaction that creates the node and connection.

---

# 3. Connection Removal

**INV-289-R1: Select + Delete**
Click on a wire emits a selection intent (hit testing per SPEC-288 §6). Press Delete emits a delete-selected intent. The owner commits a graph transaction that removes selected connections.

**INV-289-R2: Right-Click Disconnect**
Right-click on a wire → context menu with "Disconnect" option.

**INV-289-R3: Re-route Disconnect**
Dragging from a connected input port (INV-289-C3) does not mutate the graph at drag start. If the user drops on empty space, the owner commits a remove-connection transaction for the original connection.

---

# 4. Validation Pipeline

When a connection transaction is about to be committed, the following checks run in order against the authoritative document/catalog/registry state:

1. **Direction check**: Output → Input only (INV-287-CR1)
2. **Self-connection check**: Not the same node (INV-287-CR2)
3. **Node type/port existence check**: both node types and port IDs exist in `NodeTypeCatalog`
4. **Type compatibility check**: `registry.are_compatible()` (INV-287-CR4)
5. **Duplicate connection check**: the exact endpoint pair is not already connected
6. **Cardinality check**: single-cardinality replacements are represented as one compound transaction; invalid overflow is rejected
7. **Application veto**: owning application/document validator approves the proposal (INV-287-CR5)

If any check fails, the connection is rejected. The preview wire shows a "reject" visual (brief red flash) and disappears.

---

# 5. Interaction State Machine

```
IDLE
  ├── PointerDown on output port → DRAGGING_FROM_OUTPUT
  ├── PointerDown on disconnected input → DRAGGING_FROM_INPUT
  └── PointerDown on connected input → REROUTING (reference existing wire in session)

DRAGGING_FROM_OUTPUT
  ├── PointerMove → update preview wire endpoint + compatibility highlights
  ├── PointerUp on compatible input → CREATE_CONNECTION_TRANSACTION → IDLE
  ├── PointerUp on empty space → CANCEL (or open search) → IDLE
  └── Escape → CANCEL → IDLE

DRAGGING_FROM_INPUT
  ├── (same as DRAGGING_FROM_OUTPUT but reversed direction)

REROUTING
  ├── PointerMove → update preview wire (original output → cursor)
  ├── PointerUp on compatible input → REPLACE_CONNECTION_TRANSACTION → IDLE
  ├── PointerUp on empty → REMOVE_CONNECTION_TRANSACTION → IDLE
  └── Escape → DISCARD_SESSION (graph unchanged) → IDLE
```

**INV-289-SM1: State Machine Lives in Session**
Connection interaction state lives in `NodeEditorSession`. It is not stored in the canvas, node projection, wire projection, or graph model.

---

# 6. Feedback Timing

**INV-289-F1: Instant Highlight**
Compatibility highlights update on every pointer move frame. No delay.

**INV-289-F2: Snap Threshold**
Port snap activates at `port_hit_radius * 1.5` distance. The transition is instant (not animated).

**INV-289-F3: Rejection Flash**
On rejected connection: preview wire turns red for 150ms, then disappears.

---

# 7. Acceptance Criteria

- Drag from output port shows preview wire following cursor
- Compatible ports highlight, incompatible ports dim during drag
- Drop on compatible port commits a connection transaction
- Drop on empty space cancels (or opens node search)
- Drag from connected input re-routes: drop on new input commits replacement, drop on empty commits removal
- Select wire + Delete commits removal transaction
- Validation pipeline runs all 7 checks before committing
- Escape cancels drag at any point
- Snap-to-port when cursor nears compatible port
- Re-route Escape leaves the original graph connection unchanged

---

# 8. Non-Goals

- Multi-wire drag (connecting multiple ports in one gesture — future)
- Wire splitting (inserting a node in the middle of an existing wire — future)
- Auto-layout after connection (application responsibility)
