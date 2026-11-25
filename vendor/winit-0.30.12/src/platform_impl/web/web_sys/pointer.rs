// Modified for Halcyon; see HALCYON-PATCH.md in this vendored crate.
use std::cell::Cell;
use std::rc::Rc;

use super::canvas::{Common, InputListeners};
use super::event;
use crate::dpi::PhysicalPosition;
use crate::event::{Force, MouseButton};
use crate::keyboard::ModifiersState;

use event::ButtonsState;
use wasm_bindgen::{JsCast, prelude::wasm_bindgen};
use web_sys::{HtmlCanvasElement, HtmlElement, Node, PointerEvent};

#[allow(dead_code)]
pub(super) struct PointerHandler {
    on_cursor_leave: Option<InputListeners<PointerEvent>>,
    on_cursor_enter: Option<InputListeners<PointerEvent>>,
    on_cursor_move: Option<InputListeners<PointerEvent>>,
    on_pointer_press: Option<InputListeners<PointerEvent>>,
    on_pointer_release: Option<InputListeners<PointerEvent>>,
    on_touch_cancel: Option<InputListeners<PointerEvent>>,
}

fn from_canvas(event: &PointerEvent, canvas: &HtmlCanvasElement) -> bool {
    event.target().as_ref() == Some(canvas.as_ref())
}

fn position(
    event: &PointerEvent,
    canvas: &HtmlCanvasElement,
    has_input_overlay: bool,
    scale: f64,
) -> PhysicalPosition<f64> {
    if !has_input_overlay {
        return event::mouse_position(event).to_physical(scale);
    }
    #[wasm_bindgen]
    extern "C" {
        type ClientPosition;
        #[wasm_bindgen(method, getter, js_name = clientX)]
        fn client_x(this: &ClientPosition) -> f64;
        #[wasm_bindgen(method, getter, js_name = clientY)]
        fn client_y(this: &ClientPosition) -> f64;
    }
    let client: &ClientPosition = event.unchecked_ref();
    let rect = canvas.get_bounding_client_rect();
    // Both targets describe the same canvas pixel even when CSS and backing
    // store density differ or a pointer moves from the overlay to the canvas.
    PhysicalPosition::new(
        (client.client_x() - rect.x()) * f64::from(canvas.width())
            / rect.width().max(f64::EPSILON),
        (client.client_y() - rect.y()) * f64::from(canvas.height())
            / rect.height().max(f64::EPSILON),
    )
}

fn inside_input_surface(
    event: &PointerEvent,
    canvas: &HtmlCanvasElement,
    overlay: Option<&HtmlElement>,
) -> bool {
    let Some(overlay) = overlay else { return false; };
    event.related_target().is_some_and(|other| {
        other == *canvas.as_ref()
            || other.dyn_ref::<Node>().is_some_and(|node| overlay.contains(Some(node)))
    })
}

impl PointerHandler {
    pub fn new() -> Self {
        Self {
            on_cursor_leave: None,
            on_cursor_enter: None,
            on_cursor_move: None,
            on_pointer_press: None,
            on_pointer_release: None,
            on_touch_cancel: None,
        }
    }

