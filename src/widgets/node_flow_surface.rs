// SPEC-HAL-294: retained Widget adapter for the externally-owned node-flow editor.
//
// The binding stages immutable snapshots and outbound intents. The widget never
// mutates NodeGraphDocument, NodeEditorSession, NodeTypeCatalog, or
// PortTypeRegistry authority.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::event::{
    CursorIcon, Event, EventPhase, KeyCode, KeyboardAuthorityMode, Modifiers, PointerButton,
    PointerEvent, PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{Color, RoundedCorners};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::canvas::{
    CanvasBackground, CanvasEntityId, CanvasTransform, CanvasViewportState, ProjectionRevision,
    WorldPoint, WorldRect,
};
use crate::model::node_editor::{
    DanglingWireContext, NodeEditorIntent, NodeEditorSession, NodeFlowBuildError, NodeFlowConfig,
    NodeFlowCoordinator, NodeFlowInputs, NodeFlowIntentStatus, NodeSearchIntent, PortEndpoint,
    SelectionIntent, SessionIntent, WireDragOrigin, move_drag_transaction, wire_drag_transaction,
};
use crate::model::node_graph::{NodeGraphDocument, NodeTypeCatalog, PortTypeRegistry};
use crate::model::node_graph_projection::{
    FrameProjection, FrameVisualState, NodeGraphProjection, NodeProjection, NodeVisualState,
    PortProjection, WireProjection, WireStyle, WireVisualState, wire_path,
};
use crate::model::spatial::{
    CanvasSpatialBridge, SpatialCursorPolicy, SpatialGestureClass, SpatialGestureFacts,
    SpatialGridLineRole, SpatialGridPolicy, SpatialInteractionPolicy, SpatialOverlayAnchor,
    SpatialOverlayPlan, SpatialRoleVisual, SpatialVisualPolicy, realize_spatial_grid,
    resolve_spatial_overlay_rect,
};
use crate::utils::id::WidgetInternalId;

// INV-294-SURFACE-GEOMETRY: these are the retained node-flow realization's
// bounded fallback and legibility limits. All ordinary padding, radii, text,
// and border geometry comes from Theme or SpatialVisualPolicy tokens.
const FALLBACK_WIDTH: f32 = 640.0;
const FALLBACK_HEIGHT: f32 = 480.0;
const MIN_NODE_FONT_SIZE: f32 = 9.0;
const MIN_PORT_RADIUS: f32 = 2.5;
const MINIMAP_WIDTH: f32 = 180.0;
const MINIMAP_HEIGHT: f32 = 116.0;
const MINIMAP_ITEM_MIN_SIZE: f32 = 1.0;
const DOT_RADIUS: f32 = 1.0;
const CROSS_RADIUS: f32 = 1.5;
const MOVE_EPSILON: f64 = 0.01;
const Z_BACKGROUND: u32 = 0;
const Z_GRID: u32 = 1;
const Z_FRAME_FILL: u32 = 2;
const Z_FRAME_BORDER: u32 = 3;
const Z_FRAME_TEXT: u32 = 4;
const Z_WIRE: u32 = 8;
const Z_NODE_FILL: u32 = 10;
const Z_NODE_TITLE: u32 = 11;
const Z_NODE_BORDER: u32 = 12;
const Z_NODE_TEXT: u32 = 13;
const Z_PORT_OUTLINE: u32 = 16;
const Z_PORT_FILL: u32 = 17;
const Z_PREVIEW_FILL: u32 = 20;
const Z_PREVIEW_BORDER: u32 = 21;
const Z_WIRE_PREVIEW: u32 = 22;
const Z_MINIMAP: u32 = 30;
const Z_SURFACE_BORDER: u32 = 40;

#[derive(Clone)]
struct NodeFlowSurfaceSnapshot {
    document: NodeGraphDocument,
    session: NodeEditorSession,
    catalog: NodeTypeCatalog,
    port_registry: PortTypeRegistry,
    projection: NodeGraphProjection,
}

struct NodeFlowSurfaceMailbox {
    config: NodeFlowConfig,
    snapshot: Option<Rc<NodeFlowSurfaceSnapshot>>,
    generation: u64,
    intents: Vec<NodeEditorIntent>,
}

/// Owner/widget bridge for [`NodeFlowSurface`].
///
/// `publish` takes an owner-provided authority snapshot and replaces the
/// disposable projection copy wholesale. `take_intents` exposes all editor
/// intents in event order for the owner to apply. The mailbox contains no
/// write-back path to any authority.
#[derive(Clone)]
pub struct NodeFlowSurfaceBinding {
    inner: Rc<RefCell<NodeFlowSurfaceMailbox>>,
}

impl NodeFlowSurfaceBinding {
    pub fn new(config: NodeFlowConfig) -> Self {
        Self {
            inner: Rc::new(RefCell::new(NodeFlowSurfaceMailbox {
                config,
                snapshot: None,
                generation: 0,
                intents: Vec::new(),
            })),
        }
    }

