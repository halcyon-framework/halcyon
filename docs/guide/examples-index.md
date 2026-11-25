---
doc-id: halcyon-guide-examples-index
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-000
  - SPEC-HAL-309
---

# Examples Index

## Current beginner entry points (2026-11-25)

The Halcyon `0.270.3-beta` source tree exposes **109 Cargo examples**. The 2026-11-24 inventory recorded 119 targets before ten older examples were retired. The three introductory examples below were added after the July 2026 inventory. Their source paths and Cargo target discovery were checked, and all three linked in the focused native build.

| Target | Purpose |
| --- | --- |
| [`hello_world`](../../examples/hello_world.rs) | Minimal ordinary `App`/`Screen`: greeting, Button, and application-owned counter reflected in a mounted Label |
| [`listview_basic`](../../examples/listview_basic.rs) | Read-only three-column ListView with 12 stable rows and revision-qualified, bounded provider windows |
| [`treeview_basic`](../../examples/treeview_basic.rs) | Nine-node project tree with one Name column, root expanded and standard TreeView interaction |

Run these introductory examples from the Halcyon source root.

```sh
cargo run --locked -p halcyon --example hello_world
cargo run --locked -p halcyon --example listview_basic
cargo run --locked -p halcyon --example treeview_basic
```

See [Showcase](../../examples/showcase/README.md) for tab navigation and modal overlays.

For same-surface Z order, run `cargo run --locked -p halcyon --example compositor_surface_order`. Its tests cover the two- and three-rectangle paint cases. [Source](../../examples/compositor_surface_order.rs)

The detailed inventory below is a **historical snapshot**, pinned to 2026-07-24. Its counts and individual build failures must not be read as current diagnostics.

## Historical examples inventory (2026-07-24)

Complete per-file documentation of `examples/` at revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`: 99 top-level example files
plus two directory examples (`showcase/`, `rfc018_demo/`) — inventory
taken from the live file listing; every file below was mechanically
extracted (header comment, SPEC references, structural pattern,
principal types) and the extraction spot-verified against disk.

How to run any of them:

```text
cargo run --example <name>          # e.g. cargo run --example checkbox
cargo run --example showcase        # the tabbed showcase app
```

Method and honesty rules:

- **Purpose lines derive from each file's own leading header comment**
  (cited `:1-k`); they are compressed, not invented. Line counts from
  `wc -l`. Principal APIs from the file's imports and constructor calls.
- **Build status verified 2026-07-24** at a clean scratch-worktree
  checkout of this revision: `cargo build --examples --keep-going` —
  **99 of 101 targets build**. ✗ marks the two failures:
  `navigation_host` (E0382, use of partially moved value `intent`) and
  `shape_primitives` (E0004, non-exhaustive match missing
  `SceneNode::RetainedFragment(_)`). Unmarked entries build. Full
  record: `audits/implementation-status.md` §3. A build is not a
  runtime capture; no example was executed for this index.
- Pattern legend: **A/S** = App/Screen pattern (`impl Screen for …`);
  **winit** = hand-rolled winit event loop; **headless** = no window
  (layout/scene/shaping harness); **ws-client** = WebSocket client that
  drives another running example.
- `examples/showcase/tab_plot.rs` is **dirty in the current working
  tree** (part of the unsettled market-detail unit) — the showcase
  entry describes the committed revision; the dirty file is excluded
  from claims.
- Examples demonstrate intended use; they do not prove general behavior
  [authority: binding — SPEC-HAL-309 §2.6].

## Canonical Pattern

- `checkbox.rs` (141 ln; SPEC-HAL-111/208/018; A/S) — Checkbox with
  integrated labels and toggle callback (`:1-3`); the basis of
  `getting-started.md`. APIs: App, AppConfig, Screen, Checkbox, Label.
- `tutorial.rs` (229 ln; SPEC-HAL-227/018; A/S) — 3-step linear
  tutorial, signal-driven advancement, explicit dismissal, no timing
  behavior; TooltipSurface as dumb renderer (`:1-9`). APIs:
  TutorialController, TutorialDefinition, TutorialStep, TooltipSurface.

## Application Shell / Navigation

- `compose_dashboard.rs` (1332 ln; SPEC-HAL-319; ws-client) — DevOps
  dashboard composition script exercising ALL widget types, SplitPane,
  TabLayout, FormSection assemblies via JSON `add_widget` commands
  (`:1-18`). APIs: tungstenite only; drives widgets by type-name
  strings. Companion: run `wireframe` first.
- `compose_settings.rs` (753 ln; SPEC-HAL-319; ws-client) — builds a
  Settings screen entirely through agent composition commands (`:1-8`).
  Companion: `wireframe`.
- `designer.rs` (245 ln; SPEC-HAL-319; ws-client) — interactive
  composition REPL: AddWidget / ConfigureWidget / RemoveWidget /
  SetHint / Snapshot from the terminal (`:1-8`). Companion: `wireframe`
  or any app started `with_agent_server(...)`.
- `multi_window.rs` (364 ln; SPEC-HAL-081B/163; winit) — multi-window
  truth test: two real OS windows over `MultiSurfaceContract` (`:1-8`).
  APIs: Compositor, MultiSurfaceContract, ResizeSettlingManager,
  DrawPlanBuilder, WgpuRenderer. Note: `#![allow(deprecated)]` splits
  its doc block at `:10`.
