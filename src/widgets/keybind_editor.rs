// SPEC-HAL-329: KeybindEditor — Single-Stroke Keybinding Capture Widget
// SPEC-HAL-328 §11.K: Category K (Single-Stroke Capture)
//
// Invariants enforced:
//   INV-KBE-ONE-SHOT          — one commit per capture session
//   INV-KBE-NO-OVERRIDE-ESCAPE — Escape never consumed; framework releases
//   INV-KBE-NO-OVERRIDE-TAB    — Tab never consumed; framework moves focus
//   INV-KBE-VERBATIM-COMMIT    — captured value is byte-for-byte the event
//   INV-KBE-EXPLICIT-COMMIT    — only user keystroke commits
//   INV-KBE-NO-REGISTRY        — widget does not touch BindingRegistry
//   INV-KBE-DISPLAY-CANONICAL  — display via Shortcut::to_display_string()
//   INV-KBE-SOVEREIGNTY        — capture state engine-derived, not OS-derived

use std::cell::Cell;

use crate::core::animation::{Animation, Easing, TransitionHandle};
use crate::core::event::{
    Event, EventPhase, KeyCode, KeyEvent, KeyboardAuthorityMode, PointerButton, PointerEventKind,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::LAYER_CONTENT;
use crate::core::text_engine::compute_control_text_baseline_y;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::runtime::actions::Shortcut;
use crate::utils::id::WidgetInternalId;

/// SPEC-HAL-329 §6.4: Optional conflict info returned by the caller's
/// conflict_check callback. Displayed verbatim adjacent to the value.
#[derive(Clone, Debug)]
pub struct ConflictInfo {
    pub message: String,
}

/// Tracks which modifiers are currently held during a capture session.
struct ModifierTracker;

impl ModifierTracker {
    fn is_modifier(key: KeyCode) -> bool {
        matches!(
            key,
            KeyCode::Control | KeyCode::Alt | KeyCode::Shift | KeyCode::Meta
        )
    }
}

/// Outcome of the cx-independent key state-machine core.
///
/// The cx-facing `handle_key` adapter maps this onto the `bool` return
/// plus the single framework effect (`release_keyboard_capture`).
/// Splitting the pure transition from the cx edge keeps the SPEC-HAL-329
/// §5 one-shot state machine directly testable; behavior is 1:1 with the
/// §5.2 / §7.3 contract.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum KeyOutcome {
    /// Widget does not consume — framework handles (navigation / Escape / Tab).
    NotConsumed,
    /// Widget consumes (modifier tracking / non-modifier Key Up) — no commit.
    Consumed,
    /// Non-modifier Key Down committed: value set, conflict computed,
    /// on_change fired. Caller must release capture programmatically.
    CommitAndRelease,
    /// SPEC-HAL-329 §5.5 / INV-KBE-BACKSPACE-CLEARS: Backspace with no
    /// modifiers held cleared value to None; on_change(None) fired;
    /// last_conflict cleared. Caller must release capture programmatically.
    /// Functionally identical to CommitAndRelease at the cx adapter; the
    /// distinct variant exists so tests can assert which path fired.
    ClearAndRelease,
    UnavailableAndRelease,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyRecordingUnavailable {
    UnknownPhysicalKey,
    UnknownLogicalKey,
    PhysicalBinding(crate::core::event::PhysicalBindingUnavailable),
}

pub struct KeybindEditor {
    id: WidgetInternalId,
    value: Option<Shortcut>,
    placeholder: String,
    capture_prompt: String,
    conflict_check: Option<Box<dyn Fn(Shortcut) -> Option<ConflictInfo>>>,
    on_change: Option<Box<dyn Fn(Option<Shortcut>)>>,
    /// Fires when a capture session ends without a commit having fired
    /// in that session (Escape / Tab / blur during capture). NOT a
    /// commit — INV-KBE-EXPLICIT-COMMIT preserved. The host receives a
    /// session-end signal; the widget's `value` is unchanged on this
    /// path. Fires at most once per `enter_capture_core` →
    /// `leave_capture_core` cycle.
    on_cancel: Option<Box<dyn Fn()>>,
    on_unavailable: Option<Box<dyn Fn(KeyRecordingUnavailable)>>,
    record_physical: bool,

    // Capture-internal state.
    in_capture: bool,
    /// Last conflict info computed at commit, displayed in idle until value changes.
    last_conflict: Option<ConflictInfo>,
    /// Tracks whether `on_change` fired during the current capture
    /// session. Reset to `false` by `enter_capture_core`; set to `true`
    /// by `process_key`'s commit and clear branches before firing
    /// `on_change`. Read by `leave_capture_core` to decide whether to
    /// fire `on_cancel` on the in_capture true→false transition.
    committed_in_session: bool,

    // Layout-computed bounds for hit-testing.
    layout_bounds: Size,
    cached_content_width: f32,

    // Focus / capture animation (TextField / Checkbox pattern).
    focus_t: Cell<f32>,
    focus_anim: Cell<Option<TransitionHandle>>,
}

impl KeybindEditor {
    pub fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
            value: None,
            placeholder: String::new(),
            capture_prompt: "Press shortcut…".into(),
            conflict_check: None,
            on_change: None,
            on_cancel: None,
            on_unavailable: None,
            record_physical: false,
            in_capture: false,
            last_conflict: None,
            committed_in_session: false,
            layout_bounds: Size::zero(),
            cached_content_width: 0.0,
            focus_t: Cell::new(0.0),
            focus_anim: Cell::new(None),
        }
    }

    pub fn with_value(mut self, value: Option<Shortcut>) -> Self {
        self.value = value;
        self
    }

    /// Construction-time choice; ordinary editors continue recording logical keys.
    pub fn with_physical_keys(mut self, physical: bool) -> Self {
        self.record_physical = physical;
        self
    }

    pub fn on_unavailable<F: Fn(KeyRecordingUnavailable) + 'static>(mut self, callback: F) -> Self {
        self.on_unavailable = Some(Box::new(callback));
        self
    }

    pub fn with_placeholder(mut self, placeholder: impl Into<String>) -> Self {
        self.placeholder = placeholder.into();
        self
    }

    pub fn with_capture_prompt(mut self, prompt: impl Into<String>) -> Self {
        self.capture_prompt = prompt.into();
        self
    }

    pub fn with_conflict_check(
        mut self,
        check: impl Fn(Shortcut) -> Option<ConflictInfo> + 'static,
    ) -> Self {
        self.conflict_check = Some(Box::new(check));
        self
    }

    pub fn on_change(mut self, callback: impl Fn(Option<Shortcut>) + 'static) -> Self {
        self.on_change = Some(Box::new(callback));
        self
    }

    /// Set the cancel callback. Fired when a capture session ends
    /// without a commit having fired (framework Escape / Tab / blur
    /// during capture). NOT a commit — `value` is unchanged on this
    /// path. The host typically routes this to its cancel disposition
    /// (PropertyEditor routes to `EditExitReason::Cancel`).
    pub fn on_cancel(mut self, callback: impl Fn() + 'static) -> Self {
        self.on_cancel = Some(Box::new(callback));
        self
    }

    pub fn value(&self) -> Option<Shortcut> {
        self.value
    }

    /// Programmatic value set. Does NOT fire on_change (INV-KBE-EXPLICIT-COMMIT).
    pub fn set_value(&mut self, value: Option<Shortcut>) {
        self.value = value;
        self.last_conflict = None;
    }

    /// Host-side cancel hook. Releases capture without committing.
    /// Used by PropertyEditor's ForcedCancel path.
    /// Does NOT fire on_change. INV-KBE-EXPLICIT-COMMIT.
    pub fn cancel(&mut self) {
        self.in_capture = false;
    }

    // ------------------------------------------------------------------
    // cx-independent state-machine core (SPEC-HAL-329 §5).
    // ------------------------------------------------------------------

    /// SPEC-HAL-329 §5.2: capture entry. Clear the modifier tracker and
    /// flip the display to the capture prompt. `last_conflict` is
    /// preserved until a commit replaces it.
    fn enter_capture_core(&mut self) {
        self.in_capture = true;
        self.committed_in_session = false;
    }

    /// SPEC-HAL-329 §5.3 / INV-KBE-NO-OVERRIDE-ESCAPE: the framework
    /// released capture (Escape / Tab / blur / host forced-unmount). No
    /// commit occurred on this path; `value` is unchanged. Reset the
    /// capture-internal state.
    /// Returns `true` iff `in_capture` transitioned true→false on this
    /// call (a real capture-leave). Callers with `cx` access
    /// (`on_capture_leave`, `on_blur`) invalidate layout on `true` so
    /// the host's reconciliation pass runs on the next frame. Without
    /// this, the editor child stays mounted in the host's tree until
    /// some unrelated event marks layout dirty — SPEC-HAL-026.
    fn leave_capture_core(&mut self) -> bool {
        let was_in_capture = self.in_capture;
        self.in_capture = false;
        // Fire on_cancel exactly when a capture session ends without a
        // commit having fired in that session. Guards:
        //  - was_in_capture: only fire on the true→false transition.
        //    Idempotent calls (already-false leave) are no-ops.
        //  - !committed_in_session: a commit (process_key's commit or
        //    clear branch) already notified the host via on_change.
        //    on_cancel and on_change are mutually exclusive per session.
        if was_in_capture && !self.committed_in_session {
            if let Some(cb) = &self.on_cancel {
                cb();
            }
        }
        was_in_capture
    }

    /// The one-shot key state machine. `mode` is `cx.keyboard_mode`.
    /// Mirrors the SPEC-HAL-329 §5.2 transition table exactly.
    fn process_key(&mut self, mode: KeyboardAuthorityMode, key: &KeyEvent) -> KeyOutcome {
        // Navigation mode: NEVER consume keys. Framework handles
        // navigation. SPEC-HAL-329 §7.3 / SPEC-HAL-328 §11.K.
        if mode != KeyboardAuthorityMode::Capture {
            return KeyOutcome::NotConsumed;
        }

        // FORBID-KBE-ESCAPE-OVERRIDE / FORBID-KBE-TAB-OVERRIDE: Escape and
        // Tab are framework-reserved. Return NotConsumed so the framework
        // handles them via §3.2 / §3.7. The commit path is not executed,
        // so the value is unchanged.
        if matches!(key.key_code, KeyCode::Escape | KeyCode::Tab) {
            return KeyOutcome::NotConsumed;
        }

        // Modifier key: update tracker, never commit.
        // FORBID-KBE-MODIFIER-ONLY-COMMIT.
        if !self.in_capture || self.committed_in_session {
            return KeyOutcome::Consumed;
        }
        if ModifierTracker::is_modifier(key.key_code) || key.physical_key.is_modifier() {
            return KeyOutcome::Consumed;
        }

        if key.is_down && key.is_repeat {
            return KeyOutcome::Consumed;
        }

        // Backspace clear path. SPEC-HAL-329 §5.5 / INV-KBE-BACKSPACE-CLEARS.
        // Backspace Key Down with NO modifiers held mutates value to None,
        // fires on_change(None), and releases capture. With any modifier
        // held, Backspace falls through to the standard non-modifier commit
        // path. Key Up of Backspace alone falls through to the Key Up branch
        // below (Consumed, no clear).
        if key.key_code == KeyCode::Backspace && key.is_down {
            let no_modifiers_held = key.modifiers == crate::core::event::Modifiers::default();
            if no_modifiers_held {
                // Guard parallel to FORBID-KBE-MULTI-COMMIT: clear path is
                // also single-shot. After clear, capture is released; further
                // events route through Navigation mode.
                if !self.in_capture {
                    return KeyOutcome::NotConsumed;
                }
                self.value = None;
                self.last_conflict = None;
                // Mark commit-in-session BEFORE firing on_change so that
                // the eventual leave_capture_core (driven by framework
                // release via cx.release_keyboard_capture in the handle_key
                // adapter) sees committed_in_session=true and suppresses
                // on_cancel. on_change and on_cancel are mutually
                // exclusive per session.
                self.committed_in_session = true;
                if let Some(cb) = &self.on_change {
                    cb(None);
                }
                return KeyOutcome::ClearAndRelease;
            }
            // Backspace + at least one modifier held → fall through to the
            // standard non-modifier commit path below.
        }

        // Non-modifier key. Commit fires on Key Down only; Key Up is
        // observed but does not commit.
        if !key.is_down {
            return KeyOutcome::Consumed;
        }

        // FORBID-KBE-MULTI-COMMIT guard: a second Key Down in the same
        // session (race / repeat) must not re-commit. After commit the
        // framework releases capture and routes events to navigation.
        if !self.in_capture {
            return KeyOutcome::NotConsumed;
        }

        // INV-KBE-VERBATIM-COMMIT: build Shortcut from event + tracker.
        let shortcut = if self.record_physical {
            Shortcut::from_physical_response(Some(key), None).map_err(|reason| match reason {
                crate::core::event::PhysicalBindingUnavailable::UnknownPhysicalKey => {
                    KeyRecordingUnavailable::UnknownPhysicalKey
                }
                other => KeyRecordingUnavailable::PhysicalBinding(other),
            })
        } else if key.key_code == KeyCode::Unknown {
            Err(KeyRecordingUnavailable::UnknownLogicalKey)
        } else {
            Ok(Shortcut::from_key_and_modifiers(
                key.key_code,
                key.modifiers,
            ))
        };
        let shortcut = match shortcut {
            Ok(shortcut) => shortcut,
            Err(reason) => {
                if let Some(callback) = &self.on_unavailable {
                    callback(reason);
                }
                return KeyOutcome::UnavailableAndRelease;
            }
        };

        // Commit.
        self.value = Some(shortcut);

        // Optional conflict check. Display-only. FORBID-KBE-REGISTRY-ACCESS:
        // the callback may inspect the registry; the widget itself does not.
        self.last_conflict = self.conflict_check.as_ref().and_then(|f| f(shortcut));

        // Mark commit-in-session BEFORE firing on_change so that the
        // eventual leave_capture_core (driven by framework release via
        // cx.release_keyboard_capture in the handle_key adapter) sees
        // committed_in_session=true and suppresses on_cancel. on_change
        // and on_cancel are mutually exclusive per session.
        self.committed_in_session = true;

        // Fire on_change. INV-KBE-ONE-SHOT.
        if let Some(cb) = &self.on_change {
            cb(self.value);
        }

        // Caller releases capture programmatically. The framework's
        // on_capture_leave then resets in_capture / modifier_tracker.
        KeyOutcome::CommitAndRelease
    }

    /// cx-facing adapter. Reads the keyboard mode from cx, runs the pure
    /// state machine, and applies the single framework effect.
    fn handle_key(&mut self, cx: &mut EventContext, key: &KeyEvent) -> bool {
        match self.process_key(cx.keyboard_mode, key) {
            KeyOutcome::NotConsumed => false,
            KeyOutcome::Consumed => true,
            KeyOutcome::CommitAndRelease
            | KeyOutcome::ClearAndRelease
            | KeyOutcome::UnavailableAndRelease => {
                cx.release_keyboard_capture();
                true
            }
        }
    }

    /// SPEC-HAL-329 §6.1 / §6.2: the string the widget renders.
    /// In capture: the capture prompt. Else: the canonical display of the
    /// value (INV-KBE-DISPLAY-CANONICAL) or the placeholder.
    fn current_display(&self) -> String {
        if self.in_capture {
            self.capture_prompt.clone()
        } else if let Some(shortcut) = self.value {
            shortcut.to_display_string()
        } else {
            self.placeholder.clone()
        }
    }
}

