//! Paint Abstraction Layer
//!
//! This module defines scene-agnostic types and traits for widget painting.
//! Widgets depend on this module, NOT on `core::scene`. The scene module
//! implements `PaintSink` to bridge widgets to the actual scene graph.
//!
//! # Authority Contract
//!
//! - Widgets import `core::paint` for all painting operations
//! - `core::scene` imports `core::paint` and implements `PaintSink`
//! - Widgets NEVER import `core::scene` directly
//!
//! This creates a one-way dependency: widget → paint ← scene

use crate::core::math::{Point, RectF};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::GlyphRun;
use crate::renderer::path::{FillRule, PathFillError};
use std::fmt;
use std::sync::Arc;

// ============================================================================
// COLOR (owned by paint, not scene)
// ============================================================================

/// Convert a single channel from sRGB to linear space.
#[inline]
pub fn srgb_to_linear(s: f32) -> f32 {
    if s <= 0.04045 {
        s / 12.92
    } else {
        ((s + 0.055) / 1.055).powf(2.4)
    }
}

/// Decode hex value to sRGB floats [0-1].
#[inline]
pub const fn decode_hex_srgb(hex: u32) -> [f32; 3] {
    [
        ((hex >> 16) & 0xFF) as f32 / 255.0,
        ((hex >> 8) & 0xFF) as f32 / 255.0,
        (hex & 0xFF) as f32 / 255.0,
    ]
}

/// Color in LINEAR RGBA format (0.0-1.0 range)
///
/// **COLOR SPACE CONTRACT**: All Color values are stored in LINEAR space.
/// Use `from_hex` or `from_srgb` for UI/theme colors (they convert from sRGB).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Color {
    pub r: f32,
    pub g: f32,
    pub b: f32,
    pub a: f32,
}

impl Color {
    /// Create color from hex value (interpreted as sRGB, converted to linear).
    #[inline]
    pub fn from_hex(hex: u32) -> Self {
        let [r, g, b] = decode_hex_srgb(hex);
        Self {
            r: srgb_to_linear(r),
            g: srgb_to_linear(g),
            b: srgb_to_linear(b),
            a: 1.0,
        }
    }

    /// Create color from sRGB components (converted to linear).
    #[inline]
    pub fn from_srgb(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self {
            r: srgb_to_linear(r),
            g: srgb_to_linear(g),
            b: srgb_to_linear(b),
            a,
        }
    }

    /// Create color from linear RGBA components (no conversion).
    #[inline]
    pub const fn from_linear(r: f32, g: f32, b: f32, a: f32) -> Self {
        Self { r, g, b, a }
    }

    /// Create a fully transparent color.
    pub const TRANSPARENT: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 0.0,
    };

    /// Create a fully opaque white.
    pub const WHITE: Self = Self {
        r: 1.0,
        g: 1.0,
        b: 1.0,
        a: 1.0,
    };

    /// Create a fully opaque black.
    pub const BLACK: Self = Self {
        r: 0.0,
        g: 0.0,
        b: 0.0,
        a: 1.0,
    };

    /// Return a copy with modified alpha.
    #[inline]
    pub fn with_alpha(self, a: f32) -> Self {
        Self { a, ..self }
    }

    /// Premultiply alpha into RGB channels.
    #[inline]
    pub fn premultiply(self) -> Self {
        Self {
            r: self.r * self.a,
            g: self.g * self.a,
            b: self.b * self.a,
            a: self.a,
        }
    }

    /// Linear interpolation between two colors in linear space.
    #[inline]
    pub fn lerp(self, other: Self, t: f32) -> Self {
        Self {
            r: self.r + (other.r - self.r) * t,
            g: self.g + (other.g - self.g) * t,
            b: self.b + (other.b - self.b) * t,
            a: self.a + (other.a - self.a) * t,
        }
    }

    /// Lighten by blending toward white. `amount` in [0.0, 1.0].
    /// Preserves alpha.
    #[inline]
    pub fn lighten(self, amount: f32) -> Self {
        Self {
            r: self.r + (1.0 - self.r) * amount,
            g: self.g + (1.0 - self.g) * amount,
            b: self.b + (1.0 - self.b) * amount,
            a: self.a,
        }
    }

    /// Darken by blending toward black. `amount` in [0.0, 1.0].
    /// Preserves alpha.
    #[inline]
    pub fn darken(self, amount: f32) -> Self {
        Self {
            r: self.r * (1.0 - amount),
            g: self.g * (1.0 - amount),
            b: self.b * (1.0 - amount),
            a: self.a,
        }
    }

    /// Extract as [r, g, b, a] array (linear space).
    /// Use at the GPU boundary when packing vertex data.
    #[inline]
    pub fn to_array(self) -> [f32; 4] {
        [self.r, self.g, self.b, self.a]
    }
}

impl Default for Color {
    fn default() -> Self {
        Self::TRANSPARENT
    }
}

// ============================================================================
// ROUNDED CORNERS (owned by paint, not scene)
// ============================================================================

/// Rounded corner radii for rectangles.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RoundedCorners {
    pub tl: f32, // top-left
    pub tr: f32, // top-right
    pub bl: f32, // bottom-left
    pub br: f32, // bottom-right
}

impl RoundedCorners {
    /// Create uniform rounded corners (all corners same radius).
    pub fn uniform(radius: f32) -> Self {
        Self {
            tl: radius,
            tr: radius,
            bl: radius,
            br: radius,
        }
    }

    /// Create rounded corners with individual radii.
    pub fn new(tl: f32, tr: f32, bl: f32, br: f32) -> Self {
        Self { tl, tr, bl, br }
    }

    /// Check if all corners are zero (no rounding).
    pub fn is_zero(&self) -> bool {
        self.tl == 0.0 && self.tr == 0.0 && self.bl == 0.0 && self.br == 0.0
    }

    /// Check if all corners have the same radius (uniform).
    pub fn is_uniform(&self) -> bool {
        self.tl == self.tr && self.tr == self.bl && self.bl == self.br
    }

    /// Get the uniform radius if all corners are the same, else None.
    pub fn uniform_radius(&self) -> Option<f32> {
        if self.is_uniform() {
            Some(self.tl)
        } else {
            None
        }
    }

    /// Apply inset to all corners (for 2-rect border technique).
    /// Each corner radius is reduced by `inset`, clamped to 0.
    pub fn inset(&self, inset: f32) -> Self {
        Self {
            tl: (self.tl - inset).max(0.0),
            tr: (self.tr - inset).max(0.0),
            bl: (self.bl - inset).max(0.0),
            br: (self.br - inset).max(0.0),
        }
    }

    /// Maximum radius among all corners.
    pub fn max_radius(&self) -> f32 {
        self.tl.max(self.tr).max(self.bl).max(self.br)
    }

    /// Convert to array [tl, tr, br, bl] for shader upload.
    pub fn to_array(&self) -> [f32; 4] {
        [self.tl, self.tr, self.br, self.bl]
    }
}

impl Default for RoundedCorners {
    fn default() -> Self {
        Self {
            tl: 0.0,
            tr: 0.0,
            bl: 0.0,
            br: 0.0,
        }
    }
}

// ============================================================================
// BORDER WIDTHS (per-side border thickness)
// ============================================================================

/// SPEC-R-015-v2: Per-side border widths.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderWidths {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl BorderWidths {
    /// All sides equal.
    pub fn uniform(w: f32) -> Self {
        Self {
            top: w,
            right: w,
            bottom: w,
            left: w,
        }
    }

    /// All sides zero.
    pub fn zero() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }

    /// True if all sides are equal (within epsilon).
    pub fn is_uniform(&self) -> bool {
        (self.top - self.right).abs() < 0.001
            && (self.right - self.bottom).abs() < 0.001
            && (self.bottom - self.left).abs() < 0.001
    }

    /// Total horizontal border (left + right).
    pub fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    /// Total vertical border (top + bottom).
    pub fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

// ============================================================================
// TEXT BACKGROUND SPAN (owned by paint, not scene)
// ============================================================================

/// Background span for text rendering (selection, highlights, etc.)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextBackgroundSpan {
    /// X offset from text position (logical pixels)
    pub x_offset: f32,
    /// Y offset from text position (logical pixels)
    pub y_offset: f32,
    /// Width of background span (logical pixels)
    pub width: f32,
    /// Height of background span (logical pixels)
    pub height: f32,
    /// Background color
    pub color: Color,
}

// ============================================================================
// Z-INDEX (owned by paint, not scene)
// ============================================================================

/// Z-ordering index for paint operations.
pub type ZIndex = u32;

// ============================================================================
// LAYER CONSTANTS (owned by paint, not scene)
// ============================================================================

/// Layer for normal content (widgets, backgrounds).
pub const LAYER_CONTENT: ZIndex = 2;

/// Layer for focus rings (above content).
pub const LAYER_FOCUS: ZIndex = 5;

/// Engine-owned closed decorative channel above content, below enclosing chrome.
pub const LAYER_MAIN_AFFORDANCE: ZIndex = 6;

/// Layer for scrollbars (above focus rings).
pub const LAYER_SCROLLBAR: ZIndex = 7;

/// Layer for overlays rendered AFTER text (caret, tooltips, modals, dropdowns).
/// Value 1000 ensures overlay rects are drawn after all other content including text.
pub const LAYER_OVERLAY: ZIndex = 1000;

/// Engine-owned membership of one closed ordinary-control composition interval.
/// This is presentation metadata, never a primitive or widget identity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ControlSubtreeMember {
    pub(crate) scope: crate::utils::id::TreeWidgetId,
    pub(crate) z_index: ZIndex,
    pub(crate) member_index: usize,
}

/// Complete per-surface order. The original primitive Z still selects its pass.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct EffectivePaintOrder {
    members: Option<Arc<[ControlSubtreeMember]>>,
    z_index: ZIndex,
}

impl EffectivePaintOrder {
    pub(crate) fn new(z_index: ZIndex, members: Option<Arc<[ControlSubtreeMember]>>) -> Self {
        Self {
            members: members.filter(|members| !members.is_empty()),
            z_index,
        }
    }

    #[cfg(test)]
    pub(crate) fn scalar(z_index: ZIndex) -> Self {
        Self::new(z_index, None)
    }

    #[cfg(test)]
    pub(crate) fn members(&self) -> &[ControlSubtreeMember] {
        self.members.as_deref().unwrap_or(&[])
    }
}

impl Ord for EffectivePaintOrder {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        let a = self.members.as_deref().unwrap_or(&[]);
        let b = other.members.as_deref().unwrap_or(&[]);
        for depth in 0..=a.len().max(b.len()) {
            let left = a.get(depth);
            let right = b.get(depth);
            let order = left
                .map_or(self.z_index, |m| m.z_index)
                .cmp(&right.map_or(other.z_index, |m| m.z_index));
            if order != std::cmp::Ordering::Equal {
                return order;
            }
            match (left, right) {
                (Some(left), Some(right)) => {
                    // Equal-anchor unrelated scopes remain closed intervals. This
                    // identity tie is deterministic bookkeeping, not paint authority.
                    let order = left
                        .scope
                        .0
                        .cmp(&right.scope.0)
                        .then_with(|| left.member_index.cmp(&right.member_index));
                    if order != std::cmp::Ordering::Equal {
                        return order;
                    }
                }
                (None, None) => return std::cmp::Ordering::Equal,
                (None, Some(_)) => return std::cmp::Ordering::Less,
                (Some(_), None) => return std::cmp::Ordering::Greater,
            }
        }
        unreachable!("effective paint order terminates at a primitive")
    }
}

impl PartialOrd for EffectivePaintOrder {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

// ============================================================================
// LUMINOUS CONTOUR (SPEC-HAL-358)
// ============================================================================

pub const MAX_LUMINOUS_CONTOUR_POINTS: usize = 16_385;
pub const MAX_LUMINOUS_CONTOUR_FRONTS: usize = 8;
const LUMINOUS_CONTOUR_FRONT_KNOT_INTERVALS: usize = 8;
const MAX_LUMINOUS_CONTOUR_SEGMENT_KNOTS: usize =
    2 + MAX_LUMINOUS_CONTOUR_FRONTS * (LUMINOUS_CONTOUR_FRONT_KNOT_INTERVALS + 1);

/// Checked screen-space halo appearance for a luminous contour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminousContourAppearance {
    core_luminance_lift: f32,
    inner_aura_radius: f32,
    inner_aura_intensity: f32,
    halo_radius: f32,
    halo_intensity: f32,
    halo_falloff: f32,
    front_opacity_boost: f32,
}

impl LuminousContourAppearance {
    pub fn try_new(
        halo_radius: f32,
        halo_intensity: f32,
        halo_falloff: f32,
    ) -> Result<Self, LuminousContourError> {
        Self::try_new_zoned(0.0, 0.0, 0.0, halo_radius, halo_intensity, halo_falloff)
    }

    pub fn try_new_zoned(
        core_luminance_lift: f32,
        inner_aura_radius: f32,
        inner_aura_intensity: f32,
        halo_radius: f32,
        halo_intensity: f32,
        halo_falloff: f32,
    ) -> Result<Self, LuminousContourError> {
        if !core_luminance_lift.is_finite() || !(0.0..=0.75).contains(&core_luminance_lift) {
            return Err(LuminousContourError::InvalidCoreLuminanceLift);
        }
        if !inner_aura_radius.is_finite() || !(0.0..=4.0).contains(&inner_aura_radius) {
            return Err(LuminousContourError::InvalidInnerAuraRadius);
        }
        if !inner_aura_intensity.is_finite() || !(0.0..=0.75).contains(&inner_aura_intensity) {
            return Err(LuminousContourError::InvalidInnerAuraIntensity);
        }
        if !halo_radius.is_finite() || !(0.0..=8.0).contains(&halo_radius) {
            return Err(LuminousContourError::InvalidHaloRadius);
        }
        if !halo_intensity.is_finite() || !(0.0..=0.25).contains(&halo_intensity) {
            return Err(LuminousContourError::InvalidHaloIntensity);
        }
        if !halo_falloff.is_finite() || !(2.0..=8.0).contains(&halo_falloff) {
            return Err(LuminousContourError::InvalidHaloFalloff);
        }
        if inner_aura_radius > halo_radius {
            return Err(LuminousContourError::InvalidAuraRadiusOrder);
        }
        Ok(Self {
            core_luminance_lift,
            inner_aura_radius,
            inner_aura_intensity,
            halo_radius,
            halo_intensity,
            halo_falloff,
            front_opacity_boost: 0.0,
        })
    }

    /// Opt into localized source-alpha uplift at static contour fronts (HAL358 §12).
    /// Zero preserves the original appearance; the source color is unchanged.
    pub fn with_front_opacity_boost(self, boost: f32) -> Result<Self, LuminousContourError> {
        if !boost.is_finite() || !(0.0..=1.0).contains(&boost) {
            return Err(LuminousContourError::InvalidFrontOpacityBoost);
        }
        Ok(Self {
            front_opacity_boost: if boost == 0.0 { 0.0 } else { boost },
            ..self
        })
    }

