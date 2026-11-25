---
doc-id: halcyon-generated-api-quick-reference
class: generated
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-309
---

# API Quick Reference (Generated)

**Generated mechanically** from internal rust-analyzer-backed semantic
inspection at revision `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`
(INV-309-8). Authority is exactly that revision — nothing more.

**Historical method:** semantic member and source inspection in the Halcyon
repository, with bounded pagination to exhaustion. The original internal
command transcript is retained privately. Signatures are verbatim anchored
extracts; multi-line signatures are collapsed to one line
(whitespace-only transformation). **Coverage for every listed type is
complete** — all pagination cursors recorded by the original generation
were followed to exhaustion at `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`
on 2026-07-24 (`truncated: false`, `omitted_items: 0` on every final
page).

| Type | Extracted | Omitted | Coverage |
|---|---|---|---|
| `Widget` trait | 58 + 3 core fns | 0 | complete (members + head read) |
| `Screen` trait | full source | 0 | complete |
| `WidgetTree` | 122 | 0 | complete (60 + 62 continuation) |
| `Theme` | 27 | 0 | complete |
| `DensityMode` | full source | 0 | complete |
| `TextEngine` trait | full source | 0 | complete |
| `SceneBuilder` | 89 | 0 | complete (40 + 49 continuation) |
| `SceneGraph` | 9 | 0 | complete |
| `WgpuRenderer` | 203 | 0 | complete (50 + 153 continuation) |
| `TabularSubstrate` | 151 | 0 | complete (40 + 111 continuation) |
| `ListViewCore` | 116 | 0 | complete (full fresh extraction; +1 member vs `68216f25` — `dispatch_intent_with_selection_change`, added by `b3be05fc`) |
| `DataSource` trait | 10 | 0 | complete |
| `ActionRegistry` | 16 | 0 | complete |
| `BindingRegistry` | 11 | 0 | complete |
| `UpdateContext` | 14 | 0 | complete |
| `SignalStore` | 11 | 0 | complete |
| `App` / `AppConfig` | full | 0 | complete |
| `Constraints` / `LayoutResult` | full source | 0 | complete |
| `ScrollContainer` | full struct + doc | 0 | complete (`read src/core/scroll_container.rs 28 120`) |
| `KeyboardAuthorityMode`, `AgentServerProfile` | full source | 0 | complete |

## core::app

```rust
// src/core/app.rs:9060-9074
pub struct App { config: AppConfig }
pub fn new(config: AppConfig) -> Self
pub fn run<S: Screen + 'static>(mut self, screen: S)

// src/core/app.rs:7108-7190
pub struct AppConfig {
    pub title: String,
    pub width: f32,
    pub height: f32,
    pub theme: Theme,
    pub agent_tx: Option<AgentEventSender>,
    pub agent_action_rx: Option<AgentActionReceiver>,
    pub agent_server_port: Option<u16>,
    pub agent_server_profile: AgentServerProfile,
    pub tower_config: Option<TowerConfig>,
}
pub fn new(title: impl Into<String>) -> Self
pub fn with_size(mut self, width: f32, height: f32) -> Self
pub fn with_theme(mut self, theme: Theme) -> Self
pub fn with_agent_channel(mut self) -> (Self, AgentEventReceiver, AgentActionSender)

// Screen trait — src/core/app.rs:4785-4909
fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget
fn on_enter(&mut self, _cx: &mut ScreenContext<'_>)
fn on_leave(&mut self, _cx: &mut ScreenContext<'_>)
fn handle_app_event(&mut self, _event: &AppEvent, _cx: &mut ScreenContext<'_>)
fn handle_signal(&mut self, _signal: &SignalEnvelope, _cx: &mut ScreenContext<'_>) -> bool
fn paint_child_window_scene(&self, _window: &ChildWindowSceneState, _cx: &mut ChildWindowPaintContext<'_>) -> bool
fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool
fn flush_pending_actions(&mut self, _cx: &mut ScreenContext<'_>)
fn covers_point(&self, _logical_pos: Point) -> bool
fn dismiss_policy(&self) -> DismissPolicy
fn mark_rebuild(&mut self)
fn take_rebuild_flag(&mut self) -> bool          // default: true
fn update(&mut self, _tree: &mut WidgetTree)
```

## core::widget (trait Widget — src/core/widget/mod.rs:2058-2765)

```rust
fn layout(&mut self, cx: &mut LayoutContext, constraints: Constraints, children: &[NodeId]) -> LayoutResult
fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId])
fn paint_overlay(&self, _cx: &mut PaintContext, _rect: RectF, _tree_widget_id: TreeWidgetId)
fn update(&mut self, _dt: f32, _cx: &mut UpdateContext)
fn needs_update(&self) -> bool
fn needs_animation(&self) -> bool
fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool
fn hit_testable(&self) -> bool
fn focusable(&self) -> bool
fn clip_bounds(&self) -> Option<RectF>
fn clip_bounds_rounded(&self) -> Option<RoundedCorners>
fn scroll_offset(&self) -> Option<(f32, f32)>
fn scroll_container(&self) -> Option<&ScrollContainer>
fn scroll_container_mut(&mut self) -> Option<&mut ScrollContainer>
fn wants_scroll(&self) -> bool
fn needs_viewport_constraint(&self) -> bool
fn sync_scroll(&mut self, _scroll_y: f32) -> bool
fn take_requested_scroll_y(&mut self) -> Option<f32>
fn separator_hit_bands(&self) -> Option<Vec<f32>>
fn height_stable(&self) -> bool
fn on_mount(&mut self); fn on_unmount(&mut self); fn pre_layout(&mut self)
fn text_info(&self) -> Option<(&str, f32, Color)>
fn content_key(&self) -> Option<WidgetContentKey>
fn layout_axis(&self) -> Option<Axis>
fn on_focus(&mut self, _cx: &mut EventContext); fn on_blur(&mut self, _cx: &mut EventContext)
fn on_capture_enter(&mut self, _cx: &mut EventContext); fn on_capture_leave(&mut self, _cx: &mut EventContext)
fn focus_indicator_geometry(&self, own_rect: RectF) -> FocusIndicatorGeometry
fn paints_own_focus_indicator(&self) -> bool
fn is_focus_scope(&self) -> bool; fn defines_focus_scope(&self) -> bool
fn accepts_keyboard_activation(&self) -> bool; fn on_keyboard_activate(&mut self, _cx: &mut EventContext)
fn capture_eligible(&self) -> bool
fn on_hover_enter(&mut self, _cx: &mut EventContext); fn on_hover_leave(&mut self, _cx: &mut EventContext)
fn gesture_surface(&self) -> &'static [GestureKind]
fn on_gesture_commit(&mut self, _cx: &mut EventContext, _kind: GestureKind)
fn on_gesture_cancel(&mut self, _cx: &mut EventContext, _kind: GestureKind)
fn handle_projected_action(&mut self, _cx: &mut EventContext, _logical_id: LogicalItemId, _logical_index: usize, _action: ProjectedAction) -> bool
fn deferred_wiring(&self) -> Option<&dyn DeferredWiring>; fn deferred_wiring_mut(&mut self) -> Option<&mut dyn DeferredWiring>
fn provided_authority(&self) -> Option<(&dyn Any, TypeId)>; fn provided_authority_mut(&mut self) -> Option<(&mut dyn Any, TypeId)>
fn as_authority_owner(&mut self) -> Option<&mut dyn AuthorityOwner>
fn drag_payload(&self) -> Option<Box<dyn Any>>
fn paint_drag_proxy(&self, _cx: &mut PaintContext, _rect: RectF, _alpha: f32)
fn accepts_drop(&self, _payload_type: TypeId) -> bool
fn on_drag_enter(&mut self); fn on_drag_leave(&mut self); fn on_drop(&mut self, _payload: Box<dyn Any>) -> bool
fn clear_read_only_text_selection(&mut self) -> bool
fn set_paint_disabled(&mut self, _disabled: bool)
fn has_pending_anchor_resolution(&self) -> bool
fn as_anchor_resolver(&self) -> Option<&dyn AnchorResolver>
fn as_input_projection_provider(&self) -> Option<&dyn InputProjectionProvider>
fn debug_type_name(&self) -> &'static str
```

