use crate::three_d::{
    LocalPoint3, LocalVector3, Math3DError, ModelPlacement3, Orientation3, OrthographicProjection,
    PerspectiveFar, PerspectiveProjection, PositiveScale3, Projection3, Ray3, RigidPose3,
    ViewDescriptor3, WorldPoint3, WorldVector3,
};

const EPSILON_F32: f32 = 2.0e-5;
const EPSILON_F64: f64 = 5.0e-7;

fn world_point(x: f64, y: f64, z: f64) -> WorldPoint3 {
    WorldPoint3::try_new(x, y, z).unwrap()
}

fn world_vector(x: f64, y: f64, z: f64) -> WorldVector3 {
    WorldVector3::try_new(x, y, z).unwrap()
}

fn local_point(x: f32, y: f32, z: f32) -> LocalPoint3 {
    LocalPoint3::try_new(x, y, z).unwrap()
}

fn local_vector(x: f32, y: f32, z: f32) -> LocalVector3 {
    LocalVector3::try_new(x, y, z).unwrap()
}

fn assert_close_f32(actual: f32, expected: f32) {
    assert!(
        (actual - expected).abs() <= EPSILON_F32,
        "expected {expected}, got {actual}"
    );
}

fn assert_close_f64(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() <= EPSILON_F64,
        "expected {expected}, got {actual}"
    );
}

fn assert_relative_close_f64(actual: f64, expected: f64, tolerance: f64) {
    let scale = actual.abs().max(expected.abs()).max(f64::MIN_POSITIVE);
    assert!(
        (actual - expected).abs() <= tolerance * scale,
        "expected {expected}, got {actual}"
    );
}

fn assert_vec3_f32(actual: [f32; 3], expected: [f32; 3]) {
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert_close_f32(actual, expected);
    }
}

fn ndc(matrix: crate::three_d::math::Matrix4F32, point: [f32; 3]) -> [f32; 3] {
    let clip = matrix.transform_homogeneous(point);
    [clip[0] / clip[3], clip[1] / clip[3], clip[2] / clip[3]]
}

fn z_rotation(radians: f32) -> Orientation3 {
    let half = radians * 0.5;
    Orientation3::try_from_xyzw(0.0, 0.0, half.sin(), half.cos()).unwrap()
}

#[test]
fn role_based_scalar_values_reject_every_non_finite_boundary() {
    for invalid in [f64::NAN, f64::INFINITY, f64::NEG_INFINITY] {
        assert_eq!(
            WorldPoint3::try_new(invalid, 0.0, 0.0),
            Err(Math3DError::NonFiniteWorldPoint)
        );
        assert_eq!(
            WorldVector3::try_new(0.0, invalid, 0.0),
            Err(Math3DError::NonFiniteWorldVector)
        );
    }
    for invalid in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert_eq!(
            LocalPoint3::try_new(0.0, invalid, 0.0),
            Err(Math3DError::NonFiniteLocalPoint)
        );
        assert_eq!(
            LocalVector3::try_new(0.0, 0.0, invalid),
            Err(Math3DError::NonFiniteLocalVector)
        );
    }

    assert_eq!(
        world_vector(f64::MAX, f64::MAX, 0.0).try_length(),
        Err(Math3DError::ArithmeticOverflow)
    );
    assert_eq!(
        local_vector(f32::MAX, f32::MAX, 0.0).try_length(),
        Err(Math3DError::ArithmeticOverflow)
    );
}

#[test]
fn orientation_normalizes_finite_input_and_rejects_zero_or_non_finite_input() {
    let orientation = Orientation3::try_from_xyzw(0.0, 0.0, 10.0, 10.0).unwrap();
    let length = (orientation.x() as f64 * orientation.x() as f64
        + orientation.y() as f64 * orientation.y() as f64
        + orientation.z() as f64 * orientation.z() as f64
        + orientation.w() as f64 * orientation.w() as f64)
        .sqrt();
    assert_close_f64(length, 1.0);
    assert_close_f32(orientation.z(), std::f32::consts::FRAC_1_SQRT_2);
    assert_close_f32(orientation.w(), std::f32::consts::FRAC_1_SQRT_2);

    assert_eq!(
        Orientation3::try_from_xyzw(0.0, 0.0, 0.0, 0.0),
        Err(Math3DError::InvalidOrientation)
    );
    assert_eq!(
        Orientation3::try_from_xyzw(f32::NAN, 0.0, 0.0, 1.0),
        Err(Math3DError::InvalidOrientation)
    );
}

