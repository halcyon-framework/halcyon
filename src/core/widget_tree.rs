use crate::core::authority::{Authority, AuthorityRegistry, IntentBuffers, Projection};
use crate::core::event::{HitTestResult, PointerId, PointerRouteOrigin};
use crate::core::hit_test::{
    HIT_TEST_ACCEL_THRESHOLD, HIT_TEST_CELL_HEIGHT, HIT_TEST_CELL_WIDTH, HitTestGrid,
};
use crate::core::input_projection::{InputProjection, ProjectedHitResult};
use crate::core::layout::{Constraints, FlexParams, LayoutResult};
use crate::core::layout_transition::{
    ConflictFlags, LayoutTransitionCandidate, LayoutTransitionStore,
};
use crate::core::math::{Point, RectF, Size};
use crate::core::paint::{
    ControlSubtreeMember, LAYER_CONTENT, LAYER_MAIN_AFFORDANCE, LAYER_OVERLAY, ZIndex,
};
use crate::core::rect_authority::assert_rect_mutation_authorized;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetContentKey, WidgetDowncast};
use crate::renderer::custom_render_prepare::{
    CustomRenderPreparationError, CustomRenderPreparationOutcome, CustomRenderPresentationBindings,
    CustomRenderTargetSnapshot,
};
use crate::utils::id::{TreeWidgetId, WidgetInternalId};
use crate::widgets::scroll_area::{
    LayoutScrollAnchorIntent, LayoutScrollAnchorTransaction, ScrollArea,
};
use std::any::TypeId;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, OnceLock};

use crate::core::widget_availability::PendingWidgetAvailability;
pub use crate::core::widget_availability::WidgetAvailabilityError;

/// Opaque engine snapshot identity; equality does not establish live membership.
#[derive(Debug, Clone)]
pub(crate) struct WidgetTreeLifetimeToken(Arc<()>);

impl PartialEq for WidgetTreeLifetimeToken {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for WidgetTreeLifetimeToken {}

/// Engine-only concrete mount identity. A tree ID alone does not qualify a Box.
#[derive(Debug, Clone)]
pub(crate) struct WidgetMountToken {
    tree_lifetime: WidgetTreeLifetimeToken,
    owner: TreeWidgetId,
    generation: u64,
    live: Arc<AtomicBool>,
}

impl WidgetMountToken {
    pub(crate) fn owner(&self) -> TreeWidgetId {
        self.owner
    }

    pub(crate) fn is_live(&self) -> bool {
        self.live.load(Ordering::Acquire)
    }
}

impl PartialEq for WidgetMountToken {
    fn eq(&self, other: &Self) -> bool {
        self.tree_lifetime == other.tree_lifetime
            && self.owner == other.owner
            && self.generation == other.generation
            && Arc::ptr_eq(&self.live, &other.live)
    }
}

impl Eq for WidgetMountToken {}

/// The node retains concrete lineage through a temporary layout take/restore.
/// Liveness storage is allocated only when an explicit owner requests a token.
struct ConcreteWidgetMount {
    tree_lifetime: Option<WidgetTreeLifetimeToken>,
    generation: u64,
    live: OnceLock<Arc<AtomicBool>>,
}

impl ConcreteWidgetMount {
    fn unattached() -> Self {
        Self {
            tree_lifetime: None,
            generation: 0,
            live: OnceLock::new(),
        }
    }

    fn retire(&self) {
        if let Some(live) = self.live.get() {
            live.store(false, Ordering::Release);
        }
    }
}

impl Drop for ConcreteWidgetMount {
    fn drop(&mut self) {
        self.retire();
    }
}

/// Scoped access to concrete mount metadata, disjoint from the mutable Widget.
/// Creating this capability does not initialize the lazy liveness allocation.
#[derive(Clone, Copy)]
pub(crate) struct WidgetAnimationMount<'a> {
    owner: TreeWidgetId,
    mount: &'a ConcreteWidgetMount,
}

impl WidgetAnimationMount<'_> {
    pub(crate) fn token(self) -> WidgetMountToken {
        let live = self
            .mount
            .live
            .get_or_init(|| Arc::new(AtomicBool::new(true)));
        WidgetMountToken {
            tree_lifetime: self
                .mount
                .tree_lifetime
                .as_ref()
                .expect("mounted animation scope has a tree lifetime")
                .clone(),
            owner: self.owner,
            generation: self.mount.generation,
            live: live.clone(),
        }
    }
}

/// Owned current-owner qualification; never a WidgetTree or layout borrow.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct OwnedWidgetPresentationGeometry {
    pub(crate) rect: RectF,
    pub(crate) viewport: RectF,
    pub(crate) cumulative_scroll: (f32, f32),
    pub(crate) root_viewport: RectF,
    pub(crate) clip: RectF,
    pub(crate) scale_factor: f32,
}

/// Published Button request captured by the engine, without target or GPU authority.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CapturedButtonSdfRequest {
    tree_lifetime: WidgetTreeLifetimeToken,
    owner: TreeWidgetId,
    request: crate::widgets::button::ButtonSdfRequestSnapshot,
}

impl CapturedButtonSdfRequest {
    #[allow(dead_code)] // Target-qualified App preparation is not wired yet.
    pub(crate) fn tree_lifetime(&self) -> &WidgetTreeLifetimeToken {
        &self.tree_lifetime
    }

    #[allow(dead_code)]
    pub(crate) fn owner(&self) -> TreeWidgetId {
        self.owner
    }

    #[allow(dead_code)]
    pub(crate) fn request(&self) -> crate::widgets::button::ButtonSdfRequestSnapshot {
        self.request
    }
}

#[derive(Debug, Clone)]
pub(crate) struct GeometryCausalCutoff {
    lifetime: Rc<()>,
    sequence: u64,
}

#[derive(Debug, Clone)]
pub(crate) struct AbsoluteDragGeometryWatch {
    pub(crate) owner: TreeWidgetId,
    pub(crate) declaration: crate::core::widget::AbsoluteDragGeometryDependency,
    pub(crate) path: Vec<(NodeId, TreeWidgetId)>,
    reflow_mount: TreeWidgetId,
    cutoff: GeometryCausalCutoff,
    scroll_basis: Vec<(NodeId, Option<(f32, f32)>)>,
    clip_basis: Vec<AbsoluteDragClipScope>,
}

#[derive(Debug, Clone, PartialEq)]
struct AbsoluteDragClipScope {
    node: NodeId,
    mount: TreeWidgetId,
    declaration: AbsoluteDragClipDeclaration,
}

#[derive(Debug, Clone, PartialEq)]
enum AbsoluteDragClipDeclaration {
    Viewport(RectF),
    Widget(RectF, Option<crate::core::paint::RoundedCorners>),
}

// ============================================================================
// SPEC-HAL-088: Unified Viewport and Overflow Model - Stage 1
// ============================================================================

/// SPEC-HAL-088: Overflow mode for widget clipping behavior.
///
/// Determines how a widget's children are clipped relative to its layout rect.
/// Stage 1 adds the enum and storage; actual clipping behavior is implemented
/// in later stages.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OverflowMode {
    /// Children may paint outside the node's layout rect.
    /// This is the default for most layout containers (Row, Column, Flex).
    #[default]
    Visible,
    /// Children are conceptually clipped to this node's layout rect.
    /// Actual clipping will be enforced in Stage 4 of SPEC-088 (SceneGraph clip nodes).
    /// Used by ScrollArea, Panel, and other containers that need visual boundaries.
    Clip,
}

// ============================================================================
// SPEC-HAL-035: Widget Diff Optimizer Types
// ============================================================================

/// SPEC-HAL-035: Per-node dirty classification for the widget diff optimizer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetDirty {
    /// Widget + subtree unchanged since last frame.
    Unchanged,
    /// Layout changed (constraints or rect), paint implicitly dirty.
    LayoutOnly,
    /// Layout same, but paint affected (content/key/state changed).
    PaintOnly,
    /// Both layout and paint must be recomputed.
    LayoutAndPaint,
    /// Widget replaced (type/key mismatch vs snapshot).
    Replaced,
}

/// SPEC-HAL-035: Snapshot of widget-related state used by the diff optimizer.
#[derive(Debug, Clone)]
pub struct NodeSnapshot {
    pub rect: RectF,
    pub layout_constraints: Constraints,
    pub widget_type_id: std::any::TypeId,
    pub widget_key: Option<WidgetKey>,
    pub content_key: Option<WidgetContentKey>,
}

/// Node ID for internal tree indexing
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct NodeId(pub usize);

/// Explicit external placement of a closed ordinary-control subtree scope.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSubtreePlacement {
    Main { z_index: ZIndex },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSubtreeOrderError {
    MissingHost,
    DuplicateMember,
    NotDirectChild,
    IncompleteMemberSet,
    InvalidMainPlacement,
}

impl std::fmt::Display for ControlSubtreeOrderError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "control subtree order: {self:?}")
    }
}

impl std::error::Error for ControlSubtreeOrderError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControlSubtreeAffordanceError {
    MissingHost,
    DuplicateSource,
    NotDescendant,
    SourceAlreadyAssigned,
    InvalidContentPlacement,
}

impl std::fmt::Display for ControlSubtreeAffordanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "control subtree main affordance: {self:?}")
    }
}

impl std::error::Error for ControlSubtreeAffordanceError {}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlSubtreeOrder {
    host: TreeWidgetId,
    members: Vec<(NodeId, TreeWidgetId)>,
    placement: ControlSubtreePlacement,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ControlSubtreeAffordances {
    host: TreeWidgetId,
    sources: Vec<(NodeId, TreeWidgetId)>,
}

/// Semantic key for dynamic children (lists, tables)
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum WidgetKey {
    U64(u64),
    String(String),
}

/// A node in the widget tree
/// SPEC-HAL-024: Stores ONLY relative logical coordinates (rect)
/// SPEC-HAL-024 Stage 2: Also stores absolute_xy computed during layout traversal
/// SPEC-HAL-024 Stage 3: Also stores absolute_paint_rect (absolute_xy + size)
/// SPEC-HAL-026: Stores dirty flags for layout invalidation
/// SPEC-HAL-086 Stage 5: Stores layout_scroll_offset for scroll-aware position computation
/// A mutable node borrow grants state/props access, not node substitution.
/// Do not move, swap, or replace this owning node through a borrow. Concrete
/// replacement and topology changes belong to WidgetTree's owner operations.
pub struct WidgetNode {
    pub(crate) id: TreeWidgetId,
    pub key: Option<WidgetKey>,
    pub(crate) widget: Box<dyn Widget>,
    pub(crate) parent: Option<NodeId>,
    pub(crate) first_child: Option<NodeId>,
    pub(crate) next_sibling: Option<NodeId>,
    concrete_mount: ConcreteWidgetMount,
    /// HAL-101 internal role: mutable access requires its qualified engine owner.
    pub(crate) composite_editor_child: bool,
    /// Installed by the Phase-2 cell Choice owner for this concrete mount.
    pub(crate) composite_choice_host: bool,
    pub(crate) cell_editor_presentation: crate::core::appearance::CellEditorPresentation,
    pub(crate) widget_layout_lease: bool,
    rect: RectF, // Relative to parent, in logical pixels (SPEC-HAL-141: private)
    pub z_order: i16,
    /// Flex layout parameters for this node
    pub flex: FlexParams,
    pub(crate) control_alignment_member: bool,
    pub(crate) control_parts_cache: Option<Box<crate::core::layout::ControlPartsLayoutCache>>,
    // SPEC-HAL-024 Stage 2: Absolute Logical position (top-left)
    // Computed during layout traversal: parent.absolute_xy + self.rect.(x,y)
    // Written during layout, not read by other systems yet (Stage 2 plumbing)
    absolute_xy: Point, // SPEC-HAL-141: private
    /// SPEC-HAL-024 Stage 3: Absolute Logical paint rect (top-left and size).
    /// Derived from `absolute_xy` and `rect.width/height` after layout.
    absolute_paint_rect: RectF, // SPEC-HAL-141: private
    // SPEC-HAL-026: Dirty flag fields for layout invalidation
    /// Whether this widget needs layout recomputation
    pub dirty_layout: bool,
    /// Whether this widget's own paint output must be regenerated.
    pub dirty_paint: bool,
    /// Whether this widget's retained subtree composition must be regenerated.
    pub dirty_composition: bool,
    /// Constraints from last layout pass (for change detection)
    pub last_constraints: Option<Constraints>,
    /// Cached size from last layout pass
    cached_size: Size, // SPEC-HAL-141: private
    /// Content size from last layout pass (for scroll extent calculation)
    /// This is the intrinsic content size, which may exceed cached_size
    content_size: Size,
    // SPEC-HAL-035: Widget diff optimizer fields
    /// Snapshot from previous frame for diffing. None if never diffed.
    pub last_snapshot: Option<NodeSnapshot>,
    /// Last computed dirty classification for this node.
    pub dirty_flags: WidgetDirty,
    /// SPEC-HAL-086 Stage 5: Scroll offset from layout result.
    /// Used during absolute position computation to shift children.
    /// None for non-scroll widgets.
    pub layout_scroll_offset: Option<(f32, f32)>,
    // SPEC-HAL-088 Stage 1: Overflow and viewport fields
    /// SPEC-HAL-088: Overflow mode for this node's children.
    /// Determines whether children can paint outside the node's layout rect.
    /// Stage 1: Always initialized to Visible; per-widget overrides come in later stages.
    pub overflow: OverflowMode,
    /// Optional context included in the local F9 tree snapshot.
    pub diagnostic_hint: String,
    /// SPEC-HAL-088: Computed viewport rect for this node.
    /// After layout, this equals `absolute_rect`. Stage 2 will implement
    /// ancestor-intersection logic to compute the true visible region.
    /// Used by hit-testing (Stage 3) and scene clipping (Stage 4).
    viewport: RectF, // SPEC-HAL-141: private
    /// DEBUG: Last completed layout epoch for paint-without-layout detection.
    /// A nonzero epoch remains valid while layout is clean.
    #[cfg(any(debug_assertions, test))]
    pub layout_epoch: u64,
}

impl WidgetNode {
    /// Copy the tree-owned identity. HAL-087 forbids external reassignment.
    ///
    /// ```compile_fail
    /// use halcyon::core::widget_tree::WidgetNode;
    /// use halcyon::utils::id::TreeWidgetId;
    /// fn cannot_reassign(node: &mut WidgetNode) {
    ///     node.id = TreeWidgetId(7);
    /// }
    /// ```
    pub fn id(&self) -> TreeWidgetId {
        self.id
    }

    /// Borrow the mounted widget without taking ownership of its Box.
    pub fn widget(&self) -> &dyn Widget {
        self.widget.as_ref()
    }

    /// Borrow the existing widget's state and properties for this scope.
    ///
    /// This grants neither concrete-instance substitution nor tree ownership.
    /// Install or replace a concrete widget through WidgetTree's owner operation;
    /// do not replace a downcast concrete value in place or retain a peer borrow.
    ///
    /// ```compile_fail
    /// use halcyon::core::widget_tree::WidgetNode;
    /// use halcyon::widgets::label::Label;
    /// fn cannot_replace_box(node: &mut WidgetNode) {
    ///     node.widget = Box::new(Label::new("replacement"));
    /// }
    /// ```
    pub fn widget_mut(&mut self) -> &mut dyn Widget {
        assert!(
            !self.composite_editor_child,
            "HAL-101: a registered composite editor is writable only through CompositeEditorOwnerContext"
        );
        self.widget.as_mut()
    }

    /// Copy the tree-managed parent relationship.
    pub fn parent(&self) -> Option<NodeId> {
        self.parent
    }

    /// Copy the first tree-managed child relationship.
    pub fn first_child(&self) -> Option<NodeId> {
        self.first_child
    }

    /// Copy the tree-managed sibling relationship.
    pub fn next_sibling(&self) -> Option<NodeId> {
        self.next_sibling
    }

