//! SPEC-HAL-190: Engine-Level Phase Enforcement Tests
//!
//! This module proves SPEC-HAL-190 works inside the real frame loop with:
//! - Real Widget implementations
//! - Real WidgetTree
//! - Real phase contexts (LayoutContext, EventContext, PaintContext)
//! - Real authority registration
//! - Real intent routing
//! - Real deferred mutation
//!
//! These tests are in src/tests/ because they require:
//! - Access to internal WidgetTree methods
//! - Access to `TreeWidgetId::from_raw` (pub(crate))
//! - Direct manipulation of the frame loop phases

#![cfg(test)]

use std::any::TypeId;
use std::collections::HashMap;
use std::sync::Arc;

use crate::core::authority::AuthorityOwner;
use crate::core::event::{
    Event, EventPhase, PointerButton, PointerEvent, PointerEventKind, PointerId, PointerType,
};
use crate::core::layout::Constraints;
use crate::core::math::RectF;
use crate::core::paint::{
    Color, LuminousContourAppearance, LuminousContourError, LuminousContourFronts, PaintSink,
    RoundedCorners, TextBackgroundSpan, ZIndex,
};
use crate::core::scene_identity::SceneNodeKey;
use crate::core::text_engine::{GlyphRun, TextEngine, TextMetrics};
use crate::core::theme::Theme;
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget};
use crate::core::widget_tree::WidgetTree;
use crate::tests::_support::authority_probe_demo::{
    AuthorityProbeComposite, BodyProbe, FooterProbe, HeaderProbe, IncrementCounterIntent,
    ProbeAuthority, SetCounterIntent,
};

/// Helper to create test theme
fn test_theme() -> Theme {
    Theme::default()
}

/// Null text engine for testing - does nothing
struct NullTextEngine;

impl TextEngine for NullTextEngine {
    fn measure(&mut self, _text: &str, _font_size: f32) -> (f32, f32) {
        (0.0, 0.0)
    }

    fn metrics(&mut self, _text: &str, _font_size: f32) -> TextMetrics {
        TextMetrics {
            ascent: 0.0,
            descent: 0.0,
            line_gap: 0.0,
            line_height: 0.0,
        }
    }

    fn shape(&mut self, _text: &str, _font_size: f32) -> GlyphRun {
        GlyphRun {
            binding: None,
            clusters: Vec::new(),
            glyphs: Vec::new(),
            width: 0.0,
            height: 0.0,
            text: String::new(),
            font_size: 0.0,
            ascent: 0.0,
            descent: 0.0,
        }
    }

    fn draw(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _z_index: crate::core::paint::ZIndex,
        _sink: &mut dyn PaintSink,
    ) {
    }

    fn draw_with_backgrounds(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _backgrounds: Vec<TextBackgroundSpan>,
        _z_index: crate::core::paint::ZIndex,
        _sink: &mut dyn PaintSink,
    ) {
    }

    fn font_metrics(&mut self, _font_size: f32) -> TextMetrics {
        TextMetrics {
            ascent: 0.0,
            descent: 0.0,
            line_gap: 0.0,
            line_height: 0.0,
        }
    }
}

/// Void paint sink for testing - does nothing
struct VoidPaintSink;

impl PaintSink for VoidPaintSink {
    fn scale_factor(&self) -> f32 {
        1.0
    }

    fn begin_clip(&mut self, _rect: RectF, _rounded: Option<RoundedCorners>) {}
    fn end_clip(&mut self) {}
    fn begin_scroll(&mut self, _offset: (f32, f32)) {}
    fn end_scroll(&mut self) {}

