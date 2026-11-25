// SPEC-HAL-296B §4: Force-Directed Routing Utility
//
// Consumer-side utility. Takes connection endpoints + obstacle rects,
// produces waypoints via force-directed relaxation.
//
// FORBID-296B-2: This MUST NOT run inside the render loop.
// Consumer calls it explicitly and caches results.

use crate::core::math::RectF;

/// Configuration for the force-directed router.
#[derive(Debug, Clone)]
pub struct PlotRouter {
    /// Repulsion strength from obstacles.
    pub element_repulsion: f32,
    /// Repulsion strength between connector midpoints.
    pub connector_repulsion: f32,
    /// Number of relaxation iterations.
    pub iterations: u32,
    /// Minimum clearance from obstacles (pixels).
    pub clearance: f32,
    /// Points inserted per connection (resolution).
    pub midpoint_spacing: f32,
}

impl Default for PlotRouter {
    fn default() -> Self {
        Self {
            element_repulsion: 500.0,
            connector_repulsion: 200.0,
            iterations: 50,
            clearance: 8.0,
            midpoint_spacing: 40.0,
        }
    }
}

impl PlotRouter {
    /// Route connections around obstacles.
    ///
    /// `connections`: pairs of (start, end) in screen pixels.
    /// `obstacles`: bounding rects of chart elements in screen pixels.
    ///
    /// Returns: one waypoint list per connection. Each list includes
    /// the start and end points plus computed intermediate waypoints.
    pub fn route(
        &self,
        connections: &[((f32, f32), (f32, f32))],
        obstacles: &[RectF],
    ) -> Vec<Vec<(f32, f32)>> {
        // Initialize: insert midpoints along each connection
        let mut paths: Vec<Vec<(f32, f32)>> = connections
            .iter()
            .map(|&(start, end)| {
                let dx = end.0 - start.0;
                let dy = end.1 - start.1;
                let len = (dx * dx + dy * dy).sqrt();
                let n = ((len / self.midpoint_spacing).ceil() as usize).max(2);

                let mut points = Vec::with_capacity(n + 1);
                for i in 0..=n {
                    let t = i as f32 / n as f32;
                    points.push((start.0 + dx * t, start.1 + dy * t));
                }
                points
            })
            .collect();

        // Store original positions for spring attraction
        let originals: Vec<Vec<(f32, f32)>> = paths.clone();

        // Relaxation iterations
        for _ in 0..self.iterations {
            // Collect all movable points (exclude start/end of each path)
            // and compute forces
            let mut forces: Vec<Vec<(f32, f32)>> =
                paths.iter().map(|p| vec![(0.0, 0.0); p.len()]).collect();

            for (pi, path) in paths.iter().enumerate() {
                for (qi, &(px, py)) in path.iter().enumerate() {
                    // Skip endpoints (not movable)
                    if qi == 0 || qi == path.len() - 1 {
                        continue;
                    }

                    let mut fx = 0.0_f32;
                    let mut fy = 0.0_f32;

                    // Repulsion from obstacles
                    for obs in obstacles {
                        let cx = (obs.x + obs.width / 2.0).max(obs.x).min(obs.x + obs.width);
                        let cy = (obs.y + obs.height / 2.0)
                            .max(obs.y)
                            .min(obs.y + obs.height);

                        // Closest point on rect to our point
                        let near_x = px.clamp(obs.x, obs.x + obs.width);
                        let near_y = py.clamp(obs.y, obs.y + obs.height);
                        let ddx = px - near_x;
                        let ddy = py - near_y;
                        let dist = (ddx * ddx + ddy * ddy).sqrt().max(1.0);

                        if dist < self.clearance * 3.0 {
                            let strength = self.element_repulsion / (dist * dist);
                            if dist < 0.01 {
                                // Inside or on the rect — push away from center
                                let to_center_x = px - cx;
                                let to_center_y = py - cy;
                                let tc_len = (to_center_x * to_center_x
                                    + to_center_y * to_center_y)
                                    .sqrt()
                                    .max(1.0);
                                fx += (to_center_x / tc_len) * strength;
                                fy += (to_center_y / tc_len) * strength;
                            } else {
                                fx += (ddx / dist) * strength;
                                fy += (ddy / dist) * strength;
                            }
                        }
                    }

                    // Repulsion from other paths' midpoints
                    for (oi, other) in paths.iter().enumerate() {
                        if oi == pi {
                            continue;
                        }
                        for &(ox, oy) in other.iter().skip(1) {
                            let ddx = px - ox;
                            let ddy = py - oy;
                            let dist = (ddx * ddx + ddy * ddy).sqrt().max(1.0);
                            if dist < self.clearance * 5.0 {
                                let strength = self.connector_repulsion / (dist * dist);
                                fx += (ddx / dist) * strength;
                                fy += (ddy / dist) * strength;
                            }
                        }
                    }

                    // Spring attraction toward original position
                    let (orig_x, orig_y) = originals[pi][qi];
                    fx += (orig_x - px) * 0.1;
                    fy += (orig_y - py) * 0.1;

                    forces[pi][qi] = (fx, fy);
                }
            }

            // Apply forces (clamped displacement)
            let max_disp = 3.0_f32;
            for (pi, path) in paths.iter_mut().enumerate() {
                let path_len = path.len();
                for (qi, point) in path.iter_mut().enumerate() {
                    if qi == 0 || qi == path_len - 1 {
                        continue;
                    }
                    let (fx, fy) = forces[pi][qi];
                    let mag = (fx * fx + fy * fy).sqrt();
                    if mag > 0.001 {
                        let scale = mag.min(max_disp) / mag;
                        point.0 += fx * scale;
                        point.1 += fy * scale;
                    }
                }
            }
        }

        paths
    }
}