    pub const fn front_opacity_boost(self) -> f32 {
        self.front_opacity_boost
    }

    pub const fn core_luminance_lift(self) -> f32 {
        self.core_luminance_lift
    }

    pub const fn inner_aura_radius(self) -> f32 {
        self.inner_aura_radius
    }

    pub const fn inner_aura_intensity(self) -> f32 {
        self.inner_aura_intensity
    }

    pub const fn halo_radius(self) -> f32 {
        self.halo_radius
    }

    pub const fn halo_intensity(self) -> f32 {
        self.halo_intensity
    }

    pub const fn halo_falloff(self) -> f32 {
        self.halo_falloff
    }
}

/// One consumer-positioned visual energy range along a luminous contour.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminousContourFront {
    distance: f32,
    half_extent: f32,
    intensity: f32,
}

impl LuminousContourFront {
    pub fn try_new(
        distance: f32,
        half_extent: f32,
        intensity: f32,
    ) -> Result<Self, LuminousContourError> {
        if !distance.is_finite() || distance < 0.0 {
            return Err(LuminousContourError::InvalidFrontDistance);
        }
        if !half_extent.is_finite() || !(2.0..=32.0).contains(&half_extent) {
            return Err(LuminousContourError::InvalidFrontHalfExtent);
        }
        if !intensity.is_finite() || !(0.0..=1.0).contains(&intensity) {
            return Err(LuminousContourError::InvalidFrontIntensity);
        }
        Ok(Self {
            distance,
            half_extent,
            intensity,
        })
    }

    pub const fn distance(self) -> f32 {
        self.distance
    }

    pub const fn half_extent(self) -> f32 {
        self.half_extent
    }

    pub const fn intensity(self) -> f32 {
        self.intensity
    }
}

/// Inline bounded front set. Construction performs no heap allocation.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LuminousContourFronts {
    entries: [Option<LuminousContourFront>; MAX_LUMINOUS_CONTOUR_FRONTS],
    len: u8,
}

impl LuminousContourFronts {
    pub const fn empty() -> Self {
        Self {
            entries: [None; MAX_LUMINOUS_CONTOUR_FRONTS],
            len: 0,
        }
    }

    pub fn try_from_slice(fronts: &[LuminousContourFront]) -> Result<Self, LuminousContourError> {
        if fronts.len() > MAX_LUMINOUS_CONTOUR_FRONTS {
            return Err(LuminousContourError::TooManyFronts);
        }
        let mut result = Self::empty();
        for (index, front) in fronts.iter().copied().enumerate() {
            result.entries[index] = Some(front);
        }
        result.len = fronts.len() as u8;
        Ok(result)
    }

    pub const fn len(self) -> usize {
        self.len as usize
    }

    pub const fn is_empty(self) -> bool {
        self.len == 0
    }

    pub fn iter(&self) -> impl Iterator<Item = LuminousContourFront> + '_ {
        self.entries[..self.len()].iter().flatten().copied()
    }
}

impl Default for LuminousContourFronts {
    fn default() -> Self {
        Self::empty()
    }
}

/// Typed rejection from the checked luminous-contour paint boundary.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LuminousContourError {
    InvalidCoreLuminanceLift,
    InvalidInnerAuraRadius,
    InvalidInnerAuraIntensity,
    InvalidAuraRadiusOrder,
    InvalidHaloRadius,
    InvalidHaloIntensity,
    InvalidHaloFalloff,
    InvalidFrontOpacityBoost,
    InvalidFrontDistance,
    InvalidFrontHalfExtent,
    InvalidFrontIntensity,
    TooManyFronts,
    InvalidPointCount,
    NonFinitePoint,
    DegenerateSegment,
    NonFiniteTotalLength,
    InvalidCoreWidth,
    InvalidColor,
    FrontBeyondContour,
    FrontNarrowerThanCore,
}

impl fmt::Display for LuminousContourError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let message = match self {
            Self::InvalidCoreLuminanceLift => "core luminance lift must be finite and in 0..=0.75",
            Self::InvalidInnerAuraRadius => {
                "inner aura radius must be finite and in 0..=4 logical pixels"
            }
            Self::InvalidInnerAuraIntensity => {
                "inner aura intensity must be finite and in 0..=0.75"
            }
            Self::InvalidAuraRadiusOrder => "inner aura radius must not exceed outer halo radius",
            Self::InvalidHaloRadius => "halo radius must be finite and in 0..=8 logical pixels",
            Self::InvalidHaloIntensity => "halo intensity must be finite and in 0..=0.25",
            Self::InvalidHaloFalloff => "halo falloff must be finite and in 2..=8",
            Self::InvalidFrontOpacityBoost => "front opacity boost must be finite and in 0..=1",
            Self::InvalidFrontDistance => "front distance must be finite and non-negative",
            Self::InvalidFrontHalfExtent => {
                "front half extent must be finite and in 2..=32 logical pixels"
            }
            Self::InvalidFrontIntensity => "front intensity must be finite and in 0..=1",
            Self::TooManyFronts => "a luminous contour admits at most eight fronts",
            Self::InvalidPointCount => "a luminous contour requires 2..=16385 points",
            Self::NonFinitePoint => "all luminous-contour points must be finite",
            Self::DegenerateSegment => {
                "all adjacent luminous-contour points must form positive-length segments"
            }
            Self::NonFiniteTotalLength => {
                "luminous-contour total arc length must be positive and finite"
            }
            Self::InvalidCoreWidth => {
                "luminous-contour core width must be finite and in 0.25..=4 logical pixels"
            }
            Self::InvalidColor => "luminous-contour color must be finite linear RGBA in 0..=1",
            Self::FrontBeyondContour => "front distance exceeds the contour arc length",
            Self::FrontNarrowerThanCore => {
                "front half extent must be at least the contour core width"
            }
        };
        formatter.write_str(message)
    }
}

impl std::error::Error for LuminousContourError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct LuminousContourRegion {
    pub start: Point,
    pub end: Point,
    pub start_ownership_normal: Point,
    pub end_ownership_normal: Point,
    pub start_gain: f32,
    pub end_gain: f32,
}

#[derive(Debug, Clone, Copy)]
struct ResolvedLuminousContourFront {
    start: f32,
    end: f32,
    center: f32,
    intensity: f32,
}

fn resolve_luminous_contour_fronts(
    total_length: f32,
    fronts: &LuminousContourFronts,
) -> [Option<ResolvedLuminousContourFront>; MAX_LUMINOUS_CONTOUR_FRONTS] {
    let mut resolved = [None; MAX_LUMINOUS_CONTOUR_FRONTS];
    for (index, front) in fronts.iter().enumerate() {
        let target_length = (front.half_extent() * 2.0).min(total_length);
        let start =
            (front.distance() - front.half_extent()).clamp(0.0, total_length - target_length);
        let end = start + target_length;
        resolved[index] = Some(ResolvedLuminousContourFront {
            start,
            end,
            center: (start + end) * 0.5,
            intensity: front.intensity(),
        });
    }
    resolved
}

fn luminous_contour_front_gain(
    distance: f32,
    fronts: &[Option<ResolvedLuminousContourFront>; MAX_LUMINOUS_CONTOUR_FRONTS],
) -> f32 {
    fronts.iter().flatten().fold(0.0, |gain, front| {
        let half_extent = (front.end - front.start) * 0.5;
        let triangular = (1.0 - (distance - front.center).abs() / half_extent).clamp(0.0, 1.0);
        let smooth = triangular * triangular * (3.0 - 2.0 * triangular);
        gain.max(front.intensity * smooth)
    })
}

fn luminous_contour_unit_direction(a: Point, b: Point) -> Point {
    let inverse_length = 1.0 / a.distance(b);
    Point::new((b.x - a.x) * inverse_length, (b.y - a.y) * inverse_length)
}

fn luminous_contour_join_normal(incoming: Point, outgoing: Point) -> Point {
    let x = incoming.x + outgoing.x;
    let y = incoming.y + outgoing.y;
    let length_squared = x * x + y * y;
    if length_squared <= 1.0e-6 {
        return Point::new(0.0, 0.0);
    }
    let inverse_length = 1.0 / length_squared.sqrt();
    Point::new(x * inverse_length, y * inverse_length)
}

fn luminous_contour_interpolate(a: Point, b: Point, t: f32) -> Point {
    Point::new(a.x + (b.x - a.x) * t, a.y + (b.y - a.y) * t)
}

/// Visit the single-owner analytic regions for a complete luminous contour.
///
/// The return value is the number of emitted regions. Batching and GPU lowering
/// both use this function so their counts cannot diverge.
pub(crate) fn visit_luminous_contour_regions(
    points: &[Point],
    total_length: f32,
    fronts: &LuminousContourFronts,
    mut visitor: impl FnMut(LuminousContourRegion),
) -> u32 {
    let resolved_fronts = resolve_luminous_contour_fronts(total_length, fronts);
    let source_segment_count = points.len() - 1;
    let closed = points[0] == points[points.len() - 1];
    let mut emitted = 0u32;
    let mut source_start_distance = 0.0;

    for source_index in 0..source_segment_count {
        let source_start = points[source_index];
        let source_end = points[source_index + 1];
        let source_length = source_start.distance(source_end);
        let source_end_distance = source_start_distance + source_length;
        let source_direction = luminous_contour_unit_direction(source_start, source_end);

        let source_start_normal = if source_index > 0 {
            luminous_contour_join_normal(
                luminous_contour_unit_direction(points[source_index - 1], source_start),
                source_direction,
            )
        } else if closed {
            luminous_contour_join_normal(
                luminous_contour_unit_direction(
                    points[source_segment_count - 1],
                    points[source_segment_count],
                ),
                source_direction,
            )
        } else {
            Point::new(0.0, 0.0)
        };

        let source_end_normal = if source_index + 1 < source_segment_count {
            luminous_contour_join_normal(
                source_direction,
                luminous_contour_unit_direction(source_end, points[source_index + 2]),
            )
        } else if closed {
            luminous_contour_join_normal(
                source_direction,
                luminous_contour_unit_direction(points[0], points[1]),
            )
        } else {
            Point::new(0.0, 0.0)
        };

        let mut knots = [0.0; MAX_LUMINOUS_CONTOUR_SEGMENT_KNOTS];
        knots[0] = source_start_distance;
        knots[1] = source_end_distance;
        let mut knot_count = 2usize;
        for front in resolved_fronts.iter().flatten() {
            for knot_index in 0..=LUMINOUS_CONTOUR_FRONT_KNOT_INTERVALS {
                let t = knot_index as f32 / LUMINOUS_CONTOUR_FRONT_KNOT_INTERVALS as f32;
                let knot = front.start + (front.end - front.start) * t;
                if knot > source_start_distance && knot < source_end_distance {
                    knots[knot_count] = knot;
                    knot_count += 1;
                }
            }
        }

        knots[..knot_count].sort_by(f32::total_cmp);
        let mut unique_count = 1usize;
        for knot_index in 1..knot_count {
            if knots[knot_index] != knots[unique_count - 1] {
                knots[unique_count] = knots[knot_index];
                unique_count += 1;
            }
        }

        for region_index in 0..unique_count - 1 {
            let start_distance = knots[region_index];
            let end_distance = knots[region_index + 1];
            let start_t = (start_distance - source_start_distance) / source_length;
            let end_t = (end_distance - source_start_distance) / source_length;
            visitor(LuminousContourRegion {
                start: luminous_contour_interpolate(source_start, source_end, start_t),
                end: luminous_contour_interpolate(source_start, source_end, end_t),
                start_ownership_normal: if region_index == 0 {
                    source_start_normal
                } else {
                    source_direction
                },
                end_ownership_normal: if region_index + 1 == unique_count - 1 {
                    source_end_normal
                } else {
                    source_direction
                },
                start_gain: luminous_contour_front_gain(start_distance, &resolved_fronts),
                end_gain: luminous_contour_front_gain(end_distance, &resolved_fronts),
            });
            emitted += 1;
        }

        source_start_distance = source_end_distance;
    }
    emitted
}

// ============================================================================
// TRACK DISPLAY STYLE (SPEC-HAL-278)
// ============================================================================

/// SPEC-HAL-278: Display style for track-based widgets (Slider, RangeSlider, ProgressBar).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TrackDisplayStyle {
    /// Thin track line with circle thumb(s). Classic slider look.
    #[default]
    Track,
    /// Filled rectangle with edge handles. Dense data UI look.
    Bar,
    /// Thick bar with fully rounded ends (pill/capsule shape).
    Pill,
    /// Circular arc / ring gauge. Requires radial_gauge pipeline.
    Radial,
}

// ============================================================================
// SHADOW MODE (owned by paint, not scene)
// ============================================================================

/// SPEC-R-013-v2: Shadow rendering mode (paint-layer definition).
/// Used by widgets to specify shadow type when calling push_shadow.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum ShadowMode {
    /// Gaussian falloff outside shape boundary (drop shadow)
    Drop = 0,
    /// Gaussian falloff inside shape boundary (inner shadow)
    Inner = 1,
    /// Gaussian band at fixed distance from shape boundary (focus ring)
    Ring = 2,
}

// ============================================================================
// GRADIENT TYPES (owned by paint, not scene)
// ============================================================================

/// SPEC-R-012-v2: Gradient type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u32)]
pub enum GradientKind {
    /// Color interpolation along a direction vector
    Linear = 0,
    /// Color interpolation from center outward
    Radial = 1,
}

/// SPEC-R-012-v2: A single color stop in a gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    /// Position along gradient [0.0 .. 1.0]
    pub offset: f32,
    /// Straight linear RGBA; premultiplied once before GPU interpolation.
    pub color: Color,
}

/// SPEC-R-012-v2: Gradient definition.
/// Coordinates p0/p1 are in normalized rect-local space [0..1].
#[derive(Debug, Clone, PartialEq)]
pub struct GradientDef {
    /// Gradient type
    pub kind: GradientKind,
    /// Color stops (minimum 2, maximum 8, ordered by offset)
    pub stops: Vec<ColorStop>,
    /// Start point (linear) or center (radial), normalized [0..1]
    pub p0: (f32, f32),
    /// End point (linear) or edge point (radial), normalized [0..1]
    pub p1: (f32, f32),
}

/// Immutable, bounded use of the existing linear/radial Gradient representation.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceGradient(std::sync::Arc<GradientDef>);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSurfaceGradient {
    StopCount,
    Coordinates,
    Stops,
}

