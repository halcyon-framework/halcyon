// SPEC-HAL-033: Input Fusion Engine
//
// Converts raw platform input events into fused, high-level events suitable for widget routing.
// Handles multi-click detection, drag recognition, scroll normalization, and shortcut matching.

use crate::core::app::{EngineTicker, FrameTime};
use crate::core::event::{
    FusedInputEvent, GestureRegistry, InteractionDomainId, KeyCode, KeyState, Modifiers,
    PhysicalGesturePress, PointerButton, PointerButtonsState, PointerDevice, PointerId,
    PointerType, QualifiedTouchMotion, RawButtonState, RawInputEvent, ScrollSource,
    SessionQualifiedInput, ShortcutId, ShortcutScope, TouchPhase, WheelDelta, WindowId,
};
use crate::core::math::Point;
use crate::core::relative_pointer::{
    RelativeLogicalDelta, RelativePointerIngressGate, RelativePointerLeaseId,
    RelativePointerNormalizationEpoch,
};
use std::collections::{HashMap, HashSet};
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use web_time::Instant;

// ============================================================================
// Configuration Constants
// ============================================================================

/// SPEC-HAL-036 Phase 5: Fast double-click window (milliseconds)
const DOUBLE_CLICK_THRESHOLD_MS: u128 = 200;

// ============================================================================
// Internal Types
// ============================================================================

/// Internal button state for fusion processing
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ButtonState {
    Pressed,
    Released,
}

impl From<RawButtonState> for ButtonState {
    fn from(state: RawButtonState) -> Self {
        match state {
            RawButtonState::Pressed => ButtonState::Pressed,
            RawButtonState::Released => ButtonState::Released,
        }
    }
}

/// Distance threshold for multi-click detection (logical pixels)
const CLICK_DISTANCE_THRESHOLD: f32 = 4.0;

/// Distance threshold for drag recognition (logical pixels)
pub(crate) const DRAG_DISTANCE_THRESHOLD: f32 = 4.0;

/// SPEC-HAL-036 Phase 4: Device-aware scroll normalization constants
/// Mouse wheel uses higher gain for responsive scrolling
const MOUSE_WHEEL_PIXELS_PER_LINE: f32 = 56.0;
/// Touchpad uses raw/minimal gain for fine-grained control
const TOUCHPAD_PIXELS_PER_LINE: f32 = 1.0;

/// Maximum click count before reset
const MAX_CLICK_COUNT: u8 = 3;

/// SPEC-HAL-036: Time threshold for scroll burst detection (milliseconds)
const SCROLL_FLUSH_THRESHOLD_MS: u128 = 6;

// ============================================================================
// SPEC-HAL-036: Input Coalescing Types
// ============================================================================

/// SPEC-HAL-036: Pending pointer move for coalescing high-frequency events
#[derive(Debug, Clone)]
struct PendingPointer {
    press: Option<Rc<PhysicalGesturePress>>,
    /// SPEC-HAL-151: Origin window
    window_id: WindowId,
    /// Latest position (replaces old)
    pos: Point,
    /// Accumulated delta (summed)
    delta: (f32, f32),
    /// Most recent timestamp
    time: Instant,
    /// Device type
    device: PointerDevice,
    /// Current button states
    buttons: PointerButtonsState,
}

/// Canonical relative motion waiting for the next ordering boundary.
#[derive(Debug, Clone)]
struct PendingRelativePointerMotion {
    press: Option<Rc<PhysicalGesturePress>>,
    lease_id: RelativePointerLeaseId,
    normalization_epoch: RelativePointerNormalizationEpoch,
    window_id: WindowId,
    pointer_id: PointerId,
    time: Instant,
    delta: RelativeLogicalDelta,
}

/// SPEC-HAL-036: Pending scroll for coalescing scroll bursts
#[derive(Debug, Clone)]
struct PendingScroll {
    /// SPEC-HAL-151: Origin window
    window_id: WindowId,
    /// Accumulated delta in pixels
    delta: (f32, f32),
    /// Most recent timestamp
    time: Instant,
    /// Pointer position
    pos: Point,
    /// Scroll source (Wheel/Touchpad)
    source: ScrollSource,
}

/// SPEC-HAL-036 Phase 3: Pending drag update for coalescing high-frequency drag events
#[derive(Debug, Clone)]
struct PendingDragUpdate {
    press: Option<Rc<PhysicalGesturePress>>,
    /// SPEC-HAL-151: Origin window
    window_id: WindowId,
    /// Drag origin (unchanged throughout drag)
    origin: Point,
    /// Latest position in coalesced batch
    pos: Point,
    /// Accumulated delta since last emitted DragUpdate
    delta: (f32, f32),
    /// Drag button
    button: PointerButton,
    /// Most recent timestamp
    time: Instant,
}

// ============================================================================
// Pointer Fusion State
// ============================================================================

/// State for tracking active drag operation
#[derive(Debug, Clone)]
struct ActiveDrag {
    button: PointerButton,
    origin: Point,
    last_pos: Point,
}

/// Per-button click tracking state
/// SPEC-HAL-DoubleClick: Coordinates stored in RAW WINDOW-LOGICAL space.
/// This space is invariant across capture/layout cycles.
#[derive(Debug, Clone)]
struct ButtonClickState {
    last_click_time: Option<Instant>,
    /// Position in raw window-logical coordinates (NOT widget-local, NOT layout-relative).
    /// Computed as physical_pos / scale_factor in platform thread BEFORE any layout transforms.
    last_click_pos_raw: Option<Point>,
    click_count: u8,
}

impl Default for ButtonClickState {
    fn default() -> Self {
        Self {
            last_click_time: None,
            last_click_pos_raw: None,
            click_count: 0,
        }
    }
}

/// Per-button state including potential drag
#[derive(Debug, Clone, Default)]
struct ButtonDragState {
    /// Press position for potential drag start
    press_pos: Option<Point>,
    /// Press time for potential drag start
    press_time: Option<Instant>,
}

/// Pointer fusion state - tracks position, buttons, clicks, and drags
#[derive(Debug)]
pub struct PointerFusionState {
    /// Current pointer position
    current_pos: Point,
    /// Current button states
    buttons: PointerButtonsState,
    /// Click tracking per (device, button) - [Mouse, Touchpad] × [Primary, Secondary, Auxiliary]
    /// SPEC-HAL-DoubleClick: Decoupled from capture/session lifecycle.
    /// This ensures multi-click detection survives pointer capture teardown.
    click_state: [[ButtonClickState; 3]; 2],
    /// Drag tracking per button
    drag_state: [ButtonDragState; 3],
    /// Active drag if any
    active_drag: Option<ActiveDrag>,
    /// Current device type
    device: PointerDevice,
}

impl Default for PointerFusionState {
    fn default() -> Self {
        Self::new()
    }
}

impl PointerFusionState {
    pub fn new() -> Self {
        Self {
            current_pos: Point::zero(),
            buttons: PointerButtonsState::default(),
            click_state: Default::default(), // [[ButtonClickState; 3]; 2] - all None/0
            drag_state: Default::default(),
            active_drag: None,
            device: PointerDevice::Mouse,
        }
    }

    fn device_index(device: PointerDevice) -> usize {
        match device {
            PointerDevice::Mouse => 0,
            PointerDevice::Touchpad => 1,
        }
    }

    fn button_index(button: PointerButton) -> usize {
        match button {
            PointerButton::Primary => 0,
            PointerButton::Secondary => 1,
            PointerButton::Auxiliary => 2,
        }
    }

    /// Check and update click count, returns click count for this press.
    /// SPEC-HAL-DoubleClick: Now per-device AND per-button to survive capture teardown.
    /// INVARIANT: `pos_raw` must be in raw window-logical coordinates (physical / scale_factor).
    /// This coordinate space is invariant across capture/layout/scroll cycles.
    /// Multi-click detection invariant:
    /// "Multi-click spans multiple pointer sessions.
    ///  Session teardown MUST NOT affect click history."
    ///
    /// Click state resets ONLY when:
    /// - Time threshold exceeded (>200ms between clicks)
    /// - Distance threshold exceeded (>4px between click positions)
    /// - Device or button changes (tracked separately per device/button)
    fn check_click(
        &mut self,
        device: PointerDevice,
        button: PointerButton,
        pos_raw: Point,
        time: Instant,
    ) -> u8 {
        let dev_idx = Self::device_index(device);
        let btn_idx = Self::button_index(button);
        let state = &mut self.click_state[dev_idx][btn_idx];

        // PROOF LOG: Show state BEFORE check to diagnose multi-click failure
        let has_prior = state.last_click_time.is_some() && state.last_click_pos_raw.is_some();
        tracing::debug!(
            "[FUSION] check_click: has_prior={} prev_count={}",
            has_prior,
            state.click_count
        );

        let is_multi_click = if let (Some(last_time), Some(last_pos_raw)) =
            (state.last_click_time, state.last_click_pos_raw)
        {
            let time_diff = time.duration_since(last_time).as_millis();
            let distance = pos_raw.distance(last_pos_raw);

            let result =
                time_diff <= DOUBLE_CLICK_THRESHOLD_MS && distance < CLICK_DISTANCE_THRESHOLD;

            // PROOF LOG: Show thresholds and result
            tracing::debug!(
                "[FUSION] multi-click: dt={}ms (max={}) dist={:.1}px (max={:.1}) → {}",
                time_diff,
                DOUBLE_CLICK_THRESHOLD_MS,
                distance,
                CLICK_DISTANCE_THRESHOLD,
                if result { "MULTI" } else { "RESET" }
            );

            result
        } else {
            tracing::debug!("[FUSION] first click (no prior state)");
            false
        };

        if is_multi_click {
            state.click_count = (state.click_count % MAX_CLICK_COUNT) + 1;
        } else {
            // PROOF LOG: Document why click history is being reset
            if state.click_count > 0 {
                tracing::debug!(
                    "[FUSION] click_count RESET: was {} → 1 (threshold exceeded or first click)",
                    state.click_count
                );
            }
            state.click_count = 1;
        }

        state.last_click_time = Some(time);
        state.last_click_pos_raw = Some(pos_raw);

        // PROOF LOG: Final result
        tracing::debug!(
            "[FUSION] check_click result: click_count={} → {}",
            state.click_count,
            if state.click_count == 2 {
                "DOUBLE-CLICK"
            } else if state.click_count == 3 {
                "TRIPLE-CLICK"
            } else {
                "single"
            }
        );

        state.click_count
    }
}