- ✗ `navigation_host.rs` (186 ln; SPEC-HAL-228/018; A/S) —
  NavigationHost as sole navigation state authority; intent-driven
  NavigateTo/NavigateUp; deterministic state projection (`:1-9`). APIs:
  NavigationHost, NavigationIntent, NavigationNodeId. **Fails to build
  at this revision (E0382, partially moved `intent`) — pre-existing.**
- `navigator.rs` (258 ln; no SPEC refs; A/S) — content browser over
  ~1M mock items; in-file `MockGridSource` implements `DataSource`
  (`:1`). APIs: Navigator, NavigatorConfig, NavigatorPolicy,
  NavigatorViewMode, ItemPainter.
- `node_graph.rs` (1750 ln; SPEC-HAL-293/294; A/S) — node graph model
  + node flow surface demo (`:1-4`); largest example file. APIs:
  NodeGraphDocument, NodeEditorSession, NodeFlowView,
  NodeGraphProjection, NodeTypeCatalog.
- `theme_toggle.rs` (240 ln; no SPEC refs; A/S) — runtime dark/light
  theme switching via FormBinder; F5 toggles (`:1-3`). APIs: Button,
  Checkbox, ProgressBar, RadioGroup, FormBinder, Theme.
- `wireframe.rs` (89 ln; SPEC-HAL-319; A/S) — blank design surface for
  agent composition; agent server on :9317 (`:1-8`). APIs: App,
  AppConfig `with_agent_server`.
- `card.rs` (529 ln; no SPEC refs; A/S) — `Panel::card()`
  configurations beside GroupBox for visual comparison; static content
  (`:1-3`). APIs: Panel, GroupBox, ScrollArea.
- `depth.rs` (254 ln; no SPEC refs; A/S) — candle chart + order-book
  depth sidebar sharing one price Y axis via SplitPane (`:1-4`). APIs:
  Plot, CandleSeries, DepthBarSeries, SplitPane, PlotRenderPool.

## Widget Basics

- `breadcrumbs.rs` (319 ln; SPEC-HAL-220/228/018; A/S) — Breadcrumbs as
  pure projection of NavigationHost state; crumb click → NavigateTo
  intent; disabled-crumb and dropdown behavior (`:1-11`). APIs:
  Breadcrumbs, NavigationHost, NavigationIntent, BreadcrumbsSink.
- `busy_indicator.rs` (114 ln; SPEC-HAL-211/018; A/S) — indeterminate
  busy indicator, frame-driven animation, labels, disabled state
  (`:1-4`). APIs: BusyIndicator, Label.
- `button_hover_animation.rs` (118 ln; SPEC-HAL-021; A/S) — hover and
  pressed animations through the capability injection pipeline; single
  button (`:1-3`). APIs: Button.
- `collapsible_section.rs` (139 ln; SPEC-HAL-215/018; A/S) —
  CollapsibleSection demo, initialized expanded (`:1-2`). APIs:
  CollapsibleSection, SectionHeader, Divider.
- `color_picker.rs` (111 ln; SPEC-HAL-272; A/S) — interactive HSV
  picker; swatch and hex label update live (`:1-2`). APIs: ColorPicker.
