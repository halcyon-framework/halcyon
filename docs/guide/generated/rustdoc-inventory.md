---
doc-id: halcyon-generated-rustdoc-inventory
class: generated
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-25
governing-specs:
  - SPEC-HAL-309
---

# Public-API Rustdoc Inventory (Generated)

**Generated mechanically** by `scratchpad/gen_rustdoc_census.py`
(deterministic pub-mod walk from `src/lib.rs`; `src/` byte-identical
to the pin — docs-only commits above it). Rules:

- Exported module set = transitive `pub mod` chains from `src/lib.rs`
  (`name.rs` / `name/mod.rs` resolution). `pub(crate)`, `pub(super)`,
  and `pub(in …)` items are excluded (not exported).
- Item census = lines matching a leading `pub` item declaration for
  kinds use/struct/enum/trait/fn/type/const/static/macro_rules!
  (`pub mod` declarations are structure, not documented items; the
  module-doc metric is the per-file inner `//!` flag in §2).
  Indented `pub fn` are associated items of their (exported) types and
  are marked `assoc`; struct fields and enum variants are accounted at
  container granularity here and verified per-container in
  `generated/api-quick-reference.md` (complete member inventories).
- Rustdoc presence = `///` or `#[doc…]` immediately above the item
  (attributes skipped); module doc = any leading `//!` block.
- Presence is mechanical; the quality axes
  (complete/incomplete/misleading) are judged only for the audited
  sample recorded in `audits/conformance-census.md` §Rustdoc; every
  unsampled documented item is classified **unverified**, every
  undocumented item **missing** (list below).
- Reachability caveat: `pub use` re-export targets are counted as
  `use` rows where declared; glob re-exports are listed verbatim, not
  expanded.

## 1. Totals

| Kind | Exported items | With rustdoc | Missing |
|---|---|---|---|
| use | 180 | 4 | 176 |
| struct | 1043 | 720 | 323 |
| enum | 439 | 287 | 152 |
| trait | 45 | 41 | 4 |
| fn | 4296 | 2920 | 1376 |
| type | 34 | 30 | 4 |
| const | 149 | 52 | 97 |
| static | 1 | 0 | 1 |
| macro_rules! | 0 | 0 | 0 |
| **total** | **6187** | **4054** | **2133** |

Exported module files: 330 (pub-mod chain from lib.rs); module-level `//!` docs present in 84, absent in 246 (flagged NO per module in §2).

## 2. Per-Module Census

### halcyon

- file: `src/lib.rs` · module `//!` doc: NO · items 34 · documented 0
- kinds (items/documented): use:34/0

### halcyon::agent

- file: `src/agent/mod.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): use:1/0

### halcyon::agent::llm_client

- file: `src/agent/llm_client.rs` · module `//!` doc: NO · items 4 · documented 3
- kinds (items/documented): struct:1/1 fn:3/2

### halcyon::agent::mock_data

- file: `src/agent/mock_data.rs` · module `//!` doc: NO · items 6 · documented 6
- kinds (items/documented): fn:6/6

### halcyon::agent::orchestrator

- file: `src/agent/orchestrator/mod.rs` · module `//!` doc: NO · items 4 · documented 0
- kinds (items/documented): use:4/0

### halcyon::agent::orchestrator::condition

- file: `src/agent/orchestrator/condition.rs` · module `//!` doc: yes · items 1 · documented 1
- kinds (items/documented): enum:1/1

### halcyon::agent::orchestrator::directive

- file: `src/agent/orchestrator/directive.rs` · module `//!` doc: yes · items 5 · documented 3
- kinds (items/documented): struct:2/2 fn:3/1

### halcyon::agent::orchestrator::dispatch

- file: `src/agent/orchestrator/dispatch.rs` · module `//!` doc: yes · items 2 · documented 1
- kinds (items/documented): struct:1/1 fn:1/0

### halcyon::agent::orchestrator::error

- file: `src/agent/orchestrator/error.rs` · module `//!` doc: NO · items 6 · documented 3
- kinds (items/documented): enum:4/3 fn:2/0

### halcyon::agent::orchestrator::event

- file: `src/agent/orchestrator/event.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): enum:1/0

### halcyon::agent::orchestrator::id

- file: `src/agent/orchestrator/id.rs` · module `//!` doc: NO · items 13 · documented 13
- kinds (items/documented): struct:1/1 fn:12/12

### halcyon::agent::orchestrator::instruction

- file: `src/agent/orchestrator/instruction.rs` · module `//!` doc: yes · items 1 · documented 1
- kinds (items/documented): enum:1/1

### halcyon::agent::orchestrator::operation

- file: `src/agent/orchestrator/operation.rs` · module `//!` doc: yes · items 7 · documented 6
- kinds (items/documented): struct:1/1 enum:1/1 trait:1/1 fn:4/3

### halcyon::agent::orchestrator::retry

- file: `src/agent/orchestrator/retry.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::agent::orchestrator::spec

- file: `src/agent/orchestrator/spec.rs` · module `//!` doc: yes · items 1 · documented 1
- kinds (items/documented): struct:1/1

### halcyon::agent::orchestrator::task

- file: `src/agent/orchestrator/task.rs` · module `//!` doc: yes · items 5 · documented 3
- kinds (items/documented): struct:1/1 enum:1/1 fn:3/1

### halcyon::agent::playground_loop

- file: `src/agent/playground_loop.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::agent::tower

- file: `src/agent/tower.rs` · module `//!` doc: NO · items 10 · documented 9
- kinds (items/documented): struct:3/3 enum:3/3 fn:4/3

### halcyon::agent::widget_factory

- file: `src/agent/widget_factory.rs` · module `//!` doc: NO · items 3 · documented 3
- kinds (items/documented): fn:3/3

### halcyon::compositor

- file: `src/compositor/mod.rs` · module `//!` doc: yes · items 46 · documented 32
- kinds (items/documented): use:5/0 struct:4/3 enum:1/0 fn:36/29

### halcyon::core

- file: `src/core/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::core::action_palette_modal

- file: `src/core/action_palette_modal.rs` · module `//!` doc: NO · items 2 · documented 1
- kinds (items/documented): struct:1/1 fn:1/0

### halcyon::core::agent

- file: `src/core/agent.rs` · module `//!` doc: NO · items 19 · documented 18
- kinds (items/documented): struct:3/3 enum:4/4 fn:4/3 type:4/4 const:4/4

### halcyon::core::agent_client

- file: `src/core/agent_client.rs` · module `//!` doc: NO · items 9 · documented 2
- kinds (items/documented): struct:1/1 enum:1/0 fn:7/1

### halcyon::core::agent_server

- file: `src/core/agent_server.rs` · module `//!` doc: NO · items 9 · documented 8
- kinds (items/documented): struct:2/2 enum:2/2 fn:5/4

### halcyon::core::animation

- file: `src/core/animation.rs` · module `//!` doc: NO · items 23 · documented 21
- kinds (items/documented): struct:5/5 enum:1/1 trait:1/1 fn:16/14

### halcyon::core::app

- file: `src/core/app.rs` · module `//!` doc: NO · items 168 · documented 125
- kinds (items/documented): use:1/0 struct:20/15 enum:5/4 trait:1/1 fn:139/104 type:1/1 static:1/0

### halcyon::core::authority

- file: `src/core/authority.rs` · module `//!` doc: NO · items 27 · documented 27
- kinds (items/documented): struct:6/6 trait:4/4 fn:17/17

### halcyon::core::deferred_wiring

- file: `src/core/deferred_wiring.rs` · module `//!` doc: NO · items 8 · documented 8
- kinds (items/documented): trait:1/1 fn:7/7

### halcyon::core::drag_drop

- file: `src/core/drag_drop.rs` · module `//!` doc: yes · items 14 · documented 8
- kinds (items/documented): struct:2/2 fn:12/6

### halcyon::core::editing

- file: `src/core/editing.rs` · module `//!` doc: yes · items 77 · documented 77
- kinds (items/documented): struct:7/7 enum:2/2 fn:68/68

### halcyon::core::editing_history

- file: `src/core/editing_history.rs` · module `//!` doc: yes · items 24 · documented 24
- kinds (items/documented): struct:3/3 enum:1/1 fn:19/19 const:1/1

### halcyon::core::event

- file: `src/core/event/mod.rs` · module `//!` doc: NO · items 148 · documented 132
- kinds (items/documented): use:1/1 struct:22/22 enum:28/28 fn:94/78 const:3/3

### halcyon::core::event_router

- file: `src/core/event_router.rs` · module `//!` doc: NO · items 39 · documented 34
- kinds (items/documented): struct:1/1 enum:1/1 fn:37/32

### halcyon::core::focus_authority

- file: `src/core/focus_authority.rs` · module `//!` doc: NO · items 53 · documented 2
- kinds (items/documented): struct:16/2 enum:1/0 fn:36/0

### halcyon::core::frame_projection_binding_registry

- file: `src/core/frame_projection_binding_registry.rs` · module `//!` doc: NO · items 39 · documented 0
- kinds (items/documented): struct:9/0 enum:4/0 fn:26/0

### halcyon::core::glyph_quad

- file: `src/core/glyph_quad.rs` · module `//!` doc: yes · items 9 · documented 9
- kinds (items/documented): struct:4/4 fn:5/5

### halcyon::core::hit_test

- file: `src/core/hit_test.rs` · module `//!` doc: NO · items 9 · documented 9
- kinds (items/documented): struct:2/2 fn:4/4 const:3/3

### halcyon::core::icon

- file: `src/core/icon.rs` · module `//!` doc: yes · items 34 · documented 13
- kinds (items/documented): struct:5/5 enum:6/6 fn:7/2 const:16/0

### halcyon::core::icon_builder

- file: `src/core/icon_builder.rs` · module `//!` doc: yes · items 12 · documented 11
- kinds (items/documented): struct:1/1 fn:11/10

### halcyon::core::icon_palette

- file: `src/core/icon_palette.rs` · module `//!` doc: yes · items 4 · documented 4
- kinds (items/documented): enum:1/1 fn:3/3

### halcyon::core::icons

- file: `src/core/icons/mod.rs` · module `//!` doc: yes · items 1 · documented 1
- kinds (items/documented): fn:1/1

### halcyon::core::input_fusion

- file: `src/core/input_fusion.rs` · module `//!` doc: NO · items 26 · documented 20
- kinds (items/documented): struct:6/6 fn:20/14

### halcyon::core::input_projection

- file: `src/core/input_projection.rs` · module `//!` doc: NO · items 24 · documented 21
- kinds (items/documented): struct:5/5 enum:1/1 trait:1/1 fn:17/14

### halcyon::core::interaction_fuzz

- file: `src/core/interaction_fuzz.rs` · module `//!` doc: yes · items 170 · documented 1
- kinds (items/documented): struct:48/1 enum:35/0 fn:73/0 type:2/0 const:12/0

### halcyon::core::layout

- file: `src/core/layout.rs` · module `//!` doc: NO · items 47 · documented 29
- kinds (items/documented): struct:8/8 enum:4/4 fn:33/15 type:2/2

### halcyon::core::layout_transition

- file: `src/core/layout_transition.rs` · module `//!` doc: NO · items 56 · documented 51
- kinds (items/documented): struct:13/13 enum:7/7 fn:33/28 type:3/3

### halcyon::core::lifecycle

- file: `src/core/lifecycle.rs` · module `//!` doc: NO · items 20 · documented 20
- kinds (items/documented): struct:1/1 enum:1/1 fn:18/18

### halcyon::core::math

- file: `src/core/math.rs` · module `//!` doc: NO · items 29 · documented 13
- kinds (items/documented): struct:3/3 fn:26/10

### halcyon::core::navigation_application

- file: `src/core/navigation_application.rs` · module `//!` doc: NO · items 90 · documented 0
- kinds (items/documented): struct:21/0 enum:8/0 fn:61/0

### halcyon::core::navigation_domain_router

- file: `src/core/navigation_domain_router.rs` · module `//!` doc: NO · items 104 · documented 0
- kinds (items/documented): struct:26/0 enum:5/0 fn:73/0

### halcyon::core::navigation_orchestrator

- file: `src/core/navigation_orchestrator.rs` · module `//!` doc: NO · items 80 · documented 41
- kinds (items/documented): struct:14/11 enum:9/5 trait:1/1 fn:56/24

### halcyon::core::ordered_selection

- file: `src/core/ordered_selection/mod.rs` · module `//!` doc: NO · items 29 · documented 29
- kinds (items/documented): struct:2/2 enum:3/3 trait:1/1 fn:23/23

### halcyon::core::overlay_domain_coordinator

- file: `src/core/overlay_domain_coordinator.rs` · module `//!` doc: NO · items 71 · documented 1
- kinds (items/documented): struct:21/0 enum:4/0 fn:46/1

### halcyon::core::overlay_root

- file: `src/core/overlay_root.rs` · module `//!` doc: NO · items 22 · documented 22
- kinds (items/documented): struct:4/4 enum:1/1 fn:17/17

### halcyon::core::paint

- file: `src/core/paint.rs` · module `//!` doc: yes · items 42 · documented 42
- kinds (items/documented): struct:6/6 enum:3/3 trait:1/1 fn:24/24 type:1/1 const:7/7

### halcyon::core::paragraph

- file: `src/core/paragraph.rs` · module `//!` doc: yes · items 32 · documented 23
- kinds (items/documented): struct:9/9 enum:1/1 fn:22/13

### halcyon::core::popup

- file: `src/core/popup.rs` · module `//!` doc: yes · items 1 · documented 1
- kinds (items/documented): fn:1/1

### halcyon::core::rect_authority

- file: `src/core/rect_authority.rs` · module `//!` doc: NO · items 29 · documented 29
- kinds (items/documented): struct:5/5 enum:1/1 fn:23/23

### halcyon::core::resize_settling

- file: `src/core/resize_settling.rs` · module `//!` doc: yes · items 21 · documented 21
- kinds (items/documented): struct:2/2 fn:18/18 const:1/1

### halcyon::core::scene

- file: `src/core/scene/mod.rs` · module `//!` doc: NO · items 112 · documented 104
- kinds (items/documented): use:2/2 struct:31/31 enum:1/1 fn:70/62 type:1/1 const:7/7

### halcyon::core::scene_diff

- file: `src/core/scene_diff/mod.rs` · module `//!` doc: yes · items 96 · documented 96
- kinds (items/documented): struct:12/12 enum:5/5 trait:2/2 fn:77/77

### halcyon::core::scene_identity

- file: `src/core/scene_identity.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 enum:1/1 fn:1/1

### halcyon::core::scheduler

- file: `src/core/scheduler.rs` · module `//!` doc: NO · items 23 · documented 23
- kinds (items/documented): struct:6/6 enum:1/1 fn:15/15 type:1/1

### halcyon::core::scroll_container

- file: `src/core/scroll_container.rs` · module `//!` doc: NO · items 37 · documented 36
- kinds (items/documented): struct:3/3 enum:1/1 fn:33/32

### halcyon::core::scroll_link

- file: `src/core/scroll_link.rs` · module `//!` doc: NO · items 15 · documented 15
- kinds (items/documented): struct:2/2 enum:2/2 fn:11/11

### halcyon::core::scroll_state

- file: `src/core/scroll_state.rs` · module `//!` doc: NO · items 57 · documented 52
- kinds (items/documented): struct:7/7 enum:2/2 trait:1/1 fn:47/42

### halcyon::core::series

- file: `src/core/series.rs` · module `//!` doc: NO · items 15 · documented 0
- kinds (items/documented): struct:1/0 fn:14/0

### halcyon::core::signal_bus

- file: `src/core/signal_bus.rs` · module `//!` doc: yes · items 28 · documented 28
- kinds (items/documented): struct:4/4 enum:4/4 fn:19/19 const:1/1

### halcyon::core::sliding_window

- file: `src/core/sliding_window.rs` · module `//!` doc: NO · items 20 · documented 1
- kinds (items/documented): struct:3/0 fn:17/1

### halcyon::core::style

- file: `src/core/style.rs` · module `//!` doc: NO · items 27 · documented 24
- kinds (items/documented): struct:9/9 enum:1/1 fn:14/13 const:3/1

### halcyon::core::svg_import

- file: `src/core/svg_import.rs` · module `//!` doc: yes · items 2 · documented 2
- kinds (items/documented): fn:2/2

### halcyon::core::svg_path

- file: `src/core/svg_path.rs` · module `//!` doc: yes · items 2 · documented 2
- kinds (items/documented): enum:1/1 fn:1/1

### halcyon::core::text_edit_delegate

- file: `src/core/text_edit_delegate.rs` · module `//!` doc: NO · items 24 · documented 24
- kinds (items/documented): fn:24/24

### halcyon::core::text_edit_policy

- file: `src/core/text_edit_policy.rs` · module `//!` doc: NO · items 6 · documented 6
- kinds (items/documented): struct:2/2 enum:1/1 fn:3/3

### halcyon::core::text_engine

- file: `src/core/text_engine.rs` · module `//!` doc: NO · items 10 · documented 8
- kinds (items/documented): struct:3/3 trait:1/1 fn:6/4

### halcyon::core::text_linebreaker

- file: `src/core/text_linebreaker.rs` · module `//!` doc: yes · items 4 · documented 3
- kinds (items/documented): use:1/0 struct:2/2 fn:1/1

### halcyon::core::text_selection

- file: `src/core/text_selection.rs` · module `//!` doc: NO · items 23 · documented 23
- kinds (items/documented): struct:1/1 enum:1/1 fn:21/21

### halcyon::core::text_shaper

- file: `src/core/text_shaper.rs` · module `//!` doc: yes · items 10 · documented 10
- kinds (items/documented): struct:2/2 enum:1/1 fn:7/7

### halcyon::core::theme

- file: `src/core/theme.rs` · module `//!` doc: NO · items 71 · documented 68
- kinds (items/documented): struct:46/44 enum:5/5 fn:20/19

### halcyon::core::transition

- file: `src/core/transition/mod.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): use:1/0

### halcyon::core::widget

- file: `src/core/widget/mod.rs` · module `//!` doc: NO · items 92 · documented 83
- kinds (items/documented): use:1/1 struct:6/6 trait:2/2 fn:83/74

### halcyon::core::widget::forbidden

- file: `src/core/widget/forbidden.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:3/3

### halcyon::core::widget::guard

- file: `src/core/widget/guard.rs` · module `//!` doc: yes · items 0 · documented 0
- kinds (items/documented): none

### halcyon::core::widget_tree

- file: `src/core/widget_tree.rs` · module `//!` doc: NO · items 93 · documented 90
- kinds (items/documented): struct:5/5 enum:3/3 fn:85/82

### halcyon::editing

- file: `src/editing/mod.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): use:1/0

### halcyon::editing::editor_bridge

- file: `src/editing/editor_bridge.rs` · module `//!` doc: yes · items 8 · documented 3
- kinds (items/documented): struct:1/1 fn:7/2

### halcyon::engines

- file: `src/engines/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::engines::cell_mechanics

- file: `src/engines/cell_mechanics/mod.rs` · module `//!` doc: NO · items 31 · documented 30
- kinds (items/documented): struct:3/3 enum:1/1 fn:27/26

### halcyon::engines::cell_source

- file: `src/engines/cell_source/mod.rs` · module `//!` doc: NO · items 16 · documented 14
- kinds (items/documented): struct:3/2 enum:7/7 trait:2/2 fn:4/3

### halcyon::engines::column_geometry

- file: `src/engines/column_geometry/mod.rs` · module `//!` doc: NO · items 44 · documented 44
- kinds (items/documented): struct:4/4 fn:40/40

### halcyon::engines::hierarchical

- file: `src/engines/hierarchical/mod.rs` · module `//!` doc: NO · items 50 · documented 49
- kinds (items/documented): use:1/0 struct:3/3 enum:1/1 fn:44/44 type:1/1

### halcyon::engines::route_geometry

- file: `src/engines/route_geometry/mod.rs` · module `//!` doc: NO · items 7 · documented 6
- kinds (items/documented): struct:3/3 enum:1/1 fn:3/2

### halcyon::engines::row_topology

- file: `src/engines/row_topology/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::engines::temporal

- file: `src/engines/temporal/mod.rs` · module `//!` doc: NO · items 8 · documented 0
- kinds (items/documented): use:8/0

### halcyon::engines::temporal::containment

- file: `src/engines/temporal/containment.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:1/1 enum:1/1 trait:1/1 fn:4/4

### halcyon::engines::temporal::descriptors

- file: `src/engines/temporal/descriptors.rs` · module `//!` doc: NO · items 13 · documented 6
- kinds (items/documented): struct:6/4 enum:5/1 trait:1/1 fn:1/0

### halcyon::engines::temporal::emphasis

- file: `src/engines/temporal/emphasis.rs` · module `//!` doc: NO · items 8 · documented 7
- kinds (items/documented): struct:1/1 enum:2/2 fn:5/4

### halcyon::engines::temporal::hts_rows

- file: `src/engines/temporal/hts_rows.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::engines::temporal::interval_projection

- file: `src/engines/temporal/interval_projection.rs` · module `//!` doc: NO · items 8 · documented 3
- kinds (items/documented): struct:2/1 fn:6/2

### halcyon::engines::temporal::lane_layout

- file: `src/engines/temporal/lane_layout.rs` · module `//!` doc: NO · items 8 · documented 4
- kinds (items/documented): struct:1/0 enum:1/0 fn:5/3 const:1/1

### halcyon::engines::temporal::substrate

- file: `src/engines/temporal/substrate.rs` · module `//!` doc: NO · items 24 · documented 13
- kinds (items/documented): struct:2/1 enum:1/1 fn:20/10 type:1/1

### halcyon::engines::temporal::time_geometry

- file: `src/engines/temporal/time_geometry.rs` · module `//!` doc: NO · items 32 · documented 14
- kinds (items/documented): struct:5/2 enum:1/0 fn:26/12

### halcyon::engines::virtualization

- file: `src/engines/virtualization/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::model

- file: `src/model/mod.rs` · module `//!` doc: yes · items 1 · documented 0
- kinds (items/documented): use:1/0

### halcyon::model::breadcrumbs

- file: `src/model/breadcrumbs.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): fn:1/1 type:1/1

### halcyon::model::canvas

- file: `src/model/canvas.rs` · module `//!` doc: NO · items 57 · documented 0
- kinds (items/documented): struct:10/0 enum:6/0 fn:38/0 const:3/0

### halcyon::model::collapsible_section

- file: `src/model/collapsible_section.rs` · module `//!` doc: NO · items 5 · documented 1
- kinds (items/documented): struct:1/1 fn:4/0

### halcyon::model::context_menu

- file: `src/model/context_menu.rs` · module `//!` doc: NO · items 14 · documented 9
- kinds (items/documented): struct:1/1 enum:1/1 fn:10/5 type:2/2

### halcyon::model::date_value

- file: `src/model/date_value.rs` · module `//!` doc: NO · items 8 · documented 8
- kinds (items/documented): struct:1/1 fn:7/7

### halcyon::model::default_selection_resolver

- file: `src/model/default_selection_resolver.rs` · module `//!` doc: yes · items 4 · documented 4
- kinds (items/documented): struct:1/1 enum:1/1 fn:2/2

### halcyon::model::dialog

- file: `src/model/dialog.rs` · module `//!` doc: yes · items 4 · documented 1
- kinds (items/documented): struct:1/1 fn:3/0

### halcyon::model::dropdown_menu

- file: `src/model/dropdown_menu.rs` · module `//!` doc: NO · items 12 · documented 7
- kinds (items/documented): struct:1/1 fn:9/4 type:2/2

### halcyon::model::form_binder

- file: `src/model/form_binder.rs` · module `//!` doc: yes · items 32 · documented 32
- kinds (items/documented): struct:8/8 fn:24/24

### halcyon::model::menu_item

- file: `src/model/menu_item.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): enum:1/1 type:1/1

### halcyon::model::menu_model

- file: `src/model/menu_model.rs` · module `//!` doc: NO · items 15 · documented 10
- kinds (items/documented): struct:3/3 fn:12/7

### halcyon::model::menu_surface

- file: `src/model/menu_surface.rs` · module `//!` doc: NO · items 12 · documented 8
- kinds (items/documented): struct:1/1 fn:11/7

### halcyon::model::navigation_host

- file: `src/model/navigation_host.rs` · module `//!` doc: NO · items 66 · documented 17
- kinds (items/documented): struct:11/8 enum:9/4 trait:1/1 fn:45/4

### halcyon::model::node_editor

- file: `src/model/node_editor.rs` · module `//!` doc: NO · items 105 · documented 1
- kinds (items/documented): struct:24/0 enum:13/0 fn:66/0 type:1/1 const:1/0

### halcyon::model::node_graph

- file: `src/model/node_graph.rs` · module `//!` doc: NO · items 68 · documented 0
- kinds (items/documented): struct:17/0 enum:7/0 trait:1/0 fn:40/0 type:1/0 const:2/0

### halcyon::model::node_graph_projection

- file: `src/model/node_graph_projection.rs` · module `//!` doc: NO · items 30 · documented 2
- kinds (items/documented): struct:10/2 enum:5/0 fn:15/0

### halcyon::model::notification

- file: `src/model/notification.rs` · module `//!` doc: NO · items 16 · documented 4
- kinds (items/documented): struct:5/0 enum:1/0 fn:9/4 const:1/0

### halcyon::model::order_book

- file: `src/model/order_book.rs` · module `//!` doc: NO · items 7 · documented 3
- kinds (items/documented): struct:2/2 enum:2/1 fn:3/0

### halcyon::model::popup_menu

- file: `src/model/popup_menu.rs` · module `//!` doc: NO · items 11 · documented 7
- kinds (items/documented): struct:1/1 fn:8/4 type:2/2

### halcyon::model::search_projection_harness

- file: `src/model/search_projection_harness.rs` · module `//!` doc: yes · items 12 · documented 10
- kinds (items/documented): struct:3/3 enum:1/1 fn:8/6

### halcyon::model::selection_resolver

- file: `src/model/selection_resolver.rs` · module `//!` doc: yes · items 13 · documented 13
- kinds (items/documented): struct:4/4 enum:4/4 trait:1/1 fn:4/4

### halcyon::model::signal_store

- file: `src/model/signal_store.rs` · module `//!` doc: yes · items 11 · documented 10
- kinds (items/documented): struct:3/3 fn:8/7

### halcyon::model::spatial

- file: `src/model/spatial/mod.rs` · module `//!` doc: NO · items 82 · documented 6
- kinds (items/documented): struct:20/3 enum:15/1 trait:1/0 fn:46/2