// ============================================================================
// Keyboard Fusion State
// ============================================================================

/// Keyboard fusion state - tracks pressed keys and modifiers
#[derive(Debug)]
pub struct KeyboardFusionState {
    /// Currently pressed keys
    pressed_keys: HashSet<KeyCode>,
    pressed_physical_keys: HashSet<crate::core::event::PhysicalKeyCode>,
    /// Current modifier state
    modifiers: Modifiers,
}

impl Default for KeyboardFusionState {
    fn default() -> Self {
        Self::new()
    }
}

impl KeyboardFusionState {
    pub fn new() -> Self {
        Self {
            pressed_keys: HashSet::new(),
            pressed_physical_keys: HashSet::new(),
            modifiers: Modifiers::default(),
        }
    }
}

// ============================================================================
// Scroll Fusion State
// ============================================================================

/// Scroll fusion state - handles wheel delta normalization
#[derive(Debug)]
pub struct ScrollFusionState {
    // Reserved for future accumulation of sub-pixel deltas
    _residual_x: f32,
    _residual_y: f32,
    /// SPEC-HAL-033 Phase 2: Device scale factor for proper DPI-aware scroll normalization
    scale_factor: f32,
}

impl Default for ScrollFusionState {
    fn default() -> Self {
        Self {
            _residual_x: 0.0,
            _residual_y: 0.0,
            scale_factor: 1.0,
        }
    }
}

impl ScrollFusionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Set the device scale factor for DPI-aware scroll normalization
    pub fn set_scale_factor(&mut self, scale: f32) {
        self.scale_factor = scale;
    }

    /// Normalize wheel delta to logical pixels
    /// SPEC-HAL-033 Phase 2: Multiplies by scale_factor for consistent behavior across DPI settings
    /// SPEC-HAL-036 Phase 4: Device-aware normalization for responsive scroll
    fn normalize_delta(&self, delta: WheelDelta, device: PointerDevice) -> (f32, f32) {
        match delta {
            WheelDelta::Lines(x, y) => {
                // Use device-appropriate pixels-per-line constant
                let base_px_per_line = match device {
                    PointerDevice::Mouse => MOUSE_WHEEL_PIXELS_PER_LINE,
                    PointerDevice::Touchpad => TOUCHPAD_PIXELS_PER_LINE,
                };
                let px_per_line = base_px_per_line * self.scale_factor;
                (x * px_per_line, y * px_per_line)
            }
            WheelDelta::Pixels(x, y) => (x, y),
        }
    }
}

// ============================================================================
// Shortcut Fusion State
// ============================================================================

/// A key chord (single key + modifiers) for shortcut matching
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: crate::core::event::BindingKey,
    pub modifiers: Modifiers,
}

impl KeyChord {
    pub fn new(key: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            key: key.into(),
            modifiers,
        }
    }

    pub fn physical(
        key: crate::core::event::PhysicalKeyCode,
        modifiers: Modifiers,
    ) -> Result<Self, crate::core::event::UnknownPhysicalKey> {
        crate::runtime::actions::Shortcut::from_physical_key_and_modifiers(key, modifiers).map(
            |shortcut| Self {
                key: shortcut.key,
                modifiers,
            },
        )
    }
}

/// Registered shortcut entry
#[derive(Debug, Clone)]
struct ShortcutEntry {
    id: ShortcutId,
    scope: ShortcutScope,
    chord: KeyChord,
}

/// Shortcut fusion state - manages registered shortcuts
#[derive(Debug)]
pub struct ShortcutFusionState {
    shortcuts: Vec<ShortcutEntry>,
    next_id: u32,
}

impl Default for ShortcutFusionState {
    fn default() -> Self {
        Self::new()
    }
}

impl ShortcutFusionState {
    pub fn new() -> Self {
        Self {
            shortcuts: Vec::new(),
            next_id: 1,
        }
    }

    /// Register a new shortcut, returns its ID
    pub fn register(&mut self, scope: ShortcutScope, chord: KeyChord) -> ShortcutId {
        let id = ShortcutId(self.next_id);
        self.next_id += 1;
        self.shortcuts.push(ShortcutEntry { id, scope, chord });
        id
    }

    /// Unregister a shortcut by ID
    pub fn unregister(&mut self, id: ShortcutId) {
        self.shortcuts.retain(|e| e.id != id);
    }

    /// Find matching shortcut for key + modifiers
    /// Note: Character keys are compared case-insensitively to handle Shift+key combinations
    fn find_match(
        &self,
        key: KeyCode,
        physical: crate::core::event::PhysicalKeyCode,
        modifiers: Modifiers,
    ) -> Option<(ShortcutId, ShortcutScope)> {
        for entry in &self.shortcuts {
            let keys_match = match (entry.chord.key, key) {
                // Compare characters case-insensitively (Shift+p gives 'P', but shortcut may register 'p')
                (
                    crate::core::event::BindingKey::Logical(KeyCode::Character(a)),
                    KeyCode::Character(b),
                ) => a.to_ascii_lowercase() == b.to_ascii_lowercase(),
                // All other keys compare exactly
                (binding, _) => binding.matches(key, physical),
            };
            if keys_match && entry.chord.modifiers == modifiers {
                return Some((entry.id, entry.scope));
            }
        }
        None
    }
}

// ============================================================================
// Input Fusion Engine
// ============================================================================

// ----------------------------------------------------------------------------
// SPEC-HAL-036: Coalescing Flush Boundary Rules
// ----------------------------------------------------------------------------
// These rules define when pending coalesced events must be flushed to maintain
// correct event ordering and responsive interaction.
//
// Flush pending_pointer on:
//   - PointerDown, PointerUp  (click needs final position)
//   - Key, TextInput          (keyboard interaction boundary)
//   - Focus                   (window context change)
//   - Scroll                  (cross-type flush)
//   - DragStart               (transition from move to drag)
//   - Frame-end               (caller calls flush() before render)
//
// Flush pending_scroll on:
//   - PointerMoved            (cross-type flush)
//   - PointerDown, PointerUp  (interaction boundary)
//   - Key, TextInput          (keyboard interaction boundary)
//   - Focus                   (window context change)
//   - Frame-end               (caller calls flush() before render)
//
// Flush pending_drag_update on (Phase 3):
//   - Any non-drag event      (DragEnd, button release, key, etc.)
//   - Frame-end               (caller calls flush() before render)
// ----------------------------------------------------------------------------

/// Static counter for fusion instance IDs
static NEXT_FUSION_ID: AtomicU64 = AtomicU64::new(1);

trait FusionOutput {
    fn emit(&mut self, event: FusedInputEvent, press: Option<Rc<PhysicalGesturePress>>);

    fn emit_touch(
        &mut self,
        event: FusedInputEvent,
        contact: &TouchContactState,
        _position: Point,
    ) {
        self.emit(event, contact.press.clone());
    }

    fn emit_key(&mut self, event: FusedInputEvent, _global_shortcut: Option<ShortcutId>) {
        self.emit(event, None);
    }
}

impl FusionOutput for Vec<FusedInputEvent> {
    fn emit(&mut self, event: FusedInputEvent, press: Option<Rc<PhysicalGesturePress>>) {
        assert!(
            press.is_none(),
            "INV-ABSOLUTE-PRESS-QUALIFIED-DRAIN: legacy drain loses physical lineage"
        );
        self.push(event);
    }
}

impl FusionOutput for Vec<SessionQualifiedInput> {
    fn emit(&mut self, event: FusedInputEvent, press: Option<Rc<PhysicalGesturePress>>) {
        self.push(SessionQualifiedInput::emitted(event, press));
    }

    fn emit_touch(&mut self, event: FusedInputEvent, contact: &TouchContactState, position: Point) {
        let time = match &event {
            FusedInputEvent::TouchDown { time, .. } | FusedInputEvent::TouchMove { time, .. }
            | FusedInputEvent::TouchUp { time, .. } | FusedInputEvent::TouchCancel { time, .. } => *time,
            _ => unreachable!("only native touch samples use this envelope"),
        };
        let motion = contact.press.as_ref().map(|press| QualifiedTouchMotion {
            press: press.touch_identity(),
            native_contact: contact.native_contact,
            origin: press.position,
            previous: contact.pos,
            position,
            first_contact: contact.first_contact,
            origin_time: contact.origin_time,
            previous_time: contact.time,
            time,
        });
        self.push(SessionQualifiedInput::emitted_touch(
            event,
            contact.press.clone(),
            motion,
        ));
    }

    fn emit_key(&mut self, event: FusedInputEvent, global_shortcut: Option<ShortcutId>) {
        self.push(SessionQualifiedInput::emitted_key(event, global_shortcut));
    }
}

/// Contact identity and original press survive every event until terminal delivery.
#[derive(Debug)]
struct TouchContactState {
    pointer_id: PointerId,
    native_contact: u64,
    first_contact: bool,
    pos: Point,
    origin_time: Instant,
    time: Instant,
    press: Option<Rc<PhysicalGesturePress>>,
}

struct TouchPressQualification<'a> {
    domain: InteractionDomainId,
    registry: &'a mut GestureRegistry,
    tree: &'a crate::core::widget_tree::WidgetTree,
}

