//! Coordinate space conversion utilities.
//!
//! This module provides the canonical helpers for converting between logical and physical
//! coordinate spaces. All logical ↔ physical conversions should go through this module,
//! not hand-rolled `* scale` math.
//!
//! See SPEC-HAL-024 Layer C for the canonical coordinate space definitions.

use crate::core::math::{Point, RectF};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ScaleConversionError {
    InvalidPlatformScale,
    InvalidUiScale,
    InvalidEffectiveScale,
    InactiveTarget,
    InvalidLogicalExtent,
}

/// Compose the admitted App and platform factors once at their boundary.
pub(crate) fn effective_scale_factor(
    platform_scale: f32,
    ui_scale: f32,
) -> Result<f32, ScaleConversionError> {
    if !platform_scale.is_finite() || platform_scale <= 0.0 {
        return Err(ScaleConversionError::InvalidPlatformScale);
    }
    if !ui_scale.is_finite() || ui_scale <= 0.0 {
        return Err(ScaleConversionError::InvalidUiScale);
    }
    let effective_scale = platform_scale * ui_scale;
    if !effective_scale.is_finite() || effective_scale <= 0.0 {
        return Err(ScaleConversionError::InvalidEffectiveScale);
    }
    Ok(effective_scale)
}

/// Normalize the obtained physical client extent without rounding logical edges.
pub(crate) fn checked_logical_viewport(
    width: u32,
    height: u32,
    effective_scale: f32,
) -> Result<RectF, ScaleConversionError> {
    if width == 0 || height == 0 {
        return Err(ScaleConversionError::InactiveTarget);
    }
    if !effective_scale.is_finite() || effective_scale <= 0.0 {
        return Err(ScaleConversionError::InvalidEffectiveScale);
    }
    let viewport = physical_rect_to_logical(
        RectF::new(0.0, 0.0, width as f32, height as f32),
        DpiScale::uniform(effective_scale),
    );
    if !viewport.width.is_finite()
        || !viewport.height.is_finite()
        || viewport.width <= 0.0
        || viewport.height <= 0.0
    {
        return Err(ScaleConversionError::InvalidLogicalExtent);
    }
    Ok(viewport)
}

/// DPI scale factors for converting between logical and physical coordinates.
///
/// The scale factors represent the ratio of physical pixels to logical pixels:
/// - `physical = logical * dpi`
/// - `logical = physical / dpi`
///
/// Most displays have uniform scaling (x == y), but non-uniform scaling is supported
/// for edge cases.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct DpiScale {
    /// Horizontal scale factor (physical / logical)
    pub x: f32,
    /// Vertical scale factor (physical / logical)
    pub y: f32,
}

impl DpiScale {
    /// Create a new DPI scale with separate x and y factors.
    #[inline]
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }

    /// Create a uniform DPI scale (same factor for both axes).
    ///
    /// This is the common case for most displays.
    #[inline]
    pub const fn uniform(scale: f32) -> Self {
        Self { x: scale, y: scale }
    }
}

/// Convert a rectangle from logical to physical coordinates.
///
/// All components (x, y, width, height) are multiplied by the corresponding DPI scale factor.
/// No rounding is applied; the result remains in pure f32.
#[inline]
pub fn logical_rect_to_physical(rect: RectF, dpi: DpiScale) -> RectF {
    RectF::new(
        rect.x * dpi.x,
        rect.y * dpi.y,
        rect.width * dpi.x,
        rect.height * dpi.y,
    )
}

/// Convert a rectangle from physical to logical coordinates.
///
/// All components (x, y, width, height) are divided by the corresponding DPI scale factor.
/// No rounding is applied; the result remains in pure f32.
#[inline]
pub fn physical_rect_to_logical(rect: RectF, dpi: DpiScale) -> RectF {
    RectF::new(
        rect.x / dpi.x,
        rect.y / dpi.y,
        rect.width / dpi.x,
        rect.height / dpi.y,
    )
}

/// Convert a point from logical to physical coordinates.
///
/// Both x and y are multiplied by the corresponding DPI scale factor.
/// No rounding is applied; the result remains in pure f32.
#[inline]
pub fn logical_point_to_physical(point: Point, dpi: DpiScale) -> Point {
    Point::new(point.x * dpi.x, point.y * dpi.y)
}

/// Convert a point from physical to logical coordinates.
///
/// Both x and y are divided by the corresponding DPI scale factor.
/// No rounding is applied; the result remains in pure f32.
#[inline]
pub fn physical_point_to_logical(point: Point, dpi: DpiScale) -> Point {
    Point::new(point.x / dpi.x, point.y / dpi.y)
}

