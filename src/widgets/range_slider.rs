// SPEC-HAL-275: RangeSlider Widget
// Dual-handle slider for selecting a value range [low, high] within a bounded domain.
// Extends Slider architecture with second handle and range-fill semantics.
// Reuses SliderDomain and SliderGeometry — no new theme geometry.
//
// SPEC-HAL-278: Bar display style — filled rectangle with edge handles and center-drag.

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyboardAuthorityMode, PointerButton, PointerEventKind,
};
use crate::core::layout::{
    Constraints, ControlPartRequirement, ControlPartsRequest, ControlPartsRequirements,
    ControlSideRequirements, LayoutResult, ResolvedControlParts,
};
use crate::core::math::{RectF, Size};
use crate::core::paint::{LAYER_CONTENT, RoundedCorners, TrackDisplayStyle};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::{ControlChromeCategory, ControlChromeState, ControlValidationState};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::slider::{SliderDomain, paint_track_fill, paint_track_thumb, paint_track_well};

// ============================================================================
// Handle Identity
// ============================================================================

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum RangeHandle {
    Low,
    High,
}

// ============================================================================
// Drag Mode (SPEC-HAL-278)
// ============================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
enum DragMode {
    /// Dragging a single handle (edge resize).
    Handle(RangeHandle),
    /// Dragging the entire range (center-drag). Stores the grab offset in domain units
    /// (distance from the pointer's domain value to `value.low` at grab time).
    CenterSlide { grab_offset: f32 },
}

// ============================================================================
// Bar Hover Zone
// ============================================================================

/// Which zone of the bar the cursor is currently in.
#[derive(Clone, Copy, PartialEq, Debug, Default)]
enum BarHoverZone {
    #[default]
    None,
    /// Cursor is over the low (left) edge handle zone.
    LowEdge,
    /// Cursor is over the high (right) edge handle zone.
    HighEdge,
    /// Cursor is between the handles (center-drag zone).
    Center,
}

// ============================================================================
// Value Type
// ============================================================================

#[derive(Clone, Copy, PartialEq, Debug)]
pub struct RangeValue {
    pub low: f32,
    pub high: f32,
}

impl RangeValue {
    pub fn new(low: f32, high: f32) -> Self {
        debug_assert!(
            low <= high,
            "RangeValue: low ({}) must be <= high ({})",
            low,
            high
        );
        Self { low, high }
    }
}

// ============================================================================
// Widget
// ============================================================================

pub struct RangeSlider {
    id: WidgetInternalId,
    domain: SliderDomain,
    value: RangeValue,
    disabled: bool,
    focused: bool,
    /// Which handle has keyboard focus (when widget is focused).
    focused_handle: RangeHandle,
    /// Current drag state (None = no drag).
    drag_mode: Option<DragMode>,
    /// Callback fired continuously during drag.
    on_change: Option<Box<dyn FnMut(RangeValue)>>,
    /// Optional label text.
    label: Option<String>,
    /// Anchor snap points (domain values). Handles stick when dragged within threshold.
    anchors: Vec<f32>,
    /// Snap threshold in logical pixels (default: 6.0).
    snap_threshold: f32,
    /// Display style (Track = classic thin, Bar = filled rectangle).
    display_style: TrackDisplayStyle,
    /// Current bar hover zone (only used when display_style == Bar).
    bar_hover_zone: BarHoverZone,
    /// Animation [0.0, 1.0] for low edge handle hover.
    low_edge_hover_t: Cell<f32>,
    low_edge_hover_anim: Cell<Option<TransitionHandle>>,
    /// Animation [0.0, 1.0] for high edge handle hover.
    high_edge_hover_t: Cell<f32>,
    high_edge_hover_anim: Cell<Option<TransitionHandle>>,
    /// Animation [0.0, 1.0] for center zone hover.
    center_hover_t: Cell<f32>,
    center_hover_anim: Cell<Option<TransitionHandle>>,

    // Cached layout geometry
    layout_bounds: Size,
    cached_track_length: f32,
    aligned_parts: Option<ResolvedControlParts>,
    cached_thumb_diameter: f32,
    cached_handle_hit_width: f32,

    // Animation state
    hover_t: Cell<f32>,
    hover_anim: Cell<Option<TransitionHandle>>,
    drag_anim: Cell<Option<TransitionHandle>>,
    low_render_t: Cell<f32>,
    high_render_t: Cell<f32>,
}

