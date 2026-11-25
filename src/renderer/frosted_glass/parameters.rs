use super::FrostedGlassCompileRefusal as Refusal;
use crate::core::scene_identity::SceneNodeKey;
use crate::renderer::{
    batcher::{FlatPrimitive, PrimitiveData},
    render_target::TargetDescriptor,
    wgpu_renderer::RoundedClipUniform,
};
use bytemuck::{Pod, Zeroable};
use crate::renderer::pipelines::analytic_surface::{GlassSurfaceOpticsParams, SurfaceEmissionParams};

/// HAL085A / HAL385 §8.7. No dynamic offset and no hidden lanes.
#[repr(C)]
#[derive(Debug, Clone, Copy, Pod, Zeroable)]
pub(crate) struct FrostedGlassParams {
    pub rect: [f32; 4],
    pub corners: [f32; 4],
    pub tint: [f32; 4],
    pub optics: [f32; 4],
    pub target: [u32; 4],
    pub visible: [u32; 4],
    pub snapshot: [u32; 4],
    pub control: [u32; 4],
    pub grain: [f32; 4],
    pub grain_seed: [u32; 4],
    pub ancestor: RoundedClipUniform,
    pub reserved: [u32; 4],
    pub weights: [[f32; 4]; 49],
    pub filtered_domain: [u32; 4],
    /// Opacity, refraction, inward edge feather, reserved (HAL-385 §12).
    pub surface_controls: [f32; 4],
    pub extensions: [u32; 4],
    pub glass_optics: GlassSurfaceOpticsParams,
    pub emission: SurfaceEmissionParams,
    pub film: [f32; 4],
}
#[derive(Debug, Clone)]
pub(crate) struct Occurrence {
    pub ordinal: u64,
    #[allow(
        dead_code,
        reason = "Retained source occurrence attribution for HAL-385 compile diagnostics"
    )]
    pub node_key: Option<SceneNodeKey>,
    pub params: FrostedGlassParams,
}
impl FrostedGlassParams {
    pub fn terminal(size: (u32, u32)) -> Self {
        let mut p = Self::zeroed();
        p.target = [size.0, size.1, 0, 0];
        p
    }
    #[cfg(test)]
    pub fn radius(&self) -> u32 {
        self.control[0]
    }
    pub fn bytes(&self) -> Option<u64> {
        let area = |r: [u32; 4]| u64::from(r[2]).checked_mul(u64::from(r[3]));
        area(self.snapshot)?
            .checked_mul(12)?
            .checked_add(area(self.filtered_domain)?.checked_mul(8)?)?
            .checked_add(std::mem::size_of::<Self>() as u64)
    }
    #[cfg(test)]
    pub fn weight(&self, k: u32) -> f32 {
        self.weights[k as usize / 4][k as usize % 4]
    }
}

