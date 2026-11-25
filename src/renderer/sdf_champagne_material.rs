//! Private ChampagneV4 source for the current Button material producer.

use super::sdf_material_status::{CHAMPAGNE_V4_RADIUS, ChampagneV4Basis};
use super::sdf_static_material::{material_shader_layout, pipeline};
use super::wgpu_renderer::GpuResources;
use crate::core::paint::Color;
use bytemuck::{Pod, Zeroable};

const SOURCE: &str = include_str!("shaders/button_champagne_v4_sdf.wgsl");

#[repr(C, align(16))]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(super) struct ChampagneMaterial {
    half_size: [f32; 2],
    center: [f32; 2],
    scale: f32,
    radius: f32,
    sheen: f32,
    rim: f32,
    dim: f32,
    valley: f32,
    _padding: [f32; 2],
    stops: [[f32; 4]; 4],
    highlight: [f32; 4],
}

impl ChampagneMaterial {
    pub(super) fn supports_scale(scale: f32) -> bool {
        (0.5..=4.0).contains(&scale)
    }

    pub(super) fn new(
        basis: ChampagneV4Basis,
        scale: f32,
        half_size: [f32; 2],
        center: [f32; 2],
        hovered: bool,
        pressed: bool,
    ) -> Result<Self, &'static str> {
        if half_size
            .iter()
            .any(|&side| !side.is_finite() || side < CHAMPAGNE_V4_RADIUS)
            || center.iter().any(|&axis| !axis.is_finite())
            || !Self::supports_scale(scale)
        {
            return Err(
                "ChampagneV4 needs finite face geometry at radius 8 and scale 0.5 through 4",
            );
        }
        let (sheen, rim, dim, valley) = match (basis, hovered, pressed) {
            (ChampagneV4Basis::Enabled, false, false) => (0.10, 0.40, 0.0, 0.0),
            (ChampagneV4Basis::Enabled, true, false) => (0.31, 0.76, -0.08, 0.0),
            (ChampagneV4Basis::Enabled, false, true) => (0.035, 0.19, 0.10, 0.20),
            (ChampagneV4Basis::Enabled, true, true) => (0.065, 0.24, 0.10, 0.20),
            (ChampagneV4Basis::Disabled, false, false) => (0.025, 0.16, 0.075, 0.0),
            (ChampagneV4Basis::Disabled, _, _) => {
                return Err("disabled ChampagneV4 has only a static cell");
            }
        };
        let stops = [
            (0xF1D1A0, 0.0),
            (0xEBC597, 0.22),
            (0xD3A776, 0.66),
            (0xC69662, 1.0),
        ]
        .map(|(hex, offset)| {
            let color = Color::from_hex(hex);
            [color.r, color.g, color.b, offset]
        });
        let highlight = Color::from_hex(0xFFEDCE);
        Ok(Self {
            half_size,
            center,
            scale,
            radius: CHAMPAGNE_V4_RADIUS,
            sheen,
            rim,
            dim,
            valley,
            _padding: [0.0; 2],
            stops,
            highlight: [highlight.r, highlight.g, highlight.b, 1.0],
        })
    }

    pub(super) fn as_bytes(&self) -> &[u8] {
        bytemuck::bytes_of(self)
    }
}

pub(super) struct ChampagnePipeline {
    pub(super) face: wgpu::RenderPipeline,
    pub(super) bindings: wgpu::BindGroupLayout,
}

impl ChampagnePipeline {
    pub(super) fn new(gpu: &GpuResources) -> Self {
        let (shader, bindings, layout) =
            material_shader_layout(gpu, SOURCE, "ChampagneV4 SDF face");
        Self {
            face: pipeline(gpu, &shader, &layout, "fs_face"),
            bindings,
        }
    }
}
