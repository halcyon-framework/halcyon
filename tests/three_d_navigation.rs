use halcyon::core::event::{KeyCode, KeyEvent, KeyboardAuthorityMode, Modifiers, PointerButton};
use halcyon::three_d::{
    CameraNavigationController3, CameraNavigationMode3, CameraNavigationPolicy3,
    CameraNavigationPolicyDescriptor3, CameraPointerBindings3, CameraPointerOperation3,
    CameraState3, CameraWheelOperation3, FlyKeyBindings3, FlyMotionOperation3,
    OrthographicProjection, PerspectiveProjection, Projection3, WorldPoint3,
};

fn perspective() -> Projection3 {
    PerspectiveProjection::try_finite(55.0_f32.to_radians(), 0.1, 100.0)
        .unwrap()
        .into()
}

fn orthographic() -> Projection3 {
    OrthographicProjection::try_new(5.0, 0.1, 100.0)
        .unwrap()
        .into()
}

fn state(projection: Projection3) -> CameraState3 {
    CameraState3::try_orbit(
        WorldPoint3::try_new(0.0, 0.0, -3.0).unwrap(),
        0.0,
        0.0,
        3.8,
        projection,
    )
    .unwrap()
}

fn controller(mode: CameraNavigationMode3) -> CameraNavigationController3 {
    CameraNavigationController3::try_new(
        state(perspective()),
        CameraNavigationPolicy3::default(),
        mode,
    )
    .unwrap()
}

fn key(key_code: KeyCode, is_down: bool, is_repeat: bool) -> KeyEvent {
    KeyEvent {
        key_code,

        physical_key: halcyon::core::event::PhysicalKeyCode::Unknown,
        is_down,
        is_repeat,
        modifiers: Modifiers::default(),
        text: None,
    }
}

fn distance(a: WorldPoint3, b: WorldPoint3) -> f64 {
    ((a.x() - b.x()).powi(2) + (a.y() - b.y()).powi(2) + (a.z() - b.z()).powi(2)).sqrt()
}

fn perspective_tangent(state: CameraState3) -> f64 {
    let Projection3::Perspective(projection) = state.projection() else {
        panic!("expected perspective projection");
    };
    (f64::from(projection.vertical_fov_radians()) * 0.5).tan()
}

fn identity_perspective_anchor_coordinate(state: CameraState3, anchor: WorldPoint3) -> (f64, f64) {
    let Projection3::Perspective(_) = state.projection() else {
        panic!("expected perspective projection");
    };
    let depth = state.position().z() - anchor.z();
    let tangent = perspective_tangent(state);
    (
        (anchor.x() - state.position().x()) / (depth * tangent),
        (anchor.y() - state.position().y()) / (depth * tangent),
    )
}

fn assert_point_close(actual: WorldPoint3, expected: WorldPoint3, tolerance: f64) {
    assert!((actual.x() - expected.x()).abs() <= tolerance);
    assert!((actual.y() - expected.y()).abs() <= tolerance);
    assert!((actual.z() - expected.z()).abs() <= tolerance);
}

