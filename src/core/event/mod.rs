mod forbidden;
mod guard;

use crate::core::drag_drop::DragDropState;
use crate::core::focus_authority::{FocusDomainId, FocusDomainRecord, FocusRestorationFrame};
use crate::core::math::{Point, RectF};
use crate::core::relative_pointer::{
    PointerMotionPolicy, RelativeLogicalDelta, RelativePointerLeaseId,
    RelativePointerNormalizationEpoch,
};
use crate::core::widget_tree::{NodeId, WidgetMountToken, WidgetTree};
use crate::utils::id::TreeWidgetId;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::{Rc, Weak};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PhysicalPressState {
    Reserved,
    Routed,
    Completed,
}

#[derive(Debug)]
pub(crate) struct PhysicalGesturePress {
    pub(crate) registry_lifetime: Rc<()>,
    pub(crate) cutoff: crate::core::widget_tree::GeometryCausalCutoff,
    pub(crate) domain: InteractionDomainId,
    pub(crate) window: WindowId,
    pub(crate) pointer: PointerId,
    pub(crate) pointer_type: PointerType,
    pub(crate) button: PointerButton,
    pub(crate) position: Point,
    pub(crate) session: GestureSessionId,
    pub(crate) state: Cell<PhysicalPressState>,
    pub(crate) eligible: Cell<bool>,
    pub(crate) protected: Cell<bool>,
    pub(crate) physical_release: Cell<bool>,
    next_emission: Cell<u64>,
    delivered: Cell<u64>,
}

/// Identity of one native touch press admitted to the interaction domain (HAL-396).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TouchPressIdentity {
    pub pointer: PointerId,
    pub session: GestureSessionId,
    pub window: WindowId,
    pub domain: InteractionDomainId,
}

impl PhysicalGesturePress {
    pub(crate) fn touch_identity(&self) -> TouchPressIdentity {
        debug_assert_eq!(self.pointer_type, PointerType::Touch);
        TouchPressIdentity {
            pointer: self.pointer,
            session: self.session,
            window: self.window,
            domain: self.domain,
        }
    }
}

/// Window-logical positions for one ordered native touch sample (HAL-396 §2).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QualifiedTouchMotion {
    pub press: TouchPressIdentity,
    pub native_contact: u64,
    pub origin: Point,
    pub previous: Point,
    pub position: Point,
    pub first_contact: bool,
    pub origin_time: Instant,
    pub previous_time: Instant,
    pub time: Instant,
}

/// Motion delivered to the winning Scroll candidate. The first sample's delta
/// is measured from Down. Subsequent samples, including release, are incremental.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TouchScrollMotion {
    pub press: TouchPressIdentity,
    pub axis: GestureAxis,
    pub delta: Point,
    pub is_release: bool,
    /// The same position and time observations as the routed physical sample.
    pub sample: QualifiedTouchMotion,
}

/// The semantic part of a control hit by a tentative touch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchTarget {
    Control,
    Editor,
    Track,
    Increment,
    Decrement,
    Item(u64),
    DismissPopup,
}

/// Current owner-local hit facts supplied by the engine.
#[derive(Debug, Clone, Copy)]
pub struct TouchHit {
    pub local_position: Point,
    pub size: crate::core::math::Size,
    pub(crate) position: Point,
    pub(crate) origin: PointerRouteOrigin,
    pub(crate) popup: Option<crate::core::widget_tree::ComboPopupInputGeometry>,
}

/// One admitted Tap. No pointer-down or drag-selection authority is implied.
#[derive(Debug, Clone, Copy)]
pub struct TouchTap {
    pub press: TouchPressIdentity,
    pub target: TouchTarget,
    pub local_position: Point,
}

/// Absolute position for a contact after its declared Drag candidate wins.
#[derive(Debug, Clone, Copy)]
pub struct TouchDragMotion {
    pub press: TouchPressIdentity,
    pub local_position: Point,
    pub is_release: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PendingTouchTap {
    pub mount: WidgetMountToken,
    pub target: TouchTarget,
    pub origin: PointerRouteOrigin,
    pub rect: crate::core::math::RectF,
    pub viewport: Option<crate::core::math::RectF>,
    pub scroll: (f32, f32),
    pub popup_revision: Option<u64>,
    pub popup_rows: Option<crate::core::math::RectF>,
}

/// Immutable normalized input with registry-minted physical press provenance.
/// Cloning preserves the payload and seal; it never reserves another press.
#[derive(Debug, Clone)]
pub struct SessionQualifiedInput {
    event: Rc<FusedInputEvent>,
    pub(crate) press: Option<Rc<PhysicalGesturePress>>,
    ordinal: u64,
    registered_global_shortcut: Option<ShortcutId>,
    touch_motion: Option<QualifiedTouchMotion>,
}

impl SessionQualifiedInput {
    pub fn event(&self) -> &FusedInputEvent {
        &self.event
    }

    /// Motion facts from this envelope's original ingress sample.
    pub fn touch_motion(&self) -> Option<QualifiedTouchMotion> {
        self.touch_motion
    }

    pub(crate) fn emitted(event: FusedInputEvent, press: Option<Rc<PhysicalGesturePress>>) -> Self {
        let ordinal = press.as_ref().map_or(0, |press| {
            let next = press
                .next_emission
                .get()
                .checked_add(1)
                .expect("INV-ABSOLUTE-PRESS-EMISSION-NONREUSE");
            press.next_emission.set(next);
            next
        });
        Self {
            event: Rc::new(event),
            press,
            ordinal,
            registered_global_shortcut: None,
            touch_motion: None,
        }
    }

    pub(crate) fn emitted_touch(
        event: FusedInputEvent,
        press: Option<Rc<PhysicalGesturePress>>,
        motion: Option<QualifiedTouchMotion>,
    ) -> Self {
        let mut input = Self::emitted(event, press);
        input.touch_motion = motion;
        input
    }

    pub(crate) fn emitted_key(
        event: FusedInputEvent,
        registered_global_shortcut: Option<ShortcutId>,
    ) -> Self {
        let mut input = Self::emitted(event, None);
        input.registered_global_shortcut = registered_global_shortcut;
        input
    }

    pub(crate) fn registered_global_shortcut(&self) -> Option<ShortcutId> {
        self.registered_global_shortcut
    }

    #[cfg(test)]
    pub(crate) fn unqualified(event: FusedInputEvent) -> Self {
        Self::emitted(event, None)
    }

    pub(crate) fn is_replayed(&self) -> bool {
        self.press
            .as_ref()
            .is_some_and(|press| self.ordinal <= press.delivered.get())
    }

    pub(crate) fn finish_delivery(&self, consumed: bool) {
        if let Some(press) = &self.press {
            if self.ordinal <= press.delivered.get() {
                return;
            }
            press.delivered.set(press.delivered.get().max(self.ordinal));
            if consumed
                && matches!(
                    self.event(),
                    FusedInputEvent::PointerDown { .. } | FusedInputEvent::TouchDown { .. }
                )
            {
                press.eligible.set(false);
            }
        }
    }
}

/// SPEC-HAL-326 §12.x INV-F9: Re-exported for widget-facing directional
/// focus entry. Widgets read this via `EventContext::focus_entry_direction()`.
pub use crate::core::event_router::Direction;

// ============================================================================
// SPEC-HAL-151: Window Identity & Interaction Domains
// ============================================================================

/// SPEC-HAL-151: Opaque window identifier.
///
/// Wraps the OS/platform window identity. In single-window mode, all events
/// use `WindowId::PRIMARY`. Multi-window support requires each window to have
/// a unique WindowId assigned at creation.
///
/// INV-151-WINDOW-ORIGIN: Every input event MUST carry its originating WindowId.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub u64);

impl WindowId {
    /// The primary (and currently only) window.
    /// Single-window mode uses this for all events.
    pub const PRIMARY: WindowId = WindowId(0);
}

/// SPEC-HAL-151: Interaction domain identifier.
///
/// An interaction domain is an isolated input routing context. Typically 1:1
/// with windows, but modal overlays may create nested domains.
///
/// INV-151-DOMAIN-ISOLATION: Capture/focus/gesture state MUST NOT leak between domains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct InteractionDomainId(pub u64);

impl InteractionDomainId {
    /// The primary interaction domain (maps to PRIMARY window).
    pub const PRIMARY: InteractionDomainId = InteractionDomainId(0);
}

// ============================================================================
// SPEC-HAL-176: Keyboard Authority Mode
// ============================================================================

/// SPEC-HAL-176: Keyboard authority mode.
///
/// Determines how keyboard events are routed:
/// - Navigation: keys resolve through binding registry for spatial/structural navigation
/// - Capture: focused widget owns all keyboard input (except release_capture)
///
/// INV-176-MODE-1: Exactly one mode active per interaction domain at all times.
/// INV-176-MODE-2: Mode transitions occur only via explicit action dispatch.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardAuthorityMode {
    /// Keyboard events route through binding registry.
    /// hjkl/arrows for spatial nav, Tab for structural nav, Enter to activate/enter.
    Navigation,
    /// Focused widget owns all keyboard input.
    /// Only release_capture (Escape) is handled by the engine.
    Capture,
}

impl Default for KeyboardAuthorityMode {
    fn default() -> Self {
        // System starts in Navigation mode.
        // INV-176-MODE-3: Default mode is Navigation.
        Self::Navigation
    }
}

/// Construction-time application policy, independent of keyboard authority mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum UiInteractionPolicy {
    /// Existing keyboard navigation and explicit Capture behavior.
    #[default]
    NavigationAndCapture,
    /// Pointer UI interaction; widget keyboard delivery requires actual Capture.
    PointerDriven,
}

// ============================================================================
// SPEC-HAL-009: Event Types
// ============================================================================

/// Modifier keys state
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

/// Pointer button types
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PointerButton {
    Primary,   // Left mouse button
    Secondary, // Right mouse button
    Auxiliary, // Middle mouse button
}

// ============================================================================
// SPEC-HAL-128 Stage 7: Pointer Type (INV-S7-P1)
// ============================================================================

/// SPEC-HAL-128 Stage 7: Explicit pointer type.
///
/// INV-S7-P1: Every PointerEvent MUST carry an explicit PointerType.
/// Defaulting to Mouse, inferring from device, or omitting is FORBIDDEN.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerType {
    /// Physical mouse device
    Mouse,
    /// Touch screen input
    Touch,
    /// Stylus/pen input
    Pen,
}

impl PointerType {
    /// SPEC-HAL-128 Stage 7 INV-PT2: Legality is Static.
    ///
    /// Gesture legality = (PointerType, GestureKind, optional GestureAxis).
    /// No widget state, focus, capture, or runtime context involved.
    ///
    /// Legality Matrix:
    /// - Mouse → { Drag, Scroll, Resize, Select, Custom }
    /// - Touch → { Scroll, Drag, Pinch, Swipe }
    /// - Pen   → { Draw, Drag, Select }
    ///
    /// INV-PT5: Mouse backward compatibility - Custom always allowed for Mouse.
    pub fn is_gesture_legal(self, gesture: GestureKind) -> bool {
        match self {
            PointerType::Mouse => matches!(
                gesture,
                GestureKind::Drag
                    | GestureKind::Scroll
                    | GestureKind::Resize
                    | GestureKind::Select
                    | GestureKind::Custom
            ),
            PointerType::Touch => matches!(
                gesture,
                GestureKind::Scroll | GestureKind::Drag | GestureKind::Pinch | GestureKind::Swipe
            ),
            PointerType::Pen => {
                matches!(
                    gesture,
                    GestureKind::Draw | GestureKind::Drag | GestureKind::Select
                )
            }
        }
    }

    /// SPEC-HAL-128 Stage 7 INV-S7-P3: Primary Gesture Per Pointer Type.
    ///
    /// Each PointerType has exactly one primary gesture:
    /// - Mouse → Drag
    /// - Touch → Scroll
    /// - Pen   → Draw
    ///
    /// Only one primary gesture may commit per session.
    /// Secondary gestures cannot override a primary once eligible.
    pub fn primary_gesture(self) -> GestureKind {
        match self {
            PointerType::Mouse => GestureKind::Drag,
            PointerType::Touch => GestureKind::Scroll,
            PointerType::Pen => GestureKind::Draw,
        }
    }

    /// SPEC-HAL-128 Stage 7: Check if a gesture is the primary for this pointer type.
    pub fn is_primary_gesture(self, gesture: GestureKind) -> bool {
        self.primary_gesture() == gesture
    }
}

/// Pointer event kinds
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PointerEventKind {
    Down,
    Up,
    Move,
    Scroll {
        delta_x: f32,
        delta_y: f32,
    },
    DoubleClick,
    TripleClick,
    /// SPEC-HAL-128: Drag started (pointer moved beyond threshold while button held).
    /// INV-LV-RESIZE-ACTIVATION: Resize activation MUST happen via DragStart, not PointerMove.
    DragStart,
}

/// Pointer event with both global and local positions
///
/// SPEC-HAL-079: Every PointerEvent MUST carry a PointerId.
/// SPEC-HAL-128 Stage 7 INV-S7-P1: Every PointerEvent MUST carry an explicit pointer_type.
#[derive(Debug, Clone)]
pub struct PointerEvent {
    /// SPEC-HAL-079: Stable pointer identity, persistent from down to up.
    /// For mouse input, always PointerId::MOUSE.
    pub id: PointerId,
    pub global_pos: (f32, f32), // root/window space
    pub local_pos: (f32, f32),  // widget-local space
    pub kind: PointerEventKind,
    pub button: Option<PointerButton>, // for Down/Up/DoubleClick; None for Move/Scroll
    pub modifiers: Modifiers,
    /// SPEC-HAL-128 Stage 7 INV-S7-P1: Explicit pointer type (REQUIRED)
    pub pointer_type: PointerType,
}

/// Key codes (simplified for now, can be extended)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// No logical key was supplied. Physical identity remains independent.
    Unknown,
    Character(char),
    Enter,
    Tab,
    Backspace,
    Delete,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    PageUp,
    PageDown,
    Escape,
    Space,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    // Modifier keys - needed so event_router can track modifier state changes
    Shift,
    Control,
    Alt,
    Meta,
}

/// Framework-owned keyboard positions, independent of layout and composed text.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum PhysicalKeyCode {
    #[default]
    Unknown,
    KeyA,
    KeyB,
    KeyC,
    KeyD,
    KeyE,
    KeyF,
    KeyG,
    KeyH,
    KeyI,
    KeyJ,
    KeyK,
    KeyL,
    KeyM,
    KeyN,
    KeyO,
    KeyP,
    KeyQ,
    KeyR,
    KeyS,
    KeyT,
    KeyU,
    KeyV,
    KeyW,
    KeyX,
    KeyY,
    KeyZ,
    Digit0,
    Digit1,
    Digit2,
    Digit3,
    Digit4,
    Digit5,
    Digit6,
    Digit7,
    Digit8,
    Digit9,
    Backquote,
    Backslash,
    BracketLeft,
    BracketRight,
    Comma,
    Equal,
    Minus,
    Period,
    Quote,
    Semicolon,
    Slash,
    IntlBackslash,
    IntlRo,
    IntlYen,
    Enter,
    Tab,
    Backspace,
    Delete,
    Space,
    Escape,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowLeft,
    ArrowRight,
    ArrowUp,
    ArrowDown,
    ShiftLeft,
    ShiftRight,
    ControlLeft,
    ControlRight,
    AltLeft,
    AltRight,
    SuperLeft,
    SuperRight,
    CapsLock,
    NumLock,
    ScrollLock,
    ContextMenu,
    PrintScreen,
    Pause,
    Numpad0,
    Numpad1,
    Numpad2,
    Numpad3,
    Numpad4,
    Numpad5,
    Numpad6,
    Numpad7,
    Numpad8,
    Numpad9,
    NumpadAdd,
    NumpadSubtract,
    NumpadMultiply,
    NumpadDivide,
    NumpadDecimal,
    NumpadEnter,
    NumpadEqual,
    NumpadComma,
    NumpadBackspace,
    NumpadClear,
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    F13,
    F14,
    F15,
    F16,
    F17,
    F18,
    F19,
    F20,
    F21,
    F22,
    F23,
    F24,
}

