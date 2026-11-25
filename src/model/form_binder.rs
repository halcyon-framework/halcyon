//! SPEC-HAL-207: Application-layer value binding.
//!
//! FormBinder mediates between widget-produced changes and application model fields.
//! Widgets write via ChangeWriter; application code reads via FormBinder::bind().
//!
//! All binding is:
//! - explicit (string-keyed, typed accessors)
//! - exactly-once per sync pass (dirty flag cleared on read)
//! - lock-free at the API surface (no guards exposed)
//! - lifetime-enforced (handles cannot escape the bind closure)
//!
//! # Compile-time safety
//!
//! Binding handles are lifetime-tied to the `bind()` closure and cannot be stored:
//!
//! ```compile_fail
//! use halcyon::model::form_binder::FormBinder;
//! use halcyon::core::app::ScreenContext;
//!
//! fn steal(binder: &mut FormBinder, cx: &ScreenContext<'_>) {
//!     let mut stolen = None;
//!     binder.bind(cx, |b| {
//!         stolen = Some(b.bool("x"));
//!     });
//! }
//! ```
//!
//! BindingContext itself cannot escape the closure either:
//!
//! ```compile_fail
//! use halcyon::model::form_binder::FormBinder;
//! use halcyon::core::app::ScreenContext;
//!
//! fn steal_ctx(binder: &mut FormBinder, cx: &ScreenContext<'_>) {
//!     let mut ctx_ref = None;
//!     binder.bind(cx, |b| {
//!         ctx_ref = Some(b);
//!     });
//! }
//! ```

use crate::core::app::ScreenContext;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

// ============================================================================
// BindingKey
// ============================================================================

/// Interned identifier for a binding slot. Opaque to application code.
#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash)]
pub struct BindingKey(u32);

// ============================================================================
// Internal Slot Types
// ============================================================================

struct BoolSlot {
    value: bool,
    dirty: bool,
}

struct SelectionSlot {
    value: Option<u32>,
    dirty: bool,
}

struct TextSlot {
    value: String,
    dirty: bool,
}

struct F32Slot {
    value: f32,
    dirty: bool,
}

// ============================================================================
// ChangeStoreInner
// ============================================================================

struct ChangeStoreInner {
    bool_slots: HashMap<BindingKey, BoolSlot>,
    selection_slots: HashMap<BindingKey, SelectionSlot>,
    text_slots: HashMap<BindingKey, TextSlot>,
    f32_slots: HashMap<BindingKey, F32Slot>,
    intern_map: HashMap<String, BindingKey>,
    next_id: u32,
}

impl ChangeStoreInner {
    fn new() -> Self {
        Self {
            bool_slots: HashMap::new(),
            selection_slots: HashMap::new(),
            text_slots: HashMap::new(),
            f32_slots: HashMap::new(),
            intern_map: HashMap::new(),
            next_id: 0,
        }
    }

    fn intern(&mut self, key: &str) -> BindingKey {
        if let Some(&existing) = self.intern_map.get(key) {
            return existing;
        }
        let id = BindingKey(self.next_id);
        self.next_id += 1;
        self.intern_map.insert(key.to_owned(), id);
        id
    }
}

// ============================================================================
// ChangeWriter
// ============================================================================

/// Write-side handle to the change store. Cloneable. Given to widgets for emission.
///
/// All writes mark the target slot dirty. Final value wins if written multiple times per frame.
#[derive(Clone)]
pub struct ChangeWriter {
    inner: Rc<RefCell<ChangeStoreInner>>,
}

impl ChangeWriter {
    /// Intern a string key into a BindingKey. Idempotent.
    pub fn intern(&self, key: &str) -> BindingKey {
        self.inner.borrow_mut().intern(key)
    }