impl SurfaceGradient {
    pub fn new(definition: GradientDef) -> Result<Self, InvalidSurfaceGradient> {
        let unit = |v: f32| v.is_finite() && (0.0..=1.0).contains(&v);
        if !(2..=8).contains(&definition.stops.len()) {
            return Err(InvalidSurfaceGradient::StopCount);
        }
        if ![
            definition.p0.0,
            definition.p0.1,
            definition.p1.0,
            definition.p1.1,
        ]
        .into_iter()
        .all(unit)
        {
            return Err(InvalidSurfaceGradient::Coordinates);
        }
        if !definition.stops.iter().all(|s| {
            [s.offset, s.color.r, s.color.g, s.color.b, s.color.a]
                .into_iter()
                .all(unit)
        }) || !definition
            .stops
            .windows(2)
            .all(|s| s[0].offset <= s[1].offset)
        {
            return Err(InvalidSurfaceGradient::Stops);
        }
        Ok(Self(std::sync::Arc::new(definition)))
    }

    pub fn definition(&self) -> &GradientDef {
        &self.0
    }
}

/// HAL386/389: a finite, immutable light basis for the direct analytic face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AnalyticSurfaceLight {
    direction: [f32; 3],
    color: Color,
    strength: f32,
    center_uv: crate::core::math::Point,
    radii_uv: crate::core::math::Point,
    rotation: crate::core::math::Point,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnalyticSurfaceError {
    NonFiniteInput,
    InvalidBaseGradient,
    InvalidLight,
    InvalidParameters,
    InvalidRim,
    InvalidGeometry,
    UnsupportedPaintSink,
    UnsupportedBloomOpacity,
}
impl fmt::Display for AnalyticSurfaceError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HAL386 analytic surface refused: {self:?}")
    }
}
impl std::error::Error for AnalyticSurfaceError {}

fn analytic_unit_color(c: Color) -> bool {
    [c.r, c.g, c.b, c.a]
        .into_iter()
        .all(|v| (0.0..=1.0).contains(&v))
}

impl AnalyticSurfaceLight {
    pub fn try_new(
        direction: [f32; 3],
        color: Color,
        strength: f32,
        center_uv: crate::core::math::Point,
        radii_uv: crate::core::math::Point,
        rotation_cos_sin: crate::core::math::Point,
    ) -> Result<Self, AnalyticSurfaceError> {
        let values = [
            direction[0],
            direction[1],
            direction[2],
            color.r,
            color.g,
            color.b,
            color.a,
            strength,
            center_uv.x,
            center_uv.y,
            radii_uv.x,
            radii_uv.y,
            rotation_cos_sin.x,
            rotation_cos_sin.y,
        ];
        if !values.into_iter().all(f32::is_finite) {
            return Err(AnalyticSurfaceError::NonFiniteInput);
        }
        let length2 = direction.iter().map(|v| v * v).sum::<f32>();
        let rotation2 =
            rotation_cos_sin.x * rotation_cos_sin.x + rotation_cos_sin.y * rotation_cos_sin.y;
        if direction[2] <= 0.0
            || (length2 - 1.0).abs() > 0.0001
            || (rotation2 - 1.0).abs() > 0.0001
            || !length2.is_finite()
            || !rotation2.is_finite()
            || !analytic_unit_color(color)
            || color.a != 1.0
            || !(0.0..=1.0).contains(&strength)
            || ![center_uv.x, center_uv.y]
                .into_iter()
                .all(|v| (0.0..=1.0).contains(&v))
            || ![radii_uv.x, radii_uv.y]
                .into_iter()
                .all(|v| (0.02..=2.0).contains(&v))
        {
            return Err(AnalyticSurfaceError::InvalidLight);
        }
        let length = length2.sqrt();
        let rotation_length = rotation2.sqrt();
        Ok(Self {
            direction: direction.map(|v| v / length),
            color,
            strength,
            center_uv,
            radii_uv,
            rotation: crate::core::math::Point::new(
                rotation_cos_sin.x / rotation_length,
                rotation_cos_sin.y / rotation_length,
            ),
        })
    }
    pub const fn direction(self) -> [f32; 3] {
        self.direction
    }
    pub const fn color(self) -> Color {
        self.color
    }
    pub const fn strength(self) -> f32 {
        self.strength
    }
    pub const fn center_uv(self) -> crate::core::math::Point {
        self.center_uv
    }
    pub const fn radii_uv(self) -> crate::core::math::Point {
        self.radii_uv
    }
    pub const fn rotation_cos_sin(self) -> crate::core::math::Point {
        self.rotation
    }
}

/// Input record; only SurfaceAnalytic's checked constructor admits it.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SmokedGlassParameters {
    pub opacity: f32,
    pub bevel_width: f32,
    pub bevel_slope: f32,
    pub roughness: f32,
    pub ambient_strength: f32,
    pub diffuse_strength: f32,
    pub specular_strength: f32,
    pub max_interior_uplift: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceRimLight {
    color: Color,
    strength: f32,
}
impl SurfaceRimLight {
    pub fn try_new(color: Color, strength: f32) -> Result<Self, AnalyticSurfaceError> {
        if ![color.r, color.g, color.b, color.a, strength]
            .into_iter()
            .all(f32::is_finite)
        {
            return Err(AnalyticSurfaceError::NonFiniteInput);
        }
        if !analytic_unit_color(color) || !(0.0..=0.20).contains(&strength) {
            return Err(AnalyticSurfaceError::InvalidRim);
        }
        Ok(Self { color, strength })
    }
    pub const fn color(self) -> Color {
        self.color
    }
    pub const fn strength(self) -> f32 {
        self.strength
    }
}

/// Checked stationary material-local texture (HAL387 §7), in reference encoded luminance.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceMicrograin {
    scale_px: f32,
    amplitude_srgb: f32,
    seed: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceMicrograinError {
    NonFiniteInput,
    ScaleOutOfRange,
    AmplitudeOutOfRange,
}

impl std::fmt::Display for SurfaceMicrograinError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::NonFiniteInput => "micrograin inputs must be finite",
            Self::ScaleOutOfRange => "micrograin scale must be within 0.75..=3 target pixels",
            Self::AmplitudeOutOfRange => "micrograin reference amplitude must be within 0..=8/255",
        })
    }
}
impl std::error::Error for SurfaceMicrograinError {}

impl SurfaceMicrograin {
    pub fn try_new(
        scale_px: f32,
        amplitude_srgb: f32,
        seed: u32,
    ) -> Result<Self, SurfaceMicrograinError> {
        if !scale_px.is_finite() || !amplitude_srgb.is_finite() {
            return Err(SurfaceMicrograinError::NonFiniteInput);
        }
        if !(0.75..=3.0).contains(&scale_px) {
            return Err(SurfaceMicrograinError::ScaleOutOfRange);
        }
        if !(0.0..=8.0 / 255.0).contains(&amplitude_srgb) {
            return Err(SurfaceMicrograinError::AmplitudeOutOfRange);
        }
        Ok(Self {
            scale_px,
            amplitude_srgb,
            seed,
        })
    }
    pub const fn scale_px(self) -> f32 {
        self.scale_px
    }
    pub const fn amplitude_srgb(self) -> f32 {
        self.amplitude_srgb
    }
    pub const fn seed(self) -> u32 {
        self.seed
    }
}

/// Inputs to the explicitly authored optical coating (HAL386 §11).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassSurfaceOpticsParameters {
    pub bevel_width: f32,
    pub bevel_slope: f32,
    pub roughness: f32,
    pub normal_reflectance: f32,
    pub reflection_strength: f32,
    pub rim_strength: f32,
    pub rim_falloff: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlassSurfaceOpticsError {
    NonFiniteInput,
    InvalidParameters,
    InvalidColor,
}
impl fmt::Display for GlassSurfaceOpticsError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HAL386 glass surface optics refused: {self:?}")
    }
}
impl std::error::Error for GlassSurfaceOpticsError {}

/// HAL395's checked local HDR extraction and SDR Bloom selection.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceBloom {
    gain: f32,
    sigma_logical: f32,
    mix: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceBloomError {
    NonFiniteInput,
    InvalidParameters,
}

impl fmt::Display for SurfaceBloomError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HAL395 surface Bloom refused: {self:?}")
    }
}

impl std::error::Error for SurfaceBloomError {}

impl SurfaceBloom {
    pub fn try_new(gain: f32, sigma_logical: f32, mix: f32) -> Result<Self, SurfaceBloomError> {
        if ![gain, sigma_logical, mix].into_iter().all(f32::is_finite) {
            return Err(SurfaceBloomError::NonFiniteInput);
        }
        if !(0.0..=8.0).contains(&gain)
            || !(sigma_logical == 0.0 || (1.0..=16.0).contains(&sigma_logical))
            || !(0.0..=1.0).contains(&mix)
        {
            return Err(SurfaceBloomError::InvalidParameters);
        }
        Ok(Self { gain, sigma_logical, mix })
    }

    pub const fn gain(&self) -> f32 { self.gain }
    pub const fn sigma_logical(&self) -> f32 { self.sigma_logical }
    pub const fn mix(&self) -> f32 { self.mix }

    fn is_effective(&self) -> bool {
        self.gain > 0.0 && self.sigma_logical > 0.0 && self.mix > 0.0
    }
}

/// Checked parameters for HAL394's angular thin-film RGB coating.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ThinFilmParams {
    strength: f32,
    thickness_nm: f32,
    film_ior: f32,
}

impl ThinFilmParams {
    pub fn try_new(
        strength: f32,
        thickness_nm: f32,
        film_ior: f32,
    ) -> Result<Self, GlassSurfaceOpticsError> {
        if ![strength, thickness_nm, film_ior].into_iter().all(f32::is_finite) {
            return Err(GlassSurfaceOpticsError::NonFiniteInput);
        }
        if !(0.0..=1.0).contains(&strength)
            || !(100.0..=900.0).contains(&thickness_nm)
            || !(1.2..=1.7).contains(&film_ior)
        {
            return Err(GlassSurfaceOpticsError::InvalidParameters);
        }
        Ok(Self { strength, thickness_nm, film_ior })
    }

    pub const fn strength(&self) -> f32 { self.strength }
    pub const fn thickness_nm(&self) -> f32 { self.thickness_nm }
    pub const fn film_ior(&self) -> f32 { self.film_ior }
}

/// Immutable reflection and directional rim; no background, target or resource.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct GlassSurfaceOptics {
    parameters: GlassSurfaceOpticsParameters,
    lights: [AnalyticSurfaceLight; 2],
    reflection_tint: Color,
    rim_tint: Color,
    thin_film: Option<ThinFilmParams>,
    bloom: Option<SurfaceBloom>,
}
impl GlassSurfaceOptics {
    pub fn try_new(
        parameters: GlassSurfaceOpticsParameters,
        lights: [AnalyticSurfaceLight; 2],
        reflection_tint: Color,
        rim_tint: Color,
    ) -> Result<Self, GlassSurfaceOpticsError> {
        let p = parameters;
        if ![p.bevel_width, p.bevel_slope, p.roughness, p.normal_reflectance,
            p.reflection_strength, p.rim_strength, p.rim_falloff,
            reflection_tint.r, reflection_tint.g, reflection_tint.b, reflection_tint.a,
            rim_tint.r, rim_tint.g, rim_tint.b, rim_tint.a]
            .into_iter().all(f32::is_finite) {
            return Err(GlassSurfaceOpticsError::NonFiniteInput);
        }
        if !analytic_unit_color(reflection_tint) || !analytic_unit_color(rim_tint) {
            return Err(GlassSurfaceOpticsError::InvalidColor);
        }
        if !(p.bevel_width == 0.0 || (0.25..=32.0).contains(&p.bevel_width))
            || !(0.0..=0.98).contains(&p.bevel_slope)
            || (p.bevel_width == 0.0 && p.bevel_slope != 0.0)
            || !(0.04..=1.0).contains(&p.roughness)
            || ![p.normal_reflectance, p.reflection_strength, p.rim_strength]
                .into_iter().all(|v| (0.0..=1.0).contains(&v))
            || !(1.0..=8.0).contains(&p.rim_falloff)
            || (p.rim_strength > 0.0 && (p.bevel_width == 0.0 || p.bevel_slope == 0.0)) {
            return Err(GlassSurfaceOpticsError::InvalidParameters);
        }
        Ok(Self { parameters, lights, reflection_tint, rim_tint, thin_film: None, bloom: None })
    }
    /// Select a checked coating. Zero strength is identical to removing the film.
    pub fn try_with_thin_film(
        mut self,
        thin_film: Option<ThinFilmParams>,
    ) -> Result<Self, GlassSurfaceOpticsError> {
        self.thin_film = thin_film.filter(|film| film.strength() > 0.0);
        Ok(self)
    }

    pub const fn parameters(&self) -> GlassSurfaceOpticsParameters { self.parameters }
    pub const fn lights(&self) -> [AnalyticSurfaceLight; 2] { self.lights }
    pub const fn reflection_tint(&self) -> Color { self.reflection_tint }
    pub const fn rim_tint(&self) -> Color { self.rim_tint }
    pub const fn thin_film(&self) -> Option<ThinFilmParams> { self.thin_film }

    /// Zero gain, radius or mix removes the optical selection.
    pub fn with_bloom(mut self, bloom: Option<SurfaceBloom>) -> Self {
        self.bloom = bloom.filter(SurfaceBloom::is_effective);
        self
    }

    pub const fn bloom(&self) -> Option<SurfaceBloom> { self.bloom }

    pub(crate) fn is_effective(&self) -> bool {
        let p = self.parameters;
        self.lights.iter().any(|light| {
            let c = light.color();
            let radiance = light.strength() > 0.0 && c.r.max(c.g).max(c.b) > 0.0;
            let reflection = p.reflection_strength > 0.0 && self.reflection_tint.a > 0.0
                && (p.normal_reflectance > 0.0 || p.bevel_slope > 0.0);
            let d = light.direction();
            let rim = p.rim_strength > 0.0 && self.rim_tint.a > 0.0
                && (d[0] != 0.0 || d[1] != 0.0);
            radiance && (reflection || rim)
        })
    }
}

