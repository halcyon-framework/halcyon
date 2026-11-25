// SPEC-HAL-296: Plot — Coordinate-Authority Rendering System
//
// Plot is a coordinate-authority rendering surface. It owns a 2D coordinate
// system (X × Y), renders data series through its own GPU pipeline via an
// offscreen texture, supports pan/zoom/interaction, and can share axes with
// other Plots for synchronized views.
//
// This module is a peer to core/, widgets/, renderer/ — not inside widgets/.

pub mod axis;
pub mod axis_authority;
pub mod color_scheme;
pub mod grid;
pub mod hit_proxy;
pub mod market_detail;
pub mod price_level;
pub mod probe;
pub mod render_pool;
mod renderer;
pub mod router;
pub mod series;
pub mod spatial;
pub mod tool;
pub mod transform;
pub mod view_state;

use renderer::{PlotRenderer, RenderParams};
use series::candle::Vertex as OverlayVertex;
use series::circle::CircleVertex;

pub use axis::{Axis, AxisKind, ScalePolicy};
pub use axis_authority::AxisAuthority;
pub use color_scheme::PlotColorScheme;
pub use market_detail::{
    LiquiditySummary, MarketDetailConfig, MarketDetailGrouping, MarketDetailLink,
    MarketDetailPriceStep, MarketDetailProjection, MarketDetailProjector, ProjectedBookLevel,
    ProjectedPriceRange, VisibleDepthScale,
};
pub use probe::Probe;
pub use render_pool::PlotRenderPool;
pub use router::PlotRouter;
pub use series::BookUpdateKind;
pub use series::Series;
pub use series::SeriesData;
pub use series::SeriesInteraction;
pub use series::area::{AreaPoint, AreaSeries, AreaStyle};
pub use series::bar::{BarPoint, BarSeries, BarStyle};
pub use series::candle::{CandleSeries, RenderCandle};
pub use series::circle::{Circle, CircleSeries, CircleStyle};
pub use series::depth::{
    BookEvent, BookLevel, BookSide, BookSideKind, ClassificationStyle, DepthBarSeries,
    DepthBarStyle, LevelClassification,
};
pub use series::histogram::{HistogramPoint, HistogramSeries, HistogramStyle};
pub use series::line::{LinePoint, LineSeries, LineStyle};
pub use spatial::{
    PlotProjectionContext, PlotProjectionIntent, PlotProjectionKey, PlotProjectionSession,
    PlotSpatialAdapter, PlotSpatialSnapshot,
};
pub use transform::{PriceTransform, Transform};
pub use view_state::ViewState;

pub use price_level::{
    PlotBezier, PlotBezierEvent, PlotCircle, PlotCircleEvent, PlotLine, PlotLineEvent,
    PlotLineStyle, PlotPath, PlotPathEvent, PlotRegion, PlotStyledPath, PlotStyledPathEvent,
    PlotTag, PriceLevel, PriceLevelEvent, PriceLevelLine, StyledSegment, TagAnchor, TagLine,
    TagSide,
};
pub use tool::{AnnotationData, GesturePhase, GestureResult, MeasureResult, PlotTool, ToolOutput};

use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::{NodeId, WidgetMountToken};
use crate::core::frame_realization::AppFrameAttemptId;
use crate::renderer::image_binding::{ImageBinding, ImageBindingIdentity, ImageBindingRequest};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc;
use crate::model::spatial::{
    SpatialOverlayAnchor, SpatialOverlayPlan, resolve_spatial_overlay_rect,
};
use crate::renderer::wgpu_renderer::GpuResources;
use crate::renderer::{
    CustomRenderActivity, CustomRenderAdmission, CustomRenderParticipant,
    CustomRenderPreparationOutcome, CustomRenderTargetMetrics, PhysicalExtent, PhysicalRect,
};
use crate::utils::id::WidgetInternalId;
use market_detail::MarketDetailReferenceLine;
use std::sync::{Arc, Mutex};

/// Plot action IDs (reserved range 2000–2099).
/// Plot action IDs — re-exports from ActionId for convenience.
/// Canonical definitions live in `runtime::actions::ActionId`.
pub mod plot_actions {
    pub use crate::runtime::actions::ActionId;

    pub const TOOL_POINTER: ActionId = ActionId::PLOT_TOOL_POINTER;
    pub const TOOL_MEASURE: ActionId = ActionId::PLOT_TOOL_MEASURE;
    pub const TOOL_HLINE: ActionId = ActionId::PLOT_TOOL_HLINE;
    pub const TOOL_RECT: ActionId = ActionId::PLOT_TOOL_RECT;
    pub const TOOL_TRENDLINE: ActionId = ActionId::PLOT_TOOL_TRENDLINE;
    pub const TOOL_FIBONACCI: ActionId = ActionId::PLOT_TOOL_FIBONACCI;