impl Default for KeybindEditor {
    fn default() -> Self {
        Self::new()
    }
}

impl WidgetDowncast for KeybindEditor {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for KeybindEditor {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // Single-line control sized like TextField. Width derived from the
        // currently-active display string (value / placeholder / capture
        // prompt), clamped to constraints; height = control height.
        let font_size = cx.theme.typography.font_size;
        let display = self.current_display();
        let (text_width, _) = cx.text_engine.measure(&display, font_size);

        let geom = cx.theme.geometry.text_field;
        let content_width = text_width + 2.0 * geom.padding_x;
        self.cached_content_width = content_width;

        // Prefer the max constraint (field fills its slot); fall back to
        // the content width when the max is unbounded.
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            content_width
        };
        let width = width.clamp(constraints.min_width, constraints.max_width);
        let height = geom
            .height
            .clamp(constraints.min_height, constraints.max_height);
        self.layout_bounds = Size::new(width, height);
        LayoutResult::fit(self.layout_bounds)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Consume the focus-ring animation (Checkbox / TextField pattern).
        {
            let _allow = crate::core::rect_authority::AllowPaintMutationGuard::enter();
            if let Some(handle) = self.focus_anim.get() {
                if let Some(value) = cx.get_animation::<f32>(handle) {
                    self.focus_t.set(value);
                } else {
                    self.focus_anim.set(None);
                }
            }
        }

