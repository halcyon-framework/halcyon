**SPEC-HAL-003: Halcyon — Module & File Layout + Core Skeleton**

**Author:** JC
**Date:** 2025-11-16 (Updated: 2026-01-21)
**Status:** Implemented
**Scope:** Repo layout, core modules, and crate structure for Halcyon.
**Primary Use-Case:** Basis for a trading-grade UI stack (charts, ladders, blotters, etc.).

---

## 1. Repository Layout

Single crate with organized module hierarchy:

```text
halcyon/
  Cargo.toml
  Cargo.lock
  rust-toolchain.toml
  .rustfmt.toml

  src/
    lib.rs                   // Public API re-exports
    main.rs                  // Binary entry point
    prelude.rs               // Common imports

    core/                    // Core framework
      mod.rs
      app.rs                 // App runner, event loop, frame scheduling
      layout.rs              // Constraints, Size, layout context
      paint.rs               // Paint context
      theme.rs               // Theme structs, colors, typography
      style.rs               // Style system
      math.rs                // Math primitives (RectF, Point, etc.)
      widget_tree.rs         // Widget tree structure
      hit_test.rs            // Hit testing
      event_router.rs        // Event dispatch and routing
      scheduler.rs           // Frame scheduling
      animation.rs           // Animation system

      event/                 // Event types and guards
        mod.rs
        guard.rs
        forbidden.rs

      scene/                 // Scene graph
        mod.rs
        guard.rs
        forbidden.rs

      scene_diff/            // Scene diffing
        mod.rs
        guard.rs
        forbidden.rs

      widget/                // Widget trait and guards
        mod.rs
        guard.rs
        forbidden.rs

      transition/            // Layout transitions
        mod.rs
        transition_layer.rs

      // Text subsystem
      text_engine.rs         // TextEngine trait
      text_shaper.rs         // Text shaping
      text_linebreaker.rs    // Line breaking
      paragraph.rs           // Paragraph layout
      glyph_quad.rs          // Glyph quad generation

      // Editing subsystem
      editing.rs             // Text editing operations
      editing_history.rs     // Undo/redo
      text_edit_delegate.rs  // Edit delegate trait
      text_edit_policy.rs    // Edit policies

      // Scrolling subsystem
      scroll_container.rs    // Scroll container
      scroll_state.rs        // Scroll state management
      scroll_link.rs         // Linked scrolling

      // Other core components
      input_fusion.rs        // Input event fusion
      input_projection.rs    // Input coordinate projection
      overlay_root.rs        // Overlay management
      rect_authority.rs      // Rect mutation authority
      scene_identity.rs      // Scene node identity
      scene_frame_guard.rs   // Frame guard
      deferred_wiring.rs     // Deferred widget wiring
      action_palette_modal.rs // Action palette modal
      layout_transition.rs   // Layout transition helpers

    widgets/                 // Widget implementations
      mod.rs
      button.rs
      checkbox.rs
      radio_button.rs
      radio_group.rs
      radio_group_widget.rs
      label.rs
      text_block.rs
      text_field.rs
      text_area.rs
      text_edit.rs
      text_document.rs
      text_layout_cache.rs
      text_widget_test_harness.rs
      container.rs
      panel.rs
      flex.rs
      flex_spacer.rs
      fixed_spacer.rs
      flow_grid.rs
      divider.rs
      scroll_area.rs
      scrollbar.rs
      list.rs
      list_item.rs
      list_state.rs
      list_interaction_policy.rs
      list/                  // List submodule
        demo.rs
      legend.rs
      screen_host.rs
      action_palette.rs

    renderer/                // GPU rendering
      mod.rs
      wgpu_renderer.rs       // Main WGPU renderer
      batcher.rs             // Draw call batching
      draw_plan.rs           // Draw plan representation
      draw_plan_builder.rs   // Draw plan construction
      compiled_draw_plan.rs  // Compiled GPU commands
      draw_plan_guard.rs     // Draw plan authority
      render_target.rs       // Render target management
      glyph_atlas.rs         // Glyph atlas/cache
      gpu_arena.rs           // GPU memory arena
      staging_pool.rs        // Staging buffer pool
      vertex.rs              // Vertex definitions
      text_vertex.rs         // Text-specific vertices
      path.rs                // Path representation
      path_renderer.rs       // Path rendering
      authority_firewall_guard.rs

      pipelines/             // GPU pipelines
        mod.rs
        shape_pipeline.rs    // Shape/rect pipeline
        text_pipeline.rs     // Text pipeline
        path_pipeline.rs     // Path pipeline

      render_graph/          // Render graph system
        mod.rs
        builder.rs
        executor.rs
        graph.rs
        pass.rs
        resources.rs
        topology.rs
        guard.rs
        forbidden.rs
        passes/
          mod.rs
          background_prepass.rs
          main_pass.rs

      shaders/               // WGSL shaders

    compositor/              // Surface composition
      mod.rs
      contract.rs            // Compositor contract
      stack.rs               // Surface stack
      surface.rs             // Surface representation
      surface_role.rs        // Surface roles
      abi_guard.rs
      authority_firewall_guard.rs

    platform/                // Platform abstraction
      mod.rs
      window.rs              // Window management
      clipboard.rs           // Clipboard access
      font_loader.rs         // Font loading
      font_manager.rs        // Font management
      font_manager_text_engine.rs
      font_resolver.rs       // Font resolution
      system_font_provider.rs
      backend_text_engine.rs
      native_font_backend.rs
      native_font_backend_coretext.rs    // macOS
      native_font_backend_directwrite.rs // Windows
      native_font_backend_freetype.rs    // Linux
      native_font_backend_glyphon.rs     // Fallback
      text_engine_glyphon.rs

    runtime/                 // Runtime/action system
      mod.rs
      actions.rs             // Action definitions
      builtins.rs            // Built-in actions

    model/                   // Data models
      mod.rs

    utils/                   // Utilities
      mod.rs
      id.rs                  // ID generation
      coords.rs              // Coordinate utilities
      macros.rs              // Helper macros

    tests/                   // Cross-module engine-internal tests
      mod.rs
      focus_scroll_neutrality.rs
      interaction_domain_isolation.rs
      linked_scroll_engine.rs
      list_item_rebind.rs

  examples/                  // Demo applications
    text_area_demo.rs
    text_field_demo.rs
    textblock_showcase.rs
    button_hover_animation.rs
    checkbox_demo.rs
    radio_button_demo.rs
    list_transactional_demo.rs
    virtualized_list_minimal.rs
    flow_grid_layouts.rs
    flow_grid_legends.rs
    flow_grid_megatest.rs
    flow_grid_stress_test.rs
    layout_transition_basic_demo.rs
    compositor_single_surface_demo.rs
    compositor_surface_order_demo.rs
    multi_window_demo.rs
    shortcut_demo.rs
    shape_primitives.rs
    shaping.rs
    input_fusion_debug.rs
    min_hit_test.rs
    boundary_test.rs
    rounded_rect_clip_guard.rs
    text_placement_benchmark.rs
    text_quality_test.rs
    renderer_alpha_probe.rs
    renderer_border_ring_invariant.rs
    renderer_opacity_reference.rs
    renderer_path_rect_parity.rs
    SPEC018_demo/             // Multi-screen demo
      main.rs
      screens/
        mod.rs
        main_screen.rs
        debug_screen.rs
        simple_modal.rs

  tests/                      // Black-box integration test crates

  docs/                      // Documentation
    specs/                    // SPEC specifications
    *.md                     // Guides and architecture docs

  configs/                   // Configuration files
```

