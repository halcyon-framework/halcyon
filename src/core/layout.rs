use crate::core::math::Size;
use crate::core::paint::Color;

// ============================================================================
// SPEC-HAL-086 Stage 2: Viewport-Aware Layout Protocol
// ============================================================================

/// Result of widget layout computation.
///
/// SPEC-HAL-086 Stage 2: Extends layout protocol to report both the rendered
/// viewport size and the natural content size (which may overflow).
///
/// SPEC-HAL-086 Stage 5: Adds viewport_size and scroll_offset for scroll containers.
/// These enable the layout engine to apply scroll offsets during position computation.
///
/// - `size`: The final rendered size of this widget (respects constraints)
/// - `content_size`: The natural/unconstrained content size (may exceed `size`)
/// - `viewport_size`: For scroll containers, the visible viewport size (None for non-scroll)
/// - `scroll_offset`: For scroll containers, current visual scroll offset (None for non-scroll)
///
/// For non-scrollable widgets, `content_size == size` and scroll fields are None.
/// For scrollable widgets, `content_size` may exceed `size`, and scroll fields are set.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LayoutResult {
    /// Final rendered size of the widget (constrained to parent bounds)
    pub size: Size,
    /// Natural content size (may overflow the rendered size)
    pub content_size: Size,
    /// SPEC-HAL-086 Stage 5: Viewport size for scroll containers (None for non-scroll widgets)
    pub viewport_size: Option<Size>,
    /// SPEC-HAL-086 Stage 5: Current visual scroll offset (None for non-scroll widgets)
    pub scroll_offset: Option<(f32, f32)>,
}

impl LayoutResult {
    /// Create a new LayoutResult with explicit size and content_size.
    /// For non-scroll widgets (viewport_size and scroll_offset are None).
    pub const fn new(size: Size, content_size: Size) -> Self {
        Self {
            size,
            content_size,
            viewport_size: None,
            scroll_offset: None,
        }
    }

    /// SPEC-HAL-086 Stage 5: Create a LayoutResult for a scroll container.
    /// Reports viewport size, content size, and current scroll offset.
    pub const fn scroll(
        viewport_size: Size,
        content_size: Size,
        scroll_offset: (f32, f32),
    ) -> Self {
        Self {
            size: viewport_size,
            content_size,
            viewport_size: Some(viewport_size),
            scroll_offset: Some(scroll_offset),
        }
    }

    /// Create a LayoutResult where content fits exactly (no overflow).
    /// Used by most non-scrollable widgets.
    pub const fn fit(size: Size) -> Self {
        Self {
            size,
            content_size: size,
            viewport_size: None,
            scroll_offset: None,
        }
    }

    /// Create a zero-sized LayoutResult.
    pub const fn zero() -> Self {
        Self {
            size: Size::zero(),
            content_size: Size::zero(),
            viewport_size: None,
            scroll_offset: None,
        }
    }
}

impl Default for LayoutResult {
    fn default() -> Self {
        Self::zero()
    }
}

impl From<Size> for LayoutResult {
    /// Convert a Size to LayoutResult with content_size == size.
    /// Provides backward compatibility for widgets that don't overflow.
    fn from(size: Size) -> Self {
        Self {
            size,
            content_size: size,
            viewport_size: None,
            scroll_offset: None,
        }
    }
}

/// Layout constraints passed down the widget tree
/// Defines min/max bounds for width and height
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Constraints {
    pub min_width: f32,
    pub max_width: f32,
    pub min_height: f32,
    pub max_height: f32,
}

impl Constraints {
    pub fn new(min_width: f32, max_width: f32, min_height: f32, max_height: f32) -> Self {
        Self {
            min_width,
            max_width,
            min_height,
            max_height,
        }
    }

    /// Tight constraints - forces exact size
    pub fn tight(size: Size) -> Self {
        Self {
            min_width: size.width,
            max_width: size.width,
            min_height: size.height,
            max_height: size.height,
        }
    }

    /// Loose constraints - allows any size up to max
    pub fn loose(max_size: Size) -> Self {
        Self {
            min_width: 0.0,
            max_width: max_size.width,
            min_height: 0.0,
            max_height: max_size.height,
        }
    }

    /// Unbounded constraints
    pub fn unbounded() -> Self {
        Self {
            min_width: 0.0,
            max_width: f32::INFINITY,
            min_height: 0.0,
            max_height: f32::INFINITY,
        }
    }

    /// Constrain a size to fit within these constraints
    pub fn constrain(&self, size: Size) -> Size {
        Size {
            width: size.width.clamp(self.min_width, self.max_width),
            height: size.height.clamp(self.min_height, self.max_height),
        }
    }

    /// Check if size satisfies constraints
    pub fn satisfies(&self, size: Size) -> bool {
        size.width >= self.min_width
            && size.width <= self.max_width
            && size.height >= self.min_height
            && size.height <= self.max_height
    }

