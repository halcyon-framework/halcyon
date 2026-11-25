# SPEC-HAL-291: **Node Search / Quick Add**

### *Fuzzy-Search Overlay for Adding Nodes to the Canvas*

**Author:** JC
**Status:** Binding
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-284 (Infinite Canvas)
* SPEC-HAL-286 (Node Widget)
* SPEC-HAL-287 (Port Model)
* SPEC-HAL-293 (Node Graph Model)

---

# 1. Purpose

Define a search overlay that appears on the canvas for quickly finding and adding nodes by name. This is the node editor's equivalent of Halcyon's existing ActionPalette — a fuzzy-search popup that appears at the cursor, filters a catalog of available node types, and creates a node on selection.

The overlay is editor/session state. The node catalog is application-provided authority. Selecting a result emits an intent; graph mutation happens only through `NodeGraphDocument` transactions.

---

# 2. Trigger Conditions

**INV-291-T1: Right-Click on Empty Canvas**
Right-click on empty canvas space -> set `NodeEditorSession.search` to open at the click position (world coordinates).

**INV-291-T2: Tab Key**
Tab key (when no item is focused) -> set `NodeEditorSession.search` to open at viewport center.

**INV-291-T3: Double-Click Empty Space**
Double-click on empty canvas -> set `NodeEditorSession.search` to open at click position.

**INV-291-T4: Dangling Wire Drop**
When a connection drag (SPEC-289) is dropped on empty space, the search overlay opens at the drop position. The search is **pre-filtered** to node types that have a compatible port for the dangling wire. Selecting a node emits one compound graph transaction that creates the node and connects the wire.

---

# 3. Catalog View

```rust
/// Search-facing projection of a NodeTypeCatalog entry.
pub struct NodeSearchEntry {
    pub type_id: u64,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub input_port_types: Vec<PortTypeId>,
    pub output_port_types: Vec<PortTypeId>,
    pub icon: Option<IconId>,
    pub frequency: u32,  // usage count for "recent" sorting
}
```

**INV-291-C1: Application-Provided**
`NodeTypeCatalog` is defined by SPEC-293 and provided by the application, not the canvas, overlay, or node widget. The application registers node types at startup and can update dynamically.

**INV-291-C2: Category Grouping**
Results are grouped by category (e.g., "Math", "Logic", "IO", "Transform").

**INV-291-C3: Catalog Is Read-Only to Overlay**
Search reads from `NodeTypeCatalog`; it never inserts, removes, or mutates node type definitions.

**INV-291-C4: Search Entries Are Projection**
`NodeSearchEntry` is a search-facing projection of catalog data plus application-provided metadata such as description, icon, and usage frequency. It is not the authoritative node type definition.

---

# 4. Search Behavior

**INV-291-S1: Fuzzy Match**
Search query fuzzy-matches against node type name AND category using the canonical fuzzy-search scoring owned by `ActionRegistry::search` / `ActionRegistry::search_fuzzy`. `ActionPalette::update_search` is an existing UI consumer of that registry search, not the algorithm owner.

**INV-291-S2: Pre-Filter (Dangling Wire)**
When opened from a dangling wire (INV-291-T4), results are filtered to only show node types that have a compatible port:
- If dragging from an output: show nodes with a compatible input port
- If dragging from an input: show nodes with a compatible output port

**INV-291-S3: Recent Nodes**
When the query is empty, show recently used node types first (sorted by `frequency` descending), followed by all types grouped by category.

**INV-291-S4: Instant Filter**
Results update on every keystroke. No debounce.

---

# 5. Overlay Layout

```
┌──────────────────────────────┐
│ 🔍 [ search query         ] │  ← text input, auto-focused
├──────────────────────────────┤
│ ▸ Math                       │  ← category header
│   Add                        │
│   Subtract                   │
│   Multiply                   │
│ ▸ Logic                      │
│   AND Gate                   │
│   OR Gate                    │
└──────────────────────────────┘
```

**INV-291-OL1: Positioned at Trigger Point**
The overlay appears at the world-space position where it was triggered. It renders in screen space (fixed size regardless of zoom) but its creation position is in world space.

**INV-291-OL2: Fixed Width**
Width from theme (default ~250px). Height adapts to results (max ~300px, scrollable).

**INV-291-OL3: Auto-Focus**
The text input is immediately focused when the overlay opens. Typing starts filtering immediately.

**INV-291-OL4: Z-Order**
Renders above everything — nodes, wires, selection rectangles. Uses overlay layer.

**INV-291-OL5: Overlay State Is Session State**
Open/closed state, query string, selected result index, trigger position, and dangling-wire context live in `NodeEditorSession`.

---

# 6. Interaction

**INV-291-I1: Keyboard Navigation**
- Up/Down or `k`/`j` → move cursor between results
- Enter → emit a create-node intent for the selected node type at the trigger position
- Escape → close overlay, no action
- Typing → filter results

**INV-291-I2: Click**
Click on a result -> emit a create-node intent. Click outside the overlay -> close, no action.

**INV-291-I3: Auto-Connect (Dangling Wire)**
When triggered from a dangling wire drop (INV-291-T4), creating a node also creates the connection:
1. A compound graph transaction creates the node at the drop position
2. The compatible port on the new node is identified
3. The same compound graph transaction creates the connection between the dangling wire's source port and the new node's compatible port

**INV-291-I4: Dismiss on Scroll/Pan**
If the canvas is panned or zoomed while the overlay is open, the overlay closes (cancel).

---

# 7. Rendering

**INV-291-R1: Pipeline Safety**
Overlay uses `push_rect` for background/borders and `push_text_with_backgrounds` for text. Per SPEC-PM-006.

**INV-291-R2: Theme**
Background: `surfaces.overlay`. Border: `borders.default`. Selected result: `accents.primary` background. Text: standard typography colors.

**INV-291-R3: Category Headers**
Category headers render in `typography.muted` with slight indent. Result items render in `typography.primary` with larger indent.

---

# 8. Intent Boundary

```rust
pub enum NodeSearchIntent {
    Open { position: WorldPoint, dangling_wire: Option<DanglingWireContext> },
    UpdateQuery(String),
    MoveCursor(SearchCursorDelta),
    Close,
    CommitCreate { type_id: u64, position: WorldPoint },
    CommitCreateAndConnect { type_id: u64, position: WorldPoint, dangling_wire: DanglingWireContext },
}
```

**INV-291-I5: Search Does Not Mutate Graph Directly**
`CommitCreate` and `CommitCreateAndConnect` are consumed by the owner, which validates and applies `GraphTransaction` values.

---

# 9. Acceptance Criteria

- Overlay opens on right-click / Tab / double-click on empty canvas
- Fuzzy search filters node types by name and category
- Enter or click emits an intent that creates a node through graph transaction
- Escape closes without action
- Dangling wire drop opens pre-filtered search
- Selecting from pre-filtered search commits node creation and connection as one compound graph transaction
- Recently used nodes appear first when query is empty
- Overlay renders above all canvas content
- Search overlay state lives in `NodeEditorSession`

---

# 10. Non-Goals

- Node type previews in the search results (future — show a mini node diagram)
- Drag from search result to canvas (future — create node by dragging)
- Keyboard shortcut assignments per node type
- Favorites / pinned node types (application can implement via frequency)
