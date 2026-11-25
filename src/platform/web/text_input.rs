//! HAL-398. DOM focus, trusted input and viewport observation for one canvas.

use super::{AppLoopEvent, Handler};
use crate::core::event::{KeyCode, KeyState, Modifiers, PhysicalKeyCode, RawInputEvent, WindowId};
use crate::core::event_router::browser_text::{BrowserTextKind, BrowserTextProjection};
use crate::core::math::Size;
use std::cell::{Cell, RefCell};
use std::rc::{Rc, Weak};
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{Event, EventTarget, HtmlCanvasElement, HtmlElement, PointerEvent};
use winit::platform::web::WindowExtWebSys;

pub(crate) use crate::core::event_router::browser_text::BrowserTextReport;

pub(super) struct Listener {
    target: EventTarget,
    kind: &'static str,
    capture: bool,
    callback: Closure<dyn FnMut(Event)>,
}

impl Listener {
    pub(super) fn install(
        target: &EventTarget,
        kind: &'static str,
        capture: bool,
        callback: impl FnMut(Event) + 'static,
    ) -> Result<Self, JsValue> {
        let callback = Closure::wrap(Box::new(callback) as Box<dyn FnMut(Event)>);
        target.add_event_listener_with_callback_and_bool(
            kind,
            callback.as_ref().unchecked_ref(),
            capture,
        )?;
        Ok(Self {
            target: target.clone(),
            kind,
            capture,
            callback,
        })
    }
}

impl Drop for Listener {
    fn drop(&mut self) {
        let _ = self.target.remove_event_listener_with_callback_and_bool(
            self.kind,
            self.callback.as_ref().unchecked_ref(),
            self.capture,
        );
    }
}

struct Proxy {
    element: HtmlElement,
    kind: BrowserTextKind,
    projected_selection: Cell<(u32, u32)>,
    refused_clipboard_cursor: Cell<Option<u32>>,
    _listeners: Vec<Listener>,
}

impl Proxy {
    fn selection(&self) -> Option<(u32, u32)> {
        let (start, end, direction) =
            if let Some(input) = self.element.dyn_ref::<web_sys::HtmlInputElement>() {
                (
                    input.selection_start().ok()??,
                    input.selection_end().ok()??,
                    input.selection_direction().ok()?,
                )
            } else {
                let area = self.element.dyn_ref::<web_sys::HtmlTextAreaElement>()?;
                (
                    area.selection_start().ok()??,
                    area.selection_end().ok()??,
                    area.selection_direction().ok()?,
                )
            };
        Some(if direction.as_deref() == Some("backward") {
            (end, start)
        } else {
            (start, end)
        })
    }

    fn value(&self) -> Option<String> {
        let value =
            js_sys::Reflect::get(self.element.as_ref(), &JsValue::from_str("value")).ok()?;
        let value = value.dyn_into::<js_sys::JsString>().ok()?;
        value.is_valid_utf16().then(|| String::from(&value))
    }

    fn set_selection(&self, (anchor, head): (u32, u32)) -> bool {
        let (start, end) = (anchor.min(head), anchor.max(head));
        let direction = if head < anchor { "backward" } else { "forward" };
        if let Some(input) = self.element.dyn_ref::<web_sys::HtmlInputElement>() {
            input
                .set_selection_range_with_direction(start, end, direction)
                .is_ok()
        } else {
            self.element
                .dyn_ref::<web_sys::HtmlTextAreaElement>()
                .is_some_and(|area| {
                    area.set_selection_range_with_direction(start, end, direction)
                        .is_ok()
                })
        }
    }

    fn project(&self, projection: &BrowserTextProjection) {
        self.projected_selection.set(projection.selection);
        self.refused_clipboard_cursor.set(None);
        let changed = self.value().as_deref() != Some(projection.value.as_str());
        if changed {
            if let Some(input) = self.element.dyn_ref::<web_sys::HtmlInputElement>() {
                input.set_value(&projection.value);
            } else if let Some(area) = self.element.dyn_ref::<web_sys::HtmlTextAreaElement>() {
                area.set_value(&projection.value);
            }
        }
        if changed || self.selection() != Some(projection.selection) {
            let _ = self.set_selection(projection.selection);
        }
    }

    fn omit_default_clipboard_write(&self) {
        // A declined transfer leaves the event uncanceled. Clear only the proxy's
        // selectable range so the browser cannot export its transport text.
        if let Some((anchor, head)) = self.selection()
            && anchor != head
            && self.set_selection((head, head))
        {
            self.refused_clipboard_cursor.set(Some(head));
        }
    }

