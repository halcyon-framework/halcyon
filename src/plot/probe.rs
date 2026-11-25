// SPEC-HAL-296: Probe — Shared Coordinate Cursor
//
// Cross-widget coordinate probing. When a user hovers over one Plot,
// all other Plots and widgets reading the same Probe can react.
// Pure data, shared via Arc<Mutex<>>.

use crate::utils::id::WidgetInternalId;

/// SPEC-HAL-296: Shared coordinate probe for cross-widget hover feedback.
///
/// INV-296-PB1: Pure data. No widget references beyond source ID.
/// INV-296-PB2: Source widget skips rendering its own probe feedback.
/// INV-296-PB4: Coordinates are in shared world space.
#[derive(Debug, Clone)]
pub struct Probe {
    /// World X coordinate. None = no X probe active.
    pub x: Option<f64>,
    /// World Y coordinate. None = no Y probe active.
    pub y: Option<f64>,
    /// Which widget wrote the probe (for self-suppression).
    pub source: Option<WidgetInternalId>,
    /// Generation counter for change detection.
    pub generation: u64,
}

impl Probe {
    /// Create an inactive probe.
    pub fn new() -> Self {
        Self {
            x: None,
            y: None,
            source: None,
            generation: 0,
        }
    }

    /// Set probe coordinates and source.
    pub fn set(&mut self, x: Option<f64>, y: Option<f64>, source: WidgetInternalId) {
        if self.x == x && self.y == y && self.source == Some(source) {
            return;
        }
        self.x = x;
        self.y = y;
        self.source = Some(source);
        self.generation += 1;
    }

    /// Clear the probe (no active hover).
    pub fn clear(&mut self) {
        if self.x.is_none() && self.y.is_none() && self.source.is_none() {
            return;
        }
        self.x = None;
        self.y = None;
        self.source = None;
        self.generation += 1;
    }

    /// Get the generation counter.
    #[inline]
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// Check if probe is active (has at least one coordinate).
    #[inline]
    pub fn is_active(&self) -> bool {
        self.x.is_some() || self.y.is_some()
    }

    /// Check if this probe was set by the given widget (for self-suppression).
    #[inline]
    pub fn is_from(&self, id: WidgetInternalId) -> bool {
        self.source == Some(id)
    }
}

impl Default for Probe {
    fn default() -> Self {
        Self::new()
    }
}