impl PhysicalKeyCode {
    pub fn is_modifier(self) -> bool {
        matches!(
            self,
            Self::ShiftLeft
                | Self::ShiftRight
                | Self::ControlLeft
                | Self::ControlRight
                | Self::AltLeft
                | Self::AltRight
                | Self::SuperLeft
                | Self::SuperRight
        )
    }
}

/// Canonical binding identity. Neither variant is an alias for the other.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BindingKey {
    Logical(KeyCode),
    Physical(PhysicalKeyCode),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UnknownPhysicalKey;

impl std::fmt::Display for UnknownPhysicalKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("physical key identity is unknown")
    }
}

impl std::error::Error for UnknownPhysicalKey {}

/// An existing reserved path; the original operation still owns execution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardKeyReservation {
    Clipboard,
    Fullscreen,
}

impl KeyboardKeyReservation {
    /// Recognition of the existing native Shift+F operation; this executes nothing.
    pub fn from_observed_key(
        key: KeyCode,
        modifiers: Modifiers,
        is_down: bool,
        is_repeat: bool,
    ) -> Option<Self> {
        (is_down
            && !is_repeat
            && matches!(key, KeyCode::Character('f' | 'F'))
            && modifiers.shift
            && !modifiers.ctrl
            && !modifiers.alt
            && !modifiers.meta)
            .then_some(Self::Fullscreen)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyboardCaptureInterruptionReason {
    Reserved(KeyboardKeyReservation),
    AppConsumed,
    Escape,
    Tab,
    FocusLost,
}

#[derive(Debug, Clone)]
pub struct KeyboardCaptureInterruption {
    pub reason: KeyboardCaptureInterruptionReason,
    pub key: Option<KeyEvent>,
}

impl From<KeyCode> for BindingKey {
    fn from(key: KeyCode) -> Self {
        Self::Logical(key)
    }
}

impl PartialEq<KeyCode> for BindingKey {
    fn eq(&self, key: &KeyCode) -> bool {
        matches!(self, Self::Logical(logical) if logical == key)
    }
}

impl BindingKey {
    pub fn matches(self, logical: KeyCode, physical: PhysicalKeyCode) -> bool {
        match self {
            Self::Logical(key) => key != KeyCode::Unknown && key == logical,
            Self::Physical(key) => key != PhysicalKeyCode::Unknown && key == physical,
        }
    }
}

/// Keyboard event
#[derive(Debug, Clone)]
pub struct KeyEvent {
    pub key_code: KeyCode,
    pub physical_key: PhysicalKeyCode,
    pub is_down: bool,
    pub is_repeat: bool,
    pub modifiers: Modifiers,
    pub text: Option<String>, // non-empty for text input chars
}

/// Unavailable physical response input, not an incorrect trainer answer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PhysicalBindingUnavailable {
    Unobserved,
    UnknownPhysicalKey,
    NotInitialDown,
    ModifierOnly,
    Reserved(KeyboardCaptureInterruptionReason),
}

/// Checks an actual observation without promising future OS delivery or layout coverage.
pub fn physical_binding_support(
    observation: Option<&KeyEvent>,
    reservation: Option<KeyboardKeyReservation>,
) -> Result<(), PhysicalBindingUnavailable> {
    let key = observation.ok_or(PhysicalBindingUnavailable::Unobserved)?;
    if !key.is_down || key.is_repeat {
        return Err(PhysicalBindingUnavailable::NotInitialDown);
    }
    let hard = reservation
        .filter(|reservation| *reservation != KeyboardKeyReservation::Clipboard)
        .map(KeyboardCaptureInterruptionReason::Reserved)
        .or_else(|| match key.key_code {
            KeyCode::Escape => Some(KeyboardCaptureInterruptionReason::Escape),
            KeyCode::Tab => Some(KeyboardCaptureInterruptionReason::Tab),
            KeyCode::F9 => Some(KeyboardCaptureInterruptionReason::AppConsumed),
            _ => KeyboardKeyReservation::from_observed_key(
                key.key_code,
                key.modifiers,
                key.is_down,
                key.is_repeat,
            )
            .map(KeyboardCaptureInterruptionReason::Reserved),
        });
    if let Some(reason) = hard {
        return Err(PhysicalBindingUnavailable::Reserved(reason));
    }
    if key.physical_key == PhysicalKeyCode::Unknown {
        return Err(PhysicalBindingUnavailable::UnknownPhysicalKey);
    }
    if key.physical_key.is_modifier()
        || matches!(
            key.key_code,
            KeyCode::Control | KeyCode::Alt | KeyCode::Shift | KeyCode::Meta
        )
    {
        return Err(PhysicalBindingUnavailable::ModifierOnly);
    }
    Ok(())
}

impl FusedInputEvent {
    pub fn key_event(&self) -> Option<KeyEvent> {
        match self {
            Self::KeyDown {
                key_code,
                physical_key,
                modifiers,
                repeat,
                ..
            } => Some(KeyEvent {
                key_code: *key_code,
                physical_key: *physical_key,
                is_down: true,
                is_repeat: *repeat,
                modifiers: *modifiers,
                text: None,
            }),
            Self::KeyUp {
                key_code,
                physical_key,
                modifiers,
                ..
            } => Some(KeyEvent {
                key_code: *key_code,
                physical_key: *physical_key,
                is_down: false,
                is_repeat: false,
                modifiers: *modifiers,
                text: None,
            }),
            _ => None,
        }
    }
}

/// Focus events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusEvent {
    /// Window gained focus
    FocusGained,
    /// Window lost focus
    FocusLost,
    /// SPEC-HAL-038: Widget gained keyboard focus
    WidgetFocusGained(TreeWidgetId),
    /// SPEC-HAL-038: Widget lost keyboard focus
    WidgetFocusLost(TreeWidgetId),
}

/// Window events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowEvent {
    Resized { width: u32, height: u32 },
    CloseRequested,
    ScaleFactorChanged { scale_factor: f32 },
}

// ============================================================================
// Cursor Icons (for hover affordances)
// ============================================================================

/// Cursor icon for visual hover feedback.
///
/// Widgets request cursor changes via `EventContext::request_cursor()`.
/// The system applies the cursor after event dispatch.
///
/// INV-CURSOR-EPHEMERAL: Cursor requests are ephemeral per-event.
/// If no widget requests a cursor, the default (arrow) is used.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorIcon {
    /// Default arrow cursor
    #[default]
    Default,
    /// Text selection cursor (I-beam)
    Text,
    /// Pointer/hand cursor (for clickable elements)
    Pointer,
    /// Column resize cursor (horizontal double arrow with vertical bar)
    ColResize,
    /// Row resize cursor (vertical double arrow with horizontal bar)
    RowResize,
    /// Horizontal resize (east-west)
    EwResize,
    /// Vertical resize (north-south)
    NsResize,
    /// Move/drag cursor (four-way arrow)
    Move,
    /// Not allowed / forbidden
    NotAllowed,
    /// Crosshair
    Crosshair,
    /// Wait / busy
    Wait,
    /// SPEC-HAL-268: Hide cursor entirely (proximity scroll, drawing tools, etc.)
    Hidden,
}

/// Convert Halcyon CursorIcon to winit CursorIcon.
///
/// Used by the app to apply cursor changes from widgets.
pub fn cursor_to_winit(cursor: CursorIcon) -> winit::window::CursorIcon {
    match cursor {
        CursorIcon::Default => winit::window::CursorIcon::Default,
        CursorIcon::Text => winit::window::CursorIcon::Text,
        CursorIcon::Pointer => winit::window::CursorIcon::Pointer,
        CursorIcon::ColResize => winit::window::CursorIcon::ColResize,
        CursorIcon::RowResize => winit::window::CursorIcon::RowResize,
        CursorIcon::EwResize => winit::window::CursorIcon::EwResize,
        CursorIcon::NsResize => winit::window::CursorIcon::NsResize,
        CursorIcon::Move => winit::window::CursorIcon::Move,
        CursorIcon::NotAllowed => winit::window::CursorIcon::NotAllowed,
        CursorIcon::Crosshair => winit::window::CursorIcon::Crosshair,
        CursorIcon::Wait => winit::window::CursorIcon::Wait,
        CursorIcon::Hidden => winit::window::CursorIcon::Default, // unreachable in practice
    }
}

/// All event types
#[derive(Debug, Clone)]
pub enum Event {
    Pointer(PointerEvent),
    RelativePointerMotion(crate::core::relative_pointer::RelativePointerMotionEvent),
    Key(KeyEvent),
    Focus(FocusEvent),
    Window(WindowEvent),
    /// SPEC-HAL-033 Phase 2: Direct text input event (no synthetic KeyEvent)
    Text(TextInputEvent),
    /// SPEC-HAL-055 Stage 1.1: IME composition event
    Ime(ImeEvent),
}

/// SPEC-HAL-033 Phase 2: Text input event (internal routing, not synthesized from KeyEvent)
#[derive(Debug, Clone)]
pub struct TextInputEvent {
    /// The text that was entered
    pub text: String,
    /// Keyboard modifiers at time of input
    pub modifiers: Modifiers,
}

/// SPEC-HAL-055 Stage 1.1: IME composition events
#[derive(Debug, Clone)]
pub enum ImeEvent {
    /// IME composition started or updated (pre-edit text)
    PreEdit {
        /// Current composition text (may be empty on start)
        text: String,
        /// Cursor position within composition (byte offset, or None)
        cursor: Option<usize>,
    },
    /// IME composition committed (final text to insert)
    Commit {
        /// The committed text to insert
        text: String,
    },
    /// IME composition cancelled (discard pre-edit)
    Cancel,
}

// ============================================================================
// SPEC-HAL-009: Event Phases
// ============================================================================

/// Event propagation phase (Capture → Target → Bubble)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPhase {
    Capture, // Root to target
    Target,  // At target widget
    Bubble,  // Target to root
}

// ============================================================================
// SPEC-HAL-079: Input Pipeline Phases
// ============================================================================

/// SPEC-HAL-079: Four-phase input pipeline.
///
/// Events flow through these phases in strict order:
/// 1. PreHitTest - Global filters, coalescing, prediction (before hit-test)
/// 2. HitTest - Tree walk to find target widget
/// 3. Routing - Capture/Target/Bubble dispatch to widgets
/// 4. PostProcess - Gesture resolution, cleanup (placeholder today)
///
/// INV-079-PHASE-ORDER: Phases MUST execute in order; no skipping.
/// INV-079-NO-DISPATCH-PREHITTEST: Widget dispatch forbidden during PreHitTest/HitTest.
/// INV-079-NO-HITTEST-ROUTING: Hit-test forbidden during Routing/PostProcess.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputPipelinePhase {
    /// Global filters, coalescing, prediction (before hit-test).
    /// No widget dispatch allowed in this phase.
    PreHitTest,
    /// Tree walk to find target widget.
    /// No widget dispatch allowed in this phase.
    HitTest,
    /// Capture/Target/Bubble dispatch to widgets.
    /// Widget event handlers run in this phase.
    Routing,
    /// Gesture resolution, cleanup.
    /// PostProcess is a placeholder today; gesture arena is Phase-3 (not yet started).
    PostProcess,
}

// ============================================================================
// SPEC-HAL-079: Capture Modes
// ============================================================================

/// SPEC-HAL-079: Capture mode for pointer events.
///
/// Determines how capture behaves when the pointer moves.
///
/// INV-PC-MODE-EXPLICIT: Capture mode is set explicitly, never implicitly.
/// INV-PC-MODE-HARD-PRIORITY: Hard capture always takes priority over Soft.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CaptureMode {
    /// Widget receives all pointer events regardless of position.
    /// Released only on: pointer-up, explicit cancel, widget destruction.
    /// This is the default and existing behavior.
    Hard,
    /// Widget receives events first IF pointer still intersects bounds.
    /// Auto-released when pointer exits widget bounds (uses current layout rect).
    /// Entry is explicit (on PointerDown or activation), never on hover.
    Soft,
}

impl Default for CaptureMode {
    fn default() -> Self {
        CaptureMode::Hard
    }
}

// ============================================================================
// SPEC-HAL-009: Hit-Testing
// ============================================================================

/// Hit test result
#[derive(Debug, Clone)]
pub struct HitTestResult {
    pub widget_id: TreeWidgetId,
    pub local_pos: (f32, f32),
    pub absolute_bounds: (f32, f32, f32, f32), // (x, y, width, height)
}

/// Internal provenance for a routed pointer hit.
///
/// Overlay provenance is kept separate from `PointerEvent`: it is resolved by
/// the engine hit-test authority, not supplied by the platform input source.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PointerRouteOrigin {
    Regular,
    Overlay,
}

// ============================================================================
// SPEC-HAL-009: Focus Manager
// ============================================================================

/// Focus manager - tracks which widget has keyboard focus
/// SPEC-HAL-038: Extended with request_focus/clear_focus for lifecycle management
/// SPEC-HAL-038 Stage 2: Extended with focus_chain for Tab navigation
/// SPEC-HAL-038 Stage 3: Extended with focus_scope_stack for hybrid scopes
/// SPEC-HAL-038 Stage 10: Extended with last_focused_before_deactivate for auto-restore
#[derive(Debug)]
pub struct FocusManager {
    pub(crate) focused: Option<TreeWidgetId>,
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    browser_focus_epoch: u64,
    keyboard_focus_cue: Option<WidgetMountToken>,
    pub(crate) interaction_domain_id: InteractionDomainId,
    pub(crate) focus_domains: HashMap<FocusDomainId, FocusDomainRecord>,
    pub(crate) active_focus_domain: Option<FocusDomainId>,
    pub(crate) restoration_stack: Vec<FocusRestorationFrame>,
    pub(crate) next_restoration_generation: u64,
    /// SPEC-HAL-038 Stage 2: Ordered list of focusable widgets (DFS order)
    focus_chain: Vec<TreeWidgetId>,
    /// SPEC-HAL-038 Stage 3: Stack of active focus scope roots (innermost on top)
    focus_scope_stack: Vec<NodeId>,
    /// SPEC-HAL-038 Stage 10: Remember focus before window deactivation for auto-restore
    last_focused_before_deactivate: Option<TreeWidgetId>,
    /// Last widget explicitly focused by user action or programmatic focus request.
    /// Survives clear_focus (ESC, window deactivation clear, etc.).
    /// Used to restore focus position on TAB entry when no widget is currently focused.
    last_user_focused: Option<TreeWidgetId>,
    availability_cleared_focus: Option<Option<NodeId>>,
}

impl FocusManager {
    pub fn new() -> Self {
        Self::new_for_domain(InteractionDomainId::PRIMARY)
    }

    pub(crate) fn new_for_domain(interaction_domain_id: InteractionDomainId) -> Self {
        Self {
            focused: None,
            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
            browser_focus_epoch: 0,
            keyboard_focus_cue: None,
            interaction_domain_id,
            focus_domains: HashMap::new(),
            active_focus_domain: None,
            restoration_stack: Vec::new(),
            next_restoration_generation: 1,
            focus_chain: Vec::new(),
            focus_scope_stack: Vec::new(),
            last_focused_before_deactivate: None,
            last_user_focused: None,
            availability_cleared_focus: None,
        }
    }

    pub fn focused(&self) -> Option<TreeWidgetId> {
        self.focused
    }

    pub(crate) fn focused_mount(&self, tree: &WidgetTree) -> Option<WidgetMountToken> {
        tree.concrete_mount(self.focused?)
    }

    pub(crate) fn keyboard_focus_cue(&self, tree: &WidgetTree) -> bool {
        self.keyboard_focus_cue.as_ref().is_some_and(|mount| {
            self.is_focused(mount.owner()) && mount.is_live() && tree.owns_mount_tree(mount)
        })
    }

