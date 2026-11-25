//! Dialog handle for observing modal result from calling screen.

use std::cell::RefCell;
use std::rc::Rc;

/// Internal state shared between Dialog and the calling screen via DialogHandle.
struct DialogState {
    pressed_key: Option<String>,
    is_open: bool,
}

/// Observable handle for a dialog's result.
///
/// The calling screen holds a clone of this handle. When the dialog closes
/// (via button press or dismiss), the handle reflects the outcome:
/// - `is_open()` returns false
/// - `pressed_key()` returns the key of the pressed button (if any)
#[derive(Clone)]
pub struct DialogHandle {
    inner: Rc<RefCell<DialogState>>,
}

impl DialogHandle {
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(DialogState {
                pressed_key: None,
                is_open: true,
            })),
        }
    }

    pub fn is_open(&self) -> bool {
        self.inner.borrow().is_open
    }

    pub fn pressed_key(&self) -> Option<String> {
        self.inner.borrow().pressed_key.clone()
    }

    pub(crate) fn set_pressed(&self, key: String) {
        let mut state = self.inner.borrow_mut();
        state.pressed_key = Some(key);
        state.is_open = false;
    }

    pub(crate) fn set_closed(&self) {
        self.inner.borrow_mut().is_open = false;
    }
}

impl Default for DialogHandle {
    fn default() -> Self {
        Self::new()
    }
}