        let t = cx.theme;
        let geom = t.geometry.text_field;

        // SPEC-HAL-093 v3: per-coordinate snap to the physical pixel grid.
        let scale = cx.scale_factor;
        let one_px = cx.one_px();
        let bx = crate::core::math::snap_to_physical(rect.x, scale);
        let by = crate::core::math::snap_to_physical(rect.y, scale);
        let br = crate::core::math::snap_to_physical(rect.x + rect.width, scale);
        let bb = crate::core::math::snap_to_physical(rect.y + rect.height, scale);
        let bw = br - bx;
        let bh = bb - by;
        let snapped = RectF::new(bx, by, bw, bh);

        // Background fill.
        cx.scene
            .push_rect(snapped, t.colors.surfaces.content, 2, cx.scene_key);

        // SPEC-HAL-329 §6.3 / SPEC-HAL-328 §12.3 (amended): focus = 1px in
        // borders.focus, capture = 2px in borders.focus. Theme::control_border
        // supplies the canonical (color, width) pair; the same 4 edge
        // primitives are painted in every state (no new geometry on focus).
        let (border_color, border_w_logical) =
            t.control_border(cx.has_focus_cue(), cx.has_capture(), false);
        let edge_thick = if cx.has_focus_cue() {
            border_w_logical
        } else {
            one_px
        };
        let (top_color, bottom_color) = if cx.has_focus_cue() {
            (border_color, border_color)
        } else {
            (t.colors.borders.shadow, t.colors.borders.highlight)
        };
        cx.scene
            .push_rect(RectF::new(bx, by, bw, edge_thick), top_color, 3, None);
        cx.scene.push_rect(
            RectF::new(bx, by + edge_thick, edge_thick, bh - 2.0 * edge_thick),
            top_color,
            3,
            None,
        );
        cx.scene.push_rect(
            RectF::new(bx, bb - edge_thick, bw, edge_thick),
            bottom_color,
            3,
            None,
        );
        cx.scene.push_rect(
            RectF::new(
                br - edge_thick,
                by + edge_thick,
                edge_thick,
                bh - 2.0 * edge_thick,
            ),
            bottom_color,
            3,
            None,
        );

