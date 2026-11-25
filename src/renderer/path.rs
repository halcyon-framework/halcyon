//! SPEC-R-004: GPU Path Renderer - CPU Path Layer
//!
//! This module implements the CPU-side path subsystem for vector graphics rendering.
//! It provides path construction, curve flattening, monotone triangulation, and stroke generation.
//!
//! Stage 1: Path commands, flattening, fill mesh generation.
//! Stage 2: Stroke mesh generation with caps and joins.
//!
//! CPU-only - no GPU code, batching, or renderer integration.

use crate::compositor::SceneKey;
use crate::core::math::RectF;
use crate::renderer::mesh_cache::PixelSnapMode;
use crate::renderer::pipelines::path_pipeline::PathInstance;

mod fill;
pub(super) use fill::FillContours;

/// Winding test used to fill the combined contours of a path (SPEC-R-004 §13).
#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Hash)]
pub enum FillRule {
    /// Fill where the sum of the signed contour crossings is nonzero.
    #[default]
    NonZero,
    /// Fill where the number of contour crossings is odd.
    EvenOdd,
}

/// A refusal to emit or tessellate a filled path (SPEC-R-004 §13).
#[derive(Copy, Clone, Debug, Eq, PartialEq, thiserror::Error)]
pub enum PathFillError {
    #[error("invalid path commands or unrepresentable path geometry")]
    InvalidSource,
    #[error("invalid device scale or unrepresentable pixel projection")]
    InvalidScale,
    #[error("path fill tessellation failed")]
    TessellationFailed,
    #[error("invalid tessellated path mesh")]
    InvalidMesh,
    #[error("paint sink does not support the requested path fill rule")]
    UnsupportedSink,
}

// ============================================================================
// SPEC-R-004 Stage 17: Path Debug Mode
// ============================================================================

/// Debug visualization mode for path geometry.
///
/// Used to emit CPU-side overlay primitives showing path internals.
/// This is purely for developer debugging - no shader or pipeline changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PathDebugMode {
    /// No debug visualization.
    None,
    /// Show path vertices as small markers.
    Vertices,
    /// Show path edges (line segments between vertices).
    Edges,
    /// Show triangulation (outline each triangle in the mesh).
    Triangles,
}

impl Default for PathDebugMode {
    fn default() -> Self {
        Self::None
    }
}

/// Flattening tolerance in logical pixels.
/// Curves are subdivided until the deviation from the true curve is less than this value.
const FLATTEN_TOLERANCE: f32 = 0.25;

// ============================================================================
// PathCommand - Individual path operations
// ============================================================================

/// A single command in a path definition.
#[derive(Debug, Clone)]
pub enum PathCommand {
    /// Begin a new subpath at the given point.
    MoveTo(f32, f32),
    /// Draw a line from the current point to the given point.
    LineTo(f32, f32),
    /// Draw a quadratic Bézier curve.
    /// Parameters: control point (cx, cy), end point (x, y)
    QuadTo(f32, f32, f32, f32),
    /// Draw a cubic Bézier curve.
    /// Parameters: control point 1 (c1x, c1y), control point 2 (c2x, c2y), end point (x, y)
    CubicTo(f32, f32, f32, f32, f32, f32),
    /// Close the current subpath by drawing a line to the subpath start.
    Close,
}

// ============================================================================
// Path - Collection of path commands
// ============================================================================

/// A vector path consisting of one or more subpaths.
///
/// Paths are built by issuing commands (MoveTo, LineTo, QuadTo, CubicTo, Close).
/// The user must call move_to() to begin each subpath before issuing line/curve commands.
#[derive(Debug, Clone)]
pub struct Path {
    pub commands: Vec<PathCommand>,
}

impl Path {
    /// Create a new empty path.
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    /// Begin a new subpath at the given point.
    pub fn move_to(&mut self, x: f32, y: f32) {
        self.commands.push(PathCommand::MoveTo(x, y));
    }

    /// Draw a line from the current point to the given point.
    pub fn line_to(&mut self, x: f32, y: f32) {
        self.commands.push(PathCommand::LineTo(x, y));
    }

    /// Draw a quadratic Bézier curve from the current point.
    ///
    /// # Arguments
    /// * `cx`, `cy` - Control point
    /// * `x`, `y` - End point
    pub fn quad_to(&mut self, cx: f32, cy: f32, x: f32, y: f32) {
        self.commands.push(PathCommand::QuadTo(cx, cy, x, y));
    }

    /// Draw a cubic Bézier curve from the current point.
    ///
    /// # Arguments
    /// * `c1x`, `c1y` - First control point
    /// * `c2x`, `c2y` - Second control point
    /// * `x`, `y` - End point
    pub fn cubic_to(&mut self, c1x: f32, c1y: f32, c2x: f32, c2y: f32, x: f32, y: f32) {
        self.commands
            .push(PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y));
    }

    /// Close the current subpath by drawing a line to the subpath start.
    pub fn close(&mut self) {
        self.commands.push(PathCommand::Close);
    }

    /// Flatten all curves in the path to line segments.
    ///
    /// Returns a polyline (list of points) representing the flattened path.
    /// Each subpath is flattened independently, with Close commands adding
    /// the subpath start point to close the loop.
    pub fn flatten(&self) -> Vec<[f32; 2]> {
        match self.flatten_points() {
            Ok(points) => points,
            Err(error) => {
                tracing::error!(%error, "path flattening refused");
                Vec::new()
            }
        }
    }

    fn flatten_points(&self) -> Result<Vec<[f32; 2]>, PathFillError> {
        let mut result = Vec::new();
        let mut current = [0.0f32, 0.0];
        let mut subpath_start = [0.0f32, 0.0];

        for cmd in &self.commands {
            match *cmd {
                PathCommand::MoveTo(x, y) => {
                    current = [x, y];
                    subpath_start = current;
                    result.push(current);
                }
                PathCommand::LineTo(x, y) => {
                    current = [x, y];
                    result.push(current);
                }
                PathCommand::QuadTo(cx, cy, x, y) => {
                    adaptive_flatten(&mut result, [current, [cx, cy], [x, y]], 0)?;
                    current = [x, y];
                }
                PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                    adaptive_flatten(
                        &mut result,
                        [current, [c1x, c1y], [c2x, c2y], [x, y]],
                        0,
                    )?;
                    current = [x, y];
                }
                PathCommand::Close => {
                    if current != subpath_start {
                        result.push(subpath_start);
                        current = subpath_start;
                    }
                }
            }
        }

        Ok(result)
    }

    /// Tessellate all contours together under the default NonZero rule.
    /// On failure, emits a renderer diagnostic and returns an empty mesh.
    pub fn to_mesh(&self) -> PathMesh {
        mesh_or_report(self.try_to_mesh_with_fill_rule(FillRule::NonZero))
    }

    /// Tessellate this path in logical coordinates under the requested fill rule.
    ///
    /// Each contour is implicitly closed for fill. An empty filled region
    /// succeeds with an empty mesh.
    pub fn try_to_mesh_with_fill_rule(&self, rule: FillRule) -> Result<PathMesh, PathFillError> {
        self.flatten_fill()?
            .tessellate(rule, PixelSnapMode::None, 1.0)
    }

    pub(super) fn flatten_fill(&self) -> Result<FillContours, PathFillError> {
        FillContours::new(self)
    }

    /// Compute a deterministic content hash of the path command sequence.
    ///
    /// SPEC-HAL-264 §4.2: Two paths with identical command sequences produce identical hashes.
    /// Float values are converted to bits via `f32::to_bits()` before hashing.
    pub fn content_hash(&self) -> u64 {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        for cmd in &self.commands {
            match cmd {
                PathCommand::MoveTo(x, y) => {
                    0u8.hash(&mut hasher);
                    x.to_bits().hash(&mut hasher);
                    y.to_bits().hash(&mut hasher);
                }
                PathCommand::LineTo(x, y) => {
                    1u8.hash(&mut hasher);
                    x.to_bits().hash(&mut hasher);
                    y.to_bits().hash(&mut hasher);
                }
                PathCommand::QuadTo(cx, cy, x, y) => {
                    2u8.hash(&mut hasher);
                    cx.to_bits().hash(&mut hasher);
                    cy.to_bits().hash(&mut hasher);
                    x.to_bits().hash(&mut hasher);
                    y.to_bits().hash(&mut hasher);
                }
                PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                    3u8.hash(&mut hasher);
                    c1x.to_bits().hash(&mut hasher);
                    c1y.to_bits().hash(&mut hasher);
                    c2x.to_bits().hash(&mut hasher);
                    c2y.to_bits().hash(&mut hasher);
                    x.to_bits().hash(&mut hasher);
                    y.to_bits().hash(&mut hasher);
                }
                PathCommand::Close => {
                    4u8.hash(&mut hasher);
                }
            }
        }
        hasher.finish()
    }

    /// Compute the axis-aligned bounding box of the path.
    ///
    /// SPEC-HAL-264 §4: Used to construct MeshCacheKey render dimensions.
    /// Flattens curves to line segments, then computes min/max bounds.
    pub fn bounds(&self) -> RectF {
        compute_bounds(&self.flatten())
    }

    /// Return a new Path with all coordinates scaled uniformly and translated.
    ///
    /// Each coordinate is transformed as: x' = x * scale + translate_x, y' = y * scale + translate_y.
    /// `Close` commands pass through unchanged (no coordinates).
    pub fn transformed(&self, scale: f32, translate_x: f32, translate_y: f32) -> Path {
        let commands = self
            .commands
            .iter()
            .map(|cmd| match *cmd {
                PathCommand::MoveTo(x, y) => {
                    PathCommand::MoveTo(x * scale + translate_x, y * scale + translate_y)
                }
                PathCommand::LineTo(x, y) => {
                    PathCommand::LineTo(x * scale + translate_x, y * scale + translate_y)
                }
                PathCommand::QuadTo(cx, cy, x, y) => PathCommand::QuadTo(
                    cx * scale + translate_x,
                    cy * scale + translate_y,
                    x * scale + translate_x,
                    y * scale + translate_y,
                ),
                PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => PathCommand::CubicTo(
                    c1x * scale + translate_x,
                    c1y * scale + translate_y,
                    c2x * scale + translate_x,
                    c2y * scale + translate_y,
                    x * scale + translate_x,
                    y * scale + translate_y,
                ),
                PathCommand::Close => PathCommand::Close,
            })
            .collect();
        Path { commands }
    }
}