    fn restore_after_refused_clipboard_write(&self) -> bool {
        let Some(cursor) = self.refused_clipboard_cursor.take() else {
            return false;
        };
        if self.selection() != Some((cursor, cursor)) {
            return false;
        }
        let _ = self.set_selection(self.projected_selection.get());
        true
    }
}

impl Drop for Proxy {
    fn drop(&mut self) {
        self._listeners.clear();
        let _ = js_sys::Reflect::set(
            self.element.as_ref(),
            &JsValue::from_str("value"),
            &JsValue::from_str(""),
        );
        self.element.remove();
    }
}

#[derive(Clone, Copy)]
enum CanvasFocusRestore {
    Immediate,
    Reporting,
    Requested,
}

struct BrowserTextState {
    choice: Option<super::choice_picker::BrowserChoiceBridge>,
    canvas: HtmlCanvasElement,
    window: winit::window::WindowId,
    handler: Weak<RefCell<Option<Handler>>>,
    trusted_up: RefCell<Option<PointerEvent>>,
    proxy: RefCell<Option<Proxy>>,
    projecting: Cell<bool>,
    canvas_focus_restore: Cell<CanvasFocusRestore>,
    listeners: RefCell<Vec<Listener>>,
}

#[derive(Clone)]
pub(crate) struct BrowserTextBridge(Rc<BrowserTextState>);

impl BrowserTextBridge {
    pub(super) fn install(
        window: &winit::window::Window,
        handler: &Rc<RefCell<Option<Handler>>>,
        choice: Option<super::choice_picker::BrowserChoiceBridge>,
    ) -> Result<Self, JsValue> {
        let canvas = window
            .canvas()
            .ok_or_else(|| JsValue::from_str("App canvas unavailable"))?;
        let state = Rc::new(BrowserTextState {
            choice,
            canvas,
            window: window.id(),
            handler: Rc::downgrade(handler),
            trusted_up: RefCell::new(None),
            proxy: RefCell::new(None),
            projecting: Cell::new(false),
            canvas_focus_restore: Cell::new(CanvasFocusRestore::Immediate),
            listeners: RefCell::new(Vec::new()),
        });
        let weak = Rc::downgrade(&state);
        let touch = Listener::install(state.canvas.as_ref(), "pointerup", true, move |event| {
            if let Some(state) = weak.upgrade()
                && let Ok(event) = event.dyn_into::<PointerEvent>()
                && event.is_trusted()
                && event.pointer_type() == "touch"
                && event.target().as_ref() == Some(state.canvas.as_ref())
            {
                state.trusted_up.replace(Some(event));
            }
        })?;
        state.listeners.borrow_mut().push(touch);
        for kind in ["copy", "cut"] {
            let weak = Rc::downgrade(&state);
            state.listeners.borrow_mut().push(Listener::install(
                state.canvas.as_ref(),
                kind,
                false,
                move |event| {
                    if let Some(state) = weak.upgrade() {
                        BrowserTextBridge(state).clipboard_write_event(&event, false);
                    }
                },
            )?);
        }
        if let Some(viewport) = state
            .canvas
            .owner_document()
            .and_then(|document| document.default_view())
            .and_then(|window| window.visual_viewport())
        {
            for kind in ["resize", "scroll"] {
                let weak = Rc::downgrade(&state);
                state.listeners.borrow_mut().push(Listener::install(
                    viewport.as_ref(),
                    kind,
                    false,
                    move |_| {
                        let Some(state) = weak.upgrade() else {
                            return;
                        };
                        if let Some(choice) = &state.choice {
                            choice.geometry_changed();
                        }
                        if state.proxy.borrow().is_none() {
                            return;
                        }
                        let bridge = Self(state);
                        bridge.report(BrowserTextReport::Viewport(bridge.visible_canvas_size()));
                    },
                )?);
            }
        }
        Ok(Self(state))
    }

    fn report(&self, event: BrowserTextReport) {
        self.dispatch(AppLoopEvent::BrowserText {
            text: self.clone(),
            event,
        });
    }

    fn dispatch(&self, event: AppLoopEvent) {
        let Some(handler) = self.0.handler.upgrade() else {
            return;
        };
        let Ok(mut handler) = handler.try_borrow_mut() else {
            return;
        };
        self.begin_dispatch();
        if let Some(handler) = handler.as_mut() {
            handler(event, None);
        }
        // Canvas focus can synchronously dispatch a winit event back to App.
        drop(handler);
        self.finish_dispatch();
    }

    pub(super) fn begin_dispatch(&self) {
        self.0.canvas_focus_restore.set(CanvasFocusRestore::Reporting);
    }

