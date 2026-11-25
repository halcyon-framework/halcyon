use crate::compositor::SurfaceId;
use crate::core::app::{RetainedSceneTestHarness, layout_tree, test_begin_frame_layout_tracking};
use crate::core::icon::IconRegistry;
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size};
use crate::core::paint::Color;
use crate::core::rect_authority::LayoutAuthorityGuard;
use crate::core::theme::Theme;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::{NodeId, WidgetTree};
use crate::platform::font_manager_text_engine::FontManagerTextEngine;
use crate::plot::Plot;
use crate::plot::hit_proxy::HitProxy;
use crate::plot::series::Series;
use crate::plot::transform::Transform;
use crate::renderer::custom_render_pass::{CustomRenderAdmission, audit as gpu_audit};
use crate::renderer::custom_render_prepare::{
    CustomRenderActivity, CustomRenderParticipant, CustomRenderPreparationError,
    CustomRenderPreparationOutcome, CustomRenderPresentationBindings, CustomRenderTargetMetrics,
    CustomRenderTargetSnapshot, audit as preparation_audit,
};
use crate::renderer::pixel_coverage::{
    PhysicalExtent, PhysicalRect, PhysicalTargetMetrics, physical_composition_coverage,
};
use crate::renderer::render_graph::FrameConstants;
use crate::renderer::wgpu_renderer::WgpuRenderer;
use crate::utils::id::WidgetInternalId;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PhaseEvent {
    Layout,
    Prepare,
    Paint,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ProbeResourceKey {
    scale_factor_bits: u32,
    target_physical_extent: PhysicalExtent,
    physical_render_extent: PhysicalExtent,
}

#[derive(Default)]
struct ProbeAudit {
    events: Vec<PhaseEvent>,
    metrics: Vec<CustomRenderTargetMetrics>,
    resources: HashMap<SurfaceId, ProbeResourceKey>,
    allocations: HashMap<SurfaceId, usize>,
    publish_next_front: bool,
}

struct ParticipantProbe {
    id: WidgetInternalId,
    audit: Arc<Mutex<ProbeAudit>>,
    admission: Option<CustomRenderAdmission>,
}

impl ParticipantProbe {
    fn new(audit: Arc<Mutex<ProbeAudit>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            audit,
            admission: Some(CustomRenderAdmission::for_test()),
        }
    }

    fn new_unadmitted(audit: Arc<Mutex<ProbeAudit>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            audit,
            admission: None,
        }
    }
}

impl CustomRenderParticipant for ParticipantProbe {
    fn prepare_custom_render(
        &mut self,
        metrics: &CustomRenderTargetMetrics,
    ) -> CustomRenderPreparationOutcome {
        let mut audit = self.audit.lock().unwrap();
        audit.events.push(PhaseEvent::Prepare);
        audit.metrics.push(*metrics);
        if let CustomRenderActivity::Active {
            physical_render_extent,
            ..
        } = metrics.activity
        {
            let key = ProbeResourceKey {
                scale_factor_bits: metrics.scale_factor.to_bits(),
                target_physical_extent: metrics.target_physical_extent,
                physical_render_extent,
            };
            if audit.resources.get(&metrics.surface_id) != Some(&key) {
                audit.resources.insert(metrics.surface_id, key);
                *audit.allocations.entry(metrics.surface_id).or_default() += 1;
            }
        }
        let front_publication_changed = std::mem::take(&mut audit.publish_next_front);
        CustomRenderPreparationOutcome::new(front_publication_changed)
    }
}

impl WidgetDowncast for ParticipantProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ParticipantProbe {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.audit.lock().unwrap().events.push(PhaseEvent::Layout);
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.audit.lock().unwrap().events.push(PhaseEvent::Paint);
        cx.scene.push_rect(rect, Color::WHITE, 0, None);
    }

    fn custom_render_admission(&self) -> Option<&CustomRenderAdmission> {
        self.admission.as_ref()
    }

    fn custom_render_participant_mut(&mut self) -> Option<&mut dyn CustomRenderParticipant> {
        Some(self)
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

struct HitMetadataSeries {
    hit_proxy_calls: Arc<AtomicUsize>,
    z_order: u32,
}

impl HitMetadataSeries {
    fn new(hit_proxy_calls: Arc<AtomicUsize>, z_order: u32) -> Self {
        Self {
            hit_proxy_calls,
            z_order,
        }
    }
}

impl Series for HitMetadataSeries {
    fn init(&mut self, _device: &wgpu::Device, _queue: &wgpu::Queue, _format: wgpu::TextureFormat) {
    }

    fn prepare(
        &mut self,
        _device: &wgpu::Device,
        _queue: &wgpu::Queue,
        _transform: &Transform,
        _viewport: crate::plot::series::PhysicalRect,
    ) {
    }

    fn render<'a>(
        &'a self,
        _render_pass: &mut wgpu::RenderPass<'a>,
        _viewport: crate::plot::series::PhysicalRect,
    ) {
    }

    fn z_order(&self) -> u32 {
        self.z_order
    }

    fn hit_test(&self, _x: f32, _y: f32, _transform: &Transform) -> Option<Box<dyn std::any::Any>> {
        None
    }

    fn hit_proxy(&self) -> HitProxy {
        self.hit_proxy_calls.fetch_add(1, Ordering::Relaxed);
        HitProxy::Candle {
            candles: Vec::new(),
        }
    }
}

struct PlainProbe {
    id: WidgetInternalId,
}

impl PlainProbe {
    fn new() -> Self {
        Self {
            id: WidgetInternalId::new(),
        }
    }
}

impl WidgetDowncast for PlainProbe {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for PlainProbe {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(rect, Color::BLACK, 0, None);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

struct OffsetContainer {
    id: WidgetInternalId,
    child_rect: RectF,
}

impl WidgetDowncast for OffsetContainer {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for OffsetContainer {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        for child in children {
            cx.layout_child(
                *child,
                Constraints::tight(Size::new(self.child_rect.width, self.child_rect.height)),
            );
            cx.set_child_rect(*child, self.child_rect);
        }
        LayoutResult::fit(Size::new(constraints.max_width, constraints.max_height))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        cx.scene.push_rect(rect, Color::BLACK, 0, None);
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }
}

fn target(surface: u64, scale_factor: f32) -> CustomRenderTargetSnapshot {
    CustomRenderTargetSnapshot::new(
        SurfaceId::test_id(surface),
        PhysicalExtent::new((200.0 * scale_factor) as u32, (120.0 * scale_factor) as u32),
        scale_factor,
    )
    .unwrap()
}

fn frame(
    harness: &mut RetainedSceneTestHarness,
    tree: &mut WidgetTree,
    root: NodeId,
    constraints: Constraints,
    target: CustomRenderTargetSnapshot,
) -> crate::core::app::RetainedSceneTestFrame {
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let icons = IconRegistry::new(&[]);
    harness.frame_with_custom_render_target(
        tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        target,
    )
}

#[test]
fn preparation_runs_after_final_layout_before_paint_and_on_clean_reuse() {
    let audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(ParticipantProbe::new(audit.clone())), None, 0);
    let constraints = Constraints::tight(Size::new(100.25, 50.5));
    let target = target(1, 1.5);
    let mut harness = RetainedSceneTestHarness::new();

    let first = frame(&mut harness, &mut tree, root, constraints, target);
    assert!(first.scene_built);
    assert_eq!(
        audit.lock().unwrap().events,
        [PhaseEvent::Layout, PhaseEvent::Prepare, PhaseEvent::Paint]
    );

    audit.lock().unwrap().events.clear();
    let retained = frame(&mut harness, &mut tree, root, constraints, target);
    assert!(!retained.scene_built);
    assert!(!retained.visual_changed);
    assert_eq!(audit.lock().unwrap().events, [PhaseEvent::Prepare]);

    {
        let mut audit = audit.lock().unwrap();
        audit.events.clear();
        audit.publish_next_front = true;
    }
    let publication = frame(&mut harness, &mut tree, root, constraints, target);
    assert!(publication.scene_built);
    assert!(publication.visual_changed);
    assert_eq!(audit.lock().unwrap().events, [PhaseEvent::Prepare]);
}

#[test]
fn participant_index_tracks_insertion_and_removal() {
    let audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(PlainProbe::new()), None, 0);
    assert_eq!(tree.custom_render_participant_count_for_test(), 0);

    let participant = tree.add_child(root, Box::new(ParticipantProbe::new(audit)), None, 0);
    assert_eq!(tree.custom_render_participant_count_for_test(), 1);
    tree.remove_node(participant);
    assert_eq!(tree.custom_render_participant_count_for_test(), 0);
}

#[test]
fn frame_realization_attempt_reaches_preparation_without_invalidating_retained_content() {
    use crate::core::frame_realization::AppFrameAttempts;
    let audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(ParticipantProbe::new(audit.clone())), None, 0);
    let constraints = Constraints::tight(Size::new(100.25, 50.5));
    let mut harness = RetainedSceneTestHarness::new();
    let mut attempts = AppFrameAttempts::new();
    let first = attempts.next();
    let second = attempts.next();
    frame(
        &mut harness,
        &mut tree,
        root,
        constraints,
        target(1, 1.5).with_app_frame_attempt(first),
    );
    audit.lock().unwrap().events.clear();
    let retained = frame(
        &mut harness,
        &mut tree,
        root,
        constraints,
        target(1, 1.5).with_app_frame_attempt(second),
    );
    assert!(!retained.scene_built);
    assert!(!retained.visual_changed);
    let audit = audit.lock().unwrap();
    assert_eq!(audit.metrics[0].app_frame_attempt, Some(first));
    assert_eq!(audit.metrics[1].app_frame_attempt, Some(second));
    assert_eq!(audit.metrics[0].surface_id, audit.metrics[1].surface_id);
    assert_eq!(audit.allocations[&SurfaceId::test_id(1)], 1);
    assert_eq!(audit.events, [PhaseEvent::Prepare]);
}

#[test]
fn target_resources_are_surface_isolated_idempotent_and_scale_sensitive() {
    let audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(ParticipantProbe::new(audit.clone())), None, 0);
    let constraints = Constraints::tight(Size::new(100.25, 50.5));
    let mut harness = RetainedSceneTestHarness::new();
    let surface_one = SurfaceId::test_id(1);
    let surface_two = SurfaceId::test_id(2);

    frame(&mut harness, &mut tree, root, constraints, target(1, 1.0));
    frame(&mut harness, &mut tree, root, constraints, target(1, 1.0));
    {
        let audit = audit.lock().unwrap();
        assert_eq!(audit.allocations.get(&surface_one), Some(&1));
        assert_eq!(
            audit.resources[&surface_one].physical_render_extent,
            PhysicalExtent::new(100, 51)
        );
    }

    frame(&mut harness, &mut tree, root, constraints, target(2, 2.0));
    {
        let audit = audit.lock().unwrap();
        assert_eq!(audit.allocations.get(&surface_one), Some(&1));
        assert_eq!(audit.allocations.get(&surface_two), Some(&1));
        assert_eq!(
            audit.resources[&surface_two].physical_render_extent,
            PhysicalExtent::new(201, 101)
        );
    }

    frame(&mut harness, &mut tree, root, constraints, target(1, 1.5));
    let audit = audit.lock().unwrap();
    assert_eq!(audit.allocations.get(&surface_one), Some(&2));
    assert_eq!(audit.allocations.get(&surface_two), Some(&1));
    assert_eq!(
        audit.resources[&surface_one].physical_render_extent,
        PhysicalExtent::new(150, 76)
    );
}

#[test]
fn zero_and_fully_clipped_participants_are_inactive_without_allocation() {
    let zero_audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut zero_tree = WidgetTree::new();
    let zero_root =
        zero_tree.create_root(Box::new(ParticipantProbe::new(zero_audit.clone())), None, 0);
    let mut zero_harness = RetainedSceneTestHarness::new();
    frame(
        &mut zero_harness,
        &mut zero_tree,
        zero_root,
        Constraints::tight(Size::new(0.0, 0.0)),
        target(1, 1.0),
    );
    let zero_audit = zero_audit.lock().unwrap();
    assert!(zero_audit.resources.is_empty());
    assert_eq!(
        zero_audit.metrics.last().unwrap().activity,
        CustomRenderActivity::Inactive
    );
    drop(zero_audit);

    let clipped_audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut clipped_tree = WidgetTree::new();
    let clipped_root = clipped_tree.create_root(
        Box::new(OffsetContainer {
            id: WidgetInternalId::new(),
            child_rect: RectF::new(150.0, 150.0, 20.0, 20.0),
        }),
        None,
        0,
    );
    clipped_tree.add_child(
        clipped_root,
        Box::new(ParticipantProbe::new(clipped_audit.clone())),
        None,
        0,
    );
    let mut clipped_harness = RetainedSceneTestHarness::new();
    frame(
        &mut clipped_harness,
        &mut clipped_tree,
        clipped_root,
        Constraints::tight(Size::new(100.0, 100.0)),
        target(1, 1.0),
    );
    let clipped_audit = clipped_audit.lock().unwrap();
    assert!(clipped_audit.resources.is_empty());
    assert_eq!(
        clipped_audit.metrics.last().unwrap().activity,
        CustomRenderActivity::Inactive
    );
}

#[test]
fn no_participant_builds_no_custom_metrics_records() {
    preparation_audit::reset();
    let mut tree = WidgetTree::new();
    let root = tree.create_root(Box::new(PlainProbe::new()), None, 0);
    assert!(!tree.has_custom_render_participants());
    let mut harness = RetainedSceneTestHarness::new();
    frame(
        &mut harness,
        &mut tree,
        root,
        Constraints::tight(Size::new(100.0, 100.0)),
        target(1, 1.0),
    );
    assert_eq!(preparation_audit::metrics_snapshots(), 0);
    assert_eq!(harness.custom_render_presentation_binding_stats(), (0, 0));
}

