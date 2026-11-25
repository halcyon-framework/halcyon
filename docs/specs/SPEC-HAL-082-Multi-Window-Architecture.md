Status Note:

This SPEC defines the original intent and goals of Halcyon’s multi-window architecture.

Structural ownership and authority semantics in this SPEC are partially superseded by:

- SPEC-HAL-082A (Multi-Window Architecture Amendment)
- SPEC-HAL-002 (Constitution)
- SPEC-HAL-150, SPEC-HAL-151, SPEC-HAL-081B, SPEC-HAL-081C, SPEC-HAL-030, SPEC-HAL-154

This SPEC remains authoritative for intent only.

# **SPEC-HAL-082 — Multi-Window Architecture (Halcyon Windowing v1)**

**SPEC-HAL-082**
**Multi-Window Architecture — Halcyon Windowing v1**
**Author:** Halcyon Architect
**Status:** Draft
**Version:** 1.0

**Depends On:**

* SPEC-HAL-024 Unified Coordinate Model
* SPEC-HAL-033 Input Fusion Layer
* SPEC-HAL-036 Input Coalescing
* SPEC-HAL-074 Style Runtime
* SPEC-HAL-075 Layout Stability Guarantees
* SPEC-HAL-078 Layout Transitions
* SPEC-HAL-079 Unified Event Flow v3
* SPEC-HAL-080 Interaction Engine
* SPEC-HAL-081 Halcyon Compositor

**Implements:**
A unified, platform-independent **multi-window system** for Halcyon, enabling:

* multiple top-level application windows
* detachable tool windows
* floating inspector panes
* dialog windows
* multi-monitor aware positioning
* cross-window event and focus synchronization

This is the formalization required before introducing docking (SPEC-083) and editor-style workspace systems.

---

# **1. Motivation**

Current Halcyon apps operate in a **single window** model.

Limitations:

* cannot open independent tool windows
* cannot detach views
* cannot support IDE-like multi-pane environments
* no native dialog or modal windows
* no cross-window focus rules
* no consistent DPI / scaling across monitors

Modern UI frameworks require robust multi-window support.

082 defines it.

---

# **2. Window vs. Surface (Critical Distinction)**

**Surface** (SPEC-081):
Logical fragment of UI rendered inside a single window.

**Window** (SPEC-082):
OS-level top-level container containing **its own compositor**, **scene**, **input pipeline**, and **render loop**.

One window contains many surfaces.
One application contains many windows.

---

# **3. Window Structure**

```
struct HalcyonWindow {
    id: WindowId,
    os_handle: RawWindowHandle,
    compositor: Compositor,
    scene: SceneGraph,
    event_router: EventRouter,
    layout_root: WidgetId,
    dpi: f32,
    scale_factor: f32,
    bounds: RectF,
    state: WindowState,
}
```

### **WindowState**

```
Normal
Minimized
Maximized
Fullscreen
Hidden
```

---

# **4. Multi-Window Lifecycle**

### Creating a window:

```
let id = window_manager.spawn(WindowParams {
    title: "Tools",
    size: (800, 600),
    position: CenterScreen,
    resizable: true,
    visible: true,
});
```

### Destroying:

```
window_manager.close(id);
```

### Hiding:

```
window_manager.hide(id);
```

### Showing:

```
window_manager.show(id);
```

### Switching focus:

```
window_manager.focus(id);
```

---

# **5. Global WindowManager**

Central registry:

```
struct WindowManager {
    windows: HashMap<WindowId, HalcyonWindow>,
    active_window: Option<WindowId>,
    next_id: u64,
}
```

Responsibilities:

1. create windows
2. destroy windows
3. route OS events
4. manage global focus
5. orchestrate per-window render loops
6. handle monitor DPI & scale changes
7. maintain z-order of windows
8. forward global shortcuts
9. coordinate between Compositor instances

---

# **6. Event Routing in a Multi-Window Environment**

New step in the Unified Event Flow:

```
OS Event
↓
Window Resolution (which OS window?)
↓
HalcyonWindow event entry
↓
Compositor / Surface Resolution
↓
Widget Hit Test
↓
HIE (Interaction Engine)
↓
State → Style → Layout → Render
```

Rules:

* only the active window receives keyboard input
* pointer events strictly tied to OS cursor position
* dragging across windows switches event target
* modal dialogs inside one window never block others
* global hotkeys (app-level) bypass windows

---

# **7. DPI and Scaling Rules**

Each window tracks:

* **monitor DPI**
* **window-local DPI** (may change during drag between monitors)
* **scale factor** for logical → physical mapping

Upon DPI change:

```
window.scale_factor = query_os_scale_factor();
invalidate_layout(window.layout_root);
```

This triggers:

* re-layout
* re-rasterization of glyphs (via atlas)
* style recompute
* redraw

The pipeline is stable due to:

* SPEC-074 Style Runtime
* SPEC-075 Layout Stability
* SPEC-066 Unified Glyph Pipeline

---

# **8. Cross-Window Coordination**

### **Focus**

Only one window has **global focus**.

Per-window focus scopes manage:

* widget focus
* popup focus
* modal focus
* keyboard navigation

### **Drag & Drop (future SPEC)**

Cross-window drag:

* pointer events migrate window ownership
* DragContext persists across windows
* drop targets resolved by Compositor in target window

### **Global shortcuts**

Global hotkeys route through:

```
WindowManager → GlobalEventBus → Handlers
```

Not delivered to a window unless explicitly registered.

---

# **9. Window Types**

### **MainWindow**

Primary app window, required.

### **ToolWindow**

Floating tool pane, optional, detachable.

### **DialogWindow**

Standalone OS dialog (modal to window but not global).

### **PopupWindow (Optional)**

For platform-true context menus (non-client popups).

### **InspectorWindow**

For debugging, layout tree, style trace, etc.

---

# **10. Render Model**

Each HalcyonWindow:

* has its own GPU surface
* owns its own Renderer instance
* maintains its own swap chain
* uses its own RenderGraph

Renderer instances **share GPU device**, do not recreate WGPU device.

Drawing order driven by OS window manager, not Halcyon.

---

# **11. Compositor Integration**

Each window embeds a **Compositor**.

This enables:

* surfacing
* layers
* overlays
* multi-level modals
* tooltips
* window-local popups

Windows do not see each others’ surfaces.

---

# **12. Layout & Style Integration**

Each window:

* runs full layout pipeline
* has its own style cascade
* computes animations independently
* has its own Animation Clock (SPEC-115)

Style invalidations do not propagate across windows.

---

# **13. Persistence and Restore**

Built-in support for remembering:

* size
* position
* maximized/fullscreen state
* last active window

Stored via user-defined persistence adapter.

---

# **14. Testing Matrix**

### **Window Lifecycle**

* create/destroy
* hide/show
* minimize/restore
* DPI change
* multi-monitor move

### **Input**

* pointer cross-window handoff
* active window focus rules
* keyboard focus
* global hotkeys

### **Compositor**

* independent render stacks
* modal correctness
* popup anchoring per window

### **Performance**

* multiple swap chains
* shared GPU device
* stress on glyph pipeline
* no memory leaks

---

# **15. Implementation Plan**

### **Phase 1 — Core Window Abstraction**

* HalcyonWindow struct
* WindowState
* basic lifecycle

### **Phase 2 — WindowManager**

* registry
* focus tracking
* OS event routing

### **Phase 3 — Multi-Window Render Loops**

* per-window render
* swap chain management

### **Phase 4 — Compositor Integration**

* 1 compositor per window
* surface isolation

### **Phase 5 — Multi-Window Event Routing**

* cross-window resolution
* drag handover
* window focus logic

### **Phase 6 — DPI & Scaling**

* logical/physical sync
* atlas reraster triggers

### **Phase 7 — Window Types**

* tool, dialog, popup

### **Phase 8 — Persistence**

* initial restore
* save on exit
