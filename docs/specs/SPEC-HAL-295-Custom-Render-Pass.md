# SPEC-HAL-295: **Custom Render Pass Extension Point**

### *Offscreen Texture Model for GPU-Native Widget Rendering*

**Author:** JC
**Status:** Implemented / Closed
**Version:** 4 — finalized against implemented custom render pass lifecycle
**Amended by:** SPEC-HAL-295A (Adopted / Binding); SPEC-HAL-266A (Adopted / Binding)
**Normative parent clause:** SPEC-HAL-295 §§3–7 and §§11–13 are read subject to SPEC-HAL-295A. “External to the render graph” means only the admitted offscreen domain defined there. “Independent” means independently encoded and submitted; it does not mean independent authority over scene or presentation.
**Normative parent clause (SPEC-HAL-266A):** SPEC-HAL-295 §§4.2–4.3, 4.8, 5, 7, and 13 are read subject to SPEC-HAL-266A for target-dependent custom-render preparation. Target sizing and resize follow SPEC-HAL-266A §4; `GpuResources::scale_factor` is not target authority; texture allocation consumes the engine-supplied physical render extent; target alignment and visibility consume the engine-supplied physical composition coverage; and resize acceptance requires the distinct-surface and scale-change proof in SPEC-HAL-266A §9.
**Depends on:**

* SPEC-HAL-002 (Constitution & Architectural Invariants)
* SPEC-HAL-001 (Core Architecture)

---

# 1. Purpose

This SPEC defines the mechanism by which a widget can render content using
its own GPU pipelines (custom WGSL shaders, custom vertex formats, instanced
rendering), composited correctly with the rest of the UI.

The motivating use case is a candlestick chart that renders 1M+ candles via
instanced draw calls with custom fragment shaders for in-shader borders,
selection highlights, and hover effects. This cannot be expressed through
PaintSink (`push_rect`, `push_line`). The widget needs its own
`wgpu::RenderPipeline`, its own instance buffers, and its own shaders.

---

# 2. Problem

PaintSink provides high-level drawing primitives (rect, rounded_rect, text,
path, icon, gradient, shadow). These are sufficient for UI widgets.

A charting widget needs:

| Requirement | PaintSink | Custom Pipeline |
|-------------|-----------|-----------------|
| 1M candles at 120fps | ✗ (1M push_rect = 1M scene nodes) | ✓ (instanced draw, 1 call) |
| In-shader borders/selection | ✗ | ✓ (fragment shader) |
| f64 price → f32 screen transform | ✗ (PaintSink is f32 only) | ✓ (transform on CPU, f32 to GPU) |
| Custom vertex format (112 bytes/instance) | ✗ | ✓ (bytemuck::Pod) |
| Viewport culling before GPU upload | ✗ | ✓ (GeometryBuilder) |
| Multiple pipelines per widget (candle + overlay) | ✗ | ✓ |
| Independent frame rate (120fps chart, idle UI) | ✗ | ✓ (own encoder + submit) |

This extension point is implemented by the generic `CustomRenderPass`
lifecycle, front/back texture ownership, view-based registration, deferred
unregister-before-drop handling, and acceptance coverage for offscreen
rendering, double-buffer swaps, resize, composition, and stale registration
cleanup.

---

# 3. Design Principles

**P1: Offscreen texture isolation.** Custom rendering happens on the
widget's own offscreen texture. The widget creates its own command encoder,
runs its own render passes, and submits independently via `Arc<Queue>`.
Halcyon's render graph, scene graph, batcher, and main render pass are
completely unaware of the custom GPU work. Halcyon sees an image quad.

**P2: Custom rendering is compositable.** The offscreen texture is
displayed as a standard `ImageNode` in Halcyon's scene graph, clipped
and z-ordered like any other image. Halcyon widgets (labels, buttons,
annotations) render above the image quad through normal scene graph
compositing.

**P3: Custom renderers are long-lived.** Pipeline creation is expensive
(shader compilation, pipeline layout). Pipelines and persistent GPU
buffers are created once at initialization and reused across frames.
Per-frame work is limited to geometry building and buffer uploads.

**P4: Independent frame rate.** The widget can render to its offscreen
texture at any rate — 120fps continuous even while the UI is idle.
Halcyon displays whatever texture is current. The two frame loops are
decoupled.

**P5: No PaintSink contamination.** PaintSink remains the high-level UI
drawing API. Custom rendering is a separate mechanism. Widgets use
PaintSink for UI chrome (axis labels, gridlines) and the offscreen
texture for domain content (candles, wicks, overlays).

