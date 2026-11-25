// SPEC-HAL-130/131 Stage 2 + Stage 3 + Stage 4: RadioGroup Widget Shell + Member Wiring + Input Mapping + Action Emission
// SPEC-HAL-208: Integrated label support — labels are owned per row, not external siblings.
// Owns RadioGroupState and projects member values through the reducer.

use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, PointerButton, PointerEvent, PointerEventKind,
    PointerType,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{
    EventContext, LayoutContext, PaintContext, UpdateContext, Widget, WidgetDowncast,
};
use crate::core::widget_tree::NodeId;
use crate::model::form_binder::{BindingKey, ChangeWriter};
use crate::utils::id::WidgetInternalId;
use crate::widgets::radio_button::{paint_radio_button, project_radio_button};
use crate::widgets::radio_group::{RadioGroupEvent, RadioGroupState, RadioId, reduce_radio_group};

const OPTION_GAP: f32 = 1.0;

#[derive(Clone, Copy, Debug, PartialEq)]
struct RadioRowLayout {
    top: f32,
    height: f32,
    content_width: f32,
}

/// Group identifier for action emission.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct RadioGroupId(pub u32);

// ============================================================================
// RadioGroup Widget (SPEC-HAL-130/131 Stage 2 + SPEC-HAL-208)
// ============================================================================

/// RadioGroup widget shell.
///
/// Owns the RadioGroupState and a list of member IDs.
/// Members are ordinary values, not embedded Widget instances.
/// All semantic events are dispatched to `reduce_radio_group`.
///
/// SPEC-HAL-208: Each row is a single labeled control. Labels are owned
/// by this group, not by external sibling widgets.
///
/// Stage 4: Emits action via callback on selection delta only.
pub struct RadioGroup {
    internal_id: WidgetInternalId,
    group_id: RadioGroupId,
    state: RadioGroupState,
    members: Vec<RadioId>,
    labels: Vec<Option<String>>,
    /// Layout-owned member geometry, read by paint and pointer input.
    rows: Vec<RadioRowLayout>,
    /// Pointer presentation only; never a reducer or keyboard-focus target.
    hovered: Option<RadioId>,
    hover_dirty: bool,
    /// SPEC-HAL-141: Layout-computed bounds for hit-testing.
    /// NOT authoritative geometry - WidgetNode owns that.
    /// Written: layout phase only. Read: event handlers.
    layout_bounds: Size,
    /// Stage 4: Callback invoked when selection changes.
    /// Called only when reducer returns Some(new_selected).
    on_selection_changed: Option<Box<dyn FnMut(RadioGroupId, Option<RadioId>)>>,
    /// SPEC-HAL-207: Binding key + writer for change store emission.
    binding: Option<(BindingKey, ChangeWriter)>,
}

impl RadioGroup {
    /// Create a new RadioGroup with the given member IDs (no labels).
    pub fn new(members: &[RadioId]) -> Self {
        Self::with_id(RadioGroupId(0), members)
    }

    /// Create a new RadioGroup with explicit group ID (no labels).
    pub fn with_id(group_id: RadioGroupId, members: &[RadioId]) -> Self {
        Self {
            internal_id: WidgetInternalId::new(),
            group_id,
            state: RadioGroupState::default(),
            members: members.to_vec(),
            labels: vec![None; members.len()],
            rows: Vec::new(),
            hovered: None,
            hover_dirty: false,
            layout_bounds: Size::zero(),
            on_selection_changed: None,
            binding: None,
        }
    }

    /// SPEC-HAL-208: Create a RadioGroup with labeled options.
    ///
    /// Each option is a (RadioId, label) pair. Labels are owned by the
    /// group — not external widgets.
    pub fn with_labeled_options(group_id: RadioGroupId, options: &[(RadioId, &str)]) -> Self {
        let members: Vec<RadioId> = options.iter().map(|(id, _)| *id).collect();
        let labels = options
            .iter()
            .map(|(_, label)| Some((*label).to_owned()))
            .collect();
        Self {
            internal_id: WidgetInternalId::new(),
            group_id,
            state: RadioGroupState::default(),
            members,
            labels,
            rows: Vec::new(),
            hovered: None,
            hover_dirty: false,
            layout_bounds: Size::zero(),
            on_selection_changed: None,
            binding: None,
        }
    }

    /// Get the group ID.
    pub fn group_id(&self) -> RadioGroupId {
        self.group_id
    }

    /// Stage 4: Set the selection changed callback.
    ///
    /// Called only when selection actually changes (reducer returns Some).
    /// Payload: (group_id, new_selected).
    pub fn on_selection_changed<F>(mut self, callback: F) -> Self
    where
        F: FnMut(RadioGroupId, Option<RadioId>) + 'static,
    {
        self.on_selection_changed = Some(Box::new(callback));
        self
    }

    /// Stage 4: Set callback via mutable reference (for testing).
    pub fn set_on_selection_changed<F>(&mut self, callback: F)
    where
        F: FnMut(RadioGroupId, Option<RadioId>) + 'static,
    {
        self.on_selection_changed = Some(Box::new(callback));
    }

    /// Get immutable reference to the group state.
    pub fn state(&self) -> &RadioGroupState {
        &self.state
    }

    /// Get the list of member IDs.
    pub fn members(&self) -> &[RadioId] {
        &self.members
    }

    /// SPEC-HAL-207: Configure binding for change store emission.
    ///
    /// On selection change, emits the new selected RadioId (as Option<u32>) into the change store.
    pub fn set_binding(&mut self, key: &str, writer: ChangeWriter) {
        let binding_key = writer.intern(key);
        self.binding = Some((binding_key, writer));
    }