/// Main input fusion engine - converts raw events to fused events
#[derive(Debug)]
pub struct InputFusion {
    physical_presses: [Option<Rc<PhysicalGesturePress>>; 3],
    incoming_press: Option<Rc<PhysicalGesturePress>>,
    touch_contacts: HashMap<(WindowId, u64), TouchContactState>,
    next_touch_pointer_id: u64,
    /// Unique ID for this fusion instance (for persistence debugging)
    fusion_id: u64,
    pointer_state: PointerFusionState,
    keyboard_state: KeyboardFusionState,
    scroll_state: ScrollFusionState,
    shortcut_state: ShortcutFusionState,
    last_time: Instant,
    /// SPEC-HAL-151: Current window being processed (set at start of process_raw)
    current_window_id: WindowId,

    // SPEC-HAL-036: Coalescing state
    /// Pending pointer move to coalesce
    pending_pointer: Option<PendingPointer>,
    /// Pending relative pointer motion to coalesce
    pending_relative_pointer: Option<PendingRelativePointerMotion>,
    /// Pending scroll to coalesce
    pending_scroll: Option<PendingScroll>,
    /// SPEC-HAL-036 Phase 3: Pending drag update to coalesce
    pending_drag_update: Option<PendingDragUpdate>,
}

impl Default for InputFusion {
    fn default() -> Self {
        Self::new()
    }
}

impl InputFusion {
    pub fn new() -> Self {
        let fusion_id = NEXT_FUSION_ID.fetch_add(1, Ordering::Relaxed);
        tracing::debug!("[FUSION] created id={}", fusion_id);

        Self {
            physical_presses: Default::default(),
            incoming_press: None,
            touch_contacts: HashMap::new(),
            next_touch_pointer_id: 1,
            fusion_id,
            pointer_state: PointerFusionState::new(),
            keyboard_state: KeyboardFusionState::new(),
            scroll_state: ScrollFusionState::new(),
            shortcut_state: ShortcutFusionState::new(),
            last_time: Instant::now(),
            // SPEC-HAL-151: Default to PRIMARY window
            current_window_id: WindowId::PRIMARY,
            // SPEC-HAL-036: Initialize coalescing state
            pending_pointer: None,
            pending_relative_pointer: None,
            pending_scroll: None,
            pending_drag_update: None,
        }
    }

    /// Register a keyboard shortcut
    pub fn register_shortcut(&mut self, scope: ShortcutScope, chord: KeyChord) -> ShortcutId {
        self.shortcut_state.register(scope, chord)
    }

    /// Unregister a keyboard shortcut
    pub fn unregister_shortcut(&mut self, id: ShortcutId) {
        self.shortcut_state.unregister(id);
    }

    /// SPEC-HAL-033 Phase 2: Set device scale factor for DPI-aware scroll normalization
    /// Call this when scale factor changes (e.g., window moved to different display)
    pub fn set_scale_factor(&mut self, scale: f32) {
        self.scroll_state.set_scale_factor(scale);
    }

    // ========================================================================
    // SPEC-HAL-036: Coalescing API
    // ========================================================================

    /// SPEC-HAL-036: Flush any pending coalesced events
    ///
    /// Emits pending pointer move and scroll events. Called automatically
    /// before non-coalescable events (button press/release, keys, etc.)
    pub fn flush(&mut self, out: &mut Vec<FusedInputEvent>) {
        self.flush_impl(out);
    }

    fn flush_impl(&mut self, out: &mut impl FusionOutput) {
        self.flush_pending_pointer(out);
        self.flush_pending_scroll(out);
        self.flush_pending_drag_update(out);
        self.flush_pending_relative_pointer(out);
    }

    pub fn flush_with_gesture_sessions(&mut self, out: &mut Vec<SessionQualifiedInput>) {
        self.flush_impl(out);
    }

    #[cfg(any(target_os = "ios", test))]
    pub(crate) fn cancel_touch_contacts_with_gesture_sessions(
        &mut self,
        window_id: WindowId,
        time: Instant,
        out: &mut Vec<SessionQualifiedInput>,
    ) {
        self.flush_impl(out);
        self.cancel_touch_contacts(window_id, time, out);
    }

    pub fn process_frame_with_gesture_sessions(
        &mut self,
        _frame_time: &FrameTime,
        _ticker: &mut EngineTicker,
        out: &mut Vec<SessionQualifiedInput>,
    ) {
        self.flush_impl(out);
    }

    pub fn process_raw_with_gesture_sessions(
        &mut self,
        raw: RawInputEvent,
        interaction_domain: InteractionDomainId,
        registry: &mut GestureRegistry,
        tree: &crate::core::widget_tree::WidgetTree,
        relative_gate: Option<&RelativePointerIngressGate>,
        out: &mut Vec<SessionQualifiedInput>,
    ) {
        if let RawInputEvent::PointerButton {
            window_id,
            button,
            state: RawButtonState::Pressed,
            position,
            ..
        } = &raw
        {
            self.incoming_press = Some(registry.reserve_physical_press(
                interaction_domain,
                *window_id,
                PointerId::MOUSE,
                PointerType::Mouse,
                *button,
                *position,
                tree.geometry_causal_cutoff(),
            ));
        }
        self.process_raw_impl(
            raw,
            relative_gate,
            out,
            Some(TouchPressQualification {
                domain: interaction_domain,
                registry,
                tree,
            }),
        );
        assert!(
            self.incoming_press.is_none(),
            "INV-ABSOLUTE-PRESS-NORMALIZATION"
        );
    }

    fn current_physical_press(&self) -> Option<Rc<PhysicalGesturePress>> {
        self.pointer_state
            .active_drag
            .as_ref()
            .and_then(|drag| {
                self.physical_presses[PointerFusionState::button_index(drag.button)].clone()
            })
            .or_else(|| {
                [
                    PointerButton::Primary,
                    PointerButton::Secondary,
                    PointerButton::Auxiliary,
                ]
                .into_iter()
                .find_map(|button| {
                    self.pointer_state
                        .buttons
                        .is_pressed(button)
                        .then(|| {
                            self.physical_presses[PointerFusionState::button_index(button)].clone()
                        })
                        .flatten()
                })
            })
    }

    /// SPEC-HAL-042: Called once per frame, before layout/render.
    /// Flushes all coalesced batches (pointer, scroll, drag).
    /// Pushes fused events into `out` in correct order.
    #[allow(unused_variables)]
    pub fn process_frame(
        &mut self,
        frame_time: &FrameTime,
        ticker: &mut EngineTicker,
        out: &mut Vec<FusedInputEvent>,
    ) {
        self.flush(out);
    }

    /// SPEC-HAL-036: Check if scroll should be flushed due to time threshold
    fn should_flush_scroll(&self, now: Instant) -> bool {
        if let Some(ref pending) = self.pending_scroll {
            let elapsed = now.duration_since(pending.time).as_millis();
            elapsed >= SCROLL_FLUSH_THRESHOLD_MS
        } else {
            false
        }
    }

    /// SPEC-HAL-036: Flush only pending pointer move
    fn flush_pending_pointer(&mut self, out: &mut impl FusionOutput) {
        if let Some(pending) = self.pending_pointer.take() {
            out.emit(
                FusedInputEvent::PointerMove {
                    window_id: pending.window_id,
                    time: pending.time,
                    pos: pending.pos,
                    delta: pending.delta,
                    device: pending.device,
                    buttons: pending.buttons,
                },
                pending.press,
            );
        }
    }

    fn flush_pending_relative_pointer(&mut self, out: &mut impl FusionOutput) {
        if let Some(pending) = self.pending_relative_pointer.take() {
            out.emit(
                FusedInputEvent::RelativePointerMotion {
                    lease_id: pending.lease_id,
                    window_id: pending.window_id,
                    pointer_id: pending.pointer_id,
                    time: pending.time,
                    delta: pending.delta,
                },
                pending.press,
            );
        }
    }

    pub(crate) fn reconcile_relative_ingress(
        &mut self,
        relative_gate: Option<&RelativePointerIngressGate>,
    ) {
        if self
            .pending_relative_pointer
            .as_ref()
            .is_some_and(|pending| {
                !relative_gate.is_some_and(|gate| {
                    gate.admits(
                        pending.lease_id,
                        pending.window_id,
                        pending.pointer_id,
                        pending.normalization_epoch,
                    )
                })
            })
        {
            self.pending_relative_pointer = None;
        }
    }

    pub(crate) fn discard_pending_relative_pointer(&mut self) {
        self.pending_relative_pointer = None;
    }

    /// SPEC-HAL-036: Flush only pending scroll
    fn flush_pending_scroll(&mut self, out: &mut impl FusionOutput) {
        if let Some(pending) = self.pending_scroll.take() {
            out.emit(
                FusedInputEvent::Scroll {
                    window_id: pending.window_id,
                    time: pending.time,
                    pos: pending.pos,
                    delta: pending.delta,
                    source: pending.source,
                },
                None,
            );
        }
    }

    /// SPEC-HAL-036 Phase 3: Flush only pending drag update
    fn flush_pending_drag_update(&mut self, out: &mut impl FusionOutput) {
        if let Some(pending) = self.pending_drag_update.take() {
            out.emit(
                FusedInputEvent::DragUpdate {
                    window_id: pending.window_id,
                    time: pending.time,
                    origin: pending.origin,
                    current: pending.pos,
                    delta: pending.delta,
                    button: pending.button,
                },
                pending.press,
            );
        }
    }

    /// Process a raw input event, producing zero or more fused events
    pub fn process_raw(&mut self, raw: RawInputEvent, out: &mut Vec<FusedInputEvent>) {
        self.process_raw_with_relative_ingress(raw, None, out);
    }

    /// Process raw input with an externally owned relative-pointer ingress gate.
    ///
    /// InputFusion validates and orders against this immutable gate but never
    /// creates, stores, renews, or terminates the semantic lease.
    pub fn process_raw_with_relative_ingress(
        &mut self,
        raw: RawInputEvent,
        relative_gate: Option<&RelativePointerIngressGate>,
        out: &mut Vec<FusedInputEvent>,
    ) {
        self.process_raw_impl(raw, relative_gate, out, None);
    }

