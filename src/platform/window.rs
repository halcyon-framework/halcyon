use crate::core::event::{
    KeyCode as HalcyonKeyCode, KeyState, KeyboardKeyReservation, Modifiers, PhysicalKeyCode,
    PointerButton, PointerDevice, RawButtonState, RawInputEvent, TouchPhase, WheelDelta,
    WindowEvent, WindowId,
};
use crate::core::math::Point;
use crate::core::signal_bus::SignalEndpoint;
use crate::utils::coords::physical_xy_to_logical_point;
use web_time::Instant;
use winit::event::{ElementState, MouseButton, WindowEvent as WinitWindowEvent};
use winit::event_loop::{ControlFlow, EventLoop};
use winit::keyboard::{KeyCode as WinitKeyCode, PhysicalKey};
use winit::window::{Fullscreen, Window as WinitWindow, WindowAttributes};

/// Private ordinary-App transport; browser Paste retains its trusted source type.
pub(crate) enum AppLoopEvent {
    Native(winit::event::Event<()>),
    #[cfg(target_arch = "wasm32")]
    BrowserNative {
        event: winit::event::Event<()>,
        text: crate::platform::web::BrowserTextBridge,
    },
    #[cfg(target_arch = "wasm32")]
    BrowserText {
        text: crate::platform::web::BrowserTextBridge,
        event: crate::platform::web::BrowserTextReport,
    },
    #[cfg(target_arch = "wasm32")]
    BrowserChoice(crate::core::event_router::host_choice::ChoiceReply),
    #[cfg(target_arch = "wasm32")]
    BrowserChoiceGeometry(winit::window::WindowId),
    #[cfg(target_arch = "wasm32")]
    ClipboardWrite {
        window: winit::window::WindowId,
        text: crate::platform::web::BrowserTextBridge,
        from_proxy: bool,
        event: web_sys::ClipboardEvent,
    },
    #[cfg(target_arch = "wasm32")]
    Paste { window: winit::window::WindowId, event: web_sys::ClipboardEvent },
}

/// Application window — OS transport binding only.
///
/// # MWA Authority Boundary (SPEC-HAL-082A, BINDING)
///
/// This struct is **transport-only**. It provides:
/// - OS window handle (winit_window)
/// - Scale factor for coordinate conversion
/// - Cursor position tracking for event conversion
/// - Optional signal endpoint for cross-window communication (SPEC-HAL-161)
///
/// This struct **MUST NOT** own, control, or reference:
/// - Compositor (INV-MW-006)
/// - Renderer (INV-MW-006)
/// - WidgetTree (INV-MW-006)
/// - Layout roots (INV-MW-006)
/// - Focus or capture state (INV-MW-006)
/// - EventRouter (INV-MW-006)
///
/// Any field added here that grants semantic authority violates SPEC-HAL-082A
/// and the MWA closure. Such changes require explicit SPEC amendment.
///
/// # SPEC-HAL-161: SignalEndpoint
///
/// The `signal_endpoint` field is a **pure data handle** for addressing.
/// It does NOT grant authority (INV-XWIN-001). It is used only for:
/// - Identifying this window as a signal target
/// - Private channel subscription at window creation
///
/// See `docs/MWA-AUDIT-REPORT.md` for the binding audit.
pub struct Window {
    pub winit_window: WinitWindow,
    pub scale_factor: f32,
    last_cursor_pos: (f32, f32),
    /// SPEC-HAL-161: Optional signal endpoint for cross-window communication.
    /// Pure data handle; does NOT grant authority (INV-XWIN-001).
    signal_endpoint: Option<SignalEndpoint>,
}

impl Window {
    /// Create a windowed window without a signal endpoint.
    pub fn new(event_loop: &EventLoop<()>, title: &str, width: u32, height: u32) -> Self {
        Self::with_endpoint(event_loop, title, width, height, None)
    }

    /// SPEC-HAL-161: Create a new window with an optional signal endpoint.
    ///
    /// # Arguments
    ///
    /// * `endpoint` - Optional signal endpoint for cross-window communication.
    ///   If provided, this window can receive signals addressed to this endpoint.
    ///   The endpoint is a pure data handle (INV-XWIN-001: no authority).
    pub fn with_endpoint(
        event_loop: &EventLoop<()>,
        title: &str,
        width: u32,
        height: u32,
        endpoint: Option<SignalEndpoint>,
    ) -> Self {
        Self::with_endpoint_and_fullscreen(
            event_loop, title, width, height, endpoint, false, true,
            #[cfg(target_arch = "wasm32")]
            None,
            #[cfg(target_arch = "wasm32")]
            None,
        )
    }

    pub(crate) fn with_endpoint_and_fullscreen(
        event_loop: &EventLoop<()>,
        title: &str,
        width: u32,
        height: u32,
        endpoint: Option<SignalEndpoint>,
        fullscreen: bool,
        visible: bool,
        #[cfg(target_arch = "wasm32")] input_overlay: Option<web_sys::HtmlElement>,
        #[cfg(target_arch = "wasm32")] context_menu_enabled: Option<bool>,
    ) -> Self {
        let window_attributes =
            Self::creation_attributes(title, width, height, fullscreen, visible);
        #[cfg(target_arch = "wasm32")]
        let window_attributes = {
            use winit::platform::web::WindowAttributesExtWebSys;
            let window_attributes = window_attributes.with_input_overlay(input_overlay);
            match context_menu_enabled {
                Some(enabled) => window_attributes.with_context_menu_enabled(enabled),
                None => window_attributes,
            }
        };

        #[allow(deprecated)]
        let winit_window = event_loop.create_window(window_attributes).unwrap();

        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowExtWebSys;

            // HAL-396 §8: reserve native contacts on this application canvas
            // before returning control to the browser's event loop.
            let canvas = winit_window.canvas().expect("Halcyon window canvas");
            let style = canvas.style();
            if !style
                .get_property_value("touch-action")
                .is_ok_and(|value| value == "none")
            {
                style
                    .set_property("touch-action", "none")
                    .expect("Halcyon canvas touch-action");
            }
        }

