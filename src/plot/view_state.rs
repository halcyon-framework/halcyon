// SPEC-HAL-296: Plot ViewState
//
// Viewport state in world-space coordinates. The single source of truth
// for what a Plot displays. All values are f64 — no f32 until after Transform.

/// SPEC-HAL-296: Viewport state in world-space coordinates.
///
/// INV-296-VS1: All coordinate values are f64. No f32 until after Transform.
/// INV-296-VS2: Interaction modifies ViewState only. Transform is derived.
#[derive(Debug, Clone, PartialEq)]
pub struct ViewState {
    /// X-axis minimum (time, quantity, or arbitrary f64).
    pub x_min: f64,
    /// X-axis maximum.
    pub x_max: f64,
    /// Y-axis minimum (price, volume, indicator value, or arbitrary f64).
    pub y_min: f64,
    /// Y-axis maximum.
    pub y_max: f64,
    /// Auto-follow mode (scroll X to track latest data point).
    pub follow_latest: bool,
}

impl ViewState {
    /// Create a new ViewState with the given coordinate ranges.
    pub fn new(x_min: f64, x_max: f64, y_min: f64, y_max: f64) -> Self {
        Self {
            x_min,
            x_max,
            y_min,
            y_max,
            follow_latest: false,
        }
    }

    /// X-axis span.
    #[inline]
    pub fn x_span(&self) -> f64 {
        self.x_max - self.x_min
    }

    /// Y-axis span.
    #[inline]
    pub fn y_span(&self) -> f64 {
        self.y_max - self.y_min
    }

    /// Pan both axes by the given deltas.
    pub fn pan(&mut self, delta_x: f64, delta_y: f64) {
        self.x_min += delta_x;
        self.x_max += delta_x;
        self.y_min += delta_y;
        self.y_max += delta_y;
    }

    /// Pan X axis only.
    pub fn pan_x(&mut self, delta: f64) {
        self.x_min += delta;
        self.x_max += delta;
    }

    /// Pan Y axis only.
    pub fn pan_y(&mut self, delta: f64) {
        self.y_min += delta;
        self.y_max += delta;
    }

    /// Zoom both axes around a center point.
    ///
    /// factor > 1.0 zooms in (smaller range), factor < 1.0 zooms out.
    pub fn zoom(&mut self, factor: f64, center_x: f64, center_y: f64) {
        self.zoom_x(factor, center_x);
        self.zoom_y(factor, center_y);
    }

    /// Zoom X axis around a center point.
    pub fn zoom_x(&mut self, factor: f64, center: f64) {
        let span = self.x_span() / factor;
        let ratio = (center - self.x_min) / self.x_span();
        self.x_min = center - span * ratio;
        self.x_max = self.x_min + span;
    }

    /// Zoom Y axis around a center point.
    ///
    /// INV-296-PR3: Padding is proportional to span — never fixed-value.
    pub fn zoom_y(&mut self, factor: f64, center: f64) {
        let span = self.y_span() / factor;
        let ratio = (center - self.y_min) / self.y_span();
        self.y_min = center - span * ratio;
        self.y_max = self.y_min + span;
    }

    /// Apply proportional padding to Y axis.
    ///
    /// INV-296-PR3: Padding is proportional to span.
    pub fn apply_y_padding(&mut self, fraction: f64) {
        let span = self.y_span();
        self.y_min -= span * fraction;
        self.y_max += span * fraction;
    }

    /// Fit Y axis to a data range with proportional padding.
    ///
    /// Generic auto-scale: caller provides the data's min/max,
    /// ViewState sets y_min/y_max accordingly. If the range is
    /// degenerate (all values identical), creates a small synthetic range.
    ///
    /// INV-296-PR3: Padding is proportional to span.
    pub fn fit_y_to_range(&mut self, data_min: f64, data_max: f64, padding_fraction: f64) {
        if (data_max - data_min).abs() < 1e-15 {
            // Degenerate range — create synthetic span
            let mid = (data_max + data_min) / 2.0;
            let epsilon = if mid.abs() > 1e-15 {
                mid.abs() * 0.01
            } else {
                1.0
            };
            self.y_min = mid - epsilon;
            self.y_max = mid + epsilon;
        } else {
            self.y_min = data_min;
            self.y_max = data_max;
        }
        self.apply_y_padding(padding_fraction);
    }

    /// Fit X axis to a data range with proportional padding.
    ///
    /// Same logic as fit_y_to_range but for the X axis.
    pub fn fit_x_to_range(&mut self, data_min: f64, data_max: f64, padding_fraction: f64) {
        if (data_max - data_min).abs() < 1e-15 {
            let mid = (data_max + data_min) / 2.0;
            let epsilon = if mid.abs() > 1e-15 {
                mid.abs() * 0.01
            } else {
                1.0
            };
            self.x_min = mid - epsilon;
            self.x_max = mid + epsilon;
        } else {
            self.x_min = data_min;
            self.x_max = data_max;
        }
        self.apply_x_padding(padding_fraction);
    }

    /// Apply proportional padding to X axis.
    pub fn apply_x_padding(&mut self, fraction: f64) {
        let span = self.x_span();
        self.x_min -= span * fraction;
        self.x_max += span * fraction;
    }
}
