// SPEC-HAL-351: Section Surface Family
//
// These surfaces intentionally do not extend SectionHeader. Label, band,
// metadata, and disclosure own different authority boundaries.

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{Event, EventPhase, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{Color, LAYER_CONTENT, RoundedCorners};
use crate::core::widget::{
    EventContext, FocusIndicatorGeometry, LayoutContext, PaintContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;
use crate::widgets::disclosure_indicator::paint_disclosure_indicator;

// INV-351-GEOM-1: Section surfaces are dense readout chrome. Until dedicated
// theme tokens exist, these metrics mirror the existing readout/header family:
// 24px bands, 8px horizontal text inset, and 1px structural hairlines.
const BAND_HEIGHT: f32 = 24.0;
const DISCLOSURE_HEIGHT: f32 = 24.0;
const H_PADDING: f32 = 8.0;
const TEXT_TOP: f32 = 5.0;
const META_GAP: f32 = 8.0;
const DISCLOSURE_CONTROL_WIDTH: f32 = 20.0;
const DISCLOSURE_BUTTON_SIZE: f32 = 16.0;
const DISCLOSURE_ICON_BOX_SIZE: f32 = 12.0;
const DISCLOSURE_CONTROL_GAP: f32 = 4.0;
const DISCLOSURE_CONTROL_RADIUS: f32 = 3.0;
const HAIRLINE: f32 = 1.0;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionBandLevel {
    #[default]
    Section,
    SubSection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SectionBandMarker {
    #[default]
    None,
    Focus,
    Selection,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DisclosureHeaderState {
    Idle,
    Hovered,
    Pressed,
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct SectionSurfaceTypography {
    label_size: f32,
    meta_size: f32,
}

impl SectionSurfaceTypography {
    fn from_base_font_size(base: f32) -> Self {
        Self {
            label_size: (base - 2.0).max(10.0).round(),
            meta_size: (base - 2.0).max(10.0).round(),
        }
    }
}

impl Default for SectionSurfaceTypography {
    fn default() -> Self {
        Self::from_base_font_size(13.0)
    }
}

pub struct SectionBandHeader {
    id: WidgetInternalId,
    label: String,
    level: SectionBandLevel,
    marker: SectionBandMarker,
    typography: SectionSurfaceTypography,
}

impl SectionBandHeader {
    pub fn new(label: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.into(),
            level: SectionBandLevel::Section,
            marker: SectionBandMarker::None,
            typography: SectionSurfaceTypography::default(),
        }
    }

    pub fn with_level(mut self, level: SectionBandLevel) -> Self {
        self.level = level;
        self
    }

    pub fn with_marker(mut self, marker: SectionBandMarker) -> Self {
        self.marker = marker;
        self
    }

    pub fn level(&self) -> SectionBandLevel {
        self.level
    }

    pub fn marker(&self) -> SectionBandMarker {
        self.marker
    }

    #[cfg(test)]
    pub(crate) fn test_label(&self) -> &str {
        &self.label
    }
}

pub struct SectionMetaHeader {
    id: WidgetInternalId,
    label: String,
    meta: String,
    level: SectionBandLevel,
    marker: SectionBandMarker,
    typography: SectionSurfaceTypography,
}

impl SectionMetaHeader {
    pub fn new(label: impl Into<String>, meta: impl Into<String>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.into(),
            meta: meta.into(),
            level: SectionBandLevel::Section,
            marker: SectionBandMarker::None,
            typography: SectionSurfaceTypography::default(),
        }
    }

    pub fn with_level(mut self, level: SectionBandLevel) -> Self {
        self.level = level;
        self
    }

    pub fn with_marker(mut self, marker: SectionBandMarker) -> Self {
        self.marker = marker;
        self
    }

    pub fn level(&self) -> SectionBandLevel {
        self.level
    }

    pub fn marker(&self) -> SectionBandMarker {
        self.marker
    }

    pub fn meta(&self) -> &str {
        &self.meta
    }
}

pub struct DisclosureSectionHeader {
    id: WidgetInternalId,
    label: String,
    expanded: bool,
    marker: SectionBandMarker,
    on_toggle: Option<Box<dyn FnMut()>>,
    typography: SectionSurfaceTypography,
    state: DisclosureHeaderState,
    layout_bounds: Size,
    hover_t: Cell<f32>,
    hover_anim: Cell<Option<TransitionHandle>>,
    pressed_t: Cell<f32>,
    pressed_anim: Cell<Option<TransitionHandle>>,
    keyboard_pulse_active: Cell<bool>,
}

impl DisclosureSectionHeader {
    pub fn new(label: impl Into<String>, expanded: bool) -> Self {
        Self {
            id: WidgetInternalId::new(),
            label: label.into(),
            expanded,
            marker: SectionBandMarker::None,
            on_toggle: None,
            typography: SectionSurfaceTypography::default(),
            state: DisclosureHeaderState::Idle,
            layout_bounds: Size::zero(),
            hover_t: Cell::new(0.0),
            hover_anim: Cell::new(None),
            pressed_t: Cell::new(0.0),
            pressed_anim: Cell::new(None),
            keyboard_pulse_active: Cell::new(false),
        }
    }

    pub fn on_toggle<F: FnMut() + 'static>(mut self, f: F) -> Self {
        self.on_toggle = Some(Box::new(f));
        self
    }

    pub fn set_expanded(&mut self, expanded: bool) {
        self.expanded = expanded;
    }

    pub fn expanded(&self) -> bool {
        self.expanded
    }

    pub fn with_marker(mut self, marker: SectionBandMarker) -> Self {
        self.marker = marker;
        self
    }

    pub fn set_marker(&mut self, marker: SectionBandMarker) {
        self.marker = marker;
    }

    pub fn marker(&self) -> SectionBandMarker {
        self.marker
    }

    fn request_toggle(&mut self, cx: &mut EventContext) {
        if let Some(callback) = &mut self.on_toggle {
            callback();
            cx.invalidate_layout();
        }
    }

    fn pointer_inside(&self, local_pos: (f32, f32)) -> bool {
        let (local_x, local_y) = local_pos;
        local_x >= 0.0
            && local_x <= self.layout_bounds.width
            && local_y >= 0.0
            && local_y <= self.layout_bounds.height
    }

    fn animate_hover_to(&self, cx: &mut EventContext, to: f32, duration: f32, easing: Easing) {
        let anim = Animation {
            from: self.hover_t.get(),
            to,
            duration,
            easing,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.hover_anim.set(Some(handle));
        } else {
            self.hover_t.set(to);
            self.hover_anim.set(None);
        }
    }

    fn animate_pressed_to(&self, cx: &mut EventContext, to: f32, duration: f32, easing: Easing) {
        let anim = Animation {
            from: self.pressed_t.get(),
            to,
            duration,
            easing,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.pressed_anim.set(Some(handle));
        } else {
            self.pressed_t.set(to);
            self.pressed_anim.set(None);
        }
    }

    fn refresh_animation_cache(&self, cx: &mut PaintContext) {
        let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
        crate::core::rect_authority::assert_paint_mutation_authorized(
            "DisclosureSectionHeader::paint: hover_t/pressed_t",
        );

        if let Some(handle) = self.hover_anim.get() {
            if let Some(value) = cx.get_animation::<f32>(handle) {
                self.hover_t.set(value);
            } else {
                self.hover_anim.set(None);
            }
        }

        if let Some(handle) = self.pressed_anim.get() {
            if let Some(value) = cx.get_animation::<f32>(handle) {
                self.pressed_t.set(value);
            } else {
                self.pressed_anim.set(None);
            }
        }
    }
}

fn band_background(cx: &PaintContext, level: SectionBandLevel) -> Color {
    match level {
        SectionBandLevel::Section => cx.theme.colors.list.header_background,
        SectionBandLevel::SubSection => cx.theme.colors.list.subheader_background,
    }
}

fn marker_color(cx: &PaintContext, marker: SectionBandMarker) -> Option<Color> {
    match marker {
        SectionBandMarker::None => None,
        SectionBandMarker::Focus => Some(cx.theme.colors.borders.focus),
        SectionBandMarker::Selection => Some(cx.theme.colors.accents.primary),
    }
}

fn label_text(label: &str) -> String {
    label.to_ascii_uppercase()
}

fn constrained_size(constraints: Constraints, natural_width: f32, natural_height: f32) -> Size {
    Size::new(
        natural_width.clamp(constraints.min_width, constraints.max_width),
        natural_height.clamp(constraints.min_height, constraints.max_height),
    )
}

fn layout_band(
    cx: &mut LayoutContext,
    constraints: Constraints,
    label: &str,
    meta: Option<&str>,
) -> (Size, SectionSurfaceTypography) {
    let typography = SectionSurfaceTypography::from_base_font_size(cx.theme.typography.font_size);
    let label_run = cx
        .text_engine
        .shape(&label_text(label), typography.label_size);
    let meta_width = meta
        .map(|text| cx.text_engine.shape(text, typography.meta_size).width + META_GAP)
        .unwrap_or(0.0);
    let natural_width = H_PADDING * 2.0 + label_run.width + meta_width;
    let size = constrained_size(constraints, natural_width, BAND_HEIGHT);
    (size, typography)
}

fn paint_marker(cx: &mut PaintContext, rect: RectF, marker: SectionBandMarker) {
    if let Some(color) = marker_color(cx, marker) {
        let sf = cx.scene.scale_factor();
        let marker_rect = RectF::new(rect.x, rect.y, (2.0 / sf).max(HAIRLINE), rect.height)
            .snap_to_physical_pixels(sf);
        cx.scene
            .push_rect(marker_rect, color, LAYER_CONTENT + 2, None);
    }
}

fn paint_hairline(cx: &mut PaintContext, rect: RectF) {
    let sf = cx.scene.scale_factor();
    let top_y = snap_to_physical(rect.y, sf);
    let y = snap_to_physical(rect.y + rect.height - HAIRLINE, sf);
    cx.scene.push_rect(
        RectF::new(rect.x, top_y, rect.width, HAIRLINE),
        cx.theme.colors.borders.highlight,
        LAYER_CONTENT + 1,
        None,
    );
    cx.scene.push_rect(
        RectF::new(rect.x, y, rect.width, HAIRLINE),
        cx.theme.colors.borders.subtle,
        LAYER_CONTENT + 1,
        None,
    );
}

fn paint_label(
    cx: &mut PaintContext,
    rect: RectF,
    label: &str,
    typography: SectionSurfaceTypography,
) {
    let run = cx
        .text_engine
        .shape(&label_text(label), typography.label_size);
    cx.scene.push_text_with_backgrounds(
        &run,
        rect.x + H_PADDING,
        rect.y + TEXT_TOP,
        cx.theme.colors.typography.primary,
        vec![],
        LAYER_CONTENT + 2,
        None,
    );
}

fn paint_meta(
    cx: &mut PaintContext,
    rect: RectF,
    meta: &str,
    typography: SectionSurfaceTypography,
) {
    let run = cx.text_engine.shape(meta, typography.meta_size);
    let x = (rect.x + rect.width - H_PADDING - run.width).max(rect.x + H_PADDING);
    cx.scene.push_text_with_backgrounds(
        &run,
        x,
        rect.y + TEXT_TOP,
        cx.theme.colors.typography.secondary,
        vec![],
        LAYER_CONTENT + 2,
        None,
    );
}

fn paint_band(
    cx: &mut PaintContext,
    rect: RectF,
    label: &str,
    meta: Option<&str>,
    level: SectionBandLevel,
    marker: SectionBandMarker,
    typography: SectionSurfaceTypography,
) {
    cx.scene
        .push_rect(rect, band_background(cx, level), LAYER_CONTENT, None);
    paint_hairline(cx, rect);
    paint_marker(cx, rect, marker);
    paint_label(cx, rect, label, typography);
    if let Some(meta) = meta {
        paint_meta(cx, rect, meta, typography);
    }
}

fn disclosure_control_slot_rect(rect: RectF) -> RectF {
    let x = rect.x + H_PADDING;
    RectF::new(x, rect.y, DISCLOSURE_CONTROL_WIDTH, rect.height)
}

fn disclosure_control_button_rect(slot_rect: RectF) -> RectF {
    let size = DISCLOSURE_BUTTON_SIZE.min(slot_rect.width.min(slot_rect.height));
    RectF::new(
        slot_rect.x + (slot_rect.width - size) * 0.5,
        slot_rect.y + (slot_rect.height - size) * 0.5,
        size,
        size,
    )
}

fn paint_band_overlay(cx: &mut PaintContext, rect: RectF, hover_t: f32, pressed_t: f32) {
    if hover_t > 0.001 {
        let mut overlay = cx.theme.colors.list.band_hover_overlay;
        overlay.a *= hover_t;
        cx.scene.push_rect(rect, overlay, LAYER_CONTENT + 1, None);
    }

    if pressed_t > 0.001 {
        let mut overlay = cx.theme.colors.list.band_focused_overlay;
        overlay.a *= pressed_t;
        cx.scene.push_rect(rect, overlay, LAYER_CONTENT + 1, None);
    }
}

fn paint_disclosure_control(
    cx: &mut PaintContext,
    rect: RectF,
    expanded: bool,
    hover_t: f32,
    pressed_t: f32,
) {
    let sf = cx.scene.scale_factor();
    let raw_button = disclosure_control_button_rect(rect);
    let button_x = snap_to_physical(raw_button.x, sf);
    let button_y = snap_to_physical(raw_button.y, sf);
    let button_rect = RectF::new(
        button_x,
        button_y,
        snap_to_physical(raw_button.x + raw_button.width, sf) - button_x,
        snap_to_physical(raw_button.y + raw_button.height, sf) - button_y,
    );
    let active_t = hover_t.max(pressed_t);
    let button_fill = cx
        .theme
        .colors
        .controls
        .background
        .lerp(cx.theme.colors.controls.background_hover, hover_t * 0.55)
        .lerp(
            cx.theme.colors.controls.background_pressed,
            pressed_t * 0.75,
        );
    let corners = RoundedCorners::uniform(DISCLOSURE_CONTROL_RADIUS);
    cx.scene
        .push_rounded_rect(button_rect, button_fill, corners, LAYER_CONTENT + 2, None);

    if active_t > 0.001 {
        let mut overlay = cx.theme.colors.list.band_hover_overlay;
        overlay.a *= active_t * 0.28;
        cx.scene
            .push_rounded_rect(button_rect, overlay, corners, LAYER_CONTENT + 3, None);
    }

    let border = cx
        .theme
        .colors
        .controls
        .border
        .lerp(cx.theme.colors.controls.border_hover, active_t);
    cx.scene.push_border_rounded_rect(
        button_rect,
        border,
        cx.one_px(),
        corners,
        LAYER_CONTENT + 4,
        None,
    );

    let icon_size = DISCLOSURE_ICON_BOX_SIZE.min(button_rect.width.min(button_rect.height));
    let icon_rect = RectF::new(
        button_rect.x + (button_rect.width - icon_size) * 0.5,
        button_rect.y + (button_rect.height - icon_size) * 0.5,
        icon_size,
        icon_size,
    );

    paint_disclosure_indicator(cx, icon_rect, expanded, LAYER_CONTENT + 5);
}

impl WidgetDowncast for SectionBandHeader {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SectionBandHeader {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let (size, typography) = layout_band(cx, constraints, &self.label, None);
        self.typography = typography;
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        paint_band(
            cx,
            rect,
            &self.label,
            None,
            self.level,
            self.marker,
            self.typography,
        );
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "SectionBandHeader"
    }
}

impl WidgetDowncast for SectionMetaHeader {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SectionMetaHeader {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let (size, typography) = layout_band(cx, constraints, &self.label, Some(&self.meta));
        self.typography = typography;
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        paint_band(
            cx,
            rect,
            &self.label,
            Some(&self.meta),
            self.level,
            self.marker,
            self.typography,
        );
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "SectionMetaHeader"
    }
}

impl WidgetDowncast for DisclosureSectionHeader {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DisclosureSectionHeader {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.typography =
            SectionSurfaceTypography::from_base_font_size(cx.theme.typography.font_size);
        let run = cx
            .text_engine
            .shape(&label_text(&self.label), self.typography.label_size);
        let natural_width =
            H_PADDING * 2.0 + DISCLOSURE_CONTROL_WIDTH + DISCLOSURE_CONTROL_GAP + run.width;
        let size = constrained_size(constraints, natural_width, DISCLOSURE_HEIGHT);
        self.layout_bounds = size;
        LayoutResult::fit(size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.refresh_animation_cache(cx);
        let hover_t = self.hover_t.get();
        let pressed_t = self.pressed_t.get();

        cx.scene.push_rect(
            rect,
            cx.theme.colors.list.header_background,
            LAYER_CONTENT,
            None,
        );
        paint_band_overlay(cx, rect, hover_t, pressed_t);
        paint_hairline(cx, rect);
        paint_marker(cx, rect, self.marker);

        let control_rect = disclosure_control_slot_rect(rect);
        paint_disclosure_control(cx, control_rect, self.expanded, hover_t, pressed_t);

        let text_rect = RectF::new(
            rect.x + H_PADDING + DISCLOSURE_CONTROL_WIDTH + DISCLOSURE_CONTROL_GAP,
            rect.y,
            (rect.width - H_PADDING * 2.0 - DISCLOSURE_CONTROL_WIDTH - DISCLOSURE_CONTROL_GAP)
                .max(0.0),
            rect.height,
        );
        paint_label_at(cx, text_rect, &self.label, self.typography);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => match pe.kind {
                PointerEventKind::Down
                | PointerEventKind::DoubleClick
                | PointerEventKind::TripleClick => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    self.state = DisclosureHeaderState::Pressed;
                    if let Some(button) = pe.button {
                        cx.capture_pointer(button);
                    }
                    self.animate_pressed_to(cx, 1.0, 0.035, Easing::EaseOut);
                    true
                }
                PointerEventKind::Move => {
                    if self.state == DisclosureHeaderState::Pressed {
                        if self.pointer_inside(pe.local_pos) {
                            self.state = DisclosureHeaderState::Pressed;
                            self.animate_pressed_to(cx, 1.0, 0.035, Easing::EaseOut);
                        } else {
                            self.state = DisclosureHeaderState::Idle;
                            self.animate_pressed_to(cx, 0.0, 0.06, Easing::EaseIn);
                        }
                        return true;
                    }
                    false
                }
                PointerEventKind::Up => {
                    if pe.button != Some(PointerButton::Primary) {
                        return false;
                    }
                    let inside = self.pointer_inside(pe.local_pos);
                    if inside {
                        self.request_toggle(cx);
                    }
                    self.state = if inside {
                        DisclosureHeaderState::Hovered
                    } else {
                        DisclosureHeaderState::Idle
                    };
                    self.animate_pressed_to(cx, 0.0, 0.07, Easing::EaseIn);
                    true
                }
                _ => false,
            },
            Event::Key(ke) if !ke.is_down => {
                use crate::core::event::KeyCode;
                let is_activation_key = matches!(
                    ke.key_code,
                    KeyCode::Enter | KeyCode::Space | KeyCode::Character('i')
                );
                if !is_activation_key || !self.keyboard_pulse_active.get() {
                    return false;
                }
                self.keyboard_pulse_active.set(false);
                self.animate_pressed_to(cx, 0.0, 0.07, Easing::EaseIn);
                self.request_toggle(cx);
                true
            }
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn accepts_keyboard_activation(&self) -> bool {
        true
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if self.keyboard_pulse_active.get() {
            return;
        }
        self.keyboard_pulse_active.set(true);
        self.state = DisclosureHeaderState::Pressed;
        self.animate_pressed_to(cx, 1.0, 0.035, Easing::EaseOut);
        if self.pressed_anim.get().is_none() {
            cx.invalidate_paint();
        }
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        if self.state != DisclosureHeaderState::Pressed {
            self.state = DisclosureHeaderState::Hovered;
            self.animate_hover_to(cx, 1.0, 0.12, Easing::EaseOut);
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.state != DisclosureHeaderState::Pressed {
            self.state = DisclosureHeaderState::Idle;
            self.animate_hover_to(cx, 0.0, 0.12, Easing::EaseIn);
        }
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        self.keyboard_pulse_active.set(false);
        self.animate_pressed_to(cx, 0.0, 0.07, Easing::EaseIn);
    }

    fn on_mount(&mut self) {
        self.state = DisclosureHeaderState::Idle;
        self.hover_t.set(0.0);
        self.hover_anim.set(None);
        self.pressed_t.set(0.0);
        self.pressed_anim.set(None);
        self.keyboard_pulse_active.set(false);
    }

    fn needs_animation(&self) -> bool {
        self.hover_anim.get().is_some() || self.pressed_anim.get().is_some()
    }

    fn focus_indicator_geometry(&self, own_rect: RectF) -> FocusIndicatorGeometry {
        FocusIndicatorGeometry {
            rect: disclosure_control_button_rect(disclosure_control_slot_rect(own_rect)),
            radius: DISCLOSURE_CONTROL_RADIUS,
        }
    }

    fn paints_own_focus_indicator(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "DisclosureSectionHeader"
    }
}

fn paint_label_at(
    cx: &mut PaintContext,
    rect: RectF,
    label: &str,
    typography: SectionSurfaceTypography,
) {
    let run = cx
        .text_engine
        .shape(&label_text(label), typography.label_size);
    cx.scene.push_text_with_backgrounds(
        &run,
        rect.x,
        rect.y + TEXT_TOP,
        cx.theme.colors.typography.primary,
        vec![],
        LAYER_CONTENT + 2,
        None,
    );
}