```rust
// UpdateContext — src/core/widget/mod.rs:1931-2005
pub fn theme(&self) -> Option<&Theme>
pub fn start_animation<T: Interpolatable + Copy + 'static>(&mut self, anim: Animation<T>) -> Option<TransitionHandle>
pub fn invalidate_paint(&mut self)
pub fn invalidate_paint_subtree(&mut self)
pub fn invalidate_layout(&mut self)
```

## core::layout / scroll

```rust
// src/core/layout.rs:24-33, :106-111
pub struct LayoutResult { pub size: Size, pub content_size: Size,
    pub viewport_size: Option<Size>, pub scroll_offset: Option<(f32, f32)> }
pub struct Constraints { pub min_width: f32, pub max_width: f32,
    pub min_height: f32, pub max_height: f32 }

// src/core/scroll_container.rs:28-79 (full struct; SPEC-HAL-086 stages 3/4/7/8/12, SPEC-HAL-268)
pub enum ScrollInputMode { Normal /*default*/, Drag }
pub struct ScrollContainer {
    pub content_size: Size,
    pub viewport_size: Size,
    scroll_offset_target: (f32, f32),   // logical, clamped [0, max_scroll]
    scroll_offset_visual: (f32, f32),   // smoothed for rendering
    pub smoothing_factor: f32,          // default 0.25
    input_mode: ScrollInputMode,        // Normal = smooth, Drag = instant
    drag_pending_x: Option<f32>,        // Stage 7 pending drag X
    drag_pending_y: Option<f32>,        // Stage 7 pending drag Y
    drag_input_alpha: f32,              // Stage 8 low-pass alpha, default 0.35
    has_drag_raw: bool,                 // Stage 12 drag-session flag
    force_animate: bool,                // SPEC-HAL-268 proximity scroll
}
```

## core::widget_tree (WidgetTree — first 60 of 122 members)

```rust
// src/core/widget_tree.rs:205-1053
pub root: Option<NodeId>
pub fn new() -> Self
pub fn create_root(&mut self, widget: Box<dyn Widget>, key: Option<WidgetKey>, z_order: i16) -> NodeId
pub fn add_child(&mut self, parent_id: NodeId, widget: Box<dyn Widget>, key: Option<WidgetKey>, z_order: i16) -> NodeId
pub fn remove_node(&mut self, node_id: NodeId)
pub fn node(&self, node_id: NodeId) -> Option<&WidgetNode>
pub fn node_mut(&mut self, node_id: NodeId) -> Option<&mut WidgetNode>
pub fn iter_nodes(&self) -> impl Iterator<Item = (NodeId, &WidgetNode)>
pub fn find_by_id(&self, widget_id: TreeWidgetId) -> Option<NodeId>
pub fn children(&self, parent_id: NodeId) -> Vec<NodeId>
pub fn clear_children(&mut self, parent_id: NodeId)
pub fn parent(&self, node_id: NodeId) -> Option<NodeId>
pub fn drain_pending_removals(&mut self) -> Vec<TreeWidgetId>
pub fn drain_pending_node_removals(&mut self) -> Vec<NodeId>
pub fn scroll_anchor_nodes(&self) -> &[NodeId]
pub fn begin_layout_scroll_anchor(&mut self, anchor_node: NodeId, affected_region_node: NodeId, intent: LayoutScrollAnchorIntent) -> bool
pub fn for_each_node_in_stable_order(&self, f: impl FnMut(NodeId))
pub fn lookup_authority<'a, A: Authority + 'static>(&'a self, from_node: NodeId) -> Option<Projection<'a, A>>
pub fn find_authority_owner<A: Authority + 'static>(&self, from_node: NodeId) -> Option<TreeWidgetId>
pub fn intent_buffers_mut(&mut self) -> &mut IntentBuffers
pub fn drain_intent_buffers(&mut self) -> Vec<(TreeWidgetId, Vec<Box<dyn Intent>>)>
pub fn is_descendant(&self, node_id: NodeId, ancestor_id: NodeId) -> bool
pub fn filter_focus_scopes(&self, nodes: &[NodeId]) -> Vec<NodeId>
pub fn set_rect(&mut self, node_id: NodeId, rect: RectF); pub fn get_rect(&self, node_id: NodeId) -> Option<RectF>
pub fn set_flex(&mut self, node_id: NodeId, flex: FlexParams); pub fn get_flex(&self, node_id: NodeId) -> FlexParams
pub fn set_agent_hint(&mut self, node_id: NodeId, hint: impl Into<String>)
pub fn wants_scroll(&self, node_id: NodeId) -> bool
pub fn needs_viewport_constraint(&self, node_id: NodeId) -> bool
pub fn height_stable(&self, node_id: NodeId) -> bool
pub fn set_overflow(&mut self, node_id: NodeId, overflow: OverflowMode)
pub fn set_layout_scroll_offset(&mut self, node_id: NodeId, offset: Option<(f32, f32)>)
pub fn set_absolute_xy(&mut self, node_id: NodeId, pos: Point)

// continuation — src/core/widget_tree.rs:1061-2802 (members 61-122)
pub fn get_absolute_xy(&self, node_id: NodeId) -> Option<Point>
pub fn set_absolute_paint_rect(&mut self, node_id: NodeId, rect: RectF)
pub fn get_absolute_paint_rect(&self, node_id: NodeId) -> Option<RectF>
pub fn set_viewport(&mut self, node_id: NodeId, rect: RectF)
pub fn get_viewport(&self, node_id: NodeId) -> Option<RectF>
pub fn set_cached_size(&mut self, node_id: NodeId, size: Size)
pub fn get_cached_size(&self, node_id: NodeId) -> Option<Size>
pub fn set_content_size(&mut self, node_id: NodeId, size: Size)
pub fn get_content_size(&self, node_id: NodeId) -> Option<Size>
pub fn find_node_by_widget_id(&self, widget_id: TreeWidgetId) -> Option<NodeId>
pub fn query_drag_payload(&self, node_id: NodeId) -> Option<Box<dyn std::any::Any>>
pub fn query_accepts_drop(&self, node_id: NodeId, payload_type: std::any::TypeId) -> bool
pub fn widget_id_for_node(&self, node_id: NodeId) -> Option<TreeWidgetId>
pub fn verify_tree_widget_id(&self, widget_id: TreeWidgetId) -> bool
pub fn verify_all_nodes_consistent(&self)   // :1209 and :1261 (cfg-paired variants)
fn identity_violation(msg: &str)
pub fn cumulative_scroll_offset(&self, node_id: NodeId) -> (f32, f32)
pub fn mark_layout_dirty(&mut self, node_id: NodeId)
pub fn mark_paint_dirty(&mut self, node_id: NodeId)
pub fn mark_paint_dirty_by_widget_id(&mut self, widget_id: TreeWidgetId)
pub fn mark_subtree_dirty(&mut self, node_id: NodeId)
pub fn mark_subtree_paint_dirty(&mut self, node_id: NodeId)
pub fn mark_layout_dirty_by_widget_id(&mut self, widget_id: TreeWidgetId)
fn build_snapshot_for_node(&self, node_id: NodeId) -> Option<NodeSnapshot>
fn compute_dirty_flags(old: Option<&NodeSnapshot>, new: &NodeSnapshot) -> WidgetDirty
pub fn recompute_dirty_flags(&mut self)
pub fn node_count(&self) -> usize
pub fn rebuild_hit_test_grid(&mut self, viewport: RectF)
pub fn set_overlay_hit_regions(&mut self, regions: Vec<OverlayHitRegion>)
pub(crate) fn has_blocking_overlay_hit_region(&self) -> bool
fn collect_hit_test_nodes_recursive_static(nodes: &[Option<WidgetNode>], node_id: NodeId, _parent_abs_x: f32, _parent_abs_y: f32, out: &mut Vec<(NodeId, RectF)>)
pub fn take_widget(&mut self, node_id: NodeId) -> Option<(Box<dyn Widget>, Vec<NodeId>, RectF)>
pub fn restore_widget(&mut self, node_id: NodeId, widget: Box<dyn Widget>, rect: RectF)
pub fn replace_widget_only(&mut self, node_id: NodeId, widget: Box<dyn Widget>)
pub fn compute_absolute_bounds(&self, widget_id: TreeWidgetId) -> Option<RectF>
pub fn clear_input_projections(&mut self)
pub fn set_input_projection(&mut self, container_id: NodeId, projection: InputProjection)
pub fn get_input_projection(&self, container_id: NodeId) -> Option<&InputProjection>
pub fn hit_test_projected(&self, x: f32, y: f32) -> Option<ProjectedHitResult>
pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTestResult>
pub fn hit_test_in_subtree(&self, x: f32, y: f32, subtree_root: NodeId) -> Option<HitTestResult>
fn build_path_to_root(&self, node_id: NodeId, leaf_abs_rect: RectF, path: &mut Vec<(NodeId, RectF)>)
fn hit_test_node_with_path(&self, node_id: NodeId, pointer_x: f32, pointer_y: f32, scroll_offset: (f32, f32), path: &mut Vec<(NodeId, RectF)>) -> Option<HitTestResult>
fn hit_test_node(&self, node_id: NodeId, pointer_x: f32, pointer_y: f32) -> Option<HitTestResult>
pub fn update_scroll_animations(&mut self, dt_secs: f32)
pub(crate) fn sync_linked_horizontal_scroll_projections(&mut self)
pub fn any_scroll_needs_animation(&self) -> bool
pub fn any_widget_needs_animation(&self) -> bool
pub fn any_widget_needs_update(&self) -> bool
pub fn compute_viewports(&mut self, root: NodeId)
fn compute_viewports_dfs(&mut self, node_id: NodeId, incoming_clip: RectF, inside_scrollport: bool)
pub fn begin_layout_transition_frame(&mut self)
pub fn record_layout_transition(&mut self, id: TreeWidgetId, rect: RectF)
pub fn collect_layout_transition_candidates(&self, out: &mut Vec<LayoutTransitionCandidate>)
pub fn layout_transition_store(&self) -> &LayoutTransitionStore
pub fn compute_conflict_flags(&self, out: &mut HashMap<TreeWidgetId, ConflictFlags>)
fn has_scroll_ancestor(&self, node_id: NodeId) -> bool
fn has_anchor_compensated_ancestor(&self, node_id: NodeId, anchor_compensated_scroll_containers: &HashSet<NodeId>) -> bool
pub fn find_scroll_container(&self, node_id: NodeId) -> Option<NodeId>
pub fn is_ancestor_of(&self, ancestor: NodeId, descendant: NodeId) -> bool
fn default() -> Self
```

