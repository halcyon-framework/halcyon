//! Forbidden Type Markers for SceneDiff Authority Firewall
//!
//! This module defines "poison pill" marker types that reference forbidden
//! domains. These markers are used by the guard module to mechanically verify
//! that SceneDiff structs do not contain forbidden types.
//!
//! # How It Works
//!
//! Each poison marker contains a `PhantomData<ForbiddenType>` which creates
//! a compile-time dependency on the forbidden module. If the forbidden module
//! doesn't exist or the type path is wrong, this file fails to compile.
//!
//! The guard module then implements `NoForbiddenSceneDiff` for all allowed types
//! but NOT for these poison markers. If a forbidden type appears in a SceneDiff
//! struct, the `NoForbiddenSceneDiff` bound will fail.
//!
//! # Forbidden Domains for SceneDiff
//!
//! SceneDiff is pure structural diff data. It MUST NOT contain:
//! - `crate::core::widget` - Widget system (runtime UI state)
//! - `crate::core::event` - Event system (input routing)
//! - `crate::core::layout` - Layout system (constraint solving)
//! - `crate::core::hit_test` - Hit testing (input dispatch)
//! - `crate::core::style` - Styling (computed styles)
//! - `crate::compositor::*` - Compositor (any compositor types)

use std::marker::PhantomData;

// ============================================================================
// Poison Markers — each contains PhantomData of a forbidden type
// ============================================================================

/// Poison marker for widget system types.
#[allow(dead_code)]
pub struct PoisonWidget(PhantomData<crate::core::widget::LayoutContext<'static>>);

/// Poison marker for event system types.
#[allow(dead_code)]
pub struct PoisonEvent(PhantomData<crate::core::event::PointerEvent>);

/// Poison marker for layout system types.
#[allow(dead_code)]
pub struct PoisonLayout(PhantomData<crate::core::layout::LayoutResult>);

/// Poison marker for hit test types.
#[allow(dead_code)]
pub struct PoisonHitTest(PhantomData<crate::core::hit_test::HitTestGrid>);

/// Poison marker for style types.
#[allow(dead_code)]
pub struct PoisonStyle(PhantomData<crate::core::style::StyleSpec>);

/// Poison marker for Compositor.
#[allow(dead_code)]
pub struct PoisonCompositor(PhantomData<crate::compositor::Compositor>);

/// Poison marker for SurfaceStack.
#[allow(dead_code)]
pub struct PoisonSurfaceStack(PhantomData<crate::compositor::SurfaceStack>);

/// Poison marker for SurfaceStackSnapshot.
#[allow(dead_code)]
pub struct PoisonSurfaceStackSnapshot(PhantomData<crate::compositor::SurfaceStackSnapshot>);

// ============================================================================
// Compile-time verification that forbidden paths are correct
// ============================================================================

#[cfg(test)]
mod path_verification {
    //! These tests verify the poison marker paths are correct.
    //! If any path is wrong, this module fails to compile.

    use super::*;

    #[test]
    fn poison_markers_compile() {
        // If this compiles, all forbidden type paths are valid.
        let _: PhantomData<PoisonWidget> = PhantomData;
        let _: PhantomData<PoisonEvent> = PhantomData;
        let _: PhantomData<PoisonLayout> = PhantomData;
        let _: PhantomData<PoisonHitTest> = PhantomData;
        let _: PhantomData<PoisonStyle> = PhantomData;
        let _: PhantomData<PoisonCompositor> = PhantomData;
        let _: PhantomData<PoisonSurfaceStack> = PhantomData;
        let _: PhantomData<PoisonSurfaceStackSnapshot> = PhantomData;
    }
}