#[test]
fn three_d_navigation_default_bindings_name_camera_operations_and_keep_fly_primary_unclaimed() {
    let policy = CameraNavigationPolicy3::default();
    let descriptor = policy.descriptor();
    let keys = policy.fly_keys();
    assert_eq!(descriptor.fly_units_per_second, 3.0);
    assert_eq!(descriptor.speed_modifier_multiplier, 4.0);
    assert_eq!(descriptor.fly_acceleration_response_per_second, 30.0);
    assert_eq!(descriptor.fly_deceleration_response_per_second, 40.0);
    assert_eq!(keys.forward(), KeyCode::Character('w'));
    assert_eq!(keys.back(), KeyCode::Character('s'));
    assert_eq!(keys.left(), KeyCode::Character('a'));
    assert_eq!(keys.right(), KeyCode::Character('d'));
    assert_eq!(keys.orbit_modifier(), KeyCode::Space);
    assert_eq!(keys.speed_modifier(), KeyCode::Shift);
    assert_eq!(
        keys.operation_for(KeyCode::Character('w')),
        Some(FlyMotionOperation3::Forward)
    );
    assert_eq!(
        keys.operation_for(KeyCode::Space),
        Some(FlyMotionOperation3::OrbitModifier)
    );
    assert_eq!(
        keys.operation_for(KeyCode::Shift),
        Some(FlyMotionOperation3::SpeedModifier)
    );
    assert_eq!(keys.operation_for(KeyCode::Enter), None);
    assert_eq!(keys.operation_for(KeyCode::Character('x')), None);

    let pointers = policy.pointer_bindings();
    assert_eq!(
        pointers.operation_for(CameraNavigationMode3::Orbit, PointerButton::Primary),
        Some(CameraPointerOperation3::OrbitRotate)
    );
    assert_eq!(
        pointers.operation_for(CameraNavigationMode3::Orbit, PointerButton::Auxiliary),
        Some(CameraPointerOperation3::CameraPan)
    );
    assert_eq!(
        pointers.operation_for(CameraNavigationMode3::Fly, PointerButton::Secondary),
        Some(CameraPointerOperation3::FreeLook)
    );
    assert_eq!(
        pointers.operation_for(CameraNavigationMode3::Fly, PointerButton::Auxiliary),
        Some(CameraPointerOperation3::CameraPan)
    );
    assert_eq!(
        pointers.operation_for(CameraNavigationMode3::Fly, PointerButton::Primary),
        None
    );

    assert!(
        CameraPointerBindings3::try_new(
            PointerButton::Primary,
            PointerButton::Secondary,
            PointerButton::Primary,
        )
        .is_err()
    );
    let custom = CameraPointerBindings3::try_new(
        PointerButton::Secondary,
        PointerButton::Primary,
        PointerButton::Auxiliary,
    )
    .unwrap();
    assert_eq!(
        custom.operation_for(CameraNavigationMode3::Fly, PointerButton::Primary),
        Some(CameraPointerOperation3::FreeLook)
    );
    assert_eq!(
        controller(CameraNavigationMode3::Orbit).wheel_operation(),
        CameraWheelOperation3::PerspectiveOpticalFovZoom
    );
    assert_eq!(
        CameraNavigationController3::try_new(
            state(orthographic()),
            CameraNavigationPolicy3::default(),
            CameraNavigationMode3::Orbit,
        )
        .unwrap()
        .wheel_operation(),
        CameraWheelOperation3::OrthographicSpanZoom
    );
}

