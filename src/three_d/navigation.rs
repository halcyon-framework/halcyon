//! Checked, renderer-independent camera presentation and interaction policy.
//!
//! Governing authority: SPEC-HAL-356.
//!
//! The consuming widget retains framework focus, capture, invalidation, and
//! scheduling authority. This module only transforms canonical input and
//! engine-supplied frame deltas into immutable camera view descriptions.

use glam::{DVec3, Quat, Vec3};
use thiserror::Error;

use crate::core::event::{KeyCode, KeyEvent, KeyboardAuthorityMode, PointerButton};

use super::math::{
    Math3DError, Orientation3, OrthographicProjection, PerspectiveProjection, Projection3,
    RigidPose3, ViewDescriptor3, WorldPoint3,
};

const MAX_ABSOLUTE_PITCH: f32 = std::f32::consts::FRAC_PI_2 - 0.0001;
const FLY_VELOCITY_STOP_RESIDUAL_UNITS_PER_SECOND: f64 = 0.001;

/// Rejection from checked camera state, policy, or input integration.
#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum CameraNavigationError {
    #[error("camera yaw must be finite")]
    InvalidYaw,
    #[error("camera pitch must be finite and remain inside the open vertical poles")]
    InvalidPitch,
    #[error("orbit distance must be finite and strictly positive")]
    InvalidOrbitDistance,
    #[error("camera navigation policy contains an invalid finite bound")]
    InvalidPolicy,
    #[error("fly key bindings must be pairwise distinct")]
    DuplicateFlyKeyBinding,
    #[error("camera rotate/look and pan buttons must be distinct within each mode")]
    DuplicatePointerBinding,
    #[error("pointer operation is not admitted by the current camera mode")]
    InvalidPointerOperation,
    #[error("pointer position or delta must be finite")]
    InvalidPointerInput,
    #[error("wheel anchor must lie at a finite positive depth in front of the camera")]
    InvalidWheelAnchor,
    #[error("frame delta must be finite and non-negative")]
    InvalidFrameDelta,
    #[error(transparent)]
    Math(#[from] Math3DError),
}

/// Active camera interaction policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraNavigationMode3 {
    Orbit,
    Fly,
}

/// One named pointer operation targeting the controller's `CameraState3`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraPointerOperation3 {
    OrbitRotate,
    FreeLook,
    CameraPan,
}

/// One named wheel operation selected only from checked camera projection.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CameraWheelOperation3 {
    PerspectiveOpticalFovZoom,
    OrthographicSpanZoom,
}

/// Configurable pointer bindings for named camera operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CameraPointerBindings3 {
    orbit_rotate: PointerButton,
    free_look: PointerButton,
    camera_pan: PointerButton,
}

impl CameraPointerBindings3 {
    pub fn try_new(
        orbit_rotate: PointerButton,
        free_look: PointerButton,
        camera_pan: PointerButton,
    ) -> Result<Self, CameraNavigationError> {
        if orbit_rotate == camera_pan || free_look == camera_pan {
            return Err(CameraNavigationError::DuplicatePointerBinding);
        }
        Ok(Self {
            orbit_rotate,
            free_look,
            camera_pan,
        })
    }

    pub const fn orbit_rotate(self) -> PointerButton {
        self.orbit_rotate
    }

    pub const fn free_look(self) -> PointerButton {
        self.free_look
    }

    pub const fn camera_pan(self) -> PointerButton {
        self.camera_pan
    }

    pub fn operation_for(
        self,
        mode: CameraNavigationMode3,
        button: PointerButton,
    ) -> Option<CameraPointerOperation3> {
        if button == self.camera_pan {
            Some(CameraPointerOperation3::CameraPan)
        } else {
            match mode {
                CameraNavigationMode3::Orbit if button == self.orbit_rotate => {
                    Some(CameraPointerOperation3::OrbitRotate)
                }
                CameraNavigationMode3::Fly if button == self.free_look => {
                    Some(CameraPointerOperation3::FreeLook)
                }
                CameraNavigationMode3::Orbit | CameraNavigationMode3::Fly => None,
            }
        }
    }
}

impl Default for CameraPointerBindings3 {
    fn default() -> Self {
        Self {
            orbit_rotate: PointerButton::Primary,
            free_look: PointerButton::Secondary,
            camera_pan: PointerButton::Auxiliary,
        }
    }
}

/// One named held-key Fly operation.
///
/// Motion and speed operations target checked camera state. `OrbitModifier`
/// only resolves the default Fly secondary-pointer operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlyMotionOperation3 {
    Forward,
    Backward,
    StrafeLeft,
    StrafeRight,
    OrbitModifier,
    SpeedModifier,
}

/// Configurable logical key bindings for named Fly motion and pointer-modifier operations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlyKeyBindings3 {
    forward: KeyCode,
    back: KeyCode,
    left: KeyCode,
    right: KeyCode,
    orbit_modifier: KeyCode,
    speed_modifier: KeyCode,
}

impl FlyKeyBindings3 {
    pub fn try_new(
        forward: KeyCode,
        back: KeyCode,
        left: KeyCode,
        right: KeyCode,
        orbit_modifier: KeyCode,
        speed_modifier: KeyCode,
    ) -> Result<Self, CameraNavigationError> {
        let bindings = [forward, back, left, right, orbit_modifier, speed_modifier];
        for (index, binding) in bindings.iter().enumerate() {
            if bindings[index + 1..].contains(binding) {
                return Err(CameraNavigationError::DuplicateFlyKeyBinding);
            }
        }
        Ok(Self {
            forward,
            back,
            left,
            right,
            orbit_modifier,
            speed_modifier,
        })
    }

    pub const fn forward(self) -> KeyCode {
        self.forward
    }

    pub const fn back(self) -> KeyCode {
        self.back
    }

    pub const fn left(self) -> KeyCode {
        self.left
    }

    pub const fn right(self) -> KeyCode {
        self.right
    }

