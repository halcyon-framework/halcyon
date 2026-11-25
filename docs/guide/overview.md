---
doc-id: halcyon-guide-overview
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-001
  - SPEC-HAL-002
  - SPEC-HAL-003
  - SPEC-HAL-018
  - SPEC-HAL-042
  - SPEC-HAL-354
---

# Halcyon — Architecture Overview

Everything below is a statement about revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` (halcyon `0.189.3-beta`,
`Cargo.toml:1`). Claims carry two axes — Authority
(`binding`/`draft-intent`/`none`/`contradictory`) and Implementation
(`implemented`/`partial`/`absent`/`unknown`/`not-applicable`) — per
SPEC-HAL-309 §7. Four files were dirty in the working tree when this document
was verified (`examples/showcase/tab_plot.rs`, `src/tests/market_detail.rs`,
`src/widgets/aligned_order_book.rs`, `src/widgets/mod.rs`); nothing in this
document cites them.

## What Halcyon Is

Halcyon is a GPU-first, retained-mode desktop UI framework in Rust, built as
a single crate with organized module hierarchy
[authority: binding — SPEC-HAL-003:5 (`Status: Implemented`), SPEC-HAL-003:13 |
implementation: implemented — `src/lib.rs:3-16`].

- Rendering is wgpu-based: the production path constructs a `WgpuRenderer`
  and a window surface at startup [implementation: implemented —
  `src/core/app.rs:9126-9133`, symbol `App::run`].
- The UI is a persistent `WidgetTree` owned by the engine, with ID-based
  identity — nodes are addressed by `NodeId`/`TreeWidgetId`, never by
  references [authority: binding — SPEC-HAL-002 Pillar C |
  implementation: implemented — `src/core/widget_tree.rs:205-251`].
- Every UI element implements the `Widget` trait: `layout` →
  `LayoutResult` (final size + natural `content_size`, SPEC-HAL-086 Stage 2),
  `paint` into the scene, plus event, focus, capture, gesture, drag/drop,
  update, and authority hooks [implementation: implemented —
  `src/core/widget/mod.rs:2058-2765`].

The Constitution is supreme: "If any SPEC or component contradicts this
document, this document wins"
[authority: binding — SPEC-HAL-002:26]. Its pillars: retained-mode
ownership, explicit phase separation, identity-through-ID, pure data in /
explicit actions out, GPU-first rendering, engine sovereignty
(SPEC-HAL-002 §2). Nothing in `docs/guide/` overrides any of that.

## The Engine Assembly

`App` is a deliberately thin public facade — it holds an `AppConfig` and
exposes `new` + `run<S: Screen>` [implementation: implemented —
`src/core/app.rs:9060-9074`]. `App::run` assembles the entire engine before
entering the event loop [implementation: implemented — `src/core/app.rs:9115-9210`]:

| Component | Role | Evidence |
|---|---|---|
| winit event loop + `Window` | OS transport | `src/core/app.rs:9115-9123` |
| `WgpuRenderer` + wgpu surface | GPU device, text engine, render targets | `src/core/app.rs:9126-9133` (SPEC-HAL-081B) |
| `DrawPlanBuilder<MultiSurfaceContract>` | Build phase: SceneGraph → DrawPlan | `src/core/app.rs:9135-9137` (SPEC-HAL-084) |
| `RetainedSceneCache` + recycled scene storage | Retained scene fragments | `src/core/app.rs:9138-9139` (SPEC-HAL-354) |
| `WidgetTree` rooted at `ScreenHost` | Single retained tree, primary window | `src/core/app.rs:9143-9149` |
| `AppContext` + `AppRoot` | Theme, GPU resources, screens/modals/navigation | `src/core/app.rs:9152-9156` |
| Menu host (macOS native or in-window) | SPEC-HAL-333 Class G projection | `src/core/app.rs:9160-9192` |
| `EventRouter`, `InteractionDomains`, `LifecycleRegistry`, `InputFusion` | Routing, per-window interaction state (SPEC-HAL-151), lifecycle (SPEC-HAL-162), OS-input fusion | `src/core/app.rs:9199-9208` |
| `EngineTicker` | Frame timing authority (SPEC-HAL-042/043) | `src/core/app.rs:9209`, `src/core/app.rs:739-752` |

A consumer supplies a `Screen` implementation; `AppRoot` owns the screen
stack and modal stack and pushes the initial screen before the first frame
[implementation: implemented — `src/core/app.rs:9194-9197`].

## The Frame, In One Paragraph

The frame phase order — intake, deferred mutations, rebuild, layout, scene
build, GPU render — is constitutional law
[authority: binding — SPEC-HAL-002 §5]. SPEC-HAL-354 refines it: a retained
frame never reorders phases, it lets a phase prove that no work is required
[authority: binding — SPEC-HAL-354:12 (`Status: Binding`),
SPEC-HAL-354:192-193]. Concretely: data arrival never owns frame cadence
(`INV-354-AUTH-1`, SPEC-HAL-354:54-55); update code classifies each change
as layout-affecting, subtree-paint, or paint-only
[implementation: implemented — `src/core/app.rs:1261-1276`, symbol
`update_widget_recursive`]; an unchanged frame is not presented
[implementation: implemented — `src/core/app.rs:11193-11194`,
`should_present = scene_outcome.visual_changed || …`]. The full lifecycle,
with every decision owner cited, is in
`runtime-and-frame-lifecycle.md`.

## Module Map (live, `src/lib.rs:3-16`)

| Module | Purpose | Evidence |
|---|---|---|
| `core` | App/frame machinery, widget trait + tree, layout, events, scene, theme, text engine, scheduler | `src/lib.rs:5`, `src/core/app.rs`, `src/core/widget/mod.rs:2058` |
| `widgets` | The widget corpus (catalog pending; its inventory file `src/widgets/mod.rs` is dirty in the current working tree and is not cited here) | `src/lib.rs:16` |
| `renderer` | wgpu renderer, DrawPlan build phase | `src/lib.rs:12`, `src/core/app.rs:9080-9081` |
| `compositor` | Surface contracts; Multi-Window Architecture layer, procedurally CLOSED as of 2026-01-21 — modification requires explicit SPEC amendment; must not hit-test (INV-COMP-N11) | `src/compositor/mod.rs:1-25` |
| `platform` | OS windowing/font/menu integration (transport, not semantic authority) | `src/lib.rs:10`, `src/core/app.rs:9077-9079`, `src/core/app.rs:9160-9174` |
| `plot` | SPEC-HAL-296 coordinate-authority rendering surface; own GPU pipeline via offscreen texture; a peer to `core`/`widgets`/`renderer`, not inside `widgets` | `src/plot/mod.rs:1-8` |
| `engines` | SPEC-HAL-255 extracted structural engines (cell mechanics/source, column geometry, hierarchical, route geometry, row topology, temporal, virtualization); no widget, layout, or rendering dependencies | `src/engines/mod.rs:1-17` |
| `model` | UI-agnostic data structures and controllers (order book, tabular, menus, forms, …) | `src/model/mod.rs:1-15` |
| `runtime` | SPEC-HAL-020 action system: actions, binding registry, builtins | `src/runtime/mod.rs:1-11` |
| `agent` | SPEC-HAL-318 "Tower" agent orchestration: bridges a running app to an LLM via the agent WebSocket, supervised via MCP | `src/agent/mod.rs:1-14` |
| `oracles` | SPEC-HAL-188 reference data-adapter implementations for tests/examples/demos | `src/oracles/mod.rs:1-15` |
| `semantics` | Hierarchy semantics | `src/semantics/mod.rs:1` |
| `editing` | `EditorBridge` | `src/editing/mod.rs:1-3` |
| `utils` | IDs, math primitives | `src/lib.rs:15`, `src/lib.rs:36` |
| `compositor`/`renderer` split | Compositor owns surface topology; renderer owns GPU execution | `src/compositor/mod.rs:16-25` |

`src/tests/` is the internal test module (`src/lib.rs:18-19`).

## Public Entry Surface

The `prelude` re-exports the consumer-facing core: `core::app`, `core::event`,
`core::layout`, `core::math`, `core::scene`, `core::scheduler`,
`core::text_engine`, `core::theme`, `core::widget`, `core::widget_tree`,
platform window/font engine, `wgpu_renderer`, and the Tier-1 widget set
(button, container, divider, flex, label, panel, scroll area, text field, …)
[implementation: implemented — `src/lib.rs:21-58`].

Minimal consumer shape (constructor-level; the runnable version lives in
`getting-started.md`): implement `Screen` (`src/core/app.rs:4785-4909`),
then `App::new(config).run(screen)` (`src/core/app.rs:9066-9074`).

## Authority Map For Readers

- Binding architecture: `docs/specs/` — start at SPEC-HAL-002 (Constitution),
  SPEC-HAL-001 (core architecture, "central authoritative overview that all
  later SPECs refine", SPEC-HAL-001:36).
- SPEC statuses differ and matter: e.g. SPEC-HAL-003 declares `Implemented`
  (SPEC-HAL-003:5), SPEC-HAL-354 declares `Binding` (SPEC-HAL-354:12), while
  SPEC-HAL-004, SPEC-HAL-005, and SPEC-HAL-018 declare `Draft`
  (SPEC-HAL-004:5, SPEC-HAL-005:5, SPEC-HAL-018:6) — Draft text is intent,
  and the live code diverges from parts of it (recorded in
  `runtime-and-frame-lifecycle.md` §Contradictions).
- Implemented truth: source, tests, compiler output at this revision.
- This guide corpus: explanatory only; registry in `index.md`.
