//! Browser event-loop activation before asynchronous renderer initialization.

use crate::core::event_router::browser_paste::{
    PASTE_SCRATCH_UNITS, PasteRefusal, PasteTextBuffer, PasteTextPolicy, paste_chunk_end,
};
use crate::platform::window::AppLoopEvent;
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use wasm_bindgen_futures::JsFuture;
use winit::event::{DeviceEvent, DeviceId, Event, StartCause, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop, EventLoopProxy};
use winit::platform::web::{EventLoopExtWebSys, WindowExtWebSys};
use winit::window::WindowId;

type Handler = Box<dyn FnMut(AppLoopEvent, Option<&ActiveEventLoop>)>;

mod text_input;
pub(crate) use text_input::{BrowserTextBridge, BrowserTextReport};
mod choice_picker;
pub(crate) use choice_picker::create_input_overlay;
mod history;
pub(crate) use history::{BrowserHistory, BrowserTraversal};

struct PasteListener {
    canvas: web_sys::HtmlCanvasElement,
    callback: Closure<dyn FnMut(web_sys::ClipboardEvent)>,
}

impl PasteListener {
    fn install(
        window: &winit::window::Window,
        handler: &Rc<RefCell<Option<Handler>>>,
    ) -> Result<Self, JsValue> {
        let canvas = window
            .canvas()
            .ok_or_else(|| JsValue::from_str("App canvas unavailable"))?;
        let window_id = window.id();
        let target = canvas.clone();
        let handler = Rc::downgrade(handler);
        let callback = Closure::wrap(Box::new(move |event: web_sys::ClipboardEvent| {
            let document = target.owner_document();
            let focused = document.as_ref().is_some_and(|document| {
                document.has_focus().unwrap_or(false)
                    && document.active_element().as_ref() == Some(target.as_ref())
            });
            if !event.is_trusted()
                || !event.cancelable()
                || event.default_prevented()
                || !focused
                || event.target().as_ref() != Some(target.as_ref())
            {
                return;
            }
            let Some(handler) = handler.upgrade() else {
                return;
            };
            let Ok(mut handler) = handler.try_borrow_mut() else {
                tracing::warn!("Browser Paste refused: runtime busy");
                return;
            };
            if let Some(handler) = handler.as_mut() {
                handler(
                    AppLoopEvent::Paste {
                        window: window_id,
                        event,
                    },
                    None,
                );
            }
        }) as Box<dyn FnMut(web_sys::ClipboardEvent)>);
        canvas.add_event_listener_with_callback("paste", callback.as_ref().unchecked_ref())?;
        Ok(Self { canvas, callback })
    }
}

impl Drop for PasteListener {
    fn drop(&mut self) {
        if self
            .canvas
            .remove_event_listener_with_callback("paste", self.callback.as_ref().unchecked_ref())
            .is_err()
        {
            tracing::error!("Browser Paste listener retirement failed");
        }
    }
}

/// Called only within the original trusted event, after App qualified Capture.
/// Only editor policy, representability and actual fallible intake allocation
/// bound the payload. JS getData and downstream allocations have separate owners.
pub(crate) fn read_paste_text(
    event: &web_sys::ClipboardEvent,
    policy: PasteTextPolicy,
) -> Result<String, PasteRefusal> {
    if !event.is_trusted() {
        return Err(PasteRefusal::UntrustedSource);
    }
    let data = event.clipboard_data().ok_or(PasteRefusal::Unavailable)?;
    let plain_text = JsValue::from_str("text/plain");
    if !data.types().iter().any(|kind| kind == plain_text) {
        return Err(PasteRefusal::NonText);
    }
    let get_data = js_sys::Reflect::get(data.as_ref(), &JsValue::from_str("getData"))
        .map_err(|_| PasteRefusal::Unavailable)?
        .dyn_into::<js_sys::Function>()
        .map_err(|_| PasteRefusal::Unavailable)?;
    let value = get_data
        .call1(data.as_ref(), &JsValue::from_str("text/plain"))
        .map_err(|_| PasteRefusal::Unavailable)?;
    let text = value
        .dyn_into::<js_sys::JsString>()
        .map_err(|_| PasteRefusal::NonText)?;
    if !text.is_valid_utf16() {
        return Err(PasteRefusal::InvalidUnicode);
    }
    let mut output = PasteTextBuffer::new(policy);
    let total = text.length();
    let mut start = 0;
    let mut units = [0_u16; PASTE_SCRATCH_UNITS];
    while start < total {
        let end = paste_chunk_end(start, total, |index| text.char_code_at(index) as u16);
        let chunk = text.substring(start, end);
        let count = (end - start) as usize;
        for (index, unit) in units[..count].iter_mut().enumerate() {
            *unit = chunk.char_code_at(index as u32) as u16;
        }
        output.append_utf16_chunk(&units[..count])?;
        start = end;
    }
    output.finish()
}

