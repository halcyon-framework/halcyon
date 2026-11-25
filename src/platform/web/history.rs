//! Window-scoped History API projection. NavigationHost owns the route trail.

use crate::core::app::UiWakeHandle;
use js_sys::{Array, Function, Reflect};
use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::{JsCast, JsValue, closure::Closure};
use web_sys::{EventTarget, History, PopStateEvent, Window};
use winit::platform::web::WindowExtWebSys;

const MAX_PENDING_POPS: usize = 32;
const TOKEN_LENGTH: usize = 2 + 32 + 1 + 16;

#[derive(PartialEq, Eq)]
struct NativeEntry {
    index: i32,
    key: String,
    id: String,
}

impl NativeEntry {
    fn read(entry: &JsValue) -> Option<Self> {
        let index = property(entry, "index")?.as_f64()?;
        if index < 0.0 || index > f64::from(i32::MAX) || index.fract() != 0.0 {
            return None;
        }
        Some(Self {
            index: index as i32,
            key: property(entry, "key")?
                .as_string()
                .filter(|key| !key.is_empty())?,
            id: property(entry, "id")?
                .as_string()
                .filter(|id| !id.is_empty())?,
        })
    }
}

fn property(object: &JsValue, name: &str) -> Option<JsValue> {
    Reflect::get(object, &JsValue::from_str(name)).ok()
}

fn current_entry(navigation: &JsValue) -> Option<NativeEntry> {
    NativeEntry::read(&property(navigation, "currentEntry")?)
}

fn token(value: JsValue) -> Option<String> {
    let value = value.dyn_into::<js_sys::JsString>().ok()?;
    if value.length() as usize != TOKEN_LENGTH {
        return None;
    }
    let value = value.as_string()?;
    let suffix = value.strip_prefix("h:")?;
    let (nonce, sequence) = suffix.split_once(':')?;
    (nonce.len() == 32
        && sequence.len() == 16
        && nonce
            .bytes()
            .chain(sequence.bytes())
            .all(|byte| byte.is_ascii_hexdigit()))
    .then_some(value)
}

#[derive(PartialEq, Eq)]
struct Visit {
    native: NativeEntry,
    token: String,
}

impl Visit {
    fn read(navigation: &JsValue, state: JsValue) -> Option<Self> {
        Some(Self {
            native: current_entry(navigation)?,
            token: token(state)?,
        })
    }
}

#[derive(Default)]
struct PopInbox {
    visits: Vec<Visit>,
    invalid: bool,
}

pub(crate) enum BrowserTraversal {
    None,
    Owned(usize),
    Foreign,
}

/// Owns only the correspondence between confirmed host slot numbers and
/// genuine browser entries. It never stores application route state.
pub(crate) struct BrowserHistory {
    window: Window,
    history: History,
    navigation: JsValue,
    nonce: u128,
    next: u64,
    slots: Vec<Visit>,
    inbox: Rc<RefCell<PopInbox>>,
    listener: Closure<dyn FnMut(PopStateEvent)>,
}

impl BrowserHistory {
    pub(crate) fn install(window: &winit::window::Window, wake: UiWakeHandle) -> Option<Self> {
        let window = window.canvas()?.owner_document()?.default_view()?;
        if window.top().ok().flatten().as_ref() != Some(&window) {
            return None;
        }
        let history = window.history().ok()?;
        // Classic history state is writable by scripts. The browser-issued
        // entry identity independently disqualifies a copied state token.
        let navigation = property(window.as_ref(), "navigation")?;
        current_entry(&navigation)?;
        let mut random = [0; 16];
        window
            .crypto()
            .ok()?
            .get_random_values_with_u8_array(&mut random)
            .ok()?;
        let inbox = Rc::new(RefCell::new(PopInbox::default()));
        let weak = Rc::downgrade(&inbox);
        let source: EventTarget = window.clone().into();
        let event_navigation = navigation.clone();
        let listener = Closure::<dyn FnMut(PopStateEvent)>::new(move |event: PopStateEvent| {
            if !event.is_trusted()
                || event.target().as_ref() != Some(&source)
                || event.current_target().as_ref() != Some(&source)
            {
                return;
            }
            let Some(inbox) = weak.upgrade() else {
                return;
            };
            let mut inbox = inbox.borrow_mut();
            if !inbox.invalid {
                match Visit::read(&event_navigation, event.state()) {
                    Some(visit) if inbox.visits.len() < MAX_PENDING_POPS => {
                        inbox.visits.push(visit)
                    }
                    _ => {
                        inbox.invalid = true;
                        inbox.visits.clear();
                    }
                }
            }
            drop(inbox);
            // Only the ordinary App event loop may act on this observation.
            let _ = wake.wake();
        });
        window
            .add_event_listener_with_callback("popstate", listener.as_ref().unchecked_ref())
            .ok()?;
        Some(Self {
            window,
            history,
            navigation,
            nonce: u128::from_ne_bytes(random),
            next: 0,
            slots: Vec::new(),
            inbox,
            listener,
        })
    }