    /// Rebuild and stage the disposable node-flow projection from external
    /// authorities. A failed build leaves the previously staged snapshot intact.
    pub fn publish(
        &self,
        inputs: NodeFlowInputs<'_>,
    ) -> Result<ProjectionRevision, NodeFlowBuildError> {
        let config = self.inner.borrow().config.clone();
        let projection = NodeGraphProjection::build(
            inputs.document,
            inputs.catalog,
            inputs.port_registry,
            &inputs.session.projection_config(&config),
            &inputs.session.projection_state(inputs.execution_overlay),
        )
        .map_err(NodeFlowBuildError::Projection)?;
        let revision = projection.revision();
        let snapshot = Rc::new(NodeFlowSurfaceSnapshot {
            document: inputs.document.clone(),
            session: inputs.session.clone(),
            catalog: inputs.catalog.clone(),
            port_registry: inputs.port_registry.clone(),
            projection,
        });

        let mut mailbox = self.inner.borrow_mut();
        mailbox.snapshot = Some(snapshot);
        mailbox.generation = mailbox.generation.wrapping_add(1);
        Ok(revision)
    }

    /// Drain intents emitted since the previous call. The owner remains
    /// responsible for applying them and publishing the resulting state.
    pub fn take_intents(&self) -> Vec<NodeEditorIntent> {
        std::mem::take(&mut self.inner.borrow_mut().intents)
    }

    pub fn has_snapshot(&self) -> bool {
        self.inner.borrow().snapshot.is_some()
    }

    fn config(&self) -> NodeFlowConfig {
        self.inner.borrow().config.clone()
    }

    fn snapshot(&self) -> Option<Rc<NodeFlowSurfaceSnapshot>> {
        self.inner.borrow().snapshot.clone()
    }

    fn generation(&self) -> u64 {
        self.inner.borrow().generation
    }

    fn push_intents(&self, intents: impl IntoIterator<Item = NodeEditorIntent>) {
        self.inner.borrow_mut().intents.extend(intents);
    }
}