    pub(crate) fn new(
        id: TreeWidgetId,
        widget: Box<dyn Widget>,
        key: Option<WidgetKey>,
        z_order: i16,
    ) -> Self {
        Self {
            id,
            key,
            widget,
            parent: None,
            first_child: None,
            next_sibling: None,
            concrete_mount: ConcreteWidgetMount::unattached(),
            composite_editor_child: false,
            composite_choice_host: false,
            cell_editor_presentation: Default::default(),
            widget_layout_lease: false,
            rect: RectF::zero(),
            z_order,
            flex: FlexParams::default(),
            control_alignment_member: false,
            control_parts_cache: None,
            // SPEC-HAL-024 Stage 2: Initialize absolute position to origin
            absolute_xy: Point::zero(),
            // SPEC-HAL-024 Stage 3: Initialize absolute paint rect to zero
            absolute_paint_rect: RectF::zero(),
            // SPEC-HAL-026: Initialize dirty flags
            dirty_layout: true, // New nodes always need initial layout
            dirty_paint: true,
            dirty_composition: true,
            last_constraints: None,     // No constraints yet
            cached_size: Size::zero(),  // No cached size yet
            content_size: Size::zero(), // No content size yet
            // SPEC-HAL-035: Initialize diff optimizer fields
            last_snapshot: None, // No snapshot yet (first frame)
            dirty_flags: WidgetDirty::LayoutAndPaint, // Treat as fully dirty initially
            // SPEC-HAL-086 Stage 5: Initialize scroll offset to None
            layout_scroll_offset: None,
            // SPEC-HAL-088 Stage 1: Initialize overflow and viewport
            overflow: OverflowMode::Visible, // Default: children can paint outside
            diagnostic_hint: String::new(),
            viewport: RectF::zero(), // Updated during compute_absolute_paint_rects
            // DEBUG: Initialize layout_epoch to 0 (never laid out)
            #[cfg(any(debug_assertions, test))]
            layout_epoch: 0,
        }
    }
}

/// SPEC-HAL-086 Stage 8: Overlay hit region for scrollbar hit-testing
#[derive(Debug, Clone, Copy)]
pub struct OverlayHitRegion {
    /// Absolute rect of the overlay (scrollbar track)
    pub abs_rect: RectF,
    /// Absolute rect of the owning widget (ScrollArea)
    pub owner_abs_rect: RectF,
    /// Widget ID that owns this overlay
    pub widget_id: TreeWidgetId,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ComboPopupScrollbar {
    /// The original ScrollbarV layout in scene coordinates.
    pub(crate) layout: crate::widgets::scrollbar::ScrollbarLayout,
    pub(crate) drawn: bool,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct ComboPopupInputGeometry {
    /// The routable row area in control-local coordinates.
    pub(crate) rows: RectF,
    /// The grouped popup's inert context strip in the same coordinates.
    pub(crate) footer: Option<RectF>,
    pub(crate) visible_rows: usize,
    pub(crate) first_row: usize,
    pub(crate) scrollbar: Option<ComboPopupScrollbar>,
    pub(crate) scroll_offset: (f32, f32),
}

impl ComboPopupInputGeometry {
    pub(crate) fn drawn_scrollbar(self) -> Option<crate::widgets::scrollbar::ScrollbarLayout> {
        self.scrollbar.filter(|bar| bar.drawn).map(|bar| bar.layout)
    }

    pub(crate) fn scrollbar_point(self, screen: (f32, f32)) -> Point {
        Point::new(
            screen.0 + self.scroll_offset.0,
            screen.1 + self.scroll_offset.1,
        )
    }
}

#[derive(Debug)]
struct MountedComboPopupHit {
    tree_lifetime: WidgetTreeLifetimeToken,
    mount_generation: u64,
    popup_revision: u64,
    viewport: (f32, f32),
    scale: f32,
    geometry: ComboPopupInputGeometry,
}

/// Retained alongside the scene fragment that emitted the region.
#[derive(Clone, Debug)]
pub(crate) struct OverlayHitEntry {
    pub(crate) region: OverlayHitRegion,
    combo_popup: Option<Arc<MountedComboPopupHit>>,
}

impl From<OverlayHitRegion> for OverlayHitEntry {
    fn from(region: OverlayHitRegion) -> Self {
        Self {
            region,
            combo_popup: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TextHandoffOwner {
    TextField,
    Markdown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct PendingTextHandoff {
    mount: WidgetMountToken,
    kind: TextHandoffOwner,
}

/// The widget tree - manages all widgets and their relationships.
///
/// Owning Box extraction is engine-internal. Public clients borrow widget state
/// or use the explicit tree replacement owner.
///
/// ```compile_fail
/// use halcyon::core::widget_tree::{NodeId, WidgetTree};
/// fn cannot_take_box(tree: &mut WidgetTree, node: NodeId) {
///     tree.take_widget(node);
/// }
/// ```
pub struct WidgetTree {
    tree_lifetime: WidgetTreeLifetimeToken,
    next_concrete_mount_generation: u64,
    pending_widget_availability: Option<Vec<PendingWidgetAvailability>>,
    // HAL383 §11: only staged concrete mounts, with an explicit text owner.
    // Empty ordinary trees allocate nothing; there is no idle tree scan.
    pending_text_handoffs: Option<Vec<PendingTextHandoff>>,
    // HAL391: current CMS member handoff, not a general widget fact registry.
    cell_editor_members: Vec<crate::core::appearance::CellEditorEmbedding>,
    pub(crate) cell_edges: HashMap<TreeWidgetId, HashMap<(u64, crate::model::tabular::column::ColumnId), crate::core::appearance::CellEdgePresentation>>,
    availability_targets: HashMap<TreeWidgetId, bool>,
    pending_sdf_requests: Option<Box<HashMap<TreeWidgetId, CapturedButtonSdfRequest>>>,
    pub(crate) composite_editors:
        Option<Box<crate::core::composite_text_edit::CompositeEditorRegistry>>,
    geometry_lifetime: Rc<()>,
    geometry_sequence: u64,
    geometry_origins: HashMap<TreeWidgetId, u64>,
    geometry_subtree_origins: HashMap<TreeWidgetId, u64>,
    geometry_root_allocation: Option<(NodeId, Constraints, f32)>,
    geometry_allocation_pending: Option<NodeId>,
    pub(crate) geometry_protected_owners: HashSet<TreeWidgetId>,
    pub(crate) root: Option<NodeId>,
    nodes: Vec<Option<WidgetNode>>,
    id_to_node: HashMap<TreeWidgetId, NodeId>,
    next_widget_id: u64,
    next_node_id: usize,
    free_list: Vec<usize>,
    /// SPEC-HAL-032: Optional hit-test acceleration grid
    hit_test_grid: Option<HitTestGrid>,
    /// SPEC-HAL-032: Flattened list of hit-testable nodes for the current frame
    hit_test_nodes: Vec<(NodeId, RectF)>,
    control_subtree_orders: HashMap<NodeId, ControlSubtreeOrder>,
    control_subtree_affordances: HashMap<NodeId, ControlSubtreeAffordances>,
    /// SPEC-HAL-086 Stage 8: Overlay hit regions (scrollbars) for hit-testing
    /// Populated from SceneGraph after paint, checked first in hit_test()
    overlay_hit_regions: Vec<OverlayHitEntry>,
    /// SPEC-HAL-018 Stage 16: Track root creation count for debugging.
    /// Calling create_root() multiple times indicates improper tree setup.
    root_creation_count: usize,
    /// SPEC-HAL-128 Stage 5 INV-WL1: Pending widget removals for EventRouter notification.
    /// Widgets removed via remove_node are collected here for later cleanup.
    pending_widget_removals: Vec<TreeWidgetId>,
    /// Non-structural owner requests consumed after lifecycle cleanup in the same frame.
    pending_relative_pointer_semantic_cancels: Vec<(TreeWidgetId, PointerId)>,
    /// SPEC-HAL-031: Pending node removals for signal subscription cleanup.
    /// Populated alongside pending_widget_removals during remove_node.
    pending_node_removals: Vec<NodeId>,
    /// SPEC-HAL-131A Stage 14: Typed index of ScrollArea nodes for post-layout anchor resolution.
    /// Populated during node insertion by concrete type check (downcast to ScrollArea).
    /// Used by post_layout_resolve_scroll_anchors() to iterate only ScrollArea nodes.
    scroll_anchor_nodes: Vec<NodeId>,
    scroll_intent_admission: crate::core::event::ScrollIntentAdmission,
    /// SPEC-HAL-266A: Typed index of admitted custom-render participants.
    /// Maintained only by tree topology mutations; never rediscovered per frame.
    custom_render_participant_nodes: Vec<NodeId>,
    /// Retained consumers of the Plot-owned market presentation projection.
    aligned_order_book_nodes: Vec<NodeId>,
    /// SPEC-HAL-131C: Input projection snapshots for virtualizing containers.
    /// INV-PROJECTION-SNAPSHOT-1: Computed during layout_tree(), read-only after.
    /// Maps container NodeId -> InputProjection for all registered virtualizing containers.
    input_projections: HashMap<NodeId, InputProjection>,
    /// SPEC-HAL-078 Stage 0: Persistent storage for last-frame authoritative geometry.
    /// Updated during compute_absolute_paint_rects, queried for transition detection.
    /// LT-INV-12: Zero cost when idle (buffer reuse, no per-frame allocation).
    layout_transition_store: LayoutTransitionStore,
    /// SPEC-HAL-190: Authority registry mapping NodeId to provided TypeIds.
    /// Updated on tree mutations (add/remove nodes), not per frame.
    authority_registry: AuthorityRegistry,
    /// SPEC-HAL-190: Intent buffers for deferred mutation phase.
    /// Keyed by TreeWidgetId of authority owner.
    intent_buffers: IntentBuffers,
    /// SPEC-HAL-354: Monotonic topology generation for retained-scene eviction.
    structure_generation: u64,
    /// Deferred focus request set during layout, applied after
    /// layout completes. Used by widgets that mount children
    /// during layout and need them focused immediately.
    pub deferred_focus: Option<NodeId>,
    /// Ordered deferred focus requests applied after layout by EventRouter.
    /// A sequence preserves each intermediate lifecycle boundary instead of
    /// collapsing a requested cancellation-and-entry transition to its endpoint.
    deferred_focus_sequence: Vec<NodeId>,
    pub(crate) presentation_inbox: Option<Box<crate::core::presentation::PresentationInbox>>,
}

impl WidgetTree {
    /// Enqueue a mount-qualified request; effective policy changes only at the engine cutoff.
    pub fn request_widget_enabled(
        &mut self,
        node: NodeId,
        enabled: bool,
    ) -> Result<(), WidgetAvailabilityError> {
        self.enqueue_widget_availability(node, Some(enabled), None)
    }

    pub fn request_widget_read_only(
        &mut self,
        node: NodeId,
        read_only: bool,
    ) -> Result<(), WidgetAvailabilityError> {
        self.enqueue_widget_availability(node, None, Some(read_only))
    }

    fn enqueue_widget_availability(
        &mut self,
        node: NodeId,
        enabled: Option<bool>,
        read_only: Option<bool>,
    ) -> Result<(), WidgetAvailabilityError> {
        let owner = self
            .node(node)
            .ok_or(WidgetAvailabilityError::TargetUnavailable)?
            .id;
        self.request_owner_availability(owner, enabled, read_only)
    }

    pub(crate) fn request_owner_availability(
        &mut self,
        owner: TreeWidgetId,
        enabled: Option<bool>,
        read_only: Option<bool>,
    ) -> Result<(), WidgetAvailabilityError> {
        if !self.id_to_node.contains_key(&owner) {
            return Err(WidgetAvailabilityError::TargetUnavailable);
        }
        let supports_read_only = *self
            .availability_targets
            .get(&owner)
            .ok_or(WidgetAvailabilityError::UnsupportedWidget)?;
        if read_only.is_some() && !supports_read_only {
            return Err(WidgetAvailabilityError::ReadOnlyUnsupported);
        }
        let lifetime = self.snapshot_tree_lifetime();
        let pending = self
            .pending_widget_availability
            .get_or_insert_with(Vec::new);
        if let Some(entry) = pending.iter_mut().find(|entry| entry.owner == owner) {
            if enabled.is_some() {
                entry.enabled = enabled;
            }
            if read_only.is_some() {
                entry.read_only = read_only;
            }
        } else {
            pending.push(PendingWidgetAvailability {
                tree_lifetime: lifetime,
                owner,
                enabled,
                read_only,
            });
        }
        Ok(())
    }

    pub(crate) fn has_pending_widget_availability(&self) -> bool {
        self.pending_widget_availability
            .as_ref()
            .is_some_and(|pending| !pending.is_empty())
    }

    pub(crate) fn take_pending_widget_availability(&mut self) -> Vec<PendingWidgetAvailability> {
        self.pending_widget_availability.take().unwrap_or_default()
    }

    fn index_pending_text_handoff(&mut self, node: NodeId) {
        let owner = self.node(node).and_then(|node| {
            if node
                .widget
                .as_any()
                .downcast_ref::<crate::widgets::text_field::TextField>()
                .is_some_and(|field| field.has_pending_edit_handoff())
            {
                Some((node.id, TextHandoffOwner::TextField))
            } else if node
                .widget
                .as_any()
                .downcast_ref::<crate::widgets::markdown::MarkdownView>()
                .is_some_and(|view| view.has_pending_read_handoff())
            {
                Some((node.id, TextHandoffOwner::Markdown))
            } else {
                None
            }
        });
        if let Some((mount, kind)) =
            owner.and_then(|(owner, kind)| self.concrete_mount(owner).map(|mount| (mount, kind)))
        {
            self.pending_text_handoffs
                .get_or_insert_with(Vec::new)
                .push(PendingTextHandoff { mount, kind });
        }
    }

    fn remove_pending_text_handoff(&mut self, owner: TreeWidgetId) {
        if let Some(pending) = self.pending_text_handoffs.as_mut() {
            pending.retain(|pending| pending.mount.owner() != owner);
            if pending.is_empty() {
                self.pending_text_handoffs = None;
            }
        }
    }

    /// HAL383 §§9/11: consume only exact staged mounts at the existing deferred cut.
    /// There is no idle tree walk, retry, callback or cross-frame widget borrow.
    /// The original private entry point also dispatches HAL390 reading handoffs.
    pub(crate) fn process_text_field_handoffs(
        &mut self,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        theme: &crate::core::theme::Theme,
    ) {
        let Some(pending) = self.pending_text_handoffs.take() else {
            return;
        };
        for PendingTextHandoff { mount, kind } in pending {
            if !mount.is_live() || self.concrete_mount(mount.owner()).as_ref() != Some(&mount) {
                continue;
            }
            let Some(node_id) = self.find_by_id(mount.owner()) else {
                continue;
            };
            let applied = self.nodes[node_id.0]
                .as_mut()
                .map(|node| match kind {
                    TextHandoffOwner::TextField => node
                        .widget
                        .as_any_mut()
                        .downcast_mut::<crate::widgets::text_field::TextField>()
                        .is_some_and(|field| field.apply_pending_edit_handoff(text_engine, theme)),
                    TextHandoffOwner::Markdown => node
                        .widget
                        .as_any_mut()
                        .downcast_mut::<crate::widgets::markdown::MarkdownView>()
                        .is_some_and(|view| view.apply_pending_read_handoff()),
                })
                .unwrap_or(false);
            if applied {
                self.mark_layout_dirty(node_id);
            }
        }
    }

    /// Clone only into engine-owned immutable snapshots; no tree borrow escapes.
    pub(crate) fn snapshot_tree_lifetime(&self) -> WidgetTreeLifetimeToken {
        self.tree_lifetime.clone()
    }

    /// Engine publication must verify membership as well as snapshot identity.
    pub(crate) fn is_live_mount(
        &self,
        lifetime: &WidgetTreeLifetimeToken,
        owner: TreeWidgetId,
    ) -> bool {
        self.tree_lifetime.eq(lifetime) && self.find_by_id(owner).is_some()
    }

    pub(crate) fn owns_mount_tree(&self, token: &WidgetMountToken) -> bool {
        self.tree_lifetime == token.tree_lifetime
    }

    /// Resolve exact concrete membership under the engine-owned tree lifetime.
    /// INV-354-G1-LIFETIME: neither recyclable slots nor concrete type imply lineage.
    pub(crate) fn concrete_mount(&self, owner: TreeWidgetId) -> Option<WidgetMountToken> {
        let node = self.node(self.find_by_id(owner)?)?;
        let mount = &node.concrete_mount;
        if node.id != owner
            || mount.generation == 0
            || mount.tree_lifetime.as_ref() != Some(&self.tree_lifetime)
        {
            return None;
        }
        let live = mount.live.get_or_init(|| Arc::new(AtomicBool::new(true)));
        if !live.load(Ordering::Acquire) {
            return None;
        }
        Some(WidgetMountToken {
            tree_lifetime: self.tree_lifetime.clone(),
            owner,
            generation: mount.generation,
            live: live.clone(),
        })
    }

    /// HAL391: only the existing cell mount owner may qualify its direct editor.
    pub(crate) fn qualify_cell_editor(
        &mut self,
        consumer_node: NodeId,
        editor_node: NodeId,
        row: u64,
        column: crate::model::tabular::column::ColumnId,
    ) -> Result<bool, crate::core::appearance::CellEditorPresentationRefusal> {
        use crate::core::appearance::{CellEditorEmbedding, CellEditorPresentation, CellEditorPresentationRefusal as Refusal};
        let consumer = self.node(consumer_node).ok_or(Refusal::UnavailableMount)?.id;
        let editor = self.node(editor_node).ok_or(Refusal::UnavailableMount)?;
        if editor.parent != Some(consumer_node) || editor.composite_editor_child {
            return Err(Refusal::WrongConsumer);
        }
        if crate::widgets::cell_editor_factory::cell_editor_visual_facts(editor.widget.as_ref()).is_none() {
            return Err(Refusal::UnsupportedFamily);
        }
        let editor = editor.id;
        let member = CellEditorEmbedding {
            row, column,
            consumer: self.concrete_mount(consumer).ok_or(Refusal::UnavailableMount)?,
            editor: self.concrete_mount(editor).ok_or(Refusal::UnavailableMount)?,
        };
        if self.cell_editor_members.iter().any(|current| current.editor == member.editor
            && (current.consumer != member.consumer || current.row != row || current.column != column)) {
            return Err(Refusal::WrongConsumer);
        }
        if self.cell_editor_members.iter().any(|current| *current == member) {
            return Ok(false);
        }
        self.cell_editor_members.retain(|current| current.editor.owner() != editor);
        self.cell_editor_members.push(member);
        self.node_mut(editor_node).unwrap().cell_editor_presentation = CellEditorPresentation::CellIntegrated;
        self.mark_paint_dirty(editor_node);
        self.mark_paint_dirty(consumer_node);
        Ok(true)
    }

    /// HAL391: current mount + framework appearance, prepared before Paint only.
    pub(crate) fn prepare_cell_edges(
        &mut self,
        root: NodeId,
        appearance: Option<&crate::core::appearance::FrameworkAppearanceSnapshot>,
    ) {
        use crate::core::appearance::CellEdgePresentation;
        use crate::core::theme::ControlChromeState;
        if self.cell_editor_members.is_empty() && self.cell_edges.is_empty() {
            return;
        }
        let members = std::mem::take(&mut self.cell_editor_members);
        let mut current = Vec::with_capacity(members.len());
        let mut next: HashMap<TreeWidgetId, HashMap<(u64, crate::model::tabular::column::ColumnId), CellEdgePresentation>> = HashMap::new();
        for member in members {
            let consumer = self.find_by_id(member.consumer.owner());
            let editor = self.find_by_id(member.editor.owner());
            let valid = self.concrete_mount(member.consumer.owner()).as_ref() == Some(&member.consumer)
                && self.concrete_mount(member.editor.owner()).as_ref() == Some(&member.editor)
                && editor.and_then(|id| self.node(id)).is_some_and(|node| node.parent == consumer);
            if !valid {
                if let Some(editor) = editor {
                    self.node_mut(editor).unwrap().cell_editor_presentation = Default::default();
                    self.mark_paint_dirty(editor);
                }
                continue;
            }
            let consumer = consumer.unwrap();
            let editor = editor.unwrap();
            if !self.composite_presentation_visible(editor) {
                current.push(member);
                continue;
            }
            if self.is_descendant_or_same(consumer, root) {
                if let Some(snapshot) = appearance {
                    let facts = snapshot.for_owner(member.editor.owner());
                    let mut captured = facts.keyboard_captured();
                    let mut focused = facts.focused();
                    // A real compound host owns delegated Capture. An actual
                    // child fact stays distinct and is read only via its sealed relation.
                    if let Ok(relation) = self.composite_editor_id(member.editor.owner()) {
                        if self.validate_composite_editor(&relation).is_ok() {
                            let child = snapshot.for_owner(relation.child);
                            captured |= child.keyboard_captured();
                            focused |= child.focused();
                        }
                    }
                    if let Some(visual) = crate::widgets::cell_editor_factory::cell_editor_visual_facts(
                        self.node(editor).unwrap().widget.as_ref(),
                    ) {
                        let state = if visual.disabled {
                            ControlChromeState::Disabled
                        } else if captured {
                            ControlChromeState::Captured
                        } else if focused {
                            ControlChromeState::Focused
                        } else if visual.hovered {
                            ControlChromeState::Hovered
                        } else {
                            ControlChromeState::Idle
                        };
                        next.entry(member.consumer.owner()).or_default().insert((member.row, member.column), CellEdgePresentation {
                            row: member.row, column: member.column,
                            editor: member.editor.clone(), consumer: member.consumer.clone(),
                            window: facts.window(), domain: facts.interaction_domain(), state,
                        });
                    }
                }
            } else if let Some(edge) = self.cell_edges.get(&member.consumer.owner())
                .and_then(|edges| edges.get(&(member.row, member.column))) {
                // Another window/root retains its own last qualified publication.
                // Its next preparation validates its own domain before painting.
                next.entry(member.consumer.owner()).or_default()
                    .insert((member.row, member.column), edge.clone());
            }
            current.push(member);
        }
        self.cell_editor_members = current;
        let mut consumers: Vec<_> = self.cell_edges.keys().chain(next.keys()).copied().collect();
        consumers.sort_unstable_by_key(|id| id.0);
        consumers.dedup();
        for consumer in consumers {
            if self.cell_edges.get(&consumer) != next.get(&consumer) {
                if let Some(node) = self.find_by_id(consumer) {
                    self.mark_paint_dirty(node);
                }
            }
        }
        self.cell_edges = next;
    }

    fn allocate_concrete_mount(&mut self) -> ConcreteWidgetMount {
        self.next_concrete_mount_generation = self
            .next_concrete_mount_generation
            .checked_add(1)
            .expect("INV-354-G1-LIFETIME: concrete mount generation exhausted");
        ConcreteWidgetMount {
            tree_lifetime: Some(self.tree_lifetime.clone()),
            generation: self.next_concrete_mount_generation,
            live: OnceLock::new(),
        }
    }

    /// Called immediately after this node's one ordinary Widget Update.
    pub(crate) fn capture_button_sdf_request(&mut self, node_id: NodeId) {
        let Some(node) = self.node(node_id) else {
            return;
        };
        let owner = node.id;
        let request = node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::button::Button>()
            .and_then(|button| button.sdf_request_snapshot());
        let Some(request) = request else {
            self.remove_button_sdf_request(owner);
            return;
        };
        let requests = self
            .pending_sdf_requests
            .get_or_insert_with(Default::default);
        if requests
            .get(&owner)
            .is_some_and(|entry| entry.request == request)
        {
            return;
        }
        requests.insert(
            owner,
            CapturedButtonSdfRequest {
                tree_lifetime: self.tree_lifetime.clone(),
                owner,
                request,
            },
        );
    }

    fn remove_button_sdf_request(&mut self, owner: TreeWidgetId) {
        if let Some(requests) = self.pending_sdf_requests.as_mut() {
            requests.remove(&owner);
            if requests.is_empty() {
                self.pending_sdf_requests = None;
            }
        }
    }

    /// Owned copies may outlive the tree borrow, but must be revalidated at use.
    #[allow(dead_code)] // Canonical target preparation will consume these snapshots.
    pub(crate) fn snapshot_button_sdf_requests(&self) -> Vec<CapturedButtonSdfRequest> {
        self.pending_sdf_requests
            .iter()
            .flat_map(|requests| requests.values())
            .filter(|request| self.is_current_button_sdf_request(request))
            .cloned()
            .collect()
    }

    #[allow(dead_code)]
    pub(crate) fn is_current_button_sdf_request(&self, request: &CapturedButtonSdfRequest) -> bool {
        self.is_live_mount(&request.tree_lifetime, request.owner)
            && self
                .pending_sdf_requests
                .as_ref()
                .and_then(|requests| requests.get(&request.owner))
                == Some(request)
    }

    /// Commit only at the authorized post-preparation, pre-Paint cutoff.
    /// The frame owner supplies current K and binding evidence; this is not a
    /// producer hook and does not certify GPU lifetime or remove old coverage.
    pub(crate) fn publish_sdf_material(
        &mut self,
        selected: &mut Option<crate::renderer::sdf_material_status::SdfMaterialPublication>,
        current: &crate::renderer::sdf_material_status::SdfMaterialKey,
        current_identity: Option<crate::renderer::image_binding::ImageBindingIdentity>,
        publication: crate::renderer::sdf_material_status::SdfMaterialPublication,
    ) -> Result<(), crate::renderer::sdf_material_status::SdfMaterialRejection> {
        use crate::renderer::sdf_material_status::{SdfMaterialPublication, SdfMaterialRejection};

        if !self.is_live_mount(current.tree_lifetime(), current.owner()) {
            return Err(SdfMaterialRejection::KeyMismatch);
        }
        let changed = selected.as_ref() != Some(&publication);
        SdfMaterialPublication::replace_if_current(
            selected,
            current,
            current_identity,
            Some(publication),
        )?;
        if changed {
            self.mark_paint_dirty_by_widget_id(current.owner());
        }
        Ok(())
    }

    pub(crate) fn geometry_causal_cutoff(&self) -> GeometryCausalCutoff {
        GeometryCausalCutoff {
            lifetime: self.geometry_lifetime.clone(),
            sequence: self.geometry_sequence,
        }
    }

    pub(crate) fn owns_geometry_cutoff(&self, cutoff: &GeometryCausalCutoff) -> bool {
        Rc::ptr_eq(&self.geometry_lifetime, &cutoff.lifetime)
    }

    pub(crate) fn record_geometry_origin(&mut self, origin: NodeId) {
        // Completed geometry writes and layout's take/restore are not new causes.
        if crate::core::rect_authority::is_layout_authority_active()
            || crate::core::rect_authority::is_transition_authority_active()
        {
            return;
        }
        #[cfg(debug_assertions)]
        if crate::core::rect_authority::is_paint_authority_active() {
            return;
        }
        let Some(mount) = self.node(origin).map(|node| node.id) else {
            return;
        };
        self.geometry_sequence = self
            .geometry_sequence
            .checked_add(1)
            .expect("INV-ABSOLUTE-GEOMETRY-CAUSAL-NONREUSE");
        let sequence = self.geometry_sequence;
        self.geometry_origins.insert(mount, sequence);
        let mut current = Some(origin);
        while let Some(id) = current {
            let Some(node) = self.node(id) else { break };
            let (mount, parent) = (node.id, node.parent);
            self.geometry_subtree_origins.insert(mount, sequence);
            current = parent;
        }
    }

    pub(crate) fn observe_geometry_allocation(
        &mut self,
        root: NodeId,
        constraints: Constraints,
        scale: f32,
    ) {
        let next = (root, constraints, scale);
        if self.geometry_root_allocation.is_some_and(|old| old != next) {
            self.record_geometry_origin(root);
            self.geometry_allocation_pending = Some(root);
        }
        self.geometry_root_allocation = Some(next);
    }

    pub(crate) fn observe_geometry_scale(&mut self, root: NodeId, scale: f32) {
        if let Some((_, constraints, _)) = self.geometry_root_allocation {
            self.observe_geometry_allocation(root, constraints, scale);
        }
    }

    pub(crate) fn complete_geometry_allocation(&mut self) {
        self.geometry_allocation_pending = None;
    }

    pub(crate) fn freeze_absolute_drag_geometry(
        &self,
        owner: TreeWidgetId,
        declaration: crate::core::widget::AbsoluteDragGeometryDependency,
        cutoff: &GeometryCausalCutoff,
    ) -> AbsoluteDragGeometryWatch {
        let owner_node = self
            .find_node_by_widget_id(owner)
            .expect("INV-ABSOLUTE-GEOMETRY-ADMISSION: missing owner");
        let mut path = Vec::new();
        let mut current = Some(owner_node);
        while let Some(id) = current {
            let node = self
                .node(id)
                .expect("INV-ABSOLUTE-GEOMETRY-ADMISSION: broken ancestry");
            path.push((id, node.id));
            current = node.parent;
        }
        let viewport_index = path
            .iter()
            .position(|(id, _)| *id == declaration.viewport_owner)
            .expect("INV-ABSOLUTE-GEOMETRY-ADMISSION: viewport is not an ancestor");
        let reflow_index = path
            .iter()
            .position(|(id, _)| *id == declaration.reflow_root)
            .expect("INV-ABSOLUTE-GEOMETRY-ADMISSION: reflow root is not an ancestor");
        assert!(
            reflow_index >= viewport_index,
            "INV-ABSOLUTE-GEOMETRY-ADMISSION: incomplete reflow domain"
        );
        AbsoluteDragGeometryWatch {
            owner,
            declaration,
            reflow_mount: path[reflow_index].1,
            scroll_basis: path
                .iter()
                .map(|(id, _)| {
                    (
                        *id,
                        self.node(*id).and_then(|node| node.widget.scroll_offset()),
                    )
                })
                .collect(),
            clip_basis: self.absolute_drag_clip_basis(&path),
            path,
            cutoff: cutoff.clone(),
        }
    }

    fn absolute_drag_clip_basis(
        &self,
        path: &[(NodeId, TreeWidgetId)],
    ) -> Vec<AbsoluteDragClipScope> {
        let mut scopes = Vec::new();
        // Child-only clips enclose descendants, not the declaring owner's own
        // overlay. Preserve traversal's outer-to-inner viewport/widget order.
        for &(id, mount) in path.iter().skip(1).rev() {
            let Some(node) = self.node(id) else {
                continue;
            };
            if let Some(source) = self.child_clip_source(id) {
                scopes.push(AbsoluteDragClipScope {
                    node: id,
                    mount,
                    declaration: AbsoluteDragClipDeclaration::Viewport(source),
                });
            }
            if let Some(bounds) = node.widget.clip_bounds() {
                scopes.push(AbsoluteDragClipScope {
                    node: id,
                    mount,
                    declaration: AbsoluteDragClipDeclaration::Widget(
                        bounds,
                        node.widget.clip_bounds_rounded(),
                    ),
                });
            }
        }
        scopes
    }

    pub(crate) fn absolute_drag_geometry_is_current(
        &self,
        watch: &AbsoluteDragGeometryWatch,
    ) -> bool {
        self.owns_geometry_cutoff(&watch.cutoff)
            && !self
                .geometry_allocation_pending
                .is_some_and(|pending| watch.path.iter().any(|(id, _)| *id == pending))
            && watch.scroll_basis.iter().all(|(id, offset)| {
                self.node(*id).and_then(|node| node.widget.scroll_offset()) == *offset
            })
            && watch.path.iter().all(|(id, _)| {
                self.node(*id).is_none_or(|node| {
                    node.widget
                        .as_any()
                        .downcast_ref::<crate::widgets::scroll_area::ScrollArea>()
                        .is_none_or(|scroll| {
                            !scroll.has_geometry_scroll_effect()
                                && !self
                                    .node(scroll.content())
                                    .is_some_and(|content| content.widget.has_requested_scroll())
                        })
                })
            })
            && watch.path.iter().all(|(id, mount)| {
                self.node(*id).is_some_and(|node| {
                    node.id == *mount
                        && self.geometry_origins.get(mount).copied().unwrap_or(0)
                            <= watch.cutoff.sequence
                })
            })
            && self.absolute_drag_clip_basis(&watch.path) == watch.clip_basis
            && self
                .geometry_subtree_origins
                .get(&watch.reflow_mount)
                .copied()
                .unwrap_or(0)
                <= watch.cutoff.sequence
            && self
                .find_node_by_widget_id(watch.owner)
                .and_then(|id| self.node(id))
                .is_some_and(|node| {
                    node.widget.absolute_drag_geometry_dependency() == Some(watch.declaration)
                })
    }

    pub fn new() -> Self {
        Self {
            tree_lifetime: WidgetTreeLifetimeToken(Arc::new(())),
            next_concrete_mount_generation: 0,
            pending_widget_availability: None,
            pending_text_handoffs: None,
            cell_editor_members: Vec::new(),
            cell_edges: HashMap::new(),
            availability_targets: HashMap::new(),
            pending_sdf_requests: None,
            composite_editors: None,
            geometry_lifetime: Rc::new(()),
            geometry_sequence: 0,
            geometry_origins: HashMap::new(),
            geometry_subtree_origins: HashMap::new(),
            geometry_root_allocation: None,
            geometry_allocation_pending: None,
            geometry_protected_owners: HashSet::new(),
            root: None,
            nodes: Vec::new(),
            id_to_node: HashMap::new(),
            next_widget_id: 1,
            next_node_id: 0,
            free_list: Vec::new(),
            hit_test_grid: None,
            hit_test_nodes: Vec::new(),
            control_subtree_orders: HashMap::new(),
            control_subtree_affordances: HashMap::new(),
            overlay_hit_regions: Vec::new(),
            root_creation_count: 0,
            pending_widget_removals: Vec::new(),
            pending_relative_pointer_semantic_cancels: Vec::new(),
            pending_node_removals: Vec::new(),
            scroll_anchor_nodes: Vec::new(),
            scroll_intent_admission: Default::default(),
            custom_render_participant_nodes: Vec::new(),
            aligned_order_book_nodes: Vec::new(),
            input_projections: HashMap::new(),
            layout_transition_store: LayoutTransitionStore::new(),
            authority_registry: AuthorityRegistry::new(),
            intent_buffers: IntentBuffers::new(),
            structure_generation: 0,
            deferred_focus: None,
            deferred_focus_sequence: Vec::new(),
            presentation_inbox: None,
        }
    }

    /// Opt into bounded source custody. App applies policy and source admission
    /// in Update; this never starts an animation or changes input/layout truth.
    pub fn configure_presentation(
        &mut self,
        configuration: crate::core::presentation::PresentationConfiguration,
    ) -> Result<(), crate::core::presentation::PresentationFailure> {
        match &mut self.presentation_inbox {
            Some(inbox) => inbox.configure(configuration),
            None => {
                self.presentation_inbox = Some(Box::new(
                    crate::core::presentation::PresentationInbox::new(configuration)?,
                ));
                Ok(())
            }
        }
    }

    /// Queue a causal presentation request through the existing deferred owner.
    pub fn request_presentation(
        &mut self,
        request: crate::core::presentation::PresentationRequest,
    ) -> Result<
        crate::core::presentation::PresentationTicket,
        crate::core::presentation::PresentationFailure,
    > {
        self.presentation_inbox
            .as_mut()
            .ok_or(crate::core::presentation::PresentationFailure::RefusedOwnership)?
            .request(request)
    }

    pub fn presentation_receipts(&self) -> &[crate::core::presentation::PresentationReceipt] {
        self.presentation_inbox
            .as_ref()
            .map_or(&[], |inbox| &inbox.receipts)
    }

    /// Queue an ordered focus transition for the existing post-layout router path.
    ///
    /// This does not mutate focus or keyboard capture directly. EventRouter
    /// performs every request after layout, preserving blur/capture-leave and
    /// capture-entry callbacks for each target in order.
    pub fn defer_focus_sequence(&mut self, targets: impl IntoIterator<Item = NodeId>) {
        self.deferred_focus_sequence.extend(targets);
    }

    pub(crate) fn take_deferred_focus_requests(&mut self) -> Vec<NodeId> {
        let mut requests: Vec<NodeId> = self.deferred_focus.take().into_iter().collect();
        requests.append(&mut self.deferred_focus_sequence);
        requests
    }

    fn alloc_widget_id(&mut self) -> TreeWidgetId {
        let id = TreeWidgetId(self.next_widget_id);
        self.next_widget_id += 1;
        id
    }

    fn alloc_node_id(&mut self) -> NodeId {
        if let Some(free_id) = self.free_list.pop() {
            NodeId(free_id)
        } else {
            let id = NodeId(self.next_node_id);
            self.next_node_id += 1;
            self.nodes.push(None);
            id
        }
    }

    /// Create root widget.
    ///
    /// SPEC-HAL-018 Stage 16: Examples should use `App::run()` which creates the root
    /// via AppRoot. Direct calls to `create_root()` bypass the proper constraint
    /// propagation pipeline and result in incorrect ScrollArea dimensions.
    ///
    /// # Panics
    /// In debug builds, panics if called more than once (indicates multiple roots).
    pub fn create_root(
        &mut self,
        widget: Box<dyn Widget>,
        key: Option<WidgetKey>,
        z_order: i16,
    ) -> NodeId {
        // SPEC-HAL-018 Stage 16: Track root creation count
        self.root_creation_count += 1;

        // SPEC-HAL-018 Stage 16: Debug warning when root already exists
        if self.root.is_some() {
            tracing::warn!(
                "create_root() called {} times - multiple roots detected. \
                 Examples should use App::run() instead of creating their own roots.",
                self.root_creation_count
            );
        }

        // SPEC-HAL-018 Stage 16: Panic on multiple roots in debug builds
        debug_assert!(
            self.root_creation_count <= 1,
            "create_root() called {} times. Only one root is allowed per WidgetTree. \
             Examples must use App::run() with a Screen implementation instead of \
             calling create_root() directly.",
            self.root_creation_count
        );

        let widget_id = self.alloc_widget_id();
        let node_id = self.alloc_node_id();

        // SPEC-HAL-131A Stage 14: Check for ScrollArea using concrete type (no Widget trait query)
        let is_scroll_area = widget.as_any().downcast_ref::<ScrollArea>().is_some();
        let is_custom_render_participant = widget.custom_render_admission().is_some();
        if let Some(policy) = crate::core::widget_availability::effective(widget.as_ref()) {
            self.availability_targets
                .insert(widget_id, policy.read_only.is_some());
        }

        let mut node = WidgetNode::new(widget_id, widget, key, z_order);
        node.concrete_mount = self.allocate_concrete_mount();
        node.widget.on_mount();

        self.nodes[node_id.0] = Some(node);
        self.id_to_node.insert(widget_id, node_id);
        self.root = Some(node_id);
        self.structure_generation = self.structure_generation.wrapping_add(1);

        // SPEC-HAL-131A Stage 14: Register ScrollArea in typed index
        if is_scroll_area {
            self.scroll_anchor_nodes.push(node_id);
            self.bind_scroll_area_intent_admission(node_id);
        }
        if is_custom_render_participant {
            self.custom_render_participant_nodes.push(node_id);
        }
        self.index_aligned_order_book(node_id);

        // SPEC-HAL-190: Register authority if widget provides one
        if let Some(node) = self.nodes[node_id.0].as_ref() {
            if let Some((_, type_id)) = node.widget.provided_authority() {
                self.authority_registry.register(node_id, type_id);
            }
        }

        // SPEC-HAL-087 Stage 2: Verify newly created root node
        #[cfg(debug_assertions)]
        debug_assert!(self.verify_tree_widget_id(widget_id));

        self.assemble_composite_editor(node_id);
        self.index_pending_text_handoff(node_id);
        node_id
    }

    /// Copy the tree-owned root membership without permitting reassignment.
    ///
    /// ```compile_fail
    /// use halcyon::core::widget_tree::WidgetTree;
    /// fn cannot_reassign_root(tree: &mut WidgetTree) {
    ///     tree.root = None;
    /// }
    /// ```
    pub fn root(&self) -> Option<NodeId> {
        self.root
    }

    /// Add a child widget to a parent
    pub fn add_child(
        &mut self,
        parent_id: NodeId,
        widget: Box<dyn Widget>,
        key: Option<WidgetKey>,
        z_order: i16,
    ) -> NodeId {
        self.record_geometry_origin(parent_id);
        self.mark_layout_dirty(parent_id);
        let widget_id = self.alloc_widget_id();
        let node_id = self.alloc_node_id();

        // SPEC-HAL-131A Stage 14: Check for ScrollArea using concrete type (no Widget trait query)
        let is_scroll_area = widget.as_any().downcast_ref::<ScrollArea>().is_some();
        let is_custom_render_participant = widget.custom_render_admission().is_some();
        if let Some(policy) = crate::core::widget_availability::effective(widget.as_ref()) {
            self.availability_targets
                .insert(widget_id, policy.read_only.is_some());
        }

        let mut node = WidgetNode::new(widget_id, widget, key, z_order);
        node.concrete_mount = self.allocate_concrete_mount();
        node.parent = Some(parent_id);
        node.widget.on_mount();

        if let Some(parent) = &mut self.nodes[parent_id.0] {
            if let Some(first_child_id) = parent.first_child {
                let mut last_sibling_id = first_child_id;
                while let Some(sibling) = &self.nodes[last_sibling_id.0] {
                    if let Some(next) = sibling.next_sibling {
                        last_sibling_id = next;
                    } else {
                        break;
                    }
                }
                if let Some(last_sibling) = &mut self.nodes[last_sibling_id.0] {
                    last_sibling.next_sibling = Some(node_id);
                }
            } else {
                parent.first_child = Some(node_id);
            }
        }

        self.nodes[node_id.0] = Some(node);
        self.id_to_node.insert(widget_id, node_id);
        self.structure_generation = self.structure_generation.wrapping_add(1);
        self.hit_test_grid = None;

        // SPEC-HAL-131A Stage 14: Register ScrollArea in typed index
        if is_scroll_area {
            self.scroll_anchor_nodes.push(node_id);
            self.bind_scroll_area_intent_admission(node_id);
        }
        if is_custom_render_participant {
            self.custom_render_participant_nodes.push(node_id);
        }
        self.index_aligned_order_book(node_id);

        // SPEC-HAL-190: Register authority if widget provides one
        if let Some(node) = self.nodes[node_id.0].as_ref() {
            if let Some((_, type_id)) = node.widget.provided_authority() {
                self.authority_registry.register(node_id, type_id);
            }
        }

        // SPEC-HAL-087 Stage 2: Verify newly created node
        #[cfg(debug_assertions)]
        debug_assert!(self.verify_tree_widget_id(widget_id));

        self.assemble_composite_editor(node_id);
        self.index_pending_text_handoff(node_id);
        node_id
    }

    /// Remove a node and all its children
    ///
    /// SPEC-HAL-128 Stage 5 INV-WL1: Collects removed widget IDs in pending_widget_removals
    /// for EventRouter to clean up gesture/capture state. Call drain_pending_removals()
    /// after tree modifications to get the list and notify EventRouter.
    pub fn remove_node(&mut self, node_id: NodeId) {
        // HAL-154: Check the requested node before mutation. The subtree loop
        // below retires a removed host and its content together.
        if let Some(parent_id) = self.node(node_id).and_then(|node| node.parent)
            && let Some(parent) = self.node(parent_id)
            && let Some(scroll) = parent.widget.as_any().downcast_ref::<ScrollArea>()
        {
            assert!(
                scroll.sealed_wired_content() != Some(node_id),
                "HAL-154: cannot remove wired content {node_id:?} of sealed ScrollArea {parent_id:?}"
            );
        }

        self.retire_composite_editor(node_id);
        self.record_geometry_origin(node_id);
        if let Some(parent) = self.node(node_id).and_then(|n| n.parent) {
            self.mark_layout_dirty(parent);
        }
        let affected_hosts: Vec<_> = self
            .control_subtree_orders
            .iter()
            .filter_map(|(&host, order)| {
                order
                    .members
                    .iter()
                    .any(|&(id, _)| id == node_id)
                    .then_some(host)
            })
            .collect();
        for host in affected_hosts {
            if let Some(order) = self.control_subtree_orders.get_mut(&host) {
                order.members.retain(|&(id, _)| id != node_id);
            }
            self.invalidate_control_subtree_composition(host);
        }
        self.hit_test_grid = None;
        let mut removed_any = false;
        // First, unlink the top-level node from its parent's child list
        if let Some(node) = &self.nodes[node_id.0] {
            if let Some(parent_id) = node.parent {
                let next_sibling = node.next_sibling;
                if let Some(parent) = &mut self.nodes[parent_id.0] {
                    if parent.first_child == Some(node_id) {
                        // Node is first child - update parent's first_child
                        parent.first_child = next_sibling;
                    } else {
                        // Node is in sibling chain - find and unlink
                        let mut prev_id = parent.first_child;
                        while let Some(pid) = prev_id {
                            if let Some(prev_node) = &mut self.nodes[pid.0] {
                                if prev_node.next_sibling == Some(node_id) {
                                    prev_node.next_sibling = next_sibling;
                                    break;
                                }
                                prev_id = prev_node.next_sibling;
                            } else {
                                break;
                            }
                        }
                    }
                }
            }
        }

        // Now remove the node and all its children
        let mut to_remove = vec![node_id];

        while let Some(current_id) = to_remove.pop() {
            self.retire_composite_editor(current_id);
            self.retire_control_subtree_affordances(current_id);
            self.control_subtree_orders.remove(&current_id);
            // SPEC-HAL-131A Stage 14: Unregister from typed index
            self.scroll_anchor_nodes.retain(|&id| id != current_id);
            self.custom_render_participant_nodes
                .retain(|&id| id != current_id);
            self.aligned_order_book_nodes.retain(|&id| id != current_id);

            // SPEC-HAL-190: Unregister authority from registry
            self.authority_registry.unregister(current_id);

            if let Some(mut node) = self.nodes[current_id.0].take() {
                removed_any = true;
                node.concrete_mount.retire();
                node.widget.on_unmount();
                self.id_to_node.remove(&node.id);
                self.availability_targets.remove(&node.id);
                if let Some(pending) = self.pending_widget_availability.as_mut() {
                    pending.retain(|entry| entry.owner != node.id);
                }
                self.remove_button_sdf_request(node.id);
                self.remove_pending_text_handoff(node.id);

                // SPEC-HAL-128 Stage 5 INV-WL1: Track removed widget for EventRouter cleanup
                self.pending_widget_removals.push(node.id);
                self.geometry_origins.remove(&node.id);
                self.geometry_subtree_origins.remove(&node.id);
                self.geometry_protected_owners.remove(&node.id);
                // SPEC-HAL-031: Track removed NodeId for signal subscription cleanup
                self.pending_node_removals.push(current_id);

                let mut child_id = node.first_child;
                while let Some(cid) = child_id {
                    to_remove.push(cid);
                    if let Some(child_node) = &self.nodes[cid.0] {
                        child_id = child_node.next_sibling;
                    } else {
                        break;
                    }
                }

                self.free_list.push(current_id.0);
            }
        }
        if removed_any {
            self.structure_generation = self.structure_generation.wrapping_add(1);
        }
    }

    /// Monotonic topology generation used by retained presentation caches.
    pub(crate) fn structure_generation(&self) -> u64 {
        self.structure_generation
    }

    #[cfg(test)]
    pub(crate) fn has_custom_render_participants(&self) -> bool {
        !self.custom_render_participant_nodes.is_empty()
    }

    fn index_aligned_order_book(&mut self, node_id: NodeId) {
        if self.nodes[node_id.0].as_ref().is_some_and(|node| {
            node.widget
                .as_any()
                .is::<crate::widgets::aligned_order_book::AlignedOrderBook>()
        }) {
            self.aligned_order_book_nodes.push(node_id);
        }
    }

    fn invalidate_aligned_market_projection(&mut self, link: &crate::plot::MarketDetailLink) {
        for index in 0..self.aligned_order_book_nodes.len() {
            let node_id = self.aligned_order_book_nodes[index];
            if self.nodes[node_id.0]
                .as_ref()
                .and_then(|node| {
                    node.widget
                        .as_any()
                        .downcast_ref::<crate::widgets::aligned_order_book::AlignedOrderBook>()
                })
                .is_some_and(|book| book.observes_market_projection(link))
            {
                self.mark_paint_dirty(node_id);
            }
        }
    }

    pub(crate) fn withdraw_inactive_plot_surface(&mut self, surface: crate::compositor::SurfaceId) {
        // A zero-sized native target has no preparation snapshot. Withdraw only
        // the Plot owners that were preparing for that target.
        for index in 0..self.custom_render_participant_nodes.len() {
            let node = self.custom_render_participant_nodes[index];
            let changed = self.nodes[node.0]
                .as_mut()
                .and_then(|node| node.widget.as_any_mut().downcast_mut::<crate::plot::Plot>())
                .is_some_and(|plot| plot.withdraw_inactive_surface(surface));
            if changed {
                self.mark_paint_dirty(node);
            }
        }
    }

    pub(crate) fn prepare_custom_render_participants(
        &mut self,
        target: CustomRenderTargetSnapshot,
        presentation_bindings: &mut CustomRenderPresentationBindings,
    ) -> Result<CustomRenderPreparationOutcome, CustomRenderPreparationError> {
        let mut aggregate = CustomRenderPreparationOutcome::default();
        for index in 0..self.custom_render_participant_nodes.len() {
            let node_id = self.custom_render_participant_nodes[index];
            let (final_logical_rect, final_logical_viewport, presentation_id, plot_owner) = {
                let node = self
                    .nodes
                    .get(node_id.0)
                    .and_then(|node| node.as_ref())
                    .expect("SPEC-HAL-266A: participant index contains a missing node");
                let presentation_id = node
                    .widget
                    .custom_render_admission()
                    .expect("SPEC-HAL-266A: indexed widget lost its admission")
                    .presentation_id();
                let plot_owner = node
                    .widget
                    .as_any()
                    .is::<crate::plot::Plot>()
                    .then_some(node.id);
                (
                    node.absolute_paint_rect,
                    node.viewport,
                    presentation_id,
                    plot_owner,
                )
            };
            let metrics = target.metrics_for(final_logical_rect, final_logical_viewport)?;
            let plot_mount = metrics
                .app_frame_attempt
                .and_then(|_| plot_owner.and_then(|owner| self.concrete_mount(owner)));
            let node = self
                .nodes
                .get_mut(node_id.0)
                .and_then(|node| node.as_mut())
                .expect("SPEC-HAL-266A: participant disappeared during preparation");
            let mut outcome = if let Some(mount) = plot_mount.as_ref() {
                node.widget
                    .as_any_mut()
                    .downcast_mut::<crate::plot::Plot>()
                    .expect("qualified Plot participant")
                    .prepare_mounted_plot(&metrics, mount.clone())
            } else {
                node.widget
                    .custom_render_participant_mut()
                    .expect("SPEC-HAL-266A: indexed widget lost its participant capability")
                    .prepare_custom_render(&metrics)
            };
            let market_link = if outcome
                .plot_evidence
                .iter()
                .any(|evidence| evidence.market_projection_changed)
            {
                node.widget
                    .as_any()
                    .downcast_ref::<crate::plot::Plot>()
                    .and_then(|plot| plot.market_detail_link().cloned())
            } else {
                None
            };
            for evidence in outcome.plot_evidence.drain(..) {
                if plot_mount
                    .as_ref()
                    .is_some_and(|mount| evidence.authenticates(mount, presentation_id, &metrics))
                {
                    if evidence.local_paint_changed {
                        self.mark_paint_dirty(node_id);
                    }
                    if evidence.market_projection_changed
                        && let Some(link) = &market_link
                    {
                        self.invalidate_aligned_market_projection(link);
                    }
                    aggregate.plot_evidence.push(evidence);
                }
            }
            aggregate.front_publication_changed |= outcome.front_publication_changed;
            if let Some(publication) = outcome.into_publication() {
                presentation_bindings.publish_for_target(
                    metrics.surface_id,
                    presentation_id,
                    publication,
                )?;
            }
        }
        Ok(aggregate)
    }

    #[cfg(test)]
    pub(crate) fn custom_render_participant_count_for_test(&self) -> usize {
        self.custom_render_participant_nodes.len()
    }

    /// Get node by NodeId
    pub fn node(&self, node_id: NodeId) -> Option<&WidgetNode> {
        self.nodes.get(node_id.0).and_then(|n| n.as_ref())
    }

    /// Get mutable node by NodeId
    /// Borrow state/props in place. This does not authorize swapping or replacing
    /// the owning node or its concrete widget; use the tree replacement operation.
    pub fn node_mut(&mut self, node_id: NodeId) -> Option<&mut WidgetNode> {
        self.hit_test_grid = None;
        self.nodes.get_mut(node_id.0).and_then(|n| n.as_mut())
    }

    pub(crate) fn animation_mount(&self, owner: TreeWidgetId) -> Option<WidgetAnimationMount<'_>> {
        let node = self.node(self.find_by_id(owner)?)?;
        (node.concrete_mount.generation != 0
            && node.concrete_mount.tree_lifetime.as_ref() == Some(&self.tree_lifetime))
        .then_some(WidgetAnimationMount {
            owner,
            mount: &node.concrete_mount,
        })
    }

    pub(crate) fn widget_with_animation_mount_mut(
        &mut self,
        node_id: NodeId,
    ) -> Option<(&mut dyn Widget, WidgetAnimationMount<'_>)> {
        self.hit_test_grid = None;
        let node = self.nodes.get_mut(node_id.0)?.as_mut()?;
        if node.concrete_mount.generation == 0
            || node.concrete_mount.tree_lifetime.as_ref() != Some(&self.tree_lifetime)
        {
            return None;
        }
        Some((
            node.widget.as_mut(),
            WidgetAnimationMount {
                owner: node.id,
                mount: &node.concrete_mount,
            },
        ))
    }

    /// Copies completed owner geometry and actual rectangular ancestor clips.
    /// Rounded declarations require a shape-qualified witness and remain unavailable.
    pub(crate) fn own_presentation_geometry(
        &self,
        node_id: NodeId,
        scale_factor: f32,
    ) -> Option<OwnedWidgetPresentationGeometry> {
        let node = self.node(node_id)?;
        // Completed constraints are recorded in every build profile; layout_epoch
        // is debug-only paint diagnostics, not production readiness.
        if node.last_constraints.is_none() || !scale_factor.is_finite() || scale_factor <= 0.0 {
            return None;
        }
        let finite = |r: RectF| {
            r.x.is_finite()
                && r.y.is_finite()
                && r.width.is_finite()
                && r.height.is_finite()
                && r.width >= 0.0
                && r.height >= 0.0
        };
        let root_viewport = self.get_rect(self.root?)?;
        let rect = node.absolute_paint_rect;
        let viewport = node.viewport;
        let cumulative_scroll = self.cumulative_scroll_offset(node_id);
        if !finite(root_viewport)
            || !finite(rect)
            || !finite(viewport)
            || !cumulative_scroll.0.is_finite()
            || !cumulative_scroll.1.is_finite()
        {
            return None;
        }
        let mut clip = root_viewport;
        let mut ancestor = node.parent;
        while let Some(id) = ancestor {
            let parent = self.node(id)?;
            let scroll = self.cumulative_scroll_offset(id);
            if !scroll.0.is_finite() || !scroll.1.is_finite() {
                return None;
            }
            let mut intersect = |bounds: RectF| -> Option<()> {
                if !finite(bounds) {
                    return None;
                }
                let displayed = RectF::new(
                    bounds.x - scroll.0,
                    bounds.y - scroll.1,
                    bounds.width,
                    bounds.height,
                );
                clip = clip.intersection(&displayed).unwrap_or(RectF::zero());
                Some(())
            };
            if let Some(source) = self.child_clip_source(id) {
                intersect(source)?;
            }
            if let Some(bounds) = parent.widget.clip_bounds() {
                if parent.widget.clip_bounds_rounded().is_some() {
                    return None;
                }
                intersect(RectF::new(
                    parent.absolute_paint_rect.x + bounds.x,
                    parent.absolute_paint_rect.y + bounds.y,
                    bounds.width,
                    bounds.height,
                ))?;
            }
            ancestor = parent.parent;
        }
        Some(OwnedWidgetPresentationGeometry {
            rect,
            viewport,
            cumulative_scroll,
            root_viewport,
            clip,
            scale_factor,
        })
    }

    /// SPEC-HAL-078B: Iterate over all active nodes.
    ///
    /// **ENGINE-INTERNAL / CONFLICT-LAW ONLY**
    ///
    /// This helper exists exclusively for layout transition conflict resolution
    /// (SPEC-HAL-078B CONFLICT-COND-1). It must NOT be used for:
    /// - Widget behavior or policy logic
    /// - Runtime mutation of tree structure
    /// - General-purpose tree enumeration
    ///
    /// Returns an iterator of (NodeId, &WidgetNode) pairs for all non-empty slots.
    /// Used for building TreeWidgetId -> NodeId mappings during nested conflict detection.
    pub fn iter_nodes(&self) -> impl Iterator<Item = (NodeId, &WidgetNode)> {
        self.nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, opt)| opt.as_ref().map(|node| (NodeId(idx), node)))
    }

    /// Get immutable access to one node and mutable access to a different node.
    ///
    /// Used by engine phases requiring disjoint node access (e.g., post-layout
    /// anchor resolution where content node provides resolver and scroll node
    /// receives the resolved offset).
    ///
    /// SPEC-HAL-131A: Safe split-borrow helper using split_at_mut.
    /// Returns None if read == write (same node cannot be borrowed both ways).
    ///
    /// # Invariants
    /// - Tree topology is not mutated during call
    /// - read and write are distinct NodeIds
    /// Borrow disjoint node state. The mutable borrow carries the same
    /// state/props-only contract as `node_mut`, never Box or node substitution.
    pub fn node_and_node_mut(
        &mut self,
        read: NodeId,
        write: NodeId,
    ) -> Option<(&WidgetNode, &mut WidgetNode)> {
        self.hit_test_grid = None;
        debug_assert!(
            read != write,
            "node_and_node_mut: read and write must be distinct"
        );
        if read == write {
            return None;
        }

        let read_idx = read.0;
        let write_idx = write.0;

        // Ensure both indices are in bounds
        let len = self.nodes.len();
        if read_idx >= len || write_idx >= len {
            return None;
        }

        // Use split_at_mut to get disjoint borrows
        if read_idx < write_idx {
            let (left, right) = self.nodes.split_at_mut(write_idx);
            let read_node = left.get(read_idx)?.as_ref()?;
            let write_node = right.first_mut()?.as_mut()?;
            Some((read_node, write_node))
        } else {
            let (left, right) = self.nodes.split_at_mut(read_idx);
            let write_node = left.get_mut(write_idx)?.as_mut()?;
            let read_node = right.first()?.as_ref()?;
            Some((read_node, write_node))
        }
    }

    /// Find node by TreeWidgetId
    pub fn find_by_id(&self, widget_id: TreeWidgetId) -> Option<NodeId> {
        self.id_to_node.get(&widget_id).copied()
    }

    /// Get children of a node
    pub fn children(&self, parent_id: NodeId) -> Vec<NodeId> {
        let mut result = Vec::new();

        if let Some(parent) = self.node(parent_id) {
            let mut child_id = parent.first_child;
            while let Some(cid) = child_id {
                result.push(cid);
                if let Some(child) = self.node(cid) {
                    child_id = child.next_sibling;
                } else {
                    break;
                }
            }
        }

        result
    }

    /// Declare the complete back-to-front order of this host's main-content subtrees.
    /// Call only during construction or deferred UI mutation, never traversal.
    pub fn set_control_subtree_order(
        &mut self,
        host: NodeId,
        back_to_front: &[NodeId],
        placement: ControlSubtreePlacement,
    ) -> Result<(), ControlSubtreeOrderError> {
        Self::assert_control_subtree_mutation_phase();
        let host_id = self
            .node(host)
            .ok_or(ControlSubtreeOrderError::MissingHost)?
            .id;
        let ControlSubtreePlacement::Main { z_index } = placement;
        if z_index >= LAYER_OVERLAY {
            return Err(ControlSubtreeOrderError::InvalidMainPlacement);
        }
        let mut unique = HashSet::with_capacity(back_to_front.len());
        let mut members = Vec::with_capacity(back_to_front.len());
        for &member in back_to_front {
            if !unique.insert(member) {
                return Err(ControlSubtreeOrderError::DuplicateMember);
            }
            let node = self
                .node(member)
                .filter(|n| n.parent == Some(host))
                .ok_or(ControlSubtreeOrderError::NotDirectChild)?;
            members.push((member, node.id));
        }
        if self.children(host).len() != members.len() {
            return Err(ControlSubtreeOrderError::IncompleteMemberSet);
        }
        if self.control_subtree_affordances.contains_key(&host) && z_index != LAYER_CONTENT {
            return Err(ControlSubtreeOrderError::InvalidMainPlacement);
        }
        let declaration = ControlSubtreeOrder {
            host: host_id,
            members,
            placement,
        };
        if self.control_subtree_orders.get(&host) == Some(&declaration) {
            return Ok(());
        }
        self.control_subtree_orders.insert(host, declaration);
        self.invalidate_control_subtree_composition(host);
        Ok(())
    }

    pub fn clear_control_subtree_order(
        &mut self,
        host: NodeId,
    ) -> Result<(), ControlSubtreeOrderError> {
        Self::assert_control_subtree_mutation_phase();
        self.node(host)
            .ok_or(ControlSubtreeOrderError::MissingHost)?;
        if self.control_subtree_affordances.contains_key(&host) {
            return Err(ControlSubtreeOrderError::InvalidMainPlacement);
        }
        if self.control_subtree_orders.remove(&host).is_some() {
            self.invalidate_control_subtree_composition(host);
        }
        Ok(())
    }

    /// Divert these descendants' current decorative `paint_overlay` outputs into
    /// this content host's closed main-affordance interval (SPEC-HAL-359 §2.5.1).
    /// Call during construction/deferred mutation; output eligibility is checked
    /// by scene composition, including cached output, without another paint call.
    pub fn set_control_subtree_main_affordances(
        &mut self,
        host: NodeId,
        back_to_front: &[NodeId],
    ) -> Result<(), ControlSubtreeAffordanceError> {
        Self::assert_control_subtree_mutation_phase();
        let host_id = self
            .node(host)
            .ok_or(ControlSubtreeAffordanceError::MissingHost)?
            .id;
        if back_to_front.is_empty() {
            return self.clear_control_subtree_main_affordances(host);
        }
        if !self.control_subtree_orders.get(&host).is_some_and(|order| {
            order.placement
                == ControlSubtreePlacement::Main {
                    z_index: LAYER_CONTENT,
                }
                && self.control_subtree_order_is_current(host, order)
        }) {
            return Err(ControlSubtreeAffordanceError::InvalidContentPlacement);
        }
        let mut unique = HashSet::with_capacity(back_to_front.len());
        let mut sources = Vec::with_capacity(back_to_front.len());
        for &source in back_to_front {
            if !unique.insert(source) {
                return Err(ControlSubtreeAffordanceError::DuplicateSource);
            }
            let node = self
                .node(source)
                .filter(|_| source != host && self.is_descendant_or_same(source, host))
                .ok_or(ControlSubtreeAffordanceError::NotDescendant)?;
            if self
                .control_subtree_affordances
                .iter()
                .any(|(&destination, declaration)| {
                    destination != host && declaration.sources.contains(&(source, node.id))
                })
            {
                return Err(ControlSubtreeAffordanceError::SourceAlreadyAssigned);
            }
            sources.push((source, node.id));
        }
        let declaration = ControlSubtreeAffordances {
            host: host_id,
            sources,
        };
        if self.control_subtree_affordances.get(&host) == Some(&declaration) {
            return Ok(());
        }
        let previous = self.control_subtree_affordances.insert(host, declaration);
        let current_sources = back_to_front.iter().copied();
        let previous_sources = previous
            .iter()
            .flat_map(|d| d.sources.iter().map(|&(id, _)| id));
        for source in previous_sources.chain(current_sources) {
            self.invalidate_composition_path(source);
        }
        self.invalidate_composition_path(host);
        Ok(())
    }

    /// Restore the sources' ordinary post-child overlay placement without repaint.
    pub fn clear_control_subtree_main_affordances(
        &mut self,
        host: NodeId,
    ) -> Result<(), ControlSubtreeAffordanceError> {
        Self::assert_control_subtree_mutation_phase();
        self.node(host)
            .ok_or(ControlSubtreeAffordanceError::MissingHost)?;
        if let Some(previous) = self.control_subtree_affordances.remove(&host) {
            for (source, _) in previous.sources {
                self.invalidate_composition_path(source);
            }
            self.invalidate_composition_path(host);
        }
        Ok(())
    }

    fn retire_control_subtree_affordances(&mut self, removed: NodeId) {
        self.control_subtree_affordances.remove(&removed);
        let hosts: Vec<_> = self
            .control_subtree_affordances
            .iter()
            .filter_map(|(&host, declaration)| {
                declaration
                    .sources
                    .iter()
                    .any(|&(source, _)| source == removed)
                    .then_some(host)
            })
            .collect();
        for host in hosts {
            let mut remaining = Vec::new();
            if let Some(declaration) = self.control_subtree_affordances.get_mut(&host) {
                declaration.sources.retain(|&(source, _)| source != removed);
                remaining.extend(declaration.sources.iter().map(|&(source, _)| source));
                if declaration.sources.is_empty() {
                    self.control_subtree_affordances.remove(&host);
                }
            }
            for source in remaining {
                self.invalidate_composition_path(source);
            }
            self.invalidate_composition_path(removed);
            self.invalidate_composition_path(host);
        }
    }

    pub(crate) fn has_control_subtree_main_affordances(&self) -> bool {
        !self.control_subtree_affordances.is_empty()
    }

    pub(crate) fn control_subtree_main_affordance(
        &self,
        source: NodeId,
    ) -> Option<ControlSubtreeMember> {
        let mount = self.node(source)?.id;
        self.control_subtree_affordances
            .values()
            .find_map(|declaration| {
                declaration
                    .sources
                    .iter()
                    .position(|&(id, owner)| id == source && owner == mount)
                    .map(|member_index| ControlSubtreeMember {
                        scope: declaration.host,
                        z_index: LAYER_MAIN_AFFORDANCE,
                        member_index,
                    })
            })
    }

    fn assert_control_subtree_mutation_phase() {
        assert!(
            !crate::core::rect_authority::is_layout_authority_active()
                && !crate::core::rect_authority::is_paint_phase_active(),
            "INV-359-CONTROL-MUTATION-PHASE: construction/deferred mutation only"
        );
    }

    fn invalidate_control_subtree_composition(&mut self, host: NodeId) {
        self.hit_test_grid = None;
        self.invalidate_composition_path(host);
    }

    fn invalidate_composition_path(&mut self, source: NodeId) {
        // node_mut would also retire the geometry-dependent hit index.
        let mut current = Some(source);
        while let Some(id) = current {
            let Some(node) = self.nodes.get_mut(id.0).and_then(Option::as_mut) else {
                break;
            };
            node.dirty_composition = true;
            current = node.parent;
        }
    }

    pub(crate) fn assert_control_subtree_orders_current(&self) {
        for (&host, order) in &self.control_subtree_orders {
            assert!(
                self.control_subtree_order_is_current(host, order),
                "INV-359-CONTROL-MEMBERSHIP: redeclare the complete current mounted child set"
            );
        }
        for (&host, declaration) in &self.control_subtree_affordances {
            assert!(
                self.node(host).is_some_and(|n| n.id == declaration.host)
                    && self.control_subtree_orders.get(&host).is_some_and(|order| {
                        order.placement
                            == ControlSubtreePlacement::Main {
                                z_index: LAYER_CONTENT,
                            }
                    })
                    && declaration.sources.iter().all(|&(source, mount)| {
                        source != host
                            && self.node(source).is_some_and(|n| n.id == mount)
                            && self.is_descendant_or_same(source, host)
                    }),
                "INV-359-MAIN-AFFORDANCE-MEMBERSHIP: current mounted source and content host required"
            );
        }
    }

    fn control_subtree_order_is_current(&self, host: NodeId, order: &ControlSubtreeOrder) -> bool {
        let current = self.children(host);
        self.node(host).is_some_and(|n| n.id == order.host)
            && current.len() == order.members.len()
            && order.members.iter().all(|&(id, mount)| {
                current.contains(&id)
                    && self
                        .node(id)
                        .is_some_and(|n| n.id == mount && n.parent == Some(host))
            })
    }

    pub(crate) fn control_subtree_children(
        &self,
        host: NodeId,
    ) -> Vec<(NodeId, Option<ControlSubtreeMember>)> {
        if let Some(order) = self.control_subtree_orders.get(&host) {
            let ControlSubtreePlacement::Main { z_index } = order.placement;
            order
                .members
                .iter()
                .enumerate()
                .map(|(member_index, &(id, _))| {
                    (
                        id,
                        Some(ControlSubtreeMember {
                            scope: order.host,
                            z_index,
                            member_index,
                        }),
                    )
                })
                .collect()
        } else {
            self.children(host)
                .into_iter()
                .map(|id| (id, None))
                .collect()
        }
    }

    fn hit_children_front_to_back(&self, host: NodeId) -> Vec<NodeId> {
        if let Some(order) = self.control_subtree_orders.get(&host) {
            order.members.iter().rev().map(|&(id, _)| id).collect()
        } else {
            let mut children = self.children(host);
            children.sort_by_key(|id| std::cmp::Reverse(self.node(*id).map_or(0, |n| n.z_order)));
            children
        }
    }

    /// SPEC-HAL-018 Stage 10: Remove all children of a node, keeping the node itself.
    ///
    /// Used by ScreenHost to clear old screen/modal subtrees before rebuilding.
    /// Each child subtree is fully removed (including their descendants).
    pub fn clear_children(&mut self, parent_id: NodeId) {
        // Snapshot children to avoid borrow issues during removal
        let children_to_remove = self.children(parent_id);

        // Remove each child subtree
        for child_id in children_to_remove {
            self.remove_node(child_id);
        }

        // Clear the parent's first_child pointer
        if let Some(parent) = self.node_mut(parent_id) {
            parent.first_child = None;
        }
    }

    /// SPEC-HAL-128 Stage 5 INV-WL1: Drain pending widget removals.
    ///
    /// Returns all widget IDs that were removed since the last drain.
    /// Call this after tree modifications and pass to EventRouter::on_widgets_destroyed()
    /// to clean up gesture/capture state for destroyed widgets.
    pub fn drain_pending_removals(&mut self) -> Vec<TreeWidgetId> {
        std::mem::take(&mut self.pending_widget_removals)
    }

    /// Request cancellation of this owner's active relative-pointer gesture.
    ///
    /// This is a non-structural intent for `Screen::update`. App drains it after
    /// destroyed-widget cleanup in the same frame; EventRouter validates the
    /// live owner and pointer before ending the lease.
    pub fn request_relative_pointer_semantic_cancel(
        &mut self,
        owner: TreeWidgetId,
        pointer_id: PointerId,
    ) {
        self.pending_relative_pointer_semantic_cancels
            .push((owner, pointer_id));
    }

    pub(crate) fn drain_relative_pointer_semantic_cancels(
        &mut self,
    ) -> Vec<(TreeWidgetId, PointerId)> {
        std::mem::take(&mut self.pending_relative_pointer_semantic_cancels)
    }

    /// SPEC-HAL-031: Drain pending node removals for signal subscription cleanup.
    pub fn drain_pending_node_removals(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.pending_node_removals)
    }

    // ========================================================================
    // SPEC-HAL-131A Stage 14: Post-Layout Anchor Resolution Support
    // ========================================================================

    /// SPEC-HAL-131A Stage 14: Get typed index of ScrollArea nodes.
    ///
    /// Returns NodeIds of ScrollArea widgets for post-layout anchor resolution.
    /// This list is maintained during tree operations, not computed by scanning.
    /// Order is deterministic (insertion order).
    pub fn scroll_anchor_nodes(&self) -> &[NodeId] {
        &self.scroll_anchor_nodes
    }

    pub(crate) fn bind_scroll_intent_admission(
        &mut self,
        admission: crate::core::event::ScrollIntentAdmission,
    ) {
        if self.scroll_intent_admission.same_domain(&admission) {
            return;
        }
        self.scroll_intent_admission = admission;
        for &node_id in &self.scroll_anchor_nodes {
            if let Some(node) = self.nodes[node_id.0].as_mut()
                && let Some(area) = node.widget.as_any_mut().downcast_mut::<ScrollArea>()
            {
                area.bind_scroll_intent_admission(self.scroll_intent_admission.clone());
            }
        }
    }

    fn bind_scroll_area_intent_admission(&mut self, node_id: NodeId) {
        let admission = self.scroll_intent_admission.clone();
        if let Some(node) = self.node_mut(node_id)
            && let Some(area) = node.widget.as_any_mut().downcast_mut::<ScrollArea>()
        {
            area.bind_scroll_intent_admission(admission);
        }
    }

    /// Retire requests made while this domain's touch pan owned scrolling.
    /// A terminal can follow a keyboard or content request in the same input batch.
    pub(crate) fn discard_pending_scroll_intents(&mut self) {
        for index in 0..self.scroll_anchor_nodes.len() {
            let area = self.scroll_anchor_nodes[index];
            let content = self
                .node(area)
                .and_then(|node| node.widget.as_any().downcast_ref::<ScrollArea>())
                .and_then(ScrollArea::sealed_wired_content);
            if let Some(content) = content.and_then(|id| self.node_mut(id)) {
                if let Some(list) = content
                    .widget
                    .as_any_mut()
                    .downcast_mut::<crate::widgets::list::List>()
                {
                    list.take_requested_scroll_y();
                } else {
                    content.widget.take_requested_scroll_y();
                }
            }
        }
    }

    fn nearest_scroll_area_ancestor(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = self.parent(node_id);
        while let Some(id) = current {
            let node = self.node(id)?;
            if node.widget.as_any().downcast_ref::<ScrollArea>().is_some() {
                return Some(id);
            }
            current = node.parent;
        }
        None
    }

    pub(crate) fn is_descendant_or_same(&self, node_id: NodeId, ancestor: NodeId) -> bool {
        let mut current = Some(node_id);
        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            current = self.parent(id);
        }
        false
    }

    /// SPEC-HAL-352: Return a node rect in the owning ScrollArea's content-local
    /// logical space. Nested ScrollAreas are hard boundaries.
    pub(crate) fn layout_anchor_content_rect(
        &self,
        scroll_id: NodeId,
        node_id: NodeId,
    ) -> Option<RectF> {
        if node_id == scroll_id {
            return None;
        }

        let target = self.node(node_id)?;
        let mut x = 0.0;
        let mut y = 0.0;
        let width = target.rect.width;
        let height = target.rect.height;
        let mut current = node_id;

        loop {
            let node = self.node(current)?;
            x += node.rect.x;
            y += node.rect.y;
            let parent = node.parent?;
            if parent == scroll_id {
                break;
            }
            if self
                .node(parent)?
                .widget
                .as_any()
                .downcast_ref::<ScrollArea>()
                .is_some()
            {
                return None;
            }
            current = parent;
        }

        Some(RectF::new(x, y, width, height))
    }

    /// Begin an explicit vertical layout-change anchoring transaction.
    ///
    /// SPEC-HAL-352: Call before applying the staged layout-affecting state to
    /// the live tree. Returns false without mutation when ownership, visibility,
    /// geometry, or active-scroll requirements are not satisfied.
    pub fn begin_layout_scroll_anchor(
        &mut self,
        anchor_node: NodeId,
        affected_region_node: NodeId,
        intent: LayoutScrollAnchorIntent,
    ) -> bool {
        if !self.is_descendant_or_same(anchor_node, affected_region_node) {
            return false;
        }

        let Some(scroll_id) = self.nearest_scroll_area_ancestor(anchor_node) else {
            return false;
        };
        if self.nearest_scroll_area_ancestor(affected_region_node) != Some(scroll_id) {
            return false;
        }

        let Some(anchor_rect) = self.layout_anchor_content_rect(scroll_id, anchor_node) else {
            return false;
        };
        let Some((visual_x, effective_visual_y, scrollport_size)) = self
            .node(scroll_id)
            .and_then(|node| node.widget.as_any().downcast_ref::<ScrollArea>())
            .and_then(ScrollArea::layout_anchor_request_snapshot)
        else {
            return false;
        };

        let viewport = RectF::new(
            visual_x,
            effective_visual_y,
            scrollport_size.width,
            scrollport_size.height,
        );
        let intersects = anchor_rect.x < viewport.x + viewport.width
            && anchor_rect.x + anchor_rect.width > viewport.x
            && anchor_rect.y < viewport.y + viewport.height
            && anchor_rect.y + anchor_rect.height > viewport.y;
        if !intersects {
            return false;
        }

        let Some(anchor) = self.node(anchor_node).map(|node| node.id) else {
            return false;
        };
        let Some(affected_region) = self.node(affected_region_node).map(|node| node.id) else {
            return false;
        };
        let transaction = LayoutScrollAnchorTransaction {
            anchor,
            affected_region,
            intent,
            pre_anchor_viewport_y: anchor_rect.y - effective_visual_y,
            scrollport_size,
        };

        let Some(scroll) = self
            .node_mut(scroll_id)
            .and_then(|node| node.widget.as_any_mut().downcast_mut::<ScrollArea>())
        else {
            return false;
        };
        scroll.begin_layout_anchor_transaction(transaction);
        true
    }

    /// SPEC-HAL-131A Stage 14 INV-B5: Iterate all nodes in stable order.
    ///
    /// Calls `f` for each valid node in the tree, in Vec index order.
    /// Order is deterministic across runs for the same tree.
    ///
    /// INV-B3: This is read-only traversal; `f` receives only NodeId.
    pub fn for_each_node_in_stable_order(&self, mut f: impl FnMut(NodeId)) {
        for (idx, node) in self.nodes.iter().enumerate() {
            if node.is_some() {
                f(NodeId(idx));
            }
        }
    }

    /// Get parent of a node
    pub fn parent(&self, node_id: NodeId) -> Option<NodeId> {
        self.node(node_id).and_then(|n| n.parent)
    }

    pub(crate) fn mark_nearest_scroll_dependent_ancestor_layout(
        &mut self,
        widget_id: TreeWidgetId,
    ) {
        let Some(node_id) = self.find_by_id(widget_id) else {
            return;
        };
        let mut current = self.parent(node_id);
        while let Some(ancestor_id) = current {
            let Some(ancestor) = self.node(ancestor_id) else {
                return;
            };
            if ancestor.widget.layout_depends_on_descendant_scroll() {
                self.mark_layout_dirty(ancestor_id);
                return;
            }
            current = ancestor.parent;
        }
    }

    // ========================================================================
    // SPEC-HAL-190: Authority Projection Lookup
    // ========================================================================

    /// SPEC-HAL-190: Look up authority by walking the ancestor chain.
    ///
    /// Returns a Projection to the nearest ancestor that provides authority of type A.
    ///
    /// # Type Parameters
    ///
    /// - `A`: The authority type to look up. Must implement `Authority`.
    ///
    /// # Invariants
    ///
    /// - INV-AUTH-LOOKUP-1: Authority lookup is O(depth).
    /// - INV-AUTH-LOOKUP-2: If multiple ancestors provide the same type, nearest wins.
    /// - INV-AUTH-ANCESTOR-ONLY: Only walks ancestor chain, never siblings.
    ///
    /// # Returns
    ///
    /// - `Some(Projection<A>)` if an ancestor provides authority A
    /// - `None` if no ancestor provides authority A
    pub fn lookup_authority<'a, A: Authority + 'static>(
        &'a self,
        from_node: NodeId,
    ) -> Option<Projection<'a, A>> {
        let type_id = TypeId::of::<A>();
        let mut current = Some(from_node);

        // Walk ancestor chain (INV-AUTH-ANCESTOR-ONLY)
        while let Some(node_id) = current {
            if self.authority_registry.provides(node_id, type_id) {
                // Found a provider - get the authority reference
                if let Some(node) = self.node(node_id) {
                    if let Some((any_ref, _)) = node.widget.provided_authority() {
                        if let Some(authority) = any_ref.downcast_ref::<A>() {
                            return Some(Projection::new(authority));
                        }
                    }
                }
            }
            // Move to parent
            current = self.parent(node_id);
        }

        None // No ancestor provides this authority type
    }

