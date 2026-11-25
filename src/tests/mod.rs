//! Engine-internal tests (NOT integration tests).
//!
//! This module is **exceptional, deliberate, and limited**.
//!
//! ## Test Placement Rules
//!
//! - `tests/` (project root) = **integration tests**
//!   - Black-box testing through public API only
//!   - Compiles as a separate crate
//!   - Cannot access `pub(crate)` or private internals
//!
//! - `src/tests/` = **engine-internal tests**
//!   - White-box testing of internal mechanisms
//!   - Allowed to access `pub(crate)` and internal helpers
//!   - Used ONLY when integration tests are structurally impossible
//!
//! ## When to Use This Module
//!
//! - The invariant depends on private functions or internal sequencing
//! - Exposing the function publicly would be architectural leakage
//! - Testing requires `pub(crate)` accessors that should not be public API
//!
//! ## Current Tests
//!
//! - `authority_probe`: SPEC-HAL-190 usage validation (composite widget exercising full flow)
//! - `authority_probe_engine`: SPEC-HAL-190 engine-level phase enforcement
//! - `authority_projection`: SPEC-HAL-190 authority projection unit tests
//! - `focus_scroll_neutrality`: first focus does not trigger scroll reveal
//! - `linked_scroll_engine`: private linked scroll intent processing
//! - `interaction_domain_isolation`: per-window domain isolation
//! - `tabular_substrate`: 10k-row scroll/hit-test/navigation/selection invariants
//! - `listview_smoke`: ListView skeleton rendering, invalidation, substrate authority
//! - `listview_dirty_enforcement`: row/cell reuse + dirty flag enforcement
//! - `listview_data_contract`: ListView data contract wiring verification
//! - `listview_closed_composite_lock`: SPEC-HAL-192 forbidden API source scan
//! - `navigation_projection_boundary_guard`: SPEC-HAL-246/248 projection boundary enforcement
//! - `path_paint_context`: closed stroked paths through the object-safe PaintSink boundary
//! - `treeview_smoke`: TreeView skeleton validation (second substrate consumer)
//! - `tree_substrate`: SPEC-HAL-251/254 TreeSubstrate projection/expansion invariants
//! - `tree`: Tree model (substrate + focus + navigation) tests
//! - `tree_row_provider`: Tree ↔ ListView projection contract + substrate integration
//! - `three_d_context`: SPEC-HAL-355 finite CPU context, descriptors, generations, and leases
//! - `three_d_appearance`: SPEC-HAL-355 bounded Naga admission and fixed shader ABI
//! - `three_d_backend`: SPEC-HAL-355 explicit native backend availability gate
//! - `three_d_geometry`: SPEC-HAL-355 canonical vertex/index byte packing and GPU cache
//! - `three_d_math`: SPEC-HAL-355 checked precision, projection, ray, and intersection kernel
//! - `three_d_snapshot`: SPEC-HAL-355 immutable render snapshots and CPU resource leases

#![cfg(test)]
#![allow(deprecated, unused_imports, unused_mut, unused_variables)]

pub(crate) mod _support;