- `color_picker_field.rs` (95 ln; SPEC-HAL-272; A/S) — compact swatch
  trigger opening a popup picker (`:1-2`). APIs: ColorPickerField.
- `combo_box.rs` (212 ln; no SPEC refs; A/S) — ComboBox harness:
  popup open/close, highlight navigation, commit paths, type-to-search,
  strict resolver, debounced async search; 100 000-item stress
  (`:1-12`). APIs: ComboBox, DefaultSelectionResolver,
  DebouncedSearchHarness, ResolverHandle.
- `context_menu.rs` (214 ln; SPEC-HAL-217/018; A/S) — right-click
  context menu at pointer; new handle + sink per invocation; selection
  via external sink (`:1-8`). APIs: ContextMenuHandle, SelectionSink,
  Anchor, MenuItem.
- `date_picker.rs` (300 ln; SPEC-HAL-238; A/S) — inline date picker;
  four selections wired through FormBinder (`:1`). APIs:
  DatePickerWidget, DatePickerField, DatePolicy, DateValue, FormBinder.
- `dialog.rs` (234 ln; no SPEC refs; A/S) — modal Dialog in three
  configurations tracked by `Option<DialogHandle>` (`:1`). APIs:
  Dialog, DialogConfig, DialogButton, DialogHandle.
- `divider.rs` (130 ln; SPEC-HAL-213/018; A/S) — horizontal and
  vertical dividers as inert separators (`:1-3`). APIs: Divider.
- `dropdown_menu.rs` (202 ln; SPEC-HAL-218/018; A/S) — trigger button
  opens anchored action menu; external selection sink; trigger never
  displays last selection (INV-D3) (`:1-8`). APIs: DropdownMenu,
  DropdownMenuHandle, SelectionSink, MenuItem.
- `gauge.rs` (356 ln; no SPEC refs; A/S) — four gauges with
  spring-damped tracking; in-file JumpGenerator targets (`:1`). APIs:
  Gauge, GaugeConfig, GaugeFillStyle, GaugeMode, GaugeZone.
- `icon_gallery.rs` (160 ln; SPEC-HAL-265; A/S) — built-in icons at
  three sizes for visual geometry verification (`:1-6`). APIs:
  IconRegistry, IconId, IconSize, builtin_registry. Recorded
  observation: header says "all 10 built-in icons" (`:3`) while the
  runtime println says 12 (`:22`) — count claim internally
  inconsistent; not resolved here.
- `linear_gauge.rs` (409 ln; no SPEC refs; A/S) — horizontal +
  vertical bar gauges with spring-damped tracking; in-file
  JumpGenerator (`:1`). APIs: LinearGauge, LinearGaugeConfig,
  LinearGaugeOrientation, GaugeFillStyle, GaugeZone, TickSide.
- `keybind_editor.rs` (236 ln; SPEC-HAL-329/018; A/S) — single-stroke
  capture editors demonstrating the §6.5 displacement pattern across
  three editors; Demo Law: never consumes Escape (`:1-12`). APIs:
  KeybindEditor, ConflictInfo, Shortcut.
- `notification.rs` (397 ln; SPEC-HAL-269/270; A/S) — toast stack +
  notification log; four push_toast sites (`:1`). APIs: FormBinder,
  NotificationLevel, ToastAction.
- `popup_menu.rs` (208 ln; SPEC-HAL-216/018; A/S) — stateless reopen,
  items injected per invocation, selection via external sink (`:1-7`).
  APIs: PopupMenu, PopupMenuHandle, MenuItem, SelectionSink.
- `progress_bar.rs` (129 ln; SPEC-HAL-211/018; A/S) — determinate
  progress bar with external value updates, labels, disabled state
  (`:1-4`). APIs: ProgressBar.
- `radio_button.rs` (196 ln; SPEC-HAL-130/131/208/018; A/S) —
  RadioGroup with integrated labels and selection callback (`:1-3`).
  APIs: RadioGroup, RadioId, RadioGroupId.
- `range_slider.rs` (129 ln; SPEC-HAL-275; A/S) — two-handle range with
  magnetic snap (25/50/75) plus stepped variant (`:1`). APIs:
  RangeSlider, TrackDisplayStyle.
