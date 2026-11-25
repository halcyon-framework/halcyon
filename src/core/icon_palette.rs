//! SPEC-HAL-265 M7: Icon palette binary serialization.
//!
//! Serializes/deserializes `IconDefinition` values to/from a compact binary format.
//! No file I/O — operates on `&[u8]` / `Vec<u8>`.

use crate::core::icon::{
    FillRule, IconDefinition, IconId, IconLayer, IconRegistry, IconRenderMode, IconRole, LayerRole,
};
use crate::renderer::path::{Path, PathCommand};

const MAGIC: [u8; 4] = *b"HALP";
const VERSION: u16 = 1;

/// Errors from palette deserialization.
#[derive(Debug)]
pub enum PaletteError {
    InvalidMagic,
    UnsupportedVersion(u16),
    InvalidData(String),
    Io(std::io::Error),
}

/// Serialize a slice of IconDefinitions to bytes.
pub fn serialize_palette(icons: &[&IconDefinition]) -> Vec<u8> {
    let mut buf = Vec::new();
    // Header
    buf.extend_from_slice(&MAGIC);
    buf.extend_from_slice(&VERSION.to_le_bytes());
    buf.extend_from_slice(&(icons.len() as u16).to_le_bytes());

    for icon in icons {
        buf.extend_from_slice(&icon.id.0.to_le_bytes());
        let name_bytes = icon.name.as_bytes();
        buf.extend_from_slice(&(name_bytes.len() as u16).to_le_bytes());
        buf.extend_from_slice(name_bytes);
        buf.extend_from_slice(&icon.viewbox_width.to_le_bytes());
        buf.extend_from_slice(&icon.viewbox_height.to_le_bytes());
        let mode_byte: u8 = match icon.default_mode {
            IconRenderMode::Fill => 0,
            IconRenderMode::Stroke => 1,
        };
        buf.push(mode_byte);
        let role_byte: u8 = match icon.role {
            IconRole::Action => 0,
            IconRole::Navigation => 1,
            IconRole::Status => 2,
            IconRole::Structural => 3,
            IconRole::Entity => 4,
            IconRole::Indicator => 5,
        };
        buf.push(role_byte);
        buf.extend_from_slice(&(icon.layers.len() as u16).to_le_bytes());
        for layer in &icon.layers {
            buf.push(layer.role as u8);
            let fill_byte: u8 = match layer.fill_rule {
                FillRule::NonZero => 0,
                FillRule::EvenOdd => 1,
            };
            buf.push(fill_byte);
            buf.extend_from_slice(&(layer.path.commands.len() as u32).to_le_bytes());
            for cmd in &layer.path.commands {
                match cmd {
                    PathCommand::MoveTo(x, y) => {
                        buf.push(0);
                        buf.extend_from_slice(&x.to_le_bytes());
                        buf.extend_from_slice(&y.to_le_bytes());
                    }
                    PathCommand::LineTo(x, y) => {
                        buf.push(1);
                        buf.extend_from_slice(&x.to_le_bytes());
                        buf.extend_from_slice(&y.to_le_bytes());
                    }
                    PathCommand::CubicTo(c1x, c1y, c2x, c2y, x, y) => {
                        buf.push(2);
                        buf.extend_from_slice(&c1x.to_le_bytes());
                        buf.extend_from_slice(&c1y.to_le_bytes());
                        buf.extend_from_slice(&c2x.to_le_bytes());
                        buf.extend_from_slice(&c2y.to_le_bytes());
                        buf.extend_from_slice(&x.to_le_bytes());
                        buf.extend_from_slice(&y.to_le_bytes());
                    }
                    PathCommand::Close => {
                        buf.push(3);
                    }
                    PathCommand::QuadTo(cx, cy, x, y) => {
                        buf.push(4);
                        buf.extend_from_slice(&cx.to_le_bytes());
                        buf.extend_from_slice(&cy.to_le_bytes());
                        buf.extend_from_slice(&x.to_le_bytes());
                        buf.extend_from_slice(&y.to_le_bytes());
                    }
                }
            }
        }
    }
    buf
}

