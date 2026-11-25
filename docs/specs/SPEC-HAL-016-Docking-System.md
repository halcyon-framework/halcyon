**SPEC-HAL-016: Halcyon — Window & Docking System**
**Author:** JC
**Date:** 2025-11-17
**Status:** Draft
**Scope:** Floating windows, dock layout, splitters, tabs, persistence.
**Primary Use-Case:** Trading workspaces: chart + ladder + blotter + misc tools in one controllable surface.

Depends on:

* HAL-003 (modules & skeleton)
* HAL-009 (event routing & hit-testing)
* HAL-005 (scene graph, layers)
* HAL-007 (state/model wiring)
* HAL-015 (caching/diffing)

---

# 1. Goal

You need a **workstation**, not a single flat panel.

Window/docking layer must:

* Host multiple **panels**: charts, ladders, blotters, logs, config views.
* Let user arrange them into a **stable layout**:

    * dock left/right/top/bottom,
    * splitters to resize,
    * tabbed stacks,
    * floating detachable windows.
* Be **fast**, predictable, and easily persisted to disk.

This is a **UI service** Halcyon provides to your app, not something you rewrite per project.

---

# 2. Core Concepts

### 2.1 Workspace

Top-level docking manager widget:

```rust
pub struct Workspace {
    state: WorkspaceState,
}

pub struct WorkspaceState {
    pub root_dock: DockNodeId,         // docking tree
    pub floating_windows: Vec<WindowId>,
    pub active_window: Option<WindowId>,
}
```

There is typically **one** `Workspace` per OS window. Future extension: multiple OS windows running their own `Workspace`.

### 2.2 Panel

Smallest logical unit user manipulates: “Chart 1”, “Ladder BTC-PERP”, “Blotter”, etc.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanelId(u64);

pub struct PanelDescriptor {
    pub id: PanelId,
    pub title: String,
    pub icon: Option<IconId>,
    pub kind: PanelKind, // Chart, Ladder, Blotter, Custom(…)
}
```

Actual UI content is a Halcyon `Widget` bound to this panel id.

### 2.3 Window

A frame that hosts:

* either a **tabbed stack** of panels, or
* a single panel (degenerate case),

and is either docked or floating.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

pub struct WindowState {
    pub id: WindowId,
    pub panels: Vec<PanelId>,
    pub active_panel: PanelId,
    pub dock_node: Option<DockNodeId>,      // if docked
    pub floating_rect: Option<RectF>,       // in workspace coords if floating
    pub is_floating: bool,
}
```

### 2.4 Dock Tree

Docking is a **tree** over the workspace area.

Node types:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DockNodeId(u64);

pub enum DockNodeKind {
    Split {
        axis: SplitAxis,            // Horizontal or Vertical
        children: [DockNodeId; 2],
        split_ratio: f32,           // 0.0..1.0
    },
    Stack {
        window_ids: Vec<WindowId>,  // tabbed
        active_window: WindowId,
    },
}

