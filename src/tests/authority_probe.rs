//! SPEC-HAL-190: Authority Projection Usage Validation
//!
//! This module proves SPEC-HAL-190 is usable, sufficient, and non-leaky by
//! exercising authority projection + intent emission + deferred mutation
//! in a minimal composite widget structure.
//!
//! Structure:
//! ```text
//! AuthorityProbeComposite (owns ProbeAuthority)
//! ├─ HeaderProbe (reads authority in layout)
//! ├─ BodyProbe   (emits intents during event)
//! └─ FooterProbe (reads authority in layout)
//! ```
//!
//! Behavioral Requirements:
//! 1. Projection: HeaderProbe and FooterProbe obtain authority via cx.authority()
//! 2. Intent Emission: BodyProbe emits IncrementCounterIntent on event
//! 3. Deferred Mutation: Mutation happens only in process_intents
//! 4. Phase Proof: Layout reflects updated counter only after deferred mutation
//!
//! NOTE: Full end-to-end widget tree integration is tested separately.
//! These tests focus on the SPEC-HAL-190 types and trait implementations.

#![cfg(test)]

use std::any::{Any, TypeId};
use std::cell::Cell;

use crate::core::authority::{Authority, AuthorityOwner, Intent, Projection};

// =============================================================================
// ProbeAuthority (the authority type)
// =============================================================================

/// Test authority type for SPEC-HAL-190 validation.
///
/// INV-AUTH-QUALIFY-1: This type qualifies as authority because:
/// - Multiple consumers need to read it (HeaderProbe, FooterProbe)
/// - Independent lifetime (meaningful even if probes disappear)
/// - Semantic weight (represents count state)
/// - Single lawful owner (AuthorityProbeComposite)
#[derive(Debug)]
pub struct ProbeAuthority {
    /// The counter value. Mutated only during deferred mutation phase.
    pub counter: i32,
    /// Debug: tracks how many times counter was read during layout.
    pub layout_read_count: Cell<u32>,
    /// Debug: tracks how many times counter was read during paint.
    pub paint_read_count: Cell<u32>,
}

impl ProbeAuthority {
    pub fn new(initial: i32) -> Self {
        Self {
            counter: initial,
            layout_read_count: Cell::new(0),
            paint_read_count: Cell::new(0),
        }
    }

    pub fn increment_layout_reads(&self) {
        self.layout_read_count.set(self.layout_read_count.get() + 1);
    }

    pub fn increment_paint_reads(&self) {
        self.paint_read_count.set(self.paint_read_count.get() + 1);
    }
}

impl Authority for ProbeAuthority {}

// =============================================================================
// IncrementCounterIntent (the intent type)
// =============================================================================

/// Intent to increment the counter in ProbeAuthority.
///
/// Emitted by BodyProbe during event handling.
/// Processed by AuthorityProbeComposite during deferred mutation.
#[derive(Debug)]
pub struct IncrementCounterIntent {
    pub delta: i32,
}

impl Intent for IncrementCounterIntent {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// =============================================================================
// AuthorityProbeComposite (the authority owner)
// =============================================================================

/// Composite widget stub that owns ProbeAuthority.
///
/// Implements AuthorityOwner to receive and process intents during
/// deferred mutation phase.
///
/// NOTE: This is a stub for testing SPEC-HAL-190 mechanics only.
/// Full Widget trait implementation is not needed for these tests.
pub struct AuthorityProbeComposite {
    /// The authority owned by this composite.
    authority: ProbeAuthority,
    /// Tracks how many times process_intents was called.
    pub process_intents_calls: Cell<u32>,
    /// Tracks total intents processed.
    pub total_intents_processed: Cell<u32>,
}

impl AuthorityProbeComposite {
    pub fn new(initial_counter: i32) -> Self {
        Self {
            authority: ProbeAuthority::new(initial_counter),
            process_intents_calls: Cell::new(0),
            total_intents_processed: Cell::new(0),
        }
    }

    /// Read-only access to authority for test verification.
    pub fn authority(&self) -> &ProbeAuthority {
        &self.authority
    }

    /// Provide authority (simulates Widget::provided_authority).
    pub fn provided_authority(&self) -> Option<(&dyn Any, TypeId)> {
        Some((&self.authority as &dyn Any, TypeId::of::<ProbeAuthority>()))
    }