### halcyon::model::tab_bar

- file: `src/model/tab_bar.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): fn:1/1 type:1/1

### halcyon::model::tabular

- file: `src/model/tabular/mod.rs` · module `//!` doc: yes · items 0 · documented 0
- kinds (items/documented): none

### halcyon::model::tabular::column

- file: `src/model/tabular/column.rs` · module `//!` doc: NO · items 40 · documented 40
- kinds (items/documented): struct:3/3 enum:1/1 fn:35/35 const:1/1

### halcyon::model::tabular::row_expansion

- file: `src/model/tabular/row_expansion.rs` · module `//!` doc: NO · items 18 · documented 18
- kinds (items/documented): struct:1/1 enum:2/2 fn:15/15

### halcyon::model::tabular::row_heights

- file: `src/model/tabular/row_heights.rs` · module `//!` doc: NO · items 16 · documented 16
- kinds (items/documented): struct:1/1 fn:15/15

### halcyon::model::time

- file: `src/model/time.rs` · module `//!` doc: NO · items 20 · documented 9
- kinds (items/documented): struct:3/3 fn:13/6 const:4/0

### halcyon::model::time_value

- file: `src/model/time_value.rs` · module `//!` doc: NO · items 5 · documented 1
- kinds (items/documented): struct:1/1 fn:4/0

### halcyon::model::toast

- file: `src/model/toast.rs` · module `//!` doc: NO · items 20 · documented 6
- kinds (items/documented): struct:6/0 enum:1/0 fn:13/6

### halcyon::model::tooltip

- file: `src/model/tooltip.rs` · module `//!` doc: NO · items 24 · documented 22
- kinds (items/documented): struct:3/3 enum:2/2 fn:19/17

### halcyon::model::tree

- file: `src/model/tree.rs` · module `//!` doc: NO · items 20 · documented 20
- kinds (items/documented): struct:1/1 fn:19/19

### halcyon::model::tree_row_provider

- file: `src/model/tree_row_provider.rs` · module `//!` doc: NO · items 3 · documented 3
- kinds (items/documented): struct:1/1 trait:1/1 fn:1/1

### halcyon::model::tree_substrate

- file: `src/model/tree_substrate.rs` · module `//!` doc: NO · items 10 · documented 10
- kinds (items/documented): struct:1/1 fn:9/9

### halcyon::model::tree_types

- file: `src/model/tree_types.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:2/2 enum:1/1 trait:1/1 fn:3/3

### halcyon::model::tutorial

- file: `src/model/tutorial.rs` · module `//!` doc: NO · items 12 · documented 10
- kinds (items/documented): struct:4/4 enum:1/1 fn:7/5

### halcyon::oracles

- file: `src/oracles/mod.rs` · module `//!` doc: yes · items 1 · documented 0
- kinds (items/documented): use:1/0

### halcyon::platform

- file: `src/platform/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::platform::backend_text_engine

- file: `src/platform/backend_text_engine.rs` · module `//!` doc: yes · items 10 · documented 10
- kinds (items/documented): struct:1/1 fn:9/9

### halcyon::platform::clipboard

- file: `src/platform/clipboard.rs` · module `//!` doc: yes · items 4 · documented 4
- kinds (items/documented): struct:1/1 fn:3/3

### halcyon::platform::font_loader

- file: `src/platform/font_loader.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): fn:1/0

### halcyon::platform::font_manager

- file: `src/platform/font_manager.rs` · module `//!` doc: yes · items 46 · documented 46
- kinds (items/documented): struct:8/8 enum:5/5 fn:33/33

### halcyon::platform::font_manager_text_engine

- file: `src/platform/font_manager_text_engine.rs` · module `//!` doc: yes · items 19 · documented 18
- kinds (items/documented): struct:2/2 fn:17/16

### halcyon::platform::font_resolver

- file: `src/platform/font_resolver.rs` · module `//!` doc: yes · items 4 · documented 4
- kinds (items/documented): struct:1/1 fn:3/3

### halcyon::platform::menu_host

- file: `src/platform/menu_host.rs` · module `//!` doc: NO · items 3 · documented 0
- kinds (items/documented): enum:1/0 trait:1/0 fn:1/0

### halcyon::platform::native_font_backend

- file: `src/platform/native_font_backend.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 enum:1/1 trait:1/1

### halcyon::platform::native_font_backend_coretext

- file: `src/platform/native_font_backend_coretext.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 fn:2/2

### halcyon::platform::native_font_backend_directwrite

- file: `src/platform/native_font_backend_directwrite.rs` · module `//!` doc: yes · items 3 · documented 2
- kinds (items/documented): struct:1/1 fn:2/1

### halcyon::platform::native_font_backend_freetype

- file: `src/platform/native_font_backend_freetype.rs` · module `//!` doc: yes · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::platform::native_font_backend_glyphon

- file: `src/platform/native_font_backend_glyphon.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 fn:2/2

### halcyon::platform::native_menu_macos

- file: `src/platform/native_menu_macos.rs` · module `//!` doc: NO · items 2 · documented 0
- kinds (items/documented): struct:1/0 fn:1/0

### halcyon::platform::system_font_provider

- file: `src/platform/system_font_provider.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 fn:2/2

### halcyon::platform::text_engine_glyphon

- file: `src/platform/text_engine_glyphon.rs` · module `//!` doc: NO · items 5 · documented 4
- kinds (items/documented): struct:1/1 fn:4/3

### halcyon::platform::window

- file: `src/platform/window.rs` · module `//!` doc: NO · items 17 · documented 12
- kinds (items/documented): struct:1/1 enum:1/1 fn:15/10

### halcyon::plot

- file: `src/plot/mod.rs` · module `//!` doc: NO · items 138 · documented 70
- kinds (items/documented): use:24/0 struct:5/5 enum:2/2 fn:96/63 const:11/0

### halcyon::plot::axis

- file: `src/plot/axis.rs` · module `//!` doc: NO · items 11 · documented 11
- kinds (items/documented): struct:2/2 enum:2/2 fn:7/7

### halcyon::plot::axis_authority

- file: `src/plot/axis_authority.rs` · module `//!` doc: NO · items 8 · documented 8
- kinds (items/documented): struct:1/1 fn:7/7

### halcyon::plot::color_scheme

- file: `src/plot/color_scheme.rs` · module `//!` doc: NO · items 4 · documented 1
- kinds (items/documented): struct:1/1 fn:3/0

### halcyon::plot::grid

- file: `src/plot/grid.rs` · module `//!` doc: NO · items 6 · documented 6
- kinds (items/documented): fn:6/6

### halcyon::plot::hit_proxy

- file: `src/plot/hit_proxy.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:2/2 enum:1/1 fn:4/4

### halcyon::plot::market_detail

- file: `src/plot/market_detail.rs` · module `//!` doc: NO · items 27 · documented 6
- kinds (items/documented): struct:9/5 enum:1/1 fn:17/0

### halcyon::plot::price_level

- file: `src/plot/price_level.rs` · module `//!` doc: NO · items 24 · documented 21
- kinds (items/documented): struct:10/10 enum:10/10 fn:4/1

### halcyon::plot::probe

- file: `src/plot/probe.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:1/1 fn:6/6

### halcyon::plot::render_pool

- file: `src/plot/render_pool.rs` · module `//!` doc: NO · items 2 · documented 1
- kinds (items/documented): struct:1/1 fn:1/0

### halcyon::plot::router

- file: `src/plot/router.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::plot::series

- file: `src/plot/series/mod.rs` · module `//!` doc: NO · items 12 · documented 5
- kinds (items/documented): use:7/0 struct:1/1 enum:3/3 trait:1/1

### halcyon::plot::series::area

- file: `src/plot/series/area.rs` · module `//!` doc: NO · items 8 · documented 2
- kinds (items/documented): struct:3/2 fn:5/0

### halcyon::plot::series::bar

- file: `src/plot/series/bar.rs` · module `//!` doc: NO · items 10 · documented 5
- kinds (items/documented): struct:3/2 fn:7/3

### halcyon::plot::series::candle

- file: `src/plot/series/candle.rs` · module `//!` doc: NO · items 13 · documented 5
- kinds (items/documented): struct:5/1 fn:8/4

### halcyon::plot::series::circle

- file: `src/plot/series/circle.rs` · module `//!` doc: NO · items 11 · documented 2
- kinds (items/documented): struct:4/2 fn:7/0

### halcyon::plot::series::depth

- file: `src/plot/series/depth.rs` · module `//!` doc: NO · items 12 · documented 4
- kinds (items/documented): use:1/0 struct:3/2 enum:1/1 fn:7/1

### halcyon::plot::series::histogram

- file: `src/plot/series/histogram.rs` · module `//!` doc: NO · items 8 · documented 2
- kinds (items/documented): struct:3/2 fn:5/0

### halcyon::plot::series::line

- file: `src/plot/series/line.rs` · module `//!` doc: NO · items 9 · documented 4
- kinds (items/documented): struct:3/2 fn:6/2

### halcyon::plot::spatial

- file: `src/plot/spatial.rs` · module `//!` doc: NO · items 12 · documented 9
- kinds (items/documented): struct:4/4 enum:2/2 fn:6/3

### halcyon::plot::tool

- file: `src/plot/tool.rs` · module `//!` doc: NO · items 14 · documented 11
- kinds (items/documented): struct:5/5 enum:4/4 trait:1/1 fn:4/1

### halcyon::plot::transform

- file: `src/plot/transform.rs` · module `//!` doc: NO · items 19 · documented 15
- kinds (items/documented): struct:2/2 fn:17/13

### halcyon::plot::view_state

- file: `src/plot/view_state.rs` · module `//!` doc: NO · items 14 · documented 14
- kinds (items/documented): struct:1/1 fn:13/13

### halcyon::renderer

- file: `src/renderer/mod.rs` · module `//!` doc: NO · items 12 · documented 4
- kinds (items/documented): use:8/0 struct:1/1 fn:3/3

### halcyon::renderer::batcher

- file: `src/renderer/batcher.rs` · module `//!` doc: yes · items 70 · documented 62
- kinds (items/documented): struct:12/11 enum:2/2 fn:52/45 const:4/4

### halcyon::renderer::compiled_draw_plan

- file: `src/renderer/compiled_draw_plan.rs` · module `//!` doc: yes · items 15 · documented 15
- kinds (items/documented): struct:4/4 fn:11/11

### halcyon::renderer::custom_render_pass

- file: `src/renderer/custom_render_pass.rs` · module `//!` doc: NO · items 26 · documented 1
- kinds (items/documented): struct:3/1 enum:1/0 trait:1/0 fn:20/0 type:1/0

### halcyon::renderer::draw_plan

- file: `src/renderer/draw_plan.rs` · module `//!` doc: yes · items 18 · documented 14
- kinds (items/documented): struct:5/5 fn:13/9

### halcyon::renderer::draw_plan_builder

- file: `src/renderer/draw_plan_builder.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 fn:2/2

### halcyon::renderer::glyph_atlas

- file: `src/renderer/glyph_atlas.rs` · module `//!` doc: yes · items 21 · documented 20
- kinds (items/documented): struct:5/5 enum:1/1 fn:14/13 const:1/1

### halcyon::renderer::gpu_arena

- file: `src/renderer/gpu_arena.rs` · module `//!` doc: NO · items 13 · documented 13
- kinds (items/documented): struct:3/3 enum:1/1 fn:9/9

### halcyon::renderer::mesh_cache

- file: `src/renderer/mesh_cache.rs` · module `//!` doc: yes · items 13 · documented 13
- kinds (items/documented): struct:2/2 enum:1/1 fn:10/10

### halcyon::renderer::path

- file: `src/renderer/path.rs` · module `//!` doc: yes · items 33 · documented 33
- kinds (items/documented): struct:5/5 enum:4/4 fn:24/24

### halcyon::renderer::path_renderer

- file: `src/renderer/path_renderer.rs` · module `//!` doc: yes · items 9 · documented 9
- kinds (items/documented): struct:1/1 fn:8/8

### halcyon::renderer::pipelines

- file: `src/renderer/pipelines/mod.rs` · module `//!` doc: yes · items 0 · documented 0
- kinds (items/documented): none

### halcyon::renderer::pipelines::path_pipeline

- file: `src/renderer/pipelines/path_pipeline.rs` · module `//!` doc: yes · items 36 · documented 35
- kinds (items/documented): struct:7/7 fn:27/26 const:2/2

### halcyon::renderer::pipelines::shape_pipeline

- file: `src/renderer/pipelines/shape_pipeline.rs` · module `//!` doc: yes · items 12 · documented 7
- kinds (items/documented): struct:2/2 enum:1/1 fn:8/4 const:1/0

### halcyon::renderer::pipelines::text_pipeline

- file: `src/renderer/pipelines/text_pipeline.rs` · module `//!` doc: yes · items 3 · documented 2
- kinds (items/documented): use:1/0 struct:1/1 fn:1/1

### halcyon::renderer::render_graph

- file: `src/renderer/render_graph/mod.rs` · module `//!` doc: yes · items 7 · documented 0
- kinds (items/documented): use:7/0

### halcyon::renderer::render_graph::builder

- file: `src/renderer/render_graph/builder.rs` · module `//!` doc: yes · items 7 · documented 7
- kinds (items/documented): struct:1/1 enum:1/1 fn:5/5

### halcyon::renderer::render_graph::executor

- file: `src/renderer/render_graph/executor.rs` · module `//!` doc: yes · items 22 · documented 22
- kinds (items/documented): struct:8/8 enum:1/1 fn:13/13

### halcyon::renderer::render_graph::graph

- file: `src/renderer/render_graph/graph.rs` · module `//!` doc: yes · items 3 · documented 3
- kinds (items/documented): struct:1/1 fn:2/2

### halcyon::renderer::render_graph::pass

- file: `src/renderer/render_graph/pass.rs` · module `//!` doc: yes · items 10 · documented 8
- kinds (items/documented): struct:2/2 enum:1/1 trait:1/1 fn:6/4

### halcyon::renderer::render_graph::passes

- file: `src/renderer/render_graph/passes/mod.rs` · module `//!` doc: yes · items 3 · documented 0
- kinds (items/documented): use:3/0

### halcyon::renderer::render_graph::passes::background_prepass

- file: `src/renderer/render_graph/passes/background_prepass.rs` · module `//!` doc: yes · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::renderer::render_graph::passes::main_pass

- file: `src/renderer/render_graph/passes/main_pass.rs` · module `//!` doc: yes · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::renderer::render_graph::passes::overlay_pass

- file: `src/renderer/render_graph/passes/overlay_pass.rs` · module `//!` doc: yes · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::renderer::render_graph::resources

- file: `src/renderer/render_graph/resources.rs` · module `//!` doc: yes · items 8 · documented 8
- kinds (items/documented): struct:4/4 enum:3/3 const:1/1

### halcyon::renderer::render_graph::topology

- file: `src/renderer/render_graph/topology.rs` · module `//!` doc: yes · items 5 · documented 5
- kinds (items/documented): struct:1/1 fn:4/4

### halcyon::renderer::render_target

- file: `src/renderer/render_target.rs` · module `//!` doc: yes · items 38 · documented 38
- kinds (items/documented): struct:5/5 enum:1/1 fn:32/32

### halcyon::renderer::staging_pool

- file: `src/renderer/staging_pool.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:2/2 fn:5/5

### halcyon::renderer::text_vertex

- file: `src/renderer/text_vertex.rs` · module `//!` doc: yes · items 2 · documented 1
- kinds (items/documented): struct:1/1 fn:1/0

### halcyon::renderer::vertex

- file: `src/renderer/vertex.rs` · module `//!` doc: yes · items 18 · documented 9
- kinds (items/documented): struct:5/5 fn:6/1 type:1/1 const:6/2

### halcyon::renderer::wgpu_renderer

- file: `src/renderer/wgpu_renderer.rs` · module `//!` doc: NO · items 43 · documented 36
- kinds (items/documented): struct:4/2 fn:38/33 const:1/1

### halcyon::runtime

- file: `src/runtime/mod.rs` · module `//!` doc: NO · items 3 · documented 0
- kinds (items/documented): use:3/0

### halcyon::runtime::actions

- file: `src/runtime/actions.rs` · module `//!` doc: NO · items 67 · documented 29
- kinds (items/documented): struct:4/4 enum:1/1 fn:18/18 type:1/1 const:43/5

### halcyon::runtime::binding_registry

- file: `src/runtime/binding_registry.rs` · module `//!` doc: NO · items 14 · documented 10
- kinds (items/documented): struct:3/2 enum:2/2 fn:9/6

### halcyon::runtime::builtins

- file: `src/runtime/builtins.rs` · module `//!` doc: NO · items 1 · documented 1
- kinds (items/documented): fn:1/1

### halcyon::semantics

- file: `src/semantics/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::semantics::hierarchy

- file: `src/semantics/hierarchy.rs` · module `//!` doc: NO · items 20 · documented 19
- kinds (items/documented): struct:3/3 enum:2/2 fn:15/14

### halcyon::utils

- file: `src/utils/mod.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::utils::coords

- file: `src/utils/coords.rs` · module `//!` doc: yes · items 8 · documented 8
- kinds (items/documented): struct:1/1 fn:7/7

### halcyon::utils::id

- file: `src/utils/id.rs` · module `//!` doc: NO · items 5 · documented 5
- kinds (items/documented): struct:2/2 fn:3/3

### halcyon::utils::macros

- file: `src/utils/macros.rs` · module `//!` doc: NO · items 0 · documented 0
- kinds (items/documented): none

### halcyon::widgets

- file: `src/widgets/mod.rs` · module `//!` doc: NO · items 17 · documented 0
- kinds (items/documented): use:17/0

### halcyon::widgets::action_palette

- file: `src/widgets/action_palette.rs` · module `//!` doc: NO · items 15 · documented 15
- kinds (items/documented): struct:1/1 fn:14/14

### halcyon::widgets::aligned_order_book

- file: `src/widgets/aligned_order_book.rs` · module `//!` doc: NO · items 21 · documented 5
- kinds (items/documented): struct:2/2 enum:1/1 fn:18/2

### halcyon::widgets::auxiliary_text

- file: `src/widgets/auxiliary_text.rs` · module `//!` doc: NO · items 4 · documented 1
- kinds (items/documented): struct:1/1 fn:3/0

### halcyon::widgets::breadcrumbs

- file: `src/widgets/breadcrumbs.rs` · module `//!` doc: NO · items 7 · documented 5
- kinds (items/documented): struct:2/1 fn:5/4

### halcyon::widgets::busy_indicator

- file: `src/widgets/busy_indicator.rs` · module `//!` doc: NO · items 5 · documented 5
- kinds (items/documented): struct:1/1 fn:4/4

### halcyon::widgets::button

- file: `src/widgets/button.rs` · module `//!` doc: NO · items 8 · documented 3
- kinds (items/documented): struct:1/1 enum:1/0 fn:6/2

### halcyon::widgets::checkbox

- file: `src/widgets/checkbox.rs` · module `//!` doc: NO · items 19 · documented 19
- kinds (items/documented): struct:2/2 enum:1/1 fn:16/16

### halcyon::widgets::color_picker

- file: `src/widgets/color_picker.rs` · module `//!` doc: NO · items 7 · documented 5
- kinds (items/documented): struct:1/1 fn:6/4

### halcyon::widgets::color_picker_field

- file: `src/widgets/color_picker_field.rs` · module `//!` doc: NO · items 9 · documented 2
- kinds (items/documented): struct:1/0 fn:8/2

### halcyon::widgets::combo_box

- file: `src/widgets/combo_box.rs` · module `//!` doc: yes · items 23 · documented 15
- kinds (items/documented): struct:1/1 fn:22/14

### halcyon::widgets::console

- file: `src/widgets/console.rs` · module `//!` doc: NO · items 29 · documented 21
- kinds (items/documented): struct:8/5 enum:5/4 fn:16/12

### halcyon::widgets::container

- file: `src/widgets/container.rs` · module `//!` doc: NO · items 7 · documented 2
- kinds (items/documented): struct:1/1 fn:6/1

### halcyon::widgets::context_menu

- file: `src/widgets/context_menu.rs` · module `//!` doc: NO · items 5 · documented 0
- kinds (items/documented): struct:2/0 fn:3/0

### halcyon::widgets::date_picker

- file: `src/widgets/date_picker.rs` · module `//!` doc: NO · items 39 · documented 17
- kinds (items/documented): struct:4/1 enum:2/2 fn:31/14 const:2/0

### halcyon::widgets::date_picker_field

- file: `src/widgets/date_picker_field.rs` · module `//!` doc: NO · items 12 · documented 1
- kinds (items/documented): struct:1/0 enum:1/1 fn:10/0

### halcyon::widgets::dialog

- file: `src/widgets/dialog.rs` · module `//!` doc: NO · items 4 · documented 0
- kinds (items/documented): struct:3/0 fn:1/0

### halcyon::widgets::disclosure_indicator

- file: `src/widgets/disclosure_indicator.rs` · module `//!` doc: NO · items 1 · documented 1
- kinds (items/documented): fn:1/1

### halcyon::widgets::disclosure_toggle

- file: `src/widgets/disclosure_toggle.rs` · module `//!` doc: NO · items 3 · documented 1
- kinds (items/documented): fn:3/1

### halcyon::widgets::divider

- file: `src/widgets/divider.rs` · module `//!` doc: NO · items 5 · documented 1
- kinds (items/documented): struct:1/1 enum:1/0 fn:3/0

### halcyon::widgets::dropdown_menu

- file: `src/widgets/dropdown_menu.rs` · module `//!` doc: NO · items 4 · documented 0
- kinds (items/documented): struct:1/0 fn:3/0

### halcyon::widgets::event_log

- file: `src/widgets/event_log/mod.rs` · module `//!` doc: NO · items 4 · documented 0
- kinds (items/documented): use:4/0

### halcyon::widgets::exclusion_flow

- file: `src/widgets/exclusion_flow.rs` · module `//!` doc: NO · items 8 · documented 0
- kinds (items/documented): struct:1/0 enum:1/0 fn:6/0

### halcyon::widgets::fixed_spacer

- file: `src/widgets/fixed_spacer.rs` · module `//!` doc: NO · items 4 · documented 4
- kinds (items/documented): struct:1/1 fn:3/3

### halcyon::widgets::flex

- file: `src/widgets/flex.rs` · module `//!` doc: NO · items 15 · documented 10
- kinds (items/documented): struct:1/1 fn:12/7 type:2/2

### halcyon::widgets::flex_spacer

- file: `src/widgets/flex_spacer.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::widgets::flow_grid

- file: `src/widgets/flow_grid.rs` · module `//!` doc: NO · items 5 · documented 5
- kinds (items/documented): struct:1/1 fn:4/4

### halcyon::widgets::gauge

- file: `src/widgets/gauge.rs` · module `//!` doc: NO · items 10 · documented 3
- kinds (items/documented): struct:4/1 enum:2/0 fn:4/2

### halcyon::widgets::groupbox

- file: `src/widgets/groupbox.rs` · module `//!` doc: NO · items 9 · documented 2
- kinds (items/documented): struct:1/1 fn:8/1

### halcyon::widgets::keybind_editor

- file: `src/widgets/keybind_editor.rs` · module `//!` doc: NO · items 12 · documented 4
- kinds (items/documented): struct:2/1 fn:10/3

### halcyon::widgets::label

- file: `src/widgets/label.rs` · module `//!` doc: NO · items 8 · documented 4
- kinds (items/documented): struct:1/1 fn:7/3

### halcyon::widgets::linear_gauge

- file: `src/widgets/linear_gauge.rs` · module `//!` doc: NO · items 9 · documented 2
- kinds (items/documented): use:1/0 struct:2/0 enum:2/1 fn:4/1

### halcyon::widgets::list

- file: `src/widgets/list.rs` · module `//!` doc: NO · items 36 · documented 36
- kinds (items/documented): struct:1/1 fn:35/35

### halcyon::widgets::list_view

- file: `src/widgets/list_view/mod.rs` · module `//!` doc: NO · items 8 · documented 0
- kinds (items/documented): use:8/0

### halcyon::widgets::list_view::cell_painter

- file: `src/widgets/list_view/cell_painter.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): struct:1/1 trait:1/1

### halcyon::widgets::menu_bar

- file: `src/widgets/menu_bar.rs` · module `//!` doc: NO · items 16 · documented 10
- kinds (items/documented): struct:7/6 enum:1/1 fn:8/3

### halcyon::widgets::menu_surface

- file: `src/widgets/menu_surface.rs` · module `//!` doc: NO · items 11 · documented 7
- kinds (items/documented): struct:1/1 fn:10/6

### halcyon::widgets::navigator

- file: `src/widgets/navigator/mod.rs` · module `//!` doc: NO · items 3 · documented 0
- kinds (items/documented): use:3/0

### halcyon::widgets::navigator::body

- file: `src/widgets/navigator/body.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): struct:1/0

### halcyon::widgets::navigator::data_source

- file: `src/widgets/navigator/data_source.rs` · module `//!` doc: NO · items 1 · documented 1
- kinds (items/documented): trait:1/1

### halcyon::widgets::navigator::item_painter

- file: `src/widgets/navigator/item_painter.rs` · module `//!` doc: NO · items 1 · documented 1
- kinds (items/documented): trait:1/1

### halcyon::widgets::navigator::navigator

- file: `src/widgets/navigator/navigator.rs` · module `//!` doc: NO · items 28 · documented 14
- kinds (items/documented): struct:3/2 enum:3/1 fn:22/11

### halcyon::widgets::notification_log

- file: `src/widgets/notification_log.rs` · module `//!` doc: NO · items 6 · documented 0
- kinds (items/documented): struct:1/0 fn:5/0

### halcyon::widgets::order_book

- file: `src/widgets/order_book.rs` · module `//!` doc: NO · items 19 · documented 7
- kinds (items/documented): struct:2/2 enum:3/3 fn:14/2

### halcyon::widgets::order_book_body

- file: `src/widgets/order_book_body.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): struct:1/0

### halcyon::widgets::panel

- file: `src/widgets/panel.rs` · module `//!` doc: NO · items 9 · documented 2
- kinds (items/documented): struct:1/1 fn:8/1

### halcyon::widgets::popup_menu

- file: `src/widgets/popup_menu.rs` · module `//!` doc: NO · items 2 · documented 0
- kinds (items/documented): struct:1/0 fn:1/0

### halcyon::widgets::progress_bar

- file: `src/widgets/progress_bar.rs` · module `//!` doc: NO · items 8 · documented 8
- kinds (items/documented): struct:1/1 fn:7/7