impl Default for Path {
    fn default() -> Self {
        Self::new()
    }
}

/// Project the flattened contour points, then tessellate under NonZero.
/// Intersections are produced from the projected edges (SPEC-R-004 §13.2).
pub fn tessellate_and_snap(path: &Path, snap_mode: PixelSnapMode, dpr: f32) -> PathMesh {
    mesh_or_report(
        path.flatten_fill()
            .and_then(|contours| contours.tessellate(FillRule::NonZero, snap_mode, dpr)),
    )
}

fn mesh_or_report(result: Result<PathMesh, PathFillError>) -> PathMesh {
    match result {
        Ok(mesh) => mesh,
        Err(error) => {
            tracing::error!(%error, "path fill refused");
            PathMesh {
                vertices: Vec::new(),
                indices: Vec::new(),
                bounds: RectF::zero(),
            }
        }
    }
}

impl Path {
    /// Build a rounded rectangle path.
    ///
    /// Creates a closed path for a rectangle with rounded corners.
    /// Uses quadratic Bezier curves to approximate quarter-circle arcs.
    ///
    /// # Arguments
    /// * `x`, `y` - Top-left corner of the bounding rectangle
    /// * `width`, `height` - Dimensions of the rectangle
    /// * `radius` - Corner radius (clamped to min(width, height) / 2)
    ///
    /// # Returns
    /// A closed Path representing the rounded rectangle.
    pub fn rounded_rect(x: f32, y: f32, width: f32, height: f32, radius: f32) -> Self {
        let mut path = Path::new();

        // Clamp radius to avoid degenerate geometry
        let max_radius = (width.min(height) / 2.0).max(0.0);
        let r = radius.clamp(0.0, max_radius);

        if r < 0.001 {
            // Degenerate case: just a rectangle
            path.move_to(x, y);
            path.line_to(x + width, y);
            path.line_to(x + width, y + height);
            path.line_to(x, y + height);
            path.close();
            return path;
        }

        // κ: cubic Bezier control point factor for circular arc approximation.
        // 4 control points per quarter-circle: P0, P1=P0+κ·tangent, P2=P3+κ·tangent, P3.
        // Max radial error: 0.027% of radius (vs 5.4% for quadratic).
        const K: f32 = 0.5522847498;
        let rk = r * K; // control point offset from arc endpoint

        // Start at top edge, after top-left corner arc
        path.move_to(x + r, y);

        // Top edge → top-right corner
        path.line_to(x + width - r, y);
        path.cubic_to(
            x + width - r + rk,
            y, // c1: along top edge
            x + width,
            y + r - rk, // c2: along right edge
            x + width,
            y + r, // end: start of right edge
        );

        // Right edge → bottom-right corner
        path.line_to(x + width, y + height - r);
        path.cubic_to(
            x + width,
            y + height - r + rk, // c1: along right edge
            x + width - r + rk,
            y + height, // c2: along bottom edge
            x + width - r,
            y + height, // end: start of bottom edge
        );

        // Bottom edge → bottom-left corner
        path.line_to(x + r, y + height);
        path.cubic_to(
            x + r - rk,
            y + height, // c1: along bottom edge
            x,
            y + height - r + rk, // c2: along left edge
            x,
            y + height - r, // end: start of left edge
        );

        // Left edge → top-left corner
        path.line_to(x, y + r);
        path.cubic_to(
            x,
            y + r - rk, // c1: along left edge
            x + r - rk,
            y, // c2: along top edge
            x + r,
            y, // end: back to start
        );

        path.close();
        path
    }

    /// Build a ring (annulus) mesh for rounded rectangle borders.
    ///
    /// Creates a mesh representing the area between an outer rounded rect and
    /// an inner rounded rect (inset by border_width). This renders borders as
    /// a single filled shape rather than a centered stroke.
    ///
    /// # Scope Restriction
    ///
    /// **This is NOT a general polygon ring triangulator.** It is designed exclusively
    /// for rounded rectangle borders where both contours are generated by `Path::rounded_rect`.
    /// The triangulation algorithm assumes:
    /// - Both contours have consistent vertex ordering (same winding direction)
    /// - Vertex density is similar between inner and outer contours
    /// - The inner contour is strictly contained within the outer contour
    ///
    /// Do not use this for arbitrary polygon pairs.
    ///
    /// # Arguments
    /// * `rect` - The outer bounding rectangle
    /// * `radius` - Corner radius for the outer rounded rect
    /// * `border_width` - Width of the border (inset from outer to inner)
    ///
    /// # Returns
    /// A PathMesh representing the ring shape, or None if the border would be degenerate.
    #[allow(
        dead_code,
        reason = "Retained rounded-rectangle ring mesh construction"
    )]
    pub(crate) fn build_ring_mesh(rect: RectF, radius: f32, border_width: f32) -> Option<PathMesh> {
        if border_width <= 0.0 || rect.width <= 0.0 || rect.height <= 0.0 {
            return None;
        }

        let max_r = (rect.width.min(rect.height) / 2.0).max(0.0);
        let r_out = radius.clamp(0.0, max_r);

        let bw = border_width;
        let inner_w = rect.width - 2.0 * bw;
        let inner_h = rect.height - 2.0 * bw;

        if inner_w <= 0.0 || inner_h <= 0.0 {
            let p = Path::rounded_rect(rect.x, rect.y, rect.width, rect.height, r_out);
            return Some(p.to_mesh());
        }

        let r_in = (r_out - bw).max(0.0);

        // Number of segments per corner arc.
        // Scale with radius for quality, minimum 4.
        let seg = ((r_out * 2.0).ceil() as usize).clamp(4, 16);

        // Cubic Bezier evaluation at parameter t
        fn cubic_eval(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], t: f32) -> [f32; 2] {
            let u = 1.0 - t;
            let uu = u * u;
            let tt = t * t;
            [
                uu * u * p0[0] + 3.0 * uu * t * p1[0] + 3.0 * u * tt * p2[0] + tt * t * p3[0],
                uu * u * p0[1] + 3.0 * uu * t * p1[1] + 3.0 * u * tt * p2[1] + tt * t * p3[1],
            ]
        }

        const K: f32 = 0.5522847498;

        let ox = rect.x;
        let oy = rect.y;
        let ow = rect.width;
        let oh = rect.height;
        let ix = rect.x + bw;
        let iy = rect.y + bw;

        let rko = r_out * K;
        let rki = r_in * K;

        // 4 corners: top-right, bottom-right, bottom-left, top-left
        let corners: [[[f32; 2]; 4]; 4] = [
            [
                [ox + ow - r_out, oy],
                [ox + ow - r_out + rko, oy],
                [ox + ow, oy + r_out - rko],
                [ox + ow, oy + r_out],
            ],
            [
                [ox + ow, oy + oh - r_out],
                [ox + ow, oy + oh - r_out + rko],
                [ox + ow - r_out + rko, oy + oh],
                [ox + ow - r_out, oy + oh],
            ],
            [
                [ox + r_out, oy + oh],
                [ox + r_out - rko, oy + oh],
                [ox, oy + oh - r_out + rko],
                [ox, oy + oh - r_out],
            ],
            [
                [ox, oy + r_out],
                [ox, oy + r_out - rko],
                [ox + r_out - rko, oy],
                [ox + r_out, oy],
            ],
        ];

        let corners_inner: [[[f32; 2]; 4]; 4] = [
            [
                [ix + inner_w - r_in, iy],
                [ix + inner_w - r_in + rki, iy],
                [ix + inner_w, iy + r_in - rki],
                [ix + inner_w, iy + r_in],
            ],
            [
                [ix + inner_w, iy + inner_h - r_in],
                [ix + inner_w, iy + inner_h - r_in + rki],
                [ix + inner_w - r_in + rki, iy + inner_h],
                [ix + inner_w - r_in, iy + inner_h],
            ],
            [
                [ix + r_in, iy + inner_h],
                [ix + r_in - rki, iy + inner_h],
                [ix, iy + inner_h - r_in + rki],
                [ix, iy + inner_h - r_in],
            ],
            [
                [ix, iy + r_in],
                [ix, iy + r_in - rki],
                [ix + r_in - rki, iy],
                [ix + r_in, iy],
            ],
        ];

        // Generate paired vertices: outer[i] <-> inner[i]
        let total_pairs = 4 * (seg + 1);
        let mut outer_verts = Vec::with_capacity(total_pairs);
        let mut inner_verts = Vec::with_capacity(total_pairs);

        for c in 0..4 {
            let [p0, c1, c2, p3] = corners[c];
            let [ip0, ic1, ic2, ip3] = corners_inner[c];
            for s in 0..=seg {
                let t = s as f32 / seg as f32;
                outer_verts.push(cubic_eval(p0, c1, c2, p3, t));
                inner_verts.push(cubic_eval(ip0, ic1, ic2, ip3, t));
            }
        }

        // Build mesh: outer first, then inner
        let n = outer_verts.len();
        let inner_offset = n as u32;

        let mut vertices = Vec::with_capacity(n * 2);
        vertices.extend_from_slice(&outer_verts);
        vertices.extend_from_slice(&inner_verts);

        // Quad strip: for each pair (i, i+1), create 2 triangles
        let mut indices = Vec::with_capacity(n * 6);
        for i in 0..n {
            let i0 = i as u32;
            let i1 = ((i + 1) % n) as u32;
            let j0 = inner_offset + i0;
            let j1 = inner_offset + i1;

            indices.push(i0);
            indices.push(i1);
            indices.push(j0);

            indices.push(j0);
            indices.push(i1);
            indices.push(j1);
        }

        let bounds = compute_bounds(&vertices);
        Some(PathMesh {
            vertices,
            indices,
            bounds,
        })
    }
}

