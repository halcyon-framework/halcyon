//! SPEC-HAL-201: ComboBox Widget (Resolver-Driven, Zero Semantics)
//!
//! A resolver-driven selection control that uses `paint_overlay` + `OverlayHitRegion`
//! for its popup. All matching, filtering, ranking, and commit policy logic lives
//! in the `SelectionResolver` (via `ResolverHandle`), never in this widget.
//!
//! Text editing is delegated to the registered WidgetTree-owned TextField child.
//! ComboBox owns focus, intercepts popup keys, and forwards all text-editing events
//! to the TextField. TextField handles caret, selection, clipboard, undo, IME.
//!
//! Presentation snapshot (SPEC-HAL-188A): owned, inert, replace-only, authority-free
//! option data. The widget stores an internal OptionSnapshot and never interprets labels.

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::appearance::InputSurfaceAppearance;
use crate::core::event::{
    Event, EventPhase, GestureAxis, GestureKind, KeyCode, PointerButton, PointerEvent,
    PointerEventKind, PointerRouteOrigin, PointerType, QualifiedTouchMotion, TouchHit,
    TouchPressIdentity, TouchTap, TouchTarget,
};
use crate::core::icon::IconId;
use crate::core::layout::{Constraints, LayoutResult};

// Existing empty-input minimum; alignment measurement is independent of the query.
const MIN_TEXT_AREA: f32 = 60.0;
use crate::core::composite_text_edit::{
    CompositeEditOperation, CompositeEditOutcome, CompositeEditReply, CompositeEditorId,
};
use crate::core::math::{Point, RectF, Size, snap_to_physical};
use crate::core::paint::{
    Color, FrostedGlassAppearance, LAYER_CONTENT, LAYER_FOCUS, LAYER_OVERLAY, RoundedCorners,
    ZIndex,
};
use crate::core::paragraph::TextAlign;
use crate::core::popup::{PopupInteractionPolicy, PopupWidthPolicy};
use crate::core::text_engine::{GlyphRun, TextEngine, compute_control_text_baseline_y};
use crate::core::theme::{ControlChromeCategory, ControlChromeState, Theme};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::{
    ComboPopupInputGeometry, ComboPopupScrollbar, NodeId, OverlayHitRegion,
};
use crate::model::search_projection_harness::{
    MatchMode, ProjectionOutput, SearchProjectionHarness,
};
use crate::model::selection_resolver::{
    CommitSource, CommittedValue, LegacyPrefixPreview, OptionEntry, OptionId, ResolverDecision,
    ResolverHandle, ResolverInput,
};
use crate::renderer::path::{Path, StrokeParams};
use crate::utils::id::TreeWidgetId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::composite_edit_host::{
    CompositeEditorFilter, CompositeEditorSeed, CompositeEditorState, CompositeHostContinuation,
    CompositeHostStep,
};
use crate::widgets::scrollbar::{
    OVERLAY_SCROLLBAR_HIDE_DELAY_SECS, ScrollbarLayout, ScrollbarState, ScrollbarV,
};
use std::cell::Cell;
use std::sync::Arc;
use unicode_segmentation::UnicodeSegmentation;

mod option_groups;
pub(crate) use option_groups::{ChoiceDisplayRow, ChoiceGroups};
pub use option_groups::{GroupedProjectionError, OptionGroupBoundary};

// Popup geometry constants.
const ROW_HEIGHT: f32 = 24.0;
const GROUP_HEADER_FONT_SCALE: f32 = 0.95;
const GROUP_HEADER_INSET: f32 = 4.0;
const GROUP_ITEM_INSET: f32 = 12.0;
const GROUP_HEADER_MARKER_WIDTH: f32 = 3.0;
const GROUP_FOOTER_HEIGHT: f32 = 28.0;
const POPUP_REVEAL_SECS: f32 = 0.025;
// Match the overlay scrollbar fade in ScrollArea::update.
const POPUP_SCROLLBAR_FADE_SECS: f32 = 0.3;
const POPUP_SURFACE_LAYER: ZIndex = LAYER_OVERLAY + 1;
const POPUP_ROW_LAYER: ZIndex = LAYER_OVERLAY + 2;
const POPUP_MATCH_LAYER: ZIndex = LAYER_OVERLAY + 3;
const POPUP_TEXT_LAYER: ZIndex = LAYER_OVERLAY + 4;
const POPUP_CHROME_LAYER: ZIndex = LAYER_OVERLAY + 5;
const POPUP_BORDER_LAYER: ZIndex = LAYER_OVERLAY + 6;

struct GroupedPopupColors {
    heading: Color,
    footer: Color,
    text: Color,
    rule: Color,
}

impl GroupedPopupColors {
    fn from_theme(theme: &Theme) -> Self {
        let colors = &theme.colors;
        let rule = colors.borders.strong.with_alpha(1.0);
        let (heading, footer, text) = if theme.is_dark() {
            (
                colors.list.header_background.lerp(rule, 0.44),
                colors.surfaces.chrome,
                colors.typography.primary,
            )
        } else {
            (
                colors.list.header_background.lerp(colors.typography.primary, 0.92),
                colors.typography.primary,
                colors.typography.inverse,
            )
        };
        Self {
            heading: heading.with_alpha(1.0),
            footer: footer.with_alpha(1.0),
            text: text.with_alpha(1.0),
            rule,
        }
    }
}

fn shape_group_heading(
    engine: &mut dyn TextEngine,
    title: &str,
    size: f32,
    available: f32,
) -> Option<GlyphRun> {
    let full = engine.shape(title, size);
    if full.width <= available {
        return Some(full);
    }
    let mut fitted = engine.shape("…", size);
    if fitted.width > available {
        return None;
    }
    let ends: Vec<_> = title.grapheme_indices(true).map(|(index, _)| index).collect();
    let (mut start, mut end) = (0, ends.len());
    while start < end {
        let mid = start + (end - start) / 2;
        let run = engine.shape(&format!("{}…", &title[..ends[mid]]), size);
        if run.width <= available {
            fitted = run;
            start = mid + 1;
        } else {
            end = mid;
        }
    }
    Some(fitted)
}

fn selected_border(
    appearance: &InputSurfaceAppearance,
    state: ControlChromeState,
    fallback: crate::core::paint::Color,
) -> crate::core::paint::Color {
    match (state, appearance.focus_colors()) {
        (ControlChromeState::Focused, Some((focused, _))) => focused,
        (ControlChromeState::Captured, Some((_, captured))) => captured,
        (ControlChromeState::Focused | ControlChromeState::Captured, None) => fallback,
        _ => appearance.border(),
    }
}

/// Admission for the first ordinary, standalone frosted ComboBox profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComboBoxGlassError {
    RequiresDisplayMode,
    RequiresMatchTrigger,
    RequiresInputSurfaceAppearance,
}

impl std::fmt::Display for ComboBoxGlassError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "frosted ComboBox appearance refused: {self:?}")
    }
}
impl std::error::Error for ComboBoxGlassError {}

/// Per-instance display policy for an overflowing ComboBox popup.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PopupScrollbarPolicy {
    /// Keep the existing scrollbar visible whenever the popup overflows.
    #[default]
    WhenOverflowing,
    /// Reveal on opening with overflow, gutter hover, scrolling, or thumb drag.
    AutoHide,
}

/// Display mode and inactivity timing for one ComboBox popup scrollbar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupScrollbarInteractionPolicy {
    display_mode: PopupScrollbarPolicy,
    hold_seconds: f32,
    fade_seconds: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupScrollbarInteractionPolicyError {
    InvalidHoldSeconds,
    InvalidFadeSeconds,
}

impl std::fmt::Display for PopupScrollbarInteractionPolicyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidHoldSeconds => "scrollbar hold must be finite and in 0..=60 seconds",
            Self::InvalidFadeSeconds => "scrollbar fade must be finite and in 0..=10 seconds",
        })
    }
}

impl std::error::Error for PopupScrollbarInteractionPolicyError {}

impl Default for PopupScrollbarInteractionPolicy {
    fn default() -> Self {
        Self {
            display_mode: PopupScrollbarPolicy::WhenOverflowing,
            hold_seconds: OVERLAY_SCROLLBAR_HIDE_DELAY_SECS,
            fade_seconds: POPUP_SCROLLBAR_FADE_SECS,
        }
    }
}

impl PopupScrollbarInteractionPolicy {
    /// Check inactivity timing. A zero fade hides the bar as soon as the hold ends.
    pub fn try_new(
        display_mode: PopupScrollbarPolicy,
        hold_seconds: f32,
        fade_seconds: f32,
    ) -> Result<Self, PopupScrollbarInteractionPolicyError> {
        if !(0.0..=60.0).contains(&hold_seconds) {
            return Err(PopupScrollbarInteractionPolicyError::InvalidHoldSeconds);
        }
        if !(0.0..=10.0).contains(&fade_seconds) {
            return Err(PopupScrollbarInteractionPolicyError::InvalidFadeSeconds);
        }
        Ok(Self {
            display_mode,
            hold_seconds,
            fade_seconds,
        })
    }

    pub fn display_mode(&self) -> PopupScrollbarPolicy {
        self.display_mode
    }

    pub fn hold_seconds(&self) -> f32 {
        self.hold_seconds
    }

    pub fn fade_seconds(&self) -> f32 {
        self.fade_seconds
    }
}

/// Theme-relative opacity factors for one ComboBox popup scrollbar.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PopupScrollbarAppearance {
    track_opacity: f32,
    thumb_opacity: f32,
    thumb_hover_opacity: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PopupScrollbarAppearanceError {
    InvalidTrackOpacity,
    InvalidThumbOpacity,
    InvalidThumbHoverOpacity,
}

impl std::fmt::Display for PopupScrollbarAppearanceError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::InvalidTrackOpacity => "scrollbar track opacity must be finite and in 0..=1",
            Self::InvalidThumbOpacity => "scrollbar thumb opacity must be finite and in 0..=1",
            Self::InvalidThumbHoverOpacity => {
                "scrollbar hovered and pressed thumb opacity must be finite and in 0..=1"
            }
        })
    }
}

impl std::error::Error for PopupScrollbarAppearanceError {}

impl Default for PopupScrollbarAppearance {
    fn default() -> Self {
        Self {
            track_opacity: 1.0,
            thumb_opacity: 1.0,
            thumb_hover_opacity: 1.0,
        }
    }
}

impl PopupScrollbarAppearance {
    /// Each factor multiplies the corresponding theme alpha. Pressed uses the
    /// same thumb factor as Hovered.
    pub fn try_new(
        track_opacity: f32,
        thumb_opacity: f32,
        thumb_hover_opacity: f32,
    ) -> Result<Self, PopupScrollbarAppearanceError> {
        if !(0.0..=1.0).contains(&track_opacity) {
            return Err(PopupScrollbarAppearanceError::InvalidTrackOpacity);
        }
        if !(0.0..=1.0).contains(&thumb_opacity) {
            return Err(PopupScrollbarAppearanceError::InvalidThumbOpacity);
        }
        if !(0.0..=1.0).contains(&thumb_hover_opacity) {
            return Err(PopupScrollbarAppearanceError::InvalidThumbHoverOpacity);
        }
        Ok(Self {
            track_opacity,
            thumb_opacity,
            thumb_hover_opacity,
        })
    }

    pub fn track_opacity(&self) -> f32 {
        self.track_opacity
    }

    pub fn thumb_opacity(&self) -> f32 {
        self.thumb_opacity
    }

    pub fn thumb_hover_opacity(&self) -> f32 {
        self.thumb_hover_opacity
    }
}

#[derive(Default)]
struct PopupPresentation {
    reveal: f32,
    target_open: bool,
    snap_requested: bool,
    animation: Option<TransitionHandle>,
    retiring: Option<TransitionHandle>,
}

impl PopupPresentation {
    fn update(&mut self, open: bool, cx: &mut UpdateContext) {
        if let Some(handle) = self.retiring {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::TypeMismatch) => {
                    panic!("ComboBox popup handle must contain f32")
                }
                Some(_) => {
                    cx.cancel_animation(handle);
                    self.retiring = None;
                }
                None => {}
            }
        }
        let previous = (self.reveal, self.target_open);
        let endpoint = if open { 1.0 } else { 0.0 };
        if let Some(handle) = self.animation {
            self.reveal = match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::Active(value)) => value,
                Some(AnimationSample::Completed(value)) => {
                    self.animation = None;
                    value
                }
                Some(AnimationSample::Cancelled | AnimationSample::Missing) => {
                    self.animation = None;
                    if self.target_open { 1.0 } else { 0.0 }
                }
                None => {
                    self.animation = None;
                    self.retiring = Some(handle);
                    if self.target_open { 1.0 } else { 0.0 }
                }
                Some(AnimationSample::TypeMismatch) => {
                    panic!("ComboBox popup handle must contain f32")
                }
            };
        }
        if self.target_open != open || self.snap_requested {
            if let Some(handle) = self.animation.take() {
                cx.cancel_animation(handle);
            }
            self.target_open = open;
            let snap = self.snap_requested && self.reveal == 0.0;
            if !snap && self.reveal != endpoint {
                self.animation = cx.start_animation(Animation::new(
                    self.reveal,
                    endpoint,
                    POPUP_REVEAL_SECS,
                    if open {
                        Easing::EaseOut
                    } else {
                        Easing::EaseIn
                    },
                ));
            }
            if self.animation.is_none() {
                self.reveal = endpoint;
            }
            self.snap_requested = false;
        }
        // HAL-354 / PM-018: the exact terminal sample still needs local repaint.
        if previous != (self.reveal, self.target_open) {
            cx.invalidate_paint();
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub(super) struct PopupGeometry {
    pub(super) control: RectF,
    pub(super) popup: RectF,
    full_height: f32,
}

impl PopupGeometry {
    fn grouped_row_capacity(trigger: RectF, viewport_height: f32) -> usize {
        if trigger.bottom() <= 0.0 || trigger.y >= viewport_height {
            return 0;
        }
        let above = trigger.y.max(0.0);
        let below = (viewport_height - trigger.bottom()).max(0.0);
        ((above.max(below) - GROUP_FOOTER_HEIGHT).max(0.0) / ROW_HEIGHT).floor() as usize
    }

    fn body_and_footer(self, grouped: bool) -> (RectF, Option<RectF>) {
        if !grouped || self.popup.height <= 0.0 {
            return (self.popup, None);
        }
        let footer_height = self.popup.height.min(GROUP_FOOTER_HEIGHT);
        let body = RectF {
            height: self.popup.height - footer_height,
            ..self.popup
        };
        let footer = RectF {
            y: body.bottom(),
            height: footer_height,
            ..self.popup
        };
        (body, Some(footer))
    }

    pub(super) fn from_rects(control: RectF, popup: RectF) -> Self {
        Self {
            control,
            popup,
            full_height: popup.height,
        }
    }

    fn direction(from: Point, to: Point) -> Point {
        let axis = |delta: f32| if delta == 0.0 { 0.0 } else { delta.signum() };
        Point::new(axis(to.x - from.x), axis(to.y - from.y))
    }

    fn new(
        control: RectF,
        scroll: (f32, f32),
        viewport: (f32, f32),
        width: f32,
        full_height: f32,
        progress: f32,
    ) -> Self {
        let screen_x = control.x - scroll.0;
        let screen_y = control.y - scroll.1;
        let below = viewport.1 - (screen_y + control.height);
        let above = screen_y;
        let height = full_height * progress;
        let y = if below >= full_height || (above < full_height && below >= above) {
            control.y + control.height
        } else {
            control.y - height
        };
        let x = if screen_x + width > viewport.0 {
            (viewport.0 - width).max(0.0) + scroll.0
        } else {
            control.x
        };
        Self {
            control,
            popup: RectF::new(x, y, width, height),
            full_height,
        }
    }

    pub(super) fn snapped(self, scale: f32) -> Self {
        fn snap(rect: RectF, scale: f32) -> RectF {
            let x = snap_to_physical(rect.x, scale);
            let y = snap_to_physical(rect.y, scale);
            RectF::new(
                x,
                y,
                snap_to_physical(rect.x + rect.width, scale) - x,
                snap_to_physical(rect.y + rect.height, scale) - y,
            )
        }
        Self {
            control: snap(self.control, scale),
            popup: snap(self.popup, scale),
            ..self
        }
    }

    pub(super) fn input_divider(self, edge: f32, thickness: f32) -> Option<RectF> {
        let thickness = thickness.min(self.control.height).min(self.popup.height);
        let left = self.control.x.max(self.popup.x) + edge;
        let right = self.control.right().min(self.popup.right()) - edge;
        if thickness <= 0.0 || right <= left {
            return None;
        }
        let y = if self.popup.y < self.control.y {
            self.control.y
        } else {
            self.control.bottom() - thickness
        };
        Some(RectF::new(left, y, right - left, thickness))
    }

    fn outline_corners(self, radius: f32, width: f32) -> Vec<(Point, Point, f32)> {
        let width = width.min(self.control.height).min(self.popup.height);
        let (top, bottom) = if self.control.y < self.popup.y {
            (self.control, self.popup)
        } else {
            (self.popup, self.control)
        };
        let join = bottom.y;
        let mut vertices = vec![
            Point::new(top.x, top.y),
            Point::new(top.x + top.width, top.y),
            Point::new(top.x + top.width, join),
            Point::new(bottom.x + bottom.width, join),
            Point::new(bottom.x + bottom.width, bottom.y + bottom.height),
            Point::new(bottom.x, bottom.y + bottom.height),
            Point::new(bottom.x, join),
            Point::new(top.x, join),
        ];
        // Equal edges have no corner at the attachment.
        loop {
            let redundant = (0..vertices.len()).find(|&i| {
                let a = vertices[(i + vertices.len() - 1) % vertices.len()];
                let b = vertices[i];
                let c = vertices[(i + 1) % vertices.len()];
                (a.x == b.x && b.x == c.x) || (a.y == b.y && b.y == c.y)
            });
            let Some(i) = redundant else { break };
            vertices.remove(i);
        }
        let normals: Vec<_> = (0..vertices.len())
            .map(|i| {
                let a = vertices[i];
                let b = vertices[(i + 1) % vertices.len()];
                let direction = Self::direction(a, b);
                Point::new(-direction.y, direction.x)
            })
            .collect();
        let inset: Vec<_> = (0..vertices.len())
            .map(|i| {
                let prev = normals[(i + vertices.len() - 1) % vertices.len()];
                let next = normals[i];
                Point::new(
                    vertices[i].x + (prev.x + next.x) * width * 0.5,
                    vertices[i].y + (prev.y + next.y) * width * 0.5,
                )
            })
            .collect();
        (0..inset.len())
            .map(|i| {
                let a = inset[(i + inset.len() - 1) % inset.len()];
                let b = inset[i];
                let c = inset[(i + 1) % inset.len()];
                let incoming = Self::direction(a, b);
                let outgoing = Self::direction(b, c);
                let convex = incoming.x * outgoing.y - incoming.y * outgoing.x > 0.0;
                let outer = vertices[i];
                let prev = vertices[(i + vertices.len() - 1) % vertices.len()];
                let next = vertices[(i + 1) % vertices.len()];
                let outer_limit = ((outer.x - prev.x).abs() + (outer.y - prev.y).abs())
                    .min((next.x - outer.x).abs() + (next.y - outer.y).abs());
                let r = if convex {
                    (radius - width * 0.5)
                        .max(0.0)
                        .min(((b.x - a.x).abs() + (b.y - a.y).abs()) * 0.5)
                        .min(((c.x - b.x).abs() + (c.y - b.y).abs()) * 0.5)
                        .min((outer_limit - width * 0.5).max(0.0))
                } else {
                    0.0
                };
                (vertices[i], b, r)
            })
            .collect()
    }

    pub(super) fn corners(self, rect: RectF, radius: f32, width: f32) -> RoundedCorners {
        let corners = self.outline_corners(radius, width);
        let half_width = width.min(self.control.height).min(self.popup.height) * 0.5;
        let at = |x, y| {
            corners
                .iter()
                .find(|(outer, _, _)| outer.x == x && outer.y == y)
                .map_or(0.0, |(_, _, r)| if *r > 0.0 { r + half_width } else { 0.0 })
        };
        RoundedCorners::new(
            at(rect.x, rect.y),
            at(rect.x + rect.width, rect.y),
            at(rect.x, rect.y + rect.height),
            at(rect.x + rect.width, rect.y + rect.height),
        )
    }

    fn joined_surface_clip(self, radius: f32, width: f32) -> Option<(RectF, RoundedCorners)> {
        if self.control.x == self.popup.x && self.control.width == self.popup.width {
            let control = self.corners(self.control, radius, width);
            let popup = self.corners(self.popup, radius, width);
            // A full-width attachment has one rounded outer silhouette. Clipping
            // a bounded surface primitive at its own rounded rect would apply
            // inward AA to the internal join and reveal the underlying page.
            if self.popup.y == self.control.bottom() {
                return Some((
                    RectF::new(
                        self.control.x,
                        self.control.y,
                        self.control.width,
                        self.popup.bottom() - self.control.y,
                    ),
                    RoundedCorners::new(control.tl, control.tr, popup.bl, popup.br),
                ));
            }
            if self.popup.bottom() == self.control.y {
                return Some((
                    RectF::new(
                        self.control.x,
                        self.popup.y,
                        self.control.width,
                        self.control.bottom() - self.popup.y,
                    ),
                    RoundedCorners::new(popup.tl, popup.tr, control.bl, control.br),
                ));
            }
        }
        None
    }

    /// One union boundary, partitioned only by the existing main/overlay clips.
    pub(super) fn outline(self, radius: f32, width: f32) -> Path {
        let corners: Vec<_> = self
            .outline_corners(radius, width)
            .into_iter()
            .map(|(_, point, radius)| (point, radius))
            .collect();
        Self::rounded_path(&corners)
    }

    pub(super) fn fill_outline(self, radius: f32, width: f32) -> Path {
        let half_width = width.min(self.control.height).min(self.popup.height) * 0.5;
        let corners: Vec<_> = self
            .outline_corners(radius, width)
            .into_iter()
            .map(|(point, _, radius)| {
                (
                    point,
                    if radius > 0.0 {
                        radius + half_width
                    } else {
                        0.0
                    },
                )
            })
            .collect();
        Self::rounded_path(&corners)
    }

    fn rounded_path(corners: &[(Point, f32)]) -> Path {
        let mut path = Path::new();
        for i in 0..corners.len() {
            let a = corners[(i + corners.len() - 1) % corners.len()].0;
            let (b, r) = corners[i];
            let c = corners[(i + 1) % corners.len()].0;
            let incoming = Self::direction(a, b);
            let outgoing = Self::direction(b, c);
            let start = Point::new(b.x - incoming.x * r, b.y - incoming.y * r);
            let end = Point::new(b.x + outgoing.x * r, b.y + outgoing.y * r);
            if i == 0 {
                path.move_to(start.x, start.y);
            } else {
                path.line_to(start.x, start.y);
            }
            if r > 0.0 {
                let k = 0.552_284_8 * r;
                path.cubic_to(
                    start.x + incoming.x * k,
                    start.y + incoming.y * k,
                    end.x - outgoing.x * k,
                    end.y - outgoing.y * k,
                    end.x,
                    end.y,
                );
            } else {
                path.line_to(end.x, end.y);
            }
        }
        path.close();
        path
    }
}

/// Which zone of the popup had focus when commit was triggered.
/// Drives `CommitSource` from the event path, NOT from state inspection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PopupFocusZone {
    TextInput,
    OptionList,
}

/// Derived input background state for ambient visual feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum InputBgState {
    Default,
    Browsing,
    Custom,
    NoMatch,
    Loading,
}

/// Internal search intent emitted when TextField content may have changed.
///
/// Carries the current query. This is a semantic signal — it does not contain
/// results, matching logic, or policy.
///
/// The projection provider or model preview consumes the verbatim query.
struct SearchIntent {
    /// Current text in the TextField at the time of emission.
    query: String,
}

/// Internal projection snapshot consumed by ComboBox.
///
/// This is an internal representation, not a public API.
struct OptionSnapshot {
    /// Ordered visible options (id + display label).
    items: Vec<(OptionId, String)>,
    /// Optional immutable presentation indexed within these same options.
    groups: Option<Arc<ChoiceGroups>>,
    /// Per-item match range: (byte_offset, byte_len) parallel to items.
    /// Empty when no filtering active (no highlighting needed).
    match_ranges: Vec<(usize, usize)>,
    /// Data-layer highlight suggestion applied on replacement.
    highlight_hint: Option<OptionId>,
    /// Minimum rows the popup should display (SPEC-203 §3.1).
    min_visible_rows: usize,
    /// Maximum rows the popup should display (SPEC-203 §3.1).
    max_visible_rows: usize,
}

impl OptionSnapshot {
    fn empty() -> Self {
        Self {
            items: Vec::new(),
            groups: None,
            match_ranges: Vec::new(),
            highlight_hint: None,
            min_visible_rows: 0,
            max_visible_rows: 8,
        }
    }

    fn entries(&self) -> Vec<OptionEntry<'_>> {
        self.items
            .iter()
            .map(|(id, label)| OptionEntry { id: *id, label })
            .collect()
    }

    fn display_len(&self) -> usize {
        self.groups
            .as_ref()
            .map_or(self.items.len(), |groups| groups.row_count())
    }

    fn display_row(&self, row: usize) -> Option<ChoiceDisplayRow> {
        if let Some(groups) = &self.groups {
            groups.row(row)
        } else {
            (row < self.items.len()).then_some(ChoiceDisplayRow::Item(row))
        }
    }

    fn display_row_for_item(&self, item: usize) -> usize {
        self.groups
            .as_ref()
            .map_or(item, |groups| groups.item_row(item))
    }
}