    fn process_raw_impl(
        &mut self,
        raw: RawInputEvent,
        relative_gate: Option<&RelativePointerIngressGate>,
        out: &mut impl FusionOutput,
        qualification: Option<TouchPressQualification<'_>>,
    ) {
        self.reconcile_relative_ingress(relative_gate);
        let raw = match raw {
            RawInputEvent::Touch {
                window_id,
                time,
                contact_id,
                phase,
                pos,
            } => {
                self.flush_impl(out);
                self.current_window_id = window_id;
                self.process_touch_contact(
                    window_id,
                    time,
                    contact_id,
                    phase,
                    pos,
                    qualification,
                    out,
                );
                self.last_time = Instant::now();
                return;
            }
            raw => raw,
        };
        if let RawInputEvent::FocusChanged {
            window_id,
            time,
            focused: false,
        } = &raw
        {
            self.flush_impl(out);
            self.cancel_touch_contacts(*window_id, *time, out);
        }
        match raw {
            RawInputEvent::RelativePointerMotion {
                lease_id,
                normalization_epoch,
                window_id,
                pointer_id,
                time,
                delta,
            } => self.process_relative_pointer_motion(
                lease_id,
                normalization_epoch,
                window_id,
                pointer_id,
                time,
                delta,
                relative_gate,
                out,
            ),
            RawInputEvent::PointerMoved { window_id, .. }
                if relative_gate
                    .is_some_and(|gate| gate.suppresses_absolute(window_id, PointerId::MOUSE)) => {}
            RawInputEvent::FocusChanged {
                window_id,
                time,
                focused: false,
            } if relative_gate
                .is_some_and(|gate| gate.suppresses_absolute(window_id, PointerId::MOUSE)) =>
            {
                self.pending_relative_pointer = None;
                self.flush_impl(out);
                self.current_window_id = window_id;
                self.pointer_state.active_drag = None;
                self.pointer_state.buttons = PointerButtonsState::default();
                self.pointer_state.drag_state = Default::default();
                out.emit(
                    FusedInputEvent::FocusChanged {
                        window_id,
                        time,
                        focused: false,
                    },
                    None,
                );
            }
            RawInputEvent::ScaleFactorChanged { window_id, .. }
                if relative_gate
                    .is_some_and(|gate| gate.suppresses_absolute(window_id, PointerId::MOUSE)) =>
            {
                self.pending_relative_pointer = None;
            }
            raw => {
                self.flush_pending_relative_pointer(out);
                self.process_non_relative_raw(raw, out);
            }
        }
    }

    fn process_touch_contact(
        &mut self,
        window_id: WindowId,
        time: Instant,
        contact_id: u64,
        phase: TouchPhase,
        pos: Point,
        qualification: Option<TouchPressQualification<'_>>,
        out: &mut impl FusionOutput,
    ) {
        let key = (window_id, contact_id);
        let finite_position = pos.x.is_finite() && pos.y.is_finite();
        match phase {
            TouchPhase::Started => {
                if !finite_position || self.touch_contacts.get(&key).is_some_and(|c| time < c.time) {
                    return;
                }
                if let Some(previous) = self.touch_contacts.remove(&key) {
                    let previous_pos = previous.pos;
                    Self::emit_touch_cancel(window_id, time, previous, previous_pos, out);
                }
                let pointer_id = PointerId(self.next_touch_pointer_id);
                self.next_touch_pointer_id = self
                    .next_touch_pointer_id
                    .checked_add(1)
                    .expect("INV-TOUCH-POINTER-NONREUSE");
                let press = qualification.map(|qualification| {
                    qualification.registry.reserve_physical_press(
                        qualification.domain,
                        window_id,
                        pointer_id,
                        PointerType::Touch,
                        PointerButton::Primary,
                        pos,
                        qualification.tree.geometry_causal_cutoff(),
                    )
                });
                let contact = TouchContactState {
                    pointer_id,
                    native_contact: contact_id,
                    first_contact: !self
                        .touch_contacts
                        .keys()
                        .any(|(window, _)| *window == window_id),
                    pos,
                    origin_time: time,
                    time,
                    press,
                };
                out.emit_touch(
                    FusedInputEvent::TouchDown {
                        window_id,
                        time,
                        pointer_id,
                        pos,
                        button: PointerButton::Primary,
                        click_count: 1,
                    },
                    &contact,
                    pos,
                );
                self.touch_contacts.insert(key, contact);
            }
            TouchPhase::Moved => {
                let Some(contact) = self.touch_contacts.get_mut(&key) else {
                    return;
                };
                if time < contact.time || !finite_position {
                    return;
                }
                let delta = (pos.x - contact.pos.x, pos.y - contact.pos.y);
                let mut buttons = PointerButtonsState::new();
                buttons.set(PointerButton::Primary, true);
                out.emit_touch(
                    FusedInputEvent::TouchMove {
                        window_id,
                        time,
                        pointer_id: contact.pointer_id,
                        pos,
                        delta,
                        buttons,
                    },
                    contact,
                    pos,
                );
                contact.pos = pos;
                contact.time = time;
            }
            TouchPhase::Ended | TouchPhase::Cancelled => {
                let Some(contact) = self.touch_contacts.remove(&key) else {
                    return;
                };
                if phase == TouchPhase::Cancelled || time < contact.time || !finite_position {
                    let last_time = contact.time;
                    let last_position = contact.pos;
                    Self::emit_touch_cancel(
                        window_id, time.max(last_time), contact,
                        if phase == TouchPhase::Cancelled && finite_position { pos } else { last_position },
                        out,
                    );
                } else {
                    out.emit_touch(
                        FusedInputEvent::TouchUp {
                            window_id,
                            time,
                            pointer_id: contact.pointer_id,
                            pos,
                            button: PointerButton::Primary,
                        },
                        &contact,
                        pos,
                    );
                    if let Some(press) = contact.press {
                        press.physical_release.set(true);
                    }
                }
            }
        }
    }

    fn emit_touch_cancel(
        window_id: WindowId,
        time: Instant,
        contact: TouchContactState,
        pos: Point,
        out: &mut impl FusionOutput,
    ) {
        out.emit_touch(
            FusedInputEvent::TouchCancel {
                window_id,
                time,
                pointer_id: contact.pointer_id,
                pos,
            },
            &contact,
            pos,
        );
        if let Some(press) = contact.press {
            press.physical_release.set(true);
        }
    }

    fn cancel_touch_contacts(
        &mut self,
        window_id: WindowId,
        time: Instant,
        out: &mut impl FusionOutput,
    ) {
        let mut contacts: Vec<_> = self
            .touch_contacts
            .iter()
            .filter(|((window, _), _)| *window == window_id)
            .map(|(key, contact)| (*key, contact.pointer_id))
            .collect();
        contacts.sort_by_key(|(_, pointer_id)| pointer_id.0);
        for (key, _) in contacts {
            let contact = self
                .touch_contacts
                .remove(&key)
                .expect("active touch contact remains during cancellation");
            let pos = contact.pos;
            Self::emit_touch_cancel(window_id, time, contact, pos, out);
        }
    }

    fn process_non_relative_raw(&mut self, raw: RawInputEvent, out: &mut impl FusionOutput) {
        // SPEC-HAL-151: Extract and track window_id for all subsequent fused events
        self.current_window_id = raw.window_id();

        match raw {
            RawInputEvent::Touch { .. } => unreachable!("touch is normalized at shared ingress"),
            RawInputEvent::RelativePointerMotion { .. } => {}
            RawInputEvent::PointerMoved {
                window_id: _,
                time,
                device,
                position,
                delta,
            } => {
                // SPEC-HAL-036: Flush pending scroll (cross-type flush)
                self.flush_pending_scroll(out);
                self.process_pointer_moved(time, device, position, delta, out);
            }
            RawInputEvent::PointerButton {
                window_id: _,
                time,
                device,
                button,
                state,
                position,
            } => {
                // SPEC-HAL-036: Flush all pending events before button press/release
                self.flush_impl(out);
                self.process_pointer_button(time, device, button, state.into(), position, out);
            }
            RawInputEvent::PointerWheel {
                window_id: _,
                time,
                device,
                delta,
                position,
            } => {
                // SPEC-HAL-036: Flush pending pointer (cross-type flush)
                self.flush_pending_pointer(out);
                self.process_pointer_wheel(time, device, delta, position, out);
            }
            RawInputEvent::Key {
                window_id: _,
                time,
                key_code,
                physical_key,
                reservation,
                state,
                repeat,
                modifiers,
            } => {
                // SPEC-HAL-036: Flush all pending events before key events
                self.flush_impl(out);
                self.process_key(
                    time,
                    key_code,
                    physical_key,
                    reservation,
                    state,
                    repeat,
                    modifiers,
                    out,
                );
            }
            RawInputEvent::TextInput {
                window_id: _,
                time,
                text,
                modifiers,
            } => {
                // SPEC-HAL-036: Flush all pending events before text input
                self.flush_impl(out);
                out.emit(
                    FusedInputEvent::TextInput {
                        window_id: self.current_window_id,
                        time,
                        text,
                        modifiers,
                    },
                    None,
                );
            }
            RawInputEvent::FocusChanged {
                window_id: _,
                time,
                focused,
            } => {
                // SPEC-HAL-036: Flush all pending events before focus change
                self.flush_impl(out);
                // Cancel pointer interaction state on focus loss
                if !focused {
                    self.cancel_pointer_interaction(time, out);
                    self.keyboard_state.pressed_keys.clear();
                    self.keyboard_state.pressed_physical_keys.clear();
                    self.keyboard_state.modifiers = Modifiers::default();
                }
                out.emit(
                    FusedInputEvent::FocusChanged {
                        window_id: self.current_window_id,
                        time,
                        focused,
                    },
                    None,
                );
            }
            RawInputEvent::Resize {
                window_id: _,
                width,
                height,
            } => {
                // SPEC-HAL-044: Resize events update constraints AND notify screens
                // INV-RESIZE-SIGNAL-1: Every resize must be delivered to active Screen
                out.emit(
                    FusedInputEvent::Resize {
                        window_id: self.current_window_id,
                        width,
                        height,
                    },
                    None,
                );
            }
            RawInputEvent::ScaleFactorChanged { .. } => {
                // SPEC-HAL-266: Handled by run_ui_loop directly, not fused.
            }
        }
        self.last_time = Instant::now();
    }

