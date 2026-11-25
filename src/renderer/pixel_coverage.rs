use crate::core::math::{RectF, Size};

/// Integer target extent in physical pixels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalExtent {
    pub width: u32,
    pub height: u32,
}

impl PhysicalExtent {
    pub const fn new(width: u32, height: u32) -> Self {
        Self { width, height }
    }

    pub const fn is_empty(self) -> bool {
        self.width == 0 || self.height == 0
    }
}

/// Half-open target-pixel rectangle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PhysicalRect {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}

impl PhysicalRect {
    pub const fn new(x: u32, y: u32, width: u32, height: u32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    pub const fn extent(self) -> PhysicalExtent {
        PhysicalExtent::new(self.width, self.height)
    }

    pub const fn is_empty(&self) -> bool {
        self.width == 0 || self.height == 0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum PixelCoverageError {
    TargetNotRegistered,
    InvalidLogicalTargetExtent,
    InvalidPhysicalTargetExtent,
    InvalidScaleFactor,
    InvalidLogicalRect,
    PhysicalExtentOverflow,
}

impl std::fmt::Display for PixelCoverageError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let message = match self {
            Self::TargetNotRegistered => "render target is not registered",
            Self::InvalidLogicalTargetExtent => "logical target extent must be positive and finite",
            Self::InvalidPhysicalTargetExtent => "physical target extent must be non-zero",
            Self::InvalidScaleFactor => "target scale factor must be positive and finite",
            Self::InvalidLogicalRect => "logical rectangle must be finite",
            Self::PhysicalExtentOverflow => "logical rectangle exceeds the physical extent range",
        };
        f.write_str(message)
    }
}

impl std::error::Error for PixelCoverageError {}

/// Immutable value inputs for the renderer's sole logical-to-target-pixel conversion.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct PhysicalTargetMetrics {
    logical_extent: Size,
    physical_extent: PhysicalExtent,
    scale_factor: f32,
}

impl PhysicalTargetMetrics {
    pub(crate) fn new(
        logical_extent: Size,
        physical_extent: PhysicalExtent,
        scale_factor: f32,
    ) -> Result<Self, PixelCoverageError> {
        if !logical_extent.width.is_finite()
            || !logical_extent.height.is_finite()
            || logical_extent.width <= 0.0
            || logical_extent.height <= 0.0
        {
            return Err(PixelCoverageError::InvalidLogicalTargetExtent);
        }
        if physical_extent.is_empty() {
            return Err(PixelCoverageError::InvalidPhysicalTargetExtent);
        }
        if !scale_factor.is_finite() || scale_factor <= 0.0 {
            return Err(PixelCoverageError::InvalidScaleFactor);
        }
        Ok(Self {
            logical_extent,
            physical_extent,
            scale_factor,
        })
    }

    pub(crate) const fn logical_extent(self) -> Size {
        self.logical_extent
    }

    pub(crate) const fn physical_extent(self) -> PhysicalExtent {
        self.physical_extent
    }

    pub(crate) const fn scale_factor(self) -> f32 {
        self.scale_factor
    }
}

fn validate_rect(rect: RectF) -> Result<(), PixelCoverageError> {
    if rect.x.is_finite()
        && rect.y.is_finite()
        && rect.width.is_finite()
        && rect.height.is_finite()
        && rect.right().is_finite()
        && rect.bottom().is_finite()
    {
        Ok(())
    } else {
        Err(PixelCoverageError::InvalidLogicalRect)
    }
}

/// Derive a target-density offscreen extent from a logical size.
pub(crate) fn physical_render_extent(
    logical_size: Size,
    target: PhysicalTargetMetrics,
) -> Result<Option<PhysicalExtent>, PixelCoverageError> {
    if !logical_size.width.is_finite() || !logical_size.height.is_finite() {
        return Err(PixelCoverageError::InvalidLogicalRect);
    }
    if logical_size.width <= 0.0 || logical_size.height <= 0.0 {
        return Ok(None);
    }

    // Allocation density is a projection of participant-local size, independent
    // of absolute raster phase and clipping. Destination coverage is derived
    // separately from absolute edges by physical_composition_coverage().
    let width = (logical_size.width * target.scale_factor()).round();
    let height = (logical_size.height * target.scale_factor()).round();
    if !width.is_finite()
        || !height.is_finite()
        || width > u32::MAX as f32
        || height > u32::MAX as f32
    {
        return Err(PixelCoverageError::PhysicalExtentOverflow);
    }
    let extent = PhysicalExtent::new(width as u32, height as u32);
    Ok((!extent.is_empty()).then_some(extent))
}

fn raster_edge_index(edge: f32, scale_factor: f32) -> Result<i64, PixelCoverageError> {
    let scaled = edge * scale_factor;
    if !scaled.is_finite() || scaled.abs() > i64::MAX as f32 {
        return Err(PixelCoverageError::PhysicalExtentOverflow);
    }
    // WebGPU samples at pixel centers. The half-open logical edge coverage is
    // therefore the integer interval bounded by ceil(edge * DPR - 0.5).
    Ok((scaled - 0.5).ceil() as i64)
}

/// Convert a logical rect to the exact half-open target-pixel region covered by
/// an axis-aligned production primitive under the target projection.
pub(crate) fn physical_composition_coverage(
    rect: RectF,
    target: PhysicalTargetMetrics,
) -> Result<Option<PhysicalRect>, PixelCoverageError> {
    validate_rect(rect)?;
    if rect.width <= 0.0 || rect.height <= 0.0 {
        return Ok(None);
    }

    let target_bounds = RectF::new(
        0.0,
        0.0,
        target.logical_extent().width,
        target.logical_extent().height,
    );
    let Some(clipped) = rect.intersection(&target_bounds) else {
        return Ok(None);
    };

    let physical = target.physical_extent();
    let x0 = raster_edge_index(clipped.left(), target.scale_factor())?
        .clamp(0, i64::from(physical.width));
    let y0 = raster_edge_index(clipped.top(), target.scale_factor())?
        .clamp(0, i64::from(physical.height));
    let x1 = raster_edge_index(clipped.right(), target.scale_factor())?
        .clamp(0, i64::from(physical.width));
    let y1 = raster_edge_index(clipped.bottom(), target.scale_factor())?
        .clamp(0, i64::from(physical.height));

    if x1 <= x0 || y1 <= y0 {
        return Ok(None);
    }
    Ok(Some(PhysicalRect::new(
        x0 as u32,
        y0 as u32,
        (x1 - x0) as u32,
        (y1 - y0) as u32,
    )))
}
