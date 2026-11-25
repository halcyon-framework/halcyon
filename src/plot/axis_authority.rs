// SPEC-HAL-296: AxisAuthority — Shared Axis Synchronization
//
// Pure data shared via Arc<Mutex<>>. Multiple Plots reading the same
// AxisAuthority stay synchronized on that axis. Either axis (X or Y)
// can be shared independently.

/// SPEC-HAL-296: Shared axis state for Plot synchronization.
///
/// INV-296-AA1: Pure data. No widget references. No callbacks. Polls only.
#[derive(Debug, Clone)]
pub struct AxisAuthority {
    min: f64,
    max: f64,
    generation: u64,
}

impl AxisAuthority {
    /// Create a new AxisAuthority with the given range.
    pub fn new(min: f64, max: f64) -> Self {
        Self {
            min,
            max,
            generation: 0,
        }
    }

    /// Get the current range.
    #[inline]
    pub fn range(&self) -> (f64, f64) {
        (self.min, self.max)
    }

    /// Get the generation counter (incremented on every mutation).
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Get the span (max - min).
    #[inline]
    pub fn span(&self) -> f64 {
        self.max - self.min
    }

    /// Set the range explicitly.
    pub fn set_range(&mut self, min: f64, max: f64) {
        if self.min == min && self.max == max {
            return;
        }
        self.min = min;
        self.max = max;
        self.generation += 1;
    }

    /// Pan by a delta value.
    pub fn pan(&mut self, delta: f64) {
        self.min += delta;
        self.max += delta;
        self.generation += 1;
    }

    /// Zoom around a center point. factor > 1.0 zooms in.
    pub fn zoom(&mut self, factor: f64, center: f64) {
        let span = self.span() / factor;
        let ratio = (center - self.min) / self.span();
        self.min = center - span * ratio;
        self.max = self.min + span;
        self.generation += 1;
    }
}