**P6: No render graph changes.** Halcyon's render graph
(`RenderGraphBuilder`, `PassExecutor`, `DrawPlan`) is not modified.
No new `SceneNode` variants. No batcher changes. No INV-RG-015
relaxation. The custom widget's GPU work is external to the render graph.

---

# 4. Architecture

## 4.1 Implemented State

Halcyon has the finalized display-side substrate and custom-render lifecycle:

| Component | Implemented state | Location |
|-----------|-------------------|----------|
| `Arc<wgpu::Device>` access | Shared device access for widget-owned GPU resources | `WgpuRenderer::device()` and `WgpuRenderer::gpu_resources()` |
| `Arc<wgpu::Queue>` access | Shared queue access for independent widget submission | `WgpuRenderer::queue()` and `WgpuRenderer::gpu_resources()` |
| Surface texture format access | Offscreen textures use the renderer surface format | `WgpuRenderer::surface_format()` and `GpuResources::format` |
| Texture ownership registration | Renderer-owned texture registration remains available for ownership-transfer cases | `WgpuRenderer::register_image_texture()` |
| External texture-view registration | Widget-owned front texture views register without transferring texture ownership | `WgpuRenderer::register_image_from_view()` / `register_image_from_view_nearest()` |
| Texture unregistration | Registered texture IDs can be removed before widget-owned texture drop | `WgpuRenderer::unregister_image()` |
| Deferred widget registration queue | Widget registrations are drained before draw-plan compile | `GpuResources::pending_registrations` |
| Scene image quad | Custom-render output is displayed as a normal image node | `ImageNode` and `SceneBuilder::push_image()` |
| PaintSink image API | Widgets paint offscreen output through the existing image path | `PaintSink::push_image()` forwards to `SceneBuilder::push_image()` |
| Image pipeline | The existing image pipeline samples registered `TextureView`s | `WgpuRenderer::image_pipeline` and registered `ImageTextureEntry` table |
| Generic custom-render lifecycle | Init, resize, render, register, unregister, and destroy are implemented for any custom-render widget | `src/renderer/custom_render_pass.rs` |
| Double buffering | Widget-owned front/back textures isolate Halcyon sampling from widget rendering; submit happens before swap | `src/renderer/custom_render_pass.rs` |
| Presentation-coupled registration | A custom renderer may defer front registration until its UI-thread owner can publish associated presentation state in the same application frame | `src/renderer/custom_render_pass.rs`, `src/plot/` |
| Registration ownership enforcement | Pending unregisters retain old textures until renderer drain; stale pending registrations are discarded on immediate register, resize, and destroy | `src/renderer/custom_render_pass.rs`, `src/renderer/wgpu_renderer.rs` |
| Plot integration | Plot consumes `CustomRenderPass` for offscreen texture lifecycle | `src/plot/renderer.rs`, `src/plot/mod.rs`, `src/plot/render_pool.rs` |
| Acceptance coverage | Tests cover registration, offscreen render readback, custom WGSL output, resize, double-buffer swap, unregister-before-drop, stale pending registration cleanup, clipping, z-order, and surrounding scene composition | `src/tests/custom_render_pass.rs` |

## 4.2 Lifecycle

```text
Widget construction (Screen::build or on_mount)
    ↓
Widget receives Arc<Device>, Arc<Queue>, TextureFormat
    ↓ (via GpuResources — see §4.3)
    ↓
Widget creates:
    ↓ - Custom render pipelines (candle_body.wgsl, overlay.wgsl)
    ↓ - Bind group layouts
    ↓ - Persistent GPU buffers (instance, vertex, uniform)
    ↓ - Two offscreen textures (front + back, double-buffered)
    ↓
Widget queues front texture view registration via PendingImageRegistration
    ↓
[per frame — widget's own timing, independent of Halcyon]
    ↓
Widget::prepare()
    ↓ GeometryBuilder builds CandleInstance[]
    ↓ queue.write_buffer(instance_data)
    ↓ queue.write_buffer(wick_vertices)
    ↓ queue.write_buffer(uniforms)
    ↓
Widget creates own CommandEncoder
    ↓ begin_render_pass (on back offscreen texture)
    ↓ set_pipeline(candle_pipeline)
    ↓ draw(0..6, 0..instance_count)
    ↓ set_pipeline(overlay_pipeline)
    ↓ draw(0..wick_count, 0..1)
    ↓ end_render_pass
    ↓
queue.submit(encoder.finish())
    ↓
Swap front/back textures
    ↓ Re-register front texture if swapped
    ↓
[Halcyon's frame — independent timing]
    ↓
Widget::paint()
    ↓ cx.scene.push_image(rect, texture_id, uv_rect, tint, z_index, key)
    ↓ cx.scene.push_text_with_backgrounds(...)  // axis labels
    ↓
Halcyon composites image quad with other widgets normally
```