#[test]
fn three_d_orientation_composes_local_rotation_in_local_axes() {
    let half_turn = std::f32::consts::FRAC_PI_4;
    let rotate_x = Orientation3::try_from_xyzw(half_turn.sin(), 0.0, 0.0, half_turn.cos()).unwrap();
    let rotate_z = Orientation3::try_from_xyzw(0.0, 0.0, half_turn.sin(), half_turn.cos()).unwrap();

    let composed = rotate_x.compose_local(rotate_z).unwrap();
    assert_close_f32(composed.x(), 0.5);
    assert_close_f32(composed.y(), -0.5);
    assert_close_f32(composed.z(), 0.5);
    assert_close_f32(composed.w(), 0.5);

    let reversed = rotate_z.compose_local(rotate_x).unwrap();
    assert_close_f32(reversed.x(), 0.5);
    assert_close_f32(reversed.y(), 0.5);
    assert_close_f32(reversed.z(), 0.5);
    assert_close_f32(reversed.w(), 0.5);
}

#[test]
fn scale_admission_requires_positive_finite_reciprocal_and_determinant() {
    for scale in [
        PositiveScale3::try_new(0.0, 1.0, 1.0),
        PositiveScale3::try_new(-1.0, 1.0, 1.0),
        PositiveScale3::try_new(f32::INFINITY, 1.0, 1.0),
    ] {
        assert_eq!(scale, Err(Math3DError::InvalidScale));
    }

    assert_eq!(
        PositiveScale3::try_new(f32::from_bits(1), 1.0, 1.0),
        Err(Math3DError::NonRepresentableModelTransform)
    );
    assert_eq!(
        PositiveScale3::try_new(f32::MAX, f32::MAX, 1.0),
        Err(Math3DError::NonRepresentableModelTransform)
    );
    assert_eq!(
        PositiveScale3::try_new(f32::MIN_POSITIVE, f32::MIN_POSITIVE, 1.0),
        Err(Math3DError::NonRepresentableModelTransform)
    );

    let scale = PositiveScale3::try_new(2.0, 3.0, 4.0).unwrap();
    assert_eq!((scale.x(), scale.y(), scale.z()), (2.0, 3.0, 4.0));
    assert_eq!(scale.determinant(), 24.0);
}

#[test]
fn positive_non_uniform_trs_preserves_winding_and_uses_inverse_transpose_normals() {
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        z_rotation(std::f32::consts::FRAC_PI_2),
        PositiveScale3::try_new(2.0, 3.0, 4.0).unwrap(),
    )
    .unwrap();
    let transform = placement.checked_transform(WorldPoint3::origin()).unwrap();

    assert_eq!(transform.determinant(), 24.0);
    assert!(transform.determinant() > 0.0);
    assert_vec3_f32(
        transform
            .linear_to_world()
            .transform_vector([1.0, 0.0, 0.0]),
        [0.0, 2.0, 0.0],
    );
    assert_vec3_f32(
        transform
            .linear_to_world()
            .transform_vector([0.0, 1.0, 0.0]),
        [-3.0, 0.0, 0.0],
    );
    assert_vec3_f32(
        transform
            .normal_to_world()
            .transform_vector([1.0, 0.0, 0.0]),
        [0.0, 0.5, 0.0],
    );
    assert_vec3_f32(
        transform
            .normal_to_world()
            .transform_vector([0.0, 1.0, 0.0]),
        [-1.0 / 3.0, 0.0, 0.0],
    );

    let x = transform
        .linear_to_world()
        .transform_vector([1.0, 0.0, 0.0]);
    let y = transform
        .linear_to_world()
        .transform_vector([0.0, 1.0, 0.0]);
    let cross_z = x[0] * y[1] - x[1] * y[0];
    assert!(
        cross_z > 0.0,
        "positive determinant must preserve CCW winding"
    );
}