    #[allow(clippy::too_many_arguments)]
    fn process_relative_pointer_motion(
        &mut self,
        lease_id: RelativePointerLeaseId,
        normalization_epoch: RelativePointerNormalizationEpoch,
        window_id: WindowId,
        pointer_id: PointerId,
        time: Instant,
        raw_delta: (f32, f32),
        relative_gate: Option<&RelativePointerIngressGate>,
        out: &mut impl FusionOutput,
    ) {
        let Some(gate) = relative_gate.copied() else {
            return;
        };
        if !gate.admits(lease_id, window_id, pointer_id, normalization_epoch) {
            return;
        }
        let Ok(delta) = RelativeLogicalDelta::try_new(raw_delta.0, raw_delta.1) else {
            return;
        };

        if let Some(pending) = self.pending_relative_pointer.as_ref() {
            let same_stream = pending.lease_id == lease_id
                && pending.normalization_epoch == normalization_epoch
                && pending.window_id == window_id
                && pending.pointer_id == pointer_id;
            if same_stream {
                if time < pending.time {
                    return;
                }
                if let Ok(sum) = RelativeLogicalDelta::try_new(
                    pending.delta.x() + delta.x(),
                    pending.delta.y() + delta.y(),
                ) {
                    let pending = self
                        .pending_relative_pointer
                        .as_mut()
                        .expect("relative pending state was just observed");
                    pending.delta = sum;
                    pending.time = time;
                    return;
                }

                self.flush_pending_relative_pointer(out);
                self.pending_relative_pointer = Some(PendingRelativePointerMotion {
                    press: self.current_physical_press(),
                    lease_id,
                    normalization_epoch,
                    window_id,
                    pointer_id,
                    time,
                    delta,
                });
                return;
            }
        }

        self.flush_impl(out);
        self.pending_relative_pointer = Some(PendingRelativePointerMotion {
            press: self.current_physical_press(),
            lease_id,
            normalization_epoch,
            window_id,
            pointer_id,
            time,
            delta,
        });
    }

    fn process_pointer_moved(
        &mut self,
        time: Instant,
        device: PointerDevice,
        position: Point,
        delta: (f32, f32),
        out: &mut impl FusionOutput,
    ) {
        self.pointer_state.current_pos = position;
        self.pointer_state.device = device;

        // Check for drag start/update
        if let Some(ref drag) = self.pointer_state.active_drag {
            // SPEC-HAL-036 Phase 3: Coalesce drag updates instead of immediate emit
            // Extract values before calling method to avoid borrow conflict
            let origin = drag.origin;
            let button = drag.button;
            self.coalesce_drag_update(time, origin, position, delta, button);
            // Update last_pos after coalescing
            if let Some(ref mut drag) = self.pointer_state.active_drag {
                drag.last_pos = position;
            }
        } else {
            // Check if any button press should start a drag
            let mut drag_started = false;
            for (idx, drag_state) in self.pointer_state.drag_state.iter().enumerate() {
                if let (Some(press_pos), Some(_press_time)) =
                    (drag_state.press_pos, drag_state.press_time)
                {
                    let distance = position.distance(press_pos);
                    if distance >= DRAG_DISTANCE_THRESHOLD {
                        // SPEC-HAL-036: Flush pending pointer before drag start
                        self.flush_pending_pointer(out);

                        // Start drag
                        let button = match idx {
                            0 => PointerButton::Primary,
                            1 => PointerButton::Secondary,
                            _ => PointerButton::Auxiliary,
                        };

                        out.emit(
                            FusedInputEvent::DragStart {
                                window_id: self.current_window_id,
                                time,
                                origin: press_pos,
                                button,
                            },
                            self.physical_presses[idx].clone(),
                        );

                        out.emit(
                            FusedInputEvent::DragUpdate {
                                window_id: self.current_window_id,
                                time,
                                origin: press_pos,
                                current: position,
                                delta,
                                button,
                            },
                            self.physical_presses[idx].clone(),
                        );

                        self.pointer_state.active_drag = Some(ActiveDrag {
                            button,
                            origin: press_pos,
                            last_pos: position,
                        });

                        // Clear drag state for this button
                        self.pointer_state.drag_state[idx] = ButtonDragState::default();
                        drag_started = true;
                        break;
                    }
                }
            }

            // SPEC-HAL-036 Phase 2: Coalesce pointer moves when not dragging
            // Events are flushed by caller before render (frame-end flush pattern)
            if !drag_started {
                self.coalesce_pointer_move(time, device, position, delta);
            }
        }
    }

    /// SPEC-HAL-036: Coalesce pointer move event
    fn coalesce_pointer_move(
        &mut self,
        time: Instant,
        device: PointerDevice,
        position: Point,
        delta: (f32, f32),
    ) {
        if let Some(ref mut pending) = self.pending_pointer {
            // Merge with existing pending: pos replaces, delta accumulates, time updates
            pending.pos = position;
            pending.delta.0 += delta.0;
            pending.delta.1 += delta.1;
            pending.time = time;
            pending.device = device;
            pending.buttons = self.pointer_state.buttons;
        } else {
            // Store new pending
            self.pending_pointer = Some(PendingPointer {
                press: self.current_physical_press(),
                window_id: self.current_window_id,
                pos: position,
                delta,
                time,
                device,
                buttons: self.pointer_state.buttons,
            });
        }
    }

    /// SPEC-HAL-036 Phase 3: Coalesce drag update event
    fn coalesce_drag_update(
        &mut self,
        time: Instant,
        origin: Point,
        pos: Point,
        delta: (f32, f32),
        button: PointerButton,
    ) {
        if let Some(ref mut pending) = self.pending_drag_update {
            // Merge with existing pending: pos replaces, delta accumulates, time updates
            pending.pos = pos;
            pending.delta.0 += delta.0;
            pending.delta.1 += delta.1;
            pending.time = time;
        } else {
            // Store new pending
            self.pending_drag_update = Some(PendingDragUpdate {
                press: self.physical_presses[PointerFusionState::button_index(button)].clone(),
                window_id: self.current_window_id,
                origin,
                pos,
                delta,
                button,
                time,
            });
        }
    }

    fn process_pointer_button(
        &mut self,
        time: Instant,
        device: PointerDevice,
        button: PointerButton,
        state: ButtonState,
        position: Point,
        out: &mut impl FusionOutput,
    ) {
        // SPEC-HAL-036 Phase 4: Hard-flush all coalesced batches before any button event
        // This ensures crisp click/double-click detection without delay
        self.flush_pending_pointer(out);
        self.flush_pending_scroll(out);
        self.flush_pending_drag_update(out);

        self.pointer_state.current_pos = position;
        self.pointer_state.device = device;
        let idx = PointerFusionState::button_index(button);

        match state {
            ButtonState::Pressed => {
                self.physical_presses[idx] = self.incoming_press.take();
                // Log fusion_id on EVERY PointerDown to prove persistence
                tracing::debug!(
                    "[FUSION {}] PointerDown device={:?} button={:?}",
                    self.fusion_id,
                    device,
                    button
                );

                self.pointer_state.buttons.set(button, true);

                // Record for potential drag
                self.pointer_state.drag_state[idx] = ButtonDragState {
                    press_pos: Some(position),
                    press_time: Some(time),
                };

                // Check click count
                // SPEC-HAL-DoubleClick: Pass device for per-device click tracking
                let click_count = self
                    .pointer_state
                    .check_click(device, button, position, time);

                out.emit(
                    FusedInputEvent::PointerDown {
                        window_id: self.current_window_id,
                        time,
                        pos: position,
                        device,
                        button,
                        click_count,
                    },
                    self.physical_presses[idx].clone(),
                );
            }
            ButtonState::Released => {
                self.pointer_state.buttons.set(button, false);

                // Clear drag state for this button
                self.pointer_state.drag_state[idx] = ButtonDragState::default();

                // If active drag exists for this button, end it
                if let Some(ref drag) = self.pointer_state.active_drag {
                    if drag.button == button {
                        out.emit(
                            FusedInputEvent::DragEnd {
                                window_id: self.current_window_id,
                                time,
                                origin: drag.origin,
                                current: position,
                                button,
                                cancelled: false,
                            },
                            self.physical_presses[idx].clone(),
                        );
                        self.pointer_state.active_drag = None;
                    }
                }

                out.emit(
                    FusedInputEvent::PointerUp {
                        window_id: self.current_window_id,
                        time,
                        pos: position,
                        device,
                        button,
                    },
                    self.physical_presses[idx].clone(),
                );
                if let Some(press) = self.physical_presses[idx].take() {
                    press.physical_release.set(true);
                }
            }
        }
    }

    fn process_pointer_wheel(
        &mut self,
        time: Instant,
        device: PointerDevice,
        delta: WheelDelta,
        position: Point,
        out: &mut impl FusionOutput,
    ) {
        // SPEC-HAL-036 Phase 4: Pass device for device-aware normalization
        let (dx, dy) = self.scroll_state.normalize_delta(delta, device);
        let source = match device {
            PointerDevice::Mouse => ScrollSource::Wheel,
            PointerDevice::Touchpad => ScrollSource::Touchpad,
        };

        // SPEC-HAL-036 Phase 2: Coalesce scroll events within time threshold
        // Events are flushed by caller before render (frame-end flush pattern)
        self.coalesce_scroll(time, position, (dx, dy), source, out);
    }

