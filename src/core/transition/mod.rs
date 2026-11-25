// ============================================================================
// SPEC-HAL-021 Stage 3: Transition Layer Module
// ============================================================================
//
// This module provides the TransitionLayer subsystem, which owns animation
// lifetimes and policy without embedding animation into widgets or routing.
//
// ARCHITECTURAL INVARIANTS:
//
// INV-TL-1: Widgets do not own animation lifetimes
//   Widgets store TransitionHandles (IDs), not Timelines or references.
//
// INV-TL-2: Routing does not decide animation existence
//   EventRouter accepts animation capability but does not own or create it.
//
// INV-TL-3: Timeline does not request frames
//   Timeline is a passive value store. Frame scheduling is external.
//
// INV-TL-4: TransitionLayer is the sole animation policy owner
//   Only TransitionLayer decides when animation capability exists.
//
// ============================================================================

mod transition_layer;

pub use transition_layer::TransitionLayer;