#[test]
fn model_matrix_contains_checked_render_relative_translation() {
    let base = 9_007_199_254_740_992.0;
    let placement = ModelPlacement3::try_new(
        world_point(base + 4096.0, -base + 8192.0, base - 16_384.0),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap();
    let origin = world_point(base, -base, base);
    let relative = placement.render_relative_translation(origin).unwrap();
    assert_eq!(
        (relative.x(), relative.y(), relative.z()),
        (4096.0, 8192.0, -16_384.0)
    );

    let matrix = placement
        .checked_transform(origin)
        .unwrap()
        .model_to_render();
    assert_vec3_f32(
        {
            let transformed = matrix.transform_homogeneous([0.0, 0.0, 0.0]);
            [transformed[0], transformed[1], transformed[2]]
        },
        [4096.0, 8192.0, -16_384.0],
    );
}

#[test]
fn render_origin_conversion_never_silently_overflows_or_narrows_range() {
    let too_large = world_point(f32::MAX as f64 * 2.0, 0.0, 0.0);
    assert_eq!(
        too_large.render_relative_to(WorldPoint3::origin()),
        Err(Math3DError::NonRepresentableRenderRelativeTranslation)
    );

    let maximum = world_point(f64::MAX, 0.0, 0.0);
    let minimum = world_point(-f64::MAX, 0.0, 0.0);
    assert_eq!(
        maximum.render_relative_to(minimum),
        Err(Math3DError::ArithmeticOverflow)
    );

    let underflow = world_point(f64::from_bits(1), 0.0, 0.0);
    assert_eq!(
        underflow.render_relative_to(WorldPoint3::origin()),
        Err(Math3DError::NonRepresentableRenderRelativeTranslation)
    );
}

#[test]
fn perspective_descriptor_rejects_every_invalid_boundary() {
    for fov in [0.0, -1.0, std::f32::consts::PI, f32::NAN] {
        assert_eq!(
            PerspectiveProjection::try_infinite(fov, 0.1),
            Err(Math3DError::InvalidPerspectiveFieldOfView)
        );
    }
    for near in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            PerspectiveProjection::try_infinite(1.0, near),
            Err(Math3DError::InvalidPerspectiveNear)
        );
    }
    for far in [1.0, 0.5, f32::INFINITY, f32::NAN] {
        assert_eq!(
            PerspectiveProjection::try_finite(1.0, 1.0, far),
            Err(Math3DError::InvalidPerspectiveFar)
        );
    }
    assert_eq!(
        PerspectiveProjection::try_infinite(f32::from_bits(1), 0.1),
        Err(Math3DError::NonRepresentableProjection)
    );
}

#[test]
fn orthographic_descriptor_rejects_every_invalid_boundary() {
    for span in [0.0, -1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            OrthographicProjection::try_new(span, 0.0, 10.0),
            Err(Math3DError::InvalidOrthographicSpan)
        );
    }
    for near in [-1.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            OrthographicProjection::try_new(2.0, near, 10.0),
            Err(Math3DError::InvalidOrthographicNear)
        );
    }
    for far in [1.0, 0.0, f32::INFINITY, f32::NAN] {
        assert_eq!(
            OrthographicProjection::try_new(2.0, 1.0, far),
            Err(Math3DError::InvalidOrthographicFar)
        );
    }
    assert_eq!(
        OrthographicProjection::try_new(1.0, 0.0, f32::from_bits(1)),
        Err(Math3DError::NonRepresentableProjection)
    );
    assert!(OrthographicProjection::try_new(2.0, 0.0, 10.0).is_ok());
}

#[test]
fn finite_perspective_matrix_maps_near_and_far_to_reverse_z_endpoints() {
    let projection = Projection3::from(
        PerspectiveProjection::try_finite(std::f32::consts::FRAC_PI_2, 0.5, 100.0).unwrap(),
    );
    let matrix = projection.matrix(1600, 900).unwrap();
    assert_close_f32(ndc(matrix, [0.0, 0.0, -0.5])[2], 1.0);
    assert_close_f32(ndc(matrix, [0.0, 0.0, -100.0])[2], 0.0);

    let distance = 7.0_f32;
    let expected = 0.5 * (100.0 - distance) / (distance * (100.0 - 0.5));
    assert_close_f32(ndc(matrix, [0.0, 0.0, -distance])[2], expected);
}

#[test]
fn infinite_perspective_matrix_maps_near_to_one_and_approaches_zero() {
    let projection = Projection3::from(
        PerspectiveProjection::try_infinite(std::f32::consts::FRAC_PI_2, 0.25).unwrap(),
    );
    let matrix = projection.matrix(1, 1).unwrap();
    assert_close_f32(ndc(matrix, [0.0, 0.0, -0.25])[2], 1.0);
    assert_close_f32(ndc(matrix, [0.0, 0.0, -1.0])[2], 0.25);
    assert_close_f32(ndc(matrix, [0.0, 0.0, -1000.0])[2], 0.00025);
}

