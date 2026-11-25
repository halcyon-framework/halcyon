// ============================================================================
// SPEC-HAL-190: Engine-Level Phase Enforcement Demo
// ============================================================================
//
// This module proves SPEC-HAL-190 works at the engine level with:
// - Real Widget implementations
// - Real phase contexts (LayoutContext, EventContext, PaintContext)
// - Real authority lookup via cx.authority()
// - Real intent emission via cx.emit_intent()
// - Real deferred mutation via process_intents()
//
// Structure:
//
//   AuthorityProbeComposite (owns ProbeAuthority, implements AuthorityOwner)
//   ├─ HeaderProbe   (reads authority in layout)
//   ├─ BodyProbe     (emits intent in handle_event)
//   └─ FooterProbe   (reads authority in paint)
//
// Phase Sequence:
// 1. BodyProbe::handle_event → emits IncrementCounterIntent
// 2. run_deferred_mutation_phase → AuthorityProbeComposite::process_intents
// 3. HeaderProbe::layout → reads updated counter via cx.authority()
// 4. FooterProbe::paint → reads updated counter via cx.authority()
//
// Invariants Verified:
// - INV-AUTH-PHASE-BOUND: Projections cannot escape their phase
// - INV-AUTH-ANCESTOR-ONLY: Lookup walks ancestors, not siblings
// - INV-INTENT-EMIT-1: Intents emitted only during event phase
// - INV-INTENT-PROCESS-1: process_intents called once per frame if non-empty
// - INV-AUTH-OWNER-MECHANICAL: Only AuthorityOwner receives intents
// ============================================================================

use std::any::{Any, TypeId};
use std::cell::Cell;

use crate::core::authority::{Authority, AuthorityOwner, Intent};
use crate::core::event::{Event, EventPhase, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::utils::id::WidgetInternalId;

// ============================================================================
// ProbeAuthority (the authority type)
// ============================================================================

/// Authority type for SPEC-HAL-190 phase enforcement verification.
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
    /// Tracks layout phase reads (for verification).
    pub layout_reads: Cell<u32>,
    /// Tracks paint phase reads (for verification).
    pub paint_reads: Cell<u32>,
    /// Tracks event phase authority reads (for verification).
    pub event_reads: Cell<u32>,
    /// Tracks how many times counter was mutated via process_intents.
    pub mutation_count: Cell<u32>,
}

impl ProbeAuthority {
    pub fn new(initial: i32) -> Self {
        Self {
            counter: initial,
            layout_reads: Cell::new(0),
            paint_reads: Cell::new(0),
            event_reads: Cell::new(0),
            mutation_count: Cell::new(0),
        }
    }
}

impl Authority for ProbeAuthority {}

// ============================================================================
// IncrementCounterIntent (the intent type)
// ============================================================================

/// Intent to increment ProbeAuthority counter.
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

/// Intent to set ProbeAuthority counter to an absolute value.
///
/// Used for testing intent ordering: Set(100) then Inc(5) = 105,
/// but Inc(5) then Set(100) = 100.
#[derive(Debug)]
pub struct SetCounterIntent {
    pub value: i32,
}

impl Intent for SetCounterIntent {
    fn as_any(&self) -> &dyn Any {
        self
    }
}

// ============================================================================
// AuthorityProbeComposite (the authority owner)
// ============================================================================

/// Composite widget that owns ProbeAuthority and processes intents.
///
/// This is the authority owner - the single widget that:
/// - Provides authority to descendants via provided_authority()
/// - Receives intents via process_intents()
/// - Mutates authority during deferred mutation phase
pub struct AuthorityProbeComposite {
    id: WidgetInternalId,
    /// The authority owned by this composite.
    authority: ProbeAuthority,
    /// NodeIds of children (HeaderProbe, BodyProbe, FooterProbe).
    children: Vec<NodeId>,
    /// Tracks how many times process_intents was called.
    pub process_intents_calls: Cell<u32>,
}