    pub const fn orbit_modifier(self) -> KeyCode {
        self.orbit_modifier
    }

    pub const fn speed_modifier(self) -> KeyCode {
        self.speed_modifier
    }

    pub fn operation_for(self, key_code: KeyCode) -> Option<FlyMotionOperation3> {
        if key_code == self.forward {
            Some(FlyMotionOperation3::Forward)
        } else if key_code == self.back {
            Some(FlyMotionOperation3::Backward)
        } else if key_code == self.left {
            Some(FlyMotionOperation3::StrafeLeft)
        } else if key_code == self.right {
            Some(FlyMotionOperation3::StrafeRight)
        } else if key_code == self.orbit_modifier {
            Some(FlyMotionOperation3::OrbitModifier)
        } else if key_code == self.speed_modifier {
            Some(FlyMotionOperation3::SpeedModifier)
        } else {
            None
        }
    }
}

impl Default for FlyKeyBindings3 {
    fn default() -> Self {
        Self {
            forward: KeyCode::Character('w'),
            back: KeyCode::Character('s'),
            left: KeyCode::Character('a'),
            right: KeyCode::Character('d'),
            orbit_modifier: KeyCode::Space,
            speed_modifier: KeyCode::Shift,
        }
    }
}

/// Unchecked camera navigation policy input.
///
/// Pass this descriptor through [`CameraNavigationPolicy3::try_new`] before use.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraNavigationPolicyDescriptor3 {
    pub fly_keys: FlyKeyBindings3,
    pub pointer_bindings: CameraPointerBindings3,
    pub look_radians_per_pixel: f32,
    pub pan_units_per_pixel_at_unit_distance: f64,
    pub fly_units_per_second: f64,
    pub speed_modifier_multiplier: f64,
    pub fly_acceleration_response_per_second: f64,
    pub fly_deceleration_response_per_second: f64,
    pub zoom_exponent_per_logical_pixel: f64,
    pub pitch_limit_radians: f32,
    pub minimum_perspective_fov_tangent_factor: f64,
    pub minimum_orthographic_span_factor: f32,
    pub maximum_orthographic_span_factor: f32,
    pub maximum_pointer_delta: f32,
    pub maximum_scroll_delta_logical_pixels: f32,
    pub maximum_frame_delta: f32,
}

impl Default for CameraNavigationPolicyDescriptor3 {
    fn default() -> Self {
        Self {
            fly_keys: FlyKeyBindings3::default(),
            pointer_bindings: CameraPointerBindings3::default(),
            look_radians_per_pixel: 0.008,
            pan_units_per_pixel_at_unit_distance: 0.002,
            fly_units_per_second: 3.0,
            speed_modifier_multiplier: 4.0,
            fly_acceleration_response_per_second: 30.0,
            fly_deceleration_response_per_second: 40.0,
            zoom_exponent_per_logical_pixel: 0.002,
            pitch_limit_radians: 89.0_f32.to_radians(),
            minimum_perspective_fov_tangent_factor: 0.25,
            minimum_orthographic_span_factor: 0.25,
            maximum_orthographic_span_factor: 4.0,
            maximum_pointer_delta: 4096.0,
            maximum_scroll_delta_logical_pixels: 112.0,
            maximum_frame_delta: 0.1,
        }
    }
}

/// Finite, internally consistent camera navigation policy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraNavigationPolicy3 {
    descriptor: CameraNavigationPolicyDescriptor3,
}

impl CameraNavigationPolicy3 {
    pub fn try_new(
        descriptor: CameraNavigationPolicyDescriptor3,
    ) -> Result<Self, CameraNavigationError> {
        let finite_positive_f32 = |value: f32| value.is_finite() && value > 0.0;
        let finite_positive_f64 = |value: f64| value.is_finite() && value > 0.0;
        let maximum_scroll_exponent = descriptor.zoom_exponent_per_logical_pixel
            * descriptor.maximum_scroll_delta_logical_pixels as f64;
        if !finite_positive_f32(descriptor.look_radians_per_pixel)
            || !finite_positive_f64(descriptor.pan_units_per_pixel_at_unit_distance)
            || !finite_positive_f64(descriptor.fly_units_per_second)
            || !descriptor.speed_modifier_multiplier.is_finite()
            || descriptor.speed_modifier_multiplier < 1.0
            || !finite_positive_f64(descriptor.fly_acceleration_response_per_second)
            || !finite_positive_f64(descriptor.fly_deceleration_response_per_second)
            || !finite_positive_f64(descriptor.zoom_exponent_per_logical_pixel)
            || !finite_positive_f32(descriptor.pitch_limit_radians)
            || descriptor.pitch_limit_radians > MAX_ABSOLUTE_PITCH
            || !finite_positive_f64(descriptor.minimum_perspective_fov_tangent_factor)
            || descriptor.minimum_perspective_fov_tangent_factor > 1.0
            || !finite_positive_f32(descriptor.minimum_orthographic_span_factor)
            || descriptor.minimum_orthographic_span_factor > 1.0
            || !finite_positive_f32(descriptor.maximum_orthographic_span_factor)
            || descriptor.maximum_orthographic_span_factor < 1.0
            || !finite_positive_f32(descriptor.maximum_pointer_delta)
            || !finite_positive_f32(descriptor.maximum_scroll_delta_logical_pixels)
            || !maximum_scroll_exponent.is_finite()
            || !maximum_scroll_exponent.exp().is_finite()
            || (-maximum_scroll_exponent).exp() == 0.0
            || !finite_positive_f32(descriptor.maximum_frame_delta)
        {
            return Err(CameraNavigationError::InvalidPolicy);
        }

        FlyKeyBindings3::try_new(
            descriptor.fly_keys.forward(),
            descriptor.fly_keys.back(),
            descriptor.fly_keys.left(),
            descriptor.fly_keys.right(),
            descriptor.fly_keys.orbit_modifier(),
            descriptor.fly_keys.speed_modifier(),
        )?;
        CameraPointerBindings3::try_new(
            descriptor.pointer_bindings.orbit_rotate(),
            descriptor.pointer_bindings.free_look(),
            descriptor.pointer_bindings.camera_pan(),
        )?;

        Ok(Self { descriptor })
    }

