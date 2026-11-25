//! Checked compound Path fill geometry (SPEC-R-004 §13).

use lyon_tessellation::math::point;
use lyon_tessellation::{
    BuffersBuilder, FillOptions, FillRule as LyonFillRule, FillTessellator, FillVertex,
    VertexBuffers,
};

use super::{
    FillRule, Path, PathCommand, PathFillError, PathMesh, adaptive_flatten, compute_bounds,
    finite_point,
};
use crate::core::math::RectF;
use crate::renderer::mesh_cache::PixelSnapMode;

/// Flattened source points with explicit MoveTo boundaries.
pub(in crate::renderer) struct FillContours {
    points: Vec<[f32; 2]>,
    starts: Vec<usize>,
    pub(in crate::renderer) bounds: RectF,
}

impl FillContours {
    pub(super) fn new(path: &Path) -> Result<Self, PathFillError> {
        let mut points = Vec::new();
        let mut starts = Vec::new();
        for command in &path.commands {
            match *command {
                PathCommand::MoveTo(x, y) => {
                    starts.push(points.len());
                    points.push(checked_point(x, y)?);
                }
                PathCommand::LineTo(x, y) => {
                    current_point(&points)?;
                    points.push(checked_point(x, y)?);
                }
                PathCommand::QuadTo(cx, cy, x, y) => {
                    let current = current_point(&points)?;
                    adaptive_flatten(&mut points, [current, [cx, cy], [x, y]], 0)?;
                }
                PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                    let current = current_point(&points)?;
                    adaptive_flatten(&mut points, [current, [c1x, c1y], [c2x, c2y], [x, y]], 0)?;
                }
                PathCommand::Close => {
                    let start = *starts.last().ok_or(PathFillError::InvalidSource)?;
                    if current_point(&points)? != points[start] {
                        points.push(points[start]);
                    }
                }
            }
        }
        let bounds = compute_bounds(&points);
        if !bounds.width.is_finite() || !bounds.height.is_finite() {
            return Err(PathFillError::InvalidSource);
        }
        Ok(Self {
            points,
            starts,
            bounds,
        })
    }

    pub(in crate::renderer) fn validate_scale(dpr: f32) -> Result<(), PathFillError> {
        if !dpr.is_finite() || dpr <= 0.0 {
            return Err(PathFillError::InvalidScale);
        }
        Ok(())
    }

    pub(in crate::renderer) fn tessellate(
        mut self,
        rule: FillRule,
        snap_mode: PixelSnapMode,
        dpr: f32,
    ) -> Result<PathMesh, PathFillError> {
        self.project(snap_mode, dpr)?;
        self.tessellate_projected(rule)
    }

    pub(in crate::renderer) fn project(
        &mut self,
        snap_mode: PixelSnapMode,
        dpr: f32,
    ) -> Result<(), PathFillError> {
        Self::validate_scale(dpr)?;
        // INV-R004-ONE-COVERAGE: project the authored edges before the sweep.
        // The sweep's intersection vertices are left in their computed positions.
        let snap: Option<fn(f32) -> f32> = match snap_mode {
            PixelSnapMode::None => None,
            PixelSnapMode::Center => Some(f32::round),
            PixelSnapMode::Edge => Some(|value| value.floor() + 0.5),
        };
        if let Some(snap) = snap {
            for vertex in &mut self.points {
                *vertex = vertex.map(|value| snap(value * dpr) / dpr);
                if !finite_point(*vertex) {
                    return Err(PathFillError::InvalidScale);
                }
            }
            let projected_bounds = compute_bounds(&self.points);
            if !projected_bounds.width.is_finite() || !projected_bounds.height.is_finite() {
                return Err(PathFillError::InvalidScale);
            }
        }
        Ok(())
    }

    fn contours(&self) -> impl Iterator<Item = &[[f32; 2]]> {
        let ends = self
            .starts
            .iter()
            .copied()
            .skip(1)
            .chain([self.points.len()]);
        self.starts
            .iter()
            .copied()
            .zip(ends)
            .map(|(start, end)| &self.points[start..end])
    }

    pub(in crate::renderer) fn edges(&self) -> impl Iterator<Item = ([f32; 2], [f32; 2])> + '_ {
        self.contours().flat_map(|contour| {
            let points = contour.iter().copied();
            points
                .clone()
                .zip(points.cycle().skip(1))
                .filter(|(a, b)| a != b)
        })
    }

    pub(in crate::renderer) fn tessellate_projected(
        &self,
        rule: FillRule,
    ) -> Result<PathMesh, PathFillError> {
        let mut path = lyon_tessellation::path::Path::builder();
        for contour in self.contours() {
            if contour.len() < 3 {
                continue;
            }
            let first = contour[0];
            let end = contour.len() - usize::from(contour.last() == Some(&first));
            path.begin(point(first[0], first[1]));
            let mut previous = first;
            for &[x, y] in &contour[1..end] {
                if [x, y] != previous {
                    path.line_to(point(x, y));
                    previous = [x, y];
                }
            }
            path.end(true);
        }
        let lyon_rule = match rule {
            FillRule::NonZero => LyonFillRule::NonZero,
            FillRule::EvenOdd => LyonFillRule::EvenOdd,
        };
        let mut options = FillOptions::default().with_fill_rule(lyon_rule);
        options.handle_intersections = true;
        let mut buffers = VertexBuffers::<[f32; 2], u32>::new();
        FillTessellator::new()
            .tessellate_path(
                &path.build(),
                &options,
                &mut BuffersBuilder::new(&mut buffers, |vertex: FillVertex| {
                    let position = vertex.position();
                    [position.x, position.y]
                }),
            )
            .map_err(|_| PathFillError::TessellationFailed)?;
        validate_mesh(buffers)
    }
}