#[test]
fn orthographic_matrix_maps_near_and_far_to_reverse_z_endpoints() {
    let projection = Projection3::from(OrthographicProjection::try_new(4.0, 2.0, 10.0).unwrap());
    let matrix = projection.matrix(2, 1).unwrap();
    assert_close_f32(ndc(matrix, [0.0, 0.0, -2.0])[2], 1.0);
    assert_close_f32(ndc(matrix, [0.0, 0.0, -10.0])[2], 0.0);
    assert_close_f32(ndc(matrix, [0.0, 0.0, -4.0])[2], 0.75);
    assert_close_f32(ndc(matrix, [4.0, 2.0, -4.0])[0], 1.0);
    assert_close_f32(ndc(matrix, [4.0, 2.0, -4.0])[1], 1.0);
}

#[test]
fn projection_rejects_zero_physical_extent() {
    let projection = Projection3::from(PerspectiveProjection::try_infinite(1.0, 0.1).unwrap());
    assert_eq!(
        projection.matrix(0, 100),
        Err(Math3DError::InvalidPhysicalExtent)
    );
    assert_eq!(
        projection.matrix(100, 0),
        Err(Math3DError::InvalidPhysicalExtent)
    );
}

#[test]
fn prepared_view_matrix_is_the_exact_inverse_of_rigid_camera_pose() {
    let pose = RigidPose3::new(
        world_point(1008.0, 2004.0, 2998.0),
        z_rotation(std::f32::consts::FRAC_PI_2),
    );
    let view = ViewDescriptor3::new(
        pose,
        PerspectiveProjection::try_infinite(1.0, 0.1)
            .unwrap()
            .into(),
    );
    let matrices = view
        .prepared_matrices(world_point(1000.0, 2000.0, 3000.0), 800, 600)
        .unwrap();
    let camera_relative = [8.0, 4.0, -2.0];
    let transformed = matrices.view().transform_homogeneous(camera_relative);
    assert_vec3_f32(
        [transformed[0], transformed[1], transformed[2]],
        [0.0, 0.0, 0.0],
    );
    assert!(
        matrices
            .view_projection()
            .columns()
            .iter()
            .flatten()
            .all(|value| value.is_finite())
    );
}

#[test]
fn perspective_analytic_ray_projects_back_to_the_same_viewport_coordinate() {
    let view = ViewDescriptor3::new(
        RigidPose3::new(WorldPoint3::origin(), Orientation3::identity()),
        PerspectiveProjection::try_finite(1.1, 0.1, 10_000.0)
            .unwrap()
            .into(),
    );
    let u = 0.73;
    let v = 0.19;
    let ray = view.ray_for_committed_extent(1920, 1080, u, v).unwrap();
    let point = ray.point_at(10.0).unwrap();
    let matrices = view
        .prepared_matrices(WorldPoint3::origin(), 1920, 1080)
        .unwrap();
    let projected = ndc(
        matrices.view_projection(),
        [point.x() as f32, point.y() as f32, point.z() as f32],
    );
    assert_close_f64((projected[0] as f64 + 1.0) * 0.5, u);
    assert_close_f64((1.0 - projected[1] as f64) * 0.5, v);
}

#[test]
fn infinite_perspective_center_ray_is_camera_forward_after_rotation() {
    let half_turn_y = Orientation3::try_from_xyzw(0.0, 1.0, 0.0, 0.0).unwrap();
    let view = ViewDescriptor3::new(
        RigidPose3::new(world_point(4.0, 5.0, 6.0), half_turn_y),
        PerspectiveProjection::try_infinite(1.0, 0.2)
            .unwrap()
            .into(),
    );
    let ray = view.ray_for_committed_extent(1200, 800, 0.5, 0.5).unwrap();
    assert_eq!(ray.origin(), world_point(4.0, 5.0, 6.0));
    assert_close_f64(ray.direction().x(), 0.0);
    assert_close_f64(ray.direction().y(), 0.0);
    assert_close_f64(ray.direction().z(), 1.0);
}

#[test]
fn infinite_perspective_analytic_ray_agrees_with_projection_off_center() {
    let view = ViewDescriptor3::new(
        RigidPose3::new(WorldPoint3::origin(), Orientation3::identity()),
        PerspectiveProjection::try_infinite(0.9, 0.05)
            .unwrap()
            .into(),
    );
    let u = 0.13;
    let v = 0.82;
    let ray = view.ray_for_committed_extent(3440, 1440, u, v).unwrap();
    let point = ray.point_at(100.0).unwrap();
    let matrix = view
        .prepared_matrices(WorldPoint3::origin(), 3440, 1440)
        .unwrap()
        .view_projection();
    let projected = ndc(
        matrix,
        [point.x() as f32, point.y() as f32, point.z() as f32],
    );
    assert_close_f64((projected[0] as f64 + 1.0) * 0.5, u);
    assert_close_f64((1.0 - projected[1] as f64) * 0.5, v);
}