impl RangeSlider {
    pub fn new(min: f32, max: f32, low: f32, high: f32) -> Self {
        let domain = SliderDomain {
            min,
            max,
            step: None,
        };
        let clamped_low = domain.constrain(low);
        let clamped_high = domain.constrain(high).max(clamped_low);
        Self {
            id: WidgetInternalId::new(),
            domain,
            value: RangeValue::new(clamped_low, clamped_high),
            disabled: false,
            focused: false,
            focused_handle: RangeHandle::Low,
            drag_mode: None,
            on_change: None,
            label: None,
            anchors: Vec::new(),
            snap_threshold: 6.0,
            display_style: TrackDisplayStyle::Track,
            bar_hover_zone: BarHoverZone::None,
            low_edge_hover_t: Cell::new(0.0),
            low_edge_hover_anim: Cell::new(None),
            high_edge_hover_t: Cell::new(0.0),
            high_edge_hover_anim: Cell::new(None),
            center_hover_t: Cell::new(0.0),
            center_hover_anim: Cell::new(None),
            layout_bounds: Size::zero(),
            cached_track_length: 0.0,
            aligned_parts: None,
            cached_thumb_diameter: 0.0,
            cached_handle_hit_width: 0.0,
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            drag_anim: Cell::new(None),
            low_render_t: Cell::new(0.0),
            high_render_t: Cell::new(0.0),
        }
    }

    pub fn with_step(mut self, step: f32) -> Self {
        self.domain.step = Some(step);
        self.value.low = self.domain.constrain(self.value.low);
        self.value.high = self.domain.constrain(self.value.high);
        self
    }

    pub fn with_label(mut self, label: &str) -> Self {
        self.label = Some(label.to_string());
        self
    }

    pub fn with_anchors(mut self, anchors: Vec<f32>) -> Self {
        self.anchors = anchors;
        self
    }

    pub fn with_snap_threshold(mut self, px: f32) -> Self {
        self.snap_threshold = px;
        self
    }

    pub fn with_display_style(mut self, style: TrackDisplayStyle) -> Self {
        self.display_style = style;
        self
    }