    fn push_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_border_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _border_width: f32,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_rounded_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _corners: RoundedCorners,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_border_rounded_rect(
        &mut self,
        _rect: RectF,
        _color: Color,
        _border_width: f32,
        _corners: RoundedCorners,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_shadow(
        &mut self,
        _shape_rect: RectF,
        _shape_radius: f32,
        _mode: crate::core::paint::ShadowMode,
        _offset: (f32, f32),
        _spread: f32,
        _blur_radius: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_gradient_rect(
        &mut self,
        _rect: RectF,
        _radius: f32,
        _gradient: &crate::core::paint::GradientDef,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_color_picker_sv(
        &mut self,
        _rect: RectF,
        _hue: f32,
        _radius: f32,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_radial_gauge(
        &mut self,
        _rect: RectF,
        _fill: f32,
        _thickness: f32,
        _track_color: Color,
        _fill_color: Color,
        _fill_color_negative: Color,
        _start_angle: f32,
        _sweep_angle: f32,
        _origin_fill: f32,
        _mode: u32,
        _fill_style: u32,
        _zone_count: u32,
        _zones: [(f32, f32, [f32; 4]); 4],
        _major_divisions: u32,
        _minor_per_major: u32,
        _tick_length_major: f32,
        _tick_length_minor: f32,
        _tick_color: [f32; 4],
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_circle(
        &mut self,
        _center: crate::core::math::Point,
        _radius: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_ellipse(
        &mut self,
        _center: crate::core::math::Point,
        _radii: crate::core::math::Point,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_text_with_backgrounds(
        &mut self,
        _run: &GlyphRun,
        _x: f32,
        _y: f32,
        _color: Color,
        _backgrounds: Vec<TextBackgroundSpan>,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_path(
        &mut self,
        _path: crate::renderer::path::Path,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_icon(
        &mut self,
        _definition: &crate::core::icon::IconDefinition,
        _x: f32,
        _y: f32,
        _size: f32,
        _color: Color,
        _z_index: ZIndex,
    ) {
    }

    fn push_image(
        &mut self,
        _rect: RectF,
        _texture_id: u64,
        _uv_rect: RectF,
        _tint: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_target_bound_image(
        &mut self,
        _rect: RectF,
        _presentation_id: crate::renderer::CustomRenderPresentationId,
        _uv_rect: RectF,
        _tint: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_line(
        &mut self,
        _a: crate::core::math::Point,
        _b: crate::core::math::Point,
        _width: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_polyline(
        &mut self,
        _points: Vec<crate::core::math::Point>,
        _width: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }

    fn push_luminous_contour(
        &mut self,
        _points: Arc<[crate::core::math::Point]>,
        _core_width: f32,
        _color: Color,
        _appearance: LuminousContourAppearance,
        _fronts: LuminousContourFronts,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) -> Result<(), LuminousContourError> {
        Ok(())
    }

    fn set_current_painting_widget(&mut self, _widget_id: u64) {}
    fn clear_current_painting_widget(&mut self) {}

    #[cfg(debug_assertions)]
    fn push_text_probe(
        &mut self,
        _position: (f32, f32),
        _text: &str,
        _font_size: f32,
        _color: Color,
        _z_index: ZIndex,
        _key: Option<SceneNodeKey>,
    ) {
    }
}

// ============================================================================
// Test: Authority Registration and Lookup
// ============================================================================

#[test]
fn test_authority_registration_and_lookup() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create composite (authority owner)
    let composite = AuthorityProbeComposite::new(100);

    // Add to tree - this automatically registers authority
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Verify the widget was added
    assert!(tree.node(root).is_some());

    // Verify authority is registered by doing a lookup
    // If authority wasn't registered, lookup_authority would return None
    let projection = tree.lookup_authority::<ProbeAuthority>(root);
    assert!(
        projection.is_some(),
        "Authority should be registered and accessible"
    );
    assert_eq!(projection.unwrap().get().counter, 100);
}

// ============================================================================
// Test: Authority Lookup in Layout Phase
// ============================================================================

#[test]
fn test_authority_lookup_in_layout() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = test_theme();
    let mut text_engine = NullTextEngine;

    // Create composite with counter = 42
    let composite = AuthorityProbeComposite::new(42);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add HeaderProbe as child
    let header = HeaderProbe::new(30.0);
    let header_id = tree.add_child(root, Box::new(header), None, 0);
    tree.set_rect(header_id, RectF::new(0.0, 0.0, 100.0, 30.0));

    // Take widget to call layout directly
    let (mut widget, children, _rect) = tree.take_widget(header_id).unwrap();

    // Create layout context
    let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
    cx.set_current_node(header_id);

    // Call layout
    let constraints = Constraints::new(0.0, 100.0, 0.0, 100.0);
    let _result = widget.layout(&mut cx, constraints, &children);

    // Restore widget
    tree.restore_widget(header_id, widget, RectF::new(0.0, 0.0, 100.0, 30.0));

    // Verify HeaderProbe read the authority
    let header_widget = tree
        .node(header_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<HeaderProbe>()
        .unwrap();
    assert_eq!(header_widget.last_layout_counter.get(), 42);

    // Verify authority tracked the layout read
    let composite_widget = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(composite_widget.authority().layout_reads.get(), 1);
}

// ============================================================================
// Test: Intent Emission in Event Phase
// ============================================================================

#[test]
fn test_intent_emission_in_event() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create composite with counter = 0
    let composite = AuthorityProbeComposite::new(0);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add BodyProbe as child
    let body = BodyProbe::new(5, 50.0);
    let body_id = tree.add_child(root, Box::new(body), None, 0);
    tree.set_rect(body_id, RectF::new(0.0, 0.0, 100.0, 50.0));

    // Get TreeWidgetId for the body
    let body_tree_id = tree.node(body_id).unwrap().id;

    // Create event context
    let mut cx = EventContext::new(
        body_tree_id,
        &mut tree as *mut _,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false, // SPEC-HAL-268: Not relevant for tests
        crate::core::event::KeyboardAuthorityMode::Navigation,
    );

    // Create pointer down event
    let event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        local_pos: (25.0, 25.0),
        global_pos: (25.0, 25.0),
        modifiers: Default::default(),
        pointer_type: PointerType::Mouse,
    });

    // Handle event on BodyProbe
    let node = tree.node_mut(body_id).unwrap();
    let handled = node
        .widget
        .handle_event(&mut cx, &event, EventPhase::Target);

    assert!(handled, "BodyProbe should handle pointer down");

    // Verify intent was emitted
    let body_widget = tree
        .node(body_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<BodyProbe>()
        .unwrap();
    assert_eq!(body_widget.intents_emitted.get(), 1);

    // Verify intent is in the buffer (not processed yet)
    // Get the root tree id first, then check buffer
    let root_tree_id = tree.node(root).unwrap().id;
    let buffers = tree.intent_buffers_mut();
    let buffer = buffers.buffer_for(root_tree_id);
    assert!(!buffer.is_empty(), "Intent buffer should have the intent");
}

// ============================================================================
// Test: Deferred Mutation via process_intents
// ============================================================================

#[test]
fn test_deferred_mutation() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create composite with counter = 10
    let composite = AuthorityProbeComposite::new(10);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add BodyProbe as child
    let body = BodyProbe::new(7, 50.0);
    let body_id = tree.add_child(root, Box::new(body), None, 0);
    tree.set_rect(body_id, RectF::new(0.0, 0.0, 100.0, 50.0));

    // Emit intent (simulate event handling)
    {
        let body_tree_id = tree.node(body_id).unwrap().id;
        let mut cx = EventContext::new(
            body_tree_id,
            &mut tree as *mut _,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false, // SPEC-HAL-268: Not relevant for tests
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            local_pos: (25.0, 25.0),
            global_pos: (25.0, 25.0),
            modifiers: Default::default(),
            pointer_type: PointerType::Mouse,
        });

        let node = tree.node_mut(body_id).unwrap();
        node.widget
            .handle_event(&mut cx, &event, EventPhase::Target);
    }

    // Verify counter is still 10 (not yet mutated)
    {
        let composite = tree
            .node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<AuthorityProbeComposite>()
            .unwrap();
        assert_eq!(
            composite.authority().counter,
            10,
            "Counter should not change during event phase"
        );
    }

    // Run deferred mutation phase (drain and process intents)
    let pending_intents = tree.drain_intent_buffers();
    assert_eq!(
        pending_intents.len(),
        1,
        "Should have one owner with intents"
    );

    for (owner_id, intents) in pending_intents {
        if let Some(node_id) = tree.find_by_id(owner_id) {
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
    }

    // Verify counter is now 17 (10 + 7)
    {
        let composite = tree
            .node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<AuthorityProbeComposite>()
            .unwrap();
        assert_eq!(
            composite.authority().counter,
            17,
            "Counter should be updated after deferred mutation"
        );
        assert_eq!(composite.process_intents_calls.get(), 1);
        assert_eq!(composite.authority().mutation_count.get(), 1);
    }
}

// ============================================================================
// Test: Authority Lookup in Paint Phase
// ============================================================================

#[test]
fn test_authority_lookup_in_paint() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = test_theme();
    let mut text_engine = NullTextEngine;

    // Create composite with counter = 77
    let composite = AuthorityProbeComposite::new(77);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add FooterProbe as child
    let footer = FooterProbe::new(30.0);
    let footer_id = tree.add_child(root, Box::new(footer), None, 0);
    tree.set_rect(footer_id, RectF::new(0.0, 0.0, 100.0, 30.0));

    // Create paint context
    let mut void_sink = VoidPaintSink;
    let timeline = crate::core::animation::Timeline::new();
    let interpolated_rects: HashMap<_, _> = HashMap::new();
    let mut overlay_regions = Vec::new();
    let mut tooltip_triggers = Vec::new();
    let icon_registry = crate::core::icons::builtin_registry();
    let mut cx = PaintContext::new(
        &tree,
        &mut void_sink,
        &theme,
        &mut text_engine,
        &icon_registry,
        0,
        &timeline,
        &interpolated_rects,
        &mut overlay_regions,
        &mut tooltip_triggers,
        1.0,
        None,
        crate::core::event::KeyboardAuthorityMode::Navigation,
    );
    cx.set_current_node(footer_id);

    // Call paint
    let rect = RectF::new(0.0, 0.0, 100.0, 30.0);
    let node = tree.node(footer_id).unwrap();
    node.widget.paint(&mut cx, rect, &[]);

    // Verify FooterProbe read the authority
    let footer_widget = tree
        .node(footer_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<FooterProbe>()
        .unwrap();
    assert_eq!(footer_widget.last_paint_counter.get(), 77);

    // Verify authority tracked the paint read
    let composite_widget = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(composite_widget.authority().paint_reads.get(), 1);
}

// ============================================================================
// Test: Full Phase Sequence (Event → Mutation → Layout → Paint)
// ============================================================================

#[test]
fn test_full_phase_sequence() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = test_theme();
    let mut text_engine = NullTextEngine;

    // Create composite with counter = 0
    let composite = AuthorityProbeComposite::new(0);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 110.0));

    // Add children
    let header = HeaderProbe::new(30.0);
    let header_id = tree.add_child(root, Box::new(header), None, 0);
    tree.set_rect(header_id, RectF::new(0.0, 0.0, 100.0, 30.0));

    let body = BodyProbe::new(10, 50.0);
    let body_id = tree.add_child(root, Box::new(body), None, 1);
    tree.set_rect(body_id, RectF::new(0.0, 30.0, 100.0, 50.0));

    let footer = FooterProbe::new(30.0);
    let footer_id = tree.add_child(root, Box::new(footer), None, 2);
    tree.set_rect(footer_id, RectF::new(0.0, 80.0, 100.0, 30.0));

    // ========================================================================
    // PHASE 1: EVENT - Emit intent
    // ========================================================================
    {
        let body_tree_id = tree.node(body_id).unwrap().id;
        let mut cx = EventContext::new(
            body_tree_id,
            &mut tree as *mut _,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false, // SPEC-HAL-268: Not relevant for tests
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            local_pos: (25.0, 25.0),
            global_pos: (25.0, 55.0),
            modifiers: Default::default(),
            pointer_type: PointerType::Mouse,
        });

        let node = tree.node_mut(body_id).unwrap();
        let handled = node
            .widget
            .handle_event(&mut cx, &event, EventPhase::Target);
        assert!(handled);
    }

    // Verify counter unchanged during event
    {
        let composite = tree
            .node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<AuthorityProbeComposite>()
            .unwrap();
        assert_eq!(
            composite.authority().counter,
            0,
            "Counter should be 0 during event phase"
        );
    }

    // ========================================================================
    // PHASE 2: DEFERRED MUTATION - Process intents
    // ========================================================================
    let pending = tree.drain_intent_buffers();
    for (owner_id, intents) in pending {
        if let Some(node_id) = tree.find_by_id(owner_id) {
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
    }

    // Verify counter is now 10
    {
        let composite = tree
            .node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<AuthorityProbeComposite>()
            .unwrap();
        assert_eq!(
            composite.authority().counter,
            10,
            "Counter should be 10 after deferred mutation"
        );
    }

    // ========================================================================
    // PHASE 3: LAYOUT - HeaderProbe reads updated counter
    // ========================================================================
    {
        let (mut widget, children, _rect) = tree.take_widget(header_id).unwrap();
        let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        cx.set_current_node(header_id);
        let constraints = Constraints::new(0.0, 100.0, 0.0, 100.0);
        let _result = widget.layout(&mut cx, constraints, &children);
        tree.restore_widget(header_id, widget, RectF::new(0.0, 0.0, 100.0, 30.0));
    }

    // Verify HeaderProbe observed the updated counter
    {
        let header = tree
            .node(header_id)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<HeaderProbe>()
            .unwrap();
        assert_eq!(
            header.last_layout_counter.get(),
            10,
            "HeaderProbe should see counter=10 in layout"
        );
    }

    // ========================================================================
    // PHASE 4: PAINT - FooterProbe reads updated counter
    // ========================================================================
    {
        let mut void_sink = VoidPaintSink;
        let timeline = crate::core::animation::Timeline::new();
        let interpolated_rects: HashMap<_, _> = HashMap::new();
        let mut overlay_regions = Vec::new();
        let mut tooltip_triggers = Vec::new();
        let icon_registry = crate::core::icons::builtin_registry();
        let mut cx = PaintContext::new(
            &tree,
            &mut void_sink,
            &theme,
            &mut text_engine,
            &icon_registry,
            0,
            &timeline,
            &interpolated_rects,
            &mut overlay_regions,
            &mut tooltip_triggers,
            1.0,
            None,
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );
        cx.set_current_node(footer_id);

        let rect = RectF::new(0.0, 80.0, 100.0, 30.0);
        let node = tree.node(footer_id).unwrap();
        node.widget.paint(&mut cx, rect, &[]);
    }

    // Verify FooterProbe observed the updated counter
    {
        let footer = tree
            .node(footer_id)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<FooterProbe>()
            .unwrap();
        assert_eq!(
            footer.last_paint_counter.get(),
            10,
            "FooterProbe should see counter=10 in paint"
        );
    }

    // ========================================================================
    // VERIFICATION: Phase tracking
    // ========================================================================
    {
        let composite = tree
            .node(root)
            .unwrap()
            .widget
            .as_any()
            .downcast_ref::<AuthorityProbeComposite>()
            .unwrap();
        let auth = composite.authority();

        // One layout read (HeaderProbe)
        assert_eq!(auth.layout_reads.get(), 1);

        // One paint read (FooterProbe)
        assert_eq!(auth.paint_reads.get(), 1);

        // One event read (BodyProbe checked authority during event)
        assert_eq!(auth.event_reads.get(), 1);

        // One mutation (from BodyProbe's intent)
        assert_eq!(auth.mutation_count.get(), 1);
    }
}

// ============================================================================
// Test: Multiple Intents in Single Frame
// ============================================================================

#[test]
fn test_multiple_intents_in_frame() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create composite with counter = 0
    let composite = AuthorityProbeComposite::new(0);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add two BodyProbes
    let body1 = BodyProbe::new(5, 50.0);
    let body1_id = tree.add_child(root, Box::new(body1), None, 0);
    tree.set_rect(body1_id, RectF::new(0.0, 0.0, 100.0, 50.0));

    let body2 = BodyProbe::new(7, 50.0);
    let body2_id = tree.add_child(root, Box::new(body2), None, 1);
    tree.set_rect(body2_id, RectF::new(0.0, 50.0, 100.0, 50.0));

    // Both bodies emit intents
    for body_id in [body1_id, body2_id] {
        let body_tree_id = tree.node(body_id).unwrap().id;
        let mut cx = EventContext::new(
            body_tree_id,
            &mut tree as *mut _,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false, // SPEC-HAL-268: Not relevant for tests
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            local_pos: (25.0, 25.0),
            global_pos: (25.0, 25.0),
            modifiers: Default::default(),
            pointer_type: PointerType::Mouse,
        });

        let node = tree.node_mut(body_id).unwrap();
        node.widget
            .handle_event(&mut cx, &event, EventPhase::Target);
    }

    // Process all intents
    let pending = tree.drain_intent_buffers();
    for (owner_id, intents) in pending {
        if let Some(node_id) = tree.find_by_id(owner_id) {
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
    }

    // Verify counter is 12 (5 + 7)
    let composite = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(composite.authority().counter, 12);
    assert_eq!(
        composite.process_intents_calls.get(),
        1,
        "process_intents called once per frame"
    );
    assert_eq!(
        composite.authority().mutation_count.get(),
        2,
        "Two mutations from two intents"
    );
}

// ============================================================================
// Test: Intent Best-Effort (Owner Removed Before Mutation)
// ============================================================================

#[test]
fn test_intent_best_effort_owner_removed() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create composite with counter = 0
    let composite = AuthorityProbeComposite::new(0);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add BodyProbe
    let body = BodyProbe::new(5, 50.0);
    let body_id = tree.add_child(root, Box::new(body), None, 0);
    tree.set_rect(body_id, RectF::new(0.0, 0.0, 100.0, 50.0));

    // Emit intent
    {
        let body_tree_id = tree.node(body_id).unwrap().id;
        let mut cx = EventContext::new(
            body_tree_id,
            &mut tree as *mut _,
            0,
            std::ptr::null_mut(),
            None,
            None,
            false,
            false, // SPEC-HAL-268: Not relevant for tests
            crate::core::event::KeyboardAuthorityMode::Navigation,
        );

        let event = Event::Pointer(PointerEvent {
            id: PointerId::MOUSE,
            kind: PointerEventKind::Down,
            button: Some(PointerButton::Primary),
            local_pos: (25.0, 25.0),
            global_pos: (25.0, 25.0),
            modifiers: Default::default(),
            pointer_type: PointerType::Mouse,
        });

        let node = tree.node_mut(body_id).unwrap();
        node.widget
            .handle_event(&mut cx, &event, EventPhase::Target);
    }

    // Drain intents
    let pending = tree.drain_intent_buffers();
    assert!(!pending.is_empty(), "Should have pending intents");

    // Process with a non-existent owner_id
    // INV-INTENT-BEST-EFFORT: Intents are silently dropped if owner is removed
    for (_owner_id, intents) in pending {
        // Simulate owner being removed by checking with a non-existent id
        let fake_owner_id = crate::utils::id::TreeWidgetId::from_raw(99999);
        if let Some(node_id) = tree.find_by_id(fake_owner_id) {
            // This branch won't execute because owner doesn't exist
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
        // Intents are dropped silently - no panic
    }

    // Verify nothing bad happened - counter still 0
    let composite = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        composite.authority().counter,
        0,
        "Counter unchanged when owner not found"
    );
}

// ============================================================================
// Test: Authority Ancestor-Only Lookup (No Sibling Access)
// ============================================================================

#[test]
fn test_authority_ancestor_only_no_sibling_access() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = test_theme();
    let mut text_engine = NullTextEngine;

    // Create root container (not an authority owner) - using FixedSpacer as simple non-authority root
    let root_spacer = crate::widgets::fixed_spacer::FixedSpacer::new(200.0, 200.0);
    let root = tree.create_root(Box::new(root_spacer), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 200.0, 200.0));

    // Create two siblings, each with their own authority
    let composite1 = AuthorityProbeComposite::new(111);
    let composite1_id = tree.add_child(root, Box::new(composite1), None, 0);
    tree.set_rect(composite1_id, RectF::new(0.0, 0.0, 100.0, 100.0));

    let composite2 = AuthorityProbeComposite::new(222);
    let composite2_id = tree.add_child(root, Box::new(composite2), None, 1);
    tree.set_rect(composite2_id, RectF::new(100.0, 0.0, 100.0, 100.0));

    // Add HeaderProbe to composite1
    let header = HeaderProbe::new(30.0);
    let header_id = tree.add_child(composite1_id, Box::new(header), None, 0);
    tree.set_rect(header_id, RectF::new(0.0, 0.0, 100.0, 30.0));

    // Layout HeaderProbe (child of composite1)
    {
        let (mut widget, children, _rect) = tree.take_widget(header_id).unwrap();
        let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        cx.set_current_node(header_id);
        let constraints = Constraints::new(0.0, 100.0, 0.0, 100.0);
        let _result = widget.layout(&mut cx, constraints, &children);
        tree.restore_widget(header_id, widget, RectF::new(0.0, 0.0, 100.0, 30.0));
    }

    // Verify HeaderProbe saw composite1's authority (111), NOT composite2's (222)
    let header = tree
        .node(header_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<HeaderProbe>()
        .unwrap();
    assert_eq!(
        header.last_layout_counter.get(),
        111,
        "Header should see ancestor authority, not sibling"
    );

    // Verify composite1 had layout read, composite2 did not
    let composite1 = tree
        .node(composite1_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(composite1.authority().layout_reads.get(), 1);

    let composite2 = tree
        .node(composite2_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        composite2.authority().layout_reads.get(),
        0,
        "Sibling authority should not be accessed"
    );
}

// ============================================================================
// Test: INV-AUTH-LOOKUP-2 - Nearest Ancestor Wins
// ============================================================================

#[test]
fn test_nearest_ancestor_wins() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();
    let theme = test_theme();
    let mut text_engine = NullTextEngine;

    // Build 3-level chain: RootOwner(1) → MidOwner(2) → LeafProbe
    // LeafProbe should see MidOwner's authority (2), not RootOwner's (1)

    // Level 1: Root authority owner with counter = 1
    let root_owner = AuthorityProbeComposite::new(1);
    let root_id = tree.create_root(Box::new(root_owner), None, 0);
    tree.set_rect(root_id, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Level 2: Mid authority owner with counter = 2
    let mid_owner = AuthorityProbeComposite::new(2);
    let mid_id = tree.add_child(root_id, Box::new(mid_owner), None, 0);
    tree.set_rect(mid_id, RectF::new(0.0, 0.0, 100.0, 80.0));

    // Level 3: Leaf probe that reads authority
    let leaf = HeaderProbe::new(30.0);
    let leaf_id = tree.add_child(mid_id, Box::new(leaf), None, 0);
    tree.set_rect(leaf_id, RectF::new(0.0, 0.0, 100.0, 30.0));

    // Layout the leaf probe (triggers authority lookup)
    {
        let (mut widget, children, _rect) = tree.take_widget(leaf_id).unwrap();
        let mut cx = LayoutContext::new(&mut tree, &mut text_engine, &theme);
        cx.set_current_node(leaf_id);
        let constraints = Constraints::new(0.0, 100.0, 0.0, 100.0);
        let _result = widget.layout(&mut cx, constraints, &children);
        tree.restore_widget(leaf_id, widget, RectF::new(0.0, 0.0, 100.0, 30.0));
    }

    // Verify leaf saw nearest ancestor (mid_owner with counter=2)
    let leaf = tree
        .node(leaf_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<HeaderProbe>()
        .unwrap();
    assert_eq!(
        leaf.last_layout_counter.get(),
        2,
        "Leaf should see nearest ancestor's authority (mid=2), not root (1)"
    );

    // Verify mid_owner was read, root_owner was not
    let mid = tree
        .node(mid_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        mid.authority().layout_reads.get(),
        1,
        "Mid owner should have been read"
    );

    let root = tree
        .node(root_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        root.authority().layout_reads.get(),
        0,
        "Root owner should NOT have been read (nearest wins)"
    );
}

// ============================================================================
// Test: Empty Buffer - process_intents NOT Called
// ============================================================================

#[test]
fn test_empty_buffer_no_process_intents_call() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create authority owner
    let composite = AuthorityProbeComposite::new(50);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // DO NOT emit any intents - buffer should be empty

    // Run the same drain+process logic used in deferred mutation
    let pending = tree.drain_intent_buffers();

    // Process any pending intents (there should be none)
    for (owner_id, intents) in pending {
        if let Some(node_id) = tree.find_by_id(owner_id) {
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
    }

    // Verify process_intents was NOT called
    let composite = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        composite.process_intents_calls.get(),
        0,
        "process_intents should NOT be called when buffer is empty"
    );
    assert_eq!(
        composite.authority().mutation_count.get(),
        0,
        "No mutations should have occurred"
    );
    assert_eq!(
        composite.authority().counter,
        50,
        "Counter should remain unchanged"
    );
}

// ============================================================================
// Test: Intent Ordering Preserved Within Single Owner
// ============================================================================

#[test]
fn test_intent_ordering_preserved() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create authority owner with counter = 0
    let composite = AuthorityProbeComposite::new(0);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Get owner's TreeWidgetId for direct buffer access
    let root_tree_id = tree.node(root).unwrap().id;

    // Emit intents in specific order: Set(100), then Inc(5)
    // If order preserved: result = 100 + 5 = 105
    // If order reversed: result = 0 + 5 = 5, then set to 100
    {
        let buffers = tree.intent_buffers_mut();
        let buffer = buffers.buffer_for(root_tree_id);
        buffer.push(Box::new(SetCounterIntent { value: 100 }));
        buffer.push(Box::new(IncrementCounterIntent { delta: 5 }));
    }

    // Process intents
    let pending = tree.drain_intent_buffers();
    for (owner_id, intents) in pending {
        if let Some(node_id) = tree.find_by_id(owner_id) {
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
    }

    // Verify order was preserved: Set(100) then Inc(5) = 105
    let composite = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        composite.authority().counter,
        105,
        "Intent order must be preserved: Set(100) then Inc(5) = 105"
    );
}

#[test]
fn test_intent_ordering_reversed_gives_different_result() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create authority owner with counter = 0
    let composite = AuthorityProbeComposite::new(0);
    let root = tree.create_root(Box::new(composite), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Get owner's TreeWidgetId for direct buffer access
    let root_tree_id = tree.node(root).unwrap().id;

    // Emit intents in REVERSED order: Inc(5), then Set(100)
    // Result should be: 0 + 5 = 5, then set to 100 = 100
    {
        let buffers = tree.intent_buffers_mut();
        let buffer = buffers.buffer_for(root_tree_id);
        buffer.push(Box::new(IncrementCounterIntent { delta: 5 }));
        buffer.push(Box::new(SetCounterIntent { value: 100 }));
    }

    // Process intents
    let pending = tree.drain_intent_buffers();
    for (owner_id, intents) in pending {
        if let Some(node_id) = tree.find_by_id(owner_id) {
            if let Some(node) = tree.node_mut(node_id) {
                if let Some(owner) = node.widget.as_authority_owner() {
                    owner.process_intents(intents);
                }
            }
        }
    }

    // Verify reversed order gives different result: Inc(5) then Set(100) = 100
    let composite = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<AuthorityProbeComposite>()
        .unwrap();
    assert_eq!(
        composite.authority().counter,
        100,
        "Reversed order: Inc(5) then Set(100) = 100 (proves order matters)"
    );
}

// ============================================================================
// Test: emit_intent Returns False When Authority Not Found
// ============================================================================

#[test]
fn test_emit_intent_returns_false_when_no_authority() {
    let _guard = crate::core::rect_authority::LayoutAuthorityGuard::enter();
    let mut tree = WidgetTree::new();

    // Create a root that does NOT provide ProbeAuthority (using FixedSpacer)
    let root_spacer = crate::widgets::fixed_spacer::FixedSpacer::new(100.0, 100.0);
    let root = tree.create_root(Box::new(root_spacer), None, 0);
    tree.set_rect(root, RectF::new(0.0, 0.0, 100.0, 100.0));

    // Add BodyProbe as child (it will try to emit intent to ProbeAuthority)
    let body = BodyProbe::new(5, 50.0);
    let body_id = tree.add_child(root, Box::new(body), None, 0);
    tree.set_rect(body_id, RectF::new(0.0, 0.0, 100.0, 50.0));

    // Attempt to emit intent via event handling
    let body_tree_id = tree.node(body_id).unwrap().id;
    let mut cx = EventContext::new(
        body_tree_id,
        &mut tree as *mut _,
        0,
        std::ptr::null_mut(),
        None,
        None,
        false,
        false, // SPEC-HAL-268: Not relevant for tests
        crate::core::event::KeyboardAuthorityMode::Navigation,
    );

    let event = Event::Pointer(PointerEvent {
        id: PointerId::MOUSE,
        kind: PointerEventKind::Down,
        button: Some(PointerButton::Primary),
        local_pos: (25.0, 25.0),
        global_pos: (25.0, 25.0),
        modifiers: Default::default(),
        pointer_type: PointerType::Mouse,
    });

    // Handle event - BodyProbe will try to emit intent
    let node = tree.node_mut(body_id).unwrap();
    let _handled = node
        .widget
        .handle_event(&mut cx, &event, EventPhase::Target);

    // Verify emit_intent returned false (BodyProbe tracks successful emissions)
    let body_widget = tree
        .node(body_id)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<BodyProbe>()
        .unwrap();
    assert_eq!(
        body_widget.intents_emitted.get(),
        0,
        "emit_intent should return false when no authority owner exists"
    );

    // Verify intent buffers are empty
    assert!(
        tree.intent_buffers_mut().all_empty(),
        "No intents should be buffered when authority not found"
    );
}