## 4.3 GPU Resource Access

Widgets need GPU resources to create pipelines and buffers. A new
accessor type provides these:

```rust
/// GPU resource handles for widgets that perform custom rendering.
///
/// Obtained during widget initialization. Contains Arc-cloned handles
/// to the shared Device and Queue, the surface format, the deferred
/// texture registration queue, and display scale factor.
///
/// This is NOT a context type — it's a plain data struct that the
/// widget stores and uses throughout its lifetime.
pub struct PendingImageRegistration {
    pub texture_id: u64,
    pub view: wgpu::TextureView,
    pub nearest: bool,
}

pub struct GpuResources {
    pub device: Arc<wgpu::Device>,
    pub queue: Arc<wgpu::Queue>,
    pub format: wgpu::TextureFormat,
    pub pending_registrations: Arc<Mutex<Vec<PendingImageRegistration>>>,
    pub scale_factor: f32,
}
```

WgpuRenderer exposes these via accessors:

```rust
impl WgpuRenderer {
    /// Get a clone of the shared Device handle.
    pub fn device(&self) -> Arc<wgpu::Device> {
        self.device.clone()
    }

    /// Get a clone of the shared Queue handle.
    pub fn queue(&self) -> Arc<wgpu::Queue> {
        self.queue.clone()
    }

    /// Get the surface texture format.
    pub fn surface_format(&self) -> wgpu::TextureFormat {
        self.surface_format
    }

    /// Convenience: get all GPU resources needed for custom rendering.
    pub fn gpu_resources(&self) -> GpuResources {
        GpuResources {
            device: self.device.clone(),
            queue: self.queue.clone(),
            format: self.surface_format,
            pending_registrations: self.pending_registrations.clone(),
            scale_factor: 1.0,
        }
    }
}
```

**INV-295-RES1:** `GpuResources` contains only shareable renderer
handles, the deferred texture registration queue, and scalar display
metadata. It does not grant mutable renderer ownership. Multiple widgets
can hold their own `GpuResources` simultaneously. No exclusive access,
no borrowing conflicts.

## 4.4 Offscreen Texture Creation

The widget creates its own offscreen textures with both
`RENDER_ATTACHMENT` (so the widget can render to it) and
`TEXTURE_BINDING` (so Halcyon can sample from it):

```rust
fn create_offscreen_texture(
    device: &wgpu::Device,
    width: u32,
    height: u32,
    format: wgpu::TextureFormat,
) -> wgpu::Texture {
    device.create_texture(&wgpu::TextureDescriptor {
        label: Some("Plot Offscreen Texture"),
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
             | wgpu::TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    })
}
```

**INV-295-TEX1:** Offscreen textures use the same format as Halcyon's
surface (`surface_format`). This ensures the image pipeline can sample
them without format conversion.

**INV-295-TEX2:** Offscreen textures include both `RENDER_ATTACHMENT`
and `TEXTURE_BINDING` usage flags. The widget renders TO them; Halcyon
samples FROM them.

## 4.5 Double Buffering

Two textures: front (Halcyon reads) and back (widget writes). On frame
completion, swap and re-register:

```rust
// After widget finishes rendering to back texture:
std::mem::swap(&mut self.front_texture, &mut self.back_texture);
// Re-register the new front texture view with Halcyon.
let view = self.front_texture.create_view(&wgpu::TextureViewDescriptor::default());
self.gpu.pending_registrations.lock().unwrap().push(PendingImageRegistration {
    texture_id: self.texture_id,
    view,
    nearest: true,
});
```

**INV-295-DB1:** Halcyon always samples from the front texture. The
widget always renders to the back texture. Swap is atomic (pointer
swap, no GPU sync needed — the submit() for the back texture has
already been called before the swap).

## 4.6 PaintSink Extension

`push_image` must be added to the `PaintSink` trait so widgets can
display offscreen textures through the standard painting API:

```rust
// Addition to PaintSink trait:
fn push_image(
    &mut self,
    rect: RectF,
    texture_id: u64,
    uv_rect: RectF,
    tint: Color,
    z_index: ZIndex,
    key: Option<SceneNodeKey>,
);
```