        Self::from_winit_window(winit_window, endpoint)
    }

    #[cfg(target_os = "macos")]
    pub(crate) fn create_hidden_on_active_loop(
        active: &winit::event_loop::ActiveEventLoop,
        title: &str,
        width: u32,
        height: u32,
        fullscreen: bool,
    ) -> Self {
        let attributes = Self::creation_attributes(title, width, height, fullscreen, false);
        let winit_window = active.create_window(attributes).unwrap();
        Self::from_winit_window(winit_window, None)
    }

    fn from_winit_window(winit_window: WinitWindow, endpoint: Option<SignalEndpoint>) -> Self {
        let scale_factor = winit_window.scale_factor() as f32;
        Self {
            winit_window,
            scale_factor,
            last_cursor_pos: (0.0, 0.0),
            signal_endpoint: endpoint,
        }
    }

    fn creation_attributes(
        title: &str,
        width: u32,
        height: u32,
        fullscreen: bool,
        visible: bool,
    ) -> WindowAttributes {
        let attributes = WinitWindow::default_attributes()
            .with_title(title)
            .with_visible(visible)
            .with_inner_size(winit::dpi::LogicalSize::new(width, height))
            .with_min_inner_size(winit::dpi::LogicalSize::new(64.0, 64.0))
            .with_fullscreen(fullscreen.then_some(Fullscreen::Borderless(None)));
        #[cfg(target_arch = "wasm32")]
        {
            use winit::platform::web::WindowAttributesExtWebSys;
            attributes.with_append(true)
        }
        #[cfg(not(target_arch = "wasm32"))]
        attributes
    }

    /// SPEC-HAL-161: Get this window's signal endpoint, if any.
    pub fn signal_endpoint(&self) -> Option<SignalEndpoint> {
        self.signal_endpoint
    }

    pub fn request_redraw(&self) {
        self.winit_window.request_redraw();
    }

    pub fn inner_size(&self) -> (f32, f32) {
        let size = self.winit_window.inner_size(); // PhysicalSize
        (
            size.width as f32 / self.scale_factor,
            size.height as f32 / self.scale_factor,
        )
    }

    pub fn update_cursor_pos(&mut self, x: f32, y: f32) {
        self.last_cursor_pos = (x, y);
    }

    pub fn last_cursor_pos(&self) -> (f32, f32) {
        self.last_cursor_pos
    }

    pub fn update_scale_factor(&mut self, scale_factor: f32) {
        self.scale_factor = scale_factor;
    }

    /// Set the cursor icon for this window.
    ///
    /// INV-CURSOR-EPHEMERAL: Called after event dispatch to apply cursor changes.
    /// Widgets request cursors via EventContext::request_cursor().
    pub fn set_cursor(&self, cursor: crate::core::event::CursorIcon) {
        use winit::window::CursorIcon as WinitCursor;
        let winit_cursor = match cursor {
            crate::core::event::CursorIcon::Default => WinitCursor::Default,
            crate::core::event::CursorIcon::Text => WinitCursor::Text,
            crate::core::event::CursorIcon::Pointer => WinitCursor::Pointer,
            crate::core::event::CursorIcon::ColResize => WinitCursor::ColResize,
            crate::core::event::CursorIcon::RowResize => WinitCursor::RowResize,
            crate::core::event::CursorIcon::EwResize => WinitCursor::EwResize,
            crate::core::event::CursorIcon::NsResize => WinitCursor::NsResize,
            crate::core::event::CursorIcon::Move => WinitCursor::Move,
            crate::core::event::CursorIcon::NotAllowed => WinitCursor::NotAllowed,
            crate::core::event::CursorIcon::Crosshair => WinitCursor::Crosshair,
            crate::core::event::CursorIcon::Wait => WinitCursor::Wait,
            crate::core::event::CursorIcon::Hidden => WinitCursor::Default, // unreachable in practice
        };
        self.winit_window.set_cursor(winit_cursor);
    }

    /// SPEC-HAL-268: Show or hide the OS cursor.
    pub fn set_cursor_visible(&self, visible: bool) {
        self.winit_window.set_cursor_visible(visible);
    }
}

pub(crate) fn prepare_startup_visibility(_window: &WinitWindow) {
    #[cfg(windows)]
    startup_visibility_windows::prepare(_window);
}

pub(crate) fn finish_startup_visibility(_window: &WinitWindow) {
    #[cfg(windows)]
    startup_visibility_windows::finish(_window);
}

#[cfg(windows)]
mod startup_visibility_windows {
    use std::ffi::c_void;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use super::WinitWindow;