type TypeToSelectMatcher = dyn Fn(&str, &[OptionEntry<'_>]) -> Option<OptionId>;

/// Refusal to apply a model-owned selection to a display-only ComboBox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum DisplaySelectionError {
    #[error("display selection requires a noneditable ComboBox")]
    Editable,
    #[error("option ID {0:?} is absent from the current projection")]
    UnknownOption(OptionId),
    #[error("could not set the initial ComboBox caption: {0:?}")]
    Editor(CompositeEditOutcome),
}

#[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
#[derive(Clone, Copy)]
enum HostSelectedOption {
    Seed(OptionId),
    Observed(OptionId, u64),
}

/// SPEC-HAL-201: ComboBox widget.
///
/// Resolver-driven, zero-semantics interaction shell. The widget handles
/// input routing, popup display, and overlay hit regions. All commit
/// decisions are delegated to the `ResolverHandle`.
///
/// Text editing is handled by an internal `TextField` (not in the widget tree).
/// ComboBox owns focus and forwards text events to the TextField.
pub struct ComboBox {
    id: WidgetInternalId,
    // Projection snapshot: internal source-of-truth for popup contents.
    snapshot: OptionSnapshot,
    // Resolver (opaque, widget calls resolve() only)
    resolver: ResolverHandle,
    // Sealed real-child relation and owned canonical draft observation.
    text_field: CompositeEditorState,
    // Popup-transient state
    highlighted_id: Option<OptionId>,
    popup_open: Cell<bool>,
    popup_touch_pan: Option<(TouchPressIdentity, f32)>,
    // Focus zone: drives CommitSource from event path, NOT from state inspection
    focus_zone: PopupFocusZone,
    // Widget state
    enabled: bool,
    focused: bool,
    cell_hovered: bool,
    in_capture: bool,
    // Latched commit output (one-shot retrieval by parent)
    committed_latch: Option<CommittedValue>,
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    host_selected_option: Option<HostSelectedOption>,
    // Whether the current highlight was set by hover (vs keyboard)
    highlight_from_hover: bool,
    // Type-to-search: prefix-match bias (does not filter options)
    type_to_search: bool,
    // Model-owned presentation preview for the current option snapshot.
    prefix_preview: LegacyPrefixPreview,
    // Cached from theme for affordance painting and hit-testing
    affordance_width: f32,
    // Cached input metric for event-time rebuilds (EventContext has no theme).
    input_font_size: f32,
    // When true, clicking the text area while popup is open closes the popup
    close_on_text_click: bool,
    // Optional filtering provider in place of the unfiltered model preview.
    search_harness: Option<SearchProjectionHarness>,
    // Matching mode: Prefix (default) or Substring
    match_mode: MatchMode,
    // Whether custom (non-matching) values are valid. Default: false (list-only).
    allow_custom_value: bool,
    // When false, the input surface displays the current value without caret,
    // selection, or text editing. Popup selection remains active.
    text_input_enabled: bool,
    type_to_select_matcher: Option<Box<TypeToSelectMatcher>>,
    input_surface_appearance: Option<InputSurfaceAppearance>,
    hover_surface_on_closed_focus: bool,
    frosted_glass: Option<FrostedGlassAppearance>,
    leading_icon: Option<IconId>,
    leading_icon_size: Option<f32>,
    leading_icon_y_offset: f32,
    /// Layout-owned leading offset, including the gap between icon and text.
    leading_width: f32,
    chevron_half_extents: Option<Size>,
    interaction_policy: PopupInteractionPolicy,
    // Index of the first visible display row in the popup.
    popup_scroll_row: usize,
    // Effective capacity from the latest grouped overlay publication.
    grouped_popup_rows: Cell<Option<usize>>,
    // Scrollbar state (SPEC-HAL-086/110: existing ScrollbarV infrastructure)
    scrollbar_v: ScrollbarV,
    scrollbar_interaction: PopupScrollbarInteractionPolicy,
    scrollbar_appearance: PopupScrollbarAppearance,
    scrollbar_hold_remaining: f32,
    scrollbar_fade_alpha: f32,
    scrollbar_activity: bool,
    // Distinguish a newly opened popup or replacement option projection from
    // the previous scene's still-retained overlay.
    popup_input_revision: u64,
    // Option row press active — commit deferred to PointerUp (Button press/release semantics)
    option_press_active: bool,
    // Ghost completion suffix (visual only, not part of actual text).
    // Computed from best candidate after search intent. Rendered in muted color
    // after the typed text. Disappears when no candidate, popup closes, or conflicting input.
    ghost_completion: Option<String>,
    // Exact current auxiliary-source qualifier, never a text editing model.
    ghost_source: Option<(Option<u64>, Option<u64>, String)>,
    // Async search loading state. When true: popup closed, input bg = loading,
    // nav keys no-op, Enter commits only if allow_custom_value.
    // Set externally via set_loading(). Cleared when results arrive.
    loading: bool,
    popup_presentation: PopupPresentation,
    /// Widest measured option label (logical px). Computed in layout, read in
    /// paint_overlay to size the popup so the longest option fits without truncation.
    widest_option_px: Cell<f32>,
    /// Programmatic values for each option (parallel by OptionId to snapshot.items).
    /// Empty vec or empty string means label IS the value (simple mode).
    option_values: Vec<(OptionId, String)>,
    /// Fired in `attempt_commit` when a value commits via the resolver.
    /// Argument: the committed value (`ExistingOption` or `CustomValue`).
    on_committed: Option<CommitCallback>,
    /// Fired in `close_without_commit` when the popup closes without a
    /// commit (Escape, blur, focus-loss). Suppressed when a commit is
    /// already pending in `committed_latch` to avoid double-fire during
    /// post-commit tear-down.
    on_cancelled: Option<Box<dyn Fn()>>,
}

enum CommitCallback {
    Public(Box<dyn Fn(&CommittedValue)>),
    FactoryCurrent(Box<dyn Fn(String)>),
}

pub(crate) enum CompositeComboContinuation {
    Input { search: bool },
    Resolve { source: CommitSource },
    Commit { value: CommittedValue },
    Entry,
    Retire,
}

impl ComboBox {
    /// Install the adapter's prefix matcher for this non-editing popup.
    ///
    /// With text input disabled and the popup open in keyboard Capture, the
    /// adapter receives plain and Shift-modified text verbatim and borrows the
    /// current option IDs and display labels in popup order. A current ID moves the
    /// highlight into view. Enter or a row click still delegates commit to the
    /// selection resolver.
    pub fn with_type_to_select(
        mut self,
        match_prefix: impl Fn(&str, &[OptionEntry<'_>]) -> Option<OptionId> + 'static,
    ) -> Self {
        self.type_to_select_matcher = Some(Box::new(match_prefix));
        self
    }

    fn active_type_to_select(&self) -> Option<&TypeToSelectMatcher> {
        if self.text_input_enabled || !self.popup_open.get() {
            return None;
        }
        self.type_to_select_matcher.as_deref()
    }

    /// Select this ComboBox's overflow scrollbar mode and keep its timing.
    ///
    /// AutoHide keeps hidden gutter space available to the options. Scrolling
    /// and scrollbar hover reveal the same native track and thumb.
    pub fn with_popup_scrollbar_policy(mut self, policy: PopupScrollbarPolicy) -> Self {
        self.scrollbar_interaction.display_mode = policy;
        self
    }

    /// Select the display mode, inactivity hold and fade for this popup.
    pub fn with_popup_scrollbar_interaction_policy(
        mut self,
        policy: PopupScrollbarInteractionPolicy,
    ) -> Self {
        self.scrollbar_interaction = policy;
        self
    }

    /// Select this popup's track, normal thumb, and hovered/pressed thumb opacity.
    pub fn with_popup_scrollbar_appearance(mut self, appearance: PopupScrollbarAppearance) -> Self {
        self.scrollbar_appearance = appearance;
        self
    }

    /// Select per-instance popup behavior/geometry without changing application
    /// UiInteractionPolicy, keyboard authority or resolver semantics.
    pub fn with_interaction_policy(mut self, policy: PopupInteractionPolicy) -> Self {
        assert!(
            self.frosted_glass.is_none() || matches!(policy.width, PopupWidthPolicy::MatchTrigger),
            "a frosted ComboBox requires MatchTrigger"
        );
        self.interaction_policy = policy;
        self
    }

    /// Select a checked surface for this ComboBox and its existing popup.
    /// Actual Focus/Capture cues and cell-owned exterior chrome remain authoritative.
    /// Omitting this value preserves the ordinary theme paint path.
    /// Fully transparent surface stops, border, inner edge and affordance omit
    /// those decorative paints; the label, icon, caret, popup and active cue remain.
    pub fn with_input_surface_appearance(mut self, value: InputSurfaceAppearance) -> Self {
        self.input_surface_appearance = Some(value);
        self
    }

    /// Prefer the configured Hover face when this standalone ComboBox is
    /// hovered, focused, fully closed, and free of keyboard and pointer capture.
    ///
    /// The current Focus cue is painted independently. This changes only
    /// face material selection, including when the caller supplies state
    /// surfaces through `InputSurfaceAppearance::with_state_surfaces`.
    pub fn with_hover_surface_when_closed_and_focused(mut self) -> Self {
        self.hover_surface_on_closed_focus = true;
        self
    }

    /// Attach the existing same-surface backdrop material to this real control.
    /// Configure a standalone, nonediting display and MatchTrigger first. Its
    /// parent-owned label/icon/caret can then follow one joined glass marker in
    /// the existing popup scope; no tree-owned editor paint is transferred.
    /// Existing input surfaces remain state tints and chrome over the glass.
    /// The existing HAL385 geometry, clip, source and exclusive-order admission
    /// still applies; use normal control-subtree ordering when required.
    pub fn try_with_frosted_glass(
        mut self,
        appearance: FrostedGlassAppearance,
    ) -> Result<Self, ComboBoxGlassError> {
        if self.text_input_enabled {
            return Err(ComboBoxGlassError::RequiresDisplayMode);
        }
        if !matches!(
            self.interaction_policy.width,
            PopupWidthPolicy::MatchTrigger
        ) {
            return Err(ComboBoxGlassError::RequiresMatchTrigger);
        }
        if self.input_surface_appearance.is_none() {
            return Err(ComboBoxGlassError::RequiresInputSurfaceAppearance);
        }
        self.frosted_glass = Some(appearance);
        Ok(self)
    }

    /// Place a registered, noninteractive icon inside the existing input shell.
    /// Its default size follows the input font. Layout reserves its space
    /// before positioning the label or the real editor child.
    pub fn with_leading_icon(mut self, icon: IconId) -> Self {
        self.leading_icon = Some(icon);
        self
    }

    /// Request a leading icon size in logical pixels before mounting.
    ///
    /// Layout fits the request inside the shell, keeping room for the text
    /// viewport and trailing affordance.
    ///
    /// # Panics
    /// Panics when `logical_px` is non-finite or non-positive.
    pub fn with_leading_icon_size(mut self, logical_px: f32) -> Self {
        assert!(
            logical_px.is_finite() && logical_px > 0.0,
            "leading icon size must be finite and positive"
        );
        self.leading_icon_size = Some(logical_px);
        self
    }

    /// Offset only the leading icon's paint in logical pixels. Negative moves up.
    /// The offset defaults to zero and fits within the existing icon's vertical clearance.
    ///
    /// # Panics
    /// Panics when `logical_px` is non-finite.
    pub fn with_leading_icon_y_offset(mut self, logical_px: f32) -> Self {
        assert!(logical_px.is_finite(), "leading icon offset must be finite");
        self.leading_icon_y_offset = logical_px;
        self
    }

    /// Set the chevron path's half-width and half-height in logical pixels.
    /// These extents exclude the stroke and fit within the existing affordance.
    /// The default path uses `(3.0, 1.5)`. The affordance and its hit area stay fixed.
    ///
    /// # Panics
    /// Panics when either extent is non-finite or non-positive.
    pub fn with_chevron_half_extents(mut self, half_width: f32, half_height: f32) -> Self {
        assert!(
            half_width.is_finite() && half_width > 0.0 && half_height.is_finite() && half_height > 0.0,
            "chevron half-extents must be finite and positive"
        );
        self.chevron_half_extents = Some(Size::new(half_width, half_height));
        self
    }

    fn leading_width_for(&self, available: Size, theme: &Theme) -> f32 {
        if self.leading_icon.is_none() {
            return 0.0;
        }
        let geom = theme.geometry.text_field;
        let Some(size) = self.leading_icon_size else {
            return theme.typography.input_font_size + geom.padding_x;
        };
        // The slot includes one gap. Keep both text insets and one caret's
        // width available after the slot and the arrow compartment.
        let icon_width = available.width
            - theme.geometry.combo_box.affordance_width
            - geom.padding_x * 3.0
            - theme.typography.caret_thickness;
        let size = size
            .min((available.height - geom.padding_y * 2.0).max(0.0))
            .min(icon_width.max(0.0));
        if size > 0.0 { size + geom.padding_x } else { 0.0 }
    }

    pub(crate) fn cell_editor_visual_facts(
        &self,
    ) -> crate::core::appearance::CellEditorVisualFacts {
        crate::core::appearance::CellEditorVisualFacts {
            disabled: !self.enabled,
            hovered: self.cell_hovered,
        }
    }

    pub(crate) fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub(crate) fn apply_availability_policy(&mut self, enabled: bool) {
        if !enabled {
            self.close_without_commit();
            self.focused = false;
            self.in_capture = false;
            self.cell_hovered = false;
            self.focus_zone = PopupFocusZone::TextInput;
            // Remove the popup immediately and retire its handle in the next Update.
            self.popup_presentation = PopupPresentation {
                retiring: self
                    .popup_presentation
                    .animation
                    .or(self.popup_presentation.retiring),
                ..PopupPresentation::default()
            };
        }
        self.enabled = enabled;
    }

    /// Select shared text-entry presentation before mounting this control.
    pub fn with_smooth_caret_prototype(mut self, enabled: bool) -> Self {
        self.text_field
            .set_initial_motion(enabled)
            .expect("composite presentation is a construction property");
        self
    }
    pub(crate) fn composite_seed(&self) -> CompositeEditorSeed {
        let mut seed = self.text_field.construction_seed();
        seed.palette = crate::widgets::composite_edit_host::CompositeEditorPalette::InputChrome;
        seed
    }
    pub(crate) fn composite_id(&self) -> Option<&CompositeEditorId> {
        self.text_field.id.as_ref()
    }
    pub(crate) fn composite_bind(&mut self, id: CompositeEditorId, child: NodeId) -> bool {
        self.text_field.bind(id, child)
    }
    pub(crate) fn composite_active(&self) -> bool {
        self.enabled
            && self.in_capture
            && self.text_input_enabled
            && matches!(self.focus_zone, PopupFocusZone::TextInput)
    }
    pub(crate) fn composite_visible(&self) -> bool {
        self.text_input_enabled
    }
    pub(crate) fn composite_policy(&self) -> crate::core::text_edit_policy::TextEditPolicy {
        crate::core::text_edit_policy::TextEditPolicy {
            enabled: self.enabled && self.text_input_enabled,
            read_only: false,
        }
    }
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    pub(crate) fn has_seeded_display_selection(&self) -> bool {
        matches!(self.host_selected_option, Some(HostSelectedOption::Seed(_)))
    }

    pub(crate) fn composite_observe(&mut self, reply: &CompositeEditReply) -> bool {
        let applied = self.text_field.absorb(reply);
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        if applied
            && let Some(HostSelectedOption::Seed(id)) = self.host_selected_option
            && let Some(snapshot) = self.text_field.snapshot.as_ref()
        {
            self.host_selected_option = Some(HostSelectedOption::Observed(id, snapshot.revision));
        }
        if applied && self.ghost_completion.is_some() {
            self.set_ghost_completion(self.ghost_completion.clone());
        }
        applied
    }
    pub(crate) fn composite_presentation_revision(&self) -> u64 {
        self.text_field.presentation_revision()
    }

    fn set_ghost_completion(&mut self, ghost: Option<String>) {
        let source = ghost.as_ref().map(|_| {
            let label = self
                .highlighted_id
                .and_then(|id| {
                    self.snapshot
                        .items
                        .iter()
                        .find(|(item_id, _)| *item_id == id)
                })
                .map(|(_, label)| label.clone())
                .unwrap_or_default();
            (
                self.text_field
                    .snapshot
                    .as_ref()
                    .map(|snapshot| snapshot.revision),
                self.highlighted_id.map(|id| id.raw()),
                label,
            )
        });
        if self.ghost_completion != ghost || self.ghost_source != source {
            self.text_field.change_presentation_source();
            self.ghost_completion = ghost;
            self.ghost_source = source;
        }
    }
    pub(crate) fn composite_sync_options(
        &mut self,
        spec: &crate::engines::cell_source::ComboBoxCellEditorSpec,
    ) -> bool {
        let equal = self.snapshot.items.len() == spec.options.len()
            && (self.option_values.is_empty()
                || self.option_values.len() == self.snapshot.items.len())
            && self
                .snapshot
                .items
                .iter()
                .zip(&spec.options)
                .enumerate()
                .all(|(index, ((id, label), (next_label, next_value)))| {
                    let value = self
                        .option_values
                        .iter()
                        .find(|(value_id, _)| value_id == id)
                        .map(|(_, value)| value.as_str())
                        .filter(|value| !value.is_empty())
                        .unwrap_or(label);
                    *id == OptionId::from_raw(index as u64)
                        && label == next_label
                        && value
                            == if next_value.is_empty() {
                                next_label.as_str()
                            } else {
                                next_value.as_str()
                            }
                });
        if equal {
            return false;
        }
        let options: Vec<_> = spec
            .options
            .iter()
            .enumerate()
            .map(|(index, (label, value))| {
                (
                    OptionId::from_raw(index as u64),
                    label.as_str(),
                    value.as_str(),
                )
            })
            .collect();
        self.set_options_with_values(&options);
        true
    }
    pub(crate) fn composite_disable_input(&mut self) -> bool {
        let changed = self.text_input_enabled;
        self.text_input_enabled = false;
        changed
    }
    fn forward_text(&self, cx: &mut EventContext, search: bool) -> bool {
        cx.request_composite_edit(
            CompositeEditOperation::ForwardCurrentInput,
            CompositeHostContinuation::Combo(CompositeComboContinuation::Input { search }),
        )
        .is_ok()
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn handle_browser_text(
        &self, cx: &mut EventContext,
        action: crate::core::event_router::browser_text::BrowserTextAction,
    ) -> bool {
        use crate::core::event_router::browser_text::BrowserTextAction;
        let search = self.type_to_search && matches!(&action,
            BrowserTextAction::Replace { .. }
                | BrowserTextAction::Composition(crate::core::event::ImeEvent::Commit { .. }));
        self.composite_active() && cx.request_composite_edit(
            CompositeEditOperation::BrowserText(action),
            CompositeHostContinuation::Combo(CompositeComboContinuation::Input { search }),
        ).is_ok()
    }
    pub(crate) fn resume_composite(
        &mut self,
        cx: &mut EventContext,
        continuation: CompositeComboContinuation,
        reply: CompositeEditReply,
    ) -> CompositeHostStep {
        if matches!(continuation, CompositeComboContinuation::Retire) {
            return CompositeHostStep::Done { handled: true };
        }
        if !self.composite_observe(&reply) {
            return CompositeHostStep::Done {
                handled: reply.handled,
            };
        }
        match continuation {
            CompositeComboContinuation::Input { search } => {
                if reply.handled && search {
                    self.handle_search_intent(SearchIntent {
                        query: self.text_field.text().to_owned(),
                    });
                    self.ensure_highlight_visible(cx);
                }
                CompositeHostStep::Done {
                    handled: reply.handled,
                }
            }
            CompositeComboContinuation::Resolve { source } => self.resolve_commit(source, cx),
            CompositeComboContinuation::Commit { value } => {
                self.finish_commit(value, cx);
                CompositeHostStep::Done { handled: true }
            }
            CompositeComboContinuation::Entry => {
                CompositeHostStep::Done { handled: true }
            }
            CompositeComboContinuation::Retire => unreachable!(),
        }
    }
    fn set_popup_open(&mut self, open: bool, animated: bool) {
        let open = open && self.enabled;
        if self.popup_open.replace(open) != open {
            self.grouped_popup_rows.set(None);
            if !open {
                self.popup_touch_pan = None;
            }
            self.popup_input_revision = self.popup_input_revision.wrapping_add(1);
            self.popup_presentation.snap_requested = !animated;
            if self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide {
                self.reset_popup_scrollbar();
                self.refresh_popup_scrollbar();
            }
        }
    }

    pub(crate) fn popup_input_revision(&self) -> Option<u64> {
        (self.enabled && self.popup_open.get() && self.visible_rows() > 0)
            .then_some(self.popup_input_revision)
    }

    pub(crate) fn retire_cell_choice_popup_input(&mut self) {
        // This readiness interval can no longer complete a held option press
        // or drag, even if the same popup is published again after a sync.
        self.option_press_active = false;
        if self.scrollbar_v.dragging {
            self.scrollbar_v.end_drag();
        }
        self.popup_input_revision = self.popup_input_revision.wrapping_add(1);
    }

    fn paint_visible_rows(&self, cx: &PaintContext, visual_rect: RectF) -> usize {
        let mut rows = self.visible_rows();
        if self.snapshot.groups.is_some() {
            let (x, y) = cx.cumulative_scroll_offset_for_paint();
            let height = cx.viewport_size().1;
            // Both anchors use one row window during a layout transition.
            let layout_rect = cx.authoritative_rect_for_paint().unwrap_or(visual_rect);
            for anchor in [layout_rect, visual_rect] {
                rows = rows.min(PopupGeometry::grouped_row_capacity(
                    anchor.translate(-x, -y), height,
                ));
            }
        }
        rows
    }

    fn popup_geometry(&self, cx: &PaintContext, rect: RectF, rows: usize) -> PopupGeometry {
        let progress = self.popup_presentation.reveal;
        let preferred_width =
            (cx.theme.geometry.text_field.padding_x * 2.0 + self.widest_option_px.get()).max(150.0);
        let width = self
            .interaction_policy
            .resolve_width(rect.width, preferred_width);
        let scroll = cx.cumulative_scroll_offset_for_paint();
        let viewport = cx.viewport_size();
        let height = rows as f32 * ROW_HEIGHT;
        if self.snapshot.groups.is_none() {
            return PopupGeometry::new(rect, scroll, viewport, width, height, progress);
        }

        // Snap on the physical window grid, then return to the existing scene space.
        let trigger = rect.translate(-scroll.0, -scroll.1);
        let width = width.min(viewport.0);
        let height = if rows > 0 { height + GROUP_FOOTER_HEIGHT } else { height };
        let mut geometry = PopupGeometry::new(trigger, (0.0, 0.0), viewport, width, height, progress);
        geometry.popup.x = geometry.popup.x.max(0.0);
        let geometry = geometry.snapped(cx.scale_factor);
        PopupGeometry {
            control: geometry.control.translate(scroll.0, scroll.1),
            popup: geometry.popup.translate(scroll.0, scroll.1),
            ..geometry
        }
    }

    pub fn new(resolver: ResolverHandle) -> Self {
        let text_field = CompositeEditorState::new(TextAlign::Left, CompositeEditorFilter::Any);
        Self {
            id: WidgetInternalId::new(),
            snapshot: OptionSnapshot::empty(),
            resolver,
            text_field,
            input_surface_appearance: None,
            hover_surface_on_closed_focus: false,
            frosted_glass: None,
            leading_icon: None,
            leading_icon_size: None,
            leading_icon_y_offset: 0.0,
            leading_width: 0.0,
            chevron_half_extents: None,
            interaction_policy: PopupInteractionPolicy::default(),
            highlighted_id: None,
            popup_open: Cell::new(false),
            popup_touch_pan: None,
            focus_zone: PopupFocusZone::TextInput,
            enabled: true,
            focused: false,
            in_capture: false,
            committed_latch: None,
            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
            host_selected_option: None,
            highlight_from_hover: false,
            type_to_search: false,
            prefix_preview: LegacyPrefixPreview::default(),
            affordance_width: 0.0,
            // INV-CONSTRUCTOR-FALLBACK: synced from theme in layout()
            input_font_size: 14.0,
            close_on_text_click: false,
            search_harness: None,
            match_mode: MatchMode::default(),
            allow_custom_value: false,
            text_input_enabled: true,
            type_to_select_matcher: None,
            popup_scroll_row: 0,
            grouped_popup_rows: Cell::new(None),
            scrollbar_v: ScrollbarV::new(),
            scrollbar_interaction: PopupScrollbarInteractionPolicy::default(),
            scrollbar_appearance: PopupScrollbarAppearance::default(),
            scrollbar_hold_remaining: 0.0,
            scrollbar_fade_alpha: 0.0,
            scrollbar_activity: false,
            popup_input_revision: 0,
            option_press_active: false,
            ghost_completion: None,
            ghost_source: None,
            loading: false,
            popup_presentation: PopupPresentation::default(),
            widest_option_px: Cell::new(0.0),
            option_values: Vec::new(),
            cell_hovered: false,
            on_committed: None,
            on_cancelled: None,
        }
    }

    /// SPEC-HAL-319: Standalone constructor for runtime composition.
    /// Builds a `ComboBox` from a plain `Vec<String>` of option labels,
    /// using a built-in pick-by-highlight resolver. Suitable for
    /// wireframing — production code should pass a real `ResolverHandle`.
    pub fn simple(options: Vec<String>) -> Self {
        use crate::model::selection_resolver::{
            CommittedValue, RejectionReason, ResolverDecision, ResolverHandle, ResolverInput,
            SelectionResolver,
        };

        struct PickHighlightedResolver;
        impl SelectionResolver for PickHighlightedResolver {
            fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                if let Some(id) = input.highlighted_id {
                    ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                } else {
                    ResolverDecision::Reject(RejectionReason::NoMatch)
                }
            }
        }

        let mut cb = Self::new(ResolverHandle::new(PickHighlightedResolver));
        let owned: Vec<(crate::model::selection_resolver::OptionId, String)> = options
            .into_iter()
            .enumerate()
            .map(|(i, label)| {
                (
                    crate::model::selection_resolver::OptionId::from_raw(i as u64),
                    label,
                )
            })
            .collect();
        let borrowed: Vec<(crate::model::selection_resolver::OptionId, &str)> =
            owned.iter().map(|(id, s)| (*id, s.as_str())).collect();
        cb.set_options(&borrowed);
        cb.set_close_on_text_click(true);
        cb
    }

    /// Standalone constructor for runtime composition with separate display
    /// labels and programmatic values. Display shows the label; the value is
    /// retrieved via `committed_value_string()`.
    pub fn simple_with_values(options: Vec<(String, String)>) -> Self {
        use crate::model::selection_resolver::{
            CommittedValue, RejectionReason, ResolverDecision, ResolverHandle, ResolverInput,
            SelectionResolver,
        };

        struct PickHighlightedResolver;
        impl SelectionResolver for PickHighlightedResolver {
            fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                if let Some(id) = input.highlighted_id {
                    ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                } else {
                    ResolverDecision::Reject(RejectionReason::NoMatch)
                }
            }
        }

        let mut cb = Self::new(ResolverHandle::new(PickHighlightedResolver));

        let entries: Vec<(OptionId, String, String)> = options
            .into_iter()
            .enumerate()
            .map(|(i, (label, value))| (OptionId::from_raw(i as u64), label, value))
            .collect();

        let label_only: Vec<(OptionId, &str)> = entries
            .iter()
            .map(|(id, label, _)| (*id, label.as_str()))
            .collect();
        cb.set_options(&label_only);

        cb.option_values = entries
            .iter()
            .map(|(id, _, value)| (*id, value.clone()))
            .collect();

        cb.set_close_on_text_click(true);
        cb
    }

    /// Set options with separate display labels and programmatic values.
    pub fn set_options_with_values(&mut self, options: &[(OptionId, &str, &str)]) {
        let label_only: Vec<(OptionId, &str)> =
            options.iter().map(|(id, label, _)| (*id, *label)).collect();
        self.set_options(&label_only);

        self.option_values = options
            .iter()
            .map(|(id, _, value)| (*id, value.to_string()))
            .collect();
    }

    /// Programmatic value for the currently latched commit, or None if no
    /// commit is pending. Returns the rich-mode value string when set, else
    /// falls back to the display label (simple mode).
    pub fn committed_value_string(&self) -> Option<String> {
        let committed = self.committed_latch.as_ref()?;
        match committed {
            CommittedValue::ExistingOption(id) => {
                if let Some((_, value)) =
                    self.option_values.iter().find(|(opt_id, _)| *opt_id == *id)
                {
                    if !value.is_empty() {
                        return Some(value.clone());
                    }
                }
                self.snapshot
                    .items
                    .iter()
                    .find(|(opt_id, _)| *opt_id == *id)
                    .map(|(_, label)| label.clone())
            }
            _ => None,
        }
    }

    /// Replace-only snapshot update (SPEC-HAL-188A §4.2).
    /// Builds internal snapshot from borrowed input. Legacy ingress path.
    pub fn set_options(&mut self, options: &[(OptionId, &str)]) {
        let snapshot = OptionSnapshot {
            items: options
                .iter()
                .map(|(id, label)| (*id, label.to_string()))
                .collect(),
            groups: None,
            match_ranges: Vec::new(),
            highlight_hint: None,
            min_visible_rows: 0,
            max_visible_rows: 8,
        };
        self.replace_snapshot(snapshot);
    }

    /// Projection-consumer entry point.
    /// Accepts a fully-formed projection snapshot from an external source.
    /// Search-driven replacement will call this.
    ///
    /// This is the non-legacy ingress path. Does not emit search intents —
    /// passive ingress only. Both this and set_options() converge through
    /// replace_snapshot().
    pub fn set_projection(
        &mut self,
        items: &[(OptionId, &str)],
        highlight_hint: Option<OptionId>,
        min_visible_rows: usize,
        max_visible_rows: usize,
    ) {
        let snapshot = OptionSnapshot {
            items: items
                .iter()
                .map(|(id, label)| (*id, label.to_string()))
                .collect(),
            groups: None,
            match_ranges: Vec::new(),
            highlight_hint,
            min_visible_rows,
            max_visible_rows,
        };
        self.replace_snapshot(snapshot);
    }

    /// Replace the option projection with ordered, nonselectable headings.
    ///
    /// Each boundary starts a nonempty group at an index in `items`. Boundaries
    /// must start at zero and increase strictly. Titles must be nonempty and
    /// option IDs must be unique. Validation failure preserves the current
    /// projection, including its revision. The Resolver receives only `items`,
    /// in their original order. Use [Self::set_projection] for a flat list.
    pub fn set_grouped_projection(
        &mut self,
        items: &[(OptionId, &str)],
        group_boundaries: &[OptionGroupBoundary<'_>],
        highlight_hint: Option<OptionId>,
        min_visible_rows: usize,
        max_visible_rows: usize,
    ) -> Result<(), GroupedProjectionError> {
        let groups = Arc::new(ChoiceGroups::new(items, group_boundaries)?);
        self.replace_snapshot(OptionSnapshot {
            items: items
                .iter()
                .map(|(id, label)| (*id, (*label).to_owned()))
                .collect(),
            groups: Some(groups),
            match_ranges: Vec::new(),
            highlight_hint,
            min_visible_rows,
            max_visible_rows,
        });
        Ok(())
    }

    /// Apply a ProjectionOutput directly (preserves match_ranges for highlighting).
    ///
    /// Used by external debounced/async search providers that produce
    /// ProjectionOutput with match data. Converges through replace_snapshot().
    pub fn apply_projection_output(&mut self, output: ProjectionOutput) {
        let snapshot = OptionSnapshot {
            items: output.items,
            groups: None,
            match_ranges: output.match_ranges,
            highlight_hint: output.highlight_hint,
            min_visible_rows: output.min_visible_rows,
            max_visible_rows: output.max_visible_rows,
        };
        self.replace_snapshot(snapshot);
    }

    /// SOLE LEGAL PROJECTION INGRESS — no other code may write self.snapshot.
    ///
    /// Atomic replacement with SPEC-205 invalidation semantics:
    /// - Old snapshot is dropped entirely (no diff, no merge, no reuse)
    /// - highlighted_id cleared (popup-transient, tied to old projection)
    /// - highlight_from_hover cleared (tied to old projection geometry)
    /// - the model preview is rebuilt for the incoming ordered options
    /// - ghost_completion cleared (tied to old projection)
    ///
    /// Preserved across replacement (not projection-scoped):
    /// - popup_open (interaction state)
    /// - focus_zone (interaction state)
    /// - committed_latch (pending retrieval by parent)
    /// - TextField text (control-surface state)
    ///
    /// If snapshot.highlight_hint is present, it is applied after reset.
    /// Future provider code calls this method exclusively.
    fn replace_snapshot(&mut self, snapshot: OptionSnapshot) {
        let hint = snapshot.highlight_hint;
        if self.snapshot.groups.is_some() || snapshot.groups.is_some() {
            self.option_press_active = false;
        }
        self.prefix_preview.replace_options(&snapshot.items);
        self.snapshot = snapshot;
        self.popup_input_revision = self.popup_input_revision.wrapping_add(1);
        // SPEC-205: drop references to old projection state
        self.highlighted_id = None;
        self.highlight_from_hover = false;
        self.popup_scroll_row = 0;
        self.grouped_popup_rows.set(None);
        self.reset_popup_scrollbar();
        self.set_ghost_completion(None);
        // Apply data-layer highlight hint if present
        if let Some(id) = hint {
            self.highlighted_id = Some(id);
        }
    }

    pub fn input_text(&self) -> &str {
        self.text_field.text()
    }

    /// Set the display text shown in the ComboBox input area.
    /// Used by consumers (e.g. PropertyEditor) to project an initial
    /// selection's label so the closed control shows the current value.
    pub fn set_input_text(&mut self, text: String) -> Result<(), CompositeEditOutcome> {
        self.text_field.set_initial_text(text)?;
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        {
            self.host_selected_option = None;
        }
        Ok(())
    }

    /// Project the model's current selection and caption before mounting.
    ///
    /// Disable text input and install the flat or grouped options first.
    /// `selected` must identify a real current option. Use `None` for an
    /// unselected display. Mounting qualifies the selection against the child
    /// editor's initial text revision for native pickers. This operation
    /// leaves popup highlighting and explicit Resolver commits to input.
    pub fn set_display_selection(
        &mut self,
        selected: Option<OptionId>,
        caption: String,
    ) -> Result<(), DisplaySelectionError> {
        if self.text_input_enabled {
            return Err(DisplaySelectionError::Editable);
        }
        if let Some(id) = selected
            && !self.snapshot.items.iter().any(|(item, _)| *item == id)
        {
            return Err(DisplaySelectionError::UnknownOption(id));
        }
        self.text_field
            .set_initial_text(caption)
            .map_err(DisplaySelectionError::Editor)?;
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        {
            self.host_selected_option = selected.map(HostSelectedOption::Seed);
        }
        Ok(())
    }

    /// One-shot retrieval: returns and clears latch.
    pub fn take_committed(&mut self) -> Option<CommittedValue> {
        self.committed_latch.take()
    }

    pub fn set_close_on_text_click(&mut self, enabled: bool) {
        self.close_on_text_click = enabled;
    }

    pub fn set_type_to_search(&mut self, enabled: bool) {
        self.type_to_search = enabled;
    }

    /// Use a search projection harness in place of the unfiltered model preview.
    pub fn set_search_harness(&mut self, harness: SearchProjectionHarness) {
        self.search_harness = Some(harness);
    }

    pub fn set_match_mode(&mut self, mode: MatchMode) {
        self.match_mode = mode;
    }

    pub fn set_allow_custom_value(&mut self, allow: bool) {
        self.allow_custom_value = allow;
    }

    pub fn set_text_input_enabled(&mut self, enabled: bool) {
        assert!(
            !enabled || self.frosted_glass.is_none(),
            "a frosted ComboBox keeps its admitted nonediting display mode"
        );
        self.text_input_enabled = enabled;
    }

    /// Open the popup dropdown programmatically.
    ///
    /// Sets focused, in_capture, and popup_open — same state as
    /// on_capture_enter. Used by PropertyEditor to auto-open
    /// the dropdown on activation without framework focus.
    pub fn open_popup(&mut self) {
        if !self.enabled {
            return;
        }
        self.focused = true;
        self.in_capture = true;
        if !self.popup_open.get() && self.has_popup_content() {
            self.set_popup_open(true, false);
            self.popup_scroll_row = 0;
            self.focus_zone = PopupFocusZone::OptionList;
        }
    }

    /// Set async search loading state.
    ///
    /// When `loading` is true:
    /// - Popup closes (remains closed until loading clears)
    /// - Input background shows loading color
    /// - Navigation keys are no-op
    /// - Enter commits only if allow_custom_value is true
    ///
    /// When `loading` transitions to false:
    /// - If snapshot has items and text is non-empty, popup opens
    /// - Ghost completion recomputed from current state
    pub fn set_loading(&mut self, loading: bool) {
        self.loading = loading;
        if loading {
            if self.popup_open.get() {
                self.set_popup_open(false, true);
                self.highlighted_id = None;
                self.highlight_from_hover = false;
            }
            self.set_ghost_completion(None);
        } else {
            if !self.snapshot.items.is_empty() && !self.text_field.text().is_empty() {
                self.set_popup_open(true, false);
                self.popup_scroll_row = 0;
            }
            let query = self.text_field.text().to_owned();
            self.refresh_ghost_completion(&query);
        }
    }

    pub fn is_loading(&self) -> bool {
        self.loading
    }

    /// Consume a provider projection or a model preview of the current options.
    fn handle_search_intent(&mut self, intent: SearchIntent) {
        // During loading: external provider is handling search. No local processing.
        if self.loading {
            return;
        }
        if let Some(ref harness) = self.search_harness {
            let output = harness.query(&intent.query, self.match_mode);
            let snapshot = OptionSnapshot {
                items: output.items,
                groups: None,
                match_ranges: output.match_ranges,
                highlight_hint: if self.type_to_search {
                    output.highlight_hint
                } else {
                    None
                },
                min_visible_rows: output.min_visible_rows,
                max_visible_rows: output.max_visible_rows,
            };
            self.replace_snapshot(snapshot);
        } else if self.type_to_search {
            self.highlighted_id = self
                .prefix_preview
                .first_highlight(&self.snapshot.items, &intent.query);
            self.highlight_from_hover = false;
        }

        // Zero results: collapse popup unconditionally. Input background is the sole signal.
        if self.popup_open.get() && self.snapshot.items.is_empty() {
            self.set_popup_open(false, true);
        }

        self.refresh_ghost_completion(&intent.query);
    }

    /// Single auditable commit function. Called only on explicit user action.
    fn attempt_commit(&mut self, source: CommitSource, cx: &mut EventContext) -> bool {
        cx.request_composite_edit(
            CompositeEditOperation::ObserveAndActivity,
            CompositeHostContinuation::Combo(CompositeComboContinuation::Resolve { source }),
        ).is_ok()
    }

    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    pub(crate) fn host_choice_options(&self) -> Option<crate::core::event_router::host_choice::ChoiceOptions<'_>> {
        if !self.enabled || !self.has_popup_content() {
            return None;
        }
        let selected = match self.host_selected_option {
            Some(HostSelectedOption::Observed(id, revision))
                if self.text_field.snapshot.as_ref().is_some_and(|snapshot| snapshot.revision == revision)
                    && self.snapshot.items.iter().any(|(item, _)| *item == id) => Some(id),
            _ => None,
        };
        Some(crate::core::event_router::host_choice::ChoiceOptions {
            revision: self.popup_input_revision,
            items: &self.snapshot.items,
            groups: self.snapshot.groups.as_ref(),
            selected,
            popup_open: self.popup_open.get(),
            #[cfg(any(target_arch = "wasm32", test))]
            affordance: self.text_input_enabled.then_some(self.affordance_width),
        })
    }

    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    pub(crate) fn select_host_option(&mut self, cx: &mut EventContext, id: OptionId) -> bool {
        if !self.host_choice_options().is_some_and(|options| {
            !options.popup_open && options.items.iter().any(|(item, _)| *item == id)
        }) {
            return false;
        }
        let previous = self.highlighted_id.replace(id);
        let previous_hover = self.highlight_from_hover;
        self.highlight_from_hover = false;
        let admitted = self.attempt_commit(CommitSource::Selection, cx);
        if !admitted {
            self.highlighted_id = previous;
            self.highlight_from_hover = previous_hover;
        }
        admitted
    }

    fn resolve_commit(&mut self, source: CommitSource, cx: &mut EventContext) -> CompositeHostStep {
        let decision = {
            let entries = self.snapshot.entries();

            let text = self.text_field.text();
            let input = ResolverInput {
                input_text: if text.is_empty() { None } else { Some(text) },
                highlighted_id: self.highlighted_id,
                commit_source: source,
                options: &entries,
            };

            self.resolver.resolve(&input)
        };

        match decision {
            ResolverDecision::Commit(value) => {
                // Reset the real child before publishing the resolver commit.
                let display_text = match &value {
                    CommittedValue::ExistingOption(id) => self
                        .snapshot
                        .items
                        .iter()
                        .find(|(opt_id, _)| *opt_id == *id)
                        .map(|(_, label)| label.clone()),
                    CommittedValue::CustomValue(text) => Some(text.clone()),
                };
                if let Some(text) = display_text {
                    return CompositeHostStep::Final {
                        operation: CompositeEditOperation::ResetAfterResolverCommit {
                            text,
                            select_all: false,
                        },
                        continuation: CompositeHostContinuation::Combo(
                            CompositeComboContinuation::Commit { value },
                        ),
                    };
                }
                self.finish_commit(value, cx);
            }
            ResolverDecision::Reject(_) => {}
        }
        CompositeHostStep::Done { handled: true }
    }

    fn finish_commit(&mut self, value: CommittedValue, cx: &mut EventContext) {
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        {
            self.host_selected_option = match &value {
                CommittedValue::ExistingOption(id) => self
                    .text_field
                    .snapshot
                    .as_ref()
                    .map(|snapshot| HostSelectedOption::Observed(*id, snapshot.revision)),
                CommittedValue::CustomValue(_) => None,
            };
        }
        self.committed_latch = Some(value);
        self.set_popup_open(false, true);
        self.highlighted_id = None;
        self.highlight_from_hover = false;
        self.popup_scroll_row = 0;
        self.reset_popup_scrollbar();
        self.option_press_active = false;
        self.set_ghost_completion(None);
        // SPEC-HAL-328 §3.8: widget-initiated capture release on commit.
        cx.release_keyboard_capture();
        if let Some(cb) = self.on_committed.as_ref() {
            // The one callback slot fires at the original commit point. Factory
            // values come from the current mapping, never construction captures.
            let committed = self.committed_latch.as_ref().unwrap();
            match cb {
                CommitCallback::Public(cb) => cb(committed),
                CommitCallback::FactoryCurrent(cb) => {
                    let value = match committed {
                        CommittedValue::ExistingOption(id) => self
                            .option_values
                            .iter()
                            .find(|(option_id, _)| option_id == id)
                            .map(|(_, value)| value.clone()),
                        CommittedValue::CustomValue(text) => Some(text.clone()),
                    };
                    if let Some(value) = value {
                        cb(value);
                    }
                }
            }
        }
    }

    /// SPEC-HAL-201 §6.2: Close without commit.
    /// Triggered by: Escape (when open), blur/focus-loss.
    fn close_without_commit(&mut self) {
        let was_open = self.popup_open.get();
        if was_open {
            self.set_popup_open(false, true);
        }
        self.highlighted_id = None; // popup-transient, cleared
        self.highlight_from_hover = false;
        self.popup_scroll_row = 0;
        self.reset_popup_scrollbar();
        self.option_press_active = false;
        self.set_ghost_completion(None);
        // TextField text PRESERVED (control-surface state, not popup-transient)
        // Fire on_cancelled only when the popup was actually open AND no
        // commit is pending — avoids double-fire during post-commit tear-down.
        if was_open && self.committed_latch.is_none() {
            if let Some(cb) = self.on_cancelled.as_ref() {
                cb();
            }
        }
    }

    /// Move highlight to next option in list.
    fn highlight_next(&mut self) {
        if self.snapshot.items.is_empty() {
            return;
        }
        self.highlight_from_hover = false;
        match self.highlighted_id {
            None => {
                self.highlighted_id = Some(self.snapshot.items[0].0);
            }
            Some(current) => {
                if let Some(idx) = self
                    .snapshot
                    .items
                    .iter()
                    .position(|(id, _)| *id == current)
                {
                    let next = (idx + 1).min(self.snapshot.items.len() - 1);
                    self.highlighted_id = Some(self.snapshot.items[next].0);
                } else {
                    self.highlighted_id = Some(self.snapshot.items[0].0);
                }
            }
        }
    }

    /// Move highlight to previous option in list.
    fn highlight_prev(&mut self) {
        if self.snapshot.items.is_empty() {
            return;
        }
        self.highlight_from_hover = false;
        match self.highlighted_id {
            None => {
                self.highlighted_id = Some(self.snapshot.items[self.snapshot.items.len() - 1].0);
            }
            Some(current) => {
                if let Some(idx) = self
                    .snapshot
                    .items
                    .iter()
                    .position(|(id, _)| *id == current)
                {
                    let prev = idx.saturating_sub(1);
                    self.highlighted_id = Some(self.snapshot.items[prev].0);
                } else {
                    self.highlighted_id =
                        Some(self.snapshot.items[self.snapshot.items.len() - 1].0);
                }
            }
        }
    }

    /// Derive ambient input background state from current widget state.
    fn input_background_state(&self) -> InputBgState {
        if self.loading {
            return InputBgState::Loading;
        }
        // Browsing/Custom/NoMatch are interactive feedback shown DURING
        // popup browsing. Once the popup is closed (committed state), the
        // value area should look identical to a TextField — Default.
        if !self.popup_open.get() {
            return InputBgState::Default;
        }
        let text = self.text_field.text();
        if text.is_empty() {
            return InputBgState::Default;
        }
        if !self.snapshot.items.is_empty() {
            InputBgState::Browsing
        } else if self.allow_custom_value {
            InputBgState::Custom
        } else {
            InputBgState::NoMatch
        }
    }

    /// Whether popup has content to display.
    /// False when no matching items exist or when async search is loading.
    fn has_popup_content(&self) -> bool {
        !self.loading && !self.snapshot.items.is_empty()
    }

    /// Requested row count before the grouped popup fits the viewport.
    fn visible_rows(&self) -> usize {
        if self.snapshot.items.is_empty() {
            return 0;
        }
        self.snapshot
            .display_len()
            .max(self.snapshot.min_visible_rows)
            .min(self.snapshot.max_visible_rows)
    }

    fn effective_visible_rows(&self) -> usize {
        self.grouped_popup_rows.get().unwrap_or_else(|| self.visible_rows())
    }

    fn input_visible_rows(&self, cx: &EventContext) -> usize {
        if self.snapshot.groups.is_none() {
            return self.visible_rows();
        }
        cx.combo_popup_hit().map_or_else(
            || {
                // Keyboard input can open and navigate before the first overlay.
                let capacity = cx.widget_viewport_rect().map_or(0, |trigger| {
                    PopupGeometry::grouped_row_capacity(trigger, cx.viewport_size().1)
                });
                self.visible_rows().min(capacity)
            },
            |popup| popup.visible_rows,
        )
    }

    fn first_popup_row(&self, visible_rows: usize) -> usize {
        if self.snapshot.groups.is_some() {
            self.popup_scroll_row.min(self.snapshot.display_len().saturating_sub(visible_rows))
        } else {
            self.popup_scroll_row
        }
    }

    /// Ensure the highlighted item is within the visible scroll window.
    fn ensure_highlight_visible(&mut self, cx: &mut EventContext) {
        let Some(id) = self.highlighted_id else {
            return;
        };
        let Some(idx) = self
            .snapshot
            .items
            .iter()
            .position(|(item_id, _)| *item_id == id)
        else {
            return;
        };
        self.ensure_popup_item_visible(idx, cx);
    }

    fn ensure_popup_item_visible(&mut self, item_index: usize, cx: &mut EventContext) {
        let idx = self.snapshot.display_row_for_item(item_index);
        let visible_rows = self.input_visible_rows(cx);
        if visible_rows == 0 && self.snapshot.groups.is_some() {
            return;
        }
        let first = self.first_popup_row(visible_rows);
        if idx < first {
            self.scroll_popup_to(idx, cx);
        } else if idx >= first + visible_rows {
            self.scroll_popup_to(idx + 1 - visible_rows, cx);
        }
    }

    /// Scroll eligibility is determined by overflow.
    fn needs_scrollbar(&self) -> bool {
        let visible = self.effective_visible_rows();
        (self.snapshot.groups.is_none() || visible > 0) && self.snapshot.display_len() > visible
    }

    fn reset_popup_scrollbar(&mut self) {
        if self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide {
            self.scrollbar_v.end_drag();
            self.scrollbar_hold_remaining = 0.0;
            self.scrollbar_fade_alpha = 0.0;
            self.scrollbar_activity = false;
            self.option_press_active = false;
        } else {
            self.scrollbar_v = ScrollbarV::new();
        }
    }

    /// Keep the existing popup border inset and ScrollbarV's geometry authority.
    fn popup_scrollbar_layout(&self, popup: RectF) -> Option<ScrollbarLayout> {
        const BORDER: f32 = 1.0;
        let width = popup.width - BORDER;
        let height = popup.height - BORDER * 2.0;
        if !self.needs_scrollbar() || width <= 0.0 || height <= 0.0 {
            return None;
        }
        if (self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide
            || self.snapshot.groups.is_some())
            && width < self.scrollbar_v.config.thickness
        {
            return None;
        }
        Some(
            self.scrollbar_v
                .layout(
                    RectF::new(0.0, 0.0, width, height),
                    height,
                    self.snapshot.display_len() as f32 * ROW_HEIGHT,
                    self.effective_visible_rows() as f32 * ROW_HEIGHT,
                    self.first_popup_row(self.effective_visible_rows()) as f32 * ROW_HEIGHT,
                )
                .translated(popup.x, popup.y + BORDER),
        )
    }

    fn popup_scrollbar_colors(&self, theme: &Theme, fade_alpha: f32) -> (Color, Color) {
        let track = theme.colors.scrollbars.track;
        let thumb = self.scrollbar_v.thumb_color(&theme.colors);
        let appearance = self.scrollbar_appearance;
        let thumb_opacity = match self.scrollbar_v.state {
            ScrollbarState::Normal => appearance.thumb_opacity,
            ScrollbarState::Hovered | ScrollbarState::Pressed => appearance.thumb_hover_opacity,
        };
        (
            track.with_alpha(track.a * appearance.track_opacity * fade_alpha),
            thumb.with_alpha(thumb.a * thumb_opacity * fade_alpha),
        )
    }

    fn refresh_popup_scrollbar(&mut self) -> bool {
        if self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide
            && self.popup_open.get()
            && self.visible_rows() > 0
            && (self.needs_scrollbar()
                || (self.snapshot.groups.is_some() && self.grouped_popup_rows.get().is_none()))
        {
            self.scrollbar_hold_remaining = self.scrollbar_interaction.hold_seconds;
            self.scrollbar_fade_alpha = 1.0;
            self.scrollbar_activity = true;
            true
        } else {
            false
        }
    }

    fn hold_popup_scrollbar(&mut self, cx: &mut EventContext) {
        if self.refresh_popup_scrollbar() {
            cx.invalidate_paint();
        }
    }

    fn scroll_popup_to(&mut self, row: usize, cx: &mut EventContext) {
        let row = if self.snapshot.groups.is_some() {
            row.min(self.snapshot.display_len().saturating_sub(self.input_visible_rows(cx)))
        } else {
            row
        };
        if self.popup_scroll_row != row {
            self.popup_scroll_row = row;
            self.hold_popup_scrollbar(cx);
            if self.snapshot.groups.is_some() {
                cx.invalidate_paint();
            }
        }
    }

    fn set_popup_scrollbar_hovered(&mut self, hovered: bool, cx: &mut EventContext) {
        if self.scrollbar_interaction.display_mode != PopupScrollbarPolicy::AutoHide
            || self.scrollbar_v.dragging
        {
            return;
        }
        let state = if hovered {
            ScrollbarState::Hovered
        } else {
            ScrollbarState::Normal
        };
        if self.scrollbar_v.state != state {
            self.scrollbar_v.state = state;
            self.hold_popup_scrollbar(cx);
        }
    }

    fn update_popup_scrollbar_hover(&mut self, pe: &PointerEvent, cx: &mut EventContext) {
        if self.scrollbar_interaction.display_mode != PopupScrollbarPolicy::AutoHide
            || pe.pointer_type == PointerType::Touch
            || self.option_press_active
        {
            return;
        }
        let hovered = cx.combo_popup_hit().is_some_and(|popup| {
            popup.scrollbar.is_some_and(|bar| {
                bar.layout.hit_rect.contains(popup.scrollbar_point(pe.global_pos))
            })
        });
        self.set_popup_scrollbar_hovered(hovered, cx);
    }

    fn press_popup_scrollbar(
        &mut self,
        popup: ComboPopupInputGeometry,
        pe: &PointerEvent,
        cx: &mut EventContext,
    ) -> bool {
        let Some(layout) = popup.drawn_scrollbar() else {
            return false;
        };
        let point = popup.scrollbar_point(pe.global_pos);
        if !layout.hit_rect.contains(point) {
            return false;
        }
        let visible = popup.visible_rows;
        let first = if self.snapshot.groups.is_some() { popup.first_row } else { self.popup_scroll_row };
        let max_scroll_rows = self.snapshot.display_len().saturating_sub(visible);
        if layout.thumb.contains(point) {
            let scroll_y = popup.scroll_offset.1;
            self.scrollbar_v.begin_drag(
                pe.global_pos.1,
                first as f32 * ROW_HEIGHT,
                layout.track.y - scroll_y,
                layout.track.bottom() - scroll_y,
                layout.thumb.height,
                max_scroll_rows as f32 * ROW_HEIGHT,
            );
            cx.capture_pointer(PointerButton::Primary);
            self.hold_popup_scrollbar(cx);
        } else {
            let row = if point.y < layout.thumb.y {
                first.saturating_sub(visible)
            } else {
                (first + visible).min(max_scroll_rows)
            };
            self.scroll_popup_to(row, cx);
        }
        true
    }

    fn popup_row_hit(
        &self,
        popup: Option<ComboPopupInputGeometry>,
        pe: &PointerEvent,
    ) -> Option<ChoiceDisplayRow> {
        self.popup_row_at(popup, pe.local_pos, pe.global_pos)
    }

    pub(crate) fn popup_inert_press_at(
        &self,
        popup: Option<ComboPopupInputGeometry>,
        local: (f32, f32),
        global: (f32, f32),
    ) -> bool {
        popup.is_some_and(|popup| {
            popup.footer.is_some_and(|footer| footer.contains_point(local.0, local.1))
        }) || (self.snapshot.groups.is_some()
            && matches!(
                self.popup_row_at(popup, local, global),
                Some(ChoiceDisplayRow::Header(_))
            ))
    }

    fn popup_row_at(
        &self,
        popup: Option<ComboPopupInputGeometry>,
        local: (f32, f32),
        global: (f32, f32),
    ) -> Option<ChoiceDisplayRow> {
        let popup = popup?;
        let popup_rect = popup.rows;
        let (local_x, local_y) = local;
        if !(local_x >= popup_rect.x
            && local_x < popup_rect.right()
            && local_y >= popup_rect.y
            && local_y < popup_rect.bottom())
        {
            return None;
        }
        let local_row = ((local_y - popup_rect.y) / ROW_HEIGHT) as usize;
        if local_row >= popup.visible_rows {
            return None;
        }
        let first = if self.snapshot.groups.is_some() { popup.first_row } else { self.popup_scroll_row };
        let row = self.snapshot.display_row(first + local_row)?;
        if (self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide
            || matches!(row, ChoiceDisplayRow::Header(_)))
            && popup
                .drawn_scrollbar()
                .is_some_and(|layout| layout.hit_rect.contains(popup.scrollbar_point(global)))
        {
            return None;
        }
        Some(row)
    }

    fn refresh_ghost_completion(&mut self, query: &str) {
        let suffix = self
            .prefix_preview
            .ghost_suffix(&self.snapshot.items, query, self.highlighted_id);
        self.set_ghost_completion(suffix);
    }

    /// Set the callback fired when a value commits successfully via the
    /// internal resolver. The argument is the committed value
    /// (`ExistingOption` or `CustomValue`).
    pub fn set_on_committed<F: Fn(&CommittedValue) + 'static>(&mut self, f: F) {
        self.on_committed = Some(CommitCallback::Public(Box::new(f)));
    }

    pub(crate) fn set_factory_value_callback<F: Fn(String) + 'static>(&mut self, f: F) {
        self.on_committed = Some(CommitCallback::FactoryCurrent(Box::new(f)));
    }

    /// Set the callback fired when the popup closes without a commit
    /// (Escape, blur, or focus-loss). Not fired if a commit is already
    /// pending in `committed_latch`.
    pub fn set_on_cancelled<F: Fn() + 'static>(&mut self, f: F) {
        self.on_cancelled = Some(Box::new(f));
    }

    #[cfg(test)]
    pub(crate) fn test_highlighted_label(&self) -> Option<&str> {
        let highlighted_id = self.highlighted_id?;
        self.snapshot
            .items
            .iter()
            .find(|(id, _)| *id == highlighted_id)
            .map(|(_, label)| label.as_str())
    }

    #[cfg(test)]
    pub(crate) fn test_ghost_completion(&self) -> Option<&str> {
        self.ghost_completion.as_deref()
    }

    #[cfg(test)]
    pub(crate) fn test_popup_local_rect(
        tree: &crate::core::widget_tree::WidgetTree,
        node: NodeId,
    ) -> RectF {
        tree.combo_popup_input_geometry(tree.node(node).unwrap().id)
            .map_or(RectF::default(), |popup| popup.rows)
    }
}