    /// SPEC-HAL-036: Coalesce scroll event
    fn coalesce_scroll(
        &mut self,
        time: Instant,
        pos: Point,
        delta: (f32, f32),
        source: ScrollSource,
        out: &mut impl FusionOutput,
    ) {
        // Check if we should flush due to time threshold before merging
        if self.should_flush_scroll(time) {
            self.flush_pending_scroll(out);
        }

        if let Some(ref mut pending) = self.pending_scroll {
            // Only merge if same source type
            if pending.source == source {
                // Merge: accumulate delta, update time and position
                pending.delta.0 += delta.0;
                pending.delta.1 += delta.1;
                pending.time = time;
                pending.pos = pos;
            } else {
                // Different source - flush pending and store new
                self.flush_pending_scroll(out);
                self.pending_scroll = Some(PendingScroll {
                    window_id: self.current_window_id,
                    delta,
                    time,
                    pos,
                    source,
                });
            }
        } else {
            // Store new pending
            self.pending_scroll = Some(PendingScroll {
                window_id: self.current_window_id,
                delta,
                time,
                pos,
                source,
            });
        }
    }

    fn process_key(
        &mut self,
        time: Instant,
        key_code: KeyCode,
        physical_key: crate::core::event::PhysicalKeyCode,
        reservation: Option<crate::core::event::KeyboardKeyReservation>,
        state: KeyState,
        repeat: bool,
        modifiers: Modifiers,
        out: &mut impl FusionOutput,
    ) {
        self.keyboard_state.modifiers = modifiers;

        match state {
            KeyState::Down => {
                self.keyboard_state.pressed_keys.insert(key_code);
                if physical_key != crate::core::event::PhysicalKeyCode::Unknown {
                    self.keyboard_state
                        .pressed_physical_keys
                        .insert(physical_key);
                }

                let shortcut = (!repeat && reservation.is_none())
                    .then(|| {
                        self.shortcut_state
                            .find_match(key_code, physical_key, modifiers)
                    })
                    .flatten();
                out.emit_key(
                    FusedInputEvent::KeyDown {
                        window_id: self.current_window_id,
                        time,
                        key_code,
                        physical_key,
                        reservation,
                        modifiers,
                        repeat,
                    },
                    shortcut.and_then(|(id, scope)| (scope == ShortcutScope::Global).then_some(id)),
                );

                if let Some((id, scope)) = shortcut {
                    out.emit(
                        FusedInputEvent::Shortcut {
                            window_id: self.current_window_id,
                            time,
                            id,
                            scope,
                        },
                        None,
                    );
                }
            }
            KeyState::Up => {
                self.keyboard_state.pressed_keys.remove(&key_code);
                self.keyboard_state
                    .pressed_physical_keys
                    .remove(&physical_key);

                out.emit(
                    FusedInputEvent::KeyUp {
                        window_id: self.current_window_id,
                        time,
                        key_code,
                        physical_key,
                        reservation,
                        modifiers,
                    },
                    None,
                );
            }
        }
    }

    fn cancel_active_drag(&mut self, time: Instant, out: &mut impl FusionOutput) {
        if let Some(drag) = self.pointer_state.active_drag.take() {
            out.emit(
                FusedInputEvent::DragEnd {
                    window_id: self.current_window_id,
                    time,
                    origin: drag.origin,
                    current: drag.last_pos,
                    button: drag.button,
                    cancelled: true,
                },
                self.physical_presses[PointerFusionState::button_index(drag.button)].clone(),
            );
        }
    }

    fn cancel_pointer_interaction(&mut self, time: Instant, out: &mut impl FusionOutput) {
        self.cancel_active_drag(time, out);
        self.pointer_state.buttons = PointerButtonsState::default();
        self.pointer_state.drag_state = Default::default();
    }

    /// Cancel any active drag operation (e.g., on ESC or capture loss)
    pub fn cancel_drag(&mut self, out: &mut Vec<FusedInputEvent>) {
        self.cancel_active_drag(Instant::now(), out);
    }

    /// Get current pointer position
    pub fn pointer_position(&self) -> Point {
        self.pointer_state.current_pos
    }

    /// Get current button state
    pub fn button_state(&self) -> PointerButtonsState {
        self.pointer_state.buttons
    }

    /// Check if a drag is currently active
    pub fn is_dragging(&self) -> bool {
        self.pointer_state.active_drag.is_some()
    }

    /// Get current modifier state
    pub fn modifiers(&self) -> Modifiers {
        self.keyboard_state.modifiers
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(window_id: WindowId, contact_id: u64, phase: TouchPhase, pos: Point) -> RawInputEvent {
        RawInputEvent::Touch {
            window_id,
            time: Instant::now(),
            contact_id,
            phase,
            pos,
        }
    }

    fn touch_pointer(event: &FusedInputEvent) -> PointerId {
        match event {
            FusedInputEvent::TouchDown { pointer_id, .. }
            | FusedInputEvent::TouchMove { pointer_id, .. }
            | FusedInputEvent::TouchUp { pointer_id, .. }
            | FusedInputEvent::TouchCancel { pointer_id, .. } => *pointer_id,
            _ => panic!("expected a contact event"),
        }
    }

    #[test]
    fn touch_contact_native_zero_and_reused_id_have_nonmouse_lifetimes() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        for (phase, pos) in [
            (TouchPhase::Started, Point::new(10.0, 20.0)),
            (TouchPhase::Moved, Point::new(14.0, 23.0)),
            (TouchPhase::Ended, Point::new(14.0, 23.0)),
            (TouchPhase::Started, Point::new(30.0, 40.0)),
        ] {
            fusion.process_raw(touch(WindowId::PRIMARY, 0, phase, pos), &mut out);
        }
        assert_eq!(out.len(), 4);
        let first = touch_pointer(&out[0]);
        assert_ne!(first, PointerId::MOUSE);
        assert_eq!(touch_pointer(&out[1]), first);
        assert_eq!(touch_pointer(&out[2]), first);
        assert_ne!(touch_pointer(&out[3]), first);
        assert!(matches!(
            &out[0],
            FusedInputEvent::TouchDown {
                button: PointerButton::Primary,
                click_count: 1,
                ..
            }
        ));
        assert!(matches!(
            &out[1],
            FusedInputEvent::TouchMove { delta, buttons, .. }
                if *delta == (4.0, 3.0) && buttons.is_pressed(PointerButton::Primary)
        ));
        assert!(matches!(&out[2], FusedInputEvent::TouchUp { .. }));
        assert!(!fusion.button_state().any_pressed());
        assert!(!fusion.is_dragging());
    }