impl AuthorityProbeComposite {
    pub fn new(initial_counter: i32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            authority: ProbeAuthority::new(initial_counter),
            children: Vec::new(),
            process_intents_calls: Cell::new(0),
        }
    }

    /// Read-only access to authority for test verification.
    pub fn authority(&self) -> &ProbeAuthority {
        &self.authority
    }

    /// Store children NodeIds (called during tree setup).
    pub fn set_children(&mut self, children: Vec<NodeId>) {
        self.children = children;
    }
}

impl WidgetDowncast for AuthorityProbeComposite {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Widget for AuthorityProbeComposite {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        // Store children for later reference
        self.children = children.to_vec();

        // Layout children vertically (simple stack)
        let mut y_offset = 0.0;
        let mut max_width = 0.0_f32;

        for &child_id in children {
            let child_size = cx.layout_child(child_id, constraints);
            cx.set_child_rect(
                child_id,
                RectF::new(0.0, y_offset, child_size.width, child_size.height),
            );
            y_offset += child_size.height;
            max_width = max_width.max(child_size.width);
        }

        LayoutResult::fit(Size::new(
            max_width.clamp(constraints.min_width, constraints.max_width),
            y_offset.clamp(constraints.min_height, constraints.max_height),
        ))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
        // Paint background (light gray)
        cx.scene.push_rect(
            rect,
            crate::core::paint::Color::from_srgb(0.9, 0.9, 0.9, 1.0),
            0,
            cx.scene_key,
        );

        // Paint children (engine handles child paint traversal)
        let _ = children;
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "AuthorityProbeComposite"
    }

    // ========================================================================
    // SPEC-HAL-190: Authority Provider
    // ========================================================================

    fn provided_authority(&self) -> Option<(&dyn Any, TypeId)> {
        Some((&self.authority as &dyn Any, TypeId::of::<ProbeAuthority>()))
    }

    fn provided_authority_mut(&mut self) -> Option<(&mut dyn Any, TypeId)> {
        Some((
            &mut self.authority as &mut dyn Any,
            TypeId::of::<ProbeAuthority>(),
        ))
    }

    fn as_authority_owner(&mut self) -> Option<&mut dyn AuthorityOwner> {
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
                self.authority
                    .mutation_count
                    .set(self.authority.mutation_count.get() + 1);
            } else if let Some(set) = intent.as_any().downcast_ref::<SetCounterIntent>() {
                // Set counter to absolute value (for ordering tests).
                self.authority.counter = set.value;
                self.authority
                    .mutation_count
                    .set(self.authority.mutation_count.get() + 1);
            }
        }
    }
}

// ============================================================================
// HeaderProbe (reads authority in layout)
// ============================================================================

/// Child widget that reads authority during layout phase.
///
/// Verifies:
/// - Authority lookup via cx.authority() works in layout
/// - Layout phase has access to updated counter after deferred mutation
pub struct HeaderProbe {
    id: WidgetInternalId,
    /// Last counter value observed during layout.
    pub last_layout_counter: Cell<i32>,
    /// Height of this probe.
    height: f32,
}

impl HeaderProbe {
    pub fn new(height: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            last_layout_counter: Cell::new(0),
            height,
        }
    }
}

impl WidgetDowncast for HeaderProbe {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Widget for HeaderProbe {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        // SPEC-HAL-190: Look up authority from ancestor (AuthorityProbeComposite)
        if let Some(projection) = cx.authority::<ProbeAuthority>() {
            let auth = projection.get();
            self.last_layout_counter.set(auth.counter);
            auth.layout_reads.set(auth.layout_reads.get() + 1);
        }