    pub const fn descriptor(self) -> CameraNavigationPolicyDescriptor3 {
        self.descriptor
    }

    pub const fn fly_keys(self) -> FlyKeyBindings3 {
        self.descriptor.fly_keys
    }

    pub const fn pointer_bindings(self) -> CameraPointerBindings3 {
        self.descriptor.pointer_bindings
    }
}

impl Default for CameraNavigationPolicy3 {
    fn default() -> Self {
        Self::try_new(CameraNavigationPolicyDescriptor3::default())
            .expect("default camera navigation policy is valid")
    }
}

/// Checked camera presentation state shared by Orbit and Fly policies.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CameraState3 {
    position: WorldPoint3,
    orientation: Orientation3,
    yaw: f32,
    pitch: f32,
    orbit_target: WorldPoint3,
    orbit_distance: f64,
    projection: Projection3,
}

impl CameraState3 {
    pub fn try_orbit(
        orbit_target: WorldPoint3,
        yaw: f32,
        pitch: f32,
        orbit_distance: f64,
        projection: Projection3,
    ) -> Result<Self, CameraNavigationError> {
        validate_angles(yaw, pitch)?;
        validate_orbit_distance(orbit_distance)?;
        let yaw = normalize_yaw(yaw);
        let orientation = orientation_from_angles(yaw, pitch)?;
        let forward = forward_axis(orientation);
        let position = translate_point(orbit_target, -forward * orbit_distance)?;
        Ok(Self {
            position,
            orientation,
            yaw,
            pitch,
            orbit_target,
            orbit_distance,
            projection,
        })
    }

    pub fn try_fly(
        position: WorldPoint3,
        yaw: f32,
        pitch: f32,
        orbit_distance: f64,
        projection: Projection3,
    ) -> Result<Self, CameraNavigationError> {
        validate_angles(yaw, pitch)?;
        validate_orbit_distance(orbit_distance)?;
        let yaw = normalize_yaw(yaw);
        let orientation = orientation_from_angles(yaw, pitch)?;
        let orbit_target = translate_point(position, forward_axis(orientation) * orbit_distance)?;
        Ok(Self {
            position,
            orientation,
            yaw,
            pitch,
            orbit_target,
            orbit_distance,
            projection,
        })
    }

    pub const fn position(self) -> WorldPoint3 {
        self.position
    }

    pub const fn orientation(self) -> Orientation3 {
        self.orientation
    }

    pub const fn yaw(self) -> f32 {
        self.yaw
    }

    pub const fn pitch(self) -> f32 {
        self.pitch
    }

    pub const fn orbit_target(self) -> WorldPoint3 {
        self.orbit_target
    }

    pub const fn orbit_distance(self) -> f64 {
        self.orbit_distance
    }

    pub const fn projection(self) -> Projection3 {
        self.projection
    }

    pub const fn pose(self) -> RigidPose3 {
        RigidPose3::new(self.position, self.orientation)
    }