#[test]
fn presentation_bindings_validate_admission_uniqueness_and_frame_scope() {
    let expected = CustomRenderAdmission::for_test();
    let other = CustomRenderAdmission::for_test();
    let surface = SurfaceId::test_id(27);
    let mut bindings = CustomRenderPresentationBindings::default();

    let mismatch = bindings
        .publish_for_target(
            surface,
            expected.presentation_id(),
            other.presentation_binding(700),
        )
        .unwrap_err();
    assert!(matches!(
        mismatch,
        CustomRenderPreparationError::PresentationIdentityMismatch { .. }
    ));
    assert_eq!(bindings.len(), 0);

    bindings
        .publish_for_target(
            surface,
            expected.presentation_id(),
            expected.presentation_binding(701),
        )
        .unwrap();
    assert_eq!(
        bindings.resolve(expected.presentation_id(), surface),
        Some(701)
    );
    let duplicate = bindings
        .publish_for_target(
            surface,
            expected.presentation_id(),
            expected.presentation_binding(702),
        )
        .unwrap_err();
    assert!(matches!(
        duplicate,
        CustomRenderPreparationError::DuplicatePresentationBinding { .. }
    ));

    let allocated_capacity = bindings.capacity();
    bindings.reset_for_test();
    assert_eq!(bindings.len(), 0);
    assert_eq!(bindings.capacity(), allocated_capacity);
    assert_eq!(bindings.resolve(expected.presentation_id(), surface), None);
}

#[test]
fn participant_trait_without_295a_admission_cannot_enter_registry() {
    preparation_audit::reset();
    let audit = Arc::new(Mutex::new(ProbeAudit::default()));
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(ParticipantProbe::new_unadmitted(audit.clone())),
        None,
        0,
    );
    assert!(!tree.has_custom_render_participants());
    assert_eq!(tree.custom_render_participant_count_for_test(), 0);

    let mut harness = RetainedSceneTestHarness::new();
    frame(
        &mut harness,
        &mut tree,
        root,
        Constraints::tight(Size::new(100.0, 100.0)),
        target(1, 1.0),
    );
    assert_eq!(preparation_audit::metrics_snapshots(), 0);
    assert!(!audit.lock().unwrap().events.contains(&PhaseEvent::Prepare));
}

#[test]
fn invalid_target_metrics_fail_before_a_snapshot_can_reach_a_participant() {
    let surface = SurfaceId::test_id(9);
    assert!(CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(100, 100), 0.0).is_err());
    assert!(
        CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(100, 100), f32::NAN).is_err()
    );
    assert!(CustomRenderTargetSnapshot::new(surface, PhysicalExtent::new(0, 100), 1.0).is_err());
}

#[test]
fn plot_layout_stays_logical_and_preparation_owns_first_gpu_allocation() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(Plot::new().with_gpu_resources(renderer.gpu_resources())),
        None,
        0,
    );
    let constraints = Constraints::tight(Size::new(100.25, 50.5));
    let theme = Theme::default();
    let mut text = FontManagerTextEngine::new();
    let before_layout = gpu_audit::snapshot();

    test_begin_frame_layout_tracking();
    {
        let _layout = LayoutAuthorityGuard::enter();
        layout_tree(&mut tree, root, constraints, &theme, &mut text, None);
    }
    let after_layout = gpu_audit::snapshot();
    assert_eq!(
        after_layout.custom_render_passes,
        before_layout.custom_render_passes
    );
    assert_eq!(after_layout.custom_textures, before_layout.custom_textures);
    assert_eq!(after_layout.plot_renderers, before_layout.plot_renderers);
    assert_eq!(after_layout.worker_pools, before_layout.worker_pools);

    let mut harness = RetainedSceneTestHarness::new();
    let icons = IconRegistry::new(&[]);
    harness.frame_with_custom_render_target(
        &mut tree,
        root,
        constraints,
        &theme,
        &mut text,
        &icons,
        target(7, 1.5),
    );
    let after_preparation = gpu_audit::snapshot();
    assert_eq!(
        after_preparation.custom_render_passes,
        after_layout.custom_render_passes + 1
    );
    assert_eq!(
        after_preparation.custom_textures,
        after_layout.custom_textures + 2
    );
    assert_eq!(
        after_preparation.plot_renderers,
        after_layout.plot_renderers + 1
    );
    assert_eq!(
        after_preparation.worker_pools,
        after_layout.worker_pools + 1
    );
    assert_eq!(
        after_preparation.custom_registrations,
        after_layout.custom_registrations
    );

    let plot = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<Plot>()
        .unwrap();
    assert_eq!(
        plot.target_resource_for_test(),
        Some((
            SurfaceId::test_id(7),
            PhysicalExtent::new(150, 76),
            1.5_f32.to_bits(),
            true,
        ))
    );
    assert_eq!(plot.presented_texture_id_for_test(), None);

    let before_stable = gpu_audit::snapshot();
    frame(&mut harness, &mut tree, root, constraints, target(7, 1.5));
    let after_stable = gpu_audit::snapshot();
    assert_eq!(after_stable.custom_textures, before_stable.custom_textures);
    assert_eq!(
        after_stable.custom_unregistrations,
        before_stable.custom_unregistrations
    );

    let before_scale_change = gpu_audit::snapshot();
    frame(&mut harness, &mut tree, root, constraints, target(7, 2.0));
    let after_scale_change = gpu_audit::snapshot();
    assert_eq!(
        after_scale_change.custom_textures,
        before_scale_change.custom_textures + 2
    );
    assert_eq!(
        after_scale_change.custom_unregistrations,
        before_scale_change.custom_unregistrations + 1
    );
    let plot = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<Plot>()
        .unwrap();
    assert_eq!(
        plot.target_resource_for_test(),
        Some((
            SurfaceId::test_id(7),
            PhysicalExtent::new(201, 101),
            2.0_f32.to_bits(),
            true,
        ))
    );
}

#[test]
fn inactive_plot_preparation_allocates_no_gpu_resources() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let mut tree = WidgetTree::new();
    let root = tree.create_root(
        Box::new(Plot::new().with_gpu_resources(renderer.gpu_resources())),
        None,
        0,
    );
    let before = gpu_audit::snapshot();
    let mut harness = RetainedSceneTestHarness::new();
    frame(
        &mut harness,
        &mut tree,
        root,
        Constraints::tight(Size::new(0.0, 0.0)),
        target(8, 2.0),
    );
    let after = gpu_audit::snapshot();
    assert_eq!(after.custom_render_passes, before.custom_render_passes);
    assert_eq!(after.custom_textures, before.custom_textures);
    assert_eq!(after.plot_renderers, before.plot_renderers);
    assert_eq!(after.worker_pools, before.worker_pools);

    let plot = tree
        .node(root)
        .unwrap()
        .widget
        .as_any()
        .downcast_ref::<Plot>()
        .unwrap();
    assert_eq!(plot.target_resource_for_test(), None);
    assert_eq!(plot.presented_texture_id_for_test(), None);
}

mod plot_resize_pending {
    use super::*;
    use crate::compositor::{Compositor, SceneKey};
    use crate::core::app::{
        AppContext, RetainedSceneTestFrame, drain_requested_images_for_reused_scene,
    };
    use crate::core::event::{
        FusedInputEvent, InteractionDomainState, PointerButton, PointerButtonsState, PointerDevice,
        WindowId,
    };
    use crate::core::event_router::EventRouter;
    use crate::core::frame_realization::{AppFrameAttemptId, AppFrameAttempts};
    use crate::core::math::Point;
    use crate::core::scene::{SceneGraph, SceneNode};
    use crate::plot::price_level::{PriceLevel, TagLine};
    use crate::plot::series::PhysicalRect as SeriesRect;
    use crate::plot::{
        BarPoint, BarSeries, BarStyle, CandleSeries, DataUpdate, MarketDetailLink, PlotColorScheme,
        PlotRenderPool, RenderCandle, ViewState,
    };
    use crate::renderer::DrawPlanBuilder;
    use crate::renderer::image_binding::ImageBindingIdentity;
    use crate::runtime::binding_registry::BindingRegistry;
    use crate::widgets::{
        aligned_order_book::AlignedOrderBook, label::Label, split_pane::SplitPane,
    };
    use std::sync::atomic::AtomicBool;
    use std::thread::{self, ThreadId};
    use std::time::{Duration, Instant};

    #[derive(Clone, Copy)]
    struct PlotWake {
        thread: ThreadId,
        audit: gpu_audit::CustomRenderAuditSnapshot,
    }

    impl PlotWake {
        fn observe() -> Self {
            Self {
                thread: thread::current().id(),
                audit: gpu_audit::snapshot(),
            }
        }
    }

    // The real Pool invokes prepare with its own PlotRenderer guard held.
    struct HoldRender {
        armed: Arc<AtomicBool>,
        entered: crossbeam_channel::Sender<PlotWake>,
        release: crossbeam_channel::Receiver<()>,
    }

    impl Series for HoldRender {
        fn init(&mut self, _: &wgpu::Device, _: &wgpu::Queue, _: wgpu::TextureFormat) {}

        fn prepare(&mut self, _: &wgpu::Device, _: &wgpu::Queue, _: &Transform, _: SeriesRect) {
            if self.armed.swap(false, Ordering::AcqRel) {
                self.entered.send(PlotWake::observe()).unwrap();
                self.release
                    .recv_timeout(Duration::from_secs(30))
                    .expect("test releases renderer");
            }
        }

        fn render<'a>(&'a self, _: &mut wgpu::RenderPass<'a>, _: SeriesRect) {}
        fn z_order(&self) -> u32 {
            0
        }
        fn hit_test(&self, _: f32, _: f32, _: &Transform) -> Option<Box<dyn std::any::Any>> {
            None
        }
        fn hit_proxy(&self) -> HitProxy {
            HitProxy::Candle {
                candles: Vec::new(),
            }
        }
    }

    fn held_series(
        armed: bool,
    ) -> (
        HoldRender,
        crossbeam_channel::Receiver<PlotWake>,
        crossbeam_channel::Sender<()>,
    ) {
        let (entered, wait) = crossbeam_channel::bounded(1);
        let (resume, release) = crossbeam_channel::bounded(1);
        (
            HoldRender {
                armed: Arc::new(AtomicBool::new(armed)),
                entered,
                release,
            },
            wait,
            resume,
        )
    }

    fn candle(x: f64, high: f64) -> RenderCandle {
        RenderCandle {
            x_open: x - 0.4,
            x_close: x + 0.4,
            high,
            low: 33.0,
            body_top: 60.0,
            body_bottom: 40.0,
            is_up: true,
            volume: 5.0,
            body_color_override: None,
        }
    }

    fn has_reference_segment(nodes: &[SceneNode], x: f32, y: f32) -> bool {
        nodes.iter().any(|node| match node {
            SceneNode::RetainedFragment(fragment) => has_reference_segment(&fragment.children, x, y),
            SceneNode::Clip(clip) => has_reference_segment(&clip.children, x, y),
            SceneNode::Rect(rect) => rect.color == Color::from_linear(1.0, 0.0, 1.0, 1.0).into()
                && rect.rect.x == x && (rect.rect.y + 0.5 - y).abs() < 0.01,
            _ => false,
        })
    }

    fn has_tag(nodes: &[SceneNode], tag: &str) -> bool {
        nodes.iter().any(|node| match node {
            SceneNode::RetainedFragment(fragment) => has_tag(&fragment.children, tag),
            SceneNode::Clip(clip) => has_tag(&clip.children, tag),
            SceneNode::Text(text) => text.text == tag,
            _ => false,
        })
    }

    struct Fixture {
        renderer: WgpuRenderer,
        compositor: Compositor,
        surface: SurfaceId,
        output: wgpu::Texture,
        view: wgpu::TextureView,
        harness: RetainedSceneTestHarness,
        attempts: AppFrameAttempts,
        app: AppContext,
        tree: WidgetTree,
        root: NodeId,
        plot: NodeId,
        sibling: NodeId,
        theme: Theme,
        icons: IconRegistry,
        text: FontManagerTextEngine,
        router: EventRouter,
        domain: InteractionDomainState,
        bindings: BindingRegistry,
        wake: crossbeam_channel::Receiver<PlotWake>,
    }