    /// Tighten constraints with additional bounds
    pub fn tighten(
        &self,
        min_width: Option<f32>,
        max_width: Option<f32>,
        min_height: Option<f32>,
        max_height: Option<f32>,
    ) -> Self {
        Self {
            min_width: min_width.unwrap_or(self.min_width).max(self.min_width),
            max_width: max_width.unwrap_or(self.max_width).min(self.max_width),
            min_height: min_height.unwrap_or(self.min_height).max(self.min_height),
            max_height: max_height.unwrap_or(self.max_height).min(self.max_height),
        }
    }

    /// Loosen width constraints
    pub fn loosen_width(&self) -> Self {
        Self {
            min_width: 0.0,
            max_width: self.max_width,
            min_height: self.min_height,
            max_height: self.max_height,
        }
    }

    /// Loosen height constraints
    pub fn loosen_height(&self) -> Self {
        Self {
            min_width: self.min_width,
            max_width: self.max_width,
            min_height: 0.0,
            max_height: self.max_height,
        }
    }
}

/// Flexbox-style alignment
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Alignment {
    Start,
    Center,
    End,
    Stretch,
}

/// Flexbox-style justification (main axis distribution)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Justification {
    Start,
    Center,
    End,
    SpaceBetween,
    SpaceAround,
    SpaceEvenly,
}

/// Flex properties for a widget
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct FlexParams {
    pub grow: f32,
    pub shrink: f32,
    pub basis: f32,
}

impl FlexParams {
    pub fn new(grow: f32, shrink: f32, basis: f32) -> Self {
        Self {
            grow,
            shrink,
            basis,
        }
    }

    pub fn fixed(size: f32) -> Self {
        Self {
            grow: 0.0,
            shrink: 0.0,
            basis: size,
        }
    }

    pub fn flexible(grow: f32) -> Self {
        Self {
            grow,
            shrink: 1.0,
            basis: 0.0,
        }
    }
}

impl Default for FlexParams {
    fn default() -> Self {
        Self {
            grow: 0.0,
            shrink: 1.0,
            basis: 0.0,
        }
    }
}

/// Axis for flex layout
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Flex wrapping mode
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlexWrap {
    NoWrap,
    Wrap,
}

/// Spacing/padding values
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct EdgeInsets {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl EdgeInsets {
    pub const fn new(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    pub const fn all(value: f32) -> Self {
        Self {
            top: value,
            right: value,
            bottom: value,
            left: value,
        }
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            top: vertical,
            right: horizontal,
            bottom: vertical,
            left: horizontal,
        }
    }

    pub const fn zero() -> Self {
        Self {
            top: 0.0,
            right: 0.0,
            bottom: 0.0,
            left: 0.0,
        }
    }

    pub const fn horizontal(&self) -> f32 {
        self.left + self.right
    }

    pub const fn vertical(&self) -> f32 {
        self.top + self.bottom
    }
}

// ============================================================================
// Box Model Types (SPEC-HAL-010)
// ============================================================================

/// Margin (external spacing around widget)
pub type Margins = EdgeInsets;

/// Padding (internal spacing around content)
pub type Padding = EdgeInsets;

/// Border configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Borders {
    pub width: EdgeInsets,
    pub color: Color,
}

impl Borders {
    pub const fn new(width: f32, color: Color) -> Self {
        Self {
            width: EdgeInsets::all(width),
            color,
        }
    }

    pub const fn none() -> Self {
        Self {
            width: EdgeInsets::zero(),
            color: Color::TRANSPARENT,
        }
    }

    pub const fn uniform(width: f32, color: Color) -> Self {
        Self {
            width: EdgeInsets::all(width),
            color,
        }
    }
}

impl Default for Borders {
    fn default() -> Self {
        Self::none()
    }
}

/// Corner radius for rounded rectangles
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CornerRadius {
    pub top_left: f32,
    pub top_right: f32,
    pub bottom_right: f32,
    pub bottom_left: f32,
}

impl CornerRadius {
    pub const fn new(top_left: f32, top_right: f32, bottom_right: f32, bottom_left: f32) -> Self {
        Self {
            top_left,
            top_right,
            bottom_right,
            bottom_left,
        }
    }

    pub const fn all(radius: f32) -> Self {
        Self {
            top_left: radius,
            top_right: radius,
            bottom_right: radius,
            bottom_left: radius,
        }
    }

    pub const fn zero() -> Self {
        Self::all(0.0)
    }

    /// Get uniform radius (for simple cases)
    pub fn uniform(&self) -> f32 {
        self.top_left
    }
}

impl Default for CornerRadius {
    fn default() -> Self {
        Self::zero()
    }
}

