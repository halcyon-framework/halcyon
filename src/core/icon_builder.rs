//! SPEC-HAL-265: IconBuilder — high-level icon authoring API.
//!
//! Constructs `IconDefinition` values from geometric primitives
//! instead of raw path commands.

use crate::core::icon::{
    FillRule, IconDefinition, IconId, IconLayer, IconRenderMode, IconRole, LayerRole,
};
use crate::renderer::path::Path;

/// Circle approximation constant: 4 * (sqrt(2) - 1) / 3
const KAPPA: f32 = 0.5522847498;

/// High-level icon authoring builder.
///
/// Accumulates geometry from high-level primitives into path layers,
/// then produces an `IconDefinition` via `build()`.
///
/// All drawing methods consume and return `Self` for chaining.
pub struct IconBuilder {
    viewbox: f32,
    layers: Vec<(LayerRole, Path, bool)>,
    current_role: LayerRole,
    current_path: Path,
    current_has_computed: bool,
}

impl IconBuilder {
    pub fn new(viewbox: f32) -> Self {
        Self {
            viewbox,
            layers: Vec::new(),
            current_role: LayerRole::Base,
            current_path: Path::new(),
            current_has_computed: false,
        }
    }

    /// Start a new layer with the given role.
    /// Finalizes the current layer's path and begins accumulating into a new one.
    /// If the current path is empty, just switches the role without creating an empty layer.
    pub fn layer(mut self, role: LayerRole) -> Self {
        if !self.current_path.commands.is_empty() {
            self.layers.push((
                self.current_role,
                std::mem::replace(&mut self.current_path, Path::new()),
                self.current_has_computed,
            ));
        }
        self.current_role = role;
        self.current_has_computed = false;
        self
    }

    /// Filled rectangle along the line from (x0,y0) to (x1,y1),
    /// perpendicular offset by width/2.
    /// Same math as the `line_segment` helper in `icons/mod.rs`.
    pub fn line(mut self, x0: f32, y0: f32, x1: f32, y1: f32, width: f32) -> Self {
        self.current_has_computed = true;
        let hsw = width / 2.0;
        let dx = x1 - x0;
        let dy = y1 - y0;
        let len = (dx * dx + dy * dy).sqrt();
        if len < 0.001 {
            return self;
        }
        let px = -dy / len * hsw;
        let py = dx / len * hsw;
        self.current_path.move_to(x0 + px, y0 + py);
        self.current_path.line_to(x1 + px, y1 + py);
        self.current_path.line_to(x1 - px, y1 - py);
        self.current_path.line_to(x0 - px, y0 - py);
        self.current_path.close();
        self
    }

    /// Connected sequence of `line()` segments.
    pub fn polyline(mut self, points: &[(f32, f32)], width: f32) -> Self {
        self.current_has_computed = true;
        let hsw = width / 2.0;
        for pair in points.windows(2) {
            let (x0, y0) = pair[0];
            let (x1, y1) = pair[1];
            let dx = x1 - x0;
            let dy = y1 - y0;
            let len = (dx * dx + dy * dy).sqrt();
            if len < 0.001 {
                continue;
            }
            let px = -dy / len * hsw;
            let py = dx / len * hsw;
            self.current_path.move_to(x0 + px, y0 + py);
            self.current_path.line_to(x1 + px, y1 + py);
            self.current_path.line_to(x1 - px, y1 - py);
            self.current_path.line_to(x0 - px, y0 - py);
            self.current_path.close();
        }
        self
    }

    /// Closed filled polygon. `move_to` first point, `line_to` each subsequent, `close()`.
    pub fn polygon(mut self, points: &[(f32, f32)]) -> Self {
        if points.is_empty() {
            return self;
        }
        self.current_path.move_to(points[0].0, points[0].1);
        for &(x, y) in &points[1..] {
            self.current_path.line_to(x, y);
        }
        self.current_path.close();
        self
    }

    /// Filled rectangle via 4 `line_to` + `close`.
    pub fn rect(mut self, x: f32, y: f32, w: f32, h: f32) -> Self {
        self.current_path.move_to(x, y);
        self.current_path.line_to(x + w, y);
        self.current_path.line_to(x + w, y + h);
        self.current_path.line_to(x, y + h);
        self.current_path.close();
        self
    }