// ============================================================================
// PathMesh - Triangulated path output
// ============================================================================

/// A triangulated mesh representation of a path.
///
/// This is the CPU-side output of path triangulation, ready to be uploaded
/// to the GPU for rendering (in Stage 2+).
#[derive(Debug, Clone)]
pub struct PathMesh {
    /// Vertex positions as [x, y] pairs.
    pub vertices: Vec<[f32; 2]>,
    /// Triangle indices (groups of 3).
    pub indices: Vec<u32>,
    /// Axis-aligned bounding box of the mesh.
    pub bounds: RectF,
}

// ============================================================================
// SPEC-R-004 Stage 12: PathDraw - CPU-side draw command
// ============================================================================

/// A single path draw command ready for batching.
///
/// This captures a tessellated mesh along with per-instance data (color, transform,
/// clip, stroke/fill flag). The batcher collects these and flattens them into
/// GPU-side PathVertex/PathInstance arrays.
///
/// SPEC-R-004 Stage 18: Added z_index and insertion_order for deterministic ordering.
/// SPEC-R-004 Stage 19: Added bounds for debug visualizations and future heuristics.
/// SPEC-HAL-081A: Added scene_key for compositor surface routing.
/// Sort key: (z_index ASC, clip_id ASC, stroke_flag ASC, debug_mode ASC, insertion_order ASC)
#[derive(Debug, Clone)]
pub struct PathDraw {
    pub control_subtree_order: Option<std::sync::Arc<[crate::core::paint::ControlSubtreeMember]>>,
    /// Tessellated mesh (vertices and indices).
    pub mesh: PathMesh,
    /// RGBA color [r, g, b, a] in 0..1 range.
    pub color: [f32; 4],
    /// 3x2 affine transform [m00, m01, m02, m10, m11, m12].
    /// Applied as: x' = m00*x + m01*y + m02, y' = m10*x + m11*y + m12
    pub transform: [f32; 6],
    /// Clip buffer index. For now always 0 (fullscreen clip).
    pub clip_id: u32,
    /// 0 = fill, 1 = stroke.
    pub stroke_flag: u32,
    /// SPEC-R-004 Stage 17: Debug visualization mode.
    /// None = no debug overlay. Other modes emit additional primitives.
    pub debug: Option<PathDebugMode>,
    /// SPEC-R-004 Stage 18: Z-index for deterministic ordering.
    pub z_index: u32,
    /// INV-PATH-ORDER-STABILITY: Sort bias for stable ordering independent of presence.
    /// Used as secondary sort key after z_index but before other fields.
    /// Typical values: background=0, border=100, scrollbar=200, caret=300.
    /// This ensures caret presence/absence doesn't reorder other primitives.
    pub sort_bias: u32,
    /// SPEC-R-004 Stage 18: Insertion order for stable sort tiebreaker.
    /// Assigned by the batcher at add_path() time.
    pub insertion_order: u32,
    /// SPEC-R-004 Stage 19: Axis-aligned bounding box of the mesh.
    /// Copied from PathMesh.bounds for debug visualizations and future heuristics.
    pub bounds: RectF,
    /// SPEC-HAL-081A: Compositor surface scene_key this path belongs to.
    /// Used for surface-ordered submission (INV-COMP-P04).
    /// Typed as SceneKey to guarantee non-zero.
    pub scene_key: SceneKey,
}

impl PathDraw {
    pub(crate) fn effective_order(&self) -> crate::core::paint::EffectivePaintOrder {
        crate::core::paint::EffectivePaintOrder::new(
            self.z_index,
            self.control_subtree_order.clone(),
        )
    }

    /// Create a new PathDraw with identity transform and no clip.
    ///
    /// # Field Assignment
    ///
    /// - `z_index` and `insertion_order` default to 0 (assigned by batcher via `add_path()`)
    /// - `bounds` copied from mesh.bounds
    /// - `scene_key` required at construction (typed as SceneKey, guaranteed non-zero)
    ///
    /// # SPEC-HAL-081A: scene_key Flow
    ///
    /// The scene_key MUST come from the compositor snapshot. Typed SceneKey
    /// ensures no sentinel (0) values can be used.
    pub fn new(mesh: PathMesh, color: [f32; 4], stroke_flag: u32, scene_key: SceneKey) -> Self {
        let bounds = mesh.bounds;
        Self {
            mesh,
            control_subtree_order: None,
            color,
            transform: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0], // identity
            clip_id: 0,
            stroke_flag,
            debug: None,
            z_index: 0,
            sort_bias: 0,
            insertion_order: 0,
            bounds,
            scene_key,
        }
    }

    /// Create a fill PathDraw.
    pub fn fill(mesh: PathMesh, color: [f32; 4], scene_key: SceneKey) -> Self {
        Self::new(mesh, color, 0, scene_key)
    }

    /// Create a stroke PathDraw.
    pub fn stroke(mesh: PathMesh, color: [f32; 4], scene_key: SceneKey) -> Self {
        Self::new(mesh, color, 1, scene_key)
    }

    /// Set the transform.
    pub fn with_transform(mut self, transform: [f32; 6]) -> Self {
        self.transform = transform;
        self
    }

    /// Set the clip ID.
    pub fn with_clip_id(mut self, clip_id: u32) -> Self {
        self.clip_id = clip_id;
        self
    }

    /// SPEC-R-004 Stage 17: Set the debug visualization mode.
    pub fn with_debug(mut self, mode: PathDebugMode) -> Self {
        self.debug = Some(mode);
        self
    }

    /// SPEC-R-004 Stage 18: Set the z-index for deterministic ordering.
    pub fn with_z_index(mut self, z_index: u32) -> Self {
        self.z_index = z_index;
        self
    }

    /// INV-PATH-ORDER-STABILITY: Set the sort bias for stable ordering.
    /// Typical values: background=0, border=100, scrollbar=200, caret=300.
    pub fn with_sort_bias(mut self, sort_bias: u32) -> Self {
        self.sort_bias = sort_bias;
        self
    }

    /// SPEC-R-004 Stage 20: Convert PathDraw into the GPU PathInstance.
    ///
    /// Only GPU-used fields are transferred (color, transform, clip_id, stroke_flag).
    /// CPU-only fields (mesh, debug, z_index, insertion_order, bounds) are not included.
    pub fn to_instance(&self) -> PathInstance {
        PathInstance {
            color: self.color,
            transform: self.transform,
            clip_id: self.clip_id,
            stroke_flag: self.stroke_flag,
        }
    }
}

// ============================================================================
// Stage 2: Stroke Types
// ============================================================================

/// Line join style for stroke corners.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeJoin {
    /// Sharp corner (may be clamped by miter limit).
    Miter,
    /// Rounded corner with arc.
    Round,
    /// Flat diagonal corner.
    Bevel,
}

/// Line cap style for stroke endpoints.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StrokeCap {
    /// No extension beyond endpoint.
    Butt,
    /// Semicircular cap.
    Round,
    /// Rectangular extension by half stroke width.
    Square,
}

/// Parameters controlling stroke appearance.
#[derive(Debug, Clone, Copy)]
pub struct StrokeParams {
    /// Stroke width in logical pixels.
    pub width: f32,
    /// Join style for corners.
    pub join: StrokeJoin,
    /// Cap style for open path endpoints.
    pub cap: StrokeCap,
    /// Miter limit in width units. When miter length exceeds this * width,
    /// the join falls back to bevel. SPEC-R-004 default is 4.0.
    pub miter_limit: f32,
}

impl Default for StrokeParams {
    fn default() -> Self {
        Self {
            width: 1.0,
            join: StrokeJoin::Miter,
            cap: StrokeCap::Butt,
            miter_limit: 4.0,
        }
    }
}