impl WidgetDowncast for ComboBox {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl ComboBox {
    fn paint_group_footer(
        &self,
        cx: &mut PaintContext,
        footer: RectF,
        first_row: usize,
        colors: &GroupedPopupColors,
        edge: f32,
    ) {
        let Some((label, count)) = self.snapshot.groups.as_ref()
            .and_then(|groups| groups.context_at(first_row))
        else {
            return;
        };
        cx.scene.begin_clip(footer, None);
        cx.scene.push_rect(footer, colors.footer, POPUP_ROW_LAYER, None);
        let width = (footer.width - 2.0 * edge).max(0.0);
        let rule = RectF::new(footer.x + edge, footer.y, width, cx.one_px().min(footer.height));
        cx.scene.push_rect(rule, colors.rule, POPUP_MATCH_LAYER, None);

        let size = cx.theme.typography.font_size * GROUP_HEADER_FONT_SCALE;
        let padding = (cx.theme.geometry.text_field.padding_x + GROUP_HEADER_INSET).max(edge);
        let left = footer.x + padding;
        let count = cx.text_engine.shape(&count.to_string(), size);
        let count_x = footer.right() - padding - count.width;
        if count_x >= left {
            let center_y = footer.y + footer.height * 0.5;
            cx.scene.push_text_with_backgrounds(
                &count, count_x, center_y - count.height * 0.5, colors.text,
                vec![], POPUP_TEXT_LAYER, None,
            );
            if let Some(title) = shape_group_heading(cx.text_engine, label, size, count_x - padding - left) {
                cx.scene.push_text_with_backgrounds(
                    &title, left, center_y - title.height * 0.5, colors.text,
                    vec![], POPUP_TEXT_LAYER, None,
                );
            }
        }
        cx.scene.end_clip();
    }

    fn frosted_popup_visible(&self) -> bool {
        self.frosted_glass.is_some()
            && (self.popup_open.get() || self.popup_presentation.reveal > 0.0)
            && self.visible_rows() > 0
    }

    fn paint_frosted_control(&self, cx: &mut PaintContext, rect: RectF, layer: ZIndex) {
        let glass = self.frosted_glass.as_ref().expect("admitted glass value");
        let scale = cx.scale_factor;
        let x = snap_to_physical(rect.x, scale);
        let y = snap_to_physical(rect.y, scale);
        let snapped = RectF::new(
            x,
            y,
            snap_to_physical(rect.right(), scale) - x,
            snap_to_physical(rect.bottom(), scale) - y,
        );
        let radius = self
            .input_surface_appearance
            .as_ref()
            .unwrap()
            .radius()
            .min(snapped.width.max(0.0) * 0.5)
            .min(snapped.height.max(0.0) * 0.5);
        cx.scene
            .push_frosted_glass(snapped, RoundedCorners::uniform(radius), glass, layer, None)
            .expect("admitted frosted ComboBox geometry/sink");
    }

    fn paint_control(&self, cx: &mut PaintContext, rect: RectF, layer_offset: ZIndex) {
        let t = cx.theme;
        let aw = self.affordance_width;
        let cell_integrated = cx.cell_editor_presentation()
            == crate::core::appearance::CellEditorPresentation::CellIntegrated;
        let selected = self
            .input_surface_appearance
            .as_ref()
            .filter(|_| !cell_integrated);
        let chrome_state = if !self.enabled {
            ControlChromeState::Disabled
        } else if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = t.input_chrome(chrome_state);
        let chrome = style.chrome;

        // 1. Unified background + flat recessed border-shift shell.
        // ComboBox keeps its semantic input-state fills as a documented
        // RecessedEditable exception.
        let bg_color = match self.input_background_state() {
            InputBgState::Default => chrome.fill.unwrap_or(t.colors.surfaces.content),
            InputBgState::Browsing => style.browsing,
            InputBgState::Custom => t.colors.combo_box_input.custom,
            InputBgState::NoMatch => t.colors.combo_box_input.no_match,
            InputBgState::Loading => t.colors.combo_box_input.loading,
        };

        // SPEC-HAL-332 ControlChrome: no sculpted shadow/highlight bevel.
        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let bx = snap_to_physical(rect.x, scale);
        let by = snap_to_physical(rect.y, scale);
        let br = snap_to_physical(rect.x + rect.width, scale);
        let bb = snap_to_physical(rect.y + rect.height, scale);
        let bw = br - bx;
        let bh = bb - by;
        let snapped = RectF::new(bx, by, bw, bh);
        let radius = selected.map_or_else(
            || {
                t.chrome_radius(
                    ControlChromeCategory::RecessedEditable,
                    t.geometry.combo_box.radius,
                    false,
                )
            },
            |value| value.radius().min(bw.max(0.0) * 0.5).min(bh.max(0.0) * 0.5),
        );
        let edge_thick = if cx.has_capture() || cx.has_focus_cue() {
            (chrome.border_width * scale).round().max(1.0) / scale
        } else {
            one_px
        };

        let joined = (self.popup_open.get() || self.popup_presentation.reveal > 0.0)
            .then(|| {
                let geometry = self.popup_geometry(cx, rect, self.paint_visible_rows(cx, rect));
                if self.snapshot.groups.is_none() {
                    geometry.snapped(scale)
                } else {
                    geometry
                }
            })
            .filter(|geometry| geometry.popup.height > 0.0);
        let surface_corners = joined.map_or(RoundedCorners::uniform(radius), |geometry| {
            geometry.corners(snapped, radius, edge_thick)
        });
        if cell_integrated {
            cx.scene
                .push_rect(snapped, bg_color, layer_offset + 4, None);
        } else if let Some(value) = selected {
            let popup_visible = self.popup_open.get() || self.popup_presentation.reveal > 0.0;
            let prefer_hover_surface = self.hover_surface_on_closed_focus
                && self.cell_hovered
                && !popup_visible
                && chrome_state == ControlChromeState::Focused
                && cx.appearance_facts().is_some_and(|facts| {
                    facts.focused() && !facts.keyboard_captured() && !facts.pointer_captured()
                });
            let use_active_surface = matches!(
                chrome_state,
                ControlChromeState::Captured | ControlChromeState::Focused
            ) && !prefer_hover_surface;
            let surface = value.surface_for_interaction(
                self.cell_hovered,
                use_active_surface,
                popup_visible,
            );
            let border = selected_border(value, chrome_state, chrome.border_color);
            let border_layer = if cx.has_capture() || cx.has_focus_cue() {
                (layer_offset + LAYER_FOCUS) + 1
            } else {
                (layer_offset + LAYER_CONTENT) + 4
            };
            if surface
                .definition()
                .stops
                .iter()
                .any(|stop| stop.color.a > 0.0)
            {
                let (clip_rect, clip_corners) = joined
                    .and_then(|geometry| geometry.joined_surface_clip(radius, edge_thick))
                    .unwrap_or((snapped, surface_corners));
                cx.scene.begin_clip(clip_rect, Some(clip_corners));
                if matches!(
                    self.input_background_state(),
                    InputBgState::Default | InputBgState::Browsing
                ) {
                    cx.scene.push_gradient_rect(
                        snapped,
                        0.0,
                        surface.definition(),
                        layer_offset + LAYER_CONTENT,
                        cx.scene_key,
                    );
                } else {
                    // A present surface retains validation/loading feedback.
                    cx.scene.push_rect(
                        snapped,
                        bg_color,
                        layer_offset + LAYER_CONTENT,
                        cx.scene_key,
                    );
                }
                cx.scene.end_clip();
            }
            if let Some(geometry) = joined {
                cx.scene.begin_clip(snapped, None);
                if value.inner_edge().a > 0.0 {
                    cx.scene.push_stroked_path(
                        geometry.outline(radius, edge_thick * 2.0 + one_px),
                        StrokeParams {
                            width: one_px.min(bh),
                            ..Default::default()
                        },
                        value.inner_edge(),
                        (layer_offset + LAYER_CONTENT) + 1,
                        None,
                    );
                    if let Some(divider) = geometry.input_divider(edge_thick, one_px) {
                        cx.scene.push_rect(
                            divider,
                            value.inner_edge(),
                            (layer_offset + LAYER_CONTENT) + 1,
                            None,
                        );
                    }
                }
                if border.a > 0.0 {
                    cx.scene.push_stroked_path(
                        geometry.outline(radius, edge_thick),
                        StrokeParams {
                            width: edge_thick.min(geometry.popup.height),
                            ..Default::default()
                        },
                        border,
                        border_layer,
                        None,
                    );
                }
                cx.scene.end_clip();
            } else {
                let inner = RectF::new(
                    bx + edge_thick,
                    by + edge_thick,
                    (bw - edge_thick * 2.0).max(0.0),
                    (bh - edge_thick * 2.0).max(0.0),
                );
                if value.inner_edge().a > 0.0 {
                    cx.scene.push_border_rounded_rect(
                        inner,
                        value.inner_edge(),
                        one_px,
                        RoundedCorners::uniform((radius - edge_thick).max(0.0)),
                        (layer_offset + LAYER_CONTENT) + 1,
                        None,
                    );
                }
                if border.a > 0.0 {
                    cx.scene.push_border_rounded_rect(
                        snapped,
                        border,
                        edge_thick,
                        RoundedCorners::uniform(radius),
                        border_layer,
                        None,
                    );
                }
            }
        } else if let Some(geometry) = joined {
            cx.scene.begin_clip(snapped, None);
            cx.scene.push_path(
                geometry.fill_outline(radius, edge_thick),
                bg_color,
                layer_offset + 2,
                cx.scene_key,
            );
            cx.scene.push_stroked_path(
                geometry.outline(radius, edge_thick),
                StrokeParams {
                    width: edge_thick.min(geometry.popup.height),
                    ..Default::default()
                },
                chrome.border_color,
                layer_offset + 3,
                None,
            );
            if let Some(divider) = geometry.input_divider(edge_thick, one_px) {
                let neutral = t.input_chrome(ControlChromeState::Idle).chrome.border_color;
                let mut path = Path::new();
                path.move_to(divider.x, divider.y);
                path.line_to(divider.right(), divider.y);
                path.line_to(divider.right(), divider.bottom());
                path.line_to(divider.x, divider.bottom());
                path.close();
                cx.scene
                    .push_path(path, neutral, (layer_offset + LAYER_CONTENT) + 1, None);
            }
            cx.scene.end_clip();
        } else {
            cx.scene.push_rounded_rect(
                snapped,
                chrome.border_color,
                RoundedCorners::uniform(radius),
                layer_offset + 2,
                None,
            );
            let inset_rect = RectF::new(
                bx + edge_thick,
                by + edge_thick,
                (bw - edge_thick * 2.0).max(0.0),
                (bh - edge_thick * 2.0).max(0.0),
            );
            cx.scene.push_rounded_rect(
                inset_rect,
                style.inner_edge,
                RoundedCorners::uniform((radius - edge_thick).max(0.0)),
                layer_offset + 3,
                cx.scene_key,
            );
            let inset = edge_thick + one_px;
            cx.scene.push_rounded_rect(
                RectF::new(
                    bx + inset,
                    by + inset,
                    (bw - 2.0 * inset).max(0.0),
                    (bh - 2.0 * inset).max(0.0),
                ),
                bg_color,
                RoundedCorners::uniform((radius - inset).max(0.0)),
                layer_offset + 4,
                None,
            );
        }

        let inset = if cell_integrated {
            0.0
        } else {
            edge_thick + one_px
        };
        let left = snap_to_physical(rect.right() - aw, scale);
        let inner_radius = if cell_integrated {
            0.0
        } else {
            (radius - inset).max(0.0)
        };
        let affordance = RectF::new(
            left,
            by + inset,
            (br - inset - left).max(0.0),
            (bh - 2.0 * inset).max(0.0),
        );
        if let Some(value) = selected {
            if value.affordance().a > 0.0 {
                let inner = RectF::new(
                    bx + inset,
                    by + inset,
                    (bw - inset * 2.0).max(0.0),
                    (bh - inset * 2.0).max(0.0),
                );
                let corners = RoundedCorners {
                    tl: (surface_corners.tl - inset).max(0.0),
                    tr: (surface_corners.tr - inset).max(0.0),
                    bl: (surface_corners.bl - inset).max(0.0),
                    br: (surface_corners.br - inset).max(0.0),
                };
                cx.scene.begin_clip(inner, Some(corners));
                cx.scene.push_rect(
                    affordance,
                    value.affordance(),
                    (layer_offset + LAYER_CONTENT) + 3,
                    None,
                );
                cx.scene.end_clip();
            }
        } else {
            // Clip the supported uniform rounding to keep the joined left seam flush.
            cx.scene.begin_clip(affordance, None);
            cx.scene.push_rounded_rect(
                RectF::new(
                    left - inner_radius,
                    by + inset,
                    affordance.width + inner_radius,
                    (bh - 2.0 * inset).max(0.0),
                ),
                style.affordance,
                RoundedCorners::uniform(inner_radius),
                (layer_offset + LAYER_CONTENT) + 3,
                None,
            );
            cx.scene.end_clip();
        }

        // 2. Text area.
        let leading_width = self.leading_width;
        let text_rect = RectF::new(
            rect.x + leading_width,
            rect.y,
            (rect.width - aw - leading_width).max(0.0),
            rect.height,
        );
        if let Some(definition) = self.leading_icon.and_then(|id| cx.icon_registry.get(id)) {
            let padding = t.geometry.text_field.padding_x;
            let size = self
                .leading_icon_size
                .map_or(t.typography.input_font_size, |_| {
                    (leading_width - padding).max(0.0)
                })
                .min((rect.height - t.geometry.text_field.padding_y * 2.0).max(0.0))
                .min((rect.width - aw - padding * 2.0).max(0.0));
            if size > 0.0 {
                let centered_top = (rect.height - size) * 0.5;
                let clearance = centered_top.max(0.0);
                let offset = self.leading_icon_y_offset.clamp(-clearance, clearance);
                cx.scene.push_icon(
                    definition,
                    snap_to_physical(rect.x + padding, scale),
                    snap_to_physical(rect.y + centered_top + offset, scale),
                    size,
                    t.colors.typography.primary,
                    (layer_offset + LAYER_CONTENT) + 2,
                );
            }
        }
        if !self.text_input_enabled {
            let padding_x = t.geometry.text_field.padding_x;
            let text_viewport = RectF::new(
                text_rect.x + padding_x,
                text_rect.y,
                (text_rect.width - padding_x * 2.0).max(0.0),
                text_rect.height,
            );
            cx.scene.begin_clip(text_viewport, None);
            let value = self.text_field.text();
            if !value.is_empty() {
                let font_size = t.typography.font_size;
                let metrics = cx.text_engine.font_metrics(font_size);
                let baseline_y = rect.y
                    + compute_control_text_baseline_y(
                        rect.height,
                        metrics.ascent,
                        metrics.descent,
                        t.typography.baseline_offset,
                    );
                let text_y = baseline_y - metrics.ascent;
                let run = cx.text_engine.shape(value, font_size);
                cx.text_engine.draw(
                    &run,
                    text_viewport.x,
                    text_y,
                    t.colors.typography.primary,
                    (layer_offset + LAYER_CONTENT) + 2,
                    cx.scene,
                );
            }
            cx.scene.end_clip();
        }

        // 2b. Ghost completion (visual only — muted suffix after typed text, not part of value)
        if self.popup_open.get() {
            if let Some(ref ghost) = self.ghost_completion {
                let font_size = if self.text_input_enabled {
                    t.typography.input_font_size
                } else {
                    t.typography.font_size
                };
                let text_padding = t.geometry.text_field.padding_x;
                let input_run = cx.text_engine.shape(self.text_field.text(), font_size);
                let ghost_run = cx.text_engine.shape(ghost, font_size);
                let ghost_x = text_rect.x + text_padding + input_run.width;
                let ghost_y = rect.y + (rect.height - ghost_run.height) / 2.0;
                cx.scene.push_text_with_backgrounds(
                    &ghost_run,
                    ghost_x,
                    ghost_y,
                    t.colors.typography.muted,
                    vec![],
                    (layer_offset + LAYER_CONTENT) + 2,
                    None,
                );
            }
        }

        // 3. Subtle separator between text area and affordance
        // SPEC-HAL-093 v3: 1-physical-pixel thickness, snapped to physical pixel grid.
        //
        // The input face is at Z4, its compartment at Z5, and its details at Z6.
        // Strict order keeps these visible across clip and pipeline boundaries.
        let affordance_layer = (layer_offset + LAYER_CONTENT) + 4;
        let sep_x = snap_to_physical(rect.x + rect.width - aw, scale);
        // Keep the divider inside the editable body. When focused/captured,
        // the outer shell thickens; a full-height separator would cut through
        // that focus edge and make the control look unfinished.
        let sep_inset = edge_thick + 2.0 * one_px;
        let sep_top = by + sep_inset;
        let sep_bottom = bb - sep_inset;
        let separator = selected.map_or(style.inner_edge, InputSurfaceAppearance::inner_edge);
        if selected.is_none() || separator.a > 0.0 {
            cx.scene.push_rounded_rect(
                RectF::new(sep_x, sep_top, one_px, (sep_bottom - sep_top).max(0.0)),
                separator,
                RoundedCorners::uniform(0.5),
                affordance_layer,
                None,
            );
        }

        // 4. Chevron centered in the existing affordance.
        let aff_rect = RectF {
            x: rect.x + rect.width - aw,
            y: rect.y,
            width: aw,
            height: rect.height,
        };
        let indicator_color = t.colors.typography.primary;
        let center_x = aff_rect.x + aff_rect.width * 0.5;
        let center_y = aff_rect.y + aff_rect.height * 0.5;
        let direction = if self.popup_open.get() { -1.0 } else { 1.0 };
        // An explicitly absent perimeter uses the existing caret as the one
        // current-owner cue. It never clears or mirrors framework authority.
        let borderless_cue = !cell_integrated
            && selected.is_some_and(|value| {
                value.focus_colors().is_some()
                    && selected_border(value, chrome_state, chrome.border_color).a == 0.0
            });
        let indicator_width = if borderless_cue {
            match chrome_state {
                ControlChromeState::Focused => 1.875,
                ControlChromeState::Captured => 2.5,
                _ => 1.25,
            }
        } else {
            1.25
        };
        let stroke = StrokeParams {
            width: indicator_width,
            ..Default::default()
        };
        let default_extents = Size::new(3.0, 1.5);
        let extents = self.chevron_half_extents.map_or(default_extents, |requested| {
            // Butt caps extend at most half a stroke horizontally. Only
            // the vertical tip can extend as far as the miter limit.
            let half_width = center_x - aff_rect.x.max(rect.x) - stroke.width * 0.5;
            let half_height = aff_rect.height * 0.5 - stroke.width * stroke.miter_limit;
            if half_width > 0.0 && half_height > 0.0 {
                Size::new(requested.width.min(half_width), requested.height.min(half_height))
            } else {
                default_extents
            }
        });
        let tip = direction * extents.height;
        let mut arrow = Path::new();
        arrow.move_to(center_x - extents.width, center_y - tip);
        arrow.line_to(center_x, center_y + tip);
        arrow.line_to(center_x + extents.width, center_y - tip);
        cx.scene.push_stroked_path(
            arrow,
            stroke,
            indicator_color,
            affordance_layer,
            None,
        );

        // The selected perimeter or caret above is the complete owner cue.
    }
}

impl Widget for ComboBox {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let affordance_width = cx.theme.geometry.combo_box.affordance_width;
        if cx.control_parts_request() == Some(crate::core::layout::ControlPartsRequest::Measure) {
            let geom = cx.theme.geometry.text_field;
            let metrics = cx
                .text_engine
                .font_metrics(cx.theme.typography.input_font_size);
            let line_height = crate::core::text_engine::compute_control_line_height(
                cx.theme.typography.line_height,
                &metrics,
                cx.theme.typography.baseline_offset,
            );
            let height = geom.padding_y * 2.0 + line_height;
            let leading_width =
                self.leading_width_for(Size::new(f32::INFINITY, height), cx.theme);
            let req = crate::core::layout::ControlPartsRequirements {
                leading: Default::default(),
                trailing: Default::default(),
                body_width: geom.padding_x * 2.0 + MIN_TEXT_AREA + affordance_width + leading_width,
                height,
            };
            cx.report_control_parts(req);
            return LayoutResult::fit(Size::new(req.body_width, req.height));
        }
        self.affordance_width = affordance_width;
        self.input_font_size = cx.theme.typography.input_font_size;
        let measured_leading_width = self.leading_width_for(
            Size::new(constraints.max_width, constraints.max_height),
            cx.theme,
        );
        let text_constraints = Constraints {
            max_width: (constraints.max_width - affordance_width - measured_leading_width).max(0.0),
            ..constraints
        };
        let result = self
            .text_field
            .layout(cx, text_constraints, measured_leading_width, 0.0);

        // ComboBox natural width: measure widest option to determine
        // the minimum text area needed. This is the PREFERRED width
        // for Flex measurement — Flex grow can expand it further.
        let text_padding = cx.theme.geometry.text_field.padding_x;
        let font_size = cx.theme.typography.font_size;

        let widest_item = self.snapshot.items.iter()
            .map(|(_, label)| cx.text_engine.measure(label, font_size).0)
            .fold(0.0_f32, f32::max);
        let widest_option_width = self.snapshot.groups.as_ref().map_or(widest_item, |groups| {
            groups.boundaries().iter()
                .map(|(_, title)| cx.text_engine.measure(title, font_size * GROUP_HEADER_FONT_SCALE).0 + GROUP_HEADER_INSET)
                .fold(widest_item + GROUP_ITEM_INSET, f32::max)
        });

        // Also consider the current input text width
        let input_text = self.text_field.text();
        let input_width = if !input_text.is_empty() {
            let input_font_size = if self.text_input_enabled {
                self.input_font_size
            } else {
                font_size
            };
            let (w, _) = cx.text_engine.measure(input_text, input_font_size);
            w
        } else {
            0.0
        };

        let content_width = widest_option_width.max(input_width);
        self.widest_option_px.set(widest_option_width);

        // Natural width = text padding (both sides) + widest text + affordance.
        // A dropdown fills the width its parent gives it. When the parent
        // supplies a bounded width (the common case — form rows, property
        // cells, flex cells), the control fills that width; the intrinsic
        // content width is used only when the available width is unbounded.
        // Filling also keeps the control width stable across popup open/close
        // and focus changes, instead of collapsing to content between states.
        let text_area = content_width.max(MIN_TEXT_AREA);
        let leading_width =
            self.leading_width_for(Size::new(f32::INFINITY, result.size.height), cx.theme);
        let intrinsic_width = text_padding * 2.0 + text_area + affordance_width + leading_width;
        let natural_width = if constraints.max_width.is_finite() {
            constraints.max_width.max(constraints.min_width)
        } else {
            intrinsic_width.max(constraints.min_width)
        };
        let leading_width =
            self.leading_width_for(Size::new(natural_width, result.size.height), cx.theme);
        self.leading_width = leading_width;

        // After computing the final input field dimensions, re-lay out the
        // embedded TextField with tight constraints reflecting the actual
        // paint area. Ensures the TextField's layout cache is rebuilt at
        // the exact width paint will use, so geometry reads in paint
        // (boundaries, content_width, etc.) cannot find a stale cache.
        let field_width = (natural_width - affordance_width - leading_width).max(0.0);
        let field_constraints = Constraints::tight(Size::new(field_width, result.size.height));
        self.text_field
            .layout(cx, field_constraints, leading_width, 0.0);