    /// Process a semantic event through the reducer.
    ///
    /// Returns `Some(new_selected)` only when `selected` actually changes.
    /// Returns `None` otherwise.
    ///
    /// Stage 4: Invokes `on_selection_changed` callback when selection changes.
    /// SPEC-HAL-207: Emits into change store when binding is configured.
    pub fn on_semantic_event(&mut self, event: RadioGroupEvent) -> Option<Option<RadioId>> {
        let result = reduce_radio_group(&mut self.state, event);

        // Stage 4: Emit action only when selection actually changed
        if let Some(new_selected) = result {
            if let Some(callback) = &mut self.on_selection_changed {
                callback(self.group_id, new_selected);
            }
            // SPEC-HAL-207: Emit into change store
            if let Some((key, writer)) = &self.binding {
                writer.set_selection(*key, new_selected.map(|id| id.0));
            }
        }

        result
    }

    /// Set the disabled state for the entire group.
    pub fn set_disabled(&mut self, disabled: bool) {
        self.state.disabled = disabled;
        if disabled && self.set_hovered(None) {
            self.hover_dirty = true;
        }
    }

    /// Check if the group is disabled.
    pub fn is_disabled(&self) -> bool {
        self.state.disabled
    }

    /// Get the currently selected member, if any.
    pub fn selected(&self) -> Option<RadioId> {
        self.state.selected
    }

    /// Set the initial selection (bypasses reducer, for initialization only).
    pub fn set_selected(&mut self, selected: Option<RadioId>) {
        self.state.selected = selected;
    }

    // ========================================================================
    // Stage 3: Input Mapping
    // ========================================================================

    /// Handle an input event and translate to semantic events.
    ///
    /// Returns true if the event was handled.
    pub fn handle_event(&mut self, event: &Event, rect: RectF) -> bool {
        if self.state.disabled {
            return false;
        }

        match event {
            Event::Pointer(ptr) => self.handle_pointer(ptr, rect),
            Event::Key(key) => self.handle_keyboard(key),
            _ => false,
        }
    }

    /// Use the same layout-owned row bounds for both pointer entry points.
    fn hit_test_member(&self, position: (f32, f32), rect: RectF) -> Option<RadioId> {
        let x = position.0 - rect.x;
        let y = position.1 - rect.y;
        let width = self.layout_bounds.width.min(rect.width);
        let height = self.layout_bounds.height.min(rect.height);
        if !(x >= 0.0 && y >= 0.0 && y < height) {
            return None;
        }
        self.rows.iter().enumerate().find_map(|(index, row)| {
            // Preserve the preceding row's ownership of the inter-row gap.
            let bottom = self
                .rows
                .get(index + 1)
                .map_or(row.top + row.height, |next| next.top);
            if x < row.content_width.min(width) && y >= row.top && y < bottom {
                self.members.get(index).copied()
            } else {
                None
            }
        })
    }

    fn set_hovered(&mut self, hovered: Option<RadioId>) -> bool {
        let hovered = if self.state.disabled { None } else { hovered };
        if self.hovered == hovered {
            return false;
        }
        self.hovered = hovered;
        true
    }

    /// Handle pointer events.
    fn handle_pointer(&mut self, event: &PointerEvent, rect: RectF) -> bool {
        if event.pointer_type == PointerType::Mouse && event.kind == PointerEventKind::Move {
            if self.set_hovered(self.hit_test_member(event.local_pos, rect)) {
                self.hover_dirty = true;
            }
            return false;
        }
        if event.pointer_type == PointerType::Mouse && event.button != Some(PointerButton::Primary)
        {
            return false;
        }

        if event.pointer_type == PointerType::Mouse && event.kind == PointerEventKind::Up {
            if let Some(active) = self.state.active {
                self.on_semantic_event(RadioGroupEvent::Cancel(active));
            }
            return true;
        }

        let id = match self.hit_test_member(event.local_pos, rect) {
            Some(id) => id,
            None => return false,
        };

        match event.kind {
            PointerEventKind::Down => {
                self.on_semantic_event(RadioGroupEvent::FocusGain(id));
                if event.pointer_type == PointerType::Mouse {
                    self.on_semantic_event(RadioGroupEvent::Select(id));
                }
                self.on_semantic_event(RadioGroupEvent::Press(id));
                true
            }
            PointerEventKind::Up => {
                if self.state.active == Some(id) {
                    self.on_semantic_event(RadioGroupEvent::Activate(id));
                }
                true
            }
            _ => false,
        }
    }

    /// Handle keyboard events.
    fn handle_keyboard(&mut self, event: &KeyEvent) -> bool {
        if !event.is_down {
            return false;
        }

        match event.key_code {
            KeyCode::Up => {
                if let Some(next) = self.prev_member() {
                    self.on_semantic_event(RadioGroupEvent::FocusGain(next));
                    true
                } else {
                    false
                }
            }
            KeyCode::Down => {
                if let Some(next) = self.next_member() {
                    self.on_semantic_event(RadioGroupEvent::FocusGain(next));
                    true
                } else {
                    false
                }
            }
            KeyCode::Space | KeyCode::Enter => {
                if let Some(focused) = self.state.focused {
                    self.on_semantic_event(RadioGroupEvent::Activate(focused));
                    true
                } else {
                    false
                }
            }
            KeyCode::Escape => {
                if let Some(focused) = self.state.focused {
                    self.on_semantic_event(RadioGroupEvent::Cancel(focused));
                    true
                } else {
                    false
                }
            }
            _ => false,
        }
    }

    /// Get the previous member in the list. Returns None at the boundary
    /// so the framework can navigate away from the group.
    fn prev_member(&self) -> Option<RadioId> {
        if self.members.is_empty() {
            return None;
        }
        let current_index = self
            .state
            .focused
            .and_then(|f| self.members.iter().position(|&m| m == f))
            .unwrap_or(0);

        if current_index == 0 {
            None // At boundary — let framework navigate away
        } else {
            Some(self.members[current_index - 1])
        }
    }