    pub const fn view_descriptor(self) -> ViewDescriptor3 {
        ViewDescriptor3::new(self.pose(), self.projection)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct HeldFlyKeys {
    forward: bool,
    back: bool,
    left: bool,
    right: bool,
    orbit_modifier: bool,
    speed_modifier: bool,
}

impl HeldFlyKeys {
    fn movement_active(self) -> bool {
        self.forward != self.back || self.left != self.right
    }

    fn count(self) -> u8 {
        [
            self.forward,
            self.back,
            self.left,
            self.right,
            self.orbit_modifier,
            self.speed_modifier,
        ]
        .into_iter()
        .filter(|held| *held)
        .count() as u8
    }

    fn update(&mut self, operation: FlyMotionOperation3, is_down: bool) {
        let slot = match operation {
            FlyMotionOperation3::Forward => &mut self.forward,
            FlyMotionOperation3::Backward => &mut self.back,
            FlyMotionOperation3::StrafeLeft => &mut self.left,
            FlyMotionOperation3::StrafeRight => &mut self.right,
            FlyMotionOperation3::OrbitModifier => &mut self.orbit_modifier,
            FlyMotionOperation3::SpeedModifier => &mut self.speed_modifier,
        };
        *slot = is_down;
    }
}

/// Optional retained camera controller with no framework or rendering ownership.
#[derive(Debug, Clone, PartialEq)]
pub struct CameraNavigationController3 {
    state: CameraState3,
    policy: CameraNavigationPolicy3,
    mode: CameraNavigationMode3,
    perspective_fov_tangent_bounds: Option<(f64, f64)>,
    orthographic_span_bounds: Option<(f32, f32)>,
    held: HeldFlyKeys,
    fly_velocity: DVec3,
    active_pointer_operation: Option<CameraPointerOperation3>,
    last_pointer: Option<(f32, f32)>,
}

impl CameraNavigationController3 {
    pub fn try_new(
        state: CameraState3,
        policy: CameraNavigationPolicy3,
        mode: CameraNavigationMode3,
    ) -> Result<Self, CameraNavigationError> {
        if state.pitch.abs() > policy.descriptor.pitch_limit_radians {
            return Err(CameraNavigationError::InvalidPolicy);
        }
        let perspective_fov_tangent_bounds = match state.projection {
            Projection3::Perspective(projection) => Some(perspective_fov_tangent_bounds(
                projection,
                policy.descriptor,
            )?),
            Projection3::Orthographic(_) => None,
        };
        let orthographic_span_bounds = match state.projection {
            Projection3::Orthographic(projection) => {
                Some(span_bounds(projection.vertical_span(), policy.descriptor)?)
            }
            Projection3::Perspective(_) => None,
        };
        Ok(Self {
            state,
            policy,
            mode,
            perspective_fov_tangent_bounds,
            orthographic_span_bounds,
            held: HeldFlyKeys::default(),
            fly_velocity: DVec3::ZERO,
            active_pointer_operation: None,
            last_pointer: None,
        })
    }

    pub const fn state(&self) -> CameraState3 {
        self.state
    }

    pub const fn policy(&self) -> CameraNavigationPolicy3 {
        self.policy
    }

    pub const fn mode(&self) -> CameraNavigationMode3 {
        self.mode
    }

    pub const fn perspective_fov_tangent_bounds(&self) -> Option<(f64, f64)> {
        self.perspective_fov_tangent_bounds
    }

    pub const fn orthographic_span_bounds(&self) -> Option<(f32, f32)> {
        self.orthographic_span_bounds
    }

    pub fn pointer_operation_for_button(
        &self,
        button: PointerButton,
    ) -> Option<CameraPointerOperation3> {
        if self.mode == CameraNavigationMode3::Fly
            && button == self.policy.pointer_bindings().free_look()
            && self.held.orbit_modifier
        {
            return Some(CameraPointerOperation3::OrbitRotate);
        }
        self.policy
            .pointer_bindings()
            .operation_for(self.mode, button)
    }

    pub const fn wheel_operation(&self) -> CameraWheelOperation3 {
        match self.state.projection {
            Projection3::Perspective(_) => CameraWheelOperation3::PerspectiveOpticalFovZoom,
            Projection3::Orthographic(_) => CameraWheelOperation3::OrthographicSpanZoom,
        }
    }

    pub const fn view_descriptor(&self) -> ViewDescriptor3 {
        self.state.view_descriptor()
    }

    /// Changes interaction policy without changing camera pose or projection.
    pub fn set_mode(&mut self, mode: CameraNavigationMode3) -> bool {
        if self.mode == mode {
            return false;
        }
        self.cancel_input();
        self.mode = mode;
        true
    }

    pub fn set_projection(
        &mut self,
        projection: Projection3,
    ) -> Result<bool, CameraNavigationError> {
        if self.state.projection == projection {
            return Ok(false);
        }
        let perspective_fov_tangent_bounds = match projection {
            Projection3::Perspective(perspective) => Some(perspective_fov_tangent_bounds(
                perspective,
                self.policy.descriptor,
            )?),
            Projection3::Orthographic(_) => None,
        };
        let orthographic_span_bounds = match projection {
            Projection3::Orthographic(orthographic) => Some(span_bounds(
                orthographic.vertical_span(),
                self.policy.descriptor,
            )?),
            Projection3::Perspective(_) => None,
        };
        self.state.projection = projection;
        self.perspective_fov_tangent_bounds = perspective_fov_tangent_bounds;
        self.orthographic_span_bounds = orthographic_span_bounds;
        Ok(true)
    }

    pub fn reset(&mut self, state: CameraState3) -> Result<bool, CameraNavigationError> {
        let replacement = Self::try_new(state, self.policy, self.mode)?;
        let changed = self.state != state
            || self.held != HeldFlyKeys::default()
            || self.fly_velocity != DVec3::ZERO
            || self.active_pointer_operation.is_some();
        self.cancel_input();
        self.state = state;
        self.perspective_fov_tangent_bounds = replacement.perspective_fov_tangent_bounds;
        self.orthographic_span_bounds = replacement.orthographic_span_bounds;
        Ok(changed)
    }

    /// Begins one named pointer operation from a finite widget-local position.
    ///
    /// Returns `false` when another pointer operation is already active.
    pub fn begin_pointer_operation(
        &mut self,
        operation: CameraPointerOperation3,
        local_position: (f32, f32),
    ) -> Result<bool, CameraNavigationError> {
        validate_pointer_position(local_position)?;
        if !self.pointer_operation_allowed(operation) {
            return Err(CameraNavigationError::InvalidPointerOperation);
        }
        if self.active_pointer_operation.is_some() {
            return Ok(false);
        }
        self.active_pointer_operation = Some(operation);
        self.last_pointer = Some(local_position);
        Ok(true)
    }

    pub fn pointer_moved(
        &mut self,
        local_position: (f32, f32),
    ) -> Result<bool, CameraNavigationError> {
        validate_pointer_position(local_position)?;
        let (Some(operation), Some(previous)) = (self.active_pointer_operation, self.last_pointer)
        else {
            return Ok(false);
        };
        let delta_x = local_position.0 - previous.0;
        let delta_y = local_position.1 - previous.1;
        if !(delta_x.is_finite() && delta_y.is_finite()) {
            return Err(CameraNavigationError::InvalidPointerInput);
        }
        let limit = self.policy.descriptor.maximum_pointer_delta;
        let delta_x = delta_x.clamp(-limit, limit);
        let delta_y = delta_y.clamp(-limit, limit);
        let changed = match operation {
            CameraPointerOperation3::OrbitRotate => self.orbit_rotate(delta_x, delta_y)?,
            CameraPointerOperation3::FreeLook => self.free_look(delta_x, delta_y)?,
            CameraPointerOperation3::CameraPan => self.pan(delta_x, delta_y)?,
        };
        self.last_pointer = Some(local_position);
        Ok(changed)
    }

    pub fn end_pointer_operation(&mut self) -> bool {
        let was_active = self.active_pointer_operation.is_some();
        self.active_pointer_operation = None;
        self.last_pointer = None;
        was_active
    }

    pub const fn active_pointer_operation(&self) -> Option<CameraPointerOperation3> {
        self.active_pointer_operation
    }

    /// Applies a routed key transition only under explicit keyboard capture.
    ///
    /// Repeat events are consumed for bound keys but never change held state.
    pub fn handle_key_event(&mut self, event: &KeyEvent, authority: KeyboardAuthorityMode) -> bool {
        if authority != KeyboardAuthorityMode::Capture || self.mode != CameraNavigationMode3::Fly {
            return false;
        }
        let Some(operation) = self.policy.fly_keys().operation_for(event.key_code) else {
            return false;
        };
        if !event.is_repeat {
            self.held.update(operation, event.is_down);
        }
        true
    }

    /// Clears all held keys and active pointer operation state.
    pub fn cancel_input(&mut self) -> bool {
        let changed = self.held != HeldFlyKeys::default()
            || self.fly_velocity != DVec3::ZERO
            || self.active_pointer_operation.is_some();
        self.held = HeldFlyKeys::default();
        self.fly_velocity = DVec3::ZERO;
        self.active_pointer_operation = None;
        self.last_pointer = None;
        changed
    }

    pub fn needs_update(&self) -> bool {
        self.mode == CameraNavigationMode3::Fly
            && (self.held.movement_active()
                || self.fly_velocity.length() > FLY_VELOCITY_STOP_RESIDUAL_UNITS_PER_SECOND)
    }

    pub fn held_key_count(&self) -> u8 {
        self.held.count()
    }

    /// Integrates held Fly motion from the engine-supplied frame delta.
    pub fn advance(&mut self, frame_delta: f32) -> Result<bool, CameraNavigationError> {
        if !frame_delta.is_finite() || frame_delta < 0.0 {
            return Err(CameraNavigationError::InvalidFrameDelta);
        }
        if frame_delta == 0.0 || !self.needs_update() {
            return Ok(false);
        }

        let descriptor = self.policy.descriptor;
        let target_velocity = self.fly_target_velocity(descriptor);
        let response_per_second = if target_velocity == DVec3::ZERO {
            descriptor.fly_deceleration_response_per_second
        } else {
            descriptor.fly_acceleration_response_per_second
        };
        let admitted_delta = frame_delta.min(descriptor.maximum_frame_delta) as f64;
        let (next_velocity, translation) = smooth_fly_velocity(
            self.fly_velocity,
            target_velocity,
            response_per_second,
            admitted_delta,
        );
        self.fly_velocity = if next_velocity.length() <= FLY_VELOCITY_STOP_RESIDUAL_UNITS_PER_SECOND
            && target_velocity == DVec3::ZERO
        {
            DVec3::ZERO
        } else {
            next_velocity
        };
        if translation == DVec3::ZERO {
            return Ok(false);
        }
        self.translate_camera_and_target(translation)?;
        Ok(true)
    }

    fn fly_target_velocity(&self, descriptor: CameraNavigationPolicyDescriptor3) -> DVec3 {
        let forward_amount = bool_axis(self.held.forward, self.held.back);
        let right_amount = bool_axis(self.held.right, self.held.left);
        let mut direction = forward_axis(self.state.orientation) * forward_amount
            + right_axis(self.state.orientation) * right_amount;
        let length = direction.length();
        if length == 0.0 {
            return DVec3::ZERO;
        }
        direction /= length;
        let speed_multiplier = if self.held.speed_modifier {
            descriptor.speed_modifier_multiplier
        } else {
            1.0
        };
        direction * descriptor.fly_units_per_second * speed_multiplier
    }

    /// Applies bounded perspective optical FOV zoom or orthographic span zoom
    /// from a canonical routed logical-pixel scroll delta.
    pub fn scroll(&mut self, delta_y: f32) -> Result<bool, CameraNavigationError> {
        let Some(state) = self.wheel_zoom_target(self.state, delta_y)? else {
            return Ok(false);
        };
        self.apply_wheel_zoom_transition(state)
    }

    /// Computes a bounded wheel-zoom target without mutating controller state.
    ///
    /// The supplied state must be the current camera state or a previous target
    /// from this controller's wheel-zoom transition boundary. This keeps
    /// temporal presentation in the retained consumer while retaining all
    /// camera policy and bounds in the controller.
    pub fn wheel_zoom_target(
        &self,
        base: CameraState3,
        delta_y: f32,
    ) -> Result<Option<CameraState3>, CameraNavigationError> {
        if !delta_y.is_finite() {
            return Err(CameraNavigationError::InvalidPointerInput);
        }
        if delta_y == 0.0 {
            return Ok(None);
        }
        self.validate_wheel_zoom_state(base)?;
        let descriptor = self.policy.descriptor;
        let delta = delta_y.clamp(
            -descriptor.maximum_scroll_delta_logical_pixels,
            descriptor.maximum_scroll_delta_logical_pixels,
        ) as f64;
        let scale = (-delta * descriptor.zoom_exponent_per_logical_pixel).exp();

        match base.projection {
            Projection3::Orthographic(projection) => {
                let (minimum_span, maximum_span) = self
                    .orthographic_span_bounds
                    .expect("orthographic camera has derived zoom bounds");
                let span = (projection.vertical_span() as f64 * scale)
                    .clamp(minimum_span as f64, maximum_span as f64)
                    as f32;
                if span == projection.vertical_span() {
                    return Ok(None);
                }
                let mut target = base;
                target.projection =
                    OrthographicProjection::try_new(span, projection.near(), projection.far())?
                        .into();
                Ok(Some(target))
            }
            Projection3::Perspective(projection) => {
                let (minimum_tangent, maximum_tangent) = self
                    .perspective_fov_tangent_bounds
                    .expect("perspective camera has derived optical zoom bounds");
                let tangent = perspective_fov_tangent(projection);
                let target_tangent = (tangent * scale).clamp(minimum_tangent, maximum_tangent);
                let vertical_fov_radians = (2.0 * target_tangent.atan()) as f32;
                if vertical_fov_radians == projection.vertical_fov_radians() {
                    return Ok(None);
                }
                let mut target = base;
                target.projection = PerspectiveProjection::try_new(
                    vertical_fov_radians,
                    projection.near(),
                    projection.far(),
                )?
                .into();
                Ok(Some(target))
            }
        }
    }

    /// Computes a checked perspective wheel-zoom target that preserves one
    /// explicit world point's screen coordinate by coupling the FOV change to
    /// a camera-plane translation. Orthographic input retains ordinary span zoom.
    ///
    /// The caller owns the world-point source. This controller validates only
    /// camera state, FOV bounds, and the compensating camera-plane translation.
    pub fn wheel_zoom_target_anchored(
        &self,
        base: CameraState3,
        delta_y: f32,
        anchor: WorldPoint3,
    ) -> Result<Option<CameraState3>, CameraNavigationError> {
        let Some(mut target) = self.wheel_zoom_target(base, delta_y)? else {
            return Ok(None);
        };
        let Projection3::Perspective(base_projection) = base.projection else {
            return Ok(Some(target));
        };
        let Projection3::Perspective(target_projection) = target.projection else {
            unreachable!("perspective wheel zoom must preserve projection kind");
        };

        let anchor_offset = world_difference(anchor, base.position);
        let forward = forward_axis(base.orientation);
        let depth = anchor_offset.dot(forward);
        if !depth.is_finite() || depth <= 0.0 {
            return Err(CameraNavigationError::InvalidWheelAnchor);
        }
        let scale =
            perspective_fov_tangent(target_projection) / perspective_fov_tangent(base_projection);
        if !scale.is_finite() || scale <= 0.0 {
            return Err(CameraNavigationError::InvalidPolicy);
        }
        let right = right_axis(base.orientation);
        let up = up_axis(base.orientation);
        let translation =
            (right * anchor_offset.dot(right) + up * anchor_offset.dot(up)) * (1.0 - scale);
        target.position = translate_point(base.position, translation)?;
        target.orbit_target = translate_point(base.orbit_target, translation)?;
        self.validate_wheel_zoom_pair(base, target)?;
        Ok(Some(target))
    }

    /// Produces the checked intermediate state for one wheel-zoom transition
    /// sample. The caller owns temporal sampling; this controller owns camera
    /// geometry, projection semantics, and finite bounds.
    pub fn interpolate_wheel_zoom(
        &self,
        base: CameraState3,
        target: CameraState3,
        progress: f64,
    ) -> Result<CameraState3, CameraNavigationError> {
        if !progress.is_finite() || !(0.0..=1.0).contains(&progress) {
            return Err(CameraNavigationError::InvalidPolicy);
        }
        self.validate_wheel_zoom_state(base)?;
        self.validate_wheel_zoom_state(target)?;
        self.validate_wheel_zoom_pair(base, target)?;
        if progress == 0.0 {
            return Ok(base);
        }
        if progress == 1.0 {
            return Ok(target);
        }

        match base.projection {
            Projection3::Perspective(base_projection) => {
                let Projection3::Perspective(target_projection) = target.projection else {
                    unreachable!("wheel zoom pair must preserve projection kind");
                };
                let ratio = perspective_fov_tangent(target_projection)
                    / perspective_fov_tangent(base_projection);
                let tangent = perspective_fov_tangent(base_projection) * ratio.powf(progress);
                let translation = compensating_pan_translation(base, target)?;
                let translation_progress = compensating_pan_progress(ratio, progress)?;
                let mut intermediate = base;
                intermediate.position =
                    translate_point(base.position, translation * translation_progress)?;
                intermediate.orbit_target =
                    translate_point(base.orbit_target, translation * translation_progress)?;
                intermediate.projection = PerspectiveProjection::try_new(
                    (2.0 * tangent.atan()) as f32,
                    base_projection.near(),
                    base_projection.far(),
                )?
                .into();
                Ok(intermediate)
            }
            Projection3::Orthographic(base_projection) => {
                let Projection3::Orthographic(target_projection) = target.projection else {
                    unreachable!("wheel zoom pair must preserve projection kind");
                };
                let ratio = f64::from(target_projection.vertical_span())
                    / f64::from(base_projection.vertical_span());
                let span =
                    (f64::from(base_projection.vertical_span()) * ratio.powf(progress)) as f32;
                let mut intermediate = base;
                intermediate.projection = OrthographicProjection::try_new(
                    span,
                    base_projection.near(),
                    base_projection.far(),
                )?
                .into();
                Ok(intermediate)
            }
        }
    }

    /// Commits a state previously produced by the wheel-zoom transition
    /// boundary without clearing held keys or pointer gesture state.
    pub fn apply_wheel_zoom_transition(
        &mut self,
        state: CameraState3,
    ) -> Result<bool, CameraNavigationError> {
        self.validate_wheel_zoom_state(state)?;
        if self.state == state {
            return Ok(false);
        }
        self.state = state;
        Ok(true)
    }

    fn validate_wheel_zoom_state(&self, state: CameraState3) -> Result<(), CameraNavigationError> {
        let current = self.state;
        if state.orientation != current.orientation
            || state.yaw != current.yaw
            || state.pitch != current.pitch
            || !same_projection_family_and_planes(state.projection, current.projection)
        {
            return Err(CameraNavigationError::InvalidPolicy);
        }
        match state.projection {
            Projection3::Perspective(projection) => {
                let Some((minimum_tangent, maximum_tangent)) = self.perspective_fov_tangent_bounds
                else {
                    return Err(CameraNavigationError::InvalidPolicy);
                };
                if !fov_tangent_within_bounds(
                    perspective_fov_tangent(projection),
                    minimum_tangent,
                    maximum_tangent,
                ) || state.orbit_distance != current.orbit_distance
                {
                    return Err(CameraNavigationError::InvalidPolicy);
                }
                let _ = compensating_pan_translation(current, state)?;
            }
            Projection3::Orthographic(projection) => {
                let Some((minimum, maximum)) = self.orthographic_span_bounds else {
                    return Err(CameraNavigationError::InvalidPolicy);
                };
                if !(minimum..=maximum).contains(&projection.vertical_span())
                    || state.position != current.position
                    || state.orbit_target != current.orbit_target
                    || state.orbit_distance != current.orbit_distance
                {
                    return Err(CameraNavigationError::InvalidPolicy);
                }
            }
        }
        Ok(())
    }

    fn validate_wheel_zoom_pair(
        &self,
        base: CameraState3,
        target: CameraState3,
    ) -> Result<(), CameraNavigationError> {
        if !same_projection_family_and_planes(base.projection, target.projection) {
            return Err(CameraNavigationError::InvalidPolicy);
        }
        match base.projection {
            Projection3::Perspective(_) => {
                let _ = compensating_pan_translation(base, target)?;
            }
            Projection3::Orthographic(_) => {
                if base.position != target.position
                    || base.orbit_target != target.orbit_target
                    || base.orbit_distance != target.orbit_distance
                {
                    return Err(CameraNavigationError::InvalidPolicy);
                }
            }
        }
        Ok(())
    }

    fn pointer_operation_allowed(&self, operation: CameraPointerOperation3) -> bool {
        match (self.mode, operation) {
            (CameraNavigationMode3::Orbit, CameraPointerOperation3::OrbitRotate) => true,
            (CameraNavigationMode3::Fly, CameraPointerOperation3::OrbitRotate) => {
                self.held.orbit_modifier
            }
            (CameraNavigationMode3::Fly, CameraPointerOperation3::FreeLook)
            | (_, CameraPointerOperation3::CameraPan) => true,
            _ => false,
        }
    }

    fn orbit_rotate(&mut self, delta_x: f32, delta_y: f32) -> Result<bool, CameraNavigationError> {
        let Some((yaw, pitch, orientation)) = self.updated_look(delta_x, delta_y)? else {
            return Ok(false);
        };
        let forward = forward_axis(orientation);
        self.state.position = translate_point(
            self.state.orbit_target,
            -forward * self.state.orbit_distance,
        )?;
        self.state.orientation = orientation;
        self.state.yaw = yaw;
        self.state.pitch = pitch;
        Ok(true)
    }

    fn free_look(&mut self, delta_x: f32, delta_y: f32) -> Result<bool, CameraNavigationError> {
        let Some((yaw, pitch, orientation)) = self.updated_look(delta_x, delta_y)? else {
            return Ok(false);
        };
        let orbit_target = translate_point(
            self.state.position,
            forward_axis(orientation) * self.state.orbit_distance,
        )?;
        self.state.orientation = orientation;
        self.state.yaw = yaw;
        self.state.pitch = pitch;
        self.state.orbit_target = orbit_target;
        Ok(true)
    }

    fn updated_look(
        &self,
        delta_x: f32,
        delta_y: f32,
    ) -> Result<Option<(f32, f32, Orientation3)>, CameraNavigationError> {
        if delta_x == 0.0 && delta_y == 0.0 {
            return Ok(None);
        }
        let descriptor = self.policy.descriptor;
        let yaw = normalize_yaw(self.state.yaw - delta_x * descriptor.look_radians_per_pixel);
        let pitch = (self.state.pitch + delta_y * descriptor.look_radians_per_pixel).clamp(
            -descriptor.pitch_limit_radians,
            descriptor.pitch_limit_radians,
        );
        if yaw == self.state.yaw && pitch == self.state.pitch {
            return Ok(None);
        }
        Ok(Some((yaw, pitch, orientation_from_angles(yaw, pitch)?)))
    }

    fn pan(&mut self, delta_x: f32, delta_y: f32) -> Result<bool, CameraNavigationError> {
        if delta_x == 0.0 && delta_y == 0.0 {
            return Ok(false);
        }
        let scale =
            self.policy.descriptor.pan_units_per_pixel_at_unit_distance * self.state.orbit_distance;
        let translation = -right_axis(self.state.orientation) * delta_x as f64 * scale
            + up_axis(self.state.orientation) * delta_y as f64 * scale;
        self.translate_camera_and_target(translation)?;
        Ok(true)
    }

    fn translate_camera_and_target(
        &mut self,
        translation: DVec3,
    ) -> Result<(), CameraNavigationError> {
        let position = translate_point(self.state.position, translation)?;
        let orbit_target = translate_point(self.state.orbit_target, translation)?;
        self.state.position = position;
        self.state.orbit_target = orbit_target;
        Ok(())
    }
}

fn same_projection_family_and_planes(left: Projection3, right: Projection3) -> bool {
    match (left, right) {
        (Projection3::Perspective(left), Projection3::Perspective(right)) => {
            left.near() == right.near() && left.far() == right.far()
        }
        (Projection3::Orthographic(left), Projection3::Orthographic(right)) => {
            left.near() == right.near() && left.far() == right.far()
        }
        (Projection3::Perspective(_) | Projection3::Orthographic(_), _) => false,
    }
}

fn validate_angles(yaw: f32, pitch: f32) -> Result<(), CameraNavigationError> {
    if !yaw.is_finite() {
        return Err(CameraNavigationError::InvalidYaw);
    }
    if !pitch.is_finite() || pitch.abs() > MAX_ABSOLUTE_PITCH {
        return Err(CameraNavigationError::InvalidPitch);
    }
    Ok(())
}

fn validate_orbit_distance(distance: f64) -> Result<(), CameraNavigationError> {
    if !distance.is_finite() || distance <= 0.0 {
        return Err(CameraNavigationError::InvalidOrbitDistance);
    }
    Ok(())
}

fn perspective_fov_tangent_bounds(
    projection: PerspectiveProjection,
    descriptor: CameraNavigationPolicyDescriptor3,
) -> Result<(f64, f64), CameraNavigationError> {
    let reference = perspective_fov_tangent(projection);
    let minimum = reference * descriptor.minimum_perspective_fov_tangent_factor;
    let maximum = reference;
    if minimum.is_finite()
        && maximum.is_finite()
        && minimum > 0.0
        && minimum <= reference
        && maximum >= reference
        && PerspectiveProjection::try_new(
            (2.0 * minimum.atan()) as f32,
            projection.near(),
            projection.far(),
        )
        .is_ok()
        && PerspectiveProjection::try_new(
            (2.0 * maximum.atan()) as f32,
            projection.near(),
            projection.far(),
        )
        .is_ok()
    {
        Ok((minimum, maximum))
    } else {
        Err(CameraNavigationError::InvalidPolicy)
    }
}

fn perspective_fov_tangent(projection: PerspectiveProjection) -> f64 {
    (f64::from(projection.vertical_fov_radians()) * 0.5).tan()
}

fn compensating_pan_translation(
    base: CameraState3,
    target: CameraState3,
) -> Result<DVec3, CameraNavigationError> {
    if target.orientation != base.orientation
        || target.yaw != base.yaw
        || target.pitch != base.pitch
        || target.orbit_distance != base.orbit_distance
    {
        return Err(CameraNavigationError::InvalidPolicy);
    }
    let position_translation = world_difference(target.position, base.position);
    let target_translation = world_difference(target.orbit_target, base.orbit_target);
    if !vectors_approximately_equal(position_translation, target_translation)
        || !camera_plane_vector(position_translation, base.orientation)
    {
        return Err(CameraNavigationError::InvalidPolicy);
    }
    Ok(position_translation)
}

fn compensating_pan_progress(
    fov_tangent_ratio: f64,
    progress: f64,
) -> Result<f64, CameraNavigationError> {
    let denominator = 1.0 - fov_tangent_ratio;
    let numerator = 1.0 - fov_tangent_ratio.powf(progress);
    let interpolation = if denominator.abs() <= 1.0e-12 {
        progress
    } else {
        numerator / denominator
    };
    if interpolation.is_finite() {
        Ok(interpolation)
    } else {
        Err(CameraNavigationError::InvalidPolicy)
    }
}

fn world_difference(left: WorldPoint3, right: WorldPoint3) -> DVec3 {
    DVec3::new(
        left.x() - right.x(),
        left.y() - right.y(),
        left.z() - right.z(),
    )
}

fn vectors_approximately_equal(left: DVec3, right: DVec3) -> bool {
    let scale = left.abs().max(right.abs()).max_element().max(1.0);
    (left - right).abs().max_element() <= 1.0e-12 * scale
}

fn camera_plane_vector(vector: DVec3, orientation: Orientation3) -> bool {
    let scale = vector.abs().max_element().max(1.0);
    vector.dot(forward_axis(orientation)).abs() <= 1.0e-12 * scale
}

fn fov_tangent_within_bounds(value: f64, minimum: f64, maximum: f64) -> bool {
    let tolerance = maximum.abs().max(1.0) * 1.0e-6;
    value >= minimum - tolerance && value <= maximum + tolerance
}

fn span_bounds(
    reference: f32,
    descriptor: CameraNavigationPolicyDescriptor3,
) -> Result<(f32, f32), CameraNavigationError> {
    let minimum = reference * descriptor.minimum_orthographic_span_factor;
    let maximum = reference * descriptor.maximum_orthographic_span_factor;
    if minimum.is_finite()
        && maximum.is_finite()
        && minimum > 0.0
        && minimum <= reference
        && maximum >= reference
    {
        Ok((minimum, maximum))
    } else {
        Err(CameraNavigationError::InvalidPolicy)
    }
}

fn validate_pointer_position(position: (f32, f32)) -> Result<(), CameraNavigationError> {
    if position.0.is_finite() && position.1.is_finite() {
        Ok(())
    } else {
        Err(CameraNavigationError::InvalidPointerInput)
    }
}

fn normalize_yaw(yaw: f32) -> f32 {
    (yaw + std::f32::consts::PI).rem_euclid(std::f32::consts::TAU) - std::f32::consts::PI
}

fn orientation_from_angles(yaw: f32, pitch: f32) -> Result<Orientation3, CameraNavigationError> {
    validate_angles(yaw, pitch)?;
    let orientation = Quat::from_rotation_y(yaw) * Quat::from_rotation_x(-pitch);
    Ok(Orientation3::try_from_xyzw(
        orientation.x,
        orientation.y,
        orientation.z,
        orientation.w,
    )?)
}

fn orientation_quat(orientation: Orientation3) -> Quat {
    Quat::from_xyzw(
        orientation.x(),
        orientation.y(),
        orientation.z(),
        orientation.w(),
    )
}

fn forward_axis(orientation: Orientation3) -> DVec3 {
    (orientation_quat(orientation) * Vec3::NEG_Z).as_dvec3()
}

fn right_axis(orientation: Orientation3) -> DVec3 {
    (orientation_quat(orientation) * Vec3::X).as_dvec3()
}

fn up_axis(orientation: Orientation3) -> DVec3 {
    (orientation_quat(orientation) * Vec3::Y).as_dvec3()
}

fn bool_axis(positive: bool, negative: bool) -> f64 {
    f64::from(u8::from(positive)) - f64::from(u8::from(negative))
}

fn smooth_fly_velocity(
    current: DVec3,
    target: DVec3,
    response_per_second: f64,
    delta_seconds: f64,
) -> (DVec3, DVec3) {
    let decay = (-response_per_second * delta_seconds).exp();
    let difference = current - target;
    let next = target + difference * decay;
    let translation = target * delta_seconds + difference * ((1.0 - decay) / response_per_second);
    (next, translation)
}

fn translate_point(
    point: WorldPoint3,
    translation: DVec3,
) -> Result<WorldPoint3, CameraNavigationError> {
    if !translation.is_finite() {
        return Err(CameraNavigationError::Math(Math3DError::ArithmeticOverflow));
    }
    Ok(WorldPoint3::try_new(
        point.x() + translation.x,
        point.y() + translation.y,
        point.z() + translation.z,
    )?)
}