Test placement is structural:

- module-local `#[cfg(test)] mod tests` covers small private unit invariants;
- `src/tests/` covers cross-module engine internals only when the public API
  cannot prove the contract without architectural leakage; and
- root `tests/` covers black-box public behavior as separate integration crates.

New standalone Rust test files do not live outside `src/tests/` or root
`tests/`.

---

## 2. Module Architecture

### 2.1 Core (`src/core/`)

The core module contains the fundamental framework components:

- **App & Event Loop** (`app.rs`): Main application runner with winit event loop integration, frame scheduling, resize coalescing, and the build_scene pipeline.

- **Widget System** (`widget/`): The `Widget` trait with `layout()`, `paint()`, `handle_event()`, and `hit_test()` methods. Includes authority guards to enforce mutation invariants.

- **Layout Engine** (`layout.rs`): Constraint-based layout with `Constraints`, `Size`, and `LayoutContext`. Supports flexbox-style layout via the Flex widget.

- **Scene Graph** (`scene/`): Retained scene representation with primitives (Rect, Text, Path, Clip, etc.) built by widgets during paint.

- **Event System** (`event/`, `event_router.rs`): Input events, routing, focus management, hit testing, and keyboard/mouse/scroll handling.

- **Text Subsystem**: Complete text stack including `TextEngine` trait, HarfBuzz shaping (`text_shaper.rs`), line breaking (`text_linebreaker.rs`), and paragraph layout (`paragraph.rs`).