/// Shadow configuration
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Shadow {
    pub offset_x: f32,
    pub offset_y: f32,
    pub blur_radius: f32,
    pub spread_radius: f32,
    pub color: Color,
}

impl Shadow {
    pub const fn new(
        offset_x: f32,
        offset_y: f32,
        blur_radius: f32,
        spread_radius: f32,
        color: Color,
    ) -> Self {
        Self {
            offset_x,
            offset_y,
            blur_radius,
            spread_radius,
            color,
        }
    }

    pub const fn none() -> Self {
        Self {
            offset_x: 0.0,
            offset_y: 0.0,
            blur_radius: 0.0,
            spread_radius: 0.0,
            color: Color::TRANSPARENT,
        }
    }
}

impl Default for Shadow {
    fn default() -> Self {
        Self::none()
    }
}

/// Box model computation result
#[derive(Debug, Clone, Copy)]
pub struct BoxModelResult {
    /// Final widget size including all box model components
    pub size: Size,
    /// Content origin relative to widget origin (logical coordinates)
    pub content_offset: (f32, f32),
}

/// Compute box model dimensions and offsets
///
/// Returns the final widget size and the content offset within the widget.
///
/// Layout model:
/// - content_offset = (margin.left + border.left + padding.left, margin.top + border.top + padding.top)
/// - final_size = margin + border + padding + content + padding + border + margin
pub fn compute_box_model(
    content_size: Size,
    margin: Margins,
    border: Borders,
    padding: Padding,
) -> BoxModelResult {
    // Content offset from widget origin
    let content_x = margin.left + border.width.left + padding.left;
    let content_y = margin.top + border.width.top + padding.top;

    // Total horizontal space
    let total_horizontal = margin.horizontal() + border.width.horizontal() + padding.horizontal();
    // Total vertical space
    let total_vertical = margin.vertical() + border.width.vertical() + padding.vertical();

    BoxModelResult {
        size: Size::new(
            content_size.width + total_horizontal,
            content_size.height + total_vertical,
        ),
        content_offset: (content_x, content_y),
    }
}

/// Reduce constraints by box model insets
///
/// Used to compute available space for child content.
pub fn shrink_constraints(
    constraints: Constraints,
    margin: Margins,
    border: Borders,
    padding: Padding,
) -> Constraints {
    let total_horizontal = margin.horizontal() + border.width.horizontal() + padding.horizontal();
    let total_vertical = margin.vertical() + border.width.vertical() + padding.vertical();

    Constraints::new(
        (constraints.min_width - total_horizontal).max(0.0),
        (constraints.max_width - total_horizontal).max(0.0),
        (constraints.min_height - total_vertical).max(0.0),
        (constraints.max_height - total_vertical).max(0.0),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_constraints_constrain() {
        let c = Constraints::new(50.0, 200.0, 50.0, 200.0);
        assert_eq!(
            c.constrain(Size::new(100.0, 100.0)),
            Size::new(100.0, 100.0)
        );
        assert_eq!(c.constrain(Size::new(10.0, 10.0)), Size::new(50.0, 50.0));
        assert_eq!(
            c.constrain(Size::new(300.0, 300.0)),
            Size::new(200.0, 200.0)
        );
    }
}

/// Natural, owner-measured space for one optional side part.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPartRequirement {
    pub width: f32,
    pub gap: f32,
}

/// Explicit order of the two optional parts to the right of an aligned body.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ControlTrailingOrder {
    /// Existing default: body, label, value.
    #[default]
    LabelThenValue,
    /// Opt-in: body, value, label.
    ValueThenLabel,
}

/// Owned parts on one side; the layout owner chooses their slot order.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ControlSideRequirements {
    pub label: Option<ControlPartRequirement>,
    pub value: Option<ControlPartRequirement>,
}

impl ControlSideRequirements {
    pub fn width(self) -> f32 {
        [self.label, self.value]
            .into_iter()
            .flatten()
            .map(|p| p.width + p.gap)
            .sum()
    }

    pub(crate) fn include(&mut self, other: Self) {
        fn merge(a: &mut Option<ControlPartRequirement>, b: Option<ControlPartRequirement>) {
            if let Some(b) = b {
                *a = Some(a.map_or(b, |a| ControlPartRequirement {
                    width: a.width.max(b.width),
                    gap: a.gap.max(b.gap),
                }));
            }
        }
        merge(&mut self.label, other.label);
        merge(&mut self.value, other.value);
    }
}

/// Value-only natural requirements. Text, editors and input stay with the control.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPartsRequirements {
    pub leading: ControlSideRequirements,
    pub trailing: ControlSideRequirements,
    pub body_width: f32,
    pub height: f32,
}