        // Sync scrollbar config from theme (INV-SCROLLBAR-GEOM-1: canonical sync point)
        self.scrollbar_v.config.thickness = cx.theme.scrollbar_geom.thickness;
        self.scrollbar_v.config.min_thumb_size = cx.theme.scrollbar_geom.min_thumb_size;
        self.scrollbar_v.config.corner_radius = cx.theme.scrollbar_geom.corner_radius;
        self.scrollbar_v.config.hit_expansion = cx.theme.scrollbar_geom.hit_expansion;
        LayoutResult::fit(Size::new(natural_width, result.size.height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        if self.frosted_glass.is_some() {
            assert_eq!(
                cx.cell_editor_presentation(),
                crate::core::appearance::CellEditorPresentation::Standalone,
                "frosted ComboBox requires its standalone display profile"
            );
            if self.frosted_popup_visible() {
                // One overlay marker owns the joined material; all of this
                // control's display foreground follows it in that same scope.
                return;
            }
            self.paint_frosted_control(cx, rect, LAYER_CONTENT - 1);
        }
        self.paint_control(cx, rect, 0);
    }

    fn paint_overlay(&self, cx: &mut PaintContext, rect: RectF, tree_widget_id: TreeWidgetId) {
        if !self.popup_open.get() && self.popup_presentation.reveal == 0.0 {
            return;
        }
        if self.visible_rows() == 0 {
            return;
        }

        let grouped = self.snapshot.groups.is_some();
        let (scroll_x, scroll_y) = cx.cumulative_scroll_offset_for_paint();
        // Input remains on authoritative layout geometry; visual interpolation
        // is shared with the main fragment only for the painted perimeter.
        let visual_rect = cx.visual_rect_for_paint(tree_widget_id, rect);
        let visible_rows = self.paint_visible_rows(cx, visual_rect);
        if grouped {
            self.grouped_popup_rows.set(Some(visible_rows));
        }
        let first_row = self.first_popup_row(visible_rows);
        let hit_geometry = self.popup_geometry(cx, rect, visible_rows);
        let geometry = if visual_rect == rect {
            hit_geometry
        } else {
            self.popup_geometry(cx, visual_rect, visible_rows)
        };
        let geometry = if grouped { geometry } else { geometry.snapped(cx.scale_factor) };
        let popup_rect = geometry.popup;
        let (body, footer) = geometry.body_and_footer(grouped);
        let (hit_body, hit_footer) = hit_geometry.body_and_footer(grouped);
        let full_popup_height = hit_geometry.full_height;
        let screen_rect = RectF::new(
            rect.x - scroll_x,
            rect.y - scroll_y,
            rect.width,
            rect.height,
        );
        let popup_screen_rect = RectF::new(
            hit_geometry.popup.x - scroll_x,
            hit_geometry.popup.y - scroll_y,
            hit_geometry.popup.width,
            hit_geometry.popup.height,
        );
        let font_size = cx.theme.typography.font_size;
        let text_padding = cx.theme.geometry.text_field.padding_x;
        let list_colors = &cx.theme.colors.list;
        let group_colors = grouped.then(|| GroupedPopupColors::from_theme(cx.theme));
        let auto_hide = self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide;
        let scrollbar_layout = self.popup_scrollbar_layout(body);
        let fade_alpha = if auto_hide {
            self.scrollbar_fade_alpha
        } else {
            1.0
        };
        let (track, thumb) = self.popup_scrollbar_colors(cx.theme, fade_alpha);
        let draw_scrollbar = track.a > 0.0 || thumb.a > 0.0;
        if let Some(revision) = self.popup_input_revision() {
            // Retain the same row window and scrollbar layout used by Paint.
            cx.register_combo_popup_hit(
                OverlayHitRegion {
                    abs_rect: popup_screen_rect,
                    owner_abs_rect: screen_rect,
                    widget_id: tree_widget_id,
                },
                revision,
                ComboPopupInputGeometry {
                    rows: RectF::new(
                        popup_screen_rect.x - screen_rect.x,
                        popup_screen_rect.y - screen_rect.y,
                        popup_screen_rect.width,
                        if grouped { hit_body.height } else { full_popup_height },
                    ),
                    footer: hit_footer.map(|footer| footer.translate(-rect.x, -rect.y)),
                    visible_rows,
                    first_row,
                    scrollbar: scrollbar_layout.map(|layout| ComboPopupScrollbar {
                        layout,
                        drawn: draw_scrollbar,
                    }),
                    scroll_offset: (scroll_x, scroll_y),
                },
            );
        }
        let scrollbar = scrollbar_layout.filter(|_| draw_scrollbar);

        if geometry.popup.height <= 0.0 {
            if self.frosted_popup_visible() {
                self.paint_frosted_control(cx, visual_rect, POPUP_SURFACE_LAYER);
                self.paint_control(cx, visual_rect, LAYER_OVERLAY);
            }
            return;
        }
        let selected = self.input_surface_appearance.as_ref();
        let radius = selected.map_or_else(
            || {
                cx.theme.chrome_radius(
                    ControlChromeCategory::RecessedEditable,
                    cx.theme.geometry.combo_box.radius,
                    false,
                )
            },
            |value| {
                value
                    .radius()
                    .min(geometry.control.width.max(0.0) * 0.5)
                    .min(geometry.control.height.max(0.0) * 0.5)
            },
        );
        let chrome_state = if cx.has_capture() {
            ControlChromeState::Captured
        } else if cx.has_focus_cue() {
            ControlChromeState::Focused
        } else {
            ControlChromeState::Idle
        };
        let style = cx.theme.input_chrome(chrome_state);
        let chrome = style.chrome;
        let popup_surface = selected.map_or(
            cx.theme.colors.surfaces.content,
            InputSurfaceAppearance::popup_surface,
        );
        let popup_highlight =
            selected.map_or(style.affordance, InputSurfaceAppearance::popup_highlight);
        let border = selected.map_or(chrome.border_color, |value| {
            selected_border(value, chrome_state, chrome.border_color)
        });
        let edge = if cx.has_capture() || cx.has_focus_cue() {
            (chrome.border_width * cx.scale_factor).round().max(1.0) / cx.scale_factor
        } else {
            cx.one_px()
        };
        let text_viewport = if grouped {
            let left = popup_rect.x + text_padding.max(edge);
            let right = scrollbar_layout.map_or(popup_rect.right() - edge, |layout| {
                layout.track.x.min(layout.hit_rect.x)
            }) - text_padding;
            Some(RectF::new(left, body.y, (right - left).max(0.0), body.height))
        } else {
            auto_hide.then(|| {
                let right = scrollbar.map_or(popup_rect.right(), |layout| layout.track.x);
                RectF::new(
                    popup_rect.x + text_padding,
                    popup_rect.y,
                    (right - popup_rect.x - 2.0 * text_padding).max(0.0),
                    popup_rect.height,
                )
            })
        };
        let corners = geometry.corners(popup_rect, radius, edge);
        let joined_fill_clip = selected.and_then(|_| geometry.joined_surface_clip(radius, edge));
        let (fill_rect, fill_corners) = joined_fill_clip.unwrap_or((popup_rect, corners));
        if let Some(glass) = &self.frosted_glass {
            let (glass_rect, glass_corners) = joined_fill_clip
                .expect("frosted ComboBox requires an aligned MatchTrigger attachment");
            cx.scene
                .push_frosted_glass(glass_rect, glass_corners, glass, POPUP_SURFACE_LAYER, None)
                .expect("admitted frosted ComboBox geometry/sink");
            self.paint_control(cx, visual_rect, LAYER_OVERLAY);
        }
        // Aligned selected rows share the trigger's exterior coverage. Glyphs
        // and scrollbar chrome retain the popup's exact content/reveal clip.
        cx.scene.begin_clip(fill_rect, Some(fill_corners));
        if joined_fill_clip.is_none() {
            // Keep the default path unchanged. In the selected aligned case,
            // row quads alone own the fill: a hard-clipped Path behind their AA
            // edges would produce a second, wider-looking silhouette.
            cx.scene.push_path(
                geometry.fill_outline(radius, edge),
                popup_surface,
                POPUP_SURFACE_LAYER,
                None,
            );
        }
        for i in 0..visible_rows {
            let Some(row) = self.snapshot.display_row(first_row + i) else {
                break;
            };
            let row_y = body.y + i as f32 * ROW_HEIGHT;

            // Skip rows below animated popup height
            if row_y >= body.bottom() {
                break;
            }

            let (label, text_color, item_index) = match row {
                ChoiceDisplayRow::Header(heading) => (
                    self.snapshot.groups.as_ref().unwrap().title(heading),
                    group_colors.as_ref().unwrap().text,
                    None,
                ),
                ChoiceDisplayRow::Item(item_index) => (
                    self.snapshot.items[item_index].1.as_str(),
                    list_colors.text,
                    Some(item_index),
                ),
            };

            let heading = match row {
                ChoiceDisplayRow::Header(heading) => Some(heading),
                ChoiceDisplayRow::Item(_) => None,
            };
            let inset = if heading.is_some() {
                GROUP_HEADER_INSET
            } else if grouped {
                GROUP_ITEM_INSET
            } else {
                0.0
            };
            let text_x = popup_rect.x + text_padding + inset;
            let run = if heading.is_some() {
                let available = text_viewport.map_or(0.0, |viewport| viewport.right() - text_x);
                shape_group_heading(cx.text_engine, label, font_size * GROUP_HEADER_FONT_SCALE, available)
            } else {
                Some(cx.text_engine.shape(label, font_size))
            };

            let remaining_height = body.bottom() - row_y;
            let row_bg_height = ROW_HEIGHT.min(remaining_height);

            // Text-background quads currently carry only rectangular scissors.
            // Row and match fills use the existing rounded-clip-capable rect
            // path, with explicit strict ordering below the original text.
            let bg_color = if heading.is_some() {
                group_colors.as_ref().unwrap().heading
            } else if item_index
                .is_some_and(|item| self.highlighted_id == Some(self.snapshot.items[item].0))
            {
                if joined_fill_clip.is_some() {
                    // Compose the common tint and its highlight in linear RGBA
                    // before applying the exterior coverage exactly once.
                    let alpha = popup_highlight.a + popup_surface.a * (1.0 - popup_highlight.a);
                    if alpha > 0.0 {
                        popup_surface
                            .lerp(popup_highlight, popup_highlight.a / alpha)
                            .with_alpha(alpha)
                    } else {
                        Color::TRANSPARENT
                    }
                } else {
                    popup_highlight
                }
            } else {
                popup_surface
            };
            cx.scene.push_rect(
                RectF::new(popup_rect.x, row_y, popup_rect.width, row_bg_height),
                bg_color,
                POPUP_ROW_LAYER,
                None,
            );
            if let (Some(heading), Some(colors)) = (heading, &group_colors) {
                let top = row_y.max(popup_rect.y + edge);
                let top = snap_to_physical(top - scroll_y, cx.scale_factor) + scroll_y;
                let bottom = row_y + row_bg_height;
                let x = popup_rect.x + edge;
                let width = popup_rect.width - 2.0 * edge;
                if width > 0.0 && top < bottom {
                    cx.scene.begin_clip(popup_rect, Some(corners));
                    let mut marker_top = top;
                    if heading > 0 {
                        let height = (2.0 * cx.one_px()).min(bottom - top);
                        cx.scene.push_rect(
                            RectF::new(x, top, width, height), colors.rule, POPUP_MATCH_LAYER, None,
                        );
                        marker_top += height;
                    }
                    if marker_top < bottom {
                        cx.scene.push_rect(
                            RectF::new(x, marker_top, GROUP_HEADER_MARKER_WIDTH.min(width), bottom - marker_top),
                            colors.rule, POPUP_MATCH_LAYER, None,
                        );
                    }
                    cx.scene.end_clip();
                }
            }

            // Match highlight: background span on the matched substring
            if let Some(&(m_off, m_len)) =
                item_index.and_then(|item| self.snapshot.match_ranges.get(item))
            {
                if m_len > 0
                    && m_off + m_len <= label.len()
                    && label.is_char_boundary(m_off)
                    && label.is_char_boundary(m_off + m_len)
                {
                    let prefix_run = cx.text_engine.shape(&label[..m_off], font_size);
                    let match_run = cx
                        .text_engine
                        .shape(&label[m_off..m_off + m_len], font_size);
                    let match_x = text_x + prefix_run.width;
                    let match_width = text_viewport.map_or(match_run.width, |viewport| {
                        match_run.width.min((viewport.right() - match_x).max(0.0))
                    });
                    cx.scene.push_rect(
                        RectF::new(match_x, row_y, match_width, row_bg_height),
                        style.selection.with_alpha(0.25),
                        POPUP_MATCH_LAYER,
                        None,
                    );
                }
            }

            if joined_fill_clip.is_some() {
                // Row fills are bounded quads with the same exterior coverage as
                // the trigger. Text keeps the popup's exact reveal/content clip.
                cx.scene.end_clip();
                cx.scene.begin_clip(popup_rect, Some(corners));
            }
            if let Some(viewport) = text_viewport {
                cx.scene.begin_clip(viewport, None);
            }
            if let Some(run) = run {
                let text_y = row_y + (ROW_HEIGHT - run.height) / 2.0;
                cx.scene.push_text_with_backgrounds(
                    &run,
                    text_x,
                    text_y,
                    text_color,
                    vec![],
                    POPUP_TEXT_LAYER,
                    None,
                );
            }
            if text_viewport.is_some() {
                cx.scene.end_clip();
            }
            if joined_fill_clip.is_some() {
                cx.scene.end_clip();
                cx.scene.begin_clip(fill_rect, Some(fill_corners));
            }
        }

        if joined_fill_clip.is_some() {
            cx.scene.end_clip();
            cx.scene.begin_clip(popup_rect, Some(corners));
        }
        if let Some(layout) = scrollbar {
            cx.scene.push_rect(layout.track, track, POPUP_CHROME_LAYER, None);
            cx.scene.push_rect(layout.thumb, thumb, POPUP_CHROME_LAYER, None);
        }
        if let (Some(footer), Some(colors)) = (footer, &group_colors) {
            self.paint_group_footer(cx, footer, first_row, colors, edge);
        }
        cx.scene.end_clip();
        if border.a > 0.0 {
            cx.scene.begin_clip(popup_rect, None);
            cx.scene.push_stroked_path(
                geometry.outline(radius, edge),
                StrokeParams {
                    width: edge.min(popup_rect.height),
                    ..Default::default()
                },
                border,
                POPUP_BORDER_LAYER,
                None,
            );
            cx.scene.end_clip();
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if !self.enabled {
            return false;
        }

        // Scrollbar drag continuation (pointer capture ensures delivery regardless of phase)
        if self.scrollbar_v.dragging {
            if let Event::Pointer(pe) = event {
                match pe.kind {
                    PointerEventKind::Move => {
                        let (_, global_y) = pe.global_pos;
                        let scroll_px = self.scrollbar_v.update_drag(global_y);
                        let max_scroll_rows = self
                            .snapshot
                            .display_len()
                            .saturating_sub(self.input_visible_rows(cx));
                        let row = (scroll_px / ROW_HEIGHT)
                            .round()
                            .max(0.0)
                            .min(max_scroll_rows as f32)
                            as usize;
                        self.scroll_popup_to(row, cx);
                        return true;
                    }
                    PointerEventKind::Up => {
                        self.scrollbar_v.end_drag();
                        self.hold_popup_scrollbar(cx);
                        self.update_popup_scrollbar_hover(pe, cx);
                        return true;
                    }
                    _ => {}
                }
            }
        }

        if phase != EventPhase::Target {
            return false;
        }

        let control = if matches!(event, Event::Pointer(_)) {
            cx.widget_global_rect().unwrap_or_default()
        } else {
            RectF::default()
        };
        let text_right = control.width - self.affordance_width;
        let popup_hit = cx.combo_popup_hit();
        if let Event::Pointer(pe) = event
            && matches!(
                pe.kind,
                PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick
            )
            && self.popup_inert_press_at(popup_hit, pe.local_pos, pe.global_pos)
        {
            // The popup owns this press without arming a choice.
            return true;
        }
        if self.text_input_enabled {
            if let Event::Pointer(pe) = event {
                if pe.local_pos.0 >= 0.0
                    && pe.local_pos.0 < text_right
                    && pe.local_pos.1 >= 0.0
                    && pe.local_pos.1 < control.height
                {
                    cx.request_cursor(crate::core::event::CursorIcon::Text);
                }
            }
        }

        match event {
            Event::Key(ke)
                if ke.is_down
                    && cx.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture =>
            {
                match ke.key_code {
                    // Loading guards: Enter commits text if allow_custom_value, nav keys are no-op.
                    KeyCode::Enter if self.loading => {
                        if self.allow_custom_value {
                            self.attempt_commit(CommitSource::InputConfirm, cx);
                        }
                        true
                    }
                    KeyCode::Down
                    | KeyCode::Up
                    | KeyCode::PageDown
                    | KeyCode::PageUp
                    | KeyCode::Home
                    | KeyCode::End
                        if self.loading =>
                    {
                        true
                    }
                    KeyCode::Enter => {
                        if !self.popup_open.get() {
                            // Control surface focused + popup closed → open popup (if content exists)
                            if self.has_popup_content() {
                                self.set_popup_open(true, true);
                                self.popup_scroll_row = 0;
                            }
                            true
                        } else {
                            // Popup open: commit source from focus_zone
                            let source = match self.focus_zone {
                                PopupFocusZone::TextInput => CommitSource::InputConfirm,
                                PopupFocusZone::OptionList => CommitSource::Selection,
                            };
                            self.attempt_commit(source, cx);
                            true
                        }
                    }
                    // Space: no interception in text-input mode — falls through to TextField.
                    // Popup opening is handled by Enter, Down, affordance click, and text click.
                    KeyCode::Escape => {
                        if self.popup_open.get() {
                            self.close_without_commit();
                            true
                        } else {
                            false
                        }
                    }
                    KeyCode::Down => {
                        if !self.popup_open.get() {
                            if self.has_popup_content() {
                                self.set_popup_open(true, true);
                                self.popup_scroll_row = 0;
                            }
                            true
                        } else {
                            self.highlight_next();
                            self.ensure_highlight_visible(cx);
                            self.focus_zone = PopupFocusZone::OptionList;
                            true
                        }
                    }
                    KeyCode::Up => {
                        if self.popup_open.get() {
                            self.highlight_prev();
                            self.ensure_highlight_visible(cx);
                            self.focus_zone = PopupFocusZone::OptionList;
                            true
                        } else {
                            false
                        }
                    }
                    KeyCode::PageDown if self.popup_open.get() => {
                        if self.snapshot.items.is_empty() {
                            return true;
                        }
                        self.highlight_from_hover = false;
                        let visible = self.input_visible_rows(cx);
                        let last = self.snapshot.items.len() - 1;
                        let idx = self
                            .highlighted_id
                            .and_then(|id| {
                                self.snapshot
                                    .items
                                    .iter()
                                    .position(|(item_id, _)| *item_id == id)
                            })
                            .unwrap_or(0);
                        let target = (idx + visible).min(last);
                        self.highlighted_id = Some(self.snapshot.items[target].0);
                        self.focus_zone = PopupFocusZone::OptionList;
                        self.ensure_highlight_visible(cx);
                        true
                    }
                    KeyCode::PageUp if self.popup_open.get() => {
                        if self.snapshot.items.is_empty() {
                            return true;
                        }
                        self.highlight_from_hover = false;
                        let visible = self.input_visible_rows(cx);
                        let idx = self
                            .highlighted_id
                            .and_then(|id| {
                                self.snapshot
                                    .items
                                    .iter()
                                    .position(|(item_id, _)| *item_id == id)
                            })
                            .unwrap_or(0);
                        let target = idx.saturating_sub(visible);
                        self.highlighted_id = Some(self.snapshot.items[target].0);
                        self.focus_zone = PopupFocusZone::OptionList;
                        self.ensure_highlight_visible(cx);
                        true
                    }
                    KeyCode::Home if self.popup_open.get() => {
                        if self.snapshot.items.is_empty() {
                            return true;
                        }
                        self.highlight_from_hover = false;
                        self.highlighted_id = Some(self.snapshot.items[0].0);
                        self.focus_zone = PopupFocusZone::OptionList;
                        self.ensure_highlight_visible(cx);
                        true
                    }
                    KeyCode::End if self.popup_open.get() => {
                        if self.snapshot.items.is_empty() {
                            return true;
                        }
                        self.highlight_from_hover = false;
                        let last = self.snapshot.items.len() - 1;
                        self.highlighted_id = Some(self.snapshot.items[last].0);
                        self.focus_zone = PopupFocusZone::OptionList;
                        self.ensure_highlight_visible(cx);
                        true
                    }
                    KeyCode::Character(_) | KeyCode::Space | KeyCode::Unknown
                        if !ke.modifiers.ctrl
                            && !ke.modifiers.alt
                            && !ke.modifiers.meta
                            && self.active_type_to_select().is_some() =>
                    {
                        // Consuming this key would suppress the platform's
                        // paired TextInput, which carries the actual text.
                        false
                    }
                    // Forward all other key-down events to TextField
                    _ => {
                        if !self.text_input_enabled {
                            return true;
                        }
                        self.forward_text(cx, true)
                    }
                }
            }
            Event::Text(te) => {
                if cx.keyboard_mode != crate::core::event::KeyboardAuthorityMode::Capture {
                    return false;
                }

                if let Some(match_prefix) = self.active_type_to_select() {
                    if te.text.is_empty()
                        || te.modifiers.ctrl
                        || te.modifiers.alt
                        || te.modifiers.meta
                    {
                        return false;
                    }
                    let Some(id) = match_prefix(&te.text, &self.snapshot.entries()) else {
                        return false;
                    };
                    let Some(idx) = self.snapshot.items.iter().position(|(option, _)| *option == id)
                    else {
                        return false;
                    };
                    let previous = (self.highlighted_id, self.popup_scroll_row);
                    self.highlighted_id = Some(id);
                    self.highlight_from_hover = false;
                    self.focus_zone = PopupFocusZone::OptionList;
                    self.ensure_popup_item_visible(idx, cx);
                    return previous != (self.highlighted_id, self.popup_scroll_row);
                }

                // These shortcuts belong to popup browsing (HAL-328 §11.5).
                if self.popup_open.get() && matches!(self.focus_zone, PopupFocusZone::OptionList) {
                    match te.text.as_str() {
                        "j" => {
                            self.highlight_next();
                            self.ensure_highlight_visible(cx);
                            cx.request_continuous_frames();
                            return true;
                        }
                        "k" => {
                            self.highlight_prev();
                            self.ensure_highlight_visible(cx);
                            cx.request_continuous_frames();
                            return true;
                        }
                        "i" => {
                            self.attempt_commit(CommitSource::Selection, cx);
                            return true;
                        }
                        _ => {}
                    }
                }

                if !self.text_input_enabled {
                    return true;
                }

                let typing = !te.text.is_empty()
                    && !te.modifiers.ctrl
                    && !te.modifiers.alt
                    && !te.modifiers.meta;
                if typing {
                    // Restore editing eligibility before the engine delegates this input.
                    self.focus_zone = PopupFocusZone::TextInput;
                }
                self.forward_text(
                    cx,
                    typing && (self.type_to_search || self.search_harness.is_some()),
                )
            }
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let (local_x, local_y) = pe.local_pos;
                    let over_control = local_x >= 0.0
                        && local_x < control.width
                        && local_y >= 0.0
                        && local_y < control.height;
                    let over_popup = popup_hit.is_some_and(|popup| {
                        local_x >= popup.rows.x
                            && local_x < popup.rows.right()
                            && local_y >= popup.rows.y
                            && local_y < popup.rows.bottom()
                    });
                    if let Some(popup) = popup_hit
                        && (self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide
                            || (over_popup && !over_control))
                        && self.press_popup_scrollbar(popup, pe, cx)
                    {
                        return true;
                    }

                    if over_control {
                        let in_affordance = local_x >= text_right;
                        if in_affordance {
                            // Click on affordance — toggle popup, do NOT forward to TextField
                            if self.popup_open.get() {
                                self.set_popup_open(false, true);
                                self.highlighted_id = None;
                                self.highlight_from_hover = false;
                                self.set_ghost_completion(None);
                            } else if self.has_popup_content() {
                                self.set_popup_open(true, true);
                                self.popup_scroll_row = 0;
                            }
                            if self.text_input_enabled {
                                cx.request_focus();
                            } else {
                                cx.request_keyboard_capture();
                            }
                        } else if self.close_on_text_click && self.popup_open.get() {
                            // Click on text area with popup open — close popup, position cursor
                            self.close_without_commit();
                            if self.text_input_enabled {
                                self.forward_text(cx, false);
                            }
                            if self.text_input_enabled {
                                cx.request_focus();
                            } else {
                                cx.request_keyboard_capture();
                            }
                        } else {
                            if self.text_input_enabled {
                                // Click on text area — forward to TextField for cursor positioning
                                self.forward_text(cx, false);
                            }
                            if !self.popup_open.get() && self.has_popup_content() {
                                self.set_popup_open(true, true);
                                self.popup_scroll_row = 0;
                            }
                            if self.text_input_enabled {
                                cx.request_focus();
                            } else {
                                cx.request_keyboard_capture();
                            }
                        }
                        true
                    } else if over_popup {
                        if let Some(ChoiceDisplayRow::Item(item_index)) =
                            self.popup_row_hit(popup_hit, pe)
                        {
                            // Press on option row — highlight only, commit deferred to PointerUp
                            // (Button press/release semantics: visual state on Down, activate on Up)
                            self.highlighted_id = Some(self.snapshot.items[item_index].0);
                            self.highlight_from_hover = false;
                            self.option_press_active = true;
                            if let Some(button) = pe.button {
                                cx.capture_pointer(button);
                            }
                            true
                        } else {
                            false
                        }
                    } else {
                        false
                    }
                }
                PointerEventKind::Move => {
                    if !self.popup_open.get() {
                        return false;
                    }
                    self.update_popup_scrollbar_hover(pe, cx);
                    let row = self.popup_row_hit(popup_hit, pe);
                    if let Some(ChoiceDisplayRow::Item(item_index)) = row {
                        let new_id = self.snapshot.items[item_index].0;
                        if self.highlighted_id != Some(new_id) {
                            self.highlighted_id = Some(new_id);
                            self.highlight_from_hover = true;
                            self.focus_zone = PopupFocusZone::OptionList;
                        }
                        true
                    } else if self.highlight_from_hover {
                        // Cursor left popup — clear hover-originated highlight
                        self.highlighted_id = None;
                        true
                    } else {
                        matches!(row, Some(ChoiceDisplayRow::Header(_)))
                            || self.scrollbar_v.state == ScrollbarState::Hovered
                    }
                }
                PointerEventKind::Scroll { delta_y, .. } => {
                    if !self.popup_open.get() {
                        return false;
                    }
                    let visible_rows = self.input_visible_rows(cx);
                    if visible_rows == 0 && self.snapshot.groups.is_some() {
                        return false;
                    }
                    let max_scroll = self.snapshot.display_len().saturating_sub(visible_rows);
                    let rows = (-delta_y / ROW_HEIGHT).round() as isize;
                    let rows = if rows == 0 && delta_y != 0.0 {
                        if delta_y > 0.0 { -1 } else { 1 }
                    } else {
                        rows
                    };
                    let previous = self.first_popup_row(visible_rows);
                    let row = (previous as isize + rows)
                        .max(0)
                        .min(max_scroll as isize)
                        as usize;
                    self.scroll_popup_to(row, cx);
                    if row != previous {
                        // A wheel event can be the first pointer sample since
                        // opening the popup with the keyboard.
                        self.update_popup_scrollbar_hover(pe, cx);
                    }
                    self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::WhenOverflowing
                        || row != previous
                }
                PointerEventKind::Up => {
                    if !self.option_press_active {
                        return self.popup_inert_press_at(popup_hit, pe.local_pos, pe.global_pos);
                    }
                    self.option_press_active = false;
                    // Button semantics: activate only if released inside a valid row
                    if pe.button != Some(PointerButton::Primary) {
                        return true;
                    }
                    if self.popup_open.get() {
                        if let Some(ChoiceDisplayRow::Item(item_index)) =
                            self.popup_row_hit(popup_hit, pe)
                        {
                            self.highlighted_id = Some(self.snapshot.items[item_index].0);
                            self.attempt_commit(CommitSource::Selection, cx);
                        }
                    }
                    true
                }
                PointerEventKind::DoubleClick | PointerEventKind::TripleClick => {
                    // Forward multi-click to TextField for word/line selection
                    let (local_x, local_y) = pe.local_pos;
                    if local_y < control.height
                        && local_x < text_right
                        && self.text_input_enabled
                    {
                        self.forward_text(cx, false)
                    } else {
                        false
                    }
                }
                _ => false,
            },
            Event::Key(_) => {
                // SPEC-HAL-328 Category E: keyboard events only in Capture mode.
                // In Navigation mode, return false → framework handles navigation.
                if cx.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
                    if !self.text_input_enabled {
                        return true;
                    }
                    self.forward_text(cx, false)
                } else {
                    false
                }
            }
            // Forward key-up, IME, and other events to TextField
            _ => {
                if self.text_input_enabled {
                    self.forward_text(cx, false)
                } else {
                    false
                }
            }
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        self.enabled
    }

    fn touch_target(&self, hit: TouchHit) -> Option<TouchTarget> {
        if !self.enabled {
            return None;
        }
        if hit.origin == PointerRouteOrigin::Overlay {
            let popup = hit.popup?;
            let global = (hit.position.x, hit.position.y);
            if popup.drawn_scrollbar().is_some_and(|layout| {
                layout.hit_rect.contains(popup.scrollbar_point(global))
            }) {
                return None;
            }
            let ChoiceDisplayRow::Item(item_index) = self.popup_row_at(
                Some(popup), (hit.local_position.x, hit.local_position.y), global,
            )? else {
                return None;
            };
            return Some(TouchTarget::Item(self.snapshot.items[item_index].0.raw()));
        }
        if self.text_input_enabled && hit.local_position.x < hit.size.width - self.affordance_width {
            Some(TouchTarget::Editor)
        } else {
            self.has_popup_content().then_some(TouchTarget::Control)
        }
    }

    fn touch_target_revision(&self) -> Option<u64> {
        Some(self.popup_input_revision)
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, tap: TouchTap) -> bool {
        match tap.target {
            TouchTarget::Item(raw) => {
                self.highlighted_id = Some(OptionId::from_raw(raw));
                self.highlight_from_hover = false;
                self.attempt_commit(CommitSource::Selection, cx);
                true
            }
            TouchTarget::Editor => {
                self.focus_zone = PopupFocusZone::TextInput;
                cx.request_composite_edit(
                    CompositeEditOperation::PlaceTouchCaret { local_x: tap.local_position.x },
                    CompositeHostContinuation::Combo(CompositeComboContinuation::Input { search: false }),
                ).is_ok()
            }
            TouchTarget::Control => {
                if !self.popup_open.get() && self.has_popup_content() {
                    self.set_popup_open(true, true);
                    self.popup_scroll_row = 0;
                    cx.invalidate_paint();
                }
                cx.request_keyboard_capture();
                true
            }
            _ => false,
        }
    }

    fn gesture_surface(&self) -> &'static [GestureKind] {
        &[GestureKind::Scroll]
    }

    fn accepts_touch_scroll_gesture(&self) -> bool {
        true
    }

    fn touch_scroll_requires_overlay(&self) -> bool {
        true
    }

    fn touch_scroll_intent(&self, _motion: QualifiedTouchMotion, axis: GestureAxis) -> bool {
        self.enabled && self.popup_open.get() && self.needs_scrollbar() && axis == GestureAxis::Vertical
    }

    fn on_gesture_commit(&mut self, cx: &mut EventContext, kind: GestureKind) {
        if kind == GestureKind::Scroll && let Some(motion) = cx.touch_scroll_motion() {
            let first = self.first_popup_row(self.input_visible_rows(cx));
            self.popup_touch_pan = Some((motion.press, first as f32 * ROW_HEIGHT));
            self.on_touch_scroll_motion(cx);
        }
    }

    fn on_touch_scroll_motion(&mut self, cx: &mut EventContext) {
        let Some(motion) = cx.touch_scroll_motion() else { return; };
        let Some((press, offset)) = self.popup_touch_pan else { return; };
        if motion.press != press || !motion.delta.y.is_finite() {
            return;
        }
        let rows = self.input_visible_rows(cx);
        if rows == 0 && self.snapshot.groups.is_some() {
            if motion.is_release {
                self.popup_touch_pan = None;
            }
            return;
        }
        let max = self.snapshot.display_len().saturating_sub(rows);
        let offset = (offset - motion.delta.y).clamp(0.0, max as f32 * ROW_HEIGHT);
        self.scroll_popup_to((offset / ROW_HEIGHT).round() as usize, cx);
        self.popup_touch_pan = (!motion.is_release).then_some((press, offset));
    }

    fn touch_scroll_is_live(&self, press: TouchPressIdentity) -> bool {
        self.enabled && self.popup_open.get()
            && self.popup_touch_pan.is_some_and(|(owner, _)| owner == press)
    }

    fn on_gesture_cancel(&mut self, cx: &mut EventContext, kind: GestureKind) {
        if kind == GestureKind::Scroll
            && self.popup_touch_pan.is_some_and(|(press, _)| cx.touch_gesture_press() == Some(press))
        {
            self.popup_touch_pan = None;
        }
    }

    fn accepts_keyboard_activation(&self) -> bool {
        // SPEC-HAL-328 Category E: popup widgets use capture entry, not keyboard activation.
        false
    }

    fn wants_scroll(&self) -> bool {
        self.popup_open.get()
            && (self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::WhenOverflowing
                || self.needs_scrollbar())
    }