pub enum SplitAxis {
    Horizontal, // top / bottom
    Vertical,   // left / right
}
```

The **root** `DockNodeId` defines full workspace rect. All docked windows live under this tree. Floating windows live outside it.

---

# 3. Layout Model

### 3.1 Workspace Rect

Workspace has a `RectF` equal to:

* entire client area of OS window (minus optional status bar / menu bar), **or**
* a logical rect inside a bigger UI (if embedded).

All docking coordinates are inside this rect.

### 3.2 Split Layout

For `Split` node:

* Given parent rect `R`:

    * `axis = Vertical` → left and right rects:

        * `R_left.width = R.width * split_ratio`
        * `R_right.width = R.width - R_left.width`
    * `axis = Horizontal` → top and bottom rects using `split_ratio` on height.

### 3.3 Stack Layout

For `Stack` node:

* Entire rect is given to the active window’s **content area**.
* A strip at top used for **tab bar**:

    * `tab_height = theme.tabs.height`
    * content rect = `RectF { y: R.y + tab_height, .. }`.

All windows in stack share the same rect; only active panel content is drawn.

### 3.4 Floating Windows

Floating windows have their own rect:

```rust
pub struct FloatingWindow {
    pub window_id: WindowId,
    pub rect: RectF,
    pub z_order: u32,
}
```

Rendered on top of docked content in z-order (highest z last). Position and size stored in workspace state → serializable.

---

# 4. Interaction Flows

### 4.1 Dragging Window (Reposition Floating)

Source: title bar of a floating window.

Sequence:

1. `PointerDown` on title bar:

    * Capture pointer via `PointerCapture`.
    * Store initial `drag_origin` and starting `rect`.

2. `PointerMove`:

    * Update window rect position (keep size).
    * Mark workspace as dirty (HAL-014).

3. `PointerUp`:

    * Release capture.
    * If drop close to dock area, we might **dock** (see 4.2).

### 4.2 Docking / Undocking

User drags from title bar (either docked or floating):

* During drag:

    * Compute **dock target zone** based on pointer:

        * Over left edge → drop hint “dock left”.
        * Over right/top/bottom → similar.
        * Over center of a stack → “tab into this stack”.

Show **dock preview overlay**:

```rust
struct DockPreview {
    pub target_node: Option<DockNodeId>,
    pub target_slot: DockSlot, // Left / Right / Top / Bottom / TabInto / None
    pub preview_rect: Option<RectF>,
}
```

On `PointerUp`:

* If `DockSlot::None` → if it was docked, convert to floating at last pointer position.
* Else:

    * mutate dock tree:

        * if slot is Left/Right/Top/Bottom:

            * create Split node over target
            * attach source window as one child and target as other
        * if TabInto:

            * add source window to target Stack’s `window_ids`.

Window’s `is_floating` flag and `dock_node` updated accordingly.

### 4.3 Splitters (Resize Dock Panels)

Splitters appear between two child rects of a Split node.

Hit-testing:

* For each `Split`:

    * compute splitter rect around the boundary line (few px thickness).
    * treat it as interactive area.

Interaction:

1. `PointerDown` on splitter → capture pointer.
2. `PointerMove`:

    * adjust `split_ratio` based on pointer delta.
    * clamp to `[min_ratio, max_ratio]` (e.g. 0.1 .. 0.9).
    * mark workspace dirty.
3. `PointerUp` → release capture.

### 4.4 Tabs

Inside Stack nodes:

* Each window has a tab.
* Hit-testing:

    * compute rect per tab based on title width.
* Interactions:

    * Left click:

        * sets `active_window`.
    * Middle click (optional):

        * close panel/window (if app allows that).
    * Drag tab:

        * either reorder tabs within stack,
        * or “tear out” to new window (floating) if dragged outside tab strip.

---

# 5. Persistence (Layouts)

You must be able to save / restore layout for workspace(s).

### 5.1 Serializable State

Define a serializable representation:

```rust
#[derive(serde::Serialize, serde::Deserialize)]
pub struct WorkspaceLayout {
    pub root_dock: DockNodeLayout,
    pub floating_windows: Vec<FloatingWindowLayout>,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub enum DockNodeLayout {
    Split {
        axis: SplitAxis,
        split_ratio: f32,
        children: [Box<DockNodeLayout>; 2],
    },
    Stack {
        windows: Vec<WindowLayout>,
        active_index: usize,
    },
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct WindowLayout {
    pub panels: Vec<PanelId>,   // stable panel IDs from app
    pub active_panel_index: usize,
}

#[derive(serde::Serialize, serde::Deserialize)]
pub struct FloatingWindowLayout {
    pub window: WindowLayout,
    pub rect: RectF,
    pub z_order: u32,
}
```

App-level `PanelId` must be stable across runs.
The app is responsible for mapping `PanelId` → actual widget construction on restore.

### 5.2 Integration with AppState

App keeps active workspace layout in its state:

```rust
struct TradingAppState {
    workspace_layout: WorkspaceLayout,
    // ... other state
}
```

On startup:

* load layout from disk (JSON/TOML/bincode),
* reconstruct `WorkspaceState` from `WorkspaceLayout`.

On shutdown or explicit “save layout”:

* build `WorkspaceLayout` from `WorkspaceState` and panel registry,
* serialize to disk.

---

# 6. Integration with Halcyon Internals

### 6.1 Workspace as Widget

`Workspace` is itself a `Widget`:

* `layout`:

    * takes all available size.
    * computes rects for all dock nodes + floating windows.
* `paint`:

    * draws:

        * panel contents,
        * title bars,
        * tabs,
        * splitters,
        * dock overlays.

### 6.2 Event Routing

Workspace uses HAL-009 mechanisms:

* Hit-testing:

    * Works in workspace coordinates.
    * For each floating window (top-most first by z-order):

        * check title bar, tabs, splitters, content rect.
    * If no floating hit, traverse dock tree rects.

* Pointer capture:

    * Title bar drag, splitter resize, dock preview all use `PointerCapture`.
    * For example, `PointerCapture.captured_widget` can be the Workspace widget itself; internal field tracks what is being dragged.

* Focus:

    * Focused panel is determined by click inside its content rect.
    * `Workspace` sets `FocusManager`’s focused widget to that panel’s root widget id.

### 6.3 Scene Graph & Layers

Dock system uses scene layers:

* Layer 0: background / docked panel content.
* Layer 1: floating windows.
* Layer 2: drag/dock overlay (highlight rectangles).
* Layer 3: tooltips / transient overlays (if used).

This plays nicely with HAL-014 caching: moving a floating window mostly touches Layer 1 + overlay; docked content stays untouched.

---

# 7. Trading-Specific Behavior

### 7.1 Locking Layout

Trading mode often wants “locked” layouts in production:

* Flag per workspace: `layout_locked: bool`.

If locked:

* Disable:

    * change of split ratios,
    * dragging windows,
    * creating/destroying panels.
* Allow:

    * panel content interaction (chart, ladder) as usual.

UI: maybe a small lock icon in a status bar or title bar.

### 7.2 Named Workspaces / Profiles

Support multiple named layouts:

* “Execution” (ladder + blotter + position)
* “Analysis” (multi-chart + notes)
* “Debug” (logs, metrics)

Core docking system only needs to support multiple `WorkspaceLayout`s; app-level manages the naming + switching.

### 7.3 Multi-Monitor (Future)

The architecture allows:

* One `Workspace` per OS window (multi-window support).
* Panels may move between workspaces.

SPEC-015 only defines **in-window docking**. Multi-window extension will be another SPEC; layout model is already compatible.

---

# 8. Implementation Checklist (for CC)

Concrete tasks:

1. **Define Core Types**

    * `PanelId`, `PanelDescriptor`
    * `WindowId`, `WindowState`
    * `DockNodeId`, `DockNodeKind`, `SplitAxis`
    * `WorkspaceState`, `WorkspaceLayout` (serializable)

2. **Workspace Widget**

    * `Workspace` struct with state:

        * `root_dock`
        * map `DockNodeId → DockNodeKind`
        * `windows: HashMap<WindowId, WindowState>`
        * `floating_windows: Vec<FloatingWindow>` with z-order
    * `impl Widget for Workspace`:

        * `layout`: compute rects for dock tree and floating windows
        * `paint`: draw frame, tabs, splitters, overlay rectangles, then delegate to panel widgets

3. **Dock Tree Layout Functions**

    * `fn layout_dock_tree(node: DockNodeId, rect: RectF, state: &WorkspaceState, out: &mut DockLayoutMap)`
    * `DockLayoutMap` maps `DockNodeId` → rect, `WindowId` → rect, plus per-splitter rects.

4. **Hit-Testing and Interaction**

    * In `Workspace::handle_event`:

        * Use global/local positions from HAL-009.
        * Hit-test:

            * floating windows (title bar, tabs, splitters, content)
            * docked windows (same)
        * Start drags with `PointerCapture`: titlebars, splitters, tabs.
        * Implement dock preview rect geometry.

5. **Dock / Undock Mutation**

    * Functions:

        * `fn dock_window_into(node: DockNodeId, window: WindowId, slot: DockSlot, state: &mut WorkspaceState)`
        * `fn undock_window_to_floating(window: WindowId, state: &mut WorkspaceState, pos: (f32, f32))`
    * Ensure dock tree invariants: no orphan nodes, no empty stacks.

6. **Tab Bar Rendering**

    * Basic `Rect` + `Text` primitives for tabs.
    * Active tab highlighting + hover state (using theming entries).

7. **Persistence**

    * `fn to_layout(state: &WorkspaceState) -> WorkspaceLayout`
    * `fn from_layout(layout: &WorkspaceLayout, panel_registry: &PanelRegistry) -> WorkspaceState`
    * `PanelRegistry` is app-level mapping `PanelId → PanelDescriptor` / builder.

8. **Integration Hooks for App**

    * Provide:

        * `Workspace::new(initial_layout: WorkspaceLayout, panel_registry: PanelRegistry)`
        * Methods for:

            * `add_panel_to_workspace`
            * `focus_panel`
            * `close_panel` (optional)

9. **Basic Tests**

    * Single stack with two windows: tab switching works.
    * Vertical split with two stacks: splitter drag modifies `split_ratio` correctly.
    * Dragging from docked to floating and back: layout remains valid.
    * Serializing layout and restoring: same window/panel arrangement.

---

This gives you a clear window/docking system blueprint that:

* Fits Halcyon’s retained + scene-graph architecture.
* Plays nicely with event routing, caching, and state wiring.
* Is explicitly shaped around **trading workstation** interaction patterns.
