//! SPEC-HAL-265 §3–§6: Core icon type system.
//!
//! Pure data types for icon identity, classification, definition, and registry.
//! Geometry and fill rules use the ordinary renderer Path types.

use crate::core::scene::Color;
pub use crate::renderer::path::FillRule;
use crate::renderer::path::{Path, PathCommand};

/// Stable identity token for icons. 4 bytes, cheap to pass everywhere.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct IconId(pub u32);

impl IconId {
    pub const CLOSE: IconId = IconId(1);
    pub const CHEVRON_RIGHT: IconId = IconId(2);
    pub const CHEVRON_DOWN: IconId = IconId(3);
    pub const PLUS: IconId = IconId(4);
    pub const MINUS: IconId = IconId(5);
    pub const CHECK: IconId = IconId(6);
    pub const SEARCH: IconId = IconId(7);
    pub const ARROW_UP: IconId = IconId(8);
    pub const ARROW_DOWN: IconId = IconId(9);
    pub const WARNING: IconId = IconId(10);
    pub const FOLDER: IconId = IconId(11);
    pub const FILE: IconId = IconId(12);
    pub const FOLDER_OPEN: IconId = IconId(13);
    pub const CALENDAR: IconId = IconId(14);
    pub const SELECT_ALL: IconId = IconId(15);
    pub const CLEAR_ALL: IconId = IconId(16);
    pub const SETTINGS: IconId = IconId(17);
    pub const GLOBE: IconId = IconId(18);
    pub const MAIL: IconId = IconId(19);
    pub const BRIEFCASE: IconId = IconId(20);
}

/// Semantic color role for a single layer within a multi-layer icon.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[repr(u8)]
pub enum LayerRole {
    /// Primary color, used as-is.
    Base = 0,
    /// Darkened — multiply RGB by 0.65.
    Shadow = 1,
    /// Lightened — lerp RGB toward 1.0 by 0.35.
    Highlight = 2,
    /// Strongly darkened — multiply RGB by 0.45. Thin edges/borders.
    Outline = 3,
}

impl LayerRole {
    /// Resolve this role against a base color, returning the concrete RGBA.
    /// Input and output are (r, g, b, a) with components in 0.0..=1.0.
    pub fn resolve(self, r: f32, g: f32, b: f32, a: f32) -> (f32, f32, f32, f32) {
        match self {
            LayerRole::Base => (r, g, b, a),
            LayerRole::Shadow => (r * 0.65, g * 0.65, b * 0.65, a),
            LayerRole::Highlight => (
                r + (1.0 - r) * 0.35,
                g + (1.0 - g) * 0.35,
                b + (1.0 - b) * 0.35,
                a,
            ),
            LayerRole::Outline => (r * 0.45, g * 0.45, b * 0.45, a),
        }
    }
}

/// A single path layer within a multi-layer icon definition.
#[derive(Debug, Clone)]
pub struct IconLayer {
    pub role: LayerRole,
    pub path: Path,
    pub fill_rule: FillRule,
    /// True for layers with computed geometry (line/circle expansion). Validator skips precision check.
    pub skip_precision: bool,
}

/// Semantic classification for icons.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconRole {
    Navigation,
    Action,
    Status,
    Structural,
    Entity,
    Indicator,
}

/// How the icon path should be rendered.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconRenderMode {
    Fill,
    Stroke,
}

/// Logical pixel sizes per SPEC-HAL-095 §11.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconSize {
    XS,
    S,
    M,
    L,
    XL,
}

impl IconSize {
    pub fn logical_px(&self) -> f32 {
        match self {
            IconSize::XS => 10.0,
            IconSize::S => 12.0,
            IconSize::M => 14.0,
            IconSize::L => 16.0,
            IconSize::XL => 20.0,
        }
    }
}

/// Complete definition of an icon: path geometry, metadata, and classification.
pub struct IconDefinition {
    pub id: IconId,
    pub name: &'static str,
    pub layers: Vec<IconLayer>,
    pub viewbox_width: f32,
    pub viewbox_height: f32,
    pub default_mode: IconRenderMode,
    pub role: IconRole,
    pub path_hash: u64,
}

/// Widget-facing request type: "render icon X at this size/color/mode".
pub struct IconSpec {
    pub id: IconId,
    pub size: f32,
    pub color: Color,
    pub stroke_width: Option<f32>,
    pub mode: IconRenderMode,
}