    /// Write a bool change. Marks slot dirty.
    pub fn set_bool(&self, key: BindingKey, value: bool) {
        let mut store = self.inner.borrow_mut();
        let slot = store.bool_slots.entry(key).or_insert(BoolSlot {
            value: false,
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }

    /// Write a selection change. Marks slot dirty.
    pub fn set_selection(&self, key: BindingKey, value: Option<u32>) {
        let mut store = self.inner.borrow_mut();
        let slot = store.selection_slots.entry(key).or_insert(SelectionSlot {
            value: None,
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }

    /// Write a text change. Marks slot dirty.
    pub fn set_text(&self, key: BindingKey, value: String) {
        let mut store = self.inner.borrow_mut();
        let slot = store.text_slots.entry(key).or_insert(TextSlot {
            value: String::new(),
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }

    /// Write an f32 change. Marks slot dirty.
    pub fn set_f32(&self, key: BindingKey, value: f32) {
        let mut store = self.inner.borrow_mut();
        let slot = store.f32_slots.entry(key).or_insert(F32Slot {
            value: 0.0,
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }
}

// ============================================================================
// FormBinder
// ============================================================================

/// Application-layer binding facade. Owns the shared change store.
///
/// Created once per Screen. Used in `flush_pending_actions` to sync widget changes to model fields.
pub struct FormBinder {
    inner: Rc<RefCell<ChangeStoreInner>>,
}

impl FormBinder {
    /// Create a new empty FormBinder.
    pub fn new() -> Self {
        Self {
            inner: Rc::new(RefCell::new(ChangeStoreInner::new())),
        }
    }

    /// Create a ChangeWriter handle for widget emission.
    pub fn writer(&self) -> ChangeWriter {
        ChangeWriter {
            inner: Rc::clone(&self.inner),
        }
    }

    /// Execute a binding pass. All sync/take operations occur inside the closure.
    ///
    /// `_cx` serves as a phase marker — proves caller has ScreenContext access.
    /// The closure receives a `BindingContext` whose lifetime prevents handles from escaping.
    pub fn bind<F>(&mut self, _cx: &ScreenContext<'_>, f: F)
    where
        F: FnOnce(&mut BindingContext<'_>),
    {
        let mut guard = self.inner.borrow_mut();
        let mut ctx = BindingContext { store: &mut *guard };
        f(&mut ctx);
    }

    /// Manual write: set a bool value by string key. For testing and stubbing.
    pub fn set_bool(&mut self, key: &str, value: bool) {
        let mut store = self.inner.borrow_mut();
        let binding_key = store.intern(key);
        let slot = store.bool_slots.entry(binding_key).or_insert(BoolSlot {
            value: false,
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }

    /// Manual write: set a selection value by string key.
    pub fn set_selection(&mut self, key: &str, value: Option<u32>) {
        let mut store = self.inner.borrow_mut();
        let binding_key = store.intern(key);
        let slot = store
            .selection_slots
            .entry(binding_key)
            .or_insert(SelectionSlot {
                value: None,
                dirty: false,
            });
        slot.value = value;
        slot.dirty = true;
    }

    /// Manual write: set a text value by string key.
    pub fn set_text(&mut self, key: &str, value: String) {
        let mut store = self.inner.borrow_mut();
        let binding_key = store.intern(key);
        let slot = store.text_slots.entry(binding_key).or_insert(TextSlot {
            value: String::new(),
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }

    /// Manual write: set an f32 value by string key.
    pub fn set_f32(&mut self, key: &str, value: f32) {
        let mut store = self.inner.borrow_mut();
        let binding_key = store.intern(key);
        let slot = store.f32_slots.entry(binding_key).or_insert(F32Slot {
            value: 0.0,
            dirty: false,
        });
        slot.value = value;
        slot.dirty = true;
    }
}

impl Default for FormBinder {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// BindingContext
// ============================================================================

/// Scoped context passed into `bind()` closure.
///
/// Lifetime `'a` is tied to the RefCell borrow inside `bind()`.
/// Typed accessors return handles that borrow from this context,
/// preventing them from escaping the closure.
pub struct BindingContext<'a> {
    store: &'a mut ChangeStoreInner,
}

impl<'a> BindingContext<'a> {
    /// Access a bool binding by string key.
    pub fn bool(&mut self, key: &str) -> BoolBinding<'_> {
        let binding_key = self.store.intern(key);
        let slot = self.store.bool_slots.get_mut(&binding_key);
        BoolBinding { slot }
    }

    /// Access a selection binding by string key.
    pub fn selection(&mut self, key: &str) -> SelectionBinding<'_> {
        let binding_key = self.store.intern(key);
        let slot = self.store.selection_slots.get_mut(&binding_key);
        SelectionBinding { slot }
    }

    /// Access a text binding by string key.
    pub fn text(&mut self, key: &str) -> TextBinding<'_> {
        let binding_key = self.store.intern(key);
        let slot = self.store.text_slots.get_mut(&binding_key);
        TextBinding { slot }
    }

    /// Access an f32 binding by string key.
    pub fn f32(&mut self, key: &str) -> F32Binding<'_> {
        let binding_key = self.store.intern(key);
        let slot = self.store.f32_slots.get_mut(&binding_key);
        F32Binding { slot }
    }
}

// ============================================================================
// Typed Binding Handles
// ============================================================================

/// Bool binding handle. Lifetime-tied to BindingContext.
pub struct BoolBinding<'a> {
    slot: Option<&'a mut BoolSlot>,
}

impl<'a> BoolBinding<'a> {
    /// If changed since last sync, copy value into target and clear dirty flag.
    /// No-op if unchanged or key never written.
    pub fn sync(&mut self, target: &mut bool) {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                *target = slot.value;
                slot.dirty = false;
            }
        }
    }

    /// If changed since last sync, return value and clear dirty flag.
    /// Returns None if unchanged or key never written.
    pub fn take(&mut self) -> Option<bool> {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                slot.dirty = false;
                return Some(slot.value);
            }
        }
        None
    }
}

/// Selection binding handle. Lifetime-tied to BindingContext.
pub struct SelectionBinding<'a> {
    slot: Option<&'a mut SelectionSlot>,
}

impl<'a> SelectionBinding<'a> {
    /// If changed since last sync, copy value into target and clear dirty flag.
    pub fn sync(&mut self, target: &mut Option<u32>) {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                *target = slot.value;
                slot.dirty = false;
            }
        }
    }

    /// If changed since last sync, return value and clear dirty flag.
    pub fn take(&mut self) -> Option<Option<u32>> {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                slot.dirty = false;
                return Some(slot.value);
            }
        }
        None
    }
}