    pub const CLEAR_SELECTION: ActionId = ActionId::PLOT_CLEAR;
    pub const RESET_VIEW: ActionId = ActionId::PLOT_RESET_VIEW;
    pub const ZOOM_IN: ActionId = ActionId::PLOT_ZOOM_IN;
    pub const ZOOM_OUT: ActionId = ActionId::PLOT_ZOOM_OUT;
    pub const TOGGLE_TOOLBAR: ActionId = ActionId::PLOT_TOGGLE_TOOLBAR;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PlotColorSource {
    Theme,
    Explicit,
}

/// Register all Plot actions with the application's ActionRegistry.
///
/// Called once during App setup. All Plot actions are Widget-scoped —
/// they fire only when a Plot has focus. Tool actions (2000-2014)
/// are for capture mode (user clicked into chart). Spotlight (300)
/// is navigation-mode only.
pub fn register_plot_actions(registry: &mut crate::runtime::actions::ActionRegistry) {
    use crate::core::event::KeyCode;
    use crate::runtime::actions::{Action, ActionId, ActionScope, Shortcut};

    registry.add(Action {
        id: ActionId::PLOT_CLEAR,
        name: "Clear Selection",
        description: "Clear selection or deactivate active tool",
        category: "Chart Tools",
        shortcut: Some(Shortcut::key_only(KeyCode::Escape)),
        scope: ActionScope::Widget,
        handler: |_cx| {},
    });

    registry.add(Action {
        id: ActionId::PLOT_RESET_VIEW,
        name: "Reset View",
        description: "Reset chart to default view and re-enable auto-follow",
        category: "Chart Tools",
        shortcut: Some(Shortcut::key_only(KeyCode::Home)),
        scope: ActionScope::Widget,
        handler: |_cx| {},
    });

    registry.add(Action {
        id: ActionId::PLOT_ZOOM_IN,
        name: "Zoom In",
        description: "Zoom chart in at viewport center",
        category: "Chart Tools",
        shortcut: Some(Shortcut::key_only(KeyCode::Character('='))),
        scope: ActionScope::Widget,
        handler: |_cx| {},
    });

    registry.add(Action {
        id: ActionId::PLOT_ZOOM_OUT,
        name: "Zoom Out",
        description: "Zoom chart out from viewport center",
        category: "Chart Tools",
        shortcut: Some(Shortcut::key_only(KeyCode::Character('-'))),
        scope: ActionScope::Widget,
        handler: |_cx| {},
    });

    registry.add(Action {
        id: ActionId::PLOT_TOGGLE_TOOLBAR,
        name: "Toggle Toolbar",
        description: "Show or hide chart tool toolbar",
        category: "Chart Tools",
        shortcut: Some(Shortcut::key_only(KeyCode::Character('/'))),
        scope: ActionScope::Widget,
        handler: |_cx| {},
    });
}

/// SPEC-HAL-296: Optional bounds that constrain panning and zooming.
///
/// Prevents scrolling into empty space beyond the data range.
/// Each bound is optional — None means unconstrained on that edge.
#[derive(Debug, Clone, Default)]
pub struct DataBounds {
    /// Minimum X the viewport can reach. None = unconstrained.
    pub x_min: Option<f64>,
    /// Maximum X the viewport can reach. None = unconstrained.
    pub x_max: Option<f64>,
    /// Minimum Y the viewport can reach (e.g., 0.0 for prices). None = unconstrained.
    pub y_min: Option<f64>,
    /// Maximum Y the viewport can reach. None = unconstrained.
    pub y_max: Option<f64>,
}

/// Configuration for axis label rendering.
#[derive(Clone)]
pub struct LabelConfig {
    /// Font size for labels (logical pixels). Default 11.0.
    pub font_size: f32,
    /// Target tick count for Y axis. Default 4.
    pub y_tick_count: usize,
    /// Target tick count for X axis. Default 6.
    pub x_tick_count: usize,
    /// Custom formatter for Y axis values. None = default format_value.
    pub y_formatter:
        Option<Arc<dyn Fn(crate::plot::axis::AxisFormatContext) -> String + Send + Sync>>,
    /// Custom formatter for X axis values. None = default format_value.
    pub x_formatter:
        Option<Arc<dyn Fn(crate::plot::axis::AxisFormatContext) -> String + Send + Sync>>,
    /// Label text color.
    pub color: crate::core::paint::Color,
    /// Label background color.
    pub bg_color: crate::core::paint::Color,
}

impl Default for LabelConfig {
    fn default() -> Self {
        Self {
            font_size: 11.0,
            y_tick_count: 4,
            x_tick_count: 6,
            y_formatter: None,
            x_formatter: None,
            color: crate::core::paint::Color::from_srgb(0.8, 0.8, 0.8, 0.9),
            bg_color: crate::core::paint::Color::from_srgb(0.1, 0.1, 0.1, 0.7),
        }
    }
}

/// SPEC-HAL-296 §7.1.1: World-coordinate range from a drag-select gesture.
#[derive(Debug, Clone)]
pub struct RangeSelection {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
}

/// SPEC-HAL-296 §7.1.2: Hit on a specific data point within a Series.
pub struct ObjectHit {
    /// Index of the series in the Plot's series list.
    pub series_index: usize,
    /// Series-specific hit data (candle index, point index, etc.).
    pub hit: Box<dyn std::any::Any>,
    /// World coordinates of the hit point.
    pub world_x: f64,
    pub world_y: f64,
}

/// SPEC-HAL-296 §7.3: Selection/interaction event emitted by Plot.
pub enum PlotSelectionEvent {
    /// Object clicked/selected.
    ObjectSelected(ObjectHit),
    /// Range selection completed.
    RangeSelected(RangeSelection),
    /// All selections cleared.
    SelectionCleared,
    /// Series point drag in progress. Emitted on each Move during drag.
    SeriesDrag {
        series_index: usize,
        point_data: Box<dyn std::any::Any>,
        world_x: f64,
        world_y: f64,
    },
    /// Series point drag completed. Emitted on Up.
    SeriesDragEnd {
        series_index: usize,
        point_data: Box<dyn std::any::Any>,
        world_x: f64,
        world_y: f64,
    },
}

/// Live data update message. Send these to a Plot's data channel
/// to update series data without rebuilding the widget.
pub enum DataUpdate {
    /// Replace the last candle (working candle price tick).
    UpdateLastCandle {
        series_index: usize,
        candle: RenderCandle,
    },
    /// Append a new candle (period closed).
    AppendCandle {
        series_index: usize,
        candle: RenderCandle,
    },
    /// Replace all candles (full reset).
    ReplaceCandles {
        series_index: usize,
        candles: Vec<RenderCandle>,
    },
    /// Replace the last line point.
    UpdateLastLinePoint {
        series_index: usize,
        point: LinePoint,
    },
    /// Append a line point.
    AppendLinePoint {
        series_index: usize,
        point: LinePoint,
    },
    /// Replace all line points (e.g. EMA recompute).
    ReplaceLinePoints {
        series_index: usize,
        points: Vec<LinePoint>,
    },
    /// Replace the last bar point.
    UpdateLastBarPoint {
        series_index: usize,
        point: BarPoint,
    },
    /// Append a bar point.
    AppendBarPoint {
        series_index: usize,
        point: BarPoint,
    },
    /// Replace all bar points.
    ReplaceBarPoints {
        series_index: usize,
        points: Vec<BarPoint>,
    },
    /// Replace all circles.
    ReplaceCircles {
        series_index: usize,
        circles: Vec<Circle>,
    },
    /// Append a circle.
    AppendCircle { series_index: usize, circle: Circle },
    /// Replace the last circle.
    UpdateLastCircle { series_index: usize, circle: Circle },
    /// Add or replace a price level.
    AddPriceLevel(PriceLevel),
    /// Remove a price level by ID.
    RemovePriceLevel(u64),
    /// Update a price level's Y coordinate.
    UpdatePriceLevelY { id: u64, y: f64 },
    /// Add or replace a freestanding tag.
    AddTag(PlotTag),
    /// Remove a tag by ID.
    RemoveTag(u64),
    /// Add or replace a region.
    AddRegion(PlotRegion),
    /// Remove a region by ID.
    RemoveRegion(u64),
    /// Add or replace a circle marker.
    AddCircle(PlotCircle),
    /// Remove a circle marker by ID.
    RemoveCircle(u64),
    /// Add or replace a line marker.
    AddLine(PlotLine),
    /// Remove a line marker by ID.
    RemoveLine(u64),
    /// Add or replace a Bezier curve marker.
    AddBezier(PlotBezier),
    /// Remove a Bezier curve marker by ID.
    RemoveBezier(u64),
    /// Add or replace a styled path.
    AddStyledPath(PlotStyledPath),
    /// Remove a styled path by ID.
    RemoveStyledPath(u64),
    /// Add or replace a path marker.
    AddPath(PlotPath),
    /// Remove a path marker by ID.
    RemovePath(u64),
    /// Replace the bid side of the order book.
    BookBids { series_index: usize, bids: BookSide },
    /// Replace the ask side of the order book.
    BookAsks { series_index: usize, asks: BookSide },
    /// Replace both sides atomically.
    BookSnapshot {
        series_index: usize,
        bids: BookSide,
        asks: BookSide,
    },
}

/// Active drag state for a price level.
struct LevelDragState {
    /// ID of the level being dragged.
    level_id: u64,
    /// Original Y before drag started.
    original_y: f64,
    /// Current proposed Y (follows cursor, snapped if snap_fn set).
    current_y: f64,
}

/// Active series drag tracking.
struct SeriesDragState {
    series_index: usize,
    _initial_hit_x: f32,
    _initial_hit_y: f32,
}

#[derive(Clone, PartialEq)]
struct RenderSubmissionKey {
    state_generation: u64,
    series_data_generation: u64,
    market_reference: Option<MarketDetailReferenceLine>,
    view_state: ViewState,
    x_axis_kind: AxisKind,
    y_axis_kind: AxisKind,
    texture_width: u32,
    texture_height: u32,
    scale_factor: f32,
    crosshair_pos: Option<(f32, f32)>,
    box_selection: Option<(f32, f32, f32, f32)>,
    probe_generation: u64,
    selection_signature: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlotTargetResourceKey {
    surface_id: crate::compositor::SurfaceId,
    scale_factor_bits: u32,
    target_physical_extent: PhysicalExtent,
    physical_render_extent: PhysicalExtent,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct PlotTargetState {
    resource_key: PlotTargetResourceKey,
    physical_composition_coverage: PhysicalRect,
    texture_id: u64,
    active: bool,
}

/// A receipt candidate for the actual Plot image in one compiled target.
#[derive(Clone)]
pub(crate) struct PlotFrontWitness(Arc<PlotFrontWitnessState>);

struct PlotFrontWitnessState {
    texture_id: u64,
    current: Arc<AtomicBool>,
    required: Option<ImageBinding>,
    submitted: AtomicU64,
}

impl PartialEq for PlotFrontWitness {
    fn eq(&self, other: &Self) -> bool {
        Arc::ptr_eq(&self.0, &other.0)
    }
}
impl Eq for PlotFrontWitness {}

impl PlotFrontWitness {
    fn new(texture_id: u64, current: Arc<AtomicBool>, required: Option<ImageBinding>) -> Self {
        Self(Arc::new(PlotFrontWitnessState {
            texture_id,
            current,
            required,
            submitted: AtomicU64::new(0),
        }))
    }

    pub(crate) fn texture_id(&self) -> u64 {
        self.0.texture_id
    }

    pub(crate) fn required_binding(&self) -> Option<&ImageBinding> {
        (self.0.submitted.load(Ordering::Acquire) == 0)
            .then_some(self.0.required.as_ref())
            .flatten()
    }

    pub(crate) fn record_submitted(&self, identity: ImageBindingIdentity) {
        if self.0.current.load(Ordering::Acquire) && identity.texture_id() == self.0.texture_id {
            self.0
                .submitted
                .store(identity.generation().get(), Ordering::Release);
        }
    }

    pub(crate) fn withdraw(&self) {
        self.0.current.store(false, Ordering::Release);
    }
}

#[derive(Clone, PartialEq, Eq)]
pub(crate) struct PlotPreparationEvidence {
    mount: WidgetMountToken,
    attempt: AppFrameAttemptId,
    admission: crate::renderer::custom_render_pass::CustomRenderPresentationId,
    target: Option<PlotTargetState>,
    pending_generation: Option<u64>,
    pub(crate) front: Option<PlotFrontWitness>,
    pub(crate) local_paint_changed: bool,
    pub(crate) market_projection_changed: bool,
}

impl PlotPreparationEvidence {
    pub(crate) fn authenticates(
        &self,
        mount: &WidgetMountToken,
        admission: crate::renderer::custom_render_pass::CustomRenderPresentationId,
        metrics: &CustomRenderTargetMetrics,
    ) -> bool {
        if &self.mount != mount
            || !mount.is_live()
            || self.admission != admission
            || Some(self.attempt) != metrics.app_frame_attempt
        {
            return false;
        }
        let Some(target) = self.target else {
            return self.pending_generation.is_none() && self.front.is_none();
        };
        let key = target.resource_key;
        key.surface_id == metrics.surface_id
            && key.scale_factor_bits == metrics.scale_factor.to_bits()
            && key.target_physical_extent == metrics.target_physical_extent
            && matches!(metrics.activity, CustomRenderActivity::Active {
                physical_render_extent, physical_composition_coverage,
            } if physical_render_extent == key.physical_render_extent
                && physical_composition_coverage == target.physical_composition_coverage
                && target.active)
    }

    pub(crate) fn pending_surface(&self) -> Option<crate::compositor::SurfaceId> {
        self.pending_generation?;
        self.target.map(|target| target.resource_key.surface_id)
    }
}

struct PlotCanonicalFront {
    witness: PlotFrontWitness,
    render_generation: u64,
    admission: Option<ImageBindingRequest>,
    binding: Option<ImageBinding>,
}

impl PlotCanonicalFront {
    fn has_submitted_binding(&self) -> bool {
        self.binding.as_ref().is_some_and(|binding| {
            self.witness.0.current.load(Ordering::Acquire)
                && binding.availability().is_ok()
                && self.witness.0.submitted.load(Ordering::Acquire)
                    == binding.identity().generation().get()
        })
    }
}

struct PlotResizePending {
    target: PlotTargetState,
    current: Arc<AtomicBool>,
    completed: Option<mpsc::Receiver<Result<render_pool::CompletedRenderPublication, render_pool::ResizeRenderFailure>>>,
    completed_ready: Arc<AtomicBool>,
    publication: Option<render_pool::CompletedRenderPublication>,
    admission: Option<ImageBindingRequest>,
    failure: Option<render_pool::ResizeRenderFailure>,
}

/// SPEC-HAL-296: Coordinate-authority rendering surface.
///
/// A Plot owns a 2D coordinate system, renders data series to an offscreen
/// texture, and displays the result as an image quad in Halcyon's scene graph.
///
/// Each axis can optionally be shared with other Plots via AxisAuthority.
/// A shared Probe enables cross-widget coordinate feedback (hover chart →
/// order book highlights level).
pub struct Plot {
    id: WidgetInternalId,

    // Coordinate system
    x_axis: Axis,
    y_axis: Axis,
    view_state: ViewState,

    // Shared axis authorities (either, both, or neither)
    shared_x: Option<Arc<Mutex<AxisAuthority>>>,
    shared_y: Option<Arc<Mutex<AxisAuthority>>>,
    cached_x_generation: u64,
    cached_y_generation: u64,

    // Cross-widget coordinate probe
    probe: Option<Arc<Mutex<Probe>>>,
    market_detail_link: Option<MarketDetailLink>,
    market_detail_reference_level: Option<u64>,

    // Pan/zoom constraints
    data_bounds: DataBounds,

    // Data visualization (pending until GPU init moves them to renderer)
    pending_series: Vec<Box<dyn Series>>,

    // Widget state
    focused: bool,
    in_capture: bool,

    // Interaction state
    is_dragging: bool,
    drag_prev_pos: Option<(f32, f32)>,
    layout_rect: RectF,

    // Box zoom (right-click drag)
    box_zoom_active: bool,
    box_zoom_start: Option<(f32, f32)>,

    /// True while Space key is held. Used for Space+scroll horizontal pan.
    space_held: bool,
    /// True when user has manually positioned Y via Space+drag.
    /// Suppresses auto_scale_y until reset (Home / double-click).
    manual_y: bool,
    /// Y auto-scale padding fraction. Default 0.15 (15% each side).
    y_padding: f64,
    /// Last Space press time for double-Space → reset view detection.
    last_space_press: Option<web_time::Instant>,

    /// Series indices that are hidden (toggled off by user).
    hidden_series: std::collections::HashSet<usize>,

    // Initial X span for double-click reset
    initial_x_span: f64,

    // Selection
    pending_events: Vec<PlotSelectionEvent>,
    object_selection: Vec<ObjectHit>,
    /// Anchor position for click-vs-drag disambiguation.
    primary_down_pos: Option<(f32, f32)>,
    /// Whether Shift was held on Primary Down.
    primary_down_shift: bool,
    /// True once pointer has moved > 5px from primary_down_pos.
    drag_threshold_exceeded: bool,
    /// Active series drag state.
    series_drag: Option<SeriesDragState>,

    // GPU resources (set via with_gpu_resources)
    gpu: Option<GpuResources>,
    /// Explicit SPEC-HAL-295A admission required for WidgetTree registration.
    custom_render_admission: Option<CustomRenderAdmission>,

    /// Shared render pool.
    render_pool: Option<Arc<PlotRenderPool>>,
    /// Renderer wrapped in Arc<Mutex> for pool access.
    renderer: Option<Arc<Mutex<PlotRenderer>>>,
    /// Target-dependent resource identity supplied by SPEC-HAL-266A preparation.
    target_state: Option<PlotTargetState>,
    /// Texture identity currently safe for Image emission.
    presented_texture_id: Option<u64>,
    mounted_plot: Option<WidgetMountToken>,
    canonical_front: Option<PlotCanonicalFront>,
    resize_slot: Option<Arc<render_pool::ResizeRenderSlot>>,
    resize_pending: Option<PlotResizePending>,
    series_data_generation: u64,
    deferred_series_data: std::collections::BTreeMap<usize, SeriesData>,
    retire_when_available: bool,
    /// Worker publication consumed by Update and awaiting the target cutoff.
    completed_publication: Option<render_pool::CompletedRenderPublication>,
    /// Generation counter — bumped on any state change.
    render_generation: u64,
    /// Monotonic worker-job generation, advanced only for distinct submissions.
    submission_generation: u64,
    /// Last state submitted to the render pool.
    last_submission: Option<RenderSubmissionKey>,
    /// Hit-test proxies — main-thread data for interaction.
    hit_entries: Vec<hit_proxy::HitTestEntry>,
    /// Texture dimensions (needed on main thread for transforms).
    texture_width: u32,
    texture_height: u32,
    /// Immutable labels committed with the last published front.
    presented_labels: Option<Arc<render_pool::PresentedPlotLabels>>,
    /// Completed worker frame waiting for UI-thread texture/link presentation.
    completed_render: Arc<Mutex<Option<render_pool::CompletedRenderPublication>>>,

    // Crosshair + box selection (input state — stays on Plot)
    crosshair_pos: Option<(f32, f32)>,
    box_selection: Option<(f32, f32, f32, f32)>,

    // Offscreen texture identity
    texture_id: u64,
    gpu_initialized: bool,

    /// Axis label configuration. None = labels disabled.
    label_config: Option<LabelConfig>,

    /// Data update channel. Consumer sends DataUpdate messages.
    data_rx: Option<crossbeam_channel::Receiver<DataUpdate>>,
    /// Explicit construction-time override for the live-channel follow default.
    initial_follow_latest: Option<bool>,

    /// SPEC-HAL-299: Persistent price level markers.
    price_levels: Vec<PriceLevel>,
    /// Pending price level events.
    pending_level_events: Vec<PriceLevelEvent>,
    /// Currently hovered price level ID (for hover enter/leave).
    hovered_level: Option<u64>,
    /// Active price level drag state.
    level_drag: Option<LevelDragState>,
    /// Snap function for price level drag (quantize to tick size).
    level_snap_fn: Option<Box<dyn Fn(f64) -> f64 + Send + Sync>>,
    /// SPEC-HAL-299 §9: Freestanding tags at world coordinates.
    tags: Vec<PlotTag>,
    /// SPEC-HAL-299 §17: Highlighted zones at world coordinates.
    regions: Vec<PlotRegion>,
    /// Circle markers at world coordinates.
    circles: Vec<PlotCircle>,
    /// Pending circle events.
    pending_circle_events: Vec<PlotCircleEvent>,
    /// Currently hovered circle ID.
    hovered_circle: Option<u64>,
    /// Line markers between world-coordinate points.
    lines: Vec<PlotLine>,
    /// Pending line events.
    pending_line_events: Vec<PlotLineEvent>,
    /// Currently hovered line ID.
    hovered_line: Option<u64>,
    /// Bezier curve markers.
    beziers: Vec<PlotBezier>,
    /// Pending Bezier events.
    pending_bezier_events: Vec<PlotBezierEvent>,
    /// Currently hovered Bezier ID.
    hovered_bezier: Option<u64>,
    /// Styled path markers (temporal links).
    styled_paths: Vec<PlotStyledPath>,
    /// Pending styled path events.
    pending_styled_path_events: Vec<PlotStyledPathEvent>,
    /// Currently hovered styled path ID.
    hovered_styled_path: Option<u64>,
    /// Path markers (uniform-style polylines).
    paths: Vec<PlotPath>,
    /// Pending path events.
    pending_path_events: Vec<PlotPathEvent>,
    /// Currently hovered path ID.
    hovered_path: Option<u64>,
    /// SPEC-HAL-297: Registered tools (index 0 = Pointer, always present).
    tools: Vec<Box<dyn PlotTool>>,
    /// Index into tools Vec for the currently active tool.
    active_tool_index: usize,
    /// Whether a tool gesture is in progress (tool claimed Down).
    tool_gesture_active: bool,
    /// Pending tool outputs for consumer polling.
    pending_tool_outputs: Vec<ToolOutput>,
    /// Chart title/info lines displayed top-left. Always visible.
    chart_info: Vec<String>,
    /// Pending order book events for consumer polling.
    pending_book_events: Vec<series::depth::BookEvent>,
    /// Plot color scheme (background, grid, crosshair, etc.).
    pub color_scheme: PlotColorScheme,
    color_source: PlotColorSource,
    last_theme_scheme: PlotColorScheme,
    label_colors_follow_scheme: bool,
}

const CHART_INFO_EDGE_PADDING: f32 = 8.0;
const PRICE_LEVEL_SMOOTHING_RATE: f64 = 32.0 / 3.0;
const LIVE_FOLLOW_RIGHT_PADDING_FRACTION: f64 = 0.05;
const LIVE_FOLLOW_REARM_TOLERANCE_FRACTION: f64 = 0.05;

pub(crate) fn chart_info_overlay_rect(viewport_rect: RectF) -> RectF {
    resolve_spatial_overlay_rect(
        viewport_rect,
        SpatialOverlayPlan::anchored(
            SpatialOverlayAnchor::TopLeft,
            RectF::new(0.0, 0.0, 0.0, 0.0),
            CHART_INFO_EDGE_PADDING,
        ),
    )
}

impl Plot {
    /// Create a new Plot with default axes (linear X, linear Y).
    pub fn new() -> Self {
        let id = WidgetInternalId::new();
        Self {
            id,
            x_axis: Axis::new(AxisKind::Linear),
            y_axis: Axis::new(AxisKind::Linear),
            view_state: ViewState::new(0.0, 1.0, 0.0, 1.0),
            shared_x: None,
            shared_y: None,
            cached_x_generation: 0,
            cached_y_generation: 0,
            probe: None,
            market_detail_link: None,
            market_detail_reference_level: None,
            data_bounds: DataBounds::default(),
            pending_series: Vec::new(),
            focused: false,
            in_capture: false,
            is_dragging: false,
            drag_prev_pos: None,
            layout_rect: RectF::new(0.0, 0.0, 0.0, 0.0),
            box_zoom_active: false,
            box_zoom_start: None,
            space_held: false,
            manual_y: false,
            y_padding: 0.15,
            last_space_press: None,
            hidden_series: std::collections::HashSet::new(),
            initial_x_span: 0.0,
            pending_events: Vec::new(),
            object_selection: Vec::new(),
            primary_down_pos: None,
            primary_down_shift: false,
            drag_threshold_exceeded: false,
            series_drag: None,
            crosshair_pos: None,
            box_selection: None,
            gpu: None,
            custom_render_admission: None,
            render_pool: None,
            renderer: None,
            target_state: None,
            presented_texture_id: None,
            mounted_plot: None,
            canonical_front: None,
            resize_slot: None,
            resize_pending: None,
            series_data_generation: 0,
            deferred_series_data: Default::default(),
            retire_when_available: false,
            completed_publication: None,
            render_generation: 0,
            submission_generation: 0,
            last_submission: None,
            hit_entries: Vec::new(),
            texture_width: 0,
            texture_height: 0,
            presented_labels: None,
            completed_render: Arc::new(Mutex::new(None)),
            texture_id: id.raw(),
            gpu_initialized: false,
            label_config: Some(LabelConfig::default()),
            data_rx: None,
            initial_follow_latest: None,
            price_levels: Vec::new(),
            pending_level_events: Vec::new(),
            hovered_level: None,
            level_drag: None,
            level_snap_fn: None,
            tags: Vec::new(),
            regions: Vec::new(),
            circles: Vec::new(),
            pending_circle_events: Vec::new(),
            hovered_circle: None,
            lines: Vec::new(),
            pending_line_events: Vec::new(),
            hovered_line: None,
            beziers: Vec::new(),
            pending_bezier_events: Vec::new(),
            hovered_bezier: None,
            styled_paths: Vec::new(),
            pending_styled_path_events: Vec::new(),
            hovered_styled_path: None,
            paths: Vec::new(),
            pending_path_events: Vec::new(),
            hovered_path: None,
            tools: vec![Box::new(tool::PointerTool)],
            active_tool_index: 0,
            tool_gesture_active: false,
            pending_tool_outputs: Vec::new(),
            chart_info: Vec::new(),
            pending_book_events: Vec::new(),
            color_scheme: PlotColorScheme::default(),
            color_source: PlotColorSource::Theme,
            last_theme_scheme: PlotColorScheme::default(),
            label_colors_follow_scheme: true,
        }
    }

    /// Set the X axis configuration.
    pub fn x_axis(mut self, axis: Axis) -> Self {
        self.x_axis = axis;
        self.ensure_log_axis_domains();
        self
    }

    /// Set the Y axis configuration.
    pub fn y_axis(mut self, axis: Axis) -> Self {
        self.y_axis = axis;
        self.ensure_log_axis_domains();
        self
    }

    /// Set the initial view state (coordinate ranges).
    pub fn with_view_state(mut self, view_state: ViewState) -> Self {
        self.initial_x_span = view_state.x_span();
        self.view_state = view_state;
        if let Some(follow_latest) = self.initial_follow_latest {
            self.view_state.follow_latest = follow_latest;
        } else if self.data_rx.is_some() {
            self.view_state.follow_latest = true;
        }
        self.ensure_log_axis_domains();
        self
    }

    /// Override whether an attached live data channel starts in follow-latest mode.
    ///
    /// Live channels follow by default. Manual X navigation suspends following
    /// until the user explicitly resets the view. This builder is order-independent
    /// with [`Self::with_view_state`] and [`Self::with_data_channel`].
    pub fn with_initial_follow_latest(mut self, follow_latest: bool) -> Self {
        self.initial_follow_latest = Some(follow_latest);
        self.view_state.follow_latest = follow_latest;
        self
    }

    /// Share the X axis with other Plots via an AxisAuthority.
    pub fn with_shared_x(mut self, authority: Arc<Mutex<AxisAuthority>>) -> Self {
        self.shared_x = Some(authority);
        self
    }

    /// Share the Y axis with other Plots via an AxisAuthority.
    pub fn with_shared_y(mut self, authority: Arc<Mutex<AxisAuthority>>) -> Self {
        self.shared_y = Some(authority);
        self
    }

    /// Share a coordinate probe for cross-widget hover feedback.
    pub fn with_probe(mut self, probe: Arc<Mutex<Probe>>) -> Self {
        self.probe = Some(probe);
        self
    }

    /// Opt into Plot-owned price alignment with market-detail projections.
    pub fn with_market_detail_link(mut self, link: MarketDetailLink) -> Self {
        if let Some(previous) = self.market_detail_link.take() {
            previous.set_reference_line(None);
        }
        let price_axis = link.price_axis();
        if let Ok(mut authority) = price_axis.lock() {
            authority.set_range(self.view_state.y_min, self.view_state.y_max);
            self.cached_y_generation = authority.generation();
        }
        link.set_presented_price_range(self.view_state.y_min, self.view_state.y_max);
        self.y_axis = Axis::new(link.price_axis_kind());
        self.shared_y = Some(price_axis);
        self.probe = Some(link.probe());
        self.market_detail_link = Some(link);
        self.sync_market_detail_reference_line();
        self
    }

    /// Extend one existing Plot PriceLevel through the optional aligned market-detail surface.
    pub fn with_market_detail_reference_level(mut self, level_id: u64) -> Self {
        self.market_detail_reference_level = Some(level_id);
        self.sync_market_detail_reference_display();
        self.sync_market_detail_reference_line();
        self
    }

    /// Select or clear the PriceLevel extended through the aligned market-detail surface.
    pub fn set_market_detail_reference_level(&mut self, level_id: Option<u64>) {
        let changed = self.market_detail_reference_level != level_id;
        self.market_detail_reference_level = level_id;
        if self.sync_market_detail_reference_display() || changed {
            self.render_generation = self.render_generation.wrapping_add(1);
        }
        self.sync_market_detail_reference_line();
    }

    fn sync_market_detail_reference_display(&mut self) -> bool {
        let Some(level_id) = self.market_detail_reference_level else {
            return false;
        };
        let Some(level) = self
            .price_levels
            .iter_mut()
            .find(|level| level.id == level_id)
        else {
            return false;
        };
        if level.display_y == level.y {
            return false;
        }
        level.display_y = level.y;
        true
    }

    fn market_detail_price_level(&self) -> Option<&PriceLevel> {
        self.market_detail_reference_level
            .and_then(|id| self.price_levels.iter().find(|level| level.id == id))
    }

    fn market_detail_reference_line(&self) -> Option<MarketDetailReferenceLine> {
        self.market_detail_price_level()
            .map(|level| MarketDetailReferenceLine {
                price: level.display_y,
                color: self.resolve_default_color(
                    level.color,
                    crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.8),
                    self.color_scheme.annotation_neutral,
                ),
                line_style: level.line_style,
            })
    }

    fn sync_market_detail_reference_line(&self) {
        if self.gpu_initialized {
            return;
        }
        if let Some(link) = self.market_detail_link.as_ref() {
            link.set_reference_line(self.market_detail_reference_line());
        }
    }

    pub fn with_gpu_resources(mut self, gpu: GpuResources) -> Self {
        self.custom_render_admission = Some(CustomRenderAdmission::from_gpu_resources(&gpu));
        self.gpu = Some(gpu);
        self
    }

    pub fn with_render_pool(mut self, pool: Arc<PlotRenderPool>) -> Self {
        self.render_pool = Some(pool);
        self
    }

    /// Get the texture ID for this Plot. The consumer must register
    /// the offscreen texture with WgpuRenderer using this ID.
    pub fn texture_id(&self) -> u64 {
        self.target_state
            .map(|target| target.texture_id)
            .unwrap_or(self.texture_id)
    }

    /// Get a reference to the offscreen texture for registration.
    ///
    /// NOTE: With Phase 2b threading, the texture is owned by the worker.
    /// Registration is handled via PendingImageRegistration. This returns
    /// None; kept for API compatibility.
    pub fn offscreen_texture(&self) -> Option<&wgpu::Texture> {
        None
    }

    /// Initialize GPU resources manually.
    ///
    /// Widget usage is initialized by the post-layout custom-render preparation
    /// boundary. This method remains for explicit low-level, non-Widget usage.
    /// `width` and `height` are in physical pixels.
    pub fn init_rendering(&mut self, width: u32, height: u32) {
        self.init_gpu(width, height);
    }

    /// Render all series to the offscreen texture.
    ///
    /// Widget usage submits through the post-layout custom-render preparation
    /// boundary. This method remains for explicit low-level, non-Widget usage.
    pub fn render(&mut self) {
        let scale_factor = self.low_level_render_scale_factor();
        self.consume_completed_render();
        self.publish_completed_front();
        self.submit_render(scale_factor);
    }

    /// Register this Plot's offscreen texture with a WgpuRenderer.
    ///
    /// NOTE: With Phase 2b threading, registration is handled via
    /// PendingImageRegistration pushed by the declared lifecycle. This is
    /// a no-op; kept for API compatibility.
    pub fn register_texture(&self, _renderer: &mut crate::renderer::wgpu_renderer::WgpuRenderer) {
        // Registration is handled through the declared deferred lifecycle.
    }

    /// Set crosshair position in screen-space pixels (within the
    /// offscreen texture). None = crosshair hidden.
    pub fn set_crosshair(&mut self, pos: Option<(f32, f32)>) {
        self.crosshair_pos = pos;
    }

    /// Set box selection rectangle in screen-space pixels.
    /// None = no box selection visible.
    pub fn set_box_selection(&mut self, sel: Option<(f32, f32, f32, f32)>) {
        self.box_selection = sel;
    }

    /// Set axis label configuration. None disables labels entirely.
    pub fn with_label_config(mut self, config: Option<LabelConfig>) -> Self {
        let defaults = LabelConfig::default();
        self.label_colors_follow_scheme = config.as_ref().is_some_and(|config| {
            config.color == defaults.color && config.bg_color == defaults.bg_color
        });
        self.label_config = config;
        if self.label_colors_follow_scheme {
            if let Some(config) = &mut self.label_config {
                config.color = self.color_scheme.label_text;
                config.bg_color = self.color_scheme.label_background;
            }
        }
        self
    }

    /// Disable axis labels.
    pub fn without_labels(mut self) -> Self {
        self.label_config = None;
        self.label_colors_follow_scheme = false;
        self
    }

    /// Set Y auto-scale padding fraction. Default is 0.15 (15% each side).
    /// 0.0 = tight fit, 0.5 = very loose.
    pub fn with_y_padding(mut self, fraction: f64) -> Self {
        self.y_padding = fraction;
        self
    }

    /// Set chart info lines (instrument name, timeframe, etc.)
    /// displayed in the top-left corner. Always visible.
    pub fn with_chart_info(mut self, lines: Vec<String>) -> Self {
        self.chart_info = lines;
        self
    }

    /// Set a single chart info line.
    pub fn with_title(mut self, title: impl Into<String>) -> Self {
        self.chart_info = vec![title.into()];
        self
    }

    pub fn with_color_scheme(mut self, scheme: PlotColorScheme) -> Self {
        self.color_scheme = scheme;
        self.color_source = PlotColorSource::Explicit;
        self.apply_color_scheme_dependents();
        self
    }

    fn apply_color_scheme_dependents(&mut self) {
        if self.label_colors_follow_scheme {
            if let Some(config) = self.label_config.as_mut() {
                config.color = self.color_scheme.label_text;
                config.bg_color = self.color_scheme.label_background;
            }
        }
        for tool in &mut self.tools {
            tool.apply_color_scheme(&self.color_scheme);
        }
    }

    fn sync_color_scheme(&mut self, theme: &crate::core::theme::Theme) -> bool {
        if self.color_source == PlotColorSource::Theme
            && self.color_scheme != self.last_theme_scheme
        {
            self.color_source = PlotColorSource::Explicit;
            self.apply_color_scheme_dependents();
            self.render_generation += 1;
            return true;
        }

        if self.color_source == PlotColorSource::Explicit {
            return false;
        }

        let next = PlotColorScheme::from_theme(theme);
        if next == self.color_scheme {
            return false;
        }

        self.color_scheme = next;
        self.last_theme_scheme = next;
        self.apply_color_scheme_dependents();
        self.render_generation += 1;
        true
    }

    fn resolve_default_color(
        &self,
        color: crate::core::paint::Color,
        legacy_default: crate::core::paint::Color,
        themed_default: crate::core::paint::Color,
    ) -> crate::core::paint::Color {
        if color == legacy_default {
            themed_default
        } else {
            color
        }
    }

    #[cfg(test)]
    pub(crate) fn render_generation_for_test(&self) -> u64 {
        self.render_generation
    }

    #[cfg(test)]
    pub(crate) fn render_submission_for_test(&self) -> Option<(f32, u64)> {
        self.last_submission
            .as_ref()
            .map(|submission| (submission.scale_factor, self.submission_generation))
    }

    #[cfg(test)]
    pub(crate) fn target_resource_for_test(
        &self,
    ) -> Option<(crate::compositor::SurfaceId, PhysicalExtent, u32, bool)> {
        self.target_state.map(|target| {
            (
                target.resource_key.surface_id,
                target.resource_key.physical_render_extent,
                target.resource_key.scale_factor_bits,
                target.active,
            )
        })
    }

    #[cfg(test)]
    pub(crate) fn presented_texture_id_for_test(&self) -> Option<u64> {
        self.presented_texture_id
    }

    #[cfg(test)]
    pub(crate) fn plot_worker_ready_for_test(&self) -> bool {
        self.resize_pending.as_ref().map_or_else(
            || self.completed_render.try_lock().is_ok_and(|ready| ready.is_some()),
            |pending| pending.completed_ready.load(Ordering::Acquire),
        )
    }

    #[cfg(test)]
    pub(crate) fn plot_pending_signals_for_test(&self) -> (Arc<AtomicBool>, Arc<AtomicBool>) {
        let pending = self.resize_pending.as_ref().expect("accepted replacement");
        (pending.current.clone(), pending.completed_ready.clone())
    }

    #[cfg(test)]
    pub(crate) fn plot_submission_generation_for_test(&self) -> u64 {
        self.submission_generation
    }

    #[cfg(test)]
    pub(crate) fn set_render_pool_for_test(&mut self, pool: Arc<PlotRenderPool>) {
        self.render_pool = Some(pool);
    }

    #[cfg(test)]
    pub(crate) fn price_level_display_y_for_test(&self, id: u64) -> Option<f64> {
        self.price_levels
            .iter()
            .find(|level| level.id == id)
            .map(|level| level.display_y)
    }

    #[cfg(test)]
    pub(crate) fn label_colors_for_test(
        &self,
    ) -> Option<(crate::core::paint::Color, crate::core::paint::Color)> {
        self.label_config
            .as_ref()
            .map(|config| (config.color, config.bg_color))
    }

    /// Auto-scale Y axis to fit visible data across all series.
    ///
    /// Queries each series for its Y range in the current X viewport,
    /// takes the union, and fits ViewState Y to that range.
    pub fn auto_scale_y(&mut self, padding: f64) {
        if self.manual_y {
            return;
        }
        let mut lo = f64::INFINITY;
        let mut hi = f64::NEG_INFINITY;
        let mut found = false;

        for entry in &self.hit_entries {
            if !entry.meta.contributes_to_y_autoscale {
                continue;
            }
            if let Some((s_lo, s_hi)) =
                entry.data_y_range(self.view_state.x_min, self.view_state.x_max)
            {
                if self.y_axis.kind == AxisKind::Log {
                    if s_hi <= 0.0 {
                        continue;
                    }
                    lo = lo.min(if s_lo > 0.0 { s_lo } else { s_hi });
                    hi = hi.max(s_hi);
                    found = true;
                } else {
                    lo = lo.min(s_lo);
                    hi = hi.max(s_hi);
                    found = true;
                }
            }
        }

        if found {
            self.view_state.fit_y_to_range(lo, hi, padding);
            self.ensure_log_axis_domains();
        }
    }

    /// SPEC-HAL-296 §7.3: Take pending selection events (drains queue).
    pub fn take_selection_events(&mut self) -> Vec<PlotSelectionEvent> {
        std::mem::take(&mut self.pending_events)
    }

    /// SPEC-HAL-296 §7.3: Current object selection set.
    pub fn selection(&self) -> &[ObjectHit] {
        &self.object_selection
    }

    /// SPEC-HAL-296 §7.3: Clear all selections.
    pub fn clear_selection(&mut self) {
        if !self.object_selection.is_empty() {
            self.object_selection.clear();
            self.pending_events
                .push(PlotSelectionEvent::SelectionCleared);
        }
    }

    /// Set data bounds to constrain panning.
    ///
    /// Prevents scrolling past the edge of data into empty space.
    pub fn with_data_bounds(mut self, bounds: DataBounds) -> Self {
        self.data_bounds = bounds;
        self
    }

    /// Add a data visualization series.
    pub fn add_series(mut self, series: Box<dyn Series>) -> Self {
        // Main-thread hit metadata must exist before the first target-dependent
        // GPU preparation, which may be delayed or remain inactive.
        self.hit_entries.push(hit_proxy::HitTestEntry {
            proxy: series.hit_proxy(),
            meta: hit_proxy::SeriesMeta {
                interaction: series.interaction(),
                z_order: series.z_order(),
                contributes_to_y_autoscale: series.contributes_to_y_autoscale(),
            },
        });
        self.pending_series.push(series);
        self
    }

    /// Convenience: add a CandleSeries from candle data.
    pub fn with_candles(self, candles: Vec<RenderCandle>) -> Self {
        let mut series = CandleSeries::new();
        series.set_candles(candles);
        self.add_series(Box::new(series))
    }

    /// Add a circle series (markers, dots, signal indicators).
    pub fn with_circles(self, style: CircleStyle) -> Self {
        let series = CircleSeries::new().with_style(style);
        self.add_series(Box::new(series))
    }

    /// Convenience: add a volume bar overlay from candle data.
    ///
    /// `viewport_fraction` controls the height as a fraction of the
    /// viewport (e.g. 0.12 = 12% of chart height). Volume bars are
    /// color-coded by buy/sell ratio: green (bullish) → grey (neutral)
    /// → red (bearish), biased by candle direction.
    ///
    /// Z-order is 50. Auto-scale Y is disabled (viewport_fraction
    /// controls height).
    pub fn with_volume_overlay(self, candles: &[RenderCandle], viewport_fraction: f32) -> Self {
        let (bars, ratios): (Vec<BarPoint>, Vec<f32>) = candles
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let idx = i as u64;

                let mut z1 = idx
                    .wrapping_mul(0x9E3779B97F4A7C15)
                    .wrapping_add(0x6A09E667F3BCC908);
                z1 = (z1 ^ (z1 >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                z1 = (z1 ^ (z1 >> 27)).wrapping_mul(0x94D049BB133111EB);
                z1 = z1 ^ (z1 >> 31);

                let mut z2 = idx
                    .wrapping_mul(0x517CC1B727220A95)
                    .wrapping_add(0x9E3779B97F4A7C15);
                z2 = (z2 ^ (z2 >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
                z2 = (z2 ^ (z2 >> 27)).wrapping_mul(0x94D049BB133111EB);
                z2 = z2 ^ (z2 >> 31);

                let rand_vol = (z1 % 1000000) as f64 / 1000000.0;
                let rand_ratio = (z2 % 1000000) as f64 / 1000000.0;

                let vol = 0.1 + rand_vol * 0.9;
                let buy_ratio = if c.is_up {
                    0.4 + rand_ratio * 0.5
                } else {
                    0.1 + rand_ratio * 0.5
                };

                let mid_x = (c.x_open + c.x_close) / 2.0;
                (
                    BarPoint {
                        x: mid_x,
                        y: vol,
                        color: None,
                    },
                    buy_ratio as f32,
                )
            })
            .unzip();

        let mut vol_series = BarSeries::new()
            .with_viewport_fraction(viewport_fraction)
            .with_volume_ratios(ratios)
            .with_z_order(50)
            .with_auto_scale_y(false);
        vol_series.set_points(bars);
        self.add_series(Box::new(vol_series))
    }

    /// Create a data channel for live updates.
    /// Returns the Plot (for chaining) and a Sender for pushing updates.
    pub fn with_data_channel(mut self) -> (Self, crossbeam_channel::Sender<DataUpdate>) {
        let (tx, rx) = crossbeam_channel::unbounded();
        self.data_rx = Some(rx);
        self.view_state.follow_latest = self.initial_follow_latest.unwrap_or(true);
        (self, tx)
    }

    /// Builder: add a price level during construction.
    pub fn with_price_level(mut self, mut level: PriceLevel) -> Self {
        level.display_y = level.y;
        self.price_levels.push(level);
        self.sync_market_detail_reference_line();
        self
    }

    /// SPEC-HAL-299: Add a price level. Replaces if same ID exists.
    pub fn add_price_level(&mut self, mut level: PriceLevel) {
        level.display_y = level.y;
        if let Some(existing) = self.price_levels.iter_mut().find(|l| l.id == level.id) {
            let prev_display = existing.display_y;
            *existing = level;
            existing.display_y = prev_display;
        } else {
            self.price_levels.push(level);
        }
        self.sync_market_detail_reference_display();
        self.sync_market_detail_reference_line();
        self.render_generation += 1;
    }

    /// Remove a price level by ID.
    pub fn remove_price_level(&mut self, id: u64) {
        self.price_levels.retain(|l| l.id != id);
        self.sync_market_detail_reference_line();
        self.render_generation += 1;
    }

    /// Update a price level. Calls closure if found.
    pub fn update_price_level(&mut self, id: u64, f: impl FnOnce(&mut PriceLevel)) {
        if let Some(level) = self.price_levels.iter_mut().find(|l| l.id == id) {
            f(level);
            if self.market_detail_reference_level == Some(id) {
                level.display_y = level.y;
            }
            self.sync_market_detail_reference_line();
            self.render_generation += 1;
        }
    }

    /// Remove all price levels in a group.
    pub fn remove_price_level_group(&mut self, group: &str) {
        self.price_levels
            .retain(|l| l.group.as_deref() != Some(group));
        self.sync_market_detail_reference_line();
        self.render_generation += 1;
    }

    /// Remove all price levels.
    pub fn clear_price_levels(&mut self) {
        self.price_levels.clear();
        self.sync_market_detail_reference_line();
        self.render_generation += 1;
    }

    /// Get pending price level events (drains queue).
    pub fn take_price_level_events(&mut self) -> Vec<PriceLevelEvent> {
        std::mem::take(&mut self.pending_level_events)
    }

    /// Set a snap function for price level dragging.
    pub fn set_price_level_snap(&mut self, f: impl Fn(f64) -> f64 + Send + Sync + 'static) {
        self.level_snap_fn = Some(Box::new(f));
    }

    /// Builder: set a snap function for price level dragging.
    pub fn with_price_level_snap(mut self, f: impl Fn(f64) -> f64 + Send + Sync + 'static) -> Self {
        self.level_snap_fn = Some(Box::new(f));
        self
    }

    /// Add a freestanding tag at world coordinates.
    pub fn add_tag(&mut self, tag: PlotTag) {
        if let Some(existing) = self.tags.iter_mut().find(|t| t.id == tag.id) {
            *existing = tag;
        } else {
            self.tags.push(tag);
        }
        self.render_generation += 1;
    }

    /// Remove a tag by ID.
    pub fn remove_tag(&mut self, id: u64) {
        self.tags.retain(|t| t.id != id);
        self.render_generation += 1;
    }

    /// Update a tag.
    pub fn update_tag(&mut self, id: u64, f: impl FnOnce(&mut PlotTag)) {
        if let Some(tag) = self.tags.iter_mut().find(|t| t.id == id) {
            f(tag);
            self.render_generation += 1;
        }
    }

    /// Remove all tags.
    pub fn clear_tags(&mut self) {
        self.tags.clear();
        self.render_generation += 1;
    }

    /// Builder: add a tag.
    pub fn with_tag(mut self, tag: PlotTag) -> Self {
        self.tags.push(tag);
        self
    }

    /// Add a region. Replaces if same ID exists.
    pub fn add_region(&mut self, region: PlotRegion) {
        if let Some(existing) = self.regions.iter_mut().find(|r| r.id == region.id) {
            *existing = region;
        } else {
            self.regions.push(region);
        }
        self.render_generation += 1;
    }

    /// Remove a region by ID.
    pub fn remove_region(&mut self, id: u64) {
        self.regions.retain(|r| r.id != id);
        self.render_generation += 1;
    }

    /// Update a region.
    pub fn update_region(&mut self, id: u64, f: impl FnOnce(&mut PlotRegion)) {
        if let Some(region) = self.regions.iter_mut().find(|r| r.id == id) {
            f(region);
            self.render_generation += 1;
        }
    }

    /// Remove all regions in a group.
    pub fn remove_region_group(&mut self, group: &str) {
        self.regions.retain(|r| r.group.as_deref() != Some(group));
        self.render_generation += 1;
    }

    /// Remove all regions.
    pub fn clear_regions(&mut self) {
        self.regions.clear();
        self.render_generation += 1;
    }

    /// Builder: add a region.
    pub fn with_region(mut self, region: PlotRegion) -> Self {
        self.regions.push(region);
        self
    }

    pub fn with_circle(mut self, circle: PlotCircle) -> Self {
        self.circles.push(circle);
        self
    }

    pub fn add_circle(&mut self, circle: PlotCircle) {
        if let Some(existing) = self.circles.iter_mut().find(|c| c.id == circle.id) {
            *existing = circle;
        } else {
            self.circles.push(circle);
        }
        self.render_generation += 1;
    }

    pub fn remove_circle(&mut self, id: u64) {
        self.circles.retain(|c| c.id != id);
        self.render_generation += 1;
    }

    pub fn remove_circle_group(&mut self, group: &str) {
        self.circles.retain(|c| c.group.as_deref() != Some(group));
        self.render_generation += 1;
    }

    pub fn clear_circles(&mut self) {
        self.circles.clear();
        self.render_generation += 1;
    }

    pub fn take_circle_events(&mut self) -> Vec<PlotCircleEvent> {
        std::mem::take(&mut self.pending_circle_events)
    }

    pub fn with_line(mut self, line: PlotLine) -> Self {
        self.lines.push(line);
        self
    }

    pub fn add_line(&mut self, line: PlotLine) {
        if let Some(existing) = self.lines.iter_mut().find(|l| l.id == line.id) {
            *existing = line;
        } else {
            self.lines.push(line);
        }
        self.render_generation += 1;
    }

    pub fn remove_line(&mut self, id: u64) {
        self.lines.retain(|l| l.id != id);
        self.render_generation += 1;
    }

    pub fn remove_line_group(&mut self, group: &str) {
        self.lines.retain(|l| l.group.as_deref() != Some(group));
        self.render_generation += 1;
    }

    pub fn clear_lines(&mut self) {
        self.lines.clear();
        self.render_generation += 1;
    }

    pub fn take_line_events(&mut self) -> Vec<PlotLineEvent> {
        std::mem::take(&mut self.pending_line_events)
    }

    pub fn with_bezier(mut self, bezier: PlotBezier) -> Self {
        self.beziers.push(bezier);
        self
    }

    pub fn add_bezier(&mut self, bezier: PlotBezier) {
        if let Some(existing) = self.beziers.iter_mut().find(|b| b.id == bezier.id) {
            *existing = bezier;
        } else {
            self.beziers.push(bezier);
        }
        self.render_generation += 1;
    }

    pub fn remove_bezier(&mut self, id: u64) {
        self.beziers.retain(|b| b.id != id);
        self.render_generation += 1;
    }

    pub fn remove_bezier_group(&mut self, group: &str) {
        self.beziers.retain(|b| b.group.as_deref() != Some(group));
        self.render_generation += 1;
    }

    pub fn clear_beziers(&mut self) {
        self.beziers.clear();
        self.render_generation += 1;
    }

    pub fn take_bezier_events(&mut self) -> Vec<PlotBezierEvent> {
        std::mem::take(&mut self.pending_bezier_events)
    }

    pub fn with_styled_path(mut self, path: PlotStyledPath) -> Self {
        self.styled_paths.push(path);
        self
    }

    pub fn add_styled_path(&mut self, path: PlotStyledPath) {
        if let Some(existing) = self.styled_paths.iter_mut().find(|p| p.id == path.id) {
            *existing = path;
        } else {
            self.styled_paths.push(path);
        }
        self.render_generation += 1;
    }

    pub fn remove_styled_path(&mut self, id: u64) {
        self.styled_paths.retain(|p| p.id != id);
        self.render_generation += 1;
    }

    pub fn remove_styled_path_group(&mut self, group: &str) {
        self.styled_paths
            .retain(|p| p.group.as_deref() != Some(group));
        self.render_generation += 1;
    }

    pub fn clear_styled_paths(&mut self) {
        self.styled_paths.clear();
        self.render_generation += 1;
    }

    pub fn take_styled_path_events(&mut self) -> Vec<PlotStyledPathEvent> {
        std::mem::take(&mut self.pending_styled_path_events)
    }

    pub fn with_path(mut self, path: PlotPath) -> Self {
        self.paths.push(path);
        self
    }

    pub fn add_path(&mut self, path: PlotPath) {
        if let Some(existing) = self.paths.iter_mut().find(|p| p.id == path.id) {
            *existing = path;
        } else {
            self.paths.push(path);
        }
        self.render_generation += 1;
    }

    pub fn remove_path(&mut self, id: u64) {
        self.paths.retain(|p| p.id != id);
        self.render_generation += 1;
    }

    pub fn remove_path_group(&mut self, group: &str) {
        self.paths.retain(|p| p.group.as_deref() != Some(group));
        self.render_generation += 1;
    }

    pub fn clear_paths(&mut self) {
        self.paths.clear();
        self.render_generation += 1;
    }

    pub fn take_path_events(&mut self) -> Vec<PlotPathEvent> {
        std::mem::take(&mut self.pending_path_events)
    }

    /// Screen-space bounding rects of visible chart elements.
    /// Used by PlotRouter for obstacle avoidance.
    /// Includes: circle markers, tag backgrounds, price level tag areas.
    /// Excludes: grid lines, crosshair, axis labels.
    pub fn visible_element_rects(&self) -> Vec<RectF> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return Vec::new();
        }
        let transform = self.transform(w, h);
        let mut rects = Vec::new();

        // Circle markers
        for c in &self.circles {
            let px = transform.x_to_px(c.x);
            let py = transform.y_to_px(c.y);
            let r = c.radius;
            if px + r >= 0.0 && px - r <= w && py + r >= 0.0 && py - r <= h {
                rects.push(RectF::new(px - r, py - r, r * 2.0, r * 2.0));
            }
        }

        // Price level tag areas (approximate)
        let tag_w = 80.0;
        let line_height = 12.0;
        let padding_y = 3.0;
        for level in &self.price_levels {
            let y_px = transform.y_to_px(level.display_y);
            if !level.tag.is_empty() && y_px >= 0.0 && y_px <= h {
                let t_h = level.tag.len() as f32 * line_height + padding_y * 2.0;
                let tx = if matches!(level.tag_side, TagSide::Right) {
                    w - tag_w
                } else {
                    0.0
                };
                rects.push(RectF::new(tx, y_px - t_h / 2.0, tag_w, t_h));
            }
        }

        // Freestanding tags
        for tag in &self.tags {
            let px = transform.x_to_px(tag.x);
            let py = transform.y_to_px(tag.y);
            if px >= 0.0 && px <= w && py >= 0.0 && py <= h {
                let t_h = tag.tag.len() as f32 * line_height + padding_y * 2.0;
                rects.push(RectF::new(px - tag_w / 2.0, py - t_h, tag_w, t_h));
            }
        }

        rects
    }

    /// Register a tool. Returns self for builder chaining.
    pub fn with_tool(mut self, tool: Box<dyn PlotTool>) -> Self {
        let mut tool = tool;
        tool.apply_color_scheme(&self.color_scheme);
        self.tools.push(tool);
        self
    }

    /// Activate a tool by ID string. No-op if not found.
    pub fn activate_tool(&mut self, id: &str) {
        if let Some(idx) = self.tools.iter().position(|t| t.id() == id) {
            if self.active_tool_index != idx {
                self.tools[self.active_tool_index].deactivate();
                self.active_tool_index = idx;
                self.tool_gesture_active = false;
            }
        }
    }

    /// Get pending tool outputs (drains queue).
    pub fn take_tool_outputs(&mut self) -> Vec<ToolOutput> {
        std::mem::take(&mut self.pending_tool_outputs)
    }

    /// Get pending order book events (drains queue).
    pub fn take_book_events(&mut self) -> Vec<series::depth::BookEvent> {
        std::mem::take(&mut self.pending_book_events)
    }

    /// Get the active tool's ID.
    pub fn active_tool_id(&self) -> &str {
        self.tools[self.active_tool_index].id()
    }

    /// Push new data to a series. Updates both the main-thread hit
    /// proxy and the worker's Series data.
    pub fn update_series_data(&mut self, series_index: usize, data: series::SeriesData) {
        let renderer = self.renderer.clone();
        let mut renderer = match renderer.as_ref().map(|renderer| renderer.try_lock()) {
            Some(Ok(renderer)) => Some(renderer),
            Some(Err(std::sync::TryLockError::WouldBlock)) => {
                if series_index < self.hit_entries.len()
                    && !matches!(data, SeriesData::BookUpdate(_))
                {
                    self.deferred_series_data.insert(series_index, data);
                }
                return;
            }
            Some(Err(std::sync::TryLockError::Poisoned(_))) | None => None,
        };
        self.deferred_series_data.remove(&series_index);
        self.apply_series_data(series_index, data, renderer.as_deref_mut());
        if let Some(renderer) = renderer {
            self.series_data_generation = renderer.series_data_generation();
        }
    }

    fn apply_series_data(
        &mut self,
        series_index: usize,
        data: SeriesData,
        renderer: Option<&mut PlotRenderer>,
    ) {
        let series = if let Some(entry) = self.hit_entries.get_mut(series_index) {
            entry.proxy = match &data {
                series::SeriesData::Candles(c) => {
                    hit_proxy::HitProxy::Candle { candles: c.clone() }
                }
                series::SeriesData::Lines(p) => hit_proxy::HitProxy::Line { points: p.clone() },
                series::SeriesData::Bars(p) => hit_proxy::HitProxy::Bar { points: p.clone() },
                series::SeriesData::Circles(p) => hit_proxy::HitProxy::Circles {
                    circles: p.clone(),
                    default_radius: 4.0,
                },
                series::SeriesData::BookUpdate(_) => return,
            };
            self.series_for_channel_update(renderer, series_index)
        } else if matches!(data, SeriesData::BookUpdate(_)) {
            // Preserve legacy handling when a BookUpdate has no hit entry.
            renderer.and_then(|renderer| renderer.series_mut().get_mut(series_index))
        } else {
            return;
        };

        if let Some(series) = series {
            series.apply_data(&data);
        }

        self.render_generation += 1;
    }

    fn series_for_channel_update<'a>(
        &'a mut self,
        renderer: Option<&'a mut PlotRenderer>,
        series_index: usize,
    ) -> Option<&'a mut Box<dyn Series>> {
        // The queued update belongs to the series even before its first GPU target.
        let series = match renderer {
            Some(renderer) => renderer.series_mut(),
            None => self.pending_series.as_mut_slice(),
        };
        series.get_mut(series_index)
    }