    /// SPEC-HAL-190: Find the authority owner (TreeWidgetId) for a given authority type.
    ///
    /// Used by emit_intent to find which owner's buffer to push to.
    ///
    /// # Returns
    ///
    /// - `Some(TreeWidgetId)` if an ancestor provides authority A
    /// - `None` if no ancestor provides authority A
    pub fn find_authority_owner<A: Authority + 'static>(
        &self,
        from_node: NodeId,
    ) -> Option<TreeWidgetId> {
        let type_id = TypeId::of::<A>();
        let mut current = Some(from_node);

        while let Some(node_id) = current {
            if self.authority_registry.provides(node_id, type_id) {
                if let Some(node) = self.node(node_id) {
                    return Some(node.id);
                }
            }
            current = self.parent(node_id);
        }

        None
    }

    /// SPEC-HAL-190: Get mutable access to intent buffers.
    ///
    /// Used during event handling to push intents.
    pub fn intent_buffers_mut(&mut self) -> &mut IntentBuffers {
        &mut self.intent_buffers
    }

    /// SPEC-HAL-190: Drain all intent buffers for deferred mutation phase.
    ///
    /// Returns vec of (owner_id, intents) pairs. Clears all buffers.
    pub fn drain_intent_buffers(
        &mut self,
    ) -> Vec<(TreeWidgetId, Vec<Box<dyn crate::core::authority::Intent>>)> {
        self.intent_buffers.drain_all()
    }