    const DWMWA_TRANSITIONS_FORCEDISABLED: u32 = 3;

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
        fn DwmFlush() -> i32;
    }

    fn set_transitions_disabled(window: &WinitWindow, disabled: bool) {
        let handle = window
            .window_handle()
            .expect("SPEC-HAL-018 §16: startup window handle unavailable");
        let RawWindowHandle::Win32(handle) = handle.as_raw() else {
            panic!("SPEC-HAL-018 §16: startup window requires a Win32 handle");
        };
        let value: i32 = if disabled { 1 } else { 0 };
        let result = unsafe {
            DwmSetWindowAttribute(
                handle.hwnd.get() as *mut c_void,
                DWMWA_TRANSITIONS_FORCEDISABLED,
                (&value as *const i32).cast(),
                std::mem::size_of::<i32>() as u32,
            )
        };
        assert!(
            result >= 0,
            "SPEC-HAL-018 §16: startup DWM transition setting ({disabled}) failed: {:#010x}",
            result as u32,
        );
    }

    pub(super) fn prepare(window: &WinitWindow) {
        set_transitions_disabled(window, true);
    }

    pub(super) fn finish(window: &WinitWindow) {
        let result = unsafe { DwmFlush() };
        assert!(
            result >= 0,
            "SPEC-HAL-018 §16: first composition flush failed: {:#010x}",
            result as u32,
        );
        // Restore native transitions only after the first composition completes.
        set_transitions_disabled(window, false);
    }
}

// ============================================================================
// SPEC-HAL-033: HalcyonEvent - Unified event wrapper for window and raw input
// ============================================================================

/// Convert winit ModifiersState to Halcyon Modifiers
pub fn convert_modifiers(modifiers: &winit::keyboard::ModifiersState) -> Modifiers {
    Modifiers {
        ctrl: modifiers.control_key(),
        alt: modifiers.alt_key(),
        shift: modifiers.shift_key(),
        meta: modifiers.super_key(),
    }
}

/// Native modifier bookkeeping keeps left and right positions independent.
#[derive(Debug, Default)]
pub struct ModifierKeyTracker {
    held: [bool; 8],
    unobserved_held: [bool; 4],
    snapshot: Modifiers,
}

impl ModifierKeyTracker {
    pub fn observe_snapshot(&mut self, snapshot: Modifiers) {
        self.snapshot = snapshot;
        for (pair, active) in [snapshot.shift, snapshot.ctrl, snapshot.alt, snapshot.meta]
            .into_iter()
            .enumerate()
        {
            if !active {
                self.held[pair * 2] = false;
                self.held[pair * 2 + 1] = false;
            }
            self.unobserved_held[pair] = active && !self.held[pair * 2] && !self.held[pair * 2 + 1];
        }
    }

    pub fn update_key(&mut self, key: PhysicalKeyCode, down: bool) -> Modifiers {
        let index = match key {
            PhysicalKeyCode::ShiftLeft => 0,
            PhysicalKeyCode::ShiftRight => 1,
            PhysicalKeyCode::ControlLeft => 2,
            PhysicalKeyCode::ControlRight => 3,
            PhysicalKeyCode::AltLeft => 4,
            PhysicalKeyCode::AltRight => 5,
            PhysicalKeyCode::SuperLeft => 6,
            PhysicalKeyCode::SuperRight => 7,
            _ => return self.snapshot,
        };
        self.held[index] = down;
        let pair = index / 2;
        let active = self.held[pair * 2] || self.held[pair * 2 + 1] || self.unobserved_held[pair];
        match pair {
            0 => self.snapshot.shift = active,
            1 => self.snapshot.ctrl = active,
            2 => self.snapshot.alt = active,
            3 => self.snapshot.meta = active,
            _ => unreachable!(),
        }
        self.snapshot
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }
}

/// Update modifier state from a KeyboardInput event.
///
/// Winit sends KeyboardInput BEFORE ModifiersChanged for modifier key releases.
/// This causes stale modifiers when converting pointer events that immediately
/// follow a modifier key release. Call this BEFORE processing the event to ensure
/// current_modifiers is accurate.
///
/// Returns true if the event was a modifier key (and modifiers were updated).
pub fn update_modifiers_from_key_event(
    event: &WinitWindowEvent,
    current_modifiers: &mut Modifiers,
) -> bool {
    if let WinitWindowEvent::KeyboardInput {
        event: key_event, ..
    } = event
    {
        let is_pressed = matches!(key_event.state, ElementState::Pressed);

        match key_event.physical_key {
            PhysicalKey::Code(WinitKeyCode::ShiftLeft | WinitKeyCode::ShiftRight) => {
                current_modifiers.shift = is_pressed;
                return true;
            }
            PhysicalKey::Code(WinitKeyCode::ControlLeft | WinitKeyCode::ControlRight) => {
                current_modifiers.ctrl = is_pressed;
                return true;
            }
            PhysicalKey::Code(WinitKeyCode::AltLeft | WinitKeyCode::AltRight) => {
                current_modifiers.alt = is_pressed;
                return true;
            }
            PhysicalKey::Code(WinitKeyCode::SuperLeft | WinitKeyCode::SuperRight) => {
                current_modifiers.meta = is_pressed;
                return true;
            }
            _ => {}
        }
    }
    false
}

/// Unified event type from platform adapter
#[derive(Debug, Clone)]
pub enum HalcyonEvent {
    /// Window lifecycle events (resize, close, scale factor)
    Window(WindowEvent),
    /// Raw input events for fusion processing
    RawInput(RawInputEvent),
}

