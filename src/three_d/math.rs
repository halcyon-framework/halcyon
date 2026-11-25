use glam::{DQuat, DVec3, Mat3, Mat4, Quat, Vec3, Vec4};
use thiserror::Error;

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum Math3DError {
    #[error("world point components must be finite")]
    NonFiniteWorldPoint,
    #[error("world vector components must be finite")]
    NonFiniteWorldVector,
    #[error("local point components must be finite f32 values")]
    NonFiniteLocalPoint,
    #[error("local vector components must be finite f32 values")]
    NonFiniteLocalVector,
    #[error("orientation must be a finite non-zero quaternion")]
    InvalidOrientation,
    #[error("model scale components must be finite and strictly positive")]
    InvalidScale,
    #[error("model determinant, reciprocal scale, or matrix is not finite f32")]
    NonRepresentableModelTransform,
    #[error("perspective vertical field of view must be finite and in (0, PI)")]
    InvalidPerspectiveFieldOfView,
    #[error("perspective near distance must be finite and positive")]
    InvalidPerspectiveNear,
    #[error("finite perspective far distance must be finite and greater than near")]
    InvalidPerspectiveFar,
    #[error("orthographic vertical span must be finite and positive")]
    InvalidOrthographicSpan,
    #[error("orthographic near distance must be finite and non-negative")]
    InvalidOrthographicNear,
    #[error("orthographic far distance must be finite and greater than near")]
    InvalidOrthographicFar,
    #[error("projection coefficients are not finite f32 values")]
    NonRepresentableProjection,
    #[error("physical render extent must be non-zero")]
    InvalidPhysicalExtent,
    #[error("viewport-local coordinates must be finite values in [0, 1]")]
    InvalidViewportCoordinate,
    #[error("render-relative translation is not representable as finite f32")]
    NonRepresentableRenderRelativeTranslation,
    #[error("view or view-projection matrix is not representable as finite f32")]
    NonRepresentableViewTransform,
    #[error("ray direction must be finite and non-zero")]
    InvalidRayDirection,
    #[error("ray distance must be finite and non-negative")]
    InvalidRayDistance,
    #[error("plane normal must be finite and non-zero")]
    DegeneratePlane,
    #[error("sphere radius must be finite and strictly positive")]
    InvalidSphereRadius,
    #[error("axis-aligned box bounds must be ordered component-wise")]
    InvalidAabb,
    #[error("3D arithmetic overflowed its finite scalar domain")]
    ArithmeticOverflow,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldPoint3 {
    x: f64,
    y: f64,
    z: f64,
}

impl WorldPoint3 {
    pub fn try_new(x: f64, y: f64, z: f64) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(Math3DError::NonFiniteWorldPoint);
        }
        Ok(Self { x, y, z })
    }

    pub const fn origin() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub const fn x(self) -> f64 {
        self.x
    }

    pub const fn y(self) -> f64 {
        self.y
    }

    pub const fn z(self) -> f64 {
        self.z
    }

    pub fn render_relative_to(
        self,
        render_origin: Self,
    ) -> Result<RenderRelativeTranslation3, Math3DError> {
        let relative = checked_world_difference(self, render_origin)?;
        RenderRelativeTranslation3::try_from_dvec3(relative)
    }

    fn to_dvec3(self) -> DVec3 {
        DVec3::new(self.x, self.y, self.z)
    }

    fn checked_add(self, vector: DVec3) -> Result<Self, Math3DError> {
        let point = self.to_dvec3() + vector;
        if !point.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Self::try_new(point.x, point.y, point.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct WorldVector3 {
    x: f64,
    y: f64,
    z: f64,
}

impl WorldVector3 {
    pub fn try_new(x: f64, y: f64, z: f64) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(Math3DError::NonFiniteWorldVector);
        }
        Ok(Self { x, y, z })
    }

    pub const fn x(self) -> f64 {
        self.x
    }

    pub const fn y(self) -> f64 {
        self.y
    }

    pub const fn z(self) -> f64 {
        self.z
    }

    pub fn try_length(self) -> Result<f64, Math3DError> {
        let vector = self.to_dvec3();
        let maximum_component = vector.x.abs().max(vector.y.abs()).max(vector.z.abs());
        if maximum_component == 0.0 {
            return Ok(0.0);
        }
        let length = maximum_component * (vector / maximum_component).length();
        if !length.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(length)
    }

    pub fn normalized(self) -> Result<Self, Math3DError> {
        Self::try_from_normalized(self.to_dvec3())
    }

    fn to_dvec3(self) -> DVec3 {
        DVec3::new(self.x, self.y, self.z)
    }

    fn try_from_normalized(vector: DVec3) -> Result<Self, Math3DError> {
        let maximum_component = vector.x.abs().max(vector.y.abs()).max(vector.z.abs());
        if !maximum_component.is_finite() || maximum_component <= 0.0 {
            return Err(Math3DError::InvalidRayDirection);
        }
        let scaled = vector / maximum_component;
        let scaled_length = scaled.length();
        if !scaled_length.is_finite() || scaled_length <= 0.0 {
            return Err(Math3DError::InvalidRayDirection);
        }
        let normalized = scaled / scaled_length;
        if !normalized.is_finite() {
            return Err(Math3DError::InvalidRayDirection);
        }
        Self::try_new(normalized.x, normalized.y, normalized.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalPoint3 {
    x: f32,
    y: f32,
    z: f32,
}

impl LocalPoint3 {
    pub fn try_new(x: f32, y: f32, z: f32) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(Math3DError::NonFiniteLocalPoint);
        }
        Ok(Self { x, y, z })
    }

    pub const fn origin() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
        }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    fn to_dvec3(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalVector3 {
    x: f32,
    y: f32,
    z: f32,
}

impl LocalVector3 {
    pub fn try_new(x: f32, y: f32, z: f32) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite()) {
            return Err(Math3DError::NonFiniteLocalVector);
        }
        Ok(Self { x, y, z })
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    pub fn try_length(self) -> Result<f32, Math3DError> {
        let vector = Vec3::new(self.x, self.y, self.z);
        let maximum_component = vector.x.abs().max(vector.y.abs()).max(vector.z.abs());
        if maximum_component == 0.0 {
            return Ok(0.0);
        }
        checked_f32(
            maximum_component as f64 * (vector / maximum_component).length() as f64,
            Math3DError::ArithmeticOverflow,
        )
    }

    fn to_dvec3(self) -> DVec3 {
        DVec3::new(self.x as f64, self.y as f64, self.z as f64)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Orientation3 {
    x: f32,
    y: f32,
    z: f32,
    w: f32,
}

impl Orientation3 {
    pub fn try_from_xyzw(x: f32, y: f32, z: f32, w: f32) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite() && w.is_finite()) {
            return Err(Math3DError::InvalidOrientation);
        }

        let components = [x as f64, y as f64, z as f64, w as f64];
        let norm_squared = components
            .iter()
            .map(|component| component * component)
            .sum::<f64>();
        if !norm_squared.is_finite() || norm_squared <= 0.0 {
            return Err(Math3DError::InvalidOrientation);
        }

        let inverse_norm = norm_squared.sqrt().recip();
        let normalized = components.map(|component| (component * inverse_norm) as f32);
        if !normalized.iter().all(|component| component.is_finite()) {
            return Err(Math3DError::InvalidOrientation);
        }

        let correction = normalized
            .iter()
            .map(|component| (*component as f64) * (*component as f64))
            .sum::<f64>()
            .sqrt()
            .recip();
        let normalized = normalized.map(|component| (component as f64 * correction) as f32);
        if !normalized.iter().all(|component| component.is_finite()) {
            return Err(Math3DError::InvalidOrientation);
        }

        Ok(Self {
            x: normalized[0],
            y: normalized[1],
            z: normalized[2],
            w: normalized[3],
        })
    }

    pub const fn identity() -> Self {
        Self {
            x: 0.0,
            y: 0.0,
            z: 0.0,
            w: 1.0,
        }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    pub const fn w(self) -> f32 {
        self.w
    }

    /// Returns this local-to-world orientation followed by `local` in this
    /// orientation's local axes.
    pub fn compose_local(self, local: Self) -> Result<Self, Math3DError> {
        let composed = self.to_quat() * local.to_quat();
        Self::try_from_xyzw(composed.x, composed.y, composed.z, composed.w)
    }

    fn to_quat(self) -> Quat {
        Quat::from_xyzw(self.x, self.y, self.z, self.w)
    }

    fn to_dquat(self) -> DQuat {
        DQuat::from_xyzw(self.x as f64, self.y as f64, self.z as f64, self.w as f64)
    }
}

impl Default for Orientation3 {
    fn default() -> Self {
        Self::identity()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PositiveScale3 {
    x: f32,
    y: f32,
    z: f32,
}

impl PositiveScale3 {
    pub fn try_new(x: f32, y: f32, z: f32) -> Result<Self, Math3DError> {
        if !(x.is_finite() && y.is_finite() && z.is_finite() && x > 0.0 && y > 0.0 && z > 0.0) {
            return Err(Math3DError::InvalidScale);
        }

        let determinant = x as f64 * y as f64 * z as f64;
        let determinant = checked_f32(determinant, Math3DError::NonRepresentableModelTransform)?;
        if determinant <= 0.0 {
            return Err(Math3DError::NonRepresentableModelTransform);
        }

        for component in [x, y, z] {
            checked_f32(
                1.0 / component as f64,
                Math3DError::NonRepresentableModelTransform,
            )?;
        }

        Ok(Self { x, y, z })
    }

    pub const fn one() -> Self {
        Self {
            x: 1.0,
            y: 1.0,
            z: 1.0,
        }
    }

    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    pub fn determinant(self) -> f32 {
        (self.x as f64 * self.y as f64 * self.z as f64) as f32
    }

    fn to_vec3(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }

    fn reciprocal_vec3(self) -> Vec3 {
        Vec3::new(self.x.recip(), self.y.recip(), self.z.recip())
    }

    fn reciprocal_dvec3(self) -> DVec3 {
        DVec3::new(
            1.0 / self.x as f64,
            1.0 / self.y as f64,
            1.0 / self.z as f64,
        )
    }
}

impl Default for PositiveScale3 {
    fn default() -> Self {
        Self::one()
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RigidPose3 {
    translation: WorldPoint3,
    orientation: Orientation3,
}

impl RigidPose3 {
    pub const fn new(translation: WorldPoint3, orientation: Orientation3) -> Self {
        Self {
            translation,
            orientation,
        }
    }

    pub const fn translation(self) -> WorldPoint3 {
        self.translation
    }

    pub const fn orientation(self) -> Orientation3 {
        self.orientation
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ModelPlacement3 {
    translation: WorldPoint3,
    orientation: Orientation3,
    scale: PositiveScale3,
}

impl ModelPlacement3 {
    pub fn try_new(
        translation: WorldPoint3,
        orientation: Orientation3,
        scale: PositiveScale3,
    ) -> Result<Self, Math3DError> {
        let placement = Self {
            translation,
            orientation,
            scale,
        };
        placement.checked_linear_and_normal_matrices()?;
        Ok(placement)
    }

    pub const fn translation(self) -> WorldPoint3 {
        self.translation
    }

    pub const fn orientation(self) -> Orientation3 {
        self.orientation
    }

    pub const fn scale(self) -> PositiveScale3 {
        self.scale
    }

    pub fn determinant(self) -> f32 {
        self.scale.determinant()
    }

    pub fn render_relative_translation(
        self,
        render_origin: WorldPoint3,
    ) -> Result<RenderRelativeTranslation3, Math3DError> {
        self.translation.render_relative_to(render_origin)
    }

    pub(crate) fn checked_transform(
        self,
        render_origin: WorldPoint3,
    ) -> Result<CheckedModelTransform3, Math3DError> {
        let translation = self.render_relative_translation(render_origin)?;
        let (linear_to_world, normal_to_world) = self.checked_linear_and_normal_matrices()?;
        let model_to_render = Mat4::from_scale_rotation_translation(
            self.scale.to_vec3(),
            self.orientation.to_quat(),
            translation.to_vec3(),
        );

        Ok(CheckedModelTransform3 {
            model_to_render: Matrix4F32::try_from_glam(
                model_to_render,
                Math3DError::NonRepresentableModelTransform,
            )?,
            linear_to_world,
            normal_to_world,
            determinant: self.determinant(),
        })
    }

    fn checked_linear_and_normal_matrices(self) -> Result<(Matrix3F32, Matrix3F32), Math3DError> {
        let rotation = Mat3::from_quat(self.orientation.to_quat());
        let linear = rotation * Mat3::from_diagonal(self.scale.to_vec3());
        let normal = rotation * Mat3::from_diagonal(self.scale.reciprocal_vec3());
        Ok((
            Matrix3F32::try_from_glam(linear, Math3DError::NonRepresentableModelTransform)?,
            Matrix3F32::try_from_glam(normal, Math3DError::NonRepresentableModelTransform)?,
        ))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RenderRelativeTranslation3 {
    x: f32,
    y: f32,
    z: f32,
}

impl RenderRelativeTranslation3 {
    pub const fn x(self) -> f32 {
        self.x
    }

    pub const fn y(self) -> f32 {
        self.y
    }

    pub const fn z(self) -> f32 {
        self.z
    }

    fn try_from_dvec3(value: DVec3) -> Result<Self, Math3DError> {
        Ok(Self {
            x: checked_f32(
                value.x,
                Math3DError::NonRepresentableRenderRelativeTranslation,
            )?,
            y: checked_f32(
                value.y,
                Math3DError::NonRepresentableRenderRelativeTranslation,
            )?,
            z: checked_f32(
                value.z,
                Math3DError::NonRepresentableRenderRelativeTranslation,
            )?,
        })
    }

    fn to_vec3(self) -> Vec3 {
        Vec3::new(self.x, self.y, self.z)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum PerspectiveFar {
    Finite(f32),
    Infinite,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct PerspectiveProjection {
    vertical_fov_radians: f32,
    near: f32,
    far: PerspectiveFar,
}

impl PerspectiveProjection {
    pub fn try_new(
        vertical_fov_radians: f32,
        near: f32,
        far: PerspectiveFar,
    ) -> Result<Self, Math3DError> {
        if !vertical_fov_radians.is_finite()
            || vertical_fov_radians <= 0.0
            || vertical_fov_radians >= std::f32::consts::PI
        {
            return Err(Math3DError::InvalidPerspectiveFieldOfView);
        }
        if !near.is_finite() || near <= 0.0 {
            return Err(Math3DError::InvalidPerspectiveNear);
        }
        if let PerspectiveFar::Finite(far) = far
            && (!far.is_finite() || far <= near)
        {
            return Err(Math3DError::InvalidPerspectiveFar);
        }

        let projection = Self {
            vertical_fov_radians,
            near,
            far,
        };
        projection.validate_static_coefficients()?;
        Ok(projection)
    }

    pub fn try_finite(vertical_fov_radians: f32, near: f32, far: f32) -> Result<Self, Math3DError> {
        Self::try_new(vertical_fov_radians, near, PerspectiveFar::Finite(far))
    }

    pub fn try_infinite(vertical_fov_radians: f32, near: f32) -> Result<Self, Math3DError> {
        Self::try_new(vertical_fov_radians, near, PerspectiveFar::Infinite)
    }

    pub const fn vertical_fov_radians(self) -> f32 {
        self.vertical_fov_radians
    }

    pub const fn near(self) -> f32 {
        self.near
    }

    pub const fn far(self) -> PerspectiveFar {
        self.far
    }

    fn validate_static_coefficients(self) -> Result<(), Math3DError> {
        checked_f32(
            1.0 / (self.vertical_fov_radians as f64 * 0.5).tan(),
            Math3DError::NonRepresentableProjection,
        )?;
        let near = self.near as f64;
        if let PerspectiveFar::Finite(far) = self.far {
            let far = far as f64;
            let range = far - near;
            checked_f32(near / range, Math3DError::NonRepresentableProjection)?;
            checked_f32(near * far / range, Math3DError::NonRepresentableProjection)?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct OrthographicProjection {
    vertical_span: f32,
    near: f32,
    far: f32,
}

impl OrthographicProjection {
    pub fn try_new(vertical_span: f32, near: f32, far: f32) -> Result<Self, Math3DError> {
        if !vertical_span.is_finite() || vertical_span <= 0.0 {
            return Err(Math3DError::InvalidOrthographicSpan);
        }
        if !near.is_finite() || near < 0.0 {
            return Err(Math3DError::InvalidOrthographicNear);
        }
        if !far.is_finite() || far <= near {
            return Err(Math3DError::InvalidOrthographicFar);
        }

        let range = far as f64 - near as f64;
        checked_f32(
            2.0 / vertical_span as f64,
            Math3DError::NonRepresentableProjection,
        )?;
        checked_f32(1.0 / range, Math3DError::NonRepresentableProjection)?;
        checked_f32(far as f64 / range, Math3DError::NonRepresentableProjection)?;

        Ok(Self {
            vertical_span,
            near,
            far,
        })
    }

    pub const fn vertical_span(self) -> f32 {
        self.vertical_span
    }

    pub const fn near(self) -> f32 {
        self.near
    }

    pub const fn far(self) -> f32 {
        self.far
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Projection3 {
    Perspective(PerspectiveProjection),
    Orthographic(OrthographicProjection),
}

impl Projection3 {
    pub(crate) fn matrix(
        self,
        physical_width: u32,
        physical_height: u32,
    ) -> Result<Matrix4F32, Math3DError> {
        let (aspect_f32, _) = checked_aspect_ratio(physical_width, physical_height)?;
        let matrix = match self {
            Self::Perspective(projection) => perspective_matrix(projection, aspect_f32)?,
            Self::Orthographic(projection) => orthographic_matrix(projection, aspect_f32)?,
        };
        Matrix4F32::try_from_glam(matrix, Math3DError::NonRepresentableProjection)
    }
}

impl From<PerspectiveProjection> for Projection3 {
    fn from(value: PerspectiveProjection) -> Self {
        Self::Perspective(value)
    }
}

impl From<OrthographicProjection> for Projection3 {
    fn from(value: OrthographicProjection) -> Self {
        Self::Orthographic(value)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ViewDescriptor3 {
    pose: RigidPose3,
    projection: Projection3,
}

impl ViewDescriptor3 {
    pub const fn new(pose: RigidPose3, projection: Projection3) -> Self {
        Self { pose, projection }
    }

    pub const fn pose(self) -> RigidPose3 {
        self.pose
    }

    pub const fn projection(self) -> Projection3 {
        self.projection
    }

    pub(crate) fn prepared_matrices(
        self,
        render_origin: WorldPoint3,
        physical_width: u32,
        physical_height: u32,
    ) -> Result<PreparedViewMatrices3, Math3DError> {
        let camera_relative = self.pose.translation.render_relative_to(render_origin)?;
        let inverse_orientation = self.pose.orientation.to_quat().conjugate();
        let view_translation = -(inverse_orientation * camera_relative.to_vec3());
        let view = Matrix4F32::try_from_glam(
            Mat4::from_rotation_translation(inverse_orientation, view_translation),
            Math3DError::NonRepresentableViewTransform,
        )?;
        let projection = self.projection.matrix(physical_width, physical_height)?;
        let view_projection = Matrix4F32::try_from_glam(
            projection.to_glam() * view.to_glam(),
            Math3DError::NonRepresentableViewTransform,
        )?;
        Ok(PreparedViewMatrices3 {
            view,
            projection,
            view_projection,
        })
    }

    pub(crate) fn ray_for_committed_extent(
        self,
        physical_width: u32,
        physical_height: u32,
        u: f64,
        v: f64,
    ) -> Result<Ray3, Math3DError> {
        if !(u.is_finite() && v.is_finite() && (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v))
        {
            return Err(Math3DError::InvalidViewportCoordinate);
        }
        let (_, aspect) = checked_aspect_ratio(physical_width, physical_height)?;
        let x_ndc = 2.0 * u - 1.0;
        let y_ndc = 1.0 - 2.0 * v;
        let camera_to_world = self.pose.orientation.to_dquat();

        match self.projection {
            Projection3::Perspective(projection) => {
                let tan_half = (projection.vertical_fov_radians as f64 * 0.5).tan();
                let camera_direction =
                    DVec3::new(x_ndc * aspect * tan_half, y_ndc * tan_half, -1.0);
                let world_direction = camera_to_world * camera_direction;
                Ray3::try_new(
                    self.pose.translation,
                    WorldVector3::try_new(world_direction.x, world_direction.y, world_direction.z)?,
                )
            }
            Projection3::Orthographic(projection) => {
                let half_vertical = projection.vertical_span as f64 * 0.5;
                let camera_origin = DVec3::new(
                    x_ndc * aspect * half_vertical,
                    y_ndc * half_vertical,
                    -(projection.near as f64),
                );
                let world_origin = self
                    .pose
                    .translation
                    .checked_add(camera_to_world * camera_origin)?;
                let world_direction = camera_to_world * DVec3::NEG_Z;
                Ray3::try_new(
                    world_origin,
                    WorldVector3::try_new(world_direction.x, world_direction.y, world_direction.z)?,
                )
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Ray3 {
    origin: WorldPoint3,
    direction: WorldVector3,
}

impl Ray3 {
    pub fn try_new(origin: WorldPoint3, direction: WorldVector3) -> Result<Self, Math3DError> {
        Ok(Self {
            origin,
            direction: direction.normalized()?,
        })
    }

    pub const fn origin(self) -> WorldPoint3 {
        self.origin
    }

    pub const fn direction(self) -> WorldVector3 {
        self.direction
    }

    pub fn point_at(self, distance: f64) -> Result<WorldPoint3, Math3DError> {
        if !distance.is_finite() || distance < 0.0 {
            return Err(Math3DError::InvalidRayDistance);
        }
        self.origin
            .checked_add(self.direction.to_dvec3() * distance)
    }

    pub fn to_local(self, placement: ModelPlacement3) -> Result<LocalRay3, Math3DError> {
        let inverse_orientation = placement.orientation.to_dquat().conjugate();
        let inverse_scale = placement.scale.reciprocal_dvec3();
        let translated_origin = checked_world_difference(self.origin, placement.translation)?;
        let local_origin = (inverse_orientation * translated_origin) * inverse_scale;
        let raw_local_direction = (inverse_orientation * self.direction.to_dvec3()) * inverse_scale;
        let local_units_per_world_unit = raw_local_direction.length();
        if !local_origin.is_finite()
            || !raw_local_direction.is_finite()
            || !local_units_per_world_unit.is_finite()
            || local_units_per_world_unit <= 0.0
        {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let direction = raw_local_direction / local_units_per_world_unit;
        let world_units_per_parameter = local_units_per_world_unit.recip();
        if !direction.is_finite()
            || !world_units_per_parameter.is_finite()
            || world_units_per_parameter <= 0.0
        {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(LocalRay3 {
            origin: local_origin.to_array(),
            direction: direction.to_array(),
            local_units_per_world_unit,
            world_units_per_parameter,
        })
    }

    pub fn intersect_plane(
        self,
        point: WorldPoint3,
        normal: WorldVector3,
    ) -> Result<Option<RayHit3>, Math3DError> {
        self.arithmetic()
            .intersect_plane(point.to_dvec3(), normal.to_dvec3())
    }

    pub fn intersect_sphere(
        self,
        center: WorldPoint3,
        radius: f64,
    ) -> Result<Option<RayHit3>, Math3DError> {
        self.arithmetic()
            .intersect_sphere(center.to_dvec3(), radius)
    }

    pub fn intersect_aabb(
        self,
        min: WorldPoint3,
        max: WorldPoint3,
    ) -> Result<Option<RayIntervalHit3>, Math3DError> {
        self.arithmetic()
            .intersect_aabb(min.to_dvec3(), max.to_dvec3())
    }

    pub fn intersect_triangle(
        self,
        a: WorldPoint3,
        b: WorldPoint3,
        c: WorldPoint3,
    ) -> Result<Option<TriangleHit3>, Math3DError> {
        self.arithmetic()
            .intersect_triangle(a.to_dvec3(), b.to_dvec3(), c.to_dvec3())
    }

    fn arithmetic(self) -> ArithmeticRay {
        ArithmeticRay {
            origin: self.origin.to_dvec3(),
            direction: self.direction.to_dvec3(),
            world_units_per_parameter: 1.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct LocalRay3 {
    origin: [f64; 3],
    direction: [f64; 3],
    local_units_per_world_unit: f64,
    world_units_per_parameter: f64,
}

impl LocalRay3 {
    pub const fn origin(self) -> [f64; 3] {
        self.origin
    }

    pub const fn direction(self) -> [f64; 3] {
        self.direction
    }

    pub const fn local_units_per_world_unit(self) -> f64 {
        self.local_units_per_world_unit
    }

    pub fn intersect_plane(
        self,
        point: LocalPoint3,
        normal: LocalVector3,
    ) -> Result<Option<RayHit3>, Math3DError> {
        self.arithmetic()
            .intersect_plane(point.to_dvec3(), normal.to_dvec3())
    }

    pub fn intersect_sphere(
        self,
        center: LocalPoint3,
        radius: f32,
    ) -> Result<Option<RayHit3>, Math3DError> {
        self.arithmetic()
            .intersect_sphere(center.to_dvec3(), radius as f64)
    }

    pub fn intersect_aabb(
        self,
        min: LocalPoint3,
        max: LocalPoint3,
    ) -> Result<Option<RayIntervalHit3>, Math3DError> {
        self.arithmetic()
            .intersect_aabb(min.to_dvec3(), max.to_dvec3())
    }

    pub fn intersect_triangle(
        self,
        a: LocalPoint3,
        b: LocalPoint3,
        c: LocalPoint3,
    ) -> Result<Option<TriangleHit3>, Math3DError> {
        self.arithmetic()
            .intersect_triangle(a.to_dvec3(), b.to_dvec3(), c.to_dvec3())
    }

    fn arithmetic(self) -> ArithmeticRay {
        ArithmeticRay {
            origin: DVec3::from_array(self.origin),
            direction: DVec3::from_array(self.direction),
            world_units_per_parameter: self.world_units_per_parameter,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayHit3 {
    ray_parameter: f64,
    world_distance: f64,
}

impl RayHit3 {
    pub const fn ray_parameter(self) -> f64 {
        self.ray_parameter
    }

    pub const fn world_distance(self) -> f64 {
        self.world_distance
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RayIntervalHit3 {
    entry: RayHit3,
    exit: RayHit3,
}

impl RayIntervalHit3 {
    pub const fn entry(self) -> RayHit3 {
        self.entry
    }

    pub const fn exit(self) -> RayHit3 {
        self.exit
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TriangleHit3 {
    hit: RayHit3,
    barycentric: [f64; 3],
    front_facing: bool,
}

impl TriangleHit3 {
    pub const fn hit(self) -> RayHit3 {
        self.hit
    }

    pub const fn barycentric(self) -> [f64; 3] {
        self.barycentric
    }

    pub const fn is_front_facing(self) -> bool {
        self.front_facing
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct ArithmeticRay {
    origin: DVec3,
    direction: DVec3,
    world_units_per_parameter: f64,
}

// Higham's gamma(n) with a conservative 32-operation budget for the bounded
// normalization, cross-product, and dot-product chains below. Every tolerance
// is multiplied by the relevant operand scale; this is not a world-space epsilon.
const INTERSECTION_ROUNDOFF: f64 = (32.0 * f64::EPSILON) / (1.0 - 32.0 * f64::EPSILON);

#[derive(Debug, Clone, Copy)]
struct NormalizedDirection {
    unit: DVec3,
    maximum_component: f64,
    scaled_length: f64,
}

impl NormalizedDirection {
    fn try_from(direction: DVec3) -> Result<Self, Math3DError> {
        let maximum_component = maximum_absolute_component(direction);
        if !maximum_component.is_finite() || maximum_component <= 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let scaled = direction / maximum_component;
        let scaled_length = scaled.length();
        if !scaled_length.is_finite() || scaled_length <= 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let unit = scaled / scaled_length;
        if !unit.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(Self {
            unit,
            maximum_component,
            scaled_length,
        })
    }

    fn parameter_for_unit_distance(self, distance: f64) -> Result<f64, Math3DError> {
        if !distance.is_finite() || distance < 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let parameter = (distance / self.scaled_length) / self.maximum_component;
        if !parameter.is_finite() || (distance > 0.0 && parameter == 0.0) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(parameter)
    }
}

fn maximum_absolute_component(vector: DVec3) -> f64 {
    vector.x.abs().max(vector.y.abs()).max(vector.z.abs())
}

fn normalized_for_intersection(vector: DVec3, invalid: Math3DError) -> Result<DVec3, Math3DError> {
    let maximum_component = maximum_absolute_component(vector);
    if !maximum_component.is_finite() || maximum_component <= 0.0 {
        return Err(invalid);
    }
    let scaled = vector / maximum_component;
    let length = scaled.length();
    if !length.is_finite() || length <= 0.0 {
        return Err(invalid);
    }
    let normalized = scaled / length;
    if !normalized.is_finite() {
        return Err(invalid);
    }
    Ok(normalized)
}

fn dot_roundoff_bound(left: DVec3, right: DVec3) -> f64 {
    let x = INTERSECTION_ROUNDOFF * left.x.abs() * right.x.abs();
    let y = INTERSECTION_ROUNDOFF * left.y.abs() * right.y.abs();
    let z = INTERSECTION_ROUNDOFF * left.z.abs() * right.z.abs();
    x + y + z
}

impl ArithmeticRay {
    fn hit(self, parameter: f64) -> Result<RayHit3, Math3DError> {
        if !parameter.is_finite() || parameter < 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let parameter = if parameter == 0.0 { 0.0 } else { parameter };
        let world_distance = parameter * self.world_units_per_parameter;
        if !world_distance.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        Ok(RayHit3 {
            ray_parameter: parameter,
            world_distance,
        })
    }

    fn intersect_plane(self, point: DVec3, normal: DVec3) -> Result<Option<RayHit3>, Math3DError> {
        let direction = NormalizedDirection::try_from(self.direction)?;
        let normal = normalized_for_intersection(normal, Math3DError::DegeneratePlane)?;
        let denominator = direction.unit.dot(normal);
        if !denominator.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        if denominator.abs() <= INTERSECTION_ROUNDOFF {
            return Ok(None);
        }
        let offset = point - self.origin;
        if !offset.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let numerator = offset.dot(normal);
        if !numerator.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let numerator_error = dot_roundoff_bound(offset, normal)
            + dot_roundoff_bound(point, normal)
            + dot_roundoff_bound(self.origin, normal);
        if !numerator_error.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let signed_numerator = numerator * denominator.signum();
        if signed_numerator < -numerator_error {
            return Ok(None);
        }
        let unit_distance = if signed_numerator <= numerator_error {
            0.0
        } else {
            let distance = numerator / denominator;
            if !distance.is_finite() || distance < 0.0 {
                return Err(Math3DError::ArithmeticOverflow);
            }
            distance
        };
        let parameter = direction.parameter_for_unit_distance(unit_distance)?;
        self.hit(parameter).map(Some)
    }

    fn intersect_sphere(self, center: DVec3, radius: f64) -> Result<Option<RayHit3>, Math3DError> {
        if !radius.is_finite() || radius <= 0.0 {
            return Err(Math3DError::InvalidSphereRadius);
        }
        let direction = NormalizedDirection::try_from(self.direction)?;
        let origin_to_center = self.origin - center;
        if !origin_to_center.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let geometry_scale = maximum_absolute_component(origin_to_center).max(radius);
        if !geometry_scale.is_finite() || geometry_scale <= 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let scaled_offset = origin_to_center / geometry_scale;
        let scaled_radius = radius / geometry_scale;
        let a = direction.unit.dot(direction.unit);
        let half_b = scaled_offset.dot(direction.unit);
        let c = scaled_offset.dot(scaled_offset) - scaled_radius * scaled_radius;
        let half_b_squared = half_b * half_b;
        let a_times_c = a * c;
        let discriminant = half_b_squared - a_times_c;
        if !(a.is_finite()
            && a > 0.0
            && half_b.is_finite()
            && c.is_finite()
            && discriminant.is_finite())
        {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let discriminant_error = INTERSECTION_ROUNDOFF * (half_b_squared.abs() + a_times_c.abs());
        if discriminant < -discriminant_error {
            return Ok(None);
        }
        let root = discriminant.max(0.0).sqrt();
        let q = -half_b - root.copysign(half_b);
        let (first, second) = if q == 0.0 {
            let repeated = -half_b / a;
            (repeated, repeated)
        } else {
            (q / a, c / q)
        };
        if !(first.is_finite() && second.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let near = first.min(second);
        let far = first.max(second);
        let parameter = if near >= 0.0 {
            near
        } else if far >= 0.0 {
            far
        } else {
            return Ok(None);
        };
        let unit_distance = parameter * geometry_scale;
        if !unit_distance.is_finite() || (parameter > 0.0 && unit_distance == 0.0) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let parameter = direction.parameter_for_unit_distance(unit_distance)?;
        self.hit(parameter).map(Some)
    }

    fn intersect_aabb(
        self,
        min: DVec3,
        max: DVec3,
    ) -> Result<Option<RayIntervalHit3>, Math3DError> {
        if min.x > max.x || min.y > max.y || min.z > max.z {
            return Err(Math3DError::InvalidAabb);
        }

        let mut entry = 0.0_f64;
        let mut exit = f64::INFINITY;
        for axis in 0..3 {
            let origin = self.origin[axis];
            let direction = self.direction[axis];
            let axis_min = min[axis];
            let axis_max = max[axis];
            if direction == 0.0 {
                if origin < axis_min || origin > axis_max {
                    return Ok(None);
                }
                continue;
            }

            let mut first = (axis_min - origin) / direction;
            let mut second = (axis_max - origin) / direction;
            if !(first.is_finite() && second.is_finite()) {
                return Err(Math3DError::ArithmeticOverflow);
            }
            if first > second {
                std::mem::swap(&mut first, &mut second);
            }
            entry = entry.max(first);
            exit = exit.min(second);
            if exit < entry {
                return Ok(None);
            }
        }

        if !exit.is_finite() || exit < 0.0 {
            return Ok(None);
        }
        let entry = self.hit(entry.max(0.0))?;
        let exit = self.hit(exit)?;
        Ok(Some(RayIntervalHit3 { entry, exit }))
    }

    fn intersect_triangle(
        self,
        a: DVec3,
        b: DVec3,
        c: DVec3,
    ) -> Result<Option<TriangleHit3>, Math3DError> {
        let direction = NormalizedDirection::try_from(self.direction)?;
        let edge_ab = b - a;
        let edge_ac = c - a;
        if !(edge_ab.is_finite() && edge_ac.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let geometry_scale =
            maximum_absolute_component(edge_ab).max(maximum_absolute_component(edge_ac));
        if !geometry_scale.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        if geometry_scale <= 0.0 {
            return Ok(None);
        }

        let edge_ab = edge_ab / geometry_scale;
        let edge_ac = edge_ac / geometry_scale;
        let edge_ab_length = edge_ab.length();
        let edge_ac_length = edge_ac.length();
        let geometric_normal = edge_ab.cross(edge_ac);
        let normal_length = geometric_normal.length();
        if !(edge_ab_length.is_finite() && edge_ac_length.is_finite() && normal_length.is_finite())
        {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let degeneracy_bound = INTERSECTION_ROUNDOFF * edge_ab_length * edge_ac_length;
        if normal_length <= degeneracy_bound {
            return Ok(None);
        }

        let p = direction.unit.cross(edge_ac);
        let determinant = edge_ab.dot(p);
        if !determinant.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let determinant_error =
            (INTERSECTION_ROUNDOFF * normal_length).max(dot_roundoff_bound(edge_ab, p));
        if determinant.abs() <= determinant_error {
            return Ok(None);
        }

        let inverse_determinant = determinant.recip();
        let from_a = (self.origin - a) / geometry_scale;
        if !from_a.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }

        let u_numerator = from_a.dot(p);
        if !u_numerator.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let q = from_a.cross(edge_ab);
        let v_numerator = direction.unit.dot(q);
        if !v_numerator.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let w_numerator = determinant - u_numerator - v_numerator;
        if !w_numerator.is_finite() {
            return Err(Math3DError::ArithmeticOverflow);
        }

        let u_error = dot_roundoff_bound(from_a, p);
        let v_error = dot_roundoff_bound(direction.unit, q);
        let w_error = determinant_error
            + u_error
            + v_error
            + INTERSECTION_ROUNDOFF * (determinant.abs() + u_numerator.abs() + v_numerator.abs());
        if !(u_error.is_finite() && v_error.is_finite() && w_error.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let orientation = determinant.signum();
        if u_numerator * orientation < -u_error
            || v_numerator * orientation < -v_error
            || w_numerator * orientation < -w_error
        {
            return Ok(None);
        }

        let distance_numerator = edge_ac.dot(q);
        let distance_error = dot_roundoff_bound(edge_ac, q);
        if !(distance_numerator.is_finite() && distance_error.is_finite()) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let signed_distance_numerator = distance_numerator * orientation;
        if signed_distance_numerator < -distance_error {
            return Ok(None);
        }
        let scaled_unit_distance = if signed_distance_numerator <= distance_error {
            0.0
        } else {
            let distance = distance_numerator * inverse_determinant;
            if !distance.is_finite() || distance < 0.0 {
                return Err(Math3DError::ArithmeticOverflow);
            }
            distance
        };
        let unit_distance = scaled_unit_distance * geometry_scale;
        if !unit_distance.is_finite() || (scaled_unit_distance > 0.0 && unit_distance == 0.0) {
            return Err(Math3DError::ArithmeticOverflow);
        }
        let parameter = direction.parameter_for_unit_distance(unit_distance)?;

        let mut barycentric = [
            (w_numerator * inverse_determinant).max(0.0),
            (u_numerator * inverse_determinant).max(0.0),
            (v_numerator * inverse_determinant).max(0.0),
        ];
        let barycentric_sum = barycentric.iter().sum::<f64>();
        if !barycentric_sum.is_finite() || barycentric_sum <= 0.0 {
            return Err(Math3DError::ArithmeticOverflow);
        }
        for coordinate in &mut barycentric {
            *coordinate /= barycentric_sum;
        }

        Ok(Some(TriangleHit3 {
            hit: self.hit(parameter)?,
            barycentric,
            front_facing: determinant > 0.0,
        }))
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Matrix3F32 {
    columns: [[f32; 3]; 3],
}

impl Matrix3F32 {
    fn try_from_glam(matrix: Mat3, error: Math3DError) -> Result<Self, Math3DError> {
        let columns = matrix.to_cols_array_2d();
        if !columns
            .iter()
            .flatten()
            .all(|component| component.is_finite())
        {
            return Err(error);
        }
        Ok(Self { columns })
    }

    pub(crate) const fn columns(self) -> [[f32; 3]; 3] {
        self.columns
    }

    #[cfg(test)]
    pub(crate) fn transform_vector(self, vector: [f32; 3]) -> [f32; 3] {
        let [x, y, z] = vector;
        [
            self.columns[0][0] * x + self.columns[1][0] * y + self.columns[2][0] * z,
            self.columns[0][1] * x + self.columns[1][1] * y + self.columns[2][1] * z,
            self.columns[0][2] * x + self.columns[1][2] * y + self.columns[2][2] * z,
        ]
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Matrix4F32 {
    columns: [[f32; 4]; 4],
}

impl Matrix4F32 {
    fn try_from_glam(matrix: Mat4, error: Math3DError) -> Result<Self, Math3DError> {
        let columns = matrix.to_cols_array_2d();
        if !columns
            .iter()
            .flatten()
            .all(|component| component.is_finite())
        {
            return Err(error);
        }
        Ok(Self { columns })
    }

    pub(crate) const fn columns(self) -> [[f32; 4]; 4] {
        self.columns
    }

    #[cfg(test)]
    pub(crate) fn transform_homogeneous(self, point: [f32; 3]) -> [f32; 4] {
        let [x, y, z] = point;
        [
            self.columns[0][0] * x
                + self.columns[1][0] * y
                + self.columns[2][0] * z
                + self.columns[3][0],
            self.columns[0][1] * x
                + self.columns[1][1] * y
                + self.columns[2][1] * z
                + self.columns[3][1],
            self.columns[0][2] * x
                + self.columns[1][2] * y
                + self.columns[2][2] * z
                + self.columns[3][2],
            self.columns[0][3] * x
                + self.columns[1][3] * y
                + self.columns[2][3] * z
                + self.columns[3][3],
        ]
    }

    fn to_glam(self) -> Mat4 {
        Mat4::from_cols_array_2d(&self.columns)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct CheckedModelTransform3 {
    model_to_render: Matrix4F32,
    linear_to_world: Matrix3F32,
    normal_to_world: Matrix3F32,
    determinant: f32,
}

impl CheckedModelTransform3 {
    pub(crate) const fn model_to_render(self) -> Matrix4F32 {
        self.model_to_render
    }

    #[cfg(test)]
    pub(crate) const fn linear_to_world(self) -> Matrix3F32 {
        self.linear_to_world
    }

    pub(crate) const fn normal_to_world(self) -> Matrix3F32 {
        self.normal_to_world
    }

    #[cfg(test)]
    pub(crate) const fn determinant(self) -> f32 {
        self.determinant
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PreparedViewMatrices3 {
    view: Matrix4F32,
    projection: Matrix4F32,
    view_projection: Matrix4F32,
}

impl PreparedViewMatrices3 {
    #[cfg(test)]
    pub(crate) const fn view(self) -> Matrix4F32 {
        self.view
    }

    #[cfg(test)]
    pub(crate) const fn projection(self) -> Matrix4F32 {
        self.projection
    }

    pub(crate) const fn view_projection(self) -> Matrix4F32 {
        self.view_projection
    }
}

fn checked_world_difference(point: WorldPoint3, origin: WorldPoint3) -> Result<DVec3, Math3DError> {
    let difference = point.to_dvec3() - origin.to_dvec3();
    if !difference.is_finite() {
        return Err(Math3DError::ArithmeticOverflow);
    }
    Ok(difference)
}

fn checked_f32(value: f64, error: Math3DError) -> Result<f32, Math3DError> {
    if !value.is_finite() || value < -(f32::MAX as f64) || value > f32::MAX as f64 {
        return Err(error);
    }
    let narrowed = value as f32;
    if !narrowed.is_finite() || (value != 0.0 && narrowed == 0.0) {
        return Err(error);
    }
    Ok(narrowed)
}

fn checked_aspect_ratio(
    physical_width: u32,
    physical_height: u32,
) -> Result<(f32, f64), Math3DError> {
    if physical_width == 0 || physical_height == 0 {
        return Err(Math3DError::InvalidPhysicalExtent);
    }
    let aspect = physical_width as f64 / physical_height as f64;
    let aspect_f32 = checked_f32(aspect, Math3DError::NonRepresentableProjection)?;
    if aspect_f32 <= 0.0 {
        return Err(Math3DError::NonRepresentableProjection);
    }
    Ok((aspect_f32, aspect))
}

fn perspective_matrix(projection: PerspectiveProjection, aspect: f32) -> Result<Mat4, Math3DError> {
    let focal = checked_f32(
        1.0 / (projection.vertical_fov_radians as f64 * 0.5).tan(),
        Math3DError::NonRepresentableProjection,
    )?;
    let x_scale = checked_f32(
        focal as f64 / aspect as f64,
        Math3DError::NonRepresentableProjection,
    )?;
    let near = projection.near as f64;
    let (depth_scale, depth_translation) = match projection.far {
        PerspectiveFar::Finite(far) => {
            let far = far as f64;
            let range = far - near;
            (
                checked_f32(near / range, Math3DError::NonRepresentableProjection)?,
                checked_f32(near * far / range, Math3DError::NonRepresentableProjection)?,
            )
        }
        PerspectiveFar::Infinite => (0.0, projection.near),
    };

    Ok(Mat4::from_cols(
        Vec4::new(x_scale, 0.0, 0.0, 0.0),
        Vec4::new(0.0, focal, 0.0, 0.0),
        Vec4::new(0.0, 0.0, depth_scale, -1.0),
        Vec4::new(0.0, 0.0, depth_translation, 0.0),
    ))
}

fn orthographic_matrix(
    projection: OrthographicProjection,
    aspect: f32,
) -> Result<Mat4, Math3DError> {
    let y_scale = checked_f32(
        2.0 / projection.vertical_span as f64,
        Math3DError::NonRepresentableProjection,
    )?;
    let x_scale = checked_f32(
        y_scale as f64 / aspect as f64,
        Math3DError::NonRepresentableProjection,
    )?;
    let range = projection.far as f64 - projection.near as f64;
    let depth_scale = checked_f32(1.0 / range, Math3DError::NonRepresentableProjection)?;
    let depth_translation = checked_f32(
        projection.far as f64 / range,
        Math3DError::NonRepresentableProjection,
    )?;

    Ok(Mat4::from_cols(
        Vec4::new(x_scale, 0.0, 0.0, 0.0),
        Vec4::new(0.0, y_scale, 0.0, 0.0),
        Vec4::new(0.0, 0.0, depth_scale, 0.0),
        Vec4::new(0.0, 0.0, depth_translation, 1.0),
    ))
}