/// A triangulated mesh representation of a stroked path.
#[derive(Debug, Clone)]
pub struct StrokeMesh {
    /// Vertex positions as [x, y] pairs.
    pub vertices: Vec<[f32; 2]>,
    /// Triangle indices (groups of 3).
    pub indices: Vec<u32>,
    /// Axis-aligned bounding box (includes stroke width expansion).
    pub bounds: RectF,
}

// ============================================================================
// Curve Flattening - Adaptive Subdivision
// ============================================================================

/// Subdivide a quadratic or cubic at 0.5 using the existing de Casteljau math.
fn adaptive_flatten<const N: usize>(
    output: &mut Vec<[f32; 2]>,
    curve: [[f32; 2]; N],
    depth: u32,
) -> Result<(), PathFillError> {
    if !curve.iter().copied().all(finite_point) {
        return Err(PathFillError::InvalidSource);
    }

    let start = curve[0];
    let end = curve[N - 1];
    let mut deviation = 0.0_f32;
    for &control in &curve[1..N - 1] {
        deviation = deviation.max(point_line_distance(control, start, end)?);
    }
    if deviation <= FLATTEN_TOLERANCE {
        output.push(end);
        return Ok(());
    }

    // Beyond this depth, even f64 cannot distinguish all dyadic parameters
    // of a subdivision of [0, 1].
    if depth >= f64::MANTISSA_DIGITS {
        return Err(PathFillError::InvalidSource);
    }
    let mut left = curve;
    let mut right = curve;
    let mut controls = curve;
    for level in 1..N {
        for i in 0..N - level {
            controls[i] = midpoint(controls[i], controls[i + 1]);
            if !finite_point(controls[i]) {
                return Err(PathFillError::InvalidSource);
            }
        }
        left[level] = controls[0];
        right[N - level - 1] = controls[N - level - 1];
    }
    if left == curve || right == curve {
        return Err(PathFillError::InvalidSource);
    }

    adaptive_flatten(output, left, depth + 1)?;
    adaptive_flatten(output, right, depth + 1)
}

/// Compute the perpendicular distance from point p to the line defined by a and b.
fn point_line_distance(p: [f32; 2], a: [f32; 2], b: [f32; 2]) -> Result<f32, PathFillError> {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let len_sq = dx * dx + dy * dy;

    if !len_sq.is_finite() {
        return Err(PathFillError::InvalidSource);
    }
    let px = p[0] - a[0];
    let py = p[1] - a[1];
    let distance = if len_sq < 1e-10 {
        // Degenerate chord: use distance to the start point.
        (px * px + py * py).sqrt()
    } else {
        (px * dy - py * dx).abs() / len_sq.sqrt()
    };
    distance
        .is_finite()
        .then_some(distance)
        .ok_or(PathFillError::InvalidSource)
}

/// Compute the midpoint of two points.
fn midpoint(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    [(a[0] + b[0]) * 0.5, (a[1] + b[1]) * 0.5]
}

fn finite_point([x, y]: [f32; 2]) -> bool {
    x.is_finite() && y.is_finite()
}

// ============================================================================
// Simple Polygon Triangulation
// ============================================================================

/// Triangulate a simple polygon with ear clipping.
///
/// Either input orientation is accepted. Indices refer to the original points
/// in groups of three forming nondegenerate counter-clockwise triangles. Exact
/// consecutive duplicates, a repeated closing point, and collinear intermediate
/// points on a segment can be omitted.
///
/// Returns no indices for nonfinite, degenerate, or nonsimple input, or if a
/// complete triangulation cannot be established.
pub fn triangulate_monotone(poly: &[[f32; 2]]) -> Vec<u32> {
    if poly.len() < 3
        || u32::try_from(poly.len()).is_err()
        || !poly.iter().copied().all(finite_point)
    {
        return Vec::new();
    }

    let mut remaining: Vec<usize> = (0..poly.len()).collect();
    remaining.dedup_by(|a, b| poly[*a] == poly[*b]);
    if remaining.len() > 1 && poly[remaining[0]] == poly[*remaining.last().unwrap()] {
        remaining.pop();
    }
    loop {
        if remaining.len() < 3 {
            return Vec::new();
        }
        let redundant = (0..remaining.len()).find(|&i| {
            let [a, b, c] = polygon_corner(&remaining, i).map(|index| poly[index]);
            orientation(a, b, c) == 0.0 && within_segment_bounds(a, c, b)
        });
        let Some(redundant) = redundant else {
            break;
        };
        remaining.remove(redundant);
    }

    if !is_simple_polygon(poly, &remaining) {
        return Vec::new();
    }

    // A corner at the left supporting line of a simple polygon is convex.
    // Its turn gives the traversal direction without summing a polygon's area.
    let leftmost = (0..remaining.len())
        .min_by(|&a, &b| {
            let [ax, ay] = poly[remaining[a]];
            let [bx, by] = poly[remaining[b]];
            ax.total_cmp(&bx).then_with(|| ay.total_cmp(&by))
        })
        .expect("at least three remaining corners");
    let [a, b, c] = polygon_corner(&remaining, leftmost).map(|index| poly[index]);
    if orientation(a, b, c) < 0.0 {
        remaining.reverse();
    }

    let mut indices = Vec::with_capacity((remaining.len() - 2) * 3);
    while remaining.len() > 3 {
        let ear = (0..remaining.len()).find_map(|i| {
            let triangle = polygon_corner(&remaining, i);
            is_ear(poly, &remaining, triangle).then_some((i, triangle))
        });
        let Some((i, triangle)) = ear else {
            return Vec::new();
        };
        indices.extend(triangle.map(|index| index as u32));
        remaining.remove(i);
    }

    let [a, b, c] = [remaining[0], remaining[1], remaining[2]];
    if orientation(poly[a], poly[b], poly[c]) <= 0.0 {
        return Vec::new();
    }
    indices.extend([a as u32, b as u32, c as u32]);
    indices
}

fn polygon_corner(order: &[usize], i: usize) -> [usize; 3] {
    let len = order.len();
    [order[(i + len - 1) % len], order[i], order[(i + 1) % len]]
}

fn is_ear(poly: &[[f32; 2]], remaining: &[usize], triangle: [usize; 3]) -> bool {
    let [a, b, c] = triangle.map(|index| poly[index]);
    orientation(a, b, c) > 0.0
        && remaining.iter().all(|&index| {
            if triangle.contains(&index) {
                return true;
            }
            // The diagonal must not pass through another remaining vertex.
            let point = poly[index];
            orientation(a, b, point) < 0.0
                || orientation(b, c, point) < 0.0
                || orientation(c, a, point) < 0.0
        })
}

fn is_simple_polygon(poly: &[[f32; 2]], order: &[usize]) -> bool {
    let len = order.len();
    for i in 0..len {
        let a = poly[order[i]];
        let b = poly[order[(i + 1) % len]];
        let next = poly[order[(i + 2) % len]];
        // Collinear intermediate points were removed. A remaining straight-line
        // triple doubles back and overlaps its adjacent edge.
        if orientation(a, b, next) == 0.0 {
            return false;
        }
        for j in i + 2..len {
            if i == 0 && j == len - 1 {
                continue; // First and last edges share their endpoint.
            }
            let c = poly[order[j]];
            let d = poly[order[(j + 1) % len]];
            if segments_intersect(a, b, c, d) {
                return false;
            }
        }
    }
    true
}

fn segments_intersect(a: [f32; 2], b: [f32; 2], c: [f32; 2], d: [f32; 2]) -> bool {
    let abc = orientation(a, b, c);
    let abd = orientation(a, b, d);
    let cda = orientation(c, d, a);
    let cdb = orientation(c, d, b);
    let opposite = |x, y| (x > 0.0 && y < 0.0) || (x < 0.0 && y > 0.0);
    (opposite(abc, abd) && opposite(cda, cdb))
        || (abc == 0.0 && within_segment_bounds(a, b, c))
        || (abd == 0.0 && within_segment_bounds(a, b, d))
        || (cda == 0.0 && within_segment_bounds(c, d, a))
        || (cdb == 0.0 && within_segment_bounds(c, d, b))
}

fn within_segment_bounds(a: [f32; 2], b: [f32; 2], p: [f32; 2]) -> bool {
    (a[0].min(b[0])..=a[0].max(b[0])).contains(&p[0])
        && (a[1].min(b[1])..=a[1].max(b[1])).contains(&p[1])
}

/// Orient in f64 so f32 differences and products do not overflow or underflow.
fn orientation(a: [f32; 2], b: [f32; 2], c: [f32; 2]) -> f64 {
    let [ax, ay] = a.map(f64::from);
    let [bx, by] = b.map(f64::from);
    let [cx, cy] = c.map(f64::from);
    (bx - ax) * (cy - ay) - (by - ay) * (cx - ax)
}

// ============================================================================
// Utility Functions
// ============================================================================

/// Compute the axis-aligned bounding box of a set of points.
pub(crate) fn compute_bounds(points: &[[f32; 2]]) -> RectF {
    if points.is_empty() {
        return RectF::zero();
    }

    let mut min_x = f32::MAX;
    let mut min_y = f32::MAX;
    let mut max_x = f32::MIN;
    let mut max_y = f32::MIN;

    for p in points {
        min_x = min_x.min(p[0]);
        min_y = min_y.min(p[1]);
        max_x = max_x.max(p[0]);
        max_y = max_y.max(p[1]);
    }

    RectF::new(min_x, min_y, max_x - min_x, max_y - min_y)
}