- `section_header.rs` (170 ln; SPEC-HAL-214/018; A/S) — SectionHeader
  demo (`:1-2`). APIs: SectionHeader, Divider.
- `section_surfaces.rs` (576 ln; SPEC-HAL-351/018; A/S) — section
  surface family (DisclosureSectionHeader, SectionBandHeader,
  SectionMetaHeader) with externally owned disclosure state (`:1-2`).
  APIs: SectionHeader family, Panel, ScrollArea.
- `segmented_control.rs` (165 ln; SPEC-HAL-274; A/S) — segmented
  control incl. equal-width mode and a disabled segment (`:1`). APIs:
  Segment, SegmentedControl.
- `shortcut.rs` (292 ln; SPEC-HAL-033 + engine SPECs; winit) — keyboard
  shortcut registration/detection on a hand-rolled winit loop with
  `run_single_frame` engine driving (`:1-2`). APIs: EventRouter,
  InputFusion, KeyChord, ShortcutScope, Compositor.
- `slider.rs` (161 ln; SPEC-HAL-210/018; A/S) — Slider with continuous
  updates, stepping, value display, FormBinder, disabled state
  (`:1-4`). APIs: Slider, FormBinder.
- `sparkline.rs` (125 ln; SPEC-HAL-308; A/S) — miniature inline charts
  with filled-area rendering (`:1-2`). APIs: Sparkline,
  SparklineConfig.
- `spin_box.rs` (158 ln; SPEC-HAL-212/018; A/S) — discrete numeric
  editor: increment/decrement, bounds, FormBinder, disabled state
  (`:1-4`). APIs: SpinBox, FormBinder.
- `split_pane.rs` (219 ln; SPEC-HAL-267; A/S) — horizontal and vertical
  split panes: drag resize, keyboard navigation, double-click reset,
  min constraints, nesting (`:1-3`). APIs: SplitPane, ScrollArea.
- `switch.rs` (163 ln; SPEC-HAL-209/208/018; A/S) — Switch with
  integrated labels, toggle callback, binding (`:1-3`). APIs: Switch,
  FormBinder.
- `tab_bar.rs` (161 ln; SPEC-HAL-219/018; A/S) — pure navigation
  selector: 4 tabs (one disabled), external active-tab tracking, sink
  emission, keyboard navigation, no content switching (INV-A1)
  (`:1-9`). APIs: TabBar, TabId, TabBarSink.
- `tag_input.rs` (286 ln; SPEC-HAL-276; A/S) — TagInput in Full,
  CountBadge, and ToggleOnly modes (`:1-2`). APIs: TagInput,
  TagCreationMode, TagDisplayMode, TagInputPolicy, TagItem.
- `time_picker.rs` (160 ln; SPEC-HAL-273; A/S) — standalone TimePicker
  with 24h, 12h, seconds, and working-hours modes (`:1-2`). APIs:
  TimePicker, TimeDisplayMode, TimeDomain, TimeValue. (Omitted from
  this index's family listing before this revision — defect fixed in
  the 2026-07-24 mechanical pass.)
- `toast.rs` (170 ln; SPEC-HAL-269; A/S) — toast notification system
  demo (`:1`). APIs: ToastAction, FormBinder.
- `tooltip.rs` (202 ln; SPEC-HAL-226A/018; A/S) — hover tooltips with
  framework-managed 500 ms delay, instant dismiss, reshow shortcut,
  auto-placement (`:1-10`). APIs: TooltipPayload, Divider.

## Text

- `console.rs` (549 ln; SPEC-HAL-305; A/S) — scrolling log console with
  burst generator (`:1-2`). APIs: Console, ConsoleLine, ConsoleSpan,
  ConsoleStyle, ScrollArea.
- `eventlog.rs` (267 ln; SPEC-HAL-312; A/S) — EventLog composing
  ListView: 4 columns (Severity pinned), SeverityPainter, selection,
  Ctrl+C structured copy, 200 entries across 6 levels (`:1-11`). APIs:
  EventLog, EventLogConfig, EventLogEntry, EventLogLevel.