    /// Input routing can change the cue without transferring semantic focus.
    pub(crate) fn note_keyboard_focus_cue(&mut self, mount: Option<WidgetMountToken>) -> bool {
        let Some(mount) = mount.filter(|mount| self.is_focused(mount.owner()) && mount.is_live())
        else {
            return false;
        };
        if self.keyboard_focus_cue.as_ref() == Some(&mount) {
            return false;
        }
        self.keyboard_focus_cue = Some(mount);
        true
    }

    pub(crate) fn clear_keyboard_focus_cue(&mut self) -> bool {
        self.keyboard_focus_cue.take().is_some()
    }

    pub(crate) fn commit_focus_domain(
        &mut self,
        domain: Option<FocusDomainId>,
        owner: Option<TreeWidgetId>,
    ) {
        if self.active_focus_domain != domain || self.focused != owner {
            self.keyboard_focus_cue = None;
            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
            self.advance_browser_focus_epoch();
        }
        self.active_focus_domain = domain;
        self.focused = owner;
    }

    pub fn set_focus(&mut self, new_focus: Option<TreeWidgetId>) {
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        if self.focused != new_focus {
            self.advance_browser_focus_epoch();
        }
        if new_focus.is_some() {
            self.availability_cleared_focus = None;
        }
        self.focused = new_focus;
        self.keyboard_focus_cue = None;
    }

    /// SPEC-HAL-038: Check if a specific widget is currently focused.
    pub fn is_focused(&self, id: TreeWidgetId) -> bool {
        self.focused == Some(id)
    }

    /// SPEC-HAL-038: Request focus for a widget.
    ///
    /// Returns (old_focus, new_focus) tuple for the caller to handle lifecycle.
    /// - If already focused on this widget, returns (None, None) (no change)
    /// - If changing focus, returns (old_id, new_id) so caller can call on_blur/on_focus
    ///
    /// Note: Caller (EventRouter) is responsible for calling on_blur on old widget
    /// and on_focus on new widget, as well as emitting FocusEvent variants.
    pub fn request_focus(
        &mut self,
        id: TreeWidgetId,
    ) -> (Option<TreeWidgetId>, Option<TreeWidgetId>) {
        self.availability_cleared_focus = None;
        if self.focused == Some(id) {
            (None, None)
        } else {
            let old = self.focused;
            #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
            self.advance_browser_focus_epoch();
            self.focused = Some(id);
            self.keyboard_focus_cue = None;
            self.last_user_focused = Some(id);
            (old, Some(id))
        }
    }

    /// SPEC-HAL-038: Clear focus from the currently focused widget.
    ///
    /// Returns the previously focused widget ID (if any) so caller can handle lifecycle.
    /// Caller (EventRouter) is responsible for calling on_blur on the old widget.
    pub fn clear_focus(&mut self) -> Option<TreeWidgetId> {
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        if self.focused.is_some() {
            self.advance_browser_focus_epoch();
        }
        self.keyboard_focus_cue = None;
        self.focused.take()
    }

    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    pub(crate) fn browser_focus_epoch(&self) -> u64 {
        self.browser_focus_epoch
    }

    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    fn advance_browser_focus_epoch(&mut self) {
        self.browser_focus_epoch = self.browser_focus_epoch.checked_add(1)
            .expect("HAL-398 focus generation exhausted");
    }

    pub(crate) fn retain_availability_focus_clear(&mut self) {
        self.availability_cleared_focus = Some(self.active_scope());
    }