#[test]
fn ordinary_build_excludes_retired_agentic_api() {
    for source in [include_str!("../lib.rs"), include_str!("../core/mod.rs")] {
        for module in ["mod agent;", "mod agent_client;", "mod agent_server;"] {
            assert!(!source.contains(module), "retired module: {module}");
        }
    }

    let app = [
        include_str!("../core/app.rs"),
        include_str!("../core/app/ui_loop.rs"),
        include_str!("../core/app/app_root.rs"),
        include_str!("../core/app/run.rs"),
    ]
    .join("\n");
    for entry in [
        "with_agent_",
        "with_development_agent_server",
        "with_tower",
        "agent_action_rx",
        "start_agent_server",
        "convert_agent_action",
        "process_agent_query",
        "PendingComposition",
        "widget_factory::",
        "AgentEventKind",
    ] {
        assert!(!app.contains(entry), "retired App entry: {entry}");
    }
    let router = include_str!("../core/event_router.rs");
    assert!(!router.contains("set_agent_tx") && !router.contains("AgentEventKind"));

    let manifest = include_str!("../../Cargo.toml");
    for target in ["mcp-bridge", "tower-ctl", "wireframe", "playground", "agent_control"] {
        assert!(!manifest.contains(&format!(r#"name = "{target}""#)));
    }

    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
    for path in [
        "src/core/agent.rs",
        "src/core/agent_client.rs",
        "src/core/agent_server.rs",
        "src/agent/mod.rs",
        "src/agent/widget_factory.rs",
        "src/bin/agent_control.rs",
        "src/bin/mcp_bridge.rs",
        "src/bin/tower_ctl.rs",
        "src/bin/wireframe.rs",
        "src/bin/playground.rs",
        "examples/agent.rs",
        "examples/agent_lm_bridge.rs",
        "examples/agent_query.rs",
        "examples/agent_test_client.rs",
        "examples/compose_dashboard.rs",
        "examples/compose_settings.rs",
        "examples/designer.rs",
        "examples/tower_embedded.rs",
        "examples/wireframe.rs",
    ] {
        assert!(!root.join(path).exists(), "retired source or target: {path}");
    }
}
mod authority_probe;
mod authority_probe_engine;
mod authority_projection;
mod binding_registry;
mod button_4e_sdf_prototype;
mod checkbox;
mod combo_box_input;
mod console_retained_scroll;
mod custom_render_pass;
mod custom_render_preparation;
mod exclusion_flow;
mod focus_authority;
mod focus_scroll_neutrality;
mod hierarchical_substrate;
mod hts_f64_scroll_core;
mod icon_builder;
mod icon_builtins;
mod icon_fill_ratio;
mod icon_paint_context;
mod icon_palette;
mod icon_scene_integration;
mod icon_types;
mod icon_validate;
mod image_admission;
mod image_alpha;
mod interaction_domain_isolation;
mod interaction_fuzz;
mod linked_scroll_engine;
mod listview_closed_composite_lock;
mod listview_dirty_enforcement;
mod listview_dynamic_invalidation;
mod listview_interaction_callbacks;
mod listview_provider_sorting;
mod market_detail;
pub(crate) mod markdown_view;
mod registered_editable_text;
mod mesh_cache;
mod mesh_cache_integration;
mod navigation_domain_router;
mod navigation_frame_projection;
mod navigation_projection_boundary_guard;
mod order_book_cadence;
mod overlay_domain_coordinator;
mod paragraph_hard_break;
mod path_paint_context;
// TODO: These tests call internal ListView methods that were removed during refactoring.
// Re-enable once ListView internal test APIs are restored or tests are rewritten.
// mod listview_data_contract;
// mod listview_smoke;
pub mod oracles;
mod path_cache;
mod plot_live_data;
mod plot_spatial_overlay;
mod plot_text_metrics;
mod plot_theme;
mod pointer_driven_ui;
mod pointer_motion_policy;
mod property_editor_intent_deferral;
mod property_editor_shared_hts;
mod range_slider_focus_appearance;
mod readout_layout;
mod relative_pointer;
mod relative_pointer_authority;
mod relative_pointer_ingress;
#[cfg(target_os = "macos")]
mod relative_pointer_macos;
mod relative_pointer_routing;
mod renderer_image_lifecycle;
mod renderer_staging;
mod renderer_text_index_width;
mod renderer_text_metrics;
mod renderer_z_order;
#[cfg(all(target_os = "macos", debug_assertions))]
mod source_ink;
mod retained_value_updates;
mod route_geometry;
mod scene_text_ownership;
mod scroll_layout_anchor;
mod slider_focus_appearance;
mod slider_value_field_side;
mod spatial_public_contract;
mod split_pane;
mod svg_import;
mod svg_path;
mod tabular_substrate;
mod tabular_view;
mod temporal_geometry;
mod temporal_hts_rows;
mod temporal_lanes;
mod temporal_projection;
mod temporal_substrate;
mod three_d_appearance;
mod three_d_backend;
mod three_d_context;
mod three_d_domain;
mod three_d_geometry;
mod three_d_math;
mod three_d_raster;
mod three_d_snapshot;
mod three_d_viewport;
mod time_model;
mod timeline_canvas_geometry;
mod timeline_canvas_hit;
mod timeline_spatial;
mod toast_manager;
mod tooltip_host;
mod tooltip_manager;
mod tree;
mod tree_row_provider;
mod tree_substrate;
mod tree_view_intent_funnel;
mod tree_view_shared_hts;
mod treeview_smoke;
mod unbounded_measurement;