#[test]
fn orthographic_rays_start_on_corresponding_near_points_and_remain_parallel() {
    let view = ViewDescriptor3::new(
        RigidPose3::new(world_point(10.0, 20.0, 30.0), Orientation3::identity()),
        OrthographicProjection::try_new(4.0, 2.0, 20.0)
            .unwrap()
            .into(),
    );
    let top_left = view.ray_for_committed_extent(800, 400, 0.0, 0.0).unwrap();
    let bottom_right = view.ray_for_committed_extent(800, 400, 1.0, 1.0).unwrap();
    assert_eq!(top_left.origin(), world_point(6.0, 22.0, 28.0));
    assert_eq!(bottom_right.origin(), world_point(14.0, 18.0, 28.0));
    assert_eq!(top_left.direction(), bottom_right.direction());
    assert_eq!(top_left.direction(), world_vector(0.0, 0.0, -1.0));
}

#[test]
fn orthographic_analytic_ray_agrees_with_projection_at_every_distance() {
    let view = ViewDescriptor3::new(
        RigidPose3::new(WorldPoint3::origin(), Orientation3::identity()),
        OrthographicProjection::try_new(6.0, 1.0, 100.0)
            .unwrap()
            .into(),
    );
    let u = 0.91;
    let v = 0.07;
    let ray = view.ray_for_committed_extent(800, 1200, u, v).unwrap();
    let point = ray.point_at(40.0).unwrap();
    let matrix = view
        .prepared_matrices(WorldPoint3::origin(), 800, 1200)
        .unwrap()
        .view_projection();
    let projected = ndc(
        matrix,
        [point.x() as f32, point.y() as f32, point.z() as f32],
    );
    assert_close_f64((projected[0] as f64 + 1.0) * 0.5, u);
    assert_close_f64((1.0 - projected[1] as f64) * 0.5, v);
}

#[test]
fn analytic_ray_rejects_uncommitted_shape_inputs_outside_math_domain() {
    let view = ViewDescriptor3::new(
        RigidPose3::new(WorldPoint3::origin(), Orientation3::identity()),
        PerspectiveProjection::try_infinite(1.0, 0.1)
            .unwrap()
            .into(),
    );
    assert_eq!(
        view.ray_for_committed_extent(0, 100, 0.5, 0.5),
        Err(Math3DError::InvalidPhysicalExtent)
    );
    for (u, v) in [(-0.1, 0.5), (1.1, 0.5), (0.5, f64::NAN)] {
        assert_eq!(
            view.ray_for_committed_extent(100, 100, u, v),
            Err(Math3DError::InvalidViewportCoordinate)
        );
    }
}

#[test]
fn ray_is_normalized_and_point_evaluation_is_checked() {
    let ray = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -10.0)).unwrap();
    assert_close_f64(ray.direction().try_length().unwrap(), 1.0);
    assert_eq!(ray.point_at(5.0).unwrap(), world_point(0.0, 0.0, -5.0));
    assert_eq!(
        Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, 0.0)),
        Err(Math3DError::InvalidRayDirection)
    );
    assert_eq!(ray.point_at(-1.0), Err(Math3DError::InvalidRayDistance));
    assert_eq!(
        ray.point_at(f64::INFINITY),
        Err(Math3DError::InvalidRayDistance)
    );

    let huge = Ray3::try_new(WorldPoint3::origin(), world_vector(f64::MAX, f64::MAX, 0.0)).unwrap();
    assert_close_f64(huge.direction().try_length().unwrap(), 1.0);
    assert_close_f64(huge.direction().x(), std::f64::consts::FRAC_1_SQRT_2);
    assert_close_f64(huge.direction().y(), std::f64::consts::FRAC_1_SQRT_2);
}

#[test]
fn plane_intersection_handles_hit_parallel_behind_and_degenerate_cases() {
    let ray = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -1.0)).unwrap();
    let hit = ray
        .intersect_plane(world_point(0.0, 0.0, -5.0), world_vector(0.0, 0.0, 2.0))
        .unwrap()
        .unwrap();
    assert_close_f64(hit.world_distance(), 5.0);
    assert_close_f64(
        ray.intersect_plane(
            world_point(0.0, 0.0, -5.0),
            world_vector(0.0, 0.0, f64::MAX),
        )
        .unwrap()
        .unwrap()
        .world_distance(),
        5.0,
    );
    assert!(
        ray.intersect_plane(world_point(0.0, 0.0, -5.0), world_vector(1.0, 0.0, 0.0))
            .unwrap()
            .is_none()
    );
    assert!(
        ray.intersect_plane(world_point(0.0, 0.0, 5.0), world_vector(0.0, 0.0, 1.0))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        ray.intersect_plane(world_point(0.0, 0.0, -5.0), world_vector(0.0, 0.0, 0.0)),
        Err(Math3DError::DegeneratePlane)
    );
}