/// Static lookup structure for icon definitions.
pub struct IconRegistry {
    definitions: &'static [IconDefinition],
}

impl IconRegistry {
    pub const fn new(definitions: &'static [IconDefinition]) -> Self {
        Self { definitions }
    }

    pub fn get(&self, id: IconId) -> Option<&IconDefinition> {
        self.definitions.iter().find(|d| d.id == id)
    }

    pub fn len(&self) -> usize {
        self.definitions.len()
    }

    pub fn is_empty(&self) -> bool {
        self.definitions.is_empty()
    }
}

/// Violation found by `validate_icon` per SPEC-HAL-265A.
#[derive(Debug, Clone)]
pub enum IconViolation {
    /// Coordinate outside `[0.0, viewbox]` (1.0 overshoot tolerance for strokes).
    CoordinateOutOfBounds { layer: usize, x: f32, y: f32 },
    /// Coordinate not integer or half-integer.
    CoordinatePrecision { layer: usize, x: f32, y: f32 },
    /// More than 5 layers.
    TooManyLayers { count: usize },
    /// Zero layers.
    NoLayers,
    /// Single layer exceeds 24 segments.
    TooManySegments { layer: usize, count: usize },
    /// Icon exceeds 72 total segments.
    TotalSegmentsExceeded { count: usize },
    /// Viewbox not 24×24 (warning-level, still collected).
    ViewboxNotStandard { width: f32, height: f32 },
}

/// Validate an icon definition against the visual rendering contract (SPEC-HAL-265A).
pub fn validate_icon(def: &IconDefinition) -> Result<(), Vec<IconViolation>> {
    let mut violations = Vec::new();

    if def.layers.is_empty() {
        violations.push(IconViolation::NoLayers);
    }
    if def.layers.len() > 5 {
        violations.push(IconViolation::TooManyLayers {
            count: def.layers.len(),
        });
    }
    if (def.viewbox_width - 24.0).abs() > 1e-6 || (def.viewbox_height - 24.0).abs() > 1e-6 {
        violations.push(IconViolation::ViewboxNotStandard {
            width: def.viewbox_width,
            height: def.viewbox_height,
        });
    }

    let vw = def.viewbox_width;
    let vh = def.viewbox_height;
    let mut total_segments = 0usize;

    for (li, layer) in def.layers.iter().enumerate() {
        let check_precision = !layer.skip_precision;
        let mut seg_count = 0usize;
        for cmd in &layer.path.commands {
            match *cmd {
                PathCommand::MoveTo(x, y) | PathCommand::LineTo(x, y) => {
                    seg_count += 1;
                    check_coord(&mut violations, li, x, y, vw, vh, check_precision);
                }
                PathCommand::QuadTo(cx, cy, x, y) => {
                    seg_count += 1;
                    check_coord(&mut violations, li, cx, cy, vw, vh, check_precision);
                    check_coord(&mut violations, li, x, y, vw, vh, check_precision);
                }
                PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                    seg_count += 1;
                    check_coord(&mut violations, li, c1x, c1y, vw, vh, check_precision);
                    check_coord(&mut violations, li, c2x, c2y, vw, vh, check_precision);
                    check_coord(&mut violations, li, x, y, vw, vh, check_precision);
                }
                PathCommand::Close => {}
            }
        }
        if seg_count > 24 {
            violations.push(IconViolation::TooManySegments {
                layer: li,
                count: seg_count,
            });
        }
        total_segments += seg_count;
    }

    if total_segments > 72 {
        violations.push(IconViolation::TotalSegmentsExceeded {
            count: total_segments,
        });
    }

    if violations.is_empty() {
        Ok(())
    } else {
        Err(violations)
    }
}

fn check_coord(
    violations: &mut Vec<IconViolation>,
    layer: usize,
    x: f32,
    y: f32,
    vw: f32,
    vh: f32,
    check_precision: bool,
) {
    if x < -1.0 || x >= vw + 1.0 || y < -1.0 || y >= vh + 1.0 {
        violations.push(IconViolation::CoordinateOutOfBounds { layer, x, y });
    }
    if check_precision {
        let x_ok = (x * 2.0).fract().abs() < 1e-6;
        let y_ok = (y * 2.0).fract().abs() < 1e-6;
        if !x_ok || !y_ok {
            violations.push(IconViolation::CoordinatePrecision { layer, x, y });
        }
    }
}