    pub fn on_change(mut self, f: impl FnMut(RangeValue) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    pub fn set_disabled(&mut self, disabled: bool) {
        self.disabled = disabled;
    }

    pub fn value(&self) -> RangeValue {
        self.value
    }

    pub fn set_value(&mut self, low: f32, high: f32) {
        let l = self.domain.constrain(low);
        let h = self.domain.constrain(high).max(l);
        self.value = RangeValue::new(l, h);
    }

    fn emit_change(&mut self) {
        if let Some(ref mut cb) = self.on_change {
            cb(self.value);
        }
    }

    fn x_to_value(&self, local_x: f32) -> f32 {
        let thumb_radius = self.cached_thumb_diameter * 0.5;
        let usable = self.cached_track_length - self.cached_thumb_diameter;
        let t = if usable > 0.0 {
            ((local_x - thumb_radius) / usable).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.domain.denormalize(t)
    }

    fn x_to_value_bar(&self, local_x: f32) -> f32 {
        let t = if self.cached_track_length > 0.0 {
            (local_x / self.cached_track_length).clamp(0.0, 1.0)
        } else {
            0.0
        };
        self.domain.denormalize(t)
    }

    fn nearest_handle(&self, local_x: f32) -> RangeHandle {
        let thumb_r = self.cached_thumb_diameter * 0.5;
        let usable = (self.cached_track_length - self.cached_thumb_diameter).max(0.0);
        let low_t = self.domain.normalize(self.value.low);
        let high_t = self.domain.normalize(self.value.high);
        let low_cx = thumb_r + low_t * usable;
        let high_cx = thumb_r + high_t * usable;
        let dist_low = (local_x - low_cx).abs();
        let dist_high = (local_x - high_cx).abs();
        if dist_low <= dist_high {
            RangeHandle::Low
        } else {
            RangeHandle::High
        }
    }

    fn set_handle_value(&mut self, handle: RangeHandle, raw: f32) {
        let constrained = self.domain.constrain(raw);
        match handle {
            RangeHandle::Low => {
                let new_low = constrained.min(self.value.high);
                if (new_low - self.value.low).abs() > f32::EPSILON {
                    self.value.low = new_low;
                    self.emit_change();
                }
            }
            RangeHandle::High => {
                let new_high = constrained.max(self.value.low);
                if (new_high - self.value.high).abs() > f32::EPSILON {
                    self.value.high = new_high;
                    self.emit_change();
                }
            }
        }
    }

    fn adjust_handle(&mut self, handle: RangeHandle, delta: f32) {
        match handle {
            RangeHandle::Low => {
                let raw = self.value.low + delta;
                self.set_handle_value(RangeHandle::Low, raw);
            }
            RangeHandle::High => {
                let raw = self.value.high + delta;
                self.set_handle_value(RangeHandle::High, raw);
            }
        }
    }

    /// If the cursor is within snap_threshold pixels of an anchor's track position,
    /// return the anchor value. Otherwise return the raw value unchanged.
    fn snap_to_anchor(&self, raw_value: f32, local_x: f32) -> f32 {
        if self.anchors.is_empty() {
            return raw_value;
        }
        let thumb_r = self.cached_thumb_diameter * 0.5;
        let usable = self.cached_track_length - self.cached_thumb_diameter;
        if usable <= 0.0 {
            return raw_value;
        }
        for &anchor in &self.anchors {
            if anchor < self.domain.min || anchor > self.domain.max {
                continue;
            }
            let anchor_t = self.domain.normalize(anchor);
            let anchor_px = thumb_r + anchor_t * usable;
            if (local_x - anchor_px).abs() <= self.snap_threshold {
                return anchor;
            }
        }
        raw_value
    }

    /// Bar-style snap: no thumb offset in pixel mapping.
    fn snap_to_anchor_bar(&self, raw_value: f32, local_x: f32) -> f32 {
        if self.anchors.is_empty() {
            return raw_value;
        }
        let track_length = self.cached_track_length;
        if track_length <= 0.0 {
            return raw_value;
        }
        for &anchor in &self.anchors {
            if anchor < self.domain.min || anchor > self.domain.max {
                continue;
            }
            let anchor_t = self.domain.normalize(anchor);
            let anchor_px = anchor_t * track_length;
            if (local_x - anchor_px).abs() <= self.snap_threshold {
                return anchor;
            }
        }
        raw_value
    }

    fn hit_test_bar(&self, local_x: f32) -> Option<DragMode> {
        let track_length = self.cached_track_length;
        let handle_hit_w = self.cached_handle_hit_width;
        let low_t = self.domain.normalize(self.value.low);
        let high_t = self.domain.normalize(self.value.high);
        let low_px = low_t * track_length;
        let high_px = high_t * track_length;

        // Check edge handles first (priority over center)
        if (local_x - low_px).abs() <= handle_hit_w * 0.5 {
            return Some(DragMode::Handle(RangeHandle::Low));
        }
        if (local_x - high_px).abs() <= handle_hit_w * 0.5 {
            return Some(DragMode::Handle(RangeHandle::High));
        }

        // Check center zone (between handles)
        if local_x > low_px && local_x < high_px {
            let grab_value = self.x_to_value_bar(local_x);
            let offset = grab_value - self.value.low;
            return Some(DragMode::CenterSlide {
                grab_offset: offset,
            });
        }

        // Outside range — move nearest handle
        if local_x <= low_px {
            return Some(DragMode::Handle(RangeHandle::Low));
        }
        if local_x >= high_px {
            return Some(DragMode::Handle(RangeHandle::High));
        }

        None
    }

    /// Determine which bar zone the cursor is in, using cached geometry.
    fn compute_bar_zone(&self, local_x: f32) -> BarHoverZone {
        let track_length = self.cached_track_length;
        let half_hit = self.cached_handle_hit_width * 0.5;
        let low_t = self.domain.normalize(self.value.low);
        let high_t = self.domain.normalize(self.value.high);
        let low_px = low_t * track_length;
        let high_px = high_t * track_length;

        // Edge handles take priority
        if (local_x - low_px).abs() <= half_hit {
            return BarHoverZone::LowEdge;
        }
        if (local_x - high_px).abs() <= half_hit {
            return BarHoverZone::HighEdge;
        }
        // Center zone
        if local_x > low_px && local_x < high_px {
            return BarHoverZone::Center;
        }
        BarHoverZone::None
    }

    fn animate_zone_transition(&self, cx: &mut EventContext, old: BarHoverZone, new: BarHoverZone) {
        let duration = 0.07;
        let easing = Easing::EaseOut;

        // Low edge
        if (old == BarHoverZone::LowEdge) != (new == BarHoverZone::LowEdge) {
            let target = if new == BarHoverZone::LowEdge {
                1.0
            } else {
                0.0
            };
            let anim = Animation {
                from: self.low_edge_hover_t.get(),
                to: target,
                duration,
                easing,
            };
            if let Some(h) = cx.start_animation(anim) {
                self.low_edge_hover_anim.set(Some(h));
            }
        }
        // High edge
        if (old == BarHoverZone::HighEdge) != (new == BarHoverZone::HighEdge) {
            let target = if new == BarHoverZone::HighEdge {
                1.0
            } else {
                0.0
            };
            let anim = Animation {
                from: self.high_edge_hover_t.get(),
                to: target,
                duration,
                easing,
            };
            if let Some(h) = cx.start_animation(anim) {
                self.high_edge_hover_anim.set(Some(h));
            }
        }
        // Center
        if (old == BarHoverZone::Center) != (new == BarHoverZone::Center) {
            let target = if new == BarHoverZone::Center {
                1.0
            } else {
                0.0
            };
            let anim = Animation {
                from: self.center_hover_t.get(),
                to: target,
                duration,
                easing,
            };
            if let Some(h) = cx.start_animation(anim) {
                self.center_hover_anim.set(Some(h));
            }
        }
    }

    fn handle_is_active(&self, cx: &PaintContext, handle: RangeHandle) -> bool {
        !self.disabled
            && match self.drag_mode {
                Some(DragMode::Handle(active)) => active == handle,
                Some(DragMode::CenterSlide { .. }) => true,
                None => cx.has_capture() && handle == self.focused_handle,
            }
    }

    fn paint_focus_cue(&self, cx: &mut PaintContext, target: RectF, bounds: RectF, radius: f32) {
        if cx.has_capture() || self.drag_mode.is_some() {
            return;
        }
        let chrome = cx.theme.control_chrome(
            ControlChromeCategory::TrackRange,
            ControlChromeState::Focused,
            ControlValidationState::Neutral,
        );
        let width = chrome.focus_width;
        let Some(cue_rect) = target.expand(width).intersection(&bounds) else {
            return;
        };
        let cue_radius = (radius + width).min(cue_rect.width.min(cue_rect.height) * 0.5);
        cx.scene.push_border_rounded_rect(
            cue_rect,
            chrome.focus_color,
            width,
            RoundedCorners::uniform(cue_radius),
            crate::core::paint::LAYER_FOCUS,
            None,
        );
    }

    fn paint_track_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.slider;
        let (accent, accent_hover) = theme.track_accent_colors(cx.has_capture());
        let (rest_accent, rest_hover) = theme.track_accent_colors(false);

        let track_x = if self.aligned_parts.is_some() {
            crate::core::math::snap_to_physical(rect.x, cx.scale_factor)
        } else {
            rect.x
        };
        let track_length = if self.aligned_parts.is_some() {
            crate::core::math::snap_to_physical(rect.x + self.cached_track_length, cx.scale_factor)
                - track_x
        } else {
            self.cached_track_length
        };
        if track_length <= 0.0
            || (self.aligned_parts.is_some()
                && self.cached_track_length <= self.cached_thumb_diameter)
        {
            self.paint_label(cx, rect, 0.0);
            return;
        }
        let track_thickness = geom.track_thickness;
        let thumb_diameter = geom.thumb_diameter;
        let track_radius = track_thickness * 0.5;

        // Track centered vertically
        let track_y = rect.y + (rect.height - track_thickness) * 0.5;
        let track_rect = RectF::new(track_x, track_y, track_length, track_thickness);

        // Coordinate track emphasis with the current handle focus stroke.
        let focus_emphasis = if cx.has_focus_cue() { 1.0 } else { 0.0 };

        paint_track_well(cx, track_rect, track_radius, 1);

        // Anchor tick marks (below fill and thumbs)
        let usable = (track_length - thumb_diameter).max(0.0);
        let anchor_color = theme.colors.borders.strong;
        for &anchor in &self.anchors {
            if anchor < self.domain.min || anchor > self.domain.max {
                continue;
            }
            let t = self.domain.normalize(anchor);
            let tick_cx = track_x + thumb_diameter * 0.5 + t * usable;
            let tick_w = 1.0;
            let tick_h = track_thickness + 4.0;
            let tick_y = track_y - 2.0;
            let tick_rect = RectF::new(tick_cx - tick_w * 0.5, tick_y, tick_w, tick_h);
            cx.scene.push_rect(tick_rect, anchor_color, 3, None);
        }

        // Range fill between the two handles
        let low_t = self.low_render_t.get();
        let high_t = self.high_render_t.get();
        let low_cx = track_x + thumb_diameter * 0.5 + low_t * usable;
        let high_cx = track_x + thumb_diameter * 0.5 + high_t * usable;
        let fill_width = high_cx - low_cx;
        if fill_width > 0.0 {
            let fill_rect = RectF::new(low_cx, track_y, fill_width, track_thickness);
            let fill_color = if self.disabled {
                theme.colors.borders.default
            } else {
                let blend = self.hover_t.get().max(focus_emphasis);
                accent.lerp(accent_hover, blend)
            };
            paint_track_fill(cx, fill_rect, track_radius, fill_color, self.disabled, 4);
        }

        // Determine which handle is active/focused for z-ordering
        let active_or_focused = match self.drag_mode {
            Some(DragMode::Handle(h)) => h,
            _ => self.focused_handle,
        };
        let enlarge_thumbs = !self.disabled && (cx.has_capture() || self.drag_mode.is_some());

        // Paint both thumbs
        for &handle in &[RangeHandle::Low, RangeHandle::High] {
            let t = match handle {
                RangeHandle::Low => low_t,
                RangeHandle::High => high_t,
            };
            let thumb_center_x = track_x + thumb_diameter * 0.5 + t * usable;
            let thumb_x = thumb_center_x - thumb_diameter * 0.5;
            let thumb_y = rect.y + (rect.height - thumb_diameter) * 0.5;
            let thumb_rect = RectF::new(thumb_x, thumb_y, thumb_diameter, thumb_diameter);
            let painted_thumb = if enlarge_thumbs {
                thumb_rect.expand(thumb_diameter * 0.125)
            } else {
                thumb_rect
            };
            // Keyboard Capture belongs to the whole range, including both endpoints.
            let thumb_color = if self.disabled {
                theme.colors.typography.muted
            } else if cx.has_capture() {
                accent.lerp(accent_hover, self.hover_t.get())
            } else {
                rest_accent.lerp(rest_hover, self.hover_t.get())
            };

            // Keep both material layers of the active handle above the other.
            let z = if handle == active_or_focused { 8 } else { 5 };
            let pressed = matches!(self.drag_mode, Some(DragMode::CenterSlide { .. }))
                || matches!(self.drag_mode, Some(DragMode::Handle(h)) if h == handle);
            paint_track_thumb(cx, painted_thumb, thumb_color, pressed, self.disabled, z);

            if cx.has_focus_cue() && !self.disabled && handle == active_or_focused {
                let ring_radius = theme.chrome_radius(
                    ControlChromeCategory::TrackRange,
                    painted_thumb.height * 0.5,
                    true,
                );
                self.paint_focus_cue(cx, thumb_rect, rect, ring_radius);
            }
        }

        // Paint label text after track
        self.paint_label(cx, rect, track_length);
    }

    fn paint_bar_style(&self, cx: &mut PaintContext, rect: RectF) {
        let theme = cx.theme;
        let geom = &theme.geometry.track_bar;
        let (accent, accent_hover) = theme.track_accent_colors(cx.has_capture());
        let track_x = if self.aligned_parts.is_some() {
            crate::core::math::snap_to_physical(rect.x, cx.scale_factor)
        } else {
            rect.x
        };
        let track_length = if self.aligned_parts.is_some() {
            crate::core::math::snap_to_physical(rect.x + self.cached_track_length, cx.scale_factor)
                - track_x
        } else {
            self.cached_track_length
        };
        if track_length <= 0.0 || (self.aligned_parts.is_some() && self.cached_track_length <= 0.0)
        {
            self.paint_label(cx, rect, 0.0);
            return;
        }
        let bar_height = geom.bar_height;
        // Bar background (full width, centered vertically)
        let bar_y = rect.y + (rect.height - bar_height) * 0.5;
        let bar_rect = RectF::new(track_x, bar_y, track_length, bar_height);

        // Coordinate bar emphasis with the current handle focus stroke.
        let focus_emphasis = if cx.has_focus_cue() { 1.0 } else { 0.0 };

        // Background + border
        cx.scene
            .push_rect(bar_rect, theme.colors.surfaces.content, 1, None);
        cx.scene
            .push_border_rect(bar_rect, theme.colors.borders.default, 1.0, 2, None);

        // Anchor tick marks
        let anchor_color = theme.colors.borders.strong;
        for &anchor in &self.anchors {
            if anchor < self.domain.min || anchor > self.domain.max {
                continue;
            }
            let t = self.domain.normalize(anchor);
            let tick_cx = track_x + t * track_length;
            let tick_w = 1.0;
            let tick_h = bar_height + 4.0;
            let tick_y = bar_y - 2.0;
            let tick_rect = RectF::new(tick_cx - tick_w * 0.5, tick_y, tick_w, tick_h);
            cx.scene.push_rect(tick_rect, anchor_color, 4, None);
        }

        // Range fill (low -> high)
        let low_t = self.low_render_t.get();
        let high_t = self.high_render_t.get();
        let low_x = track_x + low_t * track_length;
        let high_x = track_x + high_t * track_length;
        let fill_width = high_x - low_x;
        if fill_width > 0.0 {
            let fill_rect = RectF::new(low_x, bar_y, fill_width, bar_height);
            // Keep existing center-hover/navigation emphasis within the palette
            // already qualified for this owner's keyboard Capture.
            let ct = self.center_hover_t.get();
            let fill_color = if self.disabled {
                theme.colors.borders.default
            } else {
                let blend = (ct * 0.4).max(focus_emphasis);
                accent.lerp(accent_hover, blend)
            };
            paint_track_fill(cx, fill_rect, 0.0, fill_color, self.disabled, 3);
        }

        // Edge handle tabs — simple vertical bars extending above/below the bar
        let handle_w = 4.0;
        let handle_overhang = 3.0; // extends this many px above and below the bar
        let lt = self.low_edge_hover_t.get();
        let ht = self.high_edge_hover_t.get();

        // Handle colors: dark contrasting color, brighter on hover
        let rest_handle = theme.colors.borders.strong;
        let hot_handle = theme.colors.typography.primary;
        let active_or_focused = match self.drag_mode {
            Some(DragMode::Handle(handle)) => handle,
            _ => self.focused_handle,
        };

        for (handle, handle_x, anim_t) in [
            (RangeHandle::Low, low_x, lt),
            (RangeHandle::High, high_x, ht),
        ] {
            let handle_color = if self.disabled {
                theme.colors.typography.muted
            } else if cx.has_capture() {
                accent
            } else {
                rest_handle.lerp(hot_handle, anim_t)
            };
            let handle_rect = RectF::new(
                handle_x - handle_w * 0.5,
                bar_y - handle_overhang,
                handle_w,
                bar_height + handle_overhang * 2.0,
            );
            let painted_handle = if self.handle_is_active(cx, handle) {
                handle_rect.expand(handle_w * 0.25)
            } else {
                handle_rect
            };
            cx.scene.push_rect(painted_handle, handle_color, 4, None);
        }

        if cx.has_focus_cue() && !self.disabled {
            let focus_x = match active_or_focused {
                RangeHandle::Low => low_x,
                RangeHandle::High => high_x,
            };
            let handle_rect = RectF::new(
                focus_x - handle_w * 0.5,
                bar_y - handle_overhang,
                handle_w,
                bar_height + handle_overhang * 2.0,
            );
            let ring_radius =
                theme.chrome_radius(ControlChromeCategory::TrackRange, geom.radius, false);
            self.paint_focus_cue(cx, handle_rect, rect, ring_radius);
        }

        // Paint label text after track
        self.paint_label(cx, rect, track_length);
    }

    fn paint_label(&self, cx: &mut PaintContext, rect: RectF, track_length: f32) {
        if let Some(label_text) = &self.label {
            let theme = cx.theme;
            let geom = &theme.geometry.slider;
            let text_x = self
                .aligned_parts
                .map_or(rect.x + track_length + geom.label_gap, |p| {
                    rect.x - p.body.x + p.label.unwrap().x
                });
            let font_size = theme.typography.font_size;
            let run = cx.text_engine.shape(label_text, font_size);
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = rect.y
                + compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    theme.typography.baseline_offset,
                );
            let label_y = self
                .aligned_parts
                .map_or(baseline_y - metrics.ascent, |p| rect.y + p.label.unwrap().y);
            let text_color = if self.disabled {
                theme.colors.typography.muted
            } else {
                theme.colors.typography.primary
            };
            cx.text_engine.draw(
                &run,
                text_x,
                label_y,
                text_color,
                LAYER_CONTENT + 2,
                cx.scene,
            );
        }
    }
}

