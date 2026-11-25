//! HAL-400 browser transport. The UA receives the original SELECT gesture.

use super::text_input::Listener;
use super::{AppLoopEvent, Handler};
use crate::core::event_router::host_choice::{
    ChoiceLease, ChoicePlacement, ChoiceReply, ChoiceRequest, ChoiceTarget, HostOffer,
    choice_options,
};
use crate::core::math::RectF;
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::model::selection_resolver::OptionId;
use std::cell::RefCell;
use std::collections::{HashMap, VecDeque};
use std::rc::{Rc, Weak};
use std::time::Duration;
use wasm_bindgen::{JsCast, JsValue};
use web_sys::{
    Event, HtmlCanvasElement, HtmlElement, HtmlOptionElement, HtmlSelectElement, PointerEvent,
};
use web_time::Instant;
use winit::platform::web::WindowExtWebSys;

pub(crate) fn create_input_overlay() -> Option<HtmlElement> {
    let window = web_sys::window()?;
    if window.navigator().max_touch_points() == 0 {
        return None;
    }
    let overlay = window
        .document()?
        .create_element("div")
        .ok()?
        .dyn_into::<HtmlElement>()
        .ok()?;
    overlay
        .style()
        .set_css_text("position:fixed;inset:0;pointer-events:none;z-index:1;touch-action:none");
    Some(overlay)
}

struct Selector {
    placement: ChoicePlacement,
    element: HtmlSelectElement,
    selected: Option<OptionId>,
    visible: bool,
    used: bool,
}

#[derive(Default)]
struct Selectors {
    nodes: Vec<Selector>,
    contacts: HashMap<i32, HtmlSelectElement>,
    trusted_up: Option<(PointerEvent, ChoiceTarget)>,
    active: Option<(HtmlSelectElement, ChoiceLease)>,
    activation: Option<HtmlSelectElement>,
    input_sample: Option<SelectInputSample>,
}

struct SelectInputSample {
    event: PointerEvent,
    phase: winit::event::TouchPhase,
    times: VecDeque<Instant>,
}

struct State {
    overlay: HtmlElement,
    canvas: HtmlCanvasElement,
    window: winit::window::WindowId,
    handler: Weak<RefCell<Option<Handler>>>,
    selectors: RefCell<Selectors>,
    listeners: RefCell<Vec<Listener>>,
}

#[derive(Clone)]
pub(crate) struct BrowserChoiceBridge(Rc<State>);

impl BrowserChoiceBridge {
    pub(super) fn install(
        window: &winit::window::Window,
        overlay: HtmlElement,
        handler: &Rc<RefCell<Option<Handler>>>,
    ) -> Result<Self, JsValue> {
        let canvas = window
            .canvas()
            .ok_or_else(|| JsValue::from_str("Missing choice canvas"))?;
        let state = Rc::new(State {
            overlay,
            canvas,
            window: window.id(),
            handler: Rc::downgrade(handler),
            selectors: RefCell::default(),
            listeners: RefCell::default(),
        });
        for kind in [
            "pointerdown",
            "pointermove",
            "pointerup",
            "pointercancel",
            "mousedown",
            "click",
            "contextmenu",
            "change",
            "blur",
        ] {
            let weak = Rc::downgrade(&state);
            state.listeners.borrow_mut().push(Listener::install(
                state.overlay.as_ref(),
                kind,
                true,
                move |event| {
                    if let Some(state) = weak.upgrade() {
                        Self(state).event(kind, event);
                    }
                },
            )?);
        }
        state
            .canvas
            .parent_node()
            .ok_or_else(|| JsValue::from_str("Detached choice canvas"))?
            .append_child(&state.overlay)?;
        Ok(Self(state))
    }

