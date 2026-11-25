//! Standalone Cartesian spatial viewport capability example.
//!
//! Controls:
//! - Hold Space and primary-drag, or middle-drag, to pan.
//! - Scroll to zoom around the cursor.
//! - Resize the window to verify equal X/Y scale remains intact.
//! - Close the window to exit.

use std::cell::Cell;
use std::rc::Rc;

use halcyon::core::animation::{Animation, Easing, TransitionHandle};
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::{
    CursorIcon, Event, EventPhase, FusedInputEvent, KeyCode, PointerButton, PointerEvent,
    PointerEventKind,
};
use halcyon::core::layout::{Constraints, LayoutResult};
use halcyon::core::math::{Point, RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::NodeId;
use halcyon::model::canvas::WorldPoint;
use halcyon::model::spatial::{
    SpatialAxisOrientation, SpatialDirection, SpatialGestureFacts, SpatialGridLineRole,
    SpatialGridPolicy, SpatialInteractionPolicy, SpatialTransform, SpatialViewportError,
    SpatialViewportState, SpatialVisualPolicy, SpatialWheelZoomPolicy, realize_spatial_grid,
};
use halcyon::prelude::Label;
use halcyon::utils::id::WidgetInternalId;

const WINDOW_WIDTH: f32 = 960.0;
const WINDOW_HEIGHT: f32 = 720.0;
const WORLD_CIRCLE_RADIUS: f64 = 96.0;
const ZOOM_ANIMATION_DURATION: f32 = 0.025;
const ZOOM_IN_FACTOR: f64 = 1.15;
const ZOOM_OUT_FACTOR: f64 = 0.87;
const CARTESIAN_ORIENTATION: SpatialAxisOrientation = SpatialAxisOrientation::new(
    SpatialDirection::IncreasingPixels,
    SpatialDirection::DecreasingPixels,
);

fn spatial_interaction_policy() -> SpatialInteractionPolicy {
    SpatialInteractionPolicy {
        wheel_zoom: SpatialWheelZoomPolicy::Unmodified,
        ..SpatialInteractionPolicy::default()
    }
}

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Spatial Viewport ===");
    println!("Cartesian affine surface: +X right, +Y up");
    println!("Pan: Space + primary drag, or middle drag");
    println!("Zoom: wheel at cursor");
    println!("Resize the window; the circle must remain circular");
    println!("Close the window to exit");
    println!("========================");

    App::new(AppConfig::new("Spatial Viewport").with_size(WINDOW_WIDTH, WINDOW_HEIGHT))
        .run(SpatialViewportScreen::new());
}

struct SpatialViewportScreen {
    placeholder: Label,
    state: Rc<Cell<SpatialViewportDemoState>>,
    space_pressed: Rc<Cell<bool>>,
    needs_rebuild: bool,
}

impl SpatialViewportScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            state: Rc::new(Cell::new(SpatialViewportDemoState::default())),
            space_pressed: Rc::new(Cell::new(false)),
            needs_rebuild: true,
        }
    }
}

impl Screen for SpatialViewportScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        cx.mount_child(Box::new(SpatialViewportSurface::new(
            self.state.clone(),
            self.space_pressed.clone(),
        )));
        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        match event {
            FusedInputEvent::KeyDown { key_code, .. } if *key_code == KeyCode::Space => {
                self.space_pressed.set(true);
                true
            }
            FusedInputEvent::KeyUp { key_code, .. } if *key_code == KeyCode::Space => {
                self.space_pressed.set(false);
                true
            }
            _ => false,
        }
    }

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::replace(&mut self.needs_rebuild, false)
    }
}

#[derive(Debug, Clone, Copy)]
struct PanState {
    button: PointerButton,
    last_pixel: Point,
}

#[derive(Debug, Clone, Copy)]
struct ZoomAnimationState {
    handle: TransitionHandle,
    base_viewport: SpatialViewportState,
    target_viewport: SpatialViewportState,
    anchor_pixel: Point,
}

#[derive(Debug, Clone, Copy)]
struct SpatialViewportDemoState {
    viewport: SpatialViewportState,
    pan: Option<PanState>,
    cursor_pixel: Option<Point>,
    last_error: Option<SpatialViewportError>,
}

impl Default for SpatialViewportDemoState {
    fn default() -> Self {
        Self {
            viewport: SpatialViewportState::default(),
            pan: None,
            cursor_pixel: None,
            last_error: None,
        }
    }
}

