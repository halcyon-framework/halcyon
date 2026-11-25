// SPEC-HAL-296: Plot Transform
//
// Bidirectional mapping between world-space and screen-space.
// Created fresh each frame from ViewState. Never mutated directly.
// All coordinate math stays in f64 until the final f32 pixel cast.

use super::{axis::AxisKind, view_state::ViewState};

/// Ephemeral price ↔ vertical-pixel transform shared by Plot and aligned
/// market-detail projections.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PriceTransform {
    pub min: f64,
    pub max: f64,
    pub kind: AxisKind,
    pub height_px: f32,
}

impl PriceTransform {
    pub fn new(min: f64, max: f64, kind: AxisKind, height_px: f32) -> Self {
        Self {
            min,
            max,
            kind,
            height_px,
        }
    }

    #[inline]
    pub fn price_to_px(&self, price: f64) -> Option<f32> {
        let normalized = axis_to_normalized(price, self.min, self.max, self.kind)?;
        Some(((1.0 - normalized) * self.height_px as f64) as f32)
    }

    #[inline]
    pub fn px_to_price(&self, py: f32) -> Option<f64> {
        if self.height_px <= 0.0 || !py.is_finite() {
            return None;
        }
        normalized_to_axis(
            1.0 - (py / self.height_px) as f64,
            self.min,
            self.max,
            self.kind,
        )
    }

    pub(crate) fn price_to_axis_coordinate(&self, price: f64) -> Option<f64> {
        if !price.is_finite() {
            return None;
        }
        match self.kind {
            AxisKind::Time | AxisKind::Linear => Some(price),
            AxisKind::Log => (price > 0.0).then(|| price.log10()),
        }
    }

    pub(crate) fn axis_coordinate_to_price(&self, coordinate: f64) -> Option<f64> {
        if !coordinate.is_finite() {
            return None;
        }
        match self.kind {
            AxisKind::Time | AxisKind::Linear => Some(coordinate),
            AxisKind::Log => {
                let price = 10_f64.powf(coordinate);
                price.is_finite().then_some(price)
            }
        }
    }
}

/// SPEC-HAL-296: Ephemeral world ↔ screen transform.
///
/// INV-296-TR1: Normalizes relative distances, not absolute values.
/// INV-296-TR2: Created from ViewState each frame. Never stored across frames.
#[derive(Debug, Clone, PartialEq)]
pub struct Transform {
    pub x_min: f64,
    pub x_max: f64,
    pub y_min: f64,
    pub y_max: f64,
    pub x_kind: AxisKind,
    pub y_kind: AxisKind,
    pub width_px: f32,
    pub height_px: f32,
}

impl Transform {
    /// Create Transform from ViewState and viewport dimensions (physical pixels).
    pub fn from_view_state(vs: &ViewState, width_px: f32, height_px: f32) -> Self {
        Self::from_view_state_with_axes(vs, width_px, height_px, AxisKind::Linear, AxisKind::Linear)
    }

    /// Create Transform from ViewState, viewport dimensions, and axis kinds.
    pub fn from_view_state_with_axes(
        vs: &ViewState,
        width_px: f32,
        height_px: f32,
        x_kind: AxisKind,
        y_kind: AxisKind,
    ) -> Self {
        Self {
            x_min: vs.x_min,
            x_max: vs.x_max,
            y_min: vs.y_min,
            y_max: vs.y_max,
            x_kind,
            y_kind,
            width_px,
            height_px,
        }
    }

    /// World X → screen pixel. f64 math until final f32 cast.
    #[inline]
    pub fn x_to_px(&self, x: f64) -> f32 {
        self.x_to_px_checked(x).unwrap_or(f32::NAN)
    }

    /// World Y → screen pixel. Inverted (Y=0 at top). f64 math until f32.
    ///
    /// INV-296-TR1: (y - y_min) / (y_max - y_min) in f64, then * height, then f32.
    #[inline]
    pub fn y_to_px(&self, y: f64) -> f32 {
        self.y_to_px_checked(y).unwrap_or(f32::NAN)
    }

    /// Screen pixel → world X. Returns f64.
    #[inline]
    pub fn px_to_x(&self, px: f32) -> f64 {
        self.px_to_x_checked(px).unwrap_or(f64::NAN)
    }

    /// Screen pixel → world Y. Returns f64. Inverted (Y=0 at top).
    #[inline]
    pub fn px_to_y(&self, py: f32) -> f64 {
        self.px_to_y_checked(py).unwrap_or(f64::NAN)
    }

