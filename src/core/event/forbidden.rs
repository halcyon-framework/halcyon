//! Forbidden Type Markers for Event Authority Firewall
//!
//! This module defines "poison pill" marker types that reference forbidden
//! domains. These markers are used by the guard module to mechanically verify
//! that Event structs do not contain forbidden types.
//!
//! # How It Works
//!
//! Each poison marker contains a `PhantomData<ForbiddenType>` which creates
//! a compile-time dependency on the forbidden module. If the forbidden module
//! doesn't exist or the type path is wrong, this file fails to compile.
//!
//! The guard module then implements `NoForbiddenEvent` for all allowed types
//! but NOT for these poison markers. If a forbidden type appears in an Event
//! struct, the `NoForbiddenEvent` bound will fail.
//!
//! # Forbidden Domains for Event
//!
//! Event is input routing data. It MUST NOT contain:
//! - `crate::core::scene` - SceneGraph, SceneNode (render output)
//! - `crate::core::scene_identity` - SceneNodeKey (render identity)
//! - `crate::core::layout` - Layout system (constraint solving)
//! - `crate::compositor::*` - Compositor (any compositor types)

use std::marker::PhantomData;

// ============================================================================
// Poison Markers — each contains PhantomData of a forbidden type
// ============================================================================

/// Poison marker for SceneGraph types.
#[allow(dead_code)]
pub struct PoisonSceneGraph(PhantomData<crate::core::scene::SceneGraph>);

/// Poison marker for SceneNode types.
#[allow(dead_code)]
pub struct PoisonSceneNode(PhantomData<crate::core::scene::SceneNode>);

/// Poison marker for SceneNodeKey (identity) types.
#[allow(dead_code)]
pub struct PoisonSceneNodeKey(PhantomData<crate::core::scene_identity::SceneNodeKey>);

/// Poison marker for layout system types.
#[allow(dead_code)]
pub struct PoisonLayout(PhantomData<crate::core::layout::LayoutResult>);

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
        let _: PhantomData<PoisonSceneGraph> = PhantomData;
        let _: PhantomData<PoisonSceneNode> = PhantomData;
        let _: PhantomData<PoisonSceneNodeKey> = PhantomData;
        let _: PhantomData<PoisonLayout> = PhantomData;
        let _: PhantomData<PoisonCompositor> = PhantomData;
        let _: PhantomData<PoisonSurfaceStack> = PhantomData;
        let _: PhantomData<PoisonSurfaceStackSnapshot> = PhantomData;
    }
}