    /// Rectangle with circular corner arcs (4 cubic bezier quarter-circles
    /// using KAPPA constant, connected by straight edges).
    pub fn rounded_rect(mut self, x: f32, y: f32, w: f32, h: f32, radius: f32) -> Self {
        self.current_has_computed = true;
        let r = radius;
        let k = KAPPA * r;
        // Start at top edge, after top-left corner
        self.current_path.move_to(x + r, y);
        // Top edge → top-right corner
        self.current_path.line_to(x + w - r, y);
        self.current_path
            .cubic_to(x + w - r + k, y, x + w, y + r - k, x + w, y + r);
        // Right edge → bottom-right corner
        self.current_path.line_to(x + w, y + h - r);
        self.current_path
            .cubic_to(x + w, y + h - r + k, x + w - r + k, y + h, x + w - r, y + h);
        // Bottom edge → bottom-left corner
        self.current_path.line_to(x + r, y + h);
        self.current_path
            .cubic_to(x + r - k, y + h, x, y + h - r + k, x, y + h - r);
        // Left edge → top-left corner
        self.current_path.line_to(x, y + r);
        self.current_path
            .cubic_to(x, y + r - k, x + r - k, y, x + r, y);
        self.current_path.close();
        self
    }

    /// Filled circle via 4 cubic bezier arcs (KAPPA technique).
    pub fn circle(mut self, cx: f32, cy: f32, r: f32) -> Self {
        self.current_has_computed = true;
        let k = KAPPA * r;
        self.current_path.move_to(cx + r, cy);
        self.current_path
            .cubic_to(cx + r, cy + k, cx + k, cy + r, cx, cy + r);
        self.current_path
            .cubic_to(cx - k, cy + r, cx - r, cy + k, cx - r, cy);
        self.current_path
            .cubic_to(cx - r, cy - k, cx - k, cy - r, cx, cy - r);
        self.current_path
            .cubic_to(cx + k, cy - r, cx + r, cy - k, cx + r, cy);
        self.current_path.close();
        self
    }

    /// Filled annulus. Outer circle clockwise, inner circle counter-clockwise
    /// (NonZero fill creates cutout).
    pub fn ring(mut self, cx: f32, cy: f32, r_inner: f32, r_outer: f32) -> Self {
        self.current_has_computed = true;
        // Outer circle (clockwise in screen coords)
        let ko = KAPPA * r_outer;
        self.current_path.move_to(cx + r_outer, cy);
        self.current_path.cubic_to(
            cx + r_outer,
            cy + ko,
            cx + ko,
            cy + r_outer,
            cx,
            cy + r_outer,
        );
        self.current_path.cubic_to(
            cx - ko,
            cy + r_outer,
            cx - r_outer,
            cy + ko,
            cx - r_outer,
            cy,
        );
        self.current_path.cubic_to(
            cx - r_outer,
            cy - ko,
            cx - ko,
            cy - r_outer,
            cx,
            cy - r_outer,
        );
        self.current_path.cubic_to(
            cx + ko,
            cy - r_outer,
            cx + r_outer,
            cy - ko,
            cx + r_outer,
            cy,
        );
        self.current_path.close();

        // Inner circle (counter-clockwise for NonZero cutout)
        let ki = KAPPA * r_inner;
        self.current_path.move_to(cx + r_inner, cy);
        self.current_path.cubic_to(
            cx + r_inner,
            cy - ki,
            cx + ki,
            cy - r_inner,
            cx,
            cy - r_inner,
        );
        self.current_path.cubic_to(
            cx - ki,
            cy - r_inner,
            cx - r_inner,
            cy - ki,
            cx - r_inner,
            cy,
        );
        self.current_path.cubic_to(
            cx - r_inner,
            cy + ki,
            cx - ki,
            cy + r_inner,
            cx,
            cy + r_inner,
        );
        self.current_path.cubic_to(
            cx + ki,
            cy + r_inner,
            cx + r_inner,
            cy + ki,
            cx + r_inner,
            cy,
        );
        self.current_path.close();

        self
    }