#[test]
fn plane_intersection_rejects_unresolvable_parallelism_but_keeps_resolvable_angles() {
    let numerically_parallel =
        Ray3::try_new(WorldPoint3::origin(), world_vector(1.0, 0.0, -1.0e-16)).unwrap();
    assert!(
        numerically_parallel
            .intersect_plane(world_point(0.0, 0.0, -1.0), world_vector(0.0, 0.0, 1.0))
            .unwrap()
            .is_none()
    );

    let resolvable =
        Ray3::try_new(WorldPoint3::origin(), world_vector(1.0, 0.0, -1.0e-10)).unwrap();
    let hit = resolvable
        .intersect_plane(world_point(0.0, 0.0, -1.0), world_vector(0.0, 0.0, 1.0))
        .unwrap()
        .unwrap();
    assert_relative_close_f64(hit.world_distance(), 1.0e10, 1.0e-12);
}

#[test]
fn sphere_intersection_handles_near_tangent_inside_miss_and_invalid_radius() {
    let forward = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -1.0)).unwrap();
    assert_close_f64(
        forward
            .intersect_sphere(world_point(0.0, 0.0, -5.0), 1.0)
            .unwrap()
            .unwrap()
            .world_distance(),
        4.0,
    );
    assert_close_f64(
        forward
            .intersect_sphere(world_point(1.0, 0.0, -5.0), 1.0)
            .unwrap()
            .unwrap()
            .world_distance(),
        5.0,
    );
    assert_close_f64(
        forward
            .intersect_sphere(WorldPoint3::origin(), 2.0)
            .unwrap()
            .unwrap()
            .world_distance(),
        2.0,
    );
    assert!(
        forward
            .intersect_sphere(world_point(3.0, 0.0, -5.0), 1.0)
            .unwrap()
            .is_none()
    );
    for radius in [0.0, -1.0, f64::NAN] {
        assert_eq!(
            forward.intersect_sphere(WorldPoint3::origin(), radius),
            Err(Math3DError::InvalidSphereRadius)
        );
    }
}

#[test]
fn aabb_intersection_handles_entry_exit_inside_parallel_and_invalid_bounds() {
    let forward = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -1.0)).unwrap();
    let interval = forward
        .intersect_aabb(world_point(-1.0, -1.0, -6.0), world_point(1.0, 1.0, -4.0))
        .unwrap()
        .unwrap();
    assert_close_f64(interval.entry().world_distance(), 4.0);
    assert_close_f64(interval.exit().world_distance(), 6.0);

    let inside = Ray3::try_new(WorldPoint3::origin(), world_vector(1.0, 0.0, 0.0)).unwrap();
    let interval = inside
        .intersect_aabb(world_point(-1.0, -1.0, -1.0), world_point(1.0, 1.0, 1.0))
        .unwrap()
        .unwrap();
    assert_eq!(interval.entry().world_distance(), 0.0);
    assert_eq!(interval.exit().world_distance(), 1.0);

    let parallel_outside =
        Ray3::try_new(world_point(2.0, 0.0, 0.0), world_vector(0.0, 0.0, -1.0)).unwrap();
    assert!(
        parallel_outside
            .intersect_aabb(world_point(-1.0, -1.0, -6.0), world_point(1.0, 1.0, -4.0))
            .unwrap()
            .is_none()
    );
    assert_eq!(
        forward.intersect_aabb(world_point(1.0, 0.0, 0.0), world_point(-1.0, 1.0, 1.0)),
        Err(Math3DError::InvalidAabb)
    );
}

#[test]
fn triangle_intersection_is_two_sided_and_reports_edges_barycentrics_and_facing() {
    let ray = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -1.0)).unwrap();
    let a = world_point(-1.0, -1.0, -5.0);
    let b = world_point(1.0, -1.0, -5.0);
    let c = world_point(0.0, 1.0, -5.0);
    let hit = ray.intersect_triangle(a, b, c).unwrap().unwrap();
    assert_close_f64(hit.hit().world_distance(), 5.0);
    assert_eq!(hit.barycentric(), [0.25, 0.25, 0.5]);
    assert!(hit.is_front_facing());

    let back = ray.intersect_triangle(c, b, a).unwrap().unwrap();
    assert!(!back.is_front_facing());
    let edge_ray = Ray3::try_new(WorldPoint3::origin(), world_vector(-1.0, -1.0, -5.0)).unwrap();
    assert!(edge_ray.intersect_triangle(a, b, c).unwrap().is_some());
    assert!(ray.intersect_triangle(a, a, c).unwrap().is_none());
    assert!(
        ray.intersect_triangle(
            world_point(-1.0, -1.0, 5.0),
            world_point(1.0, -1.0, 5.0),
            world_point(0.0, 1.0, 5.0),
        )
        .unwrap()
        .is_none()
    );
}