        // Display string: capture prompt while capturing, else the
        // canonical value display, else the placeholder.
        let display = self.current_display();
        if !display.is_empty() {
            let font_size = t.typography.font_size;
            let metrics = cx.text_engine.font_metrics(font_size);
            let baseline_y = rect.y
                + compute_control_text_baseline_y(
                    rect.height,
                    metrics.ascent,
                    metrics.descent,
                    t.typography.baseline_offset,
                );
            let text_top_y = baseline_y - metrics.ascent;
            let text_x = rect.x + geom.padding_x;
            let text_color = if self.in_capture {
                t.colors.typography.muted
            } else {
                t.colors.typography.primary
            };
            let run = cx.text_engine.shape(&display, font_size);
            cx.text_engine.draw(
                &run,
                text_x,
                text_top_y,
                text_color,
                LAYER_CONTENT + 2,
                cx.scene,
            );
        }

        // SPEC-HAL-329 §6.4: conflict line below the value (idle only).
        if !self.in_capture {
            if let Some(conflict) = &self.last_conflict {
                let font_size = t.typography.font_size;
                let metrics = cx.text_engine.font_metrics(font_size);
                let line_y = rect.y + rect.height + metrics.ascent;
                let run = cx.text_engine.shape(&conflict.message, font_size);
                cx.text_engine.draw(
                    &run,
                    rect.x + geom.padding_x,
                    line_y,
                    t.colors.semantic.warning,
                    LAYER_CONTENT + 2,
                    cx.scene,
                );
            }
        }

        cx.paint_focus_ring(rect, self.focus_t.get(), geom.radius);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn focusable(&self) -> bool {
        true
    }

    fn needs_animation(&self) -> bool {
        self.focus_anim.get().is_some()
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Canonical capture-widget guard (Checkbox / TextField): handle
        // only in the Target phase. Required for INV-KBE-ONE-SHOT — without
        // it the same Key Down dispatches in Capture, Target, and Bubble
        // phases and would commit up to three times.
        if phase != EventPhase::Target {
            return false;
        }

        match event {
            Event::Key(key_event) => self.handle_key(cx, key_event),
            Event::Pointer(p)
                if p.kind == PointerEventKind::Down && p.button == Some(PointerButton::Primary) =>
            {
                // SPEC-HAL-329 §5.2 / SPEC-HAL-328 §3.1 pointer path. Pattern
                // verbatim from TextField (src/widgets/text_field.rs:1212):
                // capture-eligible widgets explicitly request keyboard
                // capture on Pointer Down. The framework routes the request
                // through the engine action chain and dispatches
                // on_capture_enter. The framework does NOT auto-engage
                // capture on click — widgets call this themselves.
                cx.request_keyboard_capture();
                true
            }
            _ => false,
        }
    }

    fn on_focus(&mut self, cx: &mut EventContext) {
        // Animate focus_t 0 → 1 (Checkbox pattern). No other state change.
        let anim = Animation {
            from: self.focus_t.get(),
            to: 1.0,
            duration: 0.08,
            easing: Easing::EaseOut,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.focus_anim.set(Some(handle));
        } else {
            self.focus_t.set(1.0);
            self.focus_anim.set(None);
        }
    }

    fn on_blur(&mut self, cx: &mut EventContext) {
        // INV-KBE-EXPLICIT-COMMIT: blur does NOT commit. If blurred
        // mid-capture (programmatic focus shift), reset capture state — the
        // framework releases capture as part of the focus transition; the
        // widget must not retain a stale in_capture.
        //
        // SPEC-HAL-026: invalidate layout on the true→false transition
        // for the same reason as on_capture_leave — the host's edit
        // session may have been cleared via on_cancel, and Body's
        // reconciliation pass must run.
        if self.leave_capture_core() {
            cx.invalidate_layout();
        }
        let anim = Animation {
            from: self.focus_t.get(),
            to: 0.0,
            duration: 0.08,
            easing: Easing::EaseIn,
        };
        if let Some(handle) = cx.start_animation(anim) {
            self.focus_anim.set(Some(handle));
        } else {
            self.focus_t.set(0.0);
            self.focus_anim.set(None);
        }
    }

    fn capture_eligible(&self) -> bool {
        true
    }

    fn physical_keyboard_capture(&self) -> bool {
        self.record_physical
    }

    fn on_keyboard_capture_interrupted(
        &mut self,
        interruption: &crate::core::event::KeyboardCaptureInterruption,
        _cx: &mut EventContext,
    ) {
        if self.record_physical
            && self.in_capture
            && !self.committed_in_session
            && interruption.reason
                != crate::core::event::KeyboardCaptureInterruptionReason::FocusLost
        {
            if let Some(callback) = &self.on_unavailable {
                callback(KeyRecordingUnavailable::PhysicalBinding(
                    crate::core::event::PhysicalBindingUnavailable::Reserved(interruption.reason),
                ));
            }
        }
    }

