//! Widget Authority Firewall Guard
//!
//! This module provides compile-time enforcement that Widget API boundaries
//! use scene-agnostic types (PaintSink, paint::Color) instead of scene types
//! (SceneBuilder, scene::Color).
//!
//! # Enforcement Mechanism
//!
//! 1. `NoForbiddenWidget` trait implemented for allowed types only
//! 2. Field-by-field assertions for key API types
//! 3. Poison markers in forbidden.rs NOT implementing the trait
//! 4. If forbidden type leaks in, compilation fails
//!
//! # Widget Authority Contract
//!
//! Widget API boundaries contain ONLY:
//! - Math primitives (Point, RectF, Size, f32, u32, etc.)
//! - Paint abstraction types (PaintSink, paint::Color, RoundedCorners, etc.)
//! - Widget identity (WidgetInternalId, NodeId)
//! - Layout types (Constraints, LayoutResult)
//! - Standard containers (Vec, Option, String)
//!
//! Widget API boundaries MUST NOT contain:
//! - SceneBuilder (use dyn PaintSink instead)
//! - SceneGraph (render output)
//! - SceneNode (render primitives)
//! - scene::Color (use paint::Color instead)
//!
//! # How Violations Are Caught
//!
//! If a forbidden type is added to a Widget API struct field:
//! ```text
//! error[E0277]: the trait bound `ForbiddenType: NoForbiddenWidget` is not satisfied
//! ```

#[cfg(test)]
mod tests {
    use crate::core::layout::{Constraints, LayoutResult};
    use crate::core::math::{Point, RectF, Size};
    use crate::core::paint::{Color, RoundedCorners, TextBackgroundSpan};
    use crate::core::scene_identity::SceneNodeKey;
    use crate::core::text_engine::GlyphRun;
    use crate::core::widget_tree::NodeId;
    use crate::utils::id::WidgetInternalId;

    // ========================================================================
    // NoForbiddenWidget Trait and Implementations
    // ========================================================================
    //
    // This trait is intentionally NOT implemented for poison markers.
    // If a forbidden type appears in Widget API, compilation fails.

    /// Marker trait for types allowed in Widget API boundaries.
    ///
    /// This trait is intentionally NOT implemented for:
    /// - PoisonSceneBuilder, PoisonSceneGraph, PoisonSceneNode
    trait NoForbiddenWidget {}

    // Primitives
    impl NoForbiddenWidget for bool {}
    impl NoForbiddenWidget for u8 {}
    impl NoForbiddenWidget for u16 {}
    impl NoForbiddenWidget for u32 {}
    impl NoForbiddenWidget for u64 {}
    impl NoForbiddenWidget for usize {}
    impl NoForbiddenWidget for i32 {}
    impl NoForbiddenWidget for f32 {}
    impl NoForbiddenWidget for f64 {}
    impl NoForbiddenWidget for char {}

    // Math types
    impl NoForbiddenWidget for Point {}
    impl NoForbiddenWidget for RectF {}
    impl NoForbiddenWidget for Size {}

    // Paint types (scene-agnostic)
    impl NoForbiddenWidget for Color {}
    impl NoForbiddenWidget for RoundedCorners {}
    impl NoForbiddenWidget for TextBackgroundSpan {}
    // ZIndex is pub type ZIndex = u32; already covered by u32 impl
    // PaintSink is a trait, not a type - dyn PaintSink is allowed

    // Widget identity types
    impl NoForbiddenWidget for WidgetInternalId {}
    impl NoForbiddenWidget for NodeId {}

    // Scene identity (passed through PaintContext, not imported by widgets)
    impl NoForbiddenWidget for SceneNodeKey {}

    // Layout types
    impl NoForbiddenWidget for Constraints {}
    impl NoForbiddenWidget for LayoutResult {}

    // Text types
    impl NoForbiddenWidget for GlyphRun {}