### halcyon::widgets::property_editor

- file: `src/widgets/property_editor/mod.rs` · module `//!` doc: NO · items 8 · documented 0
- kinds (items/documented): use:8/0

### halcyon::widgets::property_editor::policy

- file: `src/widgets/property_editor/policy.rs` · module `//!` doc: NO · items 11 · documented 1
- kinds (items/documented): struct:1/1 enum:10/0

### halcyon::widgets::property_editor::types

- file: `src/widgets/property_editor/types.rs` · module `//!` doc: NO · items 4 · documented 4
- kinds (items/documented): struct:1/1 enum:2/2 trait:1/1

### halcyon::widgets::radio_button

- file: `src/widgets/radio_button.rs` · module `//!` doc: NO · items 8 · documented 8
- kinds (items/documented): struct:2/2 fn:6/6

### halcyon::widgets::radio_group

- file: `src/widgets/radio_group.rs` · module `//!` doc: NO · items 4 · documented 4
- kinds (items/documented): struct:2/2 enum:1/1 fn:1/1

### halcyon::widgets::radio_group_widget

- file: `src/widgets/radio_group_widget.rs` · module `//!` doc: NO · items 17 · documented 17
- kinds (items/documented): struct:2/2 fn:15/15

### halcyon::widgets::range_slider

- file: `src/widgets/range_slider.rs` · module `//!` doc: NO · items 14 · documented 0
- kinds (items/documented): struct:2/0 enum:1/0 fn:11/0

### halcyon::widgets::readout

- file: `src/widgets/readout.rs` · module `//!` doc: yes · items 52 · documented 51
- kinds (items/documented): struct:10/9 enum:3/3 fn:39/39

### halcyon::widgets::screen_host

- file: `src/widgets/screen_host.rs` · module `//!` doc: NO · items 6 · documented 5
- kinds (items/documented): struct:1/1 fn:5/4

### halcyon::widgets::scroll_area

- file: `src/widgets/scroll_area.rs` · module `//!` doc: NO · items 58 · documented 57
- kinds (items/documented): struct:1/1 enum:1/1 fn:56/55

### halcyon::widgets::scrollbar

- file: `src/widgets/scrollbar.rs` · module `//!` doc: NO · items 21 · documented 19
- kinds (items/documented): struct:4/4 enum:1/1 fn:16/14

### halcyon::widgets::section_header

- file: `src/widgets/section_header.rs` · module `//!` doc: NO · items 6 · documented 4
- kinds (items/documented): struct:1/1 enum:1/1 fn:4/2

### halcyon::widgets::section_surfaces

- file: `src/widgets/section_surfaces.rs` · module `//!` doc: NO · items 23 · documented 0
- kinds (items/documented): struct:3/0 enum:2/0 fn:18/0

### halcyon::widgets::segmented_control

- file: `src/widgets/segmented_control.rs` · module `//!` doc: NO · items 10 · documented 1
- kinds (items/documented): struct:2/0 fn:8/1

### halcyon::widgets::separator_group

- file: `src/widgets/separator_group.rs` · module `//!` doc: NO · items 3 · documented 0
- kinds (items/documented): struct:1/0 fn:2/0

### halcyon::widgets::slider

- file: `src/widgets/slider.rs` · module `//!` doc: NO · items 22 · documented 22
- kinds (items/documented): struct:3/3 enum:2/2 fn:17/17

### halcyon::widgets::sparkline

- file: `src/widgets/sparkline.rs` · module `//!` doc: NO · items 6 · documented 0
- kinds (items/documented): struct:2/0 fn:4/0

### halcyon::widgets::spin_box

- file: `src/widgets/spin_box.rs` · module `//!` doc: NO · items 20 · documented 20
- kinds (items/documented): struct:3/3 enum:1/1 fn:16/16

### halcyon::widgets::split_pane

- file: `src/widgets/split_pane.rs` · module `//!` doc: NO · items 7 · documented 1
- kinds (items/documented): struct:1/1 fn:6/0

### halcyon::widgets::switch

- file: `src/widgets/switch.rs` · module `//!` doc: NO · items 19 · documented 18
- kinds (items/documented): struct:2/1 enum:2/2 fn:15/15

### halcyon::widgets::tab_bar

- file: `src/widgets/tab_bar.rs` · module `//!` doc: NO · items 8 · documented 6
- kinds (items/documented): struct:2/1 fn:6/5

### halcyon::widgets::tab_switcher

- file: `src/widgets/tab_switcher.rs` · module `//!` doc: NO · items 2 · documented 0
- kinds (items/documented): struct:1/0 fn:1/0

### halcyon::widgets::tabular_view

- file: `src/widgets/tabular_view/mod.rs` · module `//!` doc: NO · items 111 · documented 95
- kinds (items/documented): use:15/0 struct:4/4 enum:1/1 fn:88/87 const:3/3

### halcyon::widgets::tabular_view::body

- file: `src/widgets/tabular_view/body.rs` · module `//!` doc: NO · items 32 · documented 32
- kinds (items/documented): struct:1/1 enum:1/1 fn:30/30

### halcyon::widgets::tabular_view::cell_content

- file: `src/widgets/tabular_view/cell_content.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): struct:1/1 fn:1/1

### halcyon::widgets::tabular_view::cell_lifecycle

- file: `src/widgets/tabular_view/cell_lifecycle.rs` · module `//!` doc: NO · items 46 · documented 45
- kinds (items/documented): struct:3/3 fn:43/42

### halcyon::widgets::tabular_view::column

- file: `src/widgets/tabular_view/column.rs` · module `//!` doc: NO · items 1 · documented 0
- kinds (items/documented): use:1/0

### halcyon::widgets::tabular_view::column_config

- file: `src/widgets/tabular_view/column_config.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:2/2 fn:5/5

### halcyon::widgets::tabular_view::data_adapter

- file: `src/widgets/tabular_view/data_adapter.rs` · module `//!` doc: yes · items 64 · documented 49
- kinds (items/documented): struct:13/13 enum:13/13 trait:1/1 fn:37/22

### halcyon::widgets::tabular_view::dirty

- file: `src/widgets/tabular_view/dirty.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:1/1 enum:2/2 fn:4/4

### halcyon::widgets::tabular_view::expandable_row_host

- file: `src/widgets/tabular_view/expandable_row_host.rs` · module `//!` doc: NO · items 10 · documented 10
- kinds (items/documented): struct:1/1 fn:9/9

### halcyon::widgets::tabular_view::geometry

- file: `src/widgets/tabular_view/geometry.rs` · module `//!` doc: NO · items 2 · documented 1
- kinds (items/documented): use:1/0 type:1/1

### halcyon::widgets::tabular_view::guards

- file: `src/widgets/tabular_view/guards.rs` · module `//!` doc: NO · items 31 · documented 31
- kinds (items/documented): struct:4/4 enum:1/1 fn:26/26

### halcyon::widgets::tabular_view::header

- file: `src/widgets/tabular_view/header.rs` · module `//!` doc: NO · items 22 · documented 22
- kinds (items/documented): struct:2/2 enum:1/1 fn:19/19

### halcyon::widgets::tabular_view::intent_policy

- file: `src/widgets/tabular_view/intent_policy.rs` · module `//!` doc: NO · items 12 · documented 12
- kinds (items/documented): struct:2/2 enum:5/5 fn:5/5

### halcyon::widgets::tabular_view::intents

- file: `src/widgets/tabular_view/intents.rs` · module `//!` doc: NO · items 21 · documented 21
- kinds (items/documented): enum:15/15 fn:6/6

### halcyon::widgets::tabular_view::row

- file: `src/widgets/tabular_view/row.rs` · module `//!` doc: NO · items 36 · documented 35
- kinds (items/documented): struct:5/5 enum:1/1 fn:30/29

### halcyon::widgets::tabular_view::separators

- file: `src/widgets/tabular_view/separators.rs` · module `//!` doc: NO · items 5 · documented 5
- kinds (items/documented): struct:1/1 fn:1/1 const:3/3

### halcyon::widgets::tabular_view::sort

- file: `src/widgets/tabular_view/sort.rs` · module `//!` doc: NO · items 13 · documented 13
- kinds (items/documented): struct:2/2 trait:1/1 fn:10/10

### halcyon::widgets::tabular_view::tree_paint

- file: `src/widgets/tabular_view/tree_paint.rs` · module `//!` doc: NO · items 12 · documented 12
- kinds (items/documented): struct:3/3 enum:2/2 trait:1/1 fn:6/6

### halcyon::widgets::tabular_view::view_config

- file: `src/widgets/tabular_view/view_config.rs` · module `//!` doc: NO · items 17 · documented 17
- kinds (items/documented): struct:2/2 trait:2/2 fn:13/13

### halcyon::widgets::tag_input

- file: `src/widgets/tag_input.rs` · module `//!` doc: yes · items 23 · documented 3
- kinds (items/documented): struct:3/1 enum:4/2 fn:16/0

### halcyon::widgets::text_area

- file: `src/widgets/text_area.rs` · module `//!` doc: NO · items 58 · documented 51
- kinds (items/documented): struct:3/3 enum:1/1 fn:54/47

### halcyon::widgets::text_block

- file: `src/widgets/text_block.rs` · module `//!` doc: NO · items 10 · documented 4
- kinds (items/documented): struct:1/1 fn:9/3

### halcyon::widgets::text_document

- file: `src/widgets/text_document.rs` · module `//!` doc: yes · items 14 · documented 14
- kinds (items/documented): struct:1/1 fn:13/13

### halcyon::widgets::text_edit

- file: `src/widgets/text_edit.rs` · module `//!` doc: NO · items 7 · documented 7
- kinds (items/documented): struct:1/1 fn:6/6

### halcyon::widgets::text_field

- file: `src/widgets/text_field.rs` · module `//!` doc: NO · items 35 · documented 28
- kinds (items/documented): struct:3/3 enum:1/1 fn:31/24

### halcyon::widgets::text_layout_cache

- file: `src/widgets/text_layout_cache.rs` · module `//!` doc: yes · items 27 · documented 27
- kinds (items/documented): struct:1/1 fn:26/26

### halcyon::widgets::text_widget_test_harness

- file: `src/widgets/text_widget_test_harness.rs` · module `//!` doc: NO · items 2 · documented 2
- kinds (items/documented): trait:2/2

### halcyon::widgets::time_picker

- file: `src/widgets/time_picker.rs` · module `//!` doc: NO · items 20 · documented 2
- kinds (items/documented): use:1/0 struct:2/1 enum:2/1 fn:15/0

### halcyon::widgets::timeline_canvas

- file: `src/widgets/timeline_canvas.rs` · module `//!` doc: NO · items 12 · documented 7
- kinds (items/documented): use:1/0 struct:1/0 enum:1/1 fn:9/6

### halcyon::widgets::timeline_header

- file: `src/widgets/timeline_header.rs` · module `//!` doc: NO · items 2 · documented 0
- kinds (items/documented): struct:1/0 fn:1/0

### halcyon::widgets::timeline_spatial

- file: `src/widgets/timeline_spatial.rs` · module `//!` doc: NO · items 11 · documented 6
- kinds (items/documented): struct:4/4 enum:2/2 fn:5/0

### halcyon::widgets::toast

- file: `src/widgets/toast.rs` · module `//!` doc: NO · items 2 · documented 0
- kinds (items/documented): struct:1/0 fn:1/0

### halcyon::widgets::tooltip_host

- file: `src/widgets/tooltip_host.rs` · module `//!` doc: NO · items 4 · documented 4
- kinds (items/documented): struct:1/1 fn:3/3

### halcyon::widgets::tooltip_surface

- file: `src/widgets/tooltip_surface.rs` · module `//!` doc: NO · items 6 · documented 4
- kinds (items/documented): struct:1/0 fn:5/4

### halcyon::widgets::tree_view

- file: `src/widgets/tree_view/mod.rs` · module `//!` doc: NO · items 3 · documented 0
- kinds (items/documented): use:3/0

### halcyon::widgets::trigger_chrome

- file: `src/widgets/trigger_chrome.rs` · module `//!` doc: NO · items 3 · documented 3
- kinds (items/documented): struct:2/2 fn:1/1

## 3. Missing-Rustdoc Items (every exported item without a doc line)

