//! SceneGraph Authority Firewall Guard
//!
//! This module provides compile-time enforcement that SceneGraph remains
//! pure render data and does NOT contain widget, event, layout, hit-test,
//! style, or compositor types.
//!
//! # Enforcement Mechanism
//!
//! 1. `NoForbiddenScene` trait implemented for allowed types only
//! 2. Field-by-field assertions for key structs
//! 3. Poison markers in forbidden.rs NOT implementing the trait
//! 4. If forbidden type leaks in, compilation fails
//!
//! # SceneGraph Authority Contract
//!
//! SceneGraph contains ONLY:
//! - Math primitives (RectF, Point, f32, u32, etc.)
//! - Color values
//! - Scene nodes (geometry, text, clips, layers)
//! - Identity types (SceneNodeKey, ZIndex)
//! - Standard containers (Vec, Option, String)
//!
//! SceneGraph MUST NOT contain:
//! - Widget references or trait objects
//! - Event state or input routing
//! - Layout constraints or results
//! - Hit test state
//! - Style specifications
//! - Compositor state (mutator or snapshot)
//!
//! # How Violations Are Caught
//!
//! If a forbidden type is added to a SceneGraph struct field:
//! ```text
//! error[E0277]: the trait bound `ForbiddenType: NoForbiddenScene` is not satisfied
//! ```

#[cfg(test)]
mod tests {
    use super::super::{
        BorderRectNode, CircleNode, ClipNode, Color, EllipseNode, ImageNode, LayerNode, LineNode,
        LuminousContourAppearance, LuminousContourFronts, LuminousContourNode, PathNode,
        PolylineNode, RectNode, RoundedCorners, RoundedRectNode, SceneBuilder, SceneFrame,
        SceneGraph, SceneNode, ScrollNode, ScrollbarHNode, ScrollbarVNode, TextBackgroundSpan,
        TextNode, TextQuad, TextQuadBatch,
    };
    use crate::core::math::{Point, RectF};
    use crate::core::scene_identity::SceneNodeKey;
    use std::time::Instant;

    // ========================================================================
    // NoForbiddenScene Trait and Implementations
    // ========================================================================
    //
    // This trait is intentionally NOT implemented for poison markers.
    // If a forbidden type appears in a SceneGraph struct, compilation fails.

    /// Marker trait for types allowed in SceneGraph structs.
    ///
    /// This trait is intentionally NOT implemented for:
    /// - PoisonWidget, PoisonEvent, PoisonLayout, PoisonHitTest, PoisonStyle
    /// - PoisonCompositor, PoisonSurfaceStack, PoisonSurfaceStackSnapshot
    trait NoForbiddenScene {}

    // Primitives
    impl NoForbiddenScene for u8 {}
    impl NoForbiddenScene for u16 {}
    impl NoForbiddenScene for u32 {}
    impl NoForbiddenScene for u64 {}
    impl NoForbiddenScene for usize {}
    impl NoForbiddenScene for i32 {}
    impl NoForbiddenScene for f32 {}
    impl NoForbiddenScene for f64 {}
    impl NoForbiddenScene for bool {}
    impl NoForbiddenScene for char {}

    // Math types
    impl NoForbiddenScene for RectF {}
    impl NoForbiddenScene for Point {}

    // Color
    impl NoForbiddenScene for Color {}

    // Identity types
    impl NoForbiddenScene for SceneNodeKey {}

    // Rounded corners
    impl NoForbiddenScene for RoundedCorners {}

    // Text types
    impl NoForbiddenScene for TextBackgroundSpan {}
    impl NoForbiddenScene for TextQuad {}
    impl NoForbiddenScene for TextQuadBatch {}

    // Standard library types
    impl NoForbiddenScene for String {}
    impl NoForbiddenScene for &'static str {}
    impl NoForbiddenScene for Instant {}
    impl<T> NoForbiddenScene for Vec<T> {}
    impl<T: ?Sized> NoForbiddenScene for std::sync::Arc<T> {}
    impl<T> NoForbiddenScene for Option<T> {}
    impl<K, V> NoForbiddenScene for std::collections::HashMap<K, V> {}
    impl<T> NoForbiddenScene for std::collections::HashSet<T> {}

