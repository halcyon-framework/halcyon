// SPEC-HAL-274: SegmentedControl Widget
// Horizontal bar of mutually exclusive option segments.

use crate::core::animation::{Animation, AnimationSample, Easing, TransitionHandle};
use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::{Color, LAYER_CONTENT, RoundedCorners};
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::theme::{ControlChromeCategory, ControlChromeState, ControlValidationState};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::utils::id::{TreeWidgetId, WidgetInternalId};

// ============================================================================
// Segment Definition
// ============================================================================

#[derive(Clone)]
pub struct Segment {
    pub label: String,
    pub enabled: bool,
}

impl Segment {
    pub fn new(label: &str) -> Self {
        Self {
            label: label.to_string(),
            enabled: true,
        }
    }

    pub fn with_enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }
}

// ============================================================================
// Widget
// ============================================================================

pub struct SegmentedControl {
    id: WidgetInternalId,
    segments: Vec<Segment>,
    selected: usize,
    focused: bool,
    focused_index: Option<usize>,
    hovered_index: Option<usize>,
    on_change: Option<Box<dyn FnMut(usize)>>,
    equal_width: bool,
    fill_width: bool,
    // Cached layout data
    segment_widths: Vec<f32>,
    segment_x_offsets: Vec<f32>,
    total_width: f32,
    // Animation
    hover_t: f32,
    hover_target: f32,
    hover_anim: Option<TransitionHandle>,
    hover_retiring: Option<TransitionHandle>,
    /// Sibling widgets to mark layout-dirty when the selection changes.
    /// Populated by tab-layout assemblies so that switching tabs forces the
    /// linked TabSwitcher to re-run its layout (otherwise it would be skipped
    /// as clean and the visible child would not update).
    invalidate_targets: Vec<TreeWidgetId>,
}

impl SegmentedControl {
    pub fn new(segments: Vec<Segment>, selected: usize) -> Self {
        debug_assert!(
            segments.len() >= 2,
            "SegmentedControl requires at least 2 segments"
        );
        debug_assert!(selected < segments.len(), "selected index out of bounds");
        Self {
            id: WidgetInternalId::new(),
            segments,
            selected,
            focused: false,
            focused_index: None,
            hovered_index: None,
            on_change: None,
            equal_width: false,
            fill_width: false,
            segment_widths: Vec::new(),
            segment_x_offsets: Vec::new(),
            total_width: 0.0,
            hover_t: 0.0,
            hover_target: 0.0,
            hover_anim: None,
            hover_retiring: None,
            invalidate_targets: Vec::new(),
        }
    }

    pub fn on_change(mut self, f: impl FnMut(usize) + 'static) -> Self {
        self.on_change = Some(Box::new(f));
        self
    }

    /// Register a sibling widget to mark layout-dirty whenever the selection
    /// changes. Used by `process_create_tab_layout` to wire a TabSwitcher.
    pub fn add_invalidate_target(&mut self, target: TreeWidgetId) {
        self.invalidate_targets.push(target);
    }

    pub fn with_equal_width(mut self, equal: bool) -> Self {
        self.equal_width = equal;
        self
    }

    /// Fill a finite offered width with equal segments (SPEC-HAL-274 layout addendum).
    /// Measured label minima remain logical overflow if the allocation is too narrow.
    /// Unbounded offers retain intrinsic sizing and the `with_equal_width` option.
    pub fn with_fill_width(mut self, fill: bool) -> Self {
        self.fill_width = fill;
        self
    }

    pub fn selected(&self) -> usize {
        self.selected
    }

    pub fn set_selected(&mut self, idx: usize) {
        if idx < self.segments.len() {
            self.selected = idx;
        }
    }

    fn selected_corners(&self, idx: usize, radius: f32) -> RoundedCorners {
        let is_first = idx == 0;
        let is_last = idx == self.segments.len() - 1;
        RoundedCorners {
            tl: if is_first { radius } else { 0.0 },
            tr: if is_last { radius } else { 0.0 },
            bl: if is_first { radius } else { 0.0 },
            br: if is_last { radius } else { 0.0 },
        }
    }

    fn index_at_x(&self, local_x: f32) -> Option<usize> {
        for (i, (&offset, &width)) in self
            .segment_x_offsets
            .iter()
            .zip(self.segment_widths.iter())
            .enumerate()
        {
            if local_x >= offset && local_x < offset + width {
                return Some(i);
            }
        }
        None
    }

    fn next_enabled(&self, from: usize, forward: bool) -> Option<usize> {
        let n = self.segments.len();
        for step in 1..=n {
            let idx = if forward {
                (from + step) % n
            } else {
                (from + n - step) % n
            };
            if self.segments[idx].enabled {
                return Some(idx);
            }
        }
        None
    }

    fn first_enabled(&self) -> Option<usize> {
        self.segments.iter().position(|s| s.enabled)
    }

    fn last_enabled(&self) -> Option<usize> {
        self.segments.iter().rposition(|s| s.enabled)
    }

    fn animate_hover(&mut self, cx: &mut EventContext, target: f32, easing: Easing) {
        self.hover_target = target;
        if cx.has_animation() {
            for handle in [self.hover_retiring.take(), self.hover_anim.take()]
                .into_iter()
                .flatten()
            {
                cx.cancel_animation(handle);
            }
            self.hover_anim = cx.start_animation(Animation {
                from: self.hover_t,
                to: target,
                duration: 0.12,
                easing,
            });
        } else if let Some(handle) = self.hover_anim.take() {
            self.hover_retiring = Some(handle);
        }
        if self.hover_anim.is_none() && self.hover_t != target {
            self.hover_t = target;
            cx.invalidate_paint();
        }
    }
}

impl Default for SegmentedControl {
    fn default() -> Self {
        Self::new(vec![Segment::new("A"), Segment::new("B")], 0)
    }
}

impl WidgetDowncast for SegmentedControl {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SegmentedControl {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let geom = &cx.theme.geometry.segmented_control;
        let font_size = cx.theme.typography.font_size;

        // Measure each segment
        let mut widths: Vec<f32> = Vec::with_capacity(self.segments.len());
        for seg in &self.segments {
            let run = cx.text_engine.shape(&seg.label, font_size);
            let w = (run.width + 2.0 * geom.padding_x).max(geom.min_segment_width);
            widths.push(w);
        }