/// Convert winit events to HalcyonEvent (SPEC-HAL-033)
/// Returns HalcyonEvent::Window for window lifecycle events,
/// and HalcyonEvent::RawInput for pointer/keyboard/focus events.
///
/// SPEC-HAL-151: All raw input events now carry window_id for multi-window support.
///
/// The `current_modifiers` parameter should be updated by tracking
/// `WindowEvent::ModifiersChanged` events in your event loop.
pub fn convert_window_event(
    event: &WinitWindowEvent,
    window_id: WindowId,
    scale_factor: f32,
    last_cursor_pos: (f32, f32),
    current_modifiers: Modifiers,
) -> Vec<HalcyonEvent> {
    convert_window_event_at(event, window_id, scale_factor, last_cursor_pos, current_modifiers, Instant::now())
}

pub(crate) fn convert_window_event_at(
    event: &WinitWindowEvent,
    window_id: WindowId,
    scale_factor: f32,
    last_cursor_pos: (f32, f32),
    current_modifiers: Modifiers,
    time: Instant,
) -> Vec<HalcyonEvent> {
    match event {
        // Window lifecycle events -> HalcyonEvent::Window
        // SPEC-HAL-024 Stage 5: Use coords helper for Physical → Logical conversion
        WinitWindowEvent::Resized(size) => {
            let logical = physical_xy_to_logical_point(
                size.width as f64,
                size.height as f64,
                scale_factor as f64,
            );

            vec![HalcyonEvent::Window(WindowEvent::Resized {
                width: logical.x.round() as u32,
                height: logical.y.round() as u32,
            })]
        }

        WinitWindowEvent::CloseRequested => vec![HalcyonEvent::Window(WindowEvent::CloseRequested)],

        WinitWindowEvent::ScaleFactorChanged { scale_factor, .. } => {
            vec![HalcyonEvent::Window(WindowEvent::ScaleFactorChanged {
                scale_factor: *scale_factor as f32,
            })]
        }

        // Pointer events -> HalcyonEvent::RawInput
        // SPEC-HAL-024 Stage 5: Use coords helper for Physical → Logical conversion
        WinitWindowEvent::CursorMoved { position, .. } => {
            let logical_pos =
                physical_xy_to_logical_point(position.x, position.y, scale_factor as f64);

            // Compute delta from last position (both in logical space)
            let delta = (
                logical_pos.x - last_cursor_pos.0,
                logical_pos.y - last_cursor_pos.1,
            );

            vec![HalcyonEvent::RawInput(RawInputEvent::PointerMoved {
                window_id,
                time,
                device: PointerDevice::Mouse,
                position: logical_pos,
                delta,
            })]
        }

        WinitWindowEvent::Touch(touch) => {
            let phase = match touch.phase {
                winit::event::TouchPhase::Started => TouchPhase::Started,
                winit::event::TouchPhase::Moved => TouchPhase::Moved,
                winit::event::TouchPhase::Ended => TouchPhase::Ended,
                winit::event::TouchPhase::Cancelled => TouchPhase::Cancelled,
            };
            let pos = physical_xy_to_logical_point(
                touch.location.x,
                touch.location.y,
                scale_factor as f64,
            );
            vec![HalcyonEvent::RawInput(RawInputEvent::Touch {
                window_id,
                time,
                contact_id: touch.id,
                phase,
                pos,
            })]
        }

        WinitWindowEvent::MouseInput { state, button, .. } => {
            let pointer_button = match button {
                MouseButton::Left => PointerButton::Primary,
                MouseButton::Right => PointerButton::Secondary,
                MouseButton::Middle => PointerButton::Auxiliary,
                _ => return vec![],
            };

            let button_state = match state {
                ElementState::Pressed => RawButtonState::Pressed,
                ElementState::Released => RawButtonState::Released,
            };

            vec![HalcyonEvent::RawInput(RawInputEvent::PointerButton {
                window_id,
                time,
                device: PointerDevice::Mouse,
                button: pointer_button,
                state: button_state,
                position: Point::new(last_cursor_pos.0, last_cursor_pos.1),
            })]
        }

        // SPEC-HAL-024 Stage 5: Use coords helper for Physical → Logical conversion
        WinitWindowEvent::MouseWheel { delta, .. } => {
            let wheel_delta = match delta {
                winit::event::MouseScrollDelta::LineDelta(x, y) => WheelDelta::Lines(*x, *y),
                winit::event::MouseScrollDelta::PixelDelta(pos) => {
                    let logical = physical_xy_to_logical_point(pos.x, pos.y, scale_factor as f64);
                    WheelDelta::Pixels(logical.x, logical.y)
                }
            };

            vec![HalcyonEvent::RawInput(RawInputEvent::PointerWheel {
                window_id,
                time,
                device: PointerDevice::Mouse,
                delta: wheel_delta,
                position: Point::new(last_cursor_pos.0, last_cursor_pos.1),
            })]
        }

        WinitWindowEvent::KeyboardInput { event, .. } => {
            // SPEC-HAL-038 Stage 6 / SPEC-HAL-176: Foundational keyboard
            // emission. Every keyboard event becomes a paired Key event
            // (Down/Up) with HalcyonKeyCode. On Press, if the event has
            // composed text, a SECONDARY TextInput event is emitted.
            //
            // Key events are the canonical stream — bindings dispatch
            // through them. TextInput is for typing into focused input
            // controls only; it never triggers framework bindings.
            //
            // Clipboard keys retain identity with a reserved disposition.
            // The existing TextInput clipboard consumer still owns the operation.

            // Step 1: Map physical_key to a HalcyonKeyCode.
            let control_key = match event.physical_key {
                PhysicalKey::Code(WinitKeyCode::Enter) => Some(HalcyonKeyCode::Enter),
                PhysicalKey::Code(WinitKeyCode::Tab) => Some(HalcyonKeyCode::Tab),
                PhysicalKey::Code(WinitKeyCode::Backspace) => Some(HalcyonKeyCode::Backspace),
                PhysicalKey::Code(WinitKeyCode::Delete) => Some(HalcyonKeyCode::Delete),
                PhysicalKey::Code(WinitKeyCode::ArrowLeft) => Some(HalcyonKeyCode::Left),
                PhysicalKey::Code(WinitKeyCode::ArrowRight) => Some(HalcyonKeyCode::Right),
                PhysicalKey::Code(WinitKeyCode::ArrowUp) => Some(HalcyonKeyCode::Up),
                PhysicalKey::Code(WinitKeyCode::ArrowDown) => Some(HalcyonKeyCode::Down),
                PhysicalKey::Code(WinitKeyCode::Home) => Some(HalcyonKeyCode::Home),
                PhysicalKey::Code(WinitKeyCode::End) => Some(HalcyonKeyCode::End),
                PhysicalKey::Code(WinitKeyCode::PageUp) => Some(HalcyonKeyCode::PageUp),
                PhysicalKey::Code(WinitKeyCode::PageDown) => Some(HalcyonKeyCode::PageDown),
                PhysicalKey::Code(WinitKeyCode::Escape) => Some(HalcyonKeyCode::Escape),
                PhysicalKey::Code(WinitKeyCode::Space) => Some(HalcyonKeyCode::Space),
                PhysicalKey::Code(WinitKeyCode::F1) => Some(HalcyonKeyCode::F1),
                PhysicalKey::Code(WinitKeyCode::F2) => Some(HalcyonKeyCode::F2),
                PhysicalKey::Code(WinitKeyCode::F3) => Some(HalcyonKeyCode::F3),
                PhysicalKey::Code(WinitKeyCode::F4) => Some(HalcyonKeyCode::F4),
                PhysicalKey::Code(WinitKeyCode::F5) => Some(HalcyonKeyCode::F5),
                PhysicalKey::Code(WinitKeyCode::F6) => Some(HalcyonKeyCode::F6),
                PhysicalKey::Code(WinitKeyCode::F7) => Some(HalcyonKeyCode::F7),
                PhysicalKey::Code(WinitKeyCode::F8) => Some(HalcyonKeyCode::F8),
                PhysicalKey::Code(WinitKeyCode::F9) => Some(HalcyonKeyCode::F9),
                PhysicalKey::Code(WinitKeyCode::F10) => Some(HalcyonKeyCode::F10),
                PhysicalKey::Code(WinitKeyCode::F11) => Some(HalcyonKeyCode::F11),
                PhysicalKey::Code(WinitKeyCode::F12) => Some(HalcyonKeyCode::F12),
                PhysicalKey::Code(WinitKeyCode::ShiftLeft | WinitKeyCode::ShiftRight) => {
                    Some(HalcyonKeyCode::Shift)
                }
                PhysicalKey::Code(WinitKeyCode::ControlLeft | WinitKeyCode::ControlRight) => {
                    Some(HalcyonKeyCode::Control)
                }
                PhysicalKey::Code(WinitKeyCode::AltLeft | WinitKeyCode::AltRight) => {
                    Some(HalcyonKeyCode::Alt)
                }
                PhysicalKey::Code(WinitKeyCode::SuperLeft | WinitKeyCode::SuperRight) => {
                    Some(HalcyonKeyCode::Meta)
                }
                _ => None,
            };

            let halcyon_key_code: Option<HalcyonKeyCode> = if let Some(ck) = control_key {
                Some(ck)
            } else {
                // Character key: extract char from logical_key, fall back
                // to physical_key mapping if logical_key is non-character
                // (e.g., dead keys on some platforms).
                let ch = match &event.logical_key {
                    winit::keyboard::Key::Character(s) => {
                        s.chars().next().map(|c| c.to_ascii_lowercase())
                    }
                    _ => None,
                };
                ch.filter(|c| c.is_ascii_alphanumeric() || "->=[]\\',./ ".contains(*c))
                    .or_else(|| match event.physical_key {
                        PhysicalKey::Code(code) => physical_key_to_char(code),
                        _ => None,
                    })
                    .map(HalcyonKeyCode::Character)
            };

            // Step 2: Determine if this is a clipboard shortcut combo —
            // those emit TextInput only (existing carve-out, preserved
            // verbatim so TextField clipboard behavior is unchanged).
            let is_clipboard_shortcut = if current_modifiers.ctrl
                || current_modifiers.alt
                || current_modifiers.meta
            {
                let ch = match &event.logical_key {
                    winit::keyboard::Key::Character(s) => s.chars().next(),
                    _ => None,
                };
                let is_cmd = crate::core::text_edit_policy::is_primary_shortcut(current_modifiers);
                is_cmd
                    && matches!(
                        ch,
                        Some('a' | 'c' | 'v' | 'x' | 'z' | 'y' | 'A' | 'C' | 'V' | 'X' | 'Z' | 'Y')
                    )
            } else {
                false
            };

            let mut events: Vec<HalcyonEvent> = Vec::new();

            let key_state = match event.state {
                ElementState::Pressed => KeyState::Down,
                ElementState::Released => KeyState::Up,
            };
            events.push(HalcyonEvent::RawInput(RawInputEvent::Key {
                window_id,
                time,
                key_code: halcyon_key_code.unwrap_or(HalcyonKeyCode::Unknown),
                physical_key: convert_physical_key(event.physical_key),
                reservation: is_clipboard_shortcut.then_some(KeyboardKeyReservation::Clipboard),
                state: key_state,
                repeat: event.repeat,
                modifiers: current_modifiers,
            }));

            // Step 4: On Press with non-empty composed text, emit
            // secondary TextInput event. For Capture mode this drives
            // text insertion in TextField/TextArea. For Navigation mode
            // it's a no-op (widgets don't consume Text events in
            // Navigation), but emitting it preserves IME-composition
            // semantics and the clipboard-shortcut path.
            //
            // SPEC-HAL-038 §6 / SPEC-HAL-176: TextInput carries TYPABLE
            // text only. winit's `event.text` field is non-empty for
            // control keys on some platforms — Backspace ("\u{8}"),
            // Delete ("\u{7f}"), Enter ("\r"/"\n"), Tab ("\t"), Escape
            // ("\u{1b}"). These keys are dispatched as Key events via
            // Step 3; emitting them as TextInput too would cause
            // TextField/TextArea to insert the Cc character as literal
            // text, corrupting the buffer's grapheme bookkeeping.
            //
            // Filter: drop any string containing ANY Cc (General_Category
            // =Control) character. Space (U+0020, category Zs) passes —
            // typing spaces is unaffected. Letters, digits, symbols,
            // combining marks, IME composed text all pass.
            if event.state == ElementState::Pressed {
                if let Some(text) = &event.text {
                    if !text.is_empty() && text.chars().all(|c| !c.is_control()) {
                        events.push(HalcyonEvent::RawInput(RawInputEvent::TextInput {
                            window_id,
                            time,
                            text: text.to_string(),
                            modifiers: current_modifiers,
                        }));
                    }
                }
            }

            events
        }

        WinitWindowEvent::Focused(focused) => {
            vec![HalcyonEvent::RawInput(RawInputEvent::FocusChanged {
                window_id,
                time,
                focused: *focused,
            })]
        }

        _ => vec![],
    }
}

