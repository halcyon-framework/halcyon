// SPEC-HAL-299: Price Levels — horizontal markers with tags.

use crate::core::paint::Color;

/// Line rendering style.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PriceLevelLine {
    Solid,
    Dashed,
    Dotted,
}

impl Default for PriceLevelLine {
    fn default() -> Self {
        Self::Solid
    }
}

/// Tag anchor position.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagSide {
    Right,
    Left,
}

impl Default for TagSide {
    fn default() -> Self {
        Self::Right
    }
}

/// A single line of text inside a tag.
#[derive(Debug, Clone)]
pub struct TagLine {
    pub text: String,
    pub color: Option<Color>,
    pub dimmed: bool,
}

impl TagLine {
    pub fn new(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            color: None,
            dimmed: false,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn dimmed(mut self) -> Self {
        self.dimmed = true;
        self
    }
}

/// A horizontal price line with an attached tag.
#[derive(Debug, Clone)]
pub struct PriceLevel {
    pub id: u64,
    pub y: f64,
    /// Smoothed display position. Lerps toward `y` each frame.
    /// Rendering reads this, not `y`. Initialized to `y`.
    pub display_y: f64,
    pub color: Color,
    pub line_style: PriceLevelLine,
    pub tag_side: TagSide,
    pub tag: Vec<TagLine>,
    pub draggable: bool,
    pub group: Option<String>,
}

impl Default for PriceLevel {
    fn default() -> Self {
        Self {
            id: 0,
            y: 0.0,
            display_y: 0.0,
            color: Color::from_srgb(0.5, 0.5, 0.5, 0.8),
            line_style: PriceLevelLine::Solid,
            tag_side: TagSide::Right,
            tag: Vec::new(),
            draggable: false,
            group: None,
        }
    }
}

/// Events emitted by price level interaction.
#[derive(Debug, Clone)]
pub enum PriceLevelEvent {
    Moved { id: u64, old_y: f64, new_y: f64 },
    Clicked { id: u64 },
    HoverEnter { id: u64 },
    HoverLeave { id: u64 },
}

/// Where the tag sits relative to its anchor point.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TagAnchor {
    Above,
    Below,
    Right,
    Left,
}

impl Default for TagAnchor {
    fn default() -> Self {
        Self::Above
    }
}

/// Freestanding label at world coordinates.
#[derive(Debug, Clone)]
pub struct PlotTag {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub tag: Vec<TagLine>,
    pub color: Color,
    pub anchor: TagAnchor,
}

impl Default for PlotTag {
    fn default() -> Self {
        Self {
            id: 0,
            x: 0.0,
            y: 0.0,
            tag: Vec::new(),
            color: Color::from_srgb(0.5, 0.5, 0.5, 0.8),
            anchor: TagAnchor::Above,
        }
    }
}

/// A rectangular highlighted zone at world coordinates.
/// None bounds span the full viewport dimension.
#[derive(Debug, Clone)]
pub struct PlotRegion {
    pub id: u64,
    /// World X bounds. None = span full viewport width.
    pub x_min: Option<f64>,
    pub x_max: Option<f64>,
    /// World Y bounds. None = span full viewport height.
    pub y_min: Option<f64>,
    pub y_max: Option<f64>,
    /// Fill color (typically semi-transparent).
    pub fill_color: Color,
    /// Optional border color. None = no border.
    pub border_color: Option<Color>,
    /// Optional label (rendered at region center).
    pub label: Option<String>,
    /// Optional group for batch removal.
    pub group: Option<String>,
}

impl Default for PlotRegion {
    fn default() -> Self {
        Self {
            id: 0,
            x_min: None,
            x_max: None,
            y_min: None,
            y_max: None,
            fill_color: Color::from_srgb(0.5, 0.5, 0.5, 0.1),
            border_color: None,
            label: None,
            group: None,
        }
    }
}

/// Circle marker at world coordinates. Same pattern as PriceLevel/PlotTag/PlotRegion.
/// Rendered at screen-space radius via SDF circle shader.
#[derive(Debug, Clone)]
pub struct PlotCircle {
    pub id: u64,
    pub x: f64,
    pub y: f64,
    pub radius: f32,
    pub fill_color: Color,
    pub border_color: Color,
    pub border_width: f32,
    /// Optional group for batch removal.
    pub group: Option<String>,
}

impl Default for PlotCircle {
    fn default() -> Self {
        Self {
            id: 0,
            x: 0.0,
            y: 0.0,
            radius: 8.0,
            fill_color: Color::TRANSPARENT,
            border_color: Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            border_width: 1.5,
            group: None,
        }
    }
}

/// Events emitted by circle marker interaction.
#[derive(Debug, Clone)]
pub enum PlotCircleEvent {
    Clicked { id: u64 },
    HoverEnter { id: u64 },
    HoverLeave { id: u64 },
}

/// Line style for plot connectors.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PlotLineStyle {
    Solid,
    Dashed { dash: f32, gap: f32 },
    Dotted { dot: f32, gap: f32 },
}

impl Default for PlotLineStyle {
    fn default() -> Self {
        Self::Solid
    }
}

/// Straight line marker between two world-coordinate points.
/// Same pattern as PriceLevel/PlotCircle — ID-based, plot-level.
#[derive(Debug, Clone)]
pub struct PlotLine {
    pub id: u64,
    pub x1: f64,
    pub y1: f64,
    pub x2: f64,
    pub y2: f64,
    pub color: Color,
    pub width: f32,
    pub line_style: PlotLineStyle,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
    pub group: Option<String>,
}