    fn defines_focus_scope(&self) -> bool {
        false
    }

    fn accepts_keyboard_activation(&self) -> bool {
        false
    }

    fn on_capture_enter(&mut self, _cx: &mut EventContext) {
        // SPEC-HAL-329 §5.2: clear modifier tracker, flip display to the
        // capture prompt. last_conflict preserved until commit replaces it.
        self.enter_capture_core();
    }

    fn on_capture_leave(&mut self, cx: &mut EventContext) {
        // SPEC-HAL-329 §5.3 / INV-KBE-NO-OVERRIDE-ESCAPE: framework
        // released capture (Escape / Tab / forced). No commit on this path;
        // value unchanged. Reset capture-internal state.
        //
        // SPEC-HAL-026: invalidate layout on the true→false transition
        // so the host's reconciliation pass runs on the next frame.
        // The framework sets request_continuous_frames_after_mode_transition
        // (app.rs:2566) after Escape, which schedules paint but NOT
        // layout. Without the dirty mark, PE Body's edit_session
        // unmount (body.rs step 6a) waits for an unrelated dirty
        // event — pointer move, hover, focus change — leaving the
        // editor child visually mounted in the meantime.
        if self.leave_capture_core() {
            cx.invalidate_layout();
        }
    }

    fn debug_type_name(&self) -> &'static str {
        "KeybindEditor"
    }
}

#[cfg(test)]
mod tests {
    //! SPEC-HAL-329 §5 / §9 / §10 conformance.
    //!
    //! The binding behavioral requirements (§9 invariants, §10 forbidden
    //! behaviors) are verified against the cx-independent state-machine
    //! core (`process_key` / `enter_capture_core` / `leave_capture_core`)
    //! plus the public API (`set_value` / `cancel` / `value` / builders).
    //! `handle_key` is a thin adapter whose only added effect over the
    //! core is `cx.release_keyboard_capture()`, surfaced verbatim as
    //! `KeyOutcome::CommitAndRelease`.

    use super::*;
    use crate::core::event::{KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers};
    use crate::runtime::actions::Shortcut;
    use std::cell::RefCell;
    use std::rc::Rc;

    const CAP: KeyboardAuthorityMode = KeyboardAuthorityMode::Capture;
    const NAV: KeyboardAuthorityMode = KeyboardAuthorityMode::Navigation;

    fn kd(code: KeyCode) -> KeyEvent {
        KeyEvent {
            key_code: code,

            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            is_down: true,
            is_repeat: false,
            modifiers: Modifiers::default(),
            text: None,
        }
    }

    fn ku(code: KeyCode) -> KeyEvent {
        KeyEvent {
            key_code: code,

            physical_key: crate::core::event::PhysicalKeyCode::Unknown,
            is_down: false,
            is_repeat: false,
            modifiers: Modifiers::default(),
            text: None,
        }
    }

    fn sc(key: KeyCode, ctrl: bool, alt: bool, shift: bool, meta: bool) -> Shortcut {
        Shortcut {
            key: key.into(),
            ctrl,
            alt,
            shift,
            meta,
        }
    }