    fn event(&self, kind: &str, event: Event) {
        let Some(element) = event
            .target()
            .and_then(|target| target.dyn_into::<HtmlSelectElement>().ok())
        else {
            return;
        };
        if !event.is_trusted() {
            if matches!(kind, "mousedown" | "click") {
                event.prevent_default();
            }
            return;
        }
        let Ok(mut selectors) = self.0.selectors.try_borrow_mut() else {
            return;
        };
        if let Some(pointer) = event.dyn_ref::<PointerEvent>()
            && pointer.pointer_type() == "touch"
        {
            let phase = match kind {
                "pointerdown" => Some(winit::event::TouchPhase::Started),
                "pointermove" => Some(winit::event::TouchPhase::Moved),
                "pointerup" => Some(winit::event::TouchPhase::Ended),
                "pointercancel" => Some(winit::event::TouchPhase::Cancelled),
                _ => None,
            };
            if let Some(phase) = phase {
                selectors.input_sample = self.input_sample(pointer, phase);
            }
        }
        match kind {
            "pointerdown" => {
                selectors.trusted_up = None;
                selectors.activation = None;
                if let Some(pointer) = event.dyn_ref::<PointerEvent>()
                    && pointer.pointer_type() == "touch"
                    && selectors
                        .nodes
                        .iter()
                        .any(|node| node.visible && !node.used && node.element == element)
                {
                    selectors
                        .contacts
                        .insert(pointer.pointer_id(), element.clone());
                    let _ = element.set_pointer_capture(pointer.pointer_id());
                }
            }
            "pointerup" | "pointercancel" => {
                let Some(pointer) = event.dyn_ref::<PointerEvent>() else {
                    return;
                };
                if selectors.contacts.remove(&pointer.pointer_id()).as_ref() != Some(&element) {
                    return;
                }
                selectors.activation = None;
                if kind == "pointerup" {
                    selectors.trusted_up = selectors
                        .nodes
                        .iter()
                        .find(|node| node.visible && node.element == element)
                        .map(|node| (pointer.clone(), node.placement.target.clone()));
                }
            }
            "mousedown" | "click" => {
                if selectors.activation.as_ref() != Some(&element)
                    || event.dyn_ref::<PointerEvent>().is_some_and(|pointer| {
                        matches!(pointer.pointer_type().as_str(), "mouse" | "pen")
                    })
                {
                    event.prevent_default();
                }
                if kind == "click" {
                    selectors.activation = None;
                }
            }
            "contextmenu" => event.prevent_default(),
            "change" | "blur" => {
                if !selectors
                    .active
                    .as_ref()
                    .is_some_and(|(target, _)| target == &element)
                {
                    return;
                }
                let (_, lease) = selectors.active.take().unwrap();
                let selection = (kind == "change")
                    .then(|| element.value().parse().ok().map(OptionId::from_raw))
                    .flatten();
                selectors.activation = None;
                if let Some(node) = selectors.nodes.iter().find(|node| node.element == element) {
                    set_selected(&element, node.selected);
                }
                let _ = element.style().set_property("pointer-events", "none");
                drop(selectors);
                self.emit(AppLoopEvent::BrowserChoice(ChoiceReply {
                    lease,
                    selection,
                }));
                if kind == "change" {
                    self.release_selector_focus(&element);
                }
            }
            _ => {}
        }
    }

    fn input_sample(
        &self,
        pointer: &PointerEvent,
        phase: winit::event::TouchPhase,
    ) -> Option<SelectInputSample> {
        let performance = self
            .0
            .canvas
            .owner_document()?
            .default_view()?
            .performance()?;
        let now = performance.now();
        let ingress = Instant::now();
        let instant = |stamp: f64| {
            let age = Duration::try_from_secs_f64((now - stamp) * 0.001).ok()?;
            ingress.checked_sub(age)
        };
        let mut times = VecDeque::new();
        let coalesces = phase == winit::event::TouchPhase::Moved
            && js_sys::Reflect::get(pointer.as_ref(), &JsValue::from_str("getCoalescedEvents"))
                .is_ok_and(|method| method.is_function());
        if coalesces {
            for event in pointer.get_coalesced_events().iter() {
                let event: PointerEvent = event.dyn_into().ok()?;
                times.push_back(instant(event.time_stamp())?);
            }
        } else {
            times.push_back(instant(pointer.time_stamp())?);
        }
        Some(SelectInputSample {
            event: pointer.clone(),
            phase,
            times,
        })
    }

    pub(crate) fn take_input_time(&self, touch: &winit::event::Touch) -> Option<Instant> {
        let mut selectors = self.0.selectors.borrow_mut();
        let sample = selectors.input_sample.as_mut()?;
        if sample.phase != touch.phase
            || sample.event.pointer_id() as u64 != touch.id
            || sample.event.event_phase() == 0
        {
            return None;
        }
        sample.times.pop_front()
    }

    fn emit(&self, event: AppLoopEvent) {
        if let Some(handler) = self.0.handler.upgrade()
            && let Ok(mut handler) = handler.try_borrow_mut()
            && let Some(handler) = handler.as_mut()
        {
            handler(event, None);
        }
    }

    fn release_selector_focus(&self, element: &HtmlSelectElement) {
        let Some(document) = element.owner_document() else {
            return;
        };
        if document.active_element().as_ref() != Some(element.as_ref()) {
            return;
        }
        if document.has_focus().unwrap_or(false) {
            let options = web_sys::FocusOptions::new();
            options.set_prevent_scroll(true);
            if self.0.canvas.focus_with_options(&options).is_ok() {
                return;
            }
        }
        let _ = element.blur();
    }