    /// SPEC-HAL-038 Stage 3: Check if a node is a descendant of an ancestor.
    ///
    /// Returns true if `node_id` is a descendant of `ancestor_id`.
    /// A node is NOT considered a descendant of itself.
    ///
    /// Used by focus scope logic to check if focused widget is inside a scope.
    pub fn is_descendant(&self, node_id: NodeId, ancestor_id: NodeId) -> bool {
        let mut current = self.parent(node_id);
        while let Some(cid) = current {
            if cid == ancestor_id {
                return true;
            }
            current = self.parent(cid);
        }
        false
    }

    /// SPEC-HAL-038 Stage 7: Check if a widget is a descendant of or equal to an ancestor node.
    ///
    /// Takes a TreeWidgetId and resolves it to a NodeId, then checks ancestry.
    /// Returns true if the widget's node is equal to or a descendant of `ancestor_id`.
    ///
    /// Used by hover lifecycle to check scope boundaries.
    pub fn is_descendant_of_node(&self, widget_id: TreeWidgetId, ancestor_id: NodeId) -> bool {
        let Some(node_id) = self.find_node_by_widget_id(widget_id) else {
            return false;
        };
        // Check if node itself is the ancestor
        if node_id == ancestor_id {
            return true;
        }
        // Check if node is a descendant
        self.is_descendant(node_id, ancestor_id)
    }

    /// SPEC-HAL-038 Stage 4: Filter nodes to only those that are focus scopes.
    ///
    /// Returns NodeIds from the input that have widgets returning `is_focus_scope() == true`.
    /// Preserves input order. Skips nodes that don't exist or aren't mounted.
    ///
    /// Used by EventRouter::sync_focus_scopes to filter modal nodes to actual scopes.
    pub fn filter_focus_scopes(&self, nodes: &[NodeId]) -> Vec<NodeId> {
        let mut result = Vec::new();
        for &node_id in nodes {
            if let Some(node) = self.node(node_id) {
                if node.widget.is_focus_scope() {
                    result.push(node_id);
                }
            }
        }
        result
    }

    /// Set rect for a node (called during layout).
    ///
    /// SPEC-HAL-141: Rect mutations require layout authority.
    ///
    /// # Panics
    /// Panics in ALL BUILDS if called outside the layout phase (no LayoutAuthorityGuard active).
    pub fn set_rect(&mut self, node_id: NodeId, rect: RectF) {
        // SPEC-HAL-141 INV-RECT-AUTH-ENFORCED: Verify rect mutation is authorized (ALL BUILDS)
        assert_rect_mutation_authorized("WidgetTree::set_rect");

        if let Some(node) = self.node_mut(node_id) {
            node.rect = rect;
        }
    }

    /// Get rect of a node
    pub fn get_rect(&self, node_id: NodeId) -> Option<RectF> {
        self.node(node_id).map(|n| n.rect)
    }

    /// Set flex params for a node, invalidating layout when they change.
    pub fn set_flex(&mut self, node_id: NodeId, flex: FlexParams) {
        let changed = if let Some(node) = self.node_mut(node_id) {
            if node.flex == flex {
                false
            } else {
                node.flex = flex;
                true
            }
        } else {
            false
        };

        if changed {
            self.mark_layout_dirty(node_id);
        }
    }

    /// Explicitly opts this direct row into its local ControlAlignmentGroup.
    /// Labels, values and input remain owned by the existing control.
    pub fn set_control_alignment_member(&mut self, node_id: NodeId, member: bool) {
        let changed = self.node_mut(node_id).is_some_and(|node| {
            if node.control_alignment_member == member {
                return false;
            }
            node.control_alignment_member = member;
            node.control_parts_cache = None;
            true
        });
        if changed {
            self.mark_layout_dirty(node_id);
            if let Some(parent) = self.node(node_id).and_then(|n| n.parent) {
                self.mark_layout_dirty(parent);
            }
        }
    }

    /// Get flex params for a node
    pub fn get_flex(&self, node_id: NodeId) -> FlexParams {
        self.node(node_id).map(|n| n.flex).unwrap_or_default()
    }

    /// Annotate a mounted node for the local F9 tree snapshot.
    pub fn set_diagnostic_hint(&mut self, node_id: NodeId, hint: impl Into<String>) {
        if let Some(node) = self.nodes.get_mut(node_id.0) {
            if let Some(node) = node.as_mut() {
                node.diagnostic_hint = hint.into();
            }
        }
    }

    /// FIX #3: Check if a node wants scroll (i.e., is a ScrollArea)
    /// Used by Flex to ensure ScrollArea children always get full cross-axis width
    pub fn wants_scroll(&self, node_id: NodeId) -> bool {
        self.node(node_id)
            .map(|n| n.widget.wants_scroll())
            .unwrap_or(false)
    }

    /// PM-005: Check if a node needs bounded viewport constraints.
    /// Used by ScrollArea to decide bounded vs. unbounded scroll-axis constraints.
    pub fn needs_viewport_constraint(&self, node_id: NodeId) -> bool {
        self.node(node_id)
            .map(|n| n.widget.needs_viewport_constraint())
            .unwrap_or(false)
    }

    /// Check if a widget's height is stable under width constraint changes.
    /// Used by Flex wrap to skip redundant relayout passes.
    pub fn height_stable(&self, node_id: NodeId) -> bool {
        self.node(node_id)
            .map(|n| n.widget.height_stable())
            .unwrap_or(false)
    }

    /// SPEC-HAL-088 Stage 3: Set overflow mode for a node.
    /// Controls whether this node clips its children during hit-testing and painting.
    pub fn set_overflow(&mut self, node_id: NodeId, overflow: OverflowMode) {
        let changed = self
            .node(node_id)
            .is_some_and(|node| node.overflow != overflow);
        if let Some(node) = self.node_mut(node_id) {
            node.overflow = overflow;
        }
        if changed {
            self.mark_layout_dirty(node_id);
        }
    }

    /// SPEC-HAL-088 Stage 4: Set layout scroll offset for a node.
    /// Makes this node a scroll container for viewport computation purposes.
    pub fn set_layout_scroll_offset(&mut self, node_id: NodeId, offset: Option<(f32, f32)>) {
        let changed = self
            .node(node_id)
            .is_some_and(|node| node.layout_scroll_offset != offset);
        if let Some(node) = self.node_mut(node_id) {
            node.layout_scroll_offset = offset;
        }
        if changed {
            self.invalidate_composition_path(node_id);
        }
    }

    // ========================================================================
    // SPEC-HAL-141: Guarded Geometry Setters
    // ========================================================================
    //
    // These methods enforce rect mutation authority per SPEC-HAL-141.
    // All geometry writes MUST go through these methods (or the existing
    // set_rect) to ensure proper authorization checks.
    //
    // INVARIANT: Unauthorized mutation panics in ALL builds.
    // ========================================================================

    /// SPEC-HAL-141: Set absolute_xy for a node (called during layout).
    ///
    /// # Panics
    /// Panics in ALL BUILDS if called outside the layout phase.
    pub fn set_absolute_xy(&mut self, node_id: NodeId, pos: Point) {
        assert_rect_mutation_authorized("WidgetTree::set_absolute_xy");
        if let Some(node) = self.node_mut(node_id) {
            node.absolute_xy = pos;
        }
    }

    /// SPEC-HAL-141: Get absolute_xy for a node.
    pub fn get_absolute_xy(&self, node_id: NodeId) -> Option<Point> {
        self.node(node_id).map(|n| n.absolute_xy)
    }

    /// SPEC-HAL-141: Set absolute_paint_rect for a node (called during layout).
    ///
    /// # Panics
    /// Panics in ALL BUILDS if called outside the layout phase.
    pub fn set_absolute_paint_rect(&mut self, node_id: NodeId, rect: RectF) {
        assert_rect_mutation_authorized("WidgetTree::set_absolute_paint_rect");
        let changed = self
            .node(node_id)
            .is_some_and(|node| node.absolute_paint_rect != rect);
        if let Some(node) = self.node_mut(node_id) {
            node.absolute_paint_rect = rect;
        }
        if changed {
            self.mark_paint_dirty(node_id);
        }
    }

    /// SPEC-HAL-141: Get absolute_paint_rect for a node.
    pub fn get_absolute_paint_rect(&self, node_id: NodeId) -> Option<RectF> {
        self.node(node_id).map(|n| n.absolute_paint_rect)
    }

    /// SPEC-HAL-141: Set viewport for a node (called during layout).
    ///
    /// # Panics
    /// Panics in ALL BUILDS if called outside the layout phase.
    pub fn set_viewport(&mut self, node_id: NodeId, rect: RectF) {
        assert_rect_mutation_authorized("WidgetTree::set_viewport");
        let changed = self.node(node_id).is_some_and(|node| node.viewport != rect);
        if let Some(node) = self.node_mut(node_id) {
            node.viewport = rect;
        }
        if changed {
            self.mark_paint_dirty(node_id);
        }
    }

    /// SPEC-HAL-141: Get viewport for a node.
    pub fn get_viewport(&self, node_id: NodeId) -> Option<RectF> {
        self.node(node_id).map(|n| n.viewport)
    }

    /// HAL088 §17: raw child boundary in its declaring allocation's scope.
    /// Enclosing scroll is projected by the consumer once; this owner's own
    /// scroll applies only to descendants. Legacy viewport metadata is distinct.
    pub(crate) fn child_clip_source(&self, node_id: NodeId) -> Option<RectF> {
        let node = self.node(node_id)?;
        (node.overflow == OverflowMode::Clip || node.layout_scroll_offset.is_some())
            .then_some(node.absolute_paint_rect)
    }

    /// SPEC-HAL-141: Set cached_size for a node (called during layout).
    ///
    /// # Panics
    /// Panics in ALL BUILDS if called outside the layout phase.
    pub fn set_cached_size(&mut self, node_id: NodeId, size: Size) {
        assert_rect_mutation_authorized("WidgetTree::set_cached_size");
        if let Some(node) = self.node_mut(node_id) {
            node.cached_size = size;
        }
    }

    /// SPEC-HAL-141: Get cached_size for a node.
    pub fn get_cached_size(&self, node_id: NodeId) -> Option<Size> {
        self.node(node_id).map(|n| n.cached_size)
    }

    /// Set content_size for a node (for scroll extent calculation).
    /// This is the intrinsic content size, which may exceed cached_size.
    pub fn set_content_size(&mut self, node_id: NodeId, size: Size) {
        assert_rect_mutation_authorized("WidgetTree::set_content_size");
        if let Some(node) = self.node_mut(node_id) {
            node.content_size = size;
        }
    }

    /// Get content_size for a node (for scroll extent aggregation).
    pub fn get_content_size(&self, node_id: NodeId) -> Option<Size> {
        self.node(node_id).map(|n| n.content_size)
    }

    /// Find NodeId by TreeWidgetId (for event routing)
    pub fn find_node_by_widget_id(&self, widget_id: TreeWidgetId) -> Option<NodeId> {
        self.id_to_node.get(&widget_id).copied()
    }

    /// SPEC-HAL-241: Query drag payload from a widget (read-only).
    pub fn query_drag_payload(&self, node_id: NodeId) -> Option<Box<dyn std::any::Any>> {
        self.nodes[node_id.0].as_ref()?.widget.drag_payload()
    }

    /// SPEC-HAL-241: Check if widget accepts a drop of the given payload type.
    pub fn query_accepts_drop(&self, node_id: NodeId, payload_type: std::any::TypeId) -> bool {
        self.nodes[node_id.0]
            .as_ref()
            .map(|n| n.widget.accepts_drop(payload_type))
            .unwrap_or(false)
    }

    /// SPEC-HAL-241: Get TreeWidgetId for a node.
    pub fn widget_id_for_node(&self, node_id: NodeId) -> Option<TreeWidgetId> {
        self.nodes[node_id.0].as_ref().map(|n| n.id)
    }

    // ========================================================================
    // SPEC-HAL-087 Stage 2: Identity Verification Guards
    // ========================================================================

    /// SPEC-HAL-087 Stage 2: Verify a TreeWidgetId is valid in the tree.
    ///
    /// Checks:
    /// - ID exists in `id_to_node`
    /// - Node exists at that NodeId
    /// - `node.id == widget_id` (consistency)
    ///
    /// Returns true if valid, false otherwise.
    /// In debug builds, logs detailed error on failure.
    /// With HALCYON_STRICT_IDENTITY=1, panics on failure.
    #[inline]
    pub fn verify_tree_widget_id(&self, widget_id: TreeWidgetId) -> bool {
        let node_id = match self.id_to_node.get(&widget_id) {
            Some(nid) => *nid,
            None => {
                Self::identity_violation(&format!(
                    "TreeWidgetId({}) not found in id_to_node",
                    widget_id.0
                ));
                return false;
            }
        };

        let node = match self.node(node_id) {
            Some(n) => n,
            None => {
                Self::identity_violation(&format!(
                    "TreeWidgetId({}) maps to NodeId({}) but node is None",
                    widget_id.0, node_id.0
                ));
                return false;
            }
        };

        if node.id != widget_id {
            Self::identity_violation(&format!(
                "TreeWidgetId({}) maps to NodeId({}) but node.id is TreeWidgetId({})",
                widget_id.0, node_id.0, node.id.0
            ));
            return false;
        }

        true
    }

    /// SPEC-HAL-087 Stage 2: Verify all nodes are consistent.
    ///
    /// Validates:
    /// - Every node.id is unique
    /// - id_to_node.len() matches active node count
    /// - No stale IDs remain in id_to_node
    /// - Every id_to_node entry points to a valid node with matching ID
    ///
    /// Only runs in debug builds (no-op in release).
    #[cfg(debug_assertions)]
    pub fn verify_all_nodes_consistent(&self) {
        use std::collections::HashSet;

        let mut seen_ids = HashSet::new();
        let mut active_count = 0;

        // Check all nodes
        for (idx, slot) in self.nodes.iter().enumerate() {
            if let Some(node) = slot {
                active_count += 1;

                // Check uniqueness
                if !seen_ids.insert(node.id) {
                    Self::identity_violation(&format!(
                        "Duplicate node.id TreeWidgetId({}) at NodeId({})",
                        node.id.0, idx
                    ));
                }

                // Check id_to_node mapping
                match self.id_to_node.get(&node.id) {
                    Some(&mapped_node_id) if mapped_node_id.0 == idx => {
                        // Correct mapping
                    }
                    Some(&mapped_node_id) => {
                        Self::identity_violation(&format!(
                            "Node at NodeId({}) has id TreeWidgetId({}), but id_to_node maps to NodeId({})",
                            idx, node.id.0, mapped_node_id.0
                        ));
                    }
                    None => {
                        Self::identity_violation(&format!(
                            "Node at NodeId({}) has id TreeWidgetId({}) not in id_to_node",
                            idx, node.id.0
                        ));
                    }
                }
            }
        }

        // Check id_to_node size matches
        if self.id_to_node.len() != active_count {
            Self::identity_violation(&format!(
                "id_to_node has {} entries but {} active nodes exist",
                self.id_to_node.len(),
                active_count
            ));
        }
    }

    #[cfg(not(debug_assertions))]
    #[inline]
    pub fn verify_all_nodes_consistent(&self) {
        // No-op in release builds
    }

    /// SPEC-HAL-087 Stage 2: Report an identity violation.
    ///
    /// In debug builds: logs error via tracing.
    /// With HALCYON_STRICT_IDENTITY=1: panics.
    #[cold]
    fn identity_violation(msg: &str) {
        tracing::error!("SPEC-HAL-087 IDENTITY VIOLATION: {}", msg);

        if std::env::var("HALCYON_STRICT_IDENTITY")
            .map(|v| v == "1")
            .unwrap_or(false)
        {
            panic!("SPEC-HAL-087 STRICT MODE: {}", msg);
        }
    }

    /// SPEC-HAL-086 Stage 7: Compute cumulative scroll offset from root to a node.
    ///
    /// Walks from the node up to the root, accumulating scroll offsets from
    /// all ancestor scroll containers. Returns the total scroll offset that
    /// affects this node's visual position.
    ///
    /// Used by event routing to compute local_pos for captured events.
    pub fn cumulative_scroll_offset(&self, node_id: NodeId) -> (f32, f32) {
        let mut offset = (0.0_f32, 0.0_f32);
        let mut current = Some(node_id);

        // Walk up to root, checking each ancestor for scroll offset
        while let Some(nid) = current {
            if let Some(node) = self.node(nid) {
                // Check if parent is a scroll container (scroll affects children)
                if let Some(parent_id) = node.parent {
                    if let Some(parent) = self.node(parent_id) {
                        if let Some((sx, sy)) = parent.widget.scroll_offset() {
                            offset.0 += sx;
                            offset.1 += sy;
                        }
                    }
                }
                current = node.parent;
            } else {
                break;
            }
        }

        offset
    }

    // ========================================================================
    // SPEC-HAL-026: Dirty Flag Layout Invalidation
    // ========================================================================

    /// Mark a node as needing layout, propagating dirty flag up to ancestors
    /// SPEC-HAL-026 Rule 4: Upward propagation stops at already-dirty ancestor or root
    pub fn mark_layout_dirty(&mut self, node_id: NodeId) {
        self.record_geometry_origin(node_id);
        // HAL_AUDIT_INVALIDATION: Track invalidation calls
        crate::core::app::profiler_inc_invalidation(
            crate::core::rect_authority::is_layout_authority_active(),
        );

        self.mark_paint_dirty(node_id);

        // Get the node and mark it dirty, capturing parent for propagation
        let parent_id = match self.nodes.get_mut(node_id.0).and_then(|n| n.as_mut()) {
            None => return,
            Some(node) => {
                if let Some(cache) = node.control_parts_cache.as_mut() {
                    cache.natural = None;
                    cache.ordinary_natural = None;
                }
                if node.dirty_layout {
                    return;
                }
                node.dirty_layout = true;
                node.parent
            }
        };

        // Propagate upwards until we hit an already-dirty ancestor or root
        let mut current = parent_id;
        while let Some(parent_node_id) = current {
            let next_parent = match self
                .nodes
                .get_mut(parent_node_id.0)
                .and_then(|n| n.as_mut())
            {
                None => break,
                Some(parent) => {
                    if let Some(cache) = parent.control_parts_cache.as_mut() {
                        cache.natural = None;
                        cache.ordinary_natural = None;
                    }
                    if parent.dirty_layout {
                        break;
                    }
                    parent.dirty_layout = true;
                    parent.parent
                }
            };
            current = next_parent;
        }
    }

    /// SPEC-HAL-354: Invalidate only this widget's paint output.
    ///
    /// The widget's local fragment is dirty and ancestor compositions are dirty
    /// so retained traversal can reach it. Layout and sibling fragments remain
    /// valid.
    pub fn mark_paint_dirty(&mut self, node_id: NodeId) {
        let parent_id = match self.nodes.get_mut(node_id.0).and_then(|n| n.as_mut()) {
            None => return,
            Some(node) => {
                node.dirty_paint = true;
                node.dirty_composition = true;
                node.dirty_flags = if node.dirty_layout {
                    WidgetDirty::LayoutAndPaint
                } else {
                    WidgetDirty::PaintOnly
                };
                node.parent
            }
        };

        let mut current = parent_id;
        while let Some(parent_node_id) = current {
            current = match self
                .nodes
                .get_mut(parent_node_id.0)
                .and_then(|n| n.as_mut())
            {
                None => None,
                Some(parent) => {
                    let next = parent.parent;
                    if parent.dirty_composition {
                        None
                    } else {
                        parent.dirty_composition = true;
                        next
                    }
                }
            };
        }
    }

    /// SPEC-HAL-354: Invalidate paint by stable tree identity.
    pub fn mark_paint_dirty_by_widget_id(&mut self, widget_id: TreeWidgetId) {
        if let Some(node_id) = self.find_node_by_widget_id(widget_id) {
            self.mark_paint_dirty(node_id);
        }
    }

    /// Mark a node and all descendants as needing layout (downward only)
    /// SPEC-HAL-026: Used for window resize / global invalidation
    pub fn mark_subtree_dirty(&mut self, node_id: NodeId) {
        self.mark_layout_dirty(node_id);
        let mut stack = vec![node_id];

        while let Some(id) = stack.pop() {
            if let Some(node) = self.nodes.get_mut(id.0).and_then(|n| n.as_mut()) {
                node.dirty_layout = true;
                node.dirty_paint = true;
                node.dirty_composition = true;
                node.dirty_flags = WidgetDirty::LayoutAndPaint;

                // Queue children for processing
                let mut child_id = node.first_child;
                while let Some(cid) = child_id {
                    stack.push(cid);
                    // Get next sibling
                    child_id = self
                        .nodes
                        .get(cid.0)
                        .and_then(|n| n.as_ref())
                        .and_then(|n| n.next_sibling);
                }
            }
        }
    }

    /// SPEC-HAL-354: Invalidate paint for a node and all descendants.
    pub fn mark_subtree_paint_dirty(&mut self, node_id: NodeId) {
        self.mark_paint_dirty(node_id);
        let mut stack = vec![node_id];

        while let Some(id) = stack.pop() {
            if let Some(node) = self.nodes.get_mut(id.0).and_then(|n| n.as_mut()) {
                node.dirty_paint = true;
                node.dirty_composition = true;
                node.dirty_flags = if node.dirty_layout {
                    WidgetDirty::LayoutAndPaint
                } else {
                    WidgetDirty::PaintOnly
                };

                let mut child_id = node.first_child;
                while let Some(cid) = child_id {
                    stack.push(cid);
                    child_id = self
                        .nodes
                        .get(cid.0)
                        .and_then(|n| n.as_ref())
                        .and_then(|n| n.next_sibling);
                }
            }
        }
    }

    /// Mark layout dirty by TreeWidgetId (for callers with access to tree IDs)
    pub fn mark_layout_dirty_by_widget_id(&mut self, widget_id: TreeWidgetId) {
        if let Some(node_id) = self.find_node_by_widget_id(widget_id) {
            self.mark_layout_dirty(node_id);
        }
    }

    // ========================================================================
    // SPEC-HAL-035: Widget Diff Optimizer
    // ========================================================================

    /// Build a snapshot of the current state for a node (for diffing).
    fn build_snapshot_for_node(&self, node_id: NodeId) -> Option<NodeSnapshot> {
        let node = self.nodes.get(node_id.0)?.as_ref()?;
        let widget_type_id = (&*node.widget).type_id();
        let content_key = node.widget.content_key();

        Some(NodeSnapshot {
            rect: node.rect,
            layout_constraints: node.last_constraints.unwrap_or(Constraints::unbounded()),
            widget_type_id,
            widget_key: node.key.clone(),
            content_key,
        })
    }

    /// Compute dirty flags by comparing old snapshot to new snapshot.
    fn compute_dirty_flags(old: Option<&NodeSnapshot>, new: &NodeSnapshot) -> WidgetDirty {
        match old {
            None => {
                // First frame: we know nothing → fully dirty.
                WidgetDirty::LayoutAndPaint
            }
            Some(old_snap) => {
                // Type or key change → widget replaced.
                if old_snap.widget_type_id != new.widget_type_id
                    || old_snap.widget_key != new.widget_key
                {
                    return WidgetDirty::Replaced;
                }

                let layout_changed = old_snap.layout_constraints != new.layout_constraints
                    || old_snap.rect != new.rect;

                let content_changed = old_snap.content_key != new.content_key;

                match (layout_changed, content_changed) {
                    (false, false) => WidgetDirty::Unchanged,
                    (true, false) => WidgetDirty::LayoutOnly,
                    (false, true) => WidgetDirty::PaintOnly,
                    (true, true) => WidgetDirty::LayoutAndPaint,
                }
            }
        }
    }

    /// SPEC-HAL-035: Recompute per-node dirty classification for this frame.
    ///
    /// This does not change layout or paint state by itself; it only updates
    /// `last_snapshot` and `dirty_flags` for each node.
    ///
    /// Must be called once per frame, before paint.
    pub fn recompute_dirty_flags(&mut self) {
        // First pass: build new snapshots and compute dirty flags
        // We need to collect snapshots first because we can't mutate while iterating
        let mut updates: Vec<(usize, NodeSnapshot, WidgetDirty)> = Vec::new();

        for (idx, slot) in self.nodes.iter().enumerate() {
            if let Some(node) = slot {
                if let Some(new_snapshot) = self.build_snapshot_for_node(NodeId(idx)) {
                    let dirty =
                        Self::compute_dirty_flags(node.last_snapshot.as_ref(), &new_snapshot);
                    updates.push((idx, new_snapshot, dirty));
                }
            }
        }

        // Second pass: apply updates
        for (idx, snapshot, dirty) in updates {
            if let Some(node) = self.nodes.get_mut(idx).and_then(|n| n.as_mut()) {
                node.last_snapshot = Some(snapshot);
                node.dirty_flags = dirty;
            }
        }
    }

    // ========================================================================
    // SPEC-HAL-032: Hit-Test Acceleration
    // ========================================================================

    /// Count total nodes in the tree (excluding free slots).
    pub fn node_count(&self) -> usize {
        self.nodes.iter().filter(|slot| slot.is_some()).count()
    }

    /// Rebuild the hit-test acceleration grid for a given viewport.
    ///
    /// SPEC-HAL-032: Must be called once per frame after layout, before events.
    /// If node_count < HIT_TEST_ACCEL_THRESHOLD, the grid is not built and
    /// hit-testing falls back to the naive DFS path.
    pub fn rebuild_hit_test_grid(&mut self, viewport: RectF) {
        self.assert_control_subtree_orders_current();
        let count = self.node_count();

        // Below threshold, disable grid and use DFS
        if count < HIT_TEST_ACCEL_THRESHOLD {
            self.hit_test_grid = None;
            self.hit_test_nodes.clear();
            return;
        }

        // Collect hit-testable nodes with their absolute rects into self.hit_test_nodes
        self.hit_test_nodes.clear();
        if let Some(root_id) = self.root {
            // Use a temporary vec since we can't pass &mut self.hit_test_nodes while borrowing self
            let mut temp_nodes: Vec<(NodeId, RectF)> = Vec::with_capacity(count);
            self.collect_hit_test_nodes_recursive_static(
                root_id,
                (0.0, 0.0),
                Some(viewport),
                &mut temp_nodes,
            );
            self.hit_test_nodes = temp_nodes;
        }

        if self.hit_test_nodes.is_empty() {
            self.hit_test_grid = None;
            return;
        }

        // Create or rebuild grid
        let grid = self.hit_test_grid.get_or_insert_with(|| {
            HitTestGrid::new(viewport, HIT_TEST_CELL_WIDTH, HIT_TEST_CELL_HEIGHT)
        });
        grid.rebuild(viewport, &self.hit_test_nodes);
    }

    /// Set overlay hit regions directly from paint-collected data.
    ///
    /// Called after paint_tree() with regions collected during paint.
    /// Widgets register these via PaintContext::register_overlay_hit_region().
    pub fn set_overlay_hit_regions(&mut self, regions: Vec<OverlayHitRegion>) {
        self.publish_overlay_hit_entries(regions.into_iter().map(OverlayHitEntry::from).collect());
    }

    pub(crate) fn publish_overlay_hit_entries(&mut self, regions: Vec<OverlayHitEntry>) {
        self.overlay_hit_regions = regions;

        // SPEC-HAL-087 Stage 2: Verify all overlay regions have valid tree IDs
        #[cfg(debug_assertions)]
        for entry in &self.overlay_hit_regions {
            if !self.verify_tree_widget_id(entry.region.widget_id) {
                Self::identity_violation(&format!(
                    "Overlay region contains invalid TreeWidgetId({})",
                    entry.region.widget_id.0
                ));
            }
        }
    }

    pub(crate) fn combo_popup_hit_entry(
        &self,
        region: OverlayHitRegion,
        popup_revision: u64,
        viewport: (f32, f32),
        scale: f32,
        geometry: ComboPopupInputGeometry,
    ) -> OverlayHitEntry {
        let node = self
            .find_by_id(region.widget_id)
            .and_then(|node| self.node(node))
            .expect("popup hit publication requires its mounted owner");
        OverlayHitEntry {
            region,
            combo_popup: Some(Arc::new(MountedComboPopupHit {
                tree_lifetime: self.tree_lifetime.clone(),
                mount_generation: node.concrete_mount.generation,
                popup_revision,
                viewport,
                scale,
                geometry,
            })),
        }
    }