    /// Get the next member in the list. Returns None at the boundary
    /// so the framework can navigate away from the group.
    fn next_member(&self) -> Option<RadioId> {
        if self.members.is_empty() {
            return None;
        }
        let current_index = self
            .state
            .focused
            .and_then(|f| self.members.iter().position(|&m| m == f))
            .unwrap_or(self.members.len() - 1);

        let next_index = current_index + 1;
        if next_index >= self.members.len() {
            None // At boundary — let framework navigate away
        } else {
            Some(self.members[next_index])
        }
    }
}

impl Default for RadioGroup {
    fn default() -> Self {
        Self::with_id(RadioGroupId(0), &[])
    }
}

impl WidgetDowncast for RadioGroup {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for RadioGroup {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let indicator_size = cx.theme.geometry.checkbox.size;
        let label_gap = cx.theme.geometry.checkbox.label_gap;
        let font_size = cx.theme.typography.font_size;
        self.rows.clear();
        let mut max_content_w: f32 = 0.0;
        let mut total_height = 0.0;
        for label in &self.labels {
            let (content_width, row_height) = if let Some(label) = label {
                let (text_width, text_height) = cx.text_engine.measure(label, font_size);
                let line_height = cx.text_engine.font_metrics(font_size).line_height;
                (
                    indicator_size + label_gap + text_width,
                    indicator_size.max(text_height).max(line_height),
                )
            } else {
                (indicator_size, indicator_size)
            };
            if !self.rows.is_empty() {
                total_height += OPTION_GAP;
            }
            self.rows.push(RadioRowLayout {
                top: total_height,
                height: row_height,
                content_width,
            });
            total_height += row_height;
            max_content_w = max_content_w.max(content_width);
        }
        let height = total_height.clamp(constraints.min_height, constraints.max_height);
        // Flex measures non-growing row children with an unbounded main axis.
        // Returning that unbounded width leaves a painted label with a zero-sized
        // hit-test rect after Flex normalizes the measurement. The labeled group
        // has a finite natural width; each row retains its own content target.
        let width = max_content_w.clamp(constraints.min_width, constraints.max_width);

        // Cache bounds for hit-testing
        self.layout_bounds = Size::new(width, height);
        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Project member values without allocating widget identity or lifecycle.
        for ((&id, label), row) in self.members.iter().zip(&self.labels).zip(&self.rows) {
            let button_rect = RectF::new(rect.x, rect.y + row.top, row.content_width, row.height);
            paint_radio_button(
                cx,
                button_rect,
                label.as_deref(),
                &project_radio_button(id, &self.state),
                if self.hovered == Some(id) && !self.state.disabled {
                    1.0
                } else {
                    0.0
                },
                0.0,
            );
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.internal_id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Only handle events in Target phase
        if phase != EventPhase::Target {
            return false;
        }

        if self.state.disabled {
            return false;
        }

        match event {
            Event::Pointer(pe) => {
                if pe.pointer_type == PointerType::Mouse && pe.kind == PointerEventKind::Move {
                    let hovered = self.hit_test_member(
                        pe.local_pos,
                        RectF::new(
                            0.0,
                            0.0,
                            self.layout_bounds.width,
                            self.layout_bounds.height,
                        ),
                    );
                    if self.set_hovered(hovered) {
                        cx.invalidate_paint();
                    }
                    return false;
                }
                if pe.pointer_type == PointerType::Mouse
                    && pe.kind == PointerEventKind::Up
                    && pe.button == Some(PointerButton::Primary)
                {
                    if let Some(active) = self.state.active {
                        self.on_semantic_event(RadioGroupEvent::Cancel(active));
                    }
                    return true;
                }

                let id = match self.hit_test_member(
                    pe.local_pos,
                    RectF::new(
                        0.0,
                        0.0,
                        self.layout_bounds.width,
                        self.layout_bounds.height,
                    ),
                ) {
                    Some(id) => id,
                    None => return false,
                };

                match pe.kind {
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick => {
                        // Only react to primary (left) button
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        self.on_semantic_event(RadioGroupEvent::FocusGain(id));
                        if pe.pointer_type == PointerType::Mouse {
                            self.on_semantic_event(RadioGroupEvent::Select(id));
                        }
                        self.on_semantic_event(RadioGroupEvent::Press(id));
                        cx.capture_pointer(PointerButton::Primary);
                        true
                    }
                    PointerEventKind::Up => {
                        // Only react to primary (left) button
                        if pe.button != Some(PointerButton::Primary) {
                            return false;
                        }
                        if self.state.active == Some(id) {
                            self.on_semantic_event(RadioGroupEvent::Activate(id));
                        }
                        true
                    }
                    _ => false,
                }
            }
            Event::Key(ke) if ke.is_down => match ke.key_code {
                KeyCode::Up => {
                    if let Some(next) = self.prev_member() {
                        self.on_semantic_event(RadioGroupEvent::FocusGain(next));
                        cx.request_continuous_frames();
                        true
                    } else {
                        false
                    }
                }
                KeyCode::Down => {
                    if let Some(next) = self.next_member() {
                        self.on_semantic_event(RadioGroupEvent::FocusGain(next));
                        cx.request_continuous_frames();
                        true
                    } else {
                        false
                    }
                }
                KeyCode::Escape => {
                    // Category B: never consume Escape.
                    // Framework handles defocus (SPEC-HAL-328 INV-FC-5).
                    false
                }
                _ => false,
            },
            _ => false,
        }
    }

    fn on_pointer_capture_cancel(
        &mut self,
        _cx: &mut EventContext,
        pointer_id: crate::core::event::PointerId,
        _reason: crate::core::event::CancellationReason,
    ) {
        if pointer_id != crate::core::event::PointerId::MOUSE {
            return;
        }

        if let Some(active) = self.state.active {
            self.on_semantic_event(RadioGroupEvent::Cancel(active));
        }
    }

    fn on_hover_leave(&mut self, cx: &mut EventContext) {
        if self.set_hovered(None) {
            cx.invalidate_paint();
        }
    }