    pub(crate) fn offer(&self, window: winit::window::WindowId, contact: u64) -> Option<HostOffer> {
        if self.0.window != window {
            return None;
        }
        let selectors = self.0.selectors.borrow();
        let (event, target) = selectors.trusted_up.as_ref()?;
        (event.event_phase() != 0 && event.pointer_id() as u64 == contact).then(|| {
            HostOffer::RenderedSelect {
                contact,
                target: target.clone(),
            }
        })
    }

    /// Keep the actual DOM activation target and let the UA perform the click.
    pub(crate) fn arm(&self, request: &ChoiceRequest) -> bool {
        let mut selectors = self.0.selectors.borrow_mut();
        let Some((event, target)) = selectors.trusted_up.take() else {
            return false;
        };
        if event.event_phase() == 0 || target != request.projected {
            return false;
        }
        let Some(node) = selectors
            .nodes
            .iter_mut()
            .find(|node| !node.used && node.visible && node.placement.target == target)
        else {
            return false;
        };
        if node.element.length() as usize != request.items.len()
            || node.selected != request.selected
        {
            return false;
        }
        node.used = true;
        // The engine may have retired this control's own Halcyon popup.
        // Keep the actual trusted SELECT target for the UA's ensuing click.
        node.placement.target = request.lease.target.clone();
        let element = node.element.clone();
        selectors.activation = Some(element.clone());
        selectors.active = Some((element, request.lease.clone()));
        true
    }

    pub(super) fn geometry_changed(&self) {
        self.retain(None);
        self.emit(AppLoopEvent::BrowserChoiceGeometry(self.0.window));
    }

    pub(crate) fn has_focus(&self) -> bool {
        self.0
            .selectors
            .borrow()
            .active
            .as_ref()
            .is_some_and(|(element, _)| {
                element.owner_document().is_some_and(|doc| {
                    doc.has_focus().unwrap_or(false)
                        && doc.active_element().as_ref() == Some(element.as_ref())
                })
            })
    }

    pub(crate) fn retain(&self, live: Option<&ChoiceLease>) {
        let retired = {
            let mut selectors = self.0.selectors.borrow_mut();
            if selectors
                .active
                .as_ref()
                .is_some_and(|(_, lease)| live != Some(lease))
            {
                selectors.activation = None;
                selectors.active.take().map(|(element, _)| element)
            } else {
                None
            }
        };
        if let Some(element) = retired {
            let _ = element.style().set_property("pointer-events", "none");
            self.release_selector_focus(&element);
        }
    }

    pub(crate) fn synchronize(&self, tree: &WidgetTree, root: NodeId, viewport: RectF) {
        let Some(basis) = CanvasBasis::new(&self.0.canvas, viewport) else {
            self.project(tree, Vec::new(), None);
            return;
        };
        self.project(
            tree,
            tree.host_choice_placements(root, basis.visible),
            Some(basis),
        );
    }

    pub(crate) fn clear(&self, tree: &WidgetTree) {
        self.retain(None);
        self.project(tree, Vec::new(), None);
    }

    fn project(
        &self,
        tree: &WidgetTree,
        placements: Vec<ChoicePlacement>,
        basis: Option<CanvasBasis>,
    ) {
        let mut selectors = self.0.selectors.borrow_mut();
        for node in &mut selectors.nodes {
            node.visible = false;
        }
        if let Some(basis) = basis {
            let active_element = selectors
                .active
                .as_ref()
                .map(|(element, _)| element.clone());
            self.0
                .overlay
                .style()
                .set_property(
                    "cursor",
                    &self
                        .0
                        .canvas
                        .style()
                        .get_property_value("cursor")
                        .unwrap_or_default(),
                )
                .ok();
            for placement in placements {
                let Some(options) = choice_options(tree, &placement.target.mount) else {
                    continue;
                };
                if let Some(node) = selectors.nodes.iter_mut().find(|node| {
                    node.placement.target == placement.target
                        && (!node.used || active_element.as_ref() == Some(&node.element))
                }) {
                    if node.selected != options.selected {
                        set_selected(&node.element, options.selected);
                        node.selected = options.selected;
                    }
                    project_rect(&node.element, &basis, &placement);
                    node.placement = placement;
                    node.visible = true;
                } else if let Ok(element) = self.make_selector(
                    options.items,
                    options.groups.map(|groups| groups.as_ref()),
                    options.selected,
                ) {
                    project_rect(&element, &basis, &placement);
                    selectors.nodes.push(Selector {
                        placement,
                        element,
                        selected: options.selected,
                        visible: true,
                        used: false,
                    });
                }
            }
        }
        let Selectors {
            nodes,
            contacts,
            active,
            ..
        } = &mut *selectors;
        nodes.retain(|node| {
            if node.visible {
                return true;
            }
            if contacts.values().any(|element| *element == node.element)
                || active
                    .as_ref()
                    .is_some_and(|(element, _)| *element == node.element)
            {
                let _ = node.element.style().set_property("pointer-events", "none");
                return true;
            }
            node.element.remove();
            false
        });
    }