    pub(crate) fn availability_keeps_focus_empty(&self) -> bool {
        self.availability_cleared_focus == Some(self.active_scope()) && self.focused.is_none()
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 10: Focus Memory for Auto-Restore
    // ========================================================================

    /// SPEC-HAL-038 Stage 10: Remember the focused widget before window deactivation.
    ///
    /// Called when window loses focus to store the currently focused widget
    /// for potential restoration when window regains focus.
    pub fn remember_focus(&mut self, id: TreeWidgetId) {
        self.last_focused_before_deactivate = Some(id);
    }

    /// SPEC-HAL-038 Stage 10: Take the last focused widget for restoration.
    ///
    /// Returns and clears the stored focus memory. Called during window activation
    /// to attempt focus restoration. Returns None if no focus was remembered.
    pub fn take_last_focus(&mut self) -> Option<TreeWidgetId> {
        self.last_focused_before_deactivate.take()
    }

    /// SPEC-HAL-038 Stage 10: Check if there's a remembered focus to restore.
    pub fn has_remembered_focus(&self) -> bool {
        self.last_focused_before_deactivate.is_some()
    }

    // ========================================================================
    // Last User Focus Memory
    // ========================================================================

    pub fn last_user_focused(&self) -> Option<TreeWidgetId> {
        self.last_user_focused
    }

    /// Check whether a specific widget is currently visible within the viewport.
    ///
    /// Uses the same screen-space rect computation as `first_visible_focusable`.
    /// Returns false if the widget is not in the tree, not focusable, or not
    /// intersecting the viewport.
    pub fn is_widget_visible_in_viewport(
        &self,
        tree: &crate::core::widget_tree::WidgetTree,
        widget_id: TreeWidgetId,
        search_root: NodeId,
        viewport: RectF,
    ) -> bool {
        let mut visible: Vec<(TreeWidgetId, RectF, NodeId)> = Vec::new();
        Self::collect_visible_focusables_screen_space(tree, search_root, viewport, &mut visible);
        visible.iter().any(|(id, _, _)| *id == widget_id)
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 3: Focus Scopes
    // ========================================================================

    /// SPEC-HAL-038 Stage 3: Push a new active focus scope.
    ///
    /// When a scope is active, Tab navigation is constrained to focusable
    /// widgets within that scope's subtree. Scopes can be nested; the
    /// innermost (most recently pushed) scope wins.
    ///
    /// Call this when opening a modal, dropdown, or other overlay that
    /// should trap focus.
    pub fn push_scope(&mut self, root: NodeId) {
        self.focus_scope_stack.push(root);
    }

    /// SPEC-HAL-038 Stage 3: Pop the active focus scope.
    ///
    /// Returns the popped scope root (if any). When a scope is popped,
    /// Tab navigation returns to the parent scope (or global if no
    /// scopes remain).
    ///
    /// Call this when closing a modal or overlay.
    pub fn pop_scope(&mut self) -> Option<NodeId> {
        self.focus_scope_stack.pop()
    }

    /// SPEC-HAL-038 Stage 3: Get the currently active focus scope root.
    ///
    /// Returns None if no scope is active (global Tab navigation).
    /// Returns Some(NodeId) for the innermost active scope root.
    pub fn active_scope(&self) -> Option<NodeId> {
        self.focus_scope_stack.last().copied()
    }

    /// SPEC-HAL-038 Stage 4: Get the number of active focus scopes.
    ///
    /// Returns 0 when no scopes are active (global Tab navigation).
    pub fn scope_depth(&self) -> usize {
        self.focus_scope_stack.len()
    }

    /// SPEC-HAL-038 Stage 3: Check if the currently focused widget is inside a scope.
    ///
    /// Used to determine if focus should be cleared when a scope is popped.
    pub fn is_focused_in_scope(
        &self,
        tree: &crate::core::widget_tree::WidgetTree,
        scope_root: NodeId,
    ) -> bool {
        if let Some(focused_id) = self.focused {
            // Find the NodeId for the focused widget
            if let Some(focused_node_id) = tree.find_node_by_widget_id(focused_id) {
                // Check if focused_node_id is a descendant of scope_root
                return tree.is_descendant(focused_node_id, scope_root);
            }
        }
        false
    }

    // ========================================================================
    // SPEC-HAL-038 Stage 2: Tab Navigation
    // ========================================================================

    /// SPEC-HAL-038 Stage 2: Rebuild the focus chain from the widget tree.
    ///
    /// Performs a DFS traversal of the tree, collecting all focusable widgets
    /// in traversal order. If the currently focused widget is no longer in
    /// the tree, focus is cleared.
    ///
    /// SPEC-HAL-038 Stage 3: If an active focus scope exists, uses that as
    /// the root instead of the provided root. This constrains Tab navigation
    /// to the scope's subtree.
    ///
    /// Call this before Tab navigation or after tree structure changes.
    pub fn rebuild_focus_chain(
        &mut self,
        tree: &crate::core::widget_tree::WidgetTree,
        root: NodeId,
    ) {
        // SPEC-HAL-038 Stage 3: Use active scope as root if present
        let effective_root = self.active_scope().unwrap_or(root);

        self.focus_chain.clear();
        Self::collect_focusables_dfs(tree, effective_root, None, &mut self.focus_chain);

        // If current focused widget is no longer in chain, clear focus
        if let Some(focused_id) = self.focused {
            if !self.focus_chain.contains(&focused_id) {
                self.clear_focus();
            }
        }
    }

    pub(crate) fn next_after_availability_change(
        &self,
        tree: &crate::core::widget_tree::WidgetTree,
        root: NodeId,
        previous: TreeWidgetId,
    ) -> Option<TreeWidgetId> {
        let mut chain = Vec::new();
        Self::collect_focusables_dfs(
            tree,
            self.active_scope().unwrap_or(root),
            Some(previous),
            &mut chain,
        );
        let index = chain.iter().position(|owner| *owner == previous)?;
        chain.get(index + 1).copied()
    }

    /// DFS helper to collect focusable widgets
    fn collect_focusables_dfs(
        tree: &crate::core::widget_tree::WidgetTree,
        node_id: NodeId,
        previous: Option<TreeWidgetId>,
        chain: &mut Vec<TreeWidgetId>,
    ) {
        if let Some(node) = tree.node(node_id) {
            // Check if this widget is focusable
            if node.widget.focusable() || previous == Some(node.id) {
                chain.push(node.id);
            }

            // Recurse into children
            let children = tree.children(node_id);
            for child_id in children {
                Self::collect_focusables_dfs(tree, child_id, previous, chain);
            }
        }
    }

    /// SPEC-HAL-038 Stage 2: Get the next focusable widget for Tab navigation.
    ///
    /// Returns the next widget ID in the focus chain, or None if:
    /// - The focus chain is empty
    ///
    /// Behavior:
    /// - If no widget is focused: returns the first focusable widget
    /// - If focused widget is in chain: returns the next widget (with wrap-around)
    /// - If only one focusable widget: returns that widget (no-op but valid)
    pub fn tab_next(&self) -> Option<TreeWidgetId> {
        if self.focus_chain.is_empty() {
            return None;
        }

        match self.focused {
            None => {
                // No focus: return first focusable
                self.focus_chain.first().copied()
            }
            Some(current_id) => {
                // Find current index and compute next with wrap
                if let Some(idx) = self.focus_chain.iter().position(|&id| id == current_id) {
                    let next_idx = (idx + 1) % self.focus_chain.len();
                    Some(self.focus_chain[next_idx])
                } else {
                    // Current not in chain (shouldn't happen after rebuild), return first
                    self.focus_chain.first().copied()
                }
            }
        }
    }

    /// SPEC-HAL-038 Stage 2: Get the previous focusable widget for Shift+Tab navigation.
    ///
    /// Returns the previous widget ID in the focus chain, or None if:
    /// - The focus chain is empty
    ///
    /// Behavior:
    /// - If no widget is focused: returns the last focusable widget
    /// - If focused widget is in chain: returns the previous widget (with wrap-around)
    /// - If only one focusable widget: returns that widget (no-op but valid)
    pub fn tab_prev(&self) -> Option<TreeWidgetId> {
        if self.focus_chain.is_empty() {
            return None;
        }

        match self.focused {
            None => {
                // No focus: return last focusable
                self.focus_chain.last().copied()
            }
            Some(current_id) => {
                // Find current index and compute prev with wrap
                if let Some(idx) = self.focus_chain.iter().position(|&id| id == current_id) {
                    let prev_idx = if idx == 0 {
                        self.focus_chain.len() - 1
                    } else {
                        idx - 1
                    };
                    Some(self.focus_chain[prev_idx])
                } else {
                    // Current not in chain, return last
                    self.focus_chain.last().copied()
                }
            }
        }
    }

    /// Get the index of a widget in the current focus chain.
    pub fn focus_chain_index(&self, widget_id: TreeWidgetId) -> Option<usize> {
        self.focus_chain.iter().position(|&id| id == widget_id)
    }

    /// Get the widget at a given focus chain index (after rebuild).
    pub fn focus_chain_at(&self, index: usize) -> Option<TreeWidgetId> {
        self.focus_chain.get(index).copied()
    }

    /// SPEC-HAL-038 Stage 2: Get the current focus chain (for testing/debugging).
    pub fn focus_chain(&self) -> &[TreeWidgetId] {
        &self.focus_chain
    }

    // ========================================================================
    // INV-FOCUS-VIEWPORT-ENTRY: Viewport-Aware TAB Focus Entry
    // ========================================================================
    //
    // When focus is entered via TAB (no widget currently focused), the target
    // MUST be the first *visible* focusable widget in the active viewport.
    // Tree order MUST NOT be used for initial TAB focus.
    //
    // INV-FOCUS-COORDSPACE: Visibility checks MUST use screen-space rectangles.
    // - Viewport = window bounds (root's absolute_paint_rect)
    // - Candidate screen_rect = absolute_paint_rect - cumulative_scroll_offset
    //
    // Visual order = sort by y (top-to-bottom), then x (left-to-right).
    // ========================================================================

    /// INV-FOCUS-VIEWPORT-ENTRY: Get the first visible focusable widget for TAB entry.
    ///
    /// Collects focusable widgets whose **screen-space rect** intersects the viewport.
    /// Screen-space rect = absolute_paint_rect translated by cumulative scroll offset.
    ///
    /// Sorts them by visual order (y then x), and returns the first (or last for reverse).
    ///
    /// Returns None if no visible focusable widgets exist.
    pub fn first_visible_focusable(
        &self,
        tree: &crate::core::widget_tree::WidgetTree,
        search_root: NodeId,
        viewport: RectF,
        reverse: bool,
    ) -> Option<TreeWidgetId> {
        let mut visible_focusables: Vec<(TreeWidgetId, RectF, NodeId)> = Vec::new();

        // Collect focusable widgets that intersect the viewport (in screen space)
        Self::collect_visible_focusables_screen_space(
            tree,
            search_root,
            viewport,
            &mut visible_focusables,
        );

        if visible_focusables.is_empty() {
            return None;
        }

        // Sort by visual order: primary y (top-to-bottom), secondary x (left-to-right)
        // Use screen_rect for sorting (already in screen space)
        visible_focusables.sort_by(|a, b| {
            let y_cmp =
                a.1.y
                    .partial_cmp(&b.1.y)
                    .unwrap_or(std::cmp::Ordering::Equal);
            if y_cmp == std::cmp::Ordering::Equal {
                a.1.x
                    .partial_cmp(&b.1.x)
                    .unwrap_or(std::cmp::Ordering::Equal)
            } else {
                y_cmp
            }
        });

        if reverse {
            visible_focusables.last().map(|(id, _, _)| *id)
        } else {
            visible_focusables.first().map(|(id, _, _)| *id)
        }
    }

    /// DFS helper to collect focusable widgets that intersect the viewport in SCREEN SPACE.
    ///
    /// INV-FOCUS-COORDSPACE: Computes screen_rect = absolute_paint_rect - scroll_offset
    /// so that visibility check compares rects in the same coordinate space.
    pub(crate) fn collect_visible_focusables_screen_space(
        tree: &crate::core::widget_tree::WidgetTree,
        node_id: NodeId,
        viewport: RectF,
        result: &mut Vec<(TreeWidgetId, RectF, NodeId)>,
    ) {
        if let Some(node) = tree.node(node_id) {
            // Get content-space rect (SPEC-HAL-141: use getter for private geometry field)
            let content_rect = tree
                .get_absolute_paint_rect(node_id)
                .unwrap_or(RectF::zero());

            // Compute screen-space rect by subtracting cumulative scroll offset
            // Screen position = content position - scroll offset
            let (scroll_x, scroll_y) = tree.cumulative_scroll_offset(node_id);
            let screen_rect = RectF::new(
                content_rect.x - scroll_x,
                content_rect.y - scroll_y,
                content_rect.width,
                content_rect.height,
            );

            // Check if this widget is focusable AND visible in viewport (screen space)
            if node.widget.focusable() && rects_intersect(screen_rect, viewport) {
                result.push((node.id, screen_rect, node_id));
            }

            // Recurse into children
            let children = tree.children(node_id);
            for child_id in children {
                Self::collect_visible_focusables_screen_space(tree, child_id, viewport, result);
            }
        }
    }
}

/// Check if two rectangles intersect.
fn rects_intersect(a: RectF, b: RectF) -> bool {
    a.x < b.x + b.width && a.x + a.width > b.x && a.y < b.y + b.height && a.y + a.height > b.y
}

impl Default for FocusManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-HAL-009: Pointer Capture
// ============================================================================

/// Pointer capture - for drag operations
pub struct PointerCapture {
    pub captured_widget: Option<TreeWidgetId>,
    pub button: Option<PointerButton>,
}

impl PointerCapture {
    pub fn new() -> Self {
        Self {
            captured_widget: None,
            button: None,
        }
    }

    pub fn capture(&mut self, widget_id: TreeWidgetId, button: PointerButton) {
        self.captured_widget = Some(widget_id);
        self.button = Some(button);
    }

    pub fn release(&mut self) {
        self.captured_widget = None;
        self.button = None;
    }

    pub fn is_captured_by(&self, widget_id: TreeWidgetId) -> bool {
        self.captured_widget == Some(widget_id)
    }

    pub fn is_captured(&self) -> bool {
        self.captured_widget.is_some()
    }
}

impl Default for PointerCapture {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-HAL-128: Pointer Capture Manager
// ============================================================================

/// SPEC-HAL-128: Unique identifier for a pointer stream.
///
/// Mouse = single pointer (id 0).
/// Touch/stylus may produce multiple pointers (future extension).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct PointerId(pub u64);

impl PointerId {
    /// The primary mouse pointer (always id 0).
    pub const MOUSE: PointerId = PointerId(0);
}

/// SPEC-HAL-128: Error returned when capture request fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CaptureError {
    /// Another widget already holds capture for this pointer (INV-PC2).
    AlreadyCaptured { by: TreeWidgetId },
}

/// SPEC-HAL-128: Pointer Capture Manager
///
/// Provides global, per-pointer capture tracking with the following invariants:
///
/// INV-PC1 — Explicit Capture Only
///   Pointer capture occurs ONLY via explicit request.
///   PointerDown alone MUST NOT capture.
///
/// INV-PC2 — Per-Pointer Exclusivity
///   At most one widget may capture a PointerId.
///
/// INV-PC3 — Capture Lifetime
///   Capture ends ONLY on:
///     - pointer up
///     - explicit cancel
///     - widget destruction
///     - SPEC-HAL-079: soft capture auto-releases when pointer exits bounds
///
/// INV-PC6 — Guaranteed Pointer-Up Delivery
///   If a widget captures a pointer, it MUST receive pointer-up,
///   even if pointer leaves window bounds.
///
/// SPEC-HAL-079: CaptureMode support (Soft vs Hard).
/// INV-PC-MODE-EXPLICIT: Capture mode is set explicitly, never implicitly.
/// INV-PC-MODE-HARD-PRIORITY: Hard capture always takes priority over Soft.
///
pub struct PointerCaptureManager {
    /// Per-pointer capture state: PointerId -> (capturing widget, mode)
    captures: std::collections::HashMap<PointerId, CaptureEntry>,
}

/// SPEC-HAL-079: Internal capture entry storing widget and mode.
#[derive(Debug)]
struct CaptureEntry {
    widget_id: TreeWidgetId,
    mode: CaptureMode,
    touch_press: Option<(TouchPressIdentity, WidgetMountToken)>,
}

impl PointerCaptureManager {
    pub fn new() -> Self {
        Self {
            captures: std::collections::HashMap::new(),
        }
    }

    /// Request capture for a pointer by a widget.
    ///
    /// INV-PC1: Capture is explicit only - this is the sole entry point.
    /// INV-PC2: Returns error if another widget already has capture.
    /// SPEC-HAL-079: Mode defaults to Hard for backward compatibility.
    ///
    /// Returns `Ok(())` on success, `Err(CaptureError::AlreadyCaptured)` if
    /// another widget holds capture for this pointer.
    pub fn request_capture(
        &mut self,
        pointer_id: PointerId,
        widget_id: TreeWidgetId,
    ) -> Result<(), CaptureError> {
        self.request_capture_with_mode(pointer_id, widget_id, CaptureMode::Hard)
    }

    /// SPEC-HAL-079: Request capture with explicit mode.
    ///
    /// INV-PC1: Capture is explicit only - this is the sole entry point.
    /// INV-PC2: Returns error if another widget already has capture.
    /// INV-PC-MODE-EXPLICIT: Mode is explicitly specified.
    ///
    /// Returns `Ok(())` on success, `Err(CaptureError::AlreadyCaptured)` if
    /// another widget holds capture for this pointer.
    pub fn request_capture_with_mode(
        &mut self,
        pointer_id: PointerId,
        widget_id: TreeWidgetId,
        mode: CaptureMode,
    ) -> Result<(), CaptureError> {
        // INV-PC2: Check if already captured by another widget
        if let Some(existing) = self.captures.get(&pointer_id) {
            if existing.widget_id != widget_id {
                return Err(CaptureError::AlreadyCaptured {
                    by: existing.widget_id,
                });
            }
            // Already captured by same widget - no-op success
            return Ok(());
        }

        // INV-PC1: Explicit capture granted
        self.captures.insert(
            pointer_id,
            CaptureEntry {
                widget_id,
                mode,
                touch_press: None,
            },
        );
        tracing::debug!(
            "SPEC-HAL-128: Widget {:?} captured pointer {:?} (mode={:?})",
            widget_id.0,
            pointer_id.0,
            mode
        );
        Ok(())
    }

    pub(crate) fn qualify_touch_capture(
        &mut self,
        press: TouchPressIdentity,
        mount: WidgetMountToken,
    ) {
        if let Some(entry) = self.captures.get_mut(&press.pointer)
            && entry.widget_id == mount.owner()
        {
            entry.touch_press = Some((press, mount));
        }
    }

    pub(crate) fn owns_touch_press(
        &self,
        press: TouchPressIdentity,
        mount: &WidgetMountToken,
    ) -> bool {
        self.captures.get(&press.pointer).is_some_and(|entry| {
            entry.widget_id == mount.owner()
                && entry.mode == CaptureMode::Hard
                && entry
                    .touch_press
                    .as_ref()
                    .is_some_and(|(identity, former_mount)| {
                        *identity == press && former_mount == mount && former_mount.is_live()
                    })
        })
    }

    /// HAL-396 §5: publish a previously validated touch-scroll capture.
    pub(crate) fn capture_touch_scroll(
        &mut self,
        press: TouchPressIdentity,
        former_mount: Option<&WidgetMountToken>,
        winner: WidgetMountToken,
    ) {
        assert!(self.current_capture(press.pointer).is_none_or(|owner| {
            owner == winner.owner()
                || former_mount.is_some_and(|mount| self.owns_touch_press(press, mount))
        }));
        self.captures.insert(
            press.pointer,
            CaptureEntry {
                widget_id: winner.owner(),
                mode: CaptureMode::Hard,
                touch_press: Some((press, winner)),
            },
        );
    }

    /// Release capture for a pointer by the owning widget.
    ///
    /// INV-PC3: Explicit release is one way capture ends.
    ///
    /// Only succeeds if the widget currently holds the capture.
    /// If another widget holds capture, or no capture exists, this is a no-op.
    pub fn release_capture(&mut self, pointer_id: PointerId, widget_id: TreeWidgetId) {
        if let Some(existing) = self.captures.get(&pointer_id) {
            if existing.widget_id == widget_id {
                self.captures.remove(&pointer_id);
                tracing::debug!(
                    "SPEC-HAL-128: Widget {:?} released pointer {:?}",
                    widget_id.0,
                    pointer_id.0
                );
            }
        }
    }

    /// Cancel capture for a pointer unconditionally.
    ///
    /// INV-PC3: Explicit cancel is one way capture ends.
    ///
    /// Used for:
    /// - Pointer up events (automatic release)
    /// - ESC cancellation
    /// - Widget destruction
    pub fn cancel_capture(&mut self, pointer_id: PointerId) {
        if let Some(entry) = self.captures.remove(&pointer_id) {
            tracing::debug!(
                "SPEC-HAL-128: Cancelled capture for pointer {:?} (was {:?})",
                pointer_id.0,
                entry.widget_id.0
            );
        }
    }

    /// Query which widget currently captures a pointer.
    ///
    /// Returns `None` if the pointer is not captured.
    pub fn current_capture(&self, pointer_id: PointerId) -> Option<TreeWidgetId> {
        self.captures.get(&pointer_id).map(|e| e.widget_id)
    }

    /// SPEC-HAL-079: Query capture with mode info.
    ///
    /// Returns `None` if the pointer is not captured.
    pub fn current_capture_with_mode(
        &self,
        pointer_id: PointerId,
    ) -> Option<(TreeWidgetId, CaptureMode)> {
        self.captures
            .get(&pointer_id)
            .map(|e| (e.widget_id, e.mode))
    }

    /// Check if a pointer is currently captured.
    pub fn is_captured(&self, pointer_id: PointerId) -> bool {
        self.captures.contains_key(&pointer_id)
    }

    /// Check if a specific widget holds capture for a pointer.
    pub fn is_captured_by(&self, pointer_id: PointerId, widget_id: TreeWidgetId) -> bool {
        self.captures
            .get(&pointer_id)
            .map(|e| e.widget_id == widget_id)
            .unwrap_or(false)
    }

    /// SPEC-HAL-145 / SPEC-HAL-078A: Whether any pointer is currently captured
    /// in this domain.
    ///
    /// Used by the frame pipeline to suppress layout-transition binding while
    /// the user is direct-manipulating (drag, drag-scroll, drag-select, etc.).
    /// During direct manipulation, layout transitions would introduce a
    /// 300ms-eased lag between input and visual response — the wrong contract.
    /// Layout transitions exist for non-user-driven layout changes only.
    ///
    /// Allocation-free (unlike `active_pointers()` which returns a `Vec`).
    pub fn is_any_captured(&self) -> bool {
        !self.captures.is_empty()
    }

    /// Release all captures held by a specific widget.
    ///
    /// INV-PC3: Widget destruction releases captures.
    ///
    /// Called when a widget is removed from the tree.
    pub fn release_all_for_widget(&mut self, widget_id: TreeWidgetId) {
        self.captures.retain(|pointer_id, entry| {
            if entry.widget_id == widget_id {
                tracing::debug!(
                    "SPEC-HAL-128: Widget {:?} destruction released pointer {:?}",
                    widget_id.0,
                    pointer_id.0
                );
                false
            } else {
                true
            }
        });
    }

    /// SPEC-HAL-128 Stage 6: Get all currently active (captured) pointer IDs.
    ///
    /// Used by `cancel_all_pointer_sessions` and other multi-pointer operations
    /// to iterate over all active pointers without hardcoding PointerId::MOUSE.
    pub fn active_pointers(&self) -> Vec<PointerId> {
        self.captures.keys().copied().collect()
    }

    /// SPEC-HAL-151: Cancel all captures unconditionally.
    ///
    /// Used for domain teardown on focus loss.
    /// INV-151-FOCUS-TEARDOWN: All captures must be released when window loses focus.
    pub fn cancel_all_captures(&mut self) {
        let count = self.captures.len();
        self.captures.clear();
        if count > 0 {
            tracing::debug!(
                "SPEC-HAL-151: Cancelled all {} captures on focus loss",
                count
            );
        }
    }

    /// SPEC-HAL-079: Check and auto-release soft captures that pointer has exited.
    ///
    /// Called during PointerMove processing.
    ///
    /// CRITICAL: Queries CURRENT layout rect from tree, never uses cached bounds.
    /// This ensures correctness after layout rebuilds.
    ///
    /// Returns true if a soft capture was released.
    pub fn check_soft_capture_exit(
        &mut self,
        pointer_id: PointerId,
        pointer_pos: (f32, f32),
        tree: &crate::core::widget_tree::WidgetTree,
    ) -> bool {
        // Check if there's a soft capture for this pointer
        let should_release = if let Some(entry) = self.captures.get(&pointer_id) {
            if entry.mode == CaptureMode::Soft {
                // Query CURRENT layout rect from tree (not cached)
                if let Some(node_id) = tree.find_node_by_widget_id(entry.widget_id) {
                    if let Some(abs_rect) = tree.get_absolute_paint_rect(node_id) {
                        // Check if pointer is outside widget bounds
                        let (px, py) = pointer_pos;
                        let inside = px >= abs_rect.x
                            && px < abs_rect.x + abs_rect.width
                            && py >= abs_rect.y
                            && py < abs_rect.y + abs_rect.height;
                        !inside
                    } else {
                        // Widget has no bounds - release
                        true
                    }
                } else {
                    // Widget not in tree anymore - release
                    true
                }
            } else {
                // Hard capture - don't auto-release
                false
            }
        } else {
            // No capture
            false
        };

        if should_release {
            if let Some(entry) = self.captures.remove(&pointer_id) {
                tracing::debug!(
                    "SPEC-HAL-079: Soft capture auto-released for widget {:?} (pointer {:?} exited bounds)",
                    entry.widget_id.0,
                    pointer_id.0
                );
            }
            true
        } else {
            false
        }
    }
}

impl Default for PointerCaptureManager {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-HAL-128 Stage 2: Gesture Candidate Registration
// SPEC-HAL-128 Stage 2.1: Session-scoped gesture identity
// ============================================================================

/// SPEC-HAL-128 Stage 2.1: Unique identifier for a gesture session.
///
/// A session begins on PointerDown and ends on PointerUp/cancel.
/// All candidate IDs within a session share this session ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GestureSessionId(pub u64);

/// SPEC-HAL-128 Stage 2.1: Unique identifier for a gesture candidate.
///
/// Composed of session ID + local counter within the session.
/// Non-reused within a single pointer-down session.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GestureCandidateId {
    pub session: GestureSessionId,
    pub local: u32,
}

/// SPEC-HAL-128 Stage 2: Kind of gesture a candidate represents.
///
/// INV-GA1: Candidates declare their kind on registration.
/// SPEC-HAL-128 Stage 7: Extended with pointer-type-specific gestures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureKind {
    /// Drag gesture (content drag, tree reorder, etc.)
    Drag,
    /// Resize gesture (resize handles, column resizers) - Mouse only
    Resize,
    /// Scroll gesture (scroll containers, pan)
    Scroll,
    /// Custom gesture (widget-specific)
    Custom,
    /// SPEC-HAL-128 Stage 7: Selection gesture (text selection, marquee) - Mouse/Pen
    Select,
    /// SPEC-HAL-128 Stage 7: Two-finger pinch gesture - Touch only
    Pinch,
    /// SPEC-HAL-128 Stage 7: Swipe gesture - Touch only
    Swipe,
    /// SPEC-HAL-128 Stage 7: Drawing gesture - Pen only
    Draw,
}

/// SPEC-HAL-128 Stage 2: Axis constraint for gesture candidates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GestureAxis {
    /// Gesture is constrained to horizontal movement
    Horizontal,
    /// Gesture is constrained to vertical movement
    Vertical,
    /// Gesture can move in any direction
    Any,
}