- **Editing**: Text editing operations (`editing.rs`), undo/redo history (`editing_history.rs`), and edit delegates.

- **Scrolling**: `ScrollContainer`, `ScrollState`, and linked scrolling support.

### 2.2 Widgets (`src/widgets/`)

Implemented widget library:

- **Text**: `TextField`, `TextArea`, `TextBlock`, `Label`, with `TextDocument` and `TextLayoutCache` backing
- **Controls**: `Button`, `Checkbox`, `RadioButton`, `RadioGroup`
- **Layout**: `Flex` (Row/Column), `Container`, `Panel`, `FlowGrid`, `FixedSpacer`, `FlexSpacer`, `Divider`
- **Scrolling**: `ScrollArea`, `Scrollbar`
- **Lists**: Virtualized `List` with `ListItem`, `ListState`, and interaction policies
- **Overlays**: `ActionPalette`, `ScreenHost`

### 2.3 Renderer (`src/renderer/`)

GPU rendering via wgpu:

- **WgpuRenderer** (`wgpu_renderer.rs`): Main renderer owning device, queue, surface, and orchestrating the render pass.

- **Pipelines** (`pipelines/`): Separate pipelines for shapes, text, and paths.

- **Render Graph** (`render_graph/`): DAG-based render pass scheduling with resource management.

- **Glyph Atlas** (`glyph_atlas.rs`): GPU texture atlas for rendered glyphs.

- **Draw Plan**: Scene-to-GPU translation via `DrawPlan` and batching.

### 2.4 Compositor (`src/compositor/`)

Surface composition layer:

- Multi-surface support with z-ordering
- Surface roles (main, overlay, popup)
- Compositor contracts for surface lifecycle

### 2.5 Platform (`src/platform/`)

Platform abstraction:

- **Window** (`window.rs`): Window creation and management via winit.
- **Clipboard** (`clipboard.rs`): System clipboard access.
- **Font Loading**: `FontLoader`, `FontManager`, `FontResolver` with platform-specific backends:
  - CoreText (macOS)
  - DirectWrite (Windows)
  - FreeType (Linux)
  - Glyphon fallback

### 2.6 Runtime (`src/runtime/`)

Action system for commands and shortcuts:

- Action definitions and dispatching
- Built-in actions (copy, paste, undo, redo, etc.)

---

## 3. Key Design Patterns

### 3.1 Authority Guards

Mutation of widget rects, scene nodes, and other critical state is protected by authority guards that enforce invariants at compile time and runtime. See `rect_authority.rs`, `widget/guard.rs`, `scene/guard.rs`.

### 3.2 Retained Widget Tree

Widgets are retained objects in a tree (`WidgetTree`). Layout and paint traverse the tree each frame. Widgets own their state and children.

### 3.3 Scene Diffing

The scene graph supports diffing (`scene_diff/`) for incremental updates, reducing GPU upload costs.

### 3.4 Text Layout Caching

Text widgets use `TextLayoutCache` to avoid re-shaping and re-laying-out text every frame. Cache invalidation is version-based.

### 3.5 Resize Coalescing

On Linux, resize events are coalesced to one per frame via `pending_resize` to avoid compositor starvation during interactive resize.

---

## 4. Related SPECs

- **SPEC-HAL-004**: WGPU/Winit Integration
- **SPEC-HAL-005**: Scene Graph
- **SPEC-HAL-006**: Text/Font System
- **SPEC-HAL-007**: State Model
- **SPEC-HAL-010**: Widget System
- **SPEC-HAL-011**: Tier 1 Widgets
- **SPEC-HAL-012**: Theming
- **SPEC-HAL-013**: TextField
- **SPEC-HAL-025**: Multi-line Text Layout
- **SPEC-HAL-042**: Frame Loop & Engine Ticker
- **SPEC-HAL-048**: Text Layout Engine
- **SPEC-HAL-054**: TextField Architecture
- **SPEC-HAL-079**: Unified Event Flow