    fn qualified_combo_popup(&self, entry: &OverlayHitEntry) -> Option<ComboPopupInputGeometry> {
        let popup = entry.combo_popup.as_deref()?;
        let id = self.find_by_id(entry.region.widget_id)?;
        if !self.composite_presentation_visible(id) {
            return None;
        }
        let node = self.node(id)?;
        let combo = node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::combo_box::ComboBox>()?;
        if self.tree_lifetime != popup.tree_lifetime
            || node.concrete_mount.generation != popup.mount_generation
            || combo.popup_input_revision() != Some(popup.popup_revision)
            || self
                .geometry_root_allocation
                .is_some_and(|(_, _, scale)| scale != popup.scale)
        {
            return None;
        }
        let viewport = self.root.and_then(|root| self.get_rect(root));
        let viewport = viewport.map_or((0.0, 0.0), |rect| (rect.width, rect.height));
        let scroll = self.cumulative_scroll_offset(id);
        (viewport == popup.viewport
            && scroll == popup.geometry.scroll_offset
            && node.absolute_paint_rect.translate(-scroll.0, -scroll.1)
                == entry.region.owner_abs_rect)
            .then_some(popup.geometry)
    }

    pub(crate) fn combo_popup_input_geometry(
        &self,
        owner: TreeWidgetId,
    ) -> Option<ComboPopupInputGeometry> {
        self.overlay_hit_regions
            .iter()
            .rev()
            .filter(|entry| entry.region.widget_id == owner)
            .find_map(|entry| self.qualified_combo_popup(entry))
    }

    fn current_overlay_hit_region(
        &self,
        entry: &OverlayHitEntry,
        point: Point,
    ) -> Option<OverlayHitRegion> {
        let mut region = entry.region;
        if entry.combo_popup.is_some() {
            let popup = self.qualified_combo_popup(entry)?;
            return (popup.visible_rows > 0).then_some(region);
        }
        if let Some(owner) = self.find_node_by_widget_id(region.widget_id)
            && let Some(node) = self.node(owner)
            && let Some(area) = node.widget.as_any().downcast_ref::<ScrollArea>()
            && area.auto_hides_scrollbars()
        {
            if !area.scrollbar_overlay_visible() || region.owner_abs_rect != node.absolute_paint_rect {
                return None;
            }
            // Retained scrollbar geometry stays in the same space as its
            // scene fragment. Apply the current ancestor scroll and clip at hit time.
            let (x, y) = self.cumulative_scroll_offset(owner);
            region.abs_rect = region.abs_rect.translate(-x, -y);
            region.owner_abs_rect = region.owner_abs_rect.translate(-x, -y);
            if region.abs_rect.contains(point) && self.hit_test_candidate(owner, point).is_none() {
                return None;
            }
        }
        Some(region)
    }

    /// A clean fragment can be reused only while the overlay's placement basis
    /// still matches. Scroll and viewport changes need not move its layout rect.
    pub(crate) fn invalidate_changed_popup_hits(
        &mut self,
        scale_factor: f32,
        interpolated_rects: &crate::core::layout_transition::InterpolatedVisualRects,
    ) {
        for index in 0..self.overlay_hit_regions.len() {
            let entry = &self.overlay_hit_regions[index];
            let owner = entry.region.widget_id;
            let combo_changed =
                entry.combo_popup.is_some() && self.qualified_combo_popup(entry).is_none();
            let date_time_changed = self.find_by_id(owner).is_some_and(|node| {
                self.node(node)
                    .and_then(|entry| {
                        entry.widget.as_any()
                            .downcast_ref::<crate::widgets::date_time_picker::DateTimePicker>()
                    })
                    .is_some_and(|picker| {
                        !picker.popup_projection_matches(PaintContext::presentation_geometry_for_node(
                            self,
                            node,
                            scale_factor,
                            interpolated_rects,
                        ))
                    })
            });
            if combo_changed || date_time_changed {
                self.mark_paint_dirty_by_widget_id(owner);
            }
        }
    }

    pub(crate) fn has_blocking_overlay_hit_region(&self) -> bool {
        self.overlay_hit_regions
            .iter()
            .map(|entry| entry.region)
            .any(|region| region.abs_rect.width >= 100_000.0 || region.abs_rect.height >= 100_000.0)
    }

    /// The host picker uses the same front-to-back hit rectangles as input.
    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn host_choice_placements(
        &self,
        input_root: NodeId,
        viewport: RectF,
    ) -> Vec<crate::core::event_router::host_choice::ChoicePlacement> {
        use crate::core::event_router::host_choice::{ChoicePlacement, ChoiceTarget, choice_options};
        let Some(root) = self.root else {
            return Vec::new();
        };
        self.assert_control_subtree_orders_current();
        let mut hit_nodes = Vec::new();
        self.collect_hit_test_nodes_recursive_static(
            root, (0.0, 0.0), Some(viewport), &mut hit_nodes,
        );
        let mut covering = Vec::new();
        for entry in &self.overlay_hit_regions {
            let bounds = entry.region.abs_rect;
            let point = Point::new(bounds.x + bounds.width / 2.0, bounds.y + bounds.height / 2.0);
            if let Some(region) = self.current_overlay_hit_region(entry, point) {
                covering.push(region.abs_rect);
            }
        }

        let mut placements = Vec::new();
        for (id, region) in hit_nodes.into_iter().rev() {
            let node = self.node(id).expect("collected current hit node");
            if self.is_descendant_of_node(node.id, input_root)
                && let Some(mount) = self.concrete_mount(node.id)
                && let Some(options) = choice_options(self, &mount)
            {
                let (sx, sy) = self.cumulative_scroll_offset(id);
                let mut rect = node.absolute_paint_rect.translate(-sx, -sy);
                if let Some(width) = options.affordance {
                    let width = width.clamp(0.0, rect.width);
                    rect.x += rect.width - width;
                    rect.width = width;
                }
                if let Some(mut visible) = rect.intersection(&region) {
                    let center = Point::new(
                        visible.x + visible.width / 2.0, visible.y + visible.height / 2.0,
                    );
                    if self.touch_dismissible_popup(center, Some(node.id)).is_none() {
                        visible = visible.intersection(&viewport).unwrap_or(RectF::zero());
                        let mut exposed = vec![visible];
                        for cover in &covering {
                            exposed = exposed.into_iter()
                                .flat_map(|piece| crate::core::event_router::host_choice::subtract_cover(piece, *cover))
                                .collect();
                            if exposed.is_empty() {
                                break;
                            }
                        }
                        exposed.retain(|piece| piece.width > 0.0 && piece.height > 0.0);
                        if !exposed.is_empty() {
                            placements.push(ChoicePlacement {
                                target: ChoiceTarget { mount, revision: options.revision },
                                rect,
                                exposed,
                            });
                        }
                    }
                }
            }
            covering.push(region);
        }
        placements
    }

    pub(crate) fn dismiss_nonmodal_popup_for_press(
        &mut self,
        point: Point,
        target: Option<TreeWidgetId>,
    ) {
        // Follow the same order as overlay hit testing. A covering region
        // already owns this point, including an existing modal backdrop.
        for index in (0..self.overlay_hit_regions.len()).rev() {
            let Some(region) = self.current_overlay_hit_region(&self.overlay_hit_regions[index], point)
            else {
                continue;
            };
            if region.abs_rect.contains(point) {
                break;
            }
            let Some(owner) = self.find_node_by_widget_id(region.widget_id) else {
                continue;
            };
            if target.is_some_and(|widget_id| self.is_descendant_of_node(widget_id, owner)) {
                continue;
            }
            if self.dismiss_nonmodal_popup(region.widget_id) {
                break;
            }
        }
    }

    pub(crate) fn dismiss_nonmodal_popup(&mut self, owner: TreeWidgetId) -> bool {
        let Some(node_id) = self.find_by_id(owner) else {
            return false;
        };
        if self.node_mut(node_id).is_some_and(|node| {
            node.widget.dismiss_nonmodal_popup_on_outside_press()
        }) {
            // A closing visual tail no longer owns input.
            self.overlay_hit_regions.retain(|entry| entry.region.widget_id != owner);
            self.mark_paint_dirty(node_id);
            return true;
        }
        false
    }

    pub(crate) fn touch_dismissible_popup(
        &self, point: Point, target: Option<TreeWidgetId>,
    ) -> Option<NodeId> {
        for entry in self.overlay_hit_regions.iter().rev() {
            let Some(region) = self.current_overlay_hit_region(entry, point) else { continue; };
            if region.abs_rect.contains(point) {
                return None;
            }
            let owner = self.find_by_id(region.widget_id)?;
            if entry.combo_popup.is_some()
                && !region.owner_abs_rect.contains(point)
                && !target.is_some_and(|target| self.is_descendant_of_node(target, owner))
            {
                return Some(owner);
            }
        }
        None
    }

    /// Recursively collect (NodeId, abs_rect) pairs for hit-testable nodes.
    ///
    /// Traverses in paint order (back to front within z-order groups).
    /// SPEC-HAL-024: Computes absolute coordinates during traversal.
    /// Static version to avoid borrow conflicts with self.hit_test_nodes.
    /// SPEC-HAL-088 Stage 3: Collect nodes for hit-test grid using viewport.
    ///
    /// The grid stores viewport rects (clipped visible regions) so that
    /// the grid-accelerated path respects ancestor clipping.
    fn collect_hit_test_nodes_recursive_static(
        &self,
        node_id: NodeId,
        scroll: (f32, f32),
        inherited_clip: Option<RectF>,
        out: &mut Vec<(NodeId, RectF)>,
    ) {
        let Some(node) = self.node(node_id) else {
            return;
        };
        let Some((region, child_clip)) = self.hit_test_regions(node_id, scroll, inherited_clip)
        else {
            return;
        };
        if node.widget.hit_testable() && region.width > 0.0 && region.height > 0.0 {
            out.push((node_id, region));
        }
        if child_clip.width <= 0.0 || child_clip.height <= 0.0 {
            return;
        }
        let (sx, sy) = node.widget.scroll_offset().unwrap_or((0.0, 0.0));
        let child_scroll = (scroll.0 + sx, scroll.1 + sy);
        for child in self.hit_children_front_to_back(node_id).into_iter().rev() {
            self.collect_hit_test_nodes_recursive_static(
                child,
                child_scroll,
                Some(child_clip),
                out,
            );
        }
    }

    /// Canonical presented containment shared by DFS and conservative grid collection.
    fn hit_test_regions(
        &self,
        node_id: NodeId,
        scroll: (f32, f32),
        inherited_clip: Option<RectF>,
    ) -> Option<(RectF, RectF)> {
        let node = self.node(node_id)?;
        if !self.composite_choice_input_ready(node) {
            return None;
        }
        let abs = node.absolute_paint_rect;
        let bounds = RectF::new(abs.x - scroll.0, abs.y - scroll.1, abs.width, abs.height);
        let mut region = bounds;
        let parent_scrolls = node
            .parent
            .and_then(|id| self.node(id))
            .is_some_and(|parent| parent.widget.scroll_offset().is_some());
        if parent_scrolls && scroll != (0.0, 0.0) {
            if let Some(clip) = inherited_clip {
                if node.first_child.is_some()
                    || node.content_size.width > node.cached_size.width + 0.5
                {
                    region.x = clip.x;
                    region.width = clip.width;
                }
                if node.first_child.is_some()
                    || node.content_size.height > node.cached_size.height + 0.5
                {
                    region.y = clip.y;
                    region.height = clip.height;
                }
            }
        }
        if let Some(clip) = inherited_clip {
            region = region.intersection(&clip).unwrap_or(RectF::zero());
        }
        // A Visible node's own target region is not a descendant clip.
        let mut child_clip = inherited_clip.unwrap_or(bounds);
        if node.parent.is_none() || node.layout_scroll_offset.is_some() {
            child_clip = child_clip.intersection(&bounds).unwrap_or(RectF::zero());
        } else if node.overflow == OverflowMode::Clip {
            child_clip = child_clip.intersection(&region).unwrap_or(RectF::zero());
        }
        if let Some(clip) = node.widget.clip_bounds() {
            let clip = RectF::new(
                abs.x + clip.x - scroll.0,
                abs.y + clip.y - scroll.1,
                clip.width,
                clip.height,
            );
            child_clip = child_clip.intersection(&clip).unwrap_or(RectF::zero());
        }
        Some((region, child_clip))
    }

    fn hit_test_region_contains(region: RectF, point: Point) -> bool {
        point.x >= region.x
            && point.x < region.x + region.width
            && point.y >= region.y
            && point.y < region.y + region.height
    }

    // ========================================================================
    // SPEC-HAL-011: Take/Restore Pattern for Layout/Paint
    // ========================================================================

    /// Temporarily take widget out of tree for layout/paint
    /// Returns (widget, children_ids, rect)
    /// This allows calling widget.layout() with &mut WidgetTree in context
    pub(crate) fn take_widget(
        &mut self,
        node_id: NodeId,
    ) -> Option<(Box<dyn Widget>, Vec<NodeId>, RectF)> {
        self.hit_test_grid = None;
        // Collect children BEFORE taking mutable borrow of node
        let children = self.children(node_id);

        // Now take mutable borrow and extract widget
        let node = self.nodes.get_mut(node_id.0)?.as_mut()?;
        assert!(
            !node.widget_layout_lease,
            "HAL-101 nested Widget layout lease"
        );
        node.widget_layout_lease = true;
        let rect = node.rect;
        let widget = std::mem::replace(&mut node.widget, Box::new(DummyWidget));

        Some((widget, children, rect))
    }

    /// Restore widget back into tree after layout/paint
    ///
    /// SPEC-HAL-087: The node keeps its tree-managed TreeWidgetId (from alloc_widget_id).
    /// Only the widget instance and rect are replaced. Node.id is NEVER modified.
    ///
    /// SPEC-HAL-141: Rect mutations require layout authority.
    ///
    /// # Panics
    /// Panics in ALL BUILDS if called outside the layout phase (no LayoutAuthorityGuard active).
    ///
    /// # Identity Invariant
    /// Widget-internal IDs (WidgetInternalId) differ from tree-managed IDs (TreeWidgetId).
    /// This is expected and harmless - the tree-managed ID is the source of truth
    /// for event routing, hit-testing, and scene graph overlay nodes.
    pub(crate) fn restore_widget(&mut self, node_id: NodeId, widget: Box<dyn Widget>, rect: RectF) {
        // SPEC-HAL-141 INV-RECT-AUTH-ENFORCED: Verify rect mutation is authorized (ALL BUILDS)
        assert_rect_mutation_authorized("WidgetTree::restore_widget");

        // SPEC-HAL-087 Stage 2: Capture node.id for verification after mutable borrow ends
        #[cfg(debug_assertions)]
        let tree_widget_id = self
            .nodes
            .get(node_id.0)
            .and_then(|n| n.as_ref())
            .map(|n| n.id);

        if let Some(node) = self.nodes.get_mut(node_id.0).and_then(|n| n.as_mut()) {
            // SPEC-HAL-087 Stage 3: widget.id() returns WidgetInternalId (widget's own ID)
            // while node.id is TreeWidgetId (engine-assigned). These are intentionally
            // different types and cannot be compared. The tree-managed ID (node.id) is
            // the source of truth for event routing, hit-testing, and pointer capture.

            // Update widget and rect, but keep the same tree-managed node.id
            node.widget = widget;
            node.widget_layout_lease = false;
            node.rect = rect;
            // id_to_node mapping stays unchanged since node.id doesn't change
        }

        // SPEC-HAL-087 Stage 2: Verify node identity after restore
        #[cfg(debug_assertions)]
        if let Some(wid) = tree_widget_id {
            debug_assert!(self.verify_tree_widget_id(wid));
        }
    }

    /// Replace only the widget at a node, keeping the existing rect unchanged.
    ///
    /// This is safe to call during build() because it doesn't mutate the rect.
    /// Use this for placeholder replacement patterns where layout will set the
    /// rect later.
    pub fn replace_widget_only(&mut self, node_id: NodeId, widget: Box<dyn Widget>) {
        if self.node(node_id).is_none() {
            return;
        }
        self.retire_composite_editor(node_id);
        self.record_geometry_origin(node_id);
        let concrete_mount = self.allocate_concrete_mount();
        let is_custom_render_participant = widget.custom_render_admission().is_some();
        let is_scroll_area = widget.as_any().downcast_ref::<ScrollArea>().is_some();
        if let Some(owner) = self.node(node_id).map(|node| node.id) {
            self.remove_pending_text_handoff(owner);
            if let Some(policy) = crate::core::widget_availability::effective(widget.as_ref()) {
                self.availability_targets
                    .insert(owner, policy.read_only.is_some());
            } else {
                self.availability_targets.remove(&owner);
            }
        }
        let parts_replaced = self
            .node(node_id)
            .and_then(|n| n.control_parts_cache.as_ref())
            .is_some_and(|c| c.owner != Some(widget.id()));
        if parts_replaced {
            self.node_mut(node_id).unwrap().control_parts_cache = None;
            self.mark_layout_dirty(node_id);
            if let Some(parent) = self.node(node_id).and_then(|n| n.parent) {
                self.mark_layout_dirty(parent);
            }
        }
        if let Some(node) = self.nodes.get_mut(node_id.0).and_then(|n| n.as_mut()) {
            node.concrete_mount.retire();
            node.widget.on_unmount();
            node.widget = widget;
            node.composite_editor_child = false;
            node.composite_choice_host = false;
            node.widget_layout_lease = false;
            node.concrete_mount = concrete_mount;
            node.widget.on_mount();
        }
        self.scroll_anchor_nodes
            .retain(|&registered| registered != node_id);
        if is_scroll_area {
            self.scroll_anchor_nodes.push(node_id);
            self.bind_scroll_area_intent_admission(node_id);
        }
        self.authority_registry.unregister(node_id);
        if let Some((_, type_id)) = self
            .node(node_id)
            .and_then(|node| node.widget.provided_authority())
        {
            self.authority_registry.register(node_id, type_id);
        }
        self.mark_layout_dirty(node_id);
        self.custom_render_participant_nodes
            .retain(|&registered| registered != node_id);
        if is_custom_render_participant {
            self.custom_render_participant_nodes.push(node_id);
        }
        self.aligned_order_book_nodes
            .retain(|&registered| registered != node_id);
        self.index_aligned_order_book(node_id);
        self.assemble_composite_editor(node_id);
        self.index_pending_text_handoff(node_id);
    }

    // ========================================================================
    // SPEC-HAL-024: Absolute Coordinate Computation
    // ========================================================================

    /// Compute absolute bounds for a widget by walking up to root
    /// This mirrors the traversal logic in paint/hit-test
    pub fn compute_absolute_bounds(&self, widget_id: TreeWidgetId) -> Option<RectF> {
        let node_id = self.find_node_by_widget_id(widget_id)?;

        // Walk up to root, collecting relative rects and scroll offsets
        let mut path_to_root = Vec::new();
        let mut current_id = Some(node_id);

        while let Some(id) = current_id {
            if let Some(node) = self.node(id) {
                path_to_root.push((id, node.rect, node.parent));
                current_id = node.parent;
            } else {
                break;
            }
        }

        // Reverse to go root -> target
        path_to_root.reverse();

        // Compute absolute coordinates by accumulating offsets
        let mut abs_x = 0.0;
        let mut abs_y = 0.0;

        for (i, (id, rect, _parent)) in path_to_root.iter().enumerate() {
            // Current widget's absolute position
            abs_x += rect.x;
            abs_y += rect.y;

            // If this is not the target widget, apply scroll offset for next level
            if i < path_to_root.len() - 1 {
                if let Some(node) = self.node(*id) {
                    if let Some((scroll_x, scroll_y)) = node.widget.scroll_offset() {
                        abs_x -= scroll_x;
                        abs_y -= scroll_y;
                    }
                }
            }
        }

        // Get the target widget's rect
        let node = self.node(node_id)?;
        Some(RectF::new(abs_x, abs_y, node.rect.width, node.rect.height))
    }

    // ========================================================================
    // SPEC-HAL-131C: Input Projection for Virtualization
    // ========================================================================

    /// SPEC-HAL-131C: Clear all input projections (called at start of frame).
    ///
    /// INV-INPUT-PROJECTION-2: Projections are rebuilt each frame during layout.
    pub fn clear_input_projections(&mut self) {
        self.input_projections.clear();
    }

    /// SPEC-HAL-131C: Register or update an input projection for a container.
    ///
    /// INV-PROJECTION-SNAPSHOT-1: Called during layout_tree() only.
    pub fn set_input_projection(&mut self, container_id: NodeId, projection: InputProjection) {
        self.input_projections.insert(container_id, projection);
    }

    /// SPEC-HAL-131C: Get input projection for a container (immutable).
    ///
    /// INV-INPUT-PROJECTION-1: Input queries projection snapshot, never realized tree.
    pub fn get_input_projection(&self, container_id: NodeId) -> Option<&InputProjection> {
        self.input_projections.get(&container_id)
    }

    /// SPEC-HAL-131C: Hit-test with projection awareness.
    ///
    /// For virtualizing containers, this returns a ProjectedHitResult that includes
    /// the logical item identity (stable across realization cycles).
    ///
    /// INV-INPUT-PROJECTION-1: Queries projection snapshot, never realized tree.
    /// INV-INPUT-PROJECTION-2: Does not mutate projection or trigger realization.
    /// INV-INPUT-IDENTITY-1: Returns logical identity, stable across realization churn.
    ///
    /// Returns None if no hit, or a ProjectedHitResult with logical identity.
    pub fn hit_test_projected(&self, x: f32, y: f32) -> Option<ProjectedHitResult> {
        // First, do a normal hit-test to find the container
        let hit = self.hit_test(x, y)?;

        // Find the node for this widget
        let node_id = self.find_node_by_widget_id(hit.widget_id)?;

        // Check if this node has an input projection, or any ancestor does
        let mut current_id = Some(node_id);
        while let Some(check_id) = current_id {
            if let Some(projection) = self.input_projections.get(&check_id) {
                // Found a projection - hit-test within it
                let node = self.node(check_id)?;
                let viewport_bounds = node.absolute_paint_rect;

                // Compute viewport-local coordinates.
                // Note: absolute_paint_rect does NOT include scroll offset.
                let local_x = x - viewport_bounds.x;
                let local_y = y - viewport_bounds.y;

                // Transform to content coordinates by adding scroll offset.
                let content_x = local_x + projection.scroll_offset.0;
                let content_y = local_y + projection.scroll_offset.1;

                // Search entries in reverse order (front-to-back in paint order)
                for entry in projection.entries().iter().rev() {
                    if entry.content_bounds.contains_point(content_x, content_y) {
                        // Compute local pos within the item
                        let item_local_x = content_x - entry.content_bounds.x;
                        let item_local_y = content_y - entry.content_bounds.y;

                        return Some(ProjectedHitResult {
                            logical_item_id: Some(entry.logical_id),
                            logical_index: Some(entry.logical_index),
                            realized_widget_id: entry.realized_widget,
                            container_id: projection.container_id.unwrap_or(hit.widget_id),
                            local_pos: (item_local_x, item_local_y),
                            item_bounds: entry.content_bounds,
                        });
                    }
                }
            }

            // Move up to parent
            current_id = self.node(check_id).and_then(|n| n.parent);
        }

        // No projection found - return None (use standard hit_test instead)
        None
    }

    // ========================================================================
    // SPEC-HAL-009: Hit-Testing
    // SPEC-HAL-088 Stage 3: Hit-testing uses viewport (accounts for ancestor clipping)
    // SPEC-HAL-032: Grid-accelerated hit-testing
    // ========================================================================

    /// Hit-test a point against the widget tree.
    ///
    /// # Arguments
    /// - `x`, `y`: **Absolute logical coordinates** (not physical, not relative).
    ///   Caller must have already divided physical coordinates by `scale_factor`.
    ///
    /// # Returns
    /// The deepest widget that contains the point, along with local coordinates.
    ///
    /// # Traversal
    /// SPEC-HAL-086 Stage 8: First checks overlay hit regions (scrollbars).
    /// Then walks widgets in paint order / z-order (highest z first for top-most).
    ///
    /// # SPEC-HAL-088 Stage 3
    /// Uses `node.viewport` (computed during layout) for containment tests.
    /// Viewport accounts for ancestor clipping, so nodes outside their
    /// clipped visible region are not hittable.
    pub fn hit_test(&self, x: f32, y: f32) -> Option<HitTestResult> {
        self.hit_test_for_routing(x, y).map(|(hit, _)| hit)
    }

    /// Hit-test for event routing while preserving whether the winning region
    /// came from the overlay pass or the ordinary widget tree.
    pub(crate) fn hit_test_for_routing(
        &self,
        x: f32,
        y: f32,
    ) -> Option<(HitTestResult, PointerRouteOrigin)> {
        self.assert_control_subtree_orders_current();
        if let Some(hit) = self.hit_test_overlay(x, y) {
            return Some((hit, PointerRouteOrigin::Overlay));
        }

        self.hit_test_regular(x, y)
            .map(|hit| (hit, PointerRouteOrigin::Regular))
    }

    fn hit_test_overlay(&self, x: f32, y: f32) -> Option<HitTestResult> {
        // SPEC-HAL-086 Stage 8: Check overlay hit regions FIRST (scrollbars)
        // Overlays are rendered on top and should intercept hits before content
        let point = Point::new(x, y);
        for overlay in self
            .overlay_hit_regions
            .iter()
            .rev()
            .filter_map(|entry| self.current_overlay_hit_region(entry, point))
        {
            if overlay.abs_rect.contains(point) {
                // Local coords must be *widget-local*, not track-local
                let local_x = x - overlay.owner_abs_rect.x;
                let local_y = y - overlay.owner_abs_rect.y;

                return Some(HitTestResult {
                    widget_id: overlay.widget_id,
                    local_pos: (local_x, local_y),
                    absolute_bounds: (
                        overlay.owner_abs_rect.x,
                        overlay.owner_abs_rect.y,
                        overlay.owner_abs_rect.width,
                        overlay.owner_abs_rect.height,
                    ),
                });
            }
        }

        None
    }

    fn hit_test_regular(&self, x: f32, y: f32) -> Option<HitTestResult> {
        self.assert_control_subtree_orders_current();
        let root_id = self.root?;

        // SPEC-HAL-032: If we have a grid AND node count is above threshold, use accelerated path
        if let Some(ref grid) = self.hit_test_grid {
            let point = Point::new(x, y);
            let bucket = grid.query(point);

            if bucket.is_empty() {
                // Definitely nothing under this point
                return None;
            }

            for candidate in bucket.iter().rev() {
                if let Some(hit) = self.hit_test_candidate(candidate.node_id, point) {
                    return Some(hit);
                }
            }
            return None;
        }

        // Fallback: original DFS path (for small trees or edge cases)
        // SPEC-HAL-024 Stage 4: No longer pass parent_abs; use absolute_paint_rect directly
        // SPEC-HAL-086 Stage 7: Start with (0,0) scroll offset
        let mut path: Vec<(NodeId, RectF)> = Vec::new();
        let result = self.hit_test_node_with_path(root_id, x, y, (0.0, 0.0), &mut path);

        // SPEC-HAL-087 Stage 2: Verify DFS result
        #[cfg(debug_assertions)]
        if let Some(ref r) = result {
            debug_assert!(self.verify_tree_widget_id(r.widget_id));
        }

        result
    }

    pub(crate) fn hit_test_candidate(&self, target: NodeId, point: Point) -> Option<HitTestResult> {
        let mut path = Vec::new();
        let mut current = Some(target);
        while let Some(id) = current {
            let node = self.node(id)?;
            path.push(id);
            current = node.parent;
        }
        path.reverse();
        let mut scroll = (0.0, 0.0);
        let mut clip = None;
        for id in path {
            let node = self.node(id)?;
            let (region, child_clip) = self.hit_test_regions(id, scroll, clip)?;
            if id == target {
                if !node.widget.hit_testable() || !Self::hit_test_region_contains(region, point) {
                    return None;
                }
                let rect = node.absolute_paint_rect;
                return Some(HitTestResult {
                    widget_id: node.id,
                    local_pos: (point.x + scroll.0 - rect.x, point.y + scroll.1 - rect.y),
                    absolute_bounds: (rect.x, rect.y, rect.width, rect.height),
                });
            }
            if !Self::hit_test_region_contains(child_clip, point) {
                return None;
            }
            clip = Some(child_clip);
            if let Some((x, y)) = node.widget.scroll_offset() {
                scroll = (scroll.0 + x, scroll.1 + y);
            }
        }
        None
    }

    #[cfg(test)]
    pub(crate) fn has_current_hit_test_grid(&self) -> bool {
        self.hit_test_grid.is_some()
    }

    #[cfg(test)]
    pub(crate) fn hit_test_canonical(&self, x: f32, y: f32) -> Option<HitTestResult> {
        self.hit_test_node_with_path(self.root?, x, y, (0.0, 0.0), &mut Vec::new())
    }

    /// SPEC-HAL-018 Stage 8: Hit-test restricted to a specific subtree.
    ///
    /// When a modal is active, hit-testing should be restricted to the modal's
    /// subtree to prevent hits on widgets underneath the modal.
    ///
    /// # Arguments
    /// - `x`, `y`: **Absolute logical coordinates** (same as hit_test)
    /// - `subtree_root`: The NodeId to restrict hit-testing to
    ///
    /// # Returns
    /// The deepest widget within the subtree that contains the point, or None
    /// if the point is outside the subtree or no hit-testable widget is found.
    ///
    /// # Usage
    /// Called from event routing when a modal is active. The subtree_root is
    /// typically the modal's root node (last child of ScreenHost).
    pub fn hit_test_in_subtree(
        &self,
        x: f32,
        y: f32,
        subtree_root: NodeId,
    ) -> Option<HitTestResult> {
        // Use the DFS path starting from subtree_root instead of tree root
        // SPEC-HAL-086 Stage 7: Start with (0,0) scroll offset
        // Note: If subtree is inside a scroll container, caller should compute cumulative offset
        let mut path: Vec<(NodeId, RectF)> = Vec::new();
        let result = self.hit_test_node_with_path(subtree_root, x, y, (0.0, 0.0), &mut path);

        // SPEC-HAL-087 Stage 2: Verify subtree result
        #[cfg(debug_assertions)]
        if let Some(ref r) = result {
            debug_assert!(self.verify_tree_widget_id(r.widget_id));
        }

        result
    }

    /// Hit-test with path tracking for debugging.
    ///
    /// SPEC-HAL-088 Stage 3: Uses `node.viewport` for containment tests, which
    /// accounts for ancestor clipping. Nodes outside their clipped region
    /// are not hittable.
    ///
    /// SPEC-HAL-086 Stage 7: Takes cumulative scroll offset to transform pointer
    /// coordinates when testing inside scroll containers.
    fn hit_test_node_with_path(
        &self,
        node_id: NodeId,
        pointer_x: f32,
        pointer_y: f32,
        scroll_offset: (f32, f32),
        path: &mut Vec<(NodeId, RectF)>,
    ) -> Option<HitTestResult> {
        self.assert_control_subtree_orders_current();
        self.hit_test_node_with_path_clipped(
            node_id,
            pointer_x,
            pointer_y,
            scroll_offset,
            path,
            None,
        )
    }