        let width = constraints.max_width;
        LayoutResult::fit(Size::new(width, self.height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Paint header background (blue-ish)
        cx.scene.push_rect(
            rect,
            crate::core::paint::Color::from_srgb(0.7, 0.8, 0.9, 1.0),
            1,
            cx.scene_key,
        );
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "HeaderProbe"
    }
}

// ============================================================================
// BodyProbe (emits intent in handle_event)
// ============================================================================

/// Child widget that emits intents during event handling.
///
/// Verifies:
/// - Intent emission via cx.emit_intent() works in event phase
/// - Intent is routed to correct authority owner
/// - Child has no direct access to parent's authority for mutation
pub struct BodyProbe {
    id: WidgetInternalId,
    /// Delta to add on each click.
    pub delta: i32,
    /// Tracks how many intents were emitted.
    pub intents_emitted: Cell<u32>,
    /// Height of this probe.
    height: f32,
}

impl BodyProbe {
    pub fn new(delta: i32, height: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            delta,
            intents_emitted: Cell::new(0),
            height,
        }
    }
}

impl WidgetDowncast for BodyProbe {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Widget for BodyProbe {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = constraints.max_width;
        LayoutResult::fit(Size::new(width, self.height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // Paint body background (green-ish)
        cx.scene.push_rect(
            rect,
            crate::core::paint::Color::from_srgb(0.8, 0.9, 0.7, 1.0),
            1,
            cx.scene_key,
        );
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        // Only handle at target phase
        if phase != EventPhase::Target {
            return false;
        }

        // SPEC-HAL-190: Authority lookup during event (for verification)
        if let Some(projection) = cx.authority::<ProbeAuthority>() {
            projection
                .get()
                .event_reads
                .set(projection.get().event_reads.get() + 1);
        }

        // Handle pointer down → emit intent
        if let Event::Pointer(pe) = event {
            if pe.kind == PointerEventKind::Down {
                // SPEC-HAL-190: Emit intent to authority owner
                // The child does NOT mutate authority directly.
                // Instead, it requests mutation via intent.
                let success = cx.emit_intent::<ProbeAuthority, IncrementCounterIntent>(
                    IncrementCounterIntent { delta: self.delta },
                );

                if success {
                    self.intents_emitted.set(self.intents_emitted.get() + 1);
                }

                return true;
            }
        }

        false
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn debug_type_name(&self) -> &'static str {
        "BodyProbe"
    }
}

// ============================================================================
// FooterProbe (reads authority in paint)
// ============================================================================

/// Child widget that reads authority during paint phase.
///
/// Verifies:
/// - Authority lookup via cx.authority() works in paint
/// - Paint phase sees counter updated by prior deferred mutation
pub struct FooterProbe {
    id: WidgetInternalId,
    /// Last counter value observed during paint.
    pub last_paint_counter: Cell<i32>,
    /// Height of this probe.
    height: f32,
}

impl FooterProbe {
    pub fn new(height: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            last_paint_counter: Cell::new(0),
            height,
        }
    }
}

impl WidgetDowncast for FooterProbe {
    fn as_any(&self) -> &dyn Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn Any {
        self
    }
}

impl Widget for FooterProbe {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = constraints.max_width;
        LayoutResult::fit(Size::new(width, self.height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        // SPEC-HAL-190: Look up authority from ancestor (AuthorityProbeComposite)
        if let Some(projection) = cx.authority::<ProbeAuthority>() {
            let auth = projection.get();
            self.last_paint_counter.set(auth.counter);
            auth.paint_reads.set(auth.paint_reads.get() + 1);
        }

        // Paint footer background (red-ish)
        cx.scene.push_rect(
            rect,
            crate::core::paint::Color::from_srgb(0.9, 0.7, 0.7, 1.0),
            1,
            cx.scene_key,
        );
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "FooterProbe"
    }
}

// ============================================================================
// Tests (in src/tests/ for engine-level verification)
// ============================================================================
//
// Engine-level tests that verify real frame loop behavior are in:
// src/tests/authority_probe_engine.rs
//
// Those tests use the real WidgetTree, real contexts, and real frame loop
// to verify the phase enforcement actually works end-to-end.
// ============================================================================