    pub(super) fn finish_dispatch(&self) {
        if let CanvasFocusRestore::Requested = self
            .0
            .canvas_focus_restore
            .replace(CanvasFocusRestore::Immediate)
        {
            let _ = self.0.canvas.focus();
        }
    }

    fn clipboard_write_event(&self, event: &Event, from_proxy: bool) {
        let Some(event) = event.dyn_ref::<web_sys::ClipboardEvent>() else {
            return;
        };
        if self.owns_clipboard_write_event(event, from_proxy) {
            self.dispatch(AppLoopEvent::ClipboardWrite {
                window: self.0.window,
                text: self.clone(),
                from_proxy,
                event: event.clone(),
            });
        }
    }

    pub(crate) fn owns_clipboard_write_event(
        &self, event: &web_sys::ClipboardEvent, from_proxy: bool,
    ) -> bool {
        let proxy = self.0.proxy.borrow();
        let target: &HtmlElement = if from_proxy {
            let Some(proxy) = proxy.as_ref() else {
                return false;
            };
            &proxy.element
        } else {
            &self.0.canvas
        };
        event.is_trusted()
            && event.cancelable()
            && !event.default_prevented()
            && event.target().as_ref() == Some(target.as_ref())
            && target.owner_document().is_some_and(|document| {
                document.has_focus().unwrap_or(false)
                    && document.active_element().as_ref() == Some(target.as_ref())
            })
    }

    pub(crate) fn choice(&self) -> Option<&super::choice_picker::BrowserChoiceBridge> {
        self.0.choice.as_ref()
    }

    pub(crate) fn trusted_touch_up(&self, window: winit::window::WindowId, contact: u64) -> bool {
        self.0.window == window
            && self.0.trusted_up.borrow().as_ref().is_some_and(|event| {
                event.event_phase() != 0
                    && event.pointer_id() as u64 == contact
                    && event.is_trusted()
            })
    }

    pub(crate) fn has_proxy_focus(&self) -> bool {
        self.0.proxy.borrow().as_ref().is_some_and(|proxy| {
            proxy.element.owner_document().is_some_and(|document| {
                document.has_focus().unwrap_or(false)
                    && document.active_element().as_ref() == Some(proxy.element.as_ref())
            })
        })
    }

    pub(crate) fn activate(
        &self,
        window: winit::window::WindowId,
        contact: u64,
        projection: &BrowserTextProjection,
    ) -> bool {
        if !self.trusted_touch_up(window, contact) {
            return false;
        }
        self.0.trusted_up.borrow_mut().take();
        let result = (|| {
            if self
                .0
                .proxy
                .borrow()
                .as_ref()
                .is_none_or(|proxy| proxy.kind != projection.kind)
            {
                self.retire();
                let proxy = self.make_proxy(projection.kind).ok()?;
                self.0.proxy.replace(Some(proxy));
            }
            self.project(projection);
            let element = self.0.proxy.borrow().as_ref()?.element.clone();
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            element.focus_with_options(&options).ok()?;
            self.has_proxy_focus().then_some(())
        })();
        if result.is_none() {
            self.retire();
        }
        result.is_some()
    }

    pub(crate) fn project(&self, projection: &BrowserTextProjection) {
        if !projection.composing {
            self.0.projecting.set(true);
            if let Some(proxy) = self.0.proxy.borrow().as_ref() {
                proxy.project(projection);
            }
            self.0.projecting.set(false);
        }
    }

    pub(crate) fn retire(&self) {
        let proxy = self.0.proxy.borrow_mut().take();
        drop(proxy);
    }

    pub(crate) fn restore_canvas_focus(&self) {
        match self.0.canvas_focus_restore.get() {
            CanvasFocusRestore::Immediate => {
                let _ = self.0.canvas.focus();
            }
            CanvasFocusRestore::Reporting | CanvasFocusRestore::Requested => {
                self.0.canvas_focus_restore.set(CanvasFocusRestore::Requested);
            }
        }
    }

    pub(crate) fn visible_canvas_size(&self) -> Option<Size> {
        let viewport = self
            .0
            .canvas
            .owner_document()?
            .default_view()?
            .visual_viewport()?;
        let canvas = self.0.canvas.get_bounding_client_rect();
        let height =
            (viewport.offset_top() + viewport.height() - canvas.top()).min(canvas.height());
        let width = (viewport.offset_left() + viewport.width() - canvas.left()).min(canvas.width());
        (width.is_finite() && height.is_finite() && width > 0.0 && height > 0.0)
            .then_some(Size::new(width as f32, height as f32))
    }