- `shaping.rs` (177 ln; SPEC-HAL-050; headless) — shapes four samples per
  mode with the bundled Noto fonts. Run from the workspace root.
  Select `arabic` (default), `hebrew`, `devanagari`, or `latin` after `--`.
  Empty output or glyph ID zero yields exit status 1.
  Uses TextShaper, ShapedCluster, and TextDirection.
- `text_area.rs` (172 ln; SPEC-HAL-108/018; A/S) — multi-line editing,
  vertical scrolling, cross-line selection (`:1-3`). APIs: TextArea.
- `text_field.rs` (109 ln; SPEC-HAL-101/018; A/S) — caret timing via
  EngineTicker, vertical alignment; no selection/editing (`:1-4`).
  APIs: TextField, Label, Flex.
- `text_placement_benchmark.rs` (882 ln; SPEC-HAL-092/090; A/S) —
  typography/control-alignment benchmark for
  `compute_control_text_baseline_y` (INV-TYPO-CTRL-1/2); D toggles
  debug overlay; DPI stress instructions (`:1-19`). APIs: Label,
  RoundedCorners, Color.
- `text_quality_test.rs` (68 ln; SPEC-HAL-018/011; A/S) — visual font
  debugging: 50 pangram lines, sizes 6-55 (`:1-3`). APIs: TextBlock,
  Label.
- `textblock_showcase.rs` (124 ln; SPEC-HAL-025/018; A/S) — multi-line
  wrapping, paragraphs, alignment (`:1-3`). APIs: TextBlock, Label.

## Layout

- `flow_grid_groupbox.rs` (380 ln; SPEC-HAL-018; A/S) — GroupBox
  containing FlowGrids and nested GroupBoxes (`:1-3`). APIs: GroupBox,
  FlowGrid, Container, ScrollArea. Note: static TRACKED_BUTTONS debug
  probe.
- `flow_grid_layouts.rs` (645 ln; SPEC-HAL-018; A/S) — five distinct
  side-by-side / wrapping FlowGrid arrangements (`:1-3`). APIs:
  FlowGrid, Container, ScrollArea.
- `flow_grid_megatest.rs` (447 ln; SPEC-HAL-018; A/S) — FlowGrid +
  Container constraint-behavior combinations, five sections (`:1-3`).
  APIs: FlowGrid, Container.
- `flow_grid_stress_test.rs` (133 ln; SPEC-HAL-018/088; A/S) — 100
  buttons in a responsive wrapping grid; prints theme geometry
  instrumentation (`:1-3`). APIs: FlowGrid, ScrollArea, Button, Theme.
- `layout_transition_basic.rs` (443 ln; SPEC-HAL-078; A/S) — layout
  transitions by toggling sizes: basic, nested, and
  scroll-conflict-denied cases (`:1-12`). APIs: ScrollArea + in-file
  CollapsiblePanel/ToggleButton.
- `list.rs` (118 ln; SPEC-HAL-094/096C; A/S) — List as
  TabularSubstrate-backed list with v1 interaction semantics;
  1 000 000 items (`:1-4`). APIs: List, ScrollArea, SelectionMode.

## Tabular / Data

- `listview.rs` (934 ln; SPEC-HAL-179/179A/353; A/S) — the ListView
  authoritative demo: 5 columns, mandatory header, 1 000 000-row
  bounded projection, hit-testing/selection, header/body lockstep
  horizontal scroll, provider-authoritative multi-column sorting,
  durable sort restoration via in-file JsonFileViewConfigStore
  (`:1-27`). APIs: ListView, Column, ScrollArea, SortDirection,
  ViewConfigStore, HorizontalScrollLink.
- `order_book.rs` (229 ln; no SPEC refs; A/S) — bid/ask ladder with
  live mock data from a background crossbeam ticker thread, 30
  levels/side (`:1`). APIs: OrderBook, OrderBookConfig,
  OrderBookUpdate, BookLevel, BookSide.
- `property_editor.rs` (751 ln; SPEC-HAL-260A + editor SPECs; A/S) —
  PropertyEditor v2 multi-column editing proof: every PropertyValue
  archetype across four columns, mixed-type cells, per-cell ReadOnly
  dimming, archetype→editor bindings (TextField/SpinBox/Checkbox/
  ComboBox/ColorPickerField/KeybindEditor) (`:1-76`). APIs:
  PropertyEditor, PropertyDataSource, CellSource, PropertyValue,
  PropertyNode.
