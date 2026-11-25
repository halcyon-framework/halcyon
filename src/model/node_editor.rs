use std::collections::{HashMap, HashSet};

use crate::core::event::{Event, KeyCode};
use crate::core::icon::IconId;
use crate::model::canvas::{
    CanvasBackground, CanvasEntityId, CanvasProjection, CanvasViewportState, InfiniteCanvas,
    ProjectedCanvasItem, ProjectionRevision, WorldPoint, WorldRect,
};
use crate::model::node_graph::{
    GraphEntityId, GraphTransaction, NodeGraphDocument, NodeType, NodeTypeCatalog, PortCardinality,
    PortDefinition, PortDirection, PortTypeId, PortTypeRegistry,
};
use crate::model::node_graph_projection::{
    FrameProjection, FrameVisualState, NodeGraphProjection, NodeGraphProjectionConfig,
    NodeGraphProjectionError, NodeGraphProjectionState, NodeProjection, NodeVisualState, WireStyle,
    WireVisualState,
};
use crate::model::spatial::{SpatialGuideAxis, SpatialGuidePlan, SpatialGuideRole};
use crate::runtime::actions::{ActionId, Shortcut};
use crate::runtime::binding_registry::{
    Binding, BindingLayer, BindingPriority, BindingRegistry, InputPattern,
};

pub const DEFAULT_FRAME_PADDING: f64 = 32.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Default)]
pub struct SessionRevision(pub u64);