/// Convert physical (x, y) coordinates to a logical Point.
///
/// SPEC-HAL-024 Stage 5: This is the canonical helper for converting winit
/// physical pointer positions to absolute logical coordinates at the platform
/// boundary. All pointer coordinate conversion should go through this function.
///
/// # Arguments
/// - `x`, `y`: Physical coordinates (e.g., from winit `PhysicalPosition`)
/// - `scale_factor`: The admitted target physical/logical ratio
///
/// # Returns
/// A `Point` in absolute logical coordinates, suitable for `hit_test` and event routing.
#[inline]
pub fn physical_xy_to_logical_point(x: f64, y: f64, scale_factor: f64) -> Point {
    let dpi = DpiScale::uniform(scale_factor as f32);
    physical_point_to_logical(Point::new(x as f32, y as f32), dpi)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ui_scale_composes_once_and_pointer_conversion_inverts_output() {
        let effective_scale = effective_scale_factor(1.25, 1.6).unwrap();
        assert_eq!(effective_scale, 2.0);
        let logical = Point::new(24.0, 36.0);
        let physical = logical_point_to_physical(logical, DpiScale::uniform(effective_scale));
        assert_eq!(physical, Point::new(48.0, 72.0));
        assert_eq!(
            physical_xy_to_logical_point(
                physical.x.into(),
                physical.y.into(),
                effective_scale.into(),
            ),
            logical
        );
    }

    #[test]
    fn ui_scale_default_preserves_platform_viewport() {
        let effective_scale = effective_scale_factor(1.5, 1.0).unwrap();
        assert_eq!(effective_scale, 1.5);
        assert_eq!(
            checked_logical_viewport(1200, 900, effective_scale).unwrap(),
            physical_rect_to_logical(RectF::new(0.0, 0.0, 1200.0, 900.0), DpiScale::uniform(1.5),)
        );
    }

    #[test]
    fn ui_scale_viewport_keeps_fractional_extent() {
        let effective_scale = effective_scale_factor(1.25, 1.5).unwrap();
        let viewport = checked_logical_viewport(1001, 713, effective_scale).unwrap();
        assert_eq!(
            viewport,
            RectF::new(0.0, 0.0, 1001.0 / 1.875, 713.0 / 1.875)
        );
        assert_ne!(viewport.width.fract(), 0.0);
        assert_ne!(viewport.height.fract(), 0.0);
    }

    #[test]
    fn ui_scale_rejects_invalid_factors_and_unrepresentable_extents() {
        assert_eq!(
            effective_scale_factor(0.0, 1.0),
            Err(ScaleConversionError::InvalidPlatformScale)
        );
        assert_eq!(
            effective_scale_factor(1.0, f32::NAN),
            Err(ScaleConversionError::InvalidUiScale)
        );
        assert_eq!(
            effective_scale_factor(f32::MAX, 2.0),
            Err(ScaleConversionError::InvalidEffectiveScale)
        );
        assert_eq!(
            effective_scale_factor(f32::MIN_POSITIVE, f32::MIN_POSITIVE),
            Err(ScaleConversionError::InvalidEffectiveScale)
        );
        assert_eq!(
            checked_logical_viewport(0, 600, 1.0),
            Err(ScaleConversionError::InactiveTarget)
        );
        assert_eq!(
            checked_logical_viewport(800, 600, f32::INFINITY),
            Err(ScaleConversionError::InvalidEffectiveScale)
        );
        assert_eq!(
            checked_logical_viewport(800, 600, f32::from_bits(1)),
            Err(ScaleConversionError::InvalidLogicalExtent)
        );
    }

    #[test]
    fn test_dpi_scale_uniform() {
        let dpi = DpiScale::uniform(2.0);
        assert_eq!(dpi.x, 2.0);
        assert_eq!(dpi.y, 2.0);
    }

    #[test]
    fn test_dpi_scale_non_uniform() {
        let dpi = DpiScale::new(1.5, 2.0);
        assert_eq!(dpi.x, 1.5);
        assert_eq!(dpi.y, 2.0);
    }

    #[test]
    fn test_logical_physical_rect_roundtrip_uniform() {
        let original = RectF::new(10.0, 20.0, 30.0, 40.0);
        let dpi = DpiScale::uniform(2.0);

        // Logical to physical
        let physical = logical_rect_to_physical(original, dpi);
        assert_eq!(physical, RectF::new(20.0, 40.0, 60.0, 80.0));

        // Physical back to logical
        let logical = physical_rect_to_logical(physical, dpi);
        assert_eq!(logical, original);
    }

    #[test]
    fn test_logical_physical_rect_roundtrip_non_uniform() {
        let original = RectF::new(10.0, 20.0, 30.0, 40.0);
        let dpi = DpiScale::new(1.5, 2.0);

        // Logical to physical
        let physical = logical_rect_to_physical(original, dpi);
        assert_eq!(physical, RectF::new(15.0, 40.0, 45.0, 80.0));

        // Physical back to logical
        let logical = physical_rect_to_logical(physical, dpi);
        assert_eq!(logical, original);
    }

    #[test]
    fn test_logical_physical_point_roundtrip_uniform() {
        let original = Point::new(100.0, 200.0);
        let dpi = DpiScale::uniform(2.0);

        // Logical to physical
        let physical = logical_point_to_physical(original, dpi);
        assert_eq!(physical, Point::new(200.0, 400.0));

        // Physical back to logical
        let logical = physical_point_to_logical(physical, dpi);
        assert_eq!(logical, original);
    }

    #[test]
    fn test_logical_physical_point_roundtrip_non_uniform() {
        let original = Point::new(100.0, 200.0);
        let dpi = DpiScale::new(1.5, 2.0);

        // Logical to physical
        let physical = logical_point_to_physical(original, dpi);
        assert_eq!(physical, Point::new(150.0, 400.0));

        // Physical back to logical
        let logical = physical_point_to_logical(physical, dpi);
        assert_eq!(logical, original);
    }

    // ========================================================================
    // SPEC-HAL-024 Stage 5: Tests for physical_xy_to_logical_point
    // ========================================================================

    #[test]
    fn test_physical_xy_to_logical_point_identity() {
        // scale_factor 1.0 → identity
        let result = physical_xy_to_logical_point(100.0, 200.0, 1.0);
        assert_eq!(result, Point::new(100.0, 200.0));
    }

    #[test]
    fn test_physical_xy_to_logical_point_scale_2() {
        // scale_factor 2.0 → halves coords
        let result = physical_xy_to_logical_point(40.0, 60.0, 2.0);
        assert_eq!(result, Point::new(20.0, 30.0));
    }

    #[test]
    fn test_physical_xy_to_logical_point_fractional_scale() {
        // scale_factor 1.5 → divides by 1.5
        let result = physical_xy_to_logical_point(150.0, 300.0, 1.5);
        assert_eq!(result, Point::new(100.0, 200.0));
    }
}