- `signal_dialog.rs` (553 ln; SPEC-HAL-161/162/163; winit) — real
  cross-window signal semantics: two winit windows (parent + edit
  dialog), explicit window-to-window addressing over SignalBus
  (`:1-7`). APIs: SignalBus, SignalEnvelope, SignalPayload,
  LifecycleRegistry, Compositor.
- `signals.rs` (352 ln; SPEC-HAL-031; A/S) — targeted widget
  invalidation via SignalStore: one counter signal drives three
  widgets without full rebuild (`:1-4`). APIs: SignalHandle,
  FormBinder.
- `span_tree.rs` (610 ln; SPEC-HAL-341/335; A/S) — synthetic CI-trace
  drill-in over RootedContainmentSource: one containment level
  rendered read-only, double-click enters a span, zoom-out pops; 22
  runs × 147 spans = 3 234 total (`:1-23`). APIs: TimelineCanvas,
  ContainmentSource, TimeGeometryEngine, TemporalCanvasPolicy.
- `treeview.rs` (266 ln; SPEC-HAL-259; A/S) — TreeView stress: 5 000
  nodes, depth ≤6, random expansion; proves no O(n²) paint, no resolve
  panic, expand/collapse and scroll at scale (`:1-14`). APIs: TreeView,
  Column, ColumnModel, NodeMeta, ScrollArea.

## Plot / Temporal

- `plot.rs` (709 ln; SPEC-HAL-296; winit) — direct-surface Plot
  rendering: CandleSeries straight to the window surface, no offscreen
  texture; 2M candles; pan/zoom/box-zoom (`:1-8`). APIs: CandleSeries,
  RenderCandle, Transform, ViewState.
- `plot_dashboard.rs` (481 ln; SPEC-HAL-296; A/S) — 20 Plot widgets in
  a 5×4 grid, 100K candles each, shared PlotRenderPool; threaded
  rendering stress (`:1-3`). APIs: Plot, PlotRenderPool, AxisAuthority,
  Probe, SplitPane.
- `plot_live.rs` (698 ln; no SPEC refs; A/S) — real-time candle
  formation over a data channel: 200-candle history, 50 ms ticker, 2 s
  candle open, EMA-20 recompute, drain in `Widget::update()` (`:1-6`).
  APIs: Plot, CandleSeries, BarSeries, LineSeries, DataUpdate,
  PlotRouter.
- `plot_quad.rs` (467 ln; SPEC-HAL-296; A/S) — 4 synchronized Plots in
  a 2×2 SplitPane grid, 500K candles each; Plot inside the App/Screen
  framework (`:1-3`). APIs: Plot, SplitPane, PlotRenderPool, ViewState.
- `scatter.rs` (272 ln; SPEC-HAL-296A; A/S) — SDF circle rendering with
  live appends every 100 ms; per-point radius/color overrides (`:1-5`).
  APIs: Plot, CircleSeries, CircleStyle, DataUpdate.
- `timeline_bound.rs` (557 ln; SPEC-HAL-335/336/337; A/S) — the bound
  Gantt/resource-board composition: SplitPane with TreeView (left) and
  TimelineHeader+TimelineCanvas (right) over one SharedHts; per-frame
  `hts_rows` → `set_row_geometry` seam; unified row selection (`:1-35`).
  APIs: SplitPane, TreeView, TimelineHeader, TimelineCanvas, SharedHts.
- `timeline_strip.rs` (302 ln; SPEC-HAL-334/335/336; A/S) — free-mode
  TemporalSubstrate: lane packing, baseline layer, dependency arrow,
  markers, Select policy, intent drain (`:1-21`). APIs: TimelineHeader,
  TimelineCanvas, TemporalCanvasPolicy.
- `track_display_styles.rs` (213 ln; SPEC-HAL-278; A/S) — Track vs Bar
  vs Pill vs Radial across Slider, ProgressBar, BusyIndicator (`:1`).
  APIs: TrackDisplayStyle, Slider, ProgressBar, BusyIndicator.

## Rendering / GPU Probes

- `animated_rect.rs` (225 ln; no SPEC refs; A/S) — single custom
  widget drawing track+thumb; click toggles t 0↔1; verifies push_rect
  position updates outside Switch/scroll context (`:1-3`). APIs:
  Animation, Easing, TransitionHandle.