/// HAL388 §10: one source hue, independently controlled face, edge and halo.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceEmissionParameters {
    pub face_strength: f32,
    pub edge_strength: f32,
    pub halo_strength: f32,
    pub edge_width: f32,
    pub halo_radius: f32,
    pub falloff: f32,
    pub directionality: f32,
    pub direction: crate::core::math::Point,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceEmissionError {
    NonFiniteInput,
    InvalidParameters,
    InvalidColor,
}
impl fmt::Display for SurfaceEmissionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HAL388 surface emission refused: {self:?}")
    }
}
impl std::error::Error for SurfaceEmissionError {}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceEmission {
    color: Color,
    parameters: SurfaceEmissionParameters,
}
impl SurfaceEmission {
    pub fn try_new(color: Color, mut parameters: SurfaceEmissionParameters)
        -> Result<Self, SurfaceEmissionError> {
        let p = parameters;
        if ![color.r, color.g, color.b, color.a, p.face_strength, p.edge_strength,
            p.halo_strength, p.edge_width, p.halo_radius, p.falloff, p.directionality,
            p.direction.x, p.direction.y].into_iter().all(f32::is_finite) {
            return Err(SurfaceEmissionError::NonFiniteInput);
        }
        if !analytic_unit_color(color) { return Err(SurfaceEmissionError::InvalidColor); }
        let length2 = p.direction.x * p.direction.x + p.direction.y * p.direction.y;
        if ![p.face_strength, p.edge_strength, p.halo_strength, p.directionality]
            .into_iter().all(|v| (0.0..=1.0).contains(&v))
            || !(0.25..=32.0).contains(&p.edge_width)
            || !(0.0..=96.0).contains(&p.halo_radius)
            || (p.halo_strength > 0.0 && p.halo_radius == 0.0)
            || !(1.0..=8.0).contains(&p.falloff)
            || !length2.is_finite() || (length2 - 1.0).abs() > 0.0001 {
            return Err(SurfaceEmissionError::InvalidParameters);
        }
        let length = length2.sqrt();
        parameters.direction.x /= length;
        parameters.direction.y /= length;
        Ok(Self { color, parameters })
    }
    pub const fn color(&self) -> Color { self.color }
    pub const fn parameters(&self) -> SurfaceEmissionParameters { self.parameters }
    pub(crate) fn is_effective(&self) -> bool {
        let p = self.parameters;
        self.color.a > 0.0 && self.color.r.max(self.color.g).max(self.color.b) > 0.0
            && (p.face_strength > 0.0 || p.edge_strength > 0.0 || p.halo_strength > 0.0)
    }
    pub(crate) fn halo_extent(&self) -> f32 {
        if self.parameters.halo_strength > 0.0 { self.parameters.halo_radius } else { 0.0 }
    }
}

/// HAL386 sections8/11. Geometry, target, inherited clip and scheduling are not material state.
#[derive(Debug, Clone, PartialEq)]
pub struct SurfaceAnalytic {
    base: SurfaceGradient,
    lights: [AnalyticSurfaceLight; 2],
    parameters: SmokedGlassParameters,
    rim: Option<SurfaceRimLight>,
    micrograin: Option<SurfaceMicrograin>,
    glass_optics: Option<GlassSurfaceOptics>,
    surface_emission: Option<SurfaceEmission>,
}
impl SurfaceAnalytic {
    pub fn try_smoked_glass(
        base: SurfaceGradient,
        lights: [AnalyticSurfaceLight; 2],
        parameters: SmokedGlassParameters,
        rim: Option<SurfaceRimLight>,
    ) -> Result<Self, AnalyticSurfaceError> {
        let d = base.definition();
        if d.kind != GradientKind::Linear
            || d.stops.len() != 3
            || d.stops
                .iter()
                .zip([0.0, 0.30_f32, 1.0])
                .any(|(s, t)| s.offset != t || s.color.a != 1.0 || !analytic_unit_color(s.color))
            || (d.p1.0 - d.p0.0).powi(2) + (d.p1.1 - d.p0.1).powi(2) <= 0.0001
        {
            return Err(AnalyticSurfaceError::InvalidBaseGradient);
        }
        let p = parameters;
        if ![
            p.opacity,
            p.bevel_width,
            p.bevel_slope,
            p.roughness,
            p.ambient_strength,
            p.diffuse_strength,
            p.specular_strength,
            p.max_interior_uplift,
        ]
        .into_iter()
        .all(f32::is_finite)
        {
            return Err(AnalyticSurfaceError::NonFiniteInput);
        }
        if !(0.0..=1.0).contains(&p.opacity)
            || !(p.bevel_width == 0.0 || (0.5..=4.0).contains(&p.bevel_width))
            || !(0.0..=0.9).contains(&p.bevel_slope)
            || !(0.04..=1.0).contains(&p.roughness)
            || ![p.ambient_strength, p.diffuse_strength, p.specular_strength]
                .into_iter()
                .all(|v| (0.0..=1.0).contains(&v))
            || !(0.0..=0.25).contains(&p.max_interior_uplift)
            || (p.bevel_width == 0.0 && p.bevel_slope != 0.0)
        {
            return Err(AnalyticSurfaceError::InvalidParameters);
        }
        if rim.is_some_and(|r| r.strength > 0.0 && r.color.a > 0.0)
            && (p.bevel_width == 0.0 || p.bevel_slope == 0.0)
        {
            return Err(AnalyticSurfaceError::InvalidRim);
        }
        Ok(Self {
            base,
            lights,
            parameters,
            rim,
            micrograin: None,
            glass_optics: None,
            surface_emission: None,
        })
    }
    pub fn base(&self) -> &SurfaceGradient {
        &self.base
    }
    pub const fn lights(&self) -> [AnalyticSurfaceLight; 2] {
        self.lights
    }
    pub const fn parameters(&self) -> SmokedGlassParameters {
        self.parameters
    }
    pub const fn rim(&self) -> Option<SurfaceRimLight> {
        self.rim
    }

    /// HAL387 §7.2: zero amplitude has the same retained identity as absence.
    pub fn with_micrograin(mut self, grain: Option<SurfaceMicrograin>) -> Self {
        self.micrograin = grain.filter(|grain| grain.amplitude_srgb() > 0.0);
        self
    }
    pub const fn micrograin(&self) -> Option<SurfaceMicrograin> {
        self.micrograin
    }

    pub fn with_glass_optics(mut self, optics: Option<GlassSurfaceOptics>) -> Self {
        self.glass_optics = optics;
        self
    }
    pub const fn glass_optics(&self) -> Option<&GlassSurfaceOptics> { self.glass_optics.as_ref() }
    pub fn with_surface_emission(mut self, emission: Option<SurfaceEmission>) -> Self {
        self.surface_emission = emission.filter(SurfaceEmission::is_effective);
        self
    }
    pub const fn surface_emission(&self) -> Option<&SurfaceEmission> { self.surface_emission.as_ref() }
    pub(crate) fn halo_extent(&self) -> f32 {
        self.surface_emission.as_ref().map_or(0.0, SurfaceEmission::halo_extent)
    }

    pub(crate) fn validate_geometry(
        &self,
        rect: RectF,
        corners: RoundedCorners,
    ) -> Result<bool, AnalyticSurfaceError> {
        let has_bloom = self.glass_optics.as_ref().and_then(GlassSurfaceOptics::bloom).is_some();
        if has_bloom && self.parameters.opacity == 0.0 {
            return Err(AnalyticSurfaceError::UnsupportedBloomOpacity);
        }
        let values = [
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            rect.x + rect.width,
            rect.y + rect.height,
            corners.tl,
            corners.tr,
            corners.bl,
            corners.br,
        ];
        if !values.into_iter().all(f32::is_finite) {
            return Err(AnalyticSurfaceError::NonFiniteInput);
        }
        if values[..6].iter().any(|v| v.abs() > 1048576.0)
            || [corners.tl, corners.tr, corners.bl, corners.br]
                .into_iter()
                .any(|v| v < 0.0)
        {
            return Err(AnalyticSurfaceError::InvalidGeometry);
        }
        if rect.width <= 0.0 || rect.height <= 0.0
            || (self.parameters.opacity == 0.0 && self.surface_emission.is_none()) {
            return if has_bloom { Err(AnalyticSurfaceError::InvalidGeometry) } else { Ok(false) };
        }
        let half = rect.width.min(rect.height) * 0.5;
        if rect.width < 1.0
            || rect.height < 1.0
            || self.parameters.bevel_width > half
            || self.glass_optics.as_ref().is_some_and(|v| v.parameters.bevel_width > half)
            || self.surface_emission.as_ref().is_some_and(|v| v.parameters.edge_width > half)
            || [corners.tl, corners.tr, corners.bl, corners.br]
                .into_iter()
                .any(|v| v > half)
        {
            return Err(AnalyticSurfaceError::InvalidGeometry);
        }
        let halo = self.halo_extent();
        if [rect.x - halo, rect.y - halo, rect.x + rect.width + halo,
            rect.y + rect.height + halo].into_iter().any(|v| !v.is_finite() || v.abs() > 1_048_576.0) {
            return Err(AnalyticSurfaceError::InvalidGeometry);
        }
        Ok(true)
    }
}

/// Image fill value. The source type preserves the author's or resource owner's
/// boundary; a registration number alone is not proof of live renderer admission.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceImage<Source = u64> {
    pub source: Source,
    pub uv_rect: RectF,
    /// Straight linear tint; the existing Image pipeline owns premultiplication.
    pub tint: Color,
}

/// Normalized source splits relative to a NinePatch's selected UV rectangle.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NinePatchSourceInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl NinePatchSourceInsets {
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
}

/// Logical paint extents; these borders do not supply layout padding or size.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct NinePatchDestinationInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl NinePatchDestinationInsets {
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn all(value: f32) -> Self {
        Self::new(value, value, value, value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InvalidSurfaceNinePatch {
    UvRect,
    SourceSplits,
    DestinationInsets,
    Tint,
}

/// Immutable Stretch NinePatch material under SPEC-R-019. Source admission and
/// lifetime remain with the existing Image owner; construction performs no lookup.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SurfaceNinePatch<Source = u64> {
    source: Source,
    uv_rect: RectF,
    source_splits: NinePatchSourceInsets,
    destination_insets: NinePatchDestinationInsets,
    tint: Color,
}

impl<Source> SurfaceNinePatch<Source> {
    pub fn new(
        source: Source,
        uv_rect: RectF,
        source_splits: NinePatchSourceInsets,
        destination_insets: NinePatchDestinationInsets,
        tint: Color,
    ) -> Result<Self, InvalidSurfaceNinePatch> {
        let unit = |value: f32| value.is_finite() && (0.0..=1.0).contains(&value);
        let nonnegative = |value: f32| value.is_finite() && value >= 0.0;
        if !unit(uv_rect.x)
            || !unit(uv_rect.y)
            || !uv_rect.width.is_finite()
            || !uv_rect.height.is_finite()
            || uv_rect.width <= 0.0
            || uv_rect.height <= 0.0
            || f64::from(uv_rect.width) > 1.0 - f64::from(uv_rect.x)
            || f64::from(uv_rect.height) > 1.0 - f64::from(uv_rect.y)
        {
            return Err(InvalidSurfaceNinePatch::UvRect);
        }
        if ![
            source_splits.top,
            source_splits.right,
            source_splits.bottom,
            source_splits.left,
        ]
        .into_iter()
        .all(nonnegative)
            || f64::from(source_splits.left) + f64::from(source_splits.right) >= 1.0
            || f64::from(source_splits.top) + f64::from(source_splits.bottom) >= 1.0
        {
            return Err(InvalidSurfaceNinePatch::SourceSplits);
        }
        if ![
            destination_insets.top,
            destination_insets.right,
            destination_insets.bottom,
            destination_insets.left,
        ]
        .into_iter()
        .all(nonnegative)
        {
            return Err(InvalidSurfaceNinePatch::DestinationInsets);
        }
        if ![tint.r, tint.g, tint.b, tint.a].into_iter().all(unit) {
            return Err(InvalidSurfaceNinePatch::Tint);
        }
        Ok(Self {
            source,
            uv_rect,
            source_splits,
            destination_insets,
            tint,
        })
    }

    pub const fn source(&self) -> &Source {
        &self.source
    }

    pub const fn uv_rect(&self) -> RectF {
        self.uv_rect
    }

    pub const fn source_splits(&self) -> NinePatchSourceInsets {
        self.source_splits
    }

    pub const fn destination_insets(&self) -> NinePatchDestinationInsets {
        self.destination_insets
    }

    pub const fn tint(&self) -> Color {
        self.tint
    }

    pub(crate) fn with_source<Other>(&self, source: Other) -> SurfaceNinePatch<Other> {
        SurfaceNinePatch {
            source,
            uv_rect: self.uv_rect,
            source_splits: self.source_splits,
            destination_insets: self.destination_insets,
            tint: self.tint,
        }
    }

    fn for_each_cell(&self, rect: RectF, mut emit: impl FnMut(RectF, RectF)) {
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return;
        }
        let x = nine_patch_destination_axis(
            rect.x,
            rect.width,
            self.destination_insets.left,
            self.destination_insets.right,
        );
        let y = nine_patch_destination_axis(
            rect.y,
            rect.height,
            self.destination_insets.top,
            self.destination_insets.bottom,
        );
        let u = nine_patch_source_axis(
            self.uv_rect.x,
            self.uv_rect.width,
            self.source_splits.left,
            self.source_splits.right,
        );
        let v = nine_patch_source_axis(
            self.uv_rect.y,
            self.uv_rect.height,
            self.source_splits.top,
            self.source_splits.bottom,
        );
        for row in 0..3 {
            for column in 0..3 {
                let destination = RectF::new(
                    x[column],
                    y[row],
                    x[column + 1] - x[column],
                    y[row + 1] - y[row],
                );
                let source = RectF::new(
                    u[column],
                    v[row],
                    u[column + 1] - u[column],
                    v[row + 1] - v[row],
                );
                if destination.width > 0.0
                    && destination.height > 0.0
                    && source.width > 0.0
                    && source.height > 0.0
                {
                    emit(destination, source);
                }
            }
        }
    }
}

fn nine_patch_destination_axis(origin: f32, length: f32, leading: f32, trailing: f32) -> [f32; 4] {
    // Wide intermediates keep finite border sums/products from overflowing.
    let p = f64::from(origin);
    let length = f64::from(length);
    let leading = f64::from(leading);
    let trailing = f64::from(trailing);
    let sum = leading + trailing;
    let end = (p + length) as f32;
    if length <= sum {
        let border = if length == sum {
            leading
        } else {
            length * (leading / sum)
        };
        let shared = (p + border) as f32;
        [origin, shared, shared, end]
    } else {
        [
            origin,
            (p + leading) as f32,
            (p + length - trailing) as f32,
            end,
        ]
    }
}

fn nine_patch_source_axis(origin: f32, length: f32, leading: f32, trailing: f32) -> [f32; 4] {
    let p = f64::from(origin);
    let length = f64::from(length);
    [
        origin,
        (p + length * f64::from(leading)) as f32,
        (p + length * (1.0 - f64::from(trailing))) as f32,
        (p + length) as f32,
    ]
}

/// Rectangular fill value, independent of geometry, theme policy and resource
/// loading. Optional material storage exists only in its selected variant.
#[derive(Debug, Clone, PartialEq)]
pub enum SurfaceFill<Source = u64> {
    Solid(Color),
    Gradient(SurfaceGradient),
    Image(SurfaceImage<Source>),
    NinePatch(SurfaceNinePatch<Source>),
}