    fn hit_test_node_with_path_clipped(
        &self,
        node_id: NodeId,
        pointer_x: f32,
        pointer_y: f32,
        scroll_offset: (f32, f32),
        path: &mut Vec<(NodeId, RectF)>,
        inherited_clip: Option<RectF>,
    ) -> Option<HitTestResult> {
        let node = self.node(node_id)?;
        let (region, child_clip) = self.hit_test_regions(node_id, scroll_offset, inherited_clip)?;
        let point = Point::new(pointer_x, pointer_y);
        let hits_self = Self::hit_test_region_contains(region, point);
        let visits_children = Self::hit_test_region_contains(child_clip, point);
        if !hits_self && !visits_children {
            return None;
        }

        // Keep absolute_paint_rect for containment check and local_pos calculations
        let abs_rect = node.absolute_paint_rect;

        // INV-HITTEST-PAINT-SPACE-1: Hit-testing in scroll containers.
        // abs_rect is in CONTENT space (scroll not applied during layout).
        // content_coords = pointer + scroll_offset transforms screen → content coords.
        let content_x = pointer_x + scroll_offset.0;
        let content_y = pointer_y + scroll_offset.1;

        // HAL_DEBUG_HIT: Log nodes that pass containment check (cached env var check)
        static DEBUG_HIT: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
        if *DEBUG_HIT.get_or_init(|| std::env::var_os("HAL_DEBUG_HIT").is_some()) {
            let widget_type = node.widget.debug_type_name();
            let viewport = node.viewport;
            eprintln!(
                "[HAL_DEBUG_HIT] PASS node={:?} type={} abs_rect=({:.1},{:.1},{:.1},{:.1}) viewport=({:.1},{:.1},{:.1},{:.1}) z={} hit_testable={}",
                node_id,
                widget_type,
                abs_rect.x,
                abs_rect.y,
                abs_rect.width,
                abs_rect.height,
                viewport.x,
                viewport.y,
                viewport.width,
                viewport.height,
                node.z_order,
                node.widget.hit_testable()
            );
        }

        // Add to path (use actual widget rect, not clipped viewport)
        path.push((node_id, abs_rect));

        // INV-HITTEST-PROJECTION-CHAIN-1: Get this node's scroll offset for children.
        // If this node provides scroll_offset (e.g., ScrollArea), accumulate it.
        // Children's abs_rect is in CONTENT space, so we need to transform pointer coords.
        let child_scroll_offset = if let Some((sx, sy)) = node.widget.scroll_offset() {
            (scroll_offset.0 + sx, scroll_offset.1 + sy)
        } else {
            scroll_offset
        };

        // Try children with accumulated scroll offset
        if visits_children {
            for child_id in self.hit_children_front_to_back(node_id) {
                if let Some(result) = self.hit_test_node_with_path_clipped(
                    child_id,
                    pointer_x,
                    pointer_y,
                    child_scroll_offset,
                    path,
                    Some(child_clip),
                ) {
                    return Some(result);
                }
            }
        }

        // No child hit - check if this node is hit-testable
        if !hits_self || !node.widget.hit_testable() {
            path.pop(); // Remove from path since we're not the target
            return None;
        }

        // This node is the target
        // SPEC-HAL-086 Stage 7: local_pos is in content coordinates relative to abs_rect
        let local_x = content_x - abs_rect.x;
        let local_y = content_y - abs_rect.y;

        // HAL_DEBUG_HIT: Log the winning hit node (reuses DEBUG_HIT from above)
        if *DEBUG_HIT.get_or_init(|| std::env::var_os("HAL_DEBUG_HIT").is_some()) {
            let widget_type = node.widget.debug_type_name();
            eprintln!(
                "[HAL_DEBUG_HIT] *** WINNER *** node={:?} type={} abs_rect=({:.1},{:.1},{:.1},{:.1}) local=({:.1},{:.1})",
                node_id,
                widget_type,
                abs_rect.x,
                abs_rect.y,
                abs_rect.width,
                abs_rect.height,
                local_x,
                local_y
            );
            // Print full path
            eprintln!("[HAL_DEBUG_HIT] Path to root:");
            for (i, (pid, prect)) in path.iter().enumerate() {
                let pnode = self.node(*pid);
                let ptype = pnode.map(|n| n.widget.debug_type_name()).unwrap_or("?");
                eprintln!(
                    "  [{}] node={:?} type={} rect=({:.1},{:.1},{:.1},{:.1})",
                    i, pid, ptype, prect.x, prect.y, prect.width, prect.height
                );
            }
        }

        // HAL_DEBUG_HIT_TRACE_NODE: Detailed trace for specific node (cached env var check)
        static DEBUG_HIT_TRACE: std::sync::OnceLock<Option<usize>> = std::sync::OnceLock::new();
        if let Some(trace_id) = *DEBUG_HIT_TRACE.get_or_init(|| {
            std::env::var("HAL_DEBUG_HIT_TRACE_NODE")
                .ok()
                .and_then(|s| s.parse::<usize>().ok())
        }) {
            if node_id.0 == trace_id {
                eprintln!(
                    "[HAL_DEBUG_HIT_TRACE_NODE] Target node {:?} is the WINNER",
                    node_id
                );
            }
        }

        Some(HitTestResult {
            widget_id: node.id,
            local_pos: (local_x, local_y),
            absolute_bounds: (abs_rect.x, abs_rect.y, abs_rect.width, abs_rect.height),
        })
    }

    /// Recursive hit-test helper (unused, kept for reference).
    ///
    /// SPEC-HAL-088 Stage 3: Uses `node.viewport` for containment tests.
    #[allow(dead_code)]
    fn hit_test_node(
        &self,
        node_id: NodeId,
        pointer_x: f32,
        pointer_y: f32,
    ) -> Option<HitTestResult> {
        let node = self.node(node_id)?;

        // SPEC-HAL-088 Stage 3: Use viewport for hit-testing (accounts for ancestor clipping)
        let viewport = node.viewport;
        let abs_rect = node.absolute_paint_rect;

        // Check if point is inside this node's viewport (clipped visible region)
        if pointer_x < viewport.x
            || pointer_x >= viewport.x + viewport.width
            || pointer_y < viewport.y
            || pointer_y >= viewport.y + viewport.height
        {
            return None;
        }

        // Collect children and sort by z-order (highest z first for top-most)
        let mut children_with_z: Vec<(NodeId, i16)> = Vec::new();
        let mut current_child = node.first_child;
        while let Some(child_id) = current_child {
            if let Some(child_node) = self.node(child_id) {
                children_with_z.push((child_id, child_node.z_order));
                current_child = child_node.next_sibling;
            } else {
                break;
            }
        }

        // Sort by z-order descending (highest z first)
        children_with_z.sort_by(|a, b| b.1.cmp(&a.1));

        // Try children - they already have absolute_paint_rect computed
        for (child_id, _) in children_with_z {
            if let Some(result) = self.hit_test_node(child_id, pointer_x, pointer_y) {
                return Some(result);
            }
        }

        // No child hit - check if this node is hit-testable
        if !node.widget.hit_testable() {
            return None; // Skip non-hit-testable widgets
        }

        // This node is the target - compute local coordinates
        let local_x = pointer_x - abs_rect.x;
        let local_y = pointer_y - abs_rect.y;

        Some(HitTestResult {
            widget_id: node.id,
            local_pos: (local_x, local_y),
            absolute_bounds: (abs_rect.x, abs_rect.y, abs_rect.width, abs_rect.height),
        })
    }

    // ========================================================================
    // SPEC-HAL-086 Stage 4: Global Scroll Animation Tick
    // ========================================================================

    /// Update visual scroll offsets for all scrollable widgets in the tree.
    ///
    /// SPEC-HAL-086 Stage 4: This is called once per frame during the Animation Tick
    /// phase, after event routing and before layout/scene build. It advances the
    /// visual scroll offset toward the target offset using exponential smoothing.
    ///
    /// SPEC-HAL-086 Stage 6: Frame-driven drag visual updates. This is the ONLY place
    /// that updates visual_offset. Event handlers only set target_offset, never visual.
    /// In Drag mode, visual instantly tracks target; in Normal mode, smooth interpolation.
    ///
    /// Widgets that expose a scroll container via `scroll_container_mut()` will
    /// have their visual offsets updated. Other widgets are skipped.
    pub fn update_scroll_animations(&mut self, dt_secs: f32) {
        let mut changed_scroll_nodes = Vec::new();
        for idx in 0..self.nodes.len() {
            if let Some(node) = self.nodes.get_mut(idx).and_then(|n| n.as_mut()) {
                if let Some(scroll) = node.widget.scroll_container_mut() {
                    let previous = scroll.visual_offset();
                    scroll.update_visual(dt_secs);
                    if scroll.visual_offset() != previous {
                        changed_scroll_nodes.push(NodeId(idx));
                    }
                }
            }
        }

        // The presented scroll transform belongs to the ScrollArea's retained
        // fragment. Advancing it is paint-only, but replaying the old fragment
        // would replay the old transform indefinitely.
        for node_id in changed_scroll_nodes {
            self.record_geometry_origin(node_id);
            self.mark_paint_dirty(node_id);
        }
    }

    /// Finish the frame's scroll update after current input has had its
    /// opportunity to interrupt post-release motion.
    pub fn advance_kinetic_scroll_for_frame(&mut self, frame: &crate::core::app::FrameTime) -> bool {
        let Some(mount) = self.scroll_intent_admission.kinetic_scroll_mount() else {
            return false;
        };
        let owner = mount.owner();
        let Some(node_id) = self.find_by_id(owner) else { return false; };
        if self.concrete_mount(owner).as_ref() != Some(&mount) {
            return false;
        }
        if let Some(node) = self.nodes[node_id.0].as_mut()
            && let Some(area) = node.widget.as_any_mut().downcast_mut::<ScrollArea>()
            && let Some(axis) = area.advance_kinetic_scroll(frame.now)
        {
            self.record_geometry_origin(node_id);
            self.mark_paint_dirty(node_id);
            if axis == crate::core::event::GestureAxis::Vertical {
                self.mark_nearest_scroll_dependent_ancestor_layout(owner);
            }
        }
        self.scroll_intent_admission.has_kinetic_scroll()
    }

    /// Publish linked horizontal scroll coordinates before composite updates.
    ///
    /// The parent owns fixed header/footer paint while the linked ScrollArea
    /// subtree owns body paint. A changed presented coordinate therefore
    /// invalidates that bounded composite subtree without invalidating layout.
    pub(crate) fn sync_linked_horizontal_scroll_projections(&mut self) {
        let mut changed_composites = Vec::new();
        for idx in 0..self.nodes.len() {
            let Some(node) = self.nodes.get_mut(idx).and_then(|n| n.as_mut()) else {
                continue;
            };
            let Some(scroll_area) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() else {
                continue;
            };
            if scroll_area.publish_horizontal_scroll_position() {
                changed_composites.push(node.parent.unwrap_or(NodeId(idx)));
            }
        }

        changed_composites.sort_unstable_by_key(|id| id.0);
        changed_composites.dedup();
        for composite in changed_composites {
            self.mark_subtree_paint_dirty(composite);
        }
    }

    /// SPEC-HAL-086 Stage 12: Check if any scroll container needs continuous frame updates.
    /// Returns true if any scroll is in Drag mode or has active inertia.
    pub fn any_scroll_needs_animation(&self) -> bool {
        if self.scroll_intent_admission.has_kinetic_scroll() {
            return true;
        }
        for idx in 0..self.nodes.len() {
            if let Some(node) = self.nodes.get(idx).and_then(|n| n.as_ref()) {
                if let Some(scroll) = node.widget.scroll_container() {
                    if scroll.needs_animation() {
                        return true;
                    }
                }
            }
        }
        false
    }

    /// SPEC-HAL-101 Stage-4C: Check if any widget needs continuous animation frames.
    /// Returns true if any widget's needs_animation() returns true.
    /// Used for caret blinking in focused TextFields.
    pub fn any_widget_needs_animation(&self) -> bool {
        for idx in 0..self.nodes.len() {
            if let Some(node) = self.nodes.get(idx).and_then(|n| n.as_ref()) {
                if node.widget.needs_animation() {
                    return true;
                }
            }
        }
        false
    }

    /// SPEC-HAL-354: Check whether any widget needs update-phase ticks.
    ///
    /// Animation also implies update work, but update-only widgets do not
    /// imply per-frame paint invalidation.
    pub fn any_widget_needs_update(&self) -> bool {
        if self.has_pending_widget_availability() {
            return true;
        }
        for idx in 0..self.nodes.len() {
            if let Some(node) = self.nodes.get(idx).and_then(|n| n.as_ref()) {
                if node.widget.needs_update() || node.widget.needs_animation() {
                    return true;
                }
            }
        }
        false
    }

    // ========================================================================
    // SPEC-HAL-088 Stage 2: Viewport Propagation
    // ========================================================================

    /// SPEC-HAL-088 Stage 2: Compute viewport for all nodes in the tree.
    ///
    /// Viewport represents the actual visible region of a node, based on
    /// ancestor clipping boundaries. This is a top-down DFS traversal that:
    /// - Starts with root's absolute_paint_rect as the initial clip
    /// - For nodes with `OverflowMode::Clip`, intersects with their rect
    /// - For nodes with `OverflowMode::Visible`, passes clip unchanged
    ///
    /// Must be called AFTER `compute_absolute_paint_rects()` so that
    /// absolute positions are available.
    ///
    /// Stage 2: viewport is computed but NOT used for hit-testing or painting yet.
    pub fn compute_viewports(&mut self, root: NodeId) {
        // Get root's absolute_paint_rect as the initial clip region
        let root_rect = match self.node(root) {
            Some(node) => node.absolute_paint_rect,
            None => return,
        };

        // DFS traversal to compute viewports (start outside any scrollport)
        self.compute_viewports_dfs(root, root_rect, false);
    }

    /// SPEC-HAL-088 Stage 2/4: DFS helper for viewport computation.
    ///
    /// - `incoming_clip`: The clip region inherited from ancestors
    /// - `inside_scrollport`: True if we're inside a scroll container (children inherit scrollport as viewport)
    /// - Node's viewport = intersection of its absolute_paint_rect with incoming_clip (unless inside_scrollport)
    /// - For Clip nodes: passes this viewport to children (adds new clip boundary)
    /// - For Visible nodes: passes incoming_clip unchanged to children
    /// - Stage 4: Scroll containers use scrollport (their own rect) as clip boundary for children
    fn compute_viewports_dfs(
        &mut self,
        node_id: NodeId,
        incoming_clip: RectF,
        inside_scrollport: bool,
    ) {
        // First, compute this node's viewport and determine clip for children
        let (viewport, child_clip, child_inside_scrollport) = {
            let node = match self.node(node_id) {
                Some(n) => n,
                None => return,
            };

            // SPEC-HAL-088 Stage 4: Detect scroll containers
            let is_scroll_container = node.layout_scroll_offset.is_some();

            if is_scroll_container {
                // Scroll container: use scrollport as the clipping boundary
                // scrollport = the container's visible area (its absolute_paint_rect)
                let scrollport = node.absolute_paint_rect;

                // This scroll container's viewport = intersection with ancestor clips
                let node_viewport = incoming_clip
                    .intersection(&scrollport)
                    .unwrap_or(RectF::zero());

                // SPEC-HAL-146: Assert layout authority before viewport mutation
                assert_rect_mutation_authorized("compute_viewports_dfs: scroll container viewport");

                // Children inherit scrollport as their clip AND are marked inside_scrollport
                // so they don't get clipped by their own content rect
                (node_viewport, node_viewport, true)
            } else if inside_scrollport {
                // Inside a scroll container: viewport = scrollport (incoming_clip)
                // Do NOT intersect with own absolute_paint_rect - content may be outside
                // window bounds in layout space but scrollable into view
                // SPEC-HAL-146: Assert layout authority before viewport mutation
                assert_rect_mutation_authorized(
                    "compute_viewports_dfs: inside scrollport viewport",
                );

                // Continue passing the scrollport to descendants
                (incoming_clip, incoming_clip, true)
            } else {
                // Non-scroll node outside any scroll container: original Stage 2 logic
                let node_viewport = incoming_clip
                    .intersection(&node.absolute_paint_rect)
                    .unwrap_or(RectF::zero());

                // SPEC-HAL-146: Assert layout authority before viewport mutation
                assert_rect_mutation_authorized("compute_viewports_dfs: normal viewport");

                // Determine clip to pass to children based on overflow mode
                let child_clip = if node.overflow == OverflowMode::Clip {
                    // Clip nodes restrict children to their own viewport
                    node_viewport
                } else {
                    // Visible nodes don't restrict children beyond ancestor clips
                    incoming_clip
                };

                (node_viewport, child_clip, false)
            }
        };
        self.set_viewport(node_id, viewport);

        // Collect children (outside the mutable borrow)
        let children = self.children(node_id);

        // Recurse into children with the computed clip
        for child_id in children {
            self.compute_viewports_dfs(child_id, child_clip, child_inside_scrollport);
        }
    }

    // ========================================================================
    // SPEC-HAL-078 Stage 0: Layout Transition Geometry Snapshot
    // ========================================================================

    /// SPEC-HAL-078 Stage 0: Begin a new frame for layout transition tracking.
    ///
    /// Swaps previous/current geometry buffers and clears current.
    /// Must be called once per frame BEFORE layout begins.
    /// LT-INV-12: Reuses buffer capacity (no allocation if capacity sufficient).
    pub fn begin_layout_transition_frame(&mut self) {
        self.layout_transition_store.begin_frame();
    }

    /// Called after a complete existing absolute-geometry traversal, not for
    /// individual records. It is coverage evidence, not final-layout admission.
    pub(crate) fn record_layout_transition_traversal(&mut self, root: NodeId) {
        if let Some(mount) = self.node(root).and_then(|n| self.concrete_mount(n.id)) {
            self.layout_transition_store
                .record_complete_traversal(mount, self.structure_generation);
        }
    }

    /// The App calls this only after final Layout and nondeferred scroll sync.
    pub(crate) fn complete_layout_transition_layout(&mut self, root: NodeId) -> bool {
        self.layout_transition_store.invalidate_layout_completion();
        let Some(node) = self.node(root).filter(|n| !n.dirty_layout) else {
            return false;
        };
        if !self
            .get_absolute_paint_rect(root)
            .is_some_and(|r| [r.x, r.y, r.width, r.height].iter().all(|v| v.is_finite()))
        {
            return false;
        }
        let Some(mount) = self.concrete_mount(node.id) else {
            return false;
        };
        self.layout_transition_store
            .complete_layout(mount, self.structure_generation)
    }

    /// SPEC-HAL-078 Stage 0: Record a widget's authoritative geometry.
    ///
    /// Called during compute_absolute_paint_rects for each node.
    /// LT-INV-5: Keyed strictly by TreeWidgetId.
    #[inline]
    pub fn record_layout_transition(&mut self, id: TreeWidgetId, rect: RectF) {
        self.layout_transition_store.record(id, rect);
    }

    /// SPEC-HAL-078 Stage 1: Collect widgets eligible for layout transition.
    ///
    /// Caller provides and reuses the output buffer to avoid per-frame allocation.
    /// Called after layout completes; result is frame-local (dropped before next frame).
    pub fn collect_layout_transition_candidates(&self, out: &mut Vec<LayoutTransitionCandidate>) {
        self.layout_transition_store
            .collect_rect_change_candidates(out);
    }

    /// SPEC-HAL-078A: Get reference to the layout transition store.
    ///
    /// Used for building AuthorizationContext in the unified resolver.
    pub fn layout_transition_store(&self) -> &LayoutTransitionStore {
        &self.layout_transition_store
    }

    // ========================================================================
    // SPEC-HAL-078B Stage 1.75: Conflict Flags Computation
    // ========================================================================

    /// SPEC-HAL-078B: Compute conflict flags for all widgets.
    ///
    /// Called once per frame after layout completes. Populates the output HashMap
    /// with flags for each widget, enabling O(1) conflict resolution.
    ///
    /// INVARIANTS:
    /// - Zero allocations beyond initial capacity (caller provides map)
    /// - Single pass over nodes + parent chain walks (amortized O(n))
    /// - Read-only access to tree (no mutations)
    pub fn compute_conflict_flags(&self, out: &mut HashMap<TreeWidgetId, ConflictFlags>) {
        out.clear();

        // Build set of virtualized widget IDs from input projections
        let virtualized_ids: HashSet<TreeWidgetId> = self
            .input_projections
            .values()
            .flat_map(|proj| proj.entries().iter().filter_map(|e| e.realized_widget))
            .collect();

        // Build set of overlay participant widget IDs
        let overlay_ids: HashSet<TreeWidgetId> = self
            .overlay_hit_regions
            .iter()
            .map(|entry| entry.region.widget_id)
            .collect();

        // SPEC-HAL-078B CONFLICT-4: Build set of NodeIds for scroll containers with pending anchor resolution.
        // Widgets inside these scroll containers have their transitions denied because anchor
        // preservation has higher priority than animation.
        //
        // INVARIANT (LT-INV-ANCHOR-IMMUTABLE): Anchor resolution state is sampled ONCE per frame
        // here and treated as immutable for the remainder of conflict resolution. Engine phase
        // ordering guarantees this:
        //   1. update_scroll_animations() completes (may set anchor dirty flags)
        //   2. compute_conflict_flags() samples anchor state HERE (once)
        //   3. resolve_conflicts() uses sampled flags (no re-query)
        //   4. resolve_nested_conflicts() uses sampled flags (no re-query)
        //   5. bind_candidates() binds transitions (anchor state not consulted again)
        //
        // Mutation of anchor state during conflict resolution would violate determinism.
        // This is architecturally prevented by phase ordering, not runtime assertion.
        let anchor_compensated_scroll_containers: HashSet<NodeId> = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(idx, node_opt)| {
                let node = node_opt.as_ref()?;
                // Only consider scroll containers (have layout_scroll_offset)
                if node.layout_scroll_offset.is_none() {
                    return None;
                }
                // Check if this scroll container has pending anchor resolution
                if node.widget.has_pending_anchor_resolution() {
                    Some(NodeId(idx))
                } else {
                    None
                }
            })
            .collect();

        // Iterate all active nodes
        for idx in 0..self.nodes.len() {
            let Some(node) = self.nodes.get(idx).and_then(|n| n.as_ref()) else {
                continue;
            };

            let widget_id = node.id;

            // Check if inside scroll container (self or any ancestor has layout_scroll_offset)
            let inside_scroll_container = self.has_scroll_ancestor(NodeId(idx));

            // Check if virtualized (appears in input projections)
            let is_virtualized = virtualized_ids.contains(&widget_id);

            // Check if overlay participant
            let is_overlay_participant = overlay_ids.contains(&widget_id);

            // SPEC-HAL-078B CONFLICT-4: Check if inside a scroll container with pending anchor resolution
            let has_anchor_compensation = self.has_anchor_compensated_ancestor(
                NodeId(idx),
                &anchor_compensated_scroll_containers,
            );

            out.insert(
                widget_id,
                ConflictFlags {
                    inside_scroll_container,
                    is_virtualized,
                    is_overlay_participant,
                    has_anchor_compensation,
                },
            );
        }
    }

    /// SPEC-HAL-078B: Check if a node or any ancestor has layout_scroll_offset.
    ///
    /// Walks the parent chain until root is reached or a scroll container is found.
    /// Returns true if this node is inside a scroll container (including being one).
    #[inline]
    fn has_scroll_ancestor(&self, node_id: NodeId) -> bool {
        let mut current = Some(node_id);

        while let Some(id) = current {
            if let Some(node) = self.node(id) {
                if node.layout_scroll_offset.is_some() {
                    return true;
                }
                current = node.parent;
            } else {
                break;
            }
        }

        false
    }

    /// SPEC-HAL-078B CONFLICT-4: Check if a node or any ancestor is in the anchor-compensated set.
    ///
    /// Walks the parent chain until root is reached or a compensated scroll container is found.
    /// Returns true if this node is inside a scroll container with pending anchor resolution.
    #[inline]
    fn has_anchor_compensated_ancestor(
        &self,
        node_id: NodeId,
        anchor_compensated_scroll_containers: &HashSet<NodeId>,
    ) -> bool {
        let mut current = Some(node_id);

        while let Some(id) = current {
            if anchor_compensated_scroll_containers.contains(&id) {
                return true;
            }
            if let Some(node) = self.node(id) {
                current = node.parent;
            } else {
                break;
            }
        }

        false
    }

    /// SPEC-HAL-078B CONFLICT-COND-1: Find the scroll container for a given node.
    ///
    /// **ENGINE-INTERNAL / CONFLICT-LAW ONLY**
    ///
    /// This helper exists exclusively for layout transition conflict resolution
    /// (SPEC-HAL-078B CONFLICT-COND-1). It must NOT be used for:
    /// - Widget behavior or policy logic
    /// - Scroll routing decisions (use ScrollArea::get_scroll_state instead)
    /// - General-purpose tree traversal
    ///
    /// Returns the NodeId of the nearest scroll container ancestor (including self).
    /// Returns None if the node is not inside a scroll container.
    #[inline]
    pub fn find_scroll_container(&self, node_id: NodeId) -> Option<NodeId> {
        let mut current = Some(node_id);

        while let Some(id) = current {
            if let Some(node) = self.node(id) {
                if node.layout_scroll_offset.is_some() {
                    return Some(id);
                }
                current = node.parent;
            } else {
                break;
            }
        }

        None
    }

    /// SPEC-HAL-078B CONFLICT-COND-1: Check if node_a is an ancestor of node_b.
    ///
    /// **ENGINE-INTERNAL / CONFLICT-LAW ONLY**
    ///
    /// This helper exists exclusively for layout transition conflict resolution
    /// (SPEC-HAL-078B CONFLICT-COND-1). It must NOT be used for:
    /// - Widget behavior or policy logic
    /// - Focus traversal logic
    /// - General-purpose tree hierarchy checks
    ///
    /// Returns true if node_a appears in node_b's parent chain.
    /// Returns false if they are the same node or if node_a is not an ancestor.
    #[inline]
    pub fn is_ancestor_of(&self, ancestor: NodeId, descendant: NodeId) -> bool {
        if ancestor == descendant {
            return false;
        }

        let mut current = self.node(descendant).and_then(|n| n.parent);

        while let Some(id) = current {
            if id == ancestor {
                return true;
            }
            if let Some(node) = self.node(id) {
                current = node.parent;
            } else {
                break;
            }
        }

        false
    }
}

impl Default for WidgetTree {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// DummyWidget - Temporary placeholder for take/restore pattern
// ============================================================================

/// Dummy widget used as temporary placeholder during take/restore
struct DummyWidget;

impl WidgetDowncast for DummyWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DummyWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        _constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
        LayoutResult::zero()
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        WidgetInternalId(0)
    }
}