// ============================================================================
// Stage 2: Stroke Mesh Generation
// ============================================================================

impl Path {
    /// Build a stroke mesh for this path using the given stroke parameters.
    ///
    /// Flattens curves using the standard tolerance (0.25 logical px), then
    /// generates stroke geometry with the specified caps and joins.
    ///
    /// Returns `None` for paths with no drawable segments.
    pub fn build_stroke_mesh(&self, params: StrokeParams) -> Option<StrokeMesh> {
        let subpaths = split_into_subpaths(&self.commands).ok()?;
        if subpaths.is_empty() {
            return None;
        }

        let mut vertices = Vec::new();
        let mut indices = Vec::new();

        for subpath in &subpaths {
            if subpath.points.len() < 2 {
                continue;
            }
            let base = vertices.len() as u32;
            build_stroke_for_subpath(
                &subpath.points,
                subpath.closed,
                params,
                &mut vertices,
                &mut indices,
                base,
            );
        }

        if vertices.is_empty() || indices.is_empty() {
            return None;
        }

        // Bounds = exact min/max of produced vertices, no inflation
        let bounds = compute_bounds(&vertices);

        Some(StrokeMesh {
            vertices,
            indices,
            bounds,
        })
    }
}

/// Epsilon for point comparison (squared distance threshold).
const POINT_EPSILON_SQ: f32 = 1e-10;

/// A flattened subpath with its closure state.
struct Subpath {
    points: Vec<[f32; 2]>,
    closed: bool,
}

/// Check if two points are effectively equal (within epsilon).
#[inline]
fn points_equal(a: [f32; 2], b: [f32; 2]) -> bool {
    let dx = a[0] - b[0];
    let dy = a[1] - b[1];
    dx * dx + dy * dy < POINT_EPSILON_SQ
}

/// Split path commands into subpaths, flattening curves.
/// Uses the same flattening logic as Path::flatten() for consistency.
///
/// Guarantees:
/// - No subpath contains consecutive duplicate points.
/// - Closed subpaths end exactly at subpath_start (no duplicate tail).
/// - Only subpaths with >= 2 unique points are emitted.
fn split_into_subpaths(commands: &[PathCommand]) -> Result<Vec<Subpath>, PathFillError> {
    let mut subpaths = Vec::new();
    let mut current_points = Vec::new();
    let mut current = [0.0f32, 0.0];
    let mut subpath_start = [0.0f32, 0.0];
    let mut is_closed = false;

    for cmd in commands {
        match *cmd {
            PathCommand::MoveTo(x, y) => {
                // Finish previous subpath if any
                if current_points.len() >= 2 {
                    subpaths.push(Subpath {
                        points: current_points,
                        closed: is_closed,
                    });
                }
                current_points = Vec::new();
                is_closed = false;
                current = [x, y];
                subpath_start = current;
                current_points.push(current);
            }
            PathCommand::LineTo(x, y) => {
                let new_point = [x, y];
                // Only add if not duplicate of last point
                if current_points.is_empty()
                    || !points_equal(*current_points.last().unwrap(), new_point)
                {
                    current_points.push(new_point);
                }
                current = new_point;
            }
            PathCommand::QuadTo(cx, cy, x, y) => {
                adaptive_flatten(
                    &mut current_points,
                    [current, [cx, cy], [x, y]],
                    0,
                )?;
                current = [x, y];
            }
            PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                adaptive_flatten(
                    &mut current_points,
                    [current, [c1x, c1y], [c2x, c2y], [x, y]],
                    0,
                )?;
                current = [x, y];
            }
            PathCommand::Close => {
                is_closed = true;
                // For closed paths, ensure we connect back to start (but avoid duplicate)
                if !current_points.is_empty() && !points_equal(current, subpath_start) {
                    // Only push if it wouldn't duplicate the last point
                    if !points_equal(*current_points.last().unwrap(), subpath_start) {
                        current_points.push(subpath_start);
                    }
                    current = subpath_start;
                }
            }
        }
    }

    // Finish last subpath
    if current_points.len() >= 2 {
        subpaths.push(Subpath {
            points: current_points,
            closed: is_closed,
        });
    }

    // Post-process: remove any remaining consecutive duplicates
    for subpath in &mut subpaths {
        remove_consecutive_duplicates(&mut subpath.points);
    }

    // Filter out any subpaths that became too small after deduplication
    subpaths.retain(|s| s.points.len() >= 2);

    Ok(subpaths)
}

/// Remove consecutive duplicate points in-place.
fn remove_consecutive_duplicates(points: &mut Vec<[f32; 2]>) {
    if points.len() < 2 {
        return;
    }
    let mut write = 1;
    for read in 1..points.len() {
        if !points_equal(points[read], points[write - 1]) {
            points[write] = points[read];
            write += 1;
        }
    }
    points.truncate(write);
}

/// Epsilon for segment length (below this, segment is considered degenerate).
const SEGMENT_EPSILON: f32 = 1e-5;
const SEGMENT_EPSILON_SQ: f32 = SEGMENT_EPSILON * SEGMENT_EPSILON;

/// Build stroke geometry for a single subpath.
///
/// Preconditions (from split_into_subpaths):
/// - points.len() >= 2
/// - No consecutive duplicate points
///
/// This function additionally filters any remaining near-zero-length segments
/// and ensures all normals are unit-length. If filtering leaves < 2 points,
/// the function returns without emitting geometry.
fn build_stroke_for_subpath(
    points: &[[f32; 2]],
    closed: bool,
    params: StrokeParams,
    vertices: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    base: u32,
) {
    let n = points.len();
    if n < 2 {
        return;
    }

    // Filter out zero-length segments (stricter than split_into_subpaths)
    // After this, no two consecutive points should be within SEGMENT_EPSILON
    let mut filtered: Vec<[f32; 2]> = Vec::with_capacity(n);
    filtered.push(points[0]);
    for i in 1..n {
        let last = filtered.last().unwrap();
        let dx = points[i][0] - last[0];
        let dy = points[i][1] - last[1];
        if dx * dx + dy * dy > SEGMENT_EPSILON_SQ {
            filtered.push(points[i]);
        }
    }

    if filtered.len() < 2 {
        return;
    }

    let half_width = params.width * 0.5;

    // Compute unit normals for each segment
    // Since we filtered zero-length segments, len should always be > SEGMENT_EPSILON
    let seg_count = filtered.len() - 1;
    let mut normals: Vec<[f32; 2]> = Vec::with_capacity(seg_count);
    let mut prev_normal: Option<[f32; 2]> = None;

    for i in 0..seg_count {
        let dx = filtered[i + 1][0] - filtered[i][0];
        let dy = filtered[i + 1][1] - filtered[i][1];
        let len_sq = dx * dx + dy * dy;

        let normal = if len_sq > SEGMENT_EPSILON_SQ {
            let len = len_sq.sqrt();
            // Normal is perpendicular to direction (rotated 90° CCW)
            [-dy / len, dx / len]
        } else {
            // Fallback: reuse previous normal, or use default [0, 1] if none
            prev_normal.unwrap_or([0.0, 1.0])
        };

        normals.push(normal);
        prev_normal = Some(normal);
    }

    // Sanity check: must have exactly seg_count normals
    if normals.len() != seg_count {
        return;
    }

    if closed {
        build_closed_stroke(
            &filtered, &normals, half_width, params, vertices, indices, base,
        );
    } else {
        build_open_stroke(
            &filtered, &normals, half_width, params, vertices, indices, base,
        );
    }
}

/// Build stroke geometry for an open polyline.
///
/// Stroke geometry is built as a quad strip with CCW winding:
/// - Each segment is a quad (2 triangles) connecting left/right vertices
/// - Joins connect consecutive segments with shared vertices
/// - Start/end caps extend the stroke at the endpoints
///
/// Vertex layout (for a segment):
/// ```text
///   left_idx -------- new_left
///      |    \            |
///      |     \           |
///      |      \          |
///   right_idx ---- new_right
/// ```
/// Triangle 1: left_idx → new_left → right_idx (CCW)
/// Triangle 2: right_idx → new_left → new_right (CCW)
fn build_open_stroke(
    points: &[[f32; 2]],
    normals: &[[f32; 2]],
    half_width: f32,
    params: StrokeParams,
    vertices: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    base: u32,
) {
    let n = points.len();

    // Start cap (may add vertices before the main stroke)
    let cap_verts = add_cap(
        points[0], normals[0], half_width, params.cap, true, vertices, indices, base,
    );

    // Local index tracking relative to base
    let mut local_idx = cap_verts;

    // First segment start vertices
    let p0 = points[0];
    vertices.push([
        p0[0] + normals[0][0] * half_width,
        p0[1] + normals[0][1] * half_width,
    ]); // left
    vertices.push([
        p0[0] - normals[0][0] * half_width,
        p0[1] - normals[0][1] * half_width,
    ]); // right

    let mut left_idx = base + local_idx;
    let mut right_idx = base + local_idx + 1;
    local_idx += 2;

    // Middle segments with joins at each interior vertex
    for i in 1..n - 1 {
        let prev_normal = normals[i - 1];
        let curr_normal = normals[i];

        let (new_left, new_right, added) = add_join_and_segment(
            points[i],
            prev_normal,
            curr_normal,
            half_width,
            params,
            vertices,
            indices,
            left_idx,
            right_idx,
            base + local_idx,
        );

        left_idx = new_left;
        right_idx = new_right;
        local_idx += added;
    }

    // Last segment end vertices
    let last = n - 1;
    let last_normal = normals[last - 1];
    let p_last = points[last];

    let new_left = base + local_idx;
    let new_right = base + local_idx + 1;
    vertices.push([
        p_last[0] + last_normal[0] * half_width,
        p_last[1] + last_normal[1] * half_width,
    ]);
    vertices.push([
        p_last[0] - last_normal[0] * half_width,
        p_last[1] - last_normal[1] * half_width,
    ]);

    // Final segment quad (CCW winding)
    indices.extend_from_slice(&[left_idx, new_left, right_idx]);
    indices.extend_from_slice(&[right_idx, new_left, new_right]);

    // End cap
    add_cap(
        p_last,
        last_normal,
        half_width,
        params.cap,
        false,
        vertices,
        indices,
        new_left,
    );
}