// ============================================================================
// LOCAL AMBIENT GLOW (HAL-388)
// ============================================================================

/// HAL-388: a checked, static elliptical glow lobe in owner-local UV space.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalGlowLobe {
    center_uv: Point,
    radii: Point,
    rotation_cos_sin: Point,
    color: Color,
    intensity: f32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LocalAmbientGlowError {
    TooManyLobes,
    NonFiniteInput,
    CenterOutOfRange,
    RadiusOutOfRange,
    NonUnitRotation,
    ColorOutOfRange,
    IntensityOutOfRange,
    InvalidRect,
    UnsupportedPaintSink,
}

impl fmt::Display for LocalAmbientGlowError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "HAL-388 local ambient glow: {self:?}")
    }
}
impl std::error::Error for LocalAmbientGlowError {}

impl LocalGlowLobe {
    pub fn try_new(
        center_uv: Point,
        radii: Point,
        rotation_cos_sin: Point,
        color: Color,
        intensity: f32,
    ) -> Result<Self, LocalAmbientGlowError> {
        let values = [
            center_uv.x,
            center_uv.y,
            radii.x,
            radii.y,
            rotation_cos_sin.x,
            rotation_cos_sin.y,
            color.r,
            color.g,
            color.b,
            color.a,
            intensity,
        ];
        if !values.into_iter().all(f32::is_finite) {
            return Err(LocalAmbientGlowError::NonFiniteInput);
        }
        if ![center_uv.x, center_uv.y]
            .into_iter()
            .all(|v| (0.0..=1.0).contains(&v))
        {
            return Err(LocalAmbientGlowError::CenterOutOfRange);
        }
        if ![radii.x, radii.y]
            .into_iter()
            .all(|v| (0.5..=48.0).contains(&v))
        {
            return Err(LocalAmbientGlowError::RadiusOutOfRange);
        }
        let norm =
            rotation_cos_sin.x * rotation_cos_sin.x + rotation_cos_sin.y * rotation_cos_sin.y;
        if !(0.9999..=1.0001).contains(&norm) {
            return Err(LocalAmbientGlowError::NonUnitRotation);
        }
        if ![color.r, color.g, color.b, color.a]
            .into_iter()
            .all(|v| (0.0..=1.0).contains(&v))
        {
            return Err(LocalAmbientGlowError::ColorOutOfRange);
        }
        if !(0.0..=1.0).contains(&intensity) {
            return Err(LocalAmbientGlowError::IntensityOutOfRange);
        }
        Ok(Self {
            center_uv,
            radii,
            rotation_cos_sin,
            color,
            intensity,
        })
    }
    pub const fn center_uv(self) -> Point {
        self.center_uv
    }
    pub const fn radii(self) -> Point {
        self.radii
    }
    pub const fn rotation_cos_sin(self) -> Point {
        self.rotation_cos_sin
    }
    pub const fn color(self) -> Color {
        self.color
    }
    pub const fn intensity(self) -> f32 {
        self.intensity
    }
    pub(crate) fn visible(self) -> bool {
        self.color.a > 0.0 && self.intensity > 0.0
    }
}

/// HAL-388: zero, one or two inline-owned lobes, independent of any widget skin.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalAmbientGlowAppearance {
    lobes: [LocalGlowLobe; 2],
    count: u8,
}

impl LocalAmbientGlowAppearance {
    pub fn try_new(lobes: &[LocalGlowLobe]) -> Result<Self, LocalAmbientGlowError> {
        if lobes.len() > 2 {
            return Err(LocalAmbientGlowError::TooManyLobes);
        }
        let empty = LocalGlowLobe {
            center_uv: Point::new(0.0, 0.0),
            radii: Point::new(1.0, 1.0),
            rotation_cos_sin: Point::new(1.0, 0.0),
            color: Color::TRANSPARENT,
            intensity: 0.0,
        };
        let mut result = Self {
            lobes: [empty; 2],
            count: lobes.len() as u8,
        };
        for (index, lobe) in lobes.iter().copied().enumerate() {
            result.lobes[index] = LocalGlowLobe::try_new(
                lobe.center_uv,
                lobe.radii,
                lobe.rotation_cos_sin,
                lobe.color,
                lobe.intensity,
            )?;
        }
        Ok(result)
    }
    pub fn lobes(&self) -> &[LocalGlowLobe] {
        &self.lobes[..usize::from(self.count)]
    }
    pub fn is_visible(self) -> bool {
        self.lobes().iter().any(|l| l.visible())
    }

    /// Validates the owner rect and derives only paint coverage, never layout.
    pub(crate) fn bounds(self, rect: RectF) -> Result<Option<RectF>, LocalAmbientGlowError> {
        let coordinates = [rect.x, rect.y, rect.x + rect.width, rect.y + rect.height];
        if !coordinates
            .into_iter()
            .all(|v| v.is_finite() && v.abs() <= 1_048_576.0)
        {
            return Err(LocalAmbientGlowError::InvalidRect);
        }
        if rect.width <= 0.0 || rect.height <= 0.0 {
            return Ok(None);
        }
        let mut bounds: Option<RectF> = None;
        for lobe in self.lobes().iter().copied().filter(|l| l.visible()) {
            let center = Point::new(
                rect.x + rect.width * lobe.center_uv.x,
                rect.y + rect.height * lobe.center_uv.y,
            );
            let c = lobe.rotation_cos_sin.x;
            let s = lobe.rotation_cos_sin.y;
            let hx = ((lobe.radii.x * c).powi(2) + (lobe.radii.y * s).powi(2)).sqrt();
            let hy = ((lobe.radii.x * s).powi(2) + (lobe.radii.y * c).powi(2)).sqrt();
            let next = RectF::new(center.x - hx, center.y - hy, 2.0 * hx, 2.0 * hy);
            bounds = Some(match bounds {
                None => next,
                Some(old) => {
                    let x = old.x.min(next.x);
                    let y = old.y.min(next.y);
                    RectF::new(
                        x,
                        y,
                        (old.x + old.width).max(next.x + next.width) - x,
                        (old.y + old.height).max(next.y + next.height) - y,
                    )
                }
            });
        }
        Ok(bounds)
    }
}

#[cfg(test)]
pub(crate) fn local_ambient_glow_reference(
    appearance: LocalAmbientGlowAppearance,
    rect: RectF,
    p: Point,
) -> [f32; 4] {
    let mut sum = 0.0;
    let mut remaining = 1.0;
    let mut rgb = [0.0; 3];
    for lobe in appearance.lobes().iter().copied().filter(|l| l.visible()) {
        let dx = p.x - (rect.x + rect.width * lobe.center_uv.x);
        let dy = p.y - (rect.y + rect.height * lobe.center_uv.y);
        let x = (dx * lobe.rotation_cos_sin.x + dy * lobe.rotation_cos_sin.y) / lobe.radii.x;
        let y = (-dx * lobe.rotation_cos_sin.y + dy * lobe.rotation_cos_sin.x) / lobe.radii.y;
        let u = (1.0 - (x * x + y * y).sqrt()).clamp(0.0, 1.0);
        let w = lobe.color.a * lobe.intensity * u * u * (3.0 - 2.0 * u);
        sum += w;
        remaining *= 1.0 - w;
        for (channel, value) in rgb
            .iter_mut()
            .zip([lobe.color.r, lobe.color.g, lobe.color.b])
        {
            *channel += w * value;
        }
    }
    if sum == 0.0 {
        return [0.0; 4];
    }
    let a = 1.0 - remaining;
    [rgb[0] / sum * a, rgb[1] / sum * a, rgb[2] / sum * a, a]
}

#[cfg(test)]
mod local_ambient_glow_tests {
    use super::*;
    fn lobe() -> LocalGlowLobe {
        LocalGlowLobe::try_new(
            Point::new(0.5, 0.5),
            Point::new(10.0, 10.0),
            Point::new(1.0, 0.0),
            Color::WHITE,
            0.5,
        )
        .unwrap()
    }
    #[test]
    fn local_ambient_glow_checked_values_and_zero() {
        let a = lobe();
        assert_eq!(
            LocalAmbientGlowAppearance::try_new(&[a; 3]),
            Err(LocalAmbientGlowError::TooManyLobes)
        );
        for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(
                LocalGlowLobe::try_new(
                    Point::new(invalid, 0.0),
                    a.radii,
                    a.rotation_cos_sin,
                    a.color,
                    0.5
                ),
                Err(LocalAmbientGlowError::NonFiniteInput)
            );
        }
        for value in [-0.01, 1.01] {
            assert_eq!(
                LocalGlowLobe::try_new(
                    Point::new(value, 0.0),
                    a.radii,
                    a.rotation_cos_sin,
                    a.color,
                    0.5
                ),
                Err(LocalAmbientGlowError::CenterOutOfRange)
            );
            assert_eq!(
                LocalGlowLobe::try_new(a.center_uv, a.radii, a.rotation_cos_sin, a.color, value),
                Err(LocalAmbientGlowError::IntensityOutOfRange)
            );
            assert_eq!(
                LocalGlowLobe::try_new(
                    a.center_uv,
                    a.radii,
                    a.rotation_cos_sin,
                    Color::from_linear(value, 0.0, 0.0, 1.0),
                    0.5
                ),
                Err(LocalAmbientGlowError::ColorOutOfRange)
            );
        }
        for radius in [0.0, 0.49, 48.01] {
            assert_eq!(
                LocalGlowLobe::try_new(
                    a.center_uv,
                    Point::new(radius, 1.0),
                    a.rotation_cos_sin,
                    a.color,
                    0.5
                ),
                Err(LocalAmbientGlowError::RadiusOutOfRange)
            );
        }
        assert_eq!(
            LocalGlowLobe::try_new(a.center_uv, a.radii, Point::new(0.0, 0.0), a.color, 0.5),
            Err(LocalAmbientGlowError::NonUnitRotation)
        );
        let empty = LocalAmbientGlowAppearance::try_new(&[]).unwrap();
        assert!(!empty.is_visible());
        assert_eq!(empty.bounds(RectF::new(0.0, 0.0, 10.0, 10.0)), Ok(None));
        assert_eq!(
            empty.bounds(RectF::new(f32::NAN, 0.0, 1.0, 1.0)),
            Err(LocalAmbientGlowError::InvalidRect)
        );
        assert_eq!(
            LocalAmbientGlowAppearance::try_new(&[a])
                .unwrap()
                .bounds(RectF::new(0.0, 0.0, 0.0, 2.0)),
            Ok(None)
        );
    }
    #[test]
    fn local_ambient_glow_reference_compact_commutative_premultiplied() {
        let a = lobe();
        let rect = RectF::new(0.0, 0.0, 20.0, 20.0);
        let one = LocalAmbientGlowAppearance::try_new(&[a]).unwrap();
        assert_eq!(
            local_ambient_glow_reference(one, rect, Point::new(10.0, 10.0)),
            [0.5; 4]
        );
        assert_eq!(
            local_ambient_glow_reference(one, rect, Point::new(20.0, 10.0)),
            [0.0; 4]
        );
        assert_eq!(
            local_ambient_glow_reference(one, rect, Point::new(30.0, 10.0)),
            [0.0; 4]
        );
        let b = LocalGlowLobe::try_new(
            a.center_uv,
            a.radii,
            a.rotation_cos_sin,
            Color::from_linear(0.0, 0.0, 1.0, 1.0),
            0.75,
        )
        .unwrap();
        let ab = LocalAmbientGlowAppearance::try_new(&[a, b]).unwrap();
        let ba = LocalAmbientGlowAppearance::try_new(&[b, a]).unwrap();
        let center = local_ambient_glow_reference(ab, rect, Point::new(10.0, 10.0));
        assert_eq!(center[3], 0.875);
        for y in -5..26 {
            for x in -5..26 {
                let p = Point::new(x as f32, y as f32);
                let v = local_ambient_glow_reference(ab, rect, p);
                assert_eq!(v, local_ambient_glow_reference(ba, rect, p));
                assert!(
                    v.into_iter()
                        .all(|c| c.is_finite() && (0.0..=1.0).contains(&c))
                );
                assert!(v[..3].iter().all(|c| *c <= v[3]));
            }
        }
    }
}

#[cfg(test)]
pub(crate) mod analytic_surface_tests {
    use super::*;
    use crate::core::math::Point;

    pub(crate) fn coating_parameters() -> GlassSurfaceOpticsParameters {
        GlassSurfaceOpticsParameters { bevel_width: 8.0, bevel_slope: 0.9,
            roughness: 0.35, normal_reflectance: 0.1, reflection_strength: 0.55,
            rim_strength: 0.7, rim_falloff: 1.5 }
    }
    pub(crate) fn coating(direction_x: f32) -> GlassSurfaceOptics {
        GlassSurfaceOptics::try_new(coating_parameters(), fixture(1.0, 0.0, direction_x).lights(),
            Color::WHITE, Color::from_linear(0.8, 0.9, 1.0, 1.0)).unwrap()
    }
    pub(crate) fn emission_parameters() -> SurfaceEmissionParameters {
        SurfaceEmissionParameters { face_strength: 0.025, edge_strength: 0.2,
            halo_strength: 0.1, edge_width: 2.0, halo_radius: 18.0,
            falloff: 2.0, directionality: 0.6, direction: Point::new(0.0, 1.0) }
    }
    pub(crate) fn emission() -> SurfaceEmission {
        SurfaceEmission::try_new(Color::from_srgb(0.82, 0.74, 0.56, 1.0), emission_parameters()).unwrap()
    }