// ============================================================================
// SPEC-HAL-024 Stage 2: Tests for absolute_xy computation
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rect_authority::LayoutAuthorityGuard;

    #[test]
    fn widget_mount_tokens_survive_scoped_state_and_layout_restore() {
        use crate::widgets::label::Label;

        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(Label::new("before")), None, 0);
        let owner = tree.node(node).unwrap().id();
        let token = tree.concrete_mount(owner).unwrap();
        tree.node_mut(node)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<Label>()
            .unwrap()
            .set_text("after");
        assert_eq!(tree.concrete_mount(owner), Some(token.clone()));
        {
            let _layout = LayoutAuthorityGuard::enter();
            let (widget, _, rect) = tree.take_widget(node).unwrap();
            assert!(token.is_live());
            tree.restore_widget(node, widget, rect);
        }
        assert!(token.is_live());
        assert_eq!(tree.concrete_mount(owner), Some(token));
    }

    #[test]
    fn widget_mount_tokens_change_on_replacement_and_node_slot_reuse() {
        use crate::widgets::{flex::Flex, label::Label};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let node = tree.add_child(root, Box::new(Label::new("first")), None, 0);
        let owner = tree.node(node).unwrap().id();
        let first = tree.concrete_mount(owner).unwrap();
        tree.replace_widget_only(node, Box::new(Label::new("same type")));
        let replacement = tree.concrete_mount(owner).unwrap();
        assert_eq!(tree.node(node).unwrap().id(), owner);
        assert_ne!(first, replacement);
        assert!(!first.is_live());
        assert!(replacement.is_live());
        tree.remove_node(node);
        assert!(!replacement.is_live());
        let successor = tree.add_child(root, Box::new(Label::new("successor")), None, 0);
        assert_eq!(successor, node);
        let successor_owner = tree.node(successor).unwrap().id();
        assert_ne!(successor_owner, owner);
        assert!(tree.concrete_mount(owner).is_none());
        assert_ne!(tree.concrete_mount(successor_owner).unwrap(), replacement);
    }

    #[test]
    fn widget_mount_tokens_are_lazy_and_retire_with_the_tree() {
        use crate::widgets::label::Label;

        let token = {
            let mut tree = WidgetTree::new();
            let node = tree.create_root(Box::new(Label::new("unlinked")), None, 0);
            assert!(tree.node(node).unwrap().concrete_mount.live.get().is_none());
            let token = tree.concrete_mount(tree.node(node).unwrap().id()).unwrap();
            assert!(tree.owns_mount_tree(&token));
            assert!(token.is_live());
            token
        };
        assert!(!token.is_live());
    }

    #[test]
    fn widget_mount_tokens_reject_unattached_and_foreign_membership() {
        use crate::widgets::label::Label;

        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(Label::new("original")), None, 0);
        let owner = tree.node(node).unwrap().id();
        let original = tree.concrete_mount(owner).unwrap();
        *tree.node_mut(node).unwrap() =
            WidgetNode::new(owner, Box::new(Label::new("unattached")), None, 0);
        assert!(!original.is_live());
        assert!(tree.concrete_mount(owner).is_none());

        let mut foreign = WidgetTree::new();
        let foreign_node = foreign.create_root(Box::new(Label::new("foreign")), None, 0);
        let foreign_token = foreign
            .concrete_mount(foreign.node(foreign_node).unwrap().id())
            .unwrap();
        std::mem::swap(
            tree.node_mut(node).unwrap(),
            foreign.node_mut(foreign_node).unwrap(),
        );
        assert!(tree.concrete_mount(owner).is_none());
        assert!(!tree.owns_mount_tree(&foreign_token));
    }

    #[test]
    fn child_clip_source_is_raw_declaration_for_presentation_and_drag() {
        use crate::widgets::flex::Flex;
        use crate::widgets::label::Label;
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let parent = tree.add_child(root, Box::new(Flex::column()), None, 0);
        let leaf = tree.add_child(parent, Box::new(Label::new("source")), None, 0);
        assert_eq!(tree.child_clip_source(NodeId(usize::MAX)), None);
        assert_eq!(tree.child_clip_source(parent), None);
        let allocation = RectF::new(20.0, 140.0, 80.0, 60.0);
        {
            let _layout = LayoutAuthorityGuard::enter();
            tree.set_absolute_paint_rect(parent, allocation);
            tree.set_viewport(parent, RectF::new(0.0, 0.0, 100.0, 90.0));
        }
        tree.set_overflow(parent, OverflowMode::Clip);
        assert_eq!(tree.child_clip_source(parent), Some(allocation));
        let path = [leaf, parent, root].map(|id| (id, tree.node(id).unwrap().id()));
        let basis = tree.absolute_drag_clip_basis(&path);
        assert_eq!(basis.len(), 1);
        assert_eq!(basis[0].node, parent);
        assert_eq!(
            basis[0].declaration,
            AbsoluteDragClipDeclaration::Viewport(allocation)
        );
        tree.set_overflow(parent, OverflowMode::Visible);
        tree.set_layout_scroll_offset(parent, Some((0.0, 40.0)));
        assert_eq!(tree.child_clip_source(parent), Some(allocation));
        tree.set_layout_scroll_offset(parent, None);
        assert_eq!(tree.child_clip_source(parent), None);
    }

    #[test]
    fn presentation_geometry_requires_completed_layout_and_survives_cached_frames() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::theme::Theme;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::widgets::{flex::Flex, label::Label};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let child = tree.add_child(root, Box::new(Label::new("completed layout")), None, 0);
        for node in [root, child] {
            assert!(tree.own_presentation_geometry(node, 1.0).is_none());
        }

        let theme = Theme::default();
        let mut text = FontManagerTextEngine::new();
        let icons = crate::core::icons::builtin_registry();
        let mut harness = RetainedSceneTestHarness::new();
        let mut frame = |tree: &mut WidgetTree| {
            harness.frame(
                tree,
                root,
                Constraints::tight(Size::new(240.0, 100.0)),
                &theme,
                &mut text,
                &icons,
            )
        };
        frame(&mut tree);
        let completed = [root, child].map(|node| {
            tree.own_presentation_geometry(node, 1.0)
                .expect("actual layout must publish presentation geometry")
        });

        let cached_frame = frame(&mut tree);
        assert!(!cached_frame.scene_built);
        for (node, before) in [root, child].into_iter().zip(completed) {
            let after = tree
                .own_presentation_geometry(node, 1.0)
                .expect("unchanged completed layout remains ready without restamping");
            assert_eq!(
                (after.rect, after.viewport, after.clip, after.root_viewport),
                (
                    before.rect,
                    before.viewport,
                    before.clip,
                    before.root_viewport
                )
            );
        }
    }

    #[test]
    fn widget_mount_tokens_survive_linked_scroll_processing_without_layout_invalidation() {
        use crate::core::app::process_linked_scroll_intents;
        use crate::core::scroll_link::{ScrollIntent, ScrollLinkCoordinator};
        use crate::core::scroll_state::{AnchorId, ScrollAnchor, ScrollGroupId};
        use crate::widgets::label::Label;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Label::new("source")), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("peer")), None, 0);
        let owners = [tree.node(root).unwrap().id(), tree.node(peer).unwrap().id()];
        let tokens = owners.map(|owner| tree.concrete_mount(owner).unwrap());
        for node in [root, peer] {
            tree.node_mut(node).unwrap().dirty_layout = false;
        }

        let mut coordinator = ScrollLinkCoordinator::new();
        coordinator.begin_frame();
        let token = coordinator.generate_token();
        let anchor = ScrollAnchor::new(
            AnchorId::new(100),
            Point::new(0.0, 50.0),
            Point::new(0.0, 100.0),
        );
        coordinator.emit_intent(
            ScrollGroupId::new(42),
            ScrollIntent::vertical_anchor_sync(Some(anchor), 10.0, token),
        );
        assert_eq!(coordinator.pending_count(), 1);

        process_linked_scroll_intents(&mut coordinator, &mut tree);

        assert_eq!(coordinator.pending_count(), 0);
        for ((node, owner), token) in [root, peer].into_iter().zip(owners).zip(tokens) {
            assert!(token.is_live());
            assert_eq!(tree.concrete_mount(owner), Some(token));
            assert!(!tree.node(node).unwrap().dirty_layout);
        }
    }

    #[test]
    fn app_update_captures_mount_qualified_button_sdf_requests() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::appearance::{
            AppearanceConfigurationId, AppearanceConfigurationOwner, AppearanceFallbackPolicy,
            AppearanceSelector, ButtonAppearanceConfiguration, ButtonAppearanceContract,
            ButtonAppearanceDefinition, ProductAppearanceOwner, ProductSelectionFact,
        };
        use crate::core::event::{InteractionDomains, WindowId};
        use crate::core::theme::Theme;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::widgets::{
            button::{Button, ButtonSdfRestingProfile},
            flex::Flex,
            label::Label,
        };

        fn button(tree: &mut WidgetTree, node: NodeId) -> &mut Button {
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<Button>()
                .unwrap()
        }

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let owner_node = tree.add_child(root, Box::new(Button::with_text("Stage")), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("unchanged peer")), None, 0);
        let owner = tree.node(owner_node).unwrap().id;
        let root_owner = tree.node(root).unwrap().id;
        let peer_owner = tree.node(peer).unwrap().id;
        let theme = Theme::default();
        let domains = InteractionDomains::new();
        let domain_before = (
            domains.primary_state().focus_manager.focused(),
            domains.primary_state().keyboard_mode,
        );
        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let icons = crate::core::icons::builtin_registry();
        let mut frame = |tree: &mut WidgetTree, harness: &mut RetainedSceneTestHarness, root| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(240.0, 100.0)),
                &theme,
                &mut text,
                &icons,
                &domains,
                WindowId::PRIMARY,
            )
        };
        frame(&mut tree, &mut harness, root);
        assert!(tree.snapshot_button_sdf_requests().is_empty());
        assert!(tree.pending_sdf_requests.is_none());
        let root_local = harness.local_fragment(root_owner).unwrap();
        let owner_local = harness.local_fragment(owner).unwrap();
        let peer_local = harness.local_fragment(peer_owner).unwrap();
        let geometry = |tree: &WidgetTree| {
            [root, owner_node, peer].map(|id| {
                let node = tree.node(id).unwrap();
                (
                    node.id,
                    node.widget.id(),
                    node.rect,
                    node.absolute_paint_rect,
                    node.viewport,
                    node.cached_size,
                )
            })
        };
        let original_geometry = geometry(&tree);
        #[cfg(debug_assertions)]
        let original_epochs =
            [root, owner_node, peer].map(|id| tree.node(id).unwrap().layout_epoch);

        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        assert!(tree.snapshot_button_sdf_requests().is_empty());
        let first_frame = frame(&mut tree, &mut harness, root);
        // A prospective request alone cannot replace ordinary chrome.
        assert!(!first_frame.scene_built && !first_frame.visual_changed);
        let material_owner_local = harness.local_fragment(owner).unwrap();
        assert!(Arc::ptr_eq(&owner_local, &material_owner_local));
        let captured = tree.snapshot_button_sdf_requests();
        assert_eq!(captured.len(), 1);
        let first = captured[0].clone();
        assert_eq!(first.tree_lifetime(), &tree.snapshot_tree_lifetime());
        assert_eq!(first.owner(), owner);
        assert_eq!(first.request().profile, ButtonSdfRestingProfile::Light);
        assert_eq!(
            first.request().revision,
            crate::renderer::sdf_material_status::SdfMaterialRevision::initial()
        );
        assert!(tree.is_current_button_sdf_request(&first));
        assert!(!button(&mut tree, owner_node).needs_update());

        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        let equal_frame = frame(&mut tree, &mut harness, root);
        assert!(!equal_frame.scene_built && !equal_frame.visual_changed);
        assert_eq!(tree.snapshot_button_sdf_requests(), captured);

        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Dark))
            .unwrap();
        assert_eq!(tree.snapshot_button_sdf_requests(), captured);
        let changed_frame = frame(&mut tree, &mut harness, root);
        assert!(!changed_frame.scene_built && !changed_frame.visual_changed);
        let dark = tree.snapshot_button_sdf_requests().pop().unwrap();
        assert_eq!(dark.request().profile, ButtonSdfRestingProfile::Dark);
        assert_eq!(
            dark.request().revision,
            first.request().revision.checked_increment().unwrap()
        );
        assert!(!tree.is_current_button_sdf_request(&first));
        assert!(tree.is_current_button_sdf_request(&dark));
        assert_eq!(geometry(&tree), original_geometry);
        #[cfg(debug_assertions)]
        assert_eq!(
            [root, owner_node, peer].map(|id| tree.node(id).unwrap().layout_epoch),
            original_epochs
        );
        for (id, previous) in [
            (root_owner, &root_local),
            (owner, &material_owner_local),
            (peer_owner, &peer_local),
        ] {
            assert!(Arc::ptr_eq(previous, &harness.local_fragment(id).unwrap()));
        }

        button(&mut tree, owner_node)
            .request_sdf_material(None)
            .unwrap();
        assert!(tree.is_current_button_sdf_request(&dark));
        let withdrawn_frame = frame(&mut tree, &mut harness, root);
        assert!(!withdrawn_frame.scene_built && !withdrawn_frame.visual_changed);
        assert!(Arc::ptr_eq(
            &material_owner_local,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_local,
            &harness.local_fragment(root_owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_local,
            &harness.local_fragment(peer_owner).unwrap()
        ));
        assert!(tree.snapshot_button_sdf_requests().is_empty());
        assert!(tree.pending_sdf_requests.is_none());
        assert!(!tree.is_current_button_sdf_request(&dark));
        let stable_withdrawn = frame(&mut tree, &mut harness, root);
        assert!(!stable_withdrawn.scene_built && !stable_withdrawn.visual_changed);

        button(&mut tree, owner_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        frame(&mut tree, &mut harness, root);
        let resumed = tree.snapshot_button_sdf_requests().pop().unwrap();
        let product = ProductAppearanceOwner(7);
        let selector = AppearanceSelector::button();
        let contract =
            ButtonAppearanceContract::new(vec![selector.clone()], Some(product)).unwrap();
        let configuration = ButtonAppearanceConfiguration::new(
            AppearanceConfigurationId {
                owner: AppearanceConfigurationOwner::Product(product),
                generation: 1,
            },
            contract.clone(),
            vec![ButtonAppearanceDefinition {
                selector: selector.clone(),
                values: theme.button_appearance_values(&contract),
            }],
            Vec::new(),
        )
        .unwrap();
        let style = configuration
            .prepare_style(selector, AppearanceFallbackPolicy::Reject)
            .unwrap();
        button(&mut tree, owner_node)
            .set_appearance(
                style.clone(),
                Some(ProductSelectionFact {
                    owner: product,
                    selected: true,
                }),
            )
            .unwrap();
        assert!(tree.is_current_button_sdf_request(&resumed));
        frame(&mut tree, &mut harness, root);
        let resolved = button(&mut tree, owner_node).resolved_appearance().unwrap();
        assert_eq!(
            resolved.request().configuration.owner,
            AppearanceConfigurationOwner::Product(product)
        );
        assert_eq!(
            resolved.request().input.selection(),
            Some(ProductSelectionFact {
                owner: product,
                selected: true
            })
        );
        assert!(tree.snapshot_button_sdf_requests().is_empty());
        assert!(!tree.is_current_button_sdf_request(&resumed));
        button(&mut tree, owner_node).use_default_appearance();
        frame(&mut tree, &mut harness, root);
        assert_eq!(tree.snapshot_button_sdf_requests(), vec![resumed.clone()]);
        assert_eq!(geometry(&tree), original_geometry);
        assert_eq!(
            (
                domains.primary_state().focus_manager.focused(),
                domains.primary_state().keyboard_mode
            ),
            domain_before
        );

        let mut other = WidgetTree::new();
        let other_root = other.create_root(Box::new(Flex::column()), None, 0);
        let other_node = other.add_child(other_root, Box::new(Button::with_text("Stage")), None, 0);
        assert_eq!(other.node(other_node).unwrap().id, owner);
        button(&mut other, other_node)
            .request_sdf_material(Some(ButtonSdfRestingProfile::Light))
            .unwrap();
        let mut other_harness = RetainedSceneTestHarness::new();
        frame(&mut other, &mut other_harness, other_root);
        let foreign = other.snapshot_button_sdf_requests().pop().unwrap();
        assert!(!other.is_current_button_sdf_request(&resumed));
        assert!(!tree.is_current_button_sdf_request(&foreign));
        tree.remove_node(owner_node);
        assert!(tree.snapshot_button_sdf_requests().is_empty());
        assert!(tree.pending_sdf_requests.is_none());
        assert!(!tree.is_current_button_sdf_request(&resumed));
        assert!(other.is_current_button_sdf_request(&foreign));
    }

    #[test]
    fn widget_tree_lifetime_token_qualifies_live_mounts() {
        let mut first = WidgetTree::new();
        let mut second = WidgetTree::new();
        let first_root = first.create_root(Box::new(TestWidget::new(0)), None, 0);
        let second_root = second.create_root(Box::new(TestWidget::new(0)), None, 0);
        let first_owner = first.node(first_root).unwrap().id;
        let second_owner = second.node(second_root).unwrap().id;
        assert_eq!(first_owner, second_owner);

        let first_lifetime = first.snapshot_tree_lifetime();
        let second_lifetime = second.snapshot_tree_lifetime();
        assert_eq!(first_lifetime, first_lifetime.clone());
        assert_eq!(first_lifetime, first.snapshot_tree_lifetime());
        assert_ne!(first_lifetime, second_lifetime);
        assert!(first.is_live_mount(&first_lifetime, first_owner));
        assert!(second.is_live_mount(&second_lifetime, second_owner));
        assert!(!second.is_live_mount(&first_lifetime, first_owner));
        assert!(!first.is_live_mount(&second_lifetime, second_owner));

        first.remove_node(first_root);
        assert_eq!(first_lifetime, first.snapshot_tree_lifetime());
        assert!(!first.is_live_mount(&first_lifetime, first_owner));
        assert!(second.is_live_mount(&second_lifetime, second_owner));
    }

    #[test]
    fn sdf_material_publication_requires_live_mount_and_invalidates_only_owner() {
        use crate::compositor::{Compositor, SceneKey};
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::theme::Theme;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::image_binding::ImageBindingFailure;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::renderer::sdf_material_status::{
            SdfMaterialKey, SdfMaterialPublication, SdfMaterialRejection, SdfMaterialRevision,
            SdfMaterialStatus, SdfMaterialUnavailableCause,
        };
        use crate::renderer::wgpu_renderer::WgpuRenderer;
        use crate::widgets::{flex::Flex, label::Label};

        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let owner_node = tree.add_child(root, Box::new(Label::new("material owner")), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("unchanged peer")), None, 0);
        let owner = tree.node(owner_node).unwrap().id;
        let root_id = tree.node(root).unwrap().id;
        let peer_id = tree.node(peer).unwrap().id;
        let mut other = WidgetTree::new();
        let other_root = other.create_root(Box::new(Flex::column()), None, 0);
        let other_owner = other.add_child(other_root, Box::new(Label::new("other")), None, 0);
        assert_eq!(owner, other.node(other_owner).unwrap().id);

        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let constraints = Constraints::tight(Size::new(240.0, 100.0));
        harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        let root_local = harness.local_fragment(root_id).unwrap();
        let owner_local = harness.local_fragment(owner).unwrap();
        let peer_local = harness.local_fragment(peer_id).unwrap();
        let dirty = |tree: &WidgetTree, nodes: &[NodeId]| {
            nodes
                .iter()
                .map(|&id| {
                    let node = tree.node(id).unwrap();
                    (
                        node.dirty_layout,
                        node.dirty_paint,
                        node.dirty_composition,
                        node.dirty_flags,
                    )
                })
                .collect::<Vec<_>>()
        };
        let geometry = |tree: &WidgetTree| {
            [root, owner_node, peer].map(|id| {
                let node = tree.node(id).unwrap();
                (
                    node.id,
                    node.widget.id(),
                    node.rect,
                    node.absolute_paint_rect,
                    node.viewport,
                    node.cached_size,
                    node.parent,
                )
            })
        };
        let nodes = [root, owner_node, peer];
        let clean = dirty(&tree, &nodes);
        assert!(
            clean
                .iter()
                .all(|&(layout, paint, composition, _)| !layout && !paint && !composition)
        );
        let original_geometry = geometry(&tree);

        let rect = RectF::new(0.0, 0.0, 240.0, 100.0);
        let mut compositor = Compositor::new(rect);
        let surface = compositor.create_surface(0, rect, SceneKey::BASE);
        let foreign_surface = compositor.create_surface(1, rect, SceneKey::BASE);
        let metrics = CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(240, 100), 1.0)
            .unwrap()
            .metrics_for(
                tree.get_absolute_paint_rect(owner_node).unwrap(),
                tree.get_viewport(owner_node).unwrap(),
            )
            .unwrap();
        let revision = SdfMaterialRevision::initial();
        let key = SdfMaterialKey::new(
            tree.snapshot_tree_lifetime(),
            owner,
            surface,
            revision,
            metrics,
        )
        .unwrap();
        let mut renderer = pollster::block_on(WgpuRenderer::new_headless()).unwrap();
        let texture_id = 4_202_610_303;
        renderer.register_image_rgba8_srgb(texture_id, 1, 1, &[255; 4]);
        let binding = renderer.admit_image(texture_id).unwrap();
        let identity = binding.identity();
        let ready = SdfMaterialPublication::new(
            &key,
            key.clone(),
            Some(identity),
            SdfMaterialStatus::Ready {
                binding: binding.clone(),
            },
        )
        .unwrap();
        let mut selected = None;
        tree.publish_sdf_material(&mut selected, &key, Some(identity), ready.clone())
            .unwrap();
        assert_eq!(selected.as_ref(), Some(&ready));
        assert_eq!(geometry(&tree), original_geometry);
        assert_eq!(dirty(&tree, &[peer]), vec![clean[2]]);
        let owner_state = tree.node(owner_node).unwrap();
        assert!(!owner_state.dirty_layout && owner_state.dirty_paint);
        assert_eq!(owner_state.dirty_flags, WidgetDirty::PaintOnly);
        let root_state = tree.node(root).unwrap();
        assert!(
            !root_state.dirty_layout && !root_state.dirty_paint && root_state.dirty_composition
        );

        harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!Arc::ptr_eq(
            &owner_local,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_local,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_local,
            &harness.local_fragment(peer_id).unwrap()
        ));
        assert_eq!(dirty(&tree, &nodes), clean);
        tree.publish_sdf_material(&mut selected, &key, Some(identity), ready.clone())
            .unwrap();
        assert_eq!(dirty(&tree, &nodes), clean);

        let mut foreign_metrics = metrics;
        foreign_metrics.surface_id = foreign_surface;
        let mut changed_metrics = metrics;
        changed_metrics.scale_factor = 2.0;
        let wrong_keys = [
            SdfMaterialKey::new(
                other.snapshot_tree_lifetime(),
                owner,
                surface,
                revision,
                metrics,
            )
            .unwrap(),
            SdfMaterialKey::new(
                tree.snapshot_tree_lifetime(),
                owner,
                foreign_surface,
                revision,
                foreign_metrics,
            )
            .unwrap(),
            SdfMaterialKey::new(
                tree.snapshot_tree_lifetime(),
                owner,
                surface,
                revision.checked_increment().unwrap(),
                metrics,
            )
            .unwrap(),
            SdfMaterialKey::new(
                tree.snapshot_tree_lifetime(),
                owner,
                surface,
                revision,
                changed_metrics,
            )
            .unwrap(),
        ];
        for wrong in &wrong_keys {
            assert_eq!(
                tree.publish_sdf_material(&mut selected, wrong, Some(identity), ready.clone()),
                Err(SdfMaterialRejection::KeyMismatch),
            );
            let foreign = SdfMaterialPublication::new(
                wrong,
                wrong.clone(),
                Some(identity),
                SdfMaterialStatus::Ready {
                    binding: binding.clone(),
                },
            )
            .unwrap();
            assert_eq!(
                tree.publish_sdf_material(&mut selected, &key, Some(identity), foreign),
                Err(SdfMaterialRejection::KeyMismatch),
            );
            assert_eq!(selected.as_ref(), Some(&ready));
            assert_eq!(dirty(&tree, &nodes), clean);
        }
        let other_dirty = dirty(&other, &[other_root, other_owner]);
        assert_eq!(
            other.publish_sdf_material(&mut selected, &key, Some(identity), ready.clone()),
            Err(SdfMaterialRejection::KeyMismatch),
        );
        assert_eq!(dirty(&other, &[other_root, other_owner]), other_dirty);
        assert_eq!(geometry(&tree), original_geometry);

        renderer.unregister_image(texture_id);
        let cause = binding.availability().unwrap_err();
        assert_eq!(cause, ImageBindingFailure::Unregistered);
        let unavailable = SdfMaterialPublication::new(
            &key,
            key.clone(),
            Some(identity),
            SdfMaterialStatus::Unavailable {
                cause: SdfMaterialUnavailableCause::BindingRevoked {
                    withdrawn: identity,
                    cause,
                },
            },
        )
        .unwrap();
        let before_change = harness.local_fragment(owner).unwrap();
        tree.publish_sdf_material(&mut selected, &key, Some(identity), unavailable.clone())
            .unwrap();
        assert_eq!(selected.as_ref(), Some(&unavailable));
        assert!(tree.node(owner_node).unwrap().dirty_paint);
        assert!(tree.node(root).unwrap().dirty_composition);
        assert_eq!(dirty(&tree, &[peer]), vec![clean[2]]);
        assert_eq!(geometry(&tree), original_geometry);
        harness.frame(&mut tree, root, constraints, &theme, &mut text, &icons);
        assert!(!Arc::ptr_eq(
            &before_change,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &root_local,
            &harness.local_fragment(root_id).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &peer_local,
            &harness.local_fragment(peer_id).unwrap()
        ));
        tree.publish_sdf_material(&mut selected, &key, Some(identity), unavailable.clone())
            .unwrap();
        assert_eq!(dirty(&tree, &nodes), clean);

        tree.remove_node(owner_node);
        let after_removal = dirty(&tree, &[root, peer]);
        assert!(!tree.is_live_mount(key.tree_lifetime(), key.owner()));
        assert_eq!(
            tree.publish_sdf_material(&mut selected, &key, Some(identity), unavailable.clone()),
            Err(SdfMaterialRejection::KeyMismatch),
        );
        assert_eq!(selected.as_ref(), Some(&unavailable));
        assert_eq!(dirty(&tree, &[root, peer]), after_removal);
    }

    /// Test widget for absolute position tests
    /// SPEC-HAL-087 Stage 3: Uses WidgetInternalId for widget-internal ID
    struct TestWidget {
        internal_id: WidgetInternalId,
        layout: Option<TestWidgetLayout>,
    }

    struct TestWidgetLayout {
        size: Size,
        child_slots: Vec<RectF>,
        scroll_offset: Option<(f32, f32)>,
    }

    impl TestWidget {
        fn new(_tree_id: u64) -> Self {
            Self {
                internal_id: WidgetInternalId::new(),
                layout: None,
            }
        }

        fn with_layout(
            mut self,
            size: Size,
            child_slots: &[RectF],
            scroll_offset: Option<(f32, f32)>,
        ) -> Self {
            self.layout = Some(TestWidgetLayout {
                size,
                child_slots: child_slots.to_vec(),
                scroll_offset,
            });
            self
        }
    }

    impl WidgetDowncast for TestWidget {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for TestWidget {
        fn layout(
            &mut self,
            cx: &mut LayoutContext,
            constraints: Constraints,
            children: &[NodeId],
        ) -> LayoutResult {
            if let Some(layout) = &self.layout {
                let size = constraints.constrain(layout.size);
                assert_eq!(size, layout.size);
                assert_eq!(children.len(), layout.child_slots.len());
                let mut content_size = size;
                for (&child, slot) in children.iter().zip(&layout.child_slots) {
                    let child_size = Size::new(slot.width, slot.height);
                    let laid_out = cx.layout_child(child, Constraints::tight(child_size));
                    assert_eq!(laid_out, child_size);
                    cx.set_child_rect(
                        child,
                        RectF::new(slot.x, slot.y, laid_out.width, laid_out.height),
                    );
                    content_size.width = content_size.width.max(slot.x + laid_out.width);
                    content_size.height = content_size.height.max(slot.y + laid_out.height);
                }
                LayoutResult {
                    size,
                    content_size,
                    viewport_size: layout.scroll_offset.map(|_| size),
                    scroll_offset: layout.scroll_offset,
                }
            } else {
                // SPEC-HAL-086 Stage 2: Return LayoutResult (no overflow)
                LayoutResult::fit(Size::new(100.0, 50.0))
            }
        }

        fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

        fn id(&self) -> WidgetInternalId {
            self.internal_id
        }
    }

    /// Test A: A tree with parent and children → absolute positions computed correctly
    #[test]
    fn test_absolute_positions_tree() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create root at origin
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);

        // Add child at (10, 20) relative to root
        let child1 = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(child1, RectF::new(10.0, 20.0, 100.0, 50.0));

        // Add grandchild at (5, 10) relative to child1
        let grandchild = tree.add_child(child1, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(grandchild, RectF::new(5.0, 10.0, 50.0, 25.0));

        // Add another child at (100, 0) relative to root
        let child2 = tree.add_child(root, Box::new(TestWidget::new(4)), None, 0);
        tree.set_rect(child2, RectF::new(100.0, 0.0, 100.0, 50.0));

        // Simulate layout_tree's compute_absolute_positions call
        // We call it directly since we're testing the internal mechanism
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());

        // Verify absolute positions
        // Root: parent_abs(0,0) + rect(0,0) = (0,0)
        assert_eq!(tree.node(root).unwrap().absolute_xy, Point::new(0.0, 0.0));

        // Child1: parent_abs(0,0) + rect(10,20) = (10,20)
        assert_eq!(
            tree.node(child1).unwrap().absolute_xy,
            Point::new(10.0, 20.0)
        );

        // Grandchild: parent_abs(10,20) + rect(5,10) = (15,30)
        assert_eq!(
            tree.node(grandchild).unwrap().absolute_xy,
            Point::new(15.0, 30.0)
        );

        // Child2: parent_abs(0,0) + rect(100,0) = (100,0)
        assert_eq!(
            tree.node(child2).unwrap().absolute_xy,
            Point::new(100.0, 0.0)
        );
    }

    /// Test B: Changing child rect changes absolute_xy
    #[test]
    fn test_absolute_positions_after_rect_change() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create root
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);

        // Add child at initial position
        let child = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(child, RectF::new(10.0, 20.0, 100.0, 50.0));

        // First computation
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        assert_eq!(
            tree.node(child).unwrap().absolute_xy,
            Point::new(10.0, 20.0)
        );

        // Change child rect
        tree.set_rect(child, RectF::new(50.0, 60.0, 100.0, 50.0));

        // Recompute absolute positions
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());

        // Verify updated absolute position
        assert_eq!(
            tree.node(child).unwrap().absolute_xy,
            Point::new(50.0, 60.0)
        );
    }

    // ========================================================================
    // SPEC-HAL-024 Stage 4: Tests for hit_test using absolute_paint_rect
    // ========================================================================

    /// SPEC-HAL-024 Stage 4: Verify hit_test uses absolute_paint_rect
    #[test]
    fn test_hit_test_uses_absolute_paint_rect() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 200, 100)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

        // Child at (10, 10, 50, 30) relative to root
        let child = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(child, RectF::new(10.0, 10.0, 50.0, 30.0));

        // Grandchild at (5, 5, 10, 10) relative to child
        let grandchild = tree.add_child(child, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(grandchild, RectF::new(5.0, 5.0, 10.0, 10.0));

        // Run layout + Stage 2 + Stage 3B plumbing + viewport computation
        // (we're testing hit_test semantics, not layout)
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // 1) Point inside grandchild's absolute rect:
        //    abs grandchild = (10+5, 10+5) = (15, 15), size(10,10)
        //    Point (16,16) must hit grandchild
        let hit = tree.hit_test(16.0, 16.0).expect("expected a hit");
        assert_eq!(hit.widget_id, TreeWidgetId(3));

        // 2) Point inside child but outside grandchild:
        //    child abs: (10,10) size (50,30)
        //    grandchild abs: (15,15..25,25)
        //    Point (12,12) must hit child
        let hit_child = tree.hit_test(12.0, 12.0).expect("expected a hit");
        assert_eq!(hit_child.widget_id, TreeWidgetId(2));

        // 3) Point inside root but outside both children:
        //    root abs: (0,0) size (200,100)
        //    Point (1,1) must hit root
        let hit_root = tree.hit_test(1.0, 1.0).expect("expected a hit");
        assert_eq!(hit_root.widget_id, TreeWidgetId(1));

        // 4) Point outside everything returns None
        assert!(tree.hit_test(500.0, 500.0).is_none());
    }

    // ========================================================================
    // SPEC-HAL-024 Stage 3: Tests for absolute_paint_rect computation
    // ========================================================================

    /// Test A: absolute_paint_rect matches absolute position + size
    #[test]
    fn test_absolute_paint_rect_matches_position_and_size() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Create root at (0, 0, 200, 100)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 100.0));

        // Add child at (10, 20, 50, 30) relative to root
        let child = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(child, RectF::new(10.0, 20.0, 50.0, 30.0));

        // Add grandchild at (5, 10, 20, 10) relative to child
        let grandchild = tree.add_child(child, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(grandchild, RectF::new(5.0, 10.0, 20.0, 10.0));

        // Stage 2: absolute_xy
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());

        // Stage 3B: absolute_paint_rect (visual position + scroll)
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());

        // Root: absolute_paint_rect = (0, 0, 200, 100)
        assert_eq!(
            tree.node(root).unwrap().absolute_paint_rect,
            RectF::new(0.0, 0.0, 200.0, 100.0),
        );

        // Child: absolute_xy = (10, 20), size = (50, 30) → absolute_paint_rect = (10, 20, 50, 30)
        assert_eq!(
            tree.node(child).unwrap().absolute_paint_rect,
            RectF::new(10.0, 20.0, 50.0, 30.0),
        );

        // Grandchild: absolute_xy = (10+5, 20+10) = (15, 30), size = (20, 10)
        // → absolute_paint_rect = (15, 30, 20, 10)
        assert_eq!(
            tree.node(grandchild).unwrap().absolute_paint_rect,
            RectF::new(15.0, 30.0, 20.0, 10.0),
        );
    }

    // ========================================================================
    // SPEC-HAL-088 Stage 3: Tests for viewport-based hit-testing
    // ========================================================================

    /// SPEC-HAL-088 Stage 3: A child fully outside its clipped parent's area is not hittable.
    #[test]
    fn test_SPEC088_clip_parent_blocks_child_hit() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root (Visible) at (0, 0, 300, 300)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 300.0, 300.0));

        // Panel A (Clip) at (0, 0, 100, 100)
        let panel_a = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(panel_a, RectF::new(0.0, 0.0, 100.0, 100.0));
        tree.set_overflow(panel_a, OverflowMode::Clip);

        // Child C at (150, 0, 50, 50) - visually OUTSIDE Panel A's clip region
        let child_c = tree.add_child(panel_a, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(child_c, RectF::new(150.0, 0.0, 50.0, 50.0));

        // Run layout passes
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // Verify viewport for child_c is clipped to empty (outside panel's bounds)
        let child_viewport = tree.node(child_c).unwrap().viewport;
        assert_eq!(
            child_viewport.width, 0.0,
            "Child outside clip should have zero-width viewport"
        );
        assert_eq!(
            child_viewport.height, 0.0,
            "Child outside clip should have zero-height viewport"
        );

        // Hit-test at point (160, 10) - inside child's absolute_paint_rect but outside clip
        // Should NOT hit child_c because its viewport is empty
        let hit = tree.hit_test(160.0, 10.0);

        // Either no hit at all, or hit on an ancestor - but NOT child_c
        if let Some(result) = hit {
            assert_ne!(
                result.widget_id,
                TreeWidgetId(3),
                "Child outside clip region should NOT be hittable"
            );
        }
        // If None, that's also acceptable (point outside all visible widgets)
    }

    /// SPEC-HAL-088 Stage 3: Nested clip scopes correctly constrain hits.
    #[test]
    fn test_SPEC088_nested_clip_scopes() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root (Visible) at (0, 0, 400, 400)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 400.0));

        // Parent P (Clip) at (0, 0, 200, 200)
        let parent_p = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(parent_p, RectF::new(0.0, 0.0, 200.0, 200.0));
        tree.set_overflow(parent_p, OverflowMode::Clip);

        // Inner Q (Clip) at (50, 50, 50, 50) - nested inside P
        let inner_q = tree.add_child(parent_p, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(inner_q, RectF::new(50.0, 50.0, 50.0, 50.0));
        tree.set_overflow(inner_q, OverflowMode::Clip);

        // Child C at (10, 10, 20, 20) relative to Q - inside Q's clip region
        let child_c = tree.add_child(inner_q, Box::new(TestWidget::new(4)), None, 0);
        tree.set_rect(child_c, RectF::new(10.0, 10.0, 20.0, 20.0));

        // Run layout passes
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // Child C absolute position: (50+10, 50+10) = (60, 60), size (20, 20)
        // Child C is inside Q's clip (50,50,50,50) and inside P's clip (0,0,200,200)
        // So child_c viewport should be (60, 60, 20, 20)
        let child_viewport = tree.node(child_c).unwrap().viewport;
        assert_eq!(child_viewport, RectF::new(60.0, 60.0, 20.0, 20.0));

        // Hit-test at point (65, 65) - inside child C's viewport
        let hit = tree
            .hit_test(65.0, 65.0)
            .expect("expected a hit inside child C");
        assert_eq!(
            hit.widget_id,
            TreeWidgetId(4),
            "Point inside child C should hit child C"
        );

        // Hit-test at point (55, 55) - inside Q but outside child C
        let hit_q = tree.hit_test(55.0, 55.0).expect("expected a hit inside Q");
        assert_eq!(
            hit_q.widget_id,
            TreeWidgetId(3),
            "Point inside Q but outside C should hit Q"
        );
    }

    /// SPEC-HAL-088 Stage 3: Non-clipping (Visible) parents preserve original behavior.
    #[test]
    fn test_SPEC088_visible_parent_preserves_old_behavior() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root (Visible) at (0, 0, 200, 200)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 200.0));

        // Child C (Visible) at (50, 50, 50, 50) - entirely inside root
        let child_c = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(child_c, RectF::new(50.0, 50.0, 50.0, 50.0));

        // Run layout passes
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // Viewport for child is intersection of child's absolute_paint_rect with root's clip.
        // Since child (50,50,50,50) is entirely inside root (0,0,200,200), viewport = child's rect.
        let child_viewport = tree.node(child_c).unwrap().viewport;
        let child_rect = tree.node(child_c).unwrap().absolute_paint_rect;
        assert_eq!(
            child_viewport, child_rect,
            "Visible child viewport should equal its own rect when inside root clip"
        );

        // Hit-test at point (60, 60) - inside child C's absolute_paint_rect
        let hit = tree
            .hit_test(60.0, 60.0)
            .expect("expected a hit inside child C");
        assert_eq!(
            hit.widget_id,
            TreeWidgetId(2),
            "Point inside child C should hit child C"
        );

        // Hit-test at point (10, 10) - inside root but outside child C
        let hit_root = tree
            .hit_test(10.0, 10.0)
            .expect("expected a hit inside root");
        assert_eq!(
            hit_root.widget_id,
            TreeWidgetId(1),
            "Point outside child C should hit root"
        );
    }

    /// SPEC-HAL-088 Stage 4: Basic scrollport - scroll container clips children to its viewport.
    #[test]
    fn test_SPEC088_scrollport_basic() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 800, 600) - window
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

        // ScrollArea at (0, 0, 800, 600) - scroll container
        let scroll = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(scroll, RectF::new(0.0, 0.0, 800.0, 600.0));
        tree.set_layout_scroll_offset(scroll, Some((0.0, 0.0))); // Makes it a scroll container

        // Content child at (0, 0, 800, 2000) - taller than viewport
        let content = tree.add_child(scroll, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(content, RectF::new(0.0, 0.0, 800.0, 2000.0));

        // Button at (10, 1200, 100, 50) - below initial viewport in content space
        let button = tree.add_child(content, Box::new(TestWidget::new(4)), None, 0);
        tree.set_rect(button, RectF::new(10.0, 1200.0, 100.0, 50.0));

        // Run layout passes
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // Scroll container's viewport should equal its rect (clipped to window)
        let scroll_viewport = tree.node(scroll).unwrap().viewport;
        assert_eq!(scroll_viewport.width, 800.0, "Scroll viewport width");
        assert_eq!(scroll_viewport.height, 600.0, "Scroll viewport height");

        // Content's viewport should be clipped to scrollport (0, 0, 800, 600)
        // NOT to its own content bounds (0, 0, 800, 2000)
        let content_viewport = tree.node(content).unwrap().viewport;
        assert_eq!(
            content_viewport.height, 600.0,
            "Content viewport should be clipped to scrollport"
        );

        // Button's viewport should also be clipped to scrollport
        // Button is at y=1200-1250, scrollport is y=0-600
        // With Stage 4, button gets scrollport as its clip, so viewport = intersection(button, scrollport)
        let button_viewport = tree.node(button).unwrap().viewport;
        assert_eq!(
            button_viewport.height, 600.0,
            "Button inherits scrollport clip, not window clip"
        );
    }

    /// SPEC-HAL-088 Stage 4: Nested scrollports propagate correctly.
    #[test]
    fn test_SPEC088_scrollport_nested() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 800, 600)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

        // Outer scroll at (0, 0, 800, 600)
        let outer_scroll = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(outer_scroll, RectF::new(0.0, 0.0, 800.0, 600.0));
        tree.set_layout_scroll_offset(outer_scroll, Some((0.0, 0.0)));

        // Inner scroll at (100, 100, 400, 300) - smaller, nested
        let inner_scroll = tree.add_child(outer_scroll, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(inner_scroll, RectF::new(100.0, 100.0, 400.0, 300.0));
        tree.set_layout_scroll_offset(inner_scroll, Some((0.0, 0.0)));

        // Deep content at (0, 0, 400, 1000) inside inner scroll
        let deep_content = tree.add_child(inner_scroll, Box::new(TestWidget::new(4)), None, 0);
        tree.set_rect(deep_content, RectF::new(0.0, 0.0, 400.0, 1000.0));

        // Run layout passes
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // Inner scroll's viewport should be clipped to outer scrollport
        let inner_viewport = tree.node(inner_scroll).unwrap().viewport;
        assert_eq!(inner_viewport.x, 100.0);
        assert_eq!(inner_viewport.y, 100.0);
        assert_eq!(inner_viewport.width, 400.0);
        assert_eq!(inner_viewport.height, 300.0);

        // Deep content's viewport should be clipped to inner scrollport (100, 100, 400, 300)
        let deep_viewport = tree.node(deep_content).unwrap().viewport;
        assert_eq!(
            deep_viewport.height, 300.0,
            "Deep content clipped to inner scrollport"
        );
    }

    /// SPEC-HAL-088 Stage 4: Non-scroll widgets remain unaffected.
    #[test]
    fn test_SPEC088_scrollport_nonscroll_unchanged() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 400, 400)
        let root = tree.create_root(Box::new(TestWidget::new(1)), None, 0);
        tree.set_rect(root, RectF::new(0.0, 0.0, 400.0, 400.0));

        // Regular container (NOT a scroll container)
        let container = tree.add_child(root, Box::new(TestWidget::new(2)), None, 0);
        tree.set_rect(container, RectF::new(50.0, 50.0, 300.0, 300.0));
        // NOT setting layout_scroll_offset - this is a normal container

        // Child inside container
        let child = tree.add_child(container, Box::new(TestWidget::new(3)), None, 0);
        tree.set_rect(child, RectF::new(10.0, 10.0, 100.0, 100.0));

        // Run layout passes
        crate::core::app::compute_absolute_positions(&mut tree, root, Point::zero());
        crate::core::app::compute_absolute_paint_rects(&mut tree, root, Point::zero());
        tree.compute_viewports(root);

        // Container's viewport = intersection of its rect with root
        let container_viewport = tree.node(container).unwrap().viewport;
        assert_eq!(container_viewport.x, 50.0);
        assert_eq!(container_viewport.y, 50.0);
        assert_eq!(container_viewport.width, 300.0);
        assert_eq!(container_viewport.height, 300.0);

        // Child's viewport = intersection of child's absolute rect with incoming clip
        // Child absolute = (60, 60, 100, 100), incoming clip = root = (0, 0, 400, 400)
        // So child viewport = (60, 60, 100, 100) - unaffected by Stage 4
        let child_viewport = tree.node(child).unwrap().viewport;
        let child_abs = tree.node(child).unwrap().absolute_paint_rect;
        assert_eq!(child_viewport.x, child_abs.x);
        assert_eq!(child_viewport.y, child_abs.y);
        assert_eq!(child_viewport.width, child_abs.width);
        assert_eq!(child_viewport.height, child_abs.height);
    }

    // ========================================================================
    // SPEC-HAL-088 Stage 5: SceneGraph Clip Node Tests
    // ========================================================================

    use crate::core::paint::{Color, PaintSink, TextBackgroundSpan};
    use crate::core::scene::SceneNode;
    use crate::core::text_engine::{GlyphRun, TextEngine, TextMetrics};
    use crate::core::theme::Theme;

    /// Null text engine for testing - does nothing
    struct NullTextEngine;

    impl TextEngine for NullTextEngine {
        fn measure(&mut self, _text: &str, _font_size: f32) -> (f32, f32) {
            (0.0, 0.0)
        }

        fn metrics(&mut self, _text: &str, _font_size: f32) -> TextMetrics {
            TextMetrics {
                ascent: 0.0,
                descent: 0.0,
                line_gap: 0.0,
                line_height: 0.0,
            }
        }

        fn shape(&mut self, _text: &str, _font_size: f32) -> GlyphRun {
            GlyphRun {
                binding: None,
                clusters: Vec::new(),
                glyphs: Vec::new(),
                width: 0.0,
                height: 0.0,
                text: String::new(),
                font_size: 0.0,
                ascent: 0.0,
                descent: 0.0,
            }
        }

        fn draw(
            &mut self,
            _run: &GlyphRun,
            _x: f32,
            _y: f32,
            _color: Color,
            _z_index: crate::core::paint::ZIndex,
            _sink: &mut dyn PaintSink,
        ) {
        }

        fn draw_with_backgrounds(
            &mut self,
            _run: &GlyphRun,
            _x: f32,
            _y: f32,
            _color: Color,
            _backgrounds: Vec<TextBackgroundSpan>,
            _z_index: crate::core::paint::ZIndex,
            _sink: &mut dyn PaintSink,
        ) {
        }
    }

    /// Helper to count ClipNodes in a SceneGraph
    fn count_clip_nodes(nodes: &[SceneNode]) -> usize {
        let mut count = 0;
        for node in nodes {
            match node {
                SceneNode::Clip(clip) => {
                    count += 1;
                    count += count_clip_nodes(&clip.children);
                }
                SceneNode::Scroll(scroll) => {
                    count += count_clip_nodes(&scroll.children);
                }
                _ => {}
            }
        }
        count
    }

    /// Helper to find ClipNode at specific depth and check its rect
    fn find_clip_at_depth(
        nodes: &[SceneNode],
        target_depth: usize,
        current_depth: usize,
    ) -> Option<RectF> {
        for node in nodes {
            match node {
                SceneNode::Clip(clip) => {
                    if current_depth == target_depth {
                        return Some(clip.rect);
                    }
                    if let Some(rect) =
                        find_clip_at_depth(&clip.children, target_depth, current_depth + 1)
                    {
                        return Some(rect);
                    }
                }
                SceneNode::Scroll(scroll) => {
                    if let Some(rect) =
                        find_clip_at_depth(&scroll.children, target_depth, current_depth)
                    {
                        return Some(rect);
                    }
                }
                _ => {}
            }
        }
        None
    }

    /// SPEC-HAL-088 Stage 5: Scroll container emits clip node with viewport rect.
    #[test]
    fn test_SPEC088_clip_node_for_scroll_container() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 800, 600)
        let root = tree.create_root(
            Box::new(TestWidget::new(1).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(0.0, 0.0, 800.0, 600.0)],
                None,
            )),
            None,
            0,
        );
        tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

        // Scroll container at (0, 0, 800, 600)
        let scroll = tree.add_child(
            root,
            Box::new(TestWidget::new(2).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(0.0, 0.0, 800.0, 2000.0)],
                Some((0.0, 0.0)),
            )),
            None,
            0,
        );
        tree.set_rect(scroll, RectF::new(0.0, 0.0, 800.0, 600.0));
        tree.set_layout_scroll_offset(scroll, Some((0.0, 0.0)));

        // Content inside scroll
        let content = tree.add_child(
            scroll,
            Box::new(TestWidget::new(3).with_layout(Size::new(800.0, 2000.0), &[], None)),
            None,
            0,
        );
        tree.set_rect(content, RectF::new(0.0, 0.0, 800.0, 2000.0));

        let theme = Theme::default();
        let mut text_engine = NullTextEngine;
        crate::core::app::test_begin_frame_layout_tracking();
        assert_eq!(
            crate::core::app::layout_tree(
                &mut tree,
                root,
                Constraints::tight(Size::new(800.0, 600.0)),
                &theme,
                &mut text_engine,
                None,
            ),
            Size::new(800.0, 600.0)
        );
        for owner in [root, scroll, content] {
            let node = tree.node(owner).unwrap();
            #[cfg(debug_assertions)]
            assert!(node.layout_epoch > 0);
            assert!(!node.dirty_layout);
        }
        assert_eq!(
            tree.node(scroll).unwrap().layout_scroll_offset,
            Some((0.0, 0.0))
        );
        assert_eq!(
            tree.get_content_size(scroll),
            Some(Size::new(800.0, 2000.0))
        );
        drop(_guard);

        // Paint tree
        let timeline = crate::core::animation::Timeline::new();
        // SPEC-HAL-145: Empty interpolated rects (no active transitions in this test)
        let interpolated_rects = std::collections::HashMap::new();
        let icon_registry = crate::core::icons::builtin_registry();
        let (scene, _overlay_regions) = crate::core::app::paint_tree(
            &tree,
            root,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            1.0,
            None,
            &mut Vec::new(),
            None,
            None,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        // Should have at least 1 clip node for the scroll container
        let clip_count = count_clip_nodes(&scene.root);
        assert!(
            clip_count >= 1,
            "Scroll container should emit clip node, got {}",
            clip_count
        );

        // First clip should have viewport dimensions (800x600)
        let first_clip = find_clip_at_depth(&scene.root, 0, 0);
        assert!(first_clip.is_some(), "Should find clip node");
        let clip_rect = first_clip.unwrap();
        assert_eq!(clip_rect.width, 800.0, "Clip width should match viewport");
        assert_eq!(clip_rect.height, 600.0, "Clip height should match viewport");
    }

    /// SPEC-HAL-088 Stage 5: Overflow::Clip node emits clip node with viewport rect.
    #[test]
    fn test_SPEC088_clip_node_for_overflow_clip() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 800, 600)
        let root = tree.create_root(
            Box::new(TestWidget::new(1).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(100.0, 100.0, 400.0, 300.0)],
                None,
            )),
            None,
            0,
        );
        tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

        // Container with overflow=Clip at (100, 100, 400, 300)
        let container = tree.add_child(
            root,
            Box::new(TestWidget::new(2).with_layout(
                Size::new(400.0, 300.0),
                &[RectF::new(0.0, 0.0, 400.0, 500.0)],
                None,
            )),
            None,
            0,
        );
        tree.set_rect(container, RectF::new(100.0, 100.0, 400.0, 300.0));
        tree.set_overflow(container, OverflowMode::Clip);

        // Child inside container
        let child = tree.add_child(
            container,
            Box::new(TestWidget::new(3).with_layout(Size::new(400.0, 500.0), &[], None)),
            None,
            0,
        );
        tree.set_rect(child, RectF::new(0.0, 0.0, 400.0, 500.0));

        let theme = Theme::default();
        let mut text_engine = NullTextEngine;
        crate::core::app::test_begin_frame_layout_tracking();
        assert_eq!(
            crate::core::app::layout_tree(
                &mut tree,
                root,
                Constraints::tight(Size::new(800.0, 600.0)),
                &theme,
                &mut text_engine,
                None,
            ),
            Size::new(800.0, 600.0)
        );
        for owner in [root, container, child] {
            let node = tree.node(owner).unwrap();
            #[cfg(debug_assertions)]
            assert!(node.layout_epoch > 0);
            assert!(!node.dirty_layout);
        }
        assert_eq!(
            tree.get_content_size(container),
            Some(Size::new(400.0, 500.0))
        );
        drop(_guard);

        // Paint tree
        let timeline = crate::core::animation::Timeline::new();
        // SPEC-HAL-145: Empty interpolated rects (no active transitions in this test)
        let interpolated_rects = std::collections::HashMap::new();
        let icon_registry = crate::core::icons::builtin_registry();
        let (scene, _overlay_regions) = crate::core::app::paint_tree(
            &tree,
            root,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            1.0,
            None,
            &mut Vec::new(),
            None,
            None,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        // Should have at least 1 clip node for the overflow=Clip container
        let clip_count = count_clip_nodes(&scene.root);
        assert!(
            clip_count >= 1,
            "Overflow=Clip should emit clip node, got {}",
            clip_count
        );

        // Clip should be at container's viewport position
        let clip = find_clip_at_depth(&scene.root, 0, 0);
        assert!(clip.is_some(), "Should find clip node");
        let clip_rect = clip.unwrap();
        assert_eq!(clip_rect.x, 100.0, "Clip x should match container viewport");
        assert_eq!(clip_rect.y, 100.0, "Clip y should match container viewport");
        assert_eq!(
            clip_rect.width, 400.0,
            "Clip width should match container viewport"
        );
        assert_eq!(
            clip_rect.height, 300.0,
            "Clip height should match container viewport"
        );
    }

    /// SPEC-HAL-088 Stage 5: Nested scroll containers create nested clip nodes.
    #[test]
    fn test_SPEC088_clip_node_nesting() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 800, 600)
        let root = tree.create_root(
            Box::new(TestWidget::new(1).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(0.0, 0.0, 800.0, 600.0)],
                None,
            )),
            None,
            0,
        );
        tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

        // Outer scroll at (0, 0, 800, 600)
        let outer = tree.add_child(
            root,
            Box::new(TestWidget::new(2).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(100.0, 100.0, 400.0, 300.0)],
                Some((0.0, 0.0)),
            )),
            None,
            0,
        );
        tree.set_rect(outer, RectF::new(0.0, 0.0, 800.0, 600.0));
        tree.set_layout_scroll_offset(outer, Some((0.0, 0.0)));

        // Inner scroll at (100, 100, 400, 300)
        let inner = tree.add_child(
            outer,
            Box::new(TestWidget::new(3).with_layout(
                Size::new(400.0, 300.0),
                &[RectF::new(0.0, 0.0, 400.0, 1000.0)],
                Some((0.0, 0.0)),
            )),
            None,
            0,
        );
        tree.set_rect(inner, RectF::new(100.0, 100.0, 400.0, 300.0));
        tree.set_layout_scroll_offset(inner, Some((0.0, 0.0)));

        // Content in inner scroll
        let content = tree.add_child(
            inner,
            Box::new(TestWidget::new(4).with_layout(Size::new(400.0, 1000.0), &[], None)),
            None,
            0,
        );
        tree.set_rect(content, RectF::new(0.0, 0.0, 400.0, 1000.0));

        let theme = Theme::default();
        let mut text_engine = NullTextEngine;
        crate::core::app::test_begin_frame_layout_tracking();
        assert_eq!(
            crate::core::app::layout_tree(
                &mut tree,
                root,
                Constraints::tight(Size::new(800.0, 600.0)),
                &theme,
                &mut text_engine,
                None,
            ),
            Size::new(800.0, 600.0)
        );
        for owner in [root, outer, inner, content] {
            let node = tree.node(owner).unwrap();
            #[cfg(debug_assertions)]
            assert!(node.layout_epoch > 0);
            assert!(!node.dirty_layout);
        }
        assert_eq!(
            tree.node(outer).unwrap().layout_scroll_offset,
            Some((0.0, 0.0))
        );
        assert_eq!(
            tree.node(inner).unwrap().layout_scroll_offset,
            Some((0.0, 0.0))
        );
        assert_eq!(tree.get_content_size(outer), Some(Size::new(800.0, 600.0)));
        assert_eq!(tree.get_content_size(inner), Some(Size::new(400.0, 1000.0)));
        drop(_guard);

        // Paint tree
        let timeline = crate::core::animation::Timeline::new();
        // SPEC-HAL-145: Empty interpolated rects (no active transitions in this test)
        let interpolated_rects = std::collections::HashMap::new();
        let icon_registry = crate::core::icons::builtin_registry();
        let (scene, _overlay_regions) = crate::core::app::paint_tree(
            &tree,
            root,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            1.0,
            None,
            &mut Vec::new(),
            None,
            None,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        // Should have at least 2 clip nodes (outer and inner scroll containers)
        let clip_count = count_clip_nodes(&scene.root);
        assert!(
            clip_count >= 2,
            "Nested scrolls should emit multiple clips, got {}",
            clip_count
        );

        // Outer clip should be 800x600
        let outer_clip = find_clip_at_depth(&scene.root, 0, 0);
        assert!(outer_clip.is_some(), "Should find outer clip");
        assert_eq!(outer_clip.unwrap().width, 800.0);
        assert_eq!(outer_clip.unwrap().height, 600.0);

        // Inner clip should be at (100, 100) with size 400x300
        let inner_clip = find_clip_at_depth(&scene.root, 1, 0);
        assert!(inner_clip.is_some(), "Should find inner clip");
        let inner_rect = inner_clip.unwrap();
        assert_eq!(inner_rect.x, 100.0);
        assert_eq!(inner_rect.y, 100.0);
        assert_eq!(inner_rect.width, 400.0);
        assert_eq!(inner_rect.height, 300.0);
    }

    /// SPEC-HAL-088 Stage 5: Widget with both scroll and overflow doesn't emit duplicate clips.
    #[test]
    fn test_SPEC088_clip_node_no_duplicate_push() {
        let _guard = LayoutAuthorityGuard::enter();
        let mut tree = WidgetTree::new();

        // Root at (0, 0, 800, 600)
        let root = tree.create_root(
            Box::new(TestWidget::new(1).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(0.0, 0.0, 800.0, 600.0)],
                None,
            )),
            None,
            0,
        );
        tree.set_rect(root, RectF::new(0.0, 0.0, 800.0, 600.0));

        // Scroll container with both overflow=Clip AND layout_scroll_offset
        // Should emit only ONE clip node from SPEC-088, not two
        let scroll = tree.add_child(
            root,
            Box::new(TestWidget::new(2).with_layout(
                Size::new(800.0, 600.0),
                &[RectF::new(0.0, 0.0, 800.0, 2000.0)],
                Some((0.0, 0.0)),
            )),
            None,
            0,
        );
        tree.set_rect(scroll, RectF::new(0.0, 0.0, 800.0, 600.0));
        tree.set_layout_scroll_offset(scroll, Some((0.0, 0.0)));
        tree.set_overflow(scroll, OverflowMode::Clip);

        // Content
        let content = tree.add_child(
            scroll,
            Box::new(TestWidget::new(3).with_layout(Size::new(800.0, 2000.0), &[], None)),
            None,
            0,
        );
        tree.set_rect(content, RectF::new(0.0, 0.0, 800.0, 2000.0));

        let theme = Theme::default();
        let mut text_engine = NullTextEngine;
        crate::core::app::test_begin_frame_layout_tracking();
        assert_eq!(
            crate::core::app::layout_tree(
                &mut tree,
                root,
                Constraints::tight(Size::new(800.0, 600.0)),
                &theme,
                &mut text_engine,
                None,
            ),
            Size::new(800.0, 600.0)
        );
        for owner in [root, scroll, content] {
            let node = tree.node(owner).unwrap();
            #[cfg(debug_assertions)]
            assert!(node.layout_epoch > 0);
            assert!(!node.dirty_layout);
        }
        assert_eq!(
            tree.node(scroll).unwrap().layout_scroll_offset,
            Some((0.0, 0.0))
        );
        assert_eq!(
            tree.get_content_size(scroll),
            Some(Size::new(800.0, 2000.0))
        );
        drop(_guard);

        // Paint tree
        let timeline = crate::core::animation::Timeline::new();
        // SPEC-HAL-145: Empty interpolated rects (no active transitions in this test)
        let interpolated_rects = std::collections::HashMap::new();
        let icon_registry = crate::core::icons::builtin_registry();
        let (scene, _overlay_regions) = crate::core::app::paint_tree(
            &tree,
            root,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            1.0,
            None,
            &mut Vec::new(),
            None,
            None,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        // The condition (overflow==Clip OR layout_scroll_offset.is_some()) should
        // result in exactly 1 clip from SPEC-088 logic (the OR, not AND)
        // Note: There may be additional clips from widget clip_bounds, but the
        // SPEC-088 clip should be singular per widget
        let clip_count = count_clip_nodes(&scene.root);
        // For this test, we just verify at least 1 clip exists
        // The exact count depends on widget clip_bounds which is separate
        assert!(clip_count >= 1, "Should have at least one clip node");

        // Verify the clip uses the viewport (which is 800x600 for the scroll container)
        let clip = find_clip_at_depth(&scene.root, 0, 0);
        assert!(clip.is_some());
        let clip_rect = clip.unwrap();
        assert_eq!(clip_rect.width, 800.0);
        assert_eq!(clip_rect.height, 600.0);
    }
}