pub fn convert_physical_key(key: PhysicalKey) -> PhysicalKeyCode {
    macro_rules! positions {
        ($($position:ident),+ $(,)?) => {
            match key {
                $(PhysicalKey::Code(WinitKeyCode::$position) => PhysicalKeyCode::$position,)+
                _ => PhysicalKeyCode::Unknown,
            }
        };
    }
    positions!(
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
    )
}

/// Map physical key codes to lowercase ASCII characters for legacy logical UI input.
/// Used when logical_key produces a control character
/// (e.g., Ctrl+S → "\x13") and we need the actual letter.
fn physical_key_to_char(code: WinitKeyCode) -> Option<char> {
    match code {
        WinitKeyCode::KeyA => Some('a'),
        WinitKeyCode::KeyB => Some('b'),
        WinitKeyCode::KeyC => Some('c'),
        WinitKeyCode::KeyD => Some('d'),
        WinitKeyCode::KeyE => Some('e'),
        WinitKeyCode::KeyF => Some('f'),
        WinitKeyCode::KeyG => Some('g'),
        WinitKeyCode::KeyH => Some('h'),
        WinitKeyCode::KeyI => Some('i'),
        WinitKeyCode::KeyJ => Some('j'),
        WinitKeyCode::KeyK => Some('k'),
        WinitKeyCode::KeyL => Some('l'),
        WinitKeyCode::KeyM => Some('m'),
        WinitKeyCode::KeyN => Some('n'),
        WinitKeyCode::KeyO => Some('o'),
        WinitKeyCode::KeyP => Some('p'),
        WinitKeyCode::KeyQ => Some('q'),
        WinitKeyCode::KeyR => Some('r'),
        WinitKeyCode::KeyS => Some('s'),
        WinitKeyCode::KeyT => Some('t'),
        WinitKeyCode::KeyU => Some('u'),
        WinitKeyCode::KeyV => Some('v'),
        WinitKeyCode::KeyW => Some('w'),
        WinitKeyCode::KeyX => Some('x'),
        WinitKeyCode::KeyY => Some('y'),
        WinitKeyCode::KeyZ => Some('z'),
        WinitKeyCode::Digit0 => Some('0'),
        WinitKeyCode::Digit1 => Some('1'),
        WinitKeyCode::Digit2 => Some('2'),
        WinitKeyCode::Digit3 => Some('3'),
        WinitKeyCode::Digit4 => Some('4'),
        WinitKeyCode::Digit5 => Some('5'),
        WinitKeyCode::Digit6 => Some('6'),
        WinitKeyCode::Digit7 => Some('7'),
        WinitKeyCode::Digit8 => Some('8'),
        WinitKeyCode::Digit9 => Some('9'),
        WinitKeyCode::Minus => Some('-'),
        WinitKeyCode::Equal => Some('='),
        WinitKeyCode::BracketLeft => Some('['),
        WinitKeyCode::BracketRight => Some(']'),
        WinitKeyCode::Backslash => Some('\\'),
        WinitKeyCode::Semicolon => Some(';'),
        WinitKeyCode::Quote => Some('\''),
        WinitKeyCode::Comma => Some(','),
        WinitKeyCode::Period => Some('.'),
        WinitKeyCode::Slash => Some('/'),
        WinitKeyCode::Backquote => Some('`'),
        _ => None,
    }
}