    #[test]
    fn analytic_surface_thin_film_is_checked_and_zero_is_absent() {
        use GlassSurfaceOpticsError::{InvalidParameters, NonFiniteInput};

        for bad in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(ThinFilmParams::try_new(bad, 320.0, 1.4), Err(NonFiniteInput));
            assert_eq!(ThinFilmParams::try_new(0.0, bad, 1.4), Err(NonFiniteInput));
            assert_eq!(ThinFilmParams::try_new(0.0, 320.0, bad), Err(NonFiniteInput));
        }
        for [strength, nm, ior] in [
            [-0.01, 320.0, 1.4], [1.01, 320.0, 1.4],
            [0.0, 99.0, 1.4], [1.0, 901.0, 1.4],
            [0.0, 320.0, 1.19], [1.0, 320.0, 1.71],
        ] {
            assert_eq!(ThinFilmParams::try_new(strength, nm, ior), Err(InvalidParameters));
        }
        let bare = coating(0.6);
        for (strength, nm, ior) in [(0.0, 100.0, 1.2), (-0.0, 900.0, 1.7)] {
            let zero = ThinFilmParams::try_new(strength, nm, ior).unwrap();
            assert_eq!(bare.try_with_thin_film(Some(zero)).unwrap(), bare);
        }
        let film = ThinFilmParams::try_new(1.0, 900.0, 1.7).unwrap();
        let coated = bare.try_with_thin_film(Some(film)).unwrap();
        assert_eq!(coated.thin_film(), Some(film));
        assert_eq!(coated.try_with_thin_film(None).unwrap(), bare);
    }

    #[test]
    fn analytic_surface_s2_checked_coating_and_independent_emission() {
        let lights = fixture(1.0, 0.0, 0.6).lights();
        let make = |p| GlassSurfaceOptics::try_new(p, lights, Color::WHITE, Color::WHITE);
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            for field in 0..7 {
                let mut p = coating_parameters();
                match field { 0=>p.bevel_width=value, 1=>p.bevel_slope=value, 2=>p.roughness=value,
                    3=>p.normal_reflectance=value, 4=>p.reflection_strength=value,
                    5=>p.rim_strength=value, _=>p.rim_falloff=value }
                assert_eq!(make(p), Err(GlassSurfaceOpticsError::NonFiniteInput));
            }
        }
        for (width, slope, roughness, rim, falloff) in [(-1.0,0.9,0.35,0.7,1.5),
            (0.1,0.9,0.35,0.7,1.5), (32.01,0.9,0.35,0.7,1.5), (0.0,0.1,0.35,0.0,1.5),
            (8.0,0.0,0.35,0.7,1.5), (8.0,0.981,0.35,0.7,1.5), (8.0,0.9,0.039,0.7,1.5),
            (8.0,0.9,1.01,0.7,1.5), (8.0,0.9,0.35,1.01,1.5), (8.0,0.9,0.35,0.7,8.01)] {
            let mut p=coating_parameters();p.bevel_width=width;p.bevel_slope=slope;
            p.roughness=roughness;p.rim_strength=rim;p.rim_falloff=falloff;
            assert_eq!(make(p),Err(GlassSurfaceOpticsError::InvalidParameters));
        }
        assert_eq!(GlassSurfaceOptics::try_new(coating_parameters(),lights,
            Color::from_linear(-0.1,0.0,0.0,1.0),Color::WHITE),Err(GlassSurfaceOpticsError::InvalidColor));
        let original=fixture(0.0,0.0,0.6);
        let emitting=original.clone().with_glass_optics(Some(coating(0.6))).with_surface_emission(Some(emission()));
        let rect=RectF::new(30.0,30.0,80.0,60.0);
        assert_eq!(original.validate_geometry(rect,RoundedCorners::uniform(4.0)),Ok(false));
        assert_eq!(emitting.validate_geometry(rect,RoundedCorners::uniform(4.0)),Ok(true));
        assert_eq!(emitting.base(),original.base());assert_eq!(emitting.parameters(),original.parameters());
        assert_eq!(emitting.lights(),original.lights());assert_eq!(emitting.rim(),original.rim());
        assert_eq!(emitting.halo_extent(),18.0);
        assert_eq!(emitting.clone().with_surface_emission(None).with_glass_optics(None),original);
        assert_eq!(emitting.validate_geometry(RectF::new(0.0,0.0,10.0,10.0),RoundedCorners::uniform(0.0)),
            Err(AnalyticSurfaceError::InvalidGeometry));
        assert_eq!(emitting.validate_geometry(RectF::new(1_048_560.0,0.0,16.0,60.0),RoundedCorners::uniform(0.0)),
            Err(AnalyticSurfaceError::InvalidGeometry));
        for color in [Color::TRANSPARENT,Color::BLACK] {
            let zero=SurfaceEmission::try_new(color,emission_parameters()).unwrap();
            assert_eq!(original.clone().with_surface_emission(Some(zero)),original);
        }
        let mut p=emission_parameters();p.face_strength=0.0;p.edge_strength=0.0;p.halo_strength=0.0;
        assert_eq!(original.clone().with_surface_emission(Some(SurfaceEmission::try_new(Color::WHITE,p).unwrap())),original);
        for radius in [-0.1,96.01,f32::NAN,f32::INFINITY] {
            let mut p=emission_parameters();p.halo_radius=radius;
            assert!(SurfaceEmission::try_new(Color::WHITE,p).is_err());
        }
        let mut p=emission_parameters();p.halo_radius=0.0;
        assert_eq!(SurfaceEmission::try_new(Color::WHITE,p),Err(SurfaceEmissionError::InvalidParameters));
        p.halo_strength=0.0;p.direction=Point::new(0.0,0.0);
        assert_eq!(SurfaceEmission::try_new(Color::WHITE,p),Err(SurfaceEmissionError::InvalidParameters));
        p.direction=Point::new(0.0,1.00001);
        let accepted=SurfaceEmission::try_new(Color::WHITE,p).unwrap();
        assert_eq!(accepted.parameters().direction,Point::new(0.0,1.0));
        assert!(SurfaceMicrograin::try_new(1.5,8.0/255.0,387).is_ok());
    }
    #[test]
    fn analytic_surface_micrograin_checked_value_and_zero_identity() {
        use SurfaceMicrograinError::*;
        for x in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            assert_eq!(SurfaceMicrograin::try_new(x, 0.0, 0), Err(NonFiniteInput));
            assert_eq!(SurfaceMicrograin::try_new(1.0, x, 0), Err(NonFiniteInput));
        }
        for x in [-0.0, 0.749, 3.001] {
            assert_eq!(SurfaceMicrograin::try_new(x, 0.0, 0), Err(ScaleOutOfRange));
        }
        for x in [-0.001, 8.0 / 255.0 + 0.00001] {
            assert_eq!(
                SurfaceMicrograin::try_new(1.0, x, 0),
                Err(AmplitudeOutOfRange)
            );
        }
        let material = fixture(0.5, 0.2, 0.6);
        for scale in [0.75, 3.0] {
            for amplitude in [0.0, -0.0] {
                let zero = SurfaceMicrograin::try_new(scale, amplitude, u32::MAX).unwrap();
                assert_eq!(material.clone().with_micrograin(Some(zero)), material);
                assert_eq!(material.micrograin(), None);
            }
        }
        let grain = SurfaceMicrograin::try_new(0.75, 2.0 / 255.0, u32::MAX).unwrap();
        assert_eq!(
            (grain.scale_px(), grain.amplitude_srgb(), grain.seed()),
            (0.75, 2.0 / 255.0, u32::MAX)
        );
        let textured = material.clone().with_micrograin(Some(grain));
        assert_eq!(textured.micrograin(), Some(grain));
        assert_eq!(textured.parameters(), material.parameters());
        assert_eq!(textured.lights(), material.lights());
        assert_eq!(textured.rim(), material.rim());
        assert_eq!(textured.with_micrograin(None), material);
    }

    pub(crate) fn fixture(opacity: f32, rim: f32, direction_x: f32) -> SurfaceAnalytic {
        let base = SurfaceGradient::new(GradientDef {
            kind: GradientKind::Linear,
            p0: (0.0, 0.0),
            p1: (0.0, 1.0),
            stops: vec![
                ColorStop {
                    offset: 0.0,
                    color: Color::from_linear(0.025, 0.040, 0.065, 1.0),
                },
                ColorStop {
                    offset: 0.30,
                    color: Color::from_linear(0.010, 0.020, 0.035, 1.0),
                },
                ColorStop {
                    offset: 1.0,
                    color: Color::from_linear(0.004, 0.008, 0.014, 1.0),
                },
            ],
        })
        .unwrap();
        let light = |x, color| {
            AnalyticSurfaceLight::try_new(
                [x, 0.0, 0.8],
                color,
                1.0,
                Point::new(0.5, 0.5),
                Point::new(0.8, 0.8),
                Point::new(1.0, 0.0),
            )
            .unwrap()
        };
        SurfaceAnalytic::try_smoked_glass(
            base,
            [
                light(direction_x, Color::from_linear(1.0, 0.65, 0.35, 1.0)),
                light(-direction_x, Color::from_linear(0.20, 0.50, 1.0, 1.0)),
            ],
            SmokedGlassParameters {
                opacity,
                bevel_width: 3.0,
                bevel_slope: 0.85,
                roughness: 0.25,
                ambient_strength: 0.05,
                diffuse_strength: 0.8,
                specular_strength: 1.0,
                max_interior_uplift: 0.25,
            },
            Some(SurfaceRimLight::try_new(Color::from_linear(0.75, 0.83, 1.0, 1.0), rim).unwrap()),
        )
        .unwrap()
    }
    #[test]
    fn smoked_glass_checked_values_and_geometry() {
        let m = fixture(1.0, 0.2, 0.6);
        let rect = RectF::new(2.0, 4.0, 100.0, 60.0);
        assert_eq!(
            m.validate_geometry(rect, RoundedCorners::uniform(12.0)),
            Ok(true)
        );
        assert_eq!(
            fixture(0.0, 0.2, 0.6).validate_geometry(rect, RoundedCorners::uniform(12.0)),
            Ok(false)
        );
        assert_eq!(
            m.validate_geometry(
                RectF::new(0.0, 0.0, 0.0, 60.0),
                RoundedCorners::uniform(0.0)
            ),
            Ok(false)
        );
        for r in [
            RectF::new(f32::NAN, 0.0, 100.0, 60.0),
            RectF::new(0.0, 0.0, f32::INFINITY, 60.0),
        ] {
            assert_eq!(
                m.validate_geometry(r, RoundedCorners::uniform(0.0)),
                Err(AnalyticSurfaceError::NonFiniteInput)
            );
        }
        for r in [
            RectF::new(0.0, 0.0, 0.5, 10.0),
            RectF::new(1048576.0, 0.0, 10.0, 10.0),
        ] {
            assert_eq!(
                m.validate_geometry(r, RoundedCorners::uniform(0.0)),
                Err(AnalyticSurfaceError::InvalidGeometry)
            );
        }
        assert_eq!(
            m.validate_geometry(rect, RoundedCorners::uniform(31.0)),
            Err(AnalyticSurfaceError::InvalidGeometry)
        );
        for v in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let mut p = m.parameters();
            p.roughness = v;
            assert!(matches!(
                SurfaceAnalytic::try_smoked_glass(m.base().clone(), m.lights(), p, None),
                Err(AnalyticSurfaceError::NonFiniteInput)
            ));
        }
        for (field, value) in [
            (0, -0.1),
            (0, 1.1),
            (1, 0.4),
            (1, 4.1),
            (2, 0.91),
            (3, 0.039),
            (3, 1.1),
            (4, 1.1),
            (5, -0.1),
            (6, 1.1),
            (7, 0.26),
        ] {
            let mut p = m.parameters();
            match field {
                0 => p.opacity = value,
                1 => p.bevel_width = value,
                2 => p.bevel_slope = value,
                3 => p.roughness = value,
                4 => p.ambient_strength = value,
                5 => p.diffuse_strength = value,
                6 => p.specular_strength = value,
                _ => p.max_interior_uplift = value,
            }
            assert!(matches!(
                SurfaceAnalytic::try_smoked_glass(m.base().clone(), m.lights(), p, None),
                Err(AnalyticSurfaceError::InvalidParameters)
            ));
        }
        for mutate in 0..5 {
            let mut base = m.base().definition().clone();
            match mutate {
                0 => base.kind = GradientKind::Radial,
                1 => base.stops[1].offset = 0.31,
                2 => base.stops[1].color.a = 0.5,
                3 => base.p1 = base.p0,
                _ => {
                    base.stops.pop();
                }
            }
            assert!(matches!(
                SurfaceAnalytic::try_smoked_glass(
                    SurfaceGradient::new(base).unwrap(),
                    m.lights(),
                    m.parameters(),
                    None
                ),
                Err(AnalyticSurfaceError::InvalidBaseGradient)
            ));
        }
    }
    #[test]
    fn smoked_glass_light_admission_is_not_normalization_repair() {
        let make = |direction, color, strength, center, radii, rotation| {
            AnalyticSurfaceLight::try_new(direction, color, strength, center, radii, rotation)
        };
        let p = Point::new(0.5, 0.5);
        let radii = Point::new(0.5, 0.5);
        let rotation = Point::new(1.0, 0.0);
        for d in [
            [0.0, 0.0, 0.0],
            [0.0, 0.0, -1.0],
            [2.0, 0.0, 1.0],
            [f32::MAX, 0.0, 1.0],
        ] {
            assert_eq!(
                make(d, Color::WHITE, 1.0, p, radii, rotation),
                Err(AnalyticSurfaceError::InvalidLight)
            );
        }
        assert_eq!(
            make(
                [0.0, 0.0, 1.0],
                Color::WHITE,
                1.0,
                p,
                Point::new(0.0, 0.5),
                rotation
            ),
            Err(AnalyticSurfaceError::InvalidLight)
        );
        assert_eq!(
            make(
                [0.0, 0.0, 1.0],
                Color::WHITE,
                1.0,
                p,
                radii,
                Point::new(0.9, 0.0)
            ),
            Err(AnalyticSurfaceError::InvalidLight)
        );
        assert_eq!(
            make([f32::NAN, 0.0, 1.0], Color::WHITE, 1.0, p, radii, rotation),
            Err(AnalyticSurfaceError::NonFiniteInput)
        );
        let light = make([0.0, 0.0, 1.00001], Color::WHITE, 0.0, p, radii, rotation).unwrap();
        assert_eq!(light.direction(), [0.0, 0.0, 1.0]);
    }
    #[test]
    fn reflective_rim_light_admission_and_zero_controls() {
        for v in [-0.01, 0.201] {
            assert_eq!(
                SurfaceRimLight::try_new(Color::WHITE, v),
                Err(AnalyticSurfaceError::InvalidRim)
            );
        }
        for v in [f32::NAN, f32::INFINITY] {
            assert_eq!(
                SurfaceRimLight::try_new(Color::WHITE, v),
                Err(AnalyticSurfaceError::NonFiniteInput)
            );
        }
        let m = fixture(1.0, 0.0, 0.6);
        let mut p = m.parameters();
        p.bevel_width = 0.0;
        p.bevel_slope = 0.0;
        assert!(
            SurfaceAnalytic::try_smoked_glass(m.base().clone(), m.lights(), p, m.rim()).is_ok()
        );
        assert!(matches!(
            SurfaceAnalytic::try_smoked_glass(
                m.base().clone(),
                m.lights(),
                p,
                Some(SurfaceRimLight::try_new(Color::WHITE, 0.2).unwrap())
            ),
            Err(AnalyticSurfaceError::InvalidRim)
        ));
        let mut leaf = crate::core::scene::SceneBuilder::new();
        leaf.push_analytic_surface(
            RectF::new(0.0, 0.0, 100.0, 60.0),
            RoundedCorners::uniform(12.0),
            &fixture(0.0, 0.2, 0.6),
            1,
            None,
        )
        .unwrap();
        assert!(
            leaf.build().root.is_empty(),
            "zero opacity emits no SceneNode"
        );
    }
}