```text
src/lib.rs:22 use crate (assoc)
src/lib.rs:23 use crate (assoc)
src/lib.rs:24 use crate (assoc)
src/lib.rs:25 use crate (assoc)
src/lib.rs:26 use crate (assoc)
src/lib.rs:27 use crate (assoc)
src/lib.rs:28 use crate (assoc)
src/lib.rs:29 use crate (assoc)
src/lib.rs:30 use crate (assoc)
src/lib.rs:31 use crate (assoc)
src/lib.rs:32 use crate (assoc)
src/lib.rs:33 use crate (assoc)
src/lib.rs:34 use crate (assoc)
src/lib.rs:35 use crate (assoc)
src/lib.rs:36 use crate (assoc)
src/lib.rs:38 use crate (assoc)
src/lib.rs:39 use crate (assoc)
src/lib.rs:41 use crate (assoc)
src/lib.rs:42 use crate (assoc)
src/lib.rs:43 use crate (assoc)
src/lib.rs:44 use crate (assoc)
src/lib.rs:45 use crate (assoc)
src/lib.rs:46 use crate (assoc)
src/lib.rs:47 use crate (assoc)
src/lib.rs:48 use crate (assoc)
src/lib.rs:49 use crate (assoc)
src/lib.rs:50 use crate (assoc)
src/lib.rs:51 use crate (assoc)
src/lib.rs:52 use crate (assoc)
src/lib.rs:53 use crate (assoc)
src/lib.rs:54 use crate (assoc)
src/lib.rs:55 use crate (assoc)
src/lib.rs:56 use crate (assoc)
src/lib.rs:57 use crate (assoc)
src/agent/mod.rs:14 use tower
src/agent/llm_client.rs:16 fn new (assoc)
src/agent/orchestrator/mod.rs:13 use directive
src/agent/orchestrator/mod.rs:14 use dispatch
src/agent/orchestrator/mod.rs:15 use error
src/agent/orchestrator/mod.rs:16 use operation
src/agent/orchestrator/directive.rs:37 fn new (assoc)
src/agent/orchestrator/directive.rs:48 fn add_task (assoc)
src/agent/orchestrator/dispatch.rs:24 fn new (assoc)
src/agent/orchestrator/error.rs:4 enum DirectiveError
src/agent/orchestrator/error.rs:40 fn is_new (assoc)
src/agent/orchestrator/error.rs:43 fn is_terminal (assoc)
src/agent/orchestrator/event.rs:5 enum DirectiveEvent
src/agent/orchestrator/operation.rs:38 fn new (assoc)
src/agent/orchestrator/task.rs:55 fn new (assoc)
src/agent/orchestrator/task.rs:66 fn add_operation (assoc)
src/agent/tower.rs:204 fn new (assoc)
src/compositor/mod.rs:60 use contract
src/compositor/mod.rs:64 use stack
src/compositor/mod.rs:68 use surface
src/compositor/mod.rs:71 use surface_role
src/compositor/mod.rs:86 use surface_role
src/compositor/mod.rs:95 struct CompositorIdentity
src/compositor/mod.rs:125 fn new (assoc)
src/compositor/mod.rs:129 fn set_z_index (assoc)
src/compositor/mod.rs:134 fn set_bounds (assoc)
src/compositor/mod.rs:139 fn set_visibility (assoc)
src/compositor/mod.rs:144 fn destroy (assoc)
src/compositor/mod.rs:148 fn is_empty (assoc)
src/compositor/mod.rs:154 enum SurfaceBatchPreparationError
src/compositor/mod.rs:357 fn apply_frame_projection (assoc)
src/core/action_palette_modal.rs:93 fn new (assoc)
src/core/agent.rs:19 fn new (assoc)
src/core/agent_client.rs:14 enum AgentClientError
src/core/agent_client.rs:34 fn connect (assoc)
src/core/agent_client.rs:45 fn with_response_timeout (assoc)
src/core/agent_client.rs:57 fn query (assoc)
src/core/agent_client.rs:63 fn find_widgets (assoc)
src/core/agent_client.rs:77 fn widget_state (assoc)
src/core/agent_client.rs:81 fn focus_state (assoc)
src/core/agent_server.rs:34 fn name (assoc)
src/core/animation.rs:92 fn new (assoc)
src/core/animation.rs:221 fn new (assoc)
src/core/app.rs:245 fn toggle_debug_outlines
src/core/app.rs:251 fn debug_outlines_enabled
src/core/app.rs:263 struct ResizeCounters (assoc)
src/core/app.rs:292 fn new (assoc)
src/core/app.rs:299 fn maybe_report (assoc)
src/core/app.rs:346 static COUNTERS (assoc)
src/core/app.rs:349 fn is_enabled (assoc)
src/core/app.rs:354 fn inc_resize_event (assoc)
src/core/app.rs:360 fn inc_mouse_move_event (assoc)
src/core/app.rs:366 fn inc_layout_pass (assoc)
src/core/app.rs:375 fn inc_layout_wrap_call (assoc)
src/core/app.rs:381 fn inc_layout_child_call (assoc)
src/core/app.rs:390 fn inc_relayout_child_call (assoc)
src/core/app.rs:399 fn inc_text_measure_call (assoc)
src/core/app.rs:405 fn add_layout_time_us (assoc)
src/core/app.rs:414 fn add_text_time_us (assoc)
src/core/app.rs:420 fn add_build_scene_time_us (assoc)
src/core/app.rs:429 fn add_draw_plan_time_us (assoc)
src/core/app.rs:438 fn add_compile_time_us (assoc)
src/core/app.rs:447 fn add_render_time_us (assoc)
src/core/app.rs:456 fn add_submit_time_us (assoc)
src/core/app.rs:465 fn inc_frame (assoc)
src/core/app.rs:489 use resize_profiler
src/core/app.rs:1167 fn post_layout_sync_scroll_to_lists
src/core/app.rs:1580 fn build_scene
src/core/app.rs:4306 struct ChildWindowRequest
src/core/app.rs:4316 struct ChildWindowSceneState
src/core/app.rs:4327 enum ChildWindowAction
src/core/app.rs:4333 struct ChildWindowActionRegion
src/core/app.rs:4338 struct ChildWindowPaintContext
src/core/app.rs:4387 fn new (assoc)
src/core/app.rs:4440 fn request_child_window (assoc)
src/core/app.rs:4461 fn take_child_window_requests (assoc)
src/core/app.rs:4638 fn request_child_window (assoc)
src/core/app.rs:4952 fn new (assoc)
src/core/app.rs:4956 fn is_empty (assoc)
src/core/app.rs:4960 fn len (assoc)
src/core/app.rs:5036 fn new (assoc)
src/core/app.rs:5040 fn is_empty (assoc)
src/core/app.rs:5044 fn len (assoc)
src/core/app.rs:5151 fn new (assoc)
src/core/app.rs:5178 fn new (assoc)
src/core/app.rs:5184 fn is_empty (assoc)
src/core/drag_drop.rs:30 fn new (assoc)
src/core/drag_drop.rs:83 fn new (assoc)
src/core/drag_drop.rs:87 fn start (assoc)
src/core/drag_drop.rs:95 fn active (assoc)
src/core/drag_drop.rs:99 fn session (assoc)
src/core/drag_drop.rs:103 fn session_mut (assoc)
src/core/event/mod.rs:493 fn new (assoc)
src/core/event/mod.rs:512 fn focused (assoc)
src/core/event/mod.rs:516 fn set_focus (assoc)
src/core/event/mod.rs:584 fn last_user_focused (assoc)
src/core/event/mod.rs:931 fn new (assoc)
src/core/event/mod.rs:938 fn capture (assoc)
src/core/event/mod.rs:943 fn release (assoc)
src/core/event/mod.rs:948 fn is_captured_by (assoc)
src/core/event/mod.rs:952 fn is_captured (assoc)
src/core/event/mod.rs:1025 fn new (assoc)
src/core/event/mod.rs:1412 fn new (assoc)
src/core/event/mod.rs:1997 fn new (assoc)
src/core/event/mod.rs:2190 fn new (assoc)
src/core/event/mod.rs:2194 fn set (assoc)
src/core/event/mod.rs:2202 fn is_pressed (assoc)
src/core/event/mod.rs:2210 fn any_pressed (assoc)
src/core/event_router.rs:122 fn new (assoc)
src/core/event_router.rs:147 fn set_agent_tx (assoc)
src/core/event_router.rs:399 fn route_pointer_event (assoc)
src/core/event_router.rs:676 fn route_key_event (assoc)
src/core/event_router.rs:1949 fn dispatch_projected_input (assoc)
src/core/focus_authority.rs:9 struct FocusDomainId
src/core/focus_authority.rs:12 struct FocusDomainAddition
src/core/focus_authority.rs:18 fn new (assoc)
src/core/focus_authority.rs:24 struct FocusTargetRegistration
src/core/focus_authority.rs:32 fn new (assoc)
src/core/focus_authority.rs:48 struct FocusProjection
src/core/focus_authority.rs:53 enum FocusAuthorityError
src/core/focus_authority.rs:98 struct FocusRestorationToken
src/core/focus_authority.rs:139 fn interaction_domain_id (assoc)
src/core/focus_authority.rs:143 fn guard_generation (assoc)
src/core/focus_authority.rs:174 struct PreparedFocusDomainActivation
src/core/focus_authority.rs:179 struct PreparedOverlayFocusActivation
src/core/focus_authority.rs:185 struct PreparedOverlayFocusExit
src/core/focus_authority.rs:190 struct PreparedFocusRestoration
src/core/focus_authority.rs:195 struct PreparedFocusTargetMutation
src/core/focus_authority.rs:200 struct PreparedFocusTargetSelection
src/core/focus_authority.rs:205 struct PreparedFocusDomainGraphMutation
src/core/focus_authority.rs:217 fn testimony (assoc)
src/core/focus_authority.rs:232 struct PreparedFocusDomainRetirement
src/core/focus_authority.rs:276 fn testimony (assoc)
src/core/focus_authority.rs:299 fn interaction_domain_id (assoc)
src/core/focus_authority.rs:303 fn parent_before (assoc)
src/core/focus_authority.rs:307 fn parent_after (assoc)
src/core/focus_authority.rs:311 fn additions (assoc)
src/core/focus_authority.rs:315 fn removals (assoc)
src/core/focus_authority.rs:337 struct FocusResolver
src/core/focus_authority.rs:340 fn new (assoc)
src/core/focus_authority.rs:344 fn focus_projection_guarded (assoc)
src/core/focus_authority.rs:369 fn prepare_focus_domain_activation (assoc)
src/core/focus_authority.rs:396 fn commit_focus_domain_activation (assoc)
src/core/focus_authority.rs:404 fn prepare_overlay_focus_activation (assoc)
src/core/focus_authority.rs:457 fn commit_overlay_focus_activation (assoc)
src/core/focus_authority.rs:467 fn prepare_overlay_focus_exit_to_domain (assoc)
src/core/focus_authority.rs:523 fn commit_overlay_focus_exit_to_domain (assoc)
src/core/focus_authority.rs:531 fn prepare_restoration_tokens (assoc)
src/core/focus_authority.rs:580 fn submit_restoration_tokens_guarded (assoc)
src/core/focus_authority.rs:588 fn prepare_focus_target_mutation (assoc)
src/core/focus_authority.rs:650 fn commit_focus_target_mutation (assoc)
src/core/focus_authority.rs:658 fn prepare_focus_target_selection (assoc)
src/core/focus_authority.rs:689 fn commit_focus_target_selection (assoc)
src/core/focus_authority.rs:697 fn prepare_focus_domain_graph_mutation (assoc)
src/core/focus_authority.rs:758 fn prepare_focus_domain_graph_transition (assoc)
src/core/focus_authority.rs:777 fn commit_focus_domain_graph_mutation (assoc)
src/core/focus_authority.rs:785 fn prepare_focus_domain_retirement (assoc)
src/core/focus_authority.rs:844 fn prepare_focus_domain_retirement_transition (assoc)
src/core/focus_authority.rs:861 fn commit_focus_domain_retirement (assoc)
src/core/focus_authority.rs:1450 fn interaction_domain_id (assoc)
src/core/focus_authority.rs:1454 fn active_focus_domain (assoc)
src/core/focus_authority.rs:1458 fn focus_domain_parent (assoc)
src/core/focus_authority.rs:1467 fn focus_domain_count (assoc)
src/core/focus_authority.rs:1471 fn restoration_depth (assoc)
src/core/frame_projection_binding_registry.rs:25 struct FrameProjectionEpoch
src/core/frame_projection_binding_registry.rs:28 fn get (assoc)
src/core/frame_projection_binding_registry.rs:34 struct ScreenHostProjectionKey
src/core/frame_projection_binding_registry.rs:40 fn new (assoc)
src/core/frame_projection_binding_registry.rs:46 enum NavigationProjectionSource
src/core/frame_projection_binding_registry.rs:55 fn standalone (assoc)
src/core/frame_projection_binding_registry.rs:65 fn registered_root (assoc)
src/core/frame_projection_binding_registry.rs:71 struct FrameProjectionBinding
src/core/frame_projection_binding_registry.rs:77 fn new (assoc)
src/core/frame_projection_binding_registry.rs:87 fn interaction_domain_id (assoc)
src/core/frame_projection_binding_registry.rs:91 fn navigation_sources (assoc)
src/core/frame_projection_binding_registry.rs:95 fn references_root (assoc)
src/core/frame_projection_binding_registry.rs:113 struct VersionedFrameProjectionBinding
src/core/frame_projection_binding_registry.rs:120 fn epoch (assoc)
src/core/frame_projection_binding_registry.rs:124 fn binding (assoc)
src/core/frame_projection_binding_registry.rs:130 struct FrameProjectionAuthorityAccess
src/core/frame_projection_binding_registry.rs:139 fn new (assoc)
src/core/frame_projection_binding_registry.rs:153 fn with_registered_roots (assoc)
src/core/frame_projection_binding_registry.rs:164 struct FrameProjectionWindowAccess
src/core/frame_projection_binding_registry.rs:172 fn new (assoc)
src/core/frame_projection_binding_registry.rs:187 struct FrameProjectionRuntimeAccess
src/core/frame_projection_binding_registry.rs:195 fn new (assoc)
src/core/frame_projection_binding_registry.rs:211 enum FrameProjectionPullOutcome
src/core/frame_projection_binding_registry.rs:217 enum FrameProjectionPullError
src/core/frame_projection_binding_registry.rs:255 fn pull_navigation_frame_projection
src/core/frame_projection_binding_registry.rs:435 enum FrameProjectionBindingError
src/core/frame_projection_binding_registry.rs:599 struct FrameProjectionBindingRegistry
src/core/frame_projection_binding_registry.rs:615 fn new (assoc)
src/core/frame_projection_binding_registry.rs:628 fn guard_identity (assoc)
src/core/frame_projection_binding_registry.rs:639 fn install_or_replace (assoc)
src/core/frame_projection_binding_registry.rs:688 fn remove (assoc)
src/core/frame_projection_binding_registry.rs:714 fn prepare_remove_or_replace_guarded (assoc)
src/core/frame_projection_binding_registry.rs:768 fn commit_remove_or_replace_guarded (assoc)
src/core/frame_projection_binding_registry.rs:818 fn invalidate_missing_screen_host (assoc)
src/core/frame_projection_binding_registry.rs:848 fn root_consumers (assoc)
src/core/frame_projection_binding_registry.rs:875 fn domain_consumers (assoc)
src/core/frame_projection_binding_registry.rs:900 fn binding_guarded (assoc)
src/core/frame_projection_binding_registry.rs:1141 struct PreparedFrameProjectionBindingMutation
src/core/frame_projection_binding_registry.rs:1154 fn key (assoc)
src/core/icon.rs:14 const CLOSE (assoc)
src/core/icon.rs:15 const CHEVRON_RIGHT (assoc)
src/core/icon.rs:16 const CHEVRON_DOWN (assoc)
src/core/icon.rs:17 const PLUS (assoc)
src/core/icon.rs:18 const MINUS (assoc)
src/core/icon.rs:19 const CHECK (assoc)
src/core/icon.rs:20 const SEARCH (assoc)
src/core/icon.rs:21 const ARROW_UP (assoc)
src/core/icon.rs:22 const ARROW_DOWN (assoc)
src/core/icon.rs:23 const WARNING (assoc)
src/core/icon.rs:24 const FOLDER (assoc)
src/core/icon.rs:25 const FILE (assoc)
src/core/icon.rs:26 const FOLDER_OPEN (assoc)
src/core/icon.rs:27 const CALENDAR (assoc)
src/core/icon.rs:28 const SELECT_ALL (assoc)
src/core/icon.rs:29 const CLEAR_ALL (assoc)
src/core/icon.rs:110 fn logical_px (assoc)
src/core/icon.rs:148 fn new (assoc)
src/core/icon.rs:152 fn get (assoc)
src/core/icon.rs:156 fn len (assoc)
src/core/icon.rs:160 fn is_empty (assoc)
src/core/icon_builder.rs:29 fn new (assoc)
src/core/input_fusion.rs:186 fn new (assoc)
src/core/input_fusion.rs:321 fn new (assoc)
src/core/input_fusion.rs:354 fn new (assoc)
src/core/input_fusion.rs:394 fn new (assoc)
src/core/input_fusion.rs:421 fn new (assoc)
src/core/input_fusion.rs:527 fn new (assoc)
src/core/input_projection.rs:25 fn new (assoc)
src/core/input_projection.rs:65 fn new (assoc)
src/core/input_projection.rs:299 fn new (assoc)
src/core/interaction_fuzz.rs:24 fn serialize (assoc)
src/core/interaction_fuzz.rs:39 fn deserialize (assoc)
src/core/interaction_fuzz.rs:106 struct InteractionDriverConfig
src/core/interaction_fuzz.rs:123 enum InteractionCommand
src/core/interaction_fuzz.rs:177 fn pointer_move (assoc)
src/core/interaction_fuzz.rs:181 fn pointer_down (assoc)
src/core/interaction_fuzz.rs:189 fn pointer_up (assoc)
src/core/interaction_fuzz.rs:197 fn frame (assoc)
src/core/interaction_fuzz.rs:203 enum InteractionDriverError
src/core/interaction_fuzz.rs:212 type InteractionDriverResult
src/core/interaction_fuzz.rs:215 struct InteractionIntentTarget
src/core/interaction_fuzz.rs:226 fn resolved_point (assoc)
src/core/interaction_fuzz.rs:246 fn synthetic_point (assoc)
src/core/interaction_fuzz.rs:262 fn from_trace (assoc)
src/core/interaction_fuzz.rs:274 fn trace (assoc)
src/core/interaction_fuzz.rs:288 enum InteractionIntentCommand
src/core/interaction_fuzz.rs:326 fn move_pointer_to (assoc)
src/core/interaction_fuzz.rs:337 fn move_pointer_to_target (assoc)
src/core/interaction_fuzz.rs:347 fn drag_pointer_to (assoc)
src/core/interaction_fuzz.rs:369 fn drag_pointer_to_targets (assoc)
src/core/interaction_fuzz.rs:382 fn click_target (assoc)
src/core/interaction_fuzz.rs:393 fn click_interaction_target (assoc)
src/core/interaction_fuzz.rs:404 fn wheel_at_target (assoc)
src/core/interaction_fuzz.rs:425 fn wheel_at_interaction_target (assoc)
src/core/interaction_fuzz.rs:441 fn keyboard_navigation (assoc)
src/core/interaction_fuzz.rs:455 fn text_at_target (assoc)
src/core/interaction_fuzz.rs:474 fn text_at_interaction_target (assoc)
src/core/interaction_fuzz.rs:513 enum WheelDeltaMode
src/core/interaction_fuzz.rs:520 enum PointerPathPolicy
src/core/interaction_fuzz.rs:546 enum KeyboardNavigationPolicy
src/core/interaction_fuzz.rs:555 enum PointerApproach
src/core/interaction_fuzz.rs:562 enum PointerFramePolicy
src/core/interaction_fuzz.rs:572 enum PointerPathSegment
src/core/interaction_fuzz.rs:589 enum IntentCommandClass
src/core/interaction_fuzz.rs:600 enum PointerPathPolicyClass
src/core/interaction_fuzz.rs:624 enum EndpointTeleportationMode
src/core/interaction_fuzz.rs:631 enum PointerPathSampleCountClass
src/core/interaction_fuzz.rs:638 enum PointerMoveExposureClass
src/core/interaction_fuzz.rs:645 enum PointerCaptureTransitionClass
src/core/interaction_fuzz.rs:653 enum CoverageKey
src/core/interaction_fuzz.rs:671 enum PrimitiveCommandClass
src/core/interaction_fuzz.rs:706 struct InteractionCoverage
src/core/interaction_fuzz.rs:711 fn new (assoc)
src/core/interaction_fuzz.rs:715 fn record_intent_trace (assoc)
src/core/interaction_fuzz.rs:809 fn record_key (assoc)
src/core/interaction_fuzz.rs:813 fn contains (assoc)
src/core/interaction_fuzz.rs:817 fn total_keys (assoc)
src/core/interaction_fuzz.rs:821 fn keys (assoc)
src/core/interaction_fuzz.rs:826 const INTERACTION_FUZZ_SCHEMA
src/core/interaction_fuzz.rs:827 const INTERACTION_REPLAY_SCHEMA_VERSION
src/core/interaction_fuzz.rs:828 const INTERACTION_CONFIDENCE_POSITIVE_CLAIM
src/core/interaction_fuzz.rs:829 const ORACLE_346_U1_ID
src/core/interaction_fuzz.rs:830 const ORACLE_346_U1_NAME
src/core/interaction_fuzz.rs:831 const ORACLE_346_U3_ID
src/core/interaction_fuzz.rs:832 const ORACLE_346_U3_NAME
src/core/interaction_fuzz.rs:833 const ORACLE_346_U7_ID
src/core/interaction_fuzz.rs:834 const ORACLE_346_U7_NAME
src/core/interaction_fuzz.rs:835 const ORACLE_346_U11_ID
src/core/interaction_fuzz.rs:836 const ORACLE_346_U11_NAME
src/core/interaction_fuzz.rs:839 struct ReplayPoint
src/core/interaction_fuzz.rs:861 struct InteractionIntentTargetTrace
src/core/interaction_fuzz.rs:873 enum ReplayPointerButton
src/core/interaction_fuzz.rs:900 struct ReplayModifiers
src/core/interaction_fuzz.rs:931 enum ReplayKeyCode
src/core/interaction_fuzz.rs:1043 enum ReplayInteractionCommand
src/core/interaction_fuzz.rs:1253 enum ReplayInteractionIntentCommand
src/core/interaction_fuzz.rs:1432 struct ReplayPrimitiveTraceStep
src/core/interaction_fuzz.rs:1450 struct ReplayIntentExpansionStep
src/core/interaction_fuzz.rs:1470 struct ReplayIntentTraceStep
src/core/interaction_fuzz.rs:1500 enum ReplayMode
src/core/interaction_fuzz.rs:1510 struct InteractionReplayArtifact
src/core/interaction_fuzz.rs:1523 fn from_driver (assoc)
src/core/interaction_fuzz.rs:1551 fn from_shrunk_driver (assoc)
src/core/interaction_fuzz.rs:1566 fn primitive_commands (assoc)
src/core/interaction_fuzz.rs:1573 fn replay_into (assoc)
src/core/interaction_fuzz.rs:1580 fn replay_shrunk_trace_into (assoc)
src/core/interaction_fuzz.rs:1588 fn is_supported_schema (assoc)
src/core/interaction_fuzz.rs:1596 enum InteractionInjectionPath
src/core/interaction_fuzz.rs:1604 enum InteractionSettleBoundary
src/core/interaction_fuzz.rs:1610 type InteractionFuzzProfileHash
src/core/interaction_fuzz.rs:1614 enum InteractionFuzzMode
src/core/interaction_fuzz.rs:1627 struct InteractionGeneratorVersionRecord
src/core/interaction_fuzz.rs:1633 struct InteractionFuzzProfile
src/core/interaction_fuzz.rs:1664 fn profile_hash (assoc)
src/core/interaction_fuzz.rs:1675 fn is_complete (assoc)
src/core/interaction_fuzz.rs:1739 struct InteractionTimeoutPolicy
src/core/interaction_fuzz.rs:1750 fn is_complete (assoc)
src/core/interaction_fuzz.rs:1761 struct InteractionProgressMarker
src/core/interaction_fuzz.rs:1774 struct InteractionVisualDiagnosticsPolicy
src/core/interaction_fuzz.rs:1781 fn is_complete (assoc)
src/core/interaction_fuzz.rs:1787 fn status (assoc)
src/core/interaction_fuzz.rs:1817 struct InteractionVisualDiagnosticsStatus
src/core/interaction_fuzz.rs:1824 struct InteractionVisualTraceArtifact
src/core/interaction_fuzz.rs:1831 struct InteractionConstitutionBoundaryProfile
src/core/interaction_fuzz.rs:1850 fn is_complete (assoc)
src/core/interaction_fuzz.rs:1870 struct InteractionConstitutionBoundaryCheckStatus
src/core/interaction_fuzz.rs:1876 fn from_oracle_evaluations (assoc)
src/core/interaction_fuzz.rs:1893 const INTERACTION_CHAOS_NO_FAILURE_CLAIM
src/core/interaction_fuzz.rs:1898 enum InteractionChaosPrimitiveFamily
src/core/interaction_fuzz.rs:1912 fn from_command (assoc)
src/core/interaction_fuzz.rs:1932 enum InteractionChaosPointerPressurePattern
src/core/interaction_fuzz.rs:1947 enum InteractionChaosPointerMotionPolicy
src/core/interaction_fuzz.rs:1961 enum InteractionChaosHaltReason
src/core/interaction_fuzz.rs:1971 enum InteractionChaosFailureDetector
src/core/interaction_fuzz.rs:1985 enum InteractionChaosHardFailureClass
src/core/interaction_fuzz.rs:1998 struct InteractionChaosStreamPolicy
src/core/interaction_fuzz.rs:2005 fn endpoint_teleportation_allowed (assoc)
src/core/interaction_fuzz.rs:2011 struct InteractionChaosSurface
src/core/interaction_fuzz.rs:2063 struct InteractionChaosPressureCoverageSummary
src/core/interaction_fuzz.rs:2073 struct InteractionChaosGeneratedCommandRecord
src/core/interaction_fuzz.rs:2085 struct InteractionChaosReport
src/core/interaction_fuzz.rs:2122 struct InteractionPrimitiveStreamRange
src/core/interaction_fuzz.rs:2128 struct InteractionChaosFailureFields
src/core/interaction_fuzz.rs:2151 enum InteractionFailureSeverity
src/core/interaction_fuzz.rs:2160 enum InteractionFailureLayer
src/core/interaction_fuzz.rs:2175 enum InteractionOracleStatus
src/core/interaction_fuzz.rs:2183 enum FiniteGeometrySink
src/core/interaction_fuzz.rs:2200 struct FiniteSinkGeometrySample
src/core/interaction_fuzz.rs:2208 fn new (assoc)
src/core/interaction_fuzz.rs:2224 struct InteractionOracleEvaluation
src/core/interaction_fuzz.rs:2239 fn passed (assoc)
src/core/interaction_fuzz.rs:2244 fn evaluate_finite_sink_geometry
src/core/interaction_fuzz.rs:2314 struct CaptureOwnerSnapshot
src/core/interaction_fuzz.rs:2326 fn new (assoc)
src/core/interaction_fuzz.rs:2349 fn evaluate_capture_owner_validity
src/core/interaction_fuzz.rs:2431 struct LayoutSettledSnapshot
src/core/interaction_fuzz.rs:2440 fn new (assoc)
src/core/interaction_fuzz.rs:2457 fn evaluate_layout_settled
src/core/interaction_fuzz.rs:2510 struct IncidentalNavigationSnapshot
src/core/interaction_fuzz.rs:2522 fn evaluate_incidental_navigation_integrity
src/core/interaction_fuzz.rs:2687 struct InteractionTargetSnapshot
src/core/interaction_fuzz.rs:2694 struct InteractionStateSnapshot
src/core/interaction_fuzz.rs:2700 struct InteractionProbeSnapshotArtifact
src/core/interaction_fuzz.rs:2711 fn from_serializable (assoc)
src/core/interaction_fuzz.rs:2733 struct InteractionFailurePacket
src/core/interaction_fuzz.rs:2781 struct InteractionElapsedTime
src/core/interaction_fuzz.rs:2786 struct InteractionCoverageSummary
src/core/interaction_fuzz.rs:2804 fn from_coverage (assoc)
src/core/interaction_fuzz.rs:2890 struct PrimitiveStepMetrics
src/core/interaction_fuzz.rs:2897 struct IntentStepMetrics
src/core/interaction_fuzz.rs:2910 struct InteractionTraceMetrics
src/core/interaction_fuzz.rs:2926 fn from_driver (assoc)
src/core/interaction_fuzz.rs:3040 struct InteractionSaturationSummary
src/core/interaction_fuzz.rs:3047 struct InteractionConfidencePacket
src/core/interaction_fuzz.rs:3077 fn from_input (assoc)
src/core/interaction_fuzz.rs:3110 struct InteractionConfidencePacketInput
src/core/interaction_fuzz.rs:3137 struct PrimitiveTraceStep
src/core/interaction_fuzz.rs:3144 struct IntentExpansionStep
src/core/interaction_fuzz.rs:3152 struct IntentTraceStep
src/core/interaction_fuzz.rs:3162 struct InteractionDriver
src/core/interaction_fuzz.rs:3177 fn new (assoc)
src/core/interaction_fuzz.rs:3181 fn with_config (assoc)
src/core/interaction_fuzz.rs:3197 fn execute (assoc)
src/core/interaction_fuzz.rs:3202 fn try_execute (assoc)
src/core/interaction_fuzz.rs:3210 fn execute_all (assoc)
src/core/interaction_fuzz.rs:3218 fn try_execute_all (assoc)
src/core/interaction_fuzz.rs:3235 fn execute_intent (assoc)
src/core/interaction_fuzz.rs:3243 fn try_execute_intent (assoc)
src/core/interaction_fuzz.rs:3280 fn drain_fused (assoc)
src/core/interaction_fuzz.rs:3284 fn flush (assoc)
src/core/interaction_fuzz.rs:3288 fn fused_events (assoc)
src/core/interaction_fuzz.rs:3292 fn intent_trace (assoc)
src/core/interaction_fuzz.rs:3296 fn drain_intent_trace (assoc)
src/core/interaction_fuzz.rs:3300 fn coverage (assoc)
src/core/interaction_fuzz.rs:3304 fn record_coverage_key (assoc)
src/core/interaction_fuzz.rs:3308 fn trace_metrics (assoc)
src/core/interaction_fuzz.rs:3312 fn primitive_trace (assoc)
src/core/interaction_fuzz.rs:3316 fn drain_primitive_trace (assoc)
src/core/interaction_fuzz.rs:3320 fn pointer_position (assoc)
src/core/layout.rs:114 fn new (assoc)
src/core/layout.rs:235 fn new (assoc)
src/core/layout.rs:243 fn fixed (assoc)
src/core/layout.rs:251 fn flexible (assoc)
src/core/layout.rs:294 fn new (assoc)
src/core/layout.rs:303 fn all (assoc)
src/core/layout.rs:312 fn symmetric (assoc)
src/core/layout.rs:321 fn zero (assoc)
src/core/layout.rs:330 fn horizontal (assoc)
src/core/layout.rs:334 fn vertical (assoc)
src/core/layout.rs:357 fn new (assoc)
src/core/layout.rs:364 fn none (assoc)
src/core/layout.rs:371 fn uniform (assoc)
src/core/layout.rs:395 fn new (assoc)
src/core/layout.rs:404 fn all (assoc)
src/core/layout.rs:413 fn zero (assoc)
src/core/layout.rs:440 fn new (assoc)
src/core/layout.rs:456 fn none (assoc)
src/core/layout_transition.rs:1484 fn candidates (assoc)
src/core/layout_transition.rs:1488 fn into_candidates (assoc)
src/core/layout_transition.rs:2127 fn new (assoc)
src/core/layout_transition.rs:2221 fn new (assoc)
src/core/layout_transition.rs:2315 fn new (assoc)
src/core/math.rs:9 fn new (assoc)
src/core/math.rs:13 fn zero (assoc)
src/core/math.rs:17 fn distance (assoc)
src/core/math.rs:32 fn new (assoc)
src/core/math.rs:36 fn zero (assoc)
src/core/math.rs:43 fn area (assoc)
src/core/math.rs:59 fn new (assoc)
src/core/math.rs:91 fn from_pos_size (assoc)
src/core/math.rs:100 fn zero (assoc)
src/core/math.rs:109 fn pos (assoc)
src/core/math.rs:116 fn size (assoc)
src/core/math.rs:123 fn left (assoc)
src/core/math.rs:127 fn top (assoc)
src/core/math.rs:131 fn right (assoc)
src/core/math.rs:135 fn bottom (assoc)
src/core/math.rs:139 fn center (assoc)
src/core/navigation_application.rs:55 struct RoutedNavigationSnapshot
src/core/navigation_application.rs:62 fn interaction_domain_id (assoc)
src/core/navigation_application.rs:66 fn root_id (assoc)
src/core/navigation_application.rs:70 fn host_snapshot (assoc)
src/core/navigation_application.rs:74 fn orchestration_generation (assoc)
src/core/navigation_application.rs:80 enum RoutedNavigationSnapshotResult
src/core/navigation_application.rs:87 struct StandaloneNavigationSnapshot
src/core/navigation_application.rs:94 fn interaction_domain_id (assoc)
src/core/navigation_application.rs:98 fn host_snapshot (assoc)
src/core/navigation_application.rs:102 fn orchestration_generation (assoc)
src/core/navigation_application.rs:108 enum StandaloneNavigationSnapshotResult
src/core/navigation_application.rs:114 enum StandaloneNavigationPreparationError
src/core/navigation_application.rs:140 enum RoutedNavigationPreparationError
src/core/navigation_application.rs:190 enum NavigationDelegationError
src/core/navigation_application.rs:226 enum NavigationRootAdmissionError
src/core/navigation_application.rs:267 struct FrameProjectionRootConsumerPlan
src/core/navigation_application.rs:273 fn remove (assoc)
src/core/navigation_application.rs:280 fn replace (assoc)
src/core/navigation_application.rs:289 enum NavigationRootRetirementError
src/core/navigation_application.rs:349 enum NavigationDomainRetirementError
src/core/navigation_application.rs:403 struct NavigationApplicationCoordinator
src/core/navigation_application.rs:409 fn new (assoc)
src/core/navigation_application.rs:416 fn overlay_domains (assoc)
src/core/navigation_application.rs:420 fn navigation_domains (assoc)
src/core/navigation_application.rs:424 fn standalone_snapshot (assoc)
src/core/navigation_application.rs:450 fn authorize_standalone_navigation (assoc)
src/core/navigation_application.rs:472 fn prepare_standalone_navigation (assoc)
src/core/navigation_application.rs:516 fn route_snapshot (assoc)
src/core/navigation_application.rs:567 fn authorize_routed_navigation (assoc)
src/core/navigation_application.rs:603 fn prepare_routed_navigation (assoc)
src/core/navigation_application.rs:683 fn prepare_routed_navigation_with_focus_graph (assoc)
src/core/navigation_application.rs:781 fn prepare_routed_navigation_with_overlay_exit (assoc)
src/core/navigation_application.rs:864 fn prepare_routed_navigation_with_overlay_and_focus_graph (assoc)
src/core/navigation_application.rs:966 fn prepare_delegation (assoc)
src/core/navigation_application.rs:1063 fn prepare_delegation_return (assoc)
src/core/navigation_application.rs:1155 fn prepare_root_admission (assoc)
src/core/navigation_application.rs:1247 fn prepare_root_retirement (assoc)
src/core/navigation_application.rs:1599 fn prepare_domain_retirement (assoc)
src/core/navigation_application.rs:1846 struct PreparedNavigationDelegation
src/core/navigation_application.rs:1856 fn frame (assoc)
src/core/navigation_application.rs:1863 fn commit (assoc)
src/core/navigation_application.rs:1933 struct CommittedNavigationDelegation
src/core/navigation_application.rs:1939 fn frame (assoc)
src/core/navigation_application.rs:1943 fn terminated_surfaces (assoc)
src/core/navigation_application.rs:1950 struct PreparedNavigationDelegationReturn
src/core/navigation_application.rs:1960 fn frame (assoc)
src/core/navigation_application.rs:1967 fn commit (assoc)
src/core/navigation_application.rs:2039 struct CommittedNavigationDelegationReturn
src/core/navigation_application.rs:2045 fn frame (assoc)
src/core/navigation_application.rs:2049 fn terminated_surfaces (assoc)
src/core/navigation_application.rs:2073 struct PreparedNavigationRootRetirement
src/core/navigation_application.rs:2090 fn root_id (assoc)
src/core/navigation_application.rs:2094 fn commit (assoc)
src/core/navigation_application.rs:2229 struct PreparedNavigationDomainRetirement
src/core/navigation_application.rs:2251 fn interaction_domain_id (assoc)
src/core/navigation_application.rs:2255 fn commit (assoc)
src/core/navigation_application.rs:2383 struct CommittedNavigationDomainRetirement
src/core/navigation_application.rs:2391 struct CommittedNavigationRootRetirement
src/core/navigation_application.rs:2397 struct PreparedStandaloneNavigationTransaction
src/core/navigation_application.rs:2405 fn staged_surfaces (assoc)
src/core/navigation_application.rs:2412 fn next_active_surface (assoc)
src/core/navigation_application.rs:2419 fn commit (assoc)
src/core/navigation_application.rs:2457 struct CommittedStandaloneNavigation
src/core/navigation_application.rs:2463 fn active_surface (assoc)
src/core/navigation_application.rs:2467 fn terminated_surfaces (assoc)
src/core/navigation_application.rs:2477 struct PreparedRoutedNavigationTransaction
src/core/navigation_application.rs:2490 fn staged_surfaces (assoc)
src/core/navigation_application.rs:2497 fn next_active_surface (assoc)
src/core/navigation_application.rs:2504 fn commit (assoc)
src/core/navigation_application.rs:2572 struct PreparedRoutedNavigationOverlayExitTransaction
src/core/navigation_application.rs:2585 fn staged_surfaces (assoc)
src/core/navigation_application.rs:2592 fn next_active_surface (assoc)
src/core/navigation_application.rs:2599 fn commit (assoc)
src/core/navigation_application.rs:2669 struct PreparedRoutedNavigationOverlayFocusGraphTransaction
src/core/navigation_application.rs:2682 fn staged_surfaces (assoc)
src/core/navigation_application.rs:2689 fn next_active_surface (assoc)
src/core/navigation_application.rs:2696 fn commit (assoc)
src/core/navigation_application.rs:2775 struct CommittedRoutedNavigation
src/core/navigation_application.rs:2781 fn active_surface (assoc)
src/core/navigation_application.rs:2785 fn terminated_surfaces (assoc)
src/core/navigation_application.rs:2795 struct PreparedRoutedNavigationFocusGraphTransaction
src/core/navigation_application.rs:2808 fn staged_surfaces (assoc)
src/core/navigation_application.rs:2815 fn next_active_surface (assoc)
src/core/navigation_application.rs:2822 fn commit (assoc)
src/core/navigation_application.rs:2898 struct PreparedNavigationRootAdmission
src/core/navigation_application.rs:2910 fn root_id (assoc)
src/core/navigation_application.rs:2917 fn prepared_surface (assoc)
src/core/navigation_application.rs:2924 fn prepared_surface_visibility (assoc)
src/core/navigation_application.rs:2931 fn commit (assoc)
src/core/navigation_application.rs:3004 struct CommittedNavigationRootAdmission
src/core/navigation_domain_router.rs:16 struct NavigationRootFocusBindingPortId
src/core/navigation_domain_router.rs:19 struct NavigationRootFocusBindingGeneration
src/core/navigation_domain_router.rs:22 fn get (assoc)
src/core/navigation_domain_router.rs:32 struct NavigationRootFocusBindingCandidateIdentity
src/core/navigation_domain_router.rs:38 enum NavigationRootFocusBindingCandidate
src/core/navigation_domain_router.rs:44 struct NavigationRootFocusBindingSnapshot
src/core/navigation_domain_router.rs:53 struct NavigationRootFocusBindingReplacement
src/core/navigation_domain_router.rs:61 fn new (assoc)
src/core/navigation_domain_router.rs:77 enum NavigationRootFocusBindingError
src/core/navigation_domain_router.rs:150 struct NavigationRootFocusBindingPort
src/core/navigation_domain_router.rs:155 fn prepare_create_guarded (assoc)
src/core/navigation_domain_router.rs:210 fn commit_create_guarded (assoc)
src/core/navigation_domain_router.rs:244 fn port_id (assoc)
src/core/navigation_domain_router.rs:248 fn root_id (assoc)
src/core/navigation_domain_router.rs:252 fn interaction_domain_id (assoc)
src/core/navigation_domain_router.rs:256 fn candidate_identity (assoc)
src/core/navigation_domain_router.rs:269 fn snapshot_guarded (assoc)
src/core/navigation_domain_router.rs:311 fn prepare_replace_guarded (assoc)
src/core/navigation_domain_router.rs:352 fn commit_replace_guarded (assoc)
src/core/navigation_domain_router.rs:377 fn prepare_owned_domain_change_guarded (assoc)
src/core/navigation_domain_router.rs:482 fn commit_owned_domain_change_guarded (assoc)
src/core/navigation_domain_router.rs:511 fn prepare_retire_guarded (assoc)
src/core/navigation_domain_router.rs:573 fn commit_retire_guarded (assoc)
src/core/navigation_domain_router.rs:638 struct PreparedRootFocusBindingPortAdmission
src/core/navigation_domain_router.rs:650 fn testimony (assoc)
src/core/navigation_domain_router.rs:665 struct PreparedRootFocusBindingPortAdmissionTestimony
src/core/navigation_domain_router.rs:677 struct PreparedRootFocusBindingReplacement
src/core/navigation_domain_router.rs:691 fn testimony (assoc)
src/core/navigation_domain_router.rs:707 struct PreparedRootFocusBindingReplacementTestimony
src/core/navigation_domain_router.rs:728 struct PreparedRootOwnedFocusDomainChange
src/core/navigation_domain_router.rs:743 fn testimony (assoc)
src/core/navigation_domain_router.rs:769 struct PreparedRootOwnedFocusDomainChangeTestimony
src/core/navigation_domain_router.rs:784 struct PreparedRootFocusBindingRetirement
src/core/navigation_domain_router.rs:798 fn testimony (assoc)
src/core/navigation_domain_router.rs:823 struct PreparedRootFocusBindingRetirementTestimony
src/core/navigation_domain_router.rs:898 struct NavigationRoutingGeneration
src/core/navigation_domain_router.rs:901 fn get (assoc)
src/core/navigation_domain_router.rs:911 enum NavigationRoutingSnapshotPhase
src/core/navigation_domain_router.rs:917 struct DelegationFrame
src/core/navigation_domain_router.rs:1002 struct NavigationDomainRouter
src/core/navigation_domain_router.rs:1007 enum NavigationDomainRouterError
src/core/navigation_domain_router.rs:1036 struct PreparedRootAssociation
src/core/navigation_domain_router.rs:1056 struct PreparedRoutedNavigationRouterCommit
src/core/navigation_domain_router.rs:1074 struct PreparedDelegationPush
src/core/navigation_domain_router.rs:1090 fn frame (assoc)
src/core/navigation_domain_router.rs:1094 fn source_owner_id (assoc)
src/core/navigation_domain_router.rs:1098 fn target_owner_id (assoc)
src/core/navigation_domain_router.rs:1102 fn destination_focus_binding (assoc)
src/core/navigation_domain_router.rs:1120 struct PreparedDelegationReturn
src/core/navigation_domain_router.rs:1136 fn frame (assoc)
src/core/navigation_domain_router.rs:1140 fn source_owner_id (assoc)
src/core/navigation_domain_router.rs:1144 fn target_owner_id (assoc)
src/core/navigation_domain_router.rs:1148 fn destination_focus_binding (assoc)
src/core/navigation_domain_router.rs:1190 struct PreparedRootFocusMembershipChange
src/core/navigation_domain_router.rs:1212 struct PreparedRootRetirement
src/core/navigation_domain_router.rs:1270 fn overlay_owner_id (assoc)
src/core/navigation_domain_router.rs:1274 fn unwound_delegation_frames_top_first (assoc)
src/core/navigation_domain_router.rs:1332 struct RoutedNavigationSnapshot
src/core/navigation_domain_router.rs:1342 struct NavigationRootRetirementSnapshot
src/core/navigation_domain_router.rs:1369 struct NavigationRootDelegationRetirementPlan
src/core/navigation_domain_router.rs:1383 fn unwound_frames_top_first (assoc)
src/core/navigation_domain_router.rs:1387 fn current_effective_root (assoc)
src/core/navigation_domain_router.rs:1391 fn final_effective_root (assoc)
src/core/navigation_domain_router.rs:1395 fn requires_delegation_unwind (assoc)
src/core/navigation_domain_router.rs:1401 fn interaction_domain_id (assoc)
src/core/navigation_domain_router.rs:1405 fn root_id (assoc)
src/core/navigation_domain_router.rs:1409 fn overlay_owner_id (assoc)
src/core/navigation_domain_router.rs:1413 fn focus_binding_port (assoc)
src/core/navigation_domain_router.rs:1417 fn focus_binding (assoc)
src/core/navigation_domain_router.rs:1421 fn router_generation (assoc)
src/core/navigation_domain_router.rs:1425 fn is_primed (assoc)
src/core/navigation_domain_router.rs:1431 enum RoutedNavigationCandidate
src/core/navigation_domain_router.rs:1438 fn new (assoc)
src/core/navigation_domain_router.rs:1447 fn register_domain (assoc)
src/core/navigation_domain_router.rs:1475 fn validate_root_admission_absence_guarded (assoc)
src/core/navigation_domain_router.rs:1516 fn route_snapshot (assoc)
src/core/navigation_domain_router.rs:1592 fn validate_projection_root_guarded (assoc)
src/core/navigation_domain_router.rs:1631 fn root_retirement_snapshot_guarded (assoc)
src/core/navigation_domain_router.rs:1718 fn root_delegation_retirement_plan_guarded (assoc)
src/core/navigation_domain_router.rs:1841 fn prepare_routed_navigation_guarded (assoc)
src/core/navigation_domain_router.rs:1910 fn begin_routed_navigation_commit_guarded (assoc)
src/core/navigation_domain_router.rs:1932 fn finish_routed_navigation_commit_guarded (assoc)
src/core/navigation_domain_router.rs:1974 fn prepare_delegation_guarded (assoc)
src/core/navigation_domain_router.rs:2054 fn begin_delegation_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2077 fn finish_delegation_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2134 fn prepare_delegation_return_guarded (assoc)
src/core/navigation_domain_router.rs:2198 fn begin_delegation_return_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2221 fn finish_delegation_return_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2278 fn overlay_activation_eligibility_guarded (assoc)
src/core/navigation_domain_router.rs:2324 fn prime_root_guarded (assoc)
src/core/navigation_domain_router.rs:2369 fn prepare_root_association_guarded (assoc)
src/core/navigation_domain_router.rs:2464 fn begin_root_association_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2486 fn apply_root_association_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2576 fn finish_root_association_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2602 fn prepare_root_focus_membership_change_guarded (assoc)
src/core/navigation_domain_router.rs:2701 fn begin_root_focus_membership_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2723 fn apply_root_focus_membership_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2800 fn finish_root_focus_membership_commit_guarded (assoc)
src/core/navigation_domain_router.rs:2826 fn prepare_root_retirement_guarded (assoc)
src/core/navigation_domain_router.rs:3079 fn begin_root_retirement_commit_guarded (assoc)
src/core/navigation_domain_router.rs:3101 fn apply_root_retirement_claims_guarded (assoc)
src/core/navigation_domain_router.rs:3153 fn apply_root_routing_retirement_guarded (assoc)
src/core/navigation_domain_router.rs:3183 fn remove_root_association_guarded (assoc)
src/core/navigation_domain_router.rs:3211 fn finish_root_retirement_commit_guarded (assoc)
src/core/navigation_orchestrator.rs:35 fn new (assoc)
src/core/navigation_orchestrator.rs:152 enum NavigationProjectionError
src/core/navigation_orchestrator.rs:164 fn get (assoc)
src/core/navigation_orchestrator.rs:208 enum NavigationSurfacePreparationError
src/core/navigation_orchestrator.rs:311 struct NavigationSurfaceOrchestratorHandle
src/core/navigation_orchestrator.rs:338 fn compositor_identity (assoc)
src/core/navigation_orchestrator.rs:393 fn project (assoc)
src/core/navigation_orchestrator.rs:437 enum FrameProjectionParticipationError
src/core/navigation_orchestrator.rs:525 fn projection_handle (assoc)
src/core/navigation_orchestrator.rs:534 fn compositor_identity (assoc)
src/core/navigation_orchestrator.rs:896 fn orchestration_generation (assoc)
src/core/navigation_orchestrator.rs:900 fn surface_lifecycle (assoc)
src/core/navigation_orchestrator.rs:929 fn try_open_overlay (assoc)
src/core/navigation_orchestrator.rs:977 fn try_close_overlay (assoc)
src/core/navigation_orchestrator.rs:1032 fn active_overlay_kind (assoc)
src/core/navigation_orchestrator.rs:1040 fn overlay_host_surface (assoc)
src/core/navigation_orchestrator.rs:1581 fn publication_binding (assoc)
src/core/navigation_orchestrator.rs:1588 fn staged_surfaces (assoc)
src/core/navigation_orchestrator.rs:1592 fn next_active_surface (assoc)
src/core/navigation_orchestrator.rs:1597 fn staged_surface_visibility (assoc)
src/core/navigation_orchestrator.rs:1606 fn terminating_surfaces (assoc)
src/core/navigation_orchestrator.rs:1610 fn step_transition (assoc)
src/core/navigation_orchestrator.rs:1614 fn abort (assoc)
src/core/navigation_orchestrator.rs:1788 enum NavigationRootPreparationError
src/core/navigation_orchestrator.rs:1824 struct RootAdmissionPublicationBinding
src/core/navigation_orchestrator.rs:1842 struct RootSurfaceTeardownPublicationBinding
src/core/navigation_orchestrator.rs:1908 fn root_id (assoc)
src/core/navigation_orchestrator.rs:1912 fn current_entry (assoc)
src/core/navigation_orchestrator.rs:1920 fn current_surface (assoc)
src/core/navigation_orchestrator.rs:1932 fn current_surface_visibility (assoc)
src/core/navigation_orchestrator.rs:1973 fn publication_binding (assoc)
src/core/navigation_orchestrator.rs:1981 fn abort (assoc)
src/core/navigation_orchestrator.rs:2160 fn publication_binding (assoc)
src/core/navigation_orchestrator.rs:2173 fn terminating_surfaces (assoc)
src/core/navigation_orchestrator.rs:2177 fn abort (assoc)
src/core/navigation_orchestrator.rs:2275 fn new (assoc)
src/core/navigation_orchestrator.rs:2284 fn compositor_identity (assoc)
src/core/navigation_orchestrator.rs:2306 fn prepare_create_root (assoc)
src/core/navigation_orchestrator.rs:2397 fn prepare_root_surface_teardown (assoc)
src/core/overlay_domain_coordinator.rs:16 struct OverlayOwnerId
src/core/overlay_domain_coordinator.rs:19 struct OverlayStackId
src/core/overlay_domain_coordinator.rs:22 struct OverlayFrameId
src/core/overlay_domain_coordinator.rs:25 enum OverlayActivationAdmission
src/core/overlay_domain_coordinator.rs:31 enum OverlayDisposition
src/core/overlay_domain_coordinator.rs:37 enum OverlayOwnerLifecycleState
src/core/overlay_domain_coordinator.rs:43 struct OverlayFrameDescriptor
src/core/overlay_domain_coordinator.rs:50 fn new (assoc)
src/core/overlay_domain_coordinator.rs:64 struct OverlayStackDescriptor
src/core/overlay_domain_coordinator.rs:70 fn new (assoc)
src/core/overlay_domain_coordinator.rs:79 struct OverlayProjection
src/core/overlay_domain_coordinator.rs:86 struct ActiveOverlayStackBinding
src/core/overlay_domain_coordinator.rs:120 enum OverlayDomainError
src/core/overlay_domain_coordinator.rs:199 struct OverlayOwnerLifecyclePort
src/core/overlay_domain_coordinator.rs:206 fn new (assoc)
src/core/overlay_domain_coordinator.rs:251 fn owner_id (assoc)
src/core/overlay_domain_coordinator.rs:255 fn interaction_domain_id (assoc)
src/core/overlay_domain_coordinator.rs:259 fn lifecycle_state (assoc)
src/core/overlay_domain_coordinator.rs:925 struct OverlayDomainCoordinator
src/core/overlay_domain_coordinator.rs:930 fn new (assoc)
src/core/overlay_domain_coordinator.rs:954 fn prepare_domain_registration (assoc)
src/core/overlay_domain_coordinator.rs:976 fn commit_domain_registration (assoc)
src/core/overlay_domain_coordinator.rs:1003 fn register_domain (assoc)
src/core/overlay_domain_coordinator.rs:1013 fn prepare_owner_registration (assoc)
src/core/overlay_domain_coordinator.rs:1054 fn commit_owner_registration (assoc)
src/core/overlay_domain_coordinator.rs:1080 fn register_owner (assoc)
src/core/overlay_domain_coordinator.rs:1145 fn prepare_owner_unregistration_after_focus_retirement_guarded (assoc)
src/core/overlay_domain_coordinator.rs:1285 fn unregister_owner (assoc)
src/core/overlay_domain_coordinator.rs:1445 fn begin_domain_transition (assoc)
src/core/overlay_domain_coordinator.rs:1477 fn prepare_activation (assoc)
src/core/overlay_domain_coordinator.rs:1540 fn commit_activation (assoc)
src/core/overlay_domain_coordinator.rs:1580 fn activate_owner (assoc)
src/core/overlay_domain_coordinator.rs:1600 fn prepare_nested_frame_push (assoc)
src/core/overlay_domain_coordinator.rs:1680 fn commit_nested_frame_push (assoc)
src/core/overlay_domain_coordinator.rs:1697 fn push_nested_frame (assoc)
src/core/overlay_domain_coordinator.rs:1717 fn prepare_nested_frame_pop (assoc)
src/core/overlay_domain_coordinator.rs:1782 fn commit_nested_frame_pop (assoc)
src/core/overlay_domain_coordinator.rs:1798 fn pop_nested_frame (assoc)
src/core/overlay_domain_coordinator.rs:1816 fn prepare_deactivation (assoc)
src/core/overlay_domain_coordinator.rs:1872 fn commit_deactivation (assoc)
src/core/overlay_domain_coordinator.rs:1905 fn deactivate_owner (assoc)
src/core/overlay_domain_coordinator.rs:1919 fn prepare_deactivation_to_domain (assoc)
src/core/overlay_domain_coordinator.rs:1977 fn commit_deactivation_to_domain (assoc)
src/core/overlay_domain_coordinator.rs:2011 fn prepare_deactivate_for_focus_graph_mutation_guarded (assoc)
src/core/overlay_domain_coordinator.rs:2108 fn prepare_deactivate_for_focus_retirement_guarded (assoc)
src/core/overlay_domain_coordinator.rs:2252 fn commit_deactivate_for_focus_graph_mutation_guarded (assoc)
src/core/overlay_domain_coordinator.rs:2285 fn commit_deactivate_for_focus_retirement_guarded (assoc)
src/core/overlay_domain_coordinator.rs:2293 fn deactivate_owner_to_domain (assoc)
src/core/overlay_domain_coordinator.rs:2313 fn active_binding_guarded (assoc)
src/core/overlay_domain_coordinator.rs:2341 fn active_projection_guarded (assoc)
src/core/overlay_domain_coordinator.rs:2362 fn owner_registration_generation (assoc)
src/core/overlay_domain_coordinator.rs:2523 struct DomainTransitionGuardToken
src/core/overlay_domain_coordinator.rs:2532 fn interaction_domain_id (assoc)
src/core/overlay_domain_coordinator.rs:2536 fn generation (assoc)
src/core/overlay_domain_coordinator.rs:2559 struct RouterEligibilityTestimony
src/core/overlay_domain_coordinator.rs:2626 fn router_generation (assoc)
src/core/overlay_domain_coordinator.rs:2630 fn root_binding_generation (assoc)
src/core/overlay_domain_coordinator.rs:2636 struct PreparedOverlayDomainRegistration
src/core/overlay_domain_coordinator.rs:2655 struct PreparedOverlayOwnerRegistration
src/core/overlay_domain_coordinator.rs:2664 fn owner_id (assoc)
src/core/overlay_domain_coordinator.rs:2668 fn interaction_domain_id (assoc)
src/core/overlay_domain_coordinator.rs:2689 struct PreparedOverlayOwnerUnregistration
src/core/overlay_domain_coordinator.rs:2715 struct PreparedOverlayDomainUnregistration
src/core/overlay_domain_coordinator.rs:2736 struct PreparedOverlayActivation
src/core/overlay_domain_coordinator.rs:2746 struct PreparedOverlayDeactivation
src/core/overlay_domain_coordinator.rs:2757 struct PreparedOverlayDeactivationToDomain
src/core/overlay_domain_coordinator.rs:2779 struct PreparedOverlayFocusGraphTransition
src/core/overlay_domain_coordinator.rs:2819 fn focus_graph_testimony (assoc)
src/core/overlay_domain_coordinator.rs:2834 struct PreparedOverlayNestedFramePush
src/core/overlay_domain_coordinator.rs:2845 struct PreparedOverlayNestedFramePop
src/core/paragraph.rs:71 fn new (assoc)
src/core/paragraph.rs:75 fn name (assoc)
src/core/paragraph.rs:131 fn new (assoc)
src/core/paragraph.rs:135 fn with_size (assoc)
src/core/paragraph.rs:140 fn with_color (assoc)
src/core/paragraph.rs:145 fn with_weight (assoc)
src/core/paragraph.rs:150 fn with_italic (assoc)
src/core/paragraph.rs:155 fn with_family (assoc)
src/core/paragraph.rs:182 fn new (assoc)
src/core/scene/mod.rs:1114 fn new (assoc)
src/core/scene/mod.rs:1129 fn clear (assoc)
src/core/scene/mod.rs:1255 fn new (assoc)
src/core/scene/mod.rs:1597 fn push_rect (assoc)
src/core/scene/mod.rs:1647 fn push_rounded_rect (assoc)
src/core/scene/mod.rs:1835 fn push_image (assoc)
src/core/scene/mod.rs:1857 fn push_clip (assoc)
src/core/scene/mod.rs:1876 fn push_layer (assoc)
src/core/scroll_container.rs:467 fn scroll_offset (assoc)
src/core/scroll_state.rs:99 fn new (assoc)
src/core/scroll_state.rs:112 fn new (assoc)
src/core/scroll_state.rs:135 fn new (assoc)
src/core/scroll_state.rs:1154 fn new (assoc)
src/core/scroll_state.rs:1250 fn new (assoc)
src/core/series.rs:5 struct Series
src/core/series.rs:31 fn new (assoc)
src/core/series.rs:36 fn is_empty (assoc)
src/core/series.rs:41 fn len (assoc)
src/core/series.rs:46 fn capacity (assoc)
src/core/series.rs:51 fn push (assoc)
src/core/series.rs:56 fn head (assoc)
src/core/series.rs:61 fn head_mut (assoc)
src/core/series.rs:66 fn tail (assoc)
src/core/series.rs:71 fn get (assoc)
src/core/series.rs:76 fn get_mut (assoc)
src/core/series.rs:81 fn iter (assoc)
src/core/series.rs:86 fn iter_mut (assoc)
src/core/series.rs:91 fn iter_rev (assoc)
src/core/series.rs:96 fn iter_rev_mut (assoc)
src/core/sliding_window.rs:1 struct SlidingWindowIter
src/core/sliding_window.rs:31 struct SlidingWindowIterRev
src/core/sliding_window.rs:64 struct SlidingWindow
src/core/sliding_window.rs:75 fn new (assoc)
src/core/sliding_window.rs:84 fn is_empty (assoc)
src/core/sliding_window.rs:88 fn capacity (assoc)
src/core/sliding_window.rs:92 fn len (assoc)
src/core/sliding_window.rs:97 fn push (assoc)
src/core/sliding_window.rs:111 fn pop (assoc)
src/core/sliding_window.rs:122 fn head (assoc)
src/core/sliding_window.rs:132 fn head_mut (assoc)
src/core/sliding_window.rs:142 fn prev (assoc)
src/core/sliding_window.rs:152 fn tail (assoc)
src/core/sliding_window.rs:161 fn get (assoc)
src/core/sliding_window.rs:180 fn get_rev_mut (assoc)
src/core/sliding_window.rs:189 fn iter (assoc)
src/core/sliding_window.rs:203 fn iter_mut (assoc)
src/core/sliding_window.rs:225 fn iter_rev (assoc)
src/core/sliding_window.rs:239 fn iter_rev_mut (assoc)
src/core/style.rs:14 const FOCUS_RING_SOFTNESS
src/core/style.rs:15 const FOCUS_RING_DEFAULT_Z
src/core/style.rs:250 fn new (assoc)
src/core/text_engine.rs:94 fn text_top_y_in_rect
src/core/text_engine.rs:135 fn compute_control_text_top_y
src/core/text_linebreaker.rs:14 use crate
src/core/theme.rs:194 struct ColorPickerGeometry
src/core/theme.rs:1025 struct GaugeColors
src/core/theme.rs:1489 fn for_density (assoc)
src/core/transition/mod.rs:26 use transition_layer
src/core/widget/mod.rs:65 fn new (assoc)
src/core/widget/mod.rs:79 fn new_with_signals (assoc)
src/core/widget/mod.rs:595 fn new (assoc)
src/core/widget/mod.rs:798 fn register_tooltip_at (assoc)
src/core/widget/mod.rs:1304 fn new (assoc)
src/core/widget/mod.rs:1453 fn focus_redirect (assoc)
src/core/widget/mod.rs:1465 fn is_keyboard_capture_requested (assoc)
src/core/widget/mod.rs:1539 fn is_keyboard_capture_release_requested (assoc)
src/core/widget/mod.rs:1942 fn new (assoc)
src/core/widget_tree.rs:150 fn new (assoc)
src/core/widget_tree.rs:255 fn new (assoc)
src/core/widget_tree.rs:1261 fn verify_all_nodes_consistent (assoc)
src/editing/mod.rs:3 use editor_bridge
src/editing/editor_bridge.rs:31 fn new (assoc)
src/editing/editor_bridge.rs:54 fn editor (assoc)
src/editing/editor_bridge.rs:58 fn editor_mut (assoc)
src/editing/editor_bridge.rs:62 fn row_id (assoc)
src/editing/editor_bridge.rs:66 fn column_id (assoc)
src/engines/cell_mechanics/mod.rs:845 fn reconcile_always_mounted_editors_with (assoc)
src/engines/cell_source/mod.rs:202 fn new (assoc)
src/engines/cell_source/mod.rs:381 struct ComboBoxCellEditorSpec
src/engines/hierarchical/mod.rs:30 use topology
src/engines/route_geometry/mod.rs:42 fn opposite (assoc)
src/engines/temporal/mod.rs:18 use containment
src/engines/temporal/mod.rs:19 use descriptors
src/engines/temporal/mod.rs:24 use emphasis
src/engines/temporal/mod.rs:25 use hts_rows
src/engines/temporal/mod.rs:26 use interval_projection
src/engines/temporal/mod.rs:27 use lane_layout
src/engines/temporal/mod.rs:28 use substrate
src/engines/temporal/mod.rs:29 use time_geometry
src/engines/temporal/descriptors.rs:32 enum IntervalStyle
src/engines/temporal/descriptors.rs:42 enum IntervalEdge
src/engines/temporal/descriptors.rs:48 enum MarkerStyle
src/engines/temporal/descriptors.rs:55 struct IntervalDescriptor
src/engines/temporal/descriptors.rs:81 struct ArrowDescriptor
src/engines/temporal/descriptors.rs:109 fn clear (assoc)
src/engines/temporal/descriptors.rs:118 enum TemporalInvalidationEvent
src/engines/temporal/emphasis.rs:60 fn new (assoc)
src/engines/temporal/interval_projection.rs:43 fn contains (assoc)
src/engines/temporal/interval_projection.rs:47 fn interval (assoc)
src/engines/temporal/interval_projection.rs:67 struct IntervalProjection
src/engines/temporal/interval_projection.rs:74 fn new (assoc)
src/engines/temporal/interval_projection.rs:169 fn snapshot (assoc)
src/engines/temporal/lane_layout.rs:32 enum LanePolicy
src/engines/temporal/lane_layout.rs:43 struct LaneLayoutEngine
src/engines/temporal/lane_layout.rs:53 fn new (assoc)
src/engines/temporal/lane_layout.rs:129 fn lanes (assoc)
src/engines/temporal/substrate.rs:116 struct TemporalSubstrate
src/engines/temporal/substrate.rs:136 fn new (assoc)
src/engines/temporal/substrate.rs:223 fn geometry (assoc)
src/engines/temporal/substrate.rs:239 fn snapshot (assoc)
src/engines/temporal/substrate.rs:243 fn lanes (assoc)
src/engines/temporal/substrate.rs:247 fn lane_policy (assoc)
src/engines/temporal/substrate.rs:251 fn set_lane_policy (assoc)
src/engines/temporal/substrate.rs:258 fn selection (assoc)
src/engines/temporal/substrate.rs:262 fn selection_mut (assoc)
src/engines/temporal/substrate.rs:324 fn has_pending_intents (assoc)
src/engines/temporal/substrate.rs:335 fn time_context (assoc)
src/engines/temporal/time_geometry.rs:52 enum TimeScale
src/engines/temporal/time_geometry.rs:82 struct WorkingCalendar
src/engines/temporal/time_geometry.rs:102 fn is_working_day (assoc)
src/engines/temporal/time_geometry.rs:160 struct HeaderTier
src/engines/temporal/time_geometry.rs:212 struct TimeGeometryEngine
src/engines/temporal/time_geometry.rs:226 fn new (assoc)
src/engines/temporal/time_geometry.rs:242 fn set_frame (assoc)
src/engines/temporal/time_geometry.rs:246 fn set_calendar (assoc)
src/engines/temporal/time_geometry.rs:255 fn set_viewport_width (assoc)
src/engines/temporal/time_geometry.rs:259 fn set_zoom_limits (assoc)
src/engines/temporal/time_geometry.rs:266 fn anchor (assoc)
src/engines/temporal/time_geometry.rs:270 fn scale (assoc)
src/engines/temporal/time_geometry.rs:274 fn px_per_unit (assoc)
src/engines/temporal/time_geometry.rs:278 fn frame (assoc)
src/engines/temporal/time_geometry.rs:282 fn viewport_width (assoc)
src/engines/temporal/time_geometry.rs:286 fn calendar (assoc)
src/engines/temporal/time_geometry.rs:306 fn visible_range (assoc)
src/engines/temporal/time_geometry.rs:318 fn scroll_to (assoc)
src/model/mod.rs:39 use toast
src/model/canvas.rs:5 const DEFAULT_MIN_ZOOM
src/model/canvas.rs:6 const DEFAULT_MAX_ZOOM
src/model/canvas.rs:7 const DEFAULT_ZOOM_TO_FIT_PADDING
src/model/canvas.rs:10 struct WorldPoint
src/model/canvas.rs:16 struct WorldRect
src/model/canvas.rs:24 struct CanvasTransform
src/model/canvas.rs:31 struct CanvasViewportState
src/model/canvas.rs:36 struct CanvasGrid
src/model/canvas.rs:46 enum CanvasBackground
src/model/canvas.rs:54 struct InfiniteCanvas
src/model/canvas.rs:64 enum CanvasEntityId
src/model/canvas.rs:133 struct ProjectedCanvasItem
src/model/canvas.rs:142 enum CanvasHitRegion
src/model/canvas.rs:163 enum HitPriority
src/model/canvas.rs:173 enum CanvasPaintDescriptor
src/model/canvas.rs:183 struct ProjectionRevision
src/model/canvas.rs:186 struct CanvasProjection
src/model/canvas.rs:192 struct CanvasHit
src/model/canvas.rs:199 enum CanvasIntent
src/model/canvas.rs:209 fn new (assoc)
src/model/canvas.rs:215 fn new (assoc)
src/model/canvas.rs:224 fn contains (assoc)
src/model/canvas.rs:228 fn intersects (assoc)
src/model/canvas.rs:235 fn union (assoc)
src/model/canvas.rs:243 fn center (assoc)
src/model/canvas.rs:249 fn identity (assoc)
src/model/canvas.rs:257 fn world_to_screen (assoc)
src/model/canvas.rs:264 fn screen_to_world (assoc)
src/model/canvas.rs:300 fn new (assoc)
src/model/canvas.rs:311 fn with_grid (assoc)
src/model/canvas.rs:316 fn with_background (assoc)
src/model/canvas.rs:321 fn with_zoom_range (assoc)
src/model/canvas.rs:329 fn with_viewport_size (assoc)
src/model/canvas.rs:334 fn grid (assoc)
src/model/canvas.rs:338 fn background (assoc)
src/model/canvas.rs:342 fn pan_to (assoc)
src/model/canvas.rs:348 fn zoom_to (assoc)
src/model/canvas.rs:353 fn zoom_around_screen_point (assoc)
src/model/canvas.rs:368 fn zoom_to_fit (assoc)
src/model/canvas.rs:398 fn reset_view (assoc)
src/model/canvas.rs:403 fn world_to_screen (assoc)
src/model/canvas.rs:407 fn screen_to_world (assoc)
src/model/canvas.rs:416 fn viewport_center (assoc)
src/model/canvas.rs:423 fn hit_test (assoc)
src/model/canvas.rs:432 fn emit_empty_click (assoc)
src/model/canvas.rs:436 fn emit_empty_context_menu (assoc)
src/model/canvas.rs:440 fn emit_rubber_band_changed (assoc)
src/model/canvas.rs:444 fn emit_rubber_band_committed (assoc)
src/model/canvas.rs:448 fn take_intents (assoc)
src/model/canvas.rs:452 fn adaptive_grid_spacing (assoc)
src/model/canvas.rs:481 fn new (assoc)
src/model/canvas.rs:485 fn revision (assoc)
src/model/canvas.rs:489 fn iter (assoc)
src/model/canvas.rs:493 fn in_draw_order (assoc)
src/model/canvas.rs:500 fn in_region (assoc)
src/model/canvas.rs:507 fn hit_test (assoc)
src/model/canvas.rs:531 fn world_bounds (assoc)
src/model/collapsible_section.rs:15 fn new (assoc)
src/model/collapsible_section.rs:19 fn is_expanded (assoc)
src/model/collapsible_section.rs:23 fn set_expanded (assoc)
src/model/collapsible_section.rs:27 fn toggle (assoc)
src/model/context_menu.rs:71 fn is_open (assoc)
src/model/context_menu.rs:75 fn items (assoc)
src/model/context_menu.rs:79 fn anchor (assoc)
src/model/context_menu.rs:83 fn surface (assoc)
src/model/context_menu.rs:87 fn surface_mut (assoc)
src/model/dialog.rs:24 fn new (assoc)
src/model/dialog.rs:33 fn is_open (assoc)
src/model/dialog.rs:37 fn pressed_key (assoc)
src/model/dropdown_menu.rs:52 fn is_open (assoc)
src/model/dropdown_menu.rs:56 fn items (assoc)
src/model/dropdown_menu.rs:60 fn anchor_rect (assoc)
src/model/dropdown_menu.rs:64 fn surface (assoc)
src/model/dropdown_menu.rs:68 fn surface_mut (assoc)
src/model/menu_model.rs:45 fn new (assoc)
src/model/menu_model.rs:151 fn titles (assoc)
src/model/menu_model.rs:155 fn item_state (assoc)
src/model/menu_model.rs:159 fn item_label (assoc)
src/model/menu_model.rs:168 fn bound_action (assoc)
src/model/menu_surface.rs:29 fn new (assoc)
src/model/menu_surface.rs:97 fn is_open (assoc)
src/model/menu_surface.rs:101 fn items (assoc)
src/model/menu_surface.rs:105 fn highlighted_index (assoc)
src/model/navigation_host.rs:24 fn new (assoc)
src/model/navigation_host.rs:32 fn with_restoration_key (assoc)
src/model/navigation_host.rs:37 fn with_parameter (assoc)
src/model/navigation_host.rs:50 fn node_id (assoc)
src/model/navigation_host.rs:54 fn restoration_key (assoc)
src/model/navigation_host.rs:58 fn parameters (assoc)
src/model/navigation_host.rs:94 fn current (assoc)
src/model/navigation_host.rs:100 fn path (assoc)
src/model/navigation_host.rs:110 fn get (assoc)
src/model/navigation_host.rs:124 fn new (assoc)
src/model/navigation_host.rs:128 fn get (assoc)
src/model/navigation_host.rs:184 enum NavigationSnapshotError
src/model/navigation_host.rs:189 enum NavigationPreparationError
src/model/navigation_host.rs:200 enum NavigationExecutionError
src/model/navigation_host.rs:423 struct NavigationMechanicsSnapshot
src/model/navigation_host.rs:431 fn generation (assoc)
src/model/navigation_host.rs:435 fn state (assoc)
src/model/navigation_host.rs:439 fn history_len (assoc)
src/model/navigation_host.rs:443 fn history_cursor (assoc)
src/model/navigation_host.rs:452 struct ResolvedNavigationCandidate
src/model/navigation_host.rs:465 fn mechanics_generation (assoc)
src/model/navigation_host.rs:469 fn requested_intent (assoc)
src/model/navigation_host.rs:473 fn evaluated_intent (assoc)
src/model/navigation_host.rs:477 fn from (assoc)
src/model/navigation_host.rs:481 fn to (assoc)
src/model/navigation_host.rs:487 enum NavigationResolution
src/model/navigation_host.rs:495 struct NavigationGuardAuthorization
src/model/navigation_host.rs:502 fn candidate (assoc)
src/model/navigation_host.rs:508 enum NavigationAuthorizationResult
src/model/navigation_host.rs:540 fn new (assoc)
src/model/navigation_host.rs:565 fn set_guard (assoc)
src/model/navigation_host.rs:569 fn clear_guard (assoc)
src/model/navigation_host.rs:573 fn snapshot (assoc)
src/model/navigation_host.rs:583 fn resolve (assoc)
src/model/navigation_host.rs:594 fn authorize (assoc)
src/model/navigation_host.rs:611 fn authorize_candidate (assoc)
src/model/navigation_host.rs:680 fn prepare_commit (assoc)
src/model/navigation_host.rs:774 fn execute (assoc)
src/model/navigation_host.rs:846 fn state (assoc)
src/model/navigation_host.rs:850 fn current_entry (assoc)
src/model/navigation_host.rs:854 fn current (assoc)
src/model/navigation_host.rs:858 fn path_entries (assoc)
src/model/navigation_host.rs:862 fn path (assoc)
src/model/navigation_host.rs:872 fn control_projection (assoc)
src/model/navigation_host.rs:997 fn from (assoc)
src/model/navigation_host.rs:1001 fn to (assoc)
src/model/navigation_host.rs:1005 fn expected_generation (assoc)
src/model/navigation_host.rs:1009 fn commit (assoc)
src/model/navigation_host.rs:1029 fn abort (assoc)
src/model/node_editor.rs:24 const DEFAULT_FRAME_PADDING
src/model/node_editor.rs:27 struct SessionRevision
src/model/node_editor.rs:30 struct NodeEditorSession
src/model/node_editor.rs:45 enum NodeEditorTool
src/model/node_editor.rs:53 struct SelectionSet
src/model/node_editor.rs:59 struct DragSession
src/model/node_editor.rs:68 struct AlignmentSnapConfig
src/model/node_editor.rs:93 enum AlignmentGuideAxis
src/model/node_editor.rs:99 enum AlignmentGuideRole
src/model/node_editor.rs:105 struct AlignmentGuide
src/model/node_editor.rs:133 fn spatial_guides_from_alignment_guides
src/model/node_editor.rs:138 struct AlignmentSnapResult
src/model/node_editor.rs:144 struct FrameDragCapture
src/model/node_editor.rs:150 struct FrameResizeSession
src/model/node_editor.rs:157 struct RubberBandSession
src/model/node_editor.rs:164 struct InlineEditSession
src/model/node_editor.rs:170 struct NodeClipboard
src/model/node_editor.rs:175 struct PortEndpoint
src/model/node_editor.rs:182 struct WireDragSession
src/model/node_editor.rs:189 enum WireDragOrigin
src/model/node_editor.rs:200 struct DanglingWireContext
src/model/node_editor.rs:206 struct NodeSearchEntry
src/model/node_editor.rs:218 struct NodeSearchMetadata
src/model/node_editor.rs:225 struct NodeSearchSession
src/model/node_editor.rs:233 enum SearchCursorDelta
src/model/node_editor.rs:239 enum SelectionIntent
src/model/node_editor.rs:267 enum NodeSearchIntent
src/model/node_editor.rs:287 enum SessionIntent
src/model/node_editor.rs:306 enum NodeEditorIntent
src/model/node_editor.rs:315 struct NodeContextMenuRequest
src/model/node_editor.rs:321 enum NodeSearchCommit
src/model/node_editor.rs:332 struct NodeFlowConfig
src/model/node_editor.rs:357 struct NodeFlowCoordinator
src/model/node_editor.rs:368 fn new (assoc)
src/model/node_editor.rs:377 fn with_config (assoc)
src/model/node_editor.rs:383 fn with_viewport_size (assoc)
src/model/node_editor.rs:388 fn config (assoc)
src/model/node_editor.rs:392 fn canvas (assoc)
src/model/node_editor.rs:396 fn build_node_graph_projection (assoc)
src/model/node_editor.rs:410 fn build_projection (assoc)
src/model/node_editor.rs:417 fn emit_intent (assoc)
src/model/node_editor.rs:426 fn take_intents (assoc)
src/model/node_editor.rs:430 fn handle_event (assoc)
src/model/node_editor.rs:461 fn zoom_to_fit_intent (assoc)
src/model/node_editor.rs:472 fn zoom_to_node_intent (assoc)
src/model/node_editor.rs:486 fn pan_to_intent (assoc)
src/model/node_editor.rs:493 fn reset_view_intent (assoc)
src/model/node_editor.rs:497 fn zoom_in_intent (assoc)
src/model/node_editor.rs:501 fn zoom_out_intent (assoc)
src/model/node_editor.rs:505 fn action_intents (assoc)
src/model/node_editor.rs:689 struct NodeFlowInputs
src/model/node_editor.rs:698 struct ExecutionOverlay
src/model/node_editor.rs:704 enum NodeEditorSessionError
src/model/node_editor.rs:726 enum NodeFlowIntentStatus
src/model/node_editor.rs:732 enum NodeFlowBuildError
src/model/node_editor.rs:737 struct NodeFlowActionContext
src/model/node_editor.rs:760 fn node_flow_default_bindings
src/model/node_editor.rs:870 fn new (assoc)
src/model/node_editor.rs:874 fn apply_selection_intent (assoc)
src/model/node_editor.rs:928 fn begin_frame_drag (assoc)
src/model/node_editor.rs:976 fn begin_frame_resize (assoc)
src/model/node_editor.rs:991 fn update_frame_resize (assoc)
src/model/node_editor.rs:1004 fn cancel_frame_resize (assoc)
src/model/node_editor.rs:1010 fn commit_frame_resize (assoc)
src/model/node_editor.rs:1020 fn begin_rubber_band (assoc)
src/model/node_editor.rs:1029 fn update_rubber_band (assoc)
src/model/node_editor.rs:1036 fn begin_wire_drag (assoc)
src/model/node_editor.rs:1080 fn update_wire_drag (assoc)
src/model/node_editor.rs:1094 fn commit_wire_drag (assoc)
src/model/node_editor.rs:1153 fn cancel_wire_drag (assoc)
src/model/node_editor.rs:1159 fn open_search (assoc)
src/model/node_editor.rs:1173 fn update_search_query (assoc)
src/model/node_editor.rs:1181 fn move_search_cursor (assoc)
src/model/node_editor.rs:1202 fn close_search (assoc)
src/model/node_editor.rs:1208 fn begin_inline_edit (assoc)
src/model/node_editor.rs:1221 fn update_inline_edit (assoc)
src/model/node_editor.rs:1230 fn cancel_inline_edit (assoc)
src/model/node_editor.rs:1236 fn commit_inline_edit (assoc)
src/model/node_editor.rs:1243 fn commit_selected_search_result (assoc)
src/model/node_editor.rs:1260 fn projection_state (assoc)
src/model/node_editor.rs:1324 fn projection_config (assoc)
src/model/node_editor.rs:1338 fn replace (assoc)
src/model/node_editor.rs:1345 fn toggle (assoc)
src/model/node_editor.rs:1365 fn clear (assoc)
src/model/node_editor.rs:1370 fn select_all (assoc)
src/model/node_editor.rs:1375 fn rubber_band_commit (assoc)
src/model/node_editor.rs:1399 fn graph_entities (assoc)
src/model/node_editor.rs:1437 fn resolve_alignment_snap
src/model/node_editor.rs:1729 fn from_node_type (assoc)
src/model/node_editor.rs:1751 fn node_search_entries
src/model/node_editor.rs:1776 fn search_node_entries
src/model/node_editor.rs:1819 fn node_entry_accepts_dangling_wire
src/model/node_editor.rs:1836 fn node_search_score
src/model/node_editor.rs:1847 fn selectable_entities
src/model/node_editor.rs:1855 fn is_selectable_entity
src/model/node_editor.rs:1862 fn entity_to_graph_entity
src/model/node_editor.rs:1873 fn selection_delete_transaction
src/model/node_editor.rs:1904 fn selection_duplicate_transaction
src/model/node_editor.rs:1912 fn create_frame_around_selection_transaction
src/model/node_editor.rs:1961 fn clipboard_paste_transaction
src/model/node_editor.rs:1973 fn selection_nudge_transaction
src/model/node_editor.rs:2011 fn move_drag_transaction
src/model/node_editor.rs:2085 fn frame_resize_transaction
src/model/node_editor.rs:2099 fn inline_edit_transaction
src/model/node_graph.rs:7 const DEFAULT_MAX_UNDO_DEPTH
src/model/node_graph.rs:10 struct NodeGraphDocument
src/model/node_graph.rs:20 struct GraphRevision
src/model/node_graph.rs:23 struct NodeGraph
src/model/node_graph.rs:33 struct GraphNode
src/model/node_graph.rs:43 struct GraphConnection
src/model/node_graph.rs:52 enum GraphEntityId
src/model/node_graph.rs:59 struct GraphFrame
src/model/node_graph.rs:70 struct GraphMetadata
src/model/node_graph.rs:78 enum ParameterValue
src/model/node_graph.rs:91 struct NodeTypeCatalog
src/model/node_graph.rs:97 struct NodeType
src/model/node_graph.rs:110 struct CatalogRevision
src/model/node_graph.rs:113 struct PortDefinition
src/model/node_graph.rs:122 enum PortDirection
src/model/node_graph.rs:128 enum PortCardinality
src/model/node_graph.rs:134 struct PortTypeId
src/model/node_graph.rs:137 const ANY (assoc)
src/model/node_graph.rs:141 struct PortTypeInfo
src/model/node_graph.rs:149 struct PortTypeRegistry
src/model/node_graph.rs:154 enum GraphTransaction
src/model/node_graph.rs:236 struct GraphTransactionContext
src/model/node_graph.rs:242 trait GraphValidator
src/model/node_graph.rs:253 struct GraphCommit
src/model/node_graph.rs:259 enum GraphDelta
src/model/node_graph.rs:333 enum GraphError
src/model/node_graph.rs:389 struct CycleError
src/model/node_graph.rs:393 type SerializableGraph
src/model/node_graph.rs:396 fn new (assoc)
src/model/node_graph.rs:407 fn node (assoc)
src/model/node_graph.rs:411 fn connection (assoc)
src/model/node_graph.rs:415 fn frame (assoc)
src/model/node_graph.rs:419 fn nodes (assoc)
src/model/node_graph.rs:423 fn connections (assoc)
src/model/node_graph.rs:427 fn frames (assoc)
src/model/node_graph.rs:444 fn new (assoc)
src/model/node_graph.rs:451 fn register (assoc)
src/model/node_graph.rs:456 fn get (assoc)
src/model/node_graph.rs:460 fn revision (assoc)
src/model/node_graph.rs:464 fn types (assoc)
src/model/node_graph.rs:476 fn new (assoc)
src/model/node_graph.rs:480 fn register (assoc)
src/model/node_graph.rs:484 fn get (assoc)
src/model/node_graph.rs:488 fn are_compatible (assoc)
src/model/node_graph.rs:498 fn color (assoc)
src/model/node_graph.rs:507 fn new (assoc)
src/model/node_graph.rs:511 fn from_graph (assoc)
src/model/node_graph.rs:523 fn with_max_undo_depth (assoc)
src/model/node_graph.rs:529 fn graph (assoc)
src/model/node_graph.rs:533 fn revision (assoc)
src/model/node_graph.rs:537 fn apply (assoc)
src/model/node_graph.rs:559 fn node (assoc)
src/model/node_graph.rs:563 fn connection (assoc)
src/model/node_graph.rs:567 fn frame (assoc)
src/model/node_graph.rs:571 fn connections_for_node (assoc)
src/model/node_graph.rs:579 fn connections_for_port (assoc)
src/model/node_graph.rs:590 fn nodes (assoc)
src/model/node_graph.rs:594 fn connections (assoc)
src/model/node_graph.rs:598 fn frames (assoc)
src/model/node_graph.rs:602 fn undo (assoc)
src/model/node_graph.rs:613 fn redo (assoc)
src/model/node_graph.rs:625 fn can_undo (assoc)
src/model/node_graph.rs:629 fn can_redo (assoc)
src/model/node_graph.rs:633 fn validate (assoc)
src/model/node_graph.rs:641 fn topological_sort (assoc)
src/model/node_graph.rs:645 fn has_cycles (assoc)
src/model/node_graph.rs:649 fn to_serializable (assoc)
src/model/node_graph.rs:653 fn from_serializable (assoc)
src/model/node_graph_projection.rs:37 struct NodeGraphProjection
src/model/node_graph_projection.rs:54 struct NodeProjection
src/model/node_graph_projection.rs:69 enum NodeVisualState
src/model/node_graph_projection.rs:80 struct PortProjection
src/model/node_graph_projection.rs:89 struct WireProjection
src/model/node_graph_projection.rs:104 enum WireStyle
src/model/node_graph_projection.rs:112 enum WireVisualState
src/model/node_graph_projection.rs:123 struct FrameProjection
src/model/node_graph_projection.rs:136 enum FrameVisualState
src/model/node_graph_projection.rs:145 struct NodeGraphProjectionGeometry
src/model/node_graph_projection.rs:170 struct NodeGraphProjectionConfig
src/model/node_graph_projection.rs:187 struct NodeGraphProjectionState
src/model/node_graph_projection.rs:198 enum NodeGraphProjectionError
src/model/node_graph_projection.rs:223 fn from_document (assoc)
src/model/node_graph_projection.rs:237 fn build (assoc)
src/model/node_graph_projection.rs:289 fn revision (assoc)
src/model/node_graph_projection.rs:293 fn nodes (assoc)
src/model/node_graph_projection.rs:297 fn ports (assoc)
src/model/node_graph_projection.rs:303 fn wires (assoc)
src/model/node_graph_projection.rs:307 fn frames (assoc)
src/model/node_graph_projection.rs:311 fn canvas (assoc)
src/model/node_graph_projection.rs:315 fn spatial_bridge (assoc)
src/model/node_graph_projection.rs:323 fn spatial_projection (assoc)
src/model/node_graph_projection.rs:445 fn size (assoc)
src/model/node_graph_projection.rs:449 fn bounds (assoc)
src/model/node_graph_projection.rs:458 fn center (assoc)
src/model/node_graph_projection.rs:464 fn bounds (assoc)
src/model/node_graph_projection.rs:485 fn wire_path
src/model/notification.rs:15 struct NotificationId
src/model/notification.rs:18 enum NotificationLevel
src/model/notification.rs:25 struct NotificationAction
src/model/notification.rs:30 struct NotificationEntry
src/model/notification.rs:44 struct NotificationHandle
src/model/notification.rs:52 const DEFAULT_CAPACITY
src/model/notification.rs:54 struct NotificationManager
src/model/notification.rs:60 fn new (assoc)
src/model/notification.rs:64 fn with_capacity (assoc)
src/model/notification.rs:122 fn len (assoc)
src/model/notification.rs:126 fn is_empty (assoc)
src/model/notification.rs:130 fn capacity (assoc)
src/model/order_book.rs:44 fn new (assoc)
src/model/order_book.rs:48 fn best_price (assoc)
src/model/order_book.rs:52 fn max_quantity (assoc)
src/model/order_book.rs:64 enum BookSideKind
src/model/popup_menu.rs:44 fn is_open (assoc)
src/model/popup_menu.rs:48 fn items (assoc)
src/model/popup_menu.rs:52 fn surface (assoc)
src/model/popup_menu.rs:56 fn surface_mut (assoc)
src/model/search_projection_harness.rs:52 fn new (assoc)
src/model/search_projection_harness.rs:134 fn new (assoc)
src/model/signal_store.rs:48 fn new (assoc)
src/model/spatial/mod.rs:14 struct SpatialItemId
src/model/spatial/mod.rs:17 fn from_opaque (assoc)
src/model/spatial/mod.rs:23 struct SpatialProjectionRevision
src/model/spatial/mod.rs:26 struct SpatialTransform
src/model/spatial/mod.rs:33 struct SpatialViewportState
src/model/spatial/mod.rs:38 enum SpatialCoordinateMode
src/model/spatial/mod.rs:44 enum SpatialItemRole
src/model/spatial/mod.rs:58 struct SpatialItem
src/model/spatial/mod.rs:67 enum SpatialPrimitive
src/model/spatial/mod.rs:90 enum SpatialStrokeRole
src/model/spatial/mod.rs:99 enum SpatialHitRegion
src/model/spatial/mod.rs:120 enum SpatialHitPriority
src/model/spatial/mod.rs:130 enum SpatialRoutePlan
src/model/spatial/mod.rs:182 fn polyline (assoc)
src/model/spatial/mod.rs:204 fn polyline_points (assoc)
src/model/spatial/mod.rs:246 enum SpatialGuideRole
src/model/spatial/mod.rs:252 enum SpatialGuideAxis
src/model/spatial/mod.rs:258 struct SpatialGuidePlan
src/model/spatial/mod.rs:267 fn vertical (assoc)
src/model/spatial/mod.rs:277 fn horizontal (assoc)
src/model/spatial/mod.rs:289 enum SpatialOverlayAnchor
src/model/spatial/mod.rs:297 struct SpatialOverlayPlan
src/model/spatial/mod.rs:305 fn anchored (assoc)
src/model/spatial/mod.rs:314 fn with_offset (assoc)
src/model/spatial/mod.rs:320 struct SpatialRoleVisual
src/model/spatial/mod.rs:327 struct SpatialVisualPolicy
src/model/spatial/mod.rs:349 struct SpatialGridPolicy
src/model/spatial/mod.rs:357 enum SpatialGridLineRole
src/model/spatial/mod.rs:363 struct SpatialGridLine
src/model/spatial/mod.rs:371 enum SpatialGestureClass
src/model/spatial/mod.rs:380 struct SpatialGestureFacts
src/model/spatial/mod.rs:387 enum SpatialCursorPolicy
src/model/spatial/mod.rs:398 struct SpatialInteractionPolicy
src/model/spatial/mod.rs:413 struct SpatialPointerFacts
src/model/spatial/mod.rs:422 enum SpatialIntent
src/model/spatial/mod.rs:442 struct SpatialProjection
src/model/spatial/mod.rs:454 trait SpatialAdapter
src/model/spatial/mod.rs:477 struct CanvasSpatialBridge
src/model/spatial/mod.rs:483 fn identity (assoc)
src/model/spatial/mod.rs:491 fn world_to_screen (assoc)
src/model/spatial/mod.rs:498 fn screen_to_world (assoc)
src/model/spatial/mod.rs:527 fn for_theme (assoc)
src/model/spatial/mod.rs:674 fn gesture_class (assoc)
src/model/spatial/mod.rs:678 fn is_pan_gesture (assoc)
src/model/spatial/mod.rs:683 fn is_rubber_band_gesture (assoc)
src/model/spatial/mod.rs:687 fn is_route_creation_gesture (assoc)
src/model/spatial/mod.rs:691 fn selection_toggle_requested (assoc)
src/model/spatial/mod.rs:695 fn cursor_for_gesture (assoc)
src/model/spatial/mod.rs:717 fn cursor_for_route_creation (assoc)
src/model/spatial/mod.rs:729 fn key_intent (assoc)
src/model/spatial/mod.rs:740 fn new (assoc)
src/model/spatial/mod.rs:759 fn from_parts (assoc)
src/model/spatial/mod.rs:783 fn revision (assoc)
src/model/spatial/mod.rs:787 fn viewport (assoc)
src/model/spatial/mod.rs:791 fn coordinate_mode (assoc)
src/model/spatial/mod.rs:795 fn items (assoc)
src/model/spatial/mod.rs:799 fn hit_regions (assoc)
src/model/spatial/mod.rs:803 fn routes (assoc)
src/model/spatial/mod.rs:807 fn guides (assoc)
src/model/spatial/mod.rs:811 fn overlays (assoc)
src/model/spatial/mod.rs:815 fn visual_policy (assoc)
src/model/spatial/mod.rs:819 fn in_draw_order (assoc)
src/model/spatial/mod.rs:825 fn in_region (assoc)
src/model/spatial/mod.rs:832 fn label_bounds (assoc)
src/model/spatial/mod.rs:839 fn hit_test (assoc)
src/model/spatial/mod.rs:867 fn world_bounds (assoc)
src/model/spatial/mod.rs:894 fn realize_spatial_grid
src/model/spatial/mod.rs:963 fn resolve_spatial_overlay_rect
src/model/spatial/mod.rs:980 fn resolve_spatial_gesture_class
src/model/spatial/mod.rs:991 struct SpatialHit
src/model/spatial/mod.rs:998 fn id (assoc)
src/model/spatial/mod.rs:1032 fn from_canvas_projection (assoc)
src/model/spatial/mod.rs:1072 fn projection (assoc)
src/model/spatial/mod.rs:1076 fn into_projection (assoc)
src/model/spatial/mod.rs:1080 fn canvas_entity (assoc)
src/model/spatial/mod.rs:1084 fn spatial_id_for_canvas (assoc)
src/model/time.rs:8 const MICROS_PER_SECOND
src/model/time.rs:9 const MICROS_PER_MINUTE
src/model/time.rs:10 const MICROS_PER_HOUR
src/model/time.rs:11 const MICROS_PER_DAY
src/model/time.rs:18 fn from_secs (assoc)
src/model/time.rs:22 fn from_millis (assoc)
src/model/time.rs:26 fn as_secs_f64 (assoc)
src/model/time.rs:39 fn new (assoc)
src/model/time.rs:47 fn duration_us (assoc)
src/model/time.rs:51 fn is_empty (assoc)
src/model/time.rs:65 fn intersect (assoc)
src/model/time_value.rs:13 fn new (assoc)
src/model/time_value.rs:26 fn format_24h (assoc)
src/model/time_value.rs:33 fn format_12h (assoc)
src/model/time_value.rs:49 fn is_am (assoc)
src/model/toast.rs:15 struct ToastId
src/model/toast.rs:18 enum DismissState
src/model/toast.rs:25 struct ToastAction
src/model/toast.rs:29 struct ToastEntry
src/model/toast.rs:41 struct ToastStats
src/model/toast.rs:60 struct ToastHandle
src/model/toast.rs:66 fn is_dismissed (assoc)
src/model/toast.rs:70 fn dismiss (assoc)
src/model/toast.rs:83 struct ToastManager
src/model/toast.rs:92 fn new (assoc)
src/model/toast.rs:162 fn dismiss (assoc)
src/model/toast.rs:170 fn dismiss_all (assoc)
src/model/toast.rs:234 fn advance_lifecycle (assoc)
src/model/toast.rs:289 fn stats (assoc)
src/model/tooltip.rs:234 fn new (assoc)
src/model/tooltip.rs:361 fn placement (assoc)
src/model/tutorial.rs:40 fn new (assoc)
src/model/tutorial.rs:65 fn new (assoc)
src/oracles/mod.rs:33 use baseline_data_oracle
src/platform/font_loader.rs:4 fn load_inter_font
src/platform/font_manager_text_engine.rs:362 fn compute_font_id_for_inter (assoc)
src/platform/menu_host.rs:5 trait MenuHost
src/platform/menu_host.rs:13 enum MenuHostKind
src/platform/menu_host.rs:18 fn resolve_menu_host_kind
src/platform/native_font_backend_directwrite.rs:64 fn new (assoc)
src/platform/native_menu_macos.rs:72 struct MacOsMenuHost
src/platform/native_menu_macos.rs:81 fn new (assoc)
src/platform/text_engine_glyphon.rs:66 fn new (assoc)
src/platform/window.rs:95 fn request_redraw (assoc)
src/platform/window.rs:99 fn inner_size (assoc)
src/platform/window.rs:107 fn update_cursor_pos (assoc)
src/platform/window.rs:111 fn last_cursor_pos (assoc)
src/platform/window.rs:115 fn update_scale_factor (assoc)
src/plot/mod.rs:31 use axis
src/plot/mod.rs:32 use axis_authority
src/plot/mod.rs:33 use color_scheme
src/plot/mod.rs:34 use market_detail
src/plot/mod.rs:39 use probe
src/plot/mod.rs:40 use render_pool
src/plot/mod.rs:41 use router
src/plot/mod.rs:42 use series
src/plot/mod.rs:43 use series
src/plot/mod.rs:44 use series
src/plot/mod.rs:45 use series
src/plot/mod.rs:46 use series
src/plot/mod.rs:47 use series
src/plot/mod.rs:48 use series
src/plot/mod.rs:49 use series
src/plot/mod.rs:50 use series
src/plot/mod.rs:54 use series
src/plot/mod.rs:55 use series
src/plot/mod.rs:56 use spatial
src/plot/mod.rs:60 use transform
src/plot/mod.rs:61 use view_state
src/plot/mod.rs:63 use price_level
src/plot/mod.rs:69 use tool
src/plot/mod.rs:90 use crate (assoc)
src/plot/mod.rs:92 const TOOL_POINTER (assoc)
src/plot/mod.rs:93 const TOOL_MEASURE (assoc)
src/plot/mod.rs:94 const TOOL_HLINE (assoc)
src/plot/mod.rs:95 const TOOL_RECT (assoc)
src/plot/mod.rs:96 const TOOL_TRENDLINE (assoc)
src/plot/mod.rs:97 const TOOL_FIBONACCI (assoc)
src/plot/mod.rs:99 const CLEAR_SELECTION (assoc)
src/plot/mod.rs:100 const RESET_VIEW (assoc)
src/plot/mod.rs:101 const ZOOM_IN (assoc)
src/plot/mod.rs:102 const ZOOM_OUT (assoc)
src/plot/mod.rs:103 const TOGGLE_TOOLBAR (assoc)
src/plot/mod.rs:821 fn with_gpu_resources (assoc)
src/plot/mod.rs:826 fn with_render_pool (assoc)
src/plot/mod.rs:927 fn with_color_scheme (assoc)
src/plot/mod.rs:1314 fn with_circle (assoc)
src/plot/mod.rs:1319 fn add_circle (assoc)
src/plot/mod.rs:1328 fn remove_circle (assoc)
src/plot/mod.rs:1333 fn remove_circle_group (assoc)
src/plot/mod.rs:1338 fn clear_circles (assoc)
src/plot/mod.rs:1343 fn take_circle_events (assoc)
src/plot/mod.rs:1347 fn with_line (assoc)
src/plot/mod.rs:1352 fn add_line (assoc)
src/plot/mod.rs:1361 fn remove_line (assoc)
src/plot/mod.rs:1366 fn remove_line_group (assoc)
src/plot/mod.rs:1371 fn clear_lines (assoc)
src/plot/mod.rs:1376 fn take_line_events (assoc)
src/plot/mod.rs:1380 fn with_bezier (assoc)
src/plot/mod.rs:1385 fn add_bezier (assoc)
src/plot/mod.rs:1394 fn remove_bezier (assoc)
src/plot/mod.rs:1399 fn remove_bezier_group (assoc)
src/plot/mod.rs:1404 fn clear_beziers (assoc)
src/plot/mod.rs:1409 fn take_bezier_events (assoc)
src/plot/mod.rs:1413 fn with_styled_path (assoc)
src/plot/mod.rs:1418 fn add_styled_path (assoc)
src/plot/mod.rs:1427 fn remove_styled_path (assoc)
src/plot/mod.rs:1432 fn remove_styled_path_group (assoc)
src/plot/mod.rs:1438 fn clear_styled_paths (assoc)
src/plot/mod.rs:1443 fn take_styled_path_events (assoc)
src/plot/mod.rs:1447 fn with_path (assoc)
src/plot/mod.rs:1452 fn add_path (assoc)
src/plot/mod.rs:1461 fn remove_path (assoc)
src/plot/mod.rs:1466 fn remove_path_group (assoc)
src/plot/mod.rs:1471 fn clear_paths (assoc)
src/plot/mod.rs:1476 fn take_path_events (assoc)
src/plot/color_scheme.rs:59 fn from_theme (assoc)
src/plot/color_scheme.rs:109 fn dark (assoc)
src/plot/color_scheme.rs:113 fn light (assoc)
src/plot/market_detail.rs:50 fn new (assoc)
src/plot/market_detail.rs:54 fn with_axis_kind (assoc)
src/plot/market_detail.rs:68 fn price_range (assoc)
src/plot/market_detail.rs:75 fn price_axis_kind (assoc)
src/plot/market_detail.rs:234 fn new (assoc)
src/plot/market_detail.rs:238 fn get (assoc)
src/plot/market_detail.rs:244 struct LiquiditySummary
src/plot/market_detail.rs:259 struct ProjectedPriceRange
src/plot/market_detail.rs:267 struct ProjectedBookLevel
src/plot/market_detail.rs:280 struct MarketDetailProjection
src/plot/market_detail.rs:377 fn replace_book (assoc)
src/plot/market_detail.rs:385 fn replace_bids (assoc)
src/plot/market_detail.rs:393 fn replace_asks (assoc)
src/plot/market_detail.rs:401 fn source_generation (assoc)
src/plot/market_detail.rs:405 fn projection_count (assoc)
src/plot/market_detail.rs:409 fn project (assoc)
src/plot/market_detail.rs:449 fn nearest_exact (assoc)
src/plot/market_detail.rs:787 fn current (assoc)
src/plot/market_detail.rs:791 fn set_visible_levels (assoc)
src/plot/market_detail.rs:815 fn advance (assoc)
src/plot/market_detail.rs:828 fn settled (assoc)
src/plot/price_level.rs:41 fn new (assoc)
src/plot/price_level.rs:49 fn color (assoc)
src/plot/price_level.rs:54 fn dimmed (assoc)
src/plot/render_pool.rs:46 fn new (assoc)
src/plot/series/mod.rs:14 use area
src/plot/series/mod.rs:15 use bar
src/plot/series/mod.rs:16 use candle
src/plot/series/mod.rs:17 use circle
src/plot/series/mod.rs:18 use depth
src/plot/series/mod.rs:22 use histogram
src/plot/series/mod.rs:23 use line
src/plot/series/area.rs:44 struct AreaSeries
src/plot/series/area.rs:60 fn new (assoc)
src/plot/series/area.rs:75 fn with_style (assoc)
src/plot/series/area.rs:81 fn with_z_order (assoc)
src/plot/series/area.rs:86 fn with_auto_scale_y (assoc)
src/plot/series/area.rs:91 fn set_points (assoc)
src/plot/series/bar.rs:51 struct BarSeries
src/plot/series/bar.rs:73 fn new (assoc)
src/plot/series/bar.rs:91 fn with_style (assoc)
src/plot/series/bar.rs:116 fn with_z_order (assoc)
src/plot/series/bar.rs:128 fn set_points (assoc)
src/plot/series/candle.rs:44 struct CandleInstance
src/plot/series/candle.rs:55 fn desc (assoc)
src/plot/series/candle.rs:99 struct Vertex
src/plot/series/candle.rs:105 fn desc (assoc)
src/plot/series/candle.rs:129 struct Uniforms
src/plot/series/candle.rs:310 struct CandleSeries
src/plot/series/candle.rs:331 fn new (assoc)
src/plot/series/candle.rs:358 fn set_candles (assoc)
src/plot/series/circle.rs:59 struct CircleVertex
src/plot/series/circle.rs:75 fn desc (assoc)
src/plot/series/circle.rs:200 struct CircleSeries
src/plot/series/circle.rs:220 fn new (assoc)
src/plot/series/circle.rs:236 fn with_style (assoc)
src/plot/series/circle.rs:242 fn with_z_order (assoc)
src/plot/series/circle.rs:247 fn with_auto_scale_y (assoc)
src/plot/series/circle.rs:252 fn with_interaction (assoc)
src/plot/series/circle.rs:257 fn set_points (assoc)
src/plot/series/depth.rs:15 use crate
src/plot/series/depth.rs:108 struct DepthBarSeries
src/plot/series/depth.rs:127 fn new (assoc)
src/plot/series/depth.rs:143 fn with_style (assoc)
src/plot/series/depth.rs:149 fn with_z_order (assoc)
src/plot/series/depth.rs:154 fn set_bids (assoc)
src/plot/series/depth.rs:158 fn set_asks (assoc)
src/plot/series/depth.rs:162 fn set_both (assoc)
src/plot/series/histogram.rs:41 struct HistogramSeries
src/plot/series/histogram.rs:57 fn new (assoc)
src/plot/series/histogram.rs:72 fn with_style (assoc)
src/plot/series/histogram.rs:78 fn with_z_order (assoc)
src/plot/series/histogram.rs:83 fn with_auto_scale_y (assoc)
src/plot/series/histogram.rs:88 fn set_points (assoc)
src/plot/series/line.rs:37 struct LineSeries
src/plot/series/line.rs:57 fn new (assoc)
src/plot/series/line.rs:73 fn with_style (assoc)
src/plot/series/line.rs:79 fn with_z_order (assoc)
src/plot/series/line.rs:96 fn set_points (assoc)
src/plot/spatial.rs:97 fn from_snapshot (assoc)
src/plot/spatial.rs:105 fn key_for (assoc)
src/plot/spatial.rs:111 fn context (assoc)
src/plot/tool.rs:154 fn new (assoc)
src/plot/tool.rs:443 fn new (assoc)
src/plot/tool.rs:564 fn new (assoc)
src/plot/transform.rs:20 fn new (assoc)
src/plot/transform.rs:30 fn price_to_px (assoc)
src/plot/transform.rs:36 fn px_to_price (assoc)
src/plot/transform.rs:192 fn price_transform (assoc)
src/renderer/mod.rs:21 use compiled_draw_plan
src/renderer/mod.rs:22 use custom_render_pass
src/renderer/mod.rs:26 use draw_plan
src/renderer/mod.rs:27 use draw_plan_builder
src/renderer/mod.rs:28 use render_target
src/renderer/mod.rs:32 use vertex
src/renderer/mod.rs:34 use path_renderer
src/renderer/mod.rs:35 use render_graph
src/renderer/batcher.rs:317 fn new (assoc)
src/renderer/batcher.rs:416 fn new (assoc)
src/renderer/batcher.rs:420 fn end (assoc)
src/renderer/batcher.rs:631 struct GradientStopData
src/renderer/batcher.rs:638 fn batch_hash (assoc)
src/renderer/batcher.rs:721 fn new (assoc)
src/renderer/batcher.rs:734 fn batch_count (assoc)
src/renderer/batcher.rs:738 fn is_empty (assoc)
src/renderer/custom_render_pass.rs:15 fn new (assoc)
src/renderer/custom_render_pass.rs:24 fn size (assoc)
src/renderer/custom_render_pass.rs:28 fn is_empty (assoc)
src/renderer/custom_render_pass.rs:34 enum CustomRenderPassError
src/renderer/custom_render_pass.rs:54 type CustomRenderPassResult
src/renderer/custom_render_pass.rs:56 trait CustomRenderLifecycle
src/renderer/custom_render_pass.rs:78 struct CustomRenderPass
src/renderer/custom_render_pass.rs:90 fn new (assoc)
src/renderer/custom_render_pass.rs:98 fn new_with_filter (assoc)
src/renderer/custom_render_pass.rs:131 fn texture_id (assoc)
src/renderer/custom_render_pass.rs:135 fn physical_rect (assoc)
src/renderer/custom_render_pass.rs:139 fn physical_size (assoc)
src/renderer/custom_render_pass.rs:143 fn texture_format (assoc)
src/renderer/custom_render_pass.rs:154 fn swap_count (assoc)
src/renderer/custom_render_pass.rs:158 fn queue_front_registration (assoc)
src/renderer/custom_render_pass.rs:169 fn queue_front_unregistration (assoc)
src/renderer/custom_render_pass.rs:191 fn register_front_now (assoc)
src/renderer/custom_render_pass.rs:202 fn begin_frame (assoc)
src/renderer/custom_render_pass.rs:219 fn submit_and_swap (assoc)
src/renderer/custom_render_pass.rs:235 fn resize (assoc)
src/renderer/custom_render_pass.rs:244 fn resize_registered (assoc)
src/renderer/custom_render_pass.rs:257 fn destroy (assoc)
src/renderer/custom_render_pass.rs:263 fn destroy_deferred (assoc)
src/renderer/custom_render_pass.rs:422 struct CustomRenderFrame
src/renderer/custom_render_pass.rs:428 fn encoder_and_view (assoc)
src/renderer/draw_plan.rs:96 fn new (assoc)
src/renderer/draw_plan.rs:104 fn desc (assoc)
src/renderer/draw_plan.rs:142 fn new (assoc)
src/renderer/draw_plan.rs:146 fn desc (assoc)
src/renderer/glyph_atlas.rs:292 fn new (assoc)
src/renderer/pipelines/path_pipeline.rs:717 fn create_dummy (assoc)
src/renderer/pipelines/shape_pipeline.rs:42 const ATTRIBUTES (assoc)
src/renderer/pipelines/shape_pipeline.rs:75 fn desc (assoc)
src/renderer/pipelines/shape_pipeline.rs:83 fn new (assoc)
src/renderer/pipelines/shape_pipeline.rs:202 fn pipeline (assoc)
src/renderer/pipelines/shape_pipeline.rs:207 fn layout (assoc)
src/renderer/pipelines/text_pipeline.rs:9 use crate
src/renderer/render_graph/mod.rs:45 use builder
src/renderer/render_graph/mod.rs:46 use executor
src/renderer/render_graph/mod.rs:50 use graph
src/renderer/render_graph/mod.rs:51 use pass
src/renderer/render_graph/mod.rs:52 use passes
src/renderer/render_graph/mod.rs:53 use resources
src/renderer/render_graph/mod.rs:57 use topology
src/renderer/render_graph/pass.rs:80 fn missing_attachment (assoc)
src/renderer/render_graph/pass.rs:84 fn missing_buffer (assoc)
src/renderer/render_graph/passes/mod.rs:11 use background_prepass
src/renderer/render_graph/passes/mod.rs:12 use main_pass
src/renderer/render_graph/passes/mod.rs:13 use overlay_pass
src/renderer/text_vertex.rs:27 fn desc (assoc)
src/renderer/vertex.rs:27 const ATTRIBUTES (assoc)
src/renderer/vertex.rs:40 fn desc (assoc)
src/renderer/vertex.rs:89 const ATTRIBUTES (assoc)
src/renderer/vertex.rs:112 fn desc (assoc)
src/renderer/vertex.rs:143 const ATTRIBUTES (assoc)
src/renderer/vertex.rs:166 fn desc (assoc)
src/renderer/vertex.rs:218 const ATTRIBUTES (assoc)
src/renderer/vertex.rs:236 fn desc (assoc)
src/renderer/vertex.rs:265 fn desc (assoc)
src/renderer/wgpu_renderer.rs:500 struct GpuResources
src/renderer/wgpu_renderer.rs:582 struct WgpuRenderer
src/renderer/wgpu_renderer.rs:831 fn new (assoc)
src/renderer/wgpu_renderer.rs:4020 fn text_engine_mut (assoc)
src/renderer/wgpu_renderer.rs:4025 fn text_engine_mut (assoc)
src/renderer/wgpu_renderer.rs:4030 fn text_engine_mut (assoc)
src/renderer/wgpu_renderer.rs:4035 fn text_engine_mut (assoc)
src/runtime/mod.rs:9 use actions
src/runtime/mod.rs:10 use binding_registry
src/runtime/mod.rs:11 use builtins
src/runtime/actions.rs:33 const FOCUS_LEFT (assoc)
src/runtime/actions.rs:34 const FOCUS_RIGHT (assoc)
src/runtime/actions.rs:35 const FOCUS_UP (assoc)
src/runtime/actions.rs:36 const FOCUS_DOWN (assoc)
src/runtime/actions.rs:39 const FOCUS_NEXT (assoc)
src/runtime/actions.rs:40 const FOCUS_PREV (assoc)
src/runtime/actions.rs:43 const ENTER_WIDGET (assoc)
src/runtime/actions.rs:44 const RELEASE_CAPTURE (assoc)
src/runtime/actions.rs:45 const EXIT_SCOPE (assoc)
src/runtime/actions.rs:48 const ACTIVATE (assoc)
src/runtime/actions.rs:53 const PLOT_TOOL_POINTER (assoc)
src/runtime/actions.rs:54 const PLOT_TOOL_MEASURE (assoc)
src/runtime/actions.rs:55 const PLOT_TOOL_HLINE (assoc)
src/runtime/actions.rs:56 const PLOT_TOOL_RECT (assoc)
src/runtime/actions.rs:57 const PLOT_TOOL_TRENDLINE (assoc)
src/runtime/actions.rs:58 const PLOT_TOOL_FIBONACCI (assoc)
src/runtime/actions.rs:75 const NODE_FLOW_DELETE_SELECTED (assoc)
src/runtime/actions.rs:76 const NODE_FLOW_SELECT_ALL (assoc)
src/runtime/actions.rs:77 const NODE_FLOW_DUPLICATE_SELECTED (assoc)
src/runtime/actions.rs:78 const NODE_FLOW_COPY_SELECTED (assoc)
src/runtime/actions.rs:79 const NODE_FLOW_PASTE (assoc)
src/runtime/actions.rs:80 const NODE_FLOW_CUT_SELECTED (assoc)
src/runtime/actions.rs:81 const NODE_FLOW_UNDO (assoc)
src/runtime/actions.rs:82 const NODE_FLOW_REDO (assoc)
src/runtime/actions.rs:83 const NODE_FLOW_OPEN_SEARCH (assoc)
src/runtime/actions.rs:84 const NODE_FLOW_CANCEL (assoc)
src/runtime/actions.rs:85 const NODE_FLOW_ZOOM_TO_FIT (assoc)
src/runtime/actions.rs:86 const NODE_FLOW_RESET_VIEW (assoc)
src/runtime/actions.rs:87 const NODE_FLOW_ZOOM_IN (assoc)
src/runtime/actions.rs:88 const NODE_FLOW_ZOOM_OUT (assoc)
src/runtime/actions.rs:89 const NODE_FLOW_NUDGE_LEFT (assoc)
src/runtime/actions.rs:90 const NODE_FLOW_NUDGE_RIGHT (assoc)
src/runtime/actions.rs:91 const NODE_FLOW_NUDGE_UP (assoc)
src/runtime/actions.rs:92 const NODE_FLOW_NUDGE_DOWN (assoc)
src/runtime/actions.rs:93 const NODE_FLOW_NUDGE_LEFT_FAST (assoc)
src/runtime/actions.rs:94 const NODE_FLOW_NUDGE_RIGHT_FAST (assoc)
src/runtime/actions.rs:95 const NODE_FLOW_NUDGE_UP_FAST (assoc)
src/runtime/actions.rs:96 const NODE_FLOW_NUDGE_DOWN_FAST (assoc)
src/runtime/binding_registry.rs:26 fn from_key_modifiers (assoc)
src/runtime/binding_registry.rs:36 fn key_only (assoc)
src/runtime/binding_registry.rs:123 struct BindingRegistry
src/runtime/binding_registry.rs:137 fn new (assoc)
src/semantics/hierarchy.rs:86 fn new (assoc)
src/widgets/mod.rs:100 use action_palette
src/widgets/mod.rs:101 use aligned_order_book
src/widgets/mod.rs:104 use auxiliary_text
src/widgets/mod.rs:107 use exclusion_flow
src/widgets/mod.rs:110 use radio_group_widget
src/widgets/mod.rs:113 use combo_box
src/widgets/mod.rs:116 use readout
src/widgets/mod.rs:121 use section_surfaces
src/widgets/mod.rs:127 use list_view
src/widgets/mod.rs:130 use tab_bar
src/widgets/mod.rs:135 use menu_bar
src/widgets/mod.rs:140 use tab_switcher
src/widgets/mod.rs:143 use breadcrumbs
src/widgets/mod.rs:146 use tooltip_surface
src/widgets/mod.rs:148 use tooltip_host
src/widgets/mod.rs:151 use date_picker_field
src/widgets/mod.rs:154 use keybind_editor
src/widgets/aligned_order_book.rs:128 fn new (assoc)
src/widgets/aligned_order_book.rs:152 fn with_config (assoc)
src/widgets/aligned_order_book.rs:157 fn with_projection_config (assoc)
src/widgets/aligned_order_book.rs:174 fn with_book (assoc)
src/widgets/aligned_order_book.rs:179 fn with_bids (assoc)
src/widgets/aligned_order_book.rs:184 fn with_asks (assoc)
src/widgets/aligned_order_book.rs:189 fn with_update_channel (assoc)
src/widgets/aligned_order_book.rs:195 fn set_book (assoc)
src/widgets/aligned_order_book.rs:199 fn set_bids (assoc)
src/widgets/aligned_order_book.rs:203 fn set_asks (assoc)
src/widgets/aligned_order_book.rs:207 fn take_events (assoc)
src/widgets/aligned_order_book.rs:211 fn selection (assoc)
src/widgets/aligned_order_book.rs:215 fn set_selection (assoc)
src/widgets/aligned_order_book.rs:219 fn clear_selection (assoc)
src/widgets/aligned_order_book.rs:223 fn projection (assoc)
src/widgets/aligned_order_book.rs:227 fn projection_count (assoc)
src/widgets/auxiliary_text.rs:22 fn new (assoc)
src/widgets/auxiliary_text.rs:31 fn with_size (assoc)
src/widgets/auxiliary_text.rs:36 fn set_text (assoc)
src/widgets/breadcrumbs.rs:49 struct Breadcrumbs
src/widgets/breadcrumbs.rs:81 fn new (assoc)
src/widgets/button.rs:23 enum ButtonState
src/widgets/button.rs:80 fn new (assoc)
src/widgets/button.rs:125 fn with_padding (assoc)
src/widgets/button.rs:130 fn on_click (assoc)
src/widgets/button.rs:146 fn with_binding (assoc)
src/widgets/color_picker.rs:87 fn new (assoc)
src/widgets/color_picker.rs:132 fn with_embedded (assoc)
src/widgets/color_picker_field.rs:16 struct ColorPickerField
src/widgets/color_picker_field.rs:49 fn new (assoc)
src/widgets/color_picker_field.rs:72 fn with_color (assoc)
src/widgets/color_picker_field.rs:77 fn color (assoc)
src/widgets/color_picker_field.rs:81 fn hex_string (assoc)
src/widgets/color_picker_field.rs:85 fn picker (assoc)
src/widgets/color_picker_field.rs:89 fn picker_mut (assoc)
src/widgets/combo_box.rs:222 fn new (assoc)
src/widgets/combo_box.rs:479 fn input_text (assoc)
src/widgets/combo_box.rs:496 fn set_close_on_text_click (assoc)
src/widgets/combo_box.rs:500 fn set_type_to_search (assoc)
src/widgets/combo_box.rs:510 fn set_match_mode (assoc)
src/widgets/combo_box.rs:514 fn set_allow_custom_value (assoc)
src/widgets/combo_box.rs:518 fn set_text_input_enabled (assoc)
src/widgets/combo_box.rs:579 fn is_loading (assoc)
src/widgets/console.rs:362 fn is_empty (assoc)
src/widgets/console.rs:367 struct ConsoleConfig
src/widgets/console.rs:496 enum ConsoleUpdate
src/widgets/console.rs:503 struct Console
src/widgets/console.rs:542 fn new (assoc)
src/widgets/console.rs:546 fn with_config (assoc)
src/widgets/console.rs:567 fn with_capacity (assoc)
src/widgets/console.rs:2122 struct ConsoleBody
src/widgets/container.rs:29 fn new (assoc)
src/widgets/container.rs:55 fn with_background (assoc)
src/widgets/container.rs:60 fn with_radius (assoc)
src/widgets/container.rs:65 fn with_width (assoc)
src/widgets/container.rs:70 fn with_height (assoc)
src/widgets/context_menu.rs:22 struct ContextMenu
src/widgets/context_menu.rs:30 fn new (assoc)
src/widgets/context_menu.rs:39 fn with_popup_rect_sink (assoc)
src/widgets/context_menu.rs:159 struct ContextMenuScreen
src/widgets/context_menu.rs:167 fn new (assoc)
src/widgets/date_picker.rs:24 const DATEPICKER_W
src/widgets/date_picker.rs:25 const DATEPICKER_H
src/widgets/date_picker.rs:37 struct DatePickerState
src/widgets/date_picker.rs:48 fn new (assoc)
src/widgets/date_picker.rs:70 fn selected (assoc)
src/widgets/date_picker.rs:74 fn view_year (assoc)
src/widgets/date_picker.rs:78 fn view_month (assoc)
src/widgets/date_picker.rs:82 fn kbd_cursor (assoc)
src/widgets/date_picker.rs:332 struct DateMark
src/widgets/date_picker.rs:368 fn new (assoc)
src/widgets/date_picker.rs:375 fn with_highlight (assoc)
src/widgets/date_picker.rs:383 fn with_disabled (assoc)
src/widgets/date_picker.rs:396 struct DatePickerWidget
src/widgets/date_picker.rs:429 fn new (assoc)
src/widgets/date_picker.rs:467 fn with_today (assoc)
src/widgets/date_picker.rs:478 fn with_binding (assoc)
src/widgets/date_picker.rs:483 fn with_marks (assoc)
src/widgets/date_picker.rs:488 fn with_policy (assoc)
src/widgets/date_picker.rs:493 fn with_embedded (assoc)
src/widgets/date_picker.rs:503 fn is_year_jump_active (assoc)
src/widgets/date_picker.rs:530 fn state (assoc)
src/widgets/date_picker.rs:534 fn state_mut (assoc)
src/widgets/date_picker_field.rs:36 struct DatePickerField
src/widgets/date_picker_field.rs:63 fn new (assoc)
src/widgets/date_picker_field.rs:83 fn with_commit_mode (assoc)
src/widgets/date_picker_field.rs:88 fn with_today (assoc)
src/widgets/date_picker_field.rs:93 fn with_state (assoc)
src/widgets/date_picker_field.rs:98 fn with_policy (assoc)
src/widgets/date_picker_field.rs:103 fn with_marks (assoc)
src/widgets/date_picker_field.rs:108 fn with_binding (assoc)
src/widgets/date_picker_field.rs:113 fn with_placeholder (assoc)
src/widgets/date_picker_field.rs:118 fn state (assoc)
src/widgets/date_picker_field.rs:122 fn state_mut (assoc)
src/widgets/dialog.rs:31 struct DialogConfig
src/widgets/dialog.rs:39 struct DialogButton
src/widgets/dialog.rs:626 struct Dialog
src/widgets/dialog.rs:636 fn new (assoc)
src/widgets/disclosure_toggle.rs:61 fn new (assoc)
src/widgets/disclosure_toggle.rs:82 fn set_expanded (assoc)
src/widgets/divider.rs:13 enum Orientation
src/widgets/divider.rs:27 fn horizontal (assoc)
src/widgets/divider.rs:35 fn vertical (assoc)
src/widgets/divider.rs:43 fn with_color (assoc)
src/widgets/dropdown_menu.rs:24 struct DropdownMenu
src/widgets/dropdown_menu.rs:37 fn new (assoc)
src/widgets/dropdown_menu.rs:50 fn set_actions (assoc)
src/widgets/dropdown_menu.rs:54 fn set_disabled (assoc)
src/widgets/event_log/mod.rs:6 use data_source
src/widgets/event_log/mod.rs:10 use event_log
src/widgets/event_log/mod.rs:11 use severity_painter
src/widgets/event_log/mod.rs:12 use types
src/widgets/exclusion_flow.rs:12 enum ExclusionFlowDebug
src/widgets/exclusion_flow.rs:134 struct ExclusionFlow
src/widgets/exclusion_flow.rs:144 fn new (assoc)
src/widgets/exclusion_flow.rs:155 fn with_padding (assoc)
src/widgets/exclusion_flow.rs:160 fn with_spacing (assoc)
src/widgets/exclusion_flow.rs:165 fn with_exclusion (assoc)
src/widgets/exclusion_flow.rs:170 fn with_exclusions (assoc)
src/widgets/exclusion_flow.rs:175 fn with_debug (assoc)
src/widgets/flex.rs:64 fn with_padding (assoc)
src/widgets/flex.rs:69 fn with_spacing (assoc)
src/widgets/flex.rs:89 fn with_align_items (assoc)
src/widgets/flex.rs:94 fn with_justify_content (assoc)
src/widgets/flex.rs:99 fn with_background (assoc)
src/widgets/gauge.rs:83 struct GaugeTickMarks
src/widgets/gauge.rs:110 enum GaugeFillStyle
src/widgets/gauge.rs:128 struct GaugeZone
src/widgets/gauge.rs:135 enum GaugeMode
src/widgets/gauge.rs:140 struct Gauge
src/widgets/gauge.rs:150 fn new (assoc)
src/widgets/gauge.rs:159 fn with_config (assoc)
src/widgets/groupbox.rs:35 fn new (assoc)
src/widgets/groupbox.rs:77 fn with_padding (assoc)
src/widgets/groupbox.rs:84 fn with_edge_padding (assoc)
src/widgets/groupbox.rs:90 fn with_border (assoc)
src/widgets/groupbox.rs:96 fn with_background (assoc)
src/widgets/groupbox.rs:101 fn with_label_color (assoc)
src/widgets/groupbox.rs:106 fn with_label_size (assoc)
src/widgets/keybind_editor.rs:87 struct KeybindEditor
src/widgets/keybind_editor.rs:124 fn new (assoc)
src/widgets/keybind_editor.rs:144 fn with_value (assoc)
src/widgets/keybind_editor.rs:149 fn with_placeholder (assoc)
src/widgets/keybind_editor.rs:154 fn with_capture_prompt (assoc)
src/widgets/keybind_editor.rs:159 fn with_conflict_check (assoc)
src/widgets/keybind_editor.rs:167 fn on_change (assoc)
src/widgets/keybind_editor.rs:182 fn value (assoc)
src/widgets/label.rs:36 fn new (assoc)
src/widgets/label.rs:54 fn with_size (assoc)
src/widgets/label.rs:59 fn with_color (assoc)
src/widgets/label.rs:64 fn set_text (assoc)
src/widgets/linear_gauge.rs:11 use crate
src/widgets/linear_gauge.rs:13 enum LinearGaugeOrientation
src/widgets/linear_gauge.rs:39 struct LinearGaugeConfig
src/widgets/linear_gauge.rs:94 struct LinearGauge
src/widgets/linear_gauge.rs:101 fn new (assoc)
src/widgets/linear_gauge.rs:110 fn with_value (assoc)
src/widgets/linear_gauge.rs:115 fn set_value (assoc)
src/widgets/list_view/mod.rs:52 use actions
src/widgets/list_view/mod.rs:53 use body
src/widgets/list_view/mod.rs:54 use cell_host
src/widgets/list_view/mod.rs:55 use cell_painter
src/widgets/list_view/mod.rs:56 use core
src/widgets/list_view/mod.rs:57 use footer
src/widgets/list_view/mod.rs:58 use list_view
src/widgets/list_view/mod.rs:63 use row_host
src/widgets/menu_bar.rs:135 struct MenuBar
src/widgets/menu_bar.rs:160 fn new (assoc)
src/widgets/menu_bar.rs:179 fn set_policy (assoc)
src/widgets/menu_bar.rs:248 fn is_open (assoc)
src/widgets/menu_bar.rs:252 fn open_title_index (assoc)
src/widgets/menu_bar.rs:685 fn new (assoc)
src/widgets/menu_surface.rs:49 fn new (assoc)
src/widgets/menu_surface.rs:402 fn handle_pointer_up (assoc)
src/widgets/menu_surface.rs:521 fn cached_popup_width (assoc)
src/widgets/menu_surface.rs:525 fn cached_popup_height (assoc)
src/widgets/navigator/mod.rs:6 use data_source
src/widgets/navigator/mod.rs:7 use item_painter
src/widgets/navigator/mod.rs:8 use navigator
src/widgets/navigator/body.rs:53 struct NavigatorBody
src/widgets/navigator/navigator.rs:54 enum GridAlignment
src/widgets/navigator/navigator.rs:217 fn details (assoc)
src/widgets/navigator/navigator.rs:227 fn list (assoc)
src/widgets/navigator/navigator.rs:237 fn small_icon (assoc)
src/widgets/navigator/navigator.rs:247 fn icon (assoc)
src/widgets/navigator/navigator.rs:250 fn large_icon (assoc)
src/widgets/navigator/navigator.rs:260 fn extra_large_icon (assoc)
src/widgets/navigator/navigator.rs:270 fn tile (assoc)
src/widgets/navigator/navigator.rs:280 fn content (assoc)
src/widgets/navigator/navigator.rs:312 enum NavigatorEvent
src/widgets/navigator/navigator.rs:423 struct Navigator
src/widgets/navigator/navigator.rs:432 fn new (assoc)
src/widgets/navigator/navigator.rs:463 fn with_config (assoc)
src/widgets/navigator/navigator.rs:468 fn with_policy (assoc)
src/widgets/notification_log.rs:29 struct NotificationLogWidget
src/widgets/notification_log.rs:39 fn new (assoc)
src/widgets/notification_log.rs:50 fn build_into (assoc)
src/widgets/notification_log.rs:157 fn flush_pending_actions (assoc)
src/widgets/notification_log.rs:192 fn mark_rebuild (assoc)
src/widgets/notification_log.rs:196 fn take_rebuild_flag (assoc)
src/widgets/order_book.rs:338 fn new (assoc)
src/widgets/order_book.rs:353 fn with_config (assoc)
src/widgets/order_book.rs:363 fn with_update_channel (assoc)
src/widgets/order_book.rs:374 fn set_asks (assoc)
src/widgets/order_book.rs:386 fn set_bids (assoc)
src/widgets/order_book.rs:398 fn set_book (assoc)
src/widgets/order_book.rs:420 fn set_last_price (assoc)
src/widgets/order_book.rs:426 fn with_asks (assoc)
src/widgets/order_book.rs:430 fn with_bids (assoc)
src/widgets/order_book.rs:434 fn with_last_price (assoc)
src/widgets/order_book.rs:439 fn take_events (assoc)
src/widgets/order_book.rs:443 fn mark_sweep (assoc)
src/widgets/order_book_body.rs:21 struct OrderBookBody
src/widgets/panel.rs:33 fn new (assoc)
src/widgets/panel.rs:65 fn with_margin (assoc)
src/widgets/panel.rs:70 fn with_padding (assoc)
src/widgets/panel.rs:75 fn with_border (assoc)
src/widgets/panel.rs:80 fn with_radius (assoc)
src/widgets/panel.rs:85 fn with_background (assoc)
src/widgets/panel.rs:90 fn with_title (assoc)
src/widgets/popup_menu.rs:16 struct PopupMenu
src/widgets/popup_menu.rs:23 fn new (assoc)
src/widgets/property_editor/mod.rs:22 use crate
src/widgets/property_editor/mod.rs:27 use crate
src/widgets/property_editor/mod.rs:28 use body
src/widgets/property_editor/mod.rs:29 use core
src/widgets/property_editor/mod.rs:30 use footer
src/widgets/property_editor/mod.rs:31 use policy
src/widgets/property_editor/mod.rs:36 use property_editor
src/widgets/property_editor/mod.rs:37 use types
src/widgets/property_editor/policy.rs:52 enum WrapBehavior
src/widgets/property_editor/policy.rs:58 enum TabRowEnd
src/widgets/property_editor/policy.rs:64 enum TabBoundary
src/widgets/property_editor/policy.rs:70 enum EditTrigger
src/widgets/property_editor/policy.rs:78 enum PointerDownAction
src/widgets/property_editor/policy.rs:84 enum HoverAction
src/widgets/property_editor/policy.rs:90 enum EditDisposition
src/widgets/property_editor/policy.rs:96 enum SectionCursorColumn
src/widgets/property_editor/policy.rs:102 enum HeaderClickAction
src/widgets/property_editor/policy.rs:108 enum EmptyAreaClickAction
src/widgets/range_slider.rs:30 enum RangeHandle
src/widgets/range_slider.rs:70 struct RangeValue
src/widgets/range_slider.rs:76 fn new (assoc)
src/widgets/range_slider.rs:91 struct RangeSlider
src/widgets/range_slider.rs:138 fn new (assoc)
src/widgets/range_slider.rs:178 fn with_step (assoc)
src/widgets/range_slider.rs:185 fn with_label (assoc)
src/widgets/range_slider.rs:190 fn with_anchors (assoc)
src/widgets/range_slider.rs:195 fn with_snap_threshold (assoc)
src/widgets/range_slider.rs:200 fn with_display_style (assoc)
src/widgets/range_slider.rs:205 fn on_change (assoc)
src/widgets/range_slider.rs:210 fn set_disabled (assoc)
src/widgets/range_slider.rs:214 fn value (assoc)
src/widgets/range_slider.rs:218 fn set_value (assoc)
src/widgets/readout.rs:74 struct ReadoutTextRegion
src/widgets/screen_host.rs:64 fn new (assoc)
src/widgets/scroll_area.rs:571 fn with_scrollbars (assoc)
src/widgets/scrollbar.rs:248 fn new (assoc)
src/widgets/scrollbar.rs:456 fn new (assoc)
src/widgets/section_header.rs:41 fn new (assoc)
src/widgets/section_header.rs:72 fn with_secondary (assoc)
src/widgets/section_surfaces.rs:36 enum SectionBandLevel
src/widgets/section_surfaces.rs:43 enum SectionBandMarker
src/widgets/section_surfaces.rs:78 struct SectionBandHeader
src/widgets/section_surfaces.rs:87 fn new (assoc)
src/widgets/section_surfaces.rs:97 fn with_level (assoc)
src/widgets/section_surfaces.rs:102 fn with_marker (assoc)
src/widgets/section_surfaces.rs:107 fn level (assoc)
src/widgets/section_surfaces.rs:111 fn marker (assoc)
src/widgets/section_surfaces.rs:121 struct SectionMetaHeader
src/widgets/section_surfaces.rs:131 fn new (assoc)
src/widgets/section_surfaces.rs:142 fn with_level (assoc)
src/widgets/section_surfaces.rs:147 fn with_marker (assoc)
src/widgets/section_surfaces.rs:152 fn level (assoc)
src/widgets/section_surfaces.rs:156 fn marker (assoc)
src/widgets/section_surfaces.rs:160 fn meta (assoc)
src/widgets/section_surfaces.rs:165 struct DisclosureSectionHeader
src/widgets/section_surfaces.rs:182 fn new (assoc)
src/widgets/section_surfaces.rs:200 fn on_toggle (assoc)
src/widgets/section_surfaces.rs:205 fn set_expanded (assoc)
src/widgets/section_surfaces.rs:209 fn expanded (assoc)
src/widgets/section_surfaces.rs:213 fn with_marker (assoc)
src/widgets/section_surfaces.rs:218 fn set_marker (assoc)
src/widgets/section_surfaces.rs:222 fn marker (assoc)
src/widgets/segmented_control.rs:22 struct Segment
src/widgets/segmented_control.rs:28 fn new (assoc)
src/widgets/segmented_control.rs:35 fn with_enabled (assoc)
src/widgets/segmented_control.rs:45 struct SegmentedControl
src/widgets/segmented_control.rs:69 fn new (assoc)
src/widgets/segmented_control.rs:93 fn on_change (assoc)
src/widgets/segmented_control.rs:104 fn with_equal_width (assoc)
src/widgets/segmented_control.rs:109 fn selected (assoc)
src/widgets/segmented_control.rs:113 fn set_selected (assoc)
src/widgets/separator_group.rs:22 struct SeparatorGroup
src/widgets/separator_group.rs:28 fn horizontal (assoc)
src/widgets/separator_group.rs:35 fn vertical (assoc)
src/widgets/sparkline.rs:17 struct SparklineConfig
src/widgets/sparkline.rs:44 struct Sparkline
src/widgets/sparkline.rs:51 fn new (assoc)
src/widgets/sparkline.rs:59 fn with_config (assoc)
src/widgets/sparkline.rs:64 fn with_data (assoc)
src/widgets/sparkline.rs:69 fn set_data (assoc)
src/widgets/split_pane.rs:73 fn horizontal (assoc)
src/widgets/split_pane.rs:93 fn vertical (assoc)
src/widgets/split_pane.rs:113 fn initial_ratio (assoc)
src/widgets/split_pane.rs:120 fn leading_min (assoc)
src/widgets/split_pane.rs:125 fn trailing_min (assoc)
src/widgets/split_pane.rs:130 fn on_ratio_changed (assoc)
src/widgets/switch.rs:104 struct Switch
src/widgets/tab_bar.rs:37 struct TabBar
src/widgets/tab_bar.rs:69 fn new (assoc)
src/widgets/tab_switcher.rs:26 struct TabSwitcher
src/widgets/tab_switcher.rs:32 fn new (assoc)
src/widgets/tabular_view/mod.rs:1760 fn hit_test_row_id (assoc)
src/widgets/tabular_view/mod.rs:2786 use crate
src/widgets/tabular_view/mod.rs:2787 use body
src/widgets/tabular_view/mod.rs:2788 use cell_content
src/widgets/tabular_view/mod.rs:2789 use column
src/widgets/tabular_view/mod.rs:2790 use geometry
src/widgets/tabular_view/mod.rs:2791 use header
src/widgets/tabular_view/mod.rs:2792 use intent_policy
src/widgets/tabular_view/mod.rs:2796 use intents
src/widgets/tabular_view/mod.rs:2801 use row
src/widgets/tabular_view/mod.rs:2802 use separators
src/widgets/tabular_view/mod.rs:2806 use sort
src/widgets/tabular_view/mod.rs:2807 use tree_paint
src/widgets/tabular_view/mod.rs:2809 use column_config
src/widgets/tabular_view/mod.rs:2810 use view_config
src/widgets/tabular_view/mod.rs:2818 use data_adapter
src/widgets/tabular_view/cell_lifecycle.rs:36 fn new (assoc)
src/widgets/tabular_view/column.rs:10 use crate
src/widgets/tabular_view/data_adapter.rs:144 fn new (assoc)
src/widgets/tabular_view/data_adapter.rs:148 fn as_str (assoc)
src/widgets/tabular_view/data_adapter.rs:158 fn new (assoc)
src/widgets/tabular_view/data_adapter.rs:162 fn as_str (assoc)
src/widgets/tabular_view/data_adapter.rs:184 fn new (assoc)
src/widgets/tabular_view/data_adapter.rs:201 fn baseline (assoc)
src/widgets/tabular_view/data_adapter.rs:278 fn request_id (assoc)
src/widgets/tabular_view/data_adapter.rs:302 fn exact (assoc)
src/widgets/tabular_view/data_adapter.rs:309 fn permits (assoc)
src/widgets/tabular_view/data_adapter.rs:313 fn is_allowed (assoc)
src/widgets/tabular_view/data_adapter.rs:335 fn end (assoc)
src/widgets/tabular_view/data_adapter.rs:339 fn row_at (assoc)
src/widgets/tabular_view/data_adapter.rs:727 fn sort_key_for_column (assoc)
src/widgets/tabular_view/data_adapter.rs:731 fn sort_capability (assoc)
src/widgets/tabular_view/data_adapter.rs:735 fn applied_sort (assoc)
src/widgets/tabular_view/geometry.rs:9 use crate
src/widgets/tabular_view/row.rs:107 fn new (assoc)
src/widgets/tag_input.rs:33 struct TagItem
src/widgets/tag_input.rs:41 enum TagInputPolicy
src/widgets/tag_input.rs:50 enum TagDisplayMode
src/widgets/tag_input.rs:122 struct TagInput
src/widgets/tag_input.rs:183 fn new (assoc)
src/widgets/tag_input.rs:226 fn with_items (assoc)
src/widgets/tag_input.rs:232 fn with_selection (assoc)
src/widgets/tag_input.rs:246 fn with_policy (assoc)
src/widgets/tag_input.rs:251 fn with_display_mode (assoc)
src/widgets/tag_input.rs:256 fn with_max_selections (assoc)
src/widgets/tag_input.rs:261 fn with_placeholder (assoc)
src/widgets/tag_input.rs:266 fn on_change (assoc)
src/widgets/tag_input.rs:271 fn with_tag_creation (assoc)
src/widgets/tag_input.rs:276 fn with_trigger_keys (assoc)
src/widgets/tag_input.rs:281 fn with_tag_validator (assoc)
src/widgets/tag_input.rs:286 fn with_allow_duplicate_tags (assoc)
src/widgets/tag_input.rs:293 fn set_items (assoc)
src/widgets/tag_input.rs:298 fn set_policy (assoc)
src/widgets/tag_input.rs:306 fn committed_ids (assoc)
src/widgets/tag_input.rs:319 fn committed_entries (assoc)
src/widgets/text_area.rs:166 fn new (assoc)
src/widgets/text_area.rs:179 fn is_composing (assoc)
src/widgets/text_area.rs:183 fn clear_composition (assoc)
src/widgets/text_area.rs:305 fn new (assoc)
src/widgets/text_area.rs:401 fn with_text (assoc)
src/widgets/text_area.rs:406 fn with_placeholder (assoc)
src/widgets/text_area.rs:411 fn text (assoc)
src/widgets/text_block.rs:40 fn new (assoc)
src/widgets/text_block.rs:56 fn with_size (assoc)
src/widgets/text_block.rs:62 fn with_color (assoc)
src/widgets/text_block.rs:67 fn with_align (assoc)
src/widgets/text_block.rs:72 fn with_wrap (assoc)
src/widgets/text_block.rs:167 fn set_selection_policy (assoc)
src/widgets/text_field.rs:125 fn new (assoc)
src/widgets/text_field.rs:228 fn new (assoc)
src/widgets/text_field.rs:265 fn set_on_submit (assoc)
src/widgets/text_field.rs:269 fn set_on_cancel (assoc)
src/widgets/text_field.rs:350 fn with_text (assoc)
src/widgets/text_field.rs:382 fn with_placeholder (assoc)
src/widgets/text_field.rs:404 fn text (assoc)
src/widgets/time_picker.rs:15 use crate
src/widgets/time_picker.rs:19 enum TimeDisplayMode
src/widgets/time_picker.rs:53 fn full (assoc)
src/widgets/time_picker.rs:63 fn working_hours (assoc)
src/widgets/time_picker.rs:73 fn custom (assoc)
src/widgets/time_picker.rs:90 struct TimePicker
src/widgets/time_picker.rs:125 fn new (assoc)
src/widgets/time_picker.rs:147 fn with_display_mode (assoc)
src/widgets/time_picker.rs:152 fn with_seconds (assoc)
src/widgets/time_picker.rs:157 fn with_embedded (assoc)
src/widgets/time_picker.rs:162 fn with_domain (assoc)
src/widgets/time_picker.rs:167 fn with_step_minutes (assoc)
src/widgets/time_picker.rs:172 fn with_step_seconds (assoc)
src/widgets/time_picker.rs:177 fn with_edit_policy (assoc)
src/widgets/time_picker.rs:182 fn on_commit (assoc)
src/widgets/time_picker.rs:187 fn value (assoc)
src/widgets/time_picker.rs:191 fn working (assoc)
src/widgets/time_picker.rs:195 fn set_value (assoc)
src/widgets/timeline_canvas.rs:30 use crate
src/widgets/timeline_canvas.rs:198 struct TimelineCanvas
src/widgets/timeline_canvas.rs:214 fn new (assoc)
src/widgets/timeline_canvas.rs:227 fn with_policy (assoc)
src/widgets/timeline_canvas.rs:232 fn policy (assoc)
src/widgets/timeline_header.rs:26 struct TimelineHeader
src/widgets/timeline_header.rs:32 fn new (assoc)
src/widgets/timeline_spatial.rs:118 fn from_snapshot (assoc)
src/widgets/timeline_spatial.rs:126 fn key_for (assoc)
src/widgets/timeline_spatial.rs:132 fn new (assoc)
src/widgets/timeline_spatial.rs:154 fn with_derived_revision (assoc)
src/widgets/timeline_spatial.rs:184 fn context (assoc)
src/widgets/toast.rs:118 struct ToastWidget
src/widgets/toast.rs:139 fn new (assoc)
src/widgets/tooltip_surface.rs:31 struct TooltipSurface
src/widgets/tooltip_surface.rs:39 fn new (assoc)
src/widgets/tree_view/mod.rs:23 use body
src/widgets/tree_view/mod.rs:30 use footer
src/widgets/tree_view/mod.rs:31 use tree_view
```