## core::theme

```rust
// src/core/theme.rs:47-58, :2006-2293
pub enum DensityMode { Compact /*default*/, Regular, Comfortable }
pub struct Theme { pub density: DensityMode, pub geometry: GeometrySpec,
    pub typography: TypographySpec, pub colors: ColorSpec,
    pub interaction: InteractionSpec, pub styles: StyleSpec,
    pub spacing: SpacingSpec, pub padding: PaddingSpec,
    pub borders: BorderSpec, pub radii: RadiusSpec,
    pub scrollbar_geom: ScrollbarGeometrySpec, pub list_geom: ListGeometrySpec,
    pub shadows: ShadowSpec, pub focus_ring: FocusRingSpec, pub tooltip: TooltipSpec }
pub fn dark() -> Self; pub fn dark_with_density(density: DensityMode) -> Self
pub fn light() -> Self; pub fn light_with_density(density: DensityMode) -> Self
pub fn is_dark(&self) -> bool; pub fn toggle(&mut self); pub fn set_density(&mut self, density: DensityMode)
pub fn control_border(&self, focused: bool, captured: bool, error: bool) -> (Color, f32)
pub fn chrome_radius(&self, category: ControlChromeCategory, requested: f32, is_round: bool) -> f32
pub fn control_chrome(&self, category: ControlChromeCategory, state: ControlChromeState, validation: ControlValidationState) -> ControlChromeSpec
impl Default for Theme { fn default() -> Self { Self::light() } }   // :2289-2293
```

## core::text_engine (trait TextEngine — src/core/text_engine.rs:221-271)

```rust
fn measure(&mut self, text: &str, font_size: f32) -> (f32, f32)
fn metrics(&mut self, text: &str, font_size: f32) -> TextMetrics
fn font_metrics(&mut self, font_size: f32) -> TextMetrics    // default impl
fn shape(&mut self, text: &str, font_size: f32) -> GlyphRun
fn draw(&mut self, run: &GlyphRun, x: f32, y: f32, color: Color, sink: &mut dyn PaintSink)
fn draw_with_backgrounds(&mut self, run: &GlyphRun, x: f32, y: f32, color: Color, backgrounds: Vec<TextBackgroundSpan>, sink: &mut dyn PaintSink)
fn shaper(&self) -> Option<&TextShaper>                       // default None
```

## core::scene (SceneBuilder 89 members complete; SceneGraph complete)