#[derive(Debug, Clone)]
pub struct NodeEditorSession {
    pub viewport: CanvasViewportState,
    pub selection: SelectionSet,
    pub active_tool: NodeEditorTool,
    pub drag: Option<DragSession>,
    pub frame_resize: Option<FrameResizeSession>,
    pub wire_drag: Option<WireDragSession>,
    pub rubber_band: Option<RubberBandSession>,
    pub search: Option<NodeSearchSession>,
    pub inline_edit: Option<InlineEditSession>,
    pub clipboard: Option<NodeClipboard>,
    pub revision: SessionRevision,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NodeEditorTool {
    #[default]
    Select,
    Pan,
    Connect,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SelectionSet {
    pub entities: Vec<CanvasEntityId>,
    pub anchor: Option<CanvasEntityId>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DragSession {
    pub anchor: WorldPoint,
    pub primary: CanvasEntityId,
    pub entities: Vec<CanvasEntityId>,
    pub delta: (f64, f64),
    pub frame_captures: Vec<FrameDragCapture>,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignmentSnapConfig {
    pub enabled: bool,
    pub tolerance: f64,
    pub snap_edges: bool,
    pub snap_centers: bool,
    pub structural_guides: bool,
    pub structural_guides_per_axis: usize,
    pub structural_guide_radius: f64,
}

impl Default for AlignmentSnapConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            tolerance: 8.0,
            snap_edges: true,
            snap_centers: true,
            structural_guides: true,
            structural_guides_per_axis: 1,
            structural_guide_radius: 96.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentGuideAxis {
    Vertical,
    Horizontal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AlignmentGuideRole {
    ActiveSnap,
    Structural,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlignmentGuide {
    pub axis: AlignmentGuideAxis,
    pub role: AlignmentGuideRole,
    pub position: f64,
    pub span_start: f64,
    pub span_end: f64,
}

impl From<AlignmentGuide> for SpatialGuidePlan {
    fn from(value: AlignmentGuide) -> Self {
        let axis = match value.axis {
            AlignmentGuideAxis::Vertical => SpatialGuideAxis::Vertical,
            AlignmentGuideAxis::Horizontal => SpatialGuideAxis::Horizontal,
        };
        let role = match value.role {
            AlignmentGuideRole::ActiveSnap => SpatialGuideRole::Active,
            AlignmentGuideRole::Structural => SpatialGuideRole::Structural,
        };
        SpatialGuidePlan {
            axis,
            role,
            position: value.position,
            start: value.span_start,
            end: value.span_end,
        }
    }
}

pub fn spatial_guides_from_alignment_guides(guides: &[AlignmentGuide]) -> Vec<SpatialGuidePlan> {
    guides.iter().copied().map(SpatialGuidePlan::from).collect()
}

#[derive(Debug, Clone, PartialEq)]
pub struct AlignmentSnapResult {
    pub delta: (f64, f64),
    pub guides: Vec<AlignmentGuide>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrameDragCapture {
    pub frame_id: u64,
    pub node_ids: Vec<u64>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameResizeSession {
    pub frame_id: u64,
    pub original_size: (f64, f64),
    pub current_size: (f64, f64),
}

#[derive(Debug, Clone, PartialEq)]
pub struct RubberBandSession {
    pub start: WorldPoint,
    pub current: WorldPoint,
    pub additive: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct InlineEditSession {
    pub entity: CanvasEntityId,
    pub draft: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeClipboard {
    pub entities: Vec<GraphEntityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PortEndpoint {
    pub node_id: u64,
    pub port_id: u32,
    pub direction: PortDirection,
}

#[derive(Debug, Clone, PartialEq)]
pub struct WireDragSession {
    pub origin: WireDragOrigin,
    pub cursor: WorldPoint,
    pub snapped_port: Option<PortEndpoint>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum WireDragOrigin {
    Output(PortEndpoint),
    Input(PortEndpoint),
    Reroute {
        connection_id: u64,
        output: PortEndpoint,
        original_input: PortEndpoint,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DanglingWireContext {
    pub endpoint: PortEndpoint,
    pub port_type: PortTypeId,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeSearchEntry {
    pub type_id: u64,
    pub name: String,
    pub category: String,
    pub description: Option<String>,
    pub input_port_types: Vec<PortTypeId>,
    pub output_port_types: Vec<PortTypeId>,
    pub icon: Option<IconId>,
    pub frequency: u32,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NodeSearchMetadata {
    pub description: Option<String>,
    pub icon: Option<IconId>,
    pub frequency: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeSearchSession {
    pub position: WorldPoint,
    pub query: String,
    pub selected_index: usize,
    pub dangling_wire: Option<DanglingWireContext>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchCursorDelta {
    Previous,
    Next,
}

#[derive(Debug, Clone, PartialEq)]
pub enum SelectionIntent {
    Replace(CanvasEntityId),
    Toggle(CanvasEntityId),
    SelectAll,
    Clear,
    RubberBandCommit {
        rect: WorldRect,
        additive: bool,
    },
    BeginMove {
        anchor: WorldPoint,
        primary: CanvasEntityId,
    },
    UpdateMove {
        delta: (f64, f64),
    },
    CancelMove,
    CommitMove,
    DeleteSelected,
    DuplicateSelected,
    CopySelected,
    PasteClipboard {
        position: WorldPoint,
    },
    CutSelected,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NodeSearchIntent {
    Open {
        position: WorldPoint,
        dangling_wire: Option<DanglingWireContext>,
    },
    UpdateQuery(String),
    MoveCursor(SearchCursorDelta),
    Close,
    CommitCreate {
        type_id: u64,
        position: WorldPoint,
    },
    CommitCreateAndConnect {
        type_id: u64,
        position: WorldPoint,
        dangling_wire: DanglingWireContext,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub enum SessionIntent {
    Selection(SelectionIntent),
    Search(NodeSearchIntent),
    BeginRubberBand {
        start: WorldPoint,
        additive: bool,
    },
    UpdateRubberBand {
        current: WorldPoint,
    },
    CancelRubberBand,
    BeginWireDrag {
        endpoint: PortEndpoint,
        cursor: WorldPoint,
    },
    UpdateWireDrag {
        cursor: WorldPoint,
        snapped_port: Option<PortEndpoint>,
    },
    CancelWireDrag,
    CancelFrameResize,
    CancelInlineEdit,
    SetViewport(CanvasViewportState),
    SetTool(NodeEditorTool),
}

#[derive(Debug, Clone)]
pub enum NodeEditorIntent {
    Session(SessionIntent),
    Graph(GraphTransaction),
    Undo,
    Redo,
    ContextMenu(NodeContextMenuRequest),
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeContextMenuRequest {
    pub position: WorldPoint,
    pub target: Option<CanvasEntityId>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeSearchCommit {
    Create {
        type_id: u64,
    },
    CreateAndConnect {
        type_id: u64,
        dangling_wire: DanglingWireContext,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct NodeFlowConfig {
    pub read_only: bool,
    pub show_minimap: bool,
    pub wire_style: WireStyle,
    pub show_arrowheads: bool,
    pub snap_to_grid: bool,
    pub auto_search_on_drop: bool,
    pub background: CanvasBackground,
}

impl Default for NodeFlowConfig {
    fn default() -> Self {
        Self {
            read_only: false,
            show_minimap: true,
            wire_style: WireStyle::Bezier,
            show_arrowheads: false,
            snap_to_grid: true,
            auto_search_on_drop: true,
            background: CanvasBackground::Dots,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NodeFlowCoordinator {
    canvas: InfiniteCanvas,
    config: NodeFlowConfig,
    bindings: BindingRegistry,
    intents: Vec<NodeEditorIntent>,
}

/// Public top-level node flow composition surface.
pub type NodeFlowView = NodeFlowCoordinator;

impl NodeFlowCoordinator {
    pub fn new() -> Self {
        Self {
            canvas: InfiniteCanvas::new(),
            config: NodeFlowConfig::default(),
            bindings: node_flow_binding_registry(),
            intents: Vec::new(),
        }
    }

    pub fn with_config(mut self, config: NodeFlowConfig) -> Self {
        self.canvas = self.canvas.clone().with_background(config.background);
        self.config = config;
        self
    }

    pub fn with_viewport_size(mut self, width: f64, height: f64) -> Self {
        self.canvas = self.canvas.with_viewport_size(width, height);
        self
    }

    pub fn config(&self) -> &NodeFlowConfig {
        &self.config
    }

    pub fn canvas(&self) -> &InfiniteCanvas {
        &self.canvas
    }

    pub fn build_node_graph_projection(
        &self,
        inputs: NodeFlowInputs<'_>,
    ) -> Result<NodeGraphProjection, NodeFlowBuildError> {
        NodeGraphProjection::build(
            inputs.document,
            inputs.catalog,
            inputs.port_registry,
            &inputs.session.projection_config(&self.config),
            &inputs.session.projection_state(inputs.execution_overlay),
        )
        .map_err(NodeFlowBuildError::Projection)
    }

    pub fn build_projection(
        &self,
        inputs: NodeFlowInputs<'_>,
    ) -> Result<CanvasProjection, NodeFlowBuildError> {
        Ok(self.build_node_graph_projection(inputs)?.canvas().clone())
    }

    pub fn emit_intent(&mut self, intent: NodeEditorIntent) -> NodeFlowIntentStatus {
        if self.config.read_only && !read_only_allows_intent(&intent) {
            return NodeFlowIntentStatus::RejectedReadOnly;
        }

        self.intents.push(intent);
        NodeFlowIntentStatus::Accepted
    }

    pub fn take_intents(&mut self) -> Vec<NodeEditorIntent> {
        std::mem::take(&mut self.intents)
    }

    pub fn handle_event(&mut self, inputs: NodeFlowInputs<'_>, event: &Event) {
        let action_id = match event {
            Event::Key(key) if key.is_down => {
                let pattern = InputPattern::from_key_modifiers(key.key_code, key.modifiers);
                self.bindings.lookup(&pattern)
            }
            _ => None,
        };
        let Some(action_id) = action_id else {
            return;
        };

        let Ok(projection) = self.build_projection(inputs) else {
            return;
        };
        let viewport_center = self.canvas.viewport_center(&inputs.session.viewport);
        let grid_unit = self.node_flow_grid_unit();
        let context = NodeFlowActionContext {
            session: inputs.session,
            document: inputs.document,
            projection: Some(&projection),
            pointer_position: None,
            viewport_center,
            grid_unit,
        };
        let intents = self.action_intents(action_id, context);
        for intent in intents {
            let _ = self.emit_intent(intent);
        }
    }

    pub fn zoom_to_fit_intent(
        &mut self,
        session: &NodeEditorSession,
        projection: &CanvasProjection,
    ) -> Option<SessionIntent> {
        let mut viewport = session.viewport;
        let changed = self.canvas.zoom_to_fit(&mut viewport, projection);
        let _ = self.canvas.take_intents();
        changed.then_some(SessionIntent::SetViewport(viewport))
    }

    pub fn zoom_to_node_intent(
        &mut self,
        session: &NodeEditorSession,
        projection: &CanvasProjection,
        node_id: u64,
    ) -> Option<SessionIntent> {
        let item = projection
            .iter()
            .find(|item| item.id == CanvasEntityId::Node(node_id))?
            .clone();
        let node_projection = projection_from_item(projection.revision(), item);
        self.zoom_to_fit_intent(session, &node_projection)
    }

    pub fn pan_to_intent(&self, session: &NodeEditorSession, pos: WorldPoint) -> SessionIntent {
        let mut viewport = session.viewport;
        viewport.transform.offset_x = pos.x;
        viewport.transform.offset_y = pos.y;
        SessionIntent::SetViewport(viewport)
    }

    pub fn reset_view_intent(&self, _session: &NodeEditorSession) -> SessionIntent {
        SessionIntent::SetViewport(CanvasViewportState::default())
    }

    pub fn zoom_in_intent(&mut self, session: &NodeEditorSession) -> SessionIntent {
        self.zoom_by_intent(session, 1.2)
    }

    pub fn zoom_out_intent(&mut self, session: &NodeEditorSession) -> SessionIntent {
        self.zoom_by_intent(session, 1.0 / 1.2)
    }

    pub fn pan_by_screen_delta_intent(
        &mut self,
        session: &NodeEditorSession,
        delta_x: f32,
        delta_y: f32,
    ) -> SessionIntent {
        let mut viewport = session.viewport;
        self.canvas
            .pan_by_screen_delta(&mut viewport, delta_x, delta_y);
        let _ = self.canvas.take_intents();
        SessionIntent::SetViewport(viewport)
    }

    pub fn zoom_around_screen_point_intent(
        &mut self,
        session: &NodeEditorSession,
        factor: f64,
        screen_x: f32,
        screen_y: f32,
    ) -> SessionIntent {
        let mut viewport = session.viewport;
        let zoom = viewport.transform.zoom * factor;
        self.canvas
            .zoom_around_screen_point(&mut viewport, zoom, screen_x, screen_y);
        let _ = self.canvas.take_intents();
        SessionIntent::SetViewport(viewport)
    }

    pub fn action_intents(
        &mut self,
        action_id: ActionId,
        context: NodeFlowActionContext<'_>,
    ) -> Vec<NodeEditorIntent> {
        let position = context.pointer_position.unwrap_or(context.viewport_center);
        let grid_unit = context.grid_unit.max(1.0);
        let mut intents = Vec::new();

        match action_id {
            ActionId::NODE_FLOW_DELETE_SELECTED => {
                if let Some(transaction) = selection_delete_transaction(&context.session.selection)
                {
                    intents.push(NodeEditorIntent::Graph(transaction));
                }
            }
            ActionId::NODE_FLOW_SELECT_ALL => {
                intents.push(NodeEditorIntent::Session(SessionIntent::Selection(
                    SelectionIntent::SelectAll,
                )));
            }
            ActionId::NODE_FLOW_DUPLICATE_SELECTED => {
                if let Some(transaction) =
                    selection_duplicate_transaction(&context.session.selection, (20.0, 20.0))
                {
                    intents.push(NodeEditorIntent::Graph(transaction));
                }
            }
            ActionId::NODE_FLOW_COPY_SELECTED => {
                intents.push(NodeEditorIntent::Session(SessionIntent::Selection(
                    SelectionIntent::CopySelected,
                )));
            }
            ActionId::NODE_FLOW_PASTE => {
                if let Some(clipboard) = &context.session.clipboard {
                    if let Some(transaction) =
                        clipboard_paste_transaction(context.document, clipboard, position)
                    {
                        intents.push(NodeEditorIntent::Graph(transaction));
                    }
                }
            }
            ActionId::NODE_FLOW_CUT_SELECTED => {
                intents.push(NodeEditorIntent::Session(SessionIntent::Selection(
                    SelectionIntent::CutSelected,
                )));
                if let Some(transaction) = selection_delete_transaction(&context.session.selection)
                {
                    intents.push(NodeEditorIntent::Graph(transaction));
                }
            }
            ActionId::NODE_FLOW_UNDO => intents.push(NodeEditorIntent::Undo),
            ActionId::NODE_FLOW_REDO => intents.push(NodeEditorIntent::Redo),
            ActionId::NODE_FLOW_OPEN_SEARCH => {
                intents.push(NodeEditorIntent::Session(SessionIntent::Search(
                    NodeSearchIntent::Open {
                        position: context.viewport_center,
                        dangling_wire: None,
                    },
                )));
            }
            ActionId::NODE_FLOW_CANCEL => {
                intents.extend(cancel_action_intents(context.session));
            }
            ActionId::NODE_FLOW_ZOOM_TO_FIT => {
                if let Some(projection) = context.projection {
                    if let Some(intent) = self.zoom_to_fit_intent(context.session, projection) {
                        intents.push(NodeEditorIntent::Session(intent));
                    }
                }
            }
            ActionId::NODE_FLOW_RESET_VIEW => {
                intents.push(NodeEditorIntent::Session(
                    self.reset_view_intent(context.session),
                ));
            }
            ActionId::NODE_FLOW_ZOOM_IN => {
                intents.push(NodeEditorIntent::Session(
                    self.zoom_in_intent(context.session),
                ));
            }
            ActionId::NODE_FLOW_ZOOM_OUT => {
                intents.push(NodeEditorIntent::Session(
                    self.zoom_out_intent(context.session),
                ));
            }
            ActionId::NODE_FLOW_NUDGE_LEFT => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (-grid_unit, 0.0),
                );
            }
            ActionId::NODE_FLOW_NUDGE_RIGHT => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (grid_unit, 0.0),
                );
            }
            ActionId::NODE_FLOW_NUDGE_UP => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (0.0, -grid_unit),
                );
            }
            ActionId::NODE_FLOW_NUDGE_DOWN => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (0.0, grid_unit),
                );
            }
            ActionId::NODE_FLOW_NUDGE_LEFT_FAST => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (-grid_unit * 10.0, 0.0),
                );
            }
            ActionId::NODE_FLOW_NUDGE_RIGHT_FAST => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (grid_unit * 10.0, 0.0),
                );
            }
            ActionId::NODE_FLOW_NUDGE_UP_FAST => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (0.0, -grid_unit * 10.0),
                );
            }
            ActionId::NODE_FLOW_NUDGE_DOWN_FAST => {
                push_nudge_intent(
                    &mut intents,
                    context.document,
                    context.session,
                    (0.0, grid_unit * 10.0),
                );
            }
            _ => {}
        }

        if self.config.read_only {
            intents.retain(read_only_allows_intent);
        }

        intents
    }

    fn zoom_by_intent(&mut self, session: &NodeEditorSession, factor: f64) -> SessionIntent {
        let mut viewport = session.viewport;
        let zoom = viewport.transform.zoom * factor;
        self.canvas.zoom_to(&mut viewport, zoom);
        let _ = self.canvas.take_intents();
        SessionIntent::SetViewport(viewport)
    }

    fn node_flow_grid_unit(&self) -> f64 {
        if self.config.snap_to_grid {
            self.canvas.grid().base_spacing.max(1.0)
        } else {
            1.0
        }
    }
}

impl Default for NodeFlowCoordinator {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct NodeFlowInputs<'a> {
    pub document: &'a NodeGraphDocument,
    pub session: &'a NodeEditorSession,
    pub catalog: &'a NodeTypeCatalog,
    pub port_registry: &'a PortTypeRegistry,
    pub execution_overlay: Option<&'a ExecutionOverlay>,
}

#[derive(Debug, Clone, Default)]
pub struct ExecutionOverlay {
    pub node_states: HashMap<u64, NodeVisualState>,
    pub wire_states: HashMap<u64, WireVisualState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeEditorSessionError {
    UnknownNode {
        node_id: u64,
    },
    UnknownNodeType {
        node_id: u64,
        type_id: u64,
    },
    UnknownPort {
        endpoint: PortEndpoint,
    },
    InvalidDirection {
        endpoint: PortEndpoint,
    },
    IncompatiblePorts {
        from: PortEndpoint,
        to: PortEndpoint,
    },
    NoWireDrag,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NodeFlowIntentStatus {
    Accepted,
    RejectedReadOnly,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NodeFlowBuildError {
    Projection(NodeGraphProjectionError),
}

#[derive(Debug, Clone, Copy)]
pub struct NodeFlowActionContext<'a> {
    pub session: &'a NodeEditorSession,
    pub document: &'a NodeGraphDocument,
    pub projection: Option<&'a CanvasProjection>,
    pub pointer_position: Option<WorldPoint>,
    pub viewport_center: WorldPoint,
    pub grid_unit: f64,
}

impl From<NodeGraphProjectionError> for NodeFlowBuildError {
    fn from(value: NodeGraphProjectionError) -> Self {
        Self::Projection(value)
    }
}

fn node_flow_binding_registry() -> BindingRegistry {
    let mut registry = BindingRegistry::new();
    for binding in node_flow_default_bindings() {
        registry.add(binding);
    }
    registry
}

pub fn node_flow_default_bindings() -> Vec<Binding> {
    vec![
        node_flow_key_binding(KeyCode::Delete, ActionId::NODE_FLOW_DELETE_SELECTED),
        node_flow_key_binding(KeyCode::Backspace, ActionId::NODE_FLOW_DELETE_SELECTED),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('a'), false),
            ActionId::NODE_FLOW_SELECT_ALL,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('d'), false),
            ActionId::NODE_FLOW_DUPLICATE_SELECTED,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('c'), false),
            ActionId::NODE_FLOW_COPY_SELECTED,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('v'), false),
            ActionId::NODE_FLOW_PASTE,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('x'), false),
            ActionId::NODE_FLOW_CUT_SELECTED,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('z'), false),
            ActionId::NODE_FLOW_UNDO,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('y'), false),
            ActionId::NODE_FLOW_REDO,
        ),
        node_flow_shortcut_binding(
            Shortcut::platform_command(KeyCode::Character('z'), true),
            ActionId::NODE_FLOW_REDO,
        ),
        node_flow_key_binding(KeyCode::Tab, ActionId::NODE_FLOW_OPEN_SEARCH),
        node_flow_key_binding(KeyCode::Escape, ActionId::NODE_FLOW_CANCEL),
        node_flow_key_binding(KeyCode::Character('f'), ActionId::NODE_FLOW_ZOOM_TO_FIT),
        node_flow_key_binding(KeyCode::Home, ActionId::NODE_FLOW_RESET_VIEW),
        node_flow_key_binding(KeyCode::Character('0'), ActionId::NODE_FLOW_RESET_VIEW),
        node_flow_key_binding(KeyCode::Character('+'), ActionId::NODE_FLOW_ZOOM_IN),
        node_flow_key_binding(KeyCode::Character('='), ActionId::NODE_FLOW_ZOOM_IN),
        node_flow_key_binding(KeyCode::Character('-'), ActionId::NODE_FLOW_ZOOM_OUT),
        node_flow_key_binding(KeyCode::Left, ActionId::NODE_FLOW_NUDGE_LEFT),
        node_flow_key_binding(KeyCode::Right, ActionId::NODE_FLOW_NUDGE_RIGHT),
        node_flow_key_binding(KeyCode::Up, ActionId::NODE_FLOW_NUDGE_UP),
        node_flow_key_binding(KeyCode::Down, ActionId::NODE_FLOW_NUDGE_DOWN),
        node_flow_shift_key_binding(KeyCode::Left, ActionId::NODE_FLOW_NUDGE_LEFT_FAST),
        node_flow_shift_key_binding(KeyCode::Right, ActionId::NODE_FLOW_NUDGE_RIGHT_FAST),
        node_flow_shift_key_binding(KeyCode::Up, ActionId::NODE_FLOW_NUDGE_UP_FAST),
        node_flow_shift_key_binding(KeyCode::Down, ActionId::NODE_FLOW_NUDGE_DOWN_FAST),
    ]
}

fn node_flow_key_binding(key: KeyCode, action_id: ActionId) -> Binding {
    Binding {
        pattern: InputPattern::key_only(key),
        action_id,
        layer: BindingLayer::App,
        priority: BindingPriority::Normal,
        tag: Some("node_flow"),
    }
}

fn node_flow_shift_key_binding(key: KeyCode, action_id: ActionId) -> Binding {
    Binding {
        pattern: InputPattern {
            key: key.into(),
            ctrl: false,
            alt: false,
            shift: true,
            meta: false,
        },
        action_id,
        layer: BindingLayer::App,
        priority: BindingPriority::Normal,
        tag: Some("node_flow"),
    }
}

fn node_flow_shortcut_binding(shortcut: Shortcut, action_id: ActionId) -> Binding {
    Binding {
        pattern: InputPattern::from(&shortcut),
        action_id,
        layer: BindingLayer::App,
        priority: BindingPriority::Normal,
        tag: Some("node_flow"),
    }
}

impl Default for NodeEditorSession {
    fn default() -> Self {
        Self {
            viewport: CanvasViewportState::default(),
            selection: SelectionSet::default(),
            active_tool: NodeEditorTool::Select,
            drag: None,
            frame_resize: None,
            wire_drag: None,
            rubber_band: None,
            search: None,
            inline_edit: None,
            clipboard: None,
            revision: SessionRevision::default(),
        }
    }
}

impl NodeEditorSession {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn apply_selection_intent(
        &mut self,
        intent: SelectionIntent,
        projection: &CanvasProjection,
    ) {
        match intent {
            SelectionIntent::Replace(entity) => self.selection.replace(entity),
            SelectionIntent::Toggle(entity) => self.selection.toggle(entity),
            SelectionIntent::SelectAll => self.selection.select_all(projection),
            SelectionIntent::Clear => self.selection.clear(),
            SelectionIntent::RubberBandCommit { rect, additive } => {
                self.selection
                    .rubber_band_commit(projection, rect, additive);
                self.rubber_band = None;
            }
            SelectionIntent::BeginMove { anchor, primary } => {
                let mut entities = self.selection.entities.clone();
                if !entities.contains(&primary) && is_selectable_entity(primary) {
                    entities = vec![primary];
                    self.selection.replace(primary);
                }
                self.drag = Some(DragSession {
                    anchor,
                    primary,
                    entities,
                    delta: (0.0, 0.0),
                    frame_captures: Vec::new(),
                });
            }
            SelectionIntent::UpdateMove { delta } => {
                if let Some(drag) = &mut self.drag {
                    drag.delta = delta;
                }
            }
            SelectionIntent::CancelMove | SelectionIntent::CommitMove => {
                self.drag = None;
            }
            SelectionIntent::CopySelected => {
                self.clipboard = Some(NodeClipboard {
                    entities: self.selection.graph_entities(),
                });
            }
            SelectionIntent::CutSelected => {
                self.clipboard = Some(NodeClipboard {
                    entities: self.selection.graph_entities(),
                });
            }
            SelectionIntent::DeleteSelected
            | SelectionIntent::DuplicateSelected
            | SelectionIntent::PasteClipboard { .. } => {}
        }
        self.bump_revision();
    }

    pub fn begin_frame_drag(
        &mut self,
        anchor: WorldPoint,
        primary_frame_id: u64,
        frames: &[FrameProjection],
    ) -> bool {
        let primary = CanvasEntityId::Frame(primary_frame_id);
        let mut entities = self.selection.entities.clone();
        if !entities.contains(&primary) {
            entities = vec![primary];
            self.selection.replace(primary);
        }

        let selected_frame_ids: HashSet<u64> = entities
            .iter()
            .filter_map(|entity| match *entity {
                CanvasEntityId::Frame(id) => Some(id),
                _ => None,
            })
            .collect();
        let mut frame_captures: Vec<FrameDragCapture> = frames
            .iter()
            .filter(|frame| selected_frame_ids.contains(&frame.id))
            .map(|frame| FrameDragCapture {
                frame_id: frame.id,
                node_ids: frame.contained_nodes.clone(),
            })
            .collect();
        frame_captures.sort_by_key(|capture| capture.frame_id);

        if !frame_captures
            .iter()
            .any(|capture| capture.frame_id == primary_frame_id)
        {
            return false;
        }

        self.drag = Some(DragSession {
            anchor,
            primary,
            entities,
            delta: (0.0, 0.0),
            frame_captures,
        });
        self.bump_revision();
        true
    }

    pub fn begin_frame_resize(&mut self, frame_id: u64, original_size: (f64, f64)) -> bool {
        if !valid_frame_size(original_size) {
            return false;
        }

        self.selection.replace(CanvasEntityId::Frame(frame_id));
        self.frame_resize = Some(FrameResizeSession {
            frame_id,
            original_size,
            current_size: original_size,
        });
        self.bump_revision();
        true
    }

    pub fn update_frame_resize(&mut self, new_size: (f64, f64)) -> bool {
        if !valid_frame_size(new_size) {
            return false;
        }

        let Some(resize) = &mut self.frame_resize else {
            return false;
        };
        resize.current_size = new_size;
        self.bump_revision();
        true
    }

    pub fn cancel_frame_resize(&mut self) {
        if self.frame_resize.take().is_some() {
            self.bump_revision();
        }
    }

    pub fn commit_frame_resize(
        &mut self,
        document: &NodeGraphDocument,
    ) -> Option<GraphTransaction> {
        let resize = self.frame_resize.take()?;
        let transaction = frame_resize_transaction(document, &resize);
        self.bump_revision();
        transaction
    }

    pub fn begin_rubber_band(&mut self, start: WorldPoint, additive: bool) {
        self.rubber_band = Some(RubberBandSession {
            start,
            current: start,
            additive,
        });
        self.bump_revision();
    }

    pub fn update_rubber_band(&mut self, current: WorldPoint) {
        if let Some(rubber_band) = &mut self.rubber_band {
            rubber_band.current = current;
            self.bump_revision();
        }
    }

    pub fn cancel_rubber_band(&mut self) {
        if self.rubber_band.take().is_some() {
            self.bump_revision();
        }
    }

    pub fn begin_wire_drag(
        &mut self,
        document: &NodeGraphDocument,
        catalog: &NodeTypeCatalog,
        endpoint: PortEndpoint,
        cursor: WorldPoint,
    ) -> Result<(), NodeEditorSessionError> {
        let definition = port_definition(document, catalog, endpoint)?;
        let origin = match definition.direction {
            PortDirection::Output => WireDragOrigin::Output(endpoint),
            PortDirection::Input => {
                let reroute_connection = document
                    .connections_for_port(endpoint.node_id, endpoint.port_id)
                    .into_iter()
                    .find(|connection| {
                        connection.to_node == endpoint.node_id
                            && connection.to_port == endpoint.port_id
                    });

                if let Some(connection) = reroute_connection {
                    WireDragOrigin::Reroute {
                        connection_id: connection.id,
                        output: PortEndpoint {
                            node_id: connection.from_node,
                            port_id: connection.from_port,
                            direction: PortDirection::Output,
                        },
                        original_input: endpoint,
                    }
                } else {
                    WireDragOrigin::Input(endpoint)
                }
            }
        };

        self.wire_drag = Some(WireDragSession {
            origin,
            cursor,
            snapped_port: None,
        });
        self.bump_revision();
        Ok(())
    }

    pub fn update_wire_drag(
        &mut self,
        cursor: WorldPoint,
        snapped_port: Option<PortEndpoint>,
    ) -> Result<(), NodeEditorSessionError> {
        let Some(wire_drag) = &mut self.wire_drag else {
            return Err(NodeEditorSessionError::NoWireDrag);
        };
        wire_drag.cursor = cursor;
        wire_drag.snapped_port = snapped_port;
        self.bump_revision();
        Ok(())
    }

    pub fn commit_wire_drag(
        &mut self,
        document: &NodeGraphDocument,
        catalog: &NodeTypeCatalog,
        registry: &PortTypeRegistry,
        target: Option<PortEndpoint>,
    ) -> Result<Option<GraphTransaction>, NodeEditorSessionError> {
        let Some(wire_drag) = self.wire_drag.take() else {
            return Err(NodeEditorSessionError::NoWireDrag);
        };
        let transaction = wire_drag_transaction(document, catalog, registry, &wire_drag, target)?;

        self.bump_revision();
        Ok(transaction)
    }

    pub fn cancel_wire_drag(&mut self) {
        if self.wire_drag.take().is_some() {
            self.bump_revision();
        }
    }

    pub fn open_search(
        &mut self,
        position: WorldPoint,
        dangling_wire: Option<DanglingWireContext>,
    ) {
        self.search = Some(NodeSearchSession {
            position,
            query: String::new(),
            selected_index: 0,
            dangling_wire,
        });
        self.bump_revision();
    }

    pub fn update_search_query(&mut self, query: impl Into<String>) {
        if let Some(search) = &mut self.search {
            search.query = query.into();
            search.selected_index = 0;
            self.bump_revision();
        }
    }

    pub fn move_search_cursor(&mut self, delta: SearchCursorDelta, result_count: usize) {
        if result_count == 0 {
            if let Some(search) = &mut self.search {
                search.selected_index = 0;
                self.bump_revision();
            }
            return;
        }

        if let Some(search) = &mut self.search {
            search.selected_index = match delta {
                SearchCursorDelta::Previous => search
                    .selected_index
                    .checked_sub(1)
                    .unwrap_or(result_count - 1),
                SearchCursorDelta::Next => (search.selected_index + 1) % result_count,
            };
            self.bump_revision();
        }
    }

    pub fn close_search(&mut self) {
        if self.search.take().is_some() {
            self.bump_revision();
        }
    }

    pub fn begin_inline_edit(&mut self, entity: CanvasEntityId, draft: impl Into<String>) -> bool {
        if !matches!(entity, CanvasEntityId::Node(_) | CanvasEntityId::Frame(_)) {
            return false;
        }

        self.inline_edit = Some(InlineEditSession {
            entity,
            draft: draft.into(),
        });
        self.bump_revision();
        true
    }

    pub fn update_inline_edit(&mut self, draft: impl Into<String>) -> bool {
        let Some(inline_edit) = &mut self.inline_edit else {
            return false;
        };
        inline_edit.draft = draft.into();
        self.bump_revision();
        true
    }

    pub fn cancel_inline_edit(&mut self) {
        if self.inline_edit.take().is_some() {
            self.bump_revision();
        }
    }

    pub fn commit_inline_edit(&mut self, document: &NodeGraphDocument) -> Option<GraphTransaction> {
        let inline_edit = self.inline_edit.take()?;
        let transaction = inline_edit_transaction(document, &inline_edit);
        self.bump_revision();
        transaction
    }

    pub fn commit_selected_search_result(
        &self,
        results: &[NodeSearchEntry],
    ) -> Option<NodeSearchCommit> {
        let search = self.search.as_ref()?;
        let entry = results.get(search.selected_index)?;
        Some(match search.dangling_wire {
            Some(dangling_wire) => NodeSearchCommit::CreateAndConnect {
                type_id: entry.type_id,
                dangling_wire,
            },
            None => NodeSearchCommit::Create {
                type_id: entry.type_id,
            },
        })
    }

    pub fn projection_state(
        &self,
        execution: Option<&ExecutionOverlay>,
    ) -> NodeGraphProjectionState {
        let mut state = NodeGraphProjectionState::default();
        state.revision_salt = self.revision.0;

        if let Some(drag) = &self.drag {
            for capture in &drag.frame_captures {
                state.frame_offsets.insert(capture.frame_id, drag.delta);
                for node_id in &capture.node_ids {
                    state.node_offsets.insert(*node_id, drag.delta);
                }
            }
            for entity in &drag.entities {
                match *entity {
                    CanvasEntityId::Node(id) => {
                        state.node_offsets.insert(id, drag.delta);
                    }
                    CanvasEntityId::Frame(id) => {
                        state.frame_offsets.insert(id, drag.delta);
                    }
                    CanvasEntityId::Wire(_)
                    | CanvasEntityId::Port { .. }
                    | CanvasEntityId::Annotation(_)
                    | CanvasEntityId::Custom(_) => {}
                }
            }
        }

        for entity in &self.selection.entities {
            match *entity {
                CanvasEntityId::Node(id) => {
                    state.node_states.insert(id, NodeVisualState::Selected);
                }
                CanvasEntityId::Wire(id) => {
                    state.wire_states.insert(id, WireVisualState::Selected);
                }
                CanvasEntityId::Frame(id) => {
                    state.frame_states.insert(id, FrameVisualState::Selected);
                }
                CanvasEntityId::Port { .. }
                | CanvasEntityId::Annotation(_)
                | CanvasEntityId::Custom(_) => {}
            }
        }

        if let Some(resize) = &self.frame_resize {
            state
                .frame_size_overrides
                .insert(resize.frame_id, resize.current_size);
            state
                .frame_states
                .insert(resize.frame_id, FrameVisualState::Resizing);
        }

        if let Some(execution) = execution {
            state.node_states.extend(execution.node_states.clone());
            state.wire_states.extend(execution.wire_states.clone());
        }

        state
    }

    pub fn projection_config(&self, config: &NodeFlowConfig) -> NodeGraphProjectionConfig {
        NodeGraphProjectionConfig {
            geometry: NodeGraphProjectionConfig::default().geometry,
            wire_style: config.wire_style,
            show_arrowheads: config.show_arrowheads,
        }
    }

    fn bump_revision(&mut self) {
        self.revision.0 += 1;
    }
}

impl SelectionSet {
    pub fn replace(&mut self, entity: CanvasEntityId) {
        if is_selectable_entity(entity) {
            self.entities = vec![entity];
            self.anchor = Some(entity);
        }
    }

    pub fn toggle(&mut self, entity: CanvasEntityId) {
        if !is_selectable_entity(entity) {
            return;
        }

        if let Some(index) = self
            .entities
            .iter()
            .position(|selected| *selected == entity)
        {
            self.entities.remove(index);
            if self.anchor == Some(entity) {
                self.anchor = self.entities.first().copied();
            }
        } else {
            self.entities.push(entity);
            self.anchor = Some(entity);
        }
    }

    pub fn clear(&mut self) {
        self.entities.clear();
        self.anchor = None;
    }

    pub fn select_all(&mut self, projection: &CanvasProjection) {
        self.entities = selectable_entities(projection);
        self.anchor = self.entities.first().copied();
    }

    pub fn rubber_band_commit(
        &mut self,
        projection: &CanvasProjection,
        rect: WorldRect,
        additive: bool,
    ) {
        let hits: Vec<CanvasEntityId> = projection
            .in_region(&rect)
            .into_iter()
            .map(|item| item.id)
            .filter(|entity| is_selectable_entity(*entity))
            .collect();
        if additive {
            for entity in hits {
                if !self.entities.contains(&entity) {
                    self.entities.push(entity);
                }
            }
        } else {
            self.entities = hits;
        }
        self.anchor = self.entities.first().copied();
    }

    pub fn graph_entities(&self) -> Vec<GraphEntityId> {
        let mut graph_entities = Vec::new();
        let mut seen = HashSet::new();
        for entity in &self.entities {
            let graph_entity = match *entity {
                CanvasEntityId::Node(id) => GraphEntityId::Node(id),
                CanvasEntityId::Wire(id) => GraphEntityId::Connection(id),
                CanvasEntityId::Frame(id) => GraphEntityId::Frame(id),
                CanvasEntityId::Port { .. }
                | CanvasEntityId::Annotation(_)
                | CanvasEntityId::Custom(_) => continue,
            };
            if seen.insert(graph_entity) {
                graph_entities.push(graph_entity);
            }
        }
        graph_entities
    }
}

#[derive(Debug, Clone, Copy)]
struct SnapCandidate {
    axis: AlignmentGuideAxis,
    correction: f64,
    target_position: f64,
    moving_rect: WorldRect,
    target_rect: WorldRect,
}

#[derive(Debug, Clone, Copy)]
struct StructuralGuideCandidate {
    axis: AlignmentGuideAxis,
    position: f64,
    distance: f64,
    moving_rect: WorldRect,
    target_rect: WorldRect,
}

pub fn resolve_alignment_snap(
    moving: &[WorldRect],
    stationary: &[WorldRect],
    requested_delta: (f64, f64),
    config: AlignmentSnapConfig,
) -> AlignmentSnapResult {
    if !config.enabled || config.tolerance <= 0.0 || moving.is_empty() || stationary.is_empty() {
        return AlignmentSnapResult {
            delta: requested_delta,
            guides: Vec::new(),
        };
    }

    let mut best_vertical: Option<SnapCandidate> = None;
    let mut best_horizontal: Option<SnapCandidate> = None;

    for moving_rect in moving {
        let translated = translate_rect(*moving_rect, requested_delta);
        for target_rect in stationary {
            for moving_x in snap_x_positions(translated, config) {
                for target_x in snap_x_positions(*target_rect, config) {
                    let correction = target_x - moving_x;
                    if correction.abs() <= config.tolerance {
                        update_snap_candidate(
                            &mut best_vertical,
                            SnapCandidate {
                                axis: AlignmentGuideAxis::Vertical,
                                correction,
                                target_position: target_x,
                                moving_rect: *moving_rect,
                                target_rect: *target_rect,
                            },
                        );
                    }
                }
            }

            for moving_y in snap_y_positions(translated, config) {
                for target_y in snap_y_positions(*target_rect, config) {
                    let correction = target_y - moving_y;
                    if correction.abs() <= config.tolerance {
                        update_snap_candidate(
                            &mut best_horizontal,
                            SnapCandidate {
                                axis: AlignmentGuideAxis::Horizontal,
                                correction,
                                target_position: target_y,
                                moving_rect: *moving_rect,
                                target_rect: *target_rect,
                            },
                        );
                    }
                }
            }
        }
    }

    let resolved_delta = (
        requested_delta.0 + best_vertical.map_or(0.0, |candidate| candidate.correction),
        requested_delta.1 + best_horizontal.map_or(0.0, |candidate| candidate.correction),
    );
    let mut guides = Vec::new();
    if let Some(candidate) = best_vertical {
        guides.push(alignment_guide(
            candidate,
            resolved_delta,
            AlignmentGuideRole::ActiveSnap,
        ));
    }
    if let Some(candidate) = best_horizontal {
        guides.push(alignment_guide(
            candidate,
            resolved_delta,
            AlignmentGuideRole::ActiveSnap,
        ));
    }
    append_structural_guides(
        &mut guides,
        moving,
        stationary,
        resolved_delta,
        config,
        best_vertical.map(|candidate| candidate.target_position),
        best_horizontal.map(|candidate| candidate.target_position),
    );

    AlignmentSnapResult {
        delta: resolved_delta,
        guides,
    }
}

fn update_snap_candidate(current: &mut Option<SnapCandidate>, candidate: SnapCandidate) {
    let replace = current
        .as_ref()
        .map(|current| candidate.correction.abs() < current.correction.abs())
        .unwrap_or(true);
    if replace {
        *current = Some(candidate);
    }
}

fn append_structural_guides(
    guides: &mut Vec<AlignmentGuide>,
    moving: &[WorldRect],
    stationary: &[WorldRect],
    resolved_delta: (f64, f64),
    config: AlignmentSnapConfig,
    active_vertical: Option<f64>,
    active_horizontal: Option<f64>,
) {
    if !config.structural_guides
        || config.structural_guides_per_axis == 0
        || config.structural_guide_radius <= 0.0
    {
        return;
    }

    let mut vertical = Vec::new();
    let mut horizontal = Vec::new();

    for moving_rect in moving {
        let translated = translate_rect(*moving_rect, resolved_delta);
        let moving_x_positions = snap_x_positions(translated, config);
        let moving_y_positions = snap_y_positions(translated, config);

        for target_rect in stationary {
            for target_x in snap_x_positions(*target_rect, config) {
                if active_vertical.map_or(false, |position| same_guide_position(position, target_x))
                {
                    continue;
                }
                if let Some(distance) = nearest_position_distance(target_x, &moving_x_positions) {
                    if distance <= config.structural_guide_radius {
                        update_structural_candidate(
                            &mut vertical,
                            StructuralGuideCandidate {
                                axis: AlignmentGuideAxis::Vertical,
                                position: target_x,
                                distance,
                                moving_rect: *moving_rect,
                                target_rect: *target_rect,
                            },
                        );
                    }
                }
            }

            for target_y in snap_y_positions(*target_rect, config) {
                if active_horizontal
                    .map_or(false, |position| same_guide_position(position, target_y))
                {
                    continue;
                }
                if let Some(distance) = nearest_position_distance(target_y, &moving_y_positions) {
                    if distance <= config.structural_guide_radius {
                        update_structural_candidate(
                            &mut horizontal,
                            StructuralGuideCandidate {
                                axis: AlignmentGuideAxis::Horizontal,
                                position: target_y,
                                distance,
                                moving_rect: *moving_rect,
                                target_rect: *target_rect,
                            },
                        );
                    }
                }
            }
        }
    }

    push_structural_guides(guides, &mut vertical, resolved_delta, config);
    push_structural_guides(guides, &mut horizontal, resolved_delta, config);
}

fn update_structural_candidate(
    candidates: &mut Vec<StructuralGuideCandidate>,
    candidate: StructuralGuideCandidate,
) {
    if let Some(existing) = candidates.iter_mut().find(|existing| {
        existing.axis == candidate.axis
            && same_guide_position(existing.position, candidate.position)
    }) {
        if candidate.distance < existing.distance {
            *existing = candidate;
        }
        return;
    }

    candidates.push(candidate);
}

fn push_structural_guides(
    guides: &mut Vec<AlignmentGuide>,
    candidates: &mut [StructuralGuideCandidate],
    resolved_delta: (f64, f64),
    config: AlignmentSnapConfig,
) {
    candidates.sort_by(|a, b| {
        a.distance
            .partial_cmp(&b.distance)
            .unwrap_or(std::cmp::Ordering::Equal)
    });

    for candidate in candidates.iter().take(config.structural_guides_per_axis) {
        guides.push(alignment_guide(
            SnapCandidate {
                axis: candidate.axis,
                correction: 0.0,
                target_position: candidate.position,
                moving_rect: candidate.moving_rect,
                target_rect: candidate.target_rect,
            },
            resolved_delta,
            AlignmentGuideRole::Structural,
        ));
    }
}

fn nearest_position_distance(position: f64, positions: &[f64]) -> Option<f64> {
    positions
        .iter()
        .map(|candidate| (position - candidate).abs())
        .min_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal))
}

fn same_guide_position(a: f64, b: f64) -> bool {
    (a - b).abs() <= 0.001
}

fn alignment_guide(
    candidate: SnapCandidate,
    resolved_delta: (f64, f64),
    role: AlignmentGuideRole,
) -> AlignmentGuide {
    let moving = translate_rect(candidate.moving_rect, resolved_delta);
    match candidate.axis {
        AlignmentGuideAxis::Vertical => AlignmentGuide {
            axis: candidate.axis,
            role,
            position: candidate.target_position,
            span_start: moving.y.min(candidate.target_rect.y) - 24.0,
            span_end: rect_bottom(moving).max(rect_bottom(candidate.target_rect)) + 24.0,
        },
        AlignmentGuideAxis::Horizontal => AlignmentGuide {
            axis: candidate.axis,
            role,
            position: candidate.target_position,
            span_start: moving.x.min(candidate.target_rect.x) - 24.0,
            span_end: rect_right(moving).max(rect_right(candidate.target_rect)) + 24.0,
        },
    }
}

fn translate_rect(rect: WorldRect, delta: (f64, f64)) -> WorldRect {
    WorldRect::new(rect.x + delta.0, rect.y + delta.1, rect.width, rect.height)
}

fn snap_x_positions(rect: WorldRect, config: AlignmentSnapConfig) -> Vec<f64> {
    let mut positions = Vec::with_capacity(3);
    if config.snap_edges {
        positions.push(rect.x);
        positions.push(rect_right(rect));
    }
    if config.snap_centers {
        positions.push(rect.center().x);
    }
    positions
}

fn snap_y_positions(rect: WorldRect, config: AlignmentSnapConfig) -> Vec<f64> {
    let mut positions = Vec::with_capacity(3);
    if config.snap_edges {
        positions.push(rect.y);
        positions.push(rect_bottom(rect));
    }
    if config.snap_centers {
        positions.push(rect.center().y);
    }
    positions
}

fn rect_right(rect: WorldRect) -> f64 {
    rect.x + rect.width
}

fn rect_bottom(rect: WorldRect) -> f64 {
    rect.y + rect.height
}

impl NodeSearchEntry {
    pub fn from_node_type(node_type: &NodeType, metadata: NodeSearchMetadata) -> NodeSearchEntry {
        NodeSearchEntry {
            type_id: node_type.type_id,
            name: node_type.name.clone(),
            category: node_type.category.clone(),
            description: metadata.description,
            input_port_types: node_type
                .input_ports
                .iter()
                .map(|port| port.type_id)
                .collect(),
            output_port_types: node_type
                .output_ports
                .iter()
                .map(|port| port.type_id)
                .collect(),
            icon: metadata.icon,
            frequency: metadata.frequency,
        }
    }
}

pub fn node_search_entries(
    catalog: &NodeTypeCatalog,
    metadata: &HashMap<u64, NodeSearchMetadata>,
) -> Vec<NodeSearchEntry> {
    let mut entries: Vec<NodeSearchEntry> = catalog
        .types()
        .map(|node_type| {
            NodeSearchEntry::from_node_type(
                node_type,
                metadata
                    .get(&node_type.type_id)
                    .cloned()
                    .unwrap_or_default(),
            )
        })
        .collect();
    entries.sort_by(|left, right| {
        left.category
            .cmp(&right.category)
            .then_with(|| left.name.cmp(&right.name))
            .then_with(|| left.type_id.cmp(&right.type_id))
    });
    entries
}

pub fn search_node_entries(
    entries: &[NodeSearchEntry],
    query: &str,
    dangling_wire: Option<DanglingWireContext>,
    registry: &PortTypeRegistry,
) -> Vec<NodeSearchEntry> {
    let mut results: Vec<(NodeSearchEntry, i32)> = entries
        .iter()
        .filter(|entry| {
            dangling_wire.is_none_or(|wire| node_entry_accepts_dangling_wire(entry, wire, registry))
        })
        .filter_map(|entry| {
            if query.is_empty() {
                Some((entry.clone(), 1))
            } else {
                let score = node_search_score(query, entry);
                (score > 0).then_some((entry.clone(), score))
            }
        })
        .collect();

    if query.is_empty() {
        results.sort_by(|(left, _), (right, _)| {
            right
                .frequency
                .cmp(&left.frequency)
                .then_with(|| left.category.cmp(&right.category))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.type_id.cmp(&right.type_id))
        });
    } else {
        results.sort_by(|(left, left_score), (right, right_score)| {
            right_score
                .cmp(left_score)
                .then_with(|| left.category.cmp(&right.category))
                .then_with(|| left.name.cmp(&right.name))
                .then_with(|| left.type_id.cmp(&right.type_id))
        });
    }

    results.into_iter().map(|(entry, _)| entry).collect()
}

pub fn node_entry_accepts_dangling_wire(
    entry: &NodeSearchEntry,
    dangling_wire: DanglingWireContext,
    registry: &PortTypeRegistry,
) -> bool {
    match dangling_wire.endpoint.direction {
        PortDirection::Output => entry
            .input_port_types
            .iter()
            .any(|input_type| registry.are_compatible(dangling_wire.port_type, *input_type)),
        PortDirection::Input => entry
            .output_port_types
            .iter()
            .any(|output_type| registry.are_compatible(*output_type, dangling_wire.port_type)),
    }
}

pub fn node_search_score(query: &str, entry: &NodeSearchEntry) -> i32 {
    let name_score = fuzzy_score(query, &entry.name);
    let category_score = fuzzy_score(query, &entry.category);
    let description_score = entry
        .description
        .as_deref()
        .map(|description| fuzzy_score(query, description))
        .unwrap_or(0);
    (name_score * 2) + category_score + description_score
}

pub fn selectable_entities(projection: &CanvasProjection) -> Vec<CanvasEntityId> {
    projection
        .iter()
        .map(|item| item.id)
        .filter(|entity| is_selectable_entity(*entity))
        .collect()
}

pub fn is_selectable_entity(entity: CanvasEntityId) -> bool {
    matches!(
        entity,
        CanvasEntityId::Node(_) | CanvasEntityId::Wire(_) | CanvasEntityId::Frame(_)
    )
}

pub fn entity_to_graph_entity(entity: CanvasEntityId) -> Option<GraphEntityId> {
    match entity {
        CanvasEntityId::Node(id) => Some(GraphEntityId::Node(id)),
        CanvasEntityId::Wire(id) => Some(GraphEntityId::Connection(id)),
        CanvasEntityId::Frame(id) => Some(GraphEntityId::Frame(id)),
        CanvasEntityId::Port { .. } | CanvasEntityId::Annotation(_) | CanvasEntityId::Custom(_) => {
            None
        }
    }
}

pub fn selection_delete_transaction(selection: &SelectionSet) -> Option<GraphTransaction> {
    let entities = selection.graph_entities();
    if entities.is_empty() {
        return None;
    }

    let mut node_ids = Vec::new();
    let mut transactions = Vec::new();
    for entity in entities {
        match entity {
            GraphEntityId::Node(id) => node_ids.push(id),
            GraphEntityId::Connection(id) => {
                transactions.push(GraphTransaction::RemoveConnection { connection_id: id });
            }
            GraphEntityId::Frame(id) => {
                transactions.push(GraphTransaction::RemoveFrame { frame_id: id });
            }
        }
    }

    if !node_ids.is_empty() {
        transactions.push(GraphTransaction::RemoveNodes { node_ids });
    }

    match transactions.len() {
        0 => None,
        1 => transactions.pop(),
        _ => Some(GraphTransaction::Compound(transactions)),
    }
}

pub fn selection_duplicate_transaction(
    selection: &SelectionSet,
    offset: (f64, f64),
) -> Option<GraphTransaction> {
    let entities = selection.graph_entities();
    (!entities.is_empty()).then_some(GraphTransaction::DuplicateSelection { entities, offset })
}

pub fn create_frame_around_selection_transaction(
    selection: &SelectionSet,
    nodes: &[NodeProjection],
    padding: f64,
    title: impl Into<String>,
    color: (u8, u8, u8),
) -> Option<GraphTransaction> {
    if !padding.is_finite() || padding < 0.0 {
        return None;
    }

    let selected_node_ids: HashSet<u64> = selection
        .entities
        .iter()
        .filter_map(|entity| match *entity {
            CanvasEntityId::Node(id) => Some(id),
            _ => None,
        })
        .collect();
    if selected_node_ids.is_empty() {
        return None;
    }

    let mut bounds = None;
    for node in nodes
        .iter()
        .filter(|node| selected_node_ids.contains(&node.id))
    {
        let node_bounds = node.bounds();
        bounds = Some(match bounds {
            Some(existing) => union_world_rect(existing, node_bounds),
            None => node_bounds,
        });
    }

    let bounds = bounds?;
    let size = (bounds.width + padding * 2.0, bounds.height + padding * 2.0);
    if !valid_frame_size(size) {
        return None;
    }

    Some(GraphTransaction::AddFrame {
        title: title.into(),
        position: (bounds.x - padding, bounds.y - padding),
        size,
        color,
    })
}

pub fn clipboard_paste_transaction(
    document: &NodeGraphDocument,
    clipboard: &NodeClipboard,
    position: WorldPoint,
) -> Option<GraphTransaction> {
    let anchor = clipboard_anchor(document, &clipboard.entities)?;
    Some(GraphTransaction::DuplicateSelection {
        entities: clipboard.entities.clone(),
        offset: (position.x - anchor.0, position.y - anchor.1),
    })
}

pub fn selection_nudge_transaction(
    document: &NodeGraphDocument,
    selection: &SelectionSet,
    delta: (f64, f64),
) -> Option<GraphTransaction> {
    let mut transactions = Vec::new();
    let mut node_moves = Vec::new();

    for entity in selection.graph_entities() {
        match entity {
            GraphEntityId::Node(id) => {
                if let Some(node) = document.node(id) {
                    node_moves.push((id, (node.position.0 + delta.0, node.position.1 + delta.1)));
                }
            }
            GraphEntityId::Frame(id) => {
                if let Some(frame) = document.frame(id) {
                    transactions.push(GraphTransaction::MoveFrame {
                        frame_id: id,
                        new_pos: (frame.position.0 + delta.0, frame.position.1 + delta.1),
                    });
                }
            }
            GraphEntityId::Connection(_) => {}
        }
    }

    if !node_moves.is_empty() {
        transactions.push(GraphTransaction::MoveNodes { moves: node_moves });
    }

    match transactions.len() {
        0 => None,
        1 => transactions.pop(),
        _ => Some(GraphTransaction::Compound(transactions)),
    }
}

pub fn move_drag_transaction(
    document: &NodeGraphDocument,
    drag: &DragSession,
) -> Option<GraphTransaction> {
    let mut transactions = Vec::new();
    let mut captured_node_ids = HashSet::new();

    for capture in &drag.frame_captures {
        let Some(frame) = document.frame(capture.frame_id) else {
            continue;
        };
        let node_moves: Vec<(u64, (f64, f64))> = capture
            .node_ids
            .iter()
            .filter_map(|node_id| {
                captured_node_ids.insert(*node_id);
                document.node(*node_id).map(|node| {
                    (
                        *node_id,
                        (
                            node.position.0 + drag.delta.0,
                            node.position.1 + drag.delta.1,
                        ),
                    )
                })
            })
            .collect();
        let new_pos = (
            frame.position.0 + drag.delta.0,
            frame.position.1 + drag.delta.1,
        );
        if node_moves.is_empty() {
            transactions.push(GraphTransaction::MoveFrame {
                frame_id: capture.frame_id,
                new_pos,
            });
        } else {
            transactions.push(GraphTransaction::MoveFrameWithNodes {
                frame_id: capture.frame_id,
                new_pos,
                node_moves,
            });
        }
    }

    let node_moves: Vec<(u64, (f64, f64))> = drag
        .entities
        .iter()
        .filter_map(|entity| match *entity {
            CanvasEntityId::Node(id) if !captured_node_ids.contains(&id) => {
                document.node(id).map(|node| {
                    (
                        id,
                        (
                            node.position.0 + drag.delta.0,
                            node.position.1 + drag.delta.1,
                        ),
                    )
                })
            }
            _ => None,
        })
        .collect();
    if !node_moves.is_empty() {
        transactions.push(GraphTransaction::MoveNodes { moves: node_moves });
    }

    match transactions.len() {
        0 => None,
        1 => transactions.pop(),
        _ => Some(GraphTransaction::Compound(transactions)),
    }
}

pub fn wire_drag_transaction(
    document: &NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    wire_drag: &WireDragSession,
    target: Option<PortEndpoint>,
) -> Result<Option<GraphTransaction>, NodeEditorSessionError> {
    match (&wire_drag.origin, target) {
        (WireDragOrigin::Output(source), Some(target)) => {
            connection_transaction(document, catalog, registry, *source, target)
        }
        (WireDragOrigin::Input(input), Some(target)) => {
            connection_transaction(document, catalog, registry, target, *input)
        }
        (
            WireDragOrigin::Reroute {
                connection_id,
                output,
                original_input,
            },
            Some(target),
        ) => {
            let (from, to) = match target.direction {
                PortDirection::Output => (target, *original_input),
                PortDirection::Input => (*output, target),
            };
            validate_connection_endpoints(document, catalog, registry, from, to)?;
            if from.node_id == output.node_id
                && from.port_id == output.port_id
                && to.node_id == original_input.node_id
                && to.port_id == original_input.port_id
            {
                Ok(None)
            } else {
                Ok(Some(GraphTransaction::ReplaceConnection {
                    old_connection_id: *connection_id,
                    from_node: from.node_id,
                    from_port: from.port_id,
                    to_node: to.node_id,
                    to_port: to.port_id,
                }))
            }
        }
        (WireDragOrigin::Reroute { connection_id, .. }, None) => {
            Ok(Some(GraphTransaction::RemoveConnection {
                connection_id: *connection_id,
            }))
        }
        (WireDragOrigin::Output(_) | WireDragOrigin::Input(_), None) => Ok(None),
    }
}

pub fn frame_resize_transaction(
    document: &NodeGraphDocument,
    resize: &FrameResizeSession,
) -> Option<GraphTransaction> {
    let frame = document.frame(resize.frame_id)?;
    if frame.size == resize.current_size || !valid_frame_size(resize.current_size) {
        return None;
    }
    Some(GraphTransaction::ResizeFrame {
        frame_id: resize.frame_id,
        new_size: resize.current_size,
    })
}

pub fn inline_edit_transaction(
    document: &NodeGraphDocument,
    inline_edit: &InlineEditSession,
) -> Option<GraphTransaction> {
    match inline_edit.entity {
        CanvasEntityId::Node(node_id) => {
            let node = document.node(node_id)?;
            (node.title != inline_edit.draft).then_some(GraphTransaction::SetNodeTitle {
                node_id,
                title: inline_edit.draft.clone(),
            })
        }
        CanvasEntityId::Frame(frame_id) => {
            let frame = document.frame(frame_id)?;
            (frame.title != inline_edit.draft).then_some(GraphTransaction::SetFrameTitle {
                frame_id,
                title: inline_edit.draft.clone(),
            })
        }
        CanvasEntityId::Wire(_)
        | CanvasEntityId::Port { .. }
        | CanvasEntityId::Annotation(_)
        | CanvasEntityId::Custom(_) => None,
    }
}

fn push_nudge_intent(
    intents: &mut Vec<NodeEditorIntent>,
    document: &NodeGraphDocument,
    session: &NodeEditorSession,
    delta: (f64, f64),
) {
    if let Some(transaction) = selection_nudge_transaction(document, &session.selection, delta) {
        intents.push(NodeEditorIntent::Graph(transaction));
    }
}

fn clipboard_anchor(
    document: &NodeGraphDocument,
    entities: &[GraphEntityId],
) -> Option<(f64, f64)> {
    let mut anchor: Option<(f64, f64)> = None;
    for entity in entities {
        let pos = match *entity {
            GraphEntityId::Node(id) => document.node(id).map(|node| node.position),
            GraphEntityId::Frame(id) => document.frame(id).map(|frame| frame.position),
            GraphEntityId::Connection(_) => None,
        };
        if let Some(pos) = pos {
            anchor = Some(match anchor {
                Some((x, y)) => (x.min(pos.0), y.min(pos.1)),
                None => pos,
            });
        }
    }
    anchor
}

fn cancel_action_intents(session: &NodeEditorSession) -> Vec<NodeEditorIntent> {
    if session.search.is_some() {
        return vec![NodeEditorIntent::Session(SessionIntent::Search(
            NodeSearchIntent::Close,
        ))];
    }
    if session.inline_edit.is_some() {
        return vec![NodeEditorIntent::Session(SessionIntent::CancelInlineEdit)];
    }
    if session.wire_drag.is_some() {
        return vec![NodeEditorIntent::Session(SessionIntent::CancelWireDrag)];
    }
    if session.rubber_band.is_some() {
        return vec![NodeEditorIntent::Session(SessionIntent::CancelRubberBand)];
    }
    if session.drag.is_some() {
        return vec![NodeEditorIntent::Session(SessionIntent::Selection(
            SelectionIntent::CancelMove,
        ))];
    }
    if session.frame_resize.is_some() {
        return vec![NodeEditorIntent::Session(SessionIntent::CancelFrameResize)];
    }
    if !session.selection.entities.is_empty() {
        return vec![NodeEditorIntent::Session(SessionIntent::Selection(
            SelectionIntent::Clear,
        ))];
    }
    Vec::new()
}

fn valid_frame_size(size: (f64, f64)) -> bool {
    size.0 > 0.0 && size.1 > 0.0
}

fn union_world_rect(left: WorldRect, right: WorldRect) -> WorldRect {
    let min_x = left.x.min(right.x);
    let min_y = left.y.min(right.y);
    let max_x = (left.x + left.width).max(right.x + right.width);
    let max_y = (left.y + left.height).max(right.y + right.height);
    WorldRect::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

fn read_only_allows_intent(intent: &NodeEditorIntent) -> bool {
    match intent {
        NodeEditorIntent::Session(session_intent) => match session_intent {
            SessionIntent::Selection(
                SelectionIntent::Replace(_)
                | SelectionIntent::Toggle(_)
                | SelectionIntent::SelectAll
                | SelectionIntent::Clear
                | SelectionIntent::RubberBandCommit { .. }
                | SelectionIntent::CancelMove,
            )
            | SessionIntent::BeginRubberBand { .. }
            | SessionIntent::UpdateRubberBand { .. }
            | SessionIntent::CancelRubberBand
            | SessionIntent::SetViewport(_) => true,
            SessionIntent::Selection(
                SelectionIntent::BeginMove { .. }
                | SelectionIntent::UpdateMove { .. }
                | SelectionIntent::CommitMove
                | SelectionIntent::DeleteSelected
                | SelectionIntent::DuplicateSelected
                | SelectionIntent::CopySelected
                | SelectionIntent::PasteClipboard { .. }
                | SelectionIntent::CutSelected,
            ) => false,
            SessionIntent::SetTool(NodeEditorTool::Select | NodeEditorTool::Pan) => true,
            SessionIntent::SetTool(NodeEditorTool::Connect) => false,
            SessionIntent::Search(NodeSearchIntent::Close) => true,
            SessionIntent::Search(_) => false,
            SessionIntent::CancelWireDrag
            | SessionIntent::CancelFrameResize
            | SessionIntent::CancelInlineEdit => true,
            SessionIntent::BeginWireDrag { .. } | SessionIntent::UpdateWireDrag { .. } => false,
        },
        NodeEditorIntent::ContextMenu(_) => true,
        NodeEditorIntent::Graph(_) | NodeEditorIntent::Undo | NodeEditorIntent::Redo => false,
    }
}

fn projection_from_item(
    revision: ProjectionRevision,
    item: ProjectedCanvasItem,
) -> CanvasProjection {
    CanvasProjection::new(revision, vec![item])
}

fn connection_transaction(
    document: &NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    from: PortEndpoint,
    to: PortEndpoint,
) -> Result<Option<GraphTransaction>, NodeEditorSessionError> {
    validate_connection_endpoints(document, catalog, registry, from, to)?;
    if document.connections().any(|connection| {
        connection.from_node == from.node_id
            && connection.from_port == from.port_id
            && connection.to_node == to.node_id
            && connection.to_port == to.port_id
    }) {
        return Ok(None);
    }

    let from_definition = port_definition(document, catalog, from)?;
    let to_definition = port_definition(document, catalog, to)?;
    let mut conflicting_connection_ids = Vec::new();

    if from_definition.max_connections == PortCardinality::Single {
        conflicting_connection_ids.extend(
            document
                .connections_for_port(from.node_id, from.port_id)
                .into_iter()
                .filter(|connection| {
                    connection.from_node == from.node_id && connection.from_port == from.port_id
                })
                .map(|connection| connection.id),
        );
    }
    if to_definition.max_connections == PortCardinality::Single {
        conflicting_connection_ids.extend(
            document
                .connections_for_port(to.node_id, to.port_id)
                .into_iter()
                .filter(|connection| {
                    connection.to_node == to.node_id && connection.to_port == to.port_id
                })
                .map(|connection| connection.id),
        );
    }
    conflicting_connection_ids.sort_unstable();
    conflicting_connection_ids.dedup();

    let add = GraphTransaction::AddConnection {
        from_node: from.node_id,
        from_port: from.port_id,
        to_node: to.node_id,
        to_port: to.port_id,
    };
    let transaction = match conflicting_connection_ids.as_slice() {
        [] => add,
        [old_connection_id] => GraphTransaction::ReplaceConnection {
            old_connection_id: *old_connection_id,
            from_node: from.node_id,
            from_port: from.port_id,
            to_node: to.node_id,
            to_port: to.port_id,
        },
        _ => {
            let mut transactions = conflicting_connection_ids
                .into_iter()
                .map(|connection_id| GraphTransaction::RemoveConnection { connection_id })
                .collect::<Vec<_>>();
            transactions.push(add);
            GraphTransaction::Compound(transactions)
        }
    };
    Ok(Some(transaction))
}

fn validate_connection_endpoints(
    document: &NodeGraphDocument,
    catalog: &NodeTypeCatalog,
    registry: &PortTypeRegistry,
    from: PortEndpoint,
    to: PortEndpoint,
) -> Result<(), NodeEditorSessionError> {
    if from.direction != PortDirection::Output {
        return Err(NodeEditorSessionError::InvalidDirection { endpoint: from });
    }
    if to.direction != PortDirection::Input {
        return Err(NodeEditorSessionError::InvalidDirection { endpoint: to });
    }

    let from_definition = port_definition(document, catalog, from)?;
    let to_definition = port_definition(document, catalog, to)?;
    if !registry.are_compatible(from_definition.type_id, to_definition.type_id) {
        return Err(NodeEditorSessionError::IncompatiblePorts { from, to });
    }
    Ok(())
}

fn port_definition<'a>(
    document: &NodeGraphDocument,
    catalog: &'a NodeTypeCatalog,
    endpoint: PortEndpoint,
) -> Result<&'a PortDefinition, NodeEditorSessionError> {
    let node = document
        .node(endpoint.node_id)
        .ok_or(NodeEditorSessionError::UnknownNode {
            node_id: endpoint.node_id,
        })?;
    let node_type = catalog
        .get(node.type_id)
        .ok_or(NodeEditorSessionError::UnknownNodeType {
            node_id: node.id,
            type_id: node.type_id,
        })?;
    let ports = match endpoint.direction {
        PortDirection::Input => &node_type.input_ports,
        PortDirection::Output => &node_type.output_ports,
    };
    ports
        .iter()
        .find(|port| port.port_id == endpoint.port_id)
        .ok_or(NodeEditorSessionError::UnknownPort { endpoint })
}

fn fuzzy_score(query: &str, text: &str) -> i32 {
    if query.is_empty() {
        return 1;
    }

    let query_lower = query.to_lowercase();
    let text_lower = text.to_lowercase();

    if text_lower.starts_with(&query_lower) {
        return 300;
    }

    if text_lower.contains(&query_lower) {
        return 150;
    }

    let query_chars: Vec<char> = query_lower.chars().collect();
    let text_chars: Vec<char> = text_lower.chars().collect();

    let mut score = 0;
    let mut text_idx = 0;
    let mut last_match_idx = 0;

    for (query_idx, q_char) in query_chars.iter().enumerate() {
        let mut found = false;
        while text_idx < text_chars.len() {
            if text_chars[text_idx] == *q_char {
                found = true;
                if query_idx > 0 {
                    let gap = text_idx - last_match_idx - 1;
                    if gap > 6 {
                        return 0;
                    }
                    score += 1;
                }
                score += 10;
                last_match_idx = text_idx;
                text_idx += 1;
                break;
            }
            text_idx += 1;
        }

        if !found {
            return 0;
        }
    }

    score
}