        let sep_total = geom.separator_width * (self.segments.len() as f32 - 1.0);
        let finite_fill = self.fill_width && constraints.max_width.is_finite();
        if self.equal_width || finite_fill {
            let mut max_w = widths.iter().copied().fold(0.0_f32, f32::max);
            if finite_fill {
                let offered = (constraints.max_width - sep_total) / self.segments.len() as f32;
                max_w = max_w.max(offered);
            }
            for w in &mut widths {
                *w = max_w;
            }
        }

        // Compute x-offsets and total width
        let total: f32 = widths.iter().sum::<f32>() + sep_total;

        let mut offsets = Vec::with_capacity(self.segments.len());
        let mut x = 0.0;
        for (i, w) in widths.iter().enumerate() {
            offsets.push(x);
            x += w;
            if i < self.segments.len() - 1 {
                x += geom.separator_width;
            }
        }

        self.segment_widths = widths;
        self.segment_x_offsets = offsets;
        self.total_width = total;

        let w = total.min(constraints.max_width).max(constraints.min_width);
        let size = Size::new(w, geom.height);
        if self.fill_width {
            LayoutResult::new(size, Size::new(total.max(w), geom.height))
        } else {
            LayoutResult::fit(size)
        }
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let geom = &cx.theme.geometry.segmented_control;
        let radius =
            cx.theme
                .chrome_radius(ControlChromeCategory::BinarySelector, geom.radius, false);
        let font_size = cx.theme.typography.font_size;
        let selector_chrome = cx.theme.control_chrome(
            ControlChromeCategory::BinarySelector,
            ControlChromeState::Idle,
            ControlValidationState::Neutral,
        );
        let selected_chrome = cx.theme.control_chrome(
            ControlChromeCategory::RaisedClickable,
            ControlChromeState::Idle,
            ControlValidationState::Neutral,
        );

        // Widget background
        cx.scene.push_rounded_rect(
            rect,
            cx.theme.colors.surfaces.content,
            RoundedCorners::uniform(radius),
            LAYER_CONTENT,
            None,
        );

        // Selected segment fill
        if self.selected < self.segment_x_offsets.len() {
            let seg_x = rect.x + self.segment_x_offsets[self.selected];
            let seg_w = self.segment_widths[self.selected];
            let seg_rect = RectF::new(seg_x, rect.y, seg_w, rect.height);
            let corners = self.selected_corners(self.selected, radius);
            cx.scene.push_rounded_rect(
                seg_rect,
                selected_chrome
                    .fill
                    .unwrap_or(cx.theme.colors.controls.background),
                corners,
                LAYER_CONTENT + 1,
                None,
            );
        }

        // Hovered segment fill (non-selected)
        let hover_alpha = self.hover_t;
        if let Some(hi) = self.hovered_index {
            if hi != self.selected && hi < self.segment_x_offsets.len() && hover_alpha > 0.001 {
                let seg_x = rect.x + self.segment_x_offsets[hi];
                let seg_w = self.segment_widths[hi];
                let seg_rect = RectF::new(seg_x, rect.y, seg_w, rect.height);
                let alpha = hover_alpha * hover_alpha * cx.theme.styles.hover.max_alpha;
                let overlay_color = Color::from_srgb(1.0, 1.0, 1.0, alpha);
                cx.scene
                    .begin_clip(rect, Some(RoundedCorners::uniform(radius)));
                cx.scene
                    .push_rect(seg_rect, overlay_color, LAYER_CONTENT + 1, None);
                cx.scene.end_clip();
            }
        }

        // Separators between segments (skip adjacent to selected)
        for i in 0..self.segments.len() - 1 {
            if i == self.selected || i + 1 == self.selected {
                continue;
            }
            let sep_x = rect.x + self.segment_x_offsets[i] + self.segment_widths[i];
            let sep_inset = rect.height * 0.2;
            let sep_rect = RectF::new(
                sep_x,
                rect.y + sep_inset,
                geom.separator_width,
                rect.height - 2.0 * sep_inset,
            );
            cx.scene.push_rect(
                sep_rect,
                cx.theme.colors.borders.default,
                LAYER_CONTENT + 1,
                None,
            );
        }

        // Outer border
        cx.scene.push_border_rounded_rect(
            rect,
            selector_chrome.border_color,
            selector_chrome.border_width,
            RoundedCorners::uniform(radius),
            LAYER_CONTENT + 2,
            None,
        );

        // SPEC-HAL-012/328: qualify the existing segment ring with current
        // framework ownership and application cue policy.
        if cx.has_focus_cue() {
            if let Some(fi) = self.focused_index {
                if fi < self.segment_x_offsets.len() {
                    let chrome = cx.theme.control_chrome(
                        ControlChromeCategory::BinarySelector,
                        ControlChromeState::Focused,
                        ControlValidationState::Neutral,
                    );
                    let ring_radius = cx.theme.chrome_radius(
                        ControlChromeCategory::BinarySelector,
                        radius,
                        false,
                    );
                    let seg_x = rect.x + self.segment_x_offsets[fi];
                    let seg_w = self.segment_widths[fi];
                    let focus_rect = RectF::new(seg_x, rect.y, seg_w, rect.height);
                    let corners = self.selected_corners(fi, ring_radius);
                    cx.scene.push_border_rounded_rect(
                        focus_rect,
                        chrome.focus_color,
                        chrome.focus_width,
                        corners,
                        LAYER_CONTENT + 3,
                        None,
                    );
                }
            }
        }

        // Labels
        let metrics = cx.text_engine.font_metrics(font_size);
        let baseline_y = rect.y
            + compute_control_text_baseline_y(
                rect.height,
                metrics.ascent,
                metrics.descent,
                cx.theme.typography.baseline_offset,
            );
        let label_y = baseline_y - metrics.ascent;

        for (i, seg) in self.segments.iter().enumerate() {
            let text_color = if i == self.selected {
                cx.theme.colors.controls.text
            } else if seg.enabled {
                cx.theme.colors.typography.secondary
            } else {
                cx.theme.colors.typography.muted
            };

            let run = cx.text_engine.shape(&seg.label, font_size);
            let seg_x = rect.x + self.segment_x_offsets[i];
            let seg_w = self.segment_widths[i];
            let text_x = seg_x + (seg_w - run.width) * 0.5;

            cx.scene.push_text_with_backgrounds(
                &run,
                text_x,
                label_y,
                text_color,
                vec![],
                LAYER_CONTENT + 2,
                None,
            );
        }
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                let (lx, ly) = pe.local_pos;