#[test]
fn triangle_intersection_is_scale_invariant_for_tiny_and_huge_geometry() {
    let ray = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -1.0)).unwrap();
    for scale in [1.0e-200, 1.0e200] {
        let a = world_point(-scale, -scale, -5.0 * scale);
        let b = world_point(scale, -scale, -5.0 * scale);
        let c = world_point(0.0, scale, -5.0 * scale);
        let hit = ray.intersect_triangle(a, b, c).unwrap().unwrap();
        assert_relative_close_f64(hit.hit().world_distance(), 5.0 * scale, 1.0e-12);
        for (actual, expected) in hit.barycentric().into_iter().zip([0.25, 0.25, 0.5]) {
            assert_relative_close_f64(actual, expected, 1.0e-12);
        }

        let vertex_ray =
            Ray3::try_new(WorldPoint3::origin(), world_vector(a.x(), a.y(), a.z())).unwrap();
        let vertex_hit = vertex_ray.intersect_triangle(a, b, c).unwrap().unwrap();
        assert_relative_close_f64(vertex_hit.barycentric()[0], 1.0, 1.0e-12);
        assert!(vertex_hit.barycentric()[1] <= 1.0e-14);
        assert!(vertex_hit.barycentric()[2] <= 1.0e-14);
    }
}

#[test]
fn triangle_intersection_rejects_degenerate_and_numerically_parallel_cases() {
    let forward = Ray3::try_new(WorldPoint3::origin(), world_vector(0.0, 0.0, -1.0)).unwrap();
    assert!(
        forward
            .intersect_triangle(
                world_point(0.0, 0.0, -1.0),
                world_point(1.0, 0.0, -1.0),
                world_point(1.0, 1.0e-16, -1.0),
            )
            .unwrap()
            .is_none()
    );

    let a = world_point(-2.0e16, -2.0e16, -1.0);
    let b = world_point(2.0e16, -2.0e16, -1.0);
    let c = world_point(0.0, 2.0e16, -1.0);
    let numerically_parallel =
        Ray3::try_new(WorldPoint3::origin(), world_vector(1.0, 0.0, -1.0e-16)).unwrap();
    assert!(
        numerically_parallel
            .intersect_triangle(a, b, c)
            .unwrap()
            .is_none()
    );

    let resolvable =
        Ray3::try_new(WorldPoint3::origin(), world_vector(1.0, 0.0, -1.0e-10)).unwrap();
    let hit = resolvable.intersect_triangle(a, b, c).unwrap().unwrap();
    assert_relative_close_f64(hit.hit().world_distance(), 1.0e10, 1.0e-12);
}

#[test]
fn ray_to_local_preserves_world_distance_under_non_uniform_scale() {
    let placement = ModelPlacement3::try_new(
        world_point(100.0, 0.0, 0.0),
        Orientation3::identity(),
        PositiveScale3::try_new(2.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let world_ray =
        Ray3::try_new(world_point(90.0, 0.0, 0.0), world_vector(1.0, 0.0, 0.0)).unwrap();
    let local_ray = world_ray.to_local(placement).unwrap();
    assert_eq!(local_ray.origin(), [-5.0, 0.0, 0.0]);
    assert_eq!(local_ray.direction(), [1.0, 0.0, 0.0]);
    assert_eq!(local_ray.local_units_per_world_unit(), 0.5);

    let hit = local_ray
        .intersect_sphere(LocalPoint3::origin(), 1.0)
        .unwrap()
        .unwrap();
    assert_relative_close_f64(hit.ray_parameter(), 4.0, 1.0e-12);
    assert_relative_close_f64(hit.world_distance(), 8.0, 1.0e-12);
}

#[test]
fn oblique_non_uniform_local_sphere_hit_preserves_parameter_and_world_distance() {
    let sqrt_five = 5.0_f64.sqrt();
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(2.0, 1.0, 1.0).unwrap(),
    )
    .unwrap();
    let world_ray = Ray3::try_new(
        world_point(-2.0 * sqrt_five, -2.0 * sqrt_five, 0.0),
        world_vector(1.0, 1.0, 0.0),
    )
    .unwrap();
    let local_ray = world_ray.to_local(placement).unwrap();
    let hit = local_ray
        .intersect_sphere(LocalPoint3::origin(), 1.0)
        .unwrap()
        .unwrap();

    assert_relative_close_f64(hit.ray_parameter(), 4.0, 1.0e-12);
    assert_relative_close_f64(hit.world_distance(), 16.0 / 10.0_f64.sqrt(), 1.0e-12);
}

#[test]
fn ray_to_local_applies_inverse_orientation_before_scale() {
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        z_rotation(std::f32::consts::FRAC_PI_2),
        PositiveScale3::one(),
    )
    .unwrap();
    let world_ray =
        Ray3::try_new(world_point(0.0, -5.0, 0.0), world_vector(0.0, 1.0, 0.0)).unwrap();
    let local_ray = world_ray.to_local(placement).unwrap();
    for (actual, expected) in local_ray.origin().into_iter().zip([-5.0, 0.0, 0.0]) {
        assert_close_f64(actual, expected);
    }
    for (actual, expected) in local_ray.direction().into_iter().zip([1.0, 0.0, 0.0]) {
        assert_close_f64(actual, expected);
    }
}