    /// Apply a single DataUpdate from the data channel.
    fn apply_data_update(&mut self, update: DataUpdate, mut renderer: Option<&mut PlotRenderer>) {
        match update {
            DataUpdate::UpdateLastCandle {
                series_index,
                candle,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Candle { ref mut candles } = entry.proxy {
                        if let Some(last) = candles.last_mut() {
                            *last = candle.clone();
                        }
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.update_last(Box::new(candle));
                }
            }
            DataUpdate::AppendCandle {
                series_index,
                candle,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Candle { ref mut candles } = entry.proxy {
                        candles.push(candle.clone());
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.append_data(Box::new(candle));
                }
            }
            DataUpdate::ReplaceCandles {
                series_index,
                candles,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    entry.proxy = hit_proxy::HitProxy::Candle {
                        candles: candles.clone(),
                    };
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.apply_data(&series::SeriesData::Candles(candles));
                }
            }
            DataUpdate::UpdateLastLinePoint {
                series_index,
                point,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Line { ref mut points } = entry.proxy {
                        if let Some(last) = points.last_mut() {
                            *last = point;
                        }
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.update_last(Box::new(point));
                }
            }
            DataUpdate::AppendLinePoint {
                series_index,
                point,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Line { ref mut points } = entry.proxy {
                        points.push(point);
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.append_data(Box::new(point));
                }
            }
            DataUpdate::ReplaceLinePoints {
                series_index,
                points,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    entry.proxy = hit_proxy::HitProxy::Line {
                        points: points.clone(),
                    };
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.apply_data(&series::SeriesData::Lines(points));
                }
            }
            DataUpdate::UpdateLastBarPoint {
                series_index,
                point,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Bar { ref mut points } = entry.proxy {
                        if let Some(last) = points.last_mut() {
                            *last = point;
                        }
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.update_last(Box::new(point));
                }
            }
            DataUpdate::AppendBarPoint {
                series_index,
                point,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Bar { ref mut points } = entry.proxy {
                        points.push(point);
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.append_data(Box::new(point));
                }
            }
            DataUpdate::ReplaceBarPoints {
                series_index,
                points,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    entry.proxy = hit_proxy::HitProxy::Bar {
                        points: points.clone(),
                    };
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.apply_data(&series::SeriesData::Bars(points));
                }
            }
            DataUpdate::UpdateLastCircle {
                series_index,
                circle,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Circles {
                        ref mut circles, ..
                    } = entry.proxy
                    {
                        if let Some(last) = circles.last_mut() {
                            *last = circle;
                        }
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.update_last(Box::new(circle));
                }
            }
            DataUpdate::AppendCircle {
                series_index,
                circle,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Circles {
                        ref mut circles, ..
                    } = entry.proxy
                    {
                        circles.push(circle);
                    }
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.append_data(Box::new(circle));
                }
            }
            DataUpdate::ReplaceCircles {
                series_index,
                circles,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    let default_radius =
                        if let hit_proxy::HitProxy::Circles { default_radius, .. } = &entry.proxy {
                            *default_radius
                        } else {
                            4.0
                        };
                    entry.proxy = hit_proxy::HitProxy::Circles {
                        circles: circles.clone(),
                        default_radius,
                    };
                }
                if let Some(series) =
                    self.series_for_channel_update(renderer.as_deref_mut(), series_index)
                {
                    series.apply_data(&series::SeriesData::Circles(circles));
                }
            }
            DataUpdate::AddPriceLevel(level) => {
                self.add_price_level(level);
            }
            DataUpdate::RemovePriceLevel(id) => {
                self.remove_price_level(id);
            }
            DataUpdate::UpdatePriceLevelY { id, y } => {
                self.update_price_level(id, |l| l.y = y);
            }
            DataUpdate::AddTag(tag) => {
                self.add_tag(tag);
            }
            DataUpdate::RemoveTag(id) => {
                self.remove_tag(id);
            }
            DataUpdate::AddRegion(region) => {
                self.add_region(region);
            }
            DataUpdate::RemoveRegion(id) => {
                self.remove_region(id);
            }
            DataUpdate::AddCircle(circle) => {
                if let Some(existing) = self.circles.iter_mut().find(|c| c.id == circle.id) {
                    *existing = circle;
                } else {
                    self.circles.push(circle);
                }
                self.render_generation += 1;
            }
            DataUpdate::RemoveCircle(id) => {
                self.circles.retain(|c| c.id != id);
                self.render_generation += 1;
            }
            DataUpdate::AddLine(line) => {
                if let Some(existing) = self.lines.iter_mut().find(|l| l.id == line.id) {
                    *existing = line;
                } else {
                    self.lines.push(line);
                }
                self.render_generation += 1;
            }
            DataUpdate::RemoveLine(id) => {
                self.lines.retain(|l| l.id != id);
                self.render_generation += 1;
            }
            DataUpdate::AddBezier(bezier) => {
                if let Some(existing) = self.beziers.iter_mut().find(|b| b.id == bezier.id) {
                    *existing = bezier;
                } else {
                    self.beziers.push(bezier);
                }
                self.render_generation += 1;
            }
            DataUpdate::RemoveBezier(id) => {
                self.beziers.retain(|b| b.id != id);
                self.render_generation += 1;
            }
            DataUpdate::AddStyledPath(path) => {
                if let Some(existing) = self.styled_paths.iter_mut().find(|p| p.id == path.id) {
                    *existing = path;
                } else {
                    self.styled_paths.push(path);
                }
                self.render_generation += 1;
            }
            DataUpdate::RemoveStyledPath(id) => {
                self.styled_paths.retain(|p| p.id != id);
                self.render_generation += 1;
            }
            DataUpdate::AddPath(path) => {
                if let Some(existing) = self.paths.iter_mut().find(|p| p.id == path.id) {
                    *existing = path;
                } else {
                    self.paths.push(path);
                }
                self.render_generation += 1;
            }
            DataUpdate::RemovePath(id) => {
                self.paths.retain(|p| p.id != id);
                self.render_generation += 1;
            }
            DataUpdate::BookBids { series_index, bids } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Depth {
                        bids: ref mut proxy_bids,
                        ..
                    } = entry.proxy
                    {
                        *proxy_bids = bids.levels.clone();
                    }
                }
                if let Some(renderer) = renderer.as_deref_mut() {
                    if let Some(s) = renderer.series_mut().get_mut(series_index) {
                        s.apply_data(&series::SeriesData::BookUpdate(
                            series::BookUpdateKind::Bids(bids),
                        ));
                    }
                }
            }
            DataUpdate::BookAsks { series_index, asks } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Depth {
                        asks: ref mut proxy_asks,
                        ..
                    } = entry.proxy
                    {
                        *proxy_asks = asks.levels.clone();
                    }
                }
                if let Some(renderer) = renderer.as_deref_mut() {
                    if let Some(s) = renderer.series_mut().get_mut(series_index) {
                        s.apply_data(&series::SeriesData::BookUpdate(
                            series::BookUpdateKind::Asks(asks),
                        ));
                    }
                }
            }
            DataUpdate::BookSnapshot {
                series_index,
                bids,
                asks,
            } => {
                if let Some(entry) = self.hit_entries.get_mut(series_index) {
                    if let hit_proxy::HitProxy::Depth {
                        bids: ref mut proxy_bids,
                        asks: ref mut proxy_asks,
                        ..
                    } = entry.proxy
                    {
                        *proxy_bids = bids.levels.clone();
                        *proxy_asks = asks.levels.clone();
                    }
                }
                if let Some(renderer) = renderer.as_deref_mut() {
                    if let Some(s) = renderer.series_mut().get_mut(series_index) {
                        s.apply_data(&series::SeriesData::BookUpdate(
                            series::BookUpdateKind::Snapshot { bids, asks },
                        ));
                    }
                }
            }
        }
        self.render_generation += 1;
    }

    /// Get the current view state.
    pub fn view_state(&self) -> &ViewState {
        &self.view_state
    }

    /// Get a mutable reference to the view state.
    pub fn view_state_mut(&mut self) -> &mut ViewState {
        &mut self.view_state
    }

    fn transform(&self, width_px: f32, height_px: f32) -> Transform {
        self.transform_from_view_state(&self.view_state, width_px, height_px)
    }

    fn transform_from_view_state(
        &self,
        view_state: &ViewState,
        width_px: f32,
        height_px: f32,
    ) -> Transform {
        Transform::from_view_state_with_axes(
            view_state,
            width_px,
            height_px,
            self.x_axis.kind,
            self.y_axis.kind,
        )
    }

    fn ensure_log_axis_domains(&mut self) {
        Self::ensure_positive_domain(
            self.x_axis.kind,
            &mut self.view_state.x_min,
            &mut self.view_state.x_max,
        );
        Self::ensure_positive_domain(
            self.y_axis.kind,
            &mut self.view_state.y_min,
            &mut self.view_state.y_max,
        );
    }

    fn ensure_positive_domain(kind: AxisKind, min: &mut f64, max: &mut f64) {
        if kind != AxisKind::Log {
            return;
        }

        if min.is_finite() && max.is_finite() && *min > 0.0 && *max > *min {
            return;
        }

        if max.is_finite() && *max > 0.0 {
            *min = (*max * 0.01).max(f64::MIN_POSITIVE);
            if *max <= *min {
                *max = *min * 10.0;
            }
        } else {
            *min = 1.0;
            *max = 10.0;
        }
    }

    fn init_gpu(&mut self, width: u32, height: u32) {
        if width == 0 || height == 0 {
            return;
        }

        let gpu = match &self.gpu {
            Some(g) => g.clone(),
            None => return,
        };
        let scale_factor = self.low_level_render_scale_factor();

        if let Some(ref renderer_arc) = self.renderer {
            if render_pool::PlotRenderPool::resize(renderer_arc, width, height).is_err() {
                return;
            }
            self.texture_width = width;
            self.texture_height = height;
        } else {
            let series = std::mem::take(&mut self.pending_series);
            let Ok(plot_renderer) = PlotRenderer::new(gpu, width, height, series, self.texture_id)
            else {
                return;
            };

            self.renderer = Some(Arc::new(Mutex::new(plot_renderer)));
            self.texture_width = width;
            self.texture_height = height;

            if self.render_pool.is_none() {
                self.render_pool = Some(Arc::new(PlotRenderPool::new()));
            }
        }

        self.target_state = None;
        self.presented_texture_id = Some(self.texture_id);
        self.presented_labels = None;
        self.completed_publication = None;
        self.gpu_initialized = true;
        self.last_submission = None;
        self.submit_render(scale_factor);
    }

    fn low_level_render_scale_factor(&self) -> f32 {
        self.gpu.as_ref().map(|gpu| gpu.scale_factor).unwrap_or(1.0)
    }

    fn consume_completed_render(&mut self) -> bool {
        if self.resize_pending.is_some() {
            return false;
        }
        let completed = self
            .completed_render
            .try_lock()
            .ok()
            .and_then(|mut completed| completed.take());
        let Some(completed) = completed else {
            return false;
        };
        let generation = completed.generation;
        self.completed_publication = Some(completed);
        if generation < self.submission_generation {
            self.last_submission = None;
        }
        true
    }

    fn publish_completed_front(&mut self) -> bool {
        let Some(generation) = self
            .completed_publication
            .as_ref()
            .map(|frame| frame.generation)
        else {
            return false;
        };
        let texture_id = match self.target_state {
            Some(target) if target.active => target.texture_id,
            Some(_) => return false,
            None => self.texture_id,
        };
        let Some(renderer) = self.renderer.as_ref() else {
            return false;
        };
        let Ok(mut renderer) = renderer.try_lock() else {
            return false;
        };
        if renderer.last_rendered_generation() != generation {
            return false;
        }
        match renderer.present_pending_frame() {
            Ok(true) => {
                self.completed_publication
                    .take()
                    .unwrap()
                    .publish(&mut self.presented_labels);
                self.presented_texture_id = Some(texture_id);
                true
            }
            Ok(false) | Err(_) => false,
        }
    }

    /// Build RenderParams from current Plot state and submit to pool.
    fn submit_render(&mut self, scale_factor: f32) -> bool {
        let pool = match &self.render_pool {
            Some(p) => p,
            None => return false,
        };
        let renderer = match &self.renderer {
            Some(r) => r.clone(),
            None => return false,
        };
        let series_generation = match renderer.try_lock() {
            Ok(renderer) if !renderer.presentation_pending() => renderer.series_generation(),
            Ok(_) => return false,
            Err(_) => return false,
        };

        let w = self.texture_width as f32;
        let h = self.texture_height as f32;
        if w == 0.0 || h == 0.0 {
            return false;
        }

        let submission = self.render_submission_key(scale_factor);
        if self.last_submission.as_ref() == Some(&submission) {
            return false;
        }
        self.last_submission = Some(submission);
        self.submission_generation = self
            .submission_generation
            .checked_add(1)
            .expect("Plot render generation exhausted");
        let (params, market_detail_publication) = self.render_request(scale_factor);
        pool.submit(
            renderer,
            params,
            self.submission_generation,
            series_generation,
            self.completed_render.clone(),
            market_detail_publication,
        );
        true
    }

    fn render_submission_key(&self, scale_factor: f32) -> RenderSubmissionKey {
        let probe_generation = self
            .probe
            .as_ref()
            .and_then(|probe| probe.lock().ok().map(|probe| probe.generation()))
            .unwrap_or(0);
        let selection_signature = self.object_selection.iter().fold(0_u64, |hash, selection| {
            hash.rotate_left(9)
                ^ selection.series_index as u64
                ^ selection.world_x.to_bits().rotate_left(21)
                ^ selection.world_y.to_bits().rotate_left(43)
        });
        RenderSubmissionKey {
            state_generation: self.render_generation,
            series_data_generation: self.series_data_generation,
            market_reference: self.market_detail_reference_line(),
            view_state: self.view_state.clone(),
            x_axis_kind: self.x_axis.kind,
            y_axis_kind: self.y_axis.kind,
            texture_width: self.texture_width,
            texture_height: self.texture_height,
            scale_factor,
            crosshair_pos: self.crosshair_pos,
            box_selection: self.box_selection,
            probe_generation,
            selection_signature,
        }
    }

    fn render_request(
        &self,
        sf: f32,
    ) -> (
        RenderParams,
        Option<(
            MarketDetailLink,
            Option<market_detail::MarketDetailReferenceLine>,
        )>,
    ) {
        let w = self.texture_width as f32;
        let h = self.texture_height as f32;
        let crosshair_suppressed = self.is_crosshair_suppressed();
        let synced_crosshair_px = self.compute_synced_crosshair(w, h);

        let phys_transform = self.transform(w, h);
        let selection_rects: Vec<[f32; 4]> = self
            .object_selection
            .iter()
            .filter_map(|sel| {
                if sel.series_index >= self.hit_entries.len() {
                    return None;
                }
                self.hit_entries[sel.series_index].selection_rect(sel.hit.as_ref(), &phys_transform)
            })
            .collect();

        let level_vertices = self.build_level_vertices(w, h, sf);
        let circle_vertices = self.build_circle_vertices(w, h);
        let line_vertices = self.build_line_vertices(w, h);
        let bezier_vertices = self.build_bezier_vertices(w, h);
        let styled_path_vertices = self.build_styled_path_vertices(w, h);
        let path_vertices = self.build_path_vertices(w, h);

        let tool_overlay_vertices = if self.active_tool_index > 0 {
            let phys_transform2 = self.transform(w, h);
            self.tools[self.active_tool_index].overlay_vertices(&phys_transform2, w, h)
        } else {
            Vec::new()
        };

        let params = RenderParams {
            view_state: self.view_state.clone(),
            reference_tag: self.market_detail_price_level().cloned().map(|mut level| {
                let dimmed_color = self.resolve_default_color(
                    level.color,
                    crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.8),
                    self.color_scheme.annotation_neutral,
                );
                for line in &mut level.tag {
                    line.color.get_or_insert(if line.dimmed {
                        dimmed_color
                    } else {
                        self.color_scheme.annotation_text
                    });
                }
                level
            }),
            x_axis_kind: self.x_axis.kind,
            y_axis_kind: self.y_axis.kind,
            scale_factor: sf,
            crosshair_pos: self.crosshair_pos,
            crosshair_suppressed,
            box_selection: self.box_selection,
            selection_rects,
            synced_crosshair_px,
            hidden_series: self.hidden_series.clone(),
            level_vertices,
            tool_overlay_vertices,
            circle_vertices,
            line_vertices,
            bezier_vertices,
            styled_path_vertices,
            path_vertices,
            color_scheme: self.color_scheme.clone(),
        };
        let market_detail_publication = self
            .market_detail_link
            .clone()
            .map(|link| (link, self.market_detail_reference_line()));

        (params, market_detail_publication)
    }

    fn clear_completed_render(&mut self) {
        if let Ok(mut completed) = self.completed_render.try_lock() {
            completed.take();
        }
        self.completed_publication = None;
    }

    fn withdraw_canonical_front(&mut self) {
        if let Some(front) = self.canonical_front.take() {
            front.witness.withdraw();
        }
    }

    fn cancel_resize(&mut self) {
        if let Some(pending) = self.resize_pending.take() {
            pending.current.store(false, Ordering::Release);
        }
    }

    fn consume_resize_completion(&mut self) {
        use render_pool::ResizeRenderFailure;
        let Some(pending) = self.resize_pending.as_mut() else { return };
        let Some(completed) = pending.completed.as_ref() else { return };
        let result = match completed.try_recv() {
            Ok(result) => result,
            Err(mpsc::TryRecvError::Empty) => return,
            Err(mpsc::TryRecvError::Disconnected) => Err(ResizeRenderFailure::WorkerAbandoned),
        };
        pending.completed = None;
        match result {
            Ok(publication) => pending.publication = Some(publication),
            Err(error) => pending.failure = Some(error),
        }
    }

    fn publish_canonical_front(&mut self, target: PlotTargetState) -> bool {
        let Some(generation) = self
            .completed_publication
            .as_ref()
            .map(|frame| frame.generation)
        else {
            return false;
        };
        let Some(renderer) = self.renderer.as_ref() else { return false };
        let Ok(renderer) = renderer.try_lock() else { return false };
        if renderer.last_rendered_generation() != generation {
            return false;
        }
        let current = Arc::new(AtomicBool::new(true));
        let request = renderer.request_pending_plot_front(current.clone());
        drop(renderer);
        match request {
            Ok(admission) => {
                if let Err(error) = admission.try_resolve() {
                    return self.refuse_resize_front(error);
                }
                self.withdraw_canonical_front();
                self.canonical_front = Some(PlotCanonicalFront {
                    witness: PlotFrontWitness::new(target.texture_id, current, None),
                    render_generation: generation,
                    admission: Some(admission),
                    binding: None,
                });
                self.completed_publication
                    .take()
                    .unwrap()
                    .publish(&mut self.presented_labels);
                self.presented_texture_id = Some(target.texture_id);
                true
            }
            Err(error) => self.refuse_resize_front(error),
        }
    }

    fn observe_canonical_front(&mut self) -> bool {
        let Some(front) = self.canonical_front.as_mut() else { return false };
        if !front.witness.0.current.load(Ordering::Acquire) {
            return self.refuse_resize_front(render_pool::ResizeRenderFailure::OutputRefused);
        }
        if let Some(admission) = &front.admission {
            match admission.try_resolve() {
                Ok(Some(binding)) => {
                    front.binding = Some(binding);
                    front.admission = None;
                }
                Ok(None) => return false,
                Err(error) => return self.refuse_resize_front(error),
            }
        }
        if let Some(error) = front
            .binding
            .as_ref()
            .and_then(|binding| binding.availability().err())
        {
            return self.refuse_resize_front(error);
        }
        if let Some(renderer) = &self.renderer
            && let Ok(mut renderer) = renderer.try_lock()
            && renderer.last_rendered_generation() == front.render_generation
        {
            renderer.finish_plot_registration();
        }
        false
    }

    fn refuse_resize_front(&mut self, error: impl std::fmt::Debug) -> bool {
        tracing::warn!(?error, "Plot front refused");
        self.cancel_resize();
        self.withdraw_canonical_front();
        self.presented_texture_id = None;
        self.target_state = None;
        self.last_submission = None;
        self.unpublish_available_front();
        self.clear_completed_render();
        true
    }

    fn prepare_resize_front(&mut self, target: PlotTargetState, resource_changed: bool) -> bool {
        use render_pool::{ResizeRenderFailure, ResizeRenderJob, ResizeRenderSlot};
        if let Some(failure) = self
            .gpu
            .as_ref()
            .and_then(|gpu| gpu.observe_device_lifetime().failure())
        {
            return self.refuse_resize_front(ResizeRenderFailure::Device(failure));
        }
        let extent = target.resource_key.physical_render_extent;
        self.texture_width = extent.width;
        self.texture_height = extent.height;
        let scale = f32::from_bits(target.resource_key.scale_factor_bits);
        let submission = self.render_submission_key(scale);
        let needs_job = self
            .resize_pending
            .as_ref()
            .is_none_or(|pending| pending.target != target)
            || resource_changed
            || self.last_submission.as_ref() != Some(&submission);
        self.target_state = Some(target);
        if needs_job {
            self.cancel_resize();
            self.withdraw_canonical_front();
            self.clear_completed_render();
            self.presented_texture_id = None;
            let (Some(pool), Some(renderer), Some(gpu)) =
                (&self.render_pool, &self.renderer, &self.gpu)
            else {
                return self.refuse_resize_front(ResizeRenderFailure::PoolClosed);
            };
            if self
                .resize_slot
                .as_ref()
                .is_some_and(|slot| slot.abandoned())
            {
                self.resize_slot = None;
            }
            let slot = self
                .resize_slot
                .get_or_insert_with(|| {
                    Arc::new(ResizeRenderSlot::new(
                        renderer.clone(),
                        gpu.image_admission.clone(),
                    ))
                })
                .clone();
            let (params, market_detail_publication) = self.render_request(scale);
            let (sender, receiver) = mpsc::channel();
            let current = Arc::new(AtomicBool::new(true));
            let completed_ready = Arc::new(AtomicBool::new(false));
            let Some(generation) = self.submission_generation.checked_add(1) else {
                return self.refuse_resize_front(ResizeRenderFailure::GenerationExhausted);
            };
            if let Err(error) = pool.submit_resize(
                &slot,
                ResizeRenderJob {
                    params,
                    width: extent.width,
                    height: extent.height,
                    generation,
                    series_data_generation: self.series_data_generation,
                    current: current.clone(),
                    market_detail_publication,
                    completed: sender,
                    completed_ready: completed_ready.clone(),
                },
            ) {
                return self.refuse_resize_front(error);
            }
            self.submission_generation = generation;
            self.last_submission = Some(submission);
            self.resize_pending = Some(PlotResizePending {
                target,
                current,
                completed: Some(receiver),
                completed_ready,
                publication: None,
                admission: None,
                failure: None,
            });
            return true;
        }

        let pending = self
            .resize_pending
            .as_mut()
            .expect("current Plot replacement");
        if let Some(error) = pending.failure.take() {
            return self.refuse_resize_front(error);
        }
        let Some(publication) = pending.publication.as_ref() else { return false };
        if pending.admission.is_none() {
            let Some(renderer) = self.renderer.as_ref() else {
                return self.refuse_resize_front(ResizeRenderFailure::RendererPoisoned);
            };
            let request = match renderer.try_lock() {
                Ok(renderer) if renderer.last_rendered_generation() == publication.generation
                    && renderer.series_data_generation() == self.series_data_generation =>
                    renderer.request_pending_plot_front(pending.current.clone())
                        .map_err(ResizeRenderFailure::Pass),
                Ok(_) => Err(ResizeRenderFailure::SourceChanged),
                Err(std::sync::TryLockError::WouldBlock) => return false,
                Err(std::sync::TryLockError::Poisoned(_)) => Err(ResizeRenderFailure::RendererPoisoned),
            };
            match request {
                Ok(admission) => {
                    if let Err(error) = admission.try_resolve() {
                        return self.refuse_resize_front(error);
                    }
                    pending.admission = Some(admission);
                }
                Err(error) => return self.refuse_resize_front(error),
            }
            return false;
        }
        let binding = match pending.admission.as_ref().unwrap().try_resolve() {
            Ok(Some(binding)) => binding,
            Ok(None) => return false,
            Err(error) => return self.refuse_resize_front(error),
        };
        if let Err(error) = binding.availability() {
            return self.refuse_resize_front(error);
        }
        if binding.identity().texture_id() != target.texture_id
            || publication.generation != self.submission_generation
        {
            return self.refuse_resize_front(ResizeRenderFailure::SourceChanged);
        }
        let mut pending = self.resize_pending.take().unwrap();
        let publication = pending.publication.take().unwrap();
        self.clear_completed_render();
        publication.publish(&mut self.presented_labels);
        self.presented_texture_id = Some(target.texture_id);
        self.canonical_front = Some(PlotCanonicalFront {
            witness: PlotFrontWitness::new(
                target.texture_id,
                pending.current,
                Some(binding.clone()),
            ),
            render_generation: self.submission_generation,
            admission: None,
            binding: Some(binding),
        });
        self.retire_when_available = false;
        self.observe_canonical_front();
        true
    }

    fn install_target_resources(&mut self, key: PlotTargetResourceKey, texture_id: u64) -> bool {
        let width = key.physical_render_extent.width;
        let height = key.physical_render_extent.height;
        let Some(gpu) = self.gpu.as_ref() else { return false };
        let limit = gpu.device.limits().max_texture_dimension_2d;
        if width > limit || height > limit || gpu.observe_device_lifetime().failure().is_some() {
            return false;
        }
        let installed = if let Some(renderer) = self.renderer.as_ref() {
            let Ok(mut renderer) = renderer.try_lock() else {
                return false;
            };
            let same_surface = self
                .target_state
                .is_some_and(|state| state.resource_key.surface_id == key.surface_id);
            if same_surface {
                renderer.resize_unpublished(width, height).is_ok()
            } else {
                renderer
                    .retarget_unpublished(texture_id, width, height)
                    .is_ok()
            }
        } else {
            let series = std::mem::take(&mut self.pending_series);
            match PlotRenderer::new_unpublished(gpu.clone(), width, height, series, texture_id) {
                Ok(renderer) => {
                    self.renderer = Some(Arc::new(Mutex::new(renderer)));
                    true
                }
                Err(_) => false,
            }
        };
        if !installed {
            return false;
        }

        if self.render_pool.is_none() {
            self.render_pool = Some(Arc::new(PlotRenderPool::new()));
        }
        self.texture_width = width;
        self.texture_height = height;
        self.gpu_initialized = true;
        self.retire_when_available = false;
        self.presented_texture_id = None;
        self.last_submission = None;
        self.clear_completed_render();
        true
    }

    fn prepare_active_target(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        physical_render_extent: PhysicalExtent,
        physical_composition_coverage: PhysicalRect,
    ) -> CustomRenderPreparationOutcome {
        let key = PlotTargetResourceKey {
            surface_id: metrics.surface_id,
            scale_factor_bits: metrics.scale_factor.to_bits(),
            target_physical_extent: metrics.target_physical_extent,
            physical_render_extent,
        };
        let canonical = metrics.app_frame_attempt.is_some()
            && self
                .mounted_plot
                .as_ref()
                .is_some_and(WidgetMountToken::is_live);
        if canonical && self.observe_canonical_front() {
            return CustomRenderPreparationOutcome::front_publication_changed();
        }
        let mut front_publication_changed = false;
        let previous = self.target_state;
        let resource_changed = previous
            .filter(|state| state.active)
            .map(|state| state.resource_key)
            != Some(key);
        let texture_id = match previous {
            Some(state) if state.resource_key.surface_id == key.surface_id => state.texture_id,
            Some(_) => WidgetInternalId::new().raw(),
            None if self.renderer.is_none() => self.texture_id,
            None => WidgetInternalId::new().raw(),
        };
        let target = PlotTargetState {
            resource_key: key,
            physical_composition_coverage,
            texture_id,
            active: true,
        };
        if canonical
            && previous.is_some_and(|prior| {
                prior.active
                    && prior.resource_key.surface_id == key.surface_id
                    && (self.resize_pending.is_some()
                        || prior.resource_key.physical_render_extent != key.physical_render_extent
                            && self
                                .canonical_front
                                .as_ref()
                                .is_some_and(PlotCanonicalFront::has_submitted_binding))
            })
        {
            front_publication_changed |= self.prepare_resize_front(target, resource_changed);
            return CustomRenderPreparationOutcome::new(front_publication_changed);
        }
        self.cancel_resize();

        if resource_changed {
            front_publication_changed |= self.presented_texture_id.is_some();
            self.withdraw_canonical_front();
            if !self.install_target_resources(key, texture_id) {
                self.target_state = None;
                self.presented_texture_id = None;
                return CustomRenderPreparationOutcome::new(front_publication_changed);
            }
        }

        self.target_state = Some(target);
        front_publication_changed |= if canonical {
            self.publish_canonical_front(target)
        } else {
            self.publish_completed_front()
        };
        self.submit_render(metrics.scale_factor);
        CustomRenderPreparationOutcome::new(front_publication_changed)
    }

    fn prepare_inactive_target(&mut self) -> CustomRenderPreparationOutcome {
        self.cancel_resize();
        self.withdraw_canonical_front();
        let was_active = self.target_state.is_some_and(|state| state.active)
            || self.presented_texture_id.is_some()
            || self.retire_when_available;
        if !was_active {
            return CustomRenderPreparationOutcome::default();
        }

        if let Some(state) = self.target_state.as_mut() {
            state.active = false;
        }
        let was_presented = self.presented_texture_id.take().is_some();
        let unpublished = self.unpublish_available_front();
        self.last_submission = None;
        self.clear_completed_render();
        CustomRenderPreparationOutcome::new(was_presented || unpublished)
    }

    fn unpublish_available_front(&mut self) -> bool {
        self.retire_when_available = false;
        let Some(renderer) = &self.renderer else { return false };
        match renderer.try_lock() {
            Ok(mut renderer) => renderer.unpublish_front().is_ok(),
            Err(std::sync::TryLockError::WouldBlock) => {
                self.retire_when_available = true;
                false
            }
            Err(std::sync::TryLockError::Poisoned(error)) => {
                error.into_inner().destroy_deferred();
                true
            }
        }
    }

    pub(crate) fn withdraw_inactive_surface(
        &mut self,
        surface: crate::compositor::SurfaceId,
    ) -> bool {
        self.target_state
            .is_some_and(|target| target.resource_key.surface_id == surface)
            && self.prepare_inactive_target().front_publication_changed
    }

    pub(crate) fn market_detail_link(&self) -> Option<&MarketDetailLink> {
        self.market_detail_link.as_ref()
    }

    pub(crate) fn prepare_mounted_plot(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
        mount: WidgetMountToken,
    ) -> CustomRenderPreparationOutcome {
        let before_image = self.presented_texture_id;
        let was_pending = self.resize_pending.is_some();
        let can_publish = was_pending || self.completed_publication.is_some();
        let before_labels = can_publish.then(|| self.presented_labels.clone()).flatten();
        let before_projection = can_publish
            .then(|| self.market_projection_generations())
            .flatten();
        if self.mounted_plot.as_ref() != Some(&mount) {
            self.cancel_resize();
            self.withdraw_canonical_front();
            self.mounted_plot = Some(mount.clone());
        }
        let mut outcome = self.prepare_custom_render(metrics);
        let Some(attempt) = metrics.app_frame_attempt else { return outcome };
        let Some(admission) = &self.custom_render_admission else { return outcome };
        let target = self.target_state.filter(|target| target.active);
        let completed_resize = was_pending && self.resize_pending.is_none();
        let labels_changed = can_publish
            && before_labels.as_ref().map(Arc::as_ptr)
                != self.presented_labels.as_ref().map(Arc::as_ptr);
        outcome.plot_evidence.push(PlotPreparationEvidence {
            mount,
            attempt,
            admission: admission.presentation_id(),
            target,
            pending_generation: self
                .resize_pending
                .as_ref()
                .map(|_| self.submission_generation),
            front: self
                .canonical_front
                .as_ref()
                .map(|front| front.witness.clone()),
            local_paint_changed: before_image != self.presented_texture_id
                || completed_resize
                || labels_changed && self.presented_texture_id.is_some(),
            market_projection_changed: can_publish
                && self.presented_texture_id.is_some()
                && before_projection != self.market_projection_generations(),
        });
        outcome
    }

    fn market_projection_generations(&self) -> Option<(u64, u64)> {
        self.market_detail_link
            .as_ref()?
            .presented_snapshot()
            .map(|snapshot| (snapshot.axis_generation, snapshot.reference_generation))
    }

    fn render_work_outstanding(&self) -> bool {
        if let Some(pending) = self.resize_pending.as_ref() {
            return pending.completed.is_some() && pending.completed_ready.load(Ordering::Acquire);
        }
        if self.target_state.is_some_and(|state| !state.active) {
            return false;
        }
        self.renderer
            .as_ref()
            .is_some_and(|renderer| match renderer.try_lock() {
                Ok(renderer) => {
                    renderer.presentation_pending()
                        || renderer.last_rendered_generation() < self.submission_generation
                }
                Err(std::sync::TryLockError::WouldBlock) => true,
                Err(std::sync::TryLockError::Poisoned(_)) => false,
            })
    }

    fn price_level_animation_active(&self) -> bool {
        let drag_id = self.level_drag.as_ref().map(|drag| drag.level_id);
        let reference_id = self.market_detail_reference_level;
        self.price_levels.iter().any(|level| {
            if drag_id == Some(level.id) || reference_id == Some(level.id) {
                return false;
            }
            let tolerance = f64::EPSILON * level.y.abs().max(1.0) * 8.0;
            (level.y - level.display_y).abs() > tolerance
        })
    }

    /// True if local crosshair should be suppressed (another Plot owns probe).
    fn is_crosshair_suppressed(&self) -> bool {
        if let Some(ref probe_arc) = self.probe {
            if let Ok(probe) = probe_arc.lock() {
                probe.is_active() && !probe.is_from(self.id)
            } else {
                false
            }
        } else {
            false
        }
    }

    /// Compute synced crosshair physical-pixel X from shared Probe.
    /// Returns None if no probe, probe inactive, self-sourced, or out of viewport.
    fn compute_synced_crosshair(&self, w: f32, h: f32) -> Option<f32> {
        let probe_arc = self.probe.as_ref()?;
        let probe = probe_arc.lock().ok()?;
        if !probe.is_active() || probe.is_from(self.id) {
            return None;
        }
        let world_x = probe.x?;
        let transform = self.transform(w, h);
        let px = transform.x_to_px(world_x);
        if px >= 0.0 && px <= w { Some(px) } else { None }
    }

    /// SPEC-HAL-299: Build overlay vertices for price level lines and tag backgrounds.
    fn build_level_vertices(
        &self,
        viewport_w: f32,
        viewport_h: f32,
        scale_factor: f32,
    ) -> Vec<crate::plot::series::candle::Vertex> {
        use crate::plot::series::candle::Vertex;

        fn push_rect(verts: &mut Vec<Vertex>, x1: f32, y1: f32, x2: f32, y2: f32, color: [f32; 4]) {
            verts.push(Vertex {
                position: [x1, y1],
                color,
            });
            verts.push(Vertex {
                position: [x2, y1],
                color,
            });
            verts.push(Vertex {
                position: [x1, y2],
                color,
            });
            verts.push(Vertex {
                position: [x1, y2],
                color,
            });
            verts.push(Vertex {
                position: [x2, y1],
                color,
            });
            verts.push(Vertex {
                position: [x2, y2],
                color,
            });
        }

        if self.price_levels.is_empty() && self.tags.is_empty() && self.regions.is_empty() {
            return Vec::new();
        }

        let transform = self.transform(viewport_w, viewport_h);
        let mut vertices = Vec::new();
        let thickness = if scale_factor > 1.5 { 2.0 } else { 1.0 };

        // PlotRegion backgrounds (behind all other overlay content)
        for region in &self.regions {
            let rx_min = region.x_min.map(|x| transform.x_to_px(x)).unwrap_or(0.0);
            let rx_max = region
                .x_max
                .map(|x| transform.x_to_px(x))
                .unwrap_or(viewport_w);
            let ry_min = region.y_min.map(|y| transform.y_to_px(y)).unwrap_or(0.0);
            let ry_max = region
                .y_max
                .map(|y| transform.y_to_px(y))
                .unwrap_or(viewport_h);

            // Skip only if entirely off-screen (GPU clips partial overlap)
            let (left, right) = if rx_min < rx_max {
                (rx_min, rx_max)
            } else {
                (rx_max, rx_min)
            };
            let (top, bottom) = if ry_min < ry_max {
                (ry_min, ry_max)
            } else {
                (ry_max, ry_min)
            };
            if right <= 0.0 || left >= viewport_w || bottom <= 0.0 || top >= viewport_h {
                continue;
            }

            push_rect(
                &mut vertices,
                left,
                top,
                right,
                bottom,
                self.resolve_default_color(
                    region.fill_color,
                    crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.1),
                    self.color_scheme.annotation_fill,
                )
                .to_array(),
            );

            if let Some(bc) = region.border_color {
                let bc = bc.to_array();
                let t = if scale_factor > 1.5 { 2.0 } else { 1.0 };
                push_rect(&mut vertices, left, top, right, top + t, bc);
                push_rect(&mut vertices, left, bottom - t, right, bottom, bc);
                push_rect(&mut vertices, left, top, left + t, bottom, bc);
                push_rect(&mut vertices, right - t, top, right, bottom, bc);
            }
        }

        for level in &self.price_levels {
            if level.display_y < self.view_state.y_min || level.display_y > self.view_state.y_max {
                continue;
            }

            let y_px = transform.y_to_px(level.display_y);

            let is_hovered = self.hovered_level == Some(level.id) && level.draggable;
            let is_dragging = self
                .level_drag
                .as_ref()
                .map(|d| d.level_id == level.id)
                .unwrap_or(false);
            let emphasized = is_hovered || is_dragging;

            // Thicker + brighter on hover/drag
            let line_thick = if emphasized {
                thickness + 2.0
            } else {
                thickness
            };
            let level_color = self.resolve_default_color(
                level.color,
                crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.8),
                self.color_scheme.annotation_neutral,
            );
            let color = if emphasized {
                level_color.with_alpha(1.0).to_array()
            } else {
                level_color.to_array()
            };

            match level.line_style {
                PriceLevelLine::Solid => {
                    push_rect(
                        &mut vertices,
                        0.0,
                        y_px - line_thick / 2.0,
                        viewport_w,
                        y_px + line_thick / 2.0,
                        color,
                    );
                }
                PriceLevelLine::Dashed => {
                    let dash = 8.0;
                    let gap = 4.0;
                    let mut x = 0.0;
                    while x < viewport_w {
                        let end = (x + dash).min(viewport_w);
                        push_rect(
                            &mut vertices,
                            x,
                            y_px - line_thick / 2.0,
                            end,
                            y_px + line_thick / 2.0,
                            color,
                        );
                        x += dash + gap;
                    }
                }
                PriceLevelLine::Dotted => {
                    let dot = 2.0;
                    let gap = 3.0;
                    let mut x = 0.0;
                    while x < viewport_w {
                        let end = (x + dot).min(viewport_w);
                        push_rect(
                            &mut vertices,
                            x,
                            y_px - line_thick / 2.0,
                            end,
                            y_px + line_thick / 2.0,
                            color,
                        );
                        x += dot + gap;
                    }
                }
            }
        }

        // Ghost line for active drag
        if let Some(ref drag) = self.level_drag {
            let ghost_y_px = transform.y_to_px(drag.original_y);
            let ghost_color = self.color_scheme.ghost_line.to_array();

            let dash = 8.0;
            let gap = 4.0;
            let mut x = 0.0;
            while x < viewport_w {
                let end = (x + dash).min(viewport_w);
                push_rect(
                    &mut vertices,
                    x,
                    ghost_y_px - thickness / 2.0,
                    end,
                    ghost_y_px + thickness / 2.0,
                    ghost_color,
                );
                x += dash + gap;
            }
        }

        // PlotTag backgrounds (freestanding world-coordinate labels)
        let line_height = 12.0;
        let padding_y = 3.0;
        let tag_w = 80.0;
        for tag_item in &self.tags {
            if tag_item.tag.is_empty() {
                continue;
            }
            if tag_item.x < self.view_state.x_min
                || tag_item.x > self.view_state.x_max
                || tag_item.y < self.view_state.y_min
                || tag_item.y > self.view_state.y_max
            {
                continue;
            }

            let anchor_x = transform.x_to_px(tag_item.x);
            let anchor_y = transform.y_to_px(tag_item.y);
            let t_h = tag_item.tag.len() as f32 * line_height + padding_y * 2.0;
            let t_w = tag_w;

            let (tx, ty) = match tag_item.anchor {
                TagAnchor::Above => (anchor_x - t_w / 2.0, anchor_y - t_h - 4.0),
                TagAnchor::Below => (anchor_x - t_w / 2.0, anchor_y + 4.0),
                TagAnchor::Right => (anchor_x + 4.0, anchor_y - t_h / 2.0),
                TagAnchor::Left => (anchor_x - t_w - 4.0, anchor_y - t_h / 2.0),
            };

            let tag_color = self.resolve_default_color(
                tag_item.color,
                crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.8),
                self.color_scheme.annotation_neutral,
            );
            let bg_color = tag_color.with_alpha(tag_color.a * 0.8).to_array();
            push_rect(&mut vertices, tx, ty, tx + t_w, ty + t_h, bg_color);
        }

        vertices
    }

    fn build_circle_vertices(&self, w: f32, h: f32) -> Vec<CircleVertex> {
        if self.circles.is_empty() {
            return Vec::new();
        }

        let transform = self.transform(w, h);
        let mut verts = Vec::with_capacity(self.circles.len() * 6);

        for c in &self.circles {
            // Viewport culling
            let px = transform.x_to_px(c.x);
            let py = transform.y_to_px(c.y);
            if px < -(c.radius + 1.0)
                || px > w + c.radius + 1.0
                || py < -(c.radius + 1.0)
                || py > h + c.radius + 1.0
            {
                continue;
            }

            let r = c.radius;

            // Hover emphasis: brighter border
            let is_hovered = self.hovered_circle == Some(c.id);
            let border_color = self.resolve_default_color(
                c.border_color,
                crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, 0.8),
                self.color_scheme.annotation_line,
            );
            let border = if is_hovered {
                border_color.with_alpha(1.0).to_array()
            } else {
                border_color.to_array()
            };
            let bw = if is_hovered {
                c.border_width + 0.5
            } else {
                c.border_width
            };
            let fill = c.fill_color.to_array();

            let margin = r + 1.0;
            let x0 = px - margin;
            let y0 = py - margin;
            let x1 = px + margin;
            let y1 = py + margin;

            let base = |pos: [f32; 2]| CircleVertex {
                position: pos,
                center: [px, py],
                radius: r,
                fill_color: fill,
                border_color: border,
                border_width: bw,
            };

            verts.push(base([x0, y0]));
            verts.push(base([x1, y0]));
            verts.push(base([x0, y1]));
            verts.push(base([x0, y1]));
            verts.push(base([x1, y0]));
            verts.push(base([x1, y1]));
        }

        verts
    }

    /// Build overlay vertices for PlotLine markers.
    /// Returns OverlayVertex triangles for the overlay pipeline.
    fn build_line_vertices(&self, w: f32, h: f32) -> Vec<OverlayVertex> {
        use crate::plot::series::candle::Vertex as OV;

        if self.lines.is_empty() {
            return Vec::new();
        }

        let transform = self.transform(w, h);
        let mut verts = Vec::new();

        for line in &self.lines {
            let px1 = transform.x_to_px(line.x1);
            let py1 = transform.y_to_px(line.y1);
            let px2 = transform.x_to_px(line.x2);
            let py2 = transform.y_to_px(line.y2);

            // Skip if entirely off-screen (generous margin)
            let margin = line.arrow_size + line.width;
            if px1.max(px2) < -margin
                || px1.min(px2) > w + margin
                || py1.max(py2) < -margin
                || py1.min(py2) > h + margin
            {
                continue;
            }

            let is_hovered = self.hovered_line == Some(line.id);
            let line_color = self.resolve_default_color(
                line.color,
                crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, 0.8),
                self.color_scheme.annotation_line,
            );
            let color = if is_hovered {
                line_color.with_alpha(1.0).to_array()
            } else {
                line_color.to_array()
            };
            let half_w = if is_hovered {
                (line.width + 1.0) / 2.0
            } else {
                line.width / 2.0
            };

            let dx = px2 - px1;
            let dy = py2 - py1;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                continue;
            }
            let dir_x = dx / len;
            let dir_y = dy / len;
            let perp_x = -dir_y;
            let perp_y = dir_x;

            // Shorten endpoints for arrowheads
            let (s1x, s1y) = if line.arrow_start {
                (
                    px1 + dir_x * line.arrow_size * 0.7,
                    py1 + dir_y * line.arrow_size * 0.7,
                )
            } else {
                (px1, py1)
            };
            let (s2x, s2y) = if line.arrow_end {
                (
                    px2 - dir_x * line.arrow_size * 0.7,
                    py2 - dir_y * line.arrow_size * 0.7,
                )
            } else {
                (px2, py2)
            };

            match line.line_style {
                PlotLineStyle::Solid => {
                    Self::push_segment(
                        &mut verts, s1x, s1y, s2x, s2y, perp_x, perp_y, half_w, color,
                    );
                }
                PlotLineStyle::Dashed { dash, gap } | PlotLineStyle::Dotted { dot: dash, gap } => {
                    let seg_len = ((s2x - s1x).powi(2) + (s2y - s1y).powi(2)).sqrt();
                    let mut t = 0.0;
                    while t < seg_len {
                        let t_end = (t + dash).min(seg_len);
                        let ax = s1x + dir_x * t;
                        let ay = s1y + dir_y * t;
                        let bx = s1x + dir_x * t_end;
                        let by = s1y + dir_y * t_end;
                        Self::push_segment(
                            &mut verts, ax, ay, bx, by, perp_x, perp_y, half_w, color,
                        );
                        t += dash + gap;
                    }
                }
            }

            // Arrowhead at start (pointing toward start)
            if line.arrow_start {
                let tip_x = px1;
                let tip_y = py1;
                let base_x = px1 + dir_x * line.arrow_size;
                let base_y = py1 + dir_y * line.arrow_size;
                let half_arrow = line.arrow_size * 0.5;
                verts.push(OV {
                    position: [tip_x, tip_y],
                    color,
                });
                verts.push(OV {
                    position: [base_x + perp_x * half_arrow, base_y + perp_y * half_arrow],
                    color,
                });
                verts.push(OV {
                    position: [base_x - perp_x * half_arrow, base_y - perp_y * half_arrow],
                    color,
                });
            }

            // Arrowhead at end (pointing toward end)
            if line.arrow_end {
                let tip_x = px2;
                let tip_y = py2;
                let base_x = px2 - dir_x * line.arrow_size;
                let base_y = py2 - dir_y * line.arrow_size;
                let half_arrow = line.arrow_size * 0.5;
                verts.push(OV {
                    position: [tip_x, tip_y],
                    color,
                });
                verts.push(OV {
                    position: [base_x + perp_x * half_arrow, base_y + perp_y * half_arrow],
                    color,
                });
                verts.push(OV {
                    position: [base_x - perp_x * half_arrow, base_y - perp_y * half_arrow],
                    color,
                });
            }
        }

        verts
    }

    /// Push a single line segment as a rotated quad (6 overlay vertices).
    fn push_segment(
        verts: &mut Vec<OverlayVertex>,
        ax: f32,
        ay: f32,
        bx: f32,
        by: f32,
        perp_x: f32,
        perp_y: f32,
        half_w: f32,
        color: [f32; 4],
    ) {
        use crate::plot::series::candle::Vertex as OV;
        let p0 = [ax + perp_x * half_w, ay + perp_y * half_w];
        let p1 = [ax - perp_x * half_w, ay - perp_y * half_w];
        let p2 = [bx + perp_x * half_w, by + perp_y * half_w];
        let p3 = [bx - perp_x * half_w, by - perp_y * half_w];
        verts.push(OV {
            position: p0,
            color,
        });
        verts.push(OV {
            position: p2,
            color,
        });
        verts.push(OV {
            position: p1,
            color,
        });
        verts.push(OV {
            position: p1,
            color,
        });
        verts.push(OV {
            position: p2,
            color,
        });
        verts.push(OV {
            position: p3,
            color,
        });
    }

    /// Build overlay vertices for PlotBezier markers.
    /// Adaptive subdivision into segments, rendered via push_segment.
    fn build_bezier_vertices(&self, w: f32, h: f32) -> Vec<OverlayVertex> {
        if self.beziers.is_empty() {
            return Vec::new();
        }

        let transform = self.transform(w, h);
        let mut verts = Vec::new();

        for bez in &self.beziers {
            // Transform all four points to screen space
            let p0x = transform.x_to_px(bez.x1);
            let p0y = transform.y_to_px(bez.y1);
            let c1x = transform.x_to_px(bez.cx1);
            let c1y = transform.y_to_px(bez.cy1);
            let c2x = transform.x_to_px(bez.cx2);
            let c2y = transform.y_to_px(bez.cy2);
            let p1x = transform.x_to_px(bez.x2);
            let p1y = transform.y_to_px(bez.y2);

            // Estimate arc length from control polygon for adaptive subdivision
            let chord1 = ((c1x - p0x).powi(2) + (c1y - p0y).powi(2)).sqrt();
            let chord2 = ((c2x - c1x).powi(2) + (c2y - c1y).powi(2)).sqrt();
            let chord3 = ((p1x - c2x).powi(2) + (p1y - c2y).powi(2)).sqrt();
            let approx_len = chord1 + chord2 + chord3;

            // INV-296B-5: minimum 16 segments, scaled by arc length
            let n = (approx_len / 4.0).max(16.0).min(200.0) as usize;

            // Evaluate cubic Bezier at n+1 points
            let mut points = Vec::with_capacity(n + 1);
            for i in 0..=n {
                let t = i as f32 / n as f32;
                let mt = 1.0 - t;
                let mt2 = mt * mt;
                let mt3 = mt2 * mt;
                let t2 = t * t;
                let t3 = t2 * t;

                let x = mt3 * p0x + 3.0 * mt2 * t * c1x + 3.0 * mt * t2 * c2x + t3 * p1x;
                let y = mt3 * p0y + 3.0 * mt2 * t * c1y + 3.0 * mt * t2 * c2y + t3 * p1y;
                points.push((x, y));
            }

            let is_hovered = self.hovered_bezier == Some(bez.id);
            let bezier_color = self.resolve_default_color(
                bez.color,
                crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, 0.8),
                self.color_scheme.annotation_line,
            );
            let color = if is_hovered {
                bezier_color.with_alpha(1.0).to_array()
            } else {
                bezier_color.to_array()
            };
            let half_w = if is_hovered {
                (bez.width + 1.0) / 2.0
            } else {
                bez.width / 2.0
            };

            // Emit segments
            for i in 0..n {
                let (ax, ay) = points[i];
                let (bx, by) = points[i + 1];
                let dx = bx - ax;
                let dy = by - ay;
                let seg_len = (dx * dx + dy * dy).sqrt();
                if seg_len < 0.001 {
                    continue;
                }
                let perp_x = -dy / seg_len;
                let perp_y = dx / seg_len;
                Self::push_segment(&mut verts, ax, ay, bx, by, perp_x, perp_y, half_w, color);
            }

            // Arrowhead at start — tangent B'(0) = 3(C1 - P0)
            if bez.arrow_start && points.len() >= 2 {
                let (tip_x, tip_y) = points[0];
                let tang_x = 3.0 * (c1x - p0x);
                let tang_y = 3.0 * (c1y - p0y);
                let tang_len = (tang_x * tang_x + tang_y * tang_y).sqrt();
                if tang_len > 0.001 {
                    let dir_x = tang_x / tang_len;
                    let dir_y = tang_y / tang_len;
                    let perp_x = -dir_y;
                    let perp_y = dir_x;
                    // Arrow points backward along tangent (toward start)
                    let base_x = tip_x + dir_x * bez.arrow_size;
                    let base_y = tip_y + dir_y * bez.arrow_size;
                    let half_arrow = bez.arrow_size * 0.5;

                    use crate::plot::series::candle::Vertex as OV;
                    verts.push(OV {
                        position: [tip_x, tip_y],
                        color,
                    });
                    verts.push(OV {
                        position: [base_x + perp_x * half_arrow, base_y + perp_y * half_arrow],
                        color,
                    });
                    verts.push(OV {
                        position: [base_x - perp_x * half_arrow, base_y - perp_y * half_arrow],
                        color,
                    });
                }
            }

            // Arrowhead at end — tangent B'(1) = 3(P1 - C2)
            if bez.arrow_end && points.len() >= 2 {
                let (tip_x, tip_y) = *points.last().unwrap();
                let tang_x = 3.0 * (p1x - c2x);
                let tang_y = 3.0 * (p1y - c2y);
                let tang_len = (tang_x * tang_x + tang_y * tang_y).sqrt();
                if tang_len > 0.001 {
                    let dir_x = tang_x / tang_len;
                    let dir_y = tang_y / tang_len;
                    let perp_x = -dir_y;
                    let perp_y = dir_x;
                    let base_x = tip_x - dir_x * bez.arrow_size;
                    let base_y = tip_y - dir_y * bez.arrow_size;
                    let half_arrow = bez.arrow_size * 0.5;

                    use crate::plot::series::candle::Vertex as OV;
                    verts.push(OV {
                        position: [tip_x, tip_y],
                        color,
                    });
                    verts.push(OV {
                        position: [base_x + perp_x * half_arrow, base_y + perp_y * half_arrow],
                        color,
                    });
                    verts.push(OV {
                        position: [base_x - perp_x * half_arrow, base_y - perp_y * half_arrow],
                        color,
                    });
                }
            }
        }

        verts
    }

    /// Build overlay vertices for PlotStyledPath markers.
    /// Each segment pair rendered with its own color/width/dash.
    fn build_styled_path_vertices(&self, w: f32, h: f32) -> Vec<OverlayVertex> {
        if self.styled_paths.is_empty() {
            return Vec::new();
        }

        let transform = self.transform(w, h);
        let mut verts = Vec::new();

        for path in &self.styled_paths {
            if path.segments.len() < 2 {
                continue;
            }

            let is_hovered = self.hovered_styled_path == Some(path.id);

            for i in 0..path.segments.len() - 1 {
                let seg = &path.segments[i];
                let next = &path.segments[i + 1];

                let ax = transform.x_to_px(seg.x);
                let ay = transform.y_to_px(seg.y);
                let bx = transform.x_to_px(next.x);
                let by = transform.y_to_px(next.y);

                let dx = bx - ax;
                let dy = by - ay;
                let seg_len = (dx * dx + dy * dy).sqrt();
                if seg_len < 0.001 {
                    continue;
                }
                let dir_x = dx / seg_len;
                let dir_y = dy / seg_len;
                let perp_x = -dir_y;
                let perp_y = dir_x;

                let color = if is_hovered {
                    seg.color.with_alpha(1.0).to_array()
                } else {
                    seg.color.to_array()
                };
                let half_w = if is_hovered {
                    (seg.width + 1.0) / 2.0
                } else {
                    seg.width / 2.0
                };

                match seg.line_style {
                    PlotLineStyle::Solid => {
                        Self::push_segment(
                            &mut verts, ax, ay, bx, by, perp_x, perp_y, half_w, color,
                        );
                    }
                    PlotLineStyle::Dashed { dash, gap }
                    | PlotLineStyle::Dotted { dot: dash, gap } => {
                        let mut t = 0.0;
                        while t < seg_len {
                            let t_end = (t + dash).min(seg_len);
                            let sx = ax + dir_x * t;
                            let sy = ay + dir_y * t;
                            let ex = ax + dir_x * t_end;
                            let ey = ay + dir_y * t_end;
                            Self::push_segment(
                                &mut verts, sx, sy, ex, ey, perp_x, perp_y, half_w, color,
                            );
                            t += dash + gap;
                        }
                    }
                }
            }
        }

        verts
    }

    fn build_path_vertices(&self, w: f32, h: f32) -> Vec<OverlayVertex> {
        if self.paths.is_empty() {
            return Vec::new();
        }

        let transform = self.transform(w, h);
        let mut verts = Vec::new();

        for path in &self.paths {
            if path.points.len() < 2 {
                continue;
            }

            let is_hovered = self.hovered_path == Some(path.id);
            let path_color = self.resolve_default_color(
                path.color,
                crate::core::paint::Color::from_srgb(1.0, 1.0, 1.0, 0.8),
                self.color_scheme.annotation_line,
            );
            let color = if is_hovered {
                path_color.with_alpha(1.0).to_array()
            } else {
                path_color.to_array()
            };
            let half_w = if is_hovered {
                (path.width + 1.0) / 2.0
            } else {
                path.width / 2.0
            };

            let screen: Vec<(f32, f32)> = path
                .points
                .iter()
                .map(|(x, y)| (transform.x_to_px(*x), transform.y_to_px(*y)))
                .collect();

            let n = screen.len();
            let last = if path.closed { n } else { n - 1 };

            for i in 0..last {
                let (ax, ay) = screen[i];
                let (bx, by) = screen[(i + 1) % n];
                let dx = bx - ax;
                let dy = by - ay;
                let seg_len = (dx * dx + dy * dy).sqrt();
                if seg_len < 0.001 {
                    continue;
                }
                let dir_x = dx / seg_len;
                let dir_y = dy / seg_len;
                let perp_x = -dir_y;
                let perp_y = dir_x;

                match path.line_style {
                    PlotLineStyle::Solid => {
                        Self::push_segment(
                            &mut verts, ax, ay, bx, by, perp_x, perp_y, half_w, color,
                        );
                    }
                    PlotLineStyle::Dashed { dash, gap }
                    | PlotLineStyle::Dotted { dot: dash, gap } => {
                        let mut t = 0.0;
                        while t < seg_len {
                            let t_end = (t + dash).min(seg_len);
                            let sx = ax + dir_x * t;
                            let sy = ay + dir_y * t;
                            let ex = ax + dir_x * t_end;
                            let ey = ay + dir_y * t_end;
                            Self::push_segment(
                                &mut verts, sx, sy, ex, ey, perp_x, perp_y, half_w, color,
                            );
                            t += dash + gap;
                        }
                    }
                }
            }

            // Arrowhead at end
            if path.arrow_end && screen.len() >= 2 {
                let (tip_x, tip_y) = *screen.last().unwrap();
                let (prev_x, prev_y) = screen[screen.len() - 2];
                let dx = tip_x - prev_x;
                let dy = tip_y - prev_y;
                let len = (dx * dx + dy * dy).sqrt();
                if len > 0.001 {
                    let dir_x = dx / len;
                    let dir_y = dy / len;
                    let perp_x = -dir_y;
                    let perp_y = dir_x;
                    let base_x = tip_x - dir_x * path.arrow_size;
                    let base_y = tip_y - dir_y * path.arrow_size;
                    let half_arrow = path.arrow_size * 0.5;

                    use crate::plot::series::candle::Vertex as OV;
                    verts.push(OV {
                        position: [tip_x, tip_y],
                        color,
                    });
                    verts.push(OV {
                        position: [base_x + perp_x * half_arrow, base_y + perp_y * half_arrow],
                        color,
                    });
                    verts.push(OV {
                        position: [base_x - perp_x * half_arrow, base_y - perp_y * half_arrow],
                        color,
                    });
                }
            }
        }

        verts
    }

    /// Constrain view state to data bounds.
    ///
    /// Called after pan/zoom operations. Clamps viewport ranges to
    /// prevent scrolling past data boundaries.
    fn constrain_to_bounds(&mut self) {
        let vs = &mut self.view_state;
        let bounds = &self.data_bounds;

        if let Some(x_min) = bounds.x_min {
            if vs.x_min < x_min {
                let shift = x_min - vs.x_min;
                vs.x_min += shift;
                vs.x_max += shift;
            }
        }
        if let Some(x_max) = bounds.x_max {
            if vs.x_max > x_max {
                let shift = vs.x_max - x_max;
                vs.x_min -= shift;
                vs.x_max -= shift;
            }
        }
        if let Some(y_min) = bounds.y_min {
            if vs.y_min < y_min {
                let shift = y_min - vs.y_min;
                vs.y_min += shift;
                vs.y_max += shift;
            }
        }
        if let Some(y_max) = bounds.y_max {
            if vs.y_max > y_max {
                let shift = vs.y_max - y_max;
                vs.y_min -= shift;
                vs.y_max -= shift;
            }
        }
        self.ensure_log_axis_domains();
    }

    fn pan_drag_delta(&mut self, dx: f32, dy: f32, width: f32, height: f32, free_pan: bool) {
        if width <= 0.0 || height <= 0.0 {
            return;
        }

        if free_pan {
            let world_dx = -(dx as f64 / width as f64) * self.view_state.x_span();
            let world_dy = (dy as f64 / height as f64) * self.view_state.y_span();
            self.view_state.pan(world_dx, world_dy);
            self.view_state.follow_latest = false;
            self.manual_y = true;
            self.constrain_to_bounds();
            self.push_to_authorities();
        } else if dx.abs() > 0.0 {
            let prev_x_min = self.view_state.x_min;
            let world_dx = -(dx as f64 / width as f64) * self.view_state.x_span();
            self.view_state.pan(world_dx, 0.0);
            self.view_state.follow_latest = false;
            self.constrain_to_bounds();
            if self.view_state.x_min != prev_x_min {
                self.auto_scale_y(self.y_padding);
            }
            self.push_to_authorities();
        }
    }

    fn apply_market_detail_navigation(&mut self) -> bool {
        let fraction = self
            .market_detail_link
            .as_ref()
            .map(MarketDetailLink::take_price_pan)
            .unwrap_or(0.0);
        if !fraction.is_finite() || fraction == 0.0 {
            return false;
        }

        match self.y_axis.kind {
            AxisKind::Time | AxisKind::Linear => {
                let world_delta = fraction * self.view_state.y_span();
                self.view_state.pan(0.0, world_delta);
            }
            AxisKind::Log => {
                if self.view_state.y_min <= 0.0 || self.view_state.y_max <= 0.0 {
                    return false;
                }
                let axis_min = self.view_state.y_min.log10();
                let axis_max = self.view_state.y_max.log10();
                let axis_delta = fraction * (axis_max - axis_min);
                self.view_state.y_min = 10_f64.powf(axis_min + axis_delta);
                self.view_state.y_max = 10_f64.powf(axis_max + axis_delta);
            }
        }
        self.manual_y = true;
        self.constrain_to_bounds();
        self.ensure_log_axis_domains();
        self.push_to_authorities();
        true
    }

    #[cfg(test)]
    pub(crate) fn pan_drag_for_test(
        &mut self,
        dx: f32,
        dy: f32,
        width: f32,
        height: f32,
        free_pan: bool,
    ) {
        self.pan_drag_delta(dx, dy, width, height, free_pan);
    }

    #[cfg(test)]
    pub(crate) fn hit_entry_count_for_test(&self) -> usize {
        self.hit_entries.len()
    }

    #[cfg(test)]
    pub(crate) fn finish_horizontal_pan_for_test(&mut self) -> bool {
        self.finish_horizontal_pan()
    }

    #[cfg(test)]
    pub(crate) fn apply_market_detail_navigation_for_test(&mut self) -> bool {
        self.apply_market_detail_navigation()
    }

    /// Sync view state from shared axis authorities.
    ///
    /// Called at the start of each frame. If a shared authority has been
    /// updated (generation changed), the corresponding axis range in
    /// ViewState is updated to match.
    fn sync_from_authorities(&mut self) {
        let x_changed = if let Some(ref authority) = self.shared_x {
            if let Ok(auth) = authority.lock() {
                if auth.generation() != self.cached_x_generation {
                    let (min, max) = auth.range();
                    self.view_state.x_min = min;
                    self.view_state.x_max = max;
                    self.cached_x_generation = auth.generation();
                    true
                } else {
                    false
                }
            } else {
                false
            }
        } else {
            false
        };
        if x_changed {
            self.auto_scale_y(self.y_padding);
        }
        if let Some(ref authority) = self.shared_y {
            if let Ok(auth) = authority.lock() {
                if auth.generation() != self.cached_y_generation {
                    let (min, max) = auth.range();
                    self.view_state.y_min = min;
                    self.view_state.y_max = max;
                    self.cached_y_generation = auth.generation();
                }
            }
        }
        self.ensure_log_axis_domains();
    }

    /// Push current view state to shared axis authorities.
    ///
    /// Called after pan/zoom. Updates the shared authority so other
    /// Plots reading the same authority see the change.
    fn push_to_authorities(&self) {
        if let Some(ref authority) = self.shared_x {
            if let Ok(mut auth) = authority.lock() {
                auth.set_range(self.view_state.x_min, self.view_state.x_max);
            }
        }
        if let Some(ref authority) = self.shared_y {
            if let Ok(mut auth) = authority.lock() {
                auth.set_range(self.view_state.y_min, self.view_state.y_max);
            }
        }
        if !self.gpu_initialized
            && let Some(link) = self.market_detail_link.as_ref()
        {
            link.set_presented_price_range(self.view_state.y_min, self.view_state.y_max);
        }
    }

    /// Toggle visibility of all series with the given z_order.
    fn toggle_series_by_z_order(&mut self, z_order: u32) {
        // Find matching series indices from hit_entries metadata
        let matching: Vec<usize> = self
            .hit_entries
            .iter()
            .enumerate()
            .filter(|(_, e)| e.meta.z_order == z_order)
            .map(|(i, _)| i)
            .collect();

        for idx in matching {
            if self.hidden_series.contains(&idx) {
                self.hidden_series.remove(&idx);
            } else {
                self.hidden_series.insert(idx);
            }
        }
    }

    fn candle_x_extent(&self) -> Option<(f64, f64)> {
        let mut earliest_x = f64::INFINITY;
        let mut latest_x = f64::NEG_INFINITY;
        for entry in &self.hit_entries {
            if let hit_proxy::HitProxy::Candle { candles } = &entry.proxy {
                if let Some(first) = candles.first() {
                    earliest_x = earliest_x.min(first.x_open);
                }
                if let Some(last) = candles.last() {
                    latest_x = latest_x.max(last.x_close);
                }
            }
        }

        (earliest_x.is_finite() && latest_x.is_finite()).then_some((earliest_x, latest_x))
    }

    fn align_x_to_latest(&mut self, latest_x: f64, span: f64) {
        if !latest_x.is_finite() || !span.is_finite() || span <= 0.0 {
            return;
        }
        let right_padding = span * LIVE_FOLLOW_RIGHT_PADDING_FRACTION;
        self.view_state.x_max = latest_x + right_padding;
        self.view_state.x_min = self.view_state.x_max - span;
    }

    fn finish_horizontal_pan(&mut self) -> bool {
        let span = self.view_state.x_span();
        let Some((_, latest_x)) = self.candle_x_extent() else {
            return false;
        };
        if !span.is_finite() || span <= 0.0 {
            return false;
        }

        let target_gap = span * LIVE_FOLLOW_RIGHT_PADDING_FRACTION;
        let tolerance = span * LIVE_FOLLOW_REARM_TOLERANCE_FRACTION;
        let current_gap = self.view_state.x_max - latest_x;
        if !current_gap.is_finite() || (current_gap - target_gap).abs() > tolerance {
            return false;
        }

        self.view_state.follow_latest = true;
        self.align_x_to_latest(latest_x, span);
        self.constrain_to_bounds();
        self.auto_scale_y(self.y_padding);
        self.push_to_authorities();
        true
    }

    /// Reset viewport to show latest data. Re-enables auto-follow.
    fn handle_reset_view(&mut self) {
        self.manual_y = false;
        let current_span = self.view_state.x_span();
        let span = if self.initial_x_span.is_finite() && self.initial_x_span > 0.0 {
            self.initial_x_span
        } else if current_span.is_finite() && current_span > 0.0 {
            current_span
        } else {
            1.0
        };

        if let Some((_, latest_x)) = self.candle_x_extent() {
            self.align_x_to_latest(latest_x, span);
        } else if let Some(x_max) = self.data_bounds.x_max {
            self.align_x_to_latest(x_max, span);
        }

        self.view_state.follow_latest = true;
        self.auto_scale_y(self.y_padding);
        self.constrain_to_bounds();
        self.push_to_authorities();
    }

    /// Update the shared Probe with world coordinates from a local pixel position.
    fn update_probe(&self, local_x: f32, local_y: f32) {
        if let Some(ref probe) = self.probe {
            let w = self.layout_rect.width;
            let h = self.layout_rect.height;
            if w > 0.0 && h > 0.0 {
                let transform = self.transform(w, h);
                let world_x = transform.px_to_x(local_x);
                let world_y = transform.px_to_y(local_y);
                if let Ok(mut p) = probe.lock() {
                    p.set(Some(world_x), Some(world_y), self.id);
                }
            }
        }
    }
    fn paint_axis_labels(&self, cx: &mut PaintContext, rect: RectF) {
        use crate::core::paint::LAYER_CONTENT;
        use crate::plot::grid::{calculate_axis_ticks, format_value_for_interval};

        let config = match &self.label_config {
            Some(c) => c,
            None => return,
        };

        let font_size = config.font_size;
        let label_color = config.color;
        let bg_color = config.bg_color;
        let padding = 4.0_f32;
        let bg_layer: u32 = LAYER_CONTENT + 1;

        // Use the ViewState that the worker last rendered, so labels
        // match the texture content. Fall back to current if worker
        // hasn't rendered yet.
        let vs = self.rendered_view_state();
        let transform = self.transform_from_view_state(vs, rect.width, rect.height);

        // Y-axis labels (right edge)
        let y_ticks =
            calculate_axis_ticks(self.y_axis.kind, vs.y_min, vs.y_max, config.y_tick_count);
        let y_tick_interval = if y_ticks.len() >= 2 {
            (y_ticks[1] - y_ticks[0]).abs()
        } else {
            vs.y_max - vs.y_min
        };
        for &tick in &y_ticks {
            let py = transform.y_to_px(tick);

            let text = match &config.y_formatter {
                Some(f) => f(crate::plot::axis::AxisFormatContext {
                    value: tick,
                    range_min: vs.y_min,
                    range_max: vs.y_max,
                    tick_interval: y_tick_interval,
                }),
                None => format_value_for_interval(tick, y_tick_interval),
            };
            let (text_w, _) = cx.text_engine.measure(&text, font_size);
            let metrics = cx.text_engine.metrics(&text, font_size);
            let text_h = metrics.line_height;

            // Vertically centered on tick, clamped to rect bounds
            let label_y = (rect.y + py - text_h * 0.5)
                .max(rect.y)
                .min(rect.y + rect.height - text_h);

            if label_y < rect.y || label_y + text_h > rect.y + rect.height {
                continue;
            }

            let label_x = rect.x + rect.width - text_w - padding;

            cx.scene.push_rect(
                RectF::new(label_x - padding * 0.5, label_y, text_w + padding, text_h),
                bg_color,
                bg_layer,
                None,
            );

            let run = cx.text_engine.shape(&text, font_size);
            cx.text_engine.draw(
                &run,
                label_x,
                label_y,
                label_color,
                crate::core::paint::LAYER_CONTENT + 1,
                cx.scene,
            );
        }

        // X-axis labels (bottom edge)
        let x_ticks =
            calculate_axis_ticks(self.x_axis.kind, vs.x_min, vs.x_max, config.x_tick_count);
        let x_tick_interval = if x_ticks.len() >= 2 {
            (x_ticks[1] - x_ticks[0]).abs()
        } else {
            vs.x_max - vs.x_min
        };
        for &tick in &x_ticks {
            let px = transform.x_to_px(tick);

            let text = match &config.x_formatter {
                Some(f) => f(crate::plot::axis::AxisFormatContext {
                    value: tick,
                    range_min: vs.x_min,
                    range_max: vs.x_max,
                    tick_interval: x_tick_interval,
                }),
                None => format_value_for_interval(tick, x_tick_interval),
            };
            let (text_w, _) = cx.text_engine.measure(&text, font_size);
            let metrics = cx.text_engine.metrics(&text, font_size);
            let text_h = metrics.line_height;

            // Centered on tick, clamped to rect bounds
            let label_x = (rect.x + px - text_w * 0.5)
                .max(rect.x)
                .min(rect.x + rect.width - text_w);

            if label_x < rect.x || label_x + text_w > rect.x + rect.width {
                continue;
            }

            let label_y = rect.y + rect.height - text_h - padding;

            cx.scene.push_rect(
                RectF::new(label_x - padding * 0.5, label_y, text_w + padding, text_h),
                bg_color,
                bg_layer,
                None,
            );

            let run = cx.text_engine.shape(&text, font_size);
            cx.text_engine.draw(
                &run,
                label_x,
                label_y,
                label_color,
                crate::core::paint::LAYER_CONTENT + 1,
                cx.scene,
            );
        }
    }

    fn hit_test_price_levels(&self, screen_y: f32) -> Option<u64> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let transform = self.transform(w, h);
        let threshold = 5.0;

        // Draggable levels first
        for level in &self.price_levels {
            if !level.draggable {
                continue;
            }
            let level_py = transform.y_to_px(level.display_y);
            if (screen_y - level_py).abs() <= threshold {
                return Some(level.id);
            }
        }
        // Then non-draggable
        for level in &self.price_levels {
            if level.draggable {
                continue;
            }
            let level_py = transform.y_to_px(level.display_y);
            if (screen_y - level_py).abs() <= threshold {
                return Some(level.id);
            }
        }
        None
    }

    fn hit_test_circles(&self, lx: f32, ly: f32) -> Option<u64> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let transform = self.transform(w, h);
        let threshold = 3.0; // extra px tolerance beyond radius

        let mut best_dist = f32::INFINITY;
        let mut best_id = None;

        for circle in &self.circles {
            let cx = transform.x_to_px(circle.x);
            let cy = transform.y_to_px(circle.y);
            let dist = ((lx - cx).powi(2) + (ly - cy).powi(2)).sqrt();
            let effective = dist - circle.radius;
            if effective <= threshold && effective < best_dist {
                best_dist = effective;
                best_id = Some(circle.id);
            }
        }

        best_id
    }

    fn hit_test_lines(&self, lx: f32, ly: f32) -> Option<u64> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let transform = self.transform(w, h);
        let threshold = 5.0;

        let mut best_dist = f32::INFINITY;
        let mut best_id = None;

        for line in &self.lines {
            let px1 = transform.x_to_px(line.x1);
            let py1 = transform.y_to_px(line.y1);
            let px2 = transform.x_to_px(line.x2);
            let py2 = transform.y_to_px(line.y2);

            let dx = px2 - px1;
            let dy = py2 - py1;
            let len_sq = dx * dx + dy * dy;
            if len_sq < 0.001 {
                continue;
            }

            let t = ((lx - px1) * dx + (ly - py1) * dy) / len_sq;
            let t = t.clamp(0.0, 1.0);
            let closest_x = px1 + t * dx;
            let closest_y = py1 + t * dy;
            let dist = ((lx - closest_x).powi(2) + (ly - closest_y).powi(2)).sqrt();
            let effective = dist - line.width / 2.0;

            if effective <= threshold && effective < best_dist {
                best_dist = effective;
                best_id = Some(line.id);
            }
        }

        best_id
    }

    fn hit_test_beziers(&self, lx: f32, ly: f32) -> Option<u64> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let transform = self.transform(w, h);
        let threshold = 5.0;
        let samples = 32;

        let mut best_dist = f32::INFINITY;
        let mut best_id = None;

        for bez in &self.beziers {
            let p0x = transform.x_to_px(bez.x1);
            let p0y = transform.y_to_px(bez.y1);
            let c1x = transform.x_to_px(bez.cx1);
            let c1y = transform.y_to_px(bez.cy1);
            let c2x = transform.x_to_px(bez.cx2);
            let c2y = transform.y_to_px(bez.cy2);
            let p1x = transform.x_to_px(bez.x2);
            let p1y = transform.y_to_px(bez.y2);

            for i in 0..=samples {
                let t = i as f32 / samples as f32;
                let mt = 1.0 - t;
                let x = mt * mt * mt * p0x
                    + 3.0 * mt * mt * t * c1x
                    + 3.0 * mt * t * t * c2x
                    + t * t * t * p1x;
                let y = mt * mt * mt * p0y
                    + 3.0 * mt * mt * t * c1y
                    + 3.0 * mt * t * t * c2y
                    + t * t * t * p1y;

                let dist = ((lx - x).powi(2) + (ly - y).powi(2)).sqrt();
                let effective = dist - bez.width / 2.0;
                if effective < best_dist {
                    best_dist = effective;
                    best_id = Some(bez.id);
                }
            }
        }

        if best_dist <= threshold {
            best_id
        } else {
            None
        }
    }

    fn hit_test_styled_paths(&self, lx: f32, ly: f32) -> Option<u64> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let transform = self.transform(w, h);
        let threshold = 5.0;

        let mut best_dist = f32::INFINITY;
        let mut best_id = None;

        for path in &self.styled_paths {
            if path.segments.len() < 2 {
                continue;
            }

            for i in 0..path.segments.len() - 1 {
                let seg = &path.segments[i];
                let next = &path.segments[i + 1];

                let ax = transform.x_to_px(seg.x);
                let ay = transform.y_to_px(seg.y);
                let bx = transform.x_to_px(next.x);
                let by = transform.y_to_px(next.y);

                let dx = bx - ax;
                let dy = by - ay;
                let len_sq = dx * dx + dy * dy;
                if len_sq < 0.001 {
                    continue;
                }

                let t = ((lx - ax) * dx + (ly - ay) * dy) / len_sq;
                let t = t.clamp(0.0, 1.0);
                let closest_x = ax + t * dx;
                let closest_y = ay + t * dy;
                let dist = ((lx - closest_x).powi(2) + (ly - closest_y).powi(2)).sqrt();
                let effective = dist - seg.width / 2.0;

                if effective < best_dist {
                    best_dist = effective;
                    best_id = Some(path.id);
                }
            }
        }

        if best_dist <= threshold {
            best_id
        } else {
            None
        }
    }

    fn hit_test_paths(&self, lx: f32, ly: f32) -> Option<u64> {
        let w = self.layout_rect.width;
        let h = self.layout_rect.height;
        if w <= 0.0 || h <= 0.0 {
            return None;
        }
        let transform = self.transform(w, h);
        let threshold = 5.0;

        let mut best_dist = f32::INFINITY;
        let mut best_id = None;

        for path in &self.paths {
            if path.points.len() < 2 {
                continue;
            }

            let n = path.points.len();
            let last = if path.closed { n } else { n - 1 };

            for i in 0..last {
                let (sx, sy) = path.points[i];
                let (ex, ey) = path.points[(i + 1) % n];
                let ax = transform.x_to_px(sx);
                let ay = transform.y_to_px(sy);
                let bx = transform.x_to_px(ex);
                let by = transform.y_to_px(ey);

                let dx = bx - ax;
                let dy = by - ay;
                let len_sq = dx * dx + dy * dy;
                if len_sq < 0.001 {
                    continue;
                }

                let t = ((lx - ax) * dx + (ly - ay) * dy) / len_sq;
                let t = t.clamp(0.0, 1.0);
                let closest_x = ax + t * dx;
                let closest_y = ay + t * dy;
                let dist = ((lx - closest_x).powi(2) + (ly - closest_y).powi(2)).sqrt();
                let effective = dist - path.width / 2.0;

                if effective < best_dist {
                    best_dist = effective;
                    best_id = Some(path.id);
                }
            }
        }

        if best_dist <= threshold {
            best_id
        } else {
            None
        }
    }

    fn rendered_view_state(&self) -> &ViewState {
        self.presented_labels
            .as_ref()
            .map_or(&self.view_state, |labels| &labels.view_state)
    }

    fn paint_level_tags(&self, cx: &mut PaintContext, rect: RectF, has_image: bool) {
        let vs = self.rendered_view_state();
        let transform = self.transform_from_view_state(vs, rect.width, rect.height);
        let published = self.presented_labels.as_ref();
        let reference = published
            .filter(|_| has_image)
            .and_then(|labels| labels.reference_tag.as_ref());
        let retained_reference =
            reference.filter(|tag| !self.price_levels.iter().any(|level| level.id == tag.id));
        let levels = self.price_levels.iter().filter_map(|level| {
            if let Some(tag) = reference.filter(|tag| tag.id == level.id) {
                return Some(tag);
            }
            // The nominated market tag comes from the presented Image's snapshot.
            if self.market_detail_reference_level == Some(level.id) {
                return None;
            }
            Some(level)
        });

        let font_size = 10.0;
        let line_height = 12.0;
        let padding_x = 6.0;
        let padding_y = 3.0;

        for level in levels.chain(retained_reference) {
            if level.tag.is_empty() {
                continue;
            }
            if level.display_y < vs.y_min || level.display_y > vs.y_max {
                continue;
            }

            // Measure tag width from actual text
            let tag_w = level
                .tag
                .iter()
                .map(|line| {
                    let (w, _) = cx.text_engine.measure(&line.text, font_size);
                    w
                })
                .fold(0.0f32, f32::max)
                + padding_x * 2.0;

            let y_px = transform.y_to_px(level.display_y);

            let tag_x = if let Some((cx_logical, _)) = self.crosshair_pos {
                let x0 = (cx_logical + 3.0).min(rect.width - tag_w);
                rect.x + x0 + padding_x
            } else {
                match level.tag_side {
                    TagSide::Right => rect.x + rect.width - tag_w - 4.0 + padding_x,
                    TagSide::Left => rect.x + 4.0 + padding_x,
                }
            };

            let tag_y = rect.y + y_px + 2.0;

            for (i, line) in level.tag.iter().enumerate() {
                let text_y = tag_y + i as f32 * line_height;
                let color = if line.dimmed {
                    let c = line.color.unwrap_or_else(|| {
                        self.resolve_default_color(
                            level.color,
                            crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.8),
                            self.color_scheme.annotation_neutral,
                        )
                    });
                    c.with_alpha(c.a * 0.6)
                } else {
                    line.color.unwrap_or(self.color_scheme.annotation_text)
                };

                let run = cx.text_engine.shape(&line.text, font_size);
                cx.text_engine.draw(
                    &run,
                    tag_x,
                    text_y,
                    color,
                    crate::core::paint::LAYER_CONTENT + 1,
                    cx.scene,
                );
            }
        }

        // PlotTag text (freestanding labels)
        for tag_item in &self.tags {
            if tag_item.tag.is_empty() {
                continue;
            }
            if tag_item.x < vs.x_min
                || tag_item.x > vs.x_max
                || tag_item.y < vs.y_min
                || tag_item.y > vs.y_max
            {
                continue;
            }

            let anchor_x_px = transform.x_to_px(tag_item.x);
            let anchor_y_px = transform.y_to_px(tag_item.y);
            let t_h = tag_item.tag.len() as f32 * line_height + padding_y * 2.0;

            // Measure tag width from actual text
            let tag_w = tag_item
                .tag
                .iter()
                .map(|line| {
                    let (w, _) = cx.text_engine.measure(&line.text, font_size);
                    w
                })
                .fold(0.0f32, f32::max)
                + padding_x * 2.0;

            let (tx, ty) = match tag_item.anchor {
                TagAnchor::Above => (anchor_x_px - tag_w / 2.0, anchor_y_px - t_h - 4.0),
                TagAnchor::Below => (anchor_x_px - tag_w / 2.0, anchor_y_px + 4.0),
                TagAnchor::Right => (anchor_x_px + 4.0, anchor_y_px - t_h / 2.0),
                TagAnchor::Left => (anchor_x_px - tag_w - 4.0, anchor_y_px - t_h / 2.0),
            };

            let text_x = rect.x + tx + padding_x;
            let text_y_start = rect.y + ty + padding_y;

            for (i, line) in tag_item.tag.iter().enumerate() {
                let text_y = text_y_start + i as f32 * line_height;
                let color = if line.dimmed {
                    let c = line.color.unwrap_or_else(|| {
                        self.resolve_default_color(
                            tag_item.color,
                            crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.8),
                            self.color_scheme.annotation_neutral,
                        )
                    });
                    c.with_alpha(c.a * 0.6)
                } else {
                    line.color.unwrap_or(self.color_scheme.annotation_text)
                };

                let run = cx.text_engine.shape(&line.text, font_size);
                cx.text_engine.draw(
                    &run,
                    text_x,
                    text_y,
                    color,
                    crate::core::paint::LAYER_CONTENT + 1,
                    cx.scene,
                );
            }
        }

        // PlotRegion labels (centered in region)
        for region in &self.regions {
            if let Some(ref label) = region.label {
                let rx_min = region
                    .x_min
                    .map(|x| transform.x_to_px(x))
                    .unwrap_or(0.0)
                    .max(0.0);
                let rx_max = region
                    .x_max
                    .map(|x| transform.x_to_px(x))
                    .unwrap_or(rect.width)
                    .min(rect.width);
                let ry_min = region.y_min.map(|y| transform.y_to_px(y)).unwrap_or(0.0);
                let ry_max = region
                    .y_max
                    .map(|y| transform.y_to_px(y))
                    .unwrap_or(rect.height);

                let (top, bottom) = if ry_min < ry_max {
                    (ry_min, ry_max)
                } else {
                    (ry_max, ry_min)
                };

                let center_x = rect.x + (rx_min + rx_max) / 2.0;
                let center_y = rect.y + (top + bottom) / 2.0;

                if center_x < rect.x
                    || center_x > rect.x + rect.width
                    || center_y < rect.y
                    || center_y > rect.y + rect.height
                {
                    continue;
                }

                let fill_color = self.resolve_default_color(
                    region.fill_color,
                    crate::core::paint::Color::from_srgb(0.5, 0.5, 0.5, 0.1),
                    self.color_scheme.annotation_fill,
                );
                let color = fill_color.with_alpha((fill_color.a * 3.0).min(1.0));

                let (text_w, _) = cx.text_engine.measure(label, font_size);
                let text_x = center_x - text_w / 2.0;
                let run = cx.text_engine.shape(label, font_size);
                cx.text_engine.draw(
                    &run,
                    text_x,
                    center_y,
                    color,
                    crate::core::paint::LAYER_CONTENT + 1,
                    cx.scene,
                );
            }
        }
    }

    fn paint_chart_info(&self, cx: &mut PaintContext, rect: RectF) {
        let font_size = 11.0;
        let line_h = 14.0;
        let overlay_rect = chart_info_overlay_rect(rect);
        let text_x = overlay_rect.x;
        let mut text_y = overlay_rect.y;

        let title_color = self.color_scheme.info_title;
        let _data_color = self.color_scheme.info_data;

        // Static info lines (always visible)
        for line in &self.chart_info {
            let run = cx.text_engine.shape(line, font_size);
            cx.text_engine.draw(
                &run,
                text_x,
                text_y,
                title_color,
                crate::core::paint::LAYER_CONTENT + 1,
                cx.scene,
            );
            text_y += line_h;
        }

        // OHLCV for nearest candle (only when crosshair active)
        if let Some((cx_local, _)) = self.crosshair_pos {
            if !self.is_crosshair_suppressed() {
                let transform = self.transform_from_view_state(
                    self.rendered_view_state(),
                    rect.width,
                    rect.height,
                );
                let world_x = transform.px_to_x(cx_local);

                for entry in &self.hit_entries {
                    if let hit_proxy::HitProxy::Candle { candles } = &entry.proxy {
                        if candles.is_empty() {
                            continue;
                        }

                        let idx =
                            candles.partition_point(|c| (c.x_open + c.x_close) / 2.0 < world_x);

                        let mut best: Option<&RenderCandle> = None;
                        let mut best_dist = f64::INFINITY;
                        for &i in &[idx.saturating_sub(1), idx.min(candles.len() - 1)] {
                            let c = &candles[i];
                            let mid = (c.x_open + c.x_close) / 2.0;
                            let d = (mid - world_x).abs();
                            if d < best_dist {
                                best_dist = d;
                                best = Some(c);
                            }
                        }

                        if let Some(c) = best {
                            let candle_w = (c.x_close - c.x_open).abs();
                            if best_dist <= candle_w * 2.0 {
                                let (o, cl) = if c.is_up {
                                    (c.body_bottom, c.body_top)
                                } else {
                                    (c.body_top, c.body_bottom)
                                };

                                let label_color = self.color_scheme.info_label;
                                let value_color = self.color_scheme.info_data;
                                let close_color = if c.is_up {
                                    self.color_scheme.info_up
                                } else {
                                    self.color_scheme.info_down
                                };

                                let gap = 6.0_f32;
                                let mut x = text_x;

                                macro_rules! draw_pair {
                                    ($label:expr, $lc:expr, $value:expr, $vc:expr) => {{
                                        let lbl = $label;
                                        let val = $value;
                                        let lbl_run = cx.text_engine.shape(lbl, font_size);
                                        let (lw, _) = cx.text_engine.measure(lbl, font_size);
                                        cx.text_engine.draw(
                                            &lbl_run,
                                            x,
                                            text_y,
                                            $lc,
                                            crate::core::paint::LAYER_CONTENT + 1,
                                            cx.scene,
                                        );
                                        let val_run = cx.text_engine.shape(val, font_size);
                                        let (vw, _) = cx.text_engine.measure(val, font_size);
                                        cx.text_engine.draw(
                                            &val_run,
                                            x + lw,
                                            text_y,
                                            $vc,
                                            crate::core::paint::LAYER_CONTENT + 1,
                                            cx.scene,
                                        );
                                        x += lw + vw + gap;
                                    }};
                                }

                                let o_s = format!("{:.2}", o);
                                let h_s = format!("{:.2}", c.high);
                                let l_s = format!("{:.2}", c.low);
                                let c_s = format!("{:.2}", cl);

                                draw_pair!("O: ", label_color, &o_s, value_color);
                                draw_pair!("H: ", label_color, &h_s, value_color);
                                draw_pair!("L: ", label_color, &l_s, value_color);
                                draw_pair!("C: ", label_color, &c_s, close_color);

                                if c.volume > 0.0 {
                                    let v_s = format!("{:.0}", c.volume);
                                    draw_pair!("V: ", label_color, &v_s, value_color);
                                }
                                let _ = x;
                            }
                        }
                        break;
                    }
                }
            }
        }
    }

    fn paint_crosshair_labels(&self, cx: &mut PaintContext, rect: RectF) {
        use crate::core::paint::LAYER_CONTENT;

        if self.is_crosshair_suppressed() {
            return;
        }

        let (cx_local, cy_local) = match self.crosshair_pos {
            Some(pos) => pos,
            None => return,
        };

        if cx_local < 0.0 || cx_local > rect.width || cy_local < 0.0 || cy_local > rect.height {
            return;
        }

        let config = match &self.label_config {
            Some(lc) => lc,
            None => return,
        };

        let vs = self.rendered_view_state();
        let transform = self.transform_from_view_state(vs, rect.width, rect.height);
        let font_size = config.font_size;

        let bg_color = self.color_scheme.crosshair_label_background;
        let text_color = self.color_scheme.crosshair_label_text;
        let padding_x = 4.0_f32;
        let padding_y = 2.0_f32;
        let label_layer: u32 = LAYER_CONTENT + 2;

        // Y-axis label (right edge)
        let world_y = transform.px_to_y(cy_local);
        let y_text = if let Some(ref fmt) = config.y_formatter {
            fmt(crate::plot::axis::AxisFormatContext {
                value: world_y,
                range_min: vs.y_min,
                range_max: vs.y_max,
                tick_interval: vs.y_max - vs.y_min,
            })
        } else {
            crate::plot::grid::format_value(world_y)
        };
        let (text_w, _) = cx.text_engine.measure(&y_text, font_size);
        let metrics = cx.text_engine.metrics(&y_text, font_size);
        let text_h = metrics.line_height;
        let label_w = text_w + padding_x * 2.0;
        let label_h = text_h + padding_y * 2.0;
        let label_x = rect.x + rect.width - label_w;
        let label_y = (rect.y + cy_local - label_h / 2.0)
            .max(rect.y)
            .min(rect.y + rect.height - label_h);

        cx.scene.push_rect(
            RectF::new(label_x, label_y, label_w, label_h),
            bg_color,
            label_layer,
            None,
        );
        let run = cx.text_engine.shape(&y_text, font_size);
        cx.text_engine.draw(
            &run,
            label_x + padding_x,
            label_y + padding_y,
            text_color,
            label_layer + 1,
            cx.scene,
        );

        // X-axis label (bottom edge)
        let world_x = transform.px_to_x(cx_local);
        let x_text = if let Some(ref fmt) = config.x_formatter {
            fmt(crate::plot::axis::AxisFormatContext {
                value: world_x,
                range_min: vs.x_min,
                range_max: vs.x_max,
                tick_interval: vs.x_max - vs.x_min,
            })
        } else {
            crate::plot::grid::format_value(world_x)
        };
        let (text_w, _) = cx.text_engine.measure(&x_text, font_size);
        let metrics = cx.text_engine.metrics(&x_text, font_size);
        let text_h = metrics.line_height;
        let label_w = text_w + padding_x * 2.0;
        let label_h = text_h + padding_y * 2.0;
        let label_x = (rect.x + cx_local - label_w / 2.0)
            .max(rect.x)
            .min(rect.x + rect.width - label_w);
        let label_y = rect.y + rect.height - label_h;

        cx.scene.push_rect(
            RectF::new(label_x, label_y, label_w, label_h),
            bg_color,
            label_layer,
            None,
        );
        let run = cx.text_engine.shape(&x_text, font_size);
        cx.text_engine.draw(
            &run,
            label_x + padding_x,
            label_y + padding_y,
            text_color,
            label_layer + 1,
            cx.scene,
        );
    }
}