impl Default for RangeSlider {
    fn default() -> Self {
        Self::new(0.0, 100.0, 25.0, 75.0)
    }
}

impl WidgetDowncast for RangeSlider {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for RangeSlider {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.aligned_parts = None;
        let geom_slider = &cx.theme.geometry.slider;
        let height = match self.display_style {
            TrackDisplayStyle::Track | TrackDisplayStyle::Radial => {
                geom_slider.thumb_diameter.max(geom_slider.track_thickness)
            }
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                cx.theme.geometry.track_bar.bar_height
            }
        };
        let track_width = geom_slider.track_length;

        self.cached_thumb_diameter = geom_slider.thumb_diameter;
        self.cached_handle_hit_width = cx.theme.geometry.track_bar.handle_hit_width;

        let mut total_width = track_width;
        let mut height = height;

        if self.label.is_some() {
            let font_size = cx.theme.typography.font_size;
            let metrics = cx.text_engine.font_metrics(font_size);
            let label_height = metrics.ascent + metrics.descent;
            height = height.max(label_height);
        }

        if let Some(label_text) = &self.label {
            let font_size = cx.theme.typography.font_size;
            let (text_width, _) = cx.text_engine.measure(label_text, font_size);
            total_width += geom_slider.label_gap + text_width;
        }