#[test]
fn local_intersection_arithmetic_remains_f64_without_mesh_storage_narrowing() {
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap();
    let origin = 1_000_000_000_000.125_f64;
    let world_ray =
        Ray3::try_new(world_point(origin, 0.0, 0.0), world_vector(-1.0, 0.0, 0.0)).unwrap();
    let local_ray = world_ray.to_local(placement).unwrap();
    assert_eq!(local_ray.origin()[0], origin);

    let hit = local_ray
        .intersect_plane(LocalPoint3::origin(), local_vector(1.0, 0.0, 0.0))
        .unwrap()
        .unwrap();
    assert_eq!(hit.ray_parameter(), origin);
    assert_eq!(hit.world_distance(), origin);
}

#[test]
fn ray_to_local_rejects_f64_subtraction_overflow() {
    let placement = ModelPlacement3::try_new(
        world_point(-f64::MAX, 0.0, 0.0),
        Orientation3::identity(),
        PositiveScale3::one(),
    )
    .unwrap();
    let ray = Ray3::try_new(
        world_point(f64::MAX, 0.0, 0.0),
        world_vector(-1.0, 0.0, 0.0),
    )
    .unwrap();
    assert_eq!(
        ray.to_local(placement),
        Err(Math3DError::ArithmeticOverflow)
    );
}

#[test]
fn local_intersection_helpers_validate_degenerate_inputs_and_keep_distance_scale() {
    let placement = ModelPlacement3::try_new(
        WorldPoint3::origin(),
        Orientation3::identity(),
        PositiveScale3::try_new(1.0, 2.0, 1.0).unwrap(),
    )
    .unwrap();
    let local_ray = Ray3::try_new(world_point(0.0, 10.0, 0.0), world_vector(0.0, -1.0, 0.0))
        .unwrap()
        .to_local(placement)
        .unwrap();
    let hit = local_ray
        .intersect_plane(LocalPoint3::origin(), local_vector(0.0, 1.0, 0.0))
        .unwrap()
        .unwrap();
    assert_eq!(hit.ray_parameter(), 5.0);
    assert_eq!(hit.world_distance(), 10.0);
    assert_eq!(
        local_ray.intersect_plane(LocalPoint3::origin(), local_vector(0.0, 0.0, 0.0)),
        Err(Math3DError::DegeneratePlane)
    );
    assert_eq!(
        local_ray.intersect_sphere(LocalPoint3::origin(), 0.0),
        Err(Math3DError::InvalidSphereRadius)
    );
    assert_eq!(
        local_ray.intersect_aabb(local_point(1.0, 0.0, 0.0), local_point(-1.0, 1.0, 1.0)),
        Err(Math3DError::InvalidAabb)
    );
}

#[test]
fn standard_projection_keeps_projected_ccw_winding_without_y_reflection() {
    let projection = Projection3::from(
        PerspectiveProjection::try_infinite(std::f32::consts::FRAC_PI_2, 0.1).unwrap(),
    );
    let matrix = projection.matrix(1, 1).unwrap();
    let a = ndc(matrix, [-1.0, -1.0, -2.0]);
    let b = ndc(matrix, [1.0, -1.0, -2.0]);
    let c = ndc(matrix, [0.0, 1.0, -2.0]);
    let signed_area_twice = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
    assert!(signed_area_twice > 0.0);
}
