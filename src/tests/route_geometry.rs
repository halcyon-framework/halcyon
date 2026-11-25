// SPEC-HAL-338 invariant tests: right-angle route geometry
// (INV-338-G1 plan law, INV-338-G2 backward targets, INV-338-G3 degeneracy,
//  INV-338-P2 determinism).

use crate::core::math::Point;
use crate::engines::route_geometry::{
    RouteDirection, RouteEndpoint, RouteParams, route_right_angle,
};

const PARAMS: RouteParams = RouteParams {
    stub: 8.0,
    head_length: 5.0,
};

fn ep(x: f32, y: f32, dir: RouteDirection) -> RouteEndpoint {
    RouteEndpoint {
        point: Point::new(x, y),
        dir,
    }
}

#[test]
fn forward_finish_to_start() {
    // INV-338-G1: canonical End→Start dependency, target right and below.
    let plan = route_right_angle(
        ep(100.0, 50.0, RouteDirection::Right),
        ep(200.0, 80.0, RouteDirection::Left),
        PARAMS,
    );
    assert_eq!(plan.points[0], Point::new(100.0, 50.0));
    assert_eq!(plan.points[1], Point::new(108.0, 50.0));
    assert_eq!(plan.points[2], Point::new(108.0, 80.0));
    assert_eq!(plan.points[3], Point::new(195.0, 80.0));
    assert_eq!(plan.head_anchor, Point::new(200.0, 80.0));
    assert_eq!(plan.head_dir, RouteDirection::Right);
}

#[test]
fn left_exit_start_edge() {
    // INV-338-G1: Start-edge departure stubs leftward (sign(Left) = −1).
    let plan = route_right_angle(
        ep(100.0, 50.0, RouteDirection::Left),
        ep(60.0, 80.0, RouteDirection::Left),
        PARAMS,
    );
    assert_eq!(plan.points[1], Point::new(92.0, 50.0));
    assert_eq!(plan.points[3], Point::new(55.0, 80.0));
    assert_eq!(plan.head_dir, RouteDirection::Right);
}

#[test]
fn end_entry_head_points_left() {
    // INV-338-G1: End-edge arrival — approach overshoots rightward by the
    // terminal inset; head travels leftward into the face.
    let plan = route_right_angle(
        ep(100.0, 50.0, RouteDirection::Right),
        ep(300.0, 80.0, RouteDirection::Right),
        PARAMS,
    );
    assert_eq!(plan.points[3], Point::new(305.0, 80.0));
    assert_eq!(plan.head_anchor, Point::new(300.0, 80.0));
    assert_eq!(plan.head_dir, RouteDirection::Left);
}

#[test]
fn backward_target_doubles_back() {
    // INV-338-G2: target behind the source — the approach segment runs back
    // across the transition. No loop-around exists for RightAngle.
    let plan = route_right_angle(
        ep(200.0, 50.0, RouteDirection::Right),
        ep(100.0, 80.0, RouteDirection::Left),
        PARAMS,
    );
    assert_eq!(plan.points[1], Point::new(208.0, 50.0));
    assert_eq!(plan.points[2], Point::new(208.0, 80.0));
    assert_eq!(plan.points[3], Point::new(95.0, 80.0));
    assert!(plan.points[3].x < plan.points[2].x);
}

#[test]
fn same_row_degenerate_vertical() {
    // INV-338-G3: equal endpoint y — the vertical transition is zero-length
    // (p1 == p2); the plan is produced without special-casing.
    let plan = route_right_angle(
        ep(100.0, 50.0, RouteDirection::Right),
        ep(160.0, 50.0, RouteDirection::Left),
        PARAMS,
    );
    assert_eq!(plan.points[1], plan.points[2]);
    assert_eq!(plan.points[1], Point::new(108.0, 50.0));
    assert_eq!(plan.points[3], Point::new(155.0, 50.0));
}

#[test]
fn direction_sign_and_opposite() {
    assert_eq!(RouteDirection::Left.sign(), -1.0);
    assert_eq!(RouteDirection::Right.sign(), 1.0);
    assert_eq!(RouteDirection::Left.opposite(), RouteDirection::Right);
    assert_eq!(RouteDirection::Right.opposite(), RouteDirection::Left);
}

#[test]
fn determinism() {
    // INV-338-P2: identical inputs → identical plans.
    let a = route_right_angle(
        ep(10.0, 20.0, RouteDirection::Right),
        ep(300.0, 240.0, RouteDirection::Left),
        PARAMS,
    );
    let b = route_right_angle(
        ep(10.0, 20.0, RouteDirection::Right),
        ep(300.0, 240.0, RouteDirection::Left),
        PARAMS,
    );
    assert_eq!(a, b);
}