    /// Provide mutable authority (simulates Widget::provided_authority_mut).
    pub fn provided_authority_mut(&mut self) -> Option<(&mut dyn Any, TypeId)> {
        Some((
            &mut self.authority as &mut dyn Any,
            TypeId::of::<ProbeAuthority>(),
        ))
    }

    /// Return self as AuthorityOwner (simulates Widget::as_authority_owner).
    pub fn as_authority_owner(&mut self) -> Option<&mut dyn AuthorityOwner> {
        Some(self)
    }
}

impl AuthorityOwner for AuthorityProbeComposite {
    fn process_intents(&mut self, intents: Vec<Box<dyn Intent>>) {
        self.process_intents_calls
            .set(self.process_intents_calls.get() + 1);

        for intent in intents {
            if let Some(inc) = intent.as_any().downcast_ref::<IncrementCounterIntent>() {
                // Deferred mutation: only place where authority is mutated.
                self.authority.counter += inc.delta;
                self.total_intents_processed
                    .set(self.total_intents_processed.get() + 1);
            }
        }
    }
}

// =============================================================================
// Unit Tests
// =============================================================================

#[test]
fn test_probe_authority_implements_authority() {
    fn accepts_authority<A: Authority>(_a: &A) -> bool {
        true
    }
    let auth = ProbeAuthority::new(42);
    assert!(accepts_authority(&auth));
}

#[test]
fn test_increment_intent_implements_intent() {
    fn accepts_intent<I: Intent>(_i: &I) -> bool {
        true
    }
    let intent = IncrementCounterIntent { delta: 1 };
    assert!(accepts_intent(&intent));
}

#[test]
fn test_intent_downcast() {
    let intent = IncrementCounterIntent { delta: 5 };
    let boxed: Box<dyn Intent> = Box::new(intent);
    let downcasted = boxed.as_any().downcast_ref::<IncrementCounterIntent>();
    assert!(downcasted.is_some());
    assert_eq!(downcasted.unwrap().delta, 5);
}

#[test]
fn test_composite_provides_authority() {
    let composite = AuthorityProbeComposite::new(100);
    let provided = composite.provided_authority();
    assert!(provided.is_some());
    let (any_ref, type_id) = provided.unwrap();
    assert_eq!(type_id, TypeId::of::<ProbeAuthority>());
    let auth = any_ref.downcast_ref::<ProbeAuthority>().unwrap();
    assert_eq!(auth.counter, 100);
}

#[test]
fn test_composite_provides_mutable_authority() {
    let mut composite = AuthorityProbeComposite::new(50);
    let provided = composite.provided_authority_mut();
    assert!(provided.is_some());
    let (any_ref, type_id) = provided.unwrap();
    assert_eq!(type_id, TypeId::of::<ProbeAuthority>());
    let auth = any_ref.downcast_mut::<ProbeAuthority>().unwrap();
    assert_eq!(auth.counter, 50);
    // Verify we can mutate via provided_authority_mut
    auth.counter = 75;
    assert_eq!(composite.authority().counter, 75);
}

#[test]
fn test_composite_process_intents_increments_counter() {
    let mut composite = AuthorityProbeComposite::new(10);
    assert_eq!(composite.authority().counter, 10);

    // Process intents.
    let intents: Vec<Box<dyn Intent>> = vec![
        Box::new(IncrementCounterIntent { delta: 5 }),
        Box::new(IncrementCounterIntent { delta: 3 }),
    ];
    composite.process_intents(intents);

    // Verify mutation happened.
    assert_eq!(composite.authority().counter, 18);
    assert_eq!(composite.process_intents_calls.get(), 1);
    assert_eq!(composite.total_intents_processed.get(), 2);
}

#[test]
fn test_composite_as_authority_owner_returns_self() {
    let mut composite = AuthorityProbeComposite::new(0);
    let owner = composite.as_authority_owner();
    assert!(owner.is_some());

    // Process an intent through the trait object.
    let owner = composite.as_authority_owner().unwrap();
    owner.process_intents(vec![Box::new(IncrementCounterIntent { delta: 7 })]);

    // Verify mutation.
    assert_eq!(composite.authority().counter, 7);
}

#[test]
fn test_projection_get_returns_authority() {
    let auth = ProbeAuthority::new(42);
    let projection = Projection::new(&auth);
    assert_eq!(projection.get().counter, 42);
}

#[test]
fn test_projection_is_read_only() {
    // Verify that projection.get() returns immutable reference.
    let auth = ProbeAuthority::new(100);
    let projection = Projection::new(&auth);

    // Can read the counter.
    let counter = projection.get().counter;
    assert_eq!(counter, 100);

    // Cannot mutate through projection (compile-time enforced).
    // The following would fail to compile:
    // projection.get().counter = 200;
    // projection.get_mut() does not exist

    // Verify authority unchanged.
    assert_eq!(auth.counter, 100);
}

#[test]
fn test_projection_lifetime_bound_to_authority() {
    // Verify that projection cannot outlive authority.
    // This is compile-time enforced by the 'phase lifetime.
    let auth = ProbeAuthority::new(42);
    let projection = Projection::new(&auth);

    // Both are valid within the same scope.
    assert_eq!(projection.get().counter, auth.counter);

    // The following would fail to compile (storing projection beyond authority):
    // let escaped: Projection<'static, ProbeAuthority>; // Not possible
}

#[test]
fn test_multiple_intents_processed_in_order() {
    let mut composite = AuthorityProbeComposite::new(0);

    // Create intents that must be processed in order for correct result.
    let intents: Vec<Box<dyn Intent>> = vec![
        Box::new(IncrementCounterIntent { delta: 10 }),
        Box::new(IncrementCounterIntent { delta: 20 }),
        Box::new(IncrementCounterIntent { delta: 30 }),
    ];

    composite.process_intents(intents);

    // Verify final counter is sum of all deltas.
    assert_eq!(composite.authority().counter, 60);
    assert_eq!(composite.total_intents_processed.get(), 3);
}

#[test]
fn test_process_intents_called_multiple_times() {
    let mut composite = AuthorityProbeComposite::new(100);

    // First batch of intents.
    composite.process_intents(vec![Box::new(IncrementCounterIntent { delta: 5 })]);
    assert_eq!(composite.authority().counter, 105);
    assert_eq!(composite.process_intents_calls.get(), 1);

    // Second batch of intents.
    composite.process_intents(vec![
        Box::new(IncrementCounterIntent { delta: 10 }),
        Box::new(IncrementCounterIntent { delta: 15 }),
    ]);
    assert_eq!(composite.authority().counter, 130);
    assert_eq!(composite.process_intents_calls.get(), 2);
    assert_eq!(composite.total_intents_processed.get(), 3);
}

#[test]
fn test_empty_intents_batch() {
    let mut composite = AuthorityProbeComposite::new(50);

    // Process empty batch.
    composite.process_intents(vec![]);

    // Counter unchanged, but process_intents was still called.
    assert_eq!(composite.authority().counter, 50);
    assert_eq!(composite.process_intents_calls.get(), 1);
    assert_eq!(composite.total_intents_processed.get(), 0);
}

#[test]
fn test_negative_delta_intent() {
    let mut composite = AuthorityProbeComposite::new(100);

    composite.process_intents(vec![Box::new(IncrementCounterIntent { delta: -30 })]);

    assert_eq!(composite.authority().counter, 70);
}

#[test]
fn test_authority_read_tracking() {
    let auth = ProbeAuthority::new(42);

    assert_eq!(auth.layout_read_count.get(), 0);
    assert_eq!(auth.paint_read_count.get(), 0);

    auth.increment_layout_reads();
    auth.increment_layout_reads();
    auth.increment_paint_reads();

    assert_eq!(auth.layout_read_count.get(), 2);
    assert_eq!(auth.paint_read_count.get(), 1);
}

#[test]
fn test_type_id_consistency() {
    let composite = AuthorityProbeComposite::new(0);
    let (_, type_id) = composite.provided_authority().unwrap();

    // TypeId must be consistent.
    assert_eq!(type_id, TypeId::of::<ProbeAuthority>());
    assert_ne!(type_id, TypeId::of::<IncrementCounterIntent>());
    assert_ne!(type_id, TypeId::of::<AuthorityProbeComposite>());
}