        let label_requirement = self.label.as_ref().map(|text| ControlPartRequirement {
            width: cx
                .text_engine
                .measure(text, cx.theme.typography.font_size)
                .0,
            gap: geom_slider.label_gap,
        });
        let requirements = ControlPartsRequirements {
            leading: ControlSideRequirements::default(),
            trailing: ControlSideRequirements {
                label: label_requirement,
                value: None,
            },
            body_width: track_width,
            height,
        };
        match cx.control_parts_request() {
            Some(ControlPartsRequest::Measure) => cx.report_control_parts(requirements),
            Some(ControlPartsRequest::Arrange(columns)) => {
                assert_ne!(
                    self.display_style,
                    TrackDisplayStyle::Radial,
                    "radial alignment is not supported"
                );
                let size = constraints.constrain(Size::new(total_width, height));
                let mut parts = columns.resolve(requirements, size.height);
                if let Some(label) = parts.label.as_mut() {
                    let metrics = cx.text_engine.font_metrics(cx.theme.typography.font_size);
                    label.y = compute_control_text_baseline_y(
                        size.height,
                        metrics.ascent,
                        metrics.descent,
                        cx.theme.typography.baseline_offset,
                    ) - metrics.ascent;
                    label.height = metrics.ascent + metrics.descent;
                }
                self.cached_track_length = parts.body.width;
                self.aligned_parts = Some(parts);
                self.layout_bounds = size;
                return LayoutResult::new(
                    size,
                    Size::new(
                        columns.content_width().max(size.width),
                        height.max(size.height),
                    ),
                );
            }
            None => {}
        }