/// Create event loop
pub fn create_event_loop() -> EventLoop<()> {
    event_loop_builder().build().unwrap()
}

fn event_loop_builder() -> winit::event_loop::EventLoopBuilder<()> {
    let builder = EventLoop::<()>::with_user_event();
    #[cfg(target_os = "windows")]
    let mut builder = builder;
    #[cfg(target_os = "windows")]
    {
        use winit::platform::windows::EventLoopBuilderExtWindows;
        builder.with_any_thread(true);
    }
    builder
}

/// HAL333 §7: ordinary App owns the mediated menu from startup. The public
/// low-level event-loop factory retains winit's original platform defaults.
pub(crate) fn create_app_event_loop() -> EventLoop<()> {
    let mut builder = event_loop_builder();
    #[cfg(target_os = "macos")]
    {
        use winit::platform::macos::EventLoopBuilderExtMacOS;
        builder.with_default_menu(false);
    }
    builder.build().unwrap()
}

/// Set control flow for event loop
pub fn set_control_flow(control_flow: &mut ControlFlow) {
    *control_flow = ControlFlow::Wait;
}

#[cfg(test)]
mod tests {
    use super::{
        EventLoop, Fullscreen, ModifierKeyTracker, SignalEndpoint, Window, convert_physical_key,
    };
    use crate::core::event::{Modifiers, PhysicalKeyCode, RawInputEvent, TouchPhase, WindowId};
    use winit::dpi::{LogicalSize, PhysicalPosition};