    fn focusable(&self) -> bool {
        !self.state.disabled && !self.members.is_empty()
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        // SPEC-HAL-326 §12.x INV-F9: Directional focus entry.
        // When the framework hands us focus via directional keyboard
        // navigation, pick the internal member by entry direction:
        //   - Up   (entering from below) → bottommost member.
        //   - Down (entering from above) → topmost member.
        //   - Left/Right or non-directional (Tab/click/programmatic)
        //     → selected member if any, else first.
        //
        // Members are stored in display order (top-to-bottom for the
        // current single-column RadioGroup layout). Left/Right falls
        // back to the default because multi-column layout does not
        // yet exist; when it does, this match expands.
        use crate::core::event::Direction;
        let target = match cx.focus_entry_direction() {
            Some(Direction::Up) => self.members.last().copied(),
            Some(Direction::Down) => self.members.first().copied(),
            Some(Direction::Left) | Some(Direction::Right) | None => self
                .state
                .selected
                .or_else(|| self.members.first().copied()),
        };
        if let Some(id) = target {
            self.on_semantic_event(RadioGroupEvent::FocusGain(id));
        }
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        // When group loses focus, clear internal focus
        if let Some(focused) = self.state.focused {
            self.on_semantic_event(RadioGroupEvent::FocusLoss(focused));
        }
    }

    fn update(&mut self, _dt: f32, cx: &mut UpdateContext) {
        if std::mem::take(&mut self.hover_dirty) {
            cx.invalidate_paint();
        }
    }

    fn needs_update(&self) -> bool {
        self.hover_dirty
    }

    fn on_mount(&mut self) {
        self.hovered = None;
        self.hover_dirty = false;
    }

    fn on_unmount(&mut self) {
        self.hovered = None;
        self.hover_dirty = false;
    }

    fn accepts_keyboard_activation(&self) -> bool {
        !self.state.disabled && !self.members.is_empty()
    }

    fn touch_target(&self, hit: crate::core::event::TouchHit) -> Option<crate::core::event::TouchTarget> {
        if self.state.disabled {
            return None;
        }
        self.hit_test_member(
            (hit.local_position.x, hit.local_position.y),
            RectF::new(0.0, 0.0, self.layout_bounds.width, self.layout_bounds.height),
        )
        .map(|id| crate::core::event::TouchTarget::Item(u64::from(id.0)))
    }

    fn on_touch_tap(&mut self, cx: &mut EventContext, tap: crate::core::event::TouchTap) -> bool {
        let crate::core::event::TouchTarget::Item(member) = tap.target else {
            return false;
        };
        let id = RadioId(member as u32);
        self.on_semantic_event(RadioGroupEvent::FocusGain(id));
        self.on_semantic_event(RadioGroupEvent::Activate(id));
        cx.invalidate_paint();
        true
    }

