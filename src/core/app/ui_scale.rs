//! SPEC-HAL-024 Layer E: bounded App-local scale requests and target readback.
//! App checkpoints own admission; existing target owners supply effective metrics
//! and submission evidence. This state performs no layout, GPU work or preference IO.

use crate::compositor::SurfaceId;
use std::collections::HashMap;

/// App-local logical UI scale, independent of platform density.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScale(f32);

impl UiScale {
    pub const ONE: Self = Self(1.0);

    pub fn get(self) -> f32 {
        self.0
    }
}

impl Default for UiScale {
    fn default() -> Self {
        Self::ONE
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleValueError {
    NonFinite,
    NonPositive,
}

impl TryFrom<f32> for UiScale {
    type Error = UiScaleValueError;

    fn try_from(value: f32) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            Err(UiScaleValueError::NonFinite)
        } else if value <= 0.0 {
            Err(UiScaleValueError::NonPositive)
        } else {
            Ok(Self(value))
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleReceipt {
    Queued { request_id: u64 },
    Unchanged { admitted_revision: u64 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleRejection {
    UnsupportedRuntime,
    PendingRequest,
    InvalidTargetMetrics { surface: SurfaceId },
    RequestIdentityExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleRequestOutcome {
    Queued,
    Admitted { revision: u64 },
    Rejected(UiScaleRejection),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScaleRequestStatus {
    pub request_id: u64,
    pub value: UiScale,
    pub outcome: UiScaleRequestOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleTargetFailure {
    UnknownTarget,
    InvalidMetrics,
    OutputNotProduced,
    TargetLost,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UiScaleTargetOutput {
    Pending,
    Submitted,
    Unavailable(UiScaleTargetFailure),
    Inactive,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScaleEffectiveSnapshot {
    pub admitted_revision: u64,
    pub metrics_revision: u64,
    pub surface: SurfaceId,
    pub platform_density: f32,
    pub physical_size: (u32, u32),
    pub effective_scale_factor: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScaleTargetStatus {
    pub surface: SurfaceId,
    pub effective: Option<UiScaleEffectiveSnapshot>,
    pub output: UiScaleTargetOutput,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UiScaleStatus {
    pub requested: Option<UiScaleRequestStatus>,
    pub admitted: UiScale,
    pub admitted_revision: u64,
    pub target: Option<UiScaleTargetStatus>,
}

pub(crate) struct UiScaleState {
    admitted: UiScale,
    revision: u64,
    last_request_id: u64,
    last_metrics_revision: u64,
    requested: Option<UiScaleRequestStatus>,
    targets: HashMap<SurfaceId, UiScaleTargetStatus>,
}

impl UiScaleState {
    pub(crate) fn initial(value: UiScale) -> Self {
        Self {
            admitted: value,
            revision: 0,
            last_request_id: 0,
            last_metrics_revision: 0,
            requested: None,
            targets: HashMap::new(),
        }
    }

    pub(crate) fn request(&mut self, value: UiScale) -> Result<UiScaleReceipt, UiScaleRejection> {
        if self.pending().is_some() {
            return Err(UiScaleRejection::PendingRequest);
        }
        if value == self.admitted {
            return Ok(UiScaleReceipt::Unchanged {
                admitted_revision: self.revision,
            });
        }
        let request_id = self
            .last_request_id
            .checked_add(1)
            .filter(|_| self.revision.checked_add(1).is_some())
            .ok_or(UiScaleRejection::RequestIdentityExhausted)?;
        self.last_request_id = request_id;
        self.requested = Some(UiScaleRequestStatus {
            request_id,
            value,
            outcome: UiScaleRequestOutcome::Queued,
        });
        Ok(UiScaleReceipt::Queued { request_id })
    }

    pub(crate) fn pending(&self) -> Option<UiScale> {
        self.requested.and_then(|request| {
            (request.outcome == UiScaleRequestOutcome::Queued).then_some(request.value)
        })
    }

    pub(crate) fn reject_pending(&mut self, reason: UiScaleRejection) {
        if let Some(request) = self.requested.as_mut() {
            if request.outcome == UiScaleRequestOutcome::Queued {
                request.outcome = UiScaleRequestOutcome::Rejected(reason);
            }
        }
    }

    pub(crate) fn admit_pending(&mut self) -> bool {
        let Some(value) = self.pending() else {
            return false;
        };
        let revision = self
            .revision
            .checked_add(1)
            .expect("queued UI scale request reserved its next revision");
        self.admitted = value;
        self.revision = revision;
        self.requested.as_mut().unwrap().outcome = UiScaleRequestOutcome::Admitted { revision };
        for target in self.targets.values_mut() {
            if target.output == UiScaleTargetOutput::Submitted {
                target.output = UiScaleTargetOutput::Pending;
            }
        }
        true
    }

    pub(crate) fn admitted(&self) -> UiScale {
        self.admitted
    }

    pub(crate) fn status(&self, surface: Option<SurfaceId>) -> UiScaleStatus {
        UiScaleStatus {
            requested: self.requested,
            admitted: self.admitted,
            admitted_revision: self.revision,
            target: surface.map(|surface| {
                self.targets
                    .get(&surface)
                    .copied()
                    .unwrap_or(UiScaleTargetStatus {
                        surface,
                        effective: None,
                        output: UiScaleTargetOutput::Unavailable(
                            UiScaleTargetFailure::UnknownTarget,
                        ),
                    })
            }),
        }
    }

    pub(crate) fn set_target(
        &mut self,
        surface: SurfaceId,
        platform_density: f32,
        physical_size: (u32, u32),
        effective_scale_factor: f32,
        output: UiScaleTargetOutput,
    ) {
        let previous = self
            .targets
            .get(&surface)
            .and_then(|target| target.effective);
        let (metrics_revision, metrics_changed) = match previous {
            Some(previous)
                if previous.admitted_revision == self.revision
                    && previous.platform_density == platform_density
                    && previous.physical_size == physical_size
                    && previous.effective_scale_factor == effective_scale_factor =>
            {
                (previous.metrics_revision, false)
            }
            _ => {
                self.last_metrics_revision = self
                    .last_metrics_revision
                    .checked_add(1)
                    .expect("App UI scale target metrics identity exhausted");
                (self.last_metrics_revision, true)
            }
        };
        self.targets.insert(
            surface,
            UiScaleTargetStatus {
                surface,
                effective: Some(UiScaleEffectiveSnapshot {
                    admitted_revision: self.revision,
                    metrics_revision,
                    surface,
                    platform_density,
                    physical_size,
                    effective_scale_factor,
                }),
                output: if metrics_changed && output == UiScaleTargetOutput::Submitted {
                    UiScaleTargetOutput::Pending
                } else {
                    output
                },
            },
        );
    }

    pub(crate) fn remove_target(&mut self, surface: SurfaceId) {
        self.targets.remove(&surface);
    }

    pub(crate) fn set_output(&mut self, surface: SurfaceId, output: UiScaleTargetOutput) {
        if let Some(target) = self.targets.get_mut(&surface) {
            target.output = output;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::*;
    use super::*;
    use crate::core::event::{
        CancellationReason, Event, EventPhase, GestureKind, InteractionDomains,
        KeyboardAuthorityMode, PointerButton, PointerDevice, PointerEventKind, PointerId,
        RawButtonState, RawInputEvent, WindowId,
    };
    use crate::core::layout::LayoutResult;
    use crate::core::widget::{
        AbsoluteDragGeometryDependency, EventContext, LayoutContext, PaintContext, Widget,
        WidgetDowncast,
    };
    use crate::platform::font_manager_text_engine::FontManagerTextEngine;
    use crate::renderer::pixel_coverage::PhysicalExtent;
    use crate::utils::id::WidgetInternalId;
    use crate::widgets::text_field::TextField;
    use std::cell::Cell;
    use std::rc::Rc;
    use std::time::{Duration, Instant};

    struct ScaleDragEditor {
        editor: TextField,
        dependency: Option<AbsoluteDragGeometryDependency>,
        down: bool,
        preview: bool,
        paints: Cell<u32>,
        painted_preview: Cell<bool>,
        capture_enters: u32,
        capture_leaves: u32,
        gesture_cancels: u32,
        pointer_cancels: u32,
        last_cancel: Option<CancellationReason>,
        product_commits: u32,
    }

    impl WidgetDowncast for ScaleDragEditor {
        fn as_any(&self) -> &dyn std::any::Any {
            self
        }
        fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
            self
        }
    }

    impl Widget for ScaleDragEditor {
        fn id(&self) -> WidgetInternalId {
            self.editor.id()
        }
        fn focusable(&self) -> bool {
            true
        }
        fn capture_eligible(&self) -> bool {
            true
        }
        fn layout(
            &mut self,
            cx: &mut LayoutContext,
            constraints: Constraints,
            children: &[NodeId],
        ) -> LayoutResult {
            self.editor.layout(cx, constraints, children)
        }
        fn paint(&self, cx: &mut PaintContext, rect: RectF, children: &[NodeId]) {
            self.paints.set(self.paints.get() + 1);
            self.painted_preview.set(self.preview);
            self.editor.paint(cx, rect, children);
            if self.preview {
                cx.scene.push_rect(
                    RectF::new(rect.x, rect.y, 8.0, 8.0),
                    crate::core::paint::Color::from_hex(0xFF00FF),
                    crate::core::paint::LAYER_OVERLAY,
                    None,
                );
            }
        }
        fn gesture_surface(&self) -> &'static [GestureKind] {
            &[GestureKind::Drag]
        }
        fn absolute_drag_geometry_dependency(&self) -> Option<AbsoluteDragGeometryDependency> {
            self.dependency
        }
        fn handle_event(
            &mut self,
            cx: &mut EventContext,
            event: &Event,
            phase: EventPhase,
        ) -> bool {
            if phase == EventPhase::Target {
                if let Event::Pointer(pointer) = event {
                    match pointer.kind {
                        PointerEventKind::Down
                            if pointer.button == Some(PointerButton::Primary) =>
                        {
                            self.down = true;
                            return true;
                        }
                        PointerEventKind::DragStart if self.down => {
                            cx.request_gesture_commit();
                            return true;
                        }
                        PointerEventKind::Move if self.preview => return true,
                        PointerEventKind::Up if pointer.button == Some(PointerButton::Primary) => {
                            self.down = false;
                            if self.preview {
                                self.preview = false;
                                self.product_commits += 1;
                                cx.invalidate_paint();
                            }
                            return true;
                        }
                        _ => {}
                    }
                }
            }
            self.editor.handle_event(cx, event, phase)
        }
        fn on_gesture_commit(&mut self, cx: &mut EventContext, kind: GestureKind) {
            assert_eq!(kind, GestureKind::Drag);
            self.preview = true;
            cx.invalidate_paint();
        }
        fn on_gesture_cancel(&mut self, cx: &mut EventContext, kind: GestureKind) {
            assert_eq!(kind, GestureKind::Drag);
            self.gesture_cancels += 1;
            self.down = false;
            self.preview = false;
            cx.invalidate_paint();
        }
        fn on_pointer_capture_cancel(
            &mut self,
            cx: &mut EventContext,
            _: PointerId,
            reason: CancellationReason,
        ) {
            self.pointer_cancels += 1;
            self.last_cancel = Some(reason);
            self.down = false;
            self.preview = false;
            cx.invalidate_paint();
        }
        fn on_focus(&mut self, cx: &mut EventContext) {
            self.editor.on_focus(cx);
        }
        fn on_blur(&mut self, cx: &mut EventContext) {
            self.editor.on_blur(cx);
        }
        fn on_capture_enter(&mut self, cx: &mut EventContext) {
            self.capture_enters += 1;
            self.editor.on_capture_enter(cx);
        }
        fn on_capture_leave(&mut self, cx: &mut EventContext) {
            self.capture_leaves += 1;
            self.editor.on_capture_leave(cx);
        }
    }

    struct ScaleIntegration {
        context: AppContext,
        tree: WidgetTree,
        root: NodeId,
        domains: InteractionDomains,
        router: EventRouter,
        fusion: InputFusion,
        text: FontManagerTextEngine,
        harness: RetainedSceneTestHarness,
        ticker: EngineTicker,
        time: Instant,
        editor_callbacks: Rc<Cell<(u32, u32)>>,
    }

    impl ScaleIntegration {
        fn dragging_editor() -> Self {
            let editor_callbacks = Rc::new(Cell::new((0, 0)));
            let mut editor = TextField::new().with_text("retained editor value");
            let submitted = editor_callbacks.clone();
            editor.set_on_submit(move |_| {
                let (submit, cancel) = submitted.get();
                submitted.set((submit + 1, cancel));
            });
            let cancelled = editor_callbacks.clone();
            editor.set_on_cancel(move || {
                let (submit, cancel) = cancelled.get();
                cancelled.set((submit, cancel + 1));
            });
            let mut tree = WidgetTree::new();
            let root = tree.create_root(
                Box::new(ScaleDragEditor {
                    editor,
                    dependency: None,
                    down: false,
                    preview: false,
                    paints: Cell::new(0),
                    painted_preview: Cell::new(false),
                    capture_enters: 0,
                    capture_leaves: 0,
                    gesture_cancels: 0,
                    pointer_cancels: 0,
                    last_cancel: None,
                    product_commits: 0,
                }),
                None,
                0,
            );
            tree.node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScaleDragEditor>()
                .unwrap()
                .dependency = Some(AbsoluteDragGeometryDependency {
                viewport_owner: root,
                reflow_root: root,
            });
            let mut context = AppContext::new(Theme::default());
            context.ui_scale_runtime = true;
            let time = Instant::now();
            let mut fixture = Self {
                context,
                tree,
                root,
                domains: InteractionDomains::new(),
                router: EventRouter::new(),
                fusion: InputFusion::new(),
                text: FontManagerTextEngine::new(),
                harness: RetainedSceneTestHarness::new(),
                ticker: EngineTicker::new(time),
                time,
                editor_callbacks,
            };
            assert!(fixture.frame(1.0).scene_built);
            fixture.button(true);
            for x in [24.0, 56.0] {
                fixture.time += Duration::from_millis(16);
                fixture.raw(RawInputEvent::PointerMoved {
                    window_id: WindowId::PRIMARY,
                    time: fixture.time,
                    device: PointerDevice::Mouse,
                    position: Point::new(x, 8.0),
                    delta: (16.0, 0.0),
                });
            }
            let owner = fixture.tree.widget_id_for_node(root).unwrap();
            assert_eq!(
                fixture
                    .domains
                    .primary_state()
                    .pointer_capture_manager
                    .current_capture(PointerId::MOUSE),
                Some(owner)
            );
            fixture.router.request_focus_for_widget(
                &mut fixture.tree,
                owner,
                fixture.domains.primary_state_mut(),
            );
            fixture.router.enter_capture_with_text_engine(
                &mut fixture.tree,
                owner,
                root,
                fixture.domains.primary_state_mut(),
                Some(&mut fixture.text),
            );
            fixture
                .tree
                .node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<ScaleDragEditor>()
                .unwrap()
                .editor
                .select_all();
            let preview = fixture.frame(1.0);
            assert!(preview.scene_built);
            assert!(scene_has_drag_preview(&preview.scene_root));
            assert!(fixture.editor().painted_preview.get());
            fixture.context.ui_scale.set_target(
                SurfaceId::test_id(41),
                1.0,
                (800, 400),
                1.0,
                UiScaleTargetOutput::Submitted,
            );
            fixture
        }

        fn editor(&self) -> &ScaleDragEditor {
            self.tree
                .node(self.root)
                .unwrap()
                .widget
                .as_any()
                .downcast_ref()
                .unwrap()
        }

        fn editor_value(&self) -> (String, usize, usize, usize) {
            let editor = &self.editor().editor;
            (
                editor.text().to_string(),
                editor.cursor_pos(),
                editor.selection().anchor,
                editor.selection().head,
            )
        }

        fn raw(&mut self, raw: RawInputEvent) {
            let mut events = Vec::new();
            self.fusion.process_raw_with_gesture_sessions(
                raw,
                crate::core::event::InteractionDomainId::PRIMARY,
                &mut self.domains.primary_state_mut().gesture_registry,
                &self.tree,
                None,
                &mut events,
            );
            self.fusion.flush_with_gesture_sessions(&mut events);
            for event in events {
                self.router.route_session_input_event(
                    &mut self.tree,
                    event,
                    Some(self.root),
                    &mut self.text,
                    None,
                    self.domains.primary_state_mut(),
                    &mut self.context.registry.binding_registry,
                );
            }
        }

        fn button(&mut self, down: bool) {
            self.time += Duration::from_secs(1);
            self.raw(RawInputEvent::PointerButton {
                window_id: WindowId::PRIMARY,
                time: self.time,
                device: PointerDevice::Mouse,
                button: PointerButton::Primary,
                state: if down {
                    RawButtonState::Pressed
                } else {
                    RawButtonState::Released
                },
                position: Point::new(4.0, 8.0),
            });
        }

        fn frame(&mut self, scale: f32) -> RetainedSceneTestFrame {
            let viewport = crate::utils::coords::checked_logical_viewport(800, 400, scale).unwrap();
            let constraints = Constraints::loose(viewport.size());
            self.tree
                .observe_geometry_allocation(self.root, constraints, scale);
            if self
                .domains
                .primary_state()
                .gesture_registry
                .has_session(PointerId::MOUSE)
            {
                assert_eq!(scale, 1.0);
                self.time += Duration::from_millis(16);
                let frame_time = self.ticker.begin_frame_at(self.time);
                let mut events = Vec::new();
                self.fusion.process_frame_with_gesture_sessions(
                    &frame_time,
                    &mut self.ticker,
                    &mut events,
                );
                for event in events {
                    self.router.route_session_input_event(
                        &mut self.tree,
                        event,
                        Some(self.root),
                        &mut self.text,
                        None,
                        self.domains.primary_state_mut(),
                        &mut self.context.registry.binding_registry,
                    );
                }
                return self.harness.frame_with_gesture_sessions(
                    &mut self.tree,
                    self.root,
                    constraints,
                    &Theme::default(),
                    &mut self.text,
                    &crate::core::icon::IconRegistry::new(&[]),
                    &frame_time,
                    None,
                    &mut self.domains,
                    WindowId::PRIMARY,
                    &mut self.router,
                    &mut self.fusion,
                );
            }
            self.harness.frame_with_optional_custom_render_target(
                &mut self.tree,
                self.root,
                constraints,
                &Theme::default(),
                &mut self.text,
                &crate::core::icon::IconRegistry::new(&[]),
                Some(
                    CustomRenderTargetSnapshot::new(
                        SurfaceId::test_id(41),
                        PhysicalExtent::new(800, 400),
                        scale,
                    )
                    .unwrap(),
                ),
                Some((&self.domains, WindowId::PRIMARY)),
            )
        }

        fn settle(&mut self) -> Option<Vec<(SurfaceId, f32, RectF)>> {
            settle_ui_scale_for_frame(
                &mut self.context,
                &mut self.tree,
                &mut self.router,
                self.domains.primary_state_mut(),
                &mut self.fusion,
                self.root,
                self.text.scale(),
                &[(SurfaceId::test_id(41), 1.0, (800, 400))],
                false,
            )
            .unwrap()
        }
    }

    fn scene_has_drag_preview(nodes: &[crate::core::scene::SceneNode]) -> bool {
        use crate::core::scene::SceneNode;
        nodes.iter().any(|node| match node {
            SceneNode::Rect(rect) => rect.color == crate::core::scene::Color::from_hex(0xFF00FF),
            SceneNode::RetainedFragment(fragment) => scene_has_drag_preview(&fragment.children),
            SceneNode::Clip(clip) => scene_has_drag_preview(&clip.children),
            SceneNode::Layer(layer) => scene_has_drag_preview(&layer.children),
            SceneNode::Scroll(scroll) => scene_has_drag_preview(&scroll.children),
            _ => false,
        })
    }

    #[test]
    fn ui_scale_settlement_retires_geometry_drag_without_committing_editor() {
        let mut fixture = ScaleIntegration::dragging_editor();
        let captures = (
            fixture.editor().capture_enters,
            fixture.editor().capture_leaves,
        );
        let owner = fixture.tree.widget_id_for_node(fixture.root).unwrap();
        let editor_value = fixture.editor_value();
        let paints = fixture.editor().paints.get();
        fixture
            .context
            .request_ui_scale(UiScale::try_from(2.0).unwrap())
            .unwrap();
        let settled = fixture.settle().unwrap();
        assert_eq!(
            settled,
            vec![(
                SurfaceId::test_id(41),
                2.0,
                RectF::new(0.0, 0.0, 400.0, 200.0)
            )]
        );
        assert_eq!(
            fixture
                .context
                .ui_scale_status(None)
                .requested
                .unwrap()
                .outcome,
            UiScaleRequestOutcome::Admitted { revision: 1 }
        );
        assert_eq!(
            fixture.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert_eq!(
            fixture
                .domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            None
        );
        assert!(
            !fixture
                .domains
                .primary_state()
                .gesture_registry
                .has_session(PointerId::MOUSE)
        );
        assert_eq!(
            fixture.editor().last_cancel,
            Some(CancellationReason::SystemCancel)
        );
        assert_eq!(
            (
                fixture.editor().gesture_cancels,
                fixture.editor().pointer_cancels
            ),
            (1, 1)
        );
        let successor = fixture.frame(settled[0].1);
        assert!(successor.scene_built);
        assert!(!scene_has_drag_preview(&successor.scene_root));
        assert_eq!(
            fixture
                .tree
                .get_absolute_paint_rect(fixture.root)
                .unwrap()
                .width,
            400.0
        );
        assert!(fixture.editor().paints.get() > paints);
        assert!(!fixture.editor().painted_preview.get());
        assert_eq!(fixture.tree.widget_id_for_node(fixture.root), Some(owner));
        assert_eq!(fixture.editor_value(), editor_value);
        assert_eq!(
            (
                fixture.editor().capture_enters,
                fixture.editor().capture_leaves
            ),
            captures
        );
        fixture.button(false);
        assert_eq!(fixture.editor().product_commits, 0);
        assert_eq!(fixture.editor_callbacks.get(), (0, 0));
    }

    #[test]
    fn ui_scale_invalid_metrics_reject_before_tree_or_capture_mutation() {
        let mut fixture = ScaleIntegration::dragging_editor();
        let captures = (
            fixture.editor().capture_enters,
            fixture.editor().capture_leaves,
        );
        let owner = fixture.tree.widget_id_for_node(fixture.root).unwrap();
        let before = fixture
            .context
            .ui_scale_status(Some(SurfaceId::test_id(41)));
        let rect = fixture.tree.get_absolute_paint_rect(fixture.root).unwrap();
        let editor_value = fixture.editor_value();
        let paints = fixture.editor().paints.get();
        assert!(!fixture.tree.node(fixture.root).unwrap().dirty_layout);
        assert!(!fixture.tree.node(fixture.root).unwrap().dirty_paint);
        fixture
            .context
            .request_ui_scale(UiScale::try_from(f32::MIN_POSITIVE).unwrap())
            .unwrap();
        assert!(fixture.settle().is_none());
        let after = fixture
            .context
            .ui_scale_status(Some(SurfaceId::test_id(41)));
        assert_eq!(
            (after.admitted, after.admitted_revision, after.target),
            (before.admitted, before.admitted_revision, before.target)
        );
        assert_eq!(
            after.requested.unwrap().outcome,
            UiScaleRequestOutcome::Rejected(UiScaleRejection::InvalidTargetMetrics {
                surface: SurfaceId::test_id(41)
            })
        );
        assert_eq!(
            fixture.tree.get_absolute_paint_rect(fixture.root).unwrap(),
            rect
        );
        assert_eq!(fixture.tree.widget_id_for_node(fixture.root), Some(owner));
        assert!(!fixture.tree.node(fixture.root).unwrap().dirty_layout);
        assert!(!fixture.tree.node(fixture.root).unwrap().dirty_paint);
        assert_eq!(
            fixture.domains.primary_state().keyboard_mode,
            KeyboardAuthorityMode::Capture
        );
        assert_eq!(
            fixture.domains.primary_state().focus_manager.focused(),
            Some(owner)
        );
        assert_eq!(
            fixture
                .domains
                .primary_state()
                .pointer_capture_manager
                .current_capture(PointerId::MOUSE),
            Some(owner)
        );
        assert!(
            fixture
                .domains
                .primary_state()
                .gesture_registry
                .has_session(PointerId::MOUSE)
        );
        assert!(fixture.editor().preview);
        assert_eq!(
            (
                fixture.editor().gesture_cancels,
                fixture.editor().pointer_cancels
            ),
            (0, 0)
        );
        assert_eq!(
            (
                fixture.editor().capture_enters,
                fixture.editor().capture_leaves
            ),
            captures
        );
        assert_eq!(fixture.editor().paints.get(), paints);
        assert_eq!(fixture.editor_value(), editor_value);
        assert_eq!(fixture.editor_callbacks.get(), (0, 0));
    }

    #[test]
    fn ui_scale_value_is_checked_before_state_mutation() {
        assert_eq!(UiScale::default(), UiScale::ONE);
        assert_eq!(UiScale::ONE.get(), 1.0);
        assert_eq!(
            UiScale::try_from(f32::NAN),
            Err(UiScaleValueError::NonFinite)
        );
        assert_eq!(UiScale::try_from(0.0), Err(UiScaleValueError::NonPositive));
        assert_eq!(UiScale::try_from(1.25).unwrap().get(), 1.25);
        let mut unsupported = AppContext::new(Theme::default());
        assert_eq!(
            unsupported.request_ui_scale(UiScale::ONE),
            Ok(UiScaleReceipt::Unchanged {
                admitted_revision: 0
            })
        );
        assert_eq!(
            unsupported.request_ui_scale(UiScale::try_from(1.25).unwrap()),
            Err(UiScaleRejection::UnsupportedRuntime)
        );
        assert_eq!(unsupported.ui_scale_status(None).requested, None);
    }

    #[test]
    fn ui_scale_request_is_bounded_and_settles_once() {
        let mut state = UiScaleState::initial(UiScale::ONE);
        let value = UiScale::try_from(1.25).unwrap();
        assert_eq!(
            state.request(value),
            Ok(UiScaleReceipt::Queued { request_id: 1 })
        );
        assert_eq!(state.admitted(), UiScale::ONE);
        assert_eq!(
            state.request(UiScale::ONE),
            Err(UiScaleRejection::PendingRequest)
        );
        assert_eq!(state.pending(), Some(value));
        assert!(state.admit_pending());
        assert!(!state.admit_pending());
        assert_eq!(state.admitted(), value);
        assert_eq!(state.status(None).admitted_revision, 1);
        assert_eq!(
            state.status(None).requested.unwrap().outcome,
            UiScaleRequestOutcome::Admitted { revision: 1 }
        );
        let unchanged = state.status(None);
        assert_eq!(
            state.request(value),
            Ok(UiScaleReceipt::Unchanged {
                admitted_revision: 1
            })
        );
        assert_eq!(state.status(None), unchanged);
    }

    #[test]
    fn ui_scale_deferred_rejection_preserves_admitted_and_effective_state() {
        let mut state = UiScaleState::initial(UiScale::ONE);
        let surface = SurfaceId::test_id(1);
        state.set_target(
            surface,
            1.5,
            (600, 300),
            1.5,
            UiScaleTargetOutput::Submitted,
        );
        let before = state.status(Some(surface));
        let value = UiScale::try_from(1.25).unwrap();
        state.request(value).unwrap();
        let reason = UiScaleRejection::InvalidTargetMetrics { surface };
        state.reject_pending(reason);
        assert_eq!(state.pending(), None);
        assert!(!state.admit_pending());
        let rejected = state.status(Some(surface));
        assert_eq!(rejected.admitted, before.admitted);
        assert_eq!(rejected.admitted_revision, before.admitted_revision);
        assert_eq!(rejected.target, before.target);
        assert_eq!(
            rejected.requested.unwrap().outcome,
            UiScaleRequestOutcome::Rejected(reason)
        );
        assert_eq!(
            state.request(value),
            Ok(UiScaleReceipt::Queued { request_id: 2 })
        );
    }

    #[test]
    fn ui_scale_apps_and_target_publications_remain_independent() {
        let mut first = UiScaleState::initial(UiScale::ONE);
        let second = UiScaleState::initial(UiScale::try_from(0.75).unwrap());
        let surface = SurfaceId::test_id(1);
        first.set_target(
            surface,
            1.5,
            (600, 300),
            1.5,
            UiScaleTargetOutput::Submitted,
        );
        first.request(UiScale::try_from(1.25).unwrap()).unwrap();
        first.admit_pending();
        let waiting = first.status(Some(surface)).target.unwrap();
        assert_eq!(waiting.output, UiScaleTargetOutput::Pending);
        assert_eq!(waiting.effective.unwrap().admitted_revision, 0);
        first.set_target(
            surface,
            1.5,
            (600, 300),
            1.875,
            UiScaleTargetOutput::Pending,
        );
        let published = first
            .status(Some(surface))
            .target
            .unwrap()
            .effective
            .unwrap();
        first.set_target(
            surface,
            1.5,
            (600, 300),
            1.875,
            UiScaleTargetOutput::Pending,
        );
        assert_eq!(
            first
                .status(Some(surface))
                .target
                .unwrap()
                .effective
                .unwrap()
                .metrics_revision,
            published.metrics_revision
        );
        first.set_output(surface, UiScaleTargetOutput::Submitted);
        assert_eq!(
            first
                .status(Some(surface))
                .target
                .unwrap()
                .effective
                .unwrap()
                .admitted_revision,
            1
        );
        first.set_target(
            surface,
            1.5,
            (600, 300),
            1.875,
            UiScaleTargetOutput::Submitted,
        );
        assert_eq!(
            first.status(Some(surface)).target.unwrap().output,
            UiScaleTargetOutput::Submitted
        );
        first.set_target(
            surface,
            1.5,
            (900, 300),
            1.875,
            UiScaleTargetOutput::Submitted,
        );
        let resized = first.status(Some(surface)).target.unwrap();
        assert_eq!(resized.output, UiScaleTargetOutput::Pending);
        assert!(resized.effective.unwrap().metrics_revision > published.metrics_revision);
        assert_eq!(second.admitted().get(), 0.75);
        assert_eq!(
            second.status(Some(surface)).target.unwrap().output,
            UiScaleTargetOutput::Unavailable(UiScaleTargetFailure::UnknownTarget)
        );
        first.remove_target(surface);
        assert_eq!(first.status(Some(surface)).target.unwrap().effective, None);
    }
}
