# **SPEC-HAL-004: Halcyon — WGPU & Winit Integration (Render Loop, Platform Layer, Frame Timing)**

**Author:** JC
**Date:** 2025-11-16
**Status:** Draft
**Scope:** Platform layer, GPU setup, main UI loop, frame pacing, winit event translation, surface/adapter/device creation, and the pipeline by which Halcyon takes a `Scene` and pushes it to the GPU.
**Primary Use-Case:** High-FPS trading UIs with stable frame pacing and ultra-low latency (charts, ladders, heatmaps, blotters).

---

# **1. Goals**

WGPU + Winit integration must:

* Initialize GPU with correct limits for UI workloads.
* Run a **predictable frame loop** (vsync or unlocked).
* Map **winit events** → Halcyon `Event` cleanly.
* Handle DPI scaling, window resizing, and swapchain recreation.
* Upload **Scene** → GPU primitives with minimal overhead.
* Support **thousands of draw calls merged into batched pipelines**.
* Provide a **clean separation**:

    * `platform_winit.rs` handles OS events & window.
    * `renderer_wgpu.rs` handles GPU pipelines & drawing.
    * `AppRunner` orchestrates dispatch + scene generation.

Non-goals (other SPECs will cover):

* Full text shaping (later SPEC-HAL-006-Typography).
* GPU instancing for complex widgets (later).
* Offscreen layers / caching (later).
* Multi-window support (later).

---

# **2. Files Involved**

Two main modules introduced:

```
src/
  platform_winit.rs     // window, event loop, mapping input → Halcyon events
  renderer_wgpu.rs       // GPU setup, swapchain, pipelines, draw calls
```

Both called by `AppRunner`.

---

# **3. Winit Integration (platform_winit.rs)**

## **3.1. Responsibilities**

`platform_winit.rs`:

1. Creates window + handles DPI scale factor.
2. Owns the main `event_loop`.
3. Receives raw OS events from winit.
4. Translates them into Halcyon `Event` (Pointer/Key/Window).
5. Feeds them to the app:

    * `app.handle_event()`
    * event routing is handled by AppRunner / WidgetTree.
6. Requests redraw.
7. Calls renderer every frame:

    * `renderer.render(&Scene)`.

### The platform layer **never** touches widgets or layout.

---

## **3.2. Window Creation**

We use winit 0.29 API shape:

```rust
let event_loop = EventLoop::new()?;
let window = WindowBuilder::new()
    .with_title("Halcyon App")
    .build(&event_loop)?;
```

We must fetch:

* `window.inner_size()`
* `window.scale_factor()` for DPI

Later passed to renderer.

---

## **3.3. Event Translation**

Winit → Halcyon:

| Winit Event                      | Halcyon Event                         |
| -------------------------------- | ------------------------------------- |
| `WindowEvent::CursorMoved`       | `Event::Pointer(PointerEvent)`        |
| `WindowEvent::MouseInput`        | PointerDown/PointerUp                 |
| `WindowEvent::MouseWheel`        | Scroll                                |
| `WindowEvent::KeyboardInput`     | KeyDown/KeyUp                         |
| `WindowEvent::ReceivedCharacter` | KeyEvent with text                    |
| `WindowEvent::Resized`           | `Event::Window(WindowEvent::Resized)` |
| `WindowEvent::CloseRequested`    | CloseRequested                        |

Critical transformation:

* Convert window physical coords → logical pixels → Halcyon root coords:

```rust
let scale = window.scale_factor() as f32;
let logical_pos = (physical_x as f32 / scale, physical_y as f32 / scale);
```

This makes Halcyon DPI-aware by default.

---

## **3.4. Event Loop Model**

We follow winit's "poll" model (not wait):

```rust
event_loop.run(move |event, target| {
    match event {
        Event::WindowEvent { event, .. } => { ... }
        Event::RedrawRequested(_) => { ... }
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        _ => {}
    }
});
```

**Every frame:**

1. Handle queued OS events → Halcyon events
2. `app.handle_event(..)`
3. AppRunner applies event routing + capture logic
4. AppRunner triggers `app.build_scene()`
5. Scene sent to renderer
6. GPU presents frame

This ensures stable frame pacing.

---

# **4. WGPU Integration (renderer_wgpu.rs)**

## **4.1. Responsibilities**

Renderer must:

* Initialize:

    * Instance
    * Surface
    * Adapter
    * Device & Queue
    * Swapchain config
* Create pipelines:

    * Solid rect
    * Rounded rect
    * Text
* Maintain:

    * Dynamic vertex buffer pool
    * Uniform buffer (viewport + DPI scaling)
* Convert Scene → Draw calls
* Redraw on:

    * Resize
    * Window recreate
    * DPI change
    * Scene update

This system is modeled after modern Rust UI stacks (Dioxus-native, Slint, vello-style thinking).

---

## **4.2. GPU Initialization (Exact Steps)**

Create instance:

```rust
let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
    backends: wgpu::Backends::PRIMARY,
    ..Default::default()
});
```

Create surface:

```rust
let surface = unsafe { instance.create_surface(&window) }?;
```

Request adapter:

```rust
let adapter = instance.request_adapter(&wgpu::RequestAdapterOptions {
    power_preference: wgpu::PowerPreference::HighPerformance,
    compatible_surface: Some(&surface),
    force_fallback_adapter: false,
}).await.unwrap();
```

Request device:

```rust
let (device, queue) = adapter.request_device(
    &wgpu::DeviceDescriptor {
        features: wgpu::Features::empty(),
        limits: wgpu::Limits::downlevel_webgl2_defaults(),
        label: None,
    },
    None,
).await?;
```

**Note:** Trading UI benefits from low-latency, not high features.
We keep features minimal unless needed (e.g. storage buffers for large scenes).

---

## **4.3. Swapchain / SurfaceConfig**

Dimensions derived from window physical size:

```rust
let size = window.inner_size();

let config = wgpu::SurfaceConfiguration {
    usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
    format: preferred_format,
    width: size.width,
    height: size.height,
    present_mode: wgpu::PresentMode::Fifo, // vsync; or Immediate for unleashing FPS
    alpha_mode: composite_alpha,
    view_formats: vec![],
};
surface.configure(&device, &config);
```

We must handle:

* Resize → reconfigure
* DPI → recalc config
* OS scale factor changes

---

## **4.4. Render Loop (Exact)**

Renderer exposes:

```rust
pub fn render(&mut self, scene: &Scene) -> Result<(), wgpu::SurfaceError>;
```

Steps:

1. Acquire surface texture:

```rust
let frame = match surface.get_current_texture() {
    Ok(f) => f,
    Err(e) => { self.recreate_surface(); return Err(e); }
};
let view = frame.texture.create_view(&Default::default());
```

2. Create encoder:

```rust
let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
```

3. Begin render pass:

```rust
let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
        view: &view,
        resolve_target: None,
        ops: wgpu::Operations {
            load: wgpu::LoadOp::Clear(clear_color),
            store: true,
        },
    })],
    depth_stencil_attachment: None,
});
```

4. Convert `Scene` primitives → GPU draws via pipelines.

5. End render pass.

6. Submit + present:

```rust
queue.submit(Some(encoder.finish()));
frame.present();
```

This matches TradingView, Binance, and general real-time chart engines.

---

# **5. Mapping `Scene` to GPU**

`Scene` is a flat vector:

```rust
pub struct Scene {
    pub primitives: Vec<Primitive>,
}
```

Renderer converts primitives:

| Scene Primitive | GPU Primitive                | Notes        |
| --------------- | ---------------------------- | ------------ |
| Rect            | Triangle strip / 2 triangles | Batched      |
| RoundedRect     | Rounded shader or 9-slice    | Later SPEC    |
| Text            | Glyphon-based text runs      | Cached atlas |
| Image           | Bind group + pipeline        | Later        |

We batch by **pipeline type**, not per primitive.

Example: all rects drawn in 1 draw call.

---

# **6. Frame Pacing & Latency**

Trading UIs require low latency and deterministic loop timing.

We support two modes:

### **6.1. VSync (Fifo) — stable visual but slight input latency**

Good for charts, typical UI.

### **6.2. Unlocked (Immediate/Mailbox) — minimal latency**

Used for:

* ladders
* scalping windows
* order-entry widgets

We expose via API:

```rust
pub enum PresentMode {
    VSync,
    LowLatency,
}
```

User sets in app initialization.

---

# **7. DPI Scaling (Critical)**

When DPI changes:

* Recalculate `scale_factor`
* Recompute UI layout
* Rebuild Scene
* Reconfigure swapchain
* Recompute uniform buffer for viewport

This guarantees crisp rendering at any resolution.

---

# **8. Error Handling**

Surface errors we must handle gracefully:

* `Lost` → recreate surface
* `OutOfMemory` → fatal, exit
* `Timeout` → retry next frame
* `Outdated` → reconfigure

This is important for multi-monitor setups and docking windows.

---

# **9. Responsibilities Separation**

To avoid contamination:

### `AppRunner`

* Manages App instance
* Interprets event routing
* Triggers layout + scene building

### `platform_winit`

* OS interaction
* Event intake
* Mapping winit → Halcyon `Event`

### `renderer_wgpu`

* GPU setup
* Scene → device.draw

This separation ensures Halcyon components remain reusable.

---

# **10. Implementation Checklist (Claude Code-ready)**

### **In `platform_winit.rs`:**

* [ ] Create window + event_loop
* [ ] Map pointer events → Halcyon coordinates
* [ ] Map key events → KeyEvent
* [ ] Map window resize → WindowEvent
* [ ] Forward events into AppRunner
* [ ] Call renderer on `RedrawRequested`
* [ ] Request redraw on `MainEventsCleared`
* [ ] Handle DPI changes

### **In `renderer_wgpu.rs`:**

* [ ] Initialize:

    * Instance
    * Surface
    * Adapter
    * Device + Queue
    * Swapchain config
* [ ] Build pipelines:

    * Rect
    * Text
    * RoundedRect (stub for now)
* [ ] Create uniform buffers (viewport)
* [ ] Implement `render(scene)`:

    * Acquire texture
    * Begin render pass
    * Draw primitives
    * Submit queue
    * Present frame
* [ ] Handle surface errors

### **AppRunner:**

* [ ] Integrate pointer capture + focus manager
* [ ] Rebuild widget tree when state changes
* [ ] Produce Scene every frame

---

# **11. Success Criteria**

* Stable 60/120/Unlimited FPS.
* Millisecond-range event-to-pixel latency.
* No stutters during resize.
* No flickering.
* Correct DPI scaling.
* Zero allocations per frame.
* Charts operate smoothly inside Halcyon widget.
* Ladder widget receives ultra-high frequency pointer events reliably.
* Scene with thousands of primitives stays < 1 ms on GPU.