#[test]
fn three_d_navigation_space_secondary_tethers_to_the_explicit_camera_orbit_target() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    let before = navigation.state();

    assert!(navigation.handle_key_event(
        &key(KeyCode::Space, true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(
        !navigation.needs_update(),
        "the tether modifier must not create vertical or other held translation"
    );
    assert!(!navigation.advance(0.1).unwrap());
    assert_eq!(navigation.state(), before);
    assert_eq!(
        navigation.pointer_operation_for_button(PointerButton::Secondary),
        Some(CameraPointerOperation3::OrbitRotate)
    );
    assert!(
        navigation
            .begin_pointer_operation(CameraPointerOperation3::OrbitRotate, (40.0, 40.0))
            .unwrap()
    );

    assert!(navigation.handle_key_event(
        &key(KeyCode::Space, false, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(navigation.pointer_moved((56.0, 30.0)).unwrap());
    assert_eq!(navigation.state().orbit_target(), before.orbit_target());
    assert_eq!(navigation.state().orbit_distance(), before.orbit_distance());
    assert_ne!(navigation.state().position(), before.position());
    assert!(navigation.end_pointer_operation());
}

#[test]
fn three_d_navigation_fly_look_capture_is_independent_from_held_motion() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    assert_eq!(
        navigation.pointer_operation_for_button(PointerButton::Primary),
        None
    );
    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(navigation.needs_update());

    let operation = navigation
        .pointer_operation_for_button(PointerButton::Secondary)
        .unwrap();
    assert!(
        navigation
            .begin_pointer_operation(operation, (40.0, 40.0))
            .unwrap()
    );
    assert!(navigation.pointer_moved((52.0, 34.0)).unwrap());
    assert!(navigation.end_pointer_operation());
    assert!(navigation.needs_update());
    assert_eq!(navigation.held_key_count(), 1);
    assert!(navigation.advance(0.05).unwrap());
}

#[test]
fn three_d_navigation_pointer_operations_are_mode_and_target_specific() {
    let mut orbit = controller(CameraNavigationMode3::Orbit);
    let orbit_state = orbit.state();
    assert_eq!(
        orbit.begin_pointer_operation(CameraPointerOperation3::FreeLook, (20.0, 20.0)),
        Err(halcyon::three_d::CameraNavigationError::InvalidPointerOperation)
    );
    assert_eq!(orbit.state(), orbit_state);
    assert_eq!(orbit.active_pointer_operation(), None);

    let mut fly = controller(CameraNavigationMode3::Fly);
    let fly_state = fly.state();
    assert_eq!(
        fly.begin_pointer_operation(CameraPointerOperation3::OrbitRotate, (20.0, 20.0)),
        Err(halcyon::three_d::CameraNavigationError::InvalidPointerOperation)
    );
    assert_eq!(fly.state(), fly_state);
    assert_eq!(fly.active_pointer_operation(), None);

    assert!(
        fly.begin_pointer_operation(CameraPointerOperation3::FreeLook, (20.0, 20.0))
            .unwrap()
    );
    assert!(fly.pointer_moved((28.0, 12.0)).unwrap());
    assert!(fly.end_pointer_operation());
    assert_eq!(fly.state().position(), fly_state.position());
    assert_ne!(fly.state().orbit_target(), fly_state.orbit_target());
}

#[test]
fn three_d_navigation_orbit_rotate_pan_and_optical_zoom_preserve_camera_only_ownership() {
    let source = include_str!("../src/three_d/navigation.rs");
    for forbidden_owner in [
        "ModelState",
        "RenderSnapshot",
        "Viewport3D",
        "EventContext",
        "Instant",
        "Timeline",
    ] {
        assert!(
            !source.contains(forbidden_owner),
            "navigation must not acquire {forbidden_owner} authority"
        );
    }

    let mut navigation = controller(CameraNavigationMode3::Orbit);
    let initial = navigation.state();
    navigation
        .begin_pointer_operation(CameraPointerOperation3::OrbitRotate, (100.0, 100.0))
        .unwrap();
    assert!(navigation.pointer_moved((124.0, 88.0)).unwrap());
    navigation.end_pointer_operation();
    assert_ne!(navigation.state().pose(), initial.pose());
    assert_eq!(navigation.state().orbit_target(), initial.orbit_target());

    let before_pan = navigation.state();
    navigation
        .begin_pointer_operation(CameraPointerOperation3::CameraPan, (50.0, 50.0))
        .unwrap();
    assert!(navigation.pointer_moved((70.0, 63.0)).unwrap());
    navigation.end_pointer_operation();
    assert_ne!(navigation.state().position(), before_pan.position());
    assert_ne!(navigation.state().orbit_target(), before_pan.orbit_target());
    assert!(
        (distance(
            navigation.state().position(),
            navigation.state().orbit_target()
        ) - navigation.state().orbit_distance())
        .abs()
            < 0.000_001
    );

    let before_zoom = navigation.state();
    assert!(navigation.scroll(1.0).unwrap());
    assert_eq!(navigation.state().position(), before_zoom.position());
    assert_eq!(navigation.state().orientation(), before_zoom.orientation());
    assert_eq!(
        navigation.state().orbit_target(),
        before_zoom.orbit_target()
    );
    assert_eq!(
        navigation.state().orbit_distance(),
        before_zoom.orbit_distance()
    );
    assert!(perspective_tangent(navigation.state()) < perspective_tangent(before_zoom));
}

#[test]
fn three_d_navigation_orthographic_wheel_changes_zoom_span_without_moving_pose() {
    let mut navigation = CameraNavigationController3::try_new(
        state(orthographic()),
        CameraNavigationPolicy3::default(),
        CameraNavigationMode3::Orbit,
    )
    .unwrap();
    let before = navigation.state();

    assert!(navigation.scroll(1.0).unwrap());
    assert_eq!(navigation.state().pose(), before.pose());
    let Projection3::Orthographic(after_projection) = navigation.state().projection() else {
        panic!("orthographic projection changed kind");
    };
    let Projection3::Orthographic(before_projection) = before.projection() else {
        unreachable!();
    };
    assert!(after_projection.vertical_span() < before_projection.vertical_span());
    assert_eq!(after_projection.near(), before_projection.near());
    assert_eq!(after_projection.far(), before_projection.far());
}

#[test]
fn three_d_navigation_logical_pixel_wheel_response_is_modest_and_proportional() {
    let mut one_line = controller(CameraNavigationMode3::Orbit);
    let mut two_half_lines = one_line.clone();
    let initial = one_line.state();
    let initial_tangent = perspective_tangent(initial);
    let descriptor = one_line.policy().descriptor();

    assert!(one_line.scroll(56.0).unwrap());
    assert!(two_half_lines.scroll(28.0).unwrap());
    assert!(two_half_lines.scroll(28.0).unwrap());
    let expected_ratio = (-56.0 * descriptor.zoom_exponent_per_logical_pixel).exp();
    let actual_ratio = perspective_tangent(one_line.state()) / initial_tangent;
    assert!((actual_ratio - expected_ratio).abs() < 0.000_001);
    assert!((0.88..=0.92).contains(&actual_ratio));
    assert!(
        (perspective_tangent(one_line.state()) - perspective_tangent(two_half_lines.state())).abs()
            < 0.000_001
    );
    assert_eq!(one_line.state().pose(), initial.pose());
    assert_eq!(one_line.state().orbit_target(), initial.orbit_target());
    assert_eq!(one_line.state().orbit_distance(), initial.orbit_distance());

    let mut extreme = controller(CameraNavigationMode3::Orbit);
    let mut admitted = extreme.clone();
    assert!(extreme.scroll(f32::MAX).unwrap());
    assert!(
        admitted
            .scroll(descriptor.maximum_scroll_delta_logical_pixels)
            .unwrap()
    );
    assert_eq!(extreme.state(), admitted.state());
}

#[test]
fn three_d_navigation_wheel_targets_compose_and_apply_without_clearing_transient_input() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    let operation = navigation
        .pointer_operation_for_button(PointerButton::Secondary)
        .unwrap();
    assert!(
        navigation
            .begin_pointer_operation(operation, (20.0, 30.0))
            .unwrap()
    );

    let base = navigation.state();
    let first = navigation.wheel_zoom_target(base, 28.0).unwrap().unwrap();
    let target = navigation.wheel_zoom_target(first, 28.0).unwrap().unwrap();
    let mut immediate = navigation.clone();
    assert!(immediate.scroll(28.0).unwrap());
    assert!(immediate.scroll(28.0).unwrap());
    assert_eq!(target, immediate.state());

    let middle = navigation
        .interpolate_wheel_zoom(base, target, 0.5)
        .unwrap();
    assert!(perspective_tangent(middle) < perspective_tangent(base));
    assert!(perspective_tangent(middle) > perspective_tangent(target));
    assert_eq!(middle.pose(), base.pose());
    assert_eq!(middle.orbit_target(), base.orbit_target());
    assert_eq!(middle.orbit_distance(), base.orbit_distance());
    assert!(navigation.apply_wheel_zoom_transition(middle).unwrap());
    assert_eq!(navigation.held_key_count(), 1);
    assert_eq!(navigation.active_pointer_operation(), Some(operation));
    assert!(navigation.apply_wheel_zoom_transition(target).unwrap());
    assert_eq!(navigation.state(), target);
}

#[test]
fn three_d_navigation_wheel_target_interpolation_preserves_perspective_geometry_and_exact_endpoint()
{
    let navigation = controller(CameraNavigationMode3::Orbit);
    let base = navigation.state();
    let target = navigation.wheel_zoom_target(base, 56.0).unwrap().unwrap();
    let ratio = perspective_tangent(target) / perspective_tangent(base);

    for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let sample = navigation
            .interpolate_wheel_zoom(base, target, progress)
            .unwrap();
        assert_eq!(sample.pose(), base.pose());
        assert_eq!(sample.orbit_target(), base.orbit_target());
        assert_eq!(sample.orbit_distance(), base.orbit_distance());
        let Projection3::Perspective(projection) = sample.projection() else {
            unreachable!();
        };
        let Projection3::Perspective(base_projection) = base.projection() else {
            unreachable!();
        };
        assert_eq!(projection.near(), base_projection.near());
        assert_eq!(projection.far(), base_projection.far());
        let expected_tangent = perspective_tangent(base) * ratio.powf(progress);
        assert!((perspective_tangent(sample) - expected_tangent).abs() < 0.000_001);
    }
    assert_eq!(
        navigation
            .interpolate_wheel_zoom(base, target, 0.0)
            .unwrap(),
        base
    );
    assert_eq!(
        navigation
            .interpolate_wheel_zoom(base, target, 1.0)
            .unwrap(),
        target
    );
}

#[test]
fn three_d_wheel_anchor_controller_preserves_geometric_anchor_without_dolly() {
    let mut navigation = controller(CameraNavigationMode3::Orbit);
    let base = navigation.state();
    let anchor = WorldPoint3::try_new(0.6, -0.3, -3.0).unwrap();
    let coordinate = identity_perspective_anchor_coordinate(base, anchor);
    let target = navigation
        .wheel_zoom_target_anchored(base, 56.0, anchor)
        .unwrap()
        .unwrap();

    assert_eq!(target.orientation(), base.orientation());
    assert_eq!(target.orbit_distance(), base.orbit_distance());
    assert_eq!(target.position().z(), base.position().z());
    assert_eq!(target.orbit_target().z(), base.orbit_target().z());
    assert_ne!(target.position(), base.position());
    assert_ne!(target.orbit_target(), base.orbit_target());

    for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let sample = navigation
            .interpolate_wheel_zoom(base, target, progress)
            .unwrap();
        let actual = identity_perspective_anchor_coordinate(sample, anchor);
        assert!((actual.0 - coordinate.0).abs() < 0.000_001);
        assert!((actual.1 - coordinate.1).abs() < 0.000_001);
    }

    let forward_translation = CameraState3::try_orbit(
        WorldPoint3::try_new(0.0, 0.0, -2.0).unwrap(),
        base.yaw(),
        base.pitch(),
        base.orbit_distance(),
        base.projection(),
    )
    .unwrap();
    assert!(
        navigation
            .apply_wheel_zoom_transition(forward_translation)
            .is_err()
    );
    let behind_camera = WorldPoint3::try_new(0.0, 0.0, base.position().z() + 1.0).unwrap();
    assert!(
        navigation
            .wheel_zoom_target_anchored(base, 56.0, behind_camera)
            .is_err()
    );
}

#[test]
fn three_d_wheel_anchor_outward_composition_restores_default_fov_and_original_pan() {
    let navigation = controller(CameraNavigationMode3::Orbit);
    let initial = navigation.state();
    let anchor = WorldPoint3::try_new(0.6, -0.3, -3.0).unwrap();
    let narrow = navigation
        .wheel_zoom_target_anchored(initial, 56.0, anchor)
        .unwrap()
        .unwrap();
    let restored = navigation
        .wheel_zoom_target_anchored(narrow, -f32::MAX, anchor)
        .unwrap()
        .unwrap();

    assert_eq!(restored.projection(), initial.projection());
    assert_eq!(restored.orientation(), initial.orientation());
    assert_eq!(restored.yaw(), initial.yaw());
    assert_eq!(restored.pitch(), initial.pitch());
    assert_eq!(restored.orbit_distance(), initial.orbit_distance());
    assert_point_close(restored.position(), initial.position(), 1.0e-12);
    assert_point_close(restored.orbit_target(), initial.orbit_target(), 1.0e-12);
    assert!(
        navigation
            .wheel_zoom_target_anchored(restored, -f32::MAX, anchor)
            .unwrap()
            .is_none()
    );
}

#[test]
fn three_d_navigation_wheel_target_interpolation_preserves_orthographic_pose_and_exact_endpoint() {
    let navigation = CameraNavigationController3::try_new(
        state(orthographic()),
        CameraNavigationPolicy3::default(),
        CameraNavigationMode3::Orbit,
    )
    .unwrap();
    let base = navigation.state();
    let target = navigation.wheel_zoom_target(base, 56.0).unwrap().unwrap();

    for progress in [0.0, 0.25, 0.5, 0.75, 1.0] {
        let sample = navigation
            .interpolate_wheel_zoom(base, target, progress)
            .unwrap();
        assert_eq!(sample.pose(), base.pose());
        let Projection3::Orthographic(projection) = sample.projection() else {
            unreachable!();
        };
        let Projection3::Orthographic(base_projection) = base.projection() else {
            unreachable!();
        };
        let Projection3::Orthographic(target_projection) = target.projection() else {
            unreachable!();
        };
        assert_eq!(projection.near(), base_projection.near());
        assert_eq!(projection.far(), base_projection.far());
        assert!(projection.vertical_span() <= base_projection.vertical_span());
        assert!(projection.vertical_span() >= target_projection.vertical_span());
    }
    assert_eq!(
        navigation
            .interpolate_wheel_zoom(base, target, 0.0)
            .unwrap(),
        base
    );
    assert_eq!(
        navigation
            .interpolate_wheel_zoom(base, target, 1.0)
            .unwrap(),
        target
    );
}

#[test]
fn three_d_navigation_perspective_optical_zoom_returns_only_to_its_initial_default_fov() {
    let mut descriptor = CameraNavigationPolicyDescriptor3::default();
    descriptor.minimum_perspective_fov_tangent_factor = 0.5;
    let policy = CameraNavigationPolicy3::try_new(descriptor).unwrap();
    let mut navigation = CameraNavigationController3::try_new(
        state(perspective()),
        policy,
        CameraNavigationMode3::Fly,
    )
    .unwrap();
    let initial = navigation.state();
    let (minimum, maximum) = navigation.perspective_fov_tangent_bounds().unwrap();
    assert!((minimum - perspective_tangent(initial) * 0.5).abs() < 0.000_001);
    assert!((maximum - perspective_tangent(initial)).abs() < 0.000_001);

    for _ in 0..32 {
        let _ = navigation.scroll(f32::MAX).unwrap();
    }
    assert!((perspective_tangent(navigation.state()) - minimum).abs() < 0.000_006);
    assert_eq!(navigation.state().pose(), initial.pose());
    assert_eq!(navigation.state().orbit_target(), initial.orbit_target());
    assert_eq!(
        navigation.state().orbit_distance(),
        initial.orbit_distance()
    );
    assert!(!navigation.scroll(f32::MAX).unwrap());

    assert!(navigation.set_mode(CameraNavigationMode3::Orbit));
    assert_eq!(
        navigation.perspective_fov_tangent_bounds(),
        Some((minimum, maximum))
    );
    for _ in 0..32 {
        let _ = navigation.scroll(-f32::MAX).unwrap();
    }
    assert!((perspective_tangent(navigation.state()) - maximum).abs() < 0.000_006);
    assert_eq!(navigation.state().pose(), initial.pose());
    assert_eq!(navigation.state().orbit_target(), initial.orbit_target());
    assert_eq!(
        navigation.state().orbit_distance(),
        initial.orbit_distance()
    );
    let Projection3::Perspective(projection) = navigation.state().projection() else {
        unreachable!();
    };
    let Projection3::Perspective(initial_projection) = initial.projection() else {
        unreachable!();
    };
    assert_eq!(projection.near(), initial_projection.near());
    assert_eq!(projection.far(), initial_projection.far());
    assert_eq!(navigation.state().projection(), initial.projection());
    assert!(!navigation.scroll(-f32::MAX).unwrap());
    assert!(navigation.state().position().x().is_finite());
    assert!(navigation.state().position().y().is_finite());
    assert!(navigation.state().position().z().is_finite());
}

#[test]
fn three_d_navigation_orthographic_zoom_saturates_and_rebases_on_projection_change() {
    let mut descriptor = CameraNavigationPolicyDescriptor3::default();
    descriptor.minimum_orthographic_span_factor = 0.5;
    descriptor.maximum_orthographic_span_factor = 2.0;
    let policy = CameraNavigationPolicy3::try_new(descriptor).unwrap();
    let mut navigation = CameraNavigationController3::try_new(
        state(orthographic()),
        policy,
        CameraNavigationMode3::Orbit,
    )
    .unwrap();
    assert_eq!(navigation.orthographic_span_bounds(), Some((2.5, 10.0)));
    assert_eq!(navigation.perspective_fov_tangent_bounds(), None);
    let pose = navigation.state().pose();

    for _ in 0..32 {
        let _ = navigation.scroll(f32::MAX).unwrap();
    }
    let Projection3::Orthographic(minimum) = navigation.state().projection() else {
        unreachable!();
    };
    assert_eq!(minimum.vertical_span(), 2.5);
    assert!(!navigation.scroll(f32::MAX).unwrap());

    for _ in 0..32 {
        let _ = navigation.scroll(-f32::MAX).unwrap();
    }
    let Projection3::Orthographic(maximum) = navigation.state().projection() else {
        unreachable!();
    };
    assert_eq!(maximum.vertical_span(), 10.0);
    assert!(!navigation.scroll(-f32::MAX).unwrap());
    assert_eq!(navigation.state().pose(), pose);

    assert!(navigation.set_projection(perspective()).unwrap());
    assert_eq!(navigation.orthographic_span_bounds(), None);
    assert!(navigation.perspective_fov_tangent_bounds().is_some());
    assert!(navigation.set_mode(CameraNavigationMode3::Fly));
    assert!(navigation.set_projection(orthographic()).unwrap());
    assert_eq!(navigation.orthographic_span_bounds(), Some((2.5, 10.0)));
    assert_eq!(navigation.perspective_fov_tangent_bounds(), None);
    assert_eq!(navigation.state().pose(), pose);
}

#[test]
fn three_d_navigation_held_fly_motion_is_cadence_independent() {
    let mut sixty_hz = controller(CameraNavigationMode3::Fly);
    let mut one_twenty_hz = sixty_hz.clone();
    let forward = key(KeyCode::Character('w'), true, false);
    assert!(sixty_hz.handle_key_event(&forward, KeyboardAuthorityMode::Capture));
    assert!(one_twenty_hz.handle_key_event(&forward, KeyboardAuthorityMode::Capture));

    for _ in 0..60 {
        assert!(sixty_hz.advance(1.0 / 60.0).unwrap());
    }
    for _ in 0..120 {
        assert!(one_twenty_hz.advance(1.0 / 120.0).unwrap());
    }

    assert_point_close(
        sixty_hz.state().position(),
        one_twenty_hz.state().position(),
        0.000_01,
    );
    assert_point_close(
        sixty_hz.state().orbit_target(),
        one_twenty_hz.state().orbit_target(),
        0.000_01,
    );
}

#[test]
fn three_d_navigation_key_repeat_never_adds_motion_and_key_up_decelerates_to_stop() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    let down = key(KeyCode::Character('w'), true, false);
    let repeat = key(KeyCode::Character('w'), true, true);
    let up = key(KeyCode::Character('w'), false, false);

    assert!(navigation.handle_key_event(&down, KeyboardAuthorityMode::Capture));
    assert_eq!(navigation.held_key_count(), 1);
    let before_repeat = navigation.state();
    for _ in 0..30 {
        assert!(navigation.handle_key_event(&repeat, KeyboardAuthorityMode::Capture));
    }
    assert_eq!(navigation.state(), before_repeat);
    assert_eq!(navigation.held_key_count(), 1);

    assert!(navigation.advance(0.05).unwrap());
    assert!(navigation.handle_key_event(&up, KeyboardAuthorityMode::Capture));
    assert!(navigation.needs_update());
    let released = navigation.state();
    assert!(navigation.advance(0.05).unwrap());
    assert_ne!(navigation.state(), released);
    for _ in 0..30 {
        let _ = navigation.advance(1.0 / 60.0).unwrap();
    }
    assert!(!navigation.needs_update());
    let stopped = navigation.state();
    assert!(!navigation.advance(0.05).unwrap());
    assert_eq!(navigation.state(), stopped);
}