```rust
// src/core/scene/mod.rs:1096-1189
pub struct SceneGraph { pub root: Vec<SceneNode>, pub clear_color: Color, /* key_widget_map */ }
pub fn new() -> Self; pub fn clear(&mut self)
pub fn key_widget_map(&self) -> &HashMap<SceneNodeKey, u64>
pub fn find_node_by_key(&self, key: SceneNodeKey) -> Option<&SceneNode>

// src/core/scene/mod.rs:1218-1843 (SceneBuilder, extracted subset)
pub fn new() -> Self; pub fn with_scale_factor(scale_factor: f32) -> Self
pub fn scale_factor(&self) -> f32; pub fn set_clear_color(&mut self, color: Color)
pub fn set_current_painting_widget(&mut self, widget_id: u64); pub fn clear_current_painting_widget(&mut self)
pub fn begin_clip(&mut self, rect: RectF, rounded: Option<RoundedCorners>); pub fn end_clip(&mut self)
pub fn with_clip<F, R>(&mut self, rect: RectF, rounded: Option<RoundedCorners>, f: F) -> R
pub fn clip_stack_depth(&self) -> usize
pub fn begin_scroll(&mut self, offset: (f32, f32)); pub fn end_scroll(&mut self)
pub fn push_rect(&mut self, rect: RectF, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_border_rect(&mut self, rect: RectF, fill_color: Color, border_color: Color, border_width: f32, radius: f32, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_rounded_rect(&mut self, rect: RectF, color: Color, radius: f32, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_text(&mut self, position: Point, text: String, font_size: f32, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_text_with_backgrounds(&mut self, position: Point, text: String, font_size: f32, color: Color, backgrounds: Vec<TextBackgroundSpan>, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_text_probe(&mut self, position: Point, text: String, font_size: f32, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_image(&mut self, rect: RectF, texture_id: u64, uv_rect: RectF, tint: Color, z_index: ZIndex, key: Option<SceneNodeKey>)

// continuation — src/core/scene/mod.rs:1857-2809 (members 41-89; the tail
// from :2455 is the PaintSink impl for SceneBuilder — same push vocabulary
// over GlyphRun/paint types)
pub fn push_clip(&mut self, rect: RectF, children: Vec<SceneNode>, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_layer(&mut self, rect: RectF, children: Vec<SceneNode>, opacity: f32, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_circle(&mut self, center: Point, radius: f32, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_ellipse(&mut self, center: Point, radii: Point, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_line(&mut self, a: Point, b: Point, width: f32, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_polyline(&mut self, points: Vec<Point>, width: f32, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_path(&mut self, path: crate::renderer::path::Path, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_path_with_mesh(&mut self, path: Path, mesh: PathMesh, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_path_debug(&mut self, path: Path, color: Color, mode: PathDebugMode, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_path_with_mesh_debug(&mut self, path: Path, mesh: PathMesh, color: Color, mode: PathDebugMode, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_icon(&mut self, definition: &IconDefinition, x: f32, y: f32, size: f32, color: Color, z_index: ZIndex)
pub fn push_icon_spec(&mut self, spec: &IconSpec, registry: &IconRegistry, x: f32, y: f32, z_index: ZIndex) -> bool
pub fn push_scrollbar_v(&mut self, track_abs: RectF, owner_abs: RectF, widget_id: TreeWidgetId, key: Option<SceneNodeKey>)
pub fn push_scrollbar_h(&mut self, track_abs: RectF, owner_abs: RectF, widget_id: TreeWidgetId, key: Option<SceneNodeKey>)
pub fn push_rect_mesh(&mut self, rects: &[RectF], color: Color, z_index: ZIndex)
pub fn push_shadow(&mut self, shape_rect: RectF, shape_radius: f32, mode: ShadowMode, offset: (f32, f32), spread: f32, blur_radius: f32, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_gradient_rect(&mut self, rect: RectF, radius: f32, gradient: GradientDef, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_color_picker_sv(&mut self, rect: RectF, hue: f32, radius: f32, z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn push_radial_gauge(&mut self, rect: RectF, fill: f32, thickness: f32, track_color: Color, fill_color: Color, fill_color_negative: Color, start_angle: f32, sweep_angle: f32, origin_fill: f32, mode: u32, fill_style: u32, zone_count: u32, zones: [(f32, f32, [f32; 4]); 4], major_divisions: u32, minor_per_major: u32, tick_length_major: f32, tick_length_minor: f32, tick_color: [f32; 4], z_index: ZIndex, key: Option<SceneNodeKey>)
pub fn debug_print_stats(&self, label: &str)
fn check_scene_text_caller(method_name: &str)
fn is_authorized_scene_text_path(file: &str) -> bool
pub fn build(self) -> SceneGraph
fn default() -> Self
// PaintSink impl (:2465-2809): scale_factor, begin_clip/end_clip,
// begin_scroll/end_scroll, push_rect, push_border_rect, push_rounded_rect,
// push_border_rounded_rect, push_shadow, push_gradient_rect,
// push_color_picker_sv, push_radial_gauge, push_circle, push_ellipse,
// push_path, push_icon, push_rect_mesh, push_text_with_backgrounds
// (GlyphRun-based), set/clear_current_painting_widget, push_image,
// push_line, push_polyline, push_text_probe
```

## runtime (actions)

```rust
// src/runtime/actions.rs:333-620
pub struct ActionRegistry { pub actions: HashMap<ActionId, Action>,
    pub shortcut_map: HashMap<Shortcut, ActionId>, pub list: Vec<ActionId>,
    pub binding_registry: BindingRegistry }
pub fn add(&mut self, action: Action)
pub fn find_action_by_shortcut(&self, shortcut: &Shortcut) -> Option<ActionId>
pub fn execute(&self, id: ActionId, cx: &mut AppContext)
pub fn try_execute_shortcut(&self, shortcut: &Shortcut, cx: &mut AppContext) -> bool
pub fn all(&self) -> impl Iterator<Item = &Action>
pub fn by_scope(&self, scope: ActionScope) -> impl Iterator<Item = &Action>
pub fn search_basic<'a>(&'a self, query: &str, out: &mut Vec<&'a Action>)
pub fn search_fuzzy<'a>(&'a self, query: &str, out: &mut Vec<(&'a Action, i32)>)
pub fn search<'a>(&'a self, query: &str, out: &mut Vec<&'a Action>)

// src/runtime/binding_registry.rs:123-221
pub fn add(&mut self, binding: Binding)
pub fn remove_by_tag(&mut self, tag: &str); pub fn remove_all_runtime(&mut self)
pub fn resolve(&mut self)
pub fn lookup(&mut self, pattern: &InputPattern) -> Option<ActionId>
pub fn has_binding(&mut self, pattern: &InputPattern) -> bool
```

## model::signal_store

```rust
// src/model/signal_store.rs:40-123
pub fn create<T: 'static + Clone + PartialEq>(&mut self, initial: T) -> SignalHandle<T>
pub fn get<T: 'static + Clone>(&self, handle: SignalHandle<T>) -> T
pub fn set<T: 'static + Clone + PartialEq>(&mut self, handle: SignalHandle<T>, value: T)
pub fn watch<T: 'static + Clone>(&mut self, handle: SignalHandle<T>, node_id: NodeId) -> T
pub fn take_pending_dirty(&mut self) -> Vec<NodeId>
pub fn remove_subscriber(&mut self, node_id: NodeId); pub fn remove_subscribers(&mut self, node_ids: &[NodeId])
```

## widgets::tabular_view (DataSource trait — src/widgets/tabular_view/data_adapter.rs:460-549)

```rust
fn row_ids(&self) -> &[RowId]
fn order_access(&self) -> OrderAccess
fn order_revision(&self) -> OrderRevision
fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow>
fn locate_row(&self, revision: OrderRevision, row_id: RowId) -> ValueState<Option<usize>>
fn sort_key_for_column(&self, column: ColumnId) -> Option<SortKey>
fn sort_capability(&self, _key: &SortKey) -> SortCapability
fn applied_sort(&self) -> Option<AppliedSortSnapshot>
fn request_sort(&mut self, request: SortRequest) -> SortRequestOutcome
fn commit_cell(&mut self, _row: RowId, _column: ColumnId, _value: CellValue) -> bool
```

## widgets::tabular_view (TabularSubstrate — 151 members complete)