impl ControlPartsRequirements {
    pub(crate) fn validate(self) {
        for n in [self.body_width, self.height].into_iter().chain(
            [
                self.leading.label,
                self.leading.value,
                self.trailing.label,
                self.trailing.value,
            ]
            .into_iter()
            .flatten()
            .flat_map(|p| [p.width, p.gap]),
        ) {
            assert!(
                n.is_finite() && n >= 0.0,
                "control alignment requires finite nonnegative metrics"
            );
        }
    }
}

/// Horizontal logical slot; no absolute position or device scale is carried.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct ControlPartInterval {
    pub start: f32,
    pub width: f32,
}

/// Final local columns, shared by the explicitly participating rows only.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ControlPartColumns {
    pub body: ControlPartInterval,
    pub leading_label: ControlPartInterval,
    pub leading_value: ControlPartInterval,
    pub trailing_label: ControlPartInterval,
    pub trailing_value: ControlPartInterval,
}

impl ControlPartColumns {
    pub(crate) fn new(
        width: f32,
        leading: ControlSideRequirements,
        trailing: ControlSideRequirements,
    ) -> Self {
        Self::with_trailing_order(width, leading, trailing, ControlTrailingOrder::default())
    }

    pub(crate) fn with_trailing_order(
        width: f32,
        leading: ControlSideRequirements,
        trailing: ControlSideRequirements,
        order: ControlTrailingOrder,
    ) -> Self {
        let mut x = 0.0;
        let mut next = |p: Option<ControlPartRequirement>, before: bool| {
            let p = p.unwrap_or(ControlPartRequirement {
                width: 0.0,
                gap: 0.0,
            });
            if before {
                x += p.gap;
            }
            let slot = ControlPartInterval {
                start: x,
                width: p.width,
            };
            x += p.width;
            if !before {
                x += p.gap;
            }
            slot
        };
        let leading_value = next(leading.value, false);
        let leading_label = next(leading.label, false);
        let body = ControlPartInterval {
            start: x,
            width: (width - leading.width() - trailing.width()).max(0.0),
        };
        x += body.width;
        let mut next = |p: Option<ControlPartRequirement>| {
            let p = p.unwrap_or(ControlPartRequirement {
                width: 0.0,
                gap: 0.0,
            });
            x += p.gap;
            let slot = ControlPartInterval {
                start: x,
                width: p.width,
            };
            x += p.width;
            slot
        };
        let (trailing_label, trailing_value) = match order {
            ControlTrailingOrder::LabelThenValue => {
                let label = next(trailing.label);
                (label, next(trailing.value))
            }
            ControlTrailingOrder::ValueThenLabel => {
                let value = next(trailing.value);
                (next(trailing.label), value)
            }
        };
        Self {
            body,
            leading_label,
            leading_value,
            trailing_label,
            trailing_value,
        }
    }

    pub(crate) fn content_width(self) -> f32 {
        [
            self.body,
            self.leading_label,
            self.leading_value,
            self.trailing_label,
            self.trailing_value,
        ]
        .into_iter()
        .map(|slot| slot.start + slot.width)
        .fold(0.0, f32::max)
    }
}

/// Scoped opt-in request for the current Layout owner; never inherited implicitly.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ControlPartsRequest {
    Measure,
    Arrange(ControlPartColumns),
}

#[derive(Debug, Default)]
pub(crate) struct ControlPartsLayoutCache {
    pub owner: Option<crate::utils::id::WidgetInternalId>,
    pub natural: Option<ControlPartsRequirements>,
    pub ordinary_natural: Option<LayoutResult>,
    pub arranged: Option<(Constraints, ControlPartColumns, LayoutResult)>,
}

/// Layout-published geometry of the parts still owned by one control.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ResolvedControlParts {
    pub body: crate::core::math::RectF,
    pub label: Option<crate::core::math::RectF>,
    pub value: Option<crate::core::math::RectF>,
}

impl ControlPartColumns {
    pub(crate) fn resolve(
        self,
        req: ControlPartsRequirements,
        height: f32,
    ) -> ResolvedControlParts {
        use crate::core::math::RectF;
        let label = req
            .leading
            .label
            .map(|p| {
                RectF::new(
                    self.leading_label.start + self.leading_label.width - p.width,
                    0.0,
                    p.width,
                    height,
                )
            })
            .or_else(|| {
                req.trailing
                    .label
                    .map(|p| RectF::new(self.trailing_label.start, 0.0, p.width, height))
            });
        let value = req
            .leading
            .value
            .map(|p| RectF::new(self.leading_value.start, 0.0, p.width, height))
            .or_else(|| {
                req.trailing.value.map(|p| {
                    RectF::new(
                        self.trailing_value.start + self.trailing_value.width - p.width,
                        0.0,
                        p.width,
                        height,
                    )
                })
            });
        ResolvedControlParts {
            body: RectF::new(self.body.start, 0.0, self.body.width, height),
            label,
            value,
        }
    }
}