/// Text binding handle. Lifetime-tied to BindingContext.
pub struct TextBinding<'a> {
    slot: Option<&'a mut TextSlot>,
}

impl<'a> TextBinding<'a> {
    /// If changed since last sync, copy value into target and clear dirty flag.
    pub fn sync(&mut self, target: &mut String) {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                target.clone_from(&slot.value);
                slot.dirty = false;
            }
        }
    }

    /// If changed since last sync, return value and clear dirty flag.
    pub fn take(&mut self) -> Option<String> {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                slot.dirty = false;
                return Some(slot.value.clone());
            }
        }
        None
    }
}

/// F32 binding handle. Lifetime-tied to BindingContext.
pub struct F32Binding<'a> {
    slot: Option<&'a mut F32Slot>,
}

impl<'a> F32Binding<'a> {
    /// If changed since last sync, copy value into target and clear dirty flag.
    pub fn sync(&mut self, target: &mut f32) {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                *target = slot.value;
                slot.dirty = false;
            }
        }
    }

    /// If changed since last sync, return value and clear dirty flag.
    pub fn take(&mut self) -> Option<f32> {
        if let Some(slot) = &mut self.slot {
            if slot.dirty {
                slot.dirty = false;
                return Some(slot.value);
            }
        }
        None
    }
}