```rust
// src/widgets/tabular_view/mod.rs:346-475 (fields)
id: WidgetInternalId
column_model: ColumnModel; geometry_table: ColumnGeometryTable
header: HeaderDomain; body: BodyDomain
sort_state: SortState
comparator: Option<Box<dyn RowComparator + Send + Sync>>
pending_comparator: Option<Box<dyn RowComparator + Send + Sync>>
focus_state: FocusState; edit_state: EditState
selection_state: SelectionState
ordered_selection_state: OrderedSelectionState<row::RowId>
evaluation_guard: EvaluationPhaseGuard; column_guard: ColumnModelGuard
row_guard: RowModelGuard; sort_guard: SortProcessingGuard
pending_intents: Vec<TabularIntent>
on_intent: Option<Box<dyn Fn(&TabularIntent) + Send + Sync>>
engine_invalidation: EngineInvalidation
row_topology: Box<dyn engines::row_topology::TopologyEngine>
virtualization: Box<dyn engines::virtualization::VirtualizationStrategy>
visible_range: Option<VisibleRangeToken>; scroll_epoch: u64
evaluation_mode: EvaluationMode; is_listview_embedded: bool
row_expansion: RowExpansion

// src/widgets/tabular_view/mod.rs:480-2533 (methods)
pub fn new() -> Self
pub fn with_columns(columns: Vec<Column>) -> Self
pub fn with_header_height(mut self, height: f32) -> Self
pub fn with_row_model(mut self, row_model: RowModel) -> Self
pub fn with_on_intent<F: Fn(&TabularIntent) + Send + Sync + 'static>(mut self, handler: F) -> Self
pub fn with_comparator<C: RowComparator + Send + Sync + 'static>(mut self, comparator: C) -> Self
pub fn column_model(&self) -> &ColumnModel
pub fn column_model_mut(&mut self) -> &mut ColumnModel
pub fn mark_geometry_dirty(&mut self)
pub fn add_column(&mut self, column: Column)
pub fn set_column_visible(&mut self, id: ColumnId, visible: bool) -> bool
pub fn toggle_column_visible(&mut self, id: ColumnId) -> bool
pub fn is_column_visible(&self, id: ColumnId) -> bool
pub fn set_column_width(&mut self, id: ColumnId, width: f32) -> bool
pub fn apply_column_configuration(&mut self, config: &column_config::ColumnConfigurationState) -> bool
pub fn geometry_view(&self) -> GeometryView<'_>
pub(crate) fn geometry_resolved_for_current_epoch(&self) -> bool
pub fn content_width(&self) -> f32
pub fn content_height(&self) -> f32
pub fn row_model(&self) -> &RowModel
fn bump_base_row_order_epoch(&mut self)
pub fn add_row(&mut self, id: RowId)
pub fn add_row_with_height(&mut self, id: RowId, height: f32)
pub fn clear_rows(&mut self)
pub fn set_fixed_row_height(&mut self, height: f32)
pub fn fixed_row_height(&self) -> Option<f32>
pub fn header(&self) -> &HeaderDomain
pub fn header_mut(&mut self) -> &mut HeaderDomain
pub fn body(&self) -> &BodyDomain
pub(crate) fn body_mut(&mut self) -> &mut BodyDomain
pub fn set_body_viewport(&mut self, viewport: RectF)
pub fn begin_evaluation(&self)
fn ensure_topology(&mut self)
pub fn prepare_evaluation(&mut self, available_width: f32, scale_factor: f32)
fn prepare_evaluation_core(&mut self, first: usize, last: usize, y_offset: f32)
pub(crate) fn set_listview_embedded(&mut self)
pub(crate) fn is_embedded_in_listview(&self) -> bool
pub(crate) fn resolve_layout_geometry(&mut self, available_width: f32, scale_factor: f32)
pub fn set_scroll_x(&mut self, offset: f32)
pub fn set_scroll_y(&mut self, offset: f32)
pub fn scroll_x(&self) -> f32
pub fn scroll_y(&self) -> f32
pub(crate) fn set_scroll_y_from_scroll_area(&mut self, scroll_y: f32) -> bool
pub fn update_visible_range_from_scroll(&mut self, scroll_y: f32) -> bool
pub fn sort_state(&self) -> &SortState
pub fn sorted_indices(&self) -> &[usize]
pub fn set_sorted_indices(&mut self, indices: Vec<usize>)
pub fn compute_scroll_to_reveal_row(&self, row_id: RowId) -> Option<f32>
pub fn compute_scroll_to_reveal_index(&self, visual_index: usize) -> Option<f32>
pub fn set_comparator<C: RowComparator + Send + Sync + 'static>(&mut self, comparator: C)
pub fn clear_comparator(&mut self)
pub fn focus_state(&self) -> &FocusState
pub fn legacy_selection_state(&self) -> &SelectionState
pub fn ordered_selection(&self) -> &OrderedSelectionState<row::RowId>
pub fn set_selection_mode(&mut self, mode: SelectionMode)
pub fn selection_mode(&self) -> SelectionMode
pub fn set_interaction_model(&mut self, model: SelectionInteractionModel)
pub fn interaction_model(&self) -> SelectionInteractionModel
pub fn is_transactional(&self) -> bool
pub fn lock_interaction_model(&mut self)
pub fn expansion(&self) -> &RowExpansion
pub fn expansion_mut(&mut self) -> &mut RowExpansion
pub fn set_expansion_policy(&mut self, policy: ExpansionPolicy)
pub fn set_expanded_height_policy(&mut self, policy: ExpandedHeightPolicy)
pub fn begin_transaction(&mut self) -> bool
pub fn is_transaction_active(&self) -> bool
pub fn commit_transaction(&mut self) -> bool
pub fn cancel_transaction(&mut self) -> bool
pub fn edit_state(&self) -> &EditState
pub fn edit_session(&self) -> Option<&EditSession>
pub fn is_editing(&self) -> bool
pub fn editing_content_projection(&self) -> Option<&CellContentProjection>
fn cancel_edit_for_destabilization(&mut self)
pub fn body_hit_test(&self, x: f32, y: f32) -> Option<body::BodyHitResult>
pub fn hit_test_row_id_viewport_y(&self, viewport_local_y: f32) -> Option<RowId>
pub fn hit_test_row_id_content_y(&self, content_y: f32) -> Option<RowId>
pub fn hit_test_row_index_content_y(&self, content_y: f32) -> Option<usize>
pub fn hit_test_row_id(&self, viewport_local_y: f32) -> Option<RowId>
pub fn hit_test_row_index(&self, viewport_local_y: f32) -> Option<usize>
pub fn take_intents(&mut self) -> Vec<TabularIntent>
fn emit_intent(&mut self, intent: TabularIntent)
fn elastic_freeze_snapshot_for_resize(&self, target_column_id: ColumnId) -> Vec<(ColumnId, f32)>
fn handle_resize_intent(&mut self, intent: &intents::ResizeIntent)
fn handle_visibility_intent(&mut self, intent: &intents::VisibilityIntent)
fn handle_reorder_intent(&mut self, intent: &intents::ReorderIntent)
fn handle_width_intent(&mut self, intent: &intents::WidthIntent)
fn handle_sort_intent(&mut self, intent: &intents::SortIntent)
fn handle_focus_intent(&mut self, intent: &intents::FocusIntent)
fn handle_selection_intent(&mut self, intent: &intents::SelectionIntent)
fn mirror_ordered_into_legacy_selection(&mut self)
fn compute_row_range(&mut self, anchor: row::RowId, target: row::RowId) -> Vec<row::RowId>
fn resolve_navigation(&mut self, direction: intents::NavigationDirection) -> Option<row::CellId>
pub(crate) fn effective_row_order(&mut self) -> Vec<usize>
pub(crate) fn page_size(&self) -> usize
pub fn handle_intent(&mut self, intent: &TabularIntent)
fn handle_edit_intent(&mut self, intent: &intents::EditIntent)
fn apply_pending_comparator(&mut self)
fn process_pending_intents(&mut self)
fn sort_visible_indices(&self, indices: &mut Vec<usize>)
fn resolve_geometry(&mut self, available_width: f32, scale_factor: f32)
fn logical_row_count(&self) -> usize
fn visible_row_range(&self, start: usize, end: usize) -> (usize, usize)
fn default() -> Self
pub(crate) fn body_ref(&self) -> &BodyDomain
// test-only surface (:2514-2620): test_begin_evaluation,
// test_mark_geometry_resolved, test_resolve_geometry,
// debug_geometry_counters, base_row_order_epoch_for_test,
// test_realize_viewport, test_set_topology_dirty, test_topology_version,
// test_compute_visible_range, test_set_virtualization_dirty,
// test_set_selection_dirty, test_has_visible_range_token,
// test_is_virtualization_dirty
// Widget impl (:2626-2777): as_any, as_any_mut, layout, paint,
// handle_event, id, hit_testable, debug_type_name
```

## widgets::list_view (ListViewCore — 116 members complete)