impl CustomRenderParticipant for Plot {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        match metrics.activity {
            CustomRenderActivity::Active {
                physical_render_extent,
                physical_composition_coverage,
            } => self.prepare_active_target(
                metrics,
                physical_render_extent,
                physical_composition_coverage,
            ),
            CustomRenderActivity::Inactive => self.prepare_inactive_target(),
        }
    }
}

impl WidgetDowncast for Plot {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for Plot {
    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        let previous_render_generation = self.render_generation;
        let previous_view_state = self.view_state.clone();

        if let Some(theme) = cx.theme() {
            self.sync_color_scheme(theme);
        }

        // Sync shared axis authorities every frame (receives pushes from other Plots)
        self.sync_from_authorities();
        self.apply_market_detail_navigation();

        // The channel keeps custody until this Update can change both source
        // mirrors. In particular, a worker in Series::prepare cannot stall UI.
        let renderer = self.renderer.clone();
        let mut renderer_guard = renderer.as_ref().map(|renderer| renderer.try_lock());
        let can_update_series = !matches!(
            renderer_guard,
            Some(Err(std::sync::TryLockError::WouldBlock))
        );
        let mut series_changed = false;
        if can_update_series {
            let mut renderer = renderer_guard
                .as_mut()
                .and_then(|r| r.as_mut().ok())
                .map(|r| &mut **r);
            for (index, data) in std::mem::take(&mut self.deferred_series_data) {
                self.apply_series_data(index, data, renderer.as_deref_mut());
                series_changed = true;
            }
            let updates: Vec<DataUpdate> = self
                .data_rx
                .as_ref()
                .map(|rx| rx.try_iter().collect())
                .unwrap_or_default();
            series_changed |= !updates.is_empty();
            for update in updates {
                self.apply_data_update(update, renderer.as_deref_mut());
            }
            if let Some(renderer) = renderer {
                self.series_data_generation = renderer.series_data_generation();
            }
        }
        drop(renderer_guard);
        if series_changed {
            let span = self.view_state.x_max - self.view_state.x_min;
            if let Some((earliest_x, latest_x)) = self.candle_x_extent() {
                let lookahead = span * 0.33;
                self.data_bounds.x_min = Some(earliest_x - lookahead);
                self.data_bounds.x_max = Some(latest_x + lookahead);

                // Keep the latest candle at the stable live edge only while
                // follow mode remains armed. Manual X navigation disarms it.
                if self.view_state.follow_latest {
                    self.align_x_to_latest(latest_x, span);
                }
            }

            self.auto_scale_y(self.y_padding);
            self.constrain_to_bounds();
            self.push_to_authorities();
        }