    fn make_proxy(&self, kind: BrowserTextKind) -> Result<Proxy, JsValue> {
        let document = self
            .0
            .canvas
            .owner_document()
            .ok_or_else(|| JsValue::from_str("App document unavailable"))?;
        let element = document
            .create_element(if kind == BrowserTextKind::Multiline {
                "textarea"
            } else {
                "input"
            })?
            .dyn_into::<HtmlElement>()?;
        if kind == BrowserTextKind::Password {
            element.set_attribute("type", "password")?;
        }
        element.set_attribute("tabindex", "-1")?;
        element.set_attribute("aria-hidden", "true")?;
        element.set_attribute("autocomplete", "off")?;
        element.set_attribute("autocapitalize", "off")?;
        element.set_attribute("spellcheck", "false")?;
        element.set_attribute("style", "position:fixed;top:0;left:0;width:1px;height:1px;opacity:0;pointer-events:none;font-size:16px;padding:0;border:0;")?;
        let mut listeners = Vec::new();
        for name in [
            "beforeinput",
            "input",
            "compositionstart",
            "compositionupdate",
            "compositionend",
            "select",
            "keydown",
            "keyup",
            "blur",
            "paste",
            "cut",
            "copy",
        ] {
            let weak = Rc::downgrade(&self.0);
            listeners.push(Listener::install(
                element.as_ref(),
                name,
                false,
                move |event| {
                    if let Some(state) = weak.upgrade() {
                        Self(state).proxy_event(name, event);
                    }
                },
            )?);
        }
        self.0
            .canvas
            .parent_node()
            .ok_or_else(|| JsValue::from_str("Canvas is detached"))?
            .append_child(&element)?;
        Ok(Proxy {
            element,
            kind,
            projected_selection: Cell::new((0, 0)),
            refused_clipboard_cursor: Cell::new(None),
            _listeners: listeners,
        })
    }

    fn proxy_event(&self, name: &str, event: Event) {
        if self.0.projecting.get() || !event.is_trusted() {
            return;
        }
        if matches!(name, "copy" | "cut") {
            let target = self.0.proxy.borrow().as_ref().map(|proxy| proxy.element.clone());
            if let Some(target) = target
                && event.target().as_ref() == Some(target.as_ref())
            {
                self.clipboard_write_event(&event, true);
                if !event.default_prevented()
                    && let Some(proxy) = self.0.proxy.borrow().as_ref()
                    && proxy.element == target
                {
                    proxy.omit_default_clipboard_write();
                }
            }
            return;
        }
        let report = {
            let proxy = self.0.proxy.borrow();
            let Some(proxy) = proxy.as_ref() else {
                return;
            };
            if event.target().as_ref() != Some(proxy.element.as_ref()) {
                return;
            }
            // setSelectionRange queues a select event. Restore the last
            // engine projection without feeding the temporary gap to App.
            if name != "blur" && proxy.restore_after_refused_clipboard_write() && name == "select" {
                return;
            }
            match name {
                "beforeinput" => {
                    let Some(input) = event.dyn_ref::<web_sys::InputEvent>() else {
                        return;
                    };
                    let kind = input.input_type();
                    if matches!(
                        kind.as_str(),
                        "insertFromPaste" | "insertFromDrop" | "deleteByCut"
                    ) {
                        event.prevent_default();
                    }
                    BrowserTextReport::BeforeInput
                }
                "input" => {
                    let Some(input) = event.dyn_ref::<web_sys::InputEvent>() else {
                        return;
                    };
                    let value = proxy.value();
                    BrowserTextReport::Input {
                        value: match value {
                            Some(value) => value,
                            None => return,
                        },
                        kind: input.input_type(),
                        composing: input.is_composing(),
                        selection: proxy.selection(),
                    }
                }
                "compositionstart" | "compositionupdate" | "compositionend" => {
                    let Some(input) = event.dyn_ref::<web_sys::CompositionEvent>() else {
                        return;
                    };
                    let data = input.data().unwrap_or_default();
                    if name == "compositionend" {
                        BrowserTextReport::Commit(data)
                    } else {
                        BrowserTextReport::PreEdit(data)
                    }
                }
                "select" => {
                    let Some((anchor, head)) = proxy.selection() else {
                        return;
                    };
                    BrowserTextReport::Select(anchor, head)
                }
                "keydown" | "keyup" => {
                    let Some(key) = event.dyn_ref::<web_sys::KeyboardEvent>() else {
                        return;
                    };
                    let Some((raw, shortcut_text)) = physical_key(key, name == "keydown") else {
                        return;
                    };
                    let character = key.key();
                    let clipboard_shortcut = (key.ctrl_key() || key.meta_key())
                        && (character.eq_ignore_ascii_case("v")
                            || (!key.alt_key()
                                && !key.shift_key()
                                && (character.eq_ignore_ascii_case("c")
                                    || character.eq_ignore_ascii_case("x"))));
                    if !clipboard_shortcut {
                        event.prevent_default();
                    }
                    BrowserTextReport::Key {
                        key: raw,
                        shortcut_text,
                    }
                }
                "paste" => {
                    let Ok(event) = event.dyn_into::<web_sys::ClipboardEvent>() else {
                        return;
                    };
                    let Some(handler) = self.0.handler.upgrade() else {
                        return;
                    };
                    if let Ok(mut handler) = handler.try_borrow_mut()
                        && let Some(handler) = handler.as_mut()
                    {
                        handler(
                            AppLoopEvent::Paste {
                                window: self.0.window,
                                event,
                            },
                            None,
                        );
                    }
                    return;
                }
                "blur" => BrowserTextReport::Blur,
                _ => return,
            }
        };
        self.report(report);
    }
}