    fn on_keyboard_activate(&mut self, cx: &mut EventContext) {
        // Activate the focused member
        if let Some(focused) = self.state.focused {
            let result = self.on_semantic_event(RadioGroupEvent::Activate(focused));
            if result.is_some() {
                cx.invalidate_paint();
            }
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "RadioGroup"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::animation::Timeline;
    use crate::core::text_engine::{TextEngine, compute_control_text_baseline_y};
    use crate::core::theme::Theme;
    use crate::core::widget_tree::WidgetTree;
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::widgets::radio_button::tests::{radio_event, radio_paint};

    #[test]
    fn radio_group_owns_labels_and_preserves_measurement_stride_and_emission() {
        use crate::core::app::{AppContext, ModalStack, ScreenContext, ScreenStack};
        use crate::model::form_binder::FormBinder;
        use std::cell::Cell;
        use std::rc::Rc;

        let ids = [RadioId(3), RadioId(1)];
        let mut owned = String::from("Owned choice");
        let mut group =
            RadioGroup::with_labeled_options(RadioGroupId(4), &[(ids[0], &owned), (ids[1], "")]);
        owned.clear();
        assert_eq!(group.members(), &ids);
        assert_eq!(
            group.labels,
            [Some("Owned choice".to_owned()), Some(String::new())]
        );
        let mut unlabeled = RadioGroup::new(&ids);
        assert_eq!(unlabeled.labels, [None, None]);
        let mut empty_label = RadioGroup::with_labeled_options(RadioGroupId(4), &[(ids[0], "")]);
        let theme = Theme::default();
        let mut tree = WidgetTree::new();
        let mut text = FontManagerTextEngine::new();
        let font_size = theme.typography.font_size;
        let line_height = text.font_metrics(font_size).line_height;
        let first_height = theme
            .geometry
            .checkbox
            .size
            .max(text.measure("Owned choice", font_size).1)
            .max(line_height);
        let empty_height = theme
            .geometry
            .checkbox
            .size
            .max(text.measure("", font_size).1)
            .max(line_height);
        let mut cx = LayoutContext::new(&mut tree, &mut text, &theme);
        let size = group.layout(&mut cx, Constraints::unbounded(), &[]).size;
        let no_label = unlabeled
            .layout(&mut cx, Constraints::unbounded(), &[])
            .size;
        let empty = empty_label
            .layout(&mut cx, Constraints::unbounded(), &[])
            .size;
        let indicator = theme.geometry.checkbox.size;
        assert!(size.width.is_finite() && size.width > no_label.width);
        assert_eq!(size.height, first_height + OPTION_GAP + empty_height);
        assert_eq!(group.rows[0].top, 0.0);
        assert_eq!(group.rows[0].height, first_height);
        assert_eq!(group.rows[1].top, first_height + OPTION_GAP);
        assert_eq!(group.rows[1].height, empty_height);
        assert_eq!(no_label.width, indicator);
        assert_eq!(no_label.height, indicator * 2.0 + OPTION_GAP);
        assert_eq!(empty.width, indicator + theme.geometry.checkbox.label_gap);
        assert_eq!(empty.height, empty_height);
        assert_eq!(
            group.hit_test_member(
                (0.0, first_height + OPTION_GAP),
                RectF::new(0.0, 0.0, size.width, size.height)
            ),
            Some(ids[1])
        );

        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        group.set_on_selection_changed(move |_, _| observed.set(observed.get() + 1));
        let mut binder = FormBinder::new();
        group.set_binding("choice", binder.writer());
        group.on_semantic_event(RadioGroupEvent::Select(ids[1]));
        group.on_semantic_event(RadioGroupEvent::FocusGain(ids[0]));
        let mut app = AppContext::new(theme.clone());
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        let screen = ScreenContext::new(&mut app, &mut screens, &mut modals, None);
        binder.bind(&screen, |binding| {
            assert_eq!(binding.selection("choice").take(), Some(Some(ids[1].0)))
        });
        let snapshot = group.state().clone();
        let nodes = radio_paint(&Timeline::new(), &theme, |cx| {
            group.paint(cx, RectF::new(0.0, 0.0, size.width, size.height), &[])
        });
        assert_eq!(group.state(), &snapshot);
        assert_eq!(changes.get(), 1);
        binder.bind(&screen, |binding| {
            assert_eq!(binding.selection("choice").take(), None)
        });
        let labels: Vec<_> = nodes
            .iter()
            .filter_map(|node| match node {
                crate::core::scene::SceneNode::Text(text) => Some(text.text.as_str()),
                _ => None,
            })
            .collect();
        assert!(labels.contains(&"Owned choice"));
        assert!(
            !nodes.iter().any(|node| matches!(
                node,
                crate::core::scene::SceneNode::BorderRect(border)
                    if border.z_index == crate::core::paint::LAYER_FOCUS
            )),
            "projected member focus requires a current framework owner"
        );
    }

    #[test]
    fn radio_group_rows_measure_and_paint_mixed_owned_labels() {
        use crate::core::scene::SceneNode;

        let mut theme = Theme::default();
        theme.typography.font_size = 32.0;
        let indicator = theme.geometry.checkbox.size;
        let label_gap = theme.geometry.checkbox.label_gap;
        let font_size = theme.typography.font_size;
        let long_label = "A longer owned radio label";
        let mut group = RadioGroup::new(&[RadioId(1), RadioId(2), RadioId(3)]);
        group.labels = vec![None, Some(String::new()), Some(long_label.to_owned())];
        let mut text = FontManagerTextEngine::new();
        let metrics = text.font_metrics(font_size);
        let (empty_width, empty_height) = text.measure("", font_size);
        let (long_width, long_height) = text.measure(long_label, font_size);
        let empty_height = indicator.max(empty_height).max(metrics.line_height);
        let long_height = indicator.max(long_height).max(metrics.line_height);
        assert!(empty_height > indicator);
        let expected = vec![
            RadioRowLayout {
                top: 0.0,
                height: indicator,
                content_width: indicator,
            },
            RadioRowLayout {
                top: indicator + OPTION_GAP,
                height: empty_height,
                content_width: indicator + label_gap + empty_width,
            },
            RadioRowLayout {
                top: indicator + OPTION_GAP + empty_height + OPTION_GAP,
                height: long_height,
                content_width: indicator + label_gap + long_width,
            },
        ];
        let mut tree = WidgetTree::new();
        let size = group
            .layout(
                &mut LayoutContext::new(&mut tree, &mut text, &theme),
                Constraints::unbounded(),
                &[],
            )
            .size;
        assert_eq!(group.rows, expected);
        assert_eq!(
            size,
            Size::new(expected[2].content_width, expected[2].top + long_height)
        );
        group.set_selected(Some(RadioId(3)));
        group.on_focus(&mut radio_event(None));
        let semantic = group.state().clone();
        let identity = group.id();
        let origin = RectF::new(17.0, 29.0, size.width + 100.0, size.height);
        let nodes = radio_paint(&Timeline::new(), &theme, |cx| group.paint(cx, origin, &[]));
        assert_eq!(group.rows, expected);
        assert_eq!(group.state(), &semantic);
        assert_eq!(group.id(), identity);
        let circles: Vec<_> = nodes
            .iter()
            .filter_map(|node| match node {
                SceneNode::RoundedRect(circle)
                    if circle.z_index == 2 && circle.rect.width == indicator =>
                {
                    Some(circle.rect)
                }
                _ => None,
            })
            .collect();
        assert_eq!(circles.len(), 3);
        for (circle, row) in circles.iter().zip(&expected) {
            assert_eq!(
                *circle,
                RectF::new(
                    origin.x,
                    origin.y + row.top + (row.height - indicator) / 2.0,
                    indicator,
                    indicator
                )
            );
        }
        let label = nodes
            .iter()
            .find_map(|node| match node {
                SceneNode::Text(text) if text.text == long_label => Some(text),
                _ => None,
            })
            .unwrap();
        let baseline = origin.y
            + expected[2].top
            + compute_control_text_baseline_y(
                expected[2].height,
                metrics.ascent,
                metrics.descent,
                theme.typography.baseline_offset,
            );
        assert_eq!(label.position.x, origin.x + indicator + label_gap);
        assert_eq!(label.position.y, baseline - metrics.ascent);
        let focus: Vec<_> = nodes
            .iter()
            .filter_map(|node| match node {
                SceneNode::BorderRect(border)
                    if border.z_index == crate::core::paint::LAYER_FOCUS =>
                {
                    Some(border.rect)
                }
                _ => None,
            })
            .collect();
        assert!(
            focus.is_empty(),
            "projected member focus requires a current framework owner"
        );

        let mut empty = RadioGroup::default();
        assert_eq!(
            empty
                .layout(
                    &mut LayoutContext::new(&mut tree, &mut text, &theme),
                    Constraints::unbounded(),
                    &[],
                )
                .size,
            Size::zero()
        );
        assert!(empty.rows.is_empty());
    }

    #[test]
    fn radio_group_hover_coexists_with_routed_keyboard_and_pointer_driven_policy() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, Modifiers, PhysicalKeyCode,
            PointerButtonsState, PointerDevice, PointerId, UiInteractionPolicy, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::Alignment;
        use crate::core::math::Point;
        use crate::core::scene::SceneNode;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use std::cell::Cell;
        use std::rc::Rc;
        use std::sync::Arc;
        use std::time::Instant;

        for theme in [Theme::light(), Theme::dark()] {
            for policy in [
                UiInteractionPolicy::NavigationAndCapture,
                UiInteractionPolicy::PointerDriven,
            ] {
                let navigation = policy == UiInteractionPolicy::NavigationAndCapture;
                let changes = Rc::new(Cell::new(0));
                let observed = changes.clone();
                let mut group = RadioGroup::with_labeled_options(
                    RadioGroupId(1),
                    &[
                        (RadioId(1), "Mouse"),
                        (RadioId(2), "Keyboard"),
                        (RadioId(3), "Selected"),
                    ],
                )
                .on_selection_changed(move |_, _| observed.set(observed.get() + 1));
                group.set_selected(Some(RadioId(3)));
                let mut tree = WidgetTree::new();
                let root = tree.create_root(
                    Box::new(Flex::column().with_align_items(Alignment::Stretch)),
                    None,
                    0,
                );
                let group_node = tree.add_child(root, Box::new(group), None, 0);
                let group_id = tree.node(group_node).unwrap().id;
                let mut domains = InteractionDomains::with_ui_interaction_policy(policy);
                let mut router = EventRouter::new();
                let mut bindings = BindingRegistry::new();
                let mut text = FontManagerTextEngine::new();
                let icons = crate::core::icons::builtin_registry();
                let constraints = Constraints::tight(Size::new(400.0, 200.0));
                let mut harness = RetainedSceneTestHarness::new();
                harness.frame_with_appearance(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                );
                let rect = tree.get_absolute_paint_rect(group_node).unwrap();
                let node = tree.node(group_node).unwrap();
                let group = node.widget.as_any().downcast_ref::<RadioGroup>().unwrap();
                let rows = group.rows.clone();
                let identity = (node.id, group.id(), node.layout_epoch);
                if navigation {
                    router.request_focus_for_widget(
                        &mut tree,
                        group_id,
                        domains.primary_state_mut(),
                    );
                }
                let time = Instant::now();
                for event in [
                    FusedInputEvent::PointerMove {
                        window_id: WindowId::PRIMARY,
                        time,
                        pos: Point::new(rect.x + 1.0, rect.y + rows[0].height / 2.0),
                        delta: (0.0, 0.0),
                        device: PointerDevice::Mouse,
                        buttons: PointerButtonsState {
                            primary: false,
                            secondary: false,
                            auxiliary: false,
                        },
                    },
                    FusedInputEvent::KeyDown {
                        window_id: WindowId::PRIMARY,
                        time,
                        key_code: KeyCode::Up,
                        physical_key: PhysicalKeyCode::ArrowUp,
                        reservation: None,
                        modifiers: Modifiers::default(),
                        repeat: false,
                    },
                ] {
                    router.route_input_event(
                        &mut tree,
                        event,
                        Some(root),
                        &mut text,
                        Some(harness.animation_context()),
                        domains.primary_state_mut(),
                        &mut bindings,
                    );
                }
                let node = tree.node(group_node).unwrap();
                let group = node.widget.as_any().downcast_ref::<RadioGroup>().unwrap();
                assert_eq!(group.hovered, Some(RadioId(1)));
                assert_eq!(group.state().focused, navigation.then_some(RadioId(2)));
                assert_eq!(group.selected(), Some(RadioId(3)));
                assert_eq!(group.state().active, None);
                assert_eq!(changes.get(), 0);
                assert_eq!(
                    domains.primary_state().focus_manager.focused(),
                    navigation.then_some(group_id)
                );
                assert_eq!(
                    domains.primary_state().keyboard_mode,
                    KeyboardAuthorityMode::Navigation
                );
                assert!(
                    domains
                        .primary_state()
                        .pointer_capture_manager
                        .current_capture(PointerId::MOUSE)
                        .is_none()
                );
                assert!(!node.dirty_layout);
                let frame = harness.frame_with_appearance(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                );
                assert!(frame.scene_built);
                let fragment = harness.local_fragment(group_id).unwrap();
                let borders: Vec<_> = fragment
                    .iter()
                    .filter_map(|node| match node {
                        SceneNode::RoundedRect(circle)
                            if circle.z_index == 2
                                && circle.rect.width == theme.geometry.checkbox.size =>
                        {
                            Some(circle.color)
                        }
                        _ => None,
                    })
                    .collect();
                let highlight = crate::core::scene::Color::from(theme.colors.borders.focus);
                let resting = crate::core::scene::Color::from(theme.colors.borders.default);
                assert_eq!(borders.len(), 3);
                for (actual, expected) in borders.iter().zip([
                    highlight,
                    if navigation { highlight } else { resting },
                    resting,
                ]) {
                    for (actual, expected) in [actual.r, actual.g, actual.b, actual.a]
                        .into_iter()
                        .zip([expected.r, expected.g, expected.b, expected.a])
                    {
                        assert!(
                            (actual - expected).abs() <= f32::EPSILON,
                            "hover and keyboard border highlights differ: {actual} versus {expected}"
                        );
                    }
                }
                let cues: Vec<_> = fragment
                    .iter()
                    .filter_map(|node| match node {
                        SceneNode::BorderRect(border)
                            if border.z_index == crate::core::paint::LAYER_FOCUS =>
                        {
                            Some(border.rect)
                        }
                        _ => None,
                    })
                    .collect();
                assert!(
                    cues.is_empty(),
                    "default radio options emit no enclosing rectangle"
                );
                let dots = fragment
                    .iter()
                    .filter_map(|node| match node {
                        SceneNode::RoundedRect(dot) if dot.z_index == 4 => Some(dot),
                        _ => None,
                    })
                    .collect::<Vec<_>>();
                assert_eq!(dots.len(), 1);
                let inset = theme.geometry.checkbox.checkmark_inset;
                let size = theme.geometry.checkbox.size;
                assert_eq!(
                    dots[0].rect,
                    RectF::new(
                        rect.x + inset,
                        rect.y + rows[2].top + (rows[2].height - size) * 0.5 + inset,
                        size - 2.0 * inset,
                        size - 2.0 * inset,
                    )
                );
                assert_eq!(dots[0].color, theme.colors.accents.primary.into());
                let node = tree.node(group_node).unwrap();
                assert_eq!((node.id, node.widget.id(), node.layout_epoch), identity);
                assert_eq!(tree.get_absolute_paint_rect(group_node).unwrap(), rect);
                let settled = harness.frame_with_appearance(
                    &mut tree,
                    root,
                    constraints,
                    &theme,
                    &mut text,
                    &icons,
                    &domains,
                    WindowId::PRIMARY,
                );
                assert!(!settled.scene_built);
                assert!(!settled.needs_next_frame);
                assert!(Arc::ptr_eq(
                    &harness.local_fragment(group_id).unwrap(),
                    &fragment
                ));
            }
        }
    }

    #[test]
    fn radio_group_hover_routes_rows_and_invalidates_only_changed_retained_paint() {
        use crate::core::app::{
            AppContext, ModalStack, RetainedSceneTestHarness, ScreenContext, ScreenStack,
        };
        use crate::core::event::{
            FusedInputEvent, InteractionDomains, KeyboardAuthorityMode, PointerButtonsState,
            PointerDevice, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::layout::Alignment;
        use crate::core::math::Point;
        use crate::core::scene::SceneNode;
        use crate::model::form_binder::FormBinder;
        use crate::runtime::binding_registry::BindingRegistry;
        use crate::widgets::flex::Flex;
        use crate::widgets::label::Label;
        use std::cell::Cell;
        use std::rc::Rc;
        use std::sync::Arc;
        use std::time::Instant;

        let theme = Theme::dark();
        let mut text = FontManagerTextEngine::new();
        let mut group = RadioGroup::with_labeled_options(
            RadioGroupId(1),
            &[
                (RadioId(1), "Short"),
                (RadioId(2), "A longer keyboard target"),
            ],
        );
        group.set_selected(Some(RadioId(2)));
        group.on_semantic_event(RadioGroupEvent::FocusGain(RadioId(2)));
        let semantic = group.state().clone();
        let changes = Rc::new(Cell::new(0));
        let observed = changes.clone();
        group.set_on_selection_changed(move |_, _| observed.set(observed.get() + 1));
        let mut binder = FormBinder::new();
        group.set_binding("choice", binder.writer());
        let mut tree = WidgetTree::new();
        let root = tree.create_root(
            Box::new(Flex::column().with_align_items(Alignment::Stretch)),
            None,
            0,
        );
        let group_node = tree.add_child(root, Box::new(group), None, 0);
        let sibling = tree.add_child(root, Box::new(Label::new("Unchanged sibling")), None, 0);
        let group_id = tree.node(group_node).unwrap().id;
        let root_id = tree.node(root).unwrap().id;
        let sibling_id = tree.node(sibling).unwrap().id;
        let mut domains = InteractionDomains::new();
        domains
            .primary_state_mut()
            .focus_manager
            .set_focus(Some(group_id));
        let mut router = EventRouter::new();
        let mut bindings = BindingRegistry::new();
        let icons = crate::core::icons::builtin_registry();
        let constraints = Constraints::tight(Size::new(400.0, 200.0));
        let mut harness = RetainedSceneTestHarness::new();
        let initial = harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut text,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        assert!(initial.scene_built);
        let node = tree.node(group_node).unwrap();
        let group = node.widget.as_any().downcast_ref::<RadioGroup>().unwrap();
        let rows = group.rows.clone();
        let geometry = (
            node.id,
            group.id(),
            tree.get_rect(group_node).unwrap(),
            node.layout_epoch,
        );
        let origin = tree.get_absolute_paint_rect(group_node).unwrap();
        assert!(origin.width > rows[1].content_width);
        let baseline = harness.local_fragment(group_id).unwrap();
        let sibling_fragment = harness.local_fragment(sibling_id).unwrap();
        let root_fragment = harness.local_fragment(root_id).unwrap();
        let focus_count = baseline.iter().filter(|node| matches!(node, SceneNode::BorderRect(border) if border.z_index == crate::core::paint::LAYER_FOCUS)).count();
        assert_eq!(
            focus_count, 0,
            "default radio options emit no enclosing rectangle"
        );
        let mut previous = baseline.clone();
        let time = Instant::now();
        for (x, y, expected, changed) in [
            (1.0, rows[0].height / 2.0, Some(RadioId(1)), true),
            (
                rows[0].content_width - 1.0,
                rows[0].height / 2.0,
                Some(RadioId(1)),
                false,
            ),
            (
                1.0,
                rows[0].height + OPTION_GAP / 2.0,
                Some(RadioId(1)),
                false,
            ),
            (
                1.0,
                rows[1].top + rows[1].height / 2.0,
                Some(RadioId(2)),
                true,
            ),
            (
                rows[0].content_width + 1.0,
                rows[0].height / 2.0,
                None,
                true,
            ),
            (1.0, rows[0].height / 2.0, Some(RadioId(1)), true),
            (-1.0, -1.0, None, true),
            (1.0, rows[0].height / 2.0, Some(RadioId(1)), true),
        ] {
            router.route_input_event(
                &mut tree,
                FusedInputEvent::PointerMove {
                    window_id: WindowId::PRIMARY,
                    time,
                    pos: Point::new(origin.x + x, origin.y + y),
                    delta: (0.0, 0.0),
                    device: PointerDevice::Mouse,
                    buttons: PointerButtonsState {
                        primary: false,
                        secondary: false,
                        auxiliary: false,
                    },
                },
                Some(root),
                &mut text,
                None,
                domains.primary_state_mut(),
                &mut bindings,
            );
            let node = tree.node(group_node).unwrap();
            let group = node.widget.as_any().downcast_ref::<RadioGroup>().unwrap();
            assert_eq!(group.hovered, expected);
            assert_eq!(group.state(), &semantic);
            assert_eq!(node.dirty_paint, changed);
            assert!(!node.dirty_layout);
            assert_eq!(tree.node(root).unwrap().dirty_composition, changed);
            assert_eq!(
                domains.primary_state().focus_manager.focused(),
                Some(group_id)
            );
            assert_eq!(
                domains.primary_state().keyboard_mode,
                KeyboardAuthorityMode::Navigation
            );
            let frame = harness.frame_with_appearance(
                &mut tree,
                root,
                constraints,
                &theme,
                &mut text,
                &icons,
                &domains,
                WindowId::PRIMARY,
            );
            assert_eq!(frame.scene_built, changed);
            assert!(!frame.needs_next_frame);
            let fragment = harness.local_fragment(group_id).unwrap();
            if !changed {
                assert!(Arc::ptr_eq(&fragment, &previous));
                assert!(!frame.visual_changed);
            }
            let borders: Vec<_> = fragment
                .iter()
                .filter_map(|node| match node {
                    SceneNode::RoundedRect(circle)
                        if circle.z_index == 2
                            && circle.rect.width == theme.geometry.checkbox.size =>
                    {
                        Some(circle.color)
                    }
                    _ => None,
                })
                .collect();
            assert_eq!(borders.len(), 2);
            assert_eq!(
                borders[0],
                crate::core::scene::Color::from(theme.colors.borders.default.lerp(
                    theme.colors.borders.focus,
                    if expected == Some(RadioId(1)) {
                        1.0
                    } else {
                        0.0
                    }
                ))
            );
            assert_eq!(
                borders[1],
                crate::core::scene::Color::from(theme.colors.borders.focus)
            );
            assert_eq!(fragment.iter().filter(|node| matches!(node, SceneNode::BorderRect(border) if border.z_index == crate::core::paint::LAYER_FOCUS)).count(), focus_count, "hover must not add a keyboard-focus cue");
            assert_eq!(
                fragment
                    .iter()
                    .filter(|node| matches!(node, SceneNode::RoundedRect(dot) if dot.z_index == 4))
                    .count(),
                1
            );
            assert!(Arc::ptr_eq(
                &harness.local_fragment(sibling_id).unwrap(),
                &sibling_fragment
            ));
            assert!(Arc::ptr_eq(
                &harness.local_fragment(root_id).unwrap(),
                &root_fragment
            ));
            let node = tree.node(group_node).unwrap();
            assert_eq!(
                (
                    node.id,
                    node.widget.id(),
                    tree.get_rect(group_node).unwrap(),
                    node.layout_epoch
                ),
                geometry
            );
            assert_eq!(
                node.widget
                    .as_any()
                    .downcast_ref::<RadioGroup>()
                    .unwrap()
                    .state(),
                &semantic
            );
            previous = fragment;
        }

        // Disabling retires only the new hover dependency through ordinary Update.
        let group = tree
            .node_mut(group_node)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<RadioGroup>()
            .unwrap();
        group.set_disabled(true);
        assert_eq!(group.hovered, None);
        assert!(group.needs_update());
        let disabled = harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut text,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        assert!(disabled.scene_built);
        assert!(!disabled.needs_next_frame);
        assert!(!tree.node(group_node).unwrap().widget.needs_update());
        let disabled_fragment = harness.local_fragment(group_id).unwrap();
        let borders: Vec<_> = disabled_fragment
            .iter()
            .filter_map(|node| match node {
                SceneNode::RoundedRect(circle)
                    if circle.z_index == 2 && circle.rect.width == theme.geometry.checkbox.size =>
                {
                    Some(circle.color)
                }
                _ => None,
            })
            .collect();
        assert_eq!(
            borders,
            vec![crate::core::scene::Color::from(theme.colors.borders.default); 2]
        );
        router.route_input_event(
            &mut tree,
            FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time,
                pos: Point::new(origin.x + 1.0, origin.y + rows[1].top + 1.0),
                delta: (0.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState {
                    primary: false,
                    secondary: false,
                    auxiliary: false,
                },
            },
            Some(root),
            &mut text,
            None,
            domains.primary_state_mut(),
            &mut bindings,
        );
        let stable = harness.frame_with_appearance(
            &mut tree,
            root,
            constraints,
            &theme,
            &mut text,
            &icons,
            &domains,
            WindowId::PRIMARY,
        );
        assert!(!stable.scene_built);
        assert!(Arc::ptr_eq(
            &harness.local_fragment(group_id).unwrap(),
            &disabled_fragment
        ));
        let group = tree
            .node(group_node)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<RadioGroup>()
            .unwrap();
        assert_eq!(group.hovered, None);
        let mut disabled_semantic = semantic;
        disabled_semantic.disabled = true;
        assert_eq!(group.state(), &disabled_semantic);
        assert_eq!(
            domains.primary_state().focus_manager.focused(),
            Some(group_id)
        );
        assert_eq!(changes.get(), 0);
        let mut app = AppContext::new(theme);
        let mut screens = ScreenStack::new();
        let mut modals = ModalStack::new();
        binder.bind(
            &ScreenContext::new(&mut app, &mut screens, &mut modals, None),
            |binding| assert_eq!(binding.selection("choice").take(), None),
        );
    }
}