    /// Stroked arc as filled shape. Outer arc at `r + width/2`, inner arc
    /// at `r - width/2`, connected at endpoints. Uses cubic bezier approximation.
    /// For arcs > 90°, splits into ≤90° segments.
    pub fn arc_stroke(
        mut self,
        cx: f32,
        cy: f32,
        r: f32,
        start_deg: f32,
        end_deg: f32,
        width: f32,
    ) -> Self {
        self.current_has_computed = true;
        let hw = width / 2.0;
        let r_outer = r + hw;
        let r_inner = r - hw;

        let start = start_deg.to_radians();
        let end = end_deg.to_radians();
        let sweep = end - start;
        if sweep.abs() < 0.001 {
            return self;
        }

        // Split into segments ≤ 90°
        let max_segment = std::f32::consts::FRAC_PI_2;
        let num_segments = (sweep.abs() / max_segment).ceil() as usize;
        let segment_sweep = sweep / num_segments as f32;

        let mut outer_segs = Vec::new();
        let mut inner_segs = Vec::new();
        for i in 0..num_segments {
            let a0 = start + i as f32 * segment_sweep;
            let a1 = a0 + segment_sweep;
            outer_segs.push(arc_cubic_segment(cx, cy, r_outer, a0, a1));
            inner_segs.push(arc_cubic_segment(cx, cy, r_inner, a0, a1));
        }

        // Outer arc forward
        let first = outer_segs[0];
        self.current_path.move_to(first.0, first.1);
        for seg in &outer_segs {
            self.current_path
                .cubic_to(seg.2, seg.3, seg.4, seg.5, seg.6, seg.7);
        }

        // Connect to inner arc end
        let last_inner = inner_segs.last().unwrap();
        self.current_path.line_to(last_inner.6, last_inner.7);

        // Inner arc backward (reverse cubic: swap control points, reverse direction)
        for seg in inner_segs.iter().rev() {
            self.current_path
                .cubic_to(seg.4, seg.5, seg.2, seg.3, seg.0, seg.1);
        }
        self.current_path.close();

        self
    }

    /// Consume builder and produce an `IconDefinition`.
    ///
    /// Computes `path_hash` from the accumulated layers, constructs `IconDefinition`
    /// with `fill_rule: FillRule::NonZero` per layer and `viewbox_width/height` from the builder's viewbox.
    pub fn build(
        mut self,
        id: IconId,
        name: &'static str,
        role: IconRole,
        mode: IconRenderMode,
    ) -> IconDefinition {
        // Finalize last layer
        if !self.current_path.commands.is_empty() {
            self.layers.push((
                self.current_role,
                self.current_path,
                self.current_has_computed,
            ));
        }

        // Compute combined path_hash: ordered fold over (role, layer_hash)
        let mut hash: u64 = 0;
        let layers: Vec<IconLayer> = self
            .layers
            .into_iter()
            .map(|(lr, path, has_computed)| {
                let layer_hash = path.content_hash();
                hash = hash.wrapping_mul(31).wrapping_add(layer_hash ^ (lr as u64));
                IconLayer {
                    role: lr,
                    path,
                    fill_rule: FillRule::NonZero,
                    skip_precision: has_computed,
                }
            })
            .collect();

        IconDefinition {
            id,
            name,
            layers,
            viewbox_width: self.viewbox,
            viewbox_height: self.viewbox,
            default_mode: mode,
            role,
            path_hash: hash,
        }
    }
}

/// Cubic bezier approximation of a single circular arc segment.
/// Returns (start_x, start_y, c1x, c1y, c2x, c2y, end_x, end_y).
fn arc_cubic_segment(
    cx: f32,
    cy: f32,
    r: f32,
    a0: f32,
    a1: f32,
) -> (f32, f32, f32, f32, f32, f32, f32, f32) {
    let sweep = a1 - a0;
    let k = (4.0 / 3.0) * (sweep / 4.0).tan();

    let cos0 = a0.cos();
    let sin0 = a0.sin();
    let cos1 = a1.cos();
    let sin1 = a1.sin();

    let p0x = cx + r * cos0;
    let p0y = cy + r * sin0;
    let p3x = cx + r * cos1;
    let p3y = cy + r * sin1;

    // Control points using tangent direction at each endpoint
    let p1x = p0x - k * r * sin0;
    let p1y = p0y + k * r * cos0;
    let p2x = p3x + k * r * sin1;
    let p2y = p3y - k * r * cos1;

    (p0x, p0y, p1x, p1y, p2x, p2y, p3x, p3y)
}