```rust
// src/widgets/list_view/core.rs:113-219 (fields)
pub cms: CellMechanicsSubstrate
pub adapter: DataAdapter<S>
pub column_titles: HashMap<ColumnId, String>
pub item_height: f32
row_ids: Vec<RowId>; row_count: usize; row_heights: RowHeights
pub selection: OrderedSelectionState<RowId>
focused_column: Option<ColumnId>; pub focused: bool
pub requested_scroll_y: Option<f32>
selection_mode: SelectionMode
pub viewport_height: f32
pub hovered_separator: Option<ColumnId>
pub active_resize_separator: Option<ColumnId>
pub body_widget_id: Option<TreeWidgetId>
pub horizontal_scroll_link: Option<HorizontalScrollLink>
pub horizontal_scroll_freeze: Option<Rc<StdCell<bool>>>
pub intent_policy: TabularIntentPolicy
pub view_id: Option<String>; pub app_namespace: Option<String>
pub config_store: Option<Box<dyn ViewConfigStore>>
pub default_column_config: Option<ColumnConfigurationState>
pub config_overlay_open: bool
pub dragged_column_id: Option<ColumnId>
pub drag_visual_offsets: Option<HashMap<ColumnId, f32>>
pub ghost_occlude_range: Option<(f32, f32)>
pub cell_painter: Box<dyn CellPainter>; pub custom_cell_painter: bool
realized_cell_values: HashMap<(u64, ColumnId), ValueState<CellValue>>
pub combo_box_options: HashMap<ColumnId, Vec<(String, String)>>
pub activation_callback: Option<Rc<dyn Fn(RowId)>>
pub focused_row_callback: Option<Rc<dyn Fn(Option<RowId>)>>
pub scale_factor: f32
pub pointer_in_expansion_region: bool
pub row_widget_factory: Option<Box<dyn Fn(RowId) -> Box<dyn Widget>>>

// src/widgets/list_view/core.rs:223-1233 (methods)
pub fn new(source: S) -> Self
fn row_count_from_extent(extent: Extent, legacy_len: usize) -> usize
fn flat_nodes_from_row_ids(row_ids: &[RowId]) -> Vec<NodeMeta>
fn rebuild_flat_topology(&mut self)
pub fn add_column(&mut self, title: &str, width: f32) -> ColumnId
pub fn add_column_raw(&mut self, title: &str, column: Column) -> ColumnId
pub fn set_column_combo_options(&mut self, id: ColumnId, options: Vec<(String, String)>)
pub fn row_count(&self) -> usize
pub fn selection_mode(&self) -> SelectionMode
pub fn set_selection_mode(&mut self, mode: SelectionMode)
pub fn set_intent_policy(&mut self, policy: TabularIntentPolicy) -> bool
pub fn index_to_row_id(&self, index: usize) -> Option<RowId>
pub fn row_id_to_index(&self, row_id: RowId) -> Option<usize>
pub fn first_column_id(&self) -> Option<ColumnId>
pub fn focused_row(&self) -> Option<RowId>
pub fn focused_index(&self) -> Option<usize>
pub fn is_row_focused(&self, row_id: RowId) -> bool
pub fn is_selected(&self, row_id: RowId) -> bool
pub fn set_focus(&mut self, index: usize)
pub fn set_focus_by_row_id(&mut self, row_id: RowId)
pub fn action_click(&mut self, index: usize)
pub fn action_ctrl_click(&mut self, index: usize)
pub fn action_shift_click(&mut self, index: usize)
pub fn action_drag_select(&mut self, index: usize)
pub fn action_select_row(&mut self, row_id: RowId)
pub fn action_extend_to_row(&mut self, row_id: RowId)
pub fn action_toggle_row(&mut self, row_id: RowId)
pub fn page_size(&self) -> usize
pub fn compute_new_focus(&self, delta: isize) -> usize
pub fn handle_nav_up(&mut self, shift: bool, ctrl: bool)
pub fn handle_nav_down(&mut self, shift: bool, ctrl: bool)
pub fn handle_nav_home(&mut self, shift: bool, ctrl: bool)
pub fn handle_nav_end(&mut self, shift: bool, ctrl: bool)
pub fn handle_nav_page_up(&mut self, shift: bool, ctrl: bool)
pub fn handle_nav_page_down(&mut self, shift: bool, ctrl: bool)
fn handle_focus_direction(&mut self, delta: isize)
fn handle_move_focus_direction(&mut self, delta: isize)
fn handle_focus_to(&mut self, target: usize)
fn handle_move_focus_to(&mut self, target: usize)
fn handle_extend_direction(&mut self, delta: isize)
fn handle_extend_to(&mut self, target: usize)
pub fn take_requested_scroll_y(&mut self) -> Option<f32>
pub fn build_column_snapshot(&self) -> Vec<ColumnMetaSnapshot>
pub fn view_config_key(&self) -> Option<ViewConfigKey>
pub fn column_model(&self) -> Ref<'_, ColumnModel>
pub fn set_column_visible(&mut self, id: ColumnId, visible: bool) -> bool
pub fn toggle_column_visible(&mut self, id: ColumnId) -> bool
pub fn is_column_visible(&self, id: ColumnId) -> bool
pub fn set_column_scroll_domain(&mut self, id: ColumnId, domain: ColumnScrollDomain) -> bool
pub fn reorder_columns(&mut self, new_order: &[ColumnId]) -> bool
pub fn set_column_width(&mut self, id: ColumnId, width: f32)
pub fn commit_user_resize(&mut self, id: ColumnId, proposed_width: f32) -> Option<f32>
pub fn apply_column_configuration(&mut self, config: &ColumnConfigurationState) -> bool
pub fn content_width(&self) -> f32
pub fn content_height(&self) -> f32
pub fn row_height_at_index(&self, index: usize) -> f32
pub fn row_top_at_index(&self, index: usize) -> f32
pub fn set_row_height_at(&mut self, row_id: RowId, height: f32)
pub fn project_visible_rows(&self, visible_rows: &[VisibleRow]) -> Vec<RowProjection>
pub fn visible_row_projections(&self) -> Vec<RowProjection>
pub fn visible_rows_for_viewport(&mut self, viewport_height: f32) -> Vec<VisibleRow>
pub fn applied_sort(&self) -> Option<AppliedSortSnapshot>
pub fn sort_key_for_column(&self, column_id: ColumnId) -> Option<SortKey>
pub fn sort_capability_for_column(&self, column_id: ColumnId) -> SortCapability
pub fn request_sort_for_column(&mut self, column_id: ColumnId, additive: bool) -> Option<SortRequestOutcome>
pub fn request_sort(&mut self, operation: SortOperation) -> SortRequestOutcome
pub fn reset_sort_preference(&mut self) -> Option<SortRequestOutcome>
pub fn realize_visible_cells(&mut self, visible_rows: &[VisibleRow])
pub fn realized_cell_value_state(&self, node_id: u64, column_id: ColumnId) -> ValueState<CellValue>
pub fn dispatch_intent(&mut self, intent: &TabularIntent)
pub(crate) fn dispatch_intents_and_focus(&mut self, intents: &[TabularIntent], focus_row: Option<RowId>) -> ListViewDispatchCallbacks
fn dispatch_intent_with_selection_change(&mut self, intent: &TabularIntent) -> bool   // added by b3be05fc
pub fn sync_data_source(&mut self)
fn dispatch_selection_intent(&mut self, intent: &SelectionIntent) -> bool
fn dispatch_focus_intent(&mut self, intent: &FocusIntent)
fn dispatch_expansion_intent(&mut self, intent: &ExpansionIntent)
fn dispatch_selection_action(&mut self, action: OrderedSelectionAction<RowId>) -> bool
fn request_scroll_to_reveal_row(&mut self, row_id: RowId)
fn compute_scroll_to_reveal_index(&self, index: usize) -> Option<f32>
fn cell_from_value_state(&self, value: ValueState<CellValue>) -> Option<DataCell>
```