/// Deserialize bytes into IconDefinitions.
/// Names are heap-allocated and leaked to &'static str for registry compatibility.
pub fn deserialize_palette(data: &[u8]) -> Result<Vec<IconDefinition>, PaletteError> {
    let mut r = Reader::new(data);

    let magic = r.read_bytes(4)?;
    if magic != MAGIC {
        return Err(PaletteError::InvalidMagic);
    }
    let version = r.read_u16()?;
    if version != VERSION {
        return Err(PaletteError::UnsupportedVersion(version));
    }
    let icon_count = r.read_u16()? as usize;

    let mut icons = Vec::with_capacity(icon_count);
    for _ in 0..icon_count {
        let id = IconId(r.read_u32()?);
        let name_len = r.read_u16()? as usize;
        let name_bytes = r.read_bytes(name_len)?;
        let name_string = std::str::from_utf8(name_bytes)
            .map_err(|e| PaletteError::InvalidData(format!("invalid UTF-8 name: {e}")))?;
        let name: &'static str = Box::leak(name_string.to_string().into_boxed_str());

        let viewbox_width = r.read_f32()?;
        let viewbox_height = r.read_f32()?;

        let mode_byte = r.read_u8()?;
        let default_mode = match mode_byte {
            0 => IconRenderMode::Fill,
            1 => IconRenderMode::Stroke,
            _ => {
                return Err(PaletteError::InvalidData(format!(
                    "invalid render mode: {mode_byte}"
                )));
            }
        };

        let role_byte = r.read_u8()?;
        let role = match role_byte {
            0 => IconRole::Action,
            1 => IconRole::Navigation,
            2 => IconRole::Status,
            3 => IconRole::Structural,
            4 => IconRole::Entity,
            5 => IconRole::Indicator,
            _ => {
                return Err(PaletteError::InvalidData(format!(
                    "invalid icon role: {role_byte}"
                )));
            }
        };

        let layer_count = r.read_u16()? as usize;
        let mut layers = Vec::with_capacity(layer_count);
        let mut hash: u64 = 0;

        for _ in 0..layer_count {
            let lr_byte = r.read_u8()?;
            let layer_role = match lr_byte {
                0 => LayerRole::Base,
                1 => LayerRole::Shadow,
                2 => LayerRole::Highlight,
                3 => LayerRole::Outline,
                _ => {
                    return Err(PaletteError::InvalidData(format!(
                        "invalid layer role: {lr_byte}"
                    )));
                }
            };

            let fr_byte = r.read_u8()?;
            let fill_rule = match fr_byte {
                0 => FillRule::NonZero,
                1 => FillRule::EvenOdd,
                _ => {
                    return Err(PaletteError::InvalidData(format!(
                        "invalid fill rule: {fr_byte}"
                    )));
                }
            };

            let command_count = r.read_u32()? as usize;
            let mut path = Path::new();
            for _ in 0..command_count {
                let tag = r.read_u8()?;
                match tag {
                    0 => {
                        let x = r.read_f32()?;
                        let y = r.read_f32()?;
                        path.move_to(x, y);
                    }
                    1 => {
                        let x = r.read_f32()?;
                        let y = r.read_f32()?;
                        path.line_to(x, y);
                    }
                    2 => {
                        let c1x = r.read_f32()?;
                        let c1y = r.read_f32()?;
                        let c2x = r.read_f32()?;
                        let c2y = r.read_f32()?;
                        let x = r.read_f32()?;
                        let y = r.read_f32()?;
                        path.cubic_to(c1x, c1y, c2x, c2y, x, y);
                    }
                    3 => {
                        path.close();
                    }
                    4 => {
                        let cx = r.read_f32()?;
                        let cy = r.read_f32()?;
                        let x = r.read_f32()?;
                        let y = r.read_f32()?;
                        path.quad_to(cx, cy, x, y);
                    }
                    _ => {
                        return Err(PaletteError::InvalidData(format!(
                            "invalid command tag: {tag}"
                        )));
                    }
                }
            }

            let layer_hash = path.content_hash();
            hash = hash
                .wrapping_mul(31)
                .wrapping_add(layer_hash ^ (layer_role as u64));

            layers.push(IconLayer {
                role: layer_role,
                path,
                fill_rule,
                skip_precision: false,
            });
        }

        icons.push(IconDefinition {
            id,
            name,
            layers,
            viewbox_width,
            viewbox_height,
            default_mode,
            role,
            path_hash: hash,
        });
    }

    Ok(icons)
}

/// Convenience: deserialize and build a registry directly.
pub fn load_registry(data: &[u8]) -> Result<IconRegistry, PaletteError> {
    let defs = deserialize_palette(data)?;
    let defs: &'static [IconDefinition] = Box::leak(defs.into_boxed_slice());
    Ok(IconRegistry::new(defs))
}

struct Reader<'a> {
    data: &'a [u8],
    pos: usize,
}

impl<'a> Reader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, pos: 0 }
    }

    fn read_bytes(&mut self, n: usize) -> Result<&'a [u8], PaletteError> {
        if self.pos + n > self.data.len() {
            return Err(PaletteError::InvalidData("unexpected end of data".into()));
        }
        let slice = &self.data[self.pos..self.pos + n];
        self.pos += n;
        Ok(slice)
    }

    fn read_u8(&mut self) -> Result<u8, PaletteError> {
        Ok(self.read_bytes(1)?[0])
    }

    fn read_u16(&mut self) -> Result<u16, PaletteError> {
        let b = self.read_bytes(2)?;
        Ok(u16::from_le_bytes([b[0], b[1]]))
    }

    fn read_u32(&mut self) -> Result<u32, PaletteError> {
        let b = self.read_bytes(4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    fn read_f32(&mut self) -> Result<f32, PaletteError> {
        let b = self.read_bytes(4)?;
        Ok(f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }
}