The `impl PaintSink for SceneBuilder` implementation forwards to
`SceneBuilder::push_image()`.

## 4.7 Widget Paint

The widget's paint method is simple — push an image quad:

```rust
fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
    // 1. Display the offscreen texture as an image quad
    cx.scene.push_image(
        rect,
        self.texture_id,
        RectF::new(0.0, 0.0, 1.0, 1.0),  // full UV
        Color::from_linear(1.0, 1.0, 1.0, 1.0),  // no tint
        LAYER_CONTENT,
        self.scene_key(),
    );

    // 2. Paint UI chrome via regular PaintSink (axis labels, etc.)
    self.paint_axes(cx, rect);
    self.paint_crosshair_labels(cx, rect);
}
```

## 4.8 Texture Resize

> **Normative parent clause:** SPEC-HAL-295 §§4.2–4.3, 4.8, 5, 7, and 13 are read
> subject to SPEC-HAL-266A for target-dependent custom-render preparation.
> Target sizing and resize follow SPEC-HAL-266A §4;
> `GpuResources::scale_factor` is not target authority; texture allocation
> consumes the engine-supplied physical render extent; target alignment and
> visibility consume the engine-supplied physical composition coverage; and
> resize acceptance requires the distinct-surface and scale-change proof in
> SPEC-HAL-266A §9.

When the widget's layout rect changes, the offscreen textures must be
recreated at the new size:

```rust
fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints, ...) -> LayoutResult {
    let size = constraints.max; // or computed size
    let physical_w = (size.width * cx.scale_factor()) as u32;
    let physical_h = (size.height * cx.scale_factor()) as u32;

    if physical_w != self.texture_width || physical_h != self.texture_height {
        // Recreate offscreen textures at new size
        self.front_texture = create_offscreen_texture(&self.gpu.device, physical_w, physical_h, self.gpu.format);
        self.back_texture = create_offscreen_texture(&self.gpu.device, physical_w, physical_h, self.gpu.format);
        self.texture_width = physical_w;
        self.texture_height = physical_h;
        // Re-register front texture
        // (via deferred action or direct renderer access)
    }

    LayoutResult::fit(size)
}
```

**INV-295-RSZ1:** Custom target-dependent resources are created or resized only
during the SPEC-HAL-266A post-layout preparation boundary from the
engine-supplied physical render extent. Layout records logical geometry only;
paint emits an Image only.

---

# 5. Physical Rect

```rust
/// Rectangle in physical pixels (post-scale-factor).
///
/// Used for viewport uniforms, GPU coordinate mapping,
/// and offscreen texture sizing.
/// Distinct from RectF (logical pixels) to prevent unit confusion.
#[derive(Debug, Clone, Copy)]
pub struct PhysicalRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
```

---

# 6. Data Flow

```text
Trading Engine
    ↓ (Arc/ArcSwap<FrameSnapshot>)
Widget (owns render-local cache and GpuResources)
    ↓
Widget::prepare()  [widget's render timing]
    ↓ GeometryBuilder builds CandleInstance[]
    ↓ queue.write_buffer(instance_data)
    ↓ queue.write_buffer(wick_vertices)
    ↓ queue.write_buffer(uniforms)
    ↓
Widget creates own CommandEncoder
    ↓ begin_render_pass (on back offscreen texture)
    ↓ render candle pipeline (instanced)
    ↓ render overlay pipeline (wicks, crosshair)
    ↓ end_render_pass
    ↓
queue.submit(encoder.finish())
    ↓ swap front/back textures
    ↓
Halcyon Frame  [Halcyon's timing]
    ↓ Widget::paint() → push_image(texture_id, rect)
    ↓ Normal scene graph compositing
    ↓ Halcyon's render graph processes ImageNode
    ↓ Image pipeline samples from front texture
    ↓
Screen
```

---

# 7. Invariants

**INV-295-1: Offscreen Isolation**

Custom rendering happens on the widget's own offscreen texture. The
widget's GPU work (command encoder, render passes, pipelines) is
completely external to Halcyon's render graph. Halcyon's main render
pass never sees custom pipelines, instance buffers, or shaders. It
sees an `ImageNode` — a textured quad.

**INV-295-2: Independent Submission**

The widget creates its own `CommandEncoder` and submits its own command
buffer via `Arc<Queue>`. wgpu handles multi-submit safely — Device and
Queue are `Send + Sync`, and `queue.submit()` is thread-safe.

**INV-295-3: Phase Separation**