#[test]
fn three_d_navigation_capture_cancel_clears_held_keys_and_pointer_gesture() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('d'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    navigation
        .begin_pointer_operation(CameraPointerOperation3::FreeLook, (20.0, 20.0))
        .unwrap();
    assert!(
        !navigation
            .begin_pointer_operation(CameraPointerOperation3::CameraPan, (21.0, 21.0))
            .unwrap()
    );
    assert_eq!(
        navigation.active_pointer_operation(),
        Some(CameraPointerOperation3::FreeLook)
    );
    assert!(navigation.needs_update());
    assert!(navigation.active_pointer_operation().is_some());
    assert!(navigation.advance(0.05).unwrap());

    assert!(navigation.cancel_input());
    assert!(!navigation.needs_update());
    assert_eq!(navigation.held_key_count(), 0);
    assert_eq!(navigation.active_pointer_operation(), None);
    let stopped = navigation.state();
    assert!(!navigation.advance(0.05).unwrap());
    assert_eq!(navigation.state(), stopped);
}

#[test]
fn three_d_navigation_reset_and_mode_change_cancel_residual_fly_velocity() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    let initial = navigation.state();
    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(navigation.advance(0.05).unwrap());
    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('w'), false, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(navigation.needs_update());

    assert!(navigation.reset(initial).unwrap());
    assert_eq!(navigation.state(), initial);
    assert!(!navigation.needs_update());
    assert!(!navigation.advance(0.05).unwrap());

    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(navigation.advance(0.05).unwrap());
    assert!(navigation.set_mode(CameraNavigationMode3::Orbit));
    let after_mode_change = navigation.state();
    assert!(!navigation.needs_update());
    assert!(!navigation.advance(0.05).unwrap());
    assert_eq!(navigation.state(), after_mode_change);
}