    pub(crate) fn path(&self) -> Option<String> {
        let location = self.window.location();
        if !location.search().ok()?.is_empty() || !location.hash().ok()?.is_empty() {
            return None;
        }
        location.pathname().ok()
    }

    pub(crate) fn can_bind_initial(&self) -> bool {
        self.history
            .state()
            .is_ok_and(|state| state.is_null() || state.is_undefined() || token(state).is_some())
    }

    pub(crate) fn initialize(&mut self, path: &str) -> bool {
        self.slots.is_empty() && self.can_bind_initial() && self.publish(path, None)
    }

    pub(crate) fn append(&mut self, cursor: usize, path: &str) -> bool {
        self.current_slot() == Some(cursor) && self.publish(path, Some(cursor))
    }

    fn publish(&mut self, path: &str, after: Option<usize>) -> bool {
        let Some(initial) = current_entry(&self.navigation) else {
            return false;
        };
        let Some(index) = initial.index.checked_add(i32::from(after.is_some())) else {
            return false;
        };
        let Some(sequence) = self.next.checked_add(1) else {
            return false;
        };
        self.next = sequence;
        let identifier = format!("h:{:032x}:{sequence:016x}", self.nonce);
        let state = JsValue::from_str(&identifier);
        let result = if after.is_some() {
            self.history.push_state_with_url(&state, "", Some(path))
        } else {
            self.history.replace_state_with_url(&state, "", Some(path))
        };
        if result.is_err() || self.path().as_deref() != Some(path) {
            return false;
        }
        let Some(visit) = self.current_visit() else {
            return false;
        };
        if visit.token != identifier || visit.native.index != index {
            return false;
        }
        if let Some(cursor) = after {
            self.slots.truncate(cursor + 1);
        }
        self.slots.push(visit);
        true
    }

    fn current_visit(&self) -> Option<Visit> {
        Visit::read(&self.navigation, self.history.state().ok()?)
    }

    fn slot(&self, visit: &Visit) -> Option<usize> {
        let first_index = self.slots.first()?.native.index;
        let index = usize::try_from(visit.native.index.checked_sub(first_index)?).ok()?;
        (self.slots.get(index)? == visit).then_some(index)
    }

    pub(crate) fn current_slot(&self) -> Option<usize> {
        self.slot(&self.current_visit()?)
    }

    pub(crate) fn take_traversal(&self) -> BrowserTraversal {
        let inbox = std::mem::take(&mut *self.inbox.borrow_mut());
        if inbox.invalid || inbox.visits.iter().any(|visit| self.slot(visit).is_none()) {
            return BrowserTraversal::Foreign;
        }
        match inbox.visits.last() {
            None => BrowserTraversal::None,
            Some(visit) if self.current_visit().as_ref() == Some(visit) => match self.slot(visit) {
                Some(slot) => BrowserTraversal::Owned(slot),
                None => BrowserTraversal::Foreign,
            },
            Some(_) => BrowserTraversal::Foreign,
        }
    }

    pub(crate) fn traverse(&self, from: usize, to: usize) -> bool {
        let (Some(from), Some(to)) = (self.slots.get(from), self.slots.get(to)) else {
            return false;
        };
        if self.current_visit().as_ref() != Some(from) {
            return false;
        }
        let Some(entries) = property(&self.navigation, "entries")
            .and_then(|function| function.dyn_into::<Function>().ok())
            .and_then(|function| function.call0(&self.navigation).ok())
            .and_then(|value| value.dyn_into::<Array>().ok())
        else {
            return false;
        };
        if NativeEntry::read(&entries.get(to.native.index as u32)).as_ref() != Some(&to.native) {
            return false;
        }
        let delta = to.native.index - from.native.index;
        delta != 0 && self.history.go_with_delta(delta).is_ok()
    }
}

impl Drop for BrowserHistory {
    fn drop(&mut self) {
        let _ = self.window.remove_event_listener_with_callback(
            "popstate",
            self.listener.as_ref().unchecked_ref(),
        );
    }
}