/// SPEC-HAL-128 Stage 2: A registered gesture candidate.
///
/// INV-GA1: Candidates are registered on pointer-down.
/// INV-GA2: No capture or side-effects occur until commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GestureCandidate {
    /// Unique identifier for this candidate
    pub candidate_id: GestureCandidateId,
    /// Widget that registered this candidate
    pub widget_id: TreeWidgetId,
    /// Type of gesture
    pub kind: GestureKind,
    /// Static motion representation requested by this candidate.
    pub motion_policy: PointerMotionPolicy,
    /// Optional axis constraint
    pub axis: Option<GestureAxis>,
    /// Priority hint for arbitration (NOT USED IN STAGE 2)
    /// Reserved for Stage 3: gesture arbitration
    pub priority_hint: u8,
}

impl GestureCandidate {
    /// SPEC-HAL-128 Stage 2.1: Create a new gesture candidate with explicit ID.
    ///
    /// IDs are generated by GestureRegistry, not by this constructor.
    /// INV-GA1: Registration is non-exclusive and local.
    pub fn new_with_id(id: GestureCandidateId, widget_id: TreeWidgetId, kind: GestureKind) -> Self {
        Self {
            candidate_id: id,
            widget_id,
            kind,
            motion_policy: PointerMotionPolicy::Absolute,
            axis: None,
            priority_hint: 0,
        }
    }

    /// SPEC-HAL-128 Stage 2.1: Create a new gesture candidate with axis constraint and explicit ID.
    pub fn with_axis_with_id(
        id: GestureCandidateId,
        widget_id: TreeWidgetId,
        kind: GestureKind,
        axis: GestureAxis,
    ) -> Self {
        Self {
            candidate_id: id,
            widget_id,
            kind,
            motion_policy: PointerMotionPolicy::Absolute,
            axis: Some(axis),
            priority_hint: 0,
        }
    }
}

/// SPEC-HAL-128 Stage 2.1: Internal session state for gesture tracking.
/// SPEC-HAL-128 Stage 3: Extended with committed field for arbitration.
/// SPEC-HAL-128 Stage 7: Extended with pointer_type for semantic binding.
struct GestureSession {
    press: Option<Rc<PhysicalGesturePress>>,
    geometry_watches: HashMap<TreeWidgetId, crate::core::widget_tree::AbsoluteDragGeometryWatch>,
    touch_scroll_mounts: Vec<(GestureCandidateId, WidgetMountToken)>,
    touch_tap: Option<PendingTouchTap>,
    touch_drag: Option<(GestureCandidateId, WidgetMountToken, GestureAxis)>,
    id: GestureSessionId,
    down_facts: Option<(PointerButton, Point)>,
    /// SPEC-HAL-128 Stage 7: The pointer type for this session (INV-S7-P6 isolation).
    pointer_type: PointerType,
    next_local: u32,
    candidates: Vec<GestureCandidate>,
    /// SPEC-HAL-128 Stage 3: The committed gesture (if any).
    /// INV-GA3: At most one gesture may commit per session.
    committed: Option<GestureCandidateId>,
    /// SPEC-HAL-128 Stage 3 Fix: Frame-scoped commit queue.
    /// Commit requests are collected during dispatch and resolved once per pointer.
    pending_commits: Vec<GestureCandidateId>,
}

/// SPEC-HAL-128 Stage 2.1: Registry for gesture candidates with session-scoped IDs.
///
/// Provides per-pointer storage of registered candidates with session management.
/// A session begins on pointer-down and ends on pointer-up or cancel.
///
/// INV-GA1: Registration on pointer-down, no capture occurs.
pub struct GestureRegistry {
    lifetime: Rc<()>,
    latest_installed: u64,
    sessions: std::collections::HashMap<PointerId, GestureSession>,
    next_session_id: u64,
}

impl GestureRegistry {
    fn allocate_session_id(&mut self) -> GestureSessionId {
        let id = GestureSessionId(self.next_session_id);
        self.next_session_id = self
            .next_session_id
            .checked_add(1)
            .expect("INV-GESTURE-SESSION-NONREUSE");
        id
    }

    pub(crate) fn reserve_physical_press(
        &mut self,
        domain: InteractionDomainId,
        window: WindowId,
        pointer: PointerId,
        pointer_type: PointerType,
        button: PointerButton,
        position: Point,
        cutoff: crate::core::widget_tree::GeometryCausalCutoff,
    ) -> Rc<PhysicalGesturePress> {
        Rc::new(PhysicalGesturePress {
            registry_lifetime: self.lifetime.clone(),
            cutoff,
            domain,
            window,
            pointer,
            pointer_type,
            button,
            position,
            session: self.allocate_session_id(),
            state: Cell::new(PhysicalPressState::Reserved),
            eligible: Cell::new(true),
            protected: Cell::new(false),
            physical_release: Cell::new(false),
            next_emission: Cell::new(0),
            delivered: Cell::new(0),
        })
    }

    pub(crate) fn owns_press(&self, press: &PhysicalGesturePress) -> bool {
        Rc::ptr_eq(&self.lifetime, &press.registry_lifetime)
    }

    pub(crate) fn can_install_press(&self, press: &PhysicalGesturePress) -> bool {
        self.owns_press(press)
            && press.eligible.get()
            && press.state.get() == PhysicalPressState::Reserved
            && press.session.0 > self.latest_installed
    }

    pub(crate) fn install_physical_press(&mut self, press: Rc<PhysicalGesturePress>) -> bool {
        if !self.can_install_press(&press) {
            return false;
        }
        assert!(
            !self.sessions.contains_key(&press.pointer),
            "INV-ABSOLUTE-PRESS-INSTALL: cancel current session before install"
        );
        self.latest_installed = press.session.0;
        press.state.set(PhysicalPressState::Routed);
        self.sessions.insert(
            press.pointer,
            GestureSession {
                id: press.session,
                down_facts: Some((press.button, press.position)),
                pointer_type: press.pointer_type,
                next_local: 0,
                candidates: Vec::new(),
                committed: None,
                pending_commits: Vec::new(),
                press: Some(press),
                geometry_watches: HashMap::new(),
                touch_scroll_mounts: Vec::new(),
                touch_tap: None,
                touch_drag: None,
            },
        );
        true
    }

    pub(crate) fn physical_press(&self, pointer: PointerId) -> Option<Rc<PhysicalGesturePress>> {
        self.sessions.get(&pointer)?.press.clone()
    }

    pub(crate) fn register_touch_tap(&mut self, press: TouchPressIdentity, tap: PendingTouchTap) {
        if let Some(session) = self.sessions.get_mut(&press.pointer)
            && session.id == press.session
            && session.pointer_type == PointerType::Touch
        {
            session.touch_tap = Some(tap);
        }
    }

    pub(crate) fn take_touch_tap(&mut self, press: TouchPressIdentity) -> Option<PendingTouchTap> {
        self.sessions
            .get_mut(&press.pointer)
            .filter(|session| session.id == press.session)?
            .touch_tap
            .take()
    }

    pub(crate) fn register_touch_drag(
        &mut self, press: TouchPressIdentity, mount: WidgetMountToken, axis: GestureAxis,
    ) {
        if self.session_id(press.pointer) == Some(press.session)
            && self.register_candidate_request(press.pointer, mount.owner(), GestureKind::Drag, Some(axis))
        {
            let session = self.sessions.get_mut(&press.pointer).expect("live touch press");
            let candidate = session.candidates.last().expect("registered touch Drag");
            session.touch_drag = Some((candidate.candidate_id, mount, axis));
        }
    }

    pub(crate) fn touch_drag(
        &self, press: TouchPressIdentity,
    ) -> Option<&(GestureCandidateId, WidgetMountToken, GestureAxis)> {
        self.sessions.get(&press.pointer)
            .filter(|session| session.id == press.session)?
            .touch_drag.as_ref()
    }

    pub(crate) fn watch_absolute_geometry(
        &mut self,
        pointer: PointerId,
        watch: crate::core::widget_tree::AbsoluteDragGeometryWatch,
    ) {
        let session = self
            .sessions
            .get_mut(&pointer)
            .expect("INV-ABSOLUTE-GEOMETRY-QUALIFIED-HOST");
        let press = session
            .press
            .as_ref()
            .expect("INV-ABSOLUTE-GEOMETRY-QUALIFIED-HOST");
        assert!(
            press.eligible.get() && press.button == PointerButton::Primary,
            "INV-ABSOLUTE-GEOMETRY-ADMISSION: primary physical press required"
        );
        press.protected.set(true);
        session.geometry_watches.insert(watch.owner, watch);
    }

    pub(crate) fn geometry_watches(
        &self,
    ) -> Vec<(
        PointerId,
        Rc<PhysicalGesturePress>,
        crate::core::widget_tree::AbsoluteDragGeometryWatch,
    )> {
        self.sessions
            .iter()
            .flat_map(|(pointer, session)| {
                session.geometry_watches.values().filter_map(|watch| {
                    session
                        .press
                        .as_ref()
                        .map(|press| (*pointer, press.clone(), watch.clone()))
                })
            })
            .collect()
    }

    pub fn new() -> Self {
        Self {
            lifetime: Rc::new(()),
            latest_installed: 0,
            sessions: std::collections::HashMap::new(),
            next_session_id: 1,
        }
    }

    /// SPEC-HAL-128 Stage 2.1: Create/replace the active session for this pointer.
    /// SPEC-HAL-128 Stage 7: Now requires pointer_type for semantic binding (INV-S7-P1).
    ///
    /// Must be called on PointerDown before any registrations.
    pub fn begin_session(
        &mut self,
        pointer_id: PointerId,
        pointer_type: PointerType,
    ) -> GestureSessionId {
        self.begin_session_with_down_facts(pointer_id, pointer_type, None)
    }

    pub fn begin_session_with_down_facts(
        &mut self,
        pointer_id: PointerId,
        pointer_type: PointerType,
        down_facts: Option<(PointerButton, Point)>,
    ) -> GestureSessionId {
        // If a session already exists (e.g., lost PointerUp or rapid clicks), clear it first.
        // This is more robust than asserting - real-world scenarios can have orphaned sessions.
        if self.sessions.contains_key(&pointer_id) {
            self.clear_candidates(pointer_id);
        }

        let sid = self.allocate_session_id();
        self.latest_installed = sid.0;

        self.sessions.insert(
            pointer_id,
            GestureSession {
                press: None,
                geometry_watches: HashMap::new(),
                touch_scroll_mounts: Vec::new(),
                touch_tap: None,
                touch_drag: None,
                id: sid,
                down_facts,
                pointer_type, // SPEC-HAL-128 Stage 7: Store pointer type for legality checks
                next_local: 0,
                candidates: Vec::new(),
                committed: None, // SPEC-HAL-128 Stage 3: No committed gesture initially
                pending_commits: Vec::new(), // SPEC-HAL-128 Stage 3 Fix: Empty commit queue
            },
        );

        tracing::debug!(
            "SPEC-HAL-128 Stage 7: Began gesture session {:?} for pointer {:?} (type {:?})",
            sid.0,
            pointer_id.0,
            pointer_type
        );

        sid
    }

    pub fn session_down_facts(&self, pointer_id: PointerId) -> Option<(PointerButton, Point)> {
        self.sessions.get(&pointer_id)?.down_facts
    }

    /// Clear all candidates for a pointer (ends the session).
    ///
    /// Called on pointer-up or cancel.
    pub fn clear_candidates(&mut self, pointer_id: PointerId) {
        if let Some(press) = self
            .sessions
            .get(&pointer_id)
            .and_then(|session| session.press.as_ref())
        {
            press.state.set(PhysicalPressState::Completed);
        }
        if let Some(session) = self.sessions.remove(&pointer_id) {
            tracing::debug!(
                "SPEC-HAL-128 Stage 2.1: Cleared session {:?} with {} candidates for pointer {:?}",
                session.id.0,
                session.candidates.len(),
                pointer_id.0
            );
        }
    }

    /// Check if any candidates are registered for a pointer.
    pub fn has_candidates(&self, pointer_id: PointerId) -> bool {
        self.sessions
            .get(&pointer_id)
            .map(|s| !s.candidates.is_empty())
            .unwrap_or(false)
    }

    /// Get all candidates for a pointer.
    pub fn candidates(&self, pointer_id: PointerId) -> &[GestureCandidate] {
        self.sessions
            .get(&pointer_id)
            .map(|s| s.candidates.as_slice())
            .unwrap_or(&[])
    }

    /// SPEC-HAL-128 Stage 2.1: Get the session ID for a pointer.
    pub fn session_id(&self, pointer_id: PointerId) -> Option<GestureSessionId> {
        self.sessions.get(&pointer_id).map(|s| s.id)
    }

    /// SPEC-HAL-128 Stage 2.1: Register a candidate request, generating the ID.
    /// SPEC-HAL-128 Stage 7 INV-PT3: Reject on Registration.
    ///
    /// INV-GA1: Registration is non-exclusive; multiple widgets may register.
    /// INV-GA2: No capture or side-effects occur from registration.
    /// INV-PT2: Legality is static (PointerType, GestureKind).
    /// INV-PT3: Illegal gestures rejected at registration. No panics. No side effects.
    /// INV-PT4: Arbiter sees only legal candidates.
    ///
    /// Panics if called without an active session (enforces INV-GA1 structurally).
    /// Returns false and does NOT register if gesture is illegal for the pointer type.
    pub fn register_candidate_request(
        &mut self,
        pointer_id: PointerId,
        widget_id: TreeWidgetId,
        kind: GestureKind,
        axis: Option<GestureAxis>,
    ) -> bool {
        self.register_candidate_request_with_motion_policy(
            pointer_id,
            widget_id,
            kind,
            axis,
            PointerMotionPolicy::Absolute,
        )
    }

    /// Register a candidate with its static motion policy.
    pub fn register_candidate_request_with_motion_policy(
        &mut self,
        pointer_id: PointerId,
        widget_id: TreeWidgetId,
        kind: GestureKind,
        axis: Option<GestureAxis>,
        motion_policy: PointerMotionPolicy,
    ) -> bool {
        let session = self
            .sessions
            .get_mut(&pointer_id)
            .expect("GestureRegistry: register_candidate_request without active session");

        // SPEC-HAL-128 Stage 7 INV-PT3: Reject on Registration
        // Illegal gestures rejected silently. No panics. Session remains valid.
        if !session.pointer_type.is_gesture_legal(kind) {
            tracing::warn!(
                "SPEC-HAL-128 Stage 7 INV-PT3: REJECTED illegal gesture {:?} for pointer type {:?} (widget {:?})",
                kind,
                session.pointer_type,
                widget_id.0
            );
            return false;
        }

        let id = GestureCandidateId {
            session: session.id,
            local: session.next_local,
        };
        session.next_local += 1;

        let mut candidate = match axis {
            Some(a) => GestureCandidate::with_axis_with_id(id, widget_id, kind, a),
            None => GestureCandidate::new_with_id(id, widget_id, kind),
        };
        candidate.motion_policy = motion_policy;

        tracing::debug!(
            "SPEC-HAL-128 Stage 7: Registered gesture candidate session={:?} local={} for pointer {:?} (type {:?}, widget {:?}, kind {:?})",
            id.session.0,
            id.local,
            pointer_id.0,
            session.pointer_type,
            widget_id.0,
            kind
        );

        session.candidates.push(candidate);
        true
    }

    pub(crate) fn register_touch_scroll_candidate(
        &mut self,
        pointer: PointerId,
        mount: WidgetMountToken,
    ) {
        if self.register_candidate_request(pointer, mount.owner(), GestureKind::Scroll, None) {
            let session = self.sessions.get_mut(&pointer).expect("live touch session");
            let candidate = session
                .candidates
                .last()
                .expect("registered Scroll candidate");
            session
                .touch_scroll_mounts
                .push((candidate.candidate_id, mount));
        }
    }

    pub(crate) fn touch_scroll_mounts(
        &self,
        pointer: PointerId,
    ) -> &[(GestureCandidateId, WidgetMountToken)] {
        self.sessions
            .get(&pointer)
            .map_or(&[], |s| s.touch_scroll_mounts.as_slice())
    }