Pipeline and buffer creation happen during widget initialization (mount).
Geometry building and buffer uploads happen in the widget's prepare phase.
Offscreen rendering happens via the widget's own command encoder.
`paint()` only pushes an image quad — no GPU work in paint.

**INV-295-4: No Render Graph Changes**

Halcyon's render graph (`RenderGraphBuilder`, `PassExecutor`, `DrawPlan`,
`Batcher`) is not modified. No new `SceneNode` variants. No batcher
integration for custom nodes. No INV-RG-015 relaxation. The widget's
GPU work is invisible to the render graph.

**INV-295-5: No Scene Graph Mutation**

The widget's custom rendering does not mutate the scene graph, widget
tree, or application state. Paint pushes an image quad. That's it.

**INV-295-6: Texture Ownership**

The widget owns its offscreen textures. It creates them, renders to them,
and drops them. Halcyon holds a registered reference (`ImageTextureEntry`)
for sampling. When the widget is destroyed, it unregisters the texture.

**INV-295-6A: No Semantic State Ownership**

Custom render widgets may own GPU resources, offscreen textures, pipelines,
buffers, and rebuildable render caches. They MUST NOT become the authority
for application/domain data. If view or interaction state must be serialized,
shared, inspected externally, or coordinated with other widgets, that state
belongs to the application/session authority and is supplied to the custom
render widget as input.

**INV-295-7: Send + Sync**

`GpuResources` is `Send + Sync` because `Arc<Device>` and `Arc<Queue>`
are `Send + Sync`. The widget can perform GPU work on any thread.

**INV-295-8: Presentation-Coupled Registration**

When sibling scene primitives must correspond to one custom-rendered frame, the
worker may submit and swap without exposing the new front texture. The UI-thread
owner then queues that front registration and publishes the associated pure-data
presentation snapshot before sibling paint. No later worker frame may reuse the
registered texture as a back buffer while presentation is pending.

---

# 8. What the Chart Renderer Becomes

The original standalone chart renderer owns its own window, surface, event loop,
and render passes. Under this SPEC, it is decomposed into:

| Current (standalone) | Under Halcyon |
|---------------------|---------------|
| `App` struct (owns window, surface, device, queue) | Gone — widget holds `GpuResources` (Arc clones from Halcyon) |
| `render_frame()` (acquires surface, creates encoder, submits) | Split: geometry in `prepare()`, render to offscreen texture, submit via `Arc<Queue>` |
| Pipeline creation (in `resumed()`) | Moved to widget init (receives `GpuResources`) |
| Buffer management (ensure_buffer_capacity) | Stays — in widget's prepare phase |
| Geometry building (GeometryBuilder) | Stays — called in prepare |
| Input handling (window_event) | Moves to widget event handling that emits application/session intents |
| ViewState / InteractionState | External if semantic; widget may keep only transient render-local mechanics |
| candle_body.wgsl / overlay.wgsl | Unchanged — compiled at widget init |
| glyphon text rendering | Replaced by Halcyon's text engine (PaintSink) |
| `FrameSnapshot` / `RenderCandle` | Unchanged — data model stays |
| `Transform` | Unchanged — coordinate mapping stays |

---

# 9. Implemented Components

1. **`WgpuRenderer::device()`** — returns `Arc<wgpu::Device>`.
2. **`WgpuRenderer::queue()`** — returns `Arc<wgpu::Queue>`.
3. **`WgpuRenderer::surface_format()`** — returns `wgpu::TextureFormat`.
4. **`WgpuRenderer::gpu_resources()`** — returns `GpuResources`.
5. **`push_image()` on PaintSink trait** — forwards to `SceneBuilder::push_image()`.
6. **`GpuResources` struct** — cloneable data containing GPU handles, registration queue, format, and scale factor.
7. **Deferred image registration drain** — widget registrations are drained before draw-plan compile.
8. **Plot offscreen rendering path** — proves one concrete custom GPU consumer.
9. **Generic lifecycle contract** — custom-render widgets have a binding protocol for GPU init, resize, render, texture registration, unregistration, and destruction.
10. **Double-buffer implementation** — custom-render widgets own front/back offscreen textures and swap only after back-texture submit.
11. **View-based registration as default** — widget-owned textures register their current front `TextureView`; ownership-transfer registration remains available only when the renderer is intended to own the texture.
12. **Deferred unregister-before-drop handling** — deferred resize and destroy paths retain widget-owned textures until the renderer drains the unregister.
13. **Stale pending registration cleanup** — direct register, direct resize, direct destroy, deferred destroy, and repeated resize discard superseded pending registrations before the renderer drains them.
14. **Feature tests** — cover offscreen render, resize, double-buffer swap, external-view registration, unregistration before drop, stale pending registration cleanup, z-order, clipping, and surrounding widgets.