    #[test]
    fn touch_contact_native_phases_identity_and_effective_scale() {
        use super::{HalcyonEvent, convert_window_event};
        use winit::event::{DeviceId, Touch, TouchPhase as NativeTouchPhase, WindowEvent};

        for (native_phase, expected_phase) in [
            (NativeTouchPhase::Started, TouchPhase::Started),
            (NativeTouchPhase::Moved, TouchPhase::Moved),
            (NativeTouchPhase::Ended, TouchPhase::Ended),
            (NativeTouchPhase::Cancelled, TouchPhase::Cancelled),
        ] {
            for contact_id in [0, u64::MAX] {
                for (scale, expected_pos) in [(2.0, (90.0, 60.0)), (3.0, (60.0, 40.0))] {
                    let native = WindowEvent::Touch(Touch {
                        device_id: DeviceId::dummy(),
                        phase: native_phase,
                        location: PhysicalPosition::new(180.0, 120.0),
                        force: None,
                        id: contact_id,
                    });
                    let events = convert_window_event(
                        &native,
                        WindowId(17),
                        scale,
                        (999.0, -30.0),
                        Modifiers::default(),
                    );
                    assert_eq!(events.len(), 1);
                    let HalcyonEvent::RawInput(RawInputEvent::Touch {
                        window_id,
                        contact_id: actual_id,
                        phase,
                        pos,
                        ..
                    }) = &events[0]
                    else {
                        panic!("native touch must enter the canonical raw contact stream");
                    };
                    assert_eq!(*window_id, WindowId(17));
                    assert_eq!(*actual_id, contact_id);
                    assert_eq!(*phase, expected_phase);
                    assert_eq!((pos.x, pos.y), expected_pos);
                }
            }
        }
    }

    #[test]
    fn hal400_projected_touch_has_one_raw_sample_with_original_time() {
        use super::{HalcyonEvent, convert_window_event_at};
        use winit::event::{DeviceId, Touch, TouchPhase as NativeTouchPhase, WindowEvent};

        let start = web_time::Instant::now();
        for (step, (native, phase)) in [
            (NativeTouchPhase::Started, TouchPhase::Started),
            (NativeTouchPhase::Moved, TouchPhase::Moved),
            (NativeTouchPhase::Ended, TouchPhase::Ended),
        ].into_iter().enumerate() {
            let time = start + std::time::Duration::from_millis(step as u64 * 20);
            let point = PhysicalPosition::new(402.75, 350.25 - 12.0 * step as f64);
            let raw = convert_window_event_at(
                &WindowEvent::Touch(Touch {
                    device_id: DeviceId::dummy(), id: 73, phase: native, force: None, location: point,
                }),
                WindowId(17), 3.0, (0.0, 0.0), Modifiers::default(), time,
            );
            let [HalcyonEvent::RawInput(RawInputEvent::Touch {
                window_id, contact_id, phase: actual_phase, pos, time: actual_time,
            })] = raw.as_slice() else {
                panic!("A projected contact must produce exactly one original raw sample");
            };
            assert_eq!((*window_id, *contact_id, *actual_phase, *actual_time), (WindowId(17), 73, phase, time));
            assert_eq!((pos.x, pos.y), (point.x as f32 / 3.0, point.y as f32 / 3.0));
        }
    }