struct SpatialViewportSurface {
    id: WidgetInternalId,
    state: Rc<Cell<SpatialViewportDemoState>>,
    space_pressed: Rc<Cell<bool>>,
    zoom_animation: Cell<Option<ZoomAnimationState>>,
}

impl SpatialViewportSurface {
    fn new(state: Rc<Cell<SpatialViewportDemoState>>, space_pressed: Rc<Cell<bool>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            state,
            space_pressed,
            zoom_animation: Cell::new(None),
        }
    }

    fn transform(&self, rect: RectF) -> Result<SpatialTransform, SpatialViewportError> {
        SpatialTransform::new(self.state.get().viewport, CARTESIAN_ORIENTATION, rect)
    }

    fn replace_viewport(&self, result: Result<SpatialViewportState, SpatialViewportError>) {
        let mut state = self.state.get();
        match result {
            Ok(viewport) => {
                state.viewport = viewport;
                state.last_error = None;
            }
            Err(error) => state.last_error = Some(error),
        }
        self.state.set(state);
    }

    fn record_error(&self, error: SpatialViewportError) {
        let mut state = self.state.get();
        state.last_error = Some(error);
        self.state.set(state);
    }

    fn cancel_zoom_animation(&self, cx: &mut EventContext) {
        if let Some(animation) = self.zoom_animation.replace(None) {
            cx.cancel_animation(animation.handle);
        }
    }

    fn advance_zoom_animation(&self, cx: &PaintContext, rect: RectF) {
        let Some(animation) = self.zoom_animation.get() else {
            return;
        };

        if let Some(factor) = cx.get_animation::<f64>(animation.handle) {
            let next = SpatialTransform::new(animation.base_viewport, CARTESIAN_ORIENTATION, rect)
                .and_then(|transform| {
                    transform.anchored_zoom_at_pixel(factor, animation.anchor_pixel)
                });
            self.replace_viewport(next);
        } else {
            self.zoom_animation.set(None);
            self.replace_viewport(Ok(animation.target_viewport));
        }
    }

    fn begin_pan(&mut self, cx: &mut EventContext, pointer: &PointerEvent, pixel: Point) -> bool {
        let Some(button) = pointer.button else {
            return false;
        };
        let policy = spatial_interaction_policy();
        let gesture = policy.gesture_class(SpatialGestureFacts {
            modifiers: pointer.modifiers,
            button,
            space_pressed: self.space_pressed.get(),
        });
        if !policy.is_pan_gesture(gesture) {
            return false;
        }

        self.cancel_zoom_animation(cx);
        let mut state = self.state.get();
        state.pan = Some(PanState {
            button,
            last_pixel: pixel,
        });
        self.state.set(state);
        cx.capture_pointer(button);
        cx.request_focus();
        cx.request_cursor(CursorIcon::Move);
        cx.invalidate_paint();
        true
    }

    fn update_pan(&mut self, cx: &mut EventContext, rect: RectF, pixel: Point) -> bool {
        let Some(mut pan) = self.state.get().pan else {
            return false;
        };
        let transform = match self.transform(rect) {
            Ok(transform) => transform,
            Err(error) => {
                self.record_error(error);
                return true;
            }
        };
        let previous_world = match transform.screen_to_world(pan.last_pixel.x, pan.last_pixel.y) {
            Ok(world) => world,
            Err(error) => {
                self.record_error(error);
                return true;
            }
        };
        let current_world = match transform.screen_to_world(pixel.x, pixel.y) {
            Ok(world) => world,
            Err(error) => {
                self.record_error(error);
                return true;
            }
        };

        pan.last_pixel = pixel;
        let mut state = self.state.get();
        state.pan = Some(pan);
        self.state.set(state);
        let delta = WorldPoint::new(
            previous_world.x - current_world.x,
            previous_world.y - current_world.y,
        );
        let next = self.state.get().viewport.pan(delta);
        self.replace_viewport(next);
        cx.request_cursor(CursorIcon::Move);
        cx.invalidate_paint();
        true
    }

    fn zoom_at_cursor(
        &mut self,
        cx: &mut EventContext,
        rect: RectF,
        pixel: Point,
        delta_y: f32,
    ) -> bool {
        if delta_y == 0.0 {
            return false;
        }

        let step_factor = if delta_y > 0.0 {
            ZOOM_IN_FACTOR
        } else {
            ZOOM_OUT_FACTOR
        };
        let current_viewport = self.state.get().viewport;
        let pending_scale = self
            .zoom_animation
            .get()
            .map_or(current_viewport.pixels_per_world_unit(), |animation| {
                animation.target_viewport.pixels_per_world_unit()
            });
        let factor = pending_scale * step_factor / current_viewport.pixels_per_world_unit();
        let target_viewport = match self
            .transform(rect)
            .and_then(|transform| transform.anchored_zoom_at_pixel(factor, pixel))
        {
            Ok(viewport) => viewport,
            Err(error) => {
                self.record_error(error);
                cx.invalidate_paint();
                return true;
            }
        };

        self.cancel_zoom_animation(cx);
        let animation = Animation::new(1.0_f64, factor, ZOOM_ANIMATION_DURATION, Easing::EaseOut);
        if let Some(handle) = cx.start_animation(animation) {
            self.zoom_animation.set(Some(ZoomAnimationState {
                handle,
                base_viewport: current_viewport,
                target_viewport,
                anchor_pixel: pixel,
            }));
        } else {
            self.replace_viewport(Ok(target_viewport));
        }
        cx.invalidate_paint();
        true
    }

    fn handle_pointer_event(&mut self, cx: &mut EventContext, pointer: &PointerEvent) -> bool {
        let pixel = Point::new(pointer.global_pos.0, pointer.global_pos.1);
        let mut state = self.state.get();
        state.cursor_pixel = Some(pixel);
        self.state.set(state);
        let Some(rect) = cx.widget_global_rect() else {
            return false;
        };

        match pointer.kind {
            PointerEventKind::Down => self.begin_pan(cx, pointer, pixel),
            PointerEventKind::Move => {
                if self.state.get().pan.is_some() {
                    self.update_pan(cx, rect, pixel)
                } else {
                    cx.request_cursor(CursorIcon::Crosshair);
                    cx.invalidate_paint();
                    true
                }
            }
            PointerEventKind::Up => {
                let Some(pan) = self.state.get().pan else {
                    return false;
                };
                if pointer.button != Some(pan.button) {
                    return false;
                }
                let mut state = self.state.get();
                state.pan = None;
                self.state.set(state);
                cx.release_pointer();
                cx.request_cursor(CursorIcon::Crosshair);
                cx.invalidate_paint();
                true
            }
            PointerEventKind::Scroll { delta_y, .. } => {
                if !spatial_interaction_policy().wheel_zoom_requested(pointer.modifiers) {
                    return false;
                }
                self.zoom_at_cursor(cx, rect, pixel, delta_y)
            }
            PointerEventKind::DragStart => self.state.get().pan.is_some(),
            PointerEventKind::DoubleClick | PointerEventKind::TripleClick => false,
        }
    }

    fn paint_hud(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        transform: Option<SpatialTransform>,
        policy: &SpatialVisualPolicy,
        frame_error: Option<SpatialViewportError>,
    ) {
        let state = self.state.get();
        let cursor_text = match (transform, state.cursor_pixel) {
            (Some(transform), Some(pixel)) => match transform.screen_to_world(pixel.x, pixel.y) {
                Ok(world) => format!("Cursor world: x={:.3}  y={:.3}", world.x, world.y),
                Err(error) => format!("Cursor world unavailable: {error:?}"),
            },
            _ => "Cursor world: move the pointer over the surface".to_string(),
        };
        let viewport = state.viewport;
        let camera_text = format!(
            "Cartesian: +X right / +Y up    Center: ({:.2}, {:.2})    Scale: {:.4} px/unit",
            viewport.world_center().x,
            viewport.world_center().y,
            viewport.pixels_per_world_unit(),
        );
        let controls_text = "Pan: Space + primary drag or middle drag    Zoom: wheel    Close the window to exit";
        let error = frame_error.or(state.last_error);
        let error_text = error.map(|error| format!("Viewport update rejected: {error:?}"));

        let edge = policy.hud_edge_padding;
        let inner = 10.0;
        let font_size = 13.0;
        let line_height = 18.0;
        let line_count = if error_text.is_some() { 4.0 } else { 3.0 };
        let hud = RectF::new(
            rect.x + edge,
            rect.y + edge,
            (rect.width - edge * 2.0).max(1.0),
            line_height * line_count + inner * 2.0,
        );
        cx.scene
            .push_rect(hud, policy.hud_background, 20, cx.scene_key);

        let text_x = hud.x + inner;
        let mut text_y = hud.y + inner;
        paint_text(cx, &camera_text, text_x, text_y, font_size, policy.hud_text);
        text_y += line_height;
        paint_text(cx, &cursor_text, text_x, text_y, font_size, policy.hud_text);
        text_y += line_height;
        paint_text(
            cx,
            controls_text,
            text_x,
            text_y,
            font_size,
            policy.hud_secondary_text,
        );
        if let Some(error_text) = error_text {
            text_y += line_height;
            paint_text(
                cx,
                &error_text,
                text_x,
                text_y,
                font_size,
                Color::from_srgb(1.0, 0.35, 0.35, 1.0),
            );
        }
    }
}