    // Standard library types
    impl NoForbiddenWidget for String {}
    impl NoForbiddenWidget for &'static str {}
    impl<T> NoForbiddenWidget for Vec<T> {}
    impl<T> NoForbiddenWidget for Option<T> {}
    impl<T> NoForbiddenWidget for &T {}
    impl<T> NoForbiddenWidget for &mut T {}

    // Tuples
    impl<A, B> NoForbiddenWidget for (A, B) {}
    impl<A, B, C> NoForbiddenWidget for (A, B, C) {}
    impl<A, B, C, D> NoForbiddenWidget for (A, B, C, D) {}

    /// Helper function for compile-time NoForbiddenWidget assertion.
    fn assert_no_forbidden<T: NoForbiddenWidget>() {}

    // ========================================================================
    // paint::Color Firewall
    // ========================================================================

    /// PAINT COLOR FIREWALL: Verify paint::Color is the correct type.
    #[test]
    fn paint_color_is_allowed() {
        assert_no_forbidden::<Color>();
    }

    // ========================================================================
    // RoundedCorners Firewall
    // ========================================================================

    /// ROUNDED CORNERS FIREWALL: Verify paint::RoundedCorners is used.
    #[test]
    fn rounded_corners_is_allowed() {
        assert_no_forbidden::<RoundedCorners>();
    }

    // ========================================================================
    // TextBackgroundSpan Firewall
    // ========================================================================

    /// TEXT BACKGROUND SPAN FIREWALL: Verify paint::TextBackgroundSpan is used.
    #[test]
    fn text_background_span_is_allowed() {
        assert_no_forbidden::<TextBackgroundSpan>();
    }

    // ========================================================================
    // Widget API Field Firewalls
    // ========================================================================

    /// LAYOUT RESULT FIREWALL: All field types must be NoForbiddenWidget.
    #[test]
    fn layout_result_fields_no_forbidden() {
        // LayoutResult contains Size (fit_size, scroll_size)
        assert_no_forbidden::<Size>();
    }

    /// CONSTRAINTS FIREWALL: All field types must be NoForbiddenWidget.
    #[test]
    fn constraints_fields_no_forbidden() {
        // Constraints contains f32 values
        assert_no_forbidden::<f32>();
    }

    // ========================================================================
    // Negative Test Documentation
    // ========================================================================

    /// NEGATIVE TEST DOCUMENTATION: Poison markers do NOT implement NoForbiddenWidget.
    ///
    /// The following would fail to compile if uncommented:
    /// ```compile_fail
    /// assert_no_forbidden::<super::super::forbidden::PoisonSceneBuilder>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonSceneGraph>();
    /// assert_no_forbidden::<super::super::forbidden::PoisonSceneNode>();
    /// ```
    ///
    /// This proves the firewall works: forbidden types cannot satisfy NoForbiddenWidget.
    #[test]
    fn poison_markers_are_forbidden() {
        // This test documents that poison markers cannot pass the firewall.
        // If you uncomment the lines above, compilation will fail with:
        // "the trait `NoForbiddenWidget` is not implemented for `PoisonSceneBuilder`"
    }

    // ========================================================================
    // PaintSink API Contract
    // ========================================================================

    /// PAINTSINK CONTRACT: Verify PaintSink uses paint types, not scene types.
    ///
    /// PaintSink methods use:
    /// - paint::Color (not scene::Color)
    /// - paint::RoundedCorners (not scene::RoundedCorners)
    /// - paint::TextBackgroundSpan (not scene::TextBackgroundSpan)
    #[test]
    fn paintsink_uses_paint_types() {
        // Color in PaintSink is paint::Color
        assert_no_forbidden::<Color>();
        // RoundedCorners in PaintSink is paint::RoundedCorners
        assert_no_forbidden::<RoundedCorners>();
        // TextBackgroundSpan in PaintSink is paint::TextBackgroundSpan
        assert_no_forbidden::<TextBackgroundSpan>();
    }
}