/// HAL385 §8.1: immutable optical data. It owns no source image or GPU handle.
#[derive(Debug, Clone, PartialEq)]
pub struct FrostedGlassAppearance {
    sigma_logical: f32,
    tint: Color,
    transmission: f32,
    micrograin: Option<SurfaceMicrograin>,
    glass_optics: Option<GlassSurfaceOptics>,
    refraction_logical: f32,
    coverage_opacity: f32,
    edge_feather_logical: f32,
    surface_emission: Option<SurfaceEmission>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrostedGlassError {
    NonFiniteInput,
    InvalidSigma,
    InvalidTint,
    InvalidTransmission,
    InvalidOptics,
    InvalidRefraction,
    InvalidOpacity,
    InvalidGeometry,
    UnsupportedPaintSink,
    UnsupportedBloomOpacity,
    InvalidEdgeFeather,
}
impl std::fmt::Display for FrostedGlassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "HAL385 Frosted Glass refused: {self:?}")
    }
}
impl std::error::Error for FrostedGlassError {}
impl FrostedGlassAppearance {
    pub fn try_new(sigma_logical: f32, tint: Color, transmission: f32)
        -> Result<Self, FrostedGlassError>
    {
        if ![sigma_logical,tint.r,tint.g,tint.b,tint.a,transmission]
            .into_iter().all(f32::is_finite) {
            return Err(FrostedGlassError::NonFiniteInput);
        }
        if !(0.0..=24.0).contains(&sigma_logical) { return Err(FrostedGlassError::InvalidSigma); }
        if ![tint.r,tint.g,tint.b,tint.a].into_iter().all(|v|(0.0..=1.0).contains(&v)) {
            return Err(FrostedGlassError::InvalidTint);
        }
        if !(0.0..=1.0).contains(&transmission) { return Err(FrostedGlassError::InvalidTransmission); }
        Ok(Self {sigma_logical,tint,transmission,micrograin:None,
            glass_optics:None,refraction_logical:0.0,coverage_opacity:1.0,
            edge_feather_logical:0.0,surface_emission:None})
    }
    pub const fn sigma_logical(&self)->f32 { self.sigma_logical }
    pub const fn tint(&self)->Color { self.tint }
    pub const fn transmission(&self)->f32 { self.transmission }
    pub const fn micrograin(&self)->Option<SurfaceMicrograin> { self.micrograin }
    pub fn with_micrograin(mut self, grain:Option<SurfaceMicrograin>)->Self {
        self.micrograin=grain.filter(|g|g.amplitude_srgb()>0.0); self
    }
    /// HAL385 §11. Coupled invalid input is refused before publishing any field.
    pub fn try_with_optics(mut self, optics:Option<GlassSurfaceOptics>,
        refraction_logical:f32, coverage_opacity:f32)->Result<Self,FrostedGlassError> {
        if !refraction_logical.is_finite() || !coverage_opacity.is_finite() {
            return Err(FrostedGlassError::NonFiniteInput);
        }
        if !(-32.0..=32.0).contains(&refraction_logical) {
            return Err(FrostedGlassError::InvalidRefraction);
        }
        if !(0.0..=1.0).contains(&coverage_opacity) {
            return Err(FrostedGlassError::InvalidOpacity);
        }
        if coverage_opacity == 0.0 && optics.as_ref().and_then(GlassSurfaceOptics::bloom).is_some() {
            return Err(FrostedGlassError::UnsupportedBloomOpacity);
        }
        if refraction_logical != 0.0 && !optics.as_ref().is_some_and(|v| v.parameters.bevel_width > 0.0) {
            return Err(FrostedGlassError::InvalidOptics);
        }
        self.glass_optics=optics;
        self.refraction_logical=refraction_logical;
        self.coverage_opacity=coverage_opacity;
        Ok(self)
    }
    pub const fn glass_optics(&self)->Option<&GlassSurfaceOptics> {self.glass_optics.as_ref()}
    pub const fn refraction_logical(&self)->f32 {self.refraction_logical}
    pub const fn coverage_opacity(&self)->f32 {self.coverage_opacity}
    /// Set the inward body feather in logical pixels (HAL-385 §12).
    /// Accepts finite widths from 0 to 32. Zero uses the unfeathered material.
    pub fn try_with_edge_feather(
        mut self,
        edge_feather_logical: f32,
    ) -> Result<Self, FrostedGlassError> {
        if !edge_feather_logical.is_finite() {
            return Err(FrostedGlassError::NonFiniteInput);
        }
        if !(0.0..=32.0).contains(&edge_feather_logical) {
            return Err(FrostedGlassError::InvalidEdgeFeather);
        }
        self.edge_feather_logical = if edge_feather_logical == 0.0 {
            0.0
        } else {
            edge_feather_logical
        };
        Ok(self)
    }
    /// The inward body feather width in logical pixels.
    pub const fn edge_feather_logical(&self) -> f32 {
        self.edge_feather_logical
    }
    pub fn with_surface_emission(mut self, emission:Option<SurfaceEmission>)->Self {
        self.surface_emission=emission.filter(SurfaceEmission::is_effective); self
    }
    pub const fn surface_emission(&self)->Option<&SurfaceEmission> {self.surface_emission.as_ref()}
    pub(crate) fn halo_extent(&self)->f32 {
        self.surface_emission.as_ref().map_or(0.0,SurfaceEmission::halo_extent)
    }
    pub(crate) fn validate_geometry(&self, rect:RectF, c:RoundedCorners)
        ->Result<bool,FrostedGlassError>
    {
        let has_bloom = self.glass_optics.as_ref().and_then(GlassSurfaceOptics::bloom).is_some();
        if has_bloom && self.coverage_opacity == 0.0 {
            return Err(FrostedGlassError::UnsupportedBloomOpacity);
        }
        let fields=[rect.x,rect.y,rect.width,rect.height,rect.x+rect.width,rect.y+rect.height];
        if !fields.into_iter().all(|v|v.is_finite() && v.abs()<=1_048_576.0)
            || ![c.tl,c.tr,c.bl,c.br].into_iter().all(|v|v.is_finite() && v>=0.0) {
            return Err(FrostedGlassError::InvalidGeometry);
        }
        if rect.width<=0.0 || rect.height<=0.0 {
            return if has_bloom { Err(FrostedGlassError::InvalidGeometry) } else { Ok(false) };
        }
        let half=rect.width.min(rect.height)*0.5;
        if rect.width<1.0 || rect.height<1.0
            || self.glass_optics.as_ref().is_some_and(|v|v.parameters.bevel_width>half)
            || self.surface_emission.as_ref().is_some_and(|v|v.parameters.edge_width>half)
            || [c.tl,c.tr,c.bl,c.br]
            .into_iter().any(|v|v>rect.width.min(rect.height)*0.5) {
            return Err(FrostedGlassError::InvalidGeometry);
        }
        let halo=self.halo_extent();
        if [rect.x-halo,rect.y-halo,rect.x+rect.width+halo,rect.y+rect.height+halo]
            .into_iter().any(|v|!v.is_finite() || v.abs()>1_048_576.0) {
            return Err(FrostedGlassError::InvalidGeometry);
        }
        if self.coverage_opacity==0.0 && self.surface_emission.is_none() {return Ok(false);}
        Ok(has_bloom || self.sigma_logical!=0.0 || self.transmission!=1.0 || self.refraction_logical!=0.0
            || self.glass_optics.as_ref().is_some_and(GlassSurfaceOptics::is_effective)
            || self.surface_emission.is_some())
    }
}

/// Scene-agnostic interface for widget painting operations.
/// Widgets emit paint without depending directly on SceneBuilder.
/// SceneBuilder implements this sink; unsupported sinks refuse glow explicitly.
pub trait PaintSink {
    /// HAL385 §8: a current-prefix request, not a captured image. Build qualifies
    /// inherited clips/order; Compile qualifies the actual target and backing.
    fn push_frosted_glass(
        &mut self,
        _rect: RectF,
        _corners: RoundedCorners,
        _appearance: &FrostedGlassAppearance,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) -> Result<(), FrostedGlassError> {
        Err(FrostedGlassError::UnsupportedPaintSink)
    }
    /// HAL386: local geometry admission only; CPU Build qualifies final inherited clips.
    fn push_analytic_surface(
        &mut self,
        _rect: RectF,
        _corners: RoundedCorners,
        _material: &SurfaceAnalytic,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) -> Result<(), AnalyticSurfaceError> {
        Err(AnalyticSurfaceError::UnsupportedPaintSink)
    }
    /// Local emission only. Final inherited-clip admission belongs to CPU Build.
    fn push_local_ambient_glow(
        &mut self,
        _rect: RectF,
        _appearance: LocalAmbientGlowAppearance,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) -> Result<(), LocalAmbientGlowError> {
        Err(LocalAmbientGlowError::UnsupportedPaintSink)
    }
    /// Engine traversal only: wrap a member without changing its local primitives.
    #[doc(hidden)]
    fn begin_control_subtree_member(&mut self, _member: ControlSubtreeMember) {
        panic!("SPEC-HAL-359: paint sink does not support closed control composition")
    }

    #[doc(hidden)]
    fn end_control_subtree_member(&mut self) {
        panic!("SPEC-HAL-359: paint sink does not support closed control composition")
    }

    // ========================================================================
    // Device Pixel Ratio (SPEC-HAL-266)
    // ========================================================================

    /// SPEC-HAL-266: Device pixel ratio for the current render target.
    /// Widgets use this for DPR-aware snapping (icons, borders, etc.).
    fn scale_factor(&self) -> f32;

    // ========================================================================
    // Clip/Scroll Stack Management
    // ========================================================================

    /// Begin a clip region. Must be paired with `end_clip`.
    fn begin_clip(&mut self, rect: RectF, rounded: Option<RoundedCorners>);

    /// End the current clip region.
    fn end_clip(&mut self);

    /// Begin a scroll offset. Must be paired with `end_scroll`.
    fn begin_scroll(&mut self, offset: (f32, f32));

    /// End the current scroll offset.
    fn end_scroll(&mut self);

    // ========================================================================
    // Primitive Drawing
    // ========================================================================

    /// Push a solid rectangle.
    fn push_rect(&mut self, rect: RectF, color: Color, z_index: ZIndex, key: Option<SceneNodeKey>);

    /// Emit a rectangular fill through existing primitives. Geometry and clips
    /// belong to the caller; Image registrations must already be admitted by
    /// their existing resource owner. This performs no lookup or preparation.
    fn push_surface_fill(
        &mut self,
        rect: RectF,
        fill: &SurfaceFill,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        match fill {
            SurfaceFill::Solid(color) => self.push_rect(rect, *color, z_index, key),
            SurfaceFill::Gradient(gradient) => {
                self.push_gradient_rect(rect, 0.0, gradient.definition(), z_index, key);
            }
            SurfaceFill::Image(image) => {
                self.push_image(rect, image.source, image.uv_rect, image.tint, z_index, key);
            }
            SurfaceFill::NinePatch(material) => {
                let mut key = key;
                material.for_each_cell(rect, |destination, uv_rect| {
                    self.push_image(
                        destination,
                        material.source,
                        uv_rect,
                        material.tint,
                        z_index,
                        key.take(),
                    );
                });
            }
        }
    }