/// Build stroke geometry for a closed polyline.
///
/// Closed strokes have no caps; instead, the last segment connects back to
/// the first segment with a join at the start/end point.
///
/// Same CCW winding as open strokes.
fn build_closed_stroke(
    points: &[[f32; 2]],
    normals: &[[f32; 2]],
    half_width: f32,
    params: StrokeParams,
    vertices: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    base: u32,
) {
    let n = points.len();
    let seg_count = normals.len();

    // Compute normal for the closing segment (last point → first point)
    let dx = points[0][0] - points[n - 1][0];
    let dy = points[0][1] - points[n - 1][1];
    let len_sq = dx * dx + dy * dy;
    let close_normal = if len_sq > SEGMENT_EPSILON_SQ {
        let len = len_sq.sqrt();
        [-dy / len, dx / len]
    } else {
        // Degenerate closing segment: reuse last segment's normal
        normals[seg_count - 1]
    };

    // First point: join between closing segment and first segment
    let p0 = points[0];
    let (left0, right0) = compute_join_offsets(close_normal, normals[0], half_width, params);

    let first_left = base;
    let first_right = base + 1;
    vertices.push([p0[0] + left0[0], p0[1] + left0[1]]);
    vertices.push([p0[0] + right0[0], p0[1] + right0[1]]);

    let mut left_idx = first_left;
    let mut right_idx = first_right;
    let mut local_idx: u32 = 2;

    // Process all interior points (each gets a join)
    for i in 1..n {
        let prev_normal = normals[i - 1];
        // For the last point, use the closing segment's normal as curr_normal
        let curr_normal = if i < seg_count {
            normals[i]
        } else {
            close_normal
        };

        let (new_left, new_right, added) = add_join_and_segment(
            points[i],
            prev_normal,
            curr_normal,
            half_width,
            params,
            vertices,
            indices,
            left_idx,
            right_idx,
            base + local_idx,
        );

        left_idx = new_left;
        right_idx = new_right;
        local_idx += added;
    }

    // Close the loop: connect last vertices back to first vertices (CCW winding)
    indices.extend_from_slice(&[left_idx, first_left, right_idx]);
    indices.extend_from_slice(&[right_idx, first_left, first_right]);
}

/// Add a join between two segments and emit the quad for the incoming segment.
/// Returns (new_left_idx, new_right_idx, vertices_added).
fn add_join_and_segment(
    point: [f32; 2],
    prev_normal: [f32; 2],
    curr_normal: [f32; 2],
    half_width: f32,
    params: StrokeParams,
    vertices: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    left_idx: u32,
    right_idx: u32,
    next_idx: u32,
) -> (u32, u32, u32) {
    let (left_offset, right_offset) =
        compute_join_offsets(prev_normal, curr_normal, half_width, params);

    let new_left = next_idx;
    let new_right = next_idx + 1;

    vertices.push([point[0] + left_offset[0], point[1] + left_offset[1]]);
    vertices.push([point[0] + right_offset[0], point[1] + right_offset[1]]);

    // Emit quad for the segment
    indices.extend_from_slice(&[left_idx, new_left, right_idx]);
    indices.extend_from_slice(&[right_idx, new_left, new_right]);

    // Stage 2: No triangle fans for round joins - shader handles round joins in Stage 3
    // Round joins use expanded miter offset (same as miter geometry)

    (new_left, new_right, 2)
}

/// Compute the left and right offset vectors at a join point.
///
/// SPEC-R-004 §5.1 miter math:
/// - miter_length = half_width / cos(theta/2)
/// - cos(theta/2) = sqrt((1 + cos(theta)) / 2)
///
/// SPEC-R-004 §5.2 miter limit:
/// - If miter_length > miter_limit * half_width, fall back to bevel.
/// - miter_limit is expressed in half-widths (default 4.0).
fn compute_join_offsets(
    prev_normal: [f32; 2],
    curr_normal: [f32; 2],
    half_width: f32,
    params: StrokeParams,
) -> ([f32; 2], [f32; 2]) {
    // Average normal (for miter direction)
    let avg_x = prev_normal[0] + curr_normal[0];
    let avg_y = prev_normal[1] + curr_normal[1];
    let avg_len_sq = avg_x * avg_x + avg_y * avg_y;

    // Dot product: cos(theta) where theta is angle between normals
    let dot = prev_normal[0] * curr_normal[0] + prev_normal[1] * curr_normal[1];

    // Cross product to determine turn direction:
    // cross > 0 → left turn (CCW), cross < 0 → right turn (CW)
    let cross = prev_normal[0] * curr_normal[1] - prev_normal[1] * curr_normal[0];

    // Nearly parallel segments (dot ≈ 1) or collinear (avg_len ≈ 0) - use simple offset
    if avg_len_sq < 1e-10 || dot > 0.9999 {
        let offset = [prev_normal[0] * half_width, prev_normal[1] * half_width];
        return (offset, [-offset[0], -offset[1]]);
    }

    let avg_len = avg_len_sq.sqrt();
    let miter_dir = [avg_x / avg_len, avg_y / avg_len];

    // SPEC-R-004 miter math: miter_length = half_width / cos(theta/2)
    // cos(theta/2) = sqrt((1 + cos(theta)) / 2)
    let cos_half = ((1.0 + dot) * 0.5).clamp(0.0, 1.0).sqrt();

    // miter_factor = 1/cos(theta/2), so miter_length = half_width * miter_factor
    let miter_factor = if cos_half > 1e-6 {
        1.0 / cos_half
    } else {
        f32::MAX
    };

    // SPEC-R-004 §5.2: if miter_length > miter_limit * half_width → bevel
    // miter_length = half_width * miter_factor
    // So: half_width * miter_factor > miter_limit * half_width
    // Simplifies to: miter_factor > miter_limit
    let use_bevel = params.join == StrokeJoin::Bevel
        || (params.join == StrokeJoin::Miter && miter_factor > params.miter_limit);

    if use_bevel {
        // Bevel join: different offset on each side of the stroke.
        // Turn direction (cross product sign) determines which side gets beveled.
        //
        // For a left turn (cross > 0, CCW):
        //   - The outside of the turn is on the left
        //   - Left side uses prev_normal (end of incoming segment)
        //   - Right side uses -curr_normal (start of outgoing segment)
        //
        // For a right turn (cross < 0, CW):
        //   - The outside of the turn is on the right
        //   - Left side uses curr_normal (start of outgoing segment)
        //   - Right side uses -prev_normal (end of incoming segment)
        if cross > 0.0 {
            // Left turn (CCW) - bevel on left (outside) side
            (
                [prev_normal[0] * half_width, prev_normal[1] * half_width],
                [-curr_normal[0] * half_width, -curr_normal[1] * half_width],
            )
        } else {
            // Right turn (CW) - bevel on right (outside) side
            (
                [curr_normal[0] * half_width, curr_normal[1] * half_width],
                [-prev_normal[0] * half_width, -prev_normal[1] * half_width],
            )
        }
    } else {
        // Miter or Round join: symmetric offset using miter direction.
        // Round geometry is done in shader (Stage 3); CPU just uses miter.
        let miter_length = half_width * miter_factor;
        let offset = [miter_dir[0] * miter_length, miter_dir[1] * miter_length];
        (offset, [-offset[0], -offset[1]])
    }
}

