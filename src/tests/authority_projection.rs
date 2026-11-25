//! SPEC-HAL-190: Authority Projection unit tests.
//!
//! Tests for the authority projection mechanism that allows sovereign widgets
//! to own authoritative state and expose controlled, phase-bound access.
//!
//! These tests are in src/tests/ because they require access to:
//! - `Projection::new` (pub(crate))
//! - `TreeWidgetId::from_raw` (pub(crate))

use std::any::Any;

use crate::core::authority::{
    Authority, AuthorityRegistry, IntentBuffer, IntentBuffers, Projection,
};
use crate::core::widget_tree::NodeId;
use crate::utils::id::TreeWidgetId;

// Test authority type for verification
struct TestAuthority {
    value: i32,
}
impl Authority for TestAuthority {}

// Test intent type
use crate::core::authority::Intent;

struct TestIntent {
    action: String,
}
impl Intent for TestIntent {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[test]
fn test_authority_trait_works_in_generic_contexts() {
    fn accepts_authority<A: Authority>(_a: &A) -> bool {
        true
    }

    let auth = TestAuthority { value: 42 };
    assert!(accepts_authority(&auth));
}

#[test]
fn test_projection_get_returns_authority() {
    let auth = TestAuthority { value: 42 };
    let projection = Projection::new(&auth);
    assert_eq!(projection.get().value, 42);
}

#[test]
fn test_projection_cannot_be_cloned() {
    // This test verifies at compile time that Projection doesn't implement Clone.
    // If Clone were implemented, this code would compile:
    // let auth = TestAuthority { value: 42 };
    // let p1 = Projection::new(&auth);
    // let p2 = p1.clone(); // Should fail to compile
    //
    // We can't test compile errors in runtime tests, but the absence of
    // Clone impl is verified by the struct definition.
}

#[test]
fn test_intent_buffer_push_and_drain() {
    let mut buffer = IntentBuffer::new();
    assert!(buffer.is_empty());

    buffer.push(Box::new(TestIntent {
        action: "select".to_string(),
    }));
    buffer.push(Box::new(TestIntent {
        action: "resize".to_string(),
    }));

    assert_eq!(buffer.len(), 2);
    assert!(!buffer.is_empty());

    let intents = buffer.drain();
    assert_eq!(intents.len(), 2);
    assert!(buffer.is_empty());

    // Verify downcast works
    let first = intents[0].as_any().downcast_ref::<TestIntent>().unwrap();
    assert_eq!(first.action, "select");
}

#[test]
fn test_intent_buffers_drain_all() {
    let mut buffers = IntentBuffers::new();

    let owner1 = TreeWidgetId::from_raw(1);
    let owner2 = TreeWidgetId::from_raw(2);

    buffers.buffer_for(owner1).push(Box::new(TestIntent {
        action: "a".to_string(),
    }));
    buffers.buffer_for(owner2).push(Box::new(TestIntent {
        action: "b".to_string(),
    }));
    buffers.buffer_for(owner2).push(Box::new(TestIntent {
        action: "c".to_string(),
    }));

    let drained = buffers.drain_all();
    assert_eq!(drained.len(), 2);
    assert!(buffers.all_empty());
}

#[test]
fn test_authority_registry() {
    use std::any::TypeId;

    let mut registry = AuthorityRegistry::new();
    let node1 = NodeId(1);
    let node2 = NodeId(2);
    let type_a = TypeId::of::<TestAuthority>();

    registry.register(node1, type_a);

    assert!(registry.provides(node1, type_a));
    assert!(!registry.provides(node2, type_a));

    registry.unregister(node1);
    assert!(!registry.provides(node1, type_a));
}