No render graph changes. No scene graph changes. No batcher changes.
No new SceneNode variants. No PassExecutor modifications.

---

# 10. Future Extensions

- **Render thread** — the widget can spawn a dedicated thread that
  loops at 120fps, builds geometry, renders to the back texture, swaps,
  and re-registers. The Halcyon UI thread just displays whatever front
  texture is current. This is possible because `Arc<Device>` and
  `Arc<Queue>` are thread-safe.

- **Shared pipelines** — multiple chart widgets could share compiled
  pipelines (same shaders) with different instance data. Pipeline
  handles are just `Arc<RenderPipeline>`.

- **Async pipeline compilation** — `device.create_render_pipeline_async()`
  for non-blocking shader compilation. The widget renders a placeholder
  via PaintSink until the pipeline is ready.

- **Compute shaders** — if a widget needs GPU compute (e.g., indicator
  calculation on GPU), the same `GpuResources` pattern works. Create
  compute pipeline, dispatch, read back results. All external to
  Halcyon's render graph.

---

# 11. Consistency Matrix (SPEC-HAL-002A Required)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Widget owns textures, pipelines, buffers, and render-local cache. Application/session owns semantic data. Halcyon owns widget lifecycle. |
| B — Phase Separation | Preserved. Widget's GPU work is external to Halcyon's phases. Paint only pushes an image quad. |
| C — ID-based Identity | Preserved. Texture referenced by u64 ID. No cross-widget references. |
| D — Pure Data | Preserved. GpuResources contains shareable handles, a deferred registration queue, and scalar display metadata. It carries no mutable renderer authority. |
| E — GPU-First | Strengthened. Widgets can use full GPU capabilities via offscreen textures. |
| F — Sovereignty | Preserved. Halcyon controls display. Widget controls its own rendering. Clean boundary. |

**INV-RG-001 (orchestration only):** Unchanged. The render graph still
does orchestration only. The widget's GPU work is external.

**INV-RG-002 (DrawPlan is sole input):** Unchanged. The DrawPlan
contains an `ImageNode` for the offscreen texture, same as any other
image.

**INV-RG-015 (constrained executor access):** Unchanged. No relaxation
needed. The widget has its own `Arc<Device>` and `Arc<Queue>` — it does
not receive them through `PassContext` or any render graph mechanism.

---

# 12. Non-Goals

- Modifying Halcyon's render graph, batcher, or scene graph
- Adding new SceneNode variants for custom rendering
- Exposing Halcyon's internal pipelines (rect, text, path) to widgets
- Plugin/hot-reload system for custom shaders
- Custom renderers for standard UI widgets (use PaintSink)
- Shared render pass (widget rendering inside Halcyon's main pass)

---

# 13. Acceptance Criteria — Implemented

All criteria below are implemented by the current custom render pass code and
focused acceptance coverage. Criterion 16 is enforced structurally: windows
without custom-render widgets do not allocate custom-render textures, custom
pipelines, or custom render passes.

1. `WgpuRenderer` exposes `device()`, `queue()`, `surface_format()` accessors
2. `PaintSink` trait has `push_image()` method
3. A widget can create offscreen textures with RENDER_ATTACHMENT + TEXTURE_BINDING
4. A widget can create its own render pipelines from custom WGSL shaders
5. A widget can render to its offscreen texture via its own CommandEncoder
6. A widget can submit GPU work via `Arc<Queue>` independently of Halcyon's frame
7. A widget owns front/back offscreen textures and never renders into the texture Halcyon is currently sampling
8. A completed custom render swaps front/back only after submitting the back-texture command buffer
9. The active front texture is registered through a current `TextureView` without transferring semantic or texture ownership to Halcyon
10. Texture registration is drained before draw-plan compile
11. Destroying a custom-render widget unregisters its texture before dropping widget-owned textures
12. The offscreen texture displays correctly as an image quad in the widget's rect
13. Other widgets (buttons, labels, text fields) render correctly around the image quad
14. Axis labels rendered via PaintSink composite above the image quad
15. Texture resize on layout change works without artifacts
16. Windows without offscreen-rendering widgets allocate no custom-render textures, custom pipelines, or custom render passes
17. Presentation-coupled custom frames expose their front texture and sibling pure-data state in one application frame