    fn dismiss_nonmodal_popup_on_outside_press(&mut self) -> bool {
        if !self.popup_open.get() {
            return false;
        }
        self.close_without_commit();
        true
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        // SPEC-HAL-328 Category E: do NOT activate the embedded TextField
        // in Navigation mode. TextField is activated only on Capture entry.
        self.focused = true;
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        self.cell_hovered = true;
        if self
            .input_surface_appearance
            .as_ref()
            .is_some_and(InputSurfaceAppearance::has_state_surfaces)
        {
            cx.invalidate_paint();
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.cell_hovered = false;
        self.set_popup_scrollbar_hovered(false, cx);
        let highlight_cleared = self.highlight_from_hover && self.highlighted_id.take().is_some();
        self.highlight_from_hover = false;
        if highlight_cleared
            || self
                .input_surface_appearance
                .as_ref()
                .is_some_and(InputSurfaceAppearance::has_state_surfaces)
        {
            cx.invalidate_paint();
        }
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.focused = false;
        if self.popup_open.get() {
            self.close_without_commit();
        }
        let _ = cx.request_composite_edit(
            CompositeEditOperation::RetireDelegation,
            CompositeHostContinuation::Combo(CompositeComboContinuation::Retire),
        );
    }

    fn needs_animation(&self) -> bool {
        (self.enabled && self.loading)
            || self.popup_presentation.animation.is_some()
            || self.popup_presentation.target_open != self.popup_open.get()
    }

    fn needs_update(&self) -> bool {
        self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide
            && self.popup_open.get()
            && self.scrollbar_fade_alpha > 0.0
            && self.scrollbar_v.state == ScrollbarState::Normal
            && (self.scrollbar_appearance.track_opacity > 0.0
                || self.scrollbar_appearance.thumb_opacity > 0.0)
    }

    fn update(&mut self, dt: f32, cx: &mut UpdateContext) {
        self.popup_presentation.update(self.popup_open.get(), cx);
        // The first Update after activity includes time from before the reveal.
        if std::mem::take(&mut self.scrollbar_activity) || !self.needs_update() {
            return;
        }
        let hold = self.scrollbar_hold_remaining;
        self.scrollbar_hold_remaining = (hold - dt).max(0.0);
        if self.scrollbar_hold_remaining > 0.0 {
            return;
        }
        let fade = self.scrollbar_interaction.fade_seconds;
        let previous = self.scrollbar_fade_alpha;
        let alpha = if fade == 0.0 {
            0.0
        } else {
            let fade_dt = (dt - hold).max(0.0);
            (previous - fade_dt / fade).max(0.0)
        };
        if alpha != previous {
            self.scrollbar_fade_alpha = alpha;
            if cx.theme().is_none_or(|theme| {
                self.popup_scrollbar_colors(theme, previous)
                    != self.popup_scrollbar_colors(theme, alpha)
            }) {
                cx.invalidate_paint();
            }
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "ComboBox"
    }

    fn capture_eligible(&self) -> bool {
        self.enabled
    }

    fn paints_own_focus_indicator(&self) -> bool {
        // The input and popup share one chrome contour using framework facts.
        true
    }

    fn on_capture_enter(&mut self, cx: &mut EventContext) {
        self.in_capture = true;
        // SPEC-HAL-328 Category E: activate the embedded TextField now.
        if self.text_input_enabled {
            let _ = cx.request_composite_edit(
                CompositeEditOperation::SeedForEntry {
                    text: self.text_field.text().to_owned(),
                    select_all: false,
                },
                CompositeHostContinuation::Combo(CompositeComboContinuation::Entry),
            );
        } else {
            self.focus_zone = PopupFocusZone::OptionList;
        }
        // SPEC-HAL-328 Category E: I/Enter (via enter_widget action) opens popup.
        if !self.popup_open.get() && self.has_popup_content() {
            self.set_popup_open(true, true);
            self.popup_scroll_row = 0;
        }
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        self.in_capture = false;
        // SPEC-HAL-328 Category E: deactivate the embedded TextField.
        let _ = cx.request_composite_edit(
            CompositeEditOperation::RetireDelegation,
            CompositeHostContinuation::Combo(CompositeComboContinuation::Retire),
        );
        if self.popup_open.get() {
            self.close_without_commit();
        }
    }

    fn on_pointer_capture_cancel(
        &mut self,
        cx: &mut EventContext,
        _pointer_id: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        if self.scrollbar_interaction.display_mode == PopupScrollbarPolicy::AutoHide {
            self.option_press_active = false;
            if self.scrollbar_v.dragging {
                self.scrollbar_v.end_drag();
                self.hold_popup_scrollbar(cx);
            }
        }
    }
}

#[cfg(test)]
mod text_z_order_tests {
    use super::*;
    use crate::core::app::RetainedSceneTestHarness;
    use crate::core::icon::IconRegistry;
    use crate::core::scene::{BorderRectNode, PathNode, RectNode, SceneNode, TextNode};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;

    fn surface_nodes(nodes: &[SceneNode]) -> Vec<&SceneNode> {
        let mut result = Vec::new();
        for node in nodes {
            result.push(node);
            let children: Option<&[SceneNode]> = match node {
                SceneNode::Clip(n) => Some(&n.children),
                SceneNode::Layer(n) => Some(&n.children),
                SceneNode::Scroll(n) => Some(&n.children),
                SceneNode::RetainedFragment(n) => Some(&n.children),
                _ => None,
            };
            if let Some(children) = children {
                result.extend(surface_nodes(children));
            }
        }
        result
    }

    fn check_input_surface_appearance(selected: bool) {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode,
            PointerDevice, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::layout::EdgeInsets;
        use crate::core::paint::{Color, ColorStop, GradientDef, GradientKind, SurfaceGradient};
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::{rc::Rc, sync::Arc, time::Instant};

        let value = InputSurfaceAppearance::try_new(
            SurfaceGradient::new(GradientDef {
                kind: GradientKind::Linear,
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
                stops: vec![
                    ColorStop {
                        offset: 0.0,
                        color: Color::from_hex(0x303234).with_alpha(0.8),
                    },
                    ColorStop {
                        offset: 1.0,
                        color: Color::from_hex(0x101214).with_alpha(0.6),
                    },
                ],
            })
            .unwrap(),
            Color::from_hex(0x515355),
            Color::from_hex(0x383A3C),
            Color::from_hex(0x232527),
            Color::from_hex(0x181A1C),
            Color::from_hex(0x454749),
            14.0,
        )
        .unwrap();
        for (scale, editable) in [(1.0, false), (2.0, false), (1.0, true)] {
            let theme = Theme::dark();
            let icons = builtin_registry();
            crate::core::icon::validate_icon(icons.get(IconId::GLOBE).unwrap()).unwrap();
            let commits = Rc::new(Cell::new(0));
            let count = commits.clone();
            let mut control = ComboBox::simple_with_values(vec![
                ("RU".into(), "ru".into()),
                ("EN".into(), "en".into()),
            ]);
            control.set_input_text("RU".into()).unwrap();
            control.set_text_input_enabled(editable);
            control.set_on_committed(move |_| count.set(count.get() + 1));
            if selected {
                control = control
                    .with_input_surface_appearance(value.clone())
                    .with_leading_icon(IconId::GLOBE);
            }
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_padding(EdgeInsets {
                    left: 20.0,
                    right: 220.0,
                    top: 12.0,
                    bottom: 0.0,
                })),
                None,
                0,
            );
            let node = tree.add_child(root, Box::new(control), None, 0);
            let peer = tree.add_child(
                root,
                Box::new(crate::widgets::label::Label::new("Peer")),
                None,
                1,
            );
            let owner = tree.node(node).unwrap().id;
            let peer_owner = tree.node(peer).unwrap().id;
            let mut engine = FontManagerTextEngine::new();
            engine.set_scale(scale);
            let mut harness = RetainedSceneTestHarness::new();
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let mut bindings = BindingRegistry::new();
            let target = CustomRenderTargetSnapshot::new(
                crate::compositor::SurfaceId::test_id(1),
                PhysicalExtent::new((360.0 * scale) as u32, (280.0 * scale) as u32),
                scale,
            )
            .unwrap();
            macro_rules! frame {
                ($time:expr) => {{
                    harness.advance_transition_time($time);
                    harness.frame_with_optional_custom_render_target(
                        &mut tree,
                        root,
                        Constraints::tight(Size::new(360.0, 280.0)),
                        &theme,
                        &mut engine,
                        &icons,
                        Some(target),
                        Some((&domains, WindowId::PRIMARY)),
                    )
                }};
            }
            macro_rules! route {
                ($event:expr) => {
                    router.route_input_event(
                        &mut tree,
                        $event,
                        Some(root),
                        &mut engine,
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    )
                };
            }
            macro_rules! combo {
                () => {
                    tree.node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<ComboBox>()
                        .unwrap()
                };
            }
            let pointer = |down, pos| {
                if down {
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    }
                } else {
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    }
                }
            };
            let key = |key_code| FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key: PhysicalKeyCode::Unknown,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            };
            frame!(0.0);
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            assert_eq!(rect.width, 120.0);
            let child = combo!().text_field.child.unwrap();
            let child_rect = tree.get_absolute_paint_rect(child).unwrap();
            let leading = if selected {
                theme.typography.input_font_size + theme.geometry.text_field.padding_x
            } else {
                0.0
            };
            assert_eq!(child_rect.x, rect.x + leading);
            assert_eq!(
                child_rect.width,
                rect.width - combo!().affordance_width - leading
            );
            let peer_rect = tree.get_absolute_paint_rect(peer).unwrap();
            let local = harness.local_fragment(owner).unwrap();
            let nodes = surface_nodes(&local);
            let gradients: Vec<_> = nodes
                .iter()
                .filter_map(|node| {
                    if let SceneNode::GradientRect(n) = node {
                        Some(n)
                    } else {
                        None
                    }
                })
                .collect();
            assert!(nodes.iter().any(
                |n| matches!(n, SceneNode::Path(p) if p.is_stroke && p.z_index == LAYER_CONTENT + 4)
            ));
            if !editable {
                assert!(nodes.iter().any(|n| matches!(n, SceneNode::Text(t) if t.text == "RU" && t.position.x == rect.x + leading + theme.geometry.text_field.padding_x)));
            }
            if selected {
                assert_eq!(gradients.len(), 1);
                assert_eq!(gradients[0].rect, rect);
                assert_eq!(&gradients[0].gradient, value.surface().definition());
                assert!(nodes.iter().any(|n| matches!(n, SceneNode::Clip(c) if c.rect == rect && c.rounded.is_some_and(|r| r.tl == value.radius().min(rect.height * 0.5)))));
                assert!(nodes.iter().any(|n| matches!(n, SceneNode::BorderRect(b) if b.rect == rect && b.border_color == value.border().into() && b.fill_color.a == 0.0)));
                assert!(nodes.iter().any(|n| matches!(n, SceneNode::BorderRect(b) if b.border_color == value.inner_edge().into() && b.fill_color.a == 0.0)));
                assert!(nodes.iter().any(|n| matches!(n, SceneNode::Rect(r) if r.color == value.affordance().into() && r.z_index == LAYER_CONTENT + 3)));
                let icon_layers = nodes.iter().filter(|n| matches!(n, SceneNode::Path(p) if !p.is_stroke && p.z_index == LAYER_CONTENT + 2)).count();
                assert_eq!(icon_layers, icons.get(IconId::GLOBE).unwrap().layers.len());
                let icon_point = Point::new(
                    rect.x
                        + theme.geometry.text_field.padding_x
                        + theme.typography.input_font_size * 0.5,
                    rect.y + rect.height * 0.5,
                );
                assert_eq!(
                    tree.hit_test(icon_point.x, icon_point.y).unwrap().widget_id,
                    owner
                );
                assert!(
                    !nodes
                        .iter()
                        .any(|n| matches!(n, SceneNode::RoundedRect(r) if r.rect == rect))
                );
            } else {
                assert!(gradients.is_empty());
                assert!(nodes.iter().any(|n| matches!(n, SceneNode::RoundedRect(r) if r.rect == rect && r.radius == 2.0 && r.color == theme.input_chrome(ControlChromeState::Idle).chrome.border_color.into())));
                assert!(!nodes.iter().any(|n| matches!(n, SceneNode::Path(p) if !p.is_stroke && p.z_index == LAYER_CONTENT + 2)));
            }
            let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
            route!(pointer(true, arrow));
            route!(pointer(false, arrow));
            assert!(combo!().popup_open.get());
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            route!(key(KeyCode::Home));
            frame!(0.010);
            frame!(0.060);
            let popup = ComboBox::test_popup_local_rect(&tree, node);
            assert_eq!(popup.y, rect.height);
            assert_eq!(popup.x, 0.0);
            assert_eq!(popup.width, 150.0);
            assert_eq!(popup.height, ROW_HEIGHT * 2.0);
            let overlay = harness.overlay_fragment(owner).unwrap();
            let popup_nodes = surface_nodes(&overlay);
            let normal = if selected {
                value.popup_surface()
            } else {
                theme.colors.surfaces.content
            };
            let highlight = if selected {
                value.popup_highlight()
            } else {
                theme.input_chrome(ControlChromeState::Captured).affordance
            };
            assert!(popup_nodes.iter().any(|n| matches!(n, SceneNode::Path(p) if !p.is_stroke && p.z_index == POPUP_SURFACE_LAYER && p.color == normal.into())));
            for n in &popup_nodes {
                if let SceneNode::Text(t) = n {
                    if t.z_index == POPUP_TEXT_LAYER {
                        let expected = if t.text == "RU" { highlight } else { normal };
                        assert!(t.backgrounds.is_empty());
                        assert!(popup_nodes.iter().any(|node| matches!(node,
                            SceneNode::Rect(row) if row.z_index == POPUP_ROW_LAYER
                                && row.rect.y <= t.position.y
                                && row.rect.bottom() > t.position.y
                                && row.color == expected.into()
                        )));
                    }
                }
            }
            assert!(popup_nodes.iter().any(|n| matches!(n, SceneNode::Path(p) if p.is_stroke && p.z_index == POPUP_BORDER_LAYER && p.color == theme.input_chrome(ControlChromeState::Captured).chrome.border_color.into())));
            let joined_local = harness.local_fragment(owner).unwrap();
            if selected {
                assert!(surface_nodes(&joined_local).iter().any(|n| matches!(n, SceneNode::GradientRect(g) if g.gradient == *value.surface().definition())));
            }
            let option = Point::new(rect.x + popup.x + 15.0, rect.y + popup.y + ROW_HEIGHT * 1.5);
            route!(pointer(true, option));
            route!(pointer(false, option));
            assert_eq!(combo!().committed_value_string().as_deref(), Some("en"));
            assert_eq!(combo!().input_text(), "EN");
            assert_eq!(commits.get(), 1);
            assert!(!combo!().popup_open.get());
            frame!(0.100);
            route!(pointer(true, arrow));
            route!(pointer(false, arrow));
            route!(key(KeyCode::Home));
            route!(key(KeyCode::Down));
            route!(key(KeyCode::Enter));
            assert_eq!(commits.get(), 2);
            assert_eq!(combo!().committed_value_string().as_deref(), Some("en"));
            frame!(0.150);
            frame!(0.200); // Original 25 ms closing presentation has settled.
            assert_eq!(tree.get_absolute_paint_rect(node).unwrap(), rect);
            assert_eq!(tree.get_absolute_paint_rect(peer).unwrap(), peer_rect);
            assert_eq!(combo!().text_field.child, Some(child));
            let peer_local = harness.local_fragment(peer_owner).unwrap();
            let settled = harness.local_fragment(owner).unwrap();
            frame!(0.210);
            assert!(Arc::ptr_eq(
                &peer_local,
                &harness.local_fragment(peer_owner).unwrap()
            ));
            if !editable {
                assert!(Arc::ptr_eq(
                    &settled,
                    &harness.local_fragment(owner).unwrap()
                ));
            }
        }
    }

    #[test]
    fn combo_input_surface_appearance_shell_popup_and_leading_icon() {
        check_input_surface_appearance(true);
    }

    #[test]
    fn combo_input_surface_appearance_absent_preserves_standard_control() {
        check_input_surface_appearance(false);
    }

    fn check_common_input_surface(
        face: crate::core::paint::Color,
        radius: f32,
        product_policy: bool,
        states: Option<(crate::core::paint::Color, crate::core::paint::Color)>,
    ) {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PointerDevice,
            WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::icons::builtin_registry;
        use crate::core::paint::{Color, ColorStop, GradientDef, GradientKind, SurfaceGradient};
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::{rc::Rc, time::Instant};

        let clear = Color::TRANSPARENT;
        let mut appearance = InputSurfaceAppearance::try_new(
            SurfaceGradient::new(GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop {
                        offset: 0.0,
                        color: face,
                    },
                    ColorStop {
                        offset: 1.0,
                        color: face,
                    },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            })
            .unwrap(),
            clear,
            clear,
            clear,
            Color::from_hex(0x181A1C),
            Color::from_hex(0x454749),
            radius,
        )
        .unwrap();
        if let Some((hover, active)) = states {
            let surface = |color| {
                SurfaceGradient::new(GradientDef {
                    kind: GradientKind::Linear,
                    stops: vec![
                        ColorStop { offset: 0.0, color },
                        ColorStop { offset: 1.0, color },
                    ],
                    p0: (0.0, 0.0),
                    p1: (0.0, 1.0),
                })
                .unwrap()
            };
            appearance = appearance.with_state_surfaces(surface(hover), surface(active));
        }
        let active_face = states.map_or(face, |(_, active)| active);
        let focused_color = Color::from_hex(0x9A958E);
        let captured_color = Color::from_hex(0xE2D5BF);
        if product_policy {
            assert!(
                appearance
                    .clone()
                    .try_with_focus_colors(clear, captured_color)
                    .is_ok()
            );
            assert!(
                appearance
                    .clone()
                    .try_with_focus_colors(focused_color, focused_color)
                    .is_err()
            );
            assert!(
                appearance
                    .clone()
                    .try_with_focus_colors(
                        Color::from_linear(f32::NAN, 0.0, 0.0, 1.0),
                        captured_color
                    )
                    .is_err()
            );
            appearance = appearance
                .try_with_focus_colors(focused_color, captured_color)
                .unwrap();
        }
        let commits = Rc::new(Cell::new(0));
        let count = commits.clone();
        let mut combo = ComboBox::simple(vec!["RU".into(), "EN".into()])
            .with_input_surface_appearance(appearance)
            .with_leading_icon(IconId::GLOBE);
        if product_policy {
            combo = combo.with_interaction_policy(PopupInteractionPolicy {
                width: crate::core::popup::PopupWidthPolicy::MatchTrigger,
            });
        }
        combo.set_input_text("RU".into()).unwrap();
        combo.set_text_input_enabled(false);
        combo.set_on_committed(move |_| count.set(count.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        let node = tree.add_child(root, Box::new(combo), None, 0);
        let owner = tree.node(node).unwrap().id;
        let mut engine = FontManagerTextEngine::new();
        engine.set_scale(2.0);
        let mut harness = RetainedSceneTestHarness::new();
        let theme = Theme::dark();
        let icons = builtin_registry();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let target = CustomRenderTargetSnapshot::new(
            crate::compositor::SurfaceId::test_id(1),
            PhysicalExtent::new(240, 560),
            2.0,
        )
        .unwrap();
        macro_rules! frame {
            ($time:expr) => {{
                harness.advance_transition_time($time);
                harness.frame_with_optional_custom_render_target(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(120.0, 280.0)),
                    &theme,
                    &mut engine,
                    &icons,
                    Some(target),
                    Some((&domains, WindowId::PRIMARY)),
                )
            }};
        }
        macro_rules! combo {
            () => {
                tree.node(node)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<ComboBox>()
                    .unwrap()
            };
        }
        macro_rules! click {
            ($pos:expr) => {
                for event in [
                    FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: $pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    },
                    FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: $pos,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    },
                ] {
                    router.route_input_event(
                        &mut tree,
                        event,
                        Some(root),
                        &mut engine,
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    );
                }
            };
        }
        let check_content = |nodes: &[SceneNode], label: &str, expected_face: Color| {
            let leaves = surface_nodes(nodes);
            assert!(
                !leaves
                    .iter()
                    .any(|n| matches!(n, SceneNode::Rect(_) | SceneNode::RoundedRect(_))),
                "no internal fill, separator or separate affordance paint"
            );
            let surfaces: Vec<_> = leaves
                .iter()
                .filter_map(|n| match n {
                    SceneNode::GradientRect(g) => Some(g),
                    _ => None,
                })
                .collect();
            assert_eq!(surfaces.len(), if expected_face.a > 0.0 { 1 } else { 0 });
            for surface in surfaces {
                assert!(
                    surface
                        .gradient
                        .stops
                        .iter()
                        .all(|stop| stop.color == expected_face)
                );
            }
            assert!(leaves.iter().all(|n| !matches!(n, SceneNode::BorderRect(b) if b.z_index != LAYER_FOCUS + 1 || b.fill_color.a != 0.0)), "only the authoritative exterior focus cue may remain");
            assert!(
                leaves
                    .iter()
                    .any(|n| matches!(n, SceneNode::Text(t) if t.text == label))
            );
            assert_eq!(
                leaves
                    .iter()
                    .filter(|n| matches!(n, SceneNode::Path(p) if !p.is_stroke))
                    .count(),
                icons.get(IconId::GLOBE).unwrap().layers.len(),
                "only the real globe contributes filled paths"
            );
            assert!(leaves.iter().any(|n| matches!(n, SceneNode::Path(p) if p.is_stroke && p.z_index == LAYER_CONTENT + 4)), "real caret remains");
        };
        frame!(0.0);
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        let child = combo!().text_field.child;
        let local = harness.local_fragment(owner).unwrap();
        check_content(&local, "RU", face);
        if face.a > 0.0 {
            let nodes = surface_nodes(&local);
            assert!(
                nodes
                    .iter()
                    .any(|n| matches!(n, SceneNode::GradientRect(g) if g.rect == rect)),
                "one dark face covers the complete common shell"
            );
            let r = radius.min(rect.width * 0.5).min(rect.height * 0.5);
            assert!(r > 2.0);
            assert!(nodes.iter().any(|n| matches!(n, SceneNode::Clip(c) if c.rect == rect && c.rounded.is_some_and(|corners| corners.tl == r && corners.tr == r && corners.bl == r && corners.br == r))), "the complete common face is rounded");
        }
        assert!(!surface_nodes(&local).iter().any(|n| matches!(n, SceneNode::BorderRect(_)) || matches!(n, SceneNode::Path(p) if p.is_stroke && p.z_index != LAYER_CONTENT + 4)));
        if let Some((hover, _)) = states {
            for (time, pos, expected) in [
                (
                    0.001,
                    Point::new(rect.x + 4.0, rect.y + rect.height * 0.5),
                    hover,
                ),
                (0.003, Point::new(rect.x + 4.0, rect.bottom() + 30.0), face),
            ] {
                router.route_input_event(
                    &mut tree,
                    FusedInputEvent::PointerMove {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos,
                        delta: (0.0, 0.0),
                        device: PointerDevice::Mouse,
                        buttons: crate::core::event::PointerButtonsState::default(),
                    },
                    Some(root),
                    &mut engine,
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                );
                frame!(time);
                let state = harness.local_fragment(owner).unwrap();
                check_content(&state, "RU", expected);
                frame!(time + 0.0005);
                assert!(
                    std::sync::Arc::ptr_eq(&state, &harness.local_fragment(owner).unwrap()),
                    "settled state reuses the original local fragment"
                );
                assert_eq!(tree.get_absolute_paint_rect(node).unwrap(), rect);
            }
        }
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        frame!(0.005);
        let focused = harness.local_fragment(owner).unwrap();
        check_content(&focused, "RU", active_face);
        let focus_border = if product_policy {
            focused_color
        } else {
            theme
                .input_chrome(ControlChromeState::Focused)
                .chrome
                .border_color
        };
        assert!(surface_nodes(&focused).iter().any(|n| matches!(n, SceneNode::BorderRect(b) if b.z_index == LAYER_FOCUS + 1 && b.border_color == focus_border.into())));
        let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
        click!(arrow);
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        frame!(0.010);
        frame!(0.060);
        let captured = harness.local_fragment(owner).unwrap();
        check_content(&captured, "RU", active_face);
        let capture_border = if product_policy {
            captured_color
        } else {
            theme
                .input_chrome(ControlChromeState::Captured)
                .chrome
                .border_color
        };
        assert!(surface_nodes(&captured).iter().any(|n| matches!(n, SceneNode::Path(p) if p.is_stroke && p.z_index == LAYER_FOCUS + 1 && p.color == capture_border.into())));
        let popup = ComboBox::test_popup_local_rect(&tree, node);
        assert_eq!(popup.y, rect.height);
        assert_eq!(popup.x, 0.0);
        assert_eq!(popup.width, if product_policy { rect.width } else { 150.0 });
        let overlay = harness.overlay_fragment(owner).unwrap();
        let popup_nodes = surface_nodes(&overlay);
        assert!(
            popup_nodes
                .iter()
                .any(|n| matches!(n, SceneNode::Text(t) if t.text == "EN"))
        );
        assert!(popup_nodes.iter().any(|n| matches!(n, SceneNode::Path(p) if !p.is_stroke && p.z_index == POPUP_SURFACE_LAYER && p.color == Color::from_hex(0x181A1C).into())));
        assert!(popup_nodes.iter().any(|n| matches!(n, SceneNode::Path(p) if p.is_stroke && p.z_index == POPUP_BORDER_LAYER && p.color == capture_border.into())));
        click!(Point::new(
            rect.x + popup.x + 15.0,
            rect.y + popup.y + ROW_HEIGHT * 1.5
        ));
        assert_eq!(combo!().input_text(), "EN");
        assert_eq!(commits.get(), 1);
        assert!(!combo!().popup_open.get());
        frame!(0.100);
        frame!(0.150);
        check_content(&harness.local_fragment(owner).unwrap(), "EN", active_face);
        assert_eq!(tree.get_absolute_paint_rect(node).unwrap(), rect);
        assert_eq!(combo!().text_field.child, child);
        if product_policy {
            click!(arrow);
            for key_code in [KeyCode::Home, KeyCode::Down, KeyCode::Enter] {
                router.route_input_event(
                    &mut tree,
                    FusedInputEvent::KeyDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        key_code,
                        physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                        reservation: None,
                        modifiers: Modifiers::default(),
                        repeat: false,
                    },
                    Some(root),
                    &mut engine,
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                );
            }
            assert_eq!(
                commits.get(),
                2,
                "one callback for each real pointer/keyboard commit"
            );
            assert_eq!(combo!().input_text(), "EN");
            assert!(!combo!().popup_open.get());
        }
    }

    #[test]
    fn combo_input_surface_appearance_transparent_preserves_content_popup_and_capture() {
        check_common_input_surface(crate::core::paint::Color::TRANSPARENT, 0.0, false, None);
    }

    #[test]
    fn combo_input_surface_appearance_common_dark_face_preserves_content_popup_and_capture() {
        check_common_input_surface(
            crate::core::paint::Color::from_hex(0x202224),
            14.0,
            false,
            None,
        );
    }

    #[test]
    fn combo_product_policy_width_focus_and_default_compatibility() {
        check_common_input_surface(
            crate::core::paint::Color::from_hex(0x171E25),
            14.0,
            true,
            None,
        );
        check_input_surface_appearance(false);
    }

    #[test]
    fn input_adornment_combo_idle_hover_active_preserves_real_popup_and_default() {
        use crate::core::paint::Color;
        check_common_input_surface(
            Color::TRANSPARENT,
            14.0,
            true,
            Some((
                Color::from_hex(0x181A1C).with_alpha(0.4),
                Color::from_hex(0x181A1C).with_alpha(0.8),
            )),
        );
        check_input_surface_appearance(false);
    }

    #[test]
    fn dropdown_pointer_bounds_combo_hover_and_captured_release() {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, Modifiers, PhysicalKeyCode, PointerButtonsState,
            PointerDevice, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::EdgeInsets;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::time::Instant;

        fn combo(tree: &WidgetTree, node: NodeId) -> &ComboBox {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn pointer(kind: PointerEventKind, pos: Point) -> FusedInputEvent {
            match kind {
                PointerEventKind::Down => FusedInputEvent::PointerDown {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                    click_count: 1,
                },
                PointerEventKind::Up => FusedInputEvent::PointerUp {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    device: PointerDevice::Mouse,
                    button: PointerButton::Primary,
                },
                PointerEventKind::Move => FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    delta: (0.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState::default(),
                },
                _ => unreachable!(),
            }
        }

        let mut control = ComboBox::simple(vec![
            "Alpha option with a wide popup label".into(),
            "Beta".into(),
            "Gamma".into(),
        ]);
        control.set_text_input_enabled(false);
        control.set_close_on_text_click(false);
        control.open_popup();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets {
                top: 200.0,
                left: 220.0,
                right: 20.0,
                bottom: 0.0,
            })),
            None,
            0,
        );
        let node = tree.add_child(root, Box::new(control), None, 0);
        let mut engine = FontManagerTextEngine::new();
        RetainedSceneTestHarness::new().frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(360.0, 260.0)),
            &Theme::light(),
            &mut engine,
            &IconRegistry::new(&[]),
        );
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        let local_popup = ComboBox::test_popup_local_rect(&tree, node);
        assert!(local_popup.y < 0.0, "exercise an above-anchor popup");
        assert!(local_popup.x < 0.0 && local_popup.width > rect.width);
        let popup = RectF::new(
            rect.x + local_popup.x,
            rect.y + local_popup.y,
            local_popup.width,
            local_popup.height,
        );
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut bindings = BindingRegistry::new();
        let mut send = |tree: &mut WidgetTree, event| {
            router.route_input_event(
                tree,
                event,
                Some(root),
                &mut engine,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        };
        let input = Point::new(rect.x + 3.0, rect.y + rect.height * 0.5);
        send(&mut tree, pointer(PointerEventKind::Down, input));
        send(&mut tree, pointer(PointerEventKind::Up, input));
        send(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Home,
                physical_key: PhysicalKeyCode::Home,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
        );
        let keyboard_id = combo(&tree, node).highlighted_id;
        assert_eq!(keyboard_id, Some(combo(&tree, node).snapshot.items[0].0));
        assert!(!combo(&tree, node).highlight_from_hover);
        let second = Point::new(popup.x + 20.0, popup.y + ROW_HEIGHT * 1.5);
        for outside in [
            Point::new(popup.x - 1.0, second.y),
            Point::new(popup.right(), second.y),
            Point::new(second.x, popup.y - 1.0),
            Point::new(second.x, popup.bottom()),
        ] {
            send(&mut tree, pointer(PointerEventKind::Move, outside));
            assert_eq!(combo(&tree, node).highlighted_id, keyboard_id);
            assert!(!combo(&tree, node).highlight_from_hover);
            assert!(matches!(
                combo(&tree, node).focus_zone,
                PopupFocusZone::OptionList
            ));
        }
        let second_id = combo(&tree, node).snapshot.items[1].0;
        for outside_x in [popup.x - 1.0, popup.right()] {
            // Enter from a different row: same-row motion preserves press/keyboard ownership.
            send(
                &mut tree,
                pointer(
                    PointerEventKind::Move,
                    Point::new(second.x, popup.y + ROW_HEIGHT * 0.5),
                ),
            );
            send(&mut tree, pointer(PointerEventKind::Move, second));
            assert_eq!(combo(&tree, node).highlighted_id, Some(second_id));
            assert!(combo(&tree, node).highlight_from_hover);
            let outside = Point::new(outside_x, second.y);
            send(&mut tree, pointer(PointerEventKind::Move, outside));
            assert_eq!(combo(&tree, node).highlighted_id, None);
            send(&mut tree, pointer(PointerEventKind::Down, second));
            assert!(combo(&tree, node).option_press_active);
            send(&mut tree, pointer(PointerEventKind::Up, outside));
            assert!(!combo(&tree, node).option_press_active);
            assert!(combo(&tree, node).committed_latch.is_none());
            assert!(combo(&tree, node).popup_open.get());
        }
        send(&mut tree, pointer(PointerEventKind::Down, second));
        send(&mut tree, pointer(PointerEventKind::Up, second));
        assert_eq!(
            combo(&tree, node).committed_value_string().as_deref(),
            Some("Beta")
        );
        assert!(!combo(&tree, node).popup_open.get());
    }

    fn collect<'a>(
        nodes: &'a [SceneNode],
        texts: &mut Vec<&'a TextNode>,
        rects: &mut Vec<&'a RectNode>,
        borders: &mut Vec<&'a BorderRectNode>,
        paths: &mut Vec<&'a PathNode>,
    ) {
        for node in nodes {
            match node {
                SceneNode::Text(text) => texts.push(text),
                SceneNode::Rect(rect) => rects.push(rect),
                SceneNode::BorderRect(border) => borders.push(border),
                SceneNode::Path(path) => paths.push(path),
                SceneNode::RetainedFragment(fragment) => {
                    collect(&fragment.children, texts, rects, borders, paths)
                }
                SceneNode::Clip(clip) => collect(&clip.children, texts, rects, borders, paths),
                SceneNode::Layer(layer) => collect(&layer.children, texts, rects, borders, paths),
                SceneNode::Scroll(scroll) => {
                    collect(&scroll.children, texts, rects, borders, paths)
                }
                _ => {}
            }
        }
    }

    #[test]
    fn explicit_text_z_open_combo_popup_preserves_strict_local_ladder() {
        let mut options = vec!["Visible popup row".to_string()];
        options.extend((1..=8).map(|index| format!("Option {index}")));
        let mut combo = ComboBox::simple(options);
        combo.open_popup();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(combo), None, 0);
        let mut harness = RetainedSceneTestHarness::new();
        let mut text_engine = FontManagerTextEngine::new();
        let frame = harness.frame(
            &mut tree,
            root,
            Constraints::tight(Size::new(240.0, 120.0)),
            &Theme::dark(),
            &mut text_engine,
            &IconRegistry::new(&[]),
        );

        let mut texts = Vec::new();
        let mut rects = Vec::new();
        let mut borders = Vec::new();
        let mut paths = Vec::new();
        collect(
            &frame.scene_root,
            &mut texts,
            &mut rects,
            &mut borders,
            &mut paths,
        );
        let row = texts
            .iter()
            .find(|text| text.text == "Visible popup row")
            .expect("an open combo box must paint its popup row");
        assert_eq!(row.z_index, POPUP_TEXT_LAYER);
        assert!(
            rects.iter().any(|rect| rect.z_index == POPUP_SURFACE_LAYER)
                || paths
                    .iter()
                    .any(|path| path.z_index == POPUP_SURFACE_LAYER && !path.is_stroke)
        );
        assert!(
            rects.iter().any(|rect| rect.z_index == POPUP_CHROME_LAYER),
            "a scrolling popup must paint its scrollbar above row backgrounds"
        );
        assert!(
            paths
                .iter()
                .any(|path| path.z_index == POPUP_BORDER_LAYER && path.is_stroke),
            "the popup must retain a visible border above its opaque surface"
        );
        assert!(POPUP_SURFACE_LAYER < POPUP_TEXT_LAYER);
        assert!(POPUP_TEXT_LAYER < POPUP_CHROME_LAYER);
        assert!(POPUP_CHROME_LAYER < POPUP_BORDER_LAYER);
    }

    #[test]
    fn combo_closed_hover_focus_preserves_independent_cues() {
        use crate::core::app::settle_test_widget_availability;
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode,
            PointerButtonsState, PointerDevice, PointerId, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::input_fusion::InputFusion;
        use crate::core::layout::EdgeInsets;
        use crate::core::paint::{Color, ColorStop, GradientDef, GradientKind, SurfaceGradient};
        use crate::runtime::actions::ActionId;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{button::Button, flex::Flex};
        use std::{rc::Rc, time::Instant};

        let gradient = |color| {
            SurfaceGradient::new(GradientDef {
                kind: GradientKind::Linear,
                stops: vec![
                    ColorStop { offset: 0.0, color },
                    ColorStop { offset: 1.0, color },
                ],
                p0: (0.0, 0.0),
                p1: (0.0, 1.0),
            })
            .unwrap()
        };
        let idle = Color::from_hex(0x181B20);
        let hover = Color::from_hex(0x32363B);
        let active = Color::from_hex(0x23282D);
        let open = Color::from_hex(0x161D24);
        let focus_cue = Color::from_hex(0x9A958E);
        let capture_cue = Color::from_hex(0xE2D5BF);
        let clear = Color::TRANSPARENT;
        let appearance = InputSurfaceAppearance::try_new(
            gradient(idle),
            clear,
            clear,
            clear,
            open,
            hover,
            8.0,
        )
        .unwrap()
        .with_state_surfaces(gradient(hover), gradient(active))
        .with_open_surface(gradient(open))
        .try_with_focus_colors(focus_cue, capture_cue)
        .unwrap();

        for preferred in [false, true] {
            let commits = Rc::new(Cell::new(0));
            let count = Rc::clone(&commits);
            let mut combo = ComboBox::simple(vec!["Individual".into(), "Company".into()])
                .with_input_surface_appearance(appearance.clone())
                .with_interaction_policy(PopupInteractionPolicy {
                    width: PopupWidthPolicy::MatchTrigger,
                });
            if preferred {
                combo = combo.with_hover_surface_when_closed_and_focused();
            }
            combo.set_text_input_enabled(false);
            combo.set_input_text("Individual".into()).unwrap();
            combo.set_on_committed(move |_| count.set(count.get() + 1));
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_padding(EdgeInsets::all(12.0)).with_spacing(6.0)),
                None,
                0,
            );
            let node = tree.add_child(root, Box::new(combo), None, 0);
            let next = tree.add_child(root, Box::new(Button::with_text("Next")), None, 0);
            let owner = tree.node(node).unwrap().id;
            let next_owner = tree.node(next).unwrap().id;
            let mut harness = RetainedSceneTestHarness::new();
            let mut engine = FontManagerTextEngine::new();
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let mut bindings = BindingRegistry::new();
            let theme = Theme::dark();
            let icons = IconRegistry::new(&[]);
            let mut now = 0.0;
            macro_rules! combo {
                () => {
                    tree.node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<ComboBox>()
                        .unwrap()
                };
            }
            macro_rules! paint {
                ($face:expr, $cue:expr) => {{
                    harness.advance_transition_time(now);
                    harness.frame_with_appearance(
                        &mut tree,
                        root,
                        Constraints::tight(Size::new(240.0, 180.0)),
                        &theme,
                        &mut engine,
                        &icons,
                        &domains,
                        WindowId::PRIMARY,
                    );
                    now += 0.050;
                    let fragment = harness.local_fragment(owner).unwrap();
                    let nodes = surface_nodes(&fragment);
                    let face = gradient($face);
                    assert!(
                        nodes.iter().any(|n| matches!(n, SceneNode::GradientRect(g)
                            if &g.gradient == face.definition())),
                        "expected face {:?} at time {now}",
                        face.definition().stops[0].color
                    );
                    let cues: Vec<_> = nodes
                        .iter()
                        .filter_map(|n| match n {
                            SceneNode::BorderRect(b) if b.z_index == LAYER_FOCUS + 1 => {
                                Some(b.border_color)
                            }
                            // The arrow can share this layer. The joined
                            // perimeter spans the shell beyond the affordance.
                            SceneNode::Path(p)
                                if p.is_stroke
                                    && p.z_index == LAYER_FOCUS + 1
                                    && p.path.bounds().width > theme.geometry.combo_box.affordance_width =>
                            {
                                Some(p.color)
                            }
                            _ => None,
                        })
                        .collect();
                    let expected: Option<Color> = $cue;
                    if let Some(color) = expected {
                        assert_eq!(cues, [color.into()]);
                    } else {
                        assert!(cues.is_empty());
                    }
                }};
            }
            macro_rules! route {
                ($event:expr) => {
                    router.route_input_event(
                        &mut tree,
                        $event,
                        Some(root),
                        &mut engine,
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    )
                };
            }
            let move_to = |pos| FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                delta: (0.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState::default(),
            };
            let down = |pos| FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            };
            let up = |pos| FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            };
            let key = |key_code, physical_key| FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            };
            let focus_hover = if preferred { hover } else { active };
            paint!(idle, None);
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
            macro_rules! press_arrow {
                () => {{
                    route!(down(arrow));
                    route!(up(arrow));
                }};
            }

            route!(move_to(arrow));
            paint!(hover, None);
            router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
            paint!(focus_hover, Some(focus_cue));
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            route!(move_to(Point::new(4.0, 172.0)));
            paint!(active, Some(focus_cue));
            route!(move_to(arrow));
            paint!(focus_hover, Some(focus_cue));
            route!(key(KeyCode::Tab, PhysicalKeyCode::Tab));
            assert_eq!(
                domains.primary_state().focus_manager.focused(),
                Some(next_owner)
            );
            paint!(hover, None);

            press_arrow!();
            assert!(combo!().popup_open.get());
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );
            paint!(open, Some(capture_cue));
            paint!(open, Some(capture_cue));
            press_arrow!();
            assert!(!combo!().popup_open.get());
            paint!(open, Some(capture_cue));
            paint!(active, Some(capture_cue));
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Capture
            );

            press_arrow!();
            paint!(open, Some(capture_cue));
            paint!(open, Some(capture_cue));
            let popup = ComboBox::test_popup_local_rect(&tree, node);
            let row = Point::new(rect.x + popup.x + 12.0, rect.y + popup.y + ROW_HEIGHT * 0.5);
            route!(down(row));
            assert_eq!(
                domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                Some(owner)
            );
            assert!(router.handle_engine_action(
                &mut tree,
                ActionId::RELEASE_CAPTURE,
                Some(root),
                domains.primary_state_mut(),
                &mut engine,
                Some(harness.animation_context()),
            ));
            assert!(!combo!().popup_open.get());
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            paint!(open, Some(focus_cue));
            paint!(active, Some(focus_cue));
            assert!(combo!().cell_hovered);
            assert!(
                domains
                    .primary_state()
                    .pointer_capture_manager
                    .is_captured(PointerId::MOUSE)
            );
            route!(up(row));
            route!(move_to(arrow));
            paint!(focus_hover, Some(focus_cue));
            assert_eq!(commits.get(), 0);
            assert_eq!(combo!().input_text(), "Individual");

            press_arrow!();
            paint!(open, Some(capture_cue));
            paint!(open, Some(capture_cue));
            route!(key(KeyCode::End, PhysicalKeyCode::End));
            route!(key(KeyCode::Enter, PhysicalKeyCode::Enter));
            assert_eq!(commits.get(), 1);
            assert_eq!(combo!().input_text(), "Company");
            assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            paint!(open, Some(focus_cue));
            paint!(focus_hover, Some(focus_cue));

            tree.request_widget_enabled(node, false).unwrap();
            assert!(
                settle_test_widget_availability(
                    &mut tree,
                    &mut router,
                    &mut InputFusion::new(),
                    domains.primary_state_mut(),
                    &mut bindings,
                    &mut engine,
                )
                .rejected
                .is_empty()
            );
            paint!(idle, None);
            assert!(!combo!().enabled);
            assert_eq!(combo!().input_text(), "Company");
            assert_eq!(commits.get(), 1);
        }
    }

    #[test]
    fn combo_popup_outside_press_hands_off_the_original_click() {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PointerDevice,
            PointerId, ScrollSource, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::{EdgeInsets, FlexParams};
        use crate::runtime::actions::ActionId;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{container::Container, flex::Flex, text_field::TextField};
        use std::{rc::Rc, time::Instant};

        fn combo_at(tree: &WidgetTree, node: NodeId) -> &ComboBox {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .unwrap()
        }

        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::row().with_padding(EdgeInsets::all(12.0)).with_spacing(12.0)),
            None,
            0,
        );
        let mount = |tree: &mut WidgetTree, width, widget: Box<dyn Widget>| {
            let holder = tree.add_child(
                root,
                Box::new(Container::empty().with_width(width).with_height(38.0)),
                None,
                0,
            );
            let node = tree.add_child(holder, widget, None, 0);
            tree.set_flex(
                node,
                FlexParams {
                    grow: 1.0,
                    shrink: 1.0,
                    basis: 0.0,
                },
            );
            node
        };
        let dismissed = [Rc::new(Cell::new(0)), Rc::new(Cell::new(0))];
        let committed = Rc::new(Cell::new(0));
        let combos = std::array::from_fn(|index| {
            let mut combo =
                ComboBox::simple((0..12).map(|row| format!("Option {row}")).collect())
                    .with_interaction_policy(PopupInteractionPolicy {
                        width: PopupWidthPolicy::MatchTrigger,
                    })
                    .with_popup_scrollbar_policy(PopupScrollbarPolicy::AutoHide);
            combo.set_text_input_enabled(false);
            combo.set_input_text(format!("Draft {index}")).unwrap();
            let cancelled = Rc::clone(&dismissed[index]);
            combo.set_on_cancelled(move || cancelled.set(cancelled.get() + 1));
            let count = Rc::clone(&committed);
            combo.set_on_committed(move |_| count.set(count.get() + 1));
            mount(&mut tree, 132.0, Box::new(combo))
        });
        let [a, b] = combos;
        let email = mount(&mut tree, 156.0, Box::new(TextField::new().with_text("mail")));
        let email_owner = tree.node(email).unwrap().id;
        let mut router = EventRouter::new();
        let mut domains = InteractionDomains::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut engine = FontManagerTextEngine::new();
        let theme = Theme::light();
        let icons = IconRegistry::new(&[]);
        let mut now = 0.0;
        macro_rules! frame {
            () => {{
                harness.advance_transition_time(now);
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(480.0, 282.0)),
                    &theme,
                    &mut engine,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                );
                now += 0.050;
            }};
        }
        macro_rules! route {
            ($event:expr) => {
                router.route_input_event(
                    &mut tree,
                    $event,
                    Some(root),
                    &mut engine,
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                )
            };
        }
        let down = |pos| FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        };
        let up = |pos| FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos,
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        };
        macro_rules! click {
            ($point:expr) => {{
                route!(down($point));
                route!(up($point));
            }};
        }
        macro_rules! assert_focus_capture {
            ($owner:expr) => {{
                assert_eq!(
                    domains.primary_state().focus_manager.focused(),
                    Some($owner)
                );
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Capture
                );
            }};
        }
        frame!();
        let rects = combos.map(|node| tree.get_absolute_paint_rect(node).unwrap());
        let arrows = rects.map(|rect| Point::new(rect.right() - 4.0, rect.y + 19.0));
        click!(arrows[0]);
        assert!(combo_at(&tree, a).popup_open.get());
        frame!();
        frame!();
        for (from, to) in [(0, 1), (1, 0)] {
            let old_owner = tree.node(combos[from]).unwrap().id;
            let dest_owner = tree.node(combos[to]).unwrap().id;
            let old_popup = ComboBox::test_popup_local_rect(&tree, combos[from]);
            let old_row = Point::new(
                rects[from].x + old_popup.x + 12.0,
                rects[from].y + old_popup.y + ROW_HEIGHT * 0.5,
            );
            assert_eq!(
                tree.hit_test(old_row.x, old_row.y).unwrap().widget_id,
                old_owner
            );
            route!(down(arrows[to]));
            assert!(!combo_at(&tree, combos[from]).popup_open.get());
            assert!(combo_at(&tree, combos[to]).popup_open.get());
            assert_eq!(
                combo_at(&tree, combos[from]).input_text(),
                format!("Draft {from}")
            );
            assert_focus_capture!(dest_owner);
            assert_ne!(
                tree.hit_test(old_row.x, old_row.y).map(|hit| hit.widget_id),
                Some(old_owner)
            );
            route!(up(arrows[to]));
            assert!(combo_at(&tree, combos[to]).popup_open.get());
            assert_eq!(dismissed[from].get(), 1);
            frame!();
            frame!();
        }

        let email_rect = tree.get_absolute_paint_rect(email).unwrap();
        let caret_start = Point::new(
            email_rect.x + theme.geometry.text_field.padding_x + 1.0,
            email_rect.y + 19.0,
        );
        route!(down(caret_start));
        assert!(!combo_at(&tree, a).popup_open.get());
        assert_eq!(dismissed[0].get(), 2);
        assert_focus_capture!(email_owner);
        assert_eq!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(email_owner)
        );
        route!(up(caret_start));
        assert!(
            !domains
                .primary_state()
                .pointer_capture_manager
                .is_captured(PointerId::MOUSE)
        );
        route!(FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: "x".into(),
            modifiers: Modifiers::default(),
        });
        let field = tree
            .node(email)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<TextField>()
            .unwrap();
        assert_eq!(field.text(), "xmail");

        click!(arrows[1]);
        frame!();
        frame!();
        assert!(combo_at(&tree, b).popup_open.get());
        click!(Point::new(8.0, 272.0));
        assert!(!combo_at(&tree, b).popup_open.get());
        assert_eq!(dismissed[1].get(), 2);
        assert_eq!(combo_at(&tree, b).input_text(), "Draft 1");
        assert_eq!(committed.get(), 0);

        click!(arrows[0]);
        frame!();
        frame!();
        let popup = ComboBox::test_popup_local_rect(&tree, a).translate(rects[0].x, rects[0].y);
        let row = Point::new(popup.x + 12.0, popup.y + ROW_HEIGHT * 1.5);
        route!(FusedInputEvent::Scroll {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: row,
            delta: (0.0, -ROW_HEIGHT),
            source: ScrollSource::Wheel,
        });
        frame!();
        assert_eq!(combo_at(&tree, a).popup_scroll_row, 1);
        let thumb = tree
            .combo_popup_input_geometry(tree.node(a).unwrap().id)
            .unwrap()
            .drawn_scrollbar()
            .unwrap()
            .thumb;
        let grab = Point::new(thumb.x + thumb.width * 0.5, thumb.y + thumb.height * 0.5);
        route!(down(grab));
        assert!(combo_at(&tree, a).scrollbar_v.dragging);
        route!(up(grab));
        assert!(combo_at(&tree, a).popup_open.get());
        assert!(!combo_at(&tree, a).scrollbar_v.dragging);
        route!(down(row));
        assert!(combo_at(&tree, a).popup_open.get());
        assert_eq!(committed.get(), 0);
        route!(up(row));
        assert!(!combo_at(&tree, a).popup_open.get());
        assert_eq!(committed.get(), 1);
        assert_eq!(
            combo_at(&tree, a).committed_value_string().as_deref(),
            Some("Option 2")
        );

        click!(arrows[1]);
        assert!(combo_at(&tree, b).popup_open.get());
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::RELEASE_CAPTURE,
            Some(root),
            domains.primary_state_mut(),
            &mut engine,
            Some(harness.animation_context()),
        ));
        assert!(!combo_at(&tree, b).popup_open.get());
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Navigation
        );
        assert_eq!(dismissed[1].get(), 3);
        assert_eq!(committed.get(), 1);
    }

    #[test]
    fn combo_legacy_type_to_search_model_preview_preserves_ghost_and_options() {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode,
            WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::EdgeInsets;
        use crate::model::selection_resolver::{RejectionReason, SelectionResolver};
        use crate::runtime::actions::ActionId;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::{cell::RefCell, rc::Rc, time::Instant};

        #[derive(Debug, PartialEq, Eq)]
        struct ResolveCall {
            source: CommitSource,
            text: Option<String>,
            highlighted: Option<OptionId>,
            options: Vec<(OptionId, String)>,
        }

        struct Resolver(Rc<RefCell<Vec<ResolveCall>>>);
        impl SelectionResolver for Resolver {
            fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                self.0.borrow_mut().push(ResolveCall {
                    source: input.commit_source,
                    text: input.input_text.map(str::to_owned),
                    highlighted: input.highlighted_id,
                    options: input
                        .options
                        .iter()
                        .map(|entry| (entry.id, entry.label.to_owned()))
                        .collect(),
                });
                ResolverDecision::Reject(RejectionReason::NoMatch)
            }
        }

        let [substring, first, second, unrelated] = [401, 87, 3, 502].map(OptionId::from_raw);
        let options = [
            (substring, "Old i\u{0307} in middle"),
            (first, "İzmir"),
            (second, "İznik"),
            (unrelated, "Else"),
        ];
        let calls = Rc::new(RefCell::new(Vec::new()));
        let mut combo = ComboBox::new(ResolverHandle::new(Resolver(calls.clone())));
        combo.set_type_to_search(true);
        combo.set_match_mode(MatchMode::Substring);
        combo.set_projection(&options, None, 0, 4);

        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets::all(12.0))),
            None,
            0,
        );
        let node = tree.add_child(root, Box::new(combo), None, 0);
        let owner = tree.node(node).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut engine = FontManagerTextEngine::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let theme = Theme::light();
        let icons = IconRegistry::new(&[]);
        macro_rules! combo {
            () => {
                tree.node(node)
                    .unwrap()
                    .widget
                    .as_any()
                    .downcast_ref::<ComboBox>()
                    .unwrap()
            };
            (mut) => {
                tree.node_mut(node)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<ComboBox>()
                    .unwrap()
            };
        }
        macro_rules! frame {
            ($time:expr) => {{
                harness.advance_transition_time($time);
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    Constraints::tight(Size::new(240.0, 180.0)),
                    &theme,
                    &mut engine,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                );
            }};
        }
        macro_rules! route {
            ($event:expr) => {
                router.route_input_event(
                    &mut tree,
                    $event,
                    Some(root),
                    &mut engine,
                    Some(harness.animation_context()),
                    domains.primary_state_mut(),
                    &mut bindings,
                )
            };
        }
        let text = |value: &str| FusedInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            text: value.into(),
            modifiers: Modifiers::default(),
        };
        let key = |key_code, physical_key| FusedInputEvent::KeyDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            key_code,
            physical_key,
            reservation: None,
            modifiers: Modifiers::default(),
            repeat: false,
        };
        let backspace = || key(KeyCode::Backspace, PhysicalKeyCode::Backspace);
        let assert_options = |combo: &ComboBox, expected: &[(OptionId, &str)]| {
            assert!(
                combo
                    .snapshot
                    .items
                    .iter()
                    .map(|(id, label)| (*id, label.as_str()))
                    .eq(expected.iter().copied())
            );
        };
        let assert_ghost = |combo: &ComboBox, id: OptionId, label: &str, suffix: &str| {
            assert_eq!(combo.highlighted_id, Some(id));
            assert_eq!(combo.ghost_completion.as_deref(), Some(suffix));
            let (revision, source_id, source_label) = combo.ghost_source.as_ref().unwrap();
            assert_eq!(
                *revision,
                Some(combo.text_field.snapshot.as_ref().unwrap().revision)
            );
            assert_eq!(*source_id, Some(id.raw()));
            assert_eq!(source_label, label);
        };

        frame!(0.0);
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        assert!(router.handle_engine_action(
            &mut tree,
            ActionId::ENTER_WIDGET,
            Some(root),
            domains.primary_state_mut(),
            &mut engine,
            Some(harness.animation_context()),
        ));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        frame!(0.01);
        frame!(0.07);
        assert!(combo!().popup_open.get());
        assert!(combo!().search_harness.is_none());

        let prefix = "i\u{0307}";
        route!(text("i\u{0307}x"));
        route!(backspace());
        assert_eq!(combo!().input_text(), prefix);
        assert_options(combo!(), &options);
        assert!(combo!().snapshot.match_ranges.is_empty());
        assert_ghost(combo!(), first, "İzmir", "mir");
        let previous_source = combo!().ghost_source.clone();
        frame!(0.08);
        assert!(
            surface_nodes(&harness.local_fragment(owner).unwrap())
                .iter()
                .any(|node| matches!(node, SceneNode::Text(text) if text.text == "mir"))
        );

        route!(text("zx"));
        route!(backspace());
        assert_eq!(combo!().input_text(), "i\u{0307}z");
        assert_ghost(combo!(), first, "İzmir", "ir");
        assert_ne!(combo!().ghost_source, previous_source);

        route!(text("!x"));
        route!(backspace());
        assert_eq!(combo!().input_text(), "i\u{0307}z!");
        assert_options(combo!(), &options);
        assert!(combo!().popup_open.get());
        assert!(combo!().highlighted_id.is_none());
        assert!(combo!().ghost_completion.is_none());
        assert!(combo!().ghost_source.is_none());
        route!(backspace());
        route!(backspace());
        assert_ghost(combo!(), first, "İzmir", "mir");
        route!(backspace());
        assert_eq!(combo!().input_text(), "");
        assert_options(combo!(), &options);
        assert!(combo!().highlighted_id.is_none());
        assert!(combo!().ghost_completion.is_none());
        assert!(combo!().ghost_source.is_none());

        route!(text("i\u{0307}x"));
        route!(backspace());
        combo!(mut).set_loading(true);
        assert!(!combo!().popup_open.get());
        route!(text("x"));
        route!(backspace());
        assert_eq!(combo!().input_text(), prefix);
        assert!(combo!().highlighted_id.is_none());
        assert!(combo!().ghost_source.is_none());
        let replacement = [
            (substring, "Old i\u{0307} in middle"),
            (second, "İznik"),
            (first, "İzmirar"),
            (unrelated, "Else"),
        ];
        combo!(mut).set_projection(&replacement, Some(first), 0, 4);
        assert_eq!(combo!().highlighted_id, Some(first));
        assert!(combo!().ghost_completion.is_none());
        combo!(mut).set_loading(false);
        assert!(combo!().popup_open.get());
        assert_ghost(combo!(), first, "İzmirar", "mirar");
        assert!(calls.borrow().is_empty());

        route!(key(KeyCode::Enter, PhysicalKeyCode::Enter));
        assert_eq!(
            calls.borrow().as_slice(),
            &[ResolveCall {
                source: CommitSource::InputConfirm,
                text: Some(prefix.into()),
                highlighted: Some(first),
                options: replacement
                    .iter()
                    .map(|(id, label)| (*id, (*label).into()))
                    .collect(),
            }]
        );
        assert!(combo!().committed_latch.is_none());

        combo!(mut).set_search_harness(SearchProjectionHarness::new(&replacement));
        route!(text("x"));
        route!(backspace());
        assert_eq!(combo!().input_text(), prefix);
        assert_options(combo!(), &replacement[..3]);
        assert_eq!(combo!().snapshot.match_ranges, [(4, 3), (0, 3), (0, 3)]);
        assert_eq!(combo!().highlighted_id, Some(substring));
        assert!(combo!().ghost_completion.is_none());
        assert!(combo!().ghost_source.is_none());
        assert_eq!(calls.borrow().len(), 1);
    }

    #[test]
    fn combo_type_to_select_routes_current_labels_and_preserves_commit_authority() {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode,
            PointerDevice, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::EdgeInsets;
        use crate::model::selection_resolver::{RejectionReason, SelectionResolver};
        use crate::runtime::actions::ActionId;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::{cell::RefCell, rc::Rc, time::Instant};

        struct Resolver(Rc<RefCell<Vec<(CommitSource, Option<OptionId>)>>>);
        impl SelectionResolver for Resolver {
            fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                self.0
                    .borrow_mut()
                    .push((input.commit_source, input.highlighted_id));
                match (input.commit_source, input.highlighted_id) {
                    (CommitSource::Selection, Some(id)) => {
                        ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                    }
                    _ => ResolverDecision::Reject(RejectionReason::NoMatch),
                }
            }
        }

        let options: Vec<_> = ["Norway", "Canada", "Japan", "Italy", "Kenya", "Sweden", "Сербия"]
            .into_iter()
            .enumerate()
            .map(|(index, label)| (OptionId::from_raw(100 + index as u64 * 7), label))
            .collect();
        for opted_in in [false, true] {
            let commits = Rc::new(RefCell::new(Vec::new()));
            let typed = Rc::new(RefCell::new(Vec::new()));
            let mut combo = ComboBox::new(ResolverHandle::new(Resolver(commits.clone())))
                .with_popup_scrollbar_policy(PopupScrollbarPolicy::AutoHide);
            if opted_in {
                let expected = options.clone();
                let typed = typed.clone();
                combo = combo.with_type_to_select(move |prefix, entries| {
                    assert!(
                        entries
                            .iter()
                            .map(|entry| (entry.id, entry.label))
                            .eq(expected.iter().copied())
                    );
                    typed.borrow_mut().push(prefix.to_owned());
                    if prefix == "x" {
                        return Some(OptionId::from_raw(999));
                    }
                    let prefix = prefix.to_lowercase();
                    entries
                        .iter()
                        .find(|entry| entry.label.to_lowercase().starts_with(&prefix))
                        .map(|entry| entry.id)
                });
            }
            combo.set_text_input_enabled(false);
            combo.set_type_to_search(false);
            combo.set_projection(&options, None, 0, 4);
            combo.set_input_text("Norway".into()).unwrap();

            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(Flex::column().with_padding(EdgeInsets::all(12.0))),
                None,
                0,
            );
            let node = tree.add_child(root, Box::new(combo), None, 0);
            let owner = tree.node(node).unwrap().id;
            let mut harness = RetainedSceneTestHarness::new();
            let mut engine = FontManagerTextEngine::new();
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let mut bindings = BindingRegistry::new();
            let theme = Theme::light();
            let icons = IconRegistry::new(&[]);
            macro_rules! combo {
                () => {
                    tree.node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<ComboBox>()
                        .unwrap()
                };
            }
            macro_rules! frame {
                ($time:expr) => {{
                    harness.advance_transition_time($time);
                    harness.frame_with_appearance(
                        &mut tree,
                        root,
                        Constraints::tight(Size::new(240.0, 180.0)),
                        &theme,
                        &mut engine,
                        &icons,
                        &domains,
                        WindowId::PRIMARY,
                    );
                }};
            }
            macro_rules! route {
                ($event:expr) => {
                    router.route_input_event(
                        &mut tree,
                        $event,
                        Some(root),
                        &mut engine,
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    )
                };
            }
            let key = |time, key_code, physical_key, modifiers| FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time,
                key_code,
                physical_key,
                reservation: None,
                modifiers,
                repeat: false,
            };
            let text = |time, text: &str, modifiers| FusedInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time,
                text: text.into(),
                modifiers,
            };
            macro_rules! type_key {
                ($logical:expr, $physical:expr, $text:expr, $shift:expr) => {{
                    let time = Instant::now();
                    let modifiers = Modifiers {
                        shift: $shift,
                        ..Modifiers::default()
                    };
                    route!(key(time, KeyCode::Character($logical), $physical, modifiers));
                    route!(text(time, $text, modifiers));
                }};
            }

            frame!(0.0);
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
            macro_rules! open_popup {
                () => {{
                    route!(FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: arrow,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    });
                    route!(FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: arrow,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    });
                    assert!(combo!().popup_open.get());
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        KeyboardAuthorityMode::Capture
                    );
                }};
            }
            open_popup!();
            frame!(0.01);
            frame!(0.06);
            type_key!('s', PhysicalKeyCode::KeyS, "S", true);
            if !opted_in {
                assert_eq!(combo!().highlighted_id, None);
                for (character, expected) in [("j", "Norway"), ("j", "Canada"), ("k", "Norway")] {
                    route!(text(Instant::now(), character, Modifiers::default()));
                    assert_eq!(combo!().test_highlighted_label(), Some(expected));
                }
                route!(text(Instant::now(), "i", Modifiers::default()));
                assert_eq!(
                    &*commits.borrow(),
                    &[(CommitSource::Selection, Some(options[0].0))]
                );
                assert_eq!(combo!().committed_value_string().as_deref(), Some("Norway"));
                continue;
            }

            assert_eq!(combo!().test_highlighted_label(), Some("Sweden"));
            assert_eq!(combo!().popup_scroll_row, 2);
            type_key!('j', PhysicalKeyCode::KeyJ, "j", false);
            assert_eq!(combo!().test_highlighted_label(), Some("Japan"));
            type_key!('k', PhysicalKeyCode::KeyK, "k", false);
            assert_eq!(combo!().test_highlighted_label(), Some("Kenya"));
            type_key!('i', PhysicalKeyCode::KeyI, "i", false);
            assert_eq!(combo!().test_highlighted_label(), Some("Italy"));
            // The physical-key fallback is ASCII while composed text is Cyrillic.
            type_key!('c', PhysicalKeyCode::KeyC, "С", true);
            assert_eq!(combo!().test_highlighted_label(), Some("Сербия"));
            assert_eq!(combo!().popup_scroll_row, 3);
            frame!(0.07);
            let overlay = harness.overlay_fragment(owner).expect("open popup");
            let painted = surface_nodes(&overlay);
            assert!(painted.iter().any(|node| matches!(node, SceneNode::Text(text)
                if text.text == "Сербия" && text.z_index == POPUP_TEXT_LAYER)));
            assert!(painted.iter().any(|node| matches!(node, SceneNode::Rect(rect)
                if rect.z_index == POPUP_CHROME_LAYER)));

            type_key!('z', PhysicalKeyCode::KeyZ, "z", false);
            type_key!('x', PhysicalKeyCode::KeyX, "x", false);
            assert_eq!(&*typed.borrow(), &["S", "j", "k", "i", "С", "z", "x"]);
            assert_eq!(combo!().test_highlighted_label(), Some("Сербия"));
            assert_eq!(combo!().input_text(), "Norway");
            assert!(combo!().committed_latch.is_none());
            assert!(commits.borrow().is_empty());
            assert!(combo!().popup_open.get());
            assert!(router.handle_engine_action(
                &mut tree,
                ActionId::RELEASE_CAPTURE,
                Some(root),
                domains.primary_state_mut(),
                &mut engine,
                Some(harness.animation_context()),
            ));
            assert!(!combo!().popup_open.get());
            assert_eq!(combo!().highlighted_id, None);
            assert_eq!(combo!().input_text(), "Norway");
            assert!(commits.borrow().is_empty());
            frame!(0.08);
            frame!(0.12);

            open_popup!();
            type_key!('c', PhysicalKeyCode::KeyC, "С", true);
            route!(key(
                Instant::now(),
                KeyCode::Enter,
                PhysicalKeyCode::Enter,
                Modifiers::default(),
            ));
            assert_eq!(
                &*commits.borrow(),
                &[(CommitSource::Selection, Some(options[6].0))]
            );
            assert_eq!(combo!().committed_value_string().as_deref(), Some("Сербия"));
            assert_eq!(combo!().input_text(), "Сербия");
            assert!(!combo!().popup_open.get());
        }
    }

    #[test]
    fn hal397_popup_tap_commits_once_and_pan_and_dismissal_do_not_select() {
        use crate::core::event::{InteractionDomains, RawInputEvent, TouchPhase, WindowId};
        use crate::core::event_router::EventRouter;
        use crate::core::input_fusion::InputFusion;
        use crate::core::layout::EdgeInsets;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{button::Button, flex::Flex};
        use std::{cell::Cell, rc::Rc};
        use web_time::Instant;

        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets::all(12.0))),
            None,
            0,
        );
        let clicks = Rc::new(Cell::new(0));
        let count = clicks.clone();
        let button = tree.add_child(
            root,
            Box::new(Button::with_text("Other").on_click(move || count.set(count.get() + 1))),
            None,
            0,
        );
        let mut combo = ComboBox::simple((0..32).map(|i| format!("Option {i}")).collect());
        combo.set_text_input_enabled(false);
        let combo = tree.add_child(root, Box::new(combo), None, 1);
        let owner = tree.node(combo).unwrap().id;
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut fusion = InputFusion::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut text = FontManagerTextEngine::new();
        let theme = Theme::dark();
        let icons = IconRegistry::new(&[]);
        macro_rules! frame {
            ($seconds:expr) => {{
                harness.advance_transition_time($seconds);
                harness.frame_with_appearance(
                    &mut tree, root, Constraints::tight(Size::new(390.0, 450.0)),
                    &theme, &mut text, &icons, &domains, WindowId::PRIMARY,
                );
            }};
        }
        macro_rules! combo {
            () => { tree.node(combo).unwrap().widget.as_any().downcast_ref::<ComboBox>().unwrap() };
        }
        macro_rules! touch {
            ($id:expr, $phase:ident, $position:expr) => {{
                let state = domains.primary_state_mut();
                let mut events = Vec::new();
                fusion.process_raw_with_gesture_sessions(
                    RawInputEvent::Touch {
                        window_id: WindowId::PRIMARY, time: Instant::now(), contact_id: $id,
                        phase: TouchPhase::$phase, pos: $position,
                    },
                    state.focus_manager.interaction_domain_id(), &mut state.gesture_registry,
                    &tree, None, &mut events,
                );
                fusion.flush_with_gesture_sessions(&mut events);
                for event in events {
                    router.route_session_input_event(
                        &mut tree, event, Some(root), &mut text,
                        Some(harness.animation_context()), state, &mut bindings,
                    );
                }
            }};
        }
        let row = |tree: &WidgetTree, index: f32| {
            let rows = tree.combo_popup_input_geometry(owner).unwrap().rows;
            let trigger = tree.get_absolute_paint_rect(combo).unwrap();
            Point::new(trigger.x + rows.x + 8.0, trigger.y + rows.y + ROW_HEIGHT * (index + 0.5))
        };
        frame!(0.0);
        let trigger = tree.get_absolute_paint_rect(combo).unwrap();
        let trigger = Point::new(trigger.right() - 5.0, trigger.y + trigger.height / 2.0);
        touch!(1, Started, trigger);
        assert!(!combo!().popup_open.get());
        touch!(1, Ended, trigger);
        assert!(combo!().popup_open.get());
        frame!(0.0);
        frame!(1.0);

        let position = row(&tree, 4.0);
        touch!(2, Started, position);
        assert!(combo!().committed_value_string().is_none());
        let end = Point::new(position.x, position.y - 2.0 * ROW_HEIGHT);
        touch!(2, Moved, end);
        touch!(2, Ended, end);
        assert_eq!(combo!().popup_scroll_row, 2);
        assert!(combo!().committed_value_string().is_none());
        frame!(1.1);
        let position = row(&tree, 0.0);
        touch!(3, Started, position);
        assert!(combo!().committed_value_string().is_none());
        touch!(3, Ended, position);
        assert_eq!(combo!().committed_value_string().as_deref(), Some("Option 2"));
        touch!(3, Ended, position);
        assert!(!combo!().popup_open.get());

        touch!(4, Started, trigger);
        touch!(4, Ended, trigger);
        frame!(1.2);
        frame!(2.0);
        let outside = tree.get_absolute_paint_rect(button).unwrap().center();
        touch!(5, Started, outside);
        assert!(combo!().popup_open.get());
        touch!(5, Ended, outside);
        assert!(!combo!().popup_open.get());
        assert_eq!(clicks.get(), 0);
    }

    struct MountedHostChoice {
        tree: WidgetTree,
        root: NodeId,
        scroll: NodeId,
        combo: NodeId,
        domains: crate::core::event::InteractionDomains,
        router: crate::core::event_router::EventRouter,
        fusion: crate::core::input_fusion::InputFusion,
        text: FontManagerTextEngine,
        bindings: crate::runtime::binding_registry::BindingRegistry,
        frame: RetainedSceneTestHarness,
        calls: std::rc::Rc<std::cell::RefCell<Vec<(CommitSource, Option<OptionId>)>>>,
    }

    impl MountedHostChoice {
        const OPTIONS: [(OptionId, &'static str); 3] = [
            (OptionId::from_raw(912), "Same"),
            (OptionId::from_raw(0), "Same"),
            (OptionId::from_raw(u64::MAX), "Other"),
        ];
        const VIEWPORT: RectF = RectF::new(0.0, 0.0, 390.0, 160.0);

        fn new(editable: bool) -> Self {
            Self::with_top_padding(editable, 0.0)
        }

        fn with_top_padding(editable: bool, top: f32) -> Self {
            use crate::core::event::{InteractionDomains, WindowId};
            use crate::core::event_router::{EventRouter, host_choice::HostOffer};
            use crate::core::input_fusion::InputFusion;
            use crate::core::layout::{EdgeInsets, FlexParams};
            use crate::model::selection_resolver::{RejectionReason, SelectionResolver};
            use crate::runtime::binding_registry::BindingRegistry;
            use crate::widgets::{fixed_spacer::FixedSpacer, flex::Flex, scroll_area::ScrollArea};
            use std::{cell::RefCell, rc::Rc};

            struct Resolver(Rc<RefCell<Vec<(CommitSource, Option<OptionId>)>>>);
            impl SelectionResolver for Resolver {
                fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                    self.0.borrow_mut().push((input.commit_source, input.highlighted_id));
                    match input.highlighted_id {
                        Some(id) if id != OptionId::from_raw(u64::MAX) => {
                            ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                        }
                        _ => ResolverDecision::Reject(RejectionReason::NoMatch),
                    }
                }
            }

            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Flex::column()), None, 0);
            let scroll = tree.add_child(root, Box::new(ScrollArea::new_unwired()), None, 0);
            tree.set_flex(scroll, FlexParams::flexible(1.0));
            let column = tree.add_child(
                scroll,
                Box::new(Flex::column().with_padding(EdgeInsets::new(top, 0.0, 0.0, 0.0))),
                None,
                0,
            );
            tree.node_mut(scroll).unwrap().widget.as_any_mut()
                .downcast_mut::<ScrollArea>().unwrap().wire_content(column);
            let calls = Rc::new(RefCell::new(Vec::new()));
            let mut combo = ComboBox::new(ResolverHandle::new(Resolver(calls.clone())));
            combo.set_options(&Self::OPTIONS);
            combo.set_text_input_enabled(editable);
            let combo = tree.add_child(column, Box::new(combo), None, 0);
            tree.add_child(column, Box::new(FixedSpacer::vertical(600.0)), None, 1);
            let mut router = EventRouter::new();
            router.set_host_choice_offer(Some(HostOffer::Native(WindowId::PRIMARY)));
            let mut fixture = Self {
                tree, root, scroll, combo, calls, router,
                domains: InteractionDomains::new(),
                fusion: InputFusion::new(),
                text: FontManagerTextEngine::new(),
                bindings: BindingRegistry::new(),
                frame: RetainedSceneTestHarness::new(),
            };
            fixture.layout(Self::VIEWPORT);
            fixture
        }

        fn layout(&mut self, viewport: RectF) {
            self.frame.frame_with_appearance(
                &mut self.tree, self.root, Constraints::tight(viewport.size()),
                &Theme::dark(), &mut self.text, &IconRegistry::new(&[]),
                &self.domains, crate::core::event::WindowId::PRIMARY,
            );
        }

        fn combo(&self) -> &ComboBox {
            self.tree.node(self.combo).unwrap().widget.as_any().downcast_ref().unwrap()
        }

        fn combo_mut(&mut self) -> &mut ComboBox {
            self.tree.node_mut(self.combo).unwrap().widget.as_any_mut().downcast_mut().unwrap()
        }

        fn touch(&mut self, contact_id: u64, phase: crate::core::event::TouchPhase, pos: Point) {
            use crate::core::event::{RawInputEvent, WindowId};
            self.route_raw(RawInputEvent::Touch {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                contact_id,
                phase,
                pos,
            });
        }

        fn route_raw(&mut self, raw: crate::core::event::RawInputEvent) {
            let state = self.domains.primary_state_mut();
            let mut events = Vec::new();
            self.fusion.process_raw_with_gesture_sessions(
                raw,
                state.focus_manager.interaction_domain_id(), &mut state.gesture_registry,
                &self.tree, None, &mut events,
            );
            self.fusion.flush_with_gesture_sessions(&mut events);
            for event in events {
                self.router.route_session_input_event(
                    &mut self.tree, event, Some(self.root), &mut self.text,
                    Some(self.frame.animation_context()), state, &mut self.bindings,
                );
            }
        }

        fn tap(&mut self, id: u64, point: Point) {
            use crate::core::event::TouchPhase;
            self.touch(id, TouchPhase::Started, point);
            assert!(self.router.take_host_choice_request().is_none());
            self.touch(id, TouchPhase::Ended, point);
        }

        fn reply(&mut self, lease: crate::core::event_router::host_choice::ChoiceLease, selection: Option<OptionId>) -> bool {
            use crate::core::event_router::host_choice::ChoiceReply;
            self.router.route_host_choice(
                &mut self.tree, self.root, self.domains.primary_state_mut(),
                ChoiceReply { lease, selection }, &mut self.text, Some(self.frame.animation_context()),
            )
        }

        fn center(&self) -> Point {
            self.tree.get_absolute_paint_rect(self.combo).unwrap().center()
        }
    }

    const COUNTRY_GROUPS: [OptionGroupBoundary<'static>; 2] = [
        OptionGroupBoundary { start_index: 0, title: "EU / EEA" },
        OptionGroupBoundary { start_index: 30, title: "Other countries and territories" },
    ];

    fn grouped_test_options() -> Vec<(OptionId, &'static str)> {
        (0..249).map(|index| {
            let label = match index {
                0 => "First EU option",
                29 => "Last EU option",
                30 => "First other option",
                248 => "Last other option",
                _ => "Example option",
            };
            (OptionId::from_raw(50_000 - index * 19), label)
        }).collect()
    }

    #[test]
    fn hal401_grouped_projection_is_atomic_and_flat_geometry_is_unchanged() {
        use GroupedProjectionError::*;
        use ChoiceDisplayRow::{Header, Item};
        let options = grouped_test_options();
        let mut combo = ComboBox::simple(Vec::new());
        let selected = Some(options[29].0);
        combo.set_grouped_projection(&options, &COUNTRY_GROUPS, selected, 0, 8).unwrap();
        assert_eq!(combo.snapshot.display_len(), 251);
        assert_eq!(combo.snapshot.entries().len(), 249);
        assert_eq!(combo.snapshot.display_row(0), Some(Header(0)));
        assert_eq!(combo.snapshot.display_row(1), Some(Item(0)));
        assert_eq!(combo.snapshot.display_row(30), Some(Item(29)));
        assert_eq!(combo.snapshot.display_row(31), Some(Header(1)));
        assert_eq!(combo.snapshot.display_row(32), Some(Item(30)));
        assert_eq!(combo.snapshot.display_row(250), Some(Item(248)));
        assert_eq!(combo.snapshot.display_row(251), None);
        let groups = combo.snapshot.groups.clone().unwrap();
        let revision = combo.popup_input_revision;
        let start = COUNTRY_GROUPS[0];
        let boundary = |start_index, title| OptionGroupBoundary { start_index, title };
        let duplicated_items = [options[0], options[0]];
        for (items, boundaries, error) in [
            (&[][..], &COUNTRY_GROUPS[..], EmptyItems),
            (&options[..], &[][..], MissingBoundaries),
            (
                &options[..],
                &COUNTRY_GROUPS[1..],
                FirstGroupStartsAfterZero,
            ),
            (
                &options[..],
                &[start, boundary(249, "Empty")][..],
                BoundaryOutOfRange(249),
            ),
            (
                &options[..],
                &[start, start][..],
                BoundariesNotIncreasing(0),
            ),
            (
                &options[..],
                &[start, COUNTRY_GROUPS[1], boundary(1, "Out of order")][..],
                BoundariesNotIncreasing(1),
            ),
            (&options[..], &[boundary(0, "")][..], EmptyTitle(0)),
            (
                &duplicated_items[..],
                &COUNTRY_GROUPS[..1],
                DuplicateOption(options[0].0),
            ),
        ] {
            assert_eq!(
                combo.set_grouped_projection(items, boundaries, None, 4, 4),
                Err(error)
            );
            assert_eq!(combo.popup_input_revision, revision);
            assert!(Arc::ptr_eq(
                combo.snapshot.groups.as_ref().unwrap(),
                &groups
            ));
            assert_eq!(
                combo
                    .snapshot
                    .items
                    .iter()
                    .map(|(id, label)| (*id, label.as_str()))
                    .collect::<Vec<_>>(),
                options,
            );
            assert_eq!(combo.highlighted_id, selected);
            assert_eq!(combo.visible_rows(), 8);
        }

        combo
            .set_grouped_projection(&options[..2], &[start], None, 0, 8)
            .unwrap();
        assert_eq!(combo.visible_rows(), 3);
        assert_eq!(combo.snapshot.display_row_for_item(0), 1);
        assert_eq!(combo.snapshot.display_row_for_item(1), 2);
        combo.set_projection(&options[..2], None, 0, 8);
        assert!(combo.snapshot.groups.is_none());
        assert_eq!(combo.visible_rows(), 2);
        assert_eq!(combo.snapshot.display_row(0), Some(Item(0)));
        assert_eq!(combo.snapshot.display_row_for_item(1), 1);
    }

    fn mounted_grouped_choice() -> (MountedHostChoice, Vec<(OptionId, &'static str)>) {
        use crate::model::selection_resolver::{RejectionReason, SelectionResolver};
        use std::{cell::RefCell, rc::Rc};
        struct Resolver {
            options: Vec<(OptionId, &'static str)>,
            calls: Rc<RefCell<Vec<(CommitSource, Option<OptionId>)>>>,
        }
        impl SelectionResolver for Resolver {
            fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                assert_eq!(input.options.len(), 249);
                assert!(
                    input
                        .options
                        .iter()
                        .zip(&self.options)
                        .all(|(entry, (id, label))| { entry.id == *id && entry.label == *label })
                );
                self.calls
                    .borrow_mut()
                    .push((input.commit_source, input.highlighted_id));
                input
                    .highlighted_id
                    .map_or(ResolverDecision::Reject(RejectionReason::NoMatch), |id| {
                        ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                    })
            }
        }
        let options = grouped_test_options();
        let mut f = MountedHostChoice::new(false);
        f.combo_mut().resolver = ResolverHandle::new(Resolver {
            options: options.clone(),
            calls: f.calls.clone(),
        });
        f.combo_mut()
            .set_grouped_projection(&options, &COUNTRY_GROUPS, None, 0, 5)
            .unwrap();
        f.layout(RectF::new(0.0, 0.0, 390.0, 280.0));
        (f, options)
    }

    fn grouped_pointer(
        f: &mut MountedHostChoice,
        position: Point,
        state: crate::core::event::RawButtonState,
    ) {
        use crate::core::event::{PointerDevice, RawInputEvent, WindowId};
        f.route_raw(RawInputEvent::PointerButton {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            state,
            position,
        });
    }

    fn click_grouped_popup(f: &mut MountedHostChoice, position: Point) {
        use crate::core::event::RawButtonState::{Pressed, Released};
        grouped_pointer(f, position, Pressed);
        grouped_pointer(f, position, Released);
    }

    fn grouped_popup_row(f: &MountedHostChoice, visible_index: usize) -> Point {
        let owner = f.tree.node(f.combo).unwrap().id;
        let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
        let trigger = f.tree.get_absolute_paint_rect(f.combo).unwrap();
        Point::new(
            trigger.x + popup.rows.x + 12.0 - popup.scroll_offset.0,
            trigger.y + popup.rows.y + (visible_index as f32 + 0.5) * ROW_HEIGHT
                - popup.scroll_offset.1,
        )
    }

    fn assert_group_footer(f: &MountedHostChoice, title: &str, count: &str) -> RectF {
        let owner = f.tree.node(f.combo).unwrap().id;
        let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
        let trigger = f.tree.get_absolute_paint_rect(f.combo).unwrap();
        let footer = popup.footer.unwrap().translate(trigger.x, trigger.y);
        assert_eq!(footer.height, 28.0);
        assert_eq!(footer.y, trigger.y + popup.rows.bottom());
        if let Some(scrollbar) = popup.drawn_scrollbar() {
            assert!(scrollbar.track.bottom() <= footer.y);
            assert!(scrollbar.thumb.bottom() <= footer.y);
            assert!(scrollbar.hit_rect.bottom() <= footer.y);
        }
        let overlay = f.frame.overlay_fragment(owner).unwrap();
        let nodes = surface_nodes(&overlay);
        let labels: Vec<_> = nodes.iter().filter_map(|node| match node {
            SceneNode::Text(t) if (footer.y..footer.bottom()).contains(&t.position.y) => Some(t),
            _ => None,
        }).collect();
        let [right, left] = labels.as_slice() else { panic!("two context labels") };
        assert_eq!((left.text.as_str(), right.text.as_str()), (title, count));
        assert!(left.position.x < right.position.x);
        footer
    }

    #[test]
    fn hal401_scrolled_form_footer_keeps_the_popup_open_through_primary_press() {
        use crate::core::event::{
            FusedInputEvent, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode, PointerDevice,
            ScrollSource, WindowId,
        };
        use crate::renderer::{
            custom_render_prepare::CustomRenderTargetSnapshot, pixel_coverage::PhysicalExtent,
        };
        use crate::widgets::scroll_area::ScrollArea;

        for (width, height) in [(320, 640), (390, 720), (1144, 920)] {
            let mut f = MountedHostChoice::with_top_padding(false, 1200.0);
            let options = grouped_test_options();
            let mut groups = COUNTRY_GROUPS;
            groups[1].title = "Outside EU / EEA";
            f.combo_mut()
                .set_grouped_projection(&options, &groups, None, 0, 11)
                .unwrap();
            let theme = Theme::dark();
            f.text.set_scale(2.0);
            let target = CustomRenderTargetSnapshot::new(
                crate::compositor::SurfaceId::test_id(1),
                PhysicalExtent::new(width * 2, height * 2),
                2.0,
            )
            .unwrap();
            let frame = |f: &mut MountedHostChoice| {
                f.frame.frame_with_optional_custom_render_target(
                    &mut f.tree,
                    f.root,
                    Constraints::tight(Size::new(width as f32, height as f32)),
                    &theme,
                    &mut f.text,
                    &IconRegistry::new(&[]),
                    Some(target),
                    Some((&f.domains, WindowId::PRIMARY)),
                );
            };
            let route = |f: &mut MountedHostChoice, event| {
                f.router.route_input_event(
                    &mut f.tree, event, Some(f.root), &mut f.text,
                    Some(f.frame.animation_context()), f.domains.primary_state_mut(), &mut f.bindings,
                );
            };
            let press = |pos, click_count| FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count,
            };
            let release = |pos| FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            };

            frame(&mut f);
            let trigger = f.tree.get_absolute_paint_rect(f.combo).unwrap();
            f.tree.node_mut(f.scroll).unwrap().widget.as_any_mut()
                .downcast_mut::<ScrollArea>().unwrap()
                .scroll_to(0.0, trigger.y - height as f32 * 0.33);
            f.tree.mark_paint_dirty(f.scroll);
            frame(&mut f);
            let (x, y) = f.tree.cumulative_scroll_offset(f.combo);
            assert!(y > 0.0);
            let trigger = f.tree.get_absolute_paint_rect(f.combo).unwrap();
            let control = trigger.center();
            let control = Point::new(control.x - x, control.y - y);
            route(&mut f, press(control, 1));
            route(&mut f, release(control));
            frame(&mut f);
            f.frame.advance_transition_time(1.0);
            frame(&mut f);
            route(&mut f, FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                key_code: KeyCode::Home,
                physical_key: PhysicalKeyCode::Home,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            });
            frame(&mut f);

            let owner = f.tree.node(f.combo).unwrap().id;
            let initial = assert_group_footer(&f, "EU / EEA", "30").translate(-x, -y);
            assert!(initial.bottom() <= height as f32);
            let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
            assert_eq!((popup.visible_rows, popup.rows.height), (11, 264.0));
            let body = grouped_popup_row(&f, 9);
            let wheel = |rows: f32| FusedInputEvent::Scroll {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                pos: body,
                delta: (0.0, -rows * ROW_HEIGHT),
                source: ScrollSource::Wheel,
            };
            route(&mut f, wheel(31.0));
            frame(&mut f);
            let header = grouped_popup_row(&f, 0);
            let (hit, origin) = f.tree.hit_test_for_routing(header.x, header.y).unwrap();
            assert_eq!((hit.widget_id, origin), (owner, PointerRouteOrigin::Overlay));
            assert!(f.combo().popup_inert_press_at(
                f.tree.combo_popup_input_geometry(owner), hit.local_pos, (header.x, header.y),
            ));
            route(&mut f, press(header, 1));
            route(&mut f, release(header));
            assert!(f.combo().popup_open.get());
            route(&mut f, wheel(12.0));
            frame(&mut f);
            let footer = assert_group_footer(&f, "Outside EU / EEA", "219").translate(-x, -y);
            assert_eq!(footer, initial);
            let point = footer.center();
            let (hit, origin) = f.tree.hit_test_for_routing(point.x, point.y).unwrap();
            assert_eq!((hit.widget_id, origin), (owner, PointerRouteOrigin::Overlay));
            let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
            assert_eq!(popup.first_row, 43);
            assert!(f.combo().popup_inert_press_at(Some(popup), hit.local_pos, (point.x, point.y)));
            for count in [1, 2] {
                route(&mut f, press(point, count));
                assert!(f.combo().popup_open.get());
                assert!(!f.combo().option_press_active);
                assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(owner));
                assert_eq!(f.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
                route(&mut f, release(point));
                assert!(f.combo().popup_open.get());
            }
            route(&mut f, wheel(-43.0));
            frame(&mut f);
            assert_eq!(assert_group_footer(&f, "EU / EEA", "30").translate(-x, -y), initial);

            let outside = Point::new(control.x, 8.0);
            route(&mut f, press(outside, 1));
            assert!(!f.combo().popup_open.get());
            assert!(f.tree.combo_popup_input_geometry(owner).is_none());
            route(&mut f, release(outside));
            assert!(f.calls.borrow().is_empty());
            assert!(f.combo_mut().take_committed().is_none());
        }
    }

    #[test]
    fn hal401_group_context_follows_first_display_row_and_shields_the_footer() {
        use crate::core::event::{
            KeyState, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode, PointerDevice,
            RawButtonState, RawInputEvent, WheelDelta, WindowId,
        };
        use crate::widgets::{button::Button, fixed_spacer::FixedSpacer};
        use std::rc::Rc;

        let (mut f, options) = mounted_grouped_choice();
        let mut groups = COUNTRY_GROUPS;
        groups[1].title = "Outside EU / EEA";
        let one = ChoiceGroups::new(&options, &groups[..1]).unwrap();
        assert_eq!(one.context_at(0), Some(("EU / EEA", 249)));
        assert_eq!(one.context_at(249), Some(("EU / EEA", 249)));
        let third = OptionGroupBoundary { start_index: 242, title: "Another group" };
        let multiple = ChoiceGroups::new(&options, &[groups[0], groups[1], third]).unwrap();
        assert_eq!(multiple.context_at(31), Some(("Outside EU / EEA", 212)));
        assert_eq!(multiple.context_at(244), Some(("Another group", 7)));
        assert_eq!(multiple.context_at(252), None);

        f.combo_mut().set_grouped_projection(&options, &groups, None, 0, 11).unwrap();
        let clicks = Rc::new(Cell::new(0));
        let counter = clicks.clone();
        let column = f.tree.parent(f.combo).unwrap();
        let old_spacer = f.tree.children(column)[1];
        f.tree.remove_node(old_spacer);
        f.tree.add_child(column, Box::new(FixedSpacer::vertical(264.0)), None, 1);
        let button = f.tree.add_child(
            column,
            Box::new(Button::with_text("Behind footer").on_click(move || counter.set(counter.get() + 1))),
            None, 2,
        );

        let viewport = RectF::new(0.0, 0.0, 320.0, 568.0);
        f.layout(viewport);
        let trigger = f.center();
        click_grouped_popup(&mut f, trigger);
        f.layout(viewport);
        f.frame.advance_transition_time(1.0);
        f.layout(viewport);
        let owner = f.tree.node(f.combo).unwrap().id;
        let footer = assert_group_footer(&f, "EU / EEA", "30");
        let shielded_button = f.tree.get_absolute_paint_rect(button).unwrap().center();
        assert!(footer.contains(shielded_button));
        let item = grouped_popup_row(&f, 1);
        grouped_pointer(&mut f, shielded_button, RawButtonState::Pressed);
        assert!(!f.combo().option_press_active);
        assert_eq!(f.combo().highlighted_id, None);
        assert!(f.domains.primary_state().pointer_capture_manager.active_pointers().is_empty());
        grouped_pointer(&mut f, item, RawButtonState::Released);
        grouped_pointer(&mut f, item, RawButtonState::Pressed);
        grouped_pointer(&mut f, shielded_button, RawButtonState::Released);
        click_grouped_popup(&mut f, shielded_button);
        f.tap(42, shielded_button);
        assert!(f.router.take_host_choice_request().is_none());
        assert_eq!(f.domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(f.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
        assert_eq!(clicks.get(), 0);
        assert!(f.calls.borrow().is_empty());

        let mut previous = 0;
        for (first, title, count) in [
            (30, "EU / EEA", "30"),
            (31, "Outside EU / EEA", "219"),
            (32, "Outside EU / EEA", "219"),
            (150, "Outside EU / EEA", "219"),
        ] {
            let position = grouped_popup_row(&f, 1);
            f.route_raw(RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                device: PointerDevice::Mouse,
                delta: WheelDelta::Pixels(0.0, -((first - previous) as f32) * ROW_HEIGHT),
                position,
            });
            f.layout(viewport);
            assert_eq!(f.tree.combo_popup_input_geometry(owner).unwrap().first_row, first);
            assert_eq!(assert_group_footer(&f, title, count), footer);
            previous = first;
        }
        for state in [KeyState::Down, KeyState::Up] {
            f.route_raw(RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: web_time::Instant::now(),
                key_code: KeyCode::End,
                physical_key: PhysicalKeyCode::End,
                reservation: None,
                state,
                repeat: false,
                modifiers: Modifiers::default(),
            });
        }
        f.layout(viewport);
        assert_eq!(f.tree.combo_popup_input_geometry(owner).unwrap().first_row, 240);
        assert_eq!(assert_group_footer(&f, "Outside EU / EEA", "219"), footer);
        assert_eq!(f.combo().highlighted_id, Some(options[248].0));
        assert!(f.calls.borrow().is_empty());
    }

    #[test]
    fn hal401_grouped_popup_paints_bands_separator_and_fitted_headings_at_retina_scale() {
        use crate::core::event::{PointerDevice, RawInputEvent, WheelDelta, WindowId};
        use crate::renderer::{custom_render_prepare::CustomRenderTargetSnapshot, pixel_coverage::PhysicalExtent};

        let luminance = |c: Color| 0.2126 * c.r + 0.7152 * c.g + 0.0722 * c.b;
        let contrast = |a, b| {
            let (a, b) = (luminance(a), luminance(b));
            (a.max(b) + 0.05) / (a.min(b) + 0.05)
        };
        for theme in [Theme::light(), Theme::dark()] {
            let colors = GroupedPopupColors::from_theme(&theme);
            assert!(contrast(colors.text, colors.heading) >= 4.5);
            assert!(contrast(colors.text, colors.footer) >= 4.5);
            assert!(contrast(colors.heading, theme.colors.surfaces.content) >= 1.7);
        }

        let (mut f, options) = mounted_grouped_choice();
        let mut groups = COUNTRY_GROUPS;
        groups[1].title = "Outside EU / EEA";
        f.combo_mut()
            .set_grouped_projection(&options, &groups, None, 0, 11)
            .unwrap();
        let theme = Theme::light();
        let colors = GroupedPopupColors::from_theme(&theme);
        let owner = f.tree.node(f.combo).unwrap().id;
        f.text.set_scale(2.0);
        let frame = |f: &mut MountedHostChoice, physical_width: u32| {
            let target = CustomRenderTargetSnapshot::new(
                crate::compositor::SurfaceId::test_id(1),
                PhysicalExtent::new(physical_width, 1136),
                2.0,
            )
            .unwrap();
            f.frame.frame_with_optional_custom_render_target(
                &mut f.tree, f.root, Constraints::tight(Size::new(physical_width as f32 / 2.0, 568.0)),
                &theme, &mut f.text, &IconRegistry::new(&[]),
                Some(target), Some((&f.domains, WindowId::PRIMARY)),
            );
        };
        frame(&mut f, 479);
        let trigger = f.center();
        click_grouped_popup(&mut f, trigger);
        frame(&mut f, 479);
        f.frame.advance_transition_time(1.0);
        frame(&mut f, 479);

        let check_band = |f: &MountedHostChoice, row: usize, title: &str, option: &str| {
            let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
            assert_eq!((popup.visible_rows, popup.rows.height), (11, 264.0));
            let row_top = grouped_popup_row(f, row).y - ROW_HEIGHT * 0.5;
            let overlay = f.frame.overlay_fragment(owner).unwrap();
            let nodes = surface_nodes(&overlay);
            let text = |label| nodes.iter().find_map(|node| match node {
                SceneNode::Text(t) if t.text == label => Some(t),
                _ => None,
            }).unwrap();
            let header = text(title);
            let item = text(option);
            assert_eq!(header.color, colors.text.into());
            assert_eq!(header.font_size, item.font_size * 0.95);
            assert_eq!(item.position.x - header.position.x, 8.0);
            assert!(header.position.y >= row_top && header.position.y < row_top + ROW_HEIGHT);
            assert!(nodes.iter().any(|node| matches!(node, SceneNode::Rect(r)
                if r.z_index == POPUP_ROW_LAYER
                    && r.color == colors.heading.into()
                    && r.rect.y == row_top && r.rect.height == ROW_HEIGHT)));
            assert!(nodes.iter().any(|node| matches!(node, SceneNode::Rect(r)
                if r.z_index == POPUP_ROW_LAYER && r.color == theme.colors.surfaces.content.into()
                    && r.rect.y == row_top + ROW_HEIGHT)));
            assert!(nodes.iter().any(|node| matches!(node, SceneNode::Rect(r)
                if r.z_index == POPUP_MATCH_LAYER && r.color == colors.rule.into()
                    && r.rect.width == 3.0 && r.rect.bottom() == row_top + ROW_HEIGHT)));
            let separators: Vec<_> = nodes.iter().filter_map(|node| match node {
                SceneNode::Rect(r) if r.z_index == POPUP_MATCH_LAYER
                    && r.color == colors.rule.into()
                    && r.rect.width > 3.0 && r.rect.y == row_top => Some(r),
                _ => None,
            }).collect();
            if row == 0 {
                assert!(separators.is_empty());
            } else {
                let [separator] = separators.as_slice() else { panic!("one section separator") };
                assert_eq!((separator.rect.y, separator.rect.height), (row_top, 1.0));
                assert_eq!((separator.rect.y * 2.0).fract(), 0.0);
                assert!(nodes.iter().any(|node| matches!(node, SceneNode::Clip(clip)
                    if clip.rounded.is_some()
                        && clip.rect.x < separator.rect.x
                        && clip.rect.right() > separator.rect.right()
                        && clip.children.iter().any(|child| matches!(child, SceneNode::Rect(r)
                            if r.rect == separator.rect && r.z_index == POPUP_MATCH_LAYER)))));
            }
        };
        check_band(&f, 0, "EU / EEA", "First EU option");

        let position = grouped_popup_row(&f, 1);
        f.route_raw(RawInputEvent::PointerWheel {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            device: PointerDevice::Mouse,
            delta: WheelDelta::Pixels(0.0, -27.0 * ROW_HEIGHT),
            position,
        });
        frame(&mut f, 479);
        check_band(&f, 4, "Outside EU / EEA", "First other option");

        frame(&mut f, 144);
        let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
        let scrollbar = popup.drawn_scrollbar().unwrap();
        let overlay = f.frame.overlay_fragment(owner).unwrap();
        let (clip, header) = surface_nodes(&overlay).iter().find_map(|node| match node {
            SceneNode::Clip(clip) => clip.children.iter().find_map(|child| match child {
                SceneNode::Text(t) if t.color == colors.text.into() && t.text.ends_with('…') => Some((clip, t)),
                _ => None,
            }),
            _ => None,
        }).unwrap();
        let prefix = header.text.strip_suffix('…').unwrap();
        assert!("Outside EU / EEA".starts_with(prefix));
        assert!(header.position.x + f.text.shape(&header.text, header.font_size).width <= clip.rect.right());
        assert!(clip.rect.right() <= scrollbar.hit_rect.x);
    }

    #[test]
    fn hal401_grouped_popup_fits_whole_rows_and_drags_to_the_last_id_in_a_short_viewport() {
        use crate::core::event::{PointerDevice, RawButtonState, RawInputEvent, WindowId};
        let mut f = MountedHostChoice::with_top_padding(false, 110.0);
        let options = grouped_test_options();
        let large = RectF::new(0.0, 0.0, 320.0, 568.0);
        let owner = f.tree.node(f.combo).unwrap().id;
        for requested in [8, 11] {
            f.combo_mut().set_grouped_projection(&options, &COUNTRY_GROUPS, None, 0, requested).unwrap();
            f.layout(large);
            if !f.combo().popup_open.get() {
                let trigger = f.center();
                click_grouped_popup(&mut f, trigger);
                f.layout(large);
                f.frame.advance_transition_time(1.0);
            }
            f.layout(large);
            let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
            assert_eq!(popup.visible_rows, requested);
            assert_eq!(popup.rows.height, requested as f32 * ROW_HEIGHT);
            assert!(popup.rows.y >= 0.0);
        }
        f.layout(RectF::new(0.0, 0.0, 390.0, 720.0));
        assert_eq!(f.tree.combo_popup_input_geometry(owner).unwrap().visible_rows, 11);

        let short = RectF::new(0.0, 0.0, 320.0, 210.0);
        f.layout(short);
        let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
        let trigger = f.tree.get_absolute_paint_rect(f.combo).unwrap();
        assert_eq!((popup.visible_rows, popup.rows.height), (3, 72.0));
        assert_eq!(popup.rows.bottom(), -28.0);
        assert_eq!(popup.footer.unwrap().bottom(), 0.0);
        assert!(trigger.y + popup.rows.y >= short.y);
        let scrollbar = popup.drawn_scrollbar().unwrap();
        let start = scrollbar.thumb.center();
        let end = Point::new(start.x, scrollbar.track.bottom() - scrollbar.thumb.height * 0.5);
        grouped_pointer(&mut f, start, RawButtonState::Pressed);
        assert!(f.combo().scrollbar_v.dragging);
        f.route_raw(RawInputEvent::PointerMoved {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            device: PointerDevice::Mouse,
            position: end,
            delta: (0.0, end.y - start.y),
        });
        grouped_pointer(&mut f, end, RawButtonState::Released);
        f.layout(short);
        let popup = f.tree.combo_popup_input_geometry(owner).unwrap();
        assert_eq!(popup.first_row, 251 - 3);
        assert_group_footer(&f, COUNTRY_GROUPS[1].title, "219");
        let last = grouped_popup_row(&f, 2);
        assert!(f.calls.borrow().is_empty());
        click_grouped_popup(&mut f, last);
        assert_eq!(&*f.calls.borrow(), &[(CommitSource::Selection, Some(options[248].0))]);
    }

    #[test]
    fn hal401_popup_headings_shield_mouse_and_touch_and_keep_scroll_eligible() {
        use crate::core::event::{
            KeyboardAuthorityMode, PointerDevice, RawButtonState, RawInputEvent, TouchPhase,
            WheelDelta, WindowId,
        };
        use crate::widgets::{button::Button, fixed_spacer::FixedSpacer};
        use std::rc::Rc;
        let (mut f, options) = mounted_grouped_choice();
        let clicks = Rc::new(Cell::new(0));
        let record = clicks.clone();
        let column = f.tree.parent(f.combo).unwrap();
        let spacer = f.tree.children(column)[1];
        f.tree.remove_node(spacer);
        let button = f.tree.add_child(
            column,
            Box::new(
                Button::with_text("Behind popup").on_click(move || record.set(record.get() + 1)),
            ),
            None,
            1,
        );
        f.tree
            .add_child(column, Box::new(FixedSpacer::vertical(600.0)), None, 2);
        let viewport = RectF::new(0.0, 0.0, 390.0, 280.0);
        f.layout(viewport);
        let trigger = f.center();
        click_grouped_popup(&mut f, trigger);
        f.layout(viewport);
        f.frame.advance_transition_time(1.0);
        f.layout(viewport);
        let owner = f.tree.node(f.combo).unwrap().id;
        let overlay = f.frame.overlay_fragment(owner).unwrap();
        assert!(surface_nodes(&overlay).iter().any(|node| matches!(node, SceneNode::Text(text)
            if text.text == "EU / EEA" && text.color == Theme::dark().colors.typography.primary.into())));
        let header = grouped_popup_row(&f, 0);
        let first = grouped_popup_row(&f, 1);
        assert!(
            f.tree
                .get_absolute_paint_rect(button)
                .unwrap()
                .contains(header)
        );

        grouped_pointer(&mut f, header, RawButtonState::Pressed);
        assert_eq!(f.combo().highlighted_id, None);
        assert!(!f.combo().option_press_active);
        assert!(
            f.domains
                .primary_state()
                .pointer_capture_manager
                .active_pointers()
                .is_empty()
        );
        grouped_pointer(&mut f, first, RawButtonState::Released);
        f.tap(20, header);
        assert!(f.router.take_host_choice_request().is_none());
        assert_eq!(f.combo().highlighted_id, None);
        assert_eq!(
            f.domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert_eq!(
            f.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        assert_eq!(clicks.get(), 0);
        assert!(f.calls.borrow().is_empty());

        let end = Point::new(header.x, header.y - 2.0 * ROW_HEIGHT);
        f.touch(21, TouchPhase::Started, header);
        f.touch(21, TouchPhase::Moved, end);
        f.touch(21, TouchPhase::Ended, end);
        assert_eq!(f.combo().popup_scroll_row, 2);
        assert_eq!(clicks.get(), 0);
        assert!(f.calls.borrow().is_empty());
        f.layout(viewport);
        let position = grouped_popup_row(&f, 0);
        f.route_raw(RawInputEvent::PointerWheel {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            device: PointerDevice::Mouse,
            delta: WheelDelta::Pixels(0.0, -26.0 * ROW_HEIGHT),
            position,
        });
        assert_eq!(f.combo().popup_scroll_row, 28);
        f.layout(viewport);
        let overlay = f.frame.overlay_fragment(owner).unwrap();
        assert!(
            surface_nodes(&overlay)
                .iter()
                .any(|node| matches!(node, SceneNode::Text(text)
            if text.text == "Other countries and territories"))
        );
        let header = grouped_popup_row(&f, 3);
        click_grouped_popup(&mut f, header);
        assert!(f.calls.borrow().is_empty());
        let first_other = grouped_popup_row(&f, 4);
        click_grouped_popup(&mut f, first_other);
        assert_eq!(
            &*f.calls.borrow(),
            &[(CommitSource::Selection, Some(options[30].0))]
        );
        assert!(matches!(f.combo_mut().take_committed(),
            Some(CommittedValue::ExistingOption(id)) if id == options[30].0));
    }

    #[test]
    fn hal401_keyboard_uses_real_indices_to_reveal_and_commit_edges() {
        use crate::core::event::{KeyState, Modifiers, PhysicalKeyCode, RawInputEvent, WindowId};
        let (mut f, options) = mounted_grouped_choice();
        let key = |f: &mut MountedHostChoice, key_code, physical_key| {
            for state in [KeyState::Down, KeyState::Up] {
                f.route_raw(RawInputEvent::Key {
                    window_id: WindowId::PRIMARY,
                    time: web_time::Instant::now(),
                    key_code,
                    physical_key,
                    reservation: None,
                    state,
                    repeat: false,
                    modifiers: Modifiers::default(),
                });
            }
        };
        let viewport = RectF::new(0.0, 0.0, 390.0, 280.0);
        let trigger = f.center();
        click_grouped_popup(&mut f, trigger);
        key(&mut f, KeyCode::Home, PhysicalKeyCode::Home);
        assert_eq!(f.combo().highlighted_id, Some(options[0].0));
        for _ in 0..29 {
            key(&mut f, KeyCode::Down, PhysicalKeyCode::ArrowDown);
        }
        assert_eq!(f.combo().highlighted_id, Some(options[29].0));
        assert_eq!(f.combo().popup_scroll_row, 26);
        key(&mut f, KeyCode::Down, PhysicalKeyCode::ArrowDown);
        assert_eq!(f.combo().highlighted_id, Some(options[30].0));
        assert_eq!(f.combo().popup_scroll_row, 28);
        key(&mut f, KeyCode::Up, PhysicalKeyCode::ArrowUp);
        assert_eq!(f.combo().highlighted_id, Some(options[29].0));
        key(&mut f, KeyCode::Enter, PhysicalKeyCode::Enter);
        assert_eq!(
            &*f.calls.borrow(),
            &[(CommitSource::Selection, Some(options[29].0))]
        );
        f.layout(viewport);
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            Some(f.frame.animation_context()),
        ));
        assert!(f.combo().popup_open.get());
        key(&mut f, KeyCode::End, PhysicalKeyCode::End);
        assert_eq!(f.combo().highlighted_id, Some(options[248].0));
        assert_eq!(f.combo().popup_scroll_row, 246);
        key(&mut f, KeyCode::Enter, PhysicalKeyCode::Enter);
        assert_eq!(
            &*f.calls.borrow(),
            &[
                (CommitSource::Selection, Some(options[29].0)),
                (CommitSource::Selection, Some(options[248].0)),
            ]
        );
    }

    #[test]
    fn hal401_host_picker_maps_done_cancel_and_stale_groups_without_neighbor_selection() {
        let (mut f, options) = mounted_grouped_choice();
        let trigger = f.center();
        f.tap(1, trigger);
        let request = f.router.take_host_choice_request().unwrap();
        assert_eq!(request.picker_row_count(), 251);
        assert_eq!(request.initial_picker_row(), 1);
        assert_eq!(request.items.len(), 249);
        assert_eq!(
            request.lease.target.revision,
            f.combo().popup_input_revision
        );
        assert_eq!(
            request.picker_row_title(request.picker_row(31).unwrap()),
            COUNTRY_GROUPS[1].title
        );
        for row in [None, Some(-1), Some(0), Some(31), Some(251)] {
            let reply = request.picker_reply(row);
            assert_eq!(reply.lease, request.lease);
            assert_eq!(reply.selection, None);
        }
        let header = request.picker_reply(Some(31));
        assert!(!f.reply(header.lease, header.selection));
        assert!(!f.reply(request.lease, Some(options[30].0)));
        assert!(f.calls.borrow().is_empty());

        for (contact, row, expected) in [(2, 1, 0), (3, 32, 30), (4, 250, 248)] {
            f.tap(contact, trigger);
            let request = f.router.take_host_choice_request().unwrap();
            let reply = request.picker_reply(Some(row));
            assert_eq!(reply.selection, Some(options[expected].0));
            assert!(f.reply(reply.lease.clone(), reply.selection));
            assert!(!f.reply(reply.lease, reply.selection));
        }
        f.tap(5, trigger);
        let cancelled = f.router.take_host_choice_request().unwrap();
        assert_eq!(cancelled.initial_picker_row(), 250);
        let cancel = cancelled.picker_reply(None);
        assert!(!f.reply(cancel.lease, cancel.selection));
        f.tap(6, trigger);
        let old = f.router.take_host_choice_request().unwrap();
        let mut renamed = COUNTRY_GROUPS;
        renamed[1].title = "Other territories";
        f.combo_mut()
            .set_grouped_projection(&options, &renamed, None, 0, 5)
            .unwrap();
        let stale = old.picker_reply(Some(30));
        assert_eq!(stale.selection, Some(options[29].0));
        assert!(!f.reply(stale.lease, stale.selection));
        assert_eq!(
            &*f.calls.borrow(),
            &[
                (CommitSource::Selection, Some(options[0].0)),
                (CommitSource::Selection, Some(options[30].0)),
                (CommitSource::Selection, Some(options[248].0)),
            ]
        );
    }

    #[test]
    fn hal401_display_selection_survives_locale_remount_without_commit() {
        use crate::model::selection_resolver::{RejectionReason, SelectionResolver};
        use std::{cell::RefCell, rc::Rc};

        struct Resolver(Rc<RefCell<Vec<(CommitSource, Option<OptionId>)>>>);
        impl SelectionResolver for Resolver {
            fn resolve(&self, input: &ResolverInput<'_>) -> ResolverDecision {
                assert_eq!(input.options.len(), 249);
                assert_eq!(input.options[248].id, OptionId::from_raw(1447254615));
                self.0
                    .borrow_mut()
                    .push((input.commit_source, input.highlighted_id));
                input
                    .highlighted_id
                    .map_or(ResolverDecision::Reject(RejectionReason::NoMatch), |id| {
                        ResolverDecision::Commit(CommittedValue::ExistingOption(id))
                    })
            }
        }

        let mut f = MountedHostChoice::new(false);
        let calls = Rc::clone(&f.calls);
        let callbacks = Rc::new(Cell::new(0));
        let selected = OptionId::from_raw(1447254615);
        let mut options = grouped_test_options();
        options[248] = (selected, "Zimbabwe");
        let build = |options: &[(OptionId, &str)], caption: &str| {
            let mut combo = ComboBox::new(ResolverHandle::new(Resolver(Rc::clone(&calls))));
            combo.set_text_input_enabled(false);
            combo
                .set_grouped_projection(options, &COUNTRY_GROUPS, Some(options[0].0), 0, 5)
                .unwrap();
            combo
                .set_display_selection(Some(selected), caption.to_owned())
                .unwrap();
            let callbacks = Rc::clone(&callbacks);
            combo.set_on_committed(move |_| callbacks.set(callbacks.get() + 1));
            combo
        };
        f.tree
            .replace_widget_only(f.combo, Box::new(build(&options, "Zimbabwe")));
        let assert_restored = |f: &MountedHostChoice, caption: &str| {
            let combo = f.combo();
            assert_eq!(combo.input_text(), caption);
            assert_eq!(combo.highlighted_id, Some(options[0].0));
            assert_eq!(
                combo.host_choice_options().unwrap().selected,
                Some(selected)
            );
            let child = combo.composite_id().unwrap();
            let snapshot = f.tree.composite_editor_snapshot(child).unwrap();
            assert_eq!(combo.text_field.snapshot.as_ref(), Some(&snapshot));
            assert!(matches!(combo.host_selected_option,
                Some(HostSelectedOption::Observed(id, revision))
                    if id == selected && revision == snapshot.revision
            ));
            assert!(!combo.popup_open.get());
            assert!(combo.committed_latch.is_none());
            assert!(calls.borrow().is_empty());
            assert_eq!(callbacks.get(), 0);
        };
        assert_restored(&f, "Zimbabwe");
        f.layout(MountedHostChoice::VIEWPORT);
        f.tap(1, f.center());
        let previous = f.router.take_host_choice_request().unwrap();
        assert_eq!(previous.selected, Some(selected));
        assert_eq!(previous.initial_picker_row(), 250);

        let mut localized = options.clone();
        for (_, label) in &mut localized[..30] {
            *label = "Choix européen";
        }
        f.tree
            .replace_widget_only(f.combo, Box::new(build(&localized, "Зимбабве")));
        assert_restored(&f, "Зимбабве");
        f.layout(MountedHostChoice::VIEWPORT);
        assert!(!f.reply(previous.lease.clone(), Some(selected)));
        f.tap(2, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        assert_ne!(request.lease.target.mount, previous.lease.target.mount);
        assert_eq!(request.selected, Some(selected));
        assert_eq!(request.items[0].1, "Choix européen");
        assert_eq!(request.initial_picker_row(), 250);
        assert_eq!(request.picker_reply(Some(250)).selection, Some(selected));
        let header = request.picker_reply(Some(31));
        assert!(!f.reply(header.lease, header.selection));
        assert_restored(&f, "Зимбабве");

        f.tap(3, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        let cancel = request.picker_reply(None);
        assert!(!f.reply(cancel.lease, cancel.selection));
        assert_restored(&f, "Зимбабве");
        f.tap(4, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        assert!(f.reply(request.lease.clone(), Some(selected)));
        assert!(!f.reply(request.lease, Some(selected)));
        assert_eq!(
            &*calls.borrow(),
            &[(CommitSource::Selection, Some(selected))]
        );
        assert_eq!(callbacks.get(), 1);
        assert!(matches!(f.combo_mut().take_committed(),
            Some(CommittedValue::ExistingOption(id)) if id == selected));
    }

    #[test]
    fn hal400_flat_display_selection_validates_atomically_and_tracks_live_sources() {
        use crate::core::event::{Modifiers, RawInputEvent, WindowId};
        use crate::runtime::actions::ActionId;
        use DisplaySelectionError::*;

        let options = MountedHostChoice::OPTIONS;
        let selected = options[1].0;
        let unknown = OptionId::from_raw(31);
        let mut f = MountedHostChoice::new(false);
        let make = || {
            let mut combo = ComboBox::simple(Vec::new());
            combo.set_projection(&options, Some(options[0].0), 0, 8);
            assert_eq!(
                combo.set_display_selection(Some(selected), "Refused".into()),
                Err(Editable)
            );
            assert!(combo.input_text().is_empty());
            combo.set_text_input_enabled(false);
            combo
                .set_display_selection(Some(selected), "Model caption".into())
                .unwrap();
            assert_eq!(
                combo.set_display_selection(Some(unknown), "Refused".into()),
                Err(UnknownOption(unknown)),
            );
            assert_eq!(combo.input_text(), "Model caption");
            combo
                .set_display_selection(Some(selected), "New model caption".into())
                .unwrap();
            combo
        };
        f.tree.replace_widget_only(f.combo, Box::new(make()));
        assert_eq!(f.combo().input_text(), "New model caption");
        assert_eq!(
            f.combo().host_choice_options().unwrap().selected,
            Some(selected)
        );
        assert_eq!(
            f.combo_mut().set_display_selection(None, "Refused".into()),
            Err(Editor(CompositeEditOutcome::OwnerContextRequired)),
        );
        assert_eq!(f.combo().input_text(), "New model caption");
        assert_eq!(
            f.combo().host_choice_options().unwrap().selected,
            Some(selected)
        );
        assert!(f.combo().committed_latch.is_none());

        f.combo_mut()
            .set_projection(&[options[2], options[1], options[0]], None, 0, 8);
        assert_eq!(
            f.combo().host_choice_options().unwrap().selected,
            Some(selected)
        );
        f.combo_mut()
            .set_projection(&[options[0], options[2]], None, 0, 8);
        assert_eq!(f.combo().host_choice_options().unwrap().selected, None);

        let mut cleared = make();
        cleared
            .set_display_selection(None, "Choose a profile".into())
            .unwrap();
        f.tree.replace_widget_only(f.combo, Box::new(cleared));
        assert_eq!(f.combo().input_text(), "Choose a profile");
        assert_eq!(f.combo().host_choice_options().unwrap().selected, None);
        assert_eq!(f.combo().highlighted_id, Some(options[0].0));

        let mut legacy = make();
        legacy.set_input_text("Unqualified caption".into()).unwrap();
        f.tree.replace_widget_only(f.combo, Box::new(legacy));
        assert_eq!(f.combo().host_choice_options().unwrap().selected, None);

        f.tree.replace_widget_only(f.combo, Box::new(make()));
        assert_eq!(
            f.combo().host_choice_options().unwrap().selected,
            Some(selected)
        );
        f.combo_mut().set_text_input_enabled(true);
        f.layout(MountedHostChoice::VIEWPORT);
        let owner = f.tree.node(f.combo).unwrap().id;
        f.router
            .request_focus_for_widget(&mut f.tree, owner, f.domains.primary_state_mut());
        assert!(f.router.handle_engine_action(
            &mut f.tree,
            ActionId::ENTER_WIDGET,
            Some(f.root),
            f.domains.primary_state_mut(),
            &mut f.text,
            Some(f.frame.animation_context()),
        ));
        f.route_raw(RawInputEvent::TextInput {
            window_id: WindowId::PRIMARY,
            time: web_time::Instant::now(),
            text: "λ".into(),
            modifiers: Modifiers::default(),
        });
        assert_ne!(f.combo().input_text(), "New model caption");
        assert_eq!(f.combo().host_choice_options().unwrap().selected, None);
        assert!(f.combo().committed_latch.is_none());
    }

    #[test]
    fn hal400_picker_forwards_one_exact_id_through_the_resolver() {
        let mut f = MountedHostChoice::new(false);
        let chosen = MountedHostChoice::OPTIONS[1].0;
        f.tap(11, f.center());
        let request = f.router.take_host_choice_request().expect("qualified Touch Up opens a host picker");
        assert_eq!(request.items, MountedHostChoice::OPTIONS.map(|(id, label)| (id, String::from(label))));
        assert_eq!(request.selected, None);
        assert!(!f.combo().popup_open.get());
        assert!(f.calls.borrow().is_empty());
        assert!(f.reply(request.lease.clone(), Some(chosen)));
        assert!(!f.reply(request.lease, Some(chosen)));
        assert_eq!(&*f.calls.borrow(), &[(CommitSource::Selection, Some(chosen))]);
        assert!(matches!(f.combo_mut().take_committed(), Some(CommittedValue::ExistingOption(id)) if id == chosen));

        f.tap(12, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        assert_eq!(request.selected, Some(chosen));
        assert!(!f.reply(request.lease.clone(), None));
        assert!(!f.reply(request.lease, Some(MountedHostChoice::OPTIONS[0].0)));
        assert_eq!(f.calls.borrow().len(), 1);
        assert_eq!(f.combo().input_text(), "Same");

        f.tap(13, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        let rejected = MountedHostChoice::OPTIONS[2].0;
        assert!(f.reply(request.lease, Some(rejected)));
        assert_eq!(f.calls.borrow().last(), Some(&(CommitSource::Selection, Some(rejected))));
        assert!(f.combo_mut().take_committed().is_none());
        assert_eq!(f.combo().input_text(), "Same");
    }

    #[test]
    fn hal400_picker_rejects_replaced_options_resize_and_retired_mounts() {
        use crate::core::widget_tree::OverlayHitRegion;
        let mut f = MountedHostChoice::new(false);
        let option = MountedHostChoice::OPTIONS[1].0;
        f.tap(1, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        assert!(!f.reply(request.lease, Some(OptionId::from_raw(42))));
        f.tap(2, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        f.combo_mut().set_options(&[MountedHostChoice::OPTIONS[1]]);
        assert!(!f.reply(request.lease, Some(option)));
        f.tap(3, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        f.layout(RectF::new(0.0, 0.0, 320.0, 100.0));
        assert!(!f.reply(request.lease, Some(option)));
        f.tap(4, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        let overlay = f.tree.get_absolute_paint_rect(f.combo).unwrap();
        f.tree.set_overlay_hit_regions(vec![OverlayHitRegion {
            abs_rect: overlay,
            owner_abs_rect: overlay,
            widget_id: f.tree.node(f.root).unwrap().id,
        }]);
        assert!(!f.reply(request.lease, Some(option)));
        f.tree.set_overlay_hit_regions(Vec::new());
        f.tap(5, f.center());
        let request = f.router.take_host_choice_request().unwrap();
        f.tree.remove_node(f.combo);
        assert!(!f.reply(request.lease, Some(option)));
        assert!(f.calls.borrow().is_empty());
    }

    #[test]
    fn hal400_second_contact_cannot_replace_or_cancel_the_first_lease() {
        use crate::core::event::TouchPhase;
        let mut f = MountedHostChoice::new(false);
        let (first, second) = (10, 23);
        let point = f.center();
        f.touch(first, TouchPhase::Started, point);
        f.touch(second, TouchPhase::Started, point);
        f.touch(first, TouchPhase::Ended, point);
        let request = f.router.take_host_choice_request().expect("first touch lease");
        f.touch(second, TouchPhase::Cancelled, point);
        assert!(f.router.take_host_choice_request().is_none());
        assert!(f.reply(request.lease, Some(MountedHostChoice::OPTIONS[1].0)));
        assert_eq!(f.calls.borrow().len(), 1);
    }

    #[test]
    fn hal400_pan_from_the_choice_does_not_open_and_uses_current_scroll_clip() {
        use crate::core::event::TouchPhase;
        use crate::widgets::scroll_area::ScrollArea;
        let mut f = MountedHostChoice::new(false);
        let before = f.tree.host_choice_placements(f.root, MountedHostChoice::VIEWPORT);
        let start = f.center();
        assert!(before[0].exposed.iter().any(|rect| rect.contains(start)));
        let end = Point::new(start.x, start.y - 12.0);
        f.touch(1, TouchPhase::Started, start);
        f.touch(1, TouchPhase::Moved, end);
        f.touch(1, TouchPhase::Ended, end);
        assert!(f.router.take_host_choice_request().is_none());
        assert!(!f.combo().popup_open.get());
        let area = f.tree.node(f.scroll).unwrap().widget.as_any().downcast_ref::<ScrollArea>().unwrap();
        assert!(area.visual_scroll_y() > 0.0);
        let clipped = f.tree.host_choice_placements(f.root, MountedHostChoice::VIEWPORT);
        assert_eq!(clipped[0].rect.y, before[0].rect.y - area.visual_scroll_y());
        assert!(clipped[0].exposed.iter().all(|rect| rect.y >= 0.0));
        assert!(f.calls.borrow().is_empty());
    }

    #[test]
    fn hal400_editor_region_keeps_keyboard_and_only_the_affordance_projects() {
        use crate::core::event::{KeyboardAuthorityMode, WindowId};
        use crate::core::event_router::host_choice::HostOffer;
        let mut f = MountedHostChoice::new(true);
        let bounds = f.tree.get_absolute_paint_rect(f.combo).unwrap();
        let places = f.tree.host_choice_placements(f.root, MountedHostChoice::VIEWPORT);
        assert_eq!(places.len(), 1);
        assert!(places[0].rect.x > bounds.x);
        assert_eq!(places[0].rect.right(), bounds.right());
        assert!(!places[0].rect.contains(bounds.center()));
        f.tap(1, bounds.center());
        assert!(f.router.take_host_choice_request().is_none());
        assert!(f.router.take_touch_keyboard_activation().is_some());
        assert_eq!(f.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Capture);
        f.layout(MountedHostChoice::VIEWPORT);
        let placement = f.tree.host_choice_placements(f.root, MountedHostChoice::VIEWPORT).remove(0);
        let point = placement.exposed[0].center();
        let (hit, origin) = f.tree.hit_test_for_routing(point.x, point.y).unwrap();
        assert_eq!(origin, PointerRouteOrigin::Regular);
        assert_eq!(hit.widget_id, placement.target.mount.owner());
        f.router.set_host_choice_offer(Some(HostOffer::RenderedSelect {
            contact: 2,
            target: placement.target.clone(),
        }));
        f.tap(2, point);
        let request = f.router.take_host_choice_request().unwrap_or_else(|| {
            panic!(
                "affordance SELECT revision {} became {} with popup={} and keyboard={:?}",
                placement.target.revision, f.combo().popup_input_revision, f.combo().popup_open.get(),
                f.domains.primary_state().keyboard_mode,
            )
        });
        assert_eq!(request.projected, placement.target);
        assert!(!f.combo().popup_open.get());
        assert_eq!(f.domains.primary_state().keyboard_mode, KeyboardAuthorityMode::Navigation);
        assert_eq!(request.lease.press.window, WindowId::PRIMARY);
        assert!(f.reply(request.lease, Some(MountedHostChoice::OPTIONS[0].0)));
    }

    #[test]
    fn combo_popup_scrollbar_auto_hide_preserves_scroll_and_row_hits() {
        use crate::compositor::SurfaceId;
        use crate::core::app::{FrameReason, FrameTime, test_begin_frame_layout_tracking};
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, Modifiers, PhysicalKeyCode, PointerButtonsState,
            PointerDevice, PointerId, ScrollSource, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::input_fusion::InputFusion;
        use crate::core::layout::{EdgeInsets, FlexParams};
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::{container::Container, flex::Flex, scroll_area::ScrollArea};
        use std::sync::Arc;
        use std::time::{Duration, Instant};

        use PopupScrollbarAppearanceError::*;
        use PopupScrollbarInteractionPolicyError::*;
        use PopupScrollbarPolicy::{AutoHide, WhenOverflowing};

        for invalid in [f32::NAN, f32::INFINITY, -0.1, 60.1] {
            assert_eq!(
                PopupScrollbarInteractionPolicy::try_new(AutoHide, invalid, 0.0),
                Err(InvalidHoldSeconds)
            );
        }
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 10.1] {
            assert_eq!(
                PopupScrollbarInteractionPolicy::try_new(AutoHide, 0.0, invalid),
                Err(InvalidFadeSeconds)
            );
        }
        assert!(PopupScrollbarInteractionPolicy::try_new(AutoHide, 60.0, 10.0).is_ok());
        for invalid in [f32::NAN, f32::INFINITY, -0.1, 1.1] {
            assert_eq!(
                PopupScrollbarAppearance::try_new(invalid, 1.0, 1.0),
                Err(InvalidTrackOpacity)
            );
            assert_eq!(
                PopupScrollbarAppearance::try_new(1.0, invalid, 1.0),
                Err(InvalidThumbOpacity)
            );
            assert_eq!(
                PopupScrollbarAppearance::try_new(1.0, 1.0, invalid),
                Err(InvalidThumbHoverOpacity)
            );
        }

        let tuned = PopupScrollbarInteractionPolicy::try_new(WhenOverflowing, 1.0, 0.5).unwrap();
        let instant = PopupScrollbarInteractionPolicy::try_new(AutoHide, 0.25, 0.0).unwrap();
        let tinted = PopupScrollbarAppearance::try_new(0.25, 0.6, 0.85).unwrap();
        let trackless = PopupScrollbarAppearance::try_new(0.0, 0.6, 0.85).unwrap();
        let transparent = PopupScrollbarAppearance::try_new(0.25, 0.0, 0.0).unwrap();

        for (interaction, appearance, mode) in [
            (None, None, None),
            (None, None, Some(AutoHide)),
            (Some(tuned), Some(tinted), Some(AutoHide)),
            (Some(instant), Some(trackless), None),
            (Some(instant), Some(transparent), None),
        ] {
            let auto_hide = mode.or(interaction.map(|policy| policy.display_mode()))
                == Some(AutoHide);
            let fully_transparent = appearance == Some(transparent);
            let mut combo =
                ComboBox::simple((0..22).map(|index| format!("Option {index}")).collect())
                    .with_interaction_policy(PopupInteractionPolicy {
                        width: PopupWidthPolicy::MatchTrigger,
                    });
            if let Some(interaction) = interaction {
                combo = combo
                    .with_popup_scrollbar_policy(WhenOverflowing)
                    .with_popup_scrollbar_interaction_policy(interaction);
                assert_eq!(combo.scrollbar_interaction, interaction);
            }
            if let Some(mode) = mode {
                combo = combo.with_popup_scrollbar_policy(mode);
            }
            if let Some(appearance) = appearance {
                combo = combo.with_popup_scrollbar_appearance(appearance);
            }
            let appearance = appearance.unwrap_or_default();
            let (hold, fade) = interaction.map_or((1.5, 0.3), |policy| {
                (policy.hold_seconds(), policy.fade_seconds())
            });
            assert_eq!(
                (
                    combo.scrollbar_interaction.hold_seconds(),
                    combo.scrollbar_interaction.fade_seconds(),
                ),
                (hold, fade)
            );
            assert_eq!(
                combo.scrollbar_interaction.display_mode(),
                if auto_hide { AutoHide } else { WhenOverflowing }
            );
            let halfway_fade = hold + fade * 0.5;
            let settle = hold + fade + 0.05;
            combo.set_text_input_enabled(false);
            let mut tree = WidgetTree::new();
            let mut viewport = Size::new(172.0, if auto_hide { 280.0 } else { 480.0 });
            let (root, parent, scroll) = if auto_hide {
                let root = tree.create_root(
                    Box::new(Flex::column().with_padding(EdgeInsets::all(12.0))),
                    None,
                    0,
                );
                (root, root, None)
            } else {
                let root = tree.create_root(Box::new(Flex::column()), None, 0);
                let scroll = tree.add_child(
                    root,
                    Box::new(ScrollArea::new_unwired().with_scrollbars(false)),
                    None,
                    0,
                );
                tree.set_flex(scroll, FlexParams::flexible(1.0));
                let parent = tree.add_child(
                    scroll,
                    Box::new(Flex::column().with_padding(EdgeInsets {
                        top: 208.0,
                        ..EdgeInsets::all(12.0)
                    })),
                    None,
                    0,
                );
                tree.node_mut(scroll)
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<ScrollArea>()
                    .unwrap()
                    .wire_content(parent);
                (root, parent, Some(scroll))
            };
            let node = tree.add_child(parent, Box::new(combo), None, 0);
            if !auto_hide {
                tree.add_child(parent, Box::new(Container::empty().with_height(480.0)), None, 0);
            }
            let owner = tree.node(node).unwrap().id;
            macro_rules! combo {
                () => {
                    tree.node(node)
                        .unwrap()
                        .widget
                        .as_any()
                        .downcast_ref::<ComboBox>()
                        .unwrap()
                };
            }
            let mut harness = RetainedSceneTestHarness::new();
            let mut engine = FontManagerTextEngine::new();
            let mut domains = InteractionDomains::new();
            let mut router = EventRouter::new();
            let mut fusion = InputFusion::new();
            let mut bindings = BindingRegistry::new();
            let mut theme = Theme::light();
            let track_alpha = if fully_transparent { 0.0 } else { 0.4 };
            theme.colors.scrollbars.track = theme.colors.scrollbars.track.with_alpha(track_alpha);
            let normal_thumb = theme.colors.scrollbars.thumb.with_alpha(0.7);
            theme.colors.scrollbars.thumb = normal_thumb;
            let hovered_thumb = theme.colors.scrollbars.thumb_hover.with_alpha(0.55);
            theme.colors.scrollbars.thumb_hover = hovered_thumb;
            let icons = IconRegistry::new(&[]);
            let mut time = FrameTime {
                now: Instant::now(),
                dt: Duration::ZERO,
                frame_index: 0,
                reason: FrameReason::Animation,
            };
            macro_rules! frame {
                ($secs:expr) => {{
                    time.dt = Duration::from_secs_f32($secs);
                    time.now += time.dt;
                    time.frame_index += 1;
                    test_begin_frame_layout_tracking();
                    harness.frame_with_gesture_sessions(
                        &mut tree,
                        root,
                        Constraints::tight(viewport),
                        &theme,
                        &mut engine,
                        &icons,
                        &time,
                        None,
                        &mut domains,
                        WindowId::PRIMARY,
                        &mut router,
                        &mut fusion,
                    )
                }};
            }
            macro_rules! route {
                ($event:expr) => {
                    router.route_input_event(
                        &mut tree,
                        $event,
                        Some(root),
                        &mut engine,
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    )
                };
            }
            let down = |pos| FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            };
            let up = |pos| FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            };
            let movement = |pos, primary| FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos,
                delta: (0.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState {
                    primary,
                    ..Default::default()
                },
            };
            let key = |key_code, physical_key| FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code,
                physical_key,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            };
            let scrollbar_nodes = |harness: &RetainedSceneTestHarness| {
                let overlay = harness.overlay_fragment(owner).expect("open popup overlay");
                surface_nodes(&overlay)
                    .iter()
                    .filter_map(|n| match n {
                        SceneNode::Rect(rect) if rect.z_index == POPUP_CHROME_LAYER => {
                            Some((rect.rect, rect.color))
                        }
                        _ => None,
                    })
                    .collect::<Vec<_>>()
            };
            let painted_scrollbar = |harness: &RetainedSceneTestHarness| {
                scrollbar_nodes(harness)
                    .into_iter()
                    .map(|(rect, _)| rect)
                    .collect::<Vec<_>>()
            };
            let assert_alpha = |harness: &RetainedSceneTestHarness, state, opacity: f32| {
                let painted = scrollbar_nodes(harness);
                assert_eq!(painted.len(), 2);
                let (thumb, thumb_opacity) = match state {
                    ScrollbarState::Normal => (normal_thumb, appearance.thumb_opacity()),
                    ScrollbarState::Hovered | ScrollbarState::Pressed => {
                        (hovered_thumb, appearance.thumb_hover_opacity())
                    }
                };
                let expected_parts = [
                    (theme.colors.scrollbars.track, appearance.track_opacity()),
                    (thumb, thumb_opacity),
                ];
                for ((_, color), (base, factor)) in painted.into_iter().zip(expected_parts) {
                    let expected: crate::core::scene::Color =
                        base.with_alpha(base.a * factor * opacity).into();
                    assert_eq!(
                        (color.r, color.g, color.b),
                        (expected.r, expected.g, expected.b)
                    );
                    assert!(
                        (color.a - expected.a).abs() < 0.0001,
                        "{color:?} vs {expected:?}"
                    );
                }
            };
            let drawn_scrollbar = |tree: &WidgetTree, harness: &RetainedSceneTestHarness| {
                let geometry = tree.combo_popup_input_geometry(owner).unwrap();
                let bar = geometry.drawn_scrollbar().unwrap();
                assert_eq!(painted_scrollbar(harness), [bar.track, bar.thumb]);
                (geometry, bar)
            };

            frame!(0.0);
            let rect = tree.get_absolute_paint_rect(node).unwrap();
            let arrow = Point::new(rect.right() - 3.0, rect.y + rect.height * 0.5);
            route!(down(arrow));
            route!(up(arrow));
            // A long idle interval before opening must not age the opening cue.
            let opening_dt = if auto_hide { 2.0 } else { 0.001 };
            frame!(opening_dt);
            {
                // Sample the real EaseOut opening exactly at half reveal on a 2x target.
                let half =
                    f64::from(opening_dt) + f64::from(POPUP_REVEAL_SECS) * (1.0 - 0.5_f64.sqrt());
                harness.advance_transition_time(half);
                let target = CustomRenderTargetSnapshot::new(
                    SurfaceId::test_id(1),
                    PhysicalExtent::new(344, (viewport.height * 2.0) as u32),
                    2.0,
                )
                .unwrap();
                macro_rules! partial_frame {
                    () => {
                        harness.frame_with_optional_custom_render_target(
                            &mut tree,
                            root,
                            Constraints::tight(viewport),
                            &theme,
                            &mut engine,
                            &icons,
                            Some(target),
                            Some((&domains, WindowId::PRIMARY)),
                        )
                    };
                }
                partial_frame!();
                if fully_transparent {
                    // Nonzero track factor over a transparent theme track is
                    // still invisible, as is the explicitly transparent thumb.
                    assert_eq!(combo!().scrollbar_fade_alpha, 1.0);
                    let partial = tree.combo_popup_input_geometry(owner).unwrap();
                    assert!(partial.drawn_scrollbar().is_none());
                    let bar = partial.scrollbar.unwrap().layout;
                    let gutter_row = Point::new(
                        bar.track.right() - 1.0,
                        rect.y + partial.rows.y + ROW_HEIGHT * 1.5,
                    );
                    route!(movement(gutter_row, false));
                    partial_frame!();
                    assert!(painted_scrollbar(&harness).is_empty());
                    assert!(
                        tree.combo_popup_input_geometry(owner)
                            .unwrap()
                            .drawn_scrollbar()
                            .is_none()
                    );
                    route!(down(gutter_row));
                    assert!(!combo!().scrollbar_v.dragging);
                    route!(up(gutter_row));
                    assert_eq!(
                        combo!().committed_value_string().as_deref(),
                        Some("Option 1")
                    );
                    assert!(tree.combo_popup_input_geometry(owner).is_none());
                    continue;
                }
                let (partial, bar) = drawn_scrollbar(&tree, &harness);
                assert_eq!(partial.rows.height, 192.0);
                assert_eq!((bar.track.height, bar.thumb.height), (94.0, 22.0));
                assert_alpha(&harness, ScrollbarState::Normal, 1.0);
                if !auto_hide || appearance == tinted {
                    let point = Point::new(bar.thumb.x + 2.0, rect.y + partial.rows.y + 45.0);
                    assert!(!bar.thumb.contains(point) && bar.track.contains(point));
                    route!(down(point));
                    assert_eq!(
                        combo!().popup_scroll_row,
                        8,
                        "45px must page inside the drawn track"
                    );
                    assert!(!combo!().scrollbar_v.dragging);
                    assert_eq!(
                        domains
                            .primary_state()
                            .pointer_capture_manager
                            .current_capture(PointerId::MOUSE),
                        None
                    );
                    route!(up(point));
                    partial_frame!();

                    let (_, paged) = drawn_scrollbar(&tree, &harness);
                    let grab =
                        Point::new(paged.thumb.x + 2.0, paged.thumb.y + paged.thumb.height * 0.5);
                    route!(down(grab));
                    assert!(combo!().scrollbar_v.dragging);
                    assert_eq!(
                        domains
                            .primary_state()
                            .pointer_capture_manager
                            .current_capture(PointerId::MOUSE),
                        Some(owner)
                    );
                    partial_frame!();
                    assert_alpha(&harness, ScrollbarState::Pressed, 1.0);
                    route!(up(grab));
                    assert!(!combo!().scrollbar_v.dragging);
                    if auto_hide {
                        route!(movement(arrow, false));
                        route!(key(KeyCode::Home, PhysicalKeyCode::Home));
                        partial_frame!();
                    }
                }
            }
            frame!(0.050);
            let popup = ComboBox::test_popup_local_rect(&tree, node).translate(rect.x, rect.y);
            assert_eq!(popup.width, rect.width);
            let popup = PopupGeometry::from_rects(rect, popup).snapped(1.0).popup;
            let bar = combo!().popup_scrollbar_layout(popup).unwrap();
            if !auto_hide {
                assert_eq!(painted_scrollbar(&harness), [bar.track, bar.thumb]);
                let overlay = harness.overlay_fragment(owner).unwrap();
                frame!(2.0);
                assert!(Arc::ptr_eq(&overlay, &harness.overlay_fragment(owner).unwrap()));
                assert_alpha(&harness, ScrollbarState::Normal, 1.0);
                assert_eq!(drawn_scrollbar(&tree, &harness).1.thumb, bar.thumb);

                // Move the viewport edge while keeping the same trigger allocation.
                viewport.height = 360.0;
                frame!(0.0);
                let (above, _) = drawn_scrollbar(&tree, &harness);
                assert_eq!(above.rows.y, -above.rows.height);
                assert_eq!(tree.get_absolute_paint_rect(node).unwrap(), rect);

                tree.node_mut(scroll.unwrap())
                    .unwrap()
                    .widget
                    .as_any_mut()
                    .downcast_mut::<ScrollArea>()
                    .unwrap()
                    .jump_to_y(180.0);
                assert!(tree.combo_popup_input_geometry(owner).is_none());
                frame!(0.0);
                let (below, _) = drawn_scrollbar(&tree, &harness);
                assert_eq!(below.scroll_offset, (0.0, 180.0));
                assert_eq!(below.rows.y, rect.height);
                assert_eq!(tree.get_absolute_paint_rect(node).unwrap(), rect);
                let row = Point::new(rect.x + below.rows.x + 12.0,
                    rect.y + below.rows.y - below.scroll_offset.1 + ROW_HEIGHT * 0.5);
                assert_eq!(tree.hit_test(row.x, row.y).unwrap().widget_id, owner);
                route!(down(row));
                route!(up(row));
                assert_eq!(combo!().committed_value_string().as_deref(), Some("Option 8"));
                assert!(tree.combo_popup_input_geometry(owner).is_none());
                continue;
            }

            let assert_text_clip = |harness: &RetainedSceneTestHarness, right| {
                let overlay = harness.overlay_fragment(owner).unwrap();
                let pad = theme.geometry.text_field.padding_x;
                assert!(surface_nodes(&overlay).iter().any(|node| matches!(node,
                    SceneNode::Clip(clip) if clip.rect.x == popup.x + pad
                        && clip.rect.right() == right - pad
                        && clip.rect.height == popup.height
                )));
            };
            let row = Point::new(popup.x + 16.0, popup.y + ROW_HEIGHT * 1.5);
            let gutter = Point::new(bar.hit_rect.x + 1.0, row.y);
            assert_alpha(&harness, ScrollbarState::Normal, 1.0);
            assert_text_clip(&harness, bar.track.x);
            if fade == 0.0 {
                // The thumb alone keeps the shared rail usable until the
                // configured hold expires, then the row owns the gutter.
                drawn_scrollbar(&tree, &harness);
                frame!(hold - 0.10);
                assert_alpha(&harness, ScrollbarState::Normal, 1.0);
                frame!(0.06);
                assert!(painted_scrollbar(&harness).is_empty());
                assert!(!combo!().needs_update());
                assert_text_clip(&harness, popup.right());
                route!(movement(gutter, false));
                frame!(0.0);
                assert_alpha(&harness, ScrollbarState::Hovered, 1.0);
                frame!(settle);
                drawn_scrollbar(&tree, &harness);
                route!(movement(row, false));
                frame!(0.0);
                frame!(settle);
                assert!(painted_scrollbar(&harness).is_empty());
                route!(down(gutter));
                assert!(!combo!().scrollbar_v.dragging);
                route!(up(gutter));
                assert_eq!(
                    combo!().committed_value_string().as_deref(),
                    Some("Option 1")
                );
                continue;
            }
            let held = harness.overlay_fragment(owner).unwrap();
            frame!(0.45);
            assert!(Arc::ptr_eq(
                &held,
                &harness.overlay_fragment(owner).unwrap()
            ));
            frame!(hold - 0.50);
            assert_alpha(&harness, ScrollbarState::Normal, 1.0);
            frame!(fade * 0.5);
            assert_alpha(&harness, ScrollbarState::Normal, 0.5);
            assert_eq!(drawn_scrollbar(&tree, &harness).1.thumb, bar.thumb);
            assert_text_clip(&harness, bar.track.x);
            frame!(fade * 0.5);
            assert!(painted_scrollbar(&harness).is_empty());
            assert!(
                tree.combo_popup_input_geometry(owner)
                    .unwrap()
                    .drawn_scrollbar()
                    .is_none()
            );
            assert_text_clip(&harness, popup.right());
            assert!(!combo!().needs_update());
            route!(movement(row, false));
            frame!(0.0);
            assert!(painted_scrollbar(&harness).is_empty());
            assert_eq!(combo!().test_highlighted_label(), Some("Option 1"));

            route!(movement(gutter, false));
            assert!(tree.combo_popup_input_geometry(owner).unwrap().drawn_scrollbar().is_none());
            frame!(0.0);
            assert_eq!(painted_scrollbar(&harness), [bar.track, bar.thumb]);
            assert_alpha(&harness, ScrollbarState::Hovered, 1.0);
            assert_text_clip(&harness, bar.track.x);
            frame!(2.0);
            assert_eq!(painted_scrollbar(&harness), [bar.track, bar.thumb]);

            route!(movement(row, false));
            frame!(0.0);
            frame!(halfway_fade);
            assert_alpha(&harness, ScrollbarState::Normal, 0.5);
            route!(movement(gutter, false));
            frame!(0.0);
            assert_alpha(&harness, ScrollbarState::Hovered, 1.0);
            route!(movement(row, false));
            frame!(0.0);
            frame!(2.0);
            assert!(painted_scrollbar(&harness).is_empty());
            assert_text_clip(&harness, popup.right());
            assert!(!combo!().needs_animation());
            assert!(!combo!().needs_update());

            route!(FusedInputEvent::Scroll {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: row,
                delta: (0.0, -2.0 * ROW_HEIGHT),
                source: ScrollSource::Wheel,
            });
            frame!(0.0);
            assert_eq!(combo!().popup_scroll_row, 2);
            let scroll = painted_scrollbar(&harness);
            assert_eq!(scroll.len(), 2);
            assert!(scroll[1].y > bar.thumb.y);
            assert_alpha(&harness, ScrollbarState::Normal, 1.0);
            frame!(halfway_fade);
            assert_alpha(&harness, ScrollbarState::Normal, 0.5);
            drawn_scrollbar(&tree, &harness);
            let end_of_track = Point::new(bar.track.right() - 1.0, bar.track.bottom() - 1.0);
            route!(down(end_of_track));
            route!(up(end_of_track));
            frame!(0.0);
            assert_alpha(&harness, ScrollbarState::Normal, 1.0);
            let visible = combo!().visible_rows();
            assert_eq!(combo!().popup_scroll_row, 2 + visible);
            let thumb = painted_scrollbar(&harness)[1];
            let grab = Point::new(thumb.x + thumb.width * 0.5, thumb.y + thumb.height * 0.5);
            route!(down(grab));
            assert!(combo!().scrollbar_v.dragging);
            let outside = Point::new(popup.x - 2.0, popup.bottom() + ROW_HEIGHT);
            route!(movement(outside, true));
            frame!(0.0);
            frame!(2.0);
            assert_alpha(&harness, ScrollbarState::Pressed, 1.0);
            assert_eq!(combo!().popup_scroll_row, 22 - visible);
            assert_eq!(
                painted_scrollbar(&harness)[1],
                combo!().popup_scrollbar_layout(popup).unwrap().thumb
            );
            route!(up(outside));
            assert!(!combo!().scrollbar_v.dragging);
            assert!(combo!().committed_value_string().is_none());
            frame!(0.0);
            frame!(2.0);
            assert!(painted_scrollbar(&harness).is_empty());

            route!(key(KeyCode::Home, PhysicalKeyCode::Home));
            frame!(0.0);
            assert_eq!(combo!().popup_scroll_row, 0);
            assert_eq!(painted_scrollbar(&harness), [bar.track, bar.thumb]);
            route!(key(KeyCode::End, PhysicalKeyCode::End));
            frame!(0.0);
            assert_eq!(combo!().test_highlighted_label(), Some("Option 21"));
            assert_eq!(combo!().popup_scroll_row, 22 - visible);
            frame!(2.0);
            assert!(painted_scrollbar(&harness).is_empty());

            // The final row remains selectable even within the hidden track.
            let last = Point::new(bar.track.right() - 1.0, popup.bottom() - ROW_HEIGHT * 0.5);
            route!(down(last));
            route!(up(last));
            assert_eq!(
                combo!().committed_value_string().as_deref(),
                Some("Option 21")
            );

            route!(down(arrow));
            route!(up(arrow));
            frame!(0.001);
            frame!(0.050);
            assert_alpha(&harness, ScrollbarState::Normal, 1.0);
            drawn_scrollbar(&tree, &harness);
            // Both policies retain ComboBox's existing raw-touch admission.
            assert!(!combo!().accepts_touch_pointer_input());
            let touch = Point::new(last.x, row.y);
            route!(FusedInputEvent::TouchDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pointer_id: PointerId(1),
                pos: touch,
                button: PointerButton::Primary,
                click_count: 1,
            });
            route!(FusedInputEvent::TouchUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pointer_id: PointerId(1),
                pos: touch,
                button: PointerButton::Primary,
            });
            assert_eq!(
                combo!().committed_value_string().as_deref(),
                Some("Option 21")
            );
            assert!(!combo!().option_press_active);
            route!(FusedInputEvent::Scroll {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: touch,
                delta: (0.0, -ROW_HEIGHT),
                source: ScrollSource::Touchpad,
            });
            frame!(0.0);
            assert_eq!(combo!().popup_scroll_row, 1);
            frame!(2.0);
            assert_eq!(painted_scrollbar(&harness).len(), 2);
            route!(movement(row, false));
            frame!(0.0);
            frame!(2.0);
            assert!(painted_scrollbar(&harness).is_empty());

            route!(key(KeyCode::Escape, PhysicalKeyCode::Escape));
            tree.node_mut(node)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ComboBox>()
                .unwrap()
                .set_options(&[(OptionId::from_raw(0), "Option 0")]);
            tree.mark_layout_dirty(node);
            route!(down(arrow));
            route!(up(arrow));
            frame!(0.0);
            frame!(0.050);
            assert!(combo!().popup_open.get());
            assert!(
                tree.combo_popup_input_geometry(owner)
                    .unwrap()
                    .scrollbar
                    .is_none()
            );
            assert!(painted_scrollbar(&harness).is_empty());
            assert!(!combo!().needs_update());
        }
    }

    fn chrome_paths(nodes: &[SceneNode]) -> Vec<&PathNode> {
        let mut texts = Vec::new();
        let mut rects = Vec::new();
        let mut borders = Vec::new();
        let mut paths = Vec::new();
        collect(nodes, &mut texts, &mut rects, &mut borders, &mut paths);
        assert!(
            !borders
                .iter()
                .any(|border| border.z_index == crate::core::paint::LAYER_FOCUS),
            "the joined chrome is the sole focus/capture outline"
        );
        paths
            .into_iter()
            .filter(|path| {
                path.is_stroke && (path.z_index == 3 || path.z_index == POPUP_BORDER_LAYER)
            })
            .collect()
    }

    #[test]
    fn combo_joined_geometry_keeps_attachment_ledges_and_fractional_corners() {
        let narrow = PopupGeometry::new(
            RectF::new(20.0, 20.0, 149.0, 24.0),
            (0.0, 0.0),
            (360.0, 280.0),
            150.0,
            96.0,
            1.0,
        )
        .snapped(1.5);
        let narrow_corners = narrow.outline_corners(2.0, 2.0 / 1.5);
        for i in 0..narrow_corners.len() {
            let (point, _, radius) = narrow_corners[i];
            if radius > 0.0 {
                let prev = narrow_corners[(i + narrow_corners.len() - 1) % narrow_corners.len()].0;
                let next = narrow_corners[(i + 1) % narrow_corners.len()].0;
                let outer_radius = radius + 1.0 / 1.5;
                assert!(outer_radius <= (point.x - prev.x).abs() + (point.y - prev.y).abs());
                assert!(outer_radius <= (next.x - point.x).abs() + (next.y - point.y).abs());
            }
        }
        let narrow_ledge =
            narrow.popup.x + narrow.popup.width - narrow.control.x - narrow.control.width;
        assert!(
            narrow.corners(narrow.popup, 2.0, 2.0 / 1.5).tr <= narrow_ledge,
            "a short ledge must bound the outer corner without reversing the contour"
        );
        for (control, scroll, viewport, width, above, shifted) in [
            (
                RectF::new(20.0, 20.0, 180.0, 24.0),
                (0.0, 0.0),
                (360.0, 280.0),
                180.0,
                false,
                false,
            ),
            (
                RectF::new(240.0, 230.0, 90.0, 24.0),
                (0.0, 0.0),
                (360.0, 280.0),
                180.0,
                true,
                true,
            ),
            (
                RectF::new(50.0, 110.0, 100.0, 24.0),
                (30.0, 90.0),
                (360.0, 280.0),
                150.0,
                false,
                false,
            ),
            (
                RectF::new(20.0, 20.0, 149.0, 24.0),
                (0.0, 0.0),
                (360.0, 280.0),
                150.0,
                false,
                false,
            ),
        ] {
            let geometry =
                PopupGeometry::new(control, scroll, viewport, width, 96.0, 1.0).snapped(1.25);
            assert_eq!(geometry.popup.y < control.y, above);
            assert_eq!(geometry.popup.x < control.x, shifted);
            let joint = if above {
                geometry.control.y
            } else {
                geometry.control.y + geometry.control.height
            };
            let shared_left = geometry.control.x.max(geometry.popup.x);
            let shared_right = (geometry.control.x + geometry.control.width)
                .min(geometry.popup.x + geometry.popup.width);
            let outline = geometry.outline(2.0, 0.8);
            let points = outline.flatten();
            assert!(matches!(
                outline.commands.last(),
                Some(crate::renderer::path::PathCommand::Close)
            ));
            assert!(
                outline
                    .build_stroke_mesh(StrokeParams {
                        width: 0.8,
                        ..Default::default()
                    })
                    .is_some()
            );
            for edge in points.windows(2) {
                assert!(
                    !((edge[0][1] - joint).abs() <= 0.8
                        && (edge[1][1] - joint).abs() <= 0.8
                        && edge[0][0].min(edge[1][0]) < shared_right - 2.0
                        && edge[0][0].max(edge[1][0]) > shared_left + 2.0),
                    "no border may cross the shared interior"
                );
            }
            let corners = geometry.corners(geometry.control, 2.0, 0.8);
            if above {
                assert_eq!(corners.bl, 2.0);
                assert_eq!(corners.br, 2.0);
            } else {
                assert_eq!(corners.tl, 2.0);
                assert_eq!(corners.tr, 2.0);
            }
            if control.width == 149.0 {
                let ledge = geometry.popup.x + geometry.popup.width
                    - geometry.control.x
                    - geometry.control.width;
                let corner = geometry.corners(geometry.popup, 2.0, 0.8).tr;
                assert!((corner - (ledge + 0.8) * 0.5).abs() < 0.0001);
                assert!(
                    corner < 2.0,
                    "a sub-radius ledge must constrain fill, clip and stroke alike"
                );
            }
            let partial =
                PopupGeometry::new(control, scroll, viewport, width, 96.0, 0.01).snapped(1.25);
            assert!(
                partial
                    .outline(2.0, 2.0)
                    .build_stroke_mesh(StrokeParams {
                        width: partial.popup.height.min(2.0),
                        ..Default::default()
                    })
                    .is_some()
            );
        }
    }

    #[test]
    fn combo_joined_capture_is_routed_and_retained_without_programmatic_capture() {
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, PointerDevice,
            ScrollSource, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::EdgeInsets;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::{sync::Arc, time::Instant};

        let theme = Theme::light();
        let icons = IconRegistry::new(&[]);
        let mut engine = FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets {
                top: 12.0,
                left: 20.0,
                right: 160.0,
                bottom: 0.0,
            })),
            None,
            0,
        );
        let mut combo = ComboBox::simple((0..12).map(|i| format!("Option {i}")).collect());
        combo.set_text_input_enabled(false);
        let node = tree.add_child(root, Box::new(combo), None, 0);
        let sibling = tree.add_child(
            root,
            Box::new(crate::widgets::label::Label::new("Sibling")),
            None,
            0,
        );
        let owner = tree.node(node).unwrap().id;
        let sibling_owner = tree.node(sibling).unwrap().id;
        let constraints = Constraints::tight(Size::new(360.0, 280.0));
        let mut domains = InteractionDomains::new();
        harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut engine,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        let rect = tree.get_absolute_paint_rect(node).unwrap();
        let sibling_fragment = harness.local_fragment(sibling_owner).unwrap();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(rect.x + rect.width - 3.0, rect.y + rect.height * 0.5),
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            Some(root),
            &mut engine,
            None,
            domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut engine,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        harness.advance_transition_time(0.05);
        let opened = harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut engine,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        let paths = chrome_paths(&opened.scene_root);
        assert_eq!(paths.len(), 2);
        assert_eq!(
            paths[0].path.flatten(),
            paths[1].path.flatten(),
            "both clip scopes must partition the very same perimeter"
        );
        let captured = theme
            .input_chrome(ControlChromeState::Captured)
            .chrome
            .border_color;
        assert!(paths.iter().all(|path| path.color == captured.into()));
        // JC10A: readable neutral attachment inside the unchanged captured union.
        // Existing joined-chrome proof did not distinguish a neutral divider from an orange cut.
        let neutral = theme
            .input_chrome(ControlChromeState::Idle)
            .chrome
            .border_color;
        let divider = |nodes: &[SceneNode]| {
            let mut texts = Vec::new();
            let mut rects = Vec::new();
            let mut borders = Vec::new();
            let mut paths = Vec::new();
            collect(nodes, &mut texts, &mut rects, &mut borders, &mut paths);
            paths
                .into_iter()
                .find(|path| {
                    !path.is_stroke
                        && path.z_index == LAYER_CONTENT + 1
                        && path.color == neutral.into()
                })
                .map(|path| path.path.flatten())
                .expect("visible popup separates its real input with neutral chrome")
        };
        let below_divider = divider(&opened.scene_root);
        assert!(below_divider.iter().any(|p| p[1] == rect.bottom()));
        assert!(below_divider.iter().all(|p| p[0] > rect.x
            && p[0] < rect.right()
            && p[1] >= rect.y
            && p[1] <= rect.bottom()));
        assert!(Arc::ptr_eq(
            &sibling_fragment,
            &harness.local_fragment(sibling_owner).unwrap()
        ));
        let local = harness.local_fragment(owner).unwrap();
        let overlay = harness.overlay_fragment(owner).unwrap();
        let settled = harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut engine,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        assert!(!settled.scene_built);
        assert!(Arc::ptr_eq(&local, &harness.local_fragment(owner).unwrap()));
        assert!(Arc::ptr_eq(
            &overlay,
            &harness.overlay_fragment(owner).unwrap()
        ));

        let popup = ComboBox::test_popup_local_rect(&tree, node);
        router.route_input_event(
            &mut tree,
            FusedInputEvent::Scroll {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(rect.x + popup.x + 20.0, rect.y + popup.y + 40.0),
                delta: (0.0, -24.0),
                source: ScrollSource::Wheel,
            },
            Some(root),
            &mut engine,
            None,
            domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .unwrap()
                .popup_scroll_row,
            1
        );
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert_eq!(
            domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        let scrolled = harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut engine,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        assert!(scrolled.scene_built);
        assert_eq!(chrome_paths(&scrolled.scene_root).len(), 2);
        assert!(!Arc::ptr_eq(
            &overlay,
            &harness.overlay_fragment(owner).unwrap()
        ));
        assert!(Arc::ptr_eq(
            &sibling_fragment,
            &harness.local_fragment(sibling_owner).unwrap()
        ));

        let option_point = Point::new(rect.x + popup.x + 20.0, rect.y + popup.y + ROW_HEIGHT * 0.5);
        for event in [
            FusedInputEvent::PointerDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: option_point,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                click_count: 1,
            },
            FusedInputEvent::PointerUp {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: option_point,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
            },
        ] {
            router.route_input_event(
                &mut tree,
                event,
                Some(root),
                &mut engine,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
        }
        assert_eq!(
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .unwrap()
                .committed_value_string()
                .as_deref(),
            Some("Option 1")
        );

        let mut programmatic = ComboBox::simple(vec!["Alpha".into(), "Beta".into()]);
        programmatic.set_text_input_enabled(false);
        programmatic.open_popup();
        let mut separate_tree = WidgetTree::new();
        let separate_root = separate_tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets {
                top: 200.0,
                left: 60.0,
                right: 180.0,
                bottom: 0.0,
            })),
            None,
            0,
        );
        let separate_control =
            separate_tree.add_child(separate_root, Box::new(programmatic), None, 0);
        let frame = RetainedSceneTestHarness::new().frame(
            &mut separate_tree,
            separate_root,
            Constraints::tight(Size::new(360.0, 240.0)),
            &theme,
            &mut engine,
            &icons,
        );
        let paths = chrome_paths(&frame.scene_root);
        assert_eq!(paths.len(), 2);
        let idle = theme
            .input_chrome(ControlChromeState::Idle)
            .chrome
            .border_color;
        assert!(
            paths
                .iter()
                .all(|path| path.color == idle.into() && path.color != captured.into())
        );
        let above_rect = separate_tree
            .get_absolute_paint_rect(separate_control)
            .unwrap();
        let above_divider = divider(&frame.scene_root);
        assert!(above_divider.iter().any(|p| p[1] == above_rect.y));
        assert!(above_divider.iter().all(|p| p[0] > above_rect.x
            && p[0] < above_rect.right()
            && p[1] >= above_rect.y
            && p[1] <= above_rect.bottom()));
    }
    #[test]
    fn combo_joined_zero_reveal_keeps_backdrop_then_restores_closed_shell() {
        use crate::core::layout::EdgeInsets;
        use crate::widgets::flex::Flex;
        let mut combo = ComboBox::simple(vec!["Alpha".into(), "Beta".into()]);
        combo.set_text_input_enabled(false);
        combo.open_popup();
        combo.popup_presentation.snap_requested = false;
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_padding(EdgeInsets::all(12.0))),
            None,
            0,
        );
        let node = tree.add_child(root, Box::new(combo), None, 0);
        let owner = tree.node(node).unwrap().id;
        let mut harness = RetainedSceneTestHarness::new();
        let mut engine = FontManagerTextEngine::new();
        let theme = Theme::light();
        let icons = IconRegistry::new(&[]);
        let constraints = Constraints::tight(Size::new(240.0, 180.0));
        harness.advance_transition_time(0.1);
        let zero = harness.frame(&mut tree, root, constraints, &theme, &mut engine, &icons);
        assert!(chrome_paths(&zero.scene_root).is_empty());
        assert_eq!(
            ComboBox::test_popup_local_rect(&tree, node).height,
            ROW_HEIGHT * 2.0
        );
        assert_eq!(
            tree.hit_test(230.0, 170.0).unwrap().widget_id,
            owner,
            "the first invisible reveal frame must retain its dismiss backdrop"
        );
        harness.advance_transition_time(0.15);
        let opened = harness.frame(&mut tree, root, constraints, &theme, &mut engine, &icons);
        assert_eq!(chrome_paths(&opened.scene_root).len(), 2);
        let combo = tree
            .node_mut(node)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<ComboBox>()
            .unwrap();
        combo.close_without_commit();
        assert!(!combo.popup_open.get());
        assert!(matches!(
            combo.input_background_state(),
            InputBgState::Default
        ));
        harness.advance_transition_time(0.2);
        harness.frame(&mut tree, root, constraints, &theme, &mut engine, &icons);
        harness.advance_transition_time(0.25);
        let closed = harness.frame(&mut tree, root, constraints, &theme, &mut engine, &icons);
        assert!(
            closed.scene_built,
            "terminal reveal change invalidates retained paint"
        );
        assert!(chrome_paths(&closed.scene_root).is_empty());
        assert!(
            !tree
                .node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<ComboBox>()
                .unwrap()
                .popup_open
                .get()
        );
        assert_ne!(
            tree.hit_test(230.0, 170.0).map(|hit| hit.widget_id),
            Some(owner)
        );
        let local = harness.local_fragment(owner).unwrap();
        let combo = tree
            .node(node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<ComboBox>()
            .unwrap();
        assert_eq!(combo.popup_presentation.reveal, 0.0);
        assert!(combo.popup_presentation.animation.is_none());
        assert!(matches!(
            combo.input_background_state(),
            InputBgState::Default
        ));
        assert!(!combo.needs_animation());
        harness.advance_transition_time(0.3);
        let quiet = harness.frame(&mut tree, root, constraints, &theme, &mut engine, &icons);
        assert!(!quiet.scene_built);
        assert!(std::sync::Arc::ptr_eq(
            &local,
            &harness.local_fragment(owner).unwrap()
        ));
    }
    #[test]
    fn combo_popup_width_ignores_long_input_in_constrained_controls() {
        use crate::compositor::SurfaceId;
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PointerDevice,
            WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::EdgeInsets;
        use crate::renderer::custom_render_prepare::CustomRenderTargetSnapshot;
        use crate::renderer::pixel_coverage::PhysicalExtent;
        use crate::runtime::actions::ActionId;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::time::Instant;

        let theme = Theme::light();
        let icons = IconRegistry::new(&[]);
        let long_input = "w".repeat(160);
        for scale in [1.0, 2.0] {
            for editable in [true, false] {
                for (left, top, right, above) in
                    [(20.0, 20.0, 440.0, false), (440.0, 310.0, 20.0, true)]
                {
                    let mut engine = FontManagerTextEngine::new();
                    let mut harness = RetainedSceneTestHarness::new();
                    let mut tree = WidgetTree::new();
                    let root = tree.create_root(
                        Box::new(Flex::column().with_padding(EdgeInsets {
                            left,
                            top,
                            right,
                            bottom: 0.0,
                        })),
                        None,
                        0,
                    );
                    let mut combo =
                        ComboBox::simple(vec!["Alpha".into(), "Beta".into(), "Gamma".into()]);
                    combo.set_text_input_enabled(editable);
                    if !editable {
                        combo
                            .set_input_text(long_input.clone())
                            .expect("unmounted initial text");
                    }
                    let node = tree.add_child(root, Box::new(combo), None, 0);
                    let owner = tree.node(node).unwrap().id;
                    let constraints = Constraints::tight(Size::new(640.0, 360.0));
                    let target = CustomRenderTargetSnapshot::new(
                        SurfaceId::test_id(1),
                        PhysicalExtent::new((640.0 * scale) as u32, (360.0 * scale) as u32),
                        scale,
                    )
                    .unwrap();
                    let mut domains = InteractionDomains::new();
                    let mut router = EventRouter::new();
                    let mut bindings = BindingRegistry::new();
                    macro_rules! frame {
                        () => {
                            harness.frame_with_custom_render_target(
                                &mut tree,
                                root,
                                constraints,
                                &theme,
                                &mut engine,
                                &icons,
                                target,
                            )
                        };
                    }
                    macro_rules! route {
                        ($event:expr) => {
                            router.route_input_event(
                                &mut tree,
                                $event,
                                Some(root),
                                &mut engine,
                                None,
                                domains.primary_state_mut(),
                                &mut bindings,
                            )
                        };
                    }
                    frame!();
                    let control = tree.get_absolute_paint_rect(node).unwrap();
                    assert_eq!(control.width, 180.0);
                    router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
                    assert!(router.handle_engine_action(
                        &mut tree,
                        ActionId::ENTER_WIDGET,
                        Some(root),
                        domains.primary_state_mut(),
                        &mut engine,
                        None,
                    ));
                    frame!();
                    harness.advance_transition_time(0.05);
                    frame!();
                    let before = ComboBox::test_popup_local_rect(&tree, node);
                    route!(FusedInputEvent::TextInput {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        text: if editable {
                            long_input.clone()
                        } else {
                            "ignored".into()
                        },
                        modifiers: Modifiers::default(),
                    });
                    frame!();
                    assert_eq!(tree.get_absolute_paint_rect(node).unwrap(), control);
                    assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
                    assert_eq!(
                        domains.primary_state().keyboard_mode,
                        KeyboardAuthorityMode::Capture
                    );
                    let popup = {
                        let combo = tree
                            .node(node)
                            .unwrap()
                            .widget
                            .as_any()
                            .downcast_ref::<ComboBox>()
                            .unwrap();
                        assert_eq!(combo.input_text(), long_input);
                        assert!(combo.popup_open.get());
                        assert_eq!(
                            combo
                                .snapshot
                                .items
                                .iter()
                                .map(|(id, _)| *id)
                                .collect::<Vec<_>>(),
                            [
                                OptionId::from_raw(0),
                                OptionId::from_raw(1),
                                OptionId::from_raw(2)
                            ],
                        );
                        ComboBox::test_popup_local_rect(&tree, node)
                    };
                    assert_eq!(
                        popup.width, control.width,
                        "long input must not resize the option popup: scale={scale}, editable={editable}, control={control:?}, popup={popup:?}",
                    );
                    assert_eq!(popup, before);
                    assert_eq!(popup.x, 0.0);
                    assert_eq!(popup.height, ROW_HEIGHT * 3.0);
                    assert_eq!(popup.y, if above { -popup.height } else { control.height });
                    let row = Point::new(
                        control.x + popup.x + 20.0,
                        control.y + popup.y + ROW_HEIGHT * 1.5,
                    );
                    route!(FusedInputEvent::PointerDown {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: row,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                        click_count: 1,
                    });
                    assert!(
                        tree.node_mut(node)
                            .unwrap()
                            .widget
                            .as_any_mut()
                            .downcast_mut::<ComboBox>()
                            .unwrap()
                            .take_committed()
                            .is_none()
                    );
                    route!(FusedInputEvent::PointerUp {
                        window_id: WindowId::PRIMARY,
                        time: Instant::now(),
                        pos: row,
                        device: PointerDevice::Mouse,
                        button: PointerButton::Primary,
                    });
                    let combo = tree
                        .node_mut(node)
                        .unwrap()
                        .widget
                        .as_any_mut()
                        .downcast_mut::<ComboBox>()
                        .unwrap();
                    assert_eq!(
                        combo.take_committed(),
                        Some(CommittedValue::ExistingOption(OptionId::from_raw(1))),
                    );
                    assert_eq!(combo.input_text(), "Beta");
                    assert!(!combo.popup_open.get());
                }
            }
        }
    }
}