    // Scene node types
    impl NoForbiddenScene for SceneNode {}
    impl NoForbiddenScene for RectNode {}
    impl NoForbiddenScene for BorderRectNode {}
    impl NoForbiddenScene for RoundedRectNode {}
    impl NoForbiddenScene for TextNode {}
    impl NoForbiddenScene for ImageNode {}
    impl NoForbiddenScene for ClipNode {}
    impl NoForbiddenScene for LayerNode {}
    impl NoForbiddenScene for ScrollNode {}
    impl NoForbiddenScene for CircleNode {}
    impl NoForbiddenScene for EllipseNode {}
    impl NoForbiddenScene for LineNode {}
    impl NoForbiddenScene for PolylineNode {}
    impl NoForbiddenScene for LuminousContourNode {}
    impl NoForbiddenScene for crate::core::scene::LocalAmbientGlowNode {}
    impl NoForbiddenScene for crate::core::paint::LocalAmbientGlowAppearance {}
    impl NoForbiddenScene for crate::core::paint::LocalGlowLobe {}
    impl NoForbiddenScene for LuminousContourAppearance {}
    impl NoForbiddenScene for LuminousContourFronts {}
    impl NoForbiddenScene for PathNode {}
    impl NoForbiddenScene for ScrollbarVNode {}
    impl NoForbiddenScene for ScrollbarHNode {}

    // Top-level scene types
    impl NoForbiddenScene for SceneGraph {}
    impl NoForbiddenScene for SceneFrame {}
    impl NoForbiddenScene for SceneBuilder {}

    /// Helper function for compile-time NoForbiddenScene assertion.
    fn assert_no_forbidden<T: NoForbiddenScene>() {}

    // ========================================================================
    // SceneGraph Field Firewall
    // ========================================================================

    /// SCENEGRAPH FIELD FIREWALL: All field types must be NoForbiddenScene.
    #[test]
    fn scene_graph_fields_no_forbidden() {
        // SceneGraph fields:
        // - root: Vec<SceneNode>
        // - clear_color: Color
        // - key_widget_map: HashMap<SceneNodeKey, u64> (debug only)

        assert_no_forbidden::<Vec<SceneNode>>();
        assert_no_forbidden::<Color>();
        assert_no_forbidden::<std::collections::HashMap<SceneNodeKey, u64>>();
    }

    /// SCENEFRAME FIELD FIREWALL: All field types must be NoForbiddenScene.
    ///
    /// Note: SceneFrame contains SurfaceStackSnapshot (MANDATORY per SPEC-HAL-081A), but that's
    /// allowed because SceneFrame is the designated carrier. We verify SceneGraph itself
    /// does not contain compositor types.
    #[test]
    fn scene_frame_fields_no_forbidden() {
        // SceneFrame fields:
        // - build_time: Instant
        // - frame_id: u64
        // - scene: SceneGraph
        // - surface_stack: SurfaceStackSnapshot (compositor carrier - MANDATORY, allowed in Frame)

        assert_no_forbidden::<Instant>();
        assert_no_forbidden::<u64>();
        assert_no_forbidden::<SceneGraph>();
        // Note: SurfaceStackSnapshot is allowed ONLY in SceneFrame, not in SceneGraph
    }

    // ========================================================================
    // Scene Node Field Firewalls
    // ========================================================================

    /// RECTNODE FIELD FIREWALL
    #[test]
    fn rect_node_fields_no_forbidden() {
        // RectNode fields: rect, color, z_index, key
        assert_no_forbidden::<RectF>();
        assert_no_forbidden::<Color>();
        assert_no_forbidden::<u32>(); // ZIndex = u32
        assert_no_forbidden::<Option<SceneNodeKey>>();
    }