struct BrowserEventHandler<F>(F);

impl<F> winit::application::ApplicationHandler<()> for BrowserEventHandler<F>
where
    F: FnMut(Event<()>, &ActiveEventLoop),
{
    fn new_events(&mut self, active: &ActiveEventLoop, cause: StartCause) {
        (self.0)(Event::NewEvents(cause), active);
    }

    fn window_event(&mut self, active: &ActiveEventLoop, window_id: WindowId, event: WindowEvent) {
        (self.0)(Event::WindowEvent { window_id, event }, active);
    }

    fn device_event(&mut self, active: &ActiveEventLoop, device_id: DeviceId, event: DeviceEvent) {
        (self.0)(Event::DeviceEvent { device_id, event }, active);
    }

    fn user_event(&mut self, active: &ActiveEventLoop, event: ()) {
        (self.0)(Event::UserEvent(event), active);
    }

    fn suspended(&mut self, active: &ActiveEventLoop) {
        (self.0)(Event::Suspended, active);
    }

    fn resumed(&mut self, active: &ActiveEventLoop) {
        (self.0)(Event::Resumed, active);
    }

    fn about_to_wait(&mut self, active: &ActiveEventLoop) {
        (self.0)(Event::AboutToWait, active);
    }

    fn exiting(&mut self, active: &ActiveEventLoop) {
        (self.0)(Event::LoopExiting, active);
    }

    fn memory_warning(&mut self, active: &ActiveEventLoop) {
        (self.0)(Event::MemoryWarning, active);
    }
}

/// The bootstrap owns no UI tree. It transfers one complete runtime callback
/// after the host has supplied a real nonempty primary-window size.
pub(crate) struct BrowserAppLoop {
    handler: Rc<RefCell<Option<Handler>>>,
    text: Rc<RefCell<Option<BrowserTextBridge>>>,
    ready: Option<JsFuture>,
    proxy: EventLoopProxy<()>,
}

impl BrowserAppLoop {
    pub(crate) fn start(event_loop: EventLoop<()>, primary: WindowId) -> Self {
        let proxy = event_loop.create_proxy();
        let handler: Rc<RefCell<Option<Handler>>> = Rc::new(RefCell::new(None));
        let target = handler.clone();
        let text: Rc<RefCell<Option<BrowserTextBridge>>> = Rc::new(RefCell::new(None));
        let dispatch_text = text.clone();
        let mut resolve_size = None;
        let promise = js_sys::Promise::new(&mut |resolve, _reject| {
            resolve_size = Some(resolve);
        });
        event_loop.spawn_app(BrowserEventHandler(move |event, active: &ActiveEventLoop| {
            let mut target = target.borrow_mut();
            if let Some(handler) = target.as_mut() {
                let text = dispatch_text.borrow();
                if let Some(text) = text.as_ref() {
                    text.begin_dispatch();
                }
                handler(AppLoopEvent::Native(event), Some(active));
                drop(target);
                if let Some(text) = text.as_ref() {
                    text.finish_dispatch();
                }
            } else if matches!(&event, Event::WindowEvent {
                window_id,
                event: WindowEvent::Resized(size),
            } if *window_id == primary && size.width > 0 && size.height > 0)
            {
                if let Some(resolve) = resolve_size.take() {
                    resolve
                        .call0(&JsValue::UNDEFINED)
                        .expect("internal browser size promise must resolve");
                }
            }
        }));
        Self {
            handler,
            text,
            ready: Some(JsFuture::from(promise)),
            proxy,
        }
    }

    pub(crate) async fn wait_for_initial_size(&mut self) -> Result<(), JsValue> {
        self.ready
            .take()
            .expect("one browser activation")
            .await
            .map(|_| ())
    }

    pub(crate) fn install(
        self,
        window: &winit::window::Window,
        choice_overlay: Option<web_sys::HtmlElement>,
        mut handler: impl FnMut(AppLoopEvent, Option<&ActiveEventLoop>) + 'static,
    ) -> Result<(), JsValue> {
        let listener = PasteListener::install(window, &self.handler)?;
        let choice = choice_overlay.and_then(|overlay| {
            choice_picker::BrowserChoiceBridge::install(window, overlay, &self.handler).ok()
        });
        let text = BrowserTextBridge::install(window, &self.handler, choice)?;
        self.text.replace(Some(text.clone()));
        let previous = self.handler.replace(Some(Box::new(move |event, active| {
            let _listener_lifetime = &listener;
            let event = match event {
                AppLoopEvent::Native(event) => AppLoopEvent::BrowserNative {
                    event, text: text.clone(),
                },
                other => other,
            };
            handler(event, active);
        })));
        assert!(
            previous.is_none(),
            "browser runtime is installed exactly once"
        );
        self.proxy
            .send_event(())
            .expect("browser activation wake failed");
        Ok(())
    }
}