    pub fn on_cursor_leave<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<i32>),
    {
        let canvas = canvas_common.raw().clone();
        let overlay = canvas_common.input_overlay().cloned();
        self.on_cursor_leave =
            Some(canvas_common.add_input_event("pointerout", move |event: PointerEvent| {
                if inside_input_surface(&event, &canvas, overlay.as_ref()) {
                    return;
                }
                let modifiers = event::mouse_modifiers(&event);

                // touch events are handled separately
                // handling them here would produce duplicate mouse events, inconsistent with
                // other platforms.
                let pointer_id = (event.pointer_type() != "touch").then(|| event.pointer_id());

                handler(modifiers, pointer_id);
            }));
    }

    pub fn on_cursor_enter<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(ModifiersState, Option<i32>),
    {
        let canvas = canvas_common.raw().clone();
        let overlay = canvas_common.input_overlay().cloned();
        self.on_cursor_enter =
            Some(canvas_common.add_input_event("pointerover", move |event: PointerEvent| {
                if inside_input_surface(&event, &canvas, overlay.as_ref()) {
                    return;
                }
                let modifiers = event::mouse_modifiers(&event);

                // touch events are handled separately
                // handling them here would produce duplicate mouse events, inconsistent with
                // other platforms.
                let pointer_id = (event.pointer_type() != "touch").then(|| event.pointer_id());

                handler(modifiers, pointer_id);
            }));
    }

    pub fn on_mouse_release<M, T>(
        &mut self,
        canvas_common: &Common,
        mut mouse_handler: M,
        mut touch_handler: T,
    ) where
        M: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        let has_input_overlay = canvas_common.input_overlay().is_some();
        self.on_pointer_release =
            Some(canvas_common.add_input_event("pointerup", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let scale = super::scale_factor(&window);

                match event.pointer_type().as_str() {
                    "touch" => touch_handler(
                        modifiers,
                        event.pointer_id(),
                        position(&event, &canvas, has_input_overlay, scale),
                        Force::Normalized(event.pressure() as f64),
                    ),
                    _ => mouse_handler(
                        modifiers,
                        event.pointer_id(),
                        position(&event, &canvas, has_input_overlay, scale),
                        event::mouse_button(&event).expect("no mouse button released"),
                    ),
                }
            }));
    }

    pub fn on_mouse_press<M, T>(
        &mut self,
        canvas_common: &Common,
        mut mouse_handler: M,
        mut touch_handler: T,
        prevent_default: Rc<Cell<bool>>,
    ) where
        M: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, MouseButton),
        T: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        let has_input_overlay = canvas_common.input_overlay().is_some();
        self.on_pointer_press =
            Some(canvas_common.add_input_event("pointerdown", move |event: PointerEvent| {
                let pointer_type = event.pointer_type();
                let on_canvas = from_canvas(&event, &canvas);
                if prevent_default.get() && (pointer_type != "touch" || on_canvas) {
                    // prevent text selection
                    event.prevent_default();
                    // but still focus element
                    let _ = canvas.focus();
                }

                let modifiers = event::mouse_modifiers(&event);
                let scale = super::scale_factor(&window);
                match pointer_type.as_str() {
                    "touch" => {
                        touch_handler(
                            modifiers,
                            event.pointer_id(),
                            position(&event, &canvas, has_input_overlay, scale),
                            Force::Normalized(event.pressure() as f64),
                        );
                    },
                    _ => {
                        if !on_canvas {
                            let _ = canvas.set_pointer_capture(event.pointer_id());
                        }
                        mouse_handler(
                            modifiers,
                            event.pointer_id(),
                            position(&event, &canvas, has_input_overlay, scale),
                            event::mouse_button(&event).expect("no mouse button pressed"),
                        );

                        if pointer_type == "mouse" && on_canvas {
                            // Error is swallowed here since the error would occur every time the
                            // mouse is clicked when the cursor is
                            // grabbed, and there is probably not a
                            // situation where this could fail, that we
                            // care if it fails.
                            let _e = canvas.set_pointer_capture(event.pointer_id());
                        }
                    },
                }
            }));
    }

    pub fn on_cursor_move<M, T, B>(
        &mut self,
        canvas_common: &Common,
        mut mouse_handler: M,
        mut touch_handler: T,
        mut button_handler: B,
        prevent_default: Rc<Cell<bool>>,
    ) where
        M: 'static + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = PhysicalPosition<f64>>),
        T: 'static
            + FnMut(ModifiersState, i32, &mut dyn Iterator<Item = (PhysicalPosition<f64>, Force)>),
        B: 'static + FnMut(ModifiersState, i32, PhysicalPosition<f64>, ButtonsState, MouseButton),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        let has_input_overlay = canvas_common.input_overlay().is_some();
        self.on_cursor_move =
            Some(canvas_common.add_input_event("pointermove", move |event: PointerEvent| {
                let modifiers = event::mouse_modifiers(&event);
                let scale = super::scale_factor(&window);

                let id = event.pointer_id();

                // chorded button event
                if let Some(button) = event::mouse_button(&event) {
                    if prevent_default.get() {
                        // prevent text selection
                        event.prevent_default();
                        // but still focus element
                        let _ = canvas.focus();
                    }

                    button_handler(
                        modifiers,
                        id,
                        position(&event, &canvas, has_input_overlay, scale),
                        event::mouse_buttons(&event),
                        button,
                    );

                    return;
                }

                // pointer move event
                match event.pointer_type().as_str() {
                    "touch" => touch_handler(
                        modifiers,
                        id,
                        &mut event::pointer_move_event(event).map(|event| {
                            (
                                position(&event, &canvas, has_input_overlay, scale),
                                Force::Normalized(event.pressure() as f64),
                            )
                        }),
                    ),
                    _ => mouse_handler(
                        modifiers,
                        id,
                        &mut event::pointer_move_event(event)
                            .map(|event| position(&event, &canvas, has_input_overlay, scale)),
                    ),
                };
            }));
    }

    pub fn on_touch_cancel<F>(&mut self, canvas_common: &Common, mut handler: F)
    where
        F: 'static + FnMut(i32, PhysicalPosition<f64>, Force),
    {
        let window = canvas_common.window.clone();
        let canvas = canvas_common.raw().clone();
        let has_input_overlay = canvas_common.input_overlay().is_some();
        self.on_touch_cancel =
            Some(canvas_common.add_input_event("pointercancel", move |event: PointerEvent| {
                if event.pointer_type() == "touch" {
                    handler(
                        event.pointer_id(),
                        position(&event, &canvas, has_input_overlay, super::scale_factor(&window)),
                        Force::Normalized(event.pressure() as f64),
                    );
                }
            }));
    }

    pub fn remove_listeners(&mut self) {
        self.on_cursor_leave = None;
        self.on_cursor_enter = None;
        self.on_cursor_move = None;
        self.on_pointer_press = None;
        self.on_pointer_release = None;
        self.on_touch_cancel = None;
    }
}