impl Default for PlotLine {
    fn default() -> Self {
        Self {
            id: 0,
            x1: 0.0,
            y1: 0.0,
            x2: 0.0,
            y2: 0.0,
            color: Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            width: 1.0,
            line_style: PlotLineStyle::Solid,
            arrow_start: false,
            arrow_end: false,
            arrow_size: 8.0,
            group: None,
        }
    }
}

/// Events emitted by line marker interaction.
#[derive(Debug, Clone)]
pub enum PlotLineEvent {
    Clicked { id: u64 },
    HoverEnter { id: u64 },
    HoverLeave { id: u64 },
}

/// Cubic Bezier curve between two world-coordinate points.
/// Same marker pattern as PlotLine/PlotCircle — ID-based, plot-level.
#[derive(Debug, Clone)]
pub struct PlotBezier {
    pub id: u64,
    /// Start point (world coordinates).
    pub x1: f64,
    pub y1: f64,
    /// Control point 1 (world coordinates).
    pub cx1: f64,
    pub cy1: f64,
    /// Control point 2 (world coordinates).
    pub cx2: f64,
    pub cy2: f64,
    /// End point (world coordinates).
    pub x2: f64,
    pub y2: f64,
    /// Visual properties.
    pub color: Color,
    pub width: f32,
    pub arrow_start: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
    /// Optional group for batch removal.
    pub group: Option<String>,
}

impl Default for PlotBezier {
    fn default() -> Self {
        Self {
            id: 0,
            x1: 0.0,
            y1: 0.0,
            cx1: 0.0,
            cy1: 0.0,
            cx2: 0.0,
            cy2: 0.0,
            x2: 0.0,
            y2: 0.0,
            color: Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            width: 1.0,
            arrow_start: false,
            arrow_end: false,
            arrow_size: 8.0,
            group: None,
        }
    }
}

impl PlotBezier {
    /// Create a Bezier arc with auto-computed control points.
    ///
    /// `bend` controls curvature: 0.0 = straight, 0.3 = gentle arc,
    /// 1.0 = dramatic curve. Positive = curve above the midpoint
    /// (perpendicular to the line direction), negative = below.
    pub fn arc(id: u64, x1: f64, y1: f64, x2: f64, y2: f64, bend: f64) -> Self {
        let dx = x2 - x1;
        let dy = y2 - y1;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 1e-9 {
            return Self {
                id,
                x1,
                y1,
                cx1: x1,
                cy1: y1,
                cx2: x2,
                cy2: y2,
                x2,
                y2,
                ..Self::default()
            };
        }
        let nx = -dy / len;
        let ny = dx / len;
        let offset = len * bend;
        Self {
            id,
            x1,
            y1,
            cx1: x1 + dx / 3.0 + nx * offset,
            cy1: y1 + dy / 3.0 + ny * offset,
            cx2: x1 + dx * 2.0 / 3.0 + nx * offset,
            cy2: y1 + dy * 2.0 / 3.0 + ny * offset,
            x2,
            y2,
            ..Self::default()
        }
    }
}

/// Events emitted by Bezier marker interaction.
#[derive(Debug, Clone)]
pub enum PlotBezierEvent {
    Clicked { id: u64 },
    HoverEnter { id: u64 },
    HoverLeave { id: u64 },
}

/// A single waypoint in a styled path.
/// Style properties apply to the segment FROM this point TO the next.
#[derive(Debug, Clone)]
pub struct StyledSegment {
    pub x: f64,
    pub y: f64,
    pub color: Color,
    pub width: f32,
    pub line_style: PlotLineStyle,
}

/// Polyline with per-segment visual properties.
/// Each segment can have independent color, width, and dash style.
/// Same marker pattern as PlotLine/PlotBezier — ID-based, plot-level.
#[derive(Debug, Clone)]
pub struct PlotStyledPath {
    pub id: u64,
    pub segments: Vec<StyledSegment>,
    /// Optional group for batch removal.
    pub group: Option<String>,
}

/// Events emitted by styled path interaction.
#[derive(Debug, Clone)]
pub enum PlotStyledPathEvent {
    Clicked { id: u64 },
    HoverEnter { id: u64 },
    HoverLeave { id: u64 },
}

/// Polyline through n world-coordinate waypoints with uniform style.
/// Rendering target for PlotRouter-produced paths and manual annotations.
#[derive(Debug, Clone)]
pub struct PlotPath {
    pub id: u64,
    pub points: Vec<(f64, f64)>,
    pub color: Color,
    pub width: f32,
    pub line_style: PlotLineStyle,
    pub closed: bool,
    pub arrow_end: bool,
    pub arrow_size: f32,
    pub group: Option<String>,
}

impl Default for PlotPath {
    fn default() -> Self {
        Self {
            id: 0,
            points: Vec::new(),
            color: Color::from_srgb(1.0, 1.0, 1.0, 0.8),
            width: 1.0,
            line_style: PlotLineStyle::Solid,
            closed: false,
            arrow_end: false,
            arrow_size: 8.0,
            group: None,
        }
    }
}

/// Events emitted by path interaction.
#[derive(Debug, Clone)]
pub enum PlotPathEvent {
    Clicked { id: u64 },
    HoverEnter { id: u64 },
    HoverLeave { id: u64 },
}