impl Default for NodeFlowSurfaceBinding {
    fn default() -> Self {
        Self::new(NodeFlowConfig::default())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
enum SurfaceGesture {
    Idle,
    Pan {
        button: PointerButton,
        last_local: (f32, f32),
    },
    MoveNodes {
        button: PointerButton,
    },
    RubberBand {
        button: PointerButton,
    },
    Wire {
        button: PointerButton,
    },
}

/// Retained, mountable node-flow editor surface.
///
/// The surface owns only rendering, coordinate mechanics, input classification,
/// and transient pointer capture. Document/session/catalog/registry authority is
/// external through [`NodeFlowSurfaceBinding`].
pub struct NodeFlowSurface {
    id: WidgetInternalId,
    binding: NodeFlowSurfaceBinding,
    coordinator: NodeFlowCoordinator,
    interaction_policy: SpatialInteractionPolicy,
    gesture: SurfaceGesture,
    space_down: bool,
    last_size: Size,
    observed_generation: u64,
}

impl NodeFlowSurface {
    pub fn new(binding: NodeFlowSurfaceBinding) -> Self {
        let config = binding.config();
        let observed_generation = binding.generation();
        Self {
            id: WidgetInternalId::new(),
            binding,
            coordinator: NodeFlowCoordinator::new().with_config(config),
            interaction_policy: SpatialInteractionPolicy::default(),
            gesture: SurfaceGesture::Idle,
            space_down: false,
            last_size: Size::new(0.0, 0.0),
            observed_generation,
        }
    }

    pub fn binding(&self) -> NodeFlowSurfaceBinding {
        self.binding.clone()
    }

    fn emit_intent(&mut self, intent: NodeEditorIntent) -> bool {
        match self.coordinator.emit_intent(intent) {
            NodeFlowIntentStatus::Accepted => {
                let intents = self.coordinator.take_intents();
                self.binding.push_intents(intents);
                true
            }
            NodeFlowIntentStatus::RejectedReadOnly => false,
        }
    }

    fn emit_session(&mut self, intent: SessionIntent) -> bool {
        self.emit_intent(NodeEditorIntent::Session(intent))
    }

    fn world_from_local(snapshot: &NodeFlowSurfaceSnapshot, local: (f32, f32)) -> WorldPoint {
        snapshot
            .session
            .viewport
            .transform
            .screen_to_world(local.0, local.1)
    }

    fn gesture_class(&self, modifiers: Modifiers, button: PointerButton) -> SpatialGestureClass {
        self.interaction_policy.gesture_class(SpatialGestureFacts {
            modifiers,
            button,
            space_pressed: self.space_down,
        })
    }

    fn hit_entity(snapshot: &NodeFlowSurfaceSnapshot, world: WorldPoint) -> Option<CanvasEntityId> {
        snapshot
            .projection
            .canvas()
            .hit_test(world)
            .map(|hit| hit.id)
    }

    fn hit_port(snapshot: &NodeFlowSurfaceSnapshot, world: WorldPoint) -> Option<PortEndpoint> {
        let CanvasEntityId::Port {
            node_id,
            port_id,
            direction,
        } = Self::hit_entity(snapshot, world)?
        else {
            return None;
        };
        Some(PortEndpoint {
            node_id,
            port_id,
            direction,
        })
    }

    fn selection_entity(entity: CanvasEntityId) -> Option<CanvasEntityId> {
        match entity {
            CanvasEntityId::Port { node_id, .. } => Some(CanvasEntityId::Node(node_id)),
            CanvasEntityId::Node(_) | CanvasEntityId::Wire(_) | CanvasEntityId::Frame(_) => {
                Some(entity)
            }
            CanvasEntityId::Annotation(_) | CanvasEntityId::Custom(_) => None,
        }
    }

    fn begin_pan(
        &mut self,
        cx: &mut EventContext,
        button: PointerButton,
        local: (f32, f32),
        gesture: SpatialGestureClass,
    ) -> bool {
        if !self.interaction_policy.is_pan_gesture(gesture) {
            return false;
        }
        self.gesture = SurfaceGesture::Pan {
            button,
            last_local: local,
        };
        cx.capture_pointer(button);
        cx.request_cursor(cursor_icon(
            self.interaction_policy.cursor_for_gesture(gesture, true),
        ));
        true
    }

    fn handle_primary_down(
        &mut self,
        cx: &mut EventContext,
        pointer: &PointerEvent,
        snapshot: &NodeFlowSurfaceSnapshot,
    ) -> bool {
        let button = PointerButton::Primary;
        let gesture = self.gesture_class(pointer.modifiers, button);
        if self.begin_pan(cx, button, pointer.local_pos, gesture) {
            return true;
        }

        let world = Self::world_from_local(snapshot, pointer.local_pos);
        let hit = Self::hit_entity(snapshot, world);
        if hit.is_none() && self.interaction_policy.is_rubber_band_gesture(gesture) {
            let additive = self
                .interaction_policy
                .selection_toggle_requested(pointer.modifiers);
            if self.emit_session(SessionIntent::BeginRubberBand {
                start: world,
                additive,
            }) {
                self.gesture = SurfaceGesture::RubberBand { button };
                cx.capture_pointer(button);
                cx.request_cursor(cursor_icon(
                    self.interaction_policy.cursor_for_gesture(gesture, true),
                ));
                return true;
            }
        }

        if let Some(endpoint) = Self::hit_port(snapshot, world) {
            if self.emit_session(SessionIntent::BeginWireDrag {
                endpoint,
                cursor: world,
            }) {
                self.gesture = SurfaceGesture::Wire { button };
                cx.capture_pointer(button);
                cx.request_cursor(cursor_icon(
                    self.interaction_policy
                        .cursor_for_route_creation(gesture, true),
                ));
                return true;
            }
        }

        let Some(entity) = hit.and_then(Self::selection_entity) else {
            return self.emit_session(SessionIntent::Selection(SelectionIntent::Clear));
        };
        let additive = self
            .interaction_policy
            .selection_toggle_requested(pointer.modifiers);
        let already_selected = snapshot.session.selection.entities.contains(&entity);
        let selected = if !additive && already_selected {
            true
        } else {
            let selection = if additive {
                SelectionIntent::Toggle(entity)
            } else {
                SelectionIntent::Replace(entity)
            };
            self.emit_session(SessionIntent::Selection(selection))
        };

        if additive && already_selected {
            return selected;
        }
        if let CanvasEntityId::Node(_) = entity {
            if self.emit_session(SessionIntent::Selection(SelectionIntent::BeginMove {
                anchor: world,
                primary: entity,
            })) {
                self.gesture = SurfaceGesture::MoveNodes { button };
                cx.capture_pointer(button);
                cx.request_cursor(cursor_icon(
                    self.interaction_policy.cursor_for_gesture(gesture, true),
                ));
            }
        }
        selected
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, pointer: &PointerEvent) -> bool {
        let Some(snapshot) = self.binding.snapshot() else {
            return false;
        };

        match pointer.kind {
            PointerEventKind::Scroll { delta_y, .. } => {
                if delta_y == 0.0
                    || !self
                        .interaction_policy
                        .wheel_zoom_requested(pointer.modifiers)
                {
                    return false;
                }
                let factor = if delta_y > 0.0 { 1.15 } else { 0.87 };
                let intent = self.coordinator.zoom_around_screen_point_intent(
                    &snapshot.session,
                    factor,
                    pointer.local_pos.0,
                    pointer.local_pos.1,
                );
                self.emit_session(intent)
            }
            PointerEventKind::Down if pointer.button == Some(PointerButton::Auxiliary) => {
                cx.request_focus();
                let gesture = self.gesture_class(pointer.modifiers, PointerButton::Auxiliary);
                self.begin_pan(cx, PointerButton::Auxiliary, pointer.local_pos, gesture)
            }
            PointerEventKind::Down if pointer.button == Some(PointerButton::Primary) => {
                cx.request_focus();
                self.handle_primary_down(cx, pointer, &snapshot)
            }
            PointerEventKind::Down if pointer.button == Some(PointerButton::Secondary) => {
                cx.request_focus();
                let world = Self::world_from_local(&snapshot, pointer.local_pos);
                self.emit_intent(NodeEditorIntent::ContextMenu(
                    crate::model::node_editor::NodeContextMenuRequest {
                        position: world,
                        target: Self::hit_entity(&snapshot, world),
                    },
                ))
            }
            PointerEventKind::Move => self.update_gesture(cx, pointer, &snapshot),
            PointerEventKind::Up => self.commit_gesture(cx, pointer, &snapshot),
            PointerEventKind::DoubleClick
            | PointerEventKind::TripleClick
            | PointerEventKind::DragStart
            | PointerEventKind::Down => false,
        }
    }

    fn update_gesture(
        &mut self,
        cx: &mut EventContext,
        pointer: &PointerEvent,
        snapshot: &NodeFlowSurfaceSnapshot,
    ) -> bool {
        match self.gesture {
            SurfaceGesture::Pan { button, last_local } => {
                let delta = (
                    pointer.local_pos.0 - last_local.0,
                    pointer.local_pos.1 - last_local.1,
                );
                self.gesture = SurfaceGesture::Pan {
                    button,
                    last_local: pointer.local_pos,
                };
                if delta.0 == 0.0 && delta.1 == 0.0 {
                    return false;
                }
                let intent = self.coordinator.pan_by_screen_delta_intent(
                    &snapshot.session,
                    delta.0,
                    delta.1,
                );
                cx.request_cursor(CursorIcon::Move);
                self.emit_session(intent)
            }
            SurfaceGesture::MoveNodes { .. } => {
                let Some(drag) = &snapshot.session.drag else {
                    return false;
                };
                let world = Self::world_from_local(snapshot, pointer.local_pos);
                cx.request_cursor(CursorIcon::Move);
                self.emit_session(SessionIntent::Selection(SelectionIntent::UpdateMove {
                    delta: (world.x - drag.anchor.x, world.y - drag.anchor.y),
                }))
            }
            SurfaceGesture::RubberBand { .. } => {
                let world = Self::world_from_local(snapshot, pointer.local_pos);
                cx.request_cursor(CursorIcon::Crosshair);
                self.emit_session(SessionIntent::UpdateRubberBand { current: world })
            }
            SurfaceGesture::Wire { .. } => {
                let world = Self::world_from_local(snapshot, pointer.local_pos);
                let snapped_port = Self::hit_port(snapshot, world);
                cx.request_cursor(CursorIcon::Crosshair);
                self.emit_session(SessionIntent::UpdateWireDrag {
                    cursor: world,
                    snapped_port,
                })
            }
            SurfaceGesture::Idle => false,
        }
    }

    fn commit_gesture(
        &mut self,
        cx: &mut EventContext,
        pointer: &PointerEvent,
        snapshot: &NodeFlowSurfaceSnapshot,
    ) -> bool {
        let gesture = self.gesture;
        let expected_button = match gesture {
            SurfaceGesture::Pan { button, .. }
            | SurfaceGesture::MoveNodes { button }
            | SurfaceGesture::RubberBand { button }
            | SurfaceGesture::Wire { button } => button,
            SurfaceGesture::Idle => return false,
        };
        if pointer.button != Some(expected_button) {
            return false;
        }

        self.gesture = SurfaceGesture::Idle;
        cx.release_pointer();
        cx.request_cursor(CursorIcon::Default);
        let world = Self::world_from_local(snapshot, pointer.local_pos);
        match gesture {
            SurfaceGesture::Pan { .. } => true,
            SurfaceGesture::MoveNodes { .. } => {
                let Some(mut drag) = snapshot.session.drag.clone() else {
                    return false;
                };
                drag.delta = (world.x - drag.anchor.x, world.y - drag.anchor.y);
                let mut handled =
                    self.emit_session(SessionIntent::Selection(SelectionIntent::UpdateMove {
                        delta: drag.delta,
                    }));
                if (drag.delta.0.abs() >= MOVE_EPSILON || drag.delta.1.abs() >= MOVE_EPSILON)
                    && let Some(transaction) = move_drag_transaction(&snapshot.document, &drag)
                {
                    handled |= self.emit_intent(NodeEditorIntent::Graph(transaction));
                }
                handled | self.emit_session(SessionIntent::Selection(SelectionIntent::CommitMove))
            }
            SurfaceGesture::RubberBand { .. } => {
                let Some(rubber_band) = &snapshot.session.rubber_band else {
                    return self.emit_session(SessionIntent::CancelRubberBand);
                };
                let rect = normalized_world_rect(rubber_band.start, world);
                let mut handled =
                    self.emit_session(SessionIntent::UpdateRubberBand { current: world });
                handled |= self.emit_session(SessionIntent::Selection(
                    SelectionIntent::RubberBandCommit {
                        rect,
                        additive: rubber_band.additive,
                    },
                ));
                handled
            }
            SurfaceGesture::Wire { .. } => {
                let target = Self::hit_port(snapshot, world);
                let mut handled = false;
                if let Some(wire_drag) = &snapshot.session.wire_drag {
                    if let Ok(Some(transaction)) = wire_drag_transaction(
                        &snapshot.document,
                        &snapshot.catalog,
                        &snapshot.port_registry,
                        wire_drag,
                        target,
                    ) {
                        handled |= self.emit_intent(NodeEditorIntent::Graph(transaction));
                    } else if target.is_none()
                        && self.coordinator.config().auto_search_on_drop
                        && let Some(dangling_wire) =
                            dangling_wire_context(&snapshot.projection, &wire_drag.origin)
                    {
                        handled |=
                            self.emit_session(SessionIntent::Search(NodeSearchIntent::Open {
                                position: world,
                                dangling_wire: Some(dangling_wire),
                            }));
                    }
                }
                handled | self.emit_session(SessionIntent::CancelWireDrag)
            }
            SurfaceGesture::Idle => false,
        }
    }

    fn handle_key_event(&mut self, cx: &mut EventContext, event: &Event) -> bool {
        if cx.keyboard_mode != KeyboardAuthorityMode::Capture {
            return false;
        }
        if let Event::Key(key) = event {
            if key.key_code == KeyCode::Space {
                self.space_down = key.is_down;
                return true;
            }
        }

        let Some(snapshot) = self.binding.snapshot() else {
            return false;
        };
        let inputs = NodeFlowInputs {
            document: &snapshot.document,
            session: &snapshot.session,
            catalog: &snapshot.catalog,
            port_registry: &snapshot.port_registry,
            execution_overlay: None,
        };
        self.coordinator.handle_event(inputs, event);
        let intents = self.coordinator.take_intents();
        let handled = !intents.is_empty();
        self.binding.push_intents(intents);
        handled
    }

    fn cancel_active_gesture(&mut self) -> bool {
        let intent = match self.gesture {
            SurfaceGesture::MoveNodes { .. } => {
                Some(SessionIntent::Selection(SelectionIntent::CancelMove))
            }
            SurfaceGesture::RubberBand { .. } => Some(SessionIntent::CancelRubberBand),
            SurfaceGesture::Wire { .. } => Some(SessionIntent::CancelWireDrag),
            SurfaceGesture::Pan { .. } | SurfaceGesture::Idle => None,
        };
        self.gesture = SurfaceGesture::Idle;
        self.space_down = false;
        intent.is_some_and(|intent| self.emit_session(intent))
    }

    fn paint_snapshot(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        snapshot: &NodeFlowSurfaceSnapshot,
        policy: &SpatialVisualPolicy,
    ) {
        let transform = snapshot.session.viewport.transform;
        paint_grid(
            cx,
            rect,
            transform,
            self.coordinator.canvas().grid(),
            self.coordinator.config().background,
            policy,
        );
        let visible = visible_world_rect(rect, &transform);

        for frame in snapshot
            .projection
            .frames()
            .iter()
            .filter(|frame| frame.bounds().intersects(&visible))
        {
            paint_frame(cx, rect, &transform, frame, policy);
        }
        for wire in snapshot.projection.wires() {
            paint_wire(cx, rect, &transform, wire, policy);
        }
        for node in snapshot
            .projection
            .nodes()
            .iter()
            .filter(|node| node.bounds().intersects(&visible))
        {
            paint_node(cx, rect, &transform, node, policy);
        }
        for port in snapshot.projection.ports() {
            if visible.contains(port.world_pos) {
                paint_port(cx, rect, &transform, port, policy);
            }
        }
        if let Some(rubber_band) = &snapshot.session.rubber_band {
            paint_rubber_band(
                cx,
                rect,
                &transform,
                normalized_world_rect(rubber_band.start, rubber_band.current),
                policy,
            );
        }
        if let Some(wire_drag) = &snapshot.session.wire_drag {
            if let Some(origin) = wire_drag_origin_position(&snapshot.projection, &wire_drag.origin)
            {
                paint_wire_preview(
                    cx,
                    rect,
                    &transform,
                    origin,
                    wire_drag.cursor,
                    wire_drag.snapped_port.is_some(),
                    self.coordinator.config().wire_style,
                    policy,
                );
            }
        }
        if self.coordinator.config().show_minimap {
            paint_minimap(cx, rect, &snapshot.projection, policy);
        }
    }
}

impl WidgetDowncast for NodeFlowSurface {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for NodeFlowSurface {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            FALLBACK_WIDTH
        };
        let height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            FALLBACK_HEIGHT
        };
        let size = constraints.constrain(Size::new(width, height));
        if size != self.last_size {
            self.coordinator = std::mem::take(&mut self.coordinator)
                .with_viewport_size(size.width as f64, size.height as f64);
            self.last_size = size;
        }
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let policy = SpatialVisualPolicy::for_theme(cx.theme.is_dark());
        cx.scene
            .push_rect(rect, policy.canvas_background, Z_BACKGROUND, cx.scene_key);
        cx.scene.begin_clip(rect, None);
        if let Some(snapshot) = self.binding.snapshot() {
            self.paint_snapshot(cx, rect, &snapshot, &policy);
        }
        cx.scene.end_clip();

        let (border, width) = cx
            .theme
            .control_border(cx.has_focus_cue(), cx.has_capture(), false);
        cx.scene
            .push_border_rect(rect, border, width, Z_SURFACE_BORDER, cx.scene_key);
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        let generation = self.binding.generation();
        if generation != self.observed_generation {
            self.observed_generation = generation;
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.binding.generation() != self.observed_generation
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        let handled = match event {
            Event::Pointer(pointer) => self.handle_pointer_event(cx, pointer),
            Event::Key(_) => self.handle_key_event(cx, event),
            _ => false,
        };
        if handled {
            cx.invalidate_paint();
        }
        handled
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        if self.cancel_active_gesture() {
            cx.invalidate_paint();
        }
        cx.release_pointer();
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn clip_bounds(&self) -> Option<RectF> {
        Some(RectF::new(
            0.0,
            0.0,
            self.last_size.width,
            self.last_size.height,
        ))
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "NodeFlowSurface"
    }
}

fn cursor_icon(policy: SpatialCursorPolicy) -> CursorIcon {
    match policy {
        SpatialCursorPolicy::Default => CursorIcon::Default,
        SpatialCursorPolicy::Text => CursorIcon::Text,
        SpatialCursorPolicy::Link => CursorIcon::Pointer,
        SpatialCursorPolicy::ResizeHorizontal => CursorIcon::EwResize,
        SpatialCursorPolicy::ResizeVertical => CursorIcon::NsResize,
        SpatialCursorPolicy::DragActive => CursorIcon::Move,
        SpatialCursorPolicy::CrosshairActive => CursorIcon::Crosshair,
    }
}

fn normalized_world_rect(start: WorldPoint, current: WorldPoint) -> WorldRect {
    WorldRect::new(
        start.x.min(current.x),
        start.y.min(current.y),
        (start.x - current.x).abs(),
        (start.y - current.y).abs(),
    )
}

fn visible_world_rect(rect: RectF, transform: &CanvasTransform) -> WorldRect {
    let top_left = transform.screen_to_world(0.0, 0.0);
    let bottom_right = transform.screen_to_world(rect.width, rect.height);
    normalized_world_rect(top_left, bottom_right)
}

fn world_point_to_screen(rect: RectF, transform: &CanvasTransform, point: WorldPoint) -> Point {
    let (x, y) = transform.world_to_screen(point);
    Point::new(rect.x + x, rect.y + y)
}

fn world_rect_to_screen(rect: RectF, transform: &CanvasTransform, world: WorldRect) -> RectF {
    let top_left = world_point_to_screen(rect, transform, WorldPoint::new(world.x, world.y));
    RectF::new(
        top_left.x,
        top_left.y,
        (world.width * transform.zoom) as f32,
        (world.height * transform.zoom) as f32,
    )
}

fn paint_grid(
    cx: &mut PaintContext,
    rect: RectF,
    transform: CanvasTransform,
    canvas_grid: &crate::model::canvas::CanvasGrid,
    background: CanvasBackground,
    policy: &SpatialVisualPolicy,
) {
    if background == CanvasBackground::Solid || !canvas_grid.enabled {
        return;
    }
    let mut grid_policy = SpatialGridPolicy::default();
    grid_policy.enabled = canvas_grid.enabled;
    grid_policy.base_spacing = canvas_grid.base_spacing;
    grid_policy.major_every = canvas_grid.subdivisions.max(1);
    let Ok(transform) =
        CanvasSpatialBridge::spatial_transform(CanvasViewportState { transform }, rect)
    else {
        return;
    };
    let Ok(lines) = realize_spatial_grid(transform, grid_policy) else {
        return;
    };

    if background == CanvasBackground::Lines {
        for line in lines {
            let visual = grid_visual(policy, line.role);
            cx.scene
                .push_rect(line.rect, visual.stroke, Z_GRID, cx.scene_key);
        }
        return;
    }

    let vertical: Vec<_> = lines
        .iter()
        .filter(|line| line.rect.width <= line.rect.height)
        .collect();
    let horizontal: Vec<_> = lines
        .iter()
        .filter(|line| line.rect.width > line.rect.height)
        .collect();
    for x in &vertical {
        for y in &horizontal {
            let visual =
                if x.role == SpatialGridLineRole::Major || y.role == SpatialGridLineRole::Major {
                    policy.grid_major
                } else {
                    policy.grid_minor
                };
            let center = Point::new(x.rect.x, y.rect.y);
            if background == CanvasBackground::Dots {
                cx.scene
                    .push_circle(center, DOT_RADIUS, visual.stroke, Z_GRID, cx.scene_key);
            } else {
                cx.scene.push_rect(
                    RectF::new(
                        center.x - CROSS_RADIUS,
                        center.y,
                        CROSS_RADIUS * 2.0,
                        visual.stroke_width,
                    ),
                    visual.stroke,
                    Z_GRID,
                    cx.scene_key,
                );
                cx.scene.push_rect(
                    RectF::new(
                        center.x,
                        center.y - CROSS_RADIUS,
                        visual.stroke_width,
                        CROSS_RADIUS * 2.0,
                    ),
                    visual.stroke,
                    Z_GRID,
                    cx.scene_key,
                );
            }
        }
    }
}

fn grid_visual(policy: &SpatialVisualPolicy, role: SpatialGridLineRole) -> SpatialRoleVisual {
    match role {
        SpatialGridLineRole::Minor => policy.grid_minor,
        SpatialGridLineRole::Major => policy.grid_major,
    }
}

fn paint_frame(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    frame: &FrameProjection,
    policy: &SpatialVisualPolicy,
) {
    let rect = world_rect_to_screen(origin, transform, frame.bounds());
    let radius = RoundedCorners::uniform(cx.theme.radii.l);
    cx.scene.push_rounded_rect(
        rect,
        frame.color.with_alpha(0.10),
        radius,
        Z_FRAME_FILL,
        cx.scene_key,
    );
    let border = match frame.state {
        FrameVisualState::Selected | FrameVisualState::Resizing => policy.selection_outline.stroke,
        FrameVisualState::Hovered => policy.active_guide.stroke,
        FrameVisualState::Default => frame.color.with_alpha(0.62),
    };
    let width = if matches!(
        frame.state,
        FrameVisualState::Selected | FrameVisualState::Resizing
    ) {
        policy.selection_outline.stroke_width
    } else {
        cx.theme.borders.b1
    };
    cx.scene
        .push_border_rect(rect, border, width, Z_FRAME_BORDER, cx.scene_key);
    paint_text(
        cx,
        (rect.x + cx.theme.spacing.xl, rect.y + cx.theme.spacing.l),
        &frame.title,
        cx.theme.typography.font_size,
        cx.theme.colors.typography.secondary,
        Z_FRAME_TEXT,
    );
}

fn wire_visual(
    policy: &SpatialVisualPolicy,
    state: WireVisualState,
    fallback: Option<Color>,
) -> SpatialRoleVisual {
    let mut visual = match state {
        WireVisualState::Default | WireVisualState::Dimmed => policy.route_default,
        WireVisualState::Hovered => policy.route_hovered,
        WireVisualState::Selected => policy.route_selected,
        WireVisualState::Critical => policy.route_blocked,
        WireVisualState::Preview => policy.route_preview,
    };
    if state == WireVisualState::Default {
        if let Some(color) = fallback {
            visual.stroke = color;
        }
    } else if state == WireVisualState::Dimmed {
        visual.stroke = visual.stroke.with_alpha(0.45);
    }
    visual
}

fn paint_wire(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    wire: &WireProjection,
    policy: &SpatialVisualPolicy,
) {
    let visual = wire_visual(policy, wire.state, wire.color);
    let points: Vec<Point> = wire
        .path
        .flatten()
        .into_iter()
        .map(|point| {
            world_point_to_screen(
                origin,
                transform,
                WorldPoint::new(point[0] as f64, point[1] as f64),
            )
        })
        .collect();
    if points.len() >= 2 {
        cx.scene.push_polyline(
            points,
            visual.stroke_width,
            visual.stroke,
            Z_WIRE,
            cx.scene_key,
        );
    }
}

fn paint_node(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    node: &NodeProjection,
    policy: &SpatialVisualPolicy,
) {
    let rect = world_rect_to_screen(origin, transform, node.bounds());
    let radius = RoundedCorners::uniform(cx.theme.radii.l);
    let title_height = ((cx.theme.typography.line_height + cx.theme.spacing.m * 2.0)
        * transform.zoom as f32)
        .min(rect.height);
    let title_rect = RectF::new(rect.x, rect.y, rect.width, title_height);
    cx.scene.push_rounded_rect(
        rect,
        cx.theme.colors.surfaces.card,
        radius,
        Z_NODE_FILL,
        cx.scene_key,
    );
    cx.scene.push_rect(
        title_rect,
        node.color.with_alpha(0.88),
        Z_NODE_TITLE,
        cx.scene_key,
    );
    let border = match node.state {
        NodeVisualState::Selected => policy.selection_outline.stroke,
        NodeVisualState::Hovered => policy.active_guide.stroke,
        NodeVisualState::Error => cx.theme.colors.semantic.error,
        NodeVisualState::Running => cx.theme.colors.semantic.warning,
        NodeVisualState::Completed => cx.theme.colors.semantic.success,
        NodeVisualState::Default => cx.theme.colors.borders.default,
    };
    let border_width = if node.state == NodeVisualState::Selected {
        policy.selection_outline.stroke_width
    } else {
        cx.theme.borders.b1
    };
    cx.scene
        .push_border_rect(rect, border, border_width, Z_NODE_BORDER, cx.scene_key);

    let font_size = (cx.theme.typography.font_size * transform.zoom as f32)
        .clamp(MIN_NODE_FONT_SIZE, cx.theme.typography.font_size);
    cx.scene.begin_clip(title_rect, None);
    paint_text(
        cx,
        (
            rect.x + cx.theme.spacing.xl,
            rect.y + (title_height - font_size) * 0.5,
        ),
        &node.title,
        font_size,
        cx.theme.colors.typography.inverse,
        Z_NODE_TEXT,
    );
    cx.scene.end_clip();
}

fn paint_port(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    port: &PortProjection,
    policy: &SpatialVisualPolicy,
) {
    let center = world_point_to_screen(origin, transform, port.world_pos);
    let radius =
        (cx.theme.spacing.xs * transform.zoom as f32).clamp(MIN_PORT_RADIUS, cx.theme.spacing.s);
    let outline = if port.connection_count > 0 {
        policy.handle_anchor.stroke
    } else {
        policy.handle_anchor.stroke.with_alpha(0.58)
    };
    cx.scene.push_circle(
        center,
        radius + policy.handle_anchor.stroke_width,
        outline,
        Z_PORT_OUTLINE,
        cx.scene_key,
    );
    cx.scene
        .push_circle(center, radius, port.color, Z_PORT_FILL, cx.scene_key);
}

fn paint_rubber_band(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    world: WorldRect,
    policy: &SpatialVisualPolicy,
) {
    let rect = world_rect_to_screen(origin, transform, world);
    cx.scene.push_rounded_rect(
        rect,
        policy.drag_preview.fill,
        RoundedCorners::uniform(cx.theme.radii.s),
        Z_PREVIEW_FILL,
        cx.scene_key,
    );
    cx.scene.push_border_rect(
        rect,
        policy.drag_preview.stroke,
        policy.drag_preview.stroke_width,
        Z_PREVIEW_BORDER,
        cx.scene_key,
    );
}

fn wire_drag_origin_position(
    projection: &NodeGraphProjection,
    origin: &WireDragOrigin,
) -> Option<WorldPoint> {
    let endpoint = match origin {
        WireDragOrigin::Output(endpoint) | WireDragOrigin::Input(endpoint) => *endpoint,
        WireDragOrigin::Reroute { output, .. } => *output,
    };
    projection
        .ports()
        .find(|port| {
            port.node_id == endpoint.node_id
                && port.definition.port_id == endpoint.port_id
                && port.definition.direction == endpoint.direction
        })
        .map(|port| port.world_pos)
}

fn dangling_wire_context(
    projection: &NodeGraphProjection,
    origin: &WireDragOrigin,
) -> Option<DanglingWireContext> {
    let endpoint = match origin {
        WireDragOrigin::Output(endpoint) | WireDragOrigin::Input(endpoint) => *endpoint,
        WireDragOrigin::Reroute { output, .. } => *output,
    };
    let port = projection.ports().find(|port| {
        port.node_id == endpoint.node_id
            && port.definition.port_id == endpoint.port_id
            && port.definition.direction == endpoint.direction
    })?;
    Some(DanglingWireContext {
        endpoint,
        port_type: port.definition.type_id,
    })
}

fn paint_wire_preview(
    cx: &mut PaintContext,
    origin: RectF,
    transform: &CanvasTransform,
    from: WorldPoint,
    to: WorldPoint,
    snapped: bool,
    style: WireStyle,
    policy: &SpatialVisualPolicy,
) {
    let visual = if snapped {
        policy.route_selected
    } else {
        policy.route_preview
    };
    let points: Vec<Point> = wire_path(style, from, to)
        .flatten()
        .into_iter()
        .map(|point| {
            world_point_to_screen(
                origin,
                transform,
                WorldPoint::new(point[0] as f64, point[1] as f64),
            )
        })
        .collect();
    if points.len() >= 2 {
        cx.scene.push_polyline(
            points,
            visual.stroke_width,
            visual.stroke,
            Z_WIRE_PREVIEW,
            cx.scene_key,
        );
    }
}

fn paint_minimap(
    cx: &mut PaintContext,
    viewport: RectF,
    projection: &NodeGraphProjection,
    policy: &SpatialVisualPolicy,
) {
    let rect = resolve_spatial_overlay_rect(
        viewport,
        SpatialOverlayPlan {
            anchor: SpatialOverlayAnchor::BottomRight,
            offset: Point::zero(),
            size: RectF::new(0.0, 0.0, MINIMAP_WIDTH, MINIMAP_HEIGHT),
            edge_padding: policy.hud_edge_padding,
        },
    );
    cx.scene.push_rounded_rect(
        rect,
        policy.hud_background,
        RoundedCorners::uniform(cx.theme.radii.l),
        Z_MINIMAP,
        cx.scene_key,
    );
    cx.scene.push_border_rect(
        rect,
        policy.hud_border,
        cx.theme.borders.b1,
        Z_MINIMAP + 1,
        cx.scene_key,
    );

    let Some(bounds) = projection.canvas().world_bounds() else {
        return;
    };
    let padding = cx.theme.spacing.m;
    let available_width = (rect.width - padding * 2.0).max(MINIMAP_ITEM_MIN_SIZE);
    let available_height = (rect.height - padding * 2.0).max(MINIMAP_ITEM_MIN_SIZE);
    let scale = (available_width as f64 / bounds.width.max(f64::EPSILON))
        .min(available_height as f64 / bounds.height.max(f64::EPSILON));
    let map_rect = |world: WorldRect| {
        RectF::new(
            rect.x + padding + ((world.x - bounds.x) * scale) as f32,
            rect.y + padding + ((world.y - bounds.y) * scale) as f32,
            (world.width * scale) as f32,
            (world.height * scale) as f32,
        )
    };
    for frame in projection.frames() {
        cx.scene.push_rect(
            map_rect(frame.bounds()),
            frame.color.with_alpha(0.22),
            Z_MINIMAP + 2,
            cx.scene_key,
        );
    }
    for node in projection.nodes() {
        let mut mapped = map_rect(node.bounds());
        mapped.width = mapped.width.max(MINIMAP_ITEM_MIN_SIZE);
        mapped.height = mapped.height.max(MINIMAP_ITEM_MIN_SIZE);
        cx.scene.push_rect(
            mapped,
            node.color.with_alpha(0.78),
            Z_MINIMAP + 3,
            cx.scene_key,
        );
    }
}

fn paint_text(
    cx: &mut PaintContext,
    position: (f32, f32),
    text: &str,
    font_size: f32,
    color: Color,
    z_index: u32,
) {
    let run = cx.text_engine.shape(text, font_size);
    cx.scene.push_text_with_backgrounds(
        &run,
        position.0,
        position.1,
        color,
        Vec::new(),
        z_index,
        None,
    );
}