    /// Push a border rectangle (outline only).
    fn push_border_rect(
        &mut self,
        rect: RectF,
        color: Color,
        border_width: f32,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// Push a rounded rectangle.
    fn push_rounded_rect(
        &mut self,
        rect: RectF,
        color: Color,
        corners: RoundedCorners,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// Push a rounded border rectangle (outline only, with corner radii).
    ///
    /// This is the rounded equivalent of `push_border_rect`. Uses 2-rect inset
    /// technique: outer border rect + inner fill rect (transparent for outline-only).
    fn push_border_rounded_rect(
        &mut self,
        rect: RectF,
        color: Color,
        border_width: f32,
        corners: RoundedCorners,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-R-013-v2: Push an analytical shadow effect.
    fn push_shadow(
        &mut self,
        shape_rect: RectF,
        shape_radius: f32,
        mode: ShadowMode,
        offset: (f32, f32),
        spread: f32,
        blur_radius: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-R-012-v2: Push a gradient-filled rectangle.
    fn push_gradient_rect(
        &mut self,
        rect: RectF,
        radius: f32,
        gradient: &GradientDef,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-HAL-272: Push a color picker saturation/value area.
    fn push_color_picker_sv(
        &mut self,
        rect: RectF,
        hue: f32,
        radius: f32,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-HAL-278: Push a radial gauge (circular arc/ring).
    fn push_radial_gauge(
        &mut self,
        rect: RectF,
        fill: f32,
        thickness: f32,
        track_color: Color,
        fill_color: Color,
        fill_color_negative: Color,
        start_angle: f32,
        sweep_angle: f32,
        origin_fill: f32,
        mode: u32,
        fill_style: u32,
        zone_count: u32,
        zones: [(f32, f32, [f32; 4]); 4],
        major_divisions: u32,
        minor_per_major: u32,
        tick_length_major: f32,
        tick_length_minor: f32,
        tick_color: [f32; 4],
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-R-003: Push a circle shape primitive.
    fn push_circle(
        &mut self,
        center: crate::core::math::Point,
        radius: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-R-003: Push an ellipse shape primitive.
    fn push_ellipse(
        &mut self,
        center: crate::core::math::Point,
        radii: crate::core::math::Point,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    // ========================================================================
    // Path / Icon Drawing
    // ========================================================================

    /// Push a filled path node.
    fn push_path(
        &mut self,
        path: crate::renderer::path::Path,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// Emit a path with an explicit fill rule (SPEC-R-004 §13.3).
    /// Success means the sink accepted the scene primitive.
    fn push_path_with_fill_rule(
        &mut self,
        path: crate::renderer::path::Path,
        rule: FillRule,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), PathFillError> {
        match rule {
            FillRule::NonZero => {
                self.push_path(path, color, z_index, key);
                Ok(())
            }
            FillRule::EvenOdd => Err(PathFillError::UnsupportedSink),
        }
    }

    /// SPEC-R-004: Push a stroked path with explicit width, joins, and caps.
    ///
    /// `PathCommand::Close` remains distinct from an open polyline whose final
    /// point repeats its first point, so closed contours receive joins instead
    /// of endpoint caps. Stroke tessellation and mesh ownership stay behind the
    /// paint boundary.
    fn push_stroked_path(
        &mut self,
        _path: crate::renderer::path::Path,
        _stroke: crate::renderer::path::StrokeParams,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
        panic!("SPEC-R-004: this PaintSink does not support stroked paths")
    }

    /// Push an icon from its definition, scaled to `size` logical pixels and translated to `(x, y)`.
    fn push_icon(
        &mut self,
        definition: &crate::core::icon::IconDefinition,
        x: f32,
        y: f32,
        size: f32,
        color: Color,
        z_index: ZIndex,
    );

    // ========================================================================
    // Batched Drawing
    // ========================================================================

    /// Push multiple rects as a single batched draw call.
    ///
    /// Default implementation falls back to per-rect `push_rect` calls.
    /// SceneBuilder overrides this to emit a single PathMesh for all rects.
    fn push_rect_mesh(&mut self, rects: &[RectF], color: Color, z_index: ZIndex) {
        for r in rects {
            self.push_rect(*r, color, z_index, None);
        }
    }

    // ========================================================================
    // Text Drawing
    // ========================================================================

    /// Push text with background spans (for selection, caret, highlights).
    fn push_text_with_backgrounds(
        &mut self,
        run: &GlyphRun,
        x: f32,
        y: f32,
        color: Color,
        backgrounds: Vec<TextBackgroundSpan>,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    // ========================================================================
    // Image Drawing
    // ========================================================================

    /// SPEC-HAL-295: Push a textured image quad.
    ///
    /// Used for offscreen textures (Plot, custom render passes) and
    /// any pre-rendered image content. The texture must be registered
    /// via `WgpuRenderer::register_image_texture()` before use.
    fn push_image(
        &mut self,
        rect: RectF,
        texture_id: u64,
        uv_rect: RectF,
        tint: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// Preserve exact CPU admission identity through the existing Image path.
    fn push_admitted_image(
        &mut self,
        _rect: RectF,
        _binding: &crate::renderer::image_binding::ImageBinding,
        _uv_rect: RectF,
        _tint: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
        panic!("this PaintSink does not support admitted Image identities")
    }

    fn push_admitted_surface_fill(
        &mut self,
        rect: RectF,
        fill: &SurfaceFill<crate::renderer::image_binding::ImageBinding>,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) {
        match fill {
            SurfaceFill::Solid(color) => self.push_rect(rect, *color, z_index, key),
            SurfaceFill::Gradient(gradient) => {
                self.push_gradient_rect(rect, 0.0, gradient.definition(), z_index, key)
            }
            SurfaceFill::Image(image) => self.push_admitted_image(
                rect,
                &image.source,
                image.uv_rect,
                image.tint,
                z_index,
                key,
            ),
            SurfaceFill::NinePatch(material) => {
                let mut key = key;
                material.for_each_cell(rect, |destination, uv_rect| {
                    self.push_admitted_image(
                        destination,
                        &material.source,
                        uv_rect,
                        material.tint,
                        z_index,
                        key.take(),
                    );
                });
            }
        }
    }

    /// SPEC-HAL-266A: Push an admitted per-target Image identity. The actual
    /// registered texture is resolved later for the renderer-owned SurfaceId.
    fn push_target_bound_image(
        &mut self,
        _rect: RectF,
        _presentation_id: crate::renderer::CustomRenderPresentationId,
        _uv_rect: RectF,
        _tint: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
        panic!("SPEC-HAL-266A: this PaintSink does not support target-bound Image identities")
    }

    // ========================================================================
    // Line Drawing
    // ========================================================================

    /// Push a line segment from point `a` to point `b`.
    fn push_line(
        &mut self,
        a: crate::core::math::Point,
        b: crate::core::math::Point,
        width: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// Push a polyline (connected line segments) through the given points.
    fn push_polyline(
        &mut self,
        points: Vec<crate::core::math::Point>,
        width: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );

    /// SPEC-HAL-358: Push a checked luminous line/polyline contour.
    fn push_luminous_contour(
        &mut self,
        points: Arc<[Point]>,
        core_width: f32,
        color: Color,
        appearance: LuminousContourAppearance,
        fronts: LuminousContourFronts,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    ) -> Result<(), LuminousContourError>;

    // ========================================================================
    // Debug/Identity Hooks
    // ========================================================================

    /// Set current painting widget for debug tracking.
    fn set_current_painting_widget(&mut self, widget_id: u64);

    /// Clear current painting widget.
    fn clear_current_painting_widget(&mut self);

    // ========================================================================
    // Debug / Probe Drawing (debug-only)
    // ========================================================================

    /// Debug-only text probe for overlays, diagnostics, and instrumentation.
    ///
    /// This is the PaintSink equivalent of SceneBuilder::push_text_probe().
    /// Available only in debug builds. Use for FPS counters, debug HUDs, etc.
    #[cfg(debug_assertions)]
    fn push_text_probe(
        &mut self,
        position: (f32, f32),
        text: &str,
        font_size: f32,
        color: Color,
        z_index: ZIndex,
        key: Option<SceneNodeKey>,
    );
}

#[cfg(test)]
mod luminous_contour_tests {
    use super::*;

    fn front(distance: f32, half_extent: f32) -> LuminousContourFront {
        LuminousContourFront::try_new(distance, half_extent, 0.75).unwrap()
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!((actual - expected).abs() < 0.0001, "{actual} != {expected}");
    }

    #[test]
    fn luminous_contour_appearance_and_fronts_reject_out_of_range_values() {
        let compatible = LuminousContourAppearance::try_new(0.0, 0.0, 2.0).unwrap();
        assert_eq!(compatible.core_luminance_lift(), 0.0);
        assert_eq!(compatible.inner_aura_radius(), 0.0);
        assert_eq!(compatible.inner_aura_intensity(), 0.0);
        assert_eq!(compatible.front_opacity_boost().to_bits(), 0.0f32.to_bits());
        assert!(LuminousContourAppearance::try_new(8.0, 0.25, 8.0).is_ok());
        assert_eq!(
            LuminousContourAppearance::try_new(-0.1, 0.2, 2.0),
            Err(LuminousContourError::InvalidHaloRadius)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.1, 0.2, 2.0),
            Err(LuminousContourError::InvalidHaloRadius)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.0, f32::NAN, 2.0),
            Err(LuminousContourError::InvalidHaloIntensity)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.0, -0.1, 2.0),
            Err(LuminousContourError::InvalidHaloIntensity)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.0, 0.26, 2.0),
            Err(LuminousContourError::InvalidHaloIntensity)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.0, 0.2, 1.9),
            Err(LuminousContourError::InvalidHaloFalloff)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.0, 0.2, 8.1),
            Err(LuminousContourError::InvalidHaloFalloff)
        );
        assert_eq!(
            LuminousContourAppearance::try_new(8.0, 0.2, f32::INFINITY),
            Err(LuminousContourError::InvalidHaloFalloff)
        );

        assert!(LuminousContourFront::try_new(0.0, 2.0, 0.0).is_ok());
        assert!(LuminousContourFront::try_new(12.0, 32.0, 1.0).is_ok());
        assert_eq!(
            LuminousContourFront::try_new(-1.0, 4.0, 1.0),
            Err(LuminousContourError::InvalidFrontDistance)
        );
        assert_eq!(
            LuminousContourFront::try_new(f32::NAN, 4.0, 1.0),
            Err(LuminousContourError::InvalidFrontDistance)
        );
        assert_eq!(
            LuminousContourFront::try_new(1.0, 1.9, 1.0),
            Err(LuminousContourError::InvalidFrontHalfExtent)
        );
        assert_eq!(
            LuminousContourFront::try_new(1.0, 32.1, 1.0),
            Err(LuminousContourError::InvalidFrontHalfExtent)
        );
        assert_eq!(
            LuminousContourFront::try_new(1.0, 4.0, -0.1),
            Err(LuminousContourError::InvalidFrontIntensity)
        );
        assert_eq!(
            LuminousContourFront::try_new(1.0, 4.0, 1.1),
            Err(LuminousContourError::InvalidFrontIntensity)
        );
        assert_eq!(
            LuminousContourFront::try_new(1.0, 4.0, f32::INFINITY),
            Err(LuminousContourError::InvalidFrontIntensity)
        );

        let eight = [front(0.0, 2.0); 8];
        assert_eq!(
            LuminousContourFronts::try_from_slice(&eight).unwrap().len(),
            8
        );
        let nine = [front(0.0, 2.0); 9];
        assert_eq!(
            LuminousContourFronts::try_from_slice(&nine),
            Err(LuminousContourError::TooManyFronts)
        );
    }

    #[test]
    fn luminous_contour_front_opacity_boost_is_checked_and_immutable() {
        for original in [
            LuminousContourAppearance::try_new(4.0, 0.1, 2.0).unwrap(),
            LuminousContourAppearance::try_new_zoned(0.4, 1.5, 0.6, 5.0, 0.1, 4.0)
                .unwrap(),
        ] {
            assert_eq!(original.front_opacity_boost().to_bits(), 0.0f32.to_bits());
            for zero in [0.0, -0.0] {
                let unchanged = original.with_front_opacity_boost(zero).unwrap();
                assert_eq!(unchanged, original);
                assert_eq!(unchanged.front_opacity_boost().to_bits(), 0.0f32.to_bits());
            }
            for invalid in [-0.01, 1.01, f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
                assert_eq!(
                    original.with_front_opacity_boost(invalid),
                    Err(LuminousContourError::InvalidFrontOpacityBoost)
                );
                assert_eq!(original.front_opacity_boost().to_bits(), 0.0f32.to_bits());
            }
            for boost in [0.0, 0.5, 1.0] {
                let boosted = original.with_front_opacity_boost(boost).unwrap();
                assert_eq!(boosted.front_opacity_boost(), boost);
                assert_eq!(boosted.with_front_opacity_boost(0.0).unwrap(), original);
            }
        }
        assert_eq!(
            LuminousContourError::InvalidFrontOpacityBoost.to_string(),
            "front opacity boost must be finite and in 0..=1"
        );
    }

    #[test]
    fn luminous_contour_zoned_appearance_checks_each_region_and_zone_order() {
        let appearance =
            LuminousContourAppearance::try_new_zoned(0.75, 4.0, 0.75, 8.0, 0.25, 8.0).unwrap();
        assert_eq!(appearance.core_luminance_lift(), 0.75);
        assert_eq!(appearance.inner_aura_radius(), 4.0);
        assert_eq!(appearance.inner_aura_intensity(), 0.75);
        assert_eq!(appearance.halo_radius(), 8.0);

        assert_eq!(
            LuminousContourAppearance::try_new_zoned(f32::NAN, 1.0, 0.5, 2.0, 0.1, 3.0,),
            Err(LuminousContourError::InvalidCoreLuminanceLift)
        );
        assert_eq!(
            LuminousContourAppearance::try_new_zoned(-0.01, 1.0, 0.5, 2.0, 0.1, 3.0),
            Err(LuminousContourError::InvalidCoreLuminanceLift)
        );
        assert_eq!(
            LuminousContourAppearance::try_new_zoned(0.2, 4.1, 0.5, 5.0, 0.1, 3.0),
            Err(LuminousContourError::InvalidInnerAuraRadius)
        );
        assert_eq!(
            LuminousContourAppearance::try_new_zoned(0.2, f32::INFINITY, 0.5, 5.0, 0.1, 3.0,),
            Err(LuminousContourError::InvalidInnerAuraRadius)
        );
        assert_eq!(
            LuminousContourAppearance::try_new_zoned(0.2, 1.0, 0.76, 5.0, 0.1, 3.0),
            Err(LuminousContourError::InvalidInnerAuraIntensity)
        );
        assert_eq!(
            LuminousContourAppearance::try_new_zoned(0.2, 1.0, f32::NAN, 5.0, 0.1, 3.0,),
            Err(LuminousContourError::InvalidInnerAuraIntensity)
        );
        assert_eq!(
            LuminousContourAppearance::try_new_zoned(0.2, 3.0, 0.5, 2.0, 0.1, 3.0),
            Err(LuminousContourError::InvalidAuraRadiusOrder)
        );
    }

    #[test]
    fn luminous_contour_endpoint_front_shifts_inward_to_keep_its_length() {
        let points = [Point::new(0.0, 0.0), Point::new(20.0, 0.0)];
        let fronts = LuminousContourFronts::try_from_slice(&[front(20.0, 4.0)]).unwrap();
        let mut regions = Vec::new();
        let count =
            visit_luminous_contour_regions(&points, 20.0, &fronts, |region| regions.push(region));

        assert_eq!(count, 9);
        assert_eq!(regions[0].start, Point::new(0.0, 0.0));
        assert_eq!(regions[0].end, Point::new(12.0, 0.0));
        assert_eq!(regions.last().unwrap().end, Point::new(20.0, 0.0));
        assert_eq!(regions[0].start_ownership_normal, Point::new(0.0, 0.0));
        assert_eq!(
            regions.last().unwrap().end_ownership_normal,
            Point::new(0.0, 0.0)
        );
        assert_close(regions[4].end_gain, 0.75);
        assert_close(regions.last().unwrap().end_gain, 0.0);
        for pair in regions.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
            assert_close(pair[0].end_gain, pair[1].start_gain);
            assert_eq!(pair[0].end_ownership_normal, pair[1].start_ownership_normal);
        }
    }

    #[test]
    fn luminous_contour_corner_and_close_joints_have_one_shared_ownership_plane() {
        let points = [
            Point::new(0.0, 0.0),
            Point::new(10.0, 0.0),
            Point::new(10.0, 10.0),
            Point::new(0.0, 10.0),
            Point::new(0.0, 0.0),
        ];
        let mut regions = Vec::new();
        let count = visit_luminous_contour_regions(
            &points,
            40.0,
            &LuminousContourFronts::empty(),
            |region| regions.push(region),
        );

        assert_eq!(count, 4);
        for index in 0..regions.len() {
            let next = (index + 1) % regions.len();
            assert_eq!(
                regions[index].end_ownership_normal,
                regions[next].start_ownership_normal
            );
            assert_ne!(regions[index].end_ownership_normal, Point::new(0.0, 0.0));
        }
        let diagonal = 0.5_f32.sqrt();
        assert_close(regions[0].end_ownership_normal.x, diagonal);
        assert_close(regions[0].end_ownership_normal.y, diagonal);
    }

    #[test]
    fn luminous_contour_overlapping_fronts_combine_by_max_with_continuous_gain() {
        let points = [Point::new(0.0, 0.0), Point::new(20.0, 0.0)];
        let fronts = LuminousContourFronts::try_from_slice(&[
            LuminousContourFront::try_new(10.0, 4.0, 0.6).unwrap(),
            LuminousContourFront::try_new(10.0, 4.0, 0.8).unwrap(),
        ])
        .unwrap();
        let mut regions = Vec::new();
        visit_luminous_contour_regions(&points, 20.0, &fronts, |region| regions.push(region));

        let centre = regions
            .iter()
            .find(|region| region.end == Point::new(10.0, 0.0))
            .expect("front centre knot");
        assert_close(centre.end_gain, 0.8);
        for pair in regions.windows(2) {
            assert_eq!(pair[0].end, pair[1].start);
            assert_close(pair[0].end_gain, pair[1].start_gain);
        }
    }
}