    pub(crate) fn candidate_mount_is_live(
        &self,
        pointer: PointerId,
        candidate: GestureCandidateId,
    ) -> bool {
        self.touch_scroll_mounts(pointer)
            .iter()
            .find(|(id, _)| *id == candidate)
            .is_none_or(|(_, mount)| mount.is_live())
    }

    /// SPEC-HAL-128 Stage 7: Get the pointer type for an active session.
    ///
    /// Returns None if no session exists for this pointer.
    pub fn session_pointer_type(&self, pointer_id: PointerId) -> Option<PointerType> {
        self.sessions.get(&pointer_id).map(|s| s.pointer_type)
    }

    // ========================================================================
    // SPEC-HAL-128 Stage 3 Fix: Frame-scoped commit queue methods
    // ========================================================================

    /// SPEC-HAL-128 Stage 3 Fix: Queue a commit request for later resolution.
    ///
    /// Commit requests are collected during event dispatch and resolved once
    /// per pointer after all widgets have processed the event.
    pub fn push_commit_request(&mut self, pointer_id: PointerId, candidate_id: GestureCandidateId) {
        let session = self.sessions.get_mut(&pointer_id);
        debug_assert!(
            session.is_some(),
            "push_commit_request: no active session for pointer"
        );
        if let Some(session) = session {
            session.pending_commits.push(candidate_id);
        }
    }

    /// SPEC-HAL-128 Stage 3 Fix: Take and clear all pending commit requests.
    ///
    /// Called once per pointer after event dispatch to resolve commits.
    pub fn take_commit_requests(&mut self, pointer_id: PointerId) -> Vec<GestureCandidateId> {
        self.sessions
            .get_mut(&pointer_id)
            .map(|s| std::mem::take(&mut s.pending_commits))
            .unwrap_or_default()
    }

    /// SPEC-HAL-128 Stage 3 Fix: Retain only the winning candidate.
    ///
    /// INV-GA7: Losers are structurally removed from the registry.
    /// After this call, candidates(pointer_id) returns exactly one candidate.
    pub fn retain_only_winner(&mut self, pointer_id: PointerId, winner: GestureCandidateId) {
        if let Some(session) = self.sessions.get_mut(&pointer_id) {
            debug_assert!(
                session.candidates.iter().any(|c| c.candidate_id == winner),
                "retain_only_winner: winner not found in candidates"
            );
            session.candidates.retain(|c| c.candidate_id == winner);
            session.touch_scroll_mounts.retain(|(id, _)| *id == winner);
            session.touch_tap = None;
            if session.touch_drag.as_ref().is_some_and(|(id, _, _)| *id != winner) {
                session.touch_drag = None;
            }
            let owner = session.candidates[0].widget_id;
            session.geometry_watches.retain(|id, _| *id == owner);
        }
    }
}

impl Default for GestureRegistry {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-HAL-128 Stage 4: Cancellation Reasons
// ============================================================================

/// SPEC-HAL-128 Stage 4: Reason for gesture/capture cancellation.
///
/// Used by `cancel_pointer_session` to track why a session was cancelled.
/// INV-PC4, INV-PC5, INV-PC7, INV-GA-CANCEL apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CancellationReason {
    /// User pressed Escape key (INV-PC4)
    Escape,
    /// Overlay took over input (INV-PC7 - SPEC-HAL-127)
    OverlayTakeover,
    /// Widget was destroyed while holding capture/gesture
    WidgetDestroyed,
    /// System-initiated cancel (e.g., window lost focus)
    SystemCancel,
}

// ============================================================================
// SPEC-HAL-128 Stage 3: Gesture Arbitration & Commit
// ============================================================================

/// SPEC-HAL-128 Stage 3: Error returned when gesture commit fails.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GestureCommitError {
    /// INV-GA3: Another gesture already committed for this pointer.
    AlreadyCommitted { winner: GestureCandidateId },
    /// Candidate not found in registry (stale or invalid ID).
    CandidateNotFound,
    /// No active session for this pointer.
    NoActiveSession,
}

/// SPEC-HAL-128 Stage 3: Result of a successful gesture commit.
#[derive(Debug, Clone)]
pub struct GestureCommitResult {
    /// The winning candidate that committed.
    pub winner: GestureCandidateId,
    /// Widget that owns the winning gesture.
    pub winner_widget: TreeWidgetId,
    /// Candidates that were cancelled as a result of this commit.
    pub cancelled: Vec<GestureCandidateId>,
}

/// SPEC-HAL-128 Stage 3: Gesture Arbiter
/// SPEC-HAL-128 Stage 7: Updated with pointer-type-aware arbitration.
///
/// Provides deterministic gesture arbitration with the following invariants:
///
/// INV-GA3 — Single Winner Rule
///   For a given PointerId, at most one gesture may commit.
///
/// INV-GA4 — Deterministic Priority Resolution (Stage 7 update)
///   SPEC-HAL-128 Stage 7 INV-S7-P4: If multiple candidates request commit, order is:
///   1. PointerType (implicit - all candidates in session share same type)
///   2. Primary Gesture (the primary gesture for this PointerType wins)
///   3. Axis Compatibility (not yet implemented)
///   4. Movement Distance / Threshold (not yet implemented)
///   5. Time (only if strictly needed) (not yet implemented)
///
/// INV-GA5 — Explicit Commit Only
///   A gesture commits only if candidate explicitly requests commit.
///   Implicit commit is FORBIDDEN.
///
/// INV-GA6 — Commit Causes Capture
///   On commit, pointer is captured exactly once by the committing widget.
///
/// INV-GA7 — Losers Are Cancelled
///   On commit, all non-winning candidates are cancelled and can never revive.
///
/// INV-S7-P7 — No Semantic Fallbacks
///   If a gesture cannot be legally committed, the session resolves to no commit.
///   Auto-downgrade, guessing intent, and "best effort" commits are FORBIDDEN.
pub struct GestureArbiter;

impl GestureArbiter {
    /// SPEC-HAL-128 Stage 3: Create a new arbiter (stateless).
    pub fn new() -> Self {
        Self
    }

    /// SPEC-HAL-128 Stage 7 INV-S7-P4: Get priority for a gesture kind relative to pointer type.
    ///
    /// Lower number = higher priority.
    /// Primary gesture for the pointer type gets priority 0.
    /// Secondary gestures get priority 1+.
    ///
    /// INV-S7-P3: Only one primary gesture may commit per session.
    fn kind_priority_for_pointer(pointer_type: PointerType, kind: GestureKind) -> u8 {
        // Primary gesture always wins (priority 0)
        if pointer_type.is_primary_gesture(kind) {
            return 0;
        }
        // Secondary gestures: assign stable priority based on kind
        match kind {
            GestureKind::Resize => 1,
            GestureKind::Drag => 2,
            GestureKind::Scroll => 3,
            GestureKind::Select => 4,
            GestureKind::Pinch => 5,
            GestureKind::Swipe => 6,
            GestureKind::Draw => 7,
            GestureKind::Custom => 8,
        }
    }

    /// SPEC-HAL-128 Stage 3: Attempt to commit a gesture.
    ///
    /// INV-GA5: Commit only happens when explicitly requested.
    /// INV-GA3: If another gesture already committed, returns error.
    /// INV-GA4: If multiple requests in same frame, highest priority wins.
    /// INV-GA7: Losing candidates are cancelled.
    ///
    /// On success, returns GestureCommitResult with winner and cancelled candidates.
    /// Caller (EventRouter) is responsible for:
    /// - INV-GA6: Capturing pointer for the winning widget
    /// - Removing cancelled candidates from registry
    pub fn try_commit(
        &self,
        registry: &mut GestureRegistry,
        pointer_id: PointerId,
        candidate_id: GestureCandidateId,
    ) -> Result<GestureCommitResult, GestureCommitError> {
        // INV-GA3: Check if already committed
        if let Some(winner) = registry.committed_gesture(pointer_id) {
            return Err(GestureCommitError::AlreadyCommitted { winner });
        }

        // Check session exists
        let session = registry
            .sessions
            .get(&pointer_id)
            .ok_or(GestureCommitError::NoActiveSession)?;

        // Find the requesting candidate
        let requesting_candidate = session
            .candidates
            .iter()
            .find(|c| c.candidate_id == candidate_id)
            .ok_or(GestureCommitError::CandidateNotFound)?;

        let winner_widget = requesting_candidate.widget_id;
        let winner_kind = requesting_candidate.kind;

        // Collect all other candidates to cancel (INV-GA7)
        let cancelled: Vec<GestureCandidateId> = session
            .candidates
            .iter()
            .filter(|c| c.candidate_id != candidate_id)
            .map(|c| c.candidate_id)
            .collect();

        tracing::debug!(
            "SPEC-HAL-128 Stage 3: Gesture commit - winner={:?}/{} (kind={:?}), cancelled={}",
            candidate_id.session.0,
            candidate_id.local,
            winner_kind,
            cancelled.len()
        );

        // Mark as committed in registry
        registry.mark_committed(pointer_id, candidate_id);

        Ok(GestureCommitResult {
            winner: candidate_id,
            winner_widget,
            cancelled,
        })
    }

    /// SPEC-HAL-128 Stage 7 INV-S7-P4: Resolve multiple simultaneous commit requests.
    ///
    /// Given a list of candidates requesting commit and the pointer type, returns
    /// the highest priority one using deterministic arbitration order:
    /// 1. Primary gesture for pointer type wins
    /// 2. Then secondary gestures by stable kind order
    ///
    /// INV-S7-P4: Same input → same committed gesture, ALWAYS.
    ///
    /// Returns None if the list is empty.
    pub fn resolve_priority<'a>(
        &self,
        candidates: &'a [&GestureCandidate],
        pointer_type: PointerType,
    ) -> Option<&'a GestureCandidate> {
        candidates
            .iter()
            .copied()
            .min_by_key(|c| Self::kind_priority_for_pointer(pointer_type, c.kind))
    }
}

impl Default for GestureArbiter {
    fn default() -> Self {
        Self::new()
    }
}

impl GestureRegistry {
    /// SPEC-HAL-128 Stage 3: Check if a gesture has committed for this pointer.
    pub fn committed_gesture(&self, pointer_id: PointerId) -> Option<GestureCandidateId> {
        self.sessions.get(&pointer_id).and_then(|s| s.committed)
    }

    /// Return the committed winner with its immutable declaration metadata.
    pub fn committed_candidate(&self, pointer_id: PointerId) -> Option<&GestureCandidate> {
        let candidate_id = self.committed_gesture(pointer_id)?;
        self.get_candidate(pointer_id, candidate_id)
    }

    /// SPEC-HAL-128 Stage 3: Check if pointer has an active committed gesture.
    pub fn is_committed(&self, pointer_id: PointerId) -> bool {
        self.sessions
            .get(&pointer_id)
            .map(|s| s.committed.is_some())
            .unwrap_or(false)
    }

    /// SPEC-HAL-128 Stage 3: Mark a candidate as committed.
    ///
    /// Called by GestureArbiter after successful commit.
    /// INV-GA3: Only one gesture may commit per session.
    fn mark_committed(&mut self, pointer_id: PointerId, candidate_id: GestureCandidateId) {
        if let Some(session) = self.sessions.get_mut(&pointer_id) {
            debug_assert!(
                session.committed.is_none(),
                "GestureRegistry: mark_committed called but session already has committed gesture"
            );
            session.committed = Some(candidate_id);
        }
    }

    /// SPEC-HAL-128 Stage 3: Get a candidate by ID.
    pub fn get_candidate(
        &self,
        pointer_id: PointerId,
        candidate_id: GestureCandidateId,
    ) -> Option<&GestureCandidate> {
        self.sessions
            .get(&pointer_id)
            .and_then(|s| s.candidates.iter().find(|c| c.candidate_id == candidate_id))
    }

    /// SPEC-HAL-128 Stage 3: Get candidate by widget ID (for commit request lookup).
    pub fn get_candidate_for_widget(
        &self,
        pointer_id: PointerId,
        widget_id: TreeWidgetId,
    ) -> Option<&GestureCandidate> {
        self.sessions
            .get(&pointer_id)
            .and_then(|s| s.candidates.iter().find(|c| c.widget_id == widget_id))
    }

    /// SPEC-HAL-128 Stage 4: Check if a session exists for this pointer.
    ///
    /// Used to verify INV-PC4 (Escape cancels everything) post-conditions.
    pub fn has_session(&self, pointer_id: PointerId) -> bool {
        self.sessions.contains_key(&pointer_id)
    }

    // ========================================================================
    // SPEC-HAL-128 Stage 5: Widget Lifecycle Safety
    // ========================================================================

    pub(crate) fn retire_dead_geometry_owner(&mut self, widget_id: TreeWidgetId) {
        let retired: Vec<_> = self
            .sessions
            .iter()
            .filter_map(|(&pointer, session)| {
                (session.geometry_watches.contains_key(&widget_id)
                    && session
                        .candidates
                        .iter()
                        .all(|candidate| candidate.widget_id == widget_id))
                .then_some(pointer)
            })
            .collect();
        for pointer in retired {
            self.clear_candidates(pointer);
        }
    }

    /// SPEC-HAL-128 Stage 5 INV-WL1: Remove all candidates for a specific widget.
    ///
    /// Called when a widget is destroyed while holding gesture candidates.
    /// Returns true if any candidates were removed.
    ///
    /// ORDERING: We must clear committed and pending_commits BEFORE removing
    /// candidates, because those lookups need the candidate list intact.
    pub fn remove_candidates_for_widget(&mut self, widget_id: TreeWidgetId) -> bool {
        let mut any_removed = false;
        for session in self.sessions.values_mut() {
            if session.touch_tap.as_ref().is_some_and(|tap| tap.mount.owner() == widget_id) {
                session.touch_tap = None;
                any_removed = true;
            }
            if session.touch_drag.as_ref().is_some_and(|(_, mount, _)| mount.owner() == widget_id) {
                session.touch_drag = None;
            }
            // Step 1: Collect candidate IDs for this widget BEFORE removal
            let widget_candidate_ids: Vec<GestureCandidateId> = session
                .candidates
                .iter()
                .filter(|c| c.widget_id == widget_id)
                .map(|c| c.candidate_id)
                .collect();

            if widget_candidate_ids.is_empty() {
                continue;
            }

            any_removed = true;

            // Step 2: Clear committed if it belonged to this widget
            if let Some(committed_id) = session.committed {
                if widget_candidate_ids.contains(&committed_id) {
                    session.committed = None;
                    tracing::debug!(
                        "SPEC-HAL-128 Stage 5: Cleared committed gesture for destroyed widget {:?}",
                        widget_id.0
                    );
                }
            }

            // Step 3: Clear pending commits for this widget
            let pending_before = session.pending_commits.len();
            session
                .pending_commits
                .retain(|id| !widget_candidate_ids.contains(id));
            if session.pending_commits.len() < pending_before {
                tracing::debug!(
                    "SPEC-HAL-128 Stage 5: Cleared {} pending commits for destroyed widget {:?}",
                    pending_before - session.pending_commits.len(),
                    widget_id.0
                );
            }

            // Step 4: NOW remove the candidates
            let before = session.candidates.len();
            session.candidates.retain(|c| c.widget_id != widget_id);
            session
                .touch_scroll_mounts
                .retain(|(_, mount)| mount.owner() != widget_id);
            tracing::debug!(
                "SPEC-HAL-128 Stage 5: Removed {} candidates for destroyed widget {:?}",
                before - session.candidates.len(),
                widget_id.0
            );
        }
        any_removed
    }

    /// SPEC-HAL-128 Stage 5 INV-WL2: Check if any session has candidates for a widget.
    ///
    /// Used to verify no dangling widget IDs after destruction.
    pub fn has_candidate_for_widget(&self, widget_id: TreeWidgetId) -> bool {
        self.sessions
            .values()
            .any(|s| s.candidates.iter().any(|c| c.widget_id == widget_id))
    }