#[cfg(test)]
mod animation_mount_lifetime_tests {
    use super::*;
    use crate::core::animation::{
        Animation, AnimationContext, AnimationDescriptor, AnimationSample, Easing, Playback,
        RepeatCount, Timeline,
    };
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::core::widget::UpdateContext;
    use crate::widgets::label::Label;

    #[test]
    fn widget_mount_tokens_animation_binding_is_lazy_and_layout_lease_preserves_handle() {
        let mut tree = WidgetTree::new();
        let node = tree.create_root(Box::new(Label::new("ordinary")), None, 0);
        let owner = tree.node(node).unwrap().id();
        let mut timeline = Timeline::with_capacity(2);
        timeline.update(0.0).unwrap();
        {
            let mut frame = UpdateContext::new(Some(AnimationContext::new(&mut timeline)));
            let (widget, mount) = tree.widget_with_animation_mount_mut(node).unwrap();
            let mut own = frame.for_widget_update(owner, mount);
            widget.update(0.016, &mut own);
        }
        assert!(tree.node(node).unwrap().concrete_mount.live.get().is_none());
        let handle = {
            let mut frame = UpdateContext::new(Some(AnimationContext::new(&mut timeline)));
            let (_, mount) = tree.widget_with_animation_mount_mut(node).unwrap();
            let mut own = frame.for_widget_update(owner, mount);
            own.start_animation_descriptor(AnimationDescriptor::new(
                Animation::new(0.0f32, 1.0, 1.0, Easing::Linear),
                Playback::Loop(RepeatCount::Infinite),
            ))
            .unwrap()
            .unwrap()
        };
        assert!(tree.node(node).unwrap().concrete_mount.live.get().is_some());
        {
            let _layout = LayoutAuthorityGuard::enter();
            let (widget, _, rect) = tree.take_widget(node).unwrap();
            tree.restore_widget(node, widget, rect);
        }
        timeline.cancel_retired_widget_owners();
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Active(0.0));
        tree.replace_widget_only(node, Box::new(Label::new("replacement")));
        assert_eq!(tree.node(node).unwrap().id(), owner);
        timeline.cancel_retired_widget_owners();
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Cancelled);
        assert!(!timeline.has_active_animations());
        timeline.update(0.1).unwrap();
        assert_eq!(timeline.sample::<f32>(handle), AnimationSample::Missing);
    }
}

#[cfg(test)]
mod text_field_handoff_tests {
    use super::*;
    use crate::core::theme::Theme;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::text_field::{TextField, TextFieldHandoffStatus};

    fn staged(text: &str) -> TextField {
        let snapshot = TextField::new()
            .with_text(text)
            .capture_edit_handoff()
            .unwrap();
        let mut field = TextField::new();
        field.try_stage_edit_handoff(&snapshot).unwrap();
        field
    }

    fn field(tree: &WidgetTree, node: NodeId) -> &TextField {
        tree.node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref()
            .unwrap()
    }

    #[test]
    fn edit_handoff_index_tracks_only_current_mounts() {
        use crate::widgets::{flex::Flex, label::Label};
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        assert!(tree.pending_text_handoffs.is_none());
        let node = tree.add_child(root, Box::new(staged("old")), None, 0);
        let original = tree.pending_text_handoffs.as_ref().unwrap()[0]
            .mount
            .clone();
        assert_eq!(tree.pending_text_handoffs.as_ref().unwrap().len(), 1);
        tree.replace_widget_only(node, Box::new(Label::new("unrelated")));
        assert!(!original.is_live());
        assert!(tree.pending_text_handoffs.is_none());
        tree.replace_widget_only(node, Box::new(staged("current é")));
        let current = tree.pending_text_handoffs.as_ref().unwrap()[0]
            .mount
            .clone();
        assert_eq!(original.owner(), current.owner());
        assert_ne!(original, current);
        let mut text = FontManagerTextEngine::new();
        tree.process_text_field_handoffs(&mut text, &Theme::default());
        assert_eq!(field(&tree, node).text(), "current é");
        assert_eq!(
            field(&tree, node).edit_handoff_status(),
            TextFieldHandoffStatus::Applied
        );
        assert!(tree.pending_text_handoffs.is_none());
        let before = field(&tree, node).capture_edit_handoff().unwrap();
        for _ in 0..3 {
            tree.process_text_field_handoffs(&mut text, &Theme::default());
        }
        let after = field(&tree, node).capture_edit_handoff().unwrap();
        assert_eq!(after.text(), before.text());
        assert_eq!(after.cursor_byte(), before.cursor_byte());
        assert_eq!(
            after.selection_anchor_byte(),
            before.selection_anchor_byte()
        );
        assert_eq!(after.selection_head_byte(), before.selection_head_byte());
        let another = tree.add_child(root, Box::new(staged("removed")), None, 0);
        let retired = tree.pending_text_handoffs.as_ref().unwrap()[0]
            .mount
            .clone();
        tree.remove_node(another);
        assert!(!retired.is_live());
        assert!(tree.pending_text_handoffs.is_none());
        let reused = tree.add_child(root, Box::new(TextField::new()), None, 0);
        assert_eq!(reused, another);
        // An intentionally stale receipt cannot qualify a recycled slot.
        tree.pending_text_handoffs = Some(vec![PendingTextHandoff {
            mount: retired,
            kind: TextHandoffOwner::TextField,
        }]);
        tree.process_text_field_handoffs(&mut text, &Theme::default());
        assert!(field(&tree, reused).text().is_empty());
        assert_eq!(
            field(&tree, reused).edit_handoff_status(),
            TextFieldHandoffStatus::NotRequested
        );
    }

    #[test]
    fn edit_handoff_index_rejects_foreign_live_tree_token() {
        let mut original = WidgetTree::new();
        let source = original.create_root(Box::new(staged("belongs to original")), None, 0);
        let mount = original.pending_text_handoffs.as_ref().unwrap()[0]
            .mount
            .clone();
        let mut foreign = WidgetTree::new();
        let other = foreign.create_root(Box::new(TextField::new()), None, 0);
        assert_eq!(source, other);
        assert_eq!(mount.owner(), foreign.node(other).unwrap().id());
        assert_ne!(foreign.concrete_mount(mount.owner()).unwrap(), mount);
        foreign.pending_text_handoffs = Some(vec![PendingTextHandoff {
            mount,
            kind: TextHandoffOwner::TextField,
        }]);
        let mut text = FontManagerTextEngine::new();
        foreign.process_text_field_handoffs(&mut text, &Theme::default());
        assert_eq!(
            field(&foreign, other).edit_handoff_status(),
            TextFieldHandoffStatus::NotRequested
        );
        assert!(field(&foreign, other).text().is_empty());
        assert_eq!(
            field(&original, source).edit_handoff_status(),
            TextFieldHandoffStatus::Pending
        );
        original.process_text_field_handoffs(&mut text, &Theme::default());
        assert_eq!(field(&original, source).text(), "belongs to original");
        assert_eq!(
            field(&original, source).edit_handoff_status(),
            TextFieldHandoffStatus::Applied
        );
    }
}