    #[test]
    fn physical_keyboard_editor_reports_reserved_without_committing() {
        use crate::core::app::{AppContext, AppRoot, dispatch_session_input_to_app_root};
        use crate::core::event::{
            InteractionDomainId, InteractionDomainState, KeyState,
            KeyboardCaptureInterruptionReason, PhysicalBindingUnavailable, PhysicalKeyCode,
            RawInputEvent, WindowId,
        };
        use crate::core::event_router::EventRouter;
        use crate::core::input_fusion::InputFusion;
        use crate::core::theme::Theme;
        use crate::core::widget_tree::WidgetTree;
        use crate::platform::font_manager_text_engine::FontManagerTextEngine;
        use std::cell::Cell;
        let unavailable = Rc::new(RefCell::new(Vec::new()));
        let report = Rc::clone(&unavailable);
        let cancelled = Rc::new(Cell::new(0));
        let cancellations = Rc::clone(&cancelled);
        let (editor, log) = editor_with_log();
        let original = Shortcut::key_only(KeyCode::Character('p'));
        let editor = editor
            .with_value(Some(original))
            .with_physical_keys(true)
            .on_unavailable(move |reason| report.borrow_mut().push(reason))
            .on_cancel(move || cancellations.set(cancellations.get() + 1));
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(editor), None, 0);
        let owner = tree.widget_id_for_node(root).unwrap();
        let mut app = AppRoot::new(AppContext::new(Theme::dark()), root);
        let mut router = EventRouter::new();
        let mut state = InteractionDomainState::new();
        let mut text = FontManagerTextEngine::new();
        router.request_focus_for_widget(&mut tree, owner, &mut state);
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        let mut fusion = InputFusion::new();
        let mut events = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                key_code: KeyCode::F12,
                physical_key: PhysicalKeyCode::F12,
                reservation: None,
                state: KeyState::Down,
                repeat: false,
                modifiers: Modifiers::default(),
            },
            InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            &tree,
            None,
            &mut events,
        );
        fusion.flush_with_gesture_sessions(&mut events);
        assert_eq!(events.len(), 1);
        assert!(dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &events[0],
            &mut text,
        ));
        assert_eq!(state.keyboard_mode, NAV);
        assert!(EventRouter::physical_keyboard_capture_owner(&tree, &state).is_none());
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert_eq!(
            unavailable.borrow().as_slice(),
            &[KeyRecordingUnavailable::PhysicalBinding(
                PhysicalBindingUnavailable::Reserved(
                    KeyboardCaptureInterruptionReason::AppConsumed,
                )
            ),]
        );
        assert_eq!(cancelled.get(), 1);
        assert!(log.borrow().is_empty());
        assert_eq!(
            tree.node(root)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<KeybindEditor>()
                .unwrap()
                .value(),
            Some(original)
        );
        assert!(router.handle_engine_action(
            &mut tree,
            crate::runtime::actions::ActionId::ENTER_WIDGET,
            Some(root),
            &mut state,
            &mut text,
            None,
        ));
        events.clear();
        fusion.process_raw_with_gesture_sessions(
            RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time: std::time::Instant::now(),
                key_code: KeyCode::F9,
                physical_key: PhysicalKeyCode::F9,
                reservation: None,
                state: KeyState::Down,
                repeat: false,
                modifiers: Modifiers::default(),
            },
            InteractionDomainId::PRIMARY,
            &mut state.gesture_registry,
            &tree,
            None,
            &mut events,
        );
        fusion.flush_with_gesture_sessions(&mut events);
        assert_eq!(events.len(), 1);
        assert!(!dispatch_session_input_to_app_root(
            &mut app,
            &mut tree,
            &mut router,
            &mut state,
            &events[0],
            &mut text,
        ));
        router.route_session_input_event(
            &mut tree,
            events.remove(0),
            Some(root),
            &mut text,
            None,
            &mut state,
            &mut app.context.registry.binding_registry,
        );
        assert_eq!(state.keyboard_mode, NAV);
        assert!(EventRouter::physical_keyboard_capture_owner(&tree, &state).is_none());
        assert_eq!(state.focus_manager.focused(), Some(owner));
        assert_eq!(unavailable.borrow().len(), 2);
        assert_eq!(
            unavailable.borrow()[1],
            KeyRecordingUnavailable::PhysicalBinding(PhysicalBindingUnavailable::Reserved(
                KeyboardCaptureInterruptionReason::AppConsumed,
            )),
        );
        assert_eq!(cancelled.get(), 2);
        assert!(log.borrow().is_empty());
        assert_eq!(
            tree.node(root)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref::<KeybindEditor>()
                .unwrap()
                .value(),
            Some(original),
        );
    }

    #[test]
    fn physical_keyboard_editor_records_position_and_authoritative_snapshot() {
        use crate::core::event::{BindingKey, PhysicalKeyCode};
        let (editor, log) = editor_with_log();
        let mut editor = editor.with_physical_keys(true);
        editor.enter_capture_core();
        let event = KeyEvent {
            key_code: KeyCode::Character('ф'),
            physical_key: PhysicalKeyCode::KeyA,
            modifiers: Modifiers {
                ctrl: true,
                meta: true,
                shift: true,
                alt: false,
            },
            ..kd(KeyCode::Character('ф'))
        };
        assert_eq!(
            editor.process_key(CAP, &event),
            KeyOutcome::CommitAndRelease
        );
        let recorded = editor.value().unwrap();
        assert_eq!(recorded.key, BindingKey::Physical(PhysicalKeyCode::KeyA));
        assert!(recorded.ctrl && recorded.meta && recorded.shift && !recorded.alt);
        assert_eq!(log.borrow().as_slice(), &[Some(recorded)]);
        // A deferred mode transition cannot permit a second commit.
        assert_eq!(editor.process_key(CAP, &event), KeyOutcome::Consumed);
        assert_eq!(log.borrow().len(), 1);
        editor.leave_capture_core();
        assert_eq!(editor.value(), Some(recorded));
    }

    #[test]
    fn physical_keyboard_editor_rejects_unknown_and_repeat_without_a_value() {
        use crate::core::event::PhysicalKeyCode;
        let unavailable = Rc::new(RefCell::new(Vec::new()));
        let report = Rc::clone(&unavailable);
        let (editor, log) = editor_with_log();
        let mut editor = editor
            .with_physical_keys(true)
            .on_unavailable(move |reason| report.borrow_mut().push(reason));
        editor.enter_capture_core();
        let mut event = kd(KeyCode::Character('a'));
        event.physical_key = PhysicalKeyCode::KeyA;
        event.is_repeat = true;
        assert_eq!(editor.process_key(CAP, &event), KeyOutcome::Consumed);
        event.is_repeat = false;
        event.physical_key = PhysicalKeyCode::ShiftRight;
        assert_eq!(editor.process_key(CAP, &event), KeyOutcome::Consumed);
        event.physical_key = PhysicalKeyCode::Unknown;
        assert_eq!(
            editor.process_key(CAP, &event),
            KeyOutcome::UnavailableAndRelease
        );
        assert_eq!(
            unavailable.borrow().as_slice(),
            &[KeyRecordingUnavailable::UnknownPhysicalKey]
        );
        assert!(editor.value().is_none());
        assert!(log.borrow().is_empty());
    }

    type Log = Rc<RefCell<Vec<Option<Shortcut>>>>;

    fn editor_with_log() -> (KeybindEditor, Log) {
        let log: Log = Rc::new(RefCell::new(Vec::new()));
        let l = log.clone();
        let e = KeybindEditor::new().on_change(move |v| l.borrow_mut().push(v));
        (e, log)
    }

    // 1. INV-KBE-ONE-SHOT: exactly one commit per capture session.
    #[test]
    fn inv_kbe_one_shot_single_commit_per_session() {
        let (mut e, log) = editor_with_log();
        e.enter_capture_core();
        let out = e.process_key(CAP, &kd(KeyCode::Character('s')));
        assert_eq!(out, KeyOutcome::CommitAndRelease);
        assert_eq!(
            e.value(),
            Some(sc(KeyCode::Character('s'), false, false, false, false))
        );
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(
            log.borrow()[0],
            Some(sc(KeyCode::Character('s'), false, false, false, false))
        );

        // Framework releases capture after CommitAndRelease; the next key
        // arrives in Navigation mode and must not re-commit.
        e.leave_capture_core();
        let out2 = e.process_key(NAV, &kd(KeyCode::Character('x')));
        assert_eq!(out2, KeyOutcome::NotConsumed);
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(
            e.value(),
            Some(sc(KeyCode::Character('s'), false, false, false, false))
        );
    }

    // 2. INV-KBE-NO-OVERRIDE-ESCAPE: Escape does not commit; value intact.
    #[test]
    fn inv_kbe_no_override_escape_does_not_commit() {
        let initial = sc(KeyCode::F1, false, false, false, false);
        let (mut e, log) = editor_with_log();
        e.set_value(Some(initial));
        e.enter_capture_core();
        let out = e.process_key(CAP, &kd(KeyCode::Escape));
        assert_eq!(out, KeyOutcome::NotConsumed);
        // Framework unconditionally releases on Escape (SPEC-HAL-328 §3.2).
        e.leave_capture_core();
        assert_eq!(e.value(), Some(initial));
        assert_eq!(log.borrow().len(), 0);
    }

    // 3. INV-KBE-NO-OVERRIDE-TAB: Tab does not commit; value intact.
    #[test]
    fn inv_kbe_no_override_tab_does_not_commit() {
        let initial = sc(KeyCode::F1, false, false, false, false);
        let (mut e, log) = editor_with_log();
        e.set_value(Some(initial));
        e.enter_capture_core();
        let out = e.process_key(CAP, &kd(KeyCode::Tab));
        assert_eq!(out, KeyOutcome::NotConsumed);
        // Framework's release_and_next runs (SPEC-HAL-328 §3.7).
        e.leave_capture_core();
        assert_eq!(e.value(), Some(initial));
        assert_eq!(log.borrow().len(), 0);
    }

    // 4. INV-KBE-VERBATIM-COMMIT: key + authoritative modifiers, verbatim.
    #[test]
    fn inv_kbe_verbatim_commit_modifier_combination() {
        let (mut e, _log) = editor_with_log();
        e.enter_capture_core();
        assert_eq!(
            e.process_key(CAP, &kd(KeyCode::Control)),
            KeyOutcome::Consumed
        );
        assert_eq!(
            e.process_key(CAP, &kd(KeyCode::Shift)),
            KeyOutcome::Consumed
        );
        assert_eq!(
            e.process_key(
                CAP,
                &KeyEvent {
                    modifiers: Modifiers {
                        ctrl: true,
                        shift: true,
                        ..Modifiers::default()
                    },
                    ..kd(KeyCode::Character('s'))
                }
            ),
            KeyOutcome::CommitAndRelease
        );
        assert_eq!(
            e.value(),
            Some(sc(KeyCode::Character('s'), true, false, true, false))
        );
    }

    // 5. INV-KBE-EXPLICIT-COMMIT: set_value does not fire on_change.
    #[test]
    fn inv_kbe_explicit_commit_set_value_does_not_fire_on_change() {
        let (mut e, log) = editor_with_log();
        let s = sc(KeyCode::Character('q'), true, false, false, false);
        e.set_value(Some(s));
        assert_eq!(e.value(), Some(s));
        assert_eq!(log.borrow().len(), 0);
    }

    // 6. INV-KBE-EXPLICIT-COMMIT: cancel does not fire on_change.
    #[test]
    fn inv_kbe_explicit_commit_cancel_does_not_fire_on_change() {
        let initial = sc(KeyCode::Enter, false, false, false, false);
        let (mut e, log) = editor_with_log();
        e.set_value(Some(initial));
        e.enter_capture_core();
        e.cancel();
        assert!(!e.in_capture);
        assert_eq!(e.value(), Some(initial));
        assert_eq!(log.borrow().len(), 0);
    }

    // 7. FORBID-KBE-MODIFIER-ONLY-COMMIT: modifier down/up never commits.
    #[test]
    fn forbid_kbe_modifier_only_commit() {
        let (mut e, log) = editor_with_log();
        e.enter_capture_core();
        assert_eq!(
            e.process_key(CAP, &kd(KeyCode::Control)),
            KeyOutcome::Consumed
        );
        assert_eq!(
            e.process_key(CAP, &ku(KeyCode::Control)),
            KeyOutcome::Consumed
        );
        assert_eq!(e.value(), None);
        assert_eq!(log.borrow().len(), 0);
        assert!(e.in_capture);
    }

    // 8. FORBID-KBE-NORMALIZATION: letter case preserved, no aliasing.
    #[test]
    fn forbid_kbe_normalization_letter_case_preserved() {
        let (mut e, _log) = editor_with_log();
        e.enter_capture_core();
        assert_eq!(
            e.process_key(CAP, &kd(KeyCode::Shift)),
            KeyOutcome::Consumed
        );
        let out = e.process_key(
            CAP,
            &KeyEvent {
                modifiers: Modifiers {
                    shift: true,
                    ..Modifiers::default()
                },
                ..kd(KeyCode::Character('A'))
            },
        );
        assert_eq!(out, KeyOutcome::CommitAndRelease);
        assert_eq!(
            e.value(),
            Some(sc(KeyCode::Character('A'), false, false, true, false))
        );
        match e.value().unwrap().key {
            crate::core::event::BindingKey::Logical(KeyCode::Character(c)) => assert_eq!(c, 'A'),
            other => panic!("expected verbatim Character('A'), got {:?}", other),
        }
    }

    // 9. INV-KBE-DISPLAY-CANONICAL: display via to_display_string().
    #[test]
    fn display_canonical_uses_shortcut_to_display_string() {
        let shortcut = Shortcut::ctrl_shift(KeyCode::Character('s'));
        let e = KeybindEditor::new().with_value(Some(shortcut));
        assert_eq!(e.current_display(), shortcut.to_display_string());
    }

    // 10. SPEC-HAL-329 §6.4: conflict_check invoked after commit.
    #[test]
    fn conflict_check_invoked_after_commit() {
        let mut e = KeybindEditor::new().with_conflict_check(|s| {
            if s.key == KeyCode::Character('s') {
                Some(ConflictInfo {
                    message: "X".into(),
                })
            } else {
                None
            }
        });
        e.enter_capture_core();
        let out = e.process_key(CAP, &kd(KeyCode::Character('s')));
        assert_eq!(out, KeyOutcome::CommitAndRelease);
        assert_eq!(
            e.last_conflict.as_ref().map(|c| c.message.as_str()),
            Some("X")
        );
        // set_value clears the conflict (value changed programmatically).
        e.set_value(None);
        assert!(e.last_conflict.is_none());
    }

    // 11. INV-KBE-BACKSPACE-CLEARS: Backspace alone (no modifiers held)
    // clears value to None and fires on_change(None).
    #[test]
    fn inv_kbe_backspace_clears_no_modifiers() {
        let initial = sc(KeyCode::F1, false, false, false, false);
        let (mut e, log) = editor_with_log();
        e.set_value(Some(initial));
        // Pre-set a conflict to verify it is cleared.
        e.last_conflict = Some(ConflictInfo {
            message: "stale".into(),
        });
        e.enter_capture_core();
        let out = e.process_key(CAP, &kd(KeyCode::Backspace));
        assert_eq!(out, KeyOutcome::ClearAndRelease);
        assert_eq!(e.value(), None);
        assert!(e.last_conflict.is_none());
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(log.borrow()[0], None);
    }

    // 12. Backspace with at least one modifier held is captured as a normal
    // binding (commit path, not clear path).
    #[test]
    fn backspace_with_modifier_commits_normally() {
        let (mut e, log) = editor_with_log();
        e.enter_capture_core();
        assert_eq!(
            e.process_key(CAP, &kd(KeyCode::Control)),
            KeyOutcome::Consumed
        );
        let out = e.process_key(
            CAP,
            &KeyEvent {
                modifiers: Modifiers {
                    ctrl: true,
                    ..Modifiers::default()
                },
                ..kd(KeyCode::Backspace)
            },
        );
        assert_eq!(out, KeyOutcome::CommitAndRelease);
        assert_eq!(
            e.value(),
            Some(sc(KeyCode::Backspace, true, false, false, false))
        );
        assert_eq!(log.borrow().len(), 1);
        assert_eq!(
            log.borrow()[0],
            Some(sc(KeyCode::Backspace, true, false, false, false))
        );
    }

    // 13. Backspace Key Up alone (no Key Down precedent in this capture)
    // is observed but ignored — clear fires on Key Down only.
    #[test]
    fn backspace_keyup_alone_ignored() {
        let (mut e, log) = editor_with_log();
        e.enter_capture_core();
        let out = e.process_key(CAP, &ku(KeyCode::Backspace));
        assert_eq!(out, KeyOutcome::Consumed);
        assert_eq!(e.value(), None);
        assert_eq!(log.borrow().len(), 0);
        assert!(e.in_capture);
    }

    #[test]
    fn on_cancel_fires_on_capture_leave_without_commit() {
        // Escape path: framework released capture without a commit. The
        // widget's leave_capture_core must fire on_cancel.
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();
        let mut editor = KeybindEditor::new().on_cancel(move || {
            *count_clone.borrow_mut() += 1;
        });
        editor.enter_capture_core();
        let did_transition = editor.leave_capture_core();
        assert!(
            did_transition,
            "leave_capture_core must report transition true"
        );
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn on_cancel_does_not_fire_after_key_commit() {
        // Commit path: process_key set committed_in_session=true before
        // firing on_change. Subsequent leave_capture_core must suppress
        // on_cancel. on_change and on_cancel are mutually exclusive
        // per session.
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();
        let mut editor = KeybindEditor::new().on_cancel(move || {
            *count_clone.borrow_mut() += 1;
        });
        editor.enter_capture_core();
        let _outcome = editor.process_key(CAP, &kd(KeyCode::Character('a')));
        let did_transition = editor.leave_capture_core();
        assert!(
            did_transition,
            "leave_capture_core must report transition true after commit"
        );
        assert_eq!(*count.borrow(), 0);
    }

    #[test]
    fn on_cancel_does_not_fire_after_backspace_clear() {
        // Backspace-clear path also sets committed_in_session=true. The
        // host received on_change(None); on_cancel must not also fire.
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();
        let mut editor = KeybindEditor::new()
            .with_value(Some(sc(KeyCode::Character('q'), true, false, false, false)))
            .on_cancel(move || {
                *count_clone.borrow_mut() += 1;
            });
        editor.enter_capture_core();
        let _outcome = editor.process_key(CAP, &kd(KeyCode::Backspace));
        let did_transition = editor.leave_capture_core();
        assert!(
            did_transition,
            "leave_capture_core must report transition true after clear"
        );
        assert_eq!(*count.borrow(), 0);
    }

    #[test]
    fn on_cancel_does_not_fire_without_prior_enter() {
        // Idempotent guard: leave_capture_core called when in_capture is
        // already false (no prior enter, or already left) is a no-op.
        let count = Rc::new(RefCell::new(0u32));
        let count_clone = count.clone();
        let mut editor = KeybindEditor::new().on_cancel(move || {
            *count_clone.borrow_mut() += 1;
        });
        // No prior enter — in_capture is false. No transition.
        assert!(
            !editor.leave_capture_core(),
            "no prior enter: no transition"
        );
        assert_eq!(*count.borrow(), 0);
        // Real transition: enter then leave.
        editor.enter_capture_core();
        assert!(
            editor.leave_capture_core(),
            "real enter→leave is a transitio
        n"
        );
        assert_eq!(*count.borrow(), 1);
        // Second leave after already-left: no transition.
        assert!(!editor.leave_capture_core(), "already-left: no transition");
        assert_eq!(*count.borrow(), 1);
    }

    #[test]
    fn leave_capture_core_reports_transition_after_commit() {
        // After a key commit, committed_in_session=true and on_cancel
        // is suppressed — but the transition still happened and must
        // be reported as true so the host can invalidate layout.
        let count_cancel = Rc::new(RefCell::new(0u32));
        let count_cancel_clone = count_cancel.clone();
        let mut editor = KeybindEditor::new().on_cancel(move || {
            *count_cancel_clone.borrow_mut() += 1;
        });
        editor.enter_capture_core();
        let _outcome = editor.process_key(CAP, &kd(KeyCode::Character('a')));
        let did_transition = editor.leave_capture_core();
        assert!(did_transition);
        assert_eq!(*count_cancel.borrow(), 0);
    }
}