## renderer::wgpu_renderer (WgpuRenderer — 203 members complete)

```rust
// src/renderer/wgpu_renderer.rs:584-785 (fields)
instance: wgpu::Instance; device: Arc<wgpu::Device>; queue: Arc<wgpu::Queue>
surface_format: wgpu::TextureFormat
default_present_mode: wgpu::PresentMode
default_alpha_mode: wgpu::CompositeAlphaMode
rect_pipeline: wgpu::RenderPipeline
rect_globals_buffer: wgpu::Buffer; rect_globals_bind_group: wgpu::BindGroup
rect_globals_layout: wgpu::BindGroupLayout; globals_slab_capacity: usize
rounded_clip_buffer: wgpu::Buffer; rounded_clip_bind_group: wgpu::BindGroup
shadow_pipeline: wgpu::RenderPipeline; shadow_params_buffer: wgpu::Buffer
shadow_params_bind_group: wgpu::BindGroup
shadow_vertex_buffer/index_buffer: Option<wgpu::Buffer>; shadow_vertex/index_capacity: u32
gradient_pipeline: wgpu::RenderPipeline; gradient_params_buffer: wgpu::Buffer
gradient_params_bind_group: wgpu::BindGroup
gradient_vertex_buffer/index_buffer: Option<wgpu::Buffer>; gradient_vertex/index_capacity: u32
color_picker_sv_pipeline: wgpu::RenderPipeline (+ params buffer/bind group, vertex/index buffers + capacities)
radial_gauge_pipeline: wgpu::RenderPipeline (+ params buffer/bind group, vertex/index buffers + capacities)
sdf_rect_pipeline: Option<wgpu::RenderPipeline> (+ vertex/index buffers + capacities)
shape_pipeline: ShapePipeline (+ vertex/index buffers + capacities)
text_engine: BackendTextEngine<CoreTextBackend | DirectWriteBackend | FreeTypeBackend | GlyphonBackend>   // cfg per platform
vertex_buffer/index_buffer: Option<wgpu::Buffer>; vertex/index_capacity: u32
image_vertex_buffer/index_buffer: Option<wgpu::Buffer>; image_vertex/index_capacity: u32
image_pipeline: wgpu::RenderPipeline; image_bind_group_layout: wgpu::BindGroupLayout
image_sampler, nearest_sampler: wgpu::Sampler
image_textures: HashMap<u64, ImageTextureEntry>
pending_registrations: Arc<Mutex<Vec<PendingImageRegistration>>>
pending_unregistrations: Arc<Mutex<Vec<PendingImageUnregistration>>>
last_batches: Option<FrameBatches>
window_focused: bool
gpu_rounded_clips_enabled: bool; rounded_clip_aa_radius: f32
glyph_atlas: GlyphAtlas; text_pipeline: TextPipeline
text_vertex_buffer/index_buffer: Option<wgpu::Buffer>; text_vertex/index_capacity: u32
text_bg_sampler: wgpu::Sampler; text_globals_buffer: wgpu::Buffer
path_renderer: PathRenderer
path_globals_buffer: wgpu::Buffer; path_globals_bind_group: wgpu::BindGroup
path_clips_buffer: wgpu::Buffer; path_clips_bind_group: wgpu::BindGroup
path_clips_capacity, path_clips_slab_capacity: u32; path_clips_slot_stride: u64
mesh_cache: MeshCache; frame_counter: u64
rasterization_count: Cell<u32>
last_compile_epoch: Cell<Option<u64>>; compile_consumed: Cell<bool>
frame_epoch: FrameEpoch; gpu_arena: GpuResourceArena
staging_pool: StagingBufferPool; staging_upload_encoder: Option<wgpu::CommandEncoder>
staging_upload_handles: Vec<StagingBufferHandle>
pending_staging_submissions: Vec<PendingStagingSubmission>
pending_staging_remaps: Vec<PendingStagingRemap>
pipeline_generation, last_validated_pipeline_generation: u64
last_atlas_texture_id: GpuResourceId; last_surface_format: wgpu::TextureFormat
multi_targets: renderer::render_target::RenderTargetRegistry

// src/renderer/wgpu_renderer.rs:831-5871 (methods)
pub async fn new(window: Arc<winit::window::Window>, _physical_width: u32, _physical_height: u32) -> Self
pub async fn new_headless() -> Option<Self>
fn init_core(instance, device, queue, surface_format, default_present_mode, default_alpha_mode, use_cpu_fallback, scale_factor) -> Self
pub fn register_headless_render_target(&mut self, surface_id: SurfaceId, physical_size: (u32, u32), scale_factor: f32) -> Result<(), RenderTargetError>
fn create_rect_pipeline(device, format) -> (RenderPipeline, BindGroupLayout, BindGroupLayout)
fn create_image_pipeline(device, format, globals_layout) -> (RenderPipeline, BindGroupLayout)
fn create_shadow_pipeline(device, format, globals_layout) -> (RenderPipeline, BindGroupLayout)
fn create_gradient_pipeline(device, format, globals_layout) -> (RenderPipeline, BindGroupLayout)
fn create_color_picker_sv_pipeline(device, surface_format, rect_globals_layout) -> (RenderPipeline, BindGroupLayout)
fn create_radial_gauge_pipeline(device, surface_format, rect_globals_layout) -> (RenderPipeline, BindGroupLayout)
fn create_image_bind_group(device, layout, texture_view, sampler, rounded_clip_buffer) -> BindGroup
fn create_text_bind_group(device, layout, atlas, bg_view, bg_sampler, text_globals_buffer) -> BindGroup
fn needs_pre_rebuild_text_bind_group(bg_recreated, bind_group_missing, pipeline_generation_changed) -> bool
fn needs_post_rebuild_text_bind_group(bg_recreated, bind_group_missing, pipeline_generation_changed) -> bool
pub fn set_gpu_rounded_clips_enabled(&mut self, enabled: bool)
pub fn set_rounded_clip_aa_radius(&mut self, radius: f32)
pub fn set_text_gamma(&mut self, gamma: f32)
fn validate_pipelines(&mut self)
pub fn set_focused(&mut self, focused: bool); pub fn is_focused(&self) -> bool
fn ensure_globals_slab_capacity(&mut self, required_slots: usize)
fn ensure_path_clips_slab_capacity(&mut self, required_slots: usize, required_stride: u64)
pub fn advance_frame(&mut self)
pub fn mesh_cache(&mut self) -> &mut MeshCache
pub fn frame_counter(&self) -> u64
pub fn submit_and_present(&self, output: MultiTargetRenderOutput)
fn align_buffer_upload_size(value: u64, alignment: u64) -> u64
fn stage_buffer_upload(&mut self, destination: wgpu::Buffer, destination_offset: u64, data: &[u8])
fn flush_staging_uploads(&mut self)
fn poll_staging_recycles(&mut self)
fn build_gpu_update(&self, prev: Option<&FrameBatches>, curr: &FrameBatches, vertices: Vec<RectVertex>, indices: Vec<u16>) -> GpuUpdate
fn apply_gpu_update(&mut self, update: GpuUpdate)
fn extend_rebased_u16_indices(target: &mut Vec<u16>, indices: Vec<u16>, vertex_base: u32)
fn extend_rebased_u32_indices(target: &mut Vec<u32>, indices: Vec<u32>, vertex_base: u32)
fn build_text_buffers(text_quads: &[TextQuad], scale_factor: f32, physical_width: u32, physical_height: u32) -> (Vec<TextVertex>, Vec<u16>)
fn update_text_gpu_buffers(&mut self, vertices: &[TextVertex], indices: &[u16])
fn add_rect(&self, rect: &RectF, color: &Color, vertices: &mut Vec<RectVertex>, indices: &mut Vec<u16>)
fn add_image_quad(rect: &RectF, uv_rect: &RectF, tint: &Color, vertices: &mut Vec<ImageVertex>, indices: &mut Vec<u16>)
fn build_image_buffers_from_surface_primitives(primitives: &[FlatPrimitive]) -> (Vec<ImageVertex>, Vec<u16>)
fn update_image_gpu_buffers(&mut self, vertices: &[ImageVertex], indices: &[u16])
fn build_shadow_buffers_from_surface_primitives(primitives: &[FlatPrimitive]) -> (Vec<ShadowVertex>, Vec<u16>, Vec<(u64, ShadowParamsUniform)>)
fn update_shadow_gpu_buffers(&mut self, vertices: &[ShadowVertex], indices: &[u16])
fn upload_shadow_params(&mut self, unique_params: &[(u64, ShadowParamsUniform)]) -> HashMap<u64, u32>
fn build_gradient_buffers_from_surface_primitives(primitives: &[FlatPrimitive]) -> (Vec<GradientVertex>, Vec<u16>, Vec<(u64, GradientParamsUniform)>)
fn update_gradient_gpu_buffers(&mut self, vertices: &[GradientVertex], indices: &[u16])
fn upload_gradient_params(&mut self, unique_params: &[(u64, GradientParamsUniform)]) -> HashMap<u64, u32>
fn build_color_picker_sv_buffers_from_surface_primitives(primitives: &[FlatPrimitive], scale_factor: f32) -> (Vec<ColorPickerSvVertex>, Vec<u32>, HashMap<u64, (u32, ColorPickerSvParamsUniform)>)
fn update_color_picker_sv_gpu_buffers(&mut self, vertices: &[ColorPickerSvVertex], indices: &[u32])
fn upload_color_picker_sv_params(&mut self, params_map: &HashMap<u64, (u32, ColorPickerSvParamsUniform)>) -> HashMap<u64, u32>
fn build_radial_gauge_buffers_from_surface_primitives(primitives: &[FlatPrimitive], _scale_factor: f32) -> (Vec<RadialGaugeVertex>, Vec<u32>, HashMap<u64, (u32, RadialGaugeParamsUniform)>)
fn update_radial_gauge_gpu_buffers(&mut self, vertices: &[RadialGaugeVertex], indices: &[u32])
fn upload_radial_gauge_params(&mut self, params_map: &HashMap<u64, (u32, RadialGaugeParamsUniform)>) -> HashMap<u64, u32>
fn create_sdf_rect_pipeline(device, format, globals_layout) -> RenderPipeline
fn build_sdf_rect_buffers_from_surface_primitives(primitives: &[FlatPrimitive]) -> (Vec<SdfRectVertex>, Vec<u16>)
fn update_sdf_rect_gpu_buffers(&mut self, vertices: &[SdfRectVertex], indices: &[u16])
fn push_shape_quad(vertices: &mut Vec<ShapeVertex>, indices: &mut Vec<u16>, rect: RectF, color: [f32; 4], params0: [f32; 4], params1: [f32; 4], shape_type: ShapeType)
fn build_shape_buffers_from_surface_primitives(primitives: &[FlatPrimitive]) -> (Vec<ShapeVertex>, Vec<u16>)
fn update_shape_gpu_buffers(&mut self, vertices: &[ShapeVertex], indices: &[u16])
fn text_quad_to_ndc_for_target(rect: &RectF, scale_factor: f32, physical_width: u32, physical_height: u32) -> (f32, f32, f32, f32)
pub fn text_engine_mut(&mut self) -> &mut BackendTextEngine<…>   // four cfg-gated platform variants (:4020-:4035)
pub fn scale_factor(&self, surface_id: SurfaceId) -> f32
pub fn device(&self) -> Arc<wgpu::Device>; pub fn queue(&self) -> Arc<wgpu::Queue>
pub fn surface_format(&self) -> wgpu::TextureFormat
pub fn gpu_resources(&self) -> GpuResources
pub fn frame_epoch(&self) -> FrameEpoch
pub fn gpu_arena(&self) -> &GpuResourceArena; fn gpu_arena_mut(&mut self) -> &mut GpuResourceArena
fn staging_pool_mut(&mut self) -> &mut StagingBufferPool
pub(crate) fn staging_pool_total_bytes_for_test(&self) -> u64
pub(crate) fn pending_staging_work_for_test(&self) -> (usize, usize)
pub(crate) fn wait_for_staging_recycles_for_test(&mut self)
pub fn register_image_texture(&mut self, texture_id: u64, texture: wgpu::Texture)
pub fn register_image_from_view(&mut self, texture_id: u64, view: wgpu::TextureView)
pub fn register_image_from_view_nearest(&mut self, texture_id: u64, view: wgpu::TextureView)
pub fn drain_pending_image_registrations(&mut self)
pub fn unregister_image(&mut self, texture_id: u64)
pub fn register_image_rgba8_srgb(&mut self, texture_id: u64, w: u32, h: u32, rgba: &[u8])
pub fn set_scale_factor(&mut self, surface_id: SurfaceId, scale_factor: f64)
fn prepare_rounded_clips(&mut self, frame: &FrameBatches, logical_width: f32, logical_height: f32) -> HashMap<u64, u32>
fn upload_path_globals(&mut self, globals_offset: u32, logical_width: f32, logical_height: f32, physical_width: f32, physical_height: f32)
fn upload_path_clips_at_offset(&mut self, slot_offset: u64, path_clips: &[RectF], physical_width: f32, physical_height: f32, scale_factor: f32)
fn build_path_batch_from_frame(&mut self, frame: &FrameBatches)
pub fn create_surface_for_window(&self, window: Arc<winit::window::Window>) -> wgpu::Surface<'static>
pub fn register_render_target(&mut self, surface_id: SurfaceId, surface: wgpu::Surface<'static>, physical_size: (u32, u32), scale_factor: f32) -> Result<(), RenderTargetError>
pub fn unregister_render_target(&mut self, surface_id: SurfaceId) -> Result<(), RenderTargetError>
pub fn resize_render_target(&mut self, surface_id: SurfaceId, physical_size: (u32, u32)) -> Result<(), RenderTargetError>
pub fn render_target_count(&self) -> usize
pub fn has_render_target(&self, surface_id: SurfaceId) -> bool
pub fn compile_multi(&mut self, draw_plan: &DrawPlan<MultiSurfaceContract>) -> Result<Option<MultiTargetDrawPlan>, wgpu::SurfaceError>
fn build_buffers_from_surface_primitives(&self, primitives: &[FlatPrimitive], scale_factor: f32) -> (Vec<RectVertex>, Vec<u16>)
fn build_text_quads_from_surface_primitives(&mut self, primitives: &[FlatPrimitive], scale_factor: f32) -> Vec<TextQuad>
pub fn render_multi(&mut self, compiled: MultiTargetDrawPlan) -> Vec<MultiTargetRenderOutput>
```

## Not Extracted (scope statement)

All truncations recorded by the original generation are closed above —
no continuation commands remain. Out of scope for this generated file by
design: widget-corpus constructors (`src/widgets/*` beyond the tabular
types above — Rustdoc territory, SPEC-HAL-309 §2.5), the Plot public
surface (inventoried in `plot-and-market-detail.md` §3 from
`src/plot/mod.rs:31-69`), paragraph/text-editing internals, and
compositor/renderer types beyond `WgpuRenderer` itself. The widget
inventory file `src/widgets/mod.rs` remains dirty in the working tree
(unsettled market-detail unit) and is read only by the working-tree-
provenance catalog, not here.