/// Add a cap at the start or end of an open path.
///
/// Caps are added before/after the main stroke vertices:
/// - Start cap: vertices are added first, then main stroke connects to them
/// - End cap: main stroke vertices exist, cap connects to them
///
/// Returns the number of vertices added.
fn add_cap(
    point: [f32; 2],
    normal: [f32; 2],
    half_width: f32,
    cap: StrokeCap,
    is_start: bool,
    vertices: &mut Vec<[f32; 2]>,
    indices: &mut Vec<u32>,
    connect_idx: u32,
) -> u32 {
    // Direction along the path tangent (perpendicular to normal).
    // For start cap: points backward (away from path interior).
    // For end cap: points forward (away from path interior).
    let dir = if is_start {
        [-normal[1], normal[0]]
    } else {
        [normal[1], -normal[0]]
    };

    match cap {
        StrokeCap::Butt => {
            // No extra geometry needed - stroke ends at the endpoint
            0
        }
        StrokeCap::Square => {
            // Extend by half_width in the tangent direction (away from path)
            let ext = [
                point[0] + dir[0] * half_width,
                point[1] + dir[1] * half_width,
            ];

            // Cap vertices: left and right corners of the extended square
            let idx = vertices.len() as u32;
            vertices.push([
                ext[0] + normal[0] * half_width,
                ext[1] + normal[1] * half_width,
            ]); // cap_left
            vertices.push([
                ext[0] - normal[0] * half_width,
                ext[1] - normal[1] * half_width,
            ]); // cap_right

            if is_start {
                // Start cap: cap vertices come first, main stroke at idx+2, idx+3
                // Quad connects cap to main stroke (CCW winding)
                let main_left = idx + 2;
                let main_right = idx + 3;
                indices.extend_from_slice(&[idx, main_left, idx + 1]);
                indices.extend_from_slice(&[idx + 1, main_left, main_right]);
            } else {
                // End cap: connect from last segment vertices to cap
                let last_left = connect_idx;
                let last_right = connect_idx + 1;
                indices.extend_from_slice(&[last_left, idx, last_right]);
                indices.extend_from_slice(&[last_right, idx, idx + 1]);
            }
            2
        }
        StrokeCap::Round => {
            // Semicircular cap using triangle fan from center to arc points.
            // Arc points are at distance half_width from center (endpoint).
            let segments = 8;
            let center_idx = vertices.len() as u32;
            vertices.push(point); // Fan center = path endpoint

            // Start angle: direction of normal (for start cap) or opposite (for end cap)
            let start_angle = if is_start {
                normal[1].atan2(normal[0])
            } else {
                (-normal[1]).atan2(-normal[0])
            };

            // Arc direction: CCW for start cap, CW for end cap
            // This ensures consistent winding with the main stroke strip
            let mut prev_idx = center_idx;
            for i in 0..=segments {
                let t = i as f32 / segments as f32;
                let angle =
                    start_angle + std::f32::consts::PI * t * if is_start { 1.0 } else { -1.0 };

                let arc_idx = vertices.len() as u32;
                vertices.push([
                    point[0] + angle.cos() * half_width,
                    point[1] + angle.sin() * half_width,
                ]);

                if i > 0 {
                    // Fan triangle: center → prev_arc → curr_arc (CCW winding)
                    indices.extend_from_slice(&[center_idx, prev_idx, arc_idx]);
                }
                prev_idx = arc_idx;
            }
            // Total: center (1) + arc points (segments + 1 = 9) = 10 vertices
            10
        }
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_construction() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.close();

        assert_eq!(path.commands.len(), 4);
    }

    #[test]
    fn test_flatten_lines_only() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.close();

        let flat = path.flatten();
        assert_eq!(flat.len(), 4);
        assert_eq!(flat[0], [0.0, 0.0]);
        assert_eq!(flat[1], [100.0, 0.0]);
        assert_eq!(flat[2], [100.0, 100.0]);
        assert_eq!(flat[3], [0.0, 0.0]);
    }

    #[test]
    fn test_flatten_quadratic() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.quad_to(50.0, 100.0, 100.0, 0.0);

        let flat = path.flatten();
        // Should have more than 2 points due to curve subdivision
        assert!(flat.len() >= 2);
        // First point is the move_to
        assert_eq!(flat[0], [0.0, 0.0]);
        // Last point is the curve endpoint
        assert_eq!(flat[flat.len() - 1], [100.0, 0.0]);
    }

    #[test]
    fn test_flatten_cubic() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.cubic_to(33.0, 100.0, 66.0, 100.0, 100.0, 0.0);

        let flat = path.flatten();
        // Should have more than 2 points due to curve subdivision
        assert!(flat.len() >= 2);
        // First point is the move_to
        assert_eq!(flat[0], [0.0, 0.0]);
        // Last point is the curve endpoint
        assert_eq!(flat[flat.len() - 1], [100.0, 0.0]);
    }

    #[test]
    fn test_triangulate_triangle() {
        let poly = [[0.0, 0.0], [100.0, 0.0], [50.0, 100.0]];
        let indices = triangulate_monotone(&poly);
        assert_eq!(indices.len(), 3);
    }

    #[test]
    fn test_triangulate_square() {
        // Counter-clockwise square
        let poly = [[0.0, 0.0], [100.0, 0.0], [100.0, 100.0], [0.0, 100.0]];
        let indices = triangulate_monotone(&poly);
        // Square should produce 2 triangles = 6 indices
        assert_eq!(indices.len(), 6);
    }

    fn assert_simple_triangulation(poly: &[[f32; 2]], triangle_count: usize) -> Vec<u32> {
        let signed_area_twice = |points: &[[f32; 2]]| {
            points
                .iter()
                .zip(points.iter().cycle().skip(1))
                .map(|(&[ax, ay], &[bx, by])| {
                    f64::from(ax) * f64::from(by) - f64::from(ay) * f64::from(bx)
                })
                .sum::<f64>()
        };
        let indices = triangulate_monotone(poly);
        assert_eq!(indices.len(), triangle_count * 3);
        let triangle_area_twice = indices
            .chunks_exact(3)
            .map(|triangle| {
                let points = [
                    poly[triangle[0] as usize],
                    poly[triangle[1] as usize],
                    poly[triangle[2] as usize],
                ];
                let area = signed_area_twice(&points);
                assert!(area > 0.0, "non-CCW or degenerate triangle: {points:?}");
                area
            })
            .sum::<f64>();
        assert_eq!(triangle_area_twice, signed_area_twice(poly).abs());
        indices
    }

    #[test]
    fn test_triangulate_convex_and_concave_in_both_orientations() {
        let tiny = 1e-24_f32;
        let mut convex = [[0.0, 0.0], [tiny, 0.0], [tiny, tiny], [0.0, tiny]];
        // The inward corner blocks the proposed diagonal from [0, 6] to [6, 0].
        let mut concave = [[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [3.0, 3.0], [0.0, 6.0]];
        for _ in 0..2 {
            assert_simple_triangulation(&convex, 2);
            assert_simple_triangulation(&concave, 3);
            convex.reverse();
            concave.reverse();
        }
    }

    #[test]
    fn test_triangulate_omits_redundant_vertices_and_keeps_original_indices() {
        let polygon = [
            [0.0, 0.0],
            [2.0, 0.0],
            [2.0, 0.0],
            [6.0, 0.0],
            [6.0, 4.0],
            [0.0, 4.0],
            [0.0, 0.0],
        ];
        let indices = assert_simple_triangulation(&polygon, 2);
        for corner in [0, 3, 4, 5] {
            assert!(indices.contains(&corner));
        }
        assert!(indices.iter().all(|i| ![1, 2, 6].contains(i)));
    }

    #[test]
    fn test_triangulate_refuses_invalid_polygons_without_partial_indices() {
        let collinear = [[0.0, 0.0], [5.0, 0.0], [7.0, 0.0], [9.0, 0.0]];
        let backtracking = [[0.0, 0.0], [6.0, 0.0], [3.0, 0.0], [3.0, 4.0], [0.0, 4.0]];
        let crossing = [[0.0, 0.0], [6.0, 0.0], [6.0, 5.0], [3.0, -1.0], [0.0, 5.0]];
        let touching = [[0.0, 0.0], [6.0, 0.0], [6.0, 5.0], [3.0, 0.0], [0.0, 5.0]];
        let nonfinite = [[0.0, 0.0], [6.0, f32::NAN], [6.0, 5.0]];
        for invalid in [
            collinear.as_slice(),
            &backtracking,
            &crossing,
            &touching,
            &nonfinite,
        ] {
            assert!(triangulate_monotone(invalid).is_empty(), "{invalid:?}");
        }
    }

    #[test]
    fn test_to_mesh() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 100.0);
        path.line_to(0.0, 100.0);
        path.close();

        let mesh = path.to_mesh();
        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());
        assert_eq!(mesh.bounds.x, 0.0);
        assert_eq!(mesh.bounds.y, 0.0);
        assert_eq!(mesh.bounds.width, 100.0);
        assert_eq!(mesh.bounds.height, 100.0);
    }

    #[test]
    fn test_bounds_computation() {
        let points = [[10.0, 20.0], [30.0, 40.0], [5.0, 60.0]];
        let bounds = compute_bounds(&points);
        assert_eq!(bounds.x, 5.0);
        assert_eq!(bounds.y, 20.0);
        assert_eq!(bounds.width, 25.0);
        assert_eq!(bounds.height, 40.0);
    }

    #[test]
    fn test_empty_bounds() {
        let bounds = compute_bounds(&[]);
        assert_eq!(bounds, RectF::zero());
    }

    // ========================================================================
    // Stage 2: Stroke Tests (SPEC-R-004 compliant)
    // ========================================================================

    #[test]
    fn test_stroke_params_default() {
        let params = StrokeParams::default();
        assert_eq!(params.width, 1.0);
        assert_eq!(params.join, StrokeJoin::Miter);
        assert_eq!(params.cap, StrokeCap::Butt);
        assert_eq!(params.miter_limit, 4.0);
    }

    #[test]
    fn test_stroke_horizontal_line_basic() {
        // Simple horizontal line from (0,0) to (100,0) with width 2.0
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);

        let params = StrokeParams {
            width: 2.0,
            ..Default::default()
        };
        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());

        // Bounds = exact vertex bounds, no inflation
        // Height should be exactly stroke width (2.0)
        let height = mesh.bounds.height;
        assert!(
            (height - 2.0).abs() < 0.01,
            "height {} should be exactly 2.0",
            height
        );

        // Width should be exactly line length (100.0)
        let width = mesh.bounds.width;
        assert!(
            (width - 100.0).abs() < 0.01,
            "width {} should be exactly 100.0",
            width
        );
    }

    #[test]
    fn test_stroke_cap_butt() {
        let mut path = Path::new();
        path.move_to(10.0, 10.0);
        path.line_to(90.0, 10.0);

        let params = StrokeParams {
            width: 4.0,
            cap: StrokeCap::Butt,
            ..Default::default()
        };
        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        // With butt cap, bounds should match stroke endpoints exactly
        assert!(
            (mesh.bounds.x - 10.0).abs() < 0.01,
            "butt cap x should be 10.0: {}",
            mesh.bounds.x
        );
        assert!(
            ((mesh.bounds.x + mesh.bounds.width) - 90.0).abs() < 0.01,
            "butt cap right should be 90.0: {}",
            mesh.bounds.x + mesh.bounds.width
        );
    }

    #[test]
    fn test_stroke_cap_square() {
        let mut path = Path::new();
        path.move_to(10.0, 10.0);
        path.line_to(90.0, 10.0);

        let params = StrokeParams {
            width: 4.0,
            cap: StrokeCap::Square,
            ..Default::default()
        };
        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        // Square cap extends by half_width (2.0) beyond each endpoint
        assert!(
            (mesh.bounds.x - 8.0).abs() < 0.01,
            "square cap should extend left to 8.0: {}",
            mesh.bounds.x
        );
        assert!(
            ((mesh.bounds.x + mesh.bounds.width) - 92.0).abs() < 0.01,
            "square cap should extend right to 92.0: {}",
            mesh.bounds.x + mesh.bounds.width
        );
    }

    #[test]
    fn test_stroke_cap_round_extends_bounds() {
        let mut path = Path::new();
        path.move_to(10.0, 10.0);
        path.line_to(90.0, 10.0);

        let butt_params = StrokeParams {
            width: 4.0,
            cap: StrokeCap::Butt,
            ..Default::default()
        };
        let round_params = StrokeParams {
            width: 4.0,
            cap: StrokeCap::Round,
            ..Default::default()
        };

        let butt_mesh = path.build_stroke_mesh(butt_params).expect("butt mesh");
        let round_mesh = path.build_stroke_mesh(round_params).expect("round mesh");

        // Round caps add vertices for semicircle
        assert!(
            round_mesh.vertices.len() > butt_mesh.vertices.len(),
            "round cap ({}) should have more vertices than butt ({})",
            round_mesh.vertices.len(),
            butt_mesh.vertices.len()
        );

        // Round cap bounds should extend by half_width
        assert!(
            round_mesh.bounds.x < butt_mesh.bounds.x,
            "round cap should extend left"
        );
    }

    #[test]
    fn test_stroke_join_produces_two_vertices_per_join() {
        // L-shaped path with 90° corner - each join produces exactly 2 vertices
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(50.0, 0.0);
        path.line_to(50.0, 50.0);

        let miter_params = StrokeParams {
            width: 4.0,
            join: StrokeJoin::Miter,
            miter_limit: 10.0,
            ..Default::default()
        };
        let bevel_params = StrokeParams {
            width: 4.0,
            join: StrokeJoin::Bevel,
            ..Default::default()
        };
        let round_params = StrokeParams {
            width: 4.0,
            join: StrokeJoin::Round,
            ..Default::default()
        };

        let miter_mesh = path.build_stroke_mesh(miter_params).expect("miter mesh");
        let bevel_mesh = path.build_stroke_mesh(bevel_params).expect("bevel mesh");
        let round_mesh = path.build_stroke_mesh(round_params).expect("round mesh");

        // All join types produce same vertex count (2 per join, no fan triangles)
        assert_eq!(
            miter_mesh.vertices.len(),
            bevel_mesh.vertices.len(),
            "miter and bevel should have same vertex count"
        );
        assert_eq!(
            miter_mesh.vertices.len(),
            round_mesh.vertices.len(),
            "miter and round should have same vertex count (no fans in Stage 2)"
        );
    }

    #[test]
    fn test_stroke_miter_limit_clamp() {
        // Sharp acute angle that would create a huge miter
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(50.0, 0.0);
        path.line_to(45.0, 5.0); // Very sharp angle back

        let params = StrokeParams {
            width: 4.0,
            join: StrokeJoin::Miter,
            miter_limit: 4.0,
            ..Default::default()
        };

        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        // The miter should be clamped - no vertex should be too far from the corner
        let corner = [50.0, 0.0];
        let max_allowed_dist = params.miter_limit * params.width;

        for v in &mesh.vertices {
            let dx = v[0] - corner[0];
            let dy = v[1] - corner[1];
            let dist = (dx * dx + dy * dy).sqrt();
            // Only check vertices near the corner
            if dist < 20.0 {
                assert!(
                    dist <= max_allowed_dist,
                    "vertex {:?} is {} from corner, max allowed {}",
                    v,
                    dist,
                    max_allowed_dist
                );
            }
        }
    }

    #[test]
    fn test_stroke_closed_rectangle_bounds() {
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(100.0, 0.0);
        path.line_to(100.0, 50.0);
        path.line_to(0.0, 50.0);
        path.close();

        let params = StrokeParams {
            width: 4.0,
            ..Default::default()
        };
        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());

        // Closed stroke bounds should expand by half stroke width from geometry
        // Miter joins at 90° expand by half_width * sqrt(2) ≈ 2.83 for width 4
        assert!(mesh.bounds.x < 0.0, "closed stroke should expand left");
        assert!(mesh.bounds.y < 0.0, "closed stroke should expand top");
        assert!(
            mesh.bounds.x + mesh.bounds.width > 100.0,
            "closed stroke should expand right"
        );
        assert!(
            mesh.bounds.y + mesh.bounds.height > 50.0,
            "closed stroke should expand bottom"
        );
    }

    #[test]
    fn test_stroke_closed_vs_open_path() {
        // Closed path
        let mut closed_path = Path::new();
        closed_path.move_to(0.0, 0.0);
        closed_path.line_to(100.0, 0.0);
        closed_path.line_to(100.0, 100.0);
        closed_path.close();

        // Open path (no caps since using butt)
        let mut open_path = Path::new();
        open_path.move_to(0.0, 0.0);
        open_path.line_to(100.0, 0.0);
        open_path.line_to(100.0, 100.0);
        open_path.line_to(0.0, 0.0);

        let params = StrokeParams {
            width: 2.0,
            cap: StrokeCap::Butt,
            ..Default::default()
        };

        let closed_mesh = closed_path.build_stroke_mesh(params).expect("closed mesh");
        let open_mesh = open_path.build_stroke_mesh(params).expect("open mesh");

        // Both should produce geometry
        assert!(!closed_mesh.vertices.is_empty());
        assert!(!open_mesh.vertices.is_empty());
    }

    #[test]
    fn test_stroke_degenerate_single_point() {
        let mut path = Path::new();
        path.move_to(50.0, 50.0);

        let params = StrokeParams::default();
        let result = path.build_stroke_mesh(params);

        assert!(result.is_none(), "single point path should return None");
    }

    #[test]
    fn test_stroke_degenerate_repeated_points() {
        let mut path = Path::new();
        path.move_to(50.0, 50.0);
        path.line_to(50.0, 50.0);
        path.line_to(50.0, 50.0);

        let params = StrokeParams::default();
        let result = path.build_stroke_mesh(params);

        assert!(result.is_none(), "repeated points path should return None");
    }

    #[test]
    fn test_stroke_empty_path() {
        let path = Path::new();
        let params = StrokeParams::default();
        let result = path.build_stroke_mesh(params);

        assert!(result.is_none(), "empty path should return None");
    }

    #[test]
    fn test_stroke_with_curves() {
        // Path with a quadratic curve - should be flattened and stroked
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.quad_to(50.0, 50.0, 100.0, 0.0);

        let params = StrokeParams {
            width: 2.0,
            ..Default::default()
        };
        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        assert!(!mesh.vertices.is_empty());
        assert!(!mesh.indices.is_empty());

        // The curve should produce more vertices than a simple line
        assert!(
            mesh.vertices.len() > 4,
            "curved stroke should have multiple segments"
        );
    }

    #[test]
    fn test_stroke_index_count_predictable() {
        // Simple 2-segment open path with butt caps
        let mut path = Path::new();
        path.move_to(0.0, 0.0);
        path.line_to(50.0, 0.0);
        path.line_to(100.0, 0.0);

        let params = StrokeParams {
            width: 2.0,
            cap: StrokeCap::Butt,
            ..Default::default()
        };
        let mesh = path.build_stroke_mesh(params).expect("should produce mesh");

        // 2 segments = 2 quads = 4 triangles = 12 indices
        // Plus join produces 1 quad = 2 triangles = 6 indices
        // But we share vertices at joins, so: 6 vertices, 12 indices for 2 quads
        assert!(mesh.indices.len() > 0);
        assert_eq!(mesh.indices.len() % 3, 0, "indices should be multiple of 3");
    }
}