pub(crate) fn supported_format(format: wgpu::TextureFormat) -> bool {
    matches!(
        format,
        wgpu::TextureFormat::Bgra8Unorm
            | wgpu::TextureFormat::Bgra8UnormSrgb
            | wgpu::TextureFormat::Rgba8Unorm
            | wgpu::TextureFormat::Rgba8UnormSrgb
    )
}
pub(crate) fn occurrence(
    p: &FlatPrimitive,
    target: &TargetDescriptor,
    aa: f32,
) -> Result<Option<Occurrence>, Refusal> {
    let PrimitiveData::FrostedGlass {
        rect,
        corners,
        appearance,
        ancestor,
        ordinal,
        node_key,
    } = &p.data
    else {
        return Ok(None);
    };
    let d = target.scale_factor;
    if target.physical_size.0 == 0
        || target.physical_size.1 == 0
        || !d.is_finite()
        || !(0.25..=8.0).contains(&d)
        || !aa.is_finite()
        || aa < 0.0
    {
        return Err(Refusal::InvalidTarget);
    }
    if appearance.validate_geometry(*rect, *corners).is_err() {
        return Err(Refusal::InvalidTarget);
    }
    let logical_margin = appearance.halo_extent() + 2.0 / d;
    if [rect.x - logical_margin, rect.y - logical_margin,
        rect.x + rect.width + logical_margin, rect.y + rect.height + logical_margin]
        .into_iter().any(|v| !v.is_finite() || v.abs() > 1_048_576.0) {
        return Err(Refusal::InvalidTarget);
    }
    let (w, h) = target.physical_size;
    let margin = 2.0 + f64::from(appearance.halo_extent()) * f64::from(d);
    let mut left = (f64::from(rect.x) * f64::from(d) - margin).floor().max(0.0);
    let mut top = (f64::from(rect.y) * f64::from(d) - margin).floor().max(0.0);
    let mut right = (f64::from(rect.x + rect.width) * f64::from(d) + margin)
        .ceil()
        .min(f64::from(w));
    let mut bottom = (f64::from(rect.y + rect.height) * f64::from(d) + margin)
        .ceil()
        .min(f64::from(h));
    if !p.clip_state.is_none() {
        // Same outward scissor convention as ordinary pipeline dispatch.
        let c = p.clip_state.rect;
        left = left.max((f64::from(c.x) * f64::from(d)).floor().max(0.0));
        top = top.max((f64::from(c.y) * f64::from(d)).floor().max(0.0));
        right = right.min((f64::from(c.x + c.width) * f64::from(d)).ceil());
        bottom = bottom.min((f64::from(c.y + c.height) * f64::from(d)).ceil());
    }
    if right <= left || bottom <= top {
        return Ok(None);
    }
    let visible = [
        left as u32,
        top as u32,
        (right - left) as u32,
        (bottom - top) as u32,
    ];
    let sigma = f64::from(appearance.sigma_logical()) * f64::from(d);
    let radius = (3.0 * sigma).ceil() as u32;
    if radius > 192 {
        return Err(Refusal::KernelLimit {
            required: radius,
            limit: 192,
        });
    }
    // Saturating endpoints express intersection with the actual u32 target,
    // never an internal-ROI clamp or a shortened kernel.
    let expand = |r: [u32; 4], pad: u32| {
        let x = r[0].saturating_sub(pad);
        let y = r[1].saturating_sub(pad);
        let end_x = r[0].saturating_add(r[2]).saturating_add(pad).min(w);
        let end_y = r[1].saturating_add(r[3]).saturating_add(pad).min(h);
        [x, y, end_x - x, end_y - y]
    };
    let filtered_domain = if appearance.refraction_logical() == 0.0 { visible } else {
        let reach = (f64::from(appearance.refraction_logical()).abs() * f64::from(d)).ceil();
        if !reach.is_finite() || reach > 256.0 { return Err(Refusal::InvalidTarget); }
        let padding = (reach as u32).checked_add(1).ok_or(Refusal::InvalidTarget)?;
        expand(visible, padding)
    };
    let snapshot = expand(filtered_domain, radius);
    let mut value = FrostedGlassParams::terminal((w, h));
    value.rect = [rect.x, rect.y, rect.width, rect.height];
    value.corners = [corners.tl, corners.tr, corners.bl, corners.br];
    let c = appearance.tint();
    value.tint = [c.r, c.g, c.b, c.a];
    value.optics = [appearance.transmission(), sigma as f32, d, 0.0];
    value.visible = visible;
    value.snapshot = snapshot;
    value.filtered_domain = filtered_domain;
    value.surface_controls = [
        appearance.coverage_opacity(),
        appearance.refraction_logical(),
        appearance.edge_feather_logical(),
        0.0,
    ];
    if let Some(optics) = appearance.glass_optics() {
        value.extensions[0] = 1;
        value.glass_optics = GlassSurfaceOpticsParams::new(optics);
        value.film = GlassSurfaceOpticsParams::film_tail(optics);
    }
    if let Some(emission) = appearance.surface_emission() {
        value.extensions[1] = 1;
        value.emission = SurfaceEmissionParams::new(emission);
    }
    value.control[0] = radius;
    value.weights[0][0] = 1.0;
    for k in 1..=radius {
        value.weights[k as usize / 4][k as usize % 4] =
            (-0.5 * (f64::from(k) / sigma).powi(2)).exp() as f32;
    }
    if let Some((rect, radii)) = ancestor {
        value.control[1] = 1;
        value.ancestor = RoundedClipUniform::new(
            [rect.x, rect.y, rect.width, rect.height],
            *radii,
            aa.max(0.5 / d),
        );
    }
    if let Some(grain) = appearance.micrograin() {
        value.grain = [grain.scale_px(), grain.amplitude_srgb(), 0.0, 0.0];
        value.grain_seed = [grain.seed(), 0, 0, 0];
    }
    Ok(Some(Occurrence {
        ordinal: *ordinal,
        node_key: *node_key,
        params: value,
    }))
}
