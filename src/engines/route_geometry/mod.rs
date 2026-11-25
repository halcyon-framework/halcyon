// =============================================================================
// SPEC-HAL-338: Connector Route Geometry — Pure Waypoint Authority
// =============================================================================
//
// Extracted from src/widgets/timeline_canvas.rs per SPEC-HAL-338 §9.
// The engine owns waypoint computation for routed connectors: directed
// endpoints + geometric parameters → ordered route plan. Endpoint semantics,
// realization, styling, hit priority, and endpoint validity are consumer
// property (SPEC-HAL-338 §3).
//
// INV-338-G1: right-angle plan law (stub-out, H–V–H, terminal inset).
// INV-338-G2: backward targets double back; no loop-around for RightAngle.
// INV-338-G3: degenerate (zero-length) segments are legal and deterministic.
// INV-338-P1: no theme, scene, widget, or realization knowledge.
// INV-338-P2: pure, stateless, allocation-free; identical inputs →
//             identical plans.
// INV-338-P3: endpoint validity is the consumer's (INV-335-D4 pattern).
// FORBID-338-1..4: no realization API, no styling fields, no obstacle
//             input, no style arms ahead of their consumer.
// =============================================================================

use crate::core::math::Point;

/// Outward direction of the face a route departs from or arrives at
/// (SPEC-HAL-338 §4). Horizontal only: every specified consumer exits and
/// enters horizontally. A vertical variant requires a consumer SPEC.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RouteDirection {
    Left,
    Right,
}

impl RouteDirection {
    /// sign(Left) = −1.0, sign(Right) = +1.0 (SPEC-HAL-338 §5).
    pub fn sign(self) -> f32 {
        match self {
            RouteDirection::Left => -1.0,
            RouteDirection::Right => 1.0,
        }
    }

    pub fn opposite(self) -> Self {
        match self {
            RouteDirection::Left => RouteDirection::Right,
            RouteDirection::Right => RouteDirection::Left,
        }
    }
}

/// A directed route endpoint: where, and which way the face points outward.
/// The route departs the source along `dir` and arrives at the target
/// against `dir` (travel at arrival = opposite of the target's `dir`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteEndpoint {
    pub point: Point,
    pub dir: RouteDirection,
}

/// Geometric routing parameters. Lengths only; no visual meaning.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RouteParams {
    /// Minimum horizontal run out of the source face before the vertical
    /// transition.
    pub stub: f32,
    /// Terminal inset: the polyline stops this far short of the target
    /// anchor; the consumer's arrowhead realization occupies the gap.
    pub head_length: f32,
}

/// Routed right-angle plan. Fixed-size: zero allocation per route.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RightAnglePlan {
    /// Ordered waypoints: [exit, stub corner, transition corner,
    /// approach end]. Consecutive equal points are legal (INV-338-G3).
    pub points: [Point; 4],
    /// Arrowhead anchor: the target endpoint itself.
    pub head_anchor: Point,
    /// Arrowhead direction: travel at arrival = opposite of the target's
    /// `dir`.
    pub head_dir: RouteDirection,
}

/// INV-338-G1: the right-angle routing law.
///
/// ```text
/// p0 = from.point
/// p1 = (from.x + sign(from.dir)·stub, from.y)
/// p2 = (p1.x, to.y)
/// p3 = (to.x + sign(to.dir)·head_length, to.y)
/// head_anchor = to.point;  head_dir = opposite(to.dir)
/// ```
pub fn route_right_angle(
    from: RouteEndpoint,
    to: RouteEndpoint,
    params: RouteParams,
) -> RightAnglePlan {
    let p0 = from.point;
    let p1 = Point::new(from.point.x + from.dir.sign() * params.stub, from.point.y);
    let p2 = Point::new(p1.x, to.point.y);
    let p3 = Point::new(to.point.x + to.dir.sign() * params.head_length, to.point.y);
    RightAnglePlan {
        points: [p0, p1, p2, p3],
        head_anchor: to.point,
        head_dir: to.dir.opposite(),
    }
}