- `border_focus.rs` (788 ln; SPEC-R-015-v2; A/S) — border & focus-ring
  catalog: FocusStyle::ShadowRing, BorderWidths, RoundedCorners;
  custom demo widgets only (`:1-6`). APIs: FocusStyle, BorderWidths,
  RoundedCorners.
- `boundary_test.rs` (317 ln; SPEC-HAL-087/141/011; headless) —
  pixel-precision boundary harness for a button's vertical edge:
  builds a tree, runs `layout_tree`, prints coordinates; no window
  (`:1-3`). APIs: layout_tree, Constraints, NodeId, StubTextEngine.
- `compositor_single_surface.rs` (244 ln; SPEC-HAL-081A; A/S) — frozen
  single-surface contract demo: widget stores surface properties at
  construction; snapshot → SceneFrame one-way flow (`:1-29`). APIs:
  Compositor, SceneKey, SurfaceId.
- `compositor_surface_order.rs` (262 ln; SPEC-HAL-081A; A/S) — surface
  stack ordering authority (INV-COMP-P04): three z-ordered rectangles
  on the base surface (`:1-31`). APIs: Compositor, SceneKey, SurfaceId.
- `gradient.rs` (640 ln; SPEC-R-012-v2; A/S) — gradient catalog:
  linear, radial, rounded corners, practical UI uses (`:1-6`). APIs:
  Color, ColorStop, RoundedCorners.
- `image_pipeline.rs` (195 ln; SPEC-HAL-263; winit) — image pipeline
  proof: checkerboard texture with and without rounded clip;
  register/unregister behavior (INV-IMG-005) (`:1-11`). APIs:
  SceneBuilder, DrawPlanBuilder, WgpuRenderer, Compositor.
- `min_hit_test.rs` (240 ln; SPEC-HAL-021/078/011; headless) —
  deterministic coordinate-propagation test through
  FlexColumn→Container→FlexRow→Button→Label; pure
  layout/paint/hit-test, no GUI (`:1-4`). APIs: WidgetTree,
  layout_tree, Container, Button.
- `renderer_alpha_probe.rs` (168 ln; SPEC-HAL-159; A/S) — blending
  comparison: primitive rect vs path rounded-rect at two alphas; rows
  must match visually (`:1-13`). APIs: Color, RoundedCorners, RectF.
- `renderer_border_ring_invariant.rs` (212 ln; no SPEC refs; A/S) —
  rounded-border ring-mesh invariant: radii [0,4,8,16] × widths
  [1,2,4] at fractional positions; uniform thickness expected
  (`:1-10`). APIs: Color, RoundedCorners, RectF.
- `renderer_opacity_reference.rs` (284 ln; SPEC-HAL-159; A/S) —
  canonical opacity/alpha correctness reference: linear color space,
  straight alpha, single premultiply, sRGB swapchain; identical result
  across primitives (`:1-29`). APIs: Color, RoundedCorners, RectF.
- `renderer_path_rect_parity.rs` (293 ln; SPEC-HAL-159; A/S) — rect vs
  path pipeline parity; Space cycles Both/Primitive/Path/Blink-Diff;
  flicker = bug (`:1-9`). APIs: Color, RoundedCorners, RectF.
- `rounded_rect_clip_guard.rs` (185 ln; SPEC-RENDER-CLIP-001; A/S) —
  permanent regression guard against the a0e0a1f clip-bleed bug:
  hover isolation across adjacent buttons; RoundedRect must stay on
  the rect pipeline (`:1-28`). APIs: Button, Flex.
- `shadow.rs` (964 ln; SPEC-R-013-v2; A/S) — desktop shadow catalog:
  popup shadows, directional light, recessed inputs, elevation scale,
  focus rings, scroll edge hints (`:1-7`). APIs: ShadowMode,
  RoundedCorners.
- ✗ `shape_primitives.rs` (155 ln; SPEC-R-003; headless) — shape nodes
  (circles, ellipses, lines, polylines) through scene → batcher →
  path pipeline (`:1-7`). APIs: SceneBuilder, CircleNode, EllipseNode,
  LineNode, PolylineNode. **Fails to build at this revision (E0004,
  match missing `SceneNode::RetainedFragment(_)`) — pre-existing.**