    /// World X → screen pixel, returning None for invalid log-domain values.
    #[inline]
    pub fn x_to_px_checked(&self, x: f64) -> Option<f32> {
        let normalized = axis_to_normalized(x, self.x_min, self.x_max, self.x_kind)?;
        Some((normalized * self.width_px as f64) as f32)
    }

    /// World Y → screen pixel, returning None for invalid log-domain values.
    #[inline]
    pub fn y_to_px_checked(&self, y: f64) -> Option<f32> {
        self.price_transform().price_to_px(y)
    }

    /// World point → screen pixel, returning None if either axis cannot map it.
    #[inline]
    pub fn point_to_px_checked(&self, x: f64, y: f64) -> Option<(f32, f32)> {
        Some((self.x_to_px_checked(x)?, self.y_to_px_checked(y)?))
    }

    /// Screen pixel → world X, returning None for invalid transform domains.
    #[inline]
    pub fn px_to_x_checked(&self, px: f32) -> Option<f64> {
        if self.width_px <= 0.0 || !px.is_finite() {
            return None;
        }
        normalized_to_axis(
            (px / self.width_px) as f64,
            self.x_min,
            self.x_max,
            self.x_kind,
        )
    }

    /// Screen pixel → world Y, returning None for invalid transform domains.
    #[inline]
    pub fn px_to_y_checked(&self, py: f32) -> Option<f64> {
        self.price_transform().px_to_price(py)
    }

    /// X-axis span in world units.
    #[inline]
    pub fn x_span(&self) -> f64 {
        self.x_max - self.x_min
    }

    /// Y-axis span in world units.
    #[inline]
    pub fn y_span(&self) -> f64 {
        self.y_max - self.y_min
    }

    #[inline]
    pub fn price_transform(&self) -> PriceTransform {
        PriceTransform::new(self.y_min, self.y_max, self.y_kind, self.height_px)
    }
}

#[inline]
fn axis_to_normalized(value: f64, min: f64, max: f64, kind: AxisKind) -> Option<f64> {
    if !value.is_finite() || !min.is_finite() || !max.is_finite() || max <= min {
        return None;
    }

    match kind {
        AxisKind::Time | AxisKind::Linear => Some((value - min) / (max - min)),
        AxisKind::Log => {
            if value <= 0.0 || min <= 0.0 || max <= 0.0 {
                return None;
            }
            let min_log = min.log10();
            let max_log = max.log10();
            let span = max_log - min_log;
            if span <= 0.0 || !span.is_finite() {
                return None;
            }
            Some((value.log10() - min_log) / span)
        }
    }
}

#[inline]
fn normalized_to_axis(normalized: f64, min: f64, max: f64, kind: AxisKind) -> Option<f64> {
    if !normalized.is_finite() || !min.is_finite() || !max.is_finite() || max <= min {
        return None;
    }

    match kind {
        AxisKind::Time | AxisKind::Linear => Some(min + normalized * (max - min)),
        AxisKind::Log => {
            if min <= 0.0 || max <= 0.0 {
                return None;
            }
            let min_log = min.log10();
            let max_log = max.log10();
            let span = max_log - min_log;
            if span <= 0.0 || !span.is_finite() {
                return None;
            }
            Some(10_f64.powf(min_log + normalized * span))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn log_y_transform_maps_decades_evenly() {
        let vs = ViewState::new(0.0, 1.0, 1.0, 100.0);
        let transform = Transform::from_view_state_with_axes(
            &vs,
            100.0,
            200.0,
            AxisKind::Linear,
            AxisKind::Log,
        );

        assert_eq!(transform.y_to_px_checked(100.0), Some(0.0));
        assert_eq!(transform.y_to_px_checked(10.0), Some(100.0));
        assert_eq!(transform.y_to_px_checked(1.0), Some(200.0));
        assert!((transform.px_to_y_checked(100.0).unwrap() - 10.0).abs() < 1e-10);
    }

    #[test]
    fn log_transform_rejects_non_positive_domain_values() {
        let vs = ViewState::new(1.0, 100.0, 1.0, 100.0);
        let transform =
            Transform::from_view_state_with_axes(&vs, 100.0, 100.0, AxisKind::Log, AxisKind::Log);

        assert_eq!(transform.x_to_px_checked(0.0), None);
        assert_eq!(transform.y_to_px_checked(-1.0), None);
    }
}