impl WidgetDowncast for SpatialViewportSurface {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for SpatialViewportSurface {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let width = if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            WINDOW_WIDTH
        };
        let height = if constraints.max_height.is_finite() {
            constraints.max_height
        } else {
            WINDOW_HEIGHT
        };
        LayoutResult::fit(constraints.constrain(Size::new(width, height)))
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        {
            let _allow = halcyon::core::rect_authority::AllowPaintMutationGuard::enter();
            self.advance_zoom_animation(cx, rect);
        }

        let policy = SpatialVisualPolicy::for_theme(cx.theme.is_dark());
        cx.scene
            .push_rect(rect, policy.canvas_background, 0, cx.scene_key);

        let transform = match self.transform(rect) {
            Ok(transform) => transform,
            Err(error) => {
                self.paint_hud(cx, rect, None, &policy, Some(error));
                return;
            }
        };
        let mut frame_error = None;

        match realize_spatial_grid(transform, SpatialGridPolicy::default()) {
            Ok(lines) => {
                for line in lines {
                    let visual = match line.role {
                        SpatialGridLineRole::Minor => policy.grid_minor,
                        SpatialGridLineRole::Major => policy.grid_major,
                    };
                    cx.scene
                        .push_rect(line.rect, visual.stroke, 1, cx.scene_key);
                }
            }
            Err(error) => frame_error = Some(error),
        }

