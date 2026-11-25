//! SPEC-HAL-031: Reactive signal store for targeted widget invalidation.
//!
//! Signals are flat, synchronous cells. When application code sets a signal value,
//! only widgets that subscribed (via `watch()`) are marked dirty. No propagation
//! graphs, no async, no re-entrancy.

use std::any::Any;
use std::collections::{HashMap, HashSet};

use crate::core::widget_tree::NodeId;

/// Opaque signal identifier. Internal to SignalStore.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SignalId(u32);

/// Lightweight, copyable handle to a signal. Stored by widgets.
/// The type parameter ensures type-safe access.
#[derive(Debug)]
pub struct SignalHandle<T> {
    id: SignalId,
    _marker: std::marker::PhantomData<T>,
}

impl<T> Clone for SignalHandle<T> {
    fn clone(&self) -> Self {
        *self
    }
}
impl<T> Copy for SignalHandle<T> {}

/// Type-erased signal slot.
struct SignalSlot {
    /// Current value (type-erased)
    value: Box<dyn Any>,
    /// Widget nodes subscribed to this signal
    subscribers: HashSet<NodeId>,
}

/// Reactive signal store. Owns all signals, tracks subscriptions, accumulates dirty nodes.
pub struct SignalStore {
    slots: HashMap<SignalId, SignalSlot>,
    next_id: u32,
    /// Accumulated dirty node IDs from set() calls, to be applied to widget tree.
    pending_dirty: Vec<NodeId>,
}

impl SignalStore {
    pub fn new() -> Self {
        Self {
            slots: HashMap::new(),
            next_id: 0,
            pending_dirty: Vec::new(),
        }
    }

    /// Create a new signal with an initial value. Returns a typed handle.
    pub fn create<T: 'static + Clone + PartialEq>(&mut self, initial: T) -> SignalHandle<T> {
        let id = SignalId(self.next_id);
        self.next_id += 1;
        self.slots.insert(
            id,
            SignalSlot {
                value: Box::new(initial),
                subscribers: HashSet::new(),
            },
        );
        SignalHandle {
            id,
            _marker: std::marker::PhantomData,
        }
    }

    /// Get the current value of a signal.
    pub fn get<T: 'static + Clone>(&self, handle: SignalHandle<T>) -> T {
        self.slots
            .get(&handle.id)
            .and_then(|slot| slot.value.downcast_ref::<T>())
            .cloned()
            .expect("Signal type mismatch or missing signal")
    }

    /// Set a signal value. If changed (via PartialEq), marks all subscribers dirty.
    pub fn set<T: 'static + Clone + PartialEq>(&mut self, handle: SignalHandle<T>, value: T) {
        let slot = self.slots.get_mut(&handle.id).expect("Signal missing");
        let current = slot
            .value
            .downcast_ref::<T>()
            .expect("Signal type mismatch");
        if *current == value {
            return; // No change, no notifications
        }
        slot.value = Box::new(value);
        for &node_id in &slot.subscribers {
            self.pending_dirty.push(node_id);
        }
    }

    /// Subscribe a widget node to a signal and return the current value.
    /// This is the primary widget-side API — combines subscribe + get.
    pub fn watch<T: 'static + Clone>(&mut self, handle: SignalHandle<T>, node_id: NodeId) -> T {
        let slot = self.slots.get_mut(&handle.id).expect("Signal missing");
        slot.subscribers.insert(node_id);
        slot.value
            .downcast_ref::<T>()
            .cloned()
            .expect("Signal type mismatch")
    }

    /// Take all pending dirty node IDs (clears the list).
    /// Called by the engine loop to apply dirty marks to the widget tree.
    pub fn take_pending_dirty(&mut self) -> Vec<NodeId> {
        std::mem::take(&mut self.pending_dirty)
    }

    /// Remove all subscriptions for a node (called on widget unmount).
    pub fn remove_subscriber(&mut self, node_id: NodeId) {
        for slot in self.slots.values_mut() {
            slot.subscribers.remove(&node_id);
        }
    }

    /// Remove all subscriptions for multiple nodes (batch unmount).
    pub fn remove_subscribers(&mut self, node_ids: &[NodeId]) {
        if node_ids.is_empty() {
            return;
        }
        let set: HashSet<NodeId> = node_ids.iter().copied().collect();
        for slot in self.slots.values_mut() {
            slot.subscribers.retain(|id| !set.contains(id));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_and_get() {
        let mut store = SignalStore::new();
        let handle = store.create(42u32);
        assert_eq!(store.get(handle), 42);
    }

    #[test]
    fn set_and_get() {
        let mut store = SignalStore::new();
        let handle = store.create(10i32);
        store.set(handle, 20);
        assert_eq!(store.get(handle), 20);
    }

    #[test]
    fn set_equal_no_dirty() {
        let mut store = SignalStore::new();
        let handle = store.create(5u32);
        let node = NodeId(1);
        store.watch(handle, node);
        let _ = store.take_pending_dirty();
        store.set(handle, 5); // same value
        assert!(store.take_pending_dirty().is_empty());
    }

    #[test]
    fn set_different_marks_dirty() {
        let mut store = SignalStore::new();
        let handle = store.create(5u32);
        let node = NodeId(1);
        store.watch(handle, node);
        let _ = store.take_pending_dirty();
        store.set(handle, 10);
        let dirty = store.take_pending_dirty();
        assert_eq!(dirty, vec![node]);
    }

    #[test]
    fn watch_subscribes() {
        let mut store = SignalStore::new();
        let handle = store.create(false);
        let node = NodeId(3);
        let val = store.watch(handle, node);
        assert!(!val);
        store.set(handle, true);
        let dirty = store.take_pending_dirty();
        assert_eq!(dirty, vec![node]);
    }

    #[test]
    fn multiple_subscribers() {
        let mut store = SignalStore::new();
        let handle = store.create(0u32);
        let n1 = NodeId(5);
        let n2 = NodeId(12);
        store.watch(handle, n1);
        store.watch(handle, n2);
        let _ = store.take_pending_dirty();
        store.set(handle, 1);
        let mut dirty = store.take_pending_dirty();
        dirty.sort_by_key(|n| n.0);
        assert_eq!(dirty, vec![n1, n2]);
    }

    #[test]
    fn remove_subscriber_no_dirty() {
        let mut store = SignalStore::new();
        let handle = store.create(0u32);
        let n1 = NodeId(1);
        let n2 = NodeId(2);
        store.watch(handle, n1);
        store.watch(handle, n2);
        let _ = store.take_pending_dirty();
        store.remove_subscriber(n1);
        store.set(handle, 99);
        let dirty = store.take_pending_dirty();
        assert_eq!(dirty, vec![n2]);
    }

    #[test]
    fn take_pending_dirty_clears() {
        let mut store = SignalStore::new();
        let handle = store.create(0u32);
        let node = NodeId(1);
        store.watch(handle, node);
        let _ = store.take_pending_dirty();
        store.set(handle, 1);
        let first = store.take_pending_dirty();
        assert_eq!(first.len(), 1);
        let second = store.take_pending_dirty();
        assert!(second.is_empty());
    }

    #[test]
    fn type_safety_via_handle() {
        let mut store = SignalStore::new();
        let bool_handle = store.create(true);
        let string_handle = store.create("hello".to_string());
        assert_eq!(store.get(bool_handle), true);
        assert_eq!(store.get(string_handle), "hello");
    }
}
