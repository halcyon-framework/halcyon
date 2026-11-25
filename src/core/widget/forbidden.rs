//! Forbidden Type Markers for Widget Authority Firewall
//!
//! This module defines "poison pill" marker types that reference forbidden
//! domains. These markers are used by the guard module to mechanically verify
//! that Widget structs do not contain forbidden types.
//!
//! # How It Works
//!
//! Each poison marker contains a `PhantomData<ForbiddenType>` which creates
//! a compile-time dependency on the forbidden module. If the forbidden module
//! doesn't exist or the type path is wrong, this file fails to compile.
//!
//! The guard module then implements `NoForbiddenWidget` for all allowed types
//! but NOT for these poison markers. If a forbidden type appears in a Widget
//! struct, the `NoForbiddenWidget` bound will fail.
//!
//! # Forbidden Domains for Widget
//!
//! Widget is the UI component abstraction. At API boundaries it MUST NOT contain:
//! - `crate::core::scene::SceneBuilder` - Use PaintSink trait instead
//! - `crate::core::scene::SceneGraph` - Render output, not widget concern
//! - `crate::core::scene::SceneNode` - Render primitives, not widget concern
//!
//! # Note on PaintContext
//!
//! PaintContext uses `&mut dyn PaintSink` which is scene-agnostic.
//! SceneNodeKey is passed through PaintContext, not imported by widgets.

use std::marker::PhantomData;

// ============================================================================
// Poison Markers — each contains PhantomData of a forbidden type
// ============================================================================

/// Poison marker for SceneBuilder.
/// Widgets should use PaintSink trait, not SceneBuilder directly.
#[allow(dead_code)]
pub struct PoisonSceneBuilder(PhantomData<crate::core::scene::SceneBuilder>);

/// Poison marker for SceneGraph.
/// SceneGraph is render output, not widget concern.
#[allow(dead_code)]
pub struct PoisonSceneGraph(PhantomData<crate::core::scene::SceneGraph>);

/// Poison marker for SceneNode.
/// SceneNode is render primitive, not widget concern.
#[allow(dead_code)]
pub struct PoisonSceneNode(PhantomData<crate::core::scene::SceneNode>);

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
        let _: PhantomData<PoisonSceneBuilder> = PhantomData;
        let _: PhantomData<PoisonSceneGraph> = PhantomData;
        let _: PhantomData<PoisonSceneNode> = PhantomData;
    }
}