    /// SPEC-HAL-128 Stage 5: Get all pointers with sessions for a specific widget.
    ///
    /// Returns PointerIds that have gesture candidates or committed gestures for the widget.
    pub fn pointers_for_widget(&self, widget_id: TreeWidgetId) -> Vec<PointerId> {
        self.sessions
            .iter()
            .filter_map(|(pid, session)| {
                if session.candidates.iter().any(|c| c.widget_id == widget_id) {
                    Some(*pid)
                } else {
                    None
                }
            })
            .collect()
    }

    /// SPEC-HAL-128 Stage 6: Get all currently active pointer IDs with sessions.
    ///
    /// Used by `cancel_all_pointer_sessions` and other multi-pointer operations
    /// to iterate over all active pointers without hardcoding PointerId::MOUSE.
    pub fn active_pointers(&self) -> Vec<PointerId> {
        self.sessions.keys().copied().collect()
    }

    /// SPEC-HAL-151: Clear all gesture sessions unconditionally.
    ///
    /// Used for domain teardown on focus loss.
    /// INV-151-FOCUS-TEARDOWN: All gesture sessions must be cleared when window loses focus.
    pub fn clear_all(&mut self) {
        let count = self.sessions.len();
        self.sessions.clear();
        if count > 0 {
            tracing::debug!(
                "SPEC-HAL-151: Cleared all {} gesture sessions on focus loss",
                count
            );
        }
    }
}

// ============================================================================
// SPEC-HAL-009: Double-Click Detection
// ============================================================================

use web_time::Instant;

/// Click tracker for double-click detection
pub struct ClickTracker {
    last_click_time: Option<Instant>,
    last_click_pos: Option<(f32, f32)>,
    last_button: Option<PointerButton>,
    threshold_time_ms: u128,
    threshold_distance: f32,
}

impl ClickTracker {
    pub fn new() -> Self {
        Self {
            last_click_time: None,
            last_click_pos: None,
            last_button: None,
            threshold_time_ms: 250,
            threshold_distance: 4.0,
        }
    }

    /// Check if a Down event should be classified as DoubleClick
    pub fn check_double_click(&mut self, button: PointerButton, pos: (f32, f32)) -> bool {
        let now = Instant::now();

        if let (Some(last_time), Some(last_pos), Some(last_btn)) =
            (self.last_click_time, self.last_click_pos, self.last_button)
        {
            if last_btn == button {
                let time_diff = now.duration_since(last_time).as_millis();
                let dx = pos.0 - last_pos.0;
                let dy = pos.1 - last_pos.1;
                let distance = (dx * dx + dy * dy).sqrt();

                if time_diff < self.threshold_time_ms && distance < self.threshold_distance {
                    // Reset tracker after double-click
                    self.last_click_time = None;
                    self.last_click_pos = None;
                    self.last_button = None;
                    return true;
                }
            }
        }

        // Record this click
        self.last_click_time = Some(now);
        self.last_click_pos = Some(pos);
        self.last_button = Some(button);
        false
    }
}

impl Default for ClickTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// SPEC-HAL-033: Raw Input Types
// ============================================================================

/// Device type for pointer input
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PointerDevice {
    Mouse,
    Touchpad,
}

impl Default for PointerDevice {
    fn default() -> Self {
        Self::Mouse
    }
}

/// Button state for raw input events (pressed vs released)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RawButtonState {
    Pressed,
    Released,
}

/// Key state for raw input events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyState {
    Down,
    Up,
}

/// Wheel delta for scroll events
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WheelDelta {
    Lines(f32, f32),
    Pixels(f32, f32),
}

/// Unique identifier for registered shortcuts
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ShortcutId(pub u32);

/// Scope for shortcut matching
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutScope {
    Global,
    FocusedWidget,
    WidgetTree(TreeWidgetId),
}

/// Native contact lifetime transported without desktop click interpretation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TouchPhase {
    Started,
    Moved,
    Ended,
    Cancelled,
}

/// Raw input event from platform layer (pre-fusion)
///
/// SPEC-HAL-151: All variants carry window_id to identify the originating window.
/// INV-151-WINDOW-ORIGIN: Every input event MUST carry its originating WindowId.
#[derive(Debug, Clone)]
pub enum RawInputEvent {
    Touch {
        window_id: WindowId,
        time: Instant,
        contact_id: u64,
        phase: TouchPhase,
        pos: Point,
    },
    RelativePointerMotion {
        lease_id: RelativePointerLeaseId,
        normalization_epoch: RelativePointerNormalizationEpoch,
        window_id: WindowId,
        pointer_id: PointerId,
        time: Instant,
        delta: (f32, f32),
    },
    PointerMoved {
        window_id: WindowId,
        time: Instant,
        device: PointerDevice,
        position: Point,
        delta: (f32, f32),
    },
    PointerButton {
        window_id: WindowId,
        time: Instant,
        device: PointerDevice,
        button: PointerButton,
        state: RawButtonState,
        position: Point,
    },
    PointerWheel {
        window_id: WindowId,
        time: Instant,
        device: PointerDevice,
        delta: WheelDelta,
        position: Point,
    },
    Key {
        window_id: WindowId,
        time: Instant,
        key_code: KeyCode,
        physical_key: PhysicalKeyCode,
        reservation: Option<KeyboardKeyReservation>,
        state: KeyState,
        repeat: bool,
        modifiers: Modifiers,
    },
    TextInput {
        window_id: WindowId,
        time: Instant,
        text: String,
        modifiers: Modifiers,
    },
    FocusChanged {
        window_id: WindowId,
        time: Instant,
        focused: bool,
    },
    /// SPEC-HAL-044: Window resize event for constraint updates
    Resize {
        window_id: WindowId,
        width: f32,
        height: f32,
    },
    /// SPEC-HAL-266: Scale factor changed (DPR update from OS).
    ScaleFactorChanged {
        window_id: WindowId,
        scale_factor: f32,
    },
}

impl RawInputEvent {
    /// SPEC-HAL-151: Get the window ID for this event.
    pub fn window_id(&self) -> WindowId {
        match self {
            RawInputEvent::Touch { window_id, .. } => *window_id,
            RawInputEvent::RelativePointerMotion { window_id, .. } => *window_id,
            RawInputEvent::PointerMoved { window_id, .. } => *window_id,
            RawInputEvent::PointerButton { window_id, .. } => *window_id,
            RawInputEvent::PointerWheel { window_id, .. } => *window_id,
            RawInputEvent::Key { window_id, .. } => *window_id,
            RawInputEvent::TextInput { window_id, .. } => *window_id,
            RawInputEvent::FocusChanged { window_id, .. } => *window_id,
            RawInputEvent::Resize { window_id, .. } => *window_id,
            RawInputEvent::ScaleFactorChanged { window_id, .. } => *window_id,
        }
    }
}

// ============================================================================
// SPEC-HAL-033: Fused Event Types
// ============================================================================

/// Source of scroll events
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollSource {
    Wheel,
    Touchpad,
}

/// Current state of pointer buttons
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PointerButtonsState {
    pub primary: bool,
    pub secondary: bool,
    pub auxiliary: bool,
}

impl PointerButtonsState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&mut self, button: PointerButton, pressed: bool) {
        match button {
            PointerButton::Primary => self.primary = pressed,
            PointerButton::Secondary => self.secondary = pressed,
            PointerButton::Auxiliary => self.auxiliary = pressed,
        }
    }

    pub fn is_pressed(&self, button: PointerButton) -> bool {
        match button {
            PointerButton::Primary => self.primary,
            PointerButton::Secondary => self.secondary,
            PointerButton::Auxiliary => self.auxiliary,
        }
    }

    pub fn any_pressed(&self) -> bool {
        self.primary || self.secondary || self.auxiliary
    }
}

/// Fused input events (post-fusion, ready for routing)
///
/// SPEC-HAL-151: All variants carry window_id to identify the originating window.
/// INV-151-WINDOW-ORIGIN: Every fused event MUST carry its originating WindowId.
#[derive(Debug, Clone)]
pub enum FusedInputEvent {
    TouchDown {
        window_id: WindowId,
        time: Instant,
        pointer_id: PointerId,
        pos: Point,
        button: PointerButton,
        click_count: u8,
    },
    TouchMove {
        window_id: WindowId,
        time: Instant,
        pointer_id: PointerId,
        pos: Point,
        delta: (f32, f32),
        buttons: PointerButtonsState,
    },
    TouchUp {
        window_id: WindowId,
        time: Instant,
        pointer_id: PointerId,
        pos: Point,
        button: PointerButton,
    },
    TouchCancel {
        window_id: WindowId,
        time: Instant,
        pointer_id: PointerId,
        pos: Point,
    },
    // Pointer events
    RelativePointerMotion {
        lease_id: RelativePointerLeaseId,
        window_id: WindowId,
        pointer_id: PointerId,
        time: Instant,
        delta: RelativeLogicalDelta,
    },
    PointerMove {
        window_id: WindowId,
        time: Instant,
        pos: Point,
        delta: (f32, f32),
        device: PointerDevice,
        buttons: PointerButtonsState,
    },
    PointerDown {
        window_id: WindowId,
        time: Instant,
        pos: Point,
        device: PointerDevice,
        button: PointerButton,
        click_count: u8, // 1=single, 2=double, 3=triple
    },
    PointerUp {
        window_id: WindowId,
        time: Instant,
        pos: Point,
        device: PointerDevice,
        button: PointerButton,
    },
    DragStart {
        window_id: WindowId,
        time: Instant,
        origin: Point,
        button: PointerButton,
    },
    DragUpdate {
        window_id: WindowId,
        time: Instant,
        origin: Point,
        current: Point,
        delta: (f32, f32),
        button: PointerButton,
    },
    DragEnd {
        window_id: WindowId,
        time: Instant,
        origin: Point,
        current: Point,
        button: PointerButton,
        cancelled: bool,
    },
    Scroll {
        window_id: WindowId,
        time: Instant,
        pos: Point,
        delta: (f32, f32), // logical pixels, normalized
        source: ScrollSource,
    },

    // Keyboard / shortcuts
    KeyDown {
        window_id: WindowId,
        time: Instant,
        key_code: KeyCode,
        physical_key: PhysicalKeyCode,
        reservation: Option<KeyboardKeyReservation>,
        modifiers: Modifiers,
        repeat: bool,
    },
    KeyUp {
        window_id: WindowId,
        time: Instant,
        key_code: KeyCode,
        physical_key: PhysicalKeyCode,
        reservation: Option<KeyboardKeyReservation>,
        modifiers: Modifiers,
    },
    TextInput {
        window_id: WindowId,
        time: Instant,
        text: String,
        modifiers: Modifiers,
    },
    Shortcut {
        window_id: WindowId,
        time: Instant,
        id: ShortcutId,
        scope: ShortcutScope,
    },

    // Focus + synthetic
    FocusChanged {
        window_id: WindowId,
        time: Instant,
        focused: bool,
    },
    Tick {
        window_id: WindowId,
        time: Instant,
        dt: std::time::Duration,
    },

    /// SPEC-HAL-044: Window resize event for viewport updates
    /// INV-RESIZE-SIGNAL-1: Every resize must be delivered to active Screen
    Resize {
        window_id: WindowId,
        width: f32,
        height: f32,
    },
}

impl FusedInputEvent {
    /// SPEC-HAL-151: Get the window ID for this event.
    pub fn window_id(&self) -> WindowId {
        match self {
            FusedInputEvent::TouchDown { window_id, .. }
            | FusedInputEvent::TouchMove { window_id, .. }
            | FusedInputEvent::TouchUp { window_id, .. }
            | FusedInputEvent::TouchCancel { window_id, .. } => *window_id,
            FusedInputEvent::RelativePointerMotion { window_id, .. } => *window_id,
            FusedInputEvent::PointerMove { window_id, .. } => *window_id,
            FusedInputEvent::PointerDown { window_id, .. } => *window_id,
            FusedInputEvent::PointerUp { window_id, .. } => *window_id,
            FusedInputEvent::DragStart { window_id, .. } => *window_id,
            FusedInputEvent::DragUpdate { window_id, .. } => *window_id,
            FusedInputEvent::DragEnd { window_id, .. } => *window_id,
            FusedInputEvent::Scroll { window_id, .. } => *window_id,
            FusedInputEvent::KeyDown { window_id, .. } => *window_id,
            FusedInputEvent::KeyUp { window_id, .. } => *window_id,
            FusedInputEvent::TextInput { window_id, .. } => *window_id,
            FusedInputEvent::Shortcut { window_id, .. } => *window_id,
            FusedInputEvent::FocusChanged { window_id, .. } => *window_id,
            FusedInputEvent::Tick { window_id, .. } => *window_id,
            FusedInputEvent::Resize { window_id, .. } => *window_id,
        }
    }
}

// ============================================================================
// Scroll Intent Pipeline
// ============================================================================

/// Scroll alignment strategy for reveal operations.
///
/// Determines how an item or rect is positioned within the viewport
/// when scrolling to reveal it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ScrollAlign {
    /// Scroll minimum amount to make item fully visible.
    /// - If item is above viewport: align top
    /// - If item is below viewport: align bottom
    /// - If already visible: no scroll
    Nearest,
    /// Align item to the top of the viewport.
    Start,
    /// Align item to the bottom of the viewport.
    End,
    /// Center item in the viewport.
    Center,
}

impl Default for ScrollAlign {
    fn default() -> Self {
        Self::Nearest
    }
}

/// Scroll intent for programmatic scroll requests.
///
/// Widgets request scroll intents; ScrollArea resolves them.
/// This decouples scroll logic from widget implementation.
///
/// INV-SCROLL-INTENT-1: Widgets never mutate scroll position directly.
/// INV-SCROLL-INTENT-2: Intents contain no pixel offsets or viewport math.
/// INV-SCROLL-INTENT-3: Resolution happens once per frame, after layout.
#[derive(Debug, Clone)]
pub enum ScrollIntent {
    /// Reveal a logical item by index.
    /// Used by List and other item-based containers.
    RevealIndex {
        logical_index: usize,
        align: ScrollAlign,
    },
    /// Reveal a rect in content coordinates.
    /// Used for cursor visibility in text fields, etc.
    RevealRect {
        rect: crate::core::math::RectF,
        align: ScrollAlign,
    },
}

// ============================================================================
// SPEC-HAL-151: Interaction Domain State & Registry
// ============================================================================

/// Provenance of the domain's scroll capture when it belongs to a touch pan.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct TouchScrollSource {
    pub press: TouchPressIdentity,
    pub axis: GestureAxis,
}

enum TouchScrollAuthority {
    Direct(TouchScrollSource),
    Kinetic { source: TouchScrollSource, owner: WidgetMountToken },
}

/// The existing domain's admission for mounted ScrollAreas. Only the router
/// can grant a pan or its successor lease. Competing admitted intents may
/// terminate a kinetic lease before they change the offset.
#[derive(Debug, Clone, Default)]
pub(crate) struct ScrollIntentAdmission(Weak<RefCell<Option<TouchScrollAuthority>>>);

impl ScrollIntentAdmission {
    pub(crate) fn blocks_competing_scroll(&self) -> bool {
        self.0.upgrade().is_some_and(|source| {
            matches!(*source.borrow(), Some(TouchScrollAuthority::Direct(_)))
        })
    }

    pub(crate) fn admit_scroll_intent(&self) -> bool {
        let Some(authority) = self.0.upgrade() else { return true; };
        let mut authority = authority.borrow_mut();
        if matches!(*authority, Some(TouchScrollAuthority::Direct(_))) {
            return false;
        }
        *authority = None;
        true
    }

    pub(crate) fn owns_kinetic_scroll(&self, press: TouchPressIdentity) -> bool {
        self.0.upgrade().is_some_and(|authority| {
            matches!(authority.borrow().as_ref(), Some(TouchScrollAuthority::Kinetic { source, owner })
                if source.press == press && owner.is_live())
        })
    }