## Input

- `drag_drop.rs` (333 ln; SPEC-HAL-241; A/S) — list reorder via
  drag-and-drop: DraggableItem widgets produce payloads and accept
  drops; reorder events via FormBinder (`:1-4`). APIs: FormBinder,
  ChangeWriter, drag hooks.
- `input_fusion_debug.rs` (360 ln; SPEC-HAL-033 + engine SPECs;
  winit) — logs every FusedInputEvent to stdout; run_single_frame
  ticker loop (`:1-2`). APIs: InputFusion, EventRouter, EngineTicker.

## Agent / Tower

- `agent.rs` (104 ln; SPEC-HAL-317; A/S) — demo app with the built-in
  agent WebSocket server on :9317; external agents observe events and
  inject actions (`:1-5`). APIs: AppConfig `with_agent_server`.
- `agent_lm_bridge.rs` (26 ln; SPEC-HAL-318; standalone CLI) — Tower
  standalone bridge between LM Studio and a running agent demo
  (`:1-10`). APIs: Tower, TowerConfig. Companion: `agent` + LM Studio.
- `agent_query.rs` (68 ln; SPEC-HAL-317; ws-client) — sends one UI
  query (widgets/focus/tab/tree) to a running app (`:1-10`). APIs:
  tungstenite only. Companion: `agent`.
- `agent_test_client.rs` (80 ln; SPEC-HAL-317; ws-client) — connects
  to ws://localhost:9317 and injects a Tab/capture/type key sequence
  (`:1-6`). APIs: tungstenite only. Companion: `agent`.
- `tower_embedded.rs` (95 ln; SPEC-HAL-318; A/S) — Tower auto-spawned
  inside a Halcyon app (control port 9318); requires LM Studio
  (`:1-7`). APIs: TowerConfig, AppConfig.

## Directory Examples

- `showcase/` (20 .rs files; `main.rs` 1385 ln; SPEC-HAL-002/101/305/
  208A/333 refs; A/S) — single-screen tabbed demo app: "tabbed
  navigation, breadcrumbs, and status bar" (`main.rs:1-3`) driving 17
  `tab_*` modules (console, data, display, event_log, inputs, layout,
  list_view, live_data, navigator, node_graph, order_book, overlays,
  plot, plot_grids, property_editor, tree_view, workbench) plus
  `menu.rs` (SPEC-HAL-333 main menu) and `tip.rs` (TooltipHost alias).
  APIs: TabBar, Breadcrumbs, ListView, Panel, Switch, FormBinder.
  `tab_tree_view.rs` notes a synthetic ~100 000-node load. Limitation:
  `tab_plot.rs` is dirty in the working tree (unsettled market-detail
  unit) — see `plot-and-market-detail.md` §4 for its composition.
- `rfc018_demo/` (main.rs 282 ln + `screens/` with
  main_screen/debug_screen/simple_modal; SPEC-HAL-018/081B/084/163/
  266; winit) — navigation and screen architecture demo: ScreenHost
  root, incremental rebuild, D→DebugScreen navigation, M→modal,
  Backspace/Esc back; hand-rolled winit loop + `spawn_ui_thread`
  (`main.rs:1-8`). APIs: ScreenHost, DrawPlanBuilder, SceneSwapchain,
  UiThreadConfig.

## Coverage Statement

All 99 top-level `.rs` files in `examples/` plus both directory
examples appear above exactly once (2 canonical, 11 shell/navigation,
35 widget basics, 8 text, 6 layout, 7 tabular/data, 8 plot/temporal,
15 rendering probes, 2 input, 5 agent — and 2 directories;
2+11+35+8+6+7+8+15+2+5 = 99). Union verified mechanically against the
live `examples/` listing at this revision. The previous revision of
this index omitted `time_picker.rs` from its family listing while
claiming completeness; the 2026-07-24 mechanical pass found and fixed
that omission (and caught its own draft omitting `linear_gauge.rs`
before commit).

## Conventions

Every major feature needs a runnable example in `examples/`
[authority: SPEC-HAL-000 §11 (:322-324)]. Example code never consumes
Escape — the framework owns that input route.