fn checked_point(x: f32, y: f32) -> Result<[f32; 2], PathFillError> {
    finite_point([x, y])
        .then_some([x, y])
        .ok_or(PathFillError::InvalidSource)
}

fn current_point(points: &[[f32; 2]]) -> Result<[f32; 2], PathFillError> {
    points.last().copied().ok_or(PathFillError::InvalidSource)
}

fn validate_mesh(buffers: VertexBuffers<[f32; 2], u32>) -> Result<PathMesh, PathFillError> {
    let triangles = buffers.indices.chunks_exact(3);
    if !triangles.remainder().is_empty()
        || u32::try_from(buffers.indices.len()).is_err()
        || !buffers.vertices.iter().copied().all(finite_point)
    {
        return Err(PathFillError::InvalidMesh);
    }

    // Only referenced vertices contribute to the resulting mesh and its bounds.
    let mut vertices = Vec::new();
    let mut indices = Vec::with_capacity(buffers.indices.len());
    let mut remap = vec![u32::MAX; buffers.vertices.len()];
    for triangle in triangles {
        let mut corners = [triangle[0], triangle[1], triangle[2]];
        let [Some(a), Some(b), Some(c)] = corners.map(|index| buffers.vertices.get(index as usize))
        else {
            return Err(PathFillError::InvalidMesh);
        };
        let [ax, ay] = a.map(f64::from);
        let [bx, by] = b.map(f64::from);
        let [cx, cy] = c.map(f64::from);
        let area = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
        if area == 0.0 {
            continue;
        }
        if area < 0.0 {
            corners.swap(1, 2);
        }
        for index in corners {
            let mapped = &mut remap[index as usize];
            if *mapped == u32::MAX {
                *mapped = u32::try_from(vertices.len()).map_err(|_| PathFillError::InvalidMesh)?;
                vertices.push(buffers.vertices[index as usize]);
            }
            indices.push(*mapped);
        }
    }
    let bounds = compute_bounds(&vertices);
    if !bounds.width.is_finite() || !bounds.height.is_finite() {
        return Err(PathFillError::InvalidMesh);
    }
    Ok(PathMesh {
        vertices,
        indices,
        bounds,
    })
}