        // Smooth price level display positions
        let alpha = 1.0 - (-PRICE_LEVEL_SMOOTHING_RATE * dt as f64).exp();
        let drag_id = self.level_drag.as_ref().map(|d| d.level_id);
        let reference_id = self.market_detail_reference_level;
        let mut price_level_moved = false;
        for level in &mut self.price_levels {
            let previous = level.display_y;
            if drag_id == Some(level.id) || reference_id == Some(level.id) {
                level.display_y = level.y;
            } else {
                let delta = level.y - level.display_y;
                let tolerance = f64::EPSILON * level.y.abs().max(1.0) * 8.0;
                if delta.abs() <= tolerance {
                    level.display_y = level.y;
                } else {
                    level.display_y += delta * alpha;
                }
            }
            price_level_moved |= level.display_y != previous;
        }
        if price_level_moved {
            self.render_generation = self.render_generation.wrapping_add(1);
        }
        self.sync_market_detail_reference_line();

        self.consume_resize_completion();
        let completed_render = self.gpu_initialized && self.consume_completed_render();
        if completed_render
            || self.render_generation != previous_render_generation
            || self.view_state != previous_view_state
        {
            cx.invalidate_paint();
        }
    }

    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.sync_from_authorities();

        let size = Size::new(constraints.max_width, constraints.max_height);
        self.layout_rect = RectF::new(0.0, 0.0, size.width, size.height);

        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let image_texture_id = match self.target_state {
            Some(target) if target.active && !target.physical_composition_coverage.is_empty() => {
                self.presented_texture_id
                    .filter(|texture_id| *texture_id == target.texture_id)
            }
            Some(_) => None,
            None => self.presented_texture_id,
        };
        if let Some(binding) = image_texture_id.and_then(|texture_id| {
            self.canonical_front
                .as_ref()
                .filter(|front| front.witness.texture_id() == texture_id)
                .and_then(|front| front.witness.0.required.as_ref())
        }) {
            cx.scene.push_admitted_image(
                rect,
                binding,
                RectF::new(0.0, 0.0, 1.0, 1.0),
                crate::core::paint::Color::WHITE,
                crate::core::paint::LAYER_CONTENT,
                None,
            );
        } else if let Some(texture_id) = image_texture_id {
            cx.scene.push_image(
                rect,
                texture_id,
                RectF::new(0.0, 0.0, 1.0, 1.0),
                crate::core::paint::Color::WHITE,
                crate::core::paint::LAYER_CONTENT,
                None,
            );
        }