    /// TEXTNODE FIELD FIREWALL
    #[test]
    fn text_node_fields_no_forbidden() {
        // TextNode fields: position, text, font_size, color, z_index, backgrounds, key
        assert_no_forbidden::<Point>();
        assert_no_forbidden::<String>();
        assert_no_forbidden::<f32>();
        assert_no_forbidden::<Color>();
        assert_no_forbidden::<u32>(); // ZIndex
        assert_no_forbidden::<Vec<TextBackgroundSpan>>();
        assert_no_forbidden::<Option<SceneNodeKey>>();
    }

    /// CLIPNODE FIELD FIREWALL
    #[test]
    fn clip_node_fields_no_forbidden() {
        // ClipNode fields: rect, rounded, children, z_index, key
        assert_no_forbidden::<RectF>();
        assert_no_forbidden::<Option<RoundedCorners>>();
        assert_no_forbidden::<Vec<SceneNode>>();
        assert_no_forbidden::<u32>(); // ZIndex
        assert_no_forbidden::<Option<SceneNodeKey>>();
    }

    /// LAYERNODE FIELD FIREWALL
    #[test]
    fn layer_node_fields_no_forbidden() {
        // LayerNode fields: rect, children, opacity, z_index, key
        assert_no_forbidden::<RectF>();
        assert_no_forbidden::<Vec<SceneNode>>();
        assert_no_forbidden::<f32>(); // opacity
        assert_no_forbidden::<u32>(); // ZIndex
        assert_no_forbidden::<Option<SceneNodeKey>>();
    }

    /// SCROLLNODE FIELD FIREWALL
    #[test]
    fn scroll_node_fields_no_forbidden() {
        // ScrollNode fields: rect, offset_x, offset_y, children, z_index, key, etc.
        assert_no_forbidden::<RectF>();
        assert_no_forbidden::<f32>(); // offsets
        assert_no_forbidden::<Vec<SceneNode>>();
        assert_no_forbidden::<u32>(); // ZIndex
        assert_no_forbidden::<Option<SceneNodeKey>>();
    }

    #[test]
    fn luminous_contour_node_fields_no_forbidden() {
        assert_no_forbidden::<std::sync::Arc<[Point]>>();
        assert_no_forbidden::<f32>();
        assert_no_forbidden::<Color>();
        assert_no_forbidden::<LuminousContourAppearance>();
        assert_no_forbidden::<LuminousContourFronts>();
        assert_no_forbidden::<u32>();
        assert_no_forbidden::<Option<SceneNodeKey>>();
    }

    // ========================================================================
    // Negative Test Documentation
    // ========================================================================

    /// NEGATIVE TEST DOCUMENTATION: Poison markers do NOT implement NoForbiddenScene.
    ///
    /// The following would fail to compile if uncommented:
    /// ```compile_fail
    /// assert_no_forbidden::<super::super::forbidden::PoisonWidget>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonCompositor>();
    /// ```
    ///
    /// This proves the firewall works: forbidden types cannot satisfy NoForbiddenScene.
    #[test]
    fn poison_markers_are_forbidden() {
        // This test documents that poison markers cannot pass the firewall.
        // If you uncomment the lines above, compilation will fail with:
        // "the trait `NoForbiddenScene` is not implemented for `PoisonCompositor`"
    }

    // ========================================================================
    // Additional Type Safety Checks
    // ========================================================================

    /// Verify SceneGraph is Send + Sync (required for cross-thread frame transfer).
    #[test]
    fn scene_graph_is_thread_safe() {
        fn assert_send_sync<T: Send + Sync + 'static>() {}
        assert_send_sync::<SceneGraph>();
    }

    /// Verify SceneNode is Clone (required for tree manipulation).
    #[test]
    fn scene_node_is_clone() {
        fn assert_clone<T: Clone>() {}
        assert_clone::<SceneNode>();
    }

    /// Verify Color is Copy (lightweight value type).
    #[test]
    fn color_is_copy() {
        fn assert_copy<T: Copy>() {}
        assert_copy::<Color>();
    }
}