fn physical_key(
    event: &web_sys::KeyboardEvent,
    down: bool,
) -> Option<(RawInputEvent, Option<RawInputEvent>)> {
    use KeyCode as K;
    use PhysicalKeyCode as P;
    if event.is_composing() {
        return None;
    }
    let modifiers = Modifiers {
        shift: event.shift_key(),
        ctrl: event.ctrl_key(),
        alt: event.alt_key(),
        meta: event.meta_key(),
    };
    let code = event.code();
    macro_rules! named {
        ($($name:ident),* $(,)?) => { match code.as_str() {
            $(stringify!($name) => P::$name,)*
            "MetaLeft" => P::SuperLeft, "MetaRight" => P::SuperRight, _ => P::Unknown,
        }};
    }
    let physical_key = named!(
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
        ArrowLeft,
        ArrowRight,
        ArrowUp,
        ArrowDown,
        Home,
        End,
        PageUp,
        PageDown,
        Tab,
        Enter,
        NumpadEnter,
        Space,
        Delete,
        Backspace,
        Escape,
        Insert,
        ShiftLeft,
        ShiftRight,
        ControlLeft,
        ControlRight,
        AltLeft,
        AltRight,
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
    );
    let key = event.key();
    let key_code = match key.as_str() {
        "Tab" => K::Tab,
        "Enter" => K::Enter,
        "Escape" => K::Escape,
        "ArrowLeft" => K::Left,
        "ArrowRight" => K::Right,
        "ArrowUp" => K::Up,
        "ArrowDown" => K::Down,
        "Home" => K::Home,
        "End" => K::End,
        "PageUp" => K::PageUp,
        "PageDown" => K::PageDown,
        "Backspace" if physical_key != P::Unknown => K::Backspace,
        "Delete" if physical_key != P::Unknown => K::Delete,
        "Shift" => K::Shift,
        "Control" => K::Control,
        "Alt" => K::Alt,
        "Meta" => K::Meta,
        "F1" => K::F1,
        "F2" => K::F2,
        "F3" => K::F3,
        "F4" => K::F4,
        "F5" => K::F5,
        "F6" => K::F6,
        "F7" => K::F7,
        "F8" => K::F8,
        "F9" => K::F9,
        "F10" => K::F10,
        "F11" => K::F11,
        "F12" => K::F12,
        // Printable edits have one source, the real DOM input transaction.
        _ if modifiers.ctrl || modifiers.meta => {
            let scalar = key.chars().next()?;
            if key.chars().count() != 1 {
                return None;
            }
            K::Character(scalar.to_ascii_lowercase())
        }
        _ => return None,
    };
    let time = web_time::Instant::now();
    // The existing shortcut router expects the observed physical key and
    // its modifier-qualified Text half to share one ingress timestamp.
    let shortcut_text =
        (down && (modifiers.ctrl || modifiers.meta) && matches!(key_code, K::Character(_)))
            .then_some(RawInputEvent::TextInput {
                window_id: WindowId::PRIMARY,
                time,
                text: key,
                modifiers,
            });
    Some((
        RawInputEvent::Key {
            window_id: WindowId::PRIMARY,
            time,
            key_code,
            physical_key,
            modifiers,
            state: if down { KeyState::Down } else { KeyState::Up },
            repeat: event.repeat(),
            reservation: None,
        },
        shortcut_text,
    ))
}