    #[test]
    fn touch_contact_two_contacts_and_windows_remain_independent() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        let other_window = WindowId(2);
        for (window, native_id) in [
            (WindowId::PRIMARY, 0),
            (WindowId::PRIMARY, 1),
            (other_window, 0),
        ] {
            fusion.process_raw(
                touch(window, native_id, TouchPhase::Started, Point::zero()),
                &mut out,
            );
        }
        let ids: Vec<_> = out.iter().map(touch_pointer).collect();
        assert_eq!(ids.iter().copied().collect::<HashSet<_>>().len(), 3);
        out.clear();
        fusion.process_raw(
            touch(WindowId::PRIMARY, 0, TouchPhase::Cancelled, Point::zero()),
            &mut out,
        );
        fusion.process_raw(
            touch(WindowId::PRIMARY, 0, TouchPhase::Ended, Point::zero()),
            &mut out,
        );
        fusion.process_raw(
            touch(
                WindowId::PRIMARY,
                1,
                TouchPhase::Moved,
                Point::new(2.0, 3.0),
            ),
            &mut out,
        );
        fusion.process_raw(
            touch(other_window, 0, TouchPhase::Moved, Point::new(4.0, 5.0)),
            &mut out,
        );
        assert_eq!(out.len(), 3);
        assert!(matches!(&out[0], FusedInputEvent::TouchCancel { .. }));
        assert_eq!(touch_pointer(&out[0]), ids[0]);
        assert_eq!(touch_pointer(&out[1]), ids[1]);
        assert_eq!(touch_pointer(&out[2]), ids[2]);
        assert_eq!(out[2].window_id(), other_window);
        assert_eq!(fusion.touch_contacts.len(), 2);
    }

    #[test]
    fn touch_contact_focus_cancels_window_before_focus_event_without_id_reuse() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        let other_window = WindowId(2);
        for (window, native_id) in [
            (WindowId::PRIMARY, 5),
            (WindowId::PRIMARY, 4),
            (other_window, 5),
        ] {
            fusion.process_raw(
                touch(window, native_id, TouchPhase::Started, Point::zero()),
                &mut out,
            );
        }
        let ids: Vec<_> = out.iter().map(touch_pointer).collect();
        out.clear();
        fusion.process_raw(
            RawInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                focused: false,
            },
            &mut out,
        );
        assert_eq!(out.len(), 3);
        assert!(matches!(&out[0], FusedInputEvent::TouchCancel { .. }));
        assert!(matches!(&out[1], FusedInputEvent::TouchCancel { .. }));
        assert_eq!(touch_pointer(&out[0]), ids[0]);
        assert_eq!(touch_pointer(&out[1]), ids[1]);
        assert!(matches!(
            &out[2],
            FusedInputEvent::FocusChanged { focused: false, .. }
        ));
        assert_eq!(fusion.touch_contacts.len(), 1);
        out.clear();
        fusion.process_raw(
            touch(other_window, 5, TouchPhase::Moved, Point::new(2.0, 1.0)),
            &mut out,
        );
        fusion.process_raw(
            touch(WindowId::PRIMARY, 5, TouchPhase::Started, Point::zero()),
            &mut out,
        );
        assert_eq!(touch_pointer(&out[0]), ids[2]);
        assert!(touch_pointer(&out[1]).0 > ids[2].0);
    }

    #[test]
    fn touch_contact_flushes_desktop_work_at_ordered_boundaries() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Mouse,
                position: Point::new(10.0, 20.0),
                delta: (10.0, 20.0),
            },
            &mut out,
        );
        assert!(out.is_empty());
        fusion.process_raw(
            touch(WindowId::PRIMARY, 1, TouchPhase::Started, Point::zero()),
            &mut out,
        );
        assert_eq!(out.len(), 2);
        assert!(matches!(&out[0], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(&out[1], FusedInputEvent::TouchDown { .. }));
        fusion.process_raw(
            RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                device: PointerDevice::Touchpad,
                delta: WheelDelta::Pixels(0.0, 3.0),
                position: Point::zero(),
            },
            &mut out,
        );
        fusion.process_raw(
            touch(
                WindowId::PRIMARY,
                1,
                TouchPhase::Moved,
                Point::new(2.0, 3.0),
            ),
            &mut out,
        );
        assert_eq!(out.len(), 4);
        assert!(matches!(&out[2], FusedInputEvent::Scroll { .. }));
        assert!(matches!(&out[3], FusedInputEvent::TouchMove { .. }));
    }

    #[test]
    fn touch_contact_qualified_events_keep_original_press_and_touch_type() {
        let mut fusion = InputFusion::new();
        let mut registry = GestureRegistry::new();
        let tree = crate::core::widget_tree::WidgetTree::new();
        let mut out = Vec::new();
        for phase in [
            TouchPhase::Started,
            TouchPhase::Moved,
            TouchPhase::Cancelled,
        ] {
            fusion.process_raw_with_gesture_sessions(
                touch(WindowId::PRIMARY, 0, phase, Point::new(10.0, 20.0)),
                InteractionDomainId::PRIMARY,
                &mut registry,
                &tree,
                None,
                &mut out,
            );
        }
        assert_eq!(out.len(), 3);
        let press = out[0].press.as_ref().unwrap();
        assert_ne!(press.pointer, PointerId::MOUSE);
        assert_eq!(press.pointer_type, PointerType::Touch);
        assert_eq!(press.window, WindowId::PRIMARY);
        assert_eq!(press.domain, InteractionDomainId::PRIMARY);
        assert!(press.physical_release.get());
        for event in &out {
            assert_eq!(touch_pointer(event.event()), press.pointer);
            assert!(Rc::ptr_eq(event.press.as_ref().unwrap(), press));
        }
        assert!(matches!(
            out[2].event(),
            FusedInputEvent::TouchCancel { .. }
        ));
        assert!(registry.install_physical_press(press.clone()));
        assert_eq!(
            registry.session_pointer_type(press.pointer),
            Some(PointerType::Touch)
        );
        assert!(fusion.touch_contacts.is_empty());
    }

    #[test]
    fn touch_contact_consumed_down_closes_qualified_eligibility() {
        let mut fusion = InputFusion::new();
        let mut registry = GestureRegistry::new();
        let tree = crate::core::widget_tree::WidgetTree::new();
        let mut out = Vec::new();
        fusion.process_raw_with_gesture_sessions(
            touch(WindowId::PRIMARY, 0, TouchPhase::Started, Point::zero()),
            InteractionDomainId::PRIMARY,
            &mut registry,
            &tree,
            None,
            &mut out,
        );
        let press = out[0].press.as_ref().unwrap();
        assert!(registry.can_install_press(press));
        out[0].finish_delivery(true);
        assert!(!registry.can_install_press(press));
        assert!(out[0].is_replayed());
    }

    #[test]
    fn touch_scroll_motion_facts_survive_batched_ingress() {
        use TouchPhase::{Ended, Moved, Started};

        let mut fusion = InputFusion::new();
        let mut registry = GestureRegistry::new();
        let tree = crate::core::widget_tree::WidgetTree::new();
        let window = WindowId(9);
        let domain = InteractionDomainId(7);
        let mut output = Vec::new();
        for (contact, phase, pos) in [
            (0, Started, Point::new(20.0, 52.0)),
            (0, Moved, Point::new(20.0, 48.0)),
            (1, Started, Point::new(42.0, 60.0)),
            (0, Ended, Point::new(20.0, 39.0)),
            (1, Moved, Point::new(41.0, 49.0)),
            (1, Ended, Point::new(41.0, 49.0)),
            (0, Started, Point::new(81.0, 16.0)),
        ] {
            fusion.process_raw_with_gesture_sessions(
                touch(window, contact, phase, pos),
                domain,
                &mut registry,
                &tree,
                None,
                &mut output,
            );
        }
        let motions: Vec<_> = output
            .iter()
            .map(|input| input.touch_motion().unwrap())
            .collect();
        let first_move = motions[1];
        assert_eq!(first_move.origin, Point::new(20.0, 52.0));
        assert_eq!(first_move.previous, first_move.origin);
        assert_eq!(first_move.position, Point::new(20.0, 48.0));
        assert!(first_move.first_contact);
        assert_eq!(first_move.native_contact, 0);
        assert_eq!(first_move.press, motions[0].press);
        assert_eq!(
            (first_move.press.window, first_move.press.domain),
            (window, domain)
        );
        assert_eq!(motions[3].previous, first_move.position);
        assert_eq!(motions[3].position, Point::new(20.0, 39.0));
        assert_eq!(motions[3].press, first_move.press);
        assert!(
            !motions[4].first_contact,
            "surviving contact does not become first"
        );
        assert_eq!(motions[4].press, motions[2].press);
        assert!(motions[6].first_contact);
        assert_ne!(motions[6].press, first_move.press);
    }

    #[test]
    fn test_single_click() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );

        assert_eq!(out.len(), 1);
        if let FusedInputEvent::PointerDown { click_count, .. } = &out[0] {
            assert_eq!(*click_count, 1);
        } else {
            panic!("Expected PointerDown event");
        }
    }

    #[test]
    fn test_double_click() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // First click
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Released,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );

        out.clear();

        // Second click (same position, immediately after)
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(101.0, 101.0),
            },
            &mut out,
        );

        assert!(!out.is_empty());
        if let FusedInputEvent::PointerDown { click_count, .. } = &out[0] {
            assert_eq!(*click_count, 2);
        } else {
            panic!("Expected PointerDown event");
        }
    }

    #[test]
    fn test_drag_detection() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Press
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );
        out.clear();

        // Move beyond threshold
        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: Point::new(110.0, 110.0),
                delta: (10.0, 10.0),
            },
            &mut out,
        );

        // Should have DragStart, DragUpdate, and PointerMove
        assert!(
            out.iter()
                .any(|e| matches!(e, FusedInputEvent::DragStart { .. }))
        );
        assert!(
            out.iter()
                .any(|e| matches!(e, FusedInputEvent::DragUpdate { .. }))
        );
    }

    #[test]
    fn test_focus_loss_clears_pre_threshold_pointer_press() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();
        let time = Instant::now();

        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );
        assert!(fusion.button_state().is_pressed(PointerButton::Primary));
        assert!(!fusion.is_dragging());
        out.clear();

        fusion.process_raw(
            RawInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time,
                focused: false,
            },
            &mut out,
        );
        assert!(!fusion.button_state().any_pressed());
        assert!(!fusion.is_dragging());

        fusion.process_raw(
            RawInputEvent::FocusChanged {
                window_id: WindowId::PRIMARY,
                time,
                focused: true,
            },
            &mut out,
        );
        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: Point::new(110.0, 110.0),
                delta: (10.0, 10.0),
            },
            &mut out,
        );
        fusion.flush(&mut out);

        assert!(
            !out.iter()
                .any(|event| matches!(event, FusedInputEvent::DragStart { .. })),
            "focus restoration and pointer movement must not start a drag from the cancelled press"
        );
        assert!(!fusion.is_dragging());
        assert!(!fusion.button_state().any_pressed());
        assert!(out.iter().any(|event| matches!(
            event,
            FusedInputEvent::PointerMove { buttons, .. } if !buttons.any_pressed()
        )));
    }

    #[test]
    fn test_scroll_normalization() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Line-based scroll
        fusion.process_raw(
            RawInputEvent::PointerWheel {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                delta: WheelDelta::Lines(0.0, 1.0),
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );

        // Scroll is coalesced - flush to emit
        fusion.flush(&mut out);
        assert_eq!(out.len(), 1);
        if let FusedInputEvent::Scroll { delta, .. } = &out[0] {
            // SPEC-HAL-036 Phase 4: Mouse wheel uses 56.0 px/line for responsive scroll
            assert_eq!(delta.1, 56.0); // 1 line * 56 pixels/line (mouse wheel)
        } else {
            panic!("Expected Scroll event");
        }
    }

    // ========================================================================
    // SPEC-HAL-036: Input Coalescing Tests (Phase 2)
    // Tests for the coalescing behavior with frame-synchronized flush pattern.
    // ========================================================================

    #[test]
    fn test_pointer_move_coalescing() {
        // SPEC-HAL-036: Feed 10 PointerMoved, expect 1 FusedInputEvent::PointerMove
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Feed 10 pointer move events
        for i in 0..10 {
            fusion.process_raw(
                RawInputEvent::PointerMoved {
                    window_id: WindowId::PRIMARY,
                    time,
                    device: PointerDevice::Mouse,
                    position: Point::new(100.0 + i as f32, 100.0 + i as f32),
                    delta: (1.0, 1.0),
                },
                &mut out,
            );
        }

        // No events emitted yet (all coalesced)
        assert_eq!(
            out.len(),
            0,
            "Events should be coalesced, not emitted immediately"
        );

        // Flush to emit coalesced event
        fusion.flush(&mut out);

        // Expect exactly 1 PointerMove
        assert_eq!(out.len(), 1, "Expected exactly 1 coalesced PointerMove");
        if let FusedInputEvent::PointerMove { pos, delta, .. } = &out[0] {
            // Position should be last (109, 109)
            assert_eq!(pos.x, 109.0);
            assert_eq!(pos.y, 109.0);
            // Delta should be sum (10, 10)
            assert_eq!(delta.0, 10.0);
            assert_eq!(delta.1, 10.0);
        } else {
            panic!("Expected PointerMove event");
        }
    }

    #[test]
    fn test_scroll_coalescing() {
        // SPEC-HAL-036: Feed 5 scroll events, expect 1 Scroll with summed delta
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Feed 5 scroll events
        for _ in 0..5 {
            fusion.process_raw(
                RawInputEvent::PointerWheel {
                    window_id: WindowId::PRIMARY,
                    time,
                    device: PointerDevice::Mouse,
                    delta: WheelDelta::Pixels(0.0, 10.0),
                    position: Point::new(100.0, 100.0),
                },
                &mut out,
            );
        }

        // No events emitted yet (all coalesced)
        assert_eq!(out.len(), 0, "Scroll events should be coalesced");

        // Flush to emit coalesced event
        fusion.flush(&mut out);

        // Expect exactly 1 Scroll
        assert_eq!(out.len(), 1, "Expected exactly 1 coalesced Scroll");
        if let FusedInputEvent::Scroll { delta, .. } = &out[0] {
            // Delta should be sum: 5 * 10 = 50
            assert_eq!(delta.1, 50.0);
        } else {
            panic!("Expected Scroll event");
        }
    }

    #[test]
    fn test_flush_on_interaction() {
        // SPEC-HAL-036: Moves → Down should flush coalesced move before Down
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Feed some pointer moves
        for i in 0..3 {
            fusion.process_raw(
                RawInputEvent::PointerMoved {
                    window_id: WindowId::PRIMARY,
                    time,
                    device: PointerDevice::Mouse,
                    position: Point::new(100.0 + i as f32, 100.0),
                    delta: (1.0, 0.0),
                },
                &mut out,
            );
        }

        // No events yet (coalesced)
        assert_eq!(out.len(), 0);

        // Now button down - should flush pending moves first
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(102.0, 100.0),
            },
            &mut out,
        );

        // Should have: 1 coalesced PointerMove + 1 PointerDown
        assert_eq!(out.len(), 2, "Expected PointerMove then PointerDown");
        assert!(matches!(out[0], FusedInputEvent::PointerMove { .. }));
        assert!(matches!(out[1], FusedInputEvent::PointerDown { .. }));

        // Verify coalesced move has correct delta
        if let FusedInputEvent::PointerMove { delta, .. } = &out[0] {
            assert_eq!(delta.0, 3.0); // Sum of 3 moves with delta 1.0 each
        }
    }

    #[test]
    fn test_drag_boundary_coalescing() {
        // SPEC-HAL-036: Moves before DragStart must coalesce
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Press button
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );
        out.clear();

        // Small moves that don't trigger drag
        for i in 0..2 {
            fusion.process_raw(
                RawInputEvent::PointerMoved {
                    window_id: WindowId::PRIMARY,
                    time,
                    device: PointerDevice::Mouse,
                    position: Point::new(101.0 + i as f32, 100.0),
                    delta: (1.0, 0.0),
                },
                &mut out,
            );
        }

        // No events yet (moves coalesced, drag threshold not reached)
        assert_eq!(out.len(), 0, "Small moves should be coalesced");

        // Move beyond drag threshold (>4px from press)
        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: Point::new(110.0, 100.0),
                delta: (8.0, 0.0),
            },
            &mut out,
        );

        // Should have: coalesced PointerMove (from before threshold), DragStart, DragUpdate
        let has_pointer_move = out
            .iter()
            .any(|e| matches!(e, FusedInputEvent::PointerMove { .. }));
        let has_drag_start = out
            .iter()
            .any(|e| matches!(e, FusedInputEvent::DragStart { .. }));
        let has_drag_update = out
            .iter()
            .any(|e| matches!(e, FusedInputEvent::DragUpdate { .. }));

        assert!(
            has_pointer_move,
            "Should flush coalesced moves before drag start"
        );
        assert!(has_drag_start, "Should emit DragStart");
        assert!(has_drag_update, "Should emit DragUpdate");
    }

    #[test]
    fn test_shortcut_matching() {
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        // Register Ctrl+S shortcut
        let shortcut_id = fusion.register_shortcut(
            ShortcutScope::Global,
            KeyChord::new(
                KeyCode::Character('s'),
                Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            ),
        );

        let time = Instant::now();

        // Press Ctrl+S
        fusion.process_raw(
            RawInputEvent::Key {
                window_id: WindowId::PRIMARY,
                time,
                key_code: KeyCode::Character('s'),
                physical_key: crate::core::event::PhysicalKeyCode::Unknown,
                reservation: None,
                state: KeyState::Down,
                repeat: false,
                modifiers: Modifiers {
                    ctrl: true,
                    ..Default::default()
                },
            },
            &mut out,
        );

        // Should have KeyDown and Shortcut events
        assert!(
            out.iter()
                .any(|e| matches!(e, FusedInputEvent::KeyDown { .. }))
        );
        assert!(out.iter().any(|e| {
            if let FusedInputEvent::Shortcut { id, .. } = e {
                *id == shortcut_id
            } else {
                false
            }
        }));
    }

    // ========================================================================
    // SPEC-HAL-036 Phase 3: Drag Update Coalescing Tests
    // ========================================================================

    #[test]
    fn test_drag_update_coalescing_basic() {
        // SPEC-HAL-036 Phase 3: Feed drag updates, expect coalescing
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Press button at (100, 100)
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );
        out.clear();

        // Move beyond threshold to start drag
        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: Point::new(110.0, 100.0),
                delta: (10.0, 0.0),
            },
            &mut out,
        );

        // Should have DragStart + first DragUpdate (immediate, not coalesced)
        assert!(
            out.iter()
                .any(|e| matches!(e, FusedInputEvent::DragStart { .. })),
            "Expected DragStart"
        );
        assert!(
            out.iter()
                .any(|e| matches!(e, FusedInputEvent::DragUpdate { .. })),
            "Expected initial DragUpdate"
        );
        out.clear();

        // Send 10 small drag moves while drag is active
        for i in 0..10 {
            fusion.process_raw(
                RawInputEvent::PointerMoved {
                    window_id: WindowId::PRIMARY,
                    time,
                    device: PointerDevice::Mouse,
                    position: Point::new(111.0 + i as f32, 100.0),
                    delta: (1.0, 0.0),
                },
                &mut out,
            );
        }

        // No DragUpdate events yet (all coalesced)
        assert_eq!(
            out.len(),
            0,
            "DragUpdate events should be coalesced, not emitted immediately"
        );

        // Flush to emit coalesced event
        fusion.flush(&mut out);

        // Expect exactly 1 coalesced DragUpdate
        assert_eq!(out.len(), 1, "Expected exactly 1 coalesced DragUpdate");
        if let FusedInputEvent::DragUpdate { current, delta, .. } = &out[0] {
            // Position should be last (120, 100)
            assert_eq!(current.x, 120.0);
            assert_eq!(current.y, 100.0);
            // Delta should be sum (10, 0)
            assert_eq!(delta.0, 10.0);
            assert_eq!(delta.1, 0.0);
        } else {
            panic!("Expected DragUpdate event");
        }
    }

    #[test]
    fn test_drag_flush_on_button_up() {
        // SPEC-HAL-036 Phase 3: Button release must flush pending drag then emit DragEnd
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Start drag
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );

        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: Point::new(110.0, 100.0),
                delta: (10.0, 0.0),
            },
            &mut out,
        );
        out.clear();

        // Send some drag moves (coalesced)
        for i in 0..5 {
            fusion.process_raw(
                RawInputEvent::PointerMoved {
                    window_id: WindowId::PRIMARY,
                    time,
                    device: PointerDevice::Mouse,
                    position: Point::new(111.0 + i as f32, 100.0),
                    delta: (1.0, 0.0),
                },
                &mut out,
            );
        }

        // No events yet
        assert_eq!(out.len(), 0, "Drag updates should be coalesced");

        // Release button
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Released,
                position: Point::new(115.0, 100.0),
            },
            &mut out,
        );

        // Should have: coalesced DragUpdate, DragEnd, PointerUp
        let drag_update_count = out
            .iter()
            .filter(|e| matches!(e, FusedInputEvent::DragUpdate { .. }))
            .count();
        let has_drag_end = out
            .iter()
            .any(|e| matches!(e, FusedInputEvent::DragEnd { .. }));
        let has_pointer_up = out
            .iter()
            .any(|e| matches!(e, FusedInputEvent::PointerUp { .. }));

        assert_eq!(
            drag_update_count, 1,
            "Should have exactly 1 coalesced DragUpdate before DragEnd"
        );
        assert!(has_drag_end, "Should have DragEnd");
        assert!(has_pointer_up, "Should have PointerUp");

        // Verify ordering: DragUpdate should come before DragEnd
        let drag_update_idx = out
            .iter()
            .position(|e| matches!(e, FusedInputEvent::DragUpdate { .. }))
            .unwrap();
        let drag_end_idx = out
            .iter()
            .position(|e| matches!(e, FusedInputEvent::DragEnd { .. }))
            .unwrap();
        assert!(
            drag_update_idx < drag_end_idx,
            "DragUpdate must come before DragEnd"
        );

        // No pending drag state after
        assert!(
            fusion.pending_drag_update.is_none(),
            "No pending drag should remain"
        );
    }

    #[test]
    fn test_drag_start_order_preserved() {
        // SPEC-HAL-036 Phase 3: Regression - DragStart before DragUpdate order must be preserved
        let mut fusion = InputFusion::new();
        let mut out = Vec::new();

        let time = Instant::now();

        // Press button
        fusion.process_raw(
            RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: RawButtonState::Pressed,
                position: Point::new(100.0, 100.0),
            },
            &mut out,
        );
        out.clear();

        // Move beyond threshold
        fusion.process_raw(
            RawInputEvent::PointerMoved {
                window_id: WindowId::PRIMARY,
                time,
                device: PointerDevice::Mouse,
                position: Point::new(110.0, 110.0),
                delta: (10.0, 10.0),
            },
            &mut out,
        );

        // Find positions
        let drag_start_idx = out
            .iter()
            .position(|e| matches!(e, FusedInputEvent::DragStart { .. }));
        let drag_update_idx = out
            .iter()
            .position(|e| matches!(e, FusedInputEvent::DragUpdate { .. }));

        assert!(drag_start_idx.is_some(), "Must have DragStart");
        assert!(drag_update_idx.is_some(), "Must have DragUpdate");
        assert!(
            drag_start_idx.unwrap() < drag_update_idx.unwrap(),
            "DragStart must come before DragUpdate"
        );
    }
}