    fn plot_output_texture(renderer: &WgpuRenderer, label: &str) -> wgpu::Texture {
        let gpu = renderer.gpu_resources();
        gpu.device.create_texture(&wgpu::TextureDescriptor {
            label: Some(label),
            size: wgpu::Extent3d {
                width: 640,
                height: 240,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: gpu.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        })
    }

    impl Fixture {
        fn new(plot: Plot, mut renderer: WgpuRenderer) -> Self {
            eprintln!(
                "plot_resize_pending headless adapter {:?}",
                renderer.headless_adapter_for_test()
            );
            let bounds = RectF::new(0.0, 0.0, 640.0, 240.0);
            let mut compositor = Compositor::new(bounds);
            let surface = compositor.create_surface(0, bounds, SceneKey::BASE);
            renderer
                .register_headless_render_target(surface, (640, 240), 1.0)
                .unwrap();
            let output = plot_output_texture(&renderer, "Plot Pending headless surface");
            let view = output.create_view(&wgpu::TextureViewDescriptor::default());
            let (send, wake) = crossbeam_channel::unbounded();
            renderer.set_image_admission_wake(Box::new(move || {
                let _ = send.send(PlotWake::observe());
            }));
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(
                    SplitPane::horizontal(NodeId(0), NodeId(0))
                        .initial_ratio(0.6)
                        .leading_min(80.0)
                        .trailing_min(100.0),
                ),
                None,
                0,
            );
            let leading = tree.add_child(root, Box::new(plot), None, 0);
            let sibling = tree.add_child(root, Box::new(Label::new("BOOK")), None, 1);
            let split = tree
                .node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<SplitPane>()
                .unwrap();
            split.leading = leading;
            split.trailing = sibling;
            let theme = Theme::dark();
            Self {
                renderer,
                compositor,
                surface,
                output,
                view,
                harness: RetainedSceneTestHarness::new(),
                attempts: AppFrameAttempts::new(),
                app: AppContext::new(theme.clone()),
                tree,
                root,
                plot: leading,
                sibling,
                theme,
                icons: IconRegistry::new(&[]),
                text: FontManagerTextEngine::new(),
                router: EventRouter::new(),
                domain: InteractionDomainState::new(),
                bindings: BindingRegistry::new(),
                wake,
            }
        }

        fn plot(&self) -> &Plot {
            self.tree
                .node(self.plot)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }

        fn plot_mut(&mut self) -> &mut Plot {
            self.tree
                .node_mut(self.plot)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut()
                .unwrap()
        }

        fn frame(&mut self, width: f32) -> RetainedSceneTestFrame {
            self.frame_for_surface(width, self.surface)
        }

        fn frame_for_surface(&mut self, width: f32, surface: SurfaceId) -> RetainedSceneTestFrame {
            let attempt = self.attempts.next();
            self.frame_at(width, surface, attempt)
        }

        fn frame_at(
            &mut self,
            width: f32,
            surface: SurfaceId,
            attempt: AppFrameAttemptId,
        ) -> RetainedSceneTestFrame {
            let target = if self.renderer.target_has_active_extent(surface) {
                Some(
                    self.renderer
                        .custom_render_target_snapshot(surface)
                        .unwrap()
                        .with_app_frame_attempt(attempt),
                )
            } else {
                self.tree.withdraw_inactive_plot_surface(surface);
                None
            };
            self.harness.frame_with_flat_viewport3d_transfers(
                &mut self.tree,
                &mut self.app,
                self.root,
                Constraints::tight(Size::new(width, 240.0)),
                &self.theme,
                &mut self.renderer,
                &self.icons,
                0.0,
                target,
                Some(attempt),
            )
        }

        fn route(&mut self, event: FusedInputEvent) {
            self.router.route_input_event(
                &mut self.tree,
                event,
                Some(self.root),
                &mut self.text,
                None,
                &mut self.domain,
                &mut self.bindings,
            );
        }

        fn assert_pending(&self, frame: &RetainedSceneTestFrame) {
            assert!(!frame.scene_built && frame.scene_root.is_empty());
            assert_eq!(
                frame
                    .plot_evidence
                    .iter()
                    .filter_map(|p| p.pending_surface())
                    .collect::<Vec<_>>(),
                [self.surface]
            );
        }

        fn wait_worker(&self) {
            let deadline = Instant::now() + Duration::from_secs(10);
            while !self.plot().plot_worker_ready_for_test() {
                self.wake
                    .recv_deadline(deadline)
                    .expect("real worker completion wake");
            }
        }

        fn warm(&mut self) -> ImageBindingIdentity {
            let cold = self.frame(640.0);
            assert!(
                cold.plot_evidence
                    .iter()
                    .all(|plot| plot.pending_surface().is_none())
            );
            self.wait_worker();
            let ready = self.frame(640.0);
            self.submit(ready)
        }

        fn submit(&mut self, frame: RetainedSceneTestFrame) -> ImageBindingIdentity {
            assert!(frame.scene_built);
            assert!(
                frame
                    .plot_evidence
                    .iter()
                    .all(|p| p.pending_surface().is_none())
            );
            let front = frame
                .plot_evidence
                .into_iter()
                .find_map(|p| p.front)
                .expect("mounted Plot supplied its own front");
            drain_requested_images_for_reused_scene(&mut self.renderer);
            let mut scene = SceneGraph::new();
            scene.root = frame.scene_root;
            let scenes = HashMap::from([(self.surface, scene)]);
            let counter = self.renderer.frame_counter();
            let plan = DrawPlanBuilder::new()
                .build_multi(
                    &scenes,
                    self.compositor.snapshot_multi(),
                    self.renderer.mesh_cache(),
                    |_| 1.0,
                    counter,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            let compiled = self
                .renderer
                .compile_multi(&plan)
                .unwrap()
                .expect_ready("native Compile must be ready");
            let binding = self
                .renderer
                .compiled_plot_image(&compiled, self.surface, front.texture_id())
                .expect("exact Plot image survived Compile");
            assert!(
                front
                    .required_binding()
                    .is_none_or(|expected| expected.identity() == binding)
            );
            let command = self
                .renderer
                .encode_compiled_target_for_test(&compiled, self.surface, &self.view)
                .expect("headless surface executed");
            self.renderer.submit_compiled_target_for_test(command);
            front.record_submitted(binding);
            self.renderer.advance_frame();
            binding
        }

        fn pixels(&self, binding: ImageBindingIdentity, range: (f64, f64), reference: f64) {
            let pixel = |x, y| self.renderer.render_registered_image_pixel_for_test(
                binding.texture_id(), [x, y], [1.0; 4], wgpu::Color::TRANSPARENT,
            );
            let y = |value: f64| ((range.1 - value) / (range.1 - range.0)) as f32;
            let candle = pixel(0.20, y(48.0));
            let volume = pixel(0.20, 0.95);
            let around = |value| [-0.6, 0.0, 0.6].map(|dy| pixel(0.31, y(value) + dy / 240.0));
            let grid = [40.0, 50.0, 60.0, 80.0]
                .into_iter()
                .flat_map(around)
                .find(|p| p[0] > 160 && p[1] < 80 && p[2] < 80);
            let level = around(reference);
            eprintln!(
                "Plot binding {:?} candle={candle:?} volume={volume:?} grid={grid:?} reference={level:?}",
                binding
            );
            assert!(candle[1] > 180 && candle[0] < 80);
            assert!(volume[2] > 180 && volume[0] < 80);
            assert!(grid.is_some());
            assert!(level.iter().any(|p| p[0] > 160 && p[1] < 80 && p[2] > 160));
        }

        fn close_glyph_pixels(&self) -> usize {
            let gpu = self.renderer.gpu_resources();
            let stride = self.output.width() * 4;
            assert_eq!(stride % wgpu::COPY_BYTES_PER_ROW_ALIGNMENT, 0);
            let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
                label: Some("Plot close tag readback"),
                size: u64::from(stride * self.output.height()),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
                mapped_at_creation: false,
            });
            let mut encoder = gpu.device.create_command_encoder(&Default::default());
            encoder.copy_texture_to_buffer(
                self.output.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: &readback,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(stride),
                        rows_per_image: Some(self.output.height()),
                    },
                },
                self.output.size(),
            );
            gpu.queue.submit([encoder.finish()]);
            let (send, ready) = std::sync::mpsc::channel();
            let slice = readback.slice(..);
            slice.map_async(wgpu::MapMode::Read, move |result| {
                send.send(result).unwrap()
            });
            gpu.device.poll(wgpu::PollType::wait_indefinitely()).unwrap();
            ready
                .recv_timeout(Duration::from_secs(10))
                .unwrap()
                .unwrap();
            let plot_width = self.tree.get_absolute_paint_rect(self.plot).unwrap().width as usize;
            slice
                .get_mapped_range().expect("mapped readback range")
                .chunks_exact(stride as usize)
                .flat_map(|row| row.chunks_exact(4).take(plot_width))
                // Only the close-tag text uses yellow. This predicate works for RGBA and BGRA.
                .filter(|pixel| {
                    pixel[1] > 140 && pixel[0].max(pixel[2]) > 140 && pixel[0].min(pixel[2]) < 95
                })
                .count()
        }
    }

    #[test]
    fn plot_historical_series_first_front_keeps_queued_data() {
        const START: i64 = 1_794_321_900_000;
        const SLOT: i64 = 900_000;
        const CLOSE: f64 = 60_033.07;
        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
        #[cfg(target_os = "macos")]
        assert_eq!(
            renderer.headless_adapter_for_test().backend,
            wgpu::Backend::Metal
        );
        let history = (0..5)
            .map(|i| {
                let x = START + i * SLOT;
                let price = 60_000.0 + (x / SLOT % 17) as f64;
                let (open, high, low, close, volume) = if i == 4 {
                    (60_033.01, CLOSE, 60_033.01, CLOSE, 1.75)
                } else {
                    (price, price + 3.0, price - 2.0, price + 1.0, 2.5)
                };
                RenderCandle {
                    x_open: x as f64,
                    x_close: (x + SLOT) as f64,
                    high,
                    low,
                    body_top: close,
                    body_bottom: open,
                    is_up: true,
                    volume,
                    body_color_override: None,
                }
            })
            .collect::<Vec<_>>();
        let bar = |c: &RenderCandle| BarPoint {
            x: (c.x_open + c.x_close) / 2.0,
            y: c.volume,
            color: None,
        };
        let mut seed = history[4].clone();
        seed.body_top = seed.body_bottom;
        seed.high = seed.low;
        seed.volume = 0.25;
        let mut volume = BarSeries::new()
            .with_style(BarStyle {
                color: Color::from_linear(0.0, 0.0, 1.0, 1.0),
                viewport_fraction: Some(0.16),
                ..BarStyle::default()
            })
            .with_z_order(50)
            .with_auto_scale_y(false);
        volume.set_points(vec![bar(&seed)]);
        let mut candles = CandleSeries::new().with_colors(
            Color::from_linear(0.0, 1.0, 0.0, 1.0),
            Color::from_linear(1.0, 0.0, 0.0, 1.0),
        );
        candles.set_candles(vec![seed.clone()]);
        let link = MarketDetailLink::new(59_400.0, 60_700.0);
        let xmax = seed.x_close + 2.0 * SLOT as f64;
        let (plot, tx) = Plot::new()
            .with_gpu_resources(renderer.gpu_resources())
            .with_view_state(ViewState::new(
                xmax - 32.0 * SLOT as f64,
                xmax,
                59_400.0,
                60_700.0,
            ))
            .with_market_detail_link(link.clone())
            .with_market_detail_reference_level(1)
            .add_series(Box::new(volume))
            .add_series(Box::new(candles))
            .with_data_channel();
        for update in [
            DataUpdate::ReplaceBarPoints {
                series_index: 0,
                points: history[..4].iter().map(bar).collect(),
            },
            DataUpdate::ReplaceCandles {
                series_index: 1,
                candles: history[..4].to_vec(),
            },
            DataUpdate::AppendBarPoint {
                series_index: 0,
                point: bar(&seed),
            },
            DataUpdate::AppendCandle {
                series_index: 1,
                candle: seed,
            },
            DataUpdate::UpdateLastBarPoint {
                series_index: 0,
                point: bar(&history[4]),
            },
            DataUpdate::UpdateLastCandle {
                series_index: 1,
                candle: history[4].clone(),
            },
            DataUpdate::AddPriceLevel(PriceLevel {
                id: 1,
                y: CLOSE,
                line_style: crate::plot::price_level::PriceLevelLine::Dotted,
                ..PriceLevel::default()
            }),
        ] {
            tx.send(update).unwrap();
        }

        let mut f = Fixture::new(plot, renderer);
        let binding = f.warm();
        assert_eq!(link.price_range(), (59_992.7395, 60_038.3305));
        assert_eq!(link.reference_line().unwrap().price, CLOSE);
        let extent = f.plot().target_resource_for_test().unwrap().1;
        let (width, height) = (extent.width as f32, extent.height as f32);
        let transform = Transform::from_view_state(f.plot().view_state(), width, height);
        let pixel = |x, y| {
            f.renderer.render_registered_image_pixel_for_test(
                binding.texture_id(),
                [x, y],
                [1.0; 4],
                wgpu::Color::TRANSPARENT,
            )
        };
        let visible = history
            .iter()
            .map(|c| {
                let x = transform.x_to_px(bar(c).x);
                let y = transform.y_to_px((c.body_bottom + c.body_top) / 2.0);
                // Sample a physical pixel in the gap of the dotted reference line.
                let body_x = (x / 5.0).floor() * 5.0 + 3.5;
                let candle = pixel(body_x / width, (y.floor() + 0.5) / height);
                let volume = pixel(x / width, 0.93);
                (
                    candle[1] > 180 && candle[0] < 80,
                    volume[2] > 180 && volume[0] < 80,
                )
            })
            .collect::<Vec<_>>();
        assert_eq!(
            visible,
            [(true, true); 5],
            "candle and volume pixels in first registered front"
        );
    }

    #[test]
    fn plot_line_and_circle_updates_reach_the_first_registered_image() {
        use crate::plot::{Circle, CircleSeries, CircleStyle, LinePoint, LineSeries, LineStyle};

        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
        #[cfg(target_os = "macos")]
        assert_eq!(
            renderer.headless_adapter_for_test().backend,
            wgpu::Backend::Metal
        );
        let line_point = |x, y| LinePoint { x, y };
        let circle = |x, y| Circle {
            x,
            y,
            radius: None,
            fill_color: None,
        };
        let mut line_series = LineSeries::new().with_style(LineStyle {
            color: Color::from_linear(1.0, 0.0, 0.0, 1.0),
            width: 4.0,
        });
        line_series.set_points(vec![line_point(1.0, 45.0)]);
        let mut circle_series = CircleSeries::new().with_style(CircleStyle {
            fill_color: Color::from_linear(0.0, 0.0, 1.0, 1.0),
            radius: 8.0,
            ..CircleStyle::default()
        });
        circle_series.set_points(vec![circle(1.0, 55.0)]);
        let (plot, tx) = Plot::new()
            .with_gpu_resources(renderer.gpu_resources())
            .with_view_state(ViewState::new(0.0, 10.0, 0.0, 100.0))
            .with_initial_follow_latest(false)
            .with_color_scheme(PlotColorScheme {
                background: Color::BLACK,
                grid: Color::BLACK,
                ..PlotColorScheme::default()
            })
            .add_series(Box::new(line_series))
            .add_series(Box::new(circle_series))
            .with_data_channel();
        let send_updates = || {
            for update in [
                DataUpdate::ReplaceLinePoints {
                    series_index: 0,
                    points: vec![line_point(2.0, 35.0), line_point(4.0, 35.0)],
                },
                DataUpdate::ReplaceCircles {
                    series_index: 1,
                    circles: vec![circle(3.0, 63.0)],
                },
                DataUpdate::AppendLinePoint {
                    series_index: 0,
                    point: line_point(8.0, 50.0),
                },
                DataUpdate::AppendCircle {
                    series_index: 1,
                    circle: circle(8.0, 73.0),
                },
                DataUpdate::UpdateLastLinePoint {
                    series_index: 0,
                    point: line_point(8.0, 20.0),
                },
                DataUpdate::UpdateLastCircle {
                    series_index: 1,
                    circle: Circle {
                        radius: Some(6.0),
                        fill_color: Some(Color::from_linear(0.0, 1.0, 0.0, 1.0)),
                        ..circle(8.0, 80.0)
                    },
                },
            ] {
                tx.send(update).unwrap();
            }
        };
        let read_pixels = |f: &Fixture, binding: ImageBindingIdentity| {
            let view = f.plot().view_state();
            // Each updated tail supplies one extreme of the hit-proxy Y range.
            assert_eq!((view.y_min, view.y_max), (11.0, 89.0));
            let extent = f.plot().target_resource_for_test().unwrap().1;
            let (width, height) = (extent.width as f32, extent.height as f32);
            let transform = Transform::from_view_state(view, width, height);
            [
                (3.0, 35.0, 0.0),   // interior of the replaced line segment
                (6.0, 27.5, 0.0),   // interior of the segment to the updated tail
                (3.0, 63.0, 6.0),   // inside the styled default radius of 8
                (8.0, 80.0, 0.0),   // updated circle center with color override
                (1.0, 55.0, 0.0),   // seed circle was replaced
                (6.0, 42.5, 0.0),   // former append-only line segment is gone
                (8.0, 73.0, 0.0),   // former appended circle is gone
                (8.0, 80.0, 7.0),   // outside the updated circle's radius of 6
            ]
            .map(|(x, y, dx)| {
                let px = (transform.x_to_px(x) + dx).floor() + 0.5;
                let py = transform.y_to_px(y).floor() + 0.5;
                f.renderer.render_registered_image_pixel_for_test(
                    binding.texture_id(),
                    [px / width, py / height],
                    [1.0; 4],
                    wgpu::Color::TRANSPARENT,
                )
            })
        };

        send_updates();
        let mut f = Fixture::new(plot, renderer);
        assert_eq!(f.plot().target_resource_for_test(), None);
        let first = f.warm();
        let cold = read_pixels(&f, first);

        // Replay the same messages with the renderer installed and a registered front.
        let target = f.plot().target_resource_for_test();
        let submission = f.plot().plot_submission_generation_for_test();
        send_updates();
        let update_frame = f.frame(640.0);
        assert!(
            update_frame
                .plot_evidence
                .iter()
                .all(|plot| plot.pending_surface().is_none())
        );
        assert_eq!(f.plot().target_resource_for_test(), target);
        assert!(f.plot().plot_submission_generation_for_test() > submission);
        f.wait_worker();
        let ready = f.frame(640.0);
        let second = f.submit(ready);
        assert_eq!(first.texture_id(), second.texture_id());
        assert_ne!(first.generation(), second.generation());
        let mounted = read_pixels(&f, second);
        eprintln!("Line/Circle first {first:?} {cold:?}, mounted {second:?} {mounted:?}");

        let expected = [
            [255, 0, 0, 255],
            [255, 0, 0, 255],
            [0, 0, 255, 255],
            [0, 255, 0, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
            [0, 0, 0, 255],
        ];
        assert_eq!(mounted, expected, "same-size mounted update");
        assert_eq!(cold, expected, "first registered Plot front");
    }

    mod direct_series {
        use super::*;
        use crate::plot::{
            Circle, CircleSeries, CircleStyle, LinePoint, LineSeries, LineStyle, SeriesData,
        };

        const BLACK: [u8; 4] = [0, 0, 0, 255];
        const GREEN: [u8; 4] = [0, 255, 0, 255];
        const BLUE: [u8; 4] = [0, 0, 255, 255];
        const RED: [u8; 4] = [255, 0, 0, 255];
        const MAGENTA: [u8; 4] = [255, 0, 255, 255];
        const SEED_SITES: [(f64, f64); 4] = [(1.0, 50.0), (1.0, 6.0), (1.0, 31.0), (1.0, 76.0)];
        const NEW_SITES: [(f64, f64); 8] = [
            (2.25, 50.0),
            (3.5, 50.0),
            (5.0, 6.0),
            (6.0, 9.0),
            (7.5, 25.0),
            (8.5, 27.5),
            (10.0, 67.0),
            (11.0, 72.0),
        ];

        fn bar(x: f64, y: f64) -> BarPoint {
            BarPoint { x, y, color: None }
        }

        fn circle(x: f64, y: f64) -> Circle {
            Circle {
                x,
                y,
                radius: None,
                fill_color: None,
            }
        }

        fn fixture(hold: Option<HoldRender>) -> Fixture {
            let renderer =
                pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
            #[cfg(target_os = "macos")]
            assert_eq!(
                renderer.headless_adapter_for_test().backend,
                wgpu::Backend::Metal
            );

            let green = Color::from_linear(0.0, 1.0, 0.0, 1.0);
            let mut candles = CandleSeries::new().with_colors(green, green);
            candles.set_candles(vec![candle(1.0, 77.0)]);
            let mut bars = BarSeries::new().with_style(BarStyle {
                color: Color::from_linear(0.0, 0.0, 1.0, 1.0),
                ..BarStyle::default()
            });
            bars.set_points(vec![bar(1.0, 16.0)]);
            let mut lines = LineSeries::new().with_style(LineStyle {
                color: Color::from_linear(1.0, 0.0, 0.0, 1.0),
                width: 4.0,
            });
            lines.set_points(vec![
                LinePoint { x: 0.6, y: 31.0 },
                LinePoint { x: 1.4, y: 31.0 },
            ]);
            let mut circles = CircleSeries::new().with_style(CircleStyle {
                fill_color: Color::from_linear(1.0, 0.0, 1.0, 1.0),
                ..CircleStyle::default()
            });
            circles.set_points(vec![circle(1.0, 76.0)]);
            let plot = Plot::new()
                .with_gpu_resources(renderer.gpu_resources())
                .with_view_state(ViewState::new(0.0, 12.0, 0.0, 100.0))
                .with_y_padding(0.0)
                .with_color_scheme(PlotColorScheme {
                    background: Color::BLACK,
                    grid: Color::BLACK,
                    ..PlotColorScheme::default()
                })
                .without_labels()
                .add_series(Box::new(candles))
                .add_series(Box::new(bars))
                .add_series(Box::new(lines))
                .add_series(Box::new(circles));
            let plot = match hold {
                Some(hold) => plot.add_series(Box::new(hold)),
                None => plot,
            };
            Fixture::new(plot, renderer)
        }

        fn replace_series(plot: &mut Plot) {
            for (index, data) in [
                SeriesData::Candles(vec![candle(2.25, 76.0), candle(3.5, 96.0)]),
                SeriesData::Bars(vec![bar(5.0, 14.0), bar(6.0, 18.0)]),
                SeriesData::Lines(vec![
                    LinePoint { x: 7.0, y: 25.0 },
                    LinePoint { x: 8.0, y: 25.0 },
                    LinePoint { x: 9.0, y: 30.0 },
                ]),
                SeriesData::Circles(vec![circle(10.0, 67.0), circle(11.0, 72.0)]),
            ]
            .into_iter()
            .enumerate()
            {
                plot.update_series_data(index, data);
            }

            // Isolate each series' hit-proxy range with a disjoint X window.
            let full_view = plot.view_state().clone();
            for (left, right, expected) in [
                (1.8, 4.0, (33.0, 96.0)),
                (4.5, 6.5, (0.0, 18.0)),
                (6.5, 9.5, (25.0, 30.0)),
                (9.5, 11.5, (67.0, 72.0)),
            ] {
                let view = plot.view_state_mut();
                view.x_min = left;
                view.x_max = right;
                plot.auto_scale_y(0.0);
                let view = plot.view_state();
                assert_eq!((view.y_min, view.y_max), expected);
            }
            *plot.view_state_mut() = full_view;
            plot.auto_scale_y(0.0);
            let view = plot.view_state();
            assert_eq!(
                (view.x_min, view.x_max, view.y_min, view.y_max),
                (0.0, 12.0, 0.0, 96.0)
            );
        }

        fn pixels<const N: usize>(
            f: &Fixture,
            binding: ImageBindingIdentity,
            sites: [(f64, f64); N],
        ) -> [[u8; 4]; N] {
            let extent = f.plot().target_resource_for_test().unwrap().1;
            let (width, height) = (extent.width as f32, extent.height as f32);
            let transform = Transform::from_view_state(f.plot().view_state(), width, height);
            sites.map(|(x, y)| {
                f.renderer.render_registered_image_pixel_for_test(
                    binding.texture_id(),
                    [
                        (transform.x_to_px(x).floor() + 0.5) / width,
                        (transform.y_to_px(y).floor() + 0.5) / height,
                    ],
                    [1.0; 4],
                    wgpu::Color::TRANSPARENT,
                )
            })
        }

        #[test]
        fn plot_resize_pending_series_supersession() {
            let renderer =
                pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
            #[cfg(target_os = "macos")]
            assert_eq!(
                renderer.headless_adapter_for_test().backend,
                wgpu::Backend::Metal
            );
            let (held, entered, resume) = held_series(false);
            let arm = held.armed.clone();
            let link = MarketDetailLink::new(0.0, 100.0);
            let green = Color::from_linear(0.0, 1.0, 0.0, 1.0);
            let mut candles = CandleSeries::new().with_colors(green, green);
            candles.set_candles(vec![candle(1.0, 77.0)]);
            let mut bars = BarSeries::new().with_style(BarStyle {
                color: Color::from_linear(0.0, 0.0, 1.0, 1.0),
                ..BarStyle::default()
            });
            bars.set_points(vec![bar(1.0, 16.0)]);
            let mut lines = LineSeries::new().with_style(LineStyle {
                color: Color::from_linear(1.0, 0.0, 0.0, 1.0),
                width: 4.0,
            });
            lines.set_points(vec![
                LinePoint { x: 0.6, y: 31.0 },
                LinePoint { x: 1.4, y: 31.0 },
            ]);
            let mut circles = CircleSeries::new().with_style(CircleStyle {
                fill_color: Color::from_linear(1.0, 0.0, 1.0, 1.0),
                ..CircleStyle::default()
            });
            circles.set_points(vec![circle(1.0, 76.0)]);
            let (plot, data) = Plot::new()
                .with_gpu_resources(renderer.gpu_resources())
                .with_view_state(ViewState::new(0.0, 12.0, 0.0, 100.0))
                .with_initial_follow_latest(false)
                .with_y_padding(0.0)
                .with_color_scheme(PlotColorScheme {
                    background: Color::BLACK,
                    grid: Color::BLACK,
                    ..PlotColorScheme::default()
                })
                .add_series(Box::new(candles))
                .add_series(Box::new(bars))
                .add_series(Box::new(lines))
                .add_series(Box::new(circles))
                .add_series(Box::new(held))
                .with_price_level(PriceLevel {
                    id: 7,
                    y: 60.0,
                    color: Color::from_linear(1.0, 0.0, 1.0, 1.0),
                    tag: vec![TagLine::new("CLOSE 60")],
                    ..PriceLevel::default()
                })
                .with_market_detail_link(link.clone())
                .with_market_detail_reference_level(7)
                .with_data_channel();
            let mut f = Fixture::new(plot, renderer);
            f.tree
                .replace_widget_only(f.sibling, Box::new(AlignedOrderBook::new(link.clone())));
            let mut peer = IndependentOutput::new(&mut f);
            let first = f.warm();
            let old_key = f.plot().target_resource_for_test().unwrap();
            assert_eq!(old_key.1, PhysicalExtent::new(382, 240));
            let old_binding = f.renderer.admit_image(first.texture_id()).unwrap();
            let before = gpu_audit::snapshot();
            let seed_pixels = pixels(&f, first, SEED_SITES);
            assert_eq!(seed_pixels, [GREEN, BLUE, RED, MAGENTA]);
            eprintln!("series supersession warm {old_key:?} {first:?} pixels={seed_pixels:?}");

            let mut peer_submissions = 0;
            let mut pending_frame = |f: &mut Fixture, width, registering| {
                let attempt = f.attempts.next();
                let frame = f.frame_at(width, f.surface, attempt);
                f.assert_pending(&frame);
                assert!(frame.plot_evidence.iter().all(|plot| plot.front.is_none()));
                assert_eq!(f.plot().presented_texture_id_for_test(), None);
                assert_eq!(link.price_range(), (0.0, 100.0));
                assert_eq!(link.reference_line().unwrap().price, 60.0);
                assert_eq!(f.renderer.has_pending_image_admissions(), registering);
                eprintln!(
                    "series supersession attempt={} key={:?} job={} registrar={registering}",
                    attempt.sequence(),
                    f.plot().target_resource_for_test().unwrap(),
                    f.plot().plot_submission_generation_for_test(),
                );
                peer.tree.mark_paint_dirty(peer.root);
                assert!(!peer.submit_at(f, frame, "Independent", first, attempt));
                peer_submissions += 1;
            };

            arm.store(true, Ordering::Release);
            pending_frame(&mut f, 620.0, false);
            entered
                .recv_timeout(Duration::from_secs(10))
                .expect("replacement owns renderer");
            let held_key = f.plot().target_resource_for_test().unwrap();
            assert_eq!(held_key.1, PhysicalExtent::new(370, 240));
            let (held_current, held_ready) = f.plot().plot_pending_signals_for_test();
            assert!(held_current.load(Ordering::Acquire));
            assert!(!held_ready.load(Ordering::Acquire));

            let latest_candle = RenderCandle {
                body_top: 72.0,
                ..candle(10.75, 96.0)
            };
            for update in [
                DataUpdate::ReplaceCandles {
                    series_index: 0,
                    candles: vec![candle(2.25, 76.0)],
                },
                DataUpdate::AppendCandle {
                    series_index: 0,
                    candle: candle(9.75, 84.0),
                },
                DataUpdate::UpdateLastCandle {
                    series_index: 0,
                    candle: latest_candle,
                },
                DataUpdate::ReplaceBarPoints {
                    series_index: 1,
                    points: vec![bar(5.0, 14.0)],
                },
                DataUpdate::AppendBarPoint {
                    series_index: 1,
                    point: bar(6.75, 17.0),
                },
                DataUpdate::UpdateLastBarPoint {
                    series_index: 1,
                    point: bar(6.0, 18.0),
                },
            ] {
                data.send(update).unwrap();
            }
            let before_direct = f.plot().render_generation_for_test();
            f.plot_mut().update_series_data(
                2,
                SeriesData::Lines(vec![
                    LinePoint { x: 7.0, y: 25.0 },
                    LinePoint { x: 8.0, y: 25.0 },
                    LinePoint { x: 9.0, y: 30.0 },
                ]),
            );
            f.plot_mut().update_series_data(
                3,
                SeriesData::Circles(vec![circle(10.0, 67.0), circle(11.0, 82.0)]),
            );
            assert_eq!(f.plot().render_generation_for_test(), before_direct);
            let view = f.plot_mut().view_state_mut();
            view.x_min = -0.5;
            view.x_max = 11.5;
            f.plot_mut().update_price_level(7, |level| {
                level.y = 72.0;
                level.tag = vec![TagLine::new("CLOSE 72")];
            });
            let requested = f.plot().render_generation_for_test();
            pending_frame(&mut f, 620.0, false);
            assert!(!held_current.load(Ordering::Acquire));
            pending_frame(&mut f, 640.0, false);
            assert_eq!(f.plot().target_resource_for_test(), Some(old_key));
            pending_frame(&mut f, 620.0, false);
            assert_eq!(f.plot().target_resource_for_test(), Some(held_key));
            assert_eq!(f.plot().render_generation_for_test(), requested);
            assert_eq!(data.len(), 6);

            // The last request can actually finish using the old series and the
            // repeated extent. The next ordinary Update must supersede it before
            // any registrar request for that result is allowed.
            let (obsolete_current, obsolete_ready) = f.plot().plot_pending_signals_for_test();
            let obsolete_job = f.plot().plot_submission_generation_for_test();
            resume.send(()).unwrap();
            f.wait_worker();
            assert!(obsolete_ready.load(Ordering::Acquire));
            assert!(!held_ready.load(Ordering::Acquire));
            pending_frame(&mut f, 620.0, false);
            assert!(!obsolete_current.load(Ordering::Acquire));
            assert_eq!(
                f.plot().plot_submission_generation_for_test(),
                obsolete_job + 1
            );
            assert_eq!(f.plot().render_generation_for_test(), requested + 8);
            assert!(data.is_empty());
            assert!(old_binding.availability().is_err());
            assert!(f.renderer.admit_image(first.texture_id()).is_err());
            assert_eq!(
                gpu_audit::snapshot().custom_registrations,
                before.custom_registrations
            );
            let view = f.plot().view_state();
            assert_eq!(
                (view.x_min, view.x_max, view.y_min, view.y_max),
                (-0.5, 11.5, 0.0, 96.0)
            );

            f.wait_worker();
            pending_frame(&mut f, 620.0, true);
            assert_eq!(peer_submissions, 6);
            assert_eq!(link.reference_line().unwrap().price, 60.0);
            let attempt = f.attempts.next();
            let ready = f.frame_at(620.0, f.surface, attempt);
            assert!(has_tag(&ready.scene_root, "CLOSE 72"));
            assert!(!has_tag(&ready.scene_root, "CLOSE 60"));
            let book = f.tree.get_absolute_paint_rect(f.sibling).unwrap();
            assert!(has_reference_segment(
                &ready.scene_root,
                book.x,
                book.y + 60.0
            ));
            assert_eq!(link.price_range(), (0.0, 96.0));
            assert_eq!(link.reference_line().unwrap().price, 72.0);
            let binding = f.submit(ready);
            assert_eq!(binding.texture_id(), first.texture_id());
            assert_ne!(binding.generation(), first.generation());
            let admitted = f.renderer.admit_image(binding.texture_id()).unwrap();
            assert_eq!(admitted.identity(), binding);
            assert_eq!(
                gpu_audit::snapshot().custom_registrations,
                before.custom_registrations + 1
            );

            let current = pixels(
                &f,
                binding,
                [
                    (2.25, 50.0),
                    (10.75, 65.0),
                    (5.0, 6.0),
                    (6.0, 9.0),
                    (7.5, 25.0),
                    (8.5, 27.5),
                    (10.0, 67.0),
                    (11.0, 82.0),
                ],
            );
            let obsolete = pixels(&f, binding, SEED_SITES);
            let replaced = pixels(&f, binding, [(9.75, 50.0), (6.75, 8.0)]);
            let reference = pixels(&f, binding, [(11.35, 71.8), (11.35, 72.0), (11.35, 72.2)]);
            eprintln!(
                "series supersession resumed attempt={} {binding:?} current={current:?} seed={obsolete:?} intermediate={replaced:?} reference={reference:?} A submissions=2 B submissions={peer_submissions} registrations=2 (one warm, one replacement)",
                attempt.sequence(),
            );
            assert_eq!(
                current,
                [GREEN, GREEN, BLUE, BLUE, RED, RED, MAGENTA, MAGENTA]
            );
            assert_eq!(obsolete, [BLACK; 4]);
            assert_eq!(replaced, [BLACK; 2]);
            assert!(reference.contains(&MAGENTA));

            f.tree.remove_node(f.plot);
            drain_requested_images_for_reused_scene(&mut f.renderer);
            assert!(admitted.availability().is_err());
            assert!(!f.renderer.has_pending_image_admissions());
            assert!(!f.renderer.has_pending_image_unregistrations());
        }

        #[test]
        fn plot_direct_series_update_reaches_first_registered_image() {
            let (cold, cold_new, cold_seed) = {
                let mut f = fixture(None);
                assert_eq!(f.plot().target_resource_for_test(), None);
                replace_series(f.plot_mut());
                let first = f.warm();
                (
                    first,
                    pixels(&f, first, NEW_SITES),
                    pixels(&f, first, SEED_SITES),
                )
            };

            let mut f = fixture(None);
            let seed = f.warm();
            assert_eq!(pixels(&f, seed, SEED_SITES), [GREEN, BLUE, RED, MAGENTA]);
            let target = f.plot().target_resource_for_test();
            let submission = f.plot().plot_submission_generation_for_test();
            replace_series(f.plot_mut());
            let update = f.frame(640.0);
            assert!(
                update
                    .plot_evidence
                    .iter()
                    .all(|plot| plot.pending_surface().is_none())
            );
            assert_eq!(f.plot().target_resource_for_test(), target);
            assert!(f.plot().plot_submission_generation_for_test() > submission);
            f.wait_worker();
            let ready = f.frame(640.0);
            let mounted = f.submit(ready);
            assert_eq!(mounted.texture_id(), seed.texture_id());
            assert_ne!(mounted.generation(), seed.generation());
            let mounted_new = pixels(&f, mounted, NEW_SITES);
            let mounted_seed = pixels(&f, mounted, SEED_SITES);
            eprintln!("Direct series cold {cold:?} replacement={cold_new:?} seed={cold_seed:?}");
            eprintln!(
                "Direct series mounted {mounted:?} replacement={mounted_new:?} seed={mounted_seed:?}"
            );

            assert_eq!(
                mounted_new,
                [GREEN, GREEN, BLUE, BLUE, RED, RED, MAGENTA, MAGENTA]
            );
            assert_eq!(mounted_seed, [BLACK; 4]);
            assert_eq!(
                (cold_new, cold_seed),
                (mounted_new, mounted_seed),
                "first registered Plot front"
            );
        }

        #[test]
        fn plot_direct_series_update_defers_when_worker_owns_renderer() {
            let (hold, entered, resume) = held_series(false);
            let arm = hold.armed.clone();
            let mut f = fixture(Some(hold));
            let resume = resume;
            let invalid_index_is_inert = |f: &mut Fixture| {
                let generation = f.plot().render_generation_for_test();
                f.plot_mut()
                    .update_series_data(usize::MAX, SeriesData::Lines(Vec::new()));
                assert_eq!(f.plot().render_generation_for_test(), generation);
            };
            invalid_index_is_inert(&mut f);
            let seed = f.warm();
            let submission = f.plot().plot_submission_generation_for_test();
            invalid_index_is_inert(&mut f);
            f.frame(640.0);
            assert_eq!(f.plot().plot_submission_generation_for_test(), submission);

            arm.store(true, Ordering::Release);
            f.plot_mut().set_crosshair(Some((1.0, 1.0)));
            f.frame(640.0);
            let blocked = entered
                .recv_timeout(Duration::from_secs(10))
                .expect("worker owns renderer");
            let generation = f.plot().render_generation_for_test();
            let submission = f.plot().plot_submission_generation_for_test();
            for y in [17.0, 22.0] {
                f.plot_mut().update_series_data(
                    2,
                    SeriesData::Lines(vec![LinePoint { x: 3.0, y }, LinePoint { x: 5.0, y }]),
                );
            }
            let held = f.frame(640.0);
            assert!(
                held.plot_evidence
                    .iter()
                    .all(|plot| plot.pending_surface().is_none())
            );
            assert_eq!(f.plot().render_generation_for_test(), generation);
            assert_eq!(f.plot().plot_submission_generation_for_test(), submission);
            assert_eq!(pixels(&f, seed, [(1.0, 31.0), (4.0, 22.0)]), [RED, BLACK]);

            while f.wake.try_recv().is_ok() {}
            resume.send(()).unwrap();
            let deadline = Instant::now() + Duration::from_secs(10);
            while f
                .wake
                .recv_deadline(deadline)
                .expect("worker completed")
                .thread
                != blocked.thread
            {}
            let released = f.frame(640.0);
            assert_eq!(f.plot().render_generation_for_test(), generation + 1);
            let intermediate = f.submit(released);
            f.frame(640.0);
            assert!(f.plot().plot_submission_generation_for_test() > submission);
            f.wait_worker();
            let ready = f.frame(640.0);
            let updated = f.submit(ready);
            assert_eq!(updated.texture_id(), seed.texture_id());
            assert_ne!(updated.generation(), intermediate.generation());
            let view = f.plot().view_state();
            // Deferred data uses the ordinary candle-bounded Update viewport.
            assert!(view.x_min < 3.0 && 5.0 < view.x_max);
            assert_eq!((view.y_min, view.y_max), (0.0, 77.0));
            assert_eq!(
                pixels(&f, updated, [(1.0, 31.0), (4.0, 17.0), (4.0, 22.0)]),
                [BLACK, BLACK, RED]
            );
        }
    }

    #[test]
    fn plot_close_tag_keeps_its_registered_candle_and_reference_until_completion() {
        const A: f64 = 60_033.0;
        const B: f64 = 60_043.0;
        const LEVEL: u64 = 7;
        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
        let link = MarketDetailLink::new(60_029.0, 60_037.0);
        let close_candle = |close: f64| RenderCandle {
            high: close + 2.0,
            low: 60_029.0,
            body_top: close,
            body_bottom: 60_031.0,
            ..candle(8.0, close + 2.0)
        };
        let mut candles = CandleSeries::new().with_colors(
            Color::from_linear(0.0, 1.0, 0.0, 1.0),
            Color::from_linear(1.0, 0.0, 0.0, 1.0),
        );
        candles.set_candles(vec![close_candle(A)]);
        let (hold, entered, resume) = held_series(false);
        let arm = hold.armed.clone();
        let tag = |close: f64| {
            TagLine::new(format!("{close:.0}")).color(Color::from_linear(1.0, 1.0, 0.0, 1.0))
        };
        let (plot, data) = Plot::new()
            .with_view_state(ViewState::new(0.0, 10.0, 60_029.0, 60_037.0))
            .with_initial_follow_latest(false)
            .with_gpu_resources(renderer.gpu_resources())
            .with_color_scheme(PlotColorScheme {
                background: Color::BLACK,
                ..PlotColorScheme::default()
            })
            .add_series(Box::new(candles))
            .add_series(Box::new(hold))
            .with_price_level(PriceLevel {
                id: LEVEL,
                y: A,
                color: Color::from_linear(1.0, 0.0, 1.0, 1.0),
                tag: vec![tag(A)],
                ..PriceLevel::default()
            })
            .with_market_detail_link(link.clone())
            .with_market_detail_reference_level(LEVEL)
            .with_data_channel();
        let mut f = Fixture::new(plot, renderer);
        let resume = resume;
        f.tree
            .replace_widget_only(f.sibling, Box::new(AlignedOrderBook::new(link.clone())));
        let first = f.warm();
        let old_target = f.plot().target_resource_for_test();
        let old_view = f.plot().view_state().clone();
        let old_range = link.price_range();
        let warm_pixels = f.close_glyph_pixels();
        assert!(old_range.0 < A && A < old_range.1 && old_range.1 < B);
        assert!(warm_pixels > 0, "warm close tag produced no glyph pixels");

        let update_close = |f: &mut Fixture, close| {
            data.send(DataUpdate::UpdateLastCandle {
                series_index: 0,
                candle: close_candle(close),
            })
            .unwrap();
            f.plot_mut().update_price_level(LEVEL, |level| {
                level.y = close;
                level.tag = vec![tag(close)];
            });
            f.tree.mark_paint_dirty(f.plot);
        };
        let plot_pixels =
            |f: &Fixture, frame: ImageBindingIdentity, view: &ViewState, reference: f64| {
                let transform = Transform::from_view_state(view, 1.0, 1.0);
                let pixel = |x, y| {
                    f.renderer.render_registered_image_pixel_for_test(
                        frame.texture_id(),
                        [x, y],
                        [1.0; 4],
                        wgpu::Color::TRANSPARENT,
                    )
                };
                let candle = pixel(
                    transform.x_to_px(8.0),
                    transform.y_to_px((60_031.0 + reference) * 0.5),
                );
                let line = [-0.7, 0.0, 0.7]
                    .map(|dy| pixel(0.30, transform.y_to_px(reference) + dy / 240.0));
                assert!(candle[1] > 180 && candle[0] < 80, "candle={candle:?}");
                assert!(
                    line.iter().any(|p| p[0] > 160 && p[1] < 80 && p[2] > 160),
                    "reference={line:?}"
                );
            };
        let book_reference =
            |f: &Fixture, frame: &RetainedSceneTestFrame, range: (f64, f64), price| {
                let book = f.tree.get_absolute_paint_rect(f.sibling).unwrap();
                let y = ((range.1 - price) / (range.1 - range.0) * book.height as f64) as f32;
                assert!(has_reference_segment(&frame.scene_root, book.x, book.y + y));
            };

        arm.store(true, Ordering::Release);
        update_close(&mut f, B);
        let held = f.frame(640.0);
        entered
            .recv_timeout(Duration::from_secs(10))
            .expect("own Plot worker held");
        let old_tag = has_tag(&held.scene_root, "60033");
        let new_tag = has_tag(&held.scene_root, "60043");
        assert_eq!(f.plot().target_resource_for_test(), old_target);
        assert_eq!(f.plot().price_level_display_y_for_test(LEVEL), Some(B));
        let requested = f.plot().view_state().clone();
        let requested_range = (requested.y_min, requested.y_max);
        assert!(requested_range.1 > B);
        assert_eq!(link.price_range(), old_range);
        assert_eq!(link.reference_line().unwrap().price, A);
        book_reference(&f, &held, old_range, A);
        assert_eq!(f.submit(held), first);
        plot_pixels(&f, first, &old_view, A);
        let held_pixels = f.close_glyph_pixels();
        resume.send(()).unwrap();
        eprintln!(
            "Plot close tag warm={warm_pixels} held={held_pixels} old_range={old_range:?} requested={requested_range:?}"
        );
        assert_eq!((old_tag, new_tag, held_pixels), (true, false, warm_pixels));

        f.wait_worker();
        let settled = f.frame(640.0);
        assert!(has_tag(&settled.scene_root, "60043"));
        assert!(!has_tag(&settled.scene_root, "60033"));
        book_reference(&f, &settled, requested_range, B);
        let second = f.submit(settled);
        assert_eq!(first.texture_id(), second.texture_id());
        assert_ne!(first.generation(), second.generation());
        assert_eq!(link.price_range(), requested_range);
        assert_eq!(link.reference_line().unwrap().price, B);
        plot_pixels(&f, second, &requested, B);
        let settled_pixels = f.close_glyph_pixels();
        eprintln!("Plot close tag settled={settled_pixels}");
        assert!(settled_pixels > 0);

        // Leave an obsolete 60033 registration queued at size B, then return
        // to size A under a different submission carrying 60043.
        update_close(&mut f, A);
        let size_b = f.frame(620.0);
        f.assert_pending(&size_b);
        f.wait_worker();
        let queued_b = f.frame(620.0);
        f.assert_pending(&queued_b);
        assert!(f.renderer.has_pending_image_admissions());
        let generation_b = f.plot().plot_submission_generation_for_test();

        update_close(&mut f, B);
        let size_a = f.frame(640.0);
        f.assert_pending(&size_a);
        assert!(f.plot().plot_submission_generation_for_test() > generation_b);
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert_eq!(link.reference_line().unwrap().price, B);
        f.wait_worker();
        let queued_a = f.frame(640.0);
        f.assert_pending(&queued_a);
        drain_requested_images_for_reused_scene(&mut f.renderer);
        let settled_a = f.frame(640.0);
        assert!(has_tag(&settled_a.scene_root, "60043"));
        assert!(!has_tag(&settled_a.scene_root, "60033"));
        book_reference(&f, &settled_a, requested_range, B);
        let next = f.submit(settled_a);
        assert_eq!(next.texture_id(), first.texture_id());
        assert_ne!(next.generation(), second.generation());
        plot_pixels(&f, next, &requested, B);
        let aba_pixels = f.close_glyph_pixels();
        eprintln!("Plot close tag A-B-A={aba_pixels}");
        assert_eq!(aba_pixels, settled_pixels);
    }

    #[test]
    fn plot_resize_pending_twenty_drags_with_the_own_renderer_busy() {
        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
        let gpu = renderer.gpu_resources();
        let pool = Arc::new(PlotRenderPool::new());
        let link = MarketDetailLink::new(0.0, 100.0);
        let mut candles = CandleSeries::new().with_colors(
            Color::from_linear(0.0, 1.0, 0.0, 1.0),
            Color::from_linear(1.0, 0.0, 0.0, 1.0),
        );
        candles.set_candles(
            [2.0, 5.0, 8.0]
                .into_iter()
                .map(|x| candle(x, 74.0))
                .collect(),
        );
        let blue = Color::from_linear(0.0, 0.0, 1.0, 1.0);
        let mut bars = BarSeries::new()
            .with_style(BarStyle {
                color: blue,
                viewport_fraction: Some(0.2),
                ..BarStyle::default()
            })
            .with_auto_scale_y(false);
        bars.set_points(
            [2.0, 5.0, 8.0]
                .into_iter()
                .map(|x| BarPoint {
                    x,
                    y: 10.0,
                    color: Some(blue),
                })
                .collect(),
        );
        let (held, own_entered, own_resume) = held_series(false);
        let arm = held.armed.clone();
        let (plot, data) = Plot::new()
            .with_view_state(ViewState::new(0.0, 10.0, 0.0, 100.0))
            .with_initial_follow_latest(false)
            .with_gpu_resources(gpu.clone())
            .with_render_pool(pool.clone())
            .with_color_scheme(PlotColorScheme {
                background: Color::BLACK,
                grid: Color::from_linear(1.0, 0.0, 0.0, 1.0),
                ..PlotColorScheme::default()
            })
            .add_series(Box::new(candles))
            .add_series(Box::new(bars))
            .add_series(Box::new(held))
            .with_price_level(PriceLevel {
                id: 7,
                y: 70.0,
                color: Color::from_linear(1.0, 0.0, 1.0, 1.0),
                tag: vec![TagLine::new("CLOSE")],
                ..PriceLevel::default()
            })
            .with_market_detail_link(link.clone())
            .with_market_detail_reference_level(7)
            .with_data_channel();
        let mut f = Fixture::new(plot, renderer);
        f.tree
            .replace_widget_only(f.sibling, Box::new(AlignedOrderBook::new(link.clone())));
        let cold = f.frame(640.0);
        assert!(
            cold.plot_evidence
                .iter()
                .all(|p| p.pending_surface().is_none())
        );
        f.wait_worker();
        let initial = f.frame(640.0);
        let first = f.submit(initial);
        f.pixels(first, (0.0, 100.0), 70.0);
        assert_eq!(
            f.plot().target_resource_for_test().unwrap().1,
            PhysicalExtent::new(382, 240)
        );

        let held_workers: Vec<_> = (1..pool.worker_count_for_test())
            .map(|_| {
                let (hold, entered, resume) = held_series(true);
                let mut other = Plot::new()
                    .with_gpu_resources(gpu.clone())
                    .with_render_pool(pool.clone())
                    .add_series(Box::new(hold));
                other.init_rendering(8, 8);
                entered.recv_timeout(Duration::from_secs(10)).unwrap();
                (other, resume)
            })
            .collect();
        arm.store(true, Ordering::Release);
        data.send(DataUpdate::UpdateLastCandle {
            series_index: 0,
            candle: candle(8.0, 92.0),
        })
        .unwrap();
        f.plot_mut().update_price_level(7, |level| level.y = 72.0);
        let same_key = f.frame(640.0);
        assert!(
            same_key
                .plot_evidence
                .iter()
                .all(|p| p.pending_surface().is_none())
        );
        assert_eq!(f.submit(same_key), first);
        own_entered
            .recv_timeout(Duration::from_secs(10))
            .expect("own renderer is occupied");
        data.send(DataUpdate::UpdateLastCandle {
            series_index: 0,
            candle: candle(8.0, 92.0),
        })
        .unwrap();

        let left = f.tree.get_absolute_paint_rect(f.plot).unwrap();
        let right = f.tree.get_absolute_paint_rect(f.sibling).unwrap();
        let handle = (left.x + left.width + right.x) * 0.5;
        f.route(FusedInputEvent::PointerDown {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(handle, 120.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
            click_count: 1,
        });
        let before = gpu_audit::snapshot();
        for step in 1..=20 {
            f.route(FusedInputEvent::PointerMove {
                window_id: WindowId::PRIMARY,
                time: Instant::now(),
                pos: Point::new(handle + step as f32 * 2.0, 120.0),
                delta: (2.0, 0.0),
                device: PointerDevice::Mouse,
                buttons: PointerButtonsState {
                    primary: true,
                    ..PointerButtonsState::default()
                },
            });
            let frame = f.frame(640.0);
            f.assert_pending(&frame);
            assert_eq!(
                f.plot().target_resource_for_test().unwrap().1,
                PhysicalExtent::new(382 + step * 2, 240)
            );
            assert_eq!(link.reference_line().unwrap().price, 70.0);
        }
        let latest = f.plot().plot_submission_generation_for_test();
        assert_eq!(
            gpu_audit::snapshot().custom_textures,
            before.custom_textures
        );
        f.route(FusedInputEvent::PointerUp {
            window_id: WindowId::PRIMARY,
            time: Instant::now(),
            pos: Point::new(handle + 40.0, 120.0),
            device: PointerDevice::Mouse,
            button: PointerButton::Primary,
        });
        f.tree.mark_layout_dirty(f.root);
        let stationary = f.frame(640.0);
        f.assert_pending(&stationary);
        assert_eq!(f.plot().plot_submission_generation_for_test(), latest);

        own_resume.send(()).unwrap();
        for (_, resume) in &held_workers {
            resume.send(()).unwrap();
        }
        f.wait_worker();
        let latest_after_data = f.frame(640.0);
        f.assert_pending(&latest_after_data);
        // The queued data update is admitted only after the held worker exits.
        // If it superseded the first replacement, consume the actual new wake.
        if !f.renderer.has_pending_image_admissions() {
            f.wait_worker();
            let queued = f.frame(640.0);
            f.assert_pending(&queued);
        }
        assert!(f.renderer.has_pending_image_admissions());
        let held_registration = f.frame(640.0);
        f.assert_pending(&held_registration);
        assert_eq!(link.price_range(), (0.0, 100.0));
        assert_eq!(link.reference_line().unwrap().price, 70.0);
        drain_requested_images_for_reused_scene(&mut f.renderer);
        let complete = f.frame(640.0);
        let book = f.tree.get_absolute_paint_rect(f.sibling).unwrap();
        let final_reference_y = ((100.85 - 72.0) / (100.85 - 24.15) * 240.0) as f32;
        assert!(has_reference_segment(
            &complete.scene_root,
            book.x,
            book.y + final_reference_y
        ));
        let second = f.submit(complete);
        assert_ne!(first.generation(), second.generation());
        assert_eq!(
            f.plot().target_resource_for_test().unwrap().1,
            PhysicalExtent::new(422, 240)
        );
        assert_eq!(link.price_range(), (24.15, 100.85));
        assert_eq!(link.reference_line().unwrap().price, 72.0);
        f.pixels(second, link.price_range(), 72.0);
        f.tree.mark_layout_dirty(f.root);
        let stable = f.frame(640.0);
        assert!(
            stable
                .plot_evidence
                .iter()
                .all(|p| p.pending_surface().is_none())
        );
        assert_eq!(
            f.plot().presented_texture_id_for_test(),
            Some(second.texture_id())
        );
    }

    fn resize_fixture() -> Fixture {
        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
        let mut bars = BarSeries::new();
        bars.set_points(vec![BarPoint {
            x: 1.0,
            y: 1.0,
            color: None,
        }]);
        let plot = Plot::new()
            .with_gpu_resources(renderer.gpu_resources())
            .with_view_state(ViewState::new(0.0, 2.0, 0.0, 2.0))
            .add_series(Box::new(bars));
        Fixture::new(plot, renderer)
    }

    #[test]
    fn plot_resize_pending_rejects_queued_b_on_return_to_a() {
        let mut f = resize_fixture();
        let first = f.warm();
        let prior = f.renderer.admit_image(first.texture_id()).unwrap();
        let b = f.frame(620.0);
        f.assert_pending(&b);
        f.wait_worker();
        let b_registration = f.frame(620.0);
        f.assert_pending(&b_registration);
        assert!(f.renderer.has_pending_image_admissions());
        let b_generation = f.plot().plot_submission_generation_for_test();

        let a = f.frame(640.0);
        f.assert_pending(&a);
        assert!(f.plot().plot_submission_generation_for_test() > b_generation);
        assert_eq!(
            f.plot().target_resource_for_test().unwrap().1,
            PhysicalExtent::new(382, 240)
        );
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(prior.availability().is_err());
        assert!(
            f.renderer.admit_image(first.texture_id()).is_err(),
            "withdrawn B registered"
        );

        f.wait_worker();
        let a_registration = f.frame(640.0);
        f.assert_pending(&a_registration);
        drain_requested_images_for_reused_scene(&mut f.renderer);
        let complete = f.frame(640.0);
        let next = f.submit(complete);
        assert_eq!(next.texture_id(), first.texture_id());
        assert_ne!(next.generation(), first.generation());
    }

    #[test]
    fn plot_resize_pending_settles_on_surface_loss_replacement_and_closed_pool() {
        let mut f = resize_fixture();
        let first = f.warm();
        let b = f.frame(620.0);
        f.assert_pending(&b);
        f.wait_worker();
        let b_registration = f.frame(620.0);
        f.assert_pending(&b_registration);

        // A zero native extent has no CustomRenderTargetSnapshot to prepare.
        f.tree.withdraw_inactive_plot_surface(f.surface);
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(f.renderer.admit_image(first.texture_id()).is_err());
        let cold_again = f.frame(640.0);
        assert!(
            cold_again
                .plot_evidence
                .iter()
                .all(|plot| plot.pending_surface().is_none())
        );
        f.wait_worker();
        let ready = f.frame(640.0);
        let before_switch = f.submit(ready);

        let pending = f.frame(620.0);
        f.assert_pending(&pending);
        f.wait_worker();
        let queued = f.frame(620.0);
        f.assert_pending(&queued);
        let bounds = RectF::new(0.0, 0.0, 640.0, 240.0);
        let other = f.compositor.create_surface(1, bounds, SceneKey::BASE);
        f.renderer
            .register_headless_render_target(other, (640, 240), 1.0)
            .unwrap();
        let switched = f.frame_for_surface(640.0, other);
        assert!(
            switched
                .plot_evidence
                .iter()
                .all(|plot| plot.pending_surface().is_none())
        );
        assert!(f.plot().presented_texture_id_for_test().is_none());
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(f.renderer.admit_image(before_switch.texture_id()).is_err());

        let replacement = Plot::new().with_gpu_resources(f.renderer.gpu_resources());
        f.tree.replace_widget_only(f.plot, Box::new(replacement));
        let cold_replacement = f.frame(620.0);
        assert!(
            cold_replacement
                .plot_evidence
                .iter()
                .all(|plot| plot.pending_surface().is_none())
        );
        assert!(f.plot().presented_texture_id_for_test().is_none());
        f.wait_worker();
        let replacement_ready = f.frame(620.0);
        let replacement_front = f.submit(replacement_ready);
        assert_ne!(replacement_front.texture_id(), before_switch.texture_id());

        f.plot_mut()
            .set_render_pool_for_test(Arc::new(PlotRenderPool::closed_for_test()));
        let refused = f.frame(640.0);
        assert!(
            refused
                .plot_evidence
                .iter()
                .all(|plot| plot.pending_surface().is_none())
        );
        assert!(f.plot().presented_texture_id_for_test().is_none());
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(
            f.renderer
                .admit_image(replacement_front.texture_id())
                .is_err()
        );
    }

    #[test]
    fn plot_resize_pending_settles_after_an_actual_worker_extent_refusal() {
        let mut f = resize_fixture();
        let front = f.warm();
        let width = 2.0
            * f.renderer
                .gpu_resources()
                .device
                .limits()
                .max_texture_dimension_2d as f32;
        let before = gpu_audit::snapshot();
        let oversized = f.frame(width);
        f.assert_pending(&oversized);
        f.wait_worker();
        let refused = f.frame(width);
        assert!(
            refused
                .plot_evidence
                .iter()
                .all(|plot| plot.pending_surface().is_none())
        );
        assert!(f.plot().presented_texture_id_for_test().is_none());
        assert_eq!(
            gpu_audit::snapshot().custom_textures,
            before.custom_textures
        );
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(f.renderer.admit_image(front.texture_id()).is_err());
    }

    #[test]
    fn plot_resize_pending_withdraws_unmounted_registration_and_settles_on_admission_failure() {
        let mut f = resize_fixture();
        let first = f.warm();
        let pending = f.frame(620.0);
        f.assert_pending(&pending);
        f.wait_worker();
        let queued = f.frame(620.0);
        f.assert_pending(&queued);
        assert!(f.renderer.has_pending_image_admissions());

        let replacement = Plot::new().with_gpu_resources(f.renderer.gpu_resources());
        f.tree.replace_widget_only(f.plot, Box::new(replacement));
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(f.renderer.admit_image(first.texture_id()).is_err());
        let cold = f.frame(620.0);
        assert!(cold.plot_evidence.iter().all(|plot| plot.pending_surface().is_none()));
        f.wait_worker();
        let ready = f.frame(620.0);
        let replacement = f.submit(ready);
        assert_ne!(first.texture_id(), replacement.texture_id());

        let pending = f.frame(640.0);
        f.assert_pending(&pending);
        f.wait_worker();
        let queued = f.frame(640.0);
        f.assert_pending(&queued);
        assert!(f.renderer.has_pending_image_admissions());
        f.renderer.gpu_resources().image_admission.close();
        let refused = f.frame(640.0);
        assert!(refused.plot_evidence.iter().all(|plot| plot.pending_surface().is_none()));
        assert!(f.plot().presented_texture_id_for_test().is_none());
        drain_requested_images_for_reused_scene(&mut f.renderer);
        assert!(f.renderer.admit_image(replacement.texture_id()).is_err());
    }

    struct IndependentOutput {
        surface: SurfaceId,
        view: wgpu::TextureView,
        tree: WidgetTree,
        root: NodeId,
        harness: RetainedSceneTestHarness,
    }

    impl IndependentOutput {
        fn new(f: &mut Fixture) -> Self {
            let bounds = RectF::new(0.0, 0.0, 640.0, 240.0);
            let surface = f.compositor.create_surface(1, bounds, SceneKey::new(2));
            f.renderer
                .register_headless_render_target(surface, (640, 240), 1.0)
                .unwrap();
            let view = plot_output_texture(&f.renderer, "Independent Plot termination output")
                .create_view(&Default::default());
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(Label::new("Independent")), None, 0);
            Self {
                surface,
                view,
                tree,
                root,
                harness: RetainedSceneTestHarness::new(),
            }
        }

        fn submit(
            &mut self,
            f: &mut Fixture,
            affected: RetainedSceneTestFrame,
            stage: &str,
            retired: ImageBindingIdentity,
        ) -> bool {
            self.tree
                .replace_widget_only(self.root, Box::new(Label::new(stage)));
            let attempt = f.attempts.next();
            self.submit_at(f, affected, stage, retired, attempt)
        }

        fn submit_at(
            &mut self,
            f: &mut Fixture,
            affected: RetainedSceneTestFrame,
            stage: &str,
            retired: ImageBindingIdentity,
            attempt: AppFrameAttemptId,
        ) -> bool {
            let target = f
                .renderer
                .custom_render_target_snapshot(self.surface)
                .unwrap()
                .with_app_frame_attempt(attempt);
            let peer = self.harness.frame_with_flat_viewport3d_transfers(
                &mut self.tree,
                &mut f.app,
                self.root,
                Constraints::tight(Size::new(640.0, 240.0)),
                &f.theme,
                &mut f.renderer,
                &f.icons,
                0.0,
                Some(target),
                Some(attempt),
            );
            assert!(peer.scene_built && has_tag(&peer.scene_root, stage));
            let mut peer_scene = SceneGraph::new();
            peer_scene.root = peer.scene_root;
            let mut scenes = HashMap::from([(self.surface, peer_scene)]);
            let affected_eligible =
                affected.scene_built && f.renderer.target_has_active_extent(f.surface);
            if affected_eligible {
                assert!(
                    affected
                        .plot_evidence
                        .iter()
                        .all(|plot| plot.front.is_none())
                );
                let mut scene = SceneGraph::new();
                scene.root = affected.scene_root;
                scenes.insert(f.surface, scene);
            }
            drain_requested_images_for_reused_scene(&mut f.renderer);
            let counter = f.renderer.frame_counter();
            let plan = DrawPlanBuilder::new()
                .build_multi(
                    &scenes,
                    f.compositor.snapshot_multi(),
                    f.renderer.mesh_cache(),
                    |_| 1.0,
                    counter,
                )
                .expect("scene must satisfy DrawPlan Build admission");
            let compiled = f
                .renderer
                .compile_multi_for_surfaces(&plan, &scenes.keys().copied().collect())
                .unwrap()
                .expect_ready("native Compile must be ready");
            assert!(
                f.renderer
                    .compiled_plot_image(&compiled, f.surface, retired.texture_id())
                    .is_none()
            );
            let affected = f
                .renderer
                .encode_compiled_target_for_test(&compiled, f.surface, &f.view);
            assert_eq!(affected.is_some(), affected_eligible);
            let peer = f
                .renderer
                .encode_compiled_target_for_test(&compiled, self.surface, &self.view)
                .expect("independent target executed");
            for commands in affected.into_iter().chain([peer]) {
                f.renderer.submit_compiled_target_for_test(commands);
            }
            f.renderer.advance_frame();
            eprintln!(
                "{stage}: affected output={affected_eligible}, independent {:?} submitted",
                self.surface
            );
            affected_eligible
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum PendingTermination {
        Remove,
        Replace,
        SwitchSurface,
        ZeroCoverage,
        ZeroTarget,
        RegistrarClosed,
    }

    fn termination_plot(
        renderer: &WgpuRenderer,
        link: &MarketDetailLink,
        price: f64,
        tag: &str,
    ) -> Plot {
        let mut series = BarSeries::new();
        series.set_points(vec![BarPoint {
            x: 1.0,
            y: 30.0,
            color: None,
        }]);
        Plot::new()
            .with_gpu_resources(renderer.gpu_resources())
            .with_view_state(ViewState::new(0.0, 2.0, 0.0, 100.0))
            .add_series(Box::new(series))
            .with_price_level(PriceLevel {
                id: 1,
                y: price,
                tag: vec![TagLine::new(tag)],
                ..PriceLevel::default()
            })
            .with_price_level(PriceLevel {
                id: 2,
                y: 65.0,
                tag: vec![TagLine::new("independent annotation")],
                ..PriceLevel::default()
            })
            .with_market_detail_link(link.clone())
            .with_market_detail_reference_level(1)
    }

    fn check_pending_termination(case: PendingTermination) {
        use PendingTermination::*;

        let renderer =
            pollster::block_on(WgpuRenderer::new_headless()).expect("required headless GPU");
        #[cfg(target_os = "macos")]
        assert_eq!(
            renderer.headless_adapter_for_test().backend,
            wgpu::Backend::Metal
        );
        let link = MarketDetailLink::new(0.0, 100.0);
        let projection = || {
            let snapshot = link.presented_snapshot().unwrap();
            (
                (snapshot.price_min, snapshot.price_max),
                (snapshot.axis_generation, snapshot.reference_generation),
                snapshot.reference_line,
            )
        };
        let (hold, entered, resume) = held_series(false);
        let arm = hold.armed.clone();
        let plot =
            termination_plot(&renderer, &link, 25.0, "warm reference").add_series(Box::new(hold));
        let mut f = Fixture::new(plot, renderer);
        let mut peer = IndependentOutput::new(&mut f);
        let surface = f.surface;
        let owner = f.tree.node(f.plot).unwrap().id;
        let mount = f.tree.concrete_mount(owner).unwrap();
        let first = f.warm();
        let warm_generation = f.plot().plot_submission_generation_for_test();
        let old_binding = f.renderer.admit_image(first.texture_id()).unwrap();
        let warm_projection = projection();
        assert_eq!(warm_projection.2.unwrap().price, 25.0);

        f.plot_mut().update_price_level(1, |level| {
            level.y = 75.0;
            level.tag = vec![TagLine::new("obsolete replacement")];
        });
        arm.store(true, Ordering::Release);
        let pending = f.frame(620.0);
        f.assert_pending(&pending);
        let started = entered
            .recv_timeout(Duration::from_secs(10))
            .expect("accepted replacement executing");
        let (current, delivered) = f.plot().plot_pending_signals_for_test();
        let old_generation = f.plot().plot_submission_generation_for_test();
        assert_eq!(old_generation, warm_generation + 1);
        let resized_rect = f.tree.get_absolute_paint_rect(f.plot).unwrap();
        let registrations = gpu_audit::snapshot().custom_registrations;
        assert!(current.load(Ordering::Acquire) && !delivered.load(Ordering::Acquire));
        assert!(!peer.submit(&mut f, pending, "Pending", first));
        assert!(old_binding.availability().is_err());
        assert!(f.renderer.admit_image(first.texture_id()).is_err());
        assert_eq!(projection(), warm_projection);

        let mut width = 620.0;
        match case {
            Remove => {
                f.tree.remove_node(f.plot);
                assert!(f.tree.node(f.plot).is_none());
                let placeholder =
                    f.tree
                        .add_child(f.root, Box::new(Label::new("Plot removed")), None, 0);
                f.tree.replace_widget_only(
                    f.root,
                    Box::new(SplitPane::horizontal(placeholder, f.sibling)),
                );
            }
            Replace => {
                let plot = termination_plot(&f.renderer, &link, 55.0, "new owner reference");
                f.tree.replace_widget_only(f.plot, Box::new(plot));
                assert_ne!(f.tree.concrete_mount(owner).unwrap(), mount);
            }
            SwitchSurface => {
                assert!(f.compositor.destroy_surface(surface));
                f.renderer.unregister_render_target(surface).unwrap();
                assert!(!f.renderer.target_has_active_extent(surface));
                assert!(f.renderer.custom_render_target_snapshot(surface).is_err());
                let bounds = RectF::new(0.0, 0.0, 640.0, 240.0);
                f.surface = f.compositor.create_surface(0, bounds, SceneKey::new(3));
                assert_ne!(f.surface, surface);
                f.renderer
                    .register_headless_render_target(f.surface, (640, 240), 1.0)
                    .unwrap();
                f.output = plot_output_texture(&f.renderer, "New Plot target");
                f.view = f.output.create_view(&Default::default());
            }
            ZeroCoverage => f.tree.replace_widget_only(
                f.root,
                Box::new(SplitPane::horizontal(f.plot, f.sibling).initial_ratio(0.0)),
            ),
            ZeroTarget => {
                f.renderer.resize_render_target(surface, (0, 240)).unwrap();
                assert!(!f.renderer.target_has_active_extent(surface));
                assert!(f.renderer.custom_render_target_snapshot(surface).is_err());
                width = 0.0;
            }
            RegistrarClosed => f.renderer.gpu_resources().image_admission.close(),
        }
        assert_eq!(mount.is_live(), !matches!(case, Remove | Replace));
        let retired_projection = projection();
        let terminal = f.frame(width);
        let cold_reference_visible =
            case == Replace && has_tag(&terminal.scene_root, "new owner reference");
        if case == Replace {
            assert_eq!(f.tree.get_absolute_paint_rect(f.plot), Some(resized_rect));
            assert_eq!(f.plot().plot_submission_generation_for_test(), 1);
            assert_eq!(f.plot().presented_texture_id_for_test(), None);
            assert!(has_tag(&terminal.scene_root, "independent annotation"));
        }
        if case == ZeroCoverage {
            assert_eq!(f.tree.get_absolute_paint_rect(f.plot).unwrap().width, 0.0);
            assert!(!f.plot().target_resource_for_test().unwrap().3);
        }
        if case == SwitchSurface {
            assert!(has_tag(&terminal.scene_root, "independent annotation"));
        }
        if case == RegistrarClosed {
            // Closing a mailbox is not a completion. The request is refused when used.
            f.assert_pending(&terminal);
            assert!(current.load(Ordering::Acquire));
        } else {
            assert!(
                terminal
                    .plot_evidence
                    .iter()
                    .all(|plot| plot.pending_surface().is_none())
            );
            assert!(!current.load(Ordering::Acquire));
        }
        let mut obsolete_tag = has_tag(&terminal.scene_root, "obsolete replacement");
        let mut warm_tag = has_tag(&terminal.scene_root, "warm reference");
        assert_eq!(
            peer.submit(&mut f, terminal, "Termination cutoff", first),
            !matches!(case, ZeroTarget | RegistrarClosed),
        );

        // The held thread cannot publish a completion wake until the release.
        for _ in f.wake.try_iter() {}
        resume.send(()).unwrap();
        let deadline = Instant::now() + Duration::from_secs(10);
        let next_worker_wake = || loop {
            let wake = f
                .wake
                .recv_deadline(deadline)
                .expect("held producer finished");
            if wake.thread == started.thread {
                break wake.audit;
            }
        };
        let finished = next_worker_wake();
        assert_eq!(
            finished.custom_unregistrations,
            started.audit.custom_unregistrations
        );
        let obsolete_submissions = finished.custom_submissions - started.audit.custom_submissions;
        assert!(obsolete_submissions <= 1);
        assert_eq!(delivered.load(Ordering::Acquire), case == RegistrarClosed);
        if matches!(case, Remove | Replace) {
            let retired = next_worker_wake();
            assert_eq!(
                retired.custom_unregistrations,
                started.audit.custom_unregistrations + 1
            );
        }
        assert_eq!(projection(), retired_projection);

        if case == RegistrarClosed {
            let refused = f.frame(width);
            assert!(
                refused
                    .plot_evidence
                    .iter()
                    .all(|plot| plot.pending_surface().is_none())
            );
            obsolete_tag |= has_tag(&refused.scene_root, "obsolete replacement");
            warm_tag |= has_tag(&refused.scene_root, "warm reference");
            assert!(has_tag(&refused.scene_root, "independent annotation"));
            assert!(peer.submit(&mut f, refused, "Registration refused", first));
        } else if matches!(case, Remove | ZeroCoverage | ZeroTarget) {
            let settled = f.frame(width);
            assert!(
                settled
                    .plot_evidence
                    .iter()
                    .all(|plot| plot.pending_surface().is_none())
            );
            obsolete_tag |= has_tag(&settled.scene_root, "obsolete replacement");
            warm_tag |= has_tag(&settled.scene_root, "warm reference");
            peer.submit(&mut f, settled, "Obsolete producer settled", first);
        }
        assert!(!current.load(Ordering::Acquire));
        assert_eq!(projection(), retired_projection);
        assert!(!f.renderer.has_pending_image_admissions());
        assert!(f.renderer.admit_image(first.texture_id()).is_err());
        let registration_attempts = gpu_audit::snapshot().custom_registrations - registrations;
        assert_eq!(registration_attempts, u64::from(case == RegistrarClosed));
        eprintln!(
            "{case:?}: surface={surface:?} mount={owner:?} retired={first:?} generation={old_generation} worker_submissions={obsolete_submissions} worker_delivery={} requests={registration_attempts} tag={obsolete_tag}",
            delivered.load(Ordering::Acquire),
        );
        assert!(
            !obsolete_tag && !warm_tag,
            "{case:?}: retired Plot reference escaped cutoff"
        );

        if matches!(case, Replace | SwitchSurface | ZeroCoverage | ZeroTarget) {
            if case != Replace {
                f.plot_mut().update_price_level(1, |level| {
                    level.y = 55.0;
                    level.tag = vec![TagLine::new("new owner reference")];
                });
            }
            if case == ZeroCoverage {
                f.tree.replace_widget_only(
                    f.root,
                    Box::new(SplitPane::horizontal(f.plot, f.sibling).initial_ratio(0.6)),
                );
            }
            if case == ZeroTarget {
                f.renderer
                    .resize_render_target(surface, (640, 240))
                    .unwrap();
            }
            if case != Replace {
                let cold = f.frame(620.0);
                assert!(
                    cold.plot_evidence
                        .iter()
                        .all(|plot| plot.pending_surface().is_none() && plot.front.is_none())
                );
                assert!(f.plot().plot_submission_generation_for_test() > old_generation);
            }
            f.wait_worker();
            let ready = f.frame(620.0);
            assert!(has_tag(&ready.scene_root, "new owner reference"));
            assert!(has_tag(&ready.scene_root, "independent annotation"));
            let new_front = f.submit(ready);
            assert_ne!(first.generation(), new_front.generation());
            if matches!(case, Replace | SwitchSurface) {
                assert_ne!(first.texture_id(), new_front.texture_id());
            }
            assert_eq!(link.reference_line().unwrap().price, 55.0);
            assert_eq!(link.price_range(), (0.0, 100.0));
            assert!(old_binding.availability().is_err());
        }
        assert!(
            !cold_reference_visible,
            "cold replacement painted its market reference before its first Image"
        );
    }

    #[test]
    fn plot_pending_owner_termination_unmount() {
        check_pending_termination(PendingTermination::Remove);
    }

    #[test]
    fn plot_pending_owner_termination_replacement() {
        check_pending_termination(PendingTermination::Replace);
    }

    #[test]
    fn plot_pending_owner_termination_surface_switch() {
        check_pending_termination(PendingTermination::SwitchSurface);
    }

    #[test]
    fn plot_pending_owner_termination_zero_coverage() {
        check_pending_termination(PendingTermination::ZeroCoverage);
    }

    #[test]
    fn plot_pending_owner_termination_zero_physical_target() {
        check_pending_termination(PendingTermination::ZeroTarget);
    }

    #[test]
    fn plot_pending_owner_termination_registration_refusal() {
        let subscriber = tracing_subscriber::fmt()
            .without_time()
            .with_ansi(false)
            .with_test_writer()
            .finish();
        tracing::subscriber::with_default(subscriber, || {
            check_pending_termination(PendingTermination::RegistrarClosed);
        });
    }
}

#[test]
fn plot_hit_metadata_is_created_once_before_low_level_gpu_initialization() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let hit_proxy_calls = Arc::new(AtomicUsize::new(0));
    let mut plot = Plot::new()
        .add_series(Box::new(HitMetadataSeries::new(
            hit_proxy_calls.clone(),
            10,
        )))
        .add_series(Box::new(HitMetadataSeries::new(
            hit_proxy_calls.clone(),
            20,
        )))
        .with_gpu_resources(renderer.gpu_resources());

    assert_eq!(plot.hit_entry_count_for_test(), 2);
    assert_eq!(hit_proxy_calls.load(Ordering::Relaxed), 2);

    plot.init_rendering(32, 24);
    assert_eq!(plot.hit_entry_count_for_test(), 2);
    assert_eq!(hit_proxy_calls.load(Ordering::Relaxed), 2);

    plot.init_rendering(48, 36);
    assert_eq!(plot.hit_entry_count_for_test(), 2);
    assert_eq!(hit_proxy_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn low_level_plot_hidpi_scale_compatibility() {
    let Some(renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    let mut gpu = renderer.gpu_resources();
    gpu.scale_factor = 2.0;
    let mut plot = Plot::new().with_gpu_resources(gpu);

    plot.init_rendering(64, 48);
    assert_eq!(plot.render_submission_for_test(), Some((2.0, 1)));

    plot.set_crosshair(Some((8.0, 6.0)));
    plot.render();
    assert_eq!(plot.render_submission_for_test(), Some((2.0, 2)));
}

#[test]
fn fractional_pixel_coverage_matches_production_image_raster_and_scissor() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    const TEXTURE_ID: u64 = 266_001;
    renderer.register_image_rgba8_srgb(TEXTURE_ID, 1, 1, &[255, 255, 255, 255]);

    let rects = [
        RectF::new(0.25, 0.75, 3.5, 2.25),
        RectF::new(-0.4, -0.2, 2.3, 1.7),
        RectF::new(6.6, 4.3, 2.2, 2.0),
        RectF::new(0.5, 1.5, 4.0, 3.0),
    ];
    for scale_factor in [1.0, 1.5, 2.0] {
        let physical_extent =
            PhysicalExtent::new((8.0 * scale_factor) as u32, (6.0 * scale_factor) as u32);
        let target =
            PhysicalTargetMetrics::new(Size::new(8.0, 6.0), physical_extent, scale_factor).unwrap();
        let constants = FrameConstants {
            viewport_size: [physical_extent.width as f32, physical_extent.height as f32],
            physical_width: physical_extent.width,
            physical_height: physical_extent.height,
            logical_width: 8.0,
            logical_height: 6.0,
            scale_factor,
            clear_color: Color::TRANSPARENT.into(),
        };

        for logical_rect in rects {
            let expected = physical_composition_coverage(logical_rect, target).unwrap();
            let raster = renderer.render_registered_image_coverage_for_test(
                TEXTURE_ID,
                logical_rect,
                None,
                physical_extent,
                scale_factor,
            );
            assert_eq!(
                raster, expected,
                "production Image coverage diverged at DPR {scale_factor} for {logical_rect:?}"
            );
            let scissor = constants.clip_rect_to_scissor(Some(&logical_rect));
            assert_eq!(
                scissor,
                expected.map_or((0, 0, 0, 0), |rect| {
                    (rect.x, rect.y, rect.width, rect.height)
                })
            );
        }

        let image_rect = RectF::new(-0.25, 0.25, 8.5, 5.4);
        let clip_rect = RectF::new(1.1, 0.7, 4.25, 3.2);
        let visible_rect = image_rect.intersection(&clip_rect).unwrap();
        let expected = physical_composition_coverage(visible_rect, target).unwrap();
        let raster = renderer.render_registered_image_coverage_for_test(
            TEXTURE_ID,
            image_rect,
            Some(clip_rect),
            physical_extent,
            scale_factor,
        );
        assert_eq!(
            raster, expected,
            "production Image plus scissor coverage diverged at DPR {scale_factor}"
        );
    }
}

#[test]
fn render_extent_is_size_anchored_while_composition_tracks_fractional_raster_phase() {
    let Some(mut renderer) = pollster::block_on(WgpuRenderer::new_headless()) else {
        return;
    };
    const DENSITY_TEXTURE_ID: u64 = 266_002;
    const COVERAGE_TEXTURE_ID: u64 = 266_003;
    renderer.register_image_rgba8_srgb(DENSITY_TEXTURE_ID, 2, 3, &[255; 2 * 3 * 4]);
    renderer.register_image_rgba8_srgb(COVERAGE_TEXTURE_ID, 1, 3, &[255; 3 * 4]);

    let physical_target = PhysicalExtent::new(12, 9);
    let snapshot =
        CustomRenderTargetSnapshot::new(SurfaceId::test_id(10), physical_target, 1.5).unwrap();
    let viewport = RectF::new(0.0, 0.0, 8.0, 6.0);
    let rects = [
        RectF::new(0.0, 0.0, 1.0, 2.0),
        RectF::new(0.25, 0.0, 1.0, 2.0),
    ];
    let mut coverage_widths = Vec::new();

    for logical_rect in rects {
        let metrics = snapshot.metrics_for(logical_rect, viewport).unwrap();
        let CustomRenderActivity::Active {
            physical_render_extent,
            physical_composition_coverage,
        } = metrics.activity
        else {
            panic!("positive visible fractional rect must remain active");
        };
        assert_eq!(physical_render_extent, PhysicalExtent::new(2, 3));
        coverage_widths.push(physical_composition_coverage.width);

        let density_raster = renderer.render_registered_image_coverage_for_test(
            DENSITY_TEXTURE_ID,
            logical_rect,
            None,
            physical_target,
            1.5,
        );
        let coverage_sized_raster = renderer.render_registered_image_coverage_for_test(
            COVERAGE_TEXTURE_ID,
            logical_rect,
            None,
            physical_target,
            1.5,
        );
        assert_eq!(density_raster, Some(physical_composition_coverage));
        assert_eq!(coverage_sized_raster, density_raster);
    }

    assert_eq!(coverage_widths, vec![1, 2]);
}