                match pe.kind {
                    PointerEventKind::Move => {
                        let prev = self.hovered_index;
                        self.hovered_index = if ly >= 0.0 && ly <= self.total_width.max(1.0) {
                            self.index_at_x(lx)
                        } else {
                            None
                        };
                        if prev != self.hovered_index {
                            // Hover changed — handled by on_hover_enter/leave at widget level
                        }
                        true
                    }
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if let Some(idx) = self.index_at_x(lx) {
                            if self.segments[idx].enabled && idx != self.selected {
                                self.selected = idx;
                                // Keep the focus ring in sync with the active
                                // segment — otherwise it stays on whatever the
                                // last keyboard-focused index was.
                                self.focused_index = Some(idx);
                                if let Some(ref mut cb) = self.on_change {
                                    cb(idx);
                                }
                                for &target in &self.invalidate_targets {
                                    cx.invalidate_layout_by_id(target);
                                }
                                cx.request_continuous_frames();
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down && self.focused => match ke.key_code {
                KeyCode::Left => {
                    if let Some(fi) = self.focused_index {
                        if let Some(next) = self.next_enabled(fi, false) {
                            self.focused_index = Some(next);
                            cx.request_continuous_frames();
                        }
                    }
                    true
                }
                KeyCode::Right => {
                    if let Some(fi) = self.focused_index {
                        if let Some(next) = self.next_enabled(fi, true) {
                            self.focused_index = Some(next);
                            cx.request_continuous_frames();
                        }
                    }
                    true
                }
                KeyCode::Home => {
                    self.focused_index = self.first_enabled();
                    cx.request_continuous_frames();
                    true
                }
                KeyCode::End => {
                    self.focused_index = self.last_enabled();
                    cx.request_continuous_frames();
                    true
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        if let Some(fi) = self.focused_index {
            // Bound first: a stale focused_index would otherwise
            // panic on `self.segments[fi]` if it ever drifted
            // past segments.len().
            if fi < self.segments.len() && fi != self.selected && self.segments[fi].enabled {
                self.selected = fi;
                if let Some(ref mut cb) = self.on_change {
                    cb(fi);
                }
                for &target in &self.invalidate_targets {
                    cx.invalidate_layout_by_id(target);
                }
                cx.invalidate_paint();
            }
        }
    }

    fn accepts_keyboard_activation(&self) -> bool {
        true
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if let Some(handle) = self.hover_retiring {
            match cx.sample_animation::<f32>(handle) {
                Some(AnimationSample::TypeMismatch) => {
                    panic!("SegmentedControl hover handle must contain f32");
                }
                Some(_) => {
                    cx.cancel_animation(handle);
                    self.hover_retiring = None;
                }
                None => {}
            }
        }
        let Some(handle) = self.hover_anim else {
            return;
        };
        let value = match cx.sample_animation::<f32>(handle) {
            Some(AnimationSample::Active(value)) => value,
            Some(AnimationSample::Completed(value)) => {
                self.hover_anim = None;
                value
            }
            Some(AnimationSample::Cancelled) => {
                self.hover_anim = None;
                self.hover_t
            }
            Some(AnimationSample::Missing) => {
                self.hover_anim = None;
                self.hover_target
            }
            None => {
                self.hover_anim = None;
                self.hover_retiring = Some(handle);
                self.hover_target
            }
            Some(AnimationSample::TypeMismatch) => {
                panic!("SegmentedControl hover handle must contain f32");
            }
        };
        if self.hover_t != value {
            self.hover_t = value;
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.hover_anim.is_some() || self.hover_retiring.is_some()
    }

    fn on_focus(&mut self, _cx: &mut EventContext) {
        self.focused = true;
        self.focused_index = Some(self.selected);
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.focused = false;
        self.focused_index = None;
    }

    fn on_hover_enter(&mut self, cx: &mut EventContext) {
        self.animate_hover(cx, 1.0, Easing::EaseOut);
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        self.hovered_index = None;
        self.animate_hover(cx, 0.0, Easing::EaseIn);
    }

    fn on_mount(&mut self) {
        self.hover_t = 0.0;
        self.hover_target = 0.0;
        self.hover_anim = None;
        self.hover_retiring = None;
    }

    fn on_unmount(&mut self) {
        self.hover_t = 0.0;
        self.hover_target = 0.0;
        self.hover_anim = None;
        self.hover_retiring = None;
    }

    fn debug_type_name(&self) -> &'static str {
        "SegmentedControl"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::app::{layout_tree, test_begin_frame_layout_tracking};
    use crate::core::text_engine::TextEngine;
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::{flex::Flex, groupbox::GroupBox, tooltip_host::TooltipHost};

    #[test]
    fn segmented_control_fill_width_preserves_intrinsic_and_segment_geometry() {
        fn measure(
            control: &mut SegmentedControl,
            theme: &Theme,
            text: &mut dyn TextEngine,
            max_width: f32,
        ) -> LayoutResult {
            let mut tree = WidgetTree::new();
            let mut cx = LayoutContext::new(&mut tree, text, theme);
            control.layout(
                &mut cx,
                Constraints::new(0.0, max_width, 0.0, f32::INFINITY),
                &[],
            )
        }

        let theme = Theme::default();
        let mut text = FontManagerTextEngine::new();
        let labels = ["Live", "Replay", "Canary", "Locked"];
        let make = || {
            SegmentedControl::new(
                labels
                    .iter()
                    .enumerate()
                    .map(|(i, label)| Segment::new(label).with_enabled(i != 3))
                    .collect(),
                1,
            )
        };
        let geom = &theme.geometry.segmented_control;
        let measured: Vec<f32> = labels
            .iter()
            .map(|label| {
                (text.shape(label, theme.typography.font_size).width + 2.0 * geom.padding_x)
                    .max(geom.min_segment_width)
            })
            .collect();
        let widest = measured.iter().copied().fold(0.0_f32, f32::max);
        let separators = geom.separator_width * (labels.len() - 1) as f32;
        let intrinsic_width = measured.iter().sum::<f32>() + separators;
        let minimum_equal_width = widest * labels.len() as f32 + separators;
        let offered = minimum_equal_width + 100.0;

        let mut intrinsic = make();
        let natural = measure(&mut intrinsic, &theme, &mut text, offered);
        assert_eq!(intrinsic.segment_widths, measured);
        assert_eq!(natural.size.width, intrinsic_width);

        let mut equal = make().with_equal_width(true);
        let legacy = measure(&mut equal, &theme, &mut text, offered);
        assert_eq!(equal.segment_widths, vec![widest; labels.len()]);
        assert_eq!(legacy.size.width, minimum_equal_width);

        let mut fill = make().with_fill_width(true);
        fill.focused = true;
        fill.focused_index = Some(2);
        fill.hovered_index = Some(3);
        let identity = fill.id;
        let filled = measure(&mut fill, &theme, &mut text, offered);
        assert!((filled.size.width - offered).abs() < 0.001);
        assert!((filled.content_size.width - offered).abs() < 0.001);
        let segment_width = (offered - separators) / labels.len() as f32;
        assert_eq!(fill.segment_widths, vec![segment_width; labels.len()]);
        assert!(
            (fill.segment_widths.iter().sum::<f32>() + separators - fill.total_width).abs() < 0.001
        );
        for i in 0..labels.len() {
            let start = fill.segment_x_offsets[i];
            let end = start + fill.segment_widths[i];
            assert_eq!(fill.index_at_x(start), Some(i));
            assert_eq!(fill.index_at_x(end - 0.01), Some(i));
            assert_eq!(fill.index_at_x(end), None);
            if i + 1 < labels.len() {
                assert_eq!(fill.segment_x_offsets[i + 1], end + geom.separator_width);
            }
        }
        assert_eq!(fill.index_at_x(fill.total_width), None);

        let narrow_width = minimum_equal_width * 0.5;
        let narrow = measure(&mut fill, &theme, &mut text, narrow_width);
        assert_eq!(narrow.size.width, narrow_width);
        assert_eq!(narrow.content_size.width, minimum_equal_width);
        assert_eq!(fill.segment_widths, vec![widest; labels.len()]);
        for (width, minimum) in fill.segment_widths.iter().zip(&measured) {
            assert!(width >= minimum);
        }
        let offsets = fill.segment_x_offsets.clone();
        let repeated = measure(&mut fill, &theme, &mut text, narrow_width);
        assert_eq!(repeated.size, narrow.size);
        assert_eq!(repeated.content_size, narrow.content_size);
        assert_eq!(fill.segment_x_offsets, offsets);
        let unbounded = measure(&mut fill, &theme, &mut text, f32::INFINITY);
        assert_eq!(unbounded.size.width, intrinsic_width);
        assert_eq!(fill.segment_widths, measured);
        assert_eq!(fill.id, identity);
        assert_eq!(fill.selected, 1);
        assert!(fill.focused);
        assert_eq!(fill.focused_index, Some(2));
        assert_eq!(fill.hovered_index, Some(3));
        assert!(!fill.segments[3].enabled);
        let mut legacy_again = make()
            .with_equal_width(true)
            .with_fill_width(true)
            .with_fill_width(false);
        assert_eq!(
            measure(&mut legacy_again, &theme, &mut text, offered).size,
            legacy.size
        );

        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(GroupBox::new(NodeId(0), "Execution Flags")),
            None,
            0,
        );
        let column = tree.add_child(root, Box::new(Flex::column().with_spacing(5.0)), None, 0);
        crate::replace_widget!(tree, root, GroupBox::new(column, "Execution Flags"));
        let tip = tree.add_child(
            column,
            Box::new(TooltipHost::new("Execution mode for the session")),
            None,
            0,
        );
        let control = tree.add_child(tip, Box::new(make().with_fill_width(true)), None, 0);
        test_begin_frame_layout_tracking();
        {
            let _layout = crate::core::rect_authority::LayoutAuthorityGuard::enter();
            layout_tree(
                &mut tree,
                root,
                Constraints::new(offered + 100.0, offered + 100.0, 0.0, f32::INFINITY),
                &theme,
                &mut text,
                None,
            );
        }
        let column_rect = tree.get_absolute_paint_rect(column).unwrap();
        let tip_rect = tree.get_absolute_paint_rect(tip).unwrap();
        let control_rect = tree.get_absolute_paint_rect(control).unwrap();
        assert!((control_rect.width - column_rect.width).abs() < 0.001);
        assert_eq!(control_rect.x, column_rect.x);
        assert_eq!(control_rect, tip_rect);
        let mounted = tree
            .node(control)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<SegmentedControl>()
            .unwrap();
        assert!((mounted.total_width - control_rect.width).abs() < 0.001);
        assert!(mounted.segment_widths.iter().all(|width| *width >= widest));
    }

    #[test]
    fn segmented_control_focus_cue_follows_framework_owner_and_policy() {
        use crate::core::app::{RetainedSceneTestFrame, RetainedSceneTestHarness};
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, Modifiers, PhysicalKeyCode, PointerId,
            UiInteractionPolicy, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::math::Point;
        use crate::core::scene::SceneNode;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::label::Label;
        use std::cell::Cell;
        use std::rc::Rc;
        use std::sync::Arc;
        use std::time::Instant;

        fn selector(tree: &WidgetTree, node: NodeId) -> &SegmentedControl {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn composition(frame: &RetainedSceneTestFrame) -> Arc<[SceneNode]> {
            let [SceneNode::RetainedFragment(root)] = frame.scene_root.as_slice() else {
                panic!()
            };
            root.children.clone()
        }
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let control = SegmentedControl::new(vec![Segment::new("Live"), Segment::new("Replay")], 0)
            .with_fill_width(true)
            .on_change(move |_| observed.set(observed.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let stable_state = |tree: &WidgetTree| {
            let value = selector(tree, node);
            (
                value
                    .segments
                    .iter()
                    .map(|segment| (segment.label.clone(), segment.enabled))
                    .collect::<Vec<_>>(),
                value.selected,
                value.equal_width,
                value.fill_width,
                value.segment_widths.clone(),
                value.segment_x_offsets.clone(),
                value.total_width,
                value.hovered_index,
                (value.hover_t, value.hover_anim),
                value.invalidate_targets.clone(),
            )
        };
        let internal_target = |tree: &WidgetTree| {
            let value = selector(tree, node);
            (value.focused, value.focused_index)
        };
        let ordinary = |nodes: &[SceneNode]| {
            let fills = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::RoundedRect(rect) => {
                        Some((rect.rect, rect.color, rect.radius, rect.z_index))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let separators = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) => Some((rect.rect, rect.color, rect.z_index)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let borders = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) if border.z_index != LAYER_CONTENT + 3 => Some((
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let labels = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(label) => Some((
                        label.text.clone(),
                        label.position,
                        label.font_size,
                        label.color,
                        label.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (fills, separators, borders, labels)
        };
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let mut text = FontManagerTextEngine::new();
        let mut harness = RetainedSceneTestHarness::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let mut pointer_domains =
            InteractionDomains::with_ui_interaction_policy(UiInteractionPolicy::PointerDriven);
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(400.0, 120.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        let first = frame(&mut tree, &mut harness, &mut text, &pointer_domains);
        assert!(first.scene_built && !first.needs_next_frame);
        let mut composed = composition(&first);
        let mut previous = harness.local_fragment(owner).unwrap();
        let resting = ordinary(&previous);
        let state = stable_state(&tree);
        let bounds = geometry(&tree);
        let origin = bounds[1].2;
        let control = selector(&tree, node);
        assert!(control.fill_width);
        assert_eq!(control.total_width, origin.width);
        assert_eq!(control.segment_widths[0], control.segment_widths[1]);
        assert_eq!(
            control.segment_x_offsets[1],
            control.segment_widths[0] + theme.geometry.segmented_control.separator_width
        );
        let targets = (0..2)
            .map(|index| {
                RectF::new(
                    origin.x + control.segment_x_offsets[index],
                    origin.y,
                    control.segment_widths[index],
                    origin.height,
                )
            })
            .collect::<Vec<_>>();
        let points = targets
            .iter()
            .map(|rect| Point::new(rect.x + rect.width * 0.5, rect.y + rect.height * 0.5))
            .collect::<Vec<_>>();
        let hits = |tree: &WidgetTree| {
            points
                .iter()
                .map(|point| {
                    let hit = tree.hit_test(point.x, point.y).unwrap();
                    (hit.widget_id, hit.local_pos, hit.absolute_bounds)
                })
                .collect::<Vec<_>>()
        };
        let original_hits = hits(&tree);
        assert!(original_hits.iter().all(|hit| hit.0 == owner));
        for (index, point) in points.iter().enumerate() {
            assert_eq!(control.index_at_x(point.x - origin.x), Some(index));
        }
        let selected = resting
            .0
            .iter()
            .filter(|rect| rect.3 == LAYER_CONTENT + 1)
            .collect::<Vec<_>>();
        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].0, targets[0]);
        let selected_chrome = theme.control_chrome(
            ControlChromeCategory::RaisedClickable,
            ControlChromeState::Idle,
            ControlValidationState::Neutral,
        );
        assert_eq!(
            selected[0].1,
            selected_chrome
                .fill
                .unwrap_or(theme.colors.controls.background)
                .into()
        );
        assert_eq!(resting.3.len(), 2);
        let radius = theme.chrome_radius(
            ControlChromeCategory::BinarySelector,
            theme.geometry.segmented_control.radius,
            false,
        );
        let ring_radius = theme.chrome_radius(ControlChromeCategory::BinarySelector, radius, false);
        let ring_radii = (0..2)
            .map(|index| control.selected_corners(index, ring_radius).max_radius())
            .collect::<Vec<_>>();
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let check_projection = |nodes: &[SceneNode], target: Option<usize>, phase: &str| {
            let rings = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) if border.z_index == LAYER_CONTENT + 3 => {
                        Some(border)
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            assert_eq!(
                rings.len(),
                usize::from(target.is_some()),
                "{phase}: segment cue must follow current framework owner and policy"
            );
            if let Some(index) = target {
                let ring = rings[0];
                let chrome = theme.control_chrome(
                    ControlChromeCategory::BinarySelector,
                    ControlChromeState::Focused,
                    ControlValidationState::Neutral,
                );
                assert_eq!(ring.rect, targets[index]);
                assert_eq!(ring.border_color, chrome.focus_color.into());
                assert_eq!(ring.border_width, chrome.focus_width);
                assert_eq!(ring.radius, ring_radii[index]);
                assert_eq!(ring.fill_color.a, 0.0);
                assert_eq!(ring.z_index, LAYER_CONTENT + 3);
                assert!(
                    ring.rect.x >= origin.x
                        && ring.rect.x + ring.rect.width <= origin.x + origin.width
                );
                assert!(resting.0.iter().all(|rect| rect.3 < ring.z_index));
                assert!(resting.2.iter().all(|border| border.5 < ring.z_index));
                assert!(resting.3.iter().all(|label| label.4 < ring.z_index));
            }
            assert_eq!(ordinary(nodes), resting);
        };
        let check_changed = |result: &RetainedSceneTestFrame,
                             harness: &RetainedSceneTestHarness,
                             previous: &Arc<[SceneNode]>,
                             composed: &Arc<[SceneNode]>| {
            assert!(result.scene_built && !result.needs_next_frame);
            assert!(!Arc::ptr_eq(
                previous,
                &harness.local_fragment(owner).unwrap()
            ));
            let next = composition(result);
            assert!(!Arc::ptr_eq(composed, &next));
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(root_id).unwrap()
            ));
            next
        };
        let check_stable = |tree: &WidgetTree| {
            assert_eq!(geometry(tree), bounds);
            assert_eq!(stable_state(tree), state);
            assert_eq!(hits(tree), original_hits);
            assert_eq!(calls.get(), 0);
            assert!(!tree.node(node).unwrap().dirty_layout);
        };
        let settle = |tree: &mut WidgetTree,
                      harness: &mut RetainedSceneTestHarness,
                      text: &mut FontManagerTextEngine,
                      domains: &InteractionDomains,
                      router: &mut EventRouter| {
            router.reset_continuous_frames_request();
            let local = harness.local_fragment(owner).unwrap();
            let idle = frame(tree, harness, text, domains);
            assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
            assert!(Arc::ptr_eq(&local, &harness.local_fragment(owner).unwrap()));
            assert!(!router.continuous_frames_requested());
            check_stable(tree);
        };
        check_projection(&previous, None, "unfocused PointerDriven");

        router.request_focus_for_widget(&mut tree, owner, pointer_domains.primary_state_mut());
        assert_eq!(
            pointer_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert_eq!(internal_target(&tree), (true, Some(0)));
        let pointer = frame(&mut tree, &mut harness, &mut text, &pointer_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, None, "first focused PointerDriven frame");
        assert_eq!(internal_target(&tree), (true, Some(0)));
        composed = check_changed(&pointer, &harness, &previous, &composed);
        previous = fragment;
        assert!(router.continuous_frames_requested());
        settle(
            &mut tree,
            &mut harness,
            &mut text,
            &pointer_domains,
            &mut router,
        );

        let mut navigation_domains = InteractionDomains::new();
        router.request_focus_for_widget(&mut tree, owner, navigation_domains.primary_state_mut());
        assert_eq!(
            navigation_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        let navigation = frame(&mut tree, &mut harness, &mut text, &navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(
            &fragment,
            Some(0),
            "first focused NavigationAndCapture frame",
        );
        composed = check_changed(&navigation, &harness, &previous, &composed);
        previous = fragment;
        assert!(router.continuous_frames_requested());
        settle(
            &mut tree,
            &mut harness,
            &mut text,
            &navigation_domains,
            &mut router,
        );

        router.route_input_event(
            &mut tree,
            FusedInputEvent::KeyDown {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                key_code: KeyCode::Right,
                physical_key: PhysicalKeyCode::ArrowRight,
                reservation: None,
                modifiers: Modifiers::default(),
                repeat: false,
            },
            Some(root),
            &mut text,
            None,
            navigation_domains.primary_state_mut(),
            &mut bindings,
        );
        assert_eq!(internal_target(&tree), (true, Some(1)));
        let moved = frame(&mut tree, &mut harness, &mut text, &navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, Some(1), "routed Right");
        composed = check_changed(&moved, &harness, &previous, &composed);
        previous = fragment;
        assert!(router.continuous_frames_requested());
        settle(
            &mut tree,
            &mut harness,
            &mut text,
            &navigation_domains,
            &mut router,
        );

        navigation_domains
            .primary_state_mut()
            .focus_manager
            .set_focus(None);
        assert_eq!(internal_target(&tree), (true, Some(1)));
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert!(!tree.node(root).unwrap().dirty_composition);
        let lost = frame(&mut tree, &mut harness, &mut text, &navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(&fragment, None, "actual owner None without widget callback");
        assert_eq!(internal_target(&tree), (true, Some(1)));
        composed = check_changed(&lost, &harness, &previous, &composed);
        previous = fragment;
        settle(
            &mut tree,
            &mut harness,
            &mut text,
            &navigation_domains,
            &mut router,
        );

        navigation_domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(owner));
        assert_eq!(internal_target(&tree), (true, Some(1)));
        assert!(!tree.node(node).unwrap().dirty_paint);
        assert!(!tree.node(root).unwrap().dirty_composition);
        let restored = frame(&mut tree, &mut harness, &mut text, &navigation_domains);
        let fragment = harness.local_fragment(owner).unwrap();
        check_projection(
            &fragment,
            Some(1),
            "actual owner restored without widget callback",
        );
        assert_eq!(internal_target(&tree), (true, Some(1)));
        check_changed(&restored, &harness, &previous, &composed);
        settle(
            &mut tree,
            &mut harness,
            &mut text,
            &navigation_domains,
            &mut router,
        );
        assert_eq!(
            navigation_domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert!(
            navigation_domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
    }

    #[test]
    fn segmented_control_hover_updates_stationary_retained_owner() {
        use crate::core::animation::{AnimationContext, AnimationSample, Timeline};
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, PointerButtonsState,
            PointerDevice, PointerId, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::math::Point;
        use crate::core::rect_authority::{PaintAuthorityGuard, UpdateAuthorityGuard};
        use crate::core::scene::{RectNode, SceneBuilder, SceneNode};
        use crate::core::widget::UpdateContext;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::label::Label;
        use std::cell::Cell;
        use std::rc::Rc;
        use std::sync::Arc;
        use std::time::Instant;

        fn selector(tree: &WidgetTree, node: NodeId) -> &SegmentedControl {
            tree.node(node)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }
        fn published(tree: &WidgetTree, node: NodeId) -> (f32, Option<TransitionHandle>) {
            let value = selector(tree, node);
            (value.hover_t, value.hover_anim)
        }
        fn publish(
            tree: &mut WidgetTree,
            node: NodeId,
            animation: Option<AnimationContext<'_>>,
        ) -> bool {
            let _update = UpdateAuthorityGuard::enter();
            let mut cx = UpdateContext::new(animation);
            cx.begin_widget_update_for_owner(tree.node(node).unwrap().id);
            tree.node_mut(node).unwrap().widget.update(0.0, &mut cx);
            let (paint, subtree, layout) = cx.take_widget_invalidations();
            assert!(!subtree && !layout);
            if paint {
                tree.mark_paint_dirty(node);
            }
            paint
        }
        fn overlay(nodes: &[SceneNode]) -> Option<(RectF, Option<[f32; 4]>, RectNode)> {
            nodes.iter().find_map(|node| {
                let SceneNode::Clip(clip) = node else {
                    return None;
                };
                let [SceneNode::Rect(surface)] = clip.children.as_slice() else {
                    panic!("hover clip must retain its one overlay")
                };
                Some((
                    clip.rect,
                    clip.rounded
                        .as_ref()
                        .map(|corners| [corners.tl, corners.tr, corners.bl, corners.br]),
                    surface.clone(),
                ))
            })
        }
        let ordinary = |nodes: &[SceneNode]| {
            let fills = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::RoundedRect(rect) => {
                        Some((rect.rect, rect.color, rect.radius, rect.z_index))
                    }
                    _ => None,
                })
                .collect::<Vec<_>>();
            let edges = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Rect(rect) => Some((rect.rect, rect.color, rect.z_index)),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let borders = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::BorderRect(border) => Some((
                        border.rect,
                        border.fill_color,
                        border.border_color,
                        border.border_width,
                        border.radius,
                        border.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            let labels = nodes
                .iter()
                .filter_map(|node| match node {
                    SceneNode::Text(text) => Some((
                        text.text.clone(),
                        text.position,
                        text.font_size,
                        text.color,
                        text.z_index,
                    )),
                    _ => None,
                })
                .collect::<Vec<_>>();
            (fills, edges, borders, labels)
        };
        let calls = Rc::new(Cell::new(0));
        let observed = calls.clone();
        let control = SegmentedControl::new(vec![Segment::new("Live"), Segment::new("Replay")], 0)
            .with_fill_width(true)
            .on_change(move |_| observed.set(observed.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column().with_spacing(12.0)), None, 0);
        let node = tree.add_child(root, Box::new(control), None, 0);
        let peer = tree.add_child(root, Box::new(Label::new("Stable peer")), None, 1);
        let [root_id, owner, peer_id] = [root, node, peer].map(|id| tree.node(id).unwrap().id);
        let geometry = |tree: &WidgetTree| {
            [root, node, peer].map(|id| {
                let entry = tree.node(id).unwrap();
                (
                    entry.id,
                    entry.widget.id(),
                    tree.get_absolute_paint_rect(id).unwrap(),
                    entry.layout_epoch,
                )
            })
        };
        let semantic = |tree: &WidgetTree| {
            let value = selector(tree, node);
            (
                value.selected,
                value.focused,
                value.focused_index,
                value
                    .segments
                    .iter()
                    .map(|segment| (segment.label.clone(), segment.enabled))
                    .collect::<Vec<_>>(),
                value.fill_width,
                value.equal_width,
                value.segment_widths.clone(),
                value.segment_x_offsets.clone(),
                value.total_width,
                value.invalidate_targets.clone(),
            )
        };
        let theme = Theme::default();
        let icons = crate::core::icons::builtin_registry();
        let mut text = FontManagerTextEngine::new();
        let mut domains = InteractionDomains::new();
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let mut harness = RetainedSceneTestHarness::new();
        let frame = |tree: &mut WidgetTree,
                     harness: &mut RetainedSceneTestHarness,
                     text: &mut FontManagerTextEngine,
                     domains: &InteractionDomains| {
            harness.frame_with_appearance(
                tree,
                root,
                Constraints::tight(Size::new(400.0, 120.0)),
                &theme,
                text,
                &icons,
                domains,
                WindowId::PRIMARY,
            )
        };
        let route = |pos: Point,
                     tree: &mut WidgetTree,
                     router: &mut EventRouter,
                     text: &mut FontManagerTextEngine,
                     domains: &mut InteractionDomains,
                     bindings: &mut BindingRegistry,
                     animation: Option<AnimationContext<'_>>| {
            router.route_input_event(
                tree,
                FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time: Instant::now(),
                    pos,
                    delta: (0.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState {
                        primary: false,
                        secondary: false,
                        auxiliary: false,
                    },
                },
                Some(root),
                text,
                animation,
                domains.primary_state_mut(),
                bindings,
            );
        };
        harness.advance_transition_time(0.0);
        frame(&mut tree, &mut harness, &mut text, &domains);
        router.request_focus_for_widget(&mut tree, owner, domains.primary_state_mut());
        assert!(router.continuous_frames_requested());
        frame(&mut tree, &mut harness, &mut text, &domains);
        router.reset_continuous_frames_request();
        let bounds = geometry(&tree);
        let state = semantic(&tree);
        let origin = bounds[1].2;
        let control = selector(&tree, node);
        assert_eq!(control.total_width, origin.width);
        let target = RectF::new(
            origin.x + control.segment_x_offsets[1],
            origin.y,
            control.segment_widths[1],
            origin.height,
        );
        let point = Point::new(
            target.x + target.width * 0.5,
            target.y + target.height * 0.5,
        );
        let outside = Point::new(origin.x - 2.0, origin.y - 2.0);
        let hit = |tree: &WidgetTree| {
            let result = tree.hit_test(point.x, point.y).unwrap();
            (result.widget_id, result.local_pos, result.absolute_bounds)
        };
        let original_hit = hit(&tree);
        let original = harness.local_fragment(owner).unwrap();
        let resting = ordinary(&original);
        assert_eq!(
            resting
                .2
                .iter()
                .filter(|border| border.5 == LAYER_CONTENT + 3)
                .count(),
            1
        );
        let peer_fragment = harness.local_fragment(peer_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let check_overlay = |nodes: &[SceneNode], progress: Option<f32>| {
            let actual = overlay(nodes);
            assert_eq!(actual.is_some(), progress.is_some());
            if let Some(progress) = progress {
                let (clip, rounded, surface) = actual.unwrap();
                assert_eq!(clip, origin);
                let radius = theme.chrome_radius(
                    ControlChromeCategory::BinarySelector,
                    theme.geometry.segmented_control.radius,
                    false,
                );
                assert_eq!(rounded, Some(RoundedCorners::uniform(radius).to_array()));
                assert_eq!(surface.rect, target);
                let alpha = progress * progress * theme.styles.hover.max_alpha;
                assert_eq!(surface.color, Color::from_srgb(1.0, 1.0, 1.0, alpha).into());
                assert_eq!(surface.z_index, LAYER_CONTENT + 1);
            }
        };
        let check_stable = |tree: &WidgetTree, harness: &RetainedSceneTestHarness| {
            assert_eq!(geometry(tree), bounds);
            assert_eq!(semantic(tree), state);
            assert_eq!(hit(tree), original_hit);
            assert_eq!(calls.get(), 0);
            assert!(!tree.node(node).unwrap().dirty_layout);
            assert!(Arc::ptr_eq(
                &peer_fragment,
                &harness.local_fragment(peer_id).unwrap()
            ));
            assert!(Arc::ptr_eq(
                &root_fragment,
                &harness.local_fragment(root_id).unwrap()
            ));
            assert_eq!(ordinary(&harness.local_fragment(owner).unwrap()), resting);
        };

        route(
            point,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            Some(harness.animation_context()),
        );
        assert_eq!(domains.primary_state().hovered_widget, Some(owner));
        assert_eq!(selector(&tree, node).hovered_index, Some(1));
        let entered = published(&tree, node).1.unwrap();
        assert_eq!(
            harness.animation_context().sample::<f32>(entered),
            AnimationSample::Active(0.0)
        );
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_overlay(&harness.local_fragment(owner).unwrap(), None);
        frame(&mut tree, &mut harness, &mut text, &domains);
        let stationary_start = harness.local_fragment(owner).unwrap();
        assert!(!tree.node(node).unwrap().dirty_paint);
        harness.advance_transition_time(0.06);
        let advanced = frame(&mut tree, &mut harness, &mut text, &domains);
        let progressing = harness.local_fragment(owner).unwrap();
        let observed_alpha = overlay(&progressing).map(|entry| entry.2.color.a);
        eprintln!(
            "stationary input: overlay_alpha={observed_alpha:?} owner_reused={} scene_built={}",
            Arc::ptr_eq(&stationary_start, &progressing),
            advanced.scene_built
        );
        assert!(
            observed_alpha.is_some_and(|alpha| alpha > 0.0),
            "stationary admitted time must progress the actual retained clipped hover overlay without another input or forced dirtiness"
        );
        let AnimationSample::Active(value) = harness.animation_context().sample::<f32>(entered)
        else {
            panic!()
        };
        assert!(value > 0.0 && value < 1.0);
        check_overlay(&progressing, Some(value));
        assert_eq!(published(&tree, node).0, value);
        assert!(!Arc::ptr_eq(&stationary_start, &progressing));
        check_stable(&tree, &harness);
        let equal = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!equal.scene_built && !equal.visual_changed);
        assert!(Arc::ptr_eq(
            &progressing,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(selector(&tree, node).needs_update());

        harness.advance_transition_time(0.09);
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        let before_paint = published(&tree, node);
        assert!(before_paint.0 > value && before_paint.0 < 1.0);
        assert!(Arc::ptr_eq(
            &progressing,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        for _ in 0..2 {
            let mut scene = SceneBuilder::new();
            let timeline = Timeline::new();
            let rects = Default::default();
            let mut hits = Vec::new();
            let mut tooltips = Vec::new();
            {
                let mut cx = PaintContext::new(
                    &tree,
                    &mut scene,
                    &theme,
                    &mut text,
                    &icons,
                    0,
                    &timeline,
                    &rects,
                    &mut hits,
                    &mut tooltips,
                    1.0,
                    None,
                    KeyboardAuthorityMode::Navigation,
                );
                let _paint = PaintAuthorityGuard::enter();
                selector(&tree, node).paint(&mut cx, origin, &[]);
            }
            check_overlay(&scene.build().root, Some(before_paint.0));
            assert_eq!(published(&tree, node), before_paint);
            assert_eq!(semantic(&tree), state);
        }
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_overlay(
            &harness.local_fragment(owner).unwrap(),
            Some(before_paint.0),
        );
        check_stable(&tree, &harness);
        harness.advance_transition_time(0.13);
        assert_eq!(
            harness.animation_context().sample::<f32>(entered),
            AnimationSample::Completed(1.0)
        );
        frame(&mut tree, &mut harness, &mut text, &domains);
        assert_eq!(published(&tree, node), (1.0, None));
        check_overlay(&harness.local_fragment(owner).unwrap(), Some(1.0));
        assert!(!selector(&tree, node).needs_update());
        let terminal = harness.local_fragment(owner).unwrap();
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &terminal,
            &harness.local_fragment(owner).unwrap()
        ));

        route(
            outside,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            Some(harness.animation_context()),
        );
        assert_eq!(selector(&tree, node).hovered_index, None);
        let leaving = published(&tree, node).1.unwrap();
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_overlay(&harness.local_fragment(owner).unwrap(), None);
        harness.advance_transition_time(0.17);
        frame(&mut tree, &mut harness, &mut text, &domains);
        let reversing_from = published(&tree, node).0;
        assert!(reversing_from > 0.0 && reversing_from < 1.0);
        route(
            point,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            Some(harness.animation_context()),
        );
        assert_eq!(
            harness.animation_context().sample::<f32>(leaving),
            AnimationSample::Cancelled
        );
        let reversed = published(&tree, node).1.unwrap();
        assert_eq!(
            harness.animation_context().sample::<f32>(reversed),
            AnimationSample::Active(reversing_from)
        );
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_overlay(
            &harness.local_fragment(owner).unwrap(),
            Some(reversing_from),
        );
        harness.advance_transition_time(0.20);
        frame(&mut tree, &mut harness, &mut text, &domains);
        let retained = published(&tree, node).0;
        harness.animation_context().cancel(reversed);
        assert_eq!(
            harness.animation_context().sample::<f32>(reversed),
            AnimationSample::Cancelled
        );
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(published(&tree, node), (retained, None));
        assert!(!selector(&tree, node).needs_update());
        let cancelled = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!cancelled.scene_built && !cancelled.needs_next_frame);
        check_overlay(&harness.local_fragment(owner).unwrap(), Some(retained));

        route(
            outside,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            Some(harness.animation_context()),
        );
        let missed = published(&tree, node).1.unwrap();
        harness.advance_transition_time(0.40);
        harness.advance_transition_time(0.41);
        assert_eq!(
            harness.animation_context().sample::<f32>(missed),
            AnimationSample::Missing
        );
        assert!(publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(published(&tree, node), (0.0, None));
        assert!(!selector(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_overlay(&harness.local_fragment(owner).unwrap(), None);

        route(
            point,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            None,
        );
        assert_eq!(published(&tree, node), (1.0, None));
        assert!(!selector(&tree, node).needs_update());
        frame(&mut tree, &mut harness, &mut text, &domains);
        check_overlay(&harness.local_fragment(owner).unwrap(), Some(1.0));
        route(
            outside,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            Some(harness.animation_context()),
        );
        let event_retiring = published(&tree, node).1.unwrap();
        route(
            point,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            None,
        );
        route(
            outside,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            None,
        );
        route(
            point,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            None,
        );
        assert_eq!(published(&tree, node), (1.0, None));
        assert!(selector(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, None));
        assert!(!publish(&mut tree, node, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(event_retiring),
            AnimationSample::Active(1.0)
        );
        assert!(selector(&tree, node).needs_update());
        route(
            outside,
            &mut tree,
            &mut router,
            &mut text,
            &mut domains,
            &mut bindings,
            Some(harness.animation_context()),
        );
        let replacement = published(&tree, node).1.unwrap();
        assert_ne!(replacement, event_retiring);
        assert_eq!(
            harness.animation_context().sample::<f32>(event_retiring),
            AnimationSample::Cancelled
        );
        assert_eq!(
            harness.animation_context().sample::<f32>(replacement),
            AnimationSample::Active(1.0)
        );
        assert!(publish(&mut tree, node, None));
        assert_eq!(published(&tree, node), (0.0, None));
        assert!(selector(&tree, node).needs_update());
        assert!(!publish(&mut tree, node, None));
        assert_eq!(
            harness.animation_context().sample::<f32>(replacement),
            AnimationSample::Active(1.0)
        );
        assert!(!publish(&mut tree, node, Some(harness.animation_context())));
        assert_eq!(
            harness.animation_context().sample::<f32>(replacement),
            AnimationSample::Cancelled
        );
        assert!(!selector(&tree, node).needs_update());
        assert!(!selector(&tree, node).needs_animation());
        frame(&mut tree, &mut harness, &mut text, &domains);
        let final_owner = harness.local_fragment(owner).unwrap();
        check_overlay(&final_owner, None);
        check_stable(&tree, &harness);
        let idle = frame(&mut tree, &mut harness, &mut text, &domains);
        assert!(!idle.scene_built && !idle.visual_changed && !idle.needs_next_frame);
        assert!(Arc::ptr_eq(
            &final_owner,
            &harness.local_fragment(owner).unwrap()
        ));
        assert!(!router.continuous_frames_requested());
        assert_eq!(domains.primary_state().focus_manager.focused(), Some(owner));
        assert!(
            domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE)
                .is_none()
        );
    }
}