#[test]
fn three_d_navigation_mode_switch_preserves_pose_and_clears_transient_input() {
    let mut navigation = controller(CameraNavigationMode3::Orbit);
    navigation
        .begin_pointer_operation(CameraPointerOperation3::OrbitRotate, (10.0, 10.0))
        .unwrap();
    navigation.pointer_moved((30.0, 22.0)).unwrap();
    navigation.end_pointer_operation();
    let orbit_pose = navigation.state().pose();
    let projection = navigation.state().projection();

    assert!(navigation.set_mode(CameraNavigationMode3::Fly));
    assert_eq!(navigation.state().pose(), orbit_pose);
    assert_eq!(navigation.state().projection(), projection);
    assert!(navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(navigation.needs_update());

    let fly_pose = navigation.state().pose();
    assert!(navigation.set_mode(CameraNavigationMode3::Orbit));
    assert_eq!(navigation.state().pose(), fly_pose);
    assert_eq!(navigation.state().projection(), projection);
    assert!(!navigation.needs_update());
    assert_eq!(navigation.held_key_count(), 0);
}

#[test]
fn three_d_navigation_mode_unbound_keys_and_orbit_mode_do_not_leak() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    let initial = navigation.state();
    assert!(!navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Navigation,
    ));
    assert!(!navigation.handle_key_event(
        &key(KeyCode::Character('z'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert_eq!(navigation.state(), initial);
    assert!(!navigation.needs_update());

    navigation.set_mode(CameraNavigationMode3::Orbit);
    assert!(!navigation.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    assert!(!navigation.needs_update());
    assert_eq!(navigation.state(), initial);
}

#[test]
fn three_d_navigation_fly_diagonal_speed_is_normalized_and_modifier_is_bounded() {
    let mut forward = controller(CameraNavigationMode3::Fly);
    let mut diagonal = forward.clone();
    let mut boosted = forward.clone();
    let mut opposed = forward.clone();
    let start = forward.state().position();

    forward.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    diagonal.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    diagonal.handle_key_event(
        &key(KeyCode::Character('d'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    boosted.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    boosted.handle_key_event(
        &key(KeyCode::Shift, true, false),
        KeyboardAuthorityMode::Capture,
    );
    opposed.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    opposed.handle_key_event(
        &key(KeyCode::Character('s'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    assert!(!opposed.needs_update());
    assert!(!opposed.advance(0.1).unwrap());
    opposed.handle_key_event(
        &key(KeyCode::Character('s'), false, false),
        KeyboardAuthorityMode::Capture,
    );
    assert!(opposed.needs_update());

    forward.advance(0.1).unwrap();
    diagonal.advance(0.1).unwrap();
    boosted.advance(0.1).unwrap();
    let normal_distance = distance(start, forward.state().position());
    let diagonal_distance = distance(start, diagonal.state().position());
    let boosted_distance = distance(start, boosted.state().position());
    assert!((diagonal_distance - normal_distance).abs() < 0.000_001);
    assert!((boosted_distance / normal_distance - 4.0).abs() < 0.000_001);
}

#[test]
fn three_d_navigation_invalid_pointer_and_frame_inputs_are_transactional() {
    let mut navigation = controller(CameraNavigationMode3::Fly);
    let initial = navigation.state();
    assert!(
        navigation
            .begin_pointer_operation(CameraPointerOperation3::OrbitRotate, (f32::NAN, 0.0))
            .is_err()
    );
    assert_eq!(navigation.active_pointer_operation(), None);
    assert!(navigation.advance(f32::NAN).is_err());
    assert!(navigation.scroll(f32::INFINITY).is_err());
    assert_eq!(navigation.state(), initial);

    let mut invalid_policy = CameraNavigationPolicyDescriptor3::default();
    invalid_policy.fly_units_per_second = f64::NAN;
    assert!(CameraNavigationPolicy3::try_new(invalid_policy).is_err());
    let mut invalid_policy = CameraNavigationPolicyDescriptor3::default();
    invalid_policy.fly_acceleration_response_per_second = 0.0;
    assert!(CameraNavigationPolicy3::try_new(invalid_policy).is_err());
    let mut invalid_policy = CameraNavigationPolicyDescriptor3::default();
    invalid_policy.fly_deceleration_response_per_second = f64::NAN;
    assert!(CameraNavigationPolicy3::try_new(invalid_policy).is_err());
    let mut invalid_policy = CameraNavigationPolicyDescriptor3::default();
    invalid_policy.minimum_perspective_fov_tangent_factor = 1.01;
    assert!(CameraNavigationPolicy3::try_new(invalid_policy).is_err());
    let mut invalid_policy = CameraNavigationPolicyDescriptor3::default();
    invalid_policy.maximum_orthographic_span_factor = 0.99;
    assert!(CameraNavigationPolicy3::try_new(invalid_policy).is_err());
    let mut invalid_policy = CameraNavigationPolicyDescriptor3::default();
    invalid_policy.zoom_exponent_per_logical_pixel = f64::MAX;
    assert!(CameraNavigationPolicy3::try_new(invalid_policy).is_err());
    assert!(
        FlyKeyBindings3::try_new(
            KeyCode::Character('w'),
            KeyCode::Character('w'),
            KeyCode::Character('a'),
            KeyCode::Character('d'),
            KeyCode::Space,
            KeyCode::Shift,
        )
        .is_err()
    );
    assert!(CameraState3::try_orbit(WorldPoint3::origin(), 0.0, 0.0, 0.0, perspective(),).is_err());

    let mut bounded = controller(CameraNavigationMode3::Fly);
    let start = bounded.state().position();
    bounded.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    );
    assert!(bounded.advance(10.0).unwrap());
    let expected = 3.0 * 0.1 - 3.0 * (1.0 - (-3.0_f64).exp()) / 30.0;
    assert!((distance(start, bounded.state().position()) - expected).abs() < 0.000_001);
}

#[test]
fn three_d_navigation_bindings_are_mode_and_authority_correct() {
    let mut fly = controller(CameraNavigationMode3::Fly);
    assert_eq!(
        fly.pointer_operation_for_button(PointerButton::Primary),
        None,
        "Fly primary is reserved for the consumer's explicit transform policy"
    );
    assert_eq!(
        fly.pointer_operation_for_button(PointerButton::Secondary),
        Some(CameraPointerOperation3::FreeLook)
    );
    assert_eq!(
        fly.wheel_operation(),
        CameraWheelOperation3::PerspectiveOpticalFovZoom
    );
    assert!(!fly.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Navigation,
    ));
    assert!(fly.handle_key_event(
        &key(KeyCode::Character('w'), true, false),
        KeyboardAuthorityMode::Capture,
    ));
    let before = fly.state();
    assert!(fly.advance(0.1).unwrap());
    assert_ne!(fly.state().position(), before.position());

    assert!(fly.set_mode(CameraNavigationMode3::Orbit));
    assert_eq!(
        fly.pointer_operation_for_button(PointerButton::Primary),
        Some(CameraPointerOperation3::OrbitRotate)
    );
    assert_eq!(
        fly.pointer_operation_for_button(PointerButton::Secondary),
        None
    );
    assert_eq!(fly.held_key_count(), 0);
}