        let width = total_width.clamp(constraints.min_width, constraints.max_width);
        self.cached_track_length = (width - (total_width - track_width)).max(0.0);
        let height = height.clamp(constraints.min_height, constraints.max_height);
        self.layout_bounds = Size::new(width, height);
        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Consume animations
        {
            let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
            if let Some(handle) = self.hover_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.hover_t.set(value);
                } else {
                    self.hover_anim.set(None);
                }
            }
            // Consume bar hover zone animations
            if let Some(h) = self.low_edge_hover_anim.get() {
                if let Some(v) = cx.get_animation::<f32>(h) {
                    self.low_edge_hover_t.set(v);
                } else {
                    self.low_edge_hover_anim.set(None);
                }
            }
            if let Some(h) = self.high_edge_hover_anim.get() {
                if let Some(v) = cx.get_animation::<f32>(h) {
                    self.high_edge_hover_t.set(v);
                } else {
                    self.high_edge_hover_anim.set(None);
                }
            }
            if let Some(h) = self.center_hover_anim.get() {
                if let Some(v) = cx.get_animation::<f32>(h) {
                    self.center_hover_t.set(v);
                } else {
                    self.center_hover_anim.set(None);
                }
            }
            // Update render positions from current values
            self.low_render_t.set(self.domain.normalize(self.value.low));
            self.high_render_t
                .set(self.domain.normalize(self.value.high));
        }

        let rect = self.aligned_parts.map_or(rect, |p| {
            RectF::new(rect.x + p.body.x, rect.y, p.body.width, rect.height)
        });
        match self.display_style {
            TrackDisplayStyle::Track | TrackDisplayStyle::Radial => {
                self.paint_track_style(cx, rect)
            }
            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => self.paint_bar_style(cx, rect),
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        if self.disabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                let pointer_travel = match self.display_style {
                    TrackDisplayStyle::Track | TrackDisplayStyle::Radial => {
                        self.cached_track_length - self.cached_thumb_diameter
                    }
                    TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => self.cached_track_length,
                };
                if pointer_travel <= 0.0 && pe.kind != PointerEventKind::Up {
                    return false;
                }
                let (mut local_x, local_y) = pe.local_pos;
                local_x -= self.aligned_parts.map_or(0.0, |p| p.body.x);
                let inside = local_x >= 0.0
                    && local_x
                        <= self
                            .aligned_parts
                            .map_or(self.layout_bounds.width, |p| p.body.width)
                    && local_y >= 0.0
                    && local_y <= self.layout_bounds.height;

                match pe.kind {
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if inside && local_x <= self.cached_track_length {
                            let mode = match self.display_style {
                                TrackDisplayStyle::Track | TrackDisplayStyle::Radial => {
                                    let handle = self.nearest_handle(local_x);
                                    let raw = self.x_to_value(local_x);
                                    let snapped = self.snap_to_anchor(raw, local_x);
                                    self.set_handle_value(handle, snapped);
                                    DragMode::Handle(handle)
                                }
                                TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                                    let mode = self
                                        .hit_test_bar(local_x)
                                        .unwrap_or(DragMode::Handle(RangeHandle::Low));
                                    // For edge handle hits, move the handle to the click position
                                    if let DragMode::Handle(handle) = mode {
                                        let raw = self.x_to_value_bar(local_x);
                                        let snapped = self.snap_to_anchor_bar(raw, local_x);
                                        self.set_handle_value(handle, snapped);
                                    }
                                    // CenterSlide: don't move anything on down, just record offset
                                    mode
                                }
                            };
                            self.drag_mode = Some(mode);
                            cx.capture_pointer(PointerButton::Primary);
                            // Keep frame loop alive during drag
                            let keep_alive = Animation {
                                from: 0.0,
                                to: 1.0,
                                duration: 3600.0,
                                easing: Easing::Linear,
                            };
                            if let Some(handle) = cx.start_animation(keep_alive) {
                                self.drag_anim.set(Some(handle));
                            }
                            true
                        } else {
                            false
                        }
                    }
                    PointerEventKind::Move => {
                        if let Some(mode) = self.drag_mode {
                            match mode {
                                DragMode::Handle(handle) => {
                                    match self.display_style {
                                        TrackDisplayStyle::Track | TrackDisplayStyle::Radial => {
                                            let raw = self.x_to_value(local_x);
                                            let snapped = self.snap_to_anchor(raw, local_x);
                                            self.set_handle_value(handle, snapped);
                                        }
                                        TrackDisplayStyle::Bar | TrackDisplayStyle::Pill => {
                                            let raw = self.x_to_value_bar(local_x);
                                            let snapped = self.snap_to_anchor_bar(raw, local_x);
                                            self.set_handle_value(handle, snapped);
                                        }
                                    }
                                    true
                                }
                                DragMode::CenterSlide { grab_offset } => {
                                    let cursor_value = self.x_to_value_bar(local_x);
                                    let new_low = cursor_value - grab_offset;
                                    let gap = self.value.high - self.value.low;
                                    let clamped_low =
                                        new_low.clamp(self.domain.min, self.domain.max - gap);
                                    let clamped_high = clamped_low + gap;
                                    if (clamped_low - self.value.low).abs() > f32::EPSILON {
                                        self.value.low = self.domain.constrain(clamped_low);
                                        self.value.high = self.domain.constrain(clamped_high);
                                        self.emit_change();
                                    }
                                    true
                                }
                            }
                        } else if matches!(
                            self.display_style,
                            TrackDisplayStyle::Bar | TrackDisplayStyle::Pill
                        ) && inside
                        {
                            // Update hover zone for visual feedback
                            let old_zone = self.bar_hover_zone;
                            let new_zone = self.compute_bar_zone(local_x);
                            if new_zone != old_zone {
                                self.animate_zone_transition(cx, old_zone, new_zone);
                                self.bar_hover_zone = new_zone;
                                true
                            } else {
                                false
                            }
                        } else {
                            if self.bar_hover_zone != BarHoverZone::None {
                                let old = self.bar_hover_zone;
                                self.bar_hover_zone = BarHoverZone::None;
                                self.animate_zone_transition(cx, old, BarHoverZone::None);
                                true
                            } else {
                                false
                            }
                        }
                    }
                    PointerEventKind::Up => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.drag_mode.is_some() {
                            self.drag_mode = None;
                            self.drag_anim.set(None);
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down && self.focused => {
                // SPEC-HAL-328 INV-FC-1: Category C arrows only respond in Capture mode.
                if cx.keyboard_mode != KeyboardAuthorityMode::Capture {
                    return false;
                }
                match ke.key_code {
                    KeyCode::Right | KeyCode::Up => {
                        let handle = if ke.modifiers.shift {
                            RangeHandle::High
                        } else {
                            RangeHandle::Low
                        };
                        self.adjust_handle(handle, self.domain.small_delta());
                        self.focused_handle = handle;
                        cx.request_continuous_frames();
                        true
                    }
                    KeyCode::Left | KeyCode::Down => {
                        let handle = if ke.modifiers.shift {
                            RangeHandle::High
                        } else {
                            RangeHandle::Low
                        };
                        self.adjust_handle(handle, -self.domain.small_delta());
                        self.focused_handle = handle;
                        cx.request_continuous_frames();
                        true
                    }
                    KeyCode::Tab => {
                        // Toggle focused handle between Low and High
                        self.focused_handle = if ke.modifiers.shift {
                            RangeHandle::Low
                        } else {
                            RangeHandle::High
                        };
                        cx.request_continuous_frames();
                        true
                    }
                    KeyCode::Home => {
                        match self.focused_handle {
                            RangeHandle::Low => {
                                self.set_handle_value(RangeHandle::Low, self.domain.min)
                            }
                            RangeHandle::High => {
                                self.set_handle_value(RangeHandle::High, self.value.low)
                            }
                        }
                        cx.request_continuous_frames();
                        true
                    }
                    KeyCode::End => {
                        match self.focused_handle {
                            RangeHandle::Low => {
                                self.set_handle_value(RangeHandle::Low, self.value.high)
                            }
                            RangeHandle::High => {
                                self.set_handle_value(RangeHandle::High, self.domain.max)
                            }
                        }
                        cx.request_continuous_frames();
                        true
                    }
                    _ => false,
                }
            }
            // SPEC-HAL-210 INV-B3 + SPEC-HAL-176: Plain h/l in Capture mode
            // adjusts the Low handle by small_delta; Shift+h / Shift+l
            // adjusts the High handle. Mirrors the arrow keys' shift-modifier
            // handle selection above. h/l arrive as Event::Text (not KeyDown)
            // for unmodified letter keys; Shift produces "H" / "L". Handle
            // selection is read from te.modifiers.shift (authoritative even
            // under CapsLock).
            Event::Text(te)
                if self.focused
                    && cx.keyboard_mode == KeyboardAuthorityMode::Capture
                    && matches!(te.text.as_str(), "h" | "H" | "l" | "L") =>
            {
                let delta = if te.text.eq_ignore_ascii_case("l") {
                    self.domain.small_delta()
                } else {
                    -self.domain.small_delta()
                };
                let handle = if te.modifiers.shift {
                    RangeHandle::High
                } else {
                    RangeHandle::Low
                };
                self.adjust_handle(handle, delta);
                self.focused_handle = handle;
                cx.request_continuous_frames();
                true
            }
            _ => false,
        }
    }

    fn focusable(&self) -> bool {
        !self.disabled
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
        self.focused_handle = RangeHandle::Low;
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        self.drag_mode = None;
    }

    fn accepts_keyboard_activation(&self) -> bool {
        false
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.disabled {
            return;
        }
        let anim = Animation {
            from: self.hover_t.get(),
            to: 1.0,
            duration: 0.08,
            easing: Easing::EaseOut,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(1.0);
            self.hover_anim.set(None);
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        let old = self.bar_hover_zone;
        self.bar_hover_zone = BarHoverZone::None;
        if old != BarHoverZone::None {
            self.animate_zone_transition(cx, old, BarHoverZone::None);
        }
        let anim = Animation {
            from: self.hover_t.get(),
            to: 0.0,
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(0.0);
            self.hover_anim.set(None);
        }
    }

    fn on_mount(&mut self) {
        self.hover_t.set(0.0);
        self.hover_anim.set(None);
    }

    fn debug_type_name(&self) -> &'static str {
        "RangeSlider"
    }

    fn capture_eligible(&self) -> bool {
        !self.disabled
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        // Capture state observed via PaintContext::has_capture(); no local mirror.
    }

    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        // Capture state observed via PaintContext::has_capture(); no local mirror.
    }
}