        let circle_center = transform.world_to_screen(WorldPoint::new(0.0, 0.0));
        let circle_edge = transform.world_to_screen(WorldPoint::new(WORLD_CIRCLE_RADIUS, 0.0));
        match (circle_center, circle_edge) {
            (Ok((center_x, center_y)), Ok((edge_x, _))) => {
                cx.scene.push_circle(
                    Point::new(center_x, center_y),
                    (edge_x - center_x).abs(),
                    Color::from_srgb(0.18, 0.52, 0.96, 0.28),
                    2,
                    cx.scene_key,
                );
            }
            (Err(error), _) | (_, Err(error)) => frame_error = Some(error),
        }

        match transform.world_to_screen(WorldPoint::new(0.0, 0.0)) {
            Ok((origin_x, origin_y)) => {
                let axis = policy.active_guide;
                let half_stroke = axis.stroke_width * 0.5;
                if origin_y >= rect.y && origin_y <= rect.y + rect.height {
                    cx.scene.push_rect(
                        RectF::new(
                            rect.x,
                            origin_y - half_stroke,
                            rect.width,
                            axis.stroke_width,
                        ),
                        axis.stroke,
                        3,
                        cx.scene_key,
                    );
                }
                if origin_x >= rect.x && origin_x <= rect.x + rect.width {
                    cx.scene.push_rect(
                        RectF::new(
                            origin_x - half_stroke,
                            rect.y,
                            axis.stroke_width,
                            rect.height,
                        ),
                        axis.stroke,
                        3,
                        cx.scene_key,
                    );
                }
            }
            Err(error) => frame_error = Some(error),
        }

        self.paint_hud(cx, rect, Some(transform), &policy, frame_error);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if phase != EventPhase::Target {
            return false;
        }
        match event {
            Event::Pointer(pointer) => self.handle_pointer_event(cx, pointer),
            _ => false,
        }
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        true
    }

    fn focusable(&self) -> bool {
        true
    }

    fn wants_scroll(&self) -> bool {
        true
    }

    fn needs_animation(&self) -> bool {
        self.zoom_animation.get().is_some()
    }

    fn debug_type_name(&self) -> &'static str {
        "SpatialViewportSurface"
    }
}

fn paint_text(cx: &mut PaintContext, text: &str, x: f32, y: f32, font_size: f32, color: Color) {
    let run = cx.text_engine.shape(text, font_size);
    cx.text_engine.draw(
        &run,
        x,
        y,
        color,
        halcyon::core::paint::LAYER_CONTENT,
        cx.scene,
    );
}
