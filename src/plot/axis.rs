// SPEC-HAL-296: Axis Configuration
//
// Axis type and display configuration for Plot dimensions.

/// Axis coordinate interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AxisKind {
    /// Continuous time axis. Values are f64 microseconds since UNIX epoch.
    /// Labels format as datetime.
    Time,
    /// Linear numeric axis. Values are f64.
    /// Labels format as numbers.
    Linear,
    /// Logarithmic numeric axis. Values are f64 (stored linear,
    /// displayed logarithmic). Future extension.
    Log,
}

/// How an axis auto-scales in response to visible data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ScalePolicy {
    /// Auto-fit range to visible data each frame (default for price axes).
    /// After pan/zoom on the OTHER axis, this axis rescales to fit
    /// whatever data is currently visible.
    #[default]
    AutoFit,

    /// Fixed range — never changes. Use for axes with known bounds
    /// (RSI 0–100, percentage 0–100, normalized 0–1).
    Fixed,

    /// User-controlled range. Does not auto-scale, but can be panned
    /// and zoomed manually. Use when the user explicitly sets a range
    /// and doesn't want it to jump around.
    Manual,
}

/// Context provided to adaptive axis formatters.
/// Enables formatting decisions based on visible range
/// and tick density, not just the value itself.
#[derive(Debug, Clone, Copy)]
pub struct AxisFormatContext {
    /// The value being formatted.
    pub value: f64,
    /// Visible range minimum on this axis.
    pub range_min: f64,
    /// Visible range maximum on this axis.
    pub range_max: f64,
    /// Distance between adjacent ticks.
    pub tick_interval: f64,
}

/// Axis configuration for one dimension of a Plot.
pub struct Axis {
    /// Coordinate interpretation and formatting.
    pub kind: AxisKind,
    /// How this axis auto-scales in response to visible data.
    pub scale_policy: ScalePolicy,
    /// Label formatting callback (context → display string).
    /// Consumer provides domain-specific formatting via with_formatter().
    pub formatter: Option<Box<dyn Fn(AxisFormatContext) -> String>>,
    /// Whether axis labels are visible.
    pub show_labels: bool,
    /// Whether gridlines are drawn for this axis.
    pub show_grid: bool,
}

impl Axis {
    /// Create a new axis with the given kind. Labels and grid on by default.
    pub fn new(kind: AxisKind) -> Self {
        Self {
            kind,
            scale_policy: ScalePolicy::default(),
            formatter: None,
            show_labels: true,
            show_grid: true,
        }
    }

    /// Convenience: create a time axis.
    pub fn time() -> Self {
        Self::new(AxisKind::Time)
    }

    /// Convenience: create a linear axis.
    pub fn linear() -> Self {
        Self::new(AxisKind::Linear)
    }

    /// Set a custom label formatter.
    pub fn with_formatter(mut self, f: impl Fn(AxisFormatContext) -> String + 'static) -> Self {
        self.formatter = Some(Box::new(f));
        self
    }

    /// Show or hide axis labels.
    pub fn with_labels(mut self, show: bool) -> Self {
        self.show_labels = show;
        self
    }

    /// Show or hide gridlines.
    pub fn with_grid(mut self, show: bool) -> Self {
        self.show_grid = show;
        self
    }

    /// Set the scale policy for this axis.
    pub fn with_scale_policy(mut self, policy: ScalePolicy) -> Self {
        self.scale_policy = policy;
        self
    }
}