    #[test]
    fn touch_scroll_platform_logical_coordinates() {
        use super::{HalcyonEvent, convert_window_event};
        use crate::core::input_fusion::DRAG_DISTANCE_THRESHOLD;
        use crate::core::math::Point;
        use winit::event::{DeviceId, Touch, TouchPhase as NativeTouchPhase, WindowEvent};

        for (css_width, dpr) in [(320.0, 2.0), (390.0, 3.0)] {
            let positions = [0.0, 3.0, 4.0].map(|displacement| {
                let native = WindowEvent::Touch(Touch {
                    device_id: DeviceId::dummy(),
                    phase: if displacement == 0.0 {
                        NativeTouchPhase::Started
                    } else {
                        NativeTouchPhase::Moved
                    },
                    location: PhysicalPosition::new(
                        css_width * dpr / 2.0,
                        (80.0 - displacement) * dpr,
                    ),
                    force: None,
                    id: 5,
                });
                let events = convert_window_event(
                    &native,
                    WindowId(17),
                    dpr as f32,
                    (999.0, 0.0),
                    Modifiers::default(),
                );
                let [
                    HalcyonEvent::RawInput(RawInputEvent::Touch {
                        window_id,
                        contact_id,
                        pos,
                        ..
                    }),
                ] = events.as_slice()
                else {
                    panic!("expected canonical native touch ingress");
                };
                assert_eq!((*window_id, *contact_id), (WindowId(17), 5));
                *pos
            });
            assert_eq!(positions[0], Point::new(css_width as f32 / 2.0, 80.0));
            assert!(positions[1].distance(positions[0]) < DRAG_DISTANCE_THRESHOLD);
            assert_eq!(positions[2].distance(positions[0]), DRAG_DISTANCE_THRESHOLD);
        }
    }

    #[test]
    fn startup_fullscreen_legacy_creation_attributes_are_windowed() {
        let _: fn(&EventLoop<()>, &str, u32, u32) -> Window = Window::new;
        let _: fn(&EventLoop<()>, &str, u32, u32, Option<SignalEndpoint>) -> Window =
            Window::with_endpoint;
        let attributes = Window::creation_attributes("Home", 720, 480, false, true);
        assert_eq!(attributes.fullscreen, None);
        assert!(attributes.visible);
        assert_eq!(attributes.title, "Home");
        assert_eq!(
            attributes.inner_size,
            Some(LogicalSize::new(720, 480).into())
        );
        assert_eq!(
            attributes.min_inner_size,
            Some(LogicalSize::new(64.0, 64.0).into()),
        );
        assert!(!attributes.maximized);
        assert!(attributes.decorations);
    }

    #[test]
    fn startup_fullscreen_creation_attributes_request_borderless() {
        let attributes = Window::creation_attributes("Home", 720, 480, true, true);
        assert_eq!(attributes.fullscreen, Some(Fullscreen::Borderless(None)));
        assert!(attributes.visible);
        assert_eq!(attributes.title, "Home");
        assert_eq!(
            attributes.inner_size,
            Some(LogicalSize::new(720, 480).into())
        );
        assert_eq!(
            attributes.min_inner_size,
            Some(LogicalSize::new(64.0, 64.0).into()),
        );
        assert!(!attributes.maximized);
        assert!(attributes.decorations);
    }

    #[test]
    fn startup_hidden_creation_preserves_requested_mode_and_dimensions() {
        for fullscreen in [false, true] {
            let attributes = Window::creation_attributes("Home", 720, 480, fullscreen, false);
            assert!(!attributes.visible);
            assert_eq!(
                attributes.fullscreen,
                fullscreen.then_some(Fullscreen::Borderless(None)),
            );
            assert_eq!(
                attributes.inner_size,
                Some(LogicalSize::new(720, 480).into()),
            );
            assert_eq!(
                attributes.min_inner_size,
                Some(LogicalSize::new(64.0, 64.0).into()),
            );
        }
    }

    #[test]
    fn physical_keyboard_native_positions_and_side_modifiers_are_distinct() {
        use winit::keyboard::{KeyCode, NativeKeyCode, PhysicalKey};
        for (native, expected) in [
            (KeyCode::KeyA, PhysicalKeyCode::KeyA),
            (KeyCode::Digit1, PhysicalKeyCode::Digit1),
            (KeyCode::Numpad1, PhysicalKeyCode::Numpad1),
            (KeyCode::Enter, PhysicalKeyCode::Enter),
            (KeyCode::NumpadEnter, PhysicalKeyCode::NumpadEnter),
            (KeyCode::ControlLeft, PhysicalKeyCode::ControlLeft),
            (KeyCode::ControlRight, PhysicalKeyCode::ControlRight),
            (KeyCode::SuperLeft, PhysicalKeyCode::SuperLeft),
        ] {
            assert_eq!(convert_physical_key(PhysicalKey::Code(native)), expected);
        }
        assert_eq!(
            convert_physical_key(PhysicalKey::Unidentified(NativeKeyCode::Unidentified)),
            PhysicalKeyCode::Unknown
        );
        let mut tracker = ModifierKeyTracker::default();
        assert!(tracker.update_key(PhysicalKeyCode::ControlLeft, true).ctrl);
        assert!(tracker.update_key(PhysicalKeyCode::ControlRight, true).ctrl);
        assert!(tracker.update_key(PhysicalKeyCode::ControlLeft, false).ctrl);
        let both = tracker.update_key(PhysicalKeyCode::SuperRight, true);
        assert!(both.ctrl && both.meta);
        let command = tracker.update_key(PhysicalKeyCode::ControlRight, false);
        assert!(!command.ctrl && command.meta);
        tracker.clear();
        tracker.observe_snapshot(Modifiers {
            shift: true,
            ..Modifiers::default()
        });
        assert!(tracker.update_key(PhysicalKeyCode::KeyA, true).shift);
        // An aggregate pre-held modifier does not identify its physical side.
        assert!(tracker.update_key(PhysicalKeyCode::ShiftLeft, false).shift);
        tracker.observe_snapshot(Modifiers::default());
        assert_eq!(
            tracker.update_key(PhysicalKeyCode::KeyA, true),
            Modifiers::default()
        );
    }
}
