//! SPEC-HAL-265 M7: SVG import bridge.
//!
//! Constructs `IconDefinition` values from SVG path `d` attribute strings.

use crate::core::icon::{
    FillRule, IconDefinition, IconId, IconLayer, IconRenderMode, IconRole, LayerRole,
};
use crate::core::svg_path::{SvgPathError, parse_svg_path};

/// Import a single-layer icon from an SVG path `d` string.
pub fn icon_from_svg(
    id: IconId,
    name: &'static str,
    d: &str,
    viewbox: f32,
    role: IconRole,
    mode: IconRenderMode,
) -> Result<IconDefinition, SvgPathError> {
    let path = parse_svg_path(d)?;
    let layer_hash = path.content_hash();
    let mut hash: u64 = 0;
    hash = hash
        .wrapping_mul(31)
        .wrapping_add(layer_hash ^ (LayerRole::Base as u64));
    let layers = vec![IconLayer {
        role: LayerRole::Base,
        path,
        fill_rule: FillRule::NonZero,
        skip_precision: false,
    }];
    Ok(IconDefinition {
        id,
        name,
        layers,
        viewbox_width: viewbox,
        viewbox_height: viewbox,
        default_mode: mode,
        role,
        path_hash: hash,
    })
}

/// Import a multi-layer icon from SVG path strings with assigned roles.
pub fn icon_from_svg_layers(
    id: IconId,
    name: &'static str,
    layers: &[(LayerRole, &str)],
    viewbox: f32,
    role: IconRole,
    mode: IconRenderMode,
) -> Result<IconDefinition, SvgPathError> {
    let mut hash: u64 = 0;
    let mut icon_layers = Vec::with_capacity(layers.len());
    for &(layer_role, d) in layers {
        let path = parse_svg_path(d)?;
        let layer_hash = path.content_hash();
        hash = hash
            .wrapping_mul(31)
            .wrapping_add(layer_hash ^ (layer_role as u64));
        icon_layers.push(IconLayer {
            role: layer_role,
            path,
            fill_rule: FillRule::NonZero,
            skip_precision: false,
        });
    }
    Ok(IconDefinition {
        id,
        name,
        layers: icon_layers,
        viewbox_width: viewbox,
        viewbox_height: viewbox,
        default_mode: mode,
        role,
        path_hash: hash,
    })
}