    pub(crate) fn has_kinetic_scroll(&self) -> bool {
        self.0.upgrade().is_some_and(|authority| {
            matches!(authority.borrow().as_ref(), Some(TouchScrollAuthority::Kinetic { owner, .. })
                if owner.is_live())
        })
    }

    pub(crate) fn kinetic_scroll_mount(&self) -> Option<WidgetMountToken> {
        let authority = self.0.upgrade()?;
        let authority = authority.borrow();
        match authority.as_ref() {
            Some(TouchScrollAuthority::Kinetic { owner, .. }) if owner.is_live() => Some(owner.clone()),
            _ => None,
        }
    }

    pub(crate) fn finish_kinetic_scroll(&self, press: TouchPressIdentity) {
        if let Some(authority) = self.0.upgrade() {
            let mut authority = authority.borrow_mut();
            if matches!(authority.as_ref(), Some(TouchScrollAuthority::Kinetic { source, .. })
                if source.press == press)
            {
                *authority = None;
            }
        }
    }

    pub(crate) fn same_domain(&self, other: &Self) -> bool {
        Weak::ptr_eq(&self.0, &other.0)
    }
}

/// SPEC-HAL-151: Per-window interaction state.
///
/// Contains all state that was previously global in EventRouter.
/// Each window/domain has its own isolated instance of this state.
///
/// INV-151-DOMAIN-ISOLATION: State does not leak between domains.
/// INV-151-SINGLE-DOMAIN-PER-EVENT: Each event routes to exactly one domain.
pub struct InteractionDomainState {
    ui_interaction_policy: UiInteractionPolicy,
    /// Focus manager for this domain
    pub focus_manager: FocusManager,
    /// Legacy pointer capture (retained for compatibility)
    pub pointer_capture: PointerCapture,
    /// SPEC-HAL-128: Per-pointer capture manager
    pub pointer_capture_manager: PointerCaptureManager,
    /// SPEC-HAL-128 Stage 2: Gesture candidate registry
    pub gesture_registry: GestureRegistry,
    pub(crate) relative_pointer_authority:
        Option<Box<dyn crate::core::relative_pointer_authority::RelativePointerLeaseControl>>,
    /// Currently hovered widget in this domain
    pub hovered_widget: Option<TreeWidgetId>,
    /// SPEC-HAL-241: Drag and drop state for this domain
    pub drag_drop: DragDropState,
    /// SPEC-HAL-038: Scroll capture owner (distinct from pointer capture).
    /// An absent touch source denotes the existing wheel routing authority.
    pub scroll_capture_owner: Option<TreeWidgetId>,
    scroll_touch_source: Rc<RefCell<Option<TouchScrollAuthority>>>,
    /// SPEC-HAL-176: Current keyboard authority mode for this domain.
    pub keyboard_mode: KeyboardAuthorityMode,
    /// SPEC-HAL-176: Timestamp of the most recent `KeyDown` consumed by
    /// `dispatch_engine_action`. The platform layer timestamps each paired
    /// `Key` + `TextInput` emission with the same `Instant`; matching on
    /// `time` lets the engine swallow the paired `TextInput` from a press
    /// whose Key half was a binding/mode-switch, preventing characters from
    /// leaking into focused input controls. Cleared by any subsequent
    /// `TextInput` (matched or not) and overwritten by the next consumed
    /// `KeyDown`. `None` outside the short window between a consumed
    /// `KeyDown` and its paired `TextInput`.
    pub swallow_text_at: Option<web_time::Instant>,
}

impl InteractionDomainState {
    /// Create a new interaction domain state with default values.
    pub fn new() -> Self {
        Self::with_ui_interaction_policy(UiInteractionPolicy::default())
    }

    pub fn with_ui_interaction_policy(policy: UiInteractionPolicy) -> Self {
        Self::new_for_domain(InteractionDomainId::PRIMARY, policy)
    }

    pub fn ui_interaction_policy(&self) -> UiInteractionPolicy {
        self.ui_interaction_policy
    }

    pub(crate) fn scroll_intent_admission(&self) -> ScrollIntentAdmission {
        ScrollIntentAdmission(Rc::downgrade(&self.scroll_touch_source))
    }

    pub(crate) fn touch_scroll_source(&self) -> Option<TouchScrollSource> {
        if let Some(TouchScrollAuthority::Direct(source)) = *self.scroll_touch_source.borrow() {
            Some(source)
        } else {
            None
        }
    }

    pub(crate) fn begin_touch_scroll(&mut self, owner: TreeWidgetId, source: TouchScrollSource) {
        debug_assert!(self.scroll_touch_source.borrow().is_none());
        self.scroll_capture_owner = Some(owner);
        self.scroll_touch_source.replace(Some(TouchScrollAuthority::Direct(source)));
    }

    pub(crate) fn admit_kinetic_scroll(&mut self, source: TouchScrollSource, owner: WidgetMountToken) {
        if self.touch_scroll_source() != Some(source) || self.scroll_capture_owner != Some(owner.owner())
            || !owner.is_live() || self.relative_pointer_authority.is_some()
            || self.pointer_capture_manager.active_pointers().iter()
                .any(|pointer| *pointer != source.press.pointer)
        {
            return;
        }
        self.scroll_capture_owner = None;
        self.scroll_touch_source.replace(Some(TouchScrollAuthority::Kinetic { source, owner }));
    }

    pub(crate) fn kinetic_scroll_mount(&self) -> Option<WidgetMountToken> {
        match self.scroll_touch_source.borrow().as_ref() {
            Some(TouchScrollAuthority::Kinetic { owner, .. }) => Some(owner.clone()),
            _ => None,
        }
    }

    pub(crate) fn interrupt_kinetic_scroll(&mut self) {
        let mut authority = self.scroll_touch_source.borrow_mut();
        if matches!(*authority, Some(TouchScrollAuthority::Kinetic { .. })) {
            *authority = None;
        }
    }

    pub(crate) fn end_touch_scroll(&mut self, pointer: PointerId) {
        if self
            .touch_scroll_source()
            .is_some_and(|source| source.press.pointer == pointer)
        {
            self.clear_scroll_capture();
        }
    }

    pub(crate) fn clear_wheel_scroll_capture(&mut self) {
        if self.touch_scroll_source().is_none() {
            self.scroll_capture_owner = None;
        }
    }

    pub(crate) fn clear_scroll_capture(&mut self) {
        self.scroll_capture_owner = None;
        self.scroll_touch_source.replace(None);
    }

    pub(crate) fn new_for_domain(
        interaction_domain_id: InteractionDomainId,
        ui_interaction_policy: UiInteractionPolicy,
    ) -> Self {
        Self {
            ui_interaction_policy,
            focus_manager: FocusManager::new_for_domain(interaction_domain_id),
            pointer_capture: PointerCapture::new(),
            pointer_capture_manager: PointerCaptureManager::new(),
            gesture_registry: GestureRegistry::new(),
            relative_pointer_authority: None,
            hovered_widget: None,
            drag_drop: DragDropState::new(),
            scroll_capture_owner: None,
            scroll_touch_source: Rc::new(RefCell::new(None)),
            keyboard_mode: KeyboardAuthorityMode::default(),
            swallow_text_at: None,
        }
    }
}

impl Default for InteractionDomainState {
    fn default() -> Self {
        Self::new()
    }
}

/// SPEC-HAL-151: Registry of interaction domains.
///
/// Maps windows to their interaction domains and owns domain state.
/// Provides the lookup required before routing any event.
///
/// INV-151-DOMAIN-RESOLVE: Every event MUST resolve to a domain before routing.
/// INV-151-NO-DEFAULT-DOMAIN: Events without a valid domain are dropped.
pub struct InteractionDomains {
    ui_interaction_policy: UiInteractionPolicy,
    /// Maps WindowId to its primary interaction domain
    primary_by_window: HashMap<WindowId, InteractionDomainId>,
    /// Owns the actual domain state
    state_by_domain: HashMap<InteractionDomainId, InteractionDomainState>,
    /// Currently active window (receives input)
    active_window: Option<WindowId>,
    /// Next domain ID for allocation
    next_domain_id: u64,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct PreparedInteractionDomainUnregistration {
    interaction_domain_id: InteractionDomainId,
    window_id: WindowId,
}

impl InteractionDomains {
    /// Create a new registry with primary window/domain pre-registered.
    pub fn new() -> Self {
        Self::with_ui_interaction_policy(UiInteractionPolicy::default())
    }

    pub fn with_ui_interaction_policy(ui_interaction_policy: UiInteractionPolicy) -> Self {
        let mut domains = Self {
            ui_interaction_policy,
            primary_by_window: HashMap::new(),
            state_by_domain: HashMap::new(),
            active_window: Some(WindowId::PRIMARY),
            next_domain_id: 1, // 0 is reserved for PRIMARY
        };

        // Pre-register the primary window and domain
        domains
            .primary_by_window
            .insert(WindowId::PRIMARY, InteractionDomainId::PRIMARY);
        domains.state_by_domain.insert(
            InteractionDomainId::PRIMARY,
            InteractionDomainState::new_for_domain(
                InteractionDomainId::PRIMARY,
                ui_interaction_policy,
            ),
        );

        domains
    }

    /// Get the domain ID for a window.
    pub fn domain_for_window(&self, window_id: WindowId) -> Option<InteractionDomainId> {
        self.primary_by_window.get(&window_id).copied()
    }

    /// Get mutable reference to domain state.
    ///
    /// INV-151-DOMAIN-RESOLVE: Returns None if domain doesn't exist.
    pub fn state_mut(
        &mut self,
        domain_id: InteractionDomainId,
    ) -> Option<&mut InteractionDomainState> {
        self.state_by_domain.get_mut(&domain_id)
    }

    /// Get immutable reference to domain state.
    pub fn state(&self, domain_id: InteractionDomainId) -> Option<&InteractionDomainState> {
        self.state_by_domain.get(&domain_id)
    }

    /// Resolve window to domain state (convenience method).
    ///
    /// Returns None if window has no registered domain.
    pub fn resolve_window_mut(
        &mut self,
        window_id: WindowId,
    ) -> Option<&mut InteractionDomainState> {
        let domain_id = self.primary_by_window.get(&window_id).copied()?;
        self.state_by_domain.get_mut(&domain_id)
    }

    /// Get the currently active window.
    pub fn active_window(&self) -> Option<WindowId> {
        self.active_window
    }

    /// Set the active window.
    ///
    /// Called when window focus changes. Only the active window receives
    /// pointer/keyboard input. FocusChanged events update this.
    pub fn set_active_window(&mut self, window_id: Option<WindowId>) {
        self.active_window = window_id;
    }

    /// Check if a window is the currently active one.
    pub fn is_active_window(&self, window_id: WindowId) -> bool {
        self.active_window == Some(window_id)
    }

    /// Register a new window with a new domain.
    ///
    /// Returns the assigned domain ID.
    pub fn register_window(&mut self, window_id: WindowId) -> InteractionDomainId {
        let domain_id = InteractionDomainId(self.next_domain_id);
        self.next_domain_id += 1;

        self.primary_by_window.insert(window_id, domain_id);
        self.state_by_domain.insert(
            domain_id,
            InteractionDomainState::new_for_domain(domain_id, self.ui_interaction_policy),
        );

        domain_id
    }

    pub(crate) fn prepare_domain_unregistration(
        &self,
        interaction_domain_id: InteractionDomainId,
    ) -> Option<PreparedInteractionDomainUnregistration> {
        self.state_by_domain.get(&interaction_domain_id)?;
        let mut matching_windows =
            self.primary_by_window
                .iter()
                .filter_map(|(window_id, domain_id)| {
                    (*domain_id == interaction_domain_id).then_some(*window_id)
                });
        let window_id = matching_windows.next()?;
        if matching_windows.next().is_some() {
            return None;
        }
        Some(PreparedInteractionDomainUnregistration {
            interaction_domain_id,
            window_id,
        })
    }

    pub(crate) fn commit_domain_unregistration(
        &mut self,
        prepared: PreparedInteractionDomainUnregistration,
    ) {
        assert_eq!(
            self.primary_by_window.get(&prepared.window_id),
            Some(&prepared.interaction_domain_id),
            "interaction-domain window mapping changed after unregistration preparation"
        );
        assert!(
            self.state_by_domain
                .contains_key(&prepared.interaction_domain_id),
            "interaction-domain state changed after unregistration preparation"
        );
        self.primary_by_window.remove(&prepared.window_id);
        self.state_by_domain.remove(&prepared.interaction_domain_id);
        if self.active_window == Some(prepared.window_id) {
            self.active_window = None;
        }
    }

    /// Get the primary domain state (convenience for single-window mode).
    pub fn primary_state_mut(&mut self) -> &mut InteractionDomainState {
        self.state_by_domain
            .get_mut(&InteractionDomainId::PRIMARY)
            .expect("Primary domain must always exist")
    }

    /// Get the primary domain state immutably.
    pub fn primary_state(&self) -> &InteractionDomainState {
        self.state_by_domain
            .get(&InteractionDomainId::PRIMARY)
            .expect("Primary domain must always exist")
    }

    /// SPEC-HAL-151: Check if an event should be routed based on active window.
    ///
    /// Returns true if the event should be routed, false if it should be dropped.
    /// Events from non-active windows are dropped, EXCEPT:
    /// - FocusChanged (needed to switch active window)
    /// - Resize (window size changes regardless of focus)
    /// - Tick (timer events are global)
    ///
    /// INV-151-ACTIVE-WINDOW-GATE: Only active window receives input events.
    pub fn should_route_event(&self, event: &FusedInputEvent) -> bool {
        // Always route these event types regardless of active window
        match event {
            FusedInputEvent::FocusChanged { .. }
            | FusedInputEvent::Resize { .. }
            | FusedInputEvent::Tick { .. } => return true,
            _ => {}
        }

        // For all other events, check active window
        let event_window_id = event.window_id();
        match self.active_window {
            Some(active) => active == event_window_id,
            None => false, // No active window, drop all non-exempt events
        }
    }

    /// SPEC-HAL-151: Handle FocusChanged event and perform domain teardown if needed.
    ///
    /// On focus gain: sets active_window to the event's window.
    /// On focus loss for the active window: clears active_window and tears down domain state.
    ///
    /// Returns the domain state for the event's window if it should be routed,
    /// or None if the domain doesn't exist (event should be dropped).
    ///
    /// INV-151-FOCUS-TEARDOWN: Focus loss cancels all pointer sessions and clears state.
    pub fn handle_focus_changed(
        &mut self,
        window_id: WindowId,
        focused: bool,
    ) -> Option<&mut InteractionDomainState> {
        if focused {
            // Window gained focus - make it active
            self.active_window = Some(window_id);
            tracing::debug!(
                "SPEC-HAL-151: Window {:?} gained focus, now active",
                window_id.0
            );
        } else {
            // Window lost focus
            if self.active_window == Some(window_id) {
                // This was the active window - clear active and teardown
                self.active_window = None;
                tracing::debug!(
                    "SPEC-HAL-151: Window {:?} lost focus, clearing active",
                    window_id.0
                );

                // Perform teardown on the domain state
                if let Some(domain_state) = self.resolve_window_mut(window_id) {
                    // INV-151-FOCUS-TEARDOWN: Clear all interaction state
                    domain_state.pointer_capture_manager.cancel_all_captures();
                    domain_state.pointer_capture.release();
                    domain_state.gesture_registry.clear_all();
                    domain_state.hovered_widget = None;
                    domain_state.clear_scroll_capture();
                    domain_state.focus_manager.clear_focus();
                    tracing::debug!(
                        "SPEC-HAL-151: Teardown complete for window {:?}",
                        window_id.0
                    );
                }
            }
        }

        // Return domain state for routing
        self.resolve_window_mut(window_id)
    }
}

impl Default for InteractionDomains {
    fn default() -> Self {
        Self::new()
    }
}