    fn make_selector(
        &self,
        items: &[(OptionId, String)],
        groups: Option<&crate::widgets::combo_box::ChoiceGroups>,
        selected: Option<OptionId>,
    ) -> Result<HtmlSelectElement, JsValue> {
        let document = self
            .0
            .canvas
            .owner_document()
            .ok_or_else(|| JsValue::from_str("Detached choice canvas"))?;
        let element = document
            .create_element("select")?
            .dyn_into::<HtmlSelectElement>()?;
        element.set_tab_index(-1);
        let append_options = |parent: &web_sys::Element, items: &[(OptionId, String)]| {
            for (id, label) in items {
                let option = document
                    .create_element("option")?
                    .dyn_into::<HtmlOptionElement>()?;
                option.set_value(&id.raw().to_string());
                option.set_text_content(Some(label));
                parent.append_child(&option)?;
            }
            Ok::<(), JsValue>(())
        };
        if let Some(groups) = groups {
            let boundaries = groups.boundaries();
            for (group_index, (start, title)) in boundaries.iter().enumerate() {
                let end = boundaries
                    .get(group_index + 1)
                    .map_or(items.len(), |(start, _)| *start);
                let group = document.create_element("optgroup")?;
                group.set_attribute("label", title)?;
                append_options(&group, &items[*start..end])?;
                element.append_child(&group)?;
            }
        } else {
            append_options(&element, items)?;
        }
        set_selected(&element, selected);
        self.0.overlay.append_child(&element)?;
        Ok(element)
    }
}

impl Drop for State {
    fn drop(&mut self) {
        self.listeners.get_mut().clear();
        self.overlay.remove();
    }
}

fn set_selected(element: &HtmlSelectElement, selected: Option<OptionId>) {
    if let Some(id) = selected {
        element.set_value(&id.raw().to_string());
    } else {
        element.set_selected_index(-1);
    }
}

struct CanvasBasis {
    bounds: web_sys::DomRect,
    x_scale: f64,
    y_scale: f64,
    visible: RectF,
}

impl CanvasBasis {
    fn new(canvas: &HtmlCanvasElement, viewport: RectF) -> Option<Self> {
        let bounds = canvas.get_bounding_client_rect();
        let x_scale = bounds.width() / f64::from(viewport.width);
        let y_scale = bounds.height() / f64::from(viewport.height);
        if !(x_scale.is_finite() && x_scale > 0.0 && y_scale.is_finite() && y_scale > 0.0) {
            return None;
        }
        let window = canvas.owner_document()?.default_view()?;
        let visible = window.visual_viewport().map_or(viewport, |visual| {
            RectF::new(
                ((visual.offset_left() - bounds.x()) / x_scale) as f32,
                ((visual.offset_top() - bounds.y()) / y_scale) as f32,
                (visual.width() / x_scale) as f32,
                (visual.height() / y_scale) as f32,
            )
            .intersection(&viewport)
            .unwrap_or(RectF::zero())
        });
        Some(Self {
            bounds,
            x_scale,
            y_scale,
            visible,
        })
    }
}

fn project_rect(element: &HtmlSelectElement, basis: &CanvasBasis, placement: &ChoicePlacement) {
    let rect = placement.rect;
    let x = basis.bounds.x() + f64::from(rect.x) * basis.x_scale;
    let y = basis.bounds.y() + f64::from(rect.y) * basis.y_scale;
    let width = f64::from(rect.width) * basis.x_scale;
    let height = f64::from(rect.height) * basis.y_scale;
    let mut clip = String::new();
    for part in &placement.exposed {
        use std::fmt::Write;
        let left = f64::from(part.x - rect.x) * basis.x_scale;
        let top = f64::from(part.y - rect.y) * basis.y_scale;
        let right = left + f64::from(part.width) * basis.x_scale;
        let bottom = top + f64::from(part.height) * basis.y_scale;
        let _ = write!(clip, "M {left} {top} H {right} V {bottom} H {left} Z ");
    }
    element.style().set_css_text(&format!(
        "position:fixed;left:{x}px;top:{y}px;width:{width}px;height:{height}px;\
         margin:0;opacity:0;pointer-events:auto;touch-action:none;cursor:inherit;\
         font-size:16px;appearance:auto;clip-path:path('{clip}')",
    ));
}