        if self.label_config.is_some() && rect.width > 0.0 && rect.height > 0.0 {
            self.paint_axis_labels(cx, rect);
        }

        let has_presented_tag = image_texture_id.is_some()
            && self
                .presented_labels
                .as_ref()
                .is_some_and(|labels| labels.reference_tag.is_some());
        if (has_presented_tag
            || !self.price_levels.is_empty()
            || !self.tags.is_empty()
            || !self.regions.is_empty())
            && rect.width > 0.0
            && rect.height > 0.0
        {
            self.paint_level_tags(cx, rect, image_texture_id.is_some());
        }

        // Chart info header (title + OHLCV)
        if rect.width > 0.0 && rect.height > 0.0 {
            self.paint_chart_info(cx, rect);
        }

        // Crosshair axis value labels
        if self.label_config.is_some() && rect.width > 0.0 && rect.height > 0.0 {
            self.paint_crosshair_labels(cx, rect);
        }

        // Tool gesture label
        if self.tool_gesture_active && self.active_tool_index > 0 {
            if let Some((label, lx, ly)) = self.tools[self.active_tool_index].gesture_label() {
                let color = self.color_scheme.tool_color;
                let font_size = 11.0;
                let run = cx.text_engine.shape(&label, font_size);
                cx.text_engine.draw(
                    &run,
                    rect.x + lx,
                    rect.y + ly,
                    color,
                    crate::core::paint::LAYER_CONTENT + 1,
                    cx.scene,
                );
            }
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        cx.invalidate_paint();

        match event {
            Event::Pointer(pe) => {
                let (lx, ly) = pe.local_pos;
                let inside = lx >= 0.0
                    && lx <= self.layout_rect.width
                    && ly >= 0.0
                    && ly <= self.layout_rect.height;

                match pe.kind {
                    PointerEventKind::Down => {
                        if !inside {
                            return false;
                        }

                        match pe.button {
                            Some(PointerButton::Primary) => {
                                // SPEC-HAL-299: Check if clicking a draggable price level
                                if let Some(level_id) = self.hit_test_price_levels(ly) {
                                    let is_draggable = self
                                        .price_levels
                                        .iter()
                                        .find(|l| l.id == level_id)
                                        .map(|l| l.draggable)
                                        .unwrap_or(false);

                                    if is_draggable {
                                        let original_y = self
                                            .price_levels
                                            .iter()
                                            .find(|l| l.id == level_id)
                                            .map(|l| l.y)
                                            .unwrap();

                                        self.level_drag = Some(LevelDragState {
                                            level_id,
                                            original_y,
                                            current_y: original_y,
                                        });

                                        self.in_capture = true;
                                        cx.capture_pointer(PointerButton::Primary);
                                        cx.request_keyboard_capture();
                                        return true;
                                    }
                                }

                                // SPEC-HAL-297: Route through active tool
                                if self.active_tool_index > 0 {
                                    let w = self.layout_rect.width;
                                    let h = self.layout_rect.height;
                                    if w > 0.0 && h > 0.0 {
                                        let transform = self.transform(w, h);
                                        let result = self.tools[self.active_tool_index]
                                            .handle_gesture(
                                                tool::GesturePhase::Down,
                                                lx,
                                                ly,
                                                &transform,
                                                pe.modifiers,
                                            );
                                        match result {
                                            tool::GestureResult::Handled => {
                                                self.tool_gesture_active = true;
                                                self.in_capture = true;
                                                cx.capture_pointer(PointerButton::Primary);
                                                cx.request_keyboard_capture();
                                                self.render_generation += 1;
                                                return true;
                                            }
                                            tool::GestureResult::Output(output) => {
                                                self.pending_tool_outputs.push(output);
                                                self.render_generation += 1;
                                                return true;
                                            }
                                            tool::GestureResult::Passthrough => {}
                                        }
                                    }
                                }

                                // Record anchor — don't commit to any mode yet.
                                // Move decides: pan or range-select (if Shift).
                                // Up without movement = click → hit-test.
                                self.primary_down_pos = Some((lx, ly));
                                self.primary_down_shift = pe.modifiers.shift;
                                self.drag_threshold_exceeded = false;
                                self.in_capture = true;
                                cx.capture_pointer(PointerButton::Primary);
                                cx.request_keyboard_capture();
                                true
                            }
                            Some(PointerButton::Secondary) => {
                                // Start box zoom — capture to receive
                                // Move/Up events during right-drag
                                self.box_zoom_active = true;
                                self.box_zoom_start = Some((lx, ly));
                                cx.capture_pointer(PointerButton::Secondary);
                                true
                            }
                            _ => false,
                        }
                    }

                    PointerEventKind::Move => {
                        // Update crosshair
                        if inside {
                            self.crosshair_pos = Some((lx, ly));
                        } else if !self.is_dragging
                            && !self.box_zoom_active
                            && self.level_drag.is_none()
                        {
                            self.crosshair_pos = None;
                        }

                        // SPEC-HAL-297: Tool gesture in progress
                        if self.tool_gesture_active {
                            let w = self.layout_rect.width;
                            let h = self.layout_rect.height;
                            if w > 0.0 && h > 0.0 {
                                let transform = Transform::from_view_state_with_axes(
                                    &self.view_state,
                                    w,
                                    h,
                                    self.x_axis.kind,
                                    self.y_axis.kind,
                                );
                                let result = self.tools[self.active_tool_index].handle_gesture(
                                    tool::GesturePhase::Move,
                                    lx,
                                    ly,
                                    &transform,
                                    pe.modifiers,
                                );
                                if let tool::GestureResult::Output(output) = result {
                                    self.pending_tool_outputs.push(output);
                                }
                                self.render_generation += 1;
                            }
                            return true;
                        }

                        // SPEC-HAL-299: Level drag in progress
                        if let Some(ref mut drag) = self.level_drag {
                            let w = self.layout_rect.width;
                            let h = self.layout_rect.height;
                            if w > 0.0 && h > 0.0 {
                                let transform = Transform::from_view_state_with_axes(
                                    &self.view_state,
                                    w,
                                    h,
                                    self.x_axis.kind,
                                    self.y_axis.kind,
                                );
                                let mut proposed_y = transform.px_to_y(ly);

                                if let Some(ref snap) = self.level_snap_fn {
                                    proposed_y = snap(proposed_y);
                                }

                                drag.current_y = proposed_y;

                                if let Some(level) =
                                    self.price_levels.iter_mut().find(|l| l.id == drag.level_id)
                                {
                                    level.y = proposed_y;
                                }

                                self.render_generation += 1;
                            }
                            return true;
                        }

                        // Update box selection visual
                        if self.box_zoom_active {
                            if let Some((sx, sy)) = self.box_zoom_start {
                                self.box_selection = Some((sx, sy, lx, ly));
                            }
                        }

                        // Primary drag: click-vs-drag disambiguation
                        if let Some((ax, ay)) = self.primary_down_pos {
                            if !self.drag_threshold_exceeded {
                                let dist = ((lx - ax).powi(2) + (ly - ay).powi(2)).sqrt();
                                if dist >= 5.0 {
                                    self.drag_threshold_exceeded = true;
                                    if self.primary_down_shift {
                                        self.box_selection = Some((ax, ay, lx, ly));
                                    } else {
                                        // Check if down is on a selected Draggable point
                                        let mut started_series_drag = false;
                                        if !self.object_selection.is_empty() {
                                            let w = self.layout_rect.width;
                                            let h = self.layout_rect.height;
                                            if w > 0.0 && h > 0.0 {
                                                let transform = self.transform(w, h);
                                                let sel = &self.object_selection[0];
                                                let idx = sel.series_index;
                                                if idx < self.hit_entries.len()
                                                    && self.hit_entries[idx].meta.interaction
                                                        == series::SeriesInteraction::Draggable
                                                {
                                                    if self.hit_entries[idx]
                                                        .hit_test(ax, ay, &transform)
                                                        .is_some()
                                                    {
                                                        self.series_drag = Some(SeriesDragState {
                                                            series_index: idx,
                                                            _initial_hit_x: ax,
                                                            _initial_hit_y: ay,
                                                        });
                                                        started_series_drag = true;
                                                    }
                                                }
                                            }
                                        }
                                        if !started_series_drag {
                                            self.is_dragging = true;
                                            self.drag_prev_pos = Some((ax, ay));
                                            self.view_state.follow_latest = false;
                                        }
                                    }
                                }
                            }

                            if self.drag_threshold_exceeded {
                                if self.primary_down_shift {
                                    self.box_selection = Some((ax, ay, lx, ly));
                                } else if let Some(ref drag) = self.series_drag {
                                    // Series drag: emit world coordinates
                                    let w = self.layout_rect.width;
                                    let h = self.layout_rect.height;
                                    if w > 0.0 && h > 0.0 {
                                        let transform = self.transform(w, h);
                                        let world_x = transform.px_to_x(lx);
                                        let world_y = transform.px_to_y(ly);
                                        let idx = drag.series_index;
                                        if let Some(point_data) = self.hit_entries[idx].hit_test(
                                            drag._initial_hit_x,
                                            drag._initial_hit_y,
                                            &transform,
                                        ) {
                                            self.pending_events.push(
                                                PlotSelectionEvent::SeriesDrag {
                                                    series_index: idx,
                                                    point_data,
                                                    world_x,
                                                    world_y,
                                                },
                                            );
                                        }
                                    }
                                } else if self.is_dragging {
                                    if let Some((prev_x, prev_y)) = self.drag_prev_pos {
                                        let w = self.layout_rect.width;
                                        let h = self.layout_rect.height;
                                        if w > 0.0 && h > 0.0 {
                                            let dx = lx - prev_x;
                                            let dy = ly - prev_y;

                                            self.pan_drag_delta(dx, dy, w, h, self.space_held);
                                        }
                                    }
                                    self.drag_prev_pos = Some((lx, ly));
                                }
                            }

                            true
                        } else if self.box_zoom_active {
                            true
                        } else if inside {
                            self.update_probe(lx, ly);

                            // Price level hover tracking
                            let hovered = self.hit_test_price_levels(ly);
                            if hovered != self.hovered_level {
                                if let Some(old) = self.hovered_level {
                                    self.pending_level_events
                                        .push(PriceLevelEvent::HoverLeave { id: old });
                                }
                                if let Some(new) = hovered {
                                    self.pending_level_events
                                        .push(PriceLevelEvent::HoverEnter { id: new });
                                }
                                self.hovered_level = hovered;
                            }

                            // Circle marker hover tracking
                            let hovered_c = self.hit_test_circles(lx, ly);
                            if hovered_c != self.hovered_circle {
                                if let Some(old) = self.hovered_circle {
                                    self.pending_circle_events
                                        .push(PlotCircleEvent::HoverLeave { id: old });
                                }
                                if let Some(new) = hovered_c {
                                    self.pending_circle_events
                                        .push(PlotCircleEvent::HoverEnter { id: new });
                                }
                                self.hovered_circle = hovered_c;
                                self.render_generation += 1;
                            }

                            // Line marker hover tracking
                            let hovered_l = self.hit_test_lines(lx, ly);
                            if hovered_l != self.hovered_line {
                                if let Some(old) = self.hovered_line {
                                    self.pending_line_events
                                        .push(PlotLineEvent::HoverLeave { id: old });
                                }
                                if let Some(new) = hovered_l {
                                    self.pending_line_events
                                        .push(PlotLineEvent::HoverEnter { id: new });
                                }
                                self.hovered_line = hovered_l;
                                self.render_generation += 1;
                            }

                            // Bezier marker hover tracking
                            let hovered_b = self.hit_test_beziers(lx, ly);
                            if hovered_b != self.hovered_bezier {
                                if let Some(old) = self.hovered_bezier {
                                    self.pending_bezier_events
                                        .push(PlotBezierEvent::HoverLeave { id: old });
                                }
                                if let Some(new) = hovered_b {
                                    self.pending_bezier_events
                                        .push(PlotBezierEvent::HoverEnter { id: new });
                                }
                                self.hovered_bezier = hovered_b;
                                self.render_generation += 1;
                            }

                            // Styled path marker hover tracking
                            let hovered_sp = self.hit_test_styled_paths(lx, ly);
                            if hovered_sp != self.hovered_styled_path {
                                if let Some(old) = self.hovered_styled_path {
                                    self.pending_styled_path_events
                                        .push(PlotStyledPathEvent::HoverLeave { id: old });
                                }
                                if let Some(new) = hovered_sp {
                                    self.pending_styled_path_events
                                        .push(PlotStyledPathEvent::HoverEnter { id: new });
                                }
                                self.hovered_styled_path = hovered_sp;
                                self.render_generation += 1;
                            }

                            // Path marker hover tracking
                            let hovered_p = self.hit_test_paths(lx, ly);
                            if hovered_p != self.hovered_path {
                                if let Some(old) = self.hovered_path {
                                    self.pending_path_events
                                        .push(PlotPathEvent::HoverLeave { id: old });
                                }
                                if let Some(new) = hovered_p {
                                    self.pending_path_events
                                        .push(PlotPathEvent::HoverEnter { id: new });
                                }
                                self.hovered_path = hovered_p;
                                self.render_generation += 1;
                            }

                            true
                        } else {
                            false
                        }
                    }

                    PointerEventKind::Up => match pe.button {
                        Some(PointerButton::Primary) => {
                            // SPEC-HAL-297: Complete tool gesture
                            if self.tool_gesture_active {
                                let w = self.layout_rect.width;
                                let h = self.layout_rect.height;
                                if w > 0.0 && h > 0.0 {
                                    let transform = self.transform(w, h);
                                    let result = self.tools[self.active_tool_index].handle_gesture(
                                        tool::GesturePhase::Up,
                                        lx,
                                        ly,
                                        &transform,
                                        pe.modifiers,
                                    );
                                    if let tool::GestureResult::Output(output) = result {
                                        self.pending_tool_outputs.push(output);
                                    }
                                }
                                self.tool_gesture_active = false;
                                self.in_capture = false;
                                cx.release_pointer();
                                self.render_generation += 1;
                                return true;
                            }

                            // SPEC-HAL-299: Complete level drag
                            if let Some(drag) = self.level_drag.take() {
                                let new_y = drag.current_y;
                                if (new_y - drag.original_y).abs() > f64::EPSILON {
                                    self.pending_level_events.push(PriceLevelEvent::Moved {
                                        id: drag.level_id,
                                        old_y: drag.original_y,
                                        new_y,
                                    });
                                } else {
                                    self.pending_level_events
                                        .push(PriceLevelEvent::Clicked { id: drag.level_id });
                                    if let Some(level) =
                                        self.price_levels.iter_mut().find(|l| l.id == drag.level_id)
                                    {
                                        level.y = drag.original_y;
                                    }
                                }
                                self.in_capture = false;
                                cx.release_pointer();
                                self.render_generation += 1;
                                return true;
                            }

                            if self.primary_down_pos.is_some() {
                                let mut handled_as_drag = false;

                                if self.drag_threshold_exceeded {
                                    if self.primary_down_shift {
                                        // Shift+drag completed: select all objects in range
                                        let w = self.layout_rect.width;
                                        let h = self.layout_rect.height;

                                        if let Some((bx1, by1, bx2, by2)) = self.box_selection {
                                            if w > 0.0 && h > 0.0 {
                                                let transform = self.transform(w, h);
                                                let world_x1 = transform.px_to_x(bx1.min(bx2));
                                                let world_x2 = transform.px_to_x(bx1.max(bx2));
                                                let world_y_top = transform.px_to_y(by1.min(by2));
                                                let world_y_bot = transform.px_to_y(by1.max(by2));
                                                let y_min = world_y_bot.min(world_y_top);
                                                let y_max = world_y_bot.max(world_y_top);

                                                // Collect all objects in range from selectable series
                                                for idx in 0..self.hit_entries.len() {
                                                    if self.hit_entries[idx].meta.interaction
                                                        == series::SeriesInteraction::None
                                                    {
                                                        continue;
                                                    }
                                                    let hits = self.hit_entries[idx]
                                                        .objects_in_range(
                                                            world_x1, world_x2, y_min, y_max,
                                                        );
                                                    for hit in hits {
                                                        self.object_selection.push(ObjectHit {
                                                            series_index: idx,
                                                            hit,
                                                            world_x: (world_x1 + world_x2) / 2.0,
                                                            world_y: (y_min + y_max) / 2.0,
                                                        });
                                                    }
                                                }

                                                // Also emit RangeSelected event
                                                let bw = (bx2 - bx1).abs();
                                                let bh = (by2 - by1).abs();
                                                if bw >= 10.0 && bh >= 10.0 {
                                                    self.pending_events.push(
                                                        PlotSelectionEvent::RangeSelected(
                                                            RangeSelection {
                                                                x_min: world_x1,
                                                                x_max: world_x2,
                                                                y_min,
                                                                y_max,
                                                            },
                                                        ),
                                                    );
                                                }
                                            }
                                        }

                                        self.box_selection = None;
                                        handled_as_drag = true;
                                    } else if let Some(drag) = self.series_drag.take() {
                                        // Series drag completed
                                        let w = self.layout_rect.width;
                                        let h = self.layout_rect.height;
                                        if w > 0.0 && h > 0.0 {
                                            let transform = self.transform(w, h);
                                            let world_x = transform.px_to_x(lx);
                                            let world_y = transform.px_to_y(ly);
                                            if let Some(point_data) =
                                                self.hit_entries[drag.series_index].hit_test(
                                                    drag._initial_hit_x,
                                                    drag._initial_hit_y,
                                                    &transform,
                                                )
                                            {
                                                self.pending_events.push(
                                                    PlotSelectionEvent::SeriesDragEnd {
                                                        series_index: drag.series_index,
                                                        point_data,
                                                        world_x,
                                                        world_y,
                                                    },
                                                );
                                            }
                                        }
                                        handled_as_drag = true;
                                    } else {
                                        // Pan completed
                                        self.is_dragging = false;
                                        self.drag_prev_pos = None;
                                        self.finish_horizontal_pan();
                                        handled_as_drag = true;
                                    }
                                }

                                // Click handler: no drag, OR Shift+drag with box too small
                                if !handled_as_drag {
                                    let (cx_pos, cy_pos) = self.primary_down_pos.unwrap();
                                    let w = self.layout_rect.width;
                                    let h = self.layout_rect.height;
                                    let mut clicked = false;

                                    // Price level click (priority over series)
                                    if let Some(level_id) = self.hit_test_price_levels(cy_pos) {
                                        self.pending_level_events
                                            .push(PriceLevelEvent::Clicked { id: level_id });
                                        clicked = true;
                                    }

                                    if !clicked && w > 0.0 && h > 0.0 {
                                        let transform = self.transform(w, h);

                                        let mut indices: Vec<usize> =
                                            (0..self.hit_entries.len()).collect();
                                        indices.sort_by(|&a, &b| {
                                            self.hit_entries[b]
                                                .meta
                                                .z_order
                                                .cmp(&self.hit_entries[a].meta.z_order)
                                        });

                                        for &idx in &indices {
                                            if self.hit_entries[idx].meta.interaction
                                                == series::SeriesInteraction::None
                                            {
                                                continue;
                                            }
                                            if let Some(hit) = self.hit_entries[idx]
                                                .hit_test(cx_pos, cy_pos, &transform)
                                            {
                                                let world_x = transform.px_to_x(cx_pos);
                                                let world_y = transform.px_to_y(cy_pos);

                                                if !self.primary_down_shift {
                                                    self.object_selection.clear();
                                                }

                                                if let Some(hit2) = self.hit_entries[idx]
                                                    .hit_test(cx_pos, cy_pos, &transform)
                                                {
                                                    self.object_selection.push(ObjectHit {
                                                        series_index: idx,
                                                        hit: hit2,
                                                        world_x,
                                                        world_y,
                                                    });
                                                }

                                                self.pending_events.push(
                                                    PlotSelectionEvent::ObjectSelected(ObjectHit {
                                                        series_index: idx,
                                                        hit,
                                                        world_x,
                                                        world_y,
                                                    }),
                                                );

                                                clicked = true;
                                                break;
                                            }
                                        }
                                    }

                                    // Circle marker click
                                    if self.object_selection.is_empty() {
                                        if let Some(cid) = self.hit_test_circles(lx, ly) {
                                            self.pending_circle_events
                                                .push(PlotCircleEvent::Clicked { id: cid });
                                        }
                                    }

                                    // Line marker click
                                    if self.object_selection.is_empty() {
                                        if let Some(lid) = self.hit_test_lines(lx, ly) {
                                            self.pending_line_events
                                                .push(PlotLineEvent::Clicked { id: lid });
                                        }
                                    }

                                    // Bezier marker click
                                    if self.object_selection.is_empty() {
                                        if let Some(bid) = self.hit_test_beziers(lx, ly) {
                                            self.pending_bezier_events
                                                .push(PlotBezierEvent::Clicked { id: bid });
                                        }
                                    }

                                    // Styled path marker click
                                    if self.object_selection.is_empty() {
                                        if let Some(spid) = self.hit_test_styled_paths(lx, ly) {
                                            self.pending_styled_path_events
                                                .push(PlotStyledPathEvent::Clicked { id: spid });
                                        }
                                    }

                                    // Path marker click
                                    if self.object_selection.is_empty() {
                                        if let Some(pid) = self.hit_test_paths(lx, ly) {
                                            self.pending_path_events
                                                .push(PlotPathEvent::Clicked { id: pid });
                                        }
                                    }

                                    if !clicked
                                        && !self.primary_down_shift
                                        && !self.object_selection.is_empty()
                                    {
                                        self.object_selection.clear();
                                        self.pending_events
                                            .push(PlotSelectionEvent::SelectionCleared);
                                    }
                                }

                                // Reset all primary state
                                self.primary_down_pos = None;
                                self.primary_down_shift = false;
                                self.drag_threshold_exceeded = false;
                                self.in_capture = false;
                                cx.release_pointer();
                                true
                            } else {
                                false
                            }
                        }
                        Some(PointerButton::Secondary) => {
                            if self.box_zoom_active {
                                if let Some((sx, sy)) = self.box_zoom_start {
                                    let bw = (lx - sx).abs();
                                    let bh = (ly - sy).abs();

                                    if bw >= 10.0 && bh >= 10.0 {
                                        let tw = self.layout_rect.width;
                                        let th = self.layout_rect.height;
                                        let transform = self.transform(tw, th);
                                        let x1 = transform.px_to_x(sx.min(lx));
                                        let x2 = transform.px_to_x(sx.max(lx));
                                        let y_top = transform.px_to_y(sy.min(ly));
                                        let y_bot = transform.px_to_y(sy.max(ly));

                                        self.view_state.x_min = x1;
                                        self.view_state.x_max = x2;
                                        self.view_state.y_min = y_bot;
                                        self.view_state.y_max = y_top;
                                        self.view_state.follow_latest = false;
                                        self.constrain_to_bounds();
                                        self.push_to_authorities();
                                    }
                                }
                                self.box_zoom_active = false;
                                self.box_zoom_start = None;
                                self.box_selection = None;
                                cx.release_pointer();
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    },

                    PointerEventKind::Scroll {
                        delta_x: _,
                        delta_y,
                    } => {
                        if !inside {
                            return false;
                        }

                        let w = self.layout_rect.width;
                        let h = self.layout_rect.height;
                        if w <= 0.0 || h <= 0.0 {
                            return false;
                        }

                        if self.space_held {
                            // Space+scroll: horizontal pan
                            let x_span = self.view_state.x_max - self.view_state.x_min;
                            let pan_amount = x_span * 0.05 * delta_y.signum() as f64;
                            self.view_state.pan(pan_amount, 0.0);
                            self.view_state.follow_latest = false;
                            self.constrain_to_bounds();
                            self.auto_scale_y(self.y_padding);
                            self.push_to_authorities();
                        } else if (cfg!(target_os = "macos") && pe.modifiers.meta)
                            || (cfg!(not(target_os = "macos")) && pe.modifiers.alt)
                        {
                            // Cmd+scroll (macOS) / Alt+scroll (Linux/Windows): Y-only zoom
                            let transform = self.transform(w, h);
                            let center_y = transform.px_to_y(ly);
                            let factor = if delta_y > 0.0 { 1.2 } else { 0.8 };
                            self.view_state.zoom_y(factor, center_y);
                            self.manual_y = true;
                            self.ensure_log_axis_domains();
                            self.push_to_authorities();
                        } else {
                            // Normal: zoom at cursor position
                            let transform = self.transform(w, h);
                            let center_x = transform.px_to_x(lx);
                            let center_y = transform.px_to_y(ly);
                            let factor = if delta_y > 0.0 { 1.2 } else { 0.8 };
                            self.view_state.zoom(factor, center_x, center_y);
                            self.view_state.follow_latest = false;
                            self.constrain_to_bounds();
                            self.auto_scale_y(self.y_padding);
                            self.push_to_authorities();
                        }
                        true
                    }

                    PointerEventKind::DoubleClick => {
                        if !inside {
                            return false;
                        }
                        self.handle_reset_view();
                        true
                    }

                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down && !ke.is_repeat => {
                use crate::core::event::KeyCode;
                // Escape is handled by on_capture_escape (Capture mode hook).
                // Other keys dispatched here. TODO(SPEC-297): migrate to ActionId.
                match ke.key_code {
                    KeyCode::Home => {
                        self.handle_reset_view();
                        true
                    }
                    KeyCode::Character('=') | KeyCode::Character('+') => {
                        // PLOT_ZOOM_IN: zoom in at viewport center
                        let w = self.layout_rect.width;
                        let h = self.layout_rect.height;
                        if w > 0.0 && h > 0.0 {
                            let transform = self.transform(w, h);
                            let center_x = transform.px_to_x(w / 2.0);
                            let center_y = transform.px_to_y(h / 2.0);
                            self.view_state.zoom(1.2, center_x, center_y);
                            self.view_state.follow_latest = false;
                            self.constrain_to_bounds();
                            self.auto_scale_y(self.y_padding);
                            self.push_to_authorities();
                        }
                        true
                    }
                    KeyCode::Character('-') => {
                        // PLOT_ZOOM_OUT: zoom out from viewport center
                        let w = self.layout_rect.width;
                        let h = self.layout_rect.height;
                        if w > 0.0 && h > 0.0 {
                            let transform = self.transform(w, h);
                            let center_x = transform.px_to_x(w / 2.0);
                            let center_y = transform.px_to_y(h / 2.0);
                            self.view_state.zoom(0.8, center_x, center_y);
                            self.view_state.follow_latest = false;
                            self.constrain_to_bounds();
                            self.auto_scale_y(self.y_padding);
                            self.push_to_authorities();
                        }
                        true
                    }
                    KeyCode::Character('v') => {
                        // Toggle volume overlay (series at z_order 50)
                        self.toggle_series_by_z_order(50);
                        self.render_generation += 1;
                        true
                    }
                    KeyCode::Space => {
                        self.space_held = true;
                        // Double-Space → reset view (same as double-click / Home)
                        let now = web_time::Instant::now();
                        if let Some(prev) = self.last_space_press {
                            if now.duration_since(prev).as_millis() < 300 {
                                self.last_space_press = None;
                                self.handle_reset_view();
                                return true;
                            }
                        }
                        self.last_space_press = Some(now);
                        false
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if !ke.is_down => {
                if ke.key_code == crate::core::event::KeyCode::Space {
                    self.space_held = false;
                }
                false
            }
            Event::Key(_) => false,
            Event::Text(te) => {
                // Character shortcuts via TextInput (macOS delivers these
                // separately from KeyDown in some configurations).
                match te.text.as_str() {
                    "=" | "+" => {
                        let w = self.layout_rect.width;
                        let h = self.layout_rect.height;
                        if w > 0.0 && h > 0.0 {
                            let transform = self.transform(w, h);
                            let center_x = transform.px_to_x(w / 2.0);
                            let center_y = transform.px_to_y(h / 2.0);
                            self.view_state.zoom(1.2, center_x, center_y);
                            self.view_state.follow_latest = false;
                            self.constrain_to_bounds();
                            self.auto_scale_y(self.y_padding);
                            self.push_to_authorities();
                        }
                        true
                    }
                    "-" | "_" => {
                        let w = self.layout_rect.width;
                        let h = self.layout_rect.height;
                        if w > 0.0 && h > 0.0 {
                            let transform = self.transform(w, h);
                            let center_x = transform.px_to_x(w / 2.0);
                            let center_y = transform.px_to_y(h / 2.0);
                            self.view_state.zoom(0.8, center_x, center_y);
                            self.view_state.follow_latest = false;
                            self.constrain_to_bounds();
                            self.auto_scale_y(self.y_padding);
                            self.push_to_authorities();
                        }
                        true
                    }
                    "v" | "V" => {
                        self.toggle_series_by_z_order(50);
                        self.render_generation += 1;
                        true
                    }
                    "m" | "M" => {
                        self.activate_tool("measure");
                        true
                    }
                    "h" | "H" => {
                        self.activate_tool("hline");
                        true
                    }
                    "r" | "R" => {
                        self.activate_tool("rect");
                        true
                    }
                    "1" => {
                        self.activate_tool("pointer");
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    /// SPEC-PM-005 / INV-FLEX-MEASURE-UNBOUNDED: Plot records its final logical
    /// viewport from `constraints.max_height`, so it requires the bounded main
    /// axis that post-layout custom-render preparation later projects.
    fn needs_viewport_constraint(&self) -> bool {
        true
    }

    fn custom_render_admission(&self) -> Option<&CustomRenderAdmission> {
        self.custom_render_admission.as_ref()
    }

    fn custom_render_participant_mut(&mut self) -> Option<&mut dyn CustomRenderParticipant> {
        Some(self)
    }

    fn on_unmount(&mut self) {
        self.cancel_resize();
        self.withdraw_canonical_front();
        self.mounted_plot = None;
        self.resize_slot = None;
        self.series_data_generation = 0;
        self.retire_when_available = false;
        self.deferred_series_data.clear();
        if let Some(renderer_arc) = self.renderer.take() {
            match renderer_arc.try_lock() {
                Ok(mut renderer) => renderer.destroy_deferred(),
                Err(std::sync::TryLockError::Poisoned(error)) => error.into_inner().destroy_deferred(),
                Err(std::sync::TryLockError::WouldBlock) => {}
            }
        }
        self.target_state = None;
        self.presented_texture_id = None;
        self.presented_labels = None;
        self.clear_completed_render();
        self.gpu_initialized = false;
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        self.focused = true;
        cx.invalidate_paint();
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.focused = false;
        self.crosshair_pos = None;
        if let Some(ref probe) = self.probe {
            if let Ok(mut p) = probe.lock() {
                p.clear();
            }
        }
        cx.invalidate_paint();
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "Plot"
    }

    fn needs_update(&self) -> bool {
        self.data_rx.is_some()
            || self.render_work_outstanding()
            || self.price_level_animation_active()
    }

    fn needs_animation(&self) -> bool {
        self.price_level_animation_active()
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.in_capture = false;
        self.space_held = false;
        cx.invalidate_paint();
    }
}
