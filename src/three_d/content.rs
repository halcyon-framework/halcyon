use std::sync::Arc;

use thiserror::Error;

use super::context::{SamplerHandle, TextureHandle};
use super::{LocalPoint3, LocalVector3};

pub const CANONICAL_POSITION_OFFSET: u64 = 0;
pub const CANONICAL_NORMAL_OFFSET: u64 = 12;
pub const CANONICAL_UV_OFFSET: u64 = 24;
pub const CANONICAL_COLOR_OFFSET: u64 = 32;
pub const CANONICAL_VERTEX_STRIDE: u64 = 36;

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Geometry3DError {
    #[error("geometry must contain at least one complete triangle")]
    EmptyGeometry,
    #[error("{semantic} stream length {actual} does not match position count {positions}")]
    SemanticLengthMismatch {
        semantic: &'static str,
        positions: usize,
        actual: usize,
    },
    #[error("normal at vertex {vertex} must be finite and non-zero")]
    InvalidNormal { vertex: usize },
    #[error("texture coordinate components must be finite")]
    NonFiniteTextureCoordinate,
    #[error("draw count {count} is not a non-zero triangle-list count")]
    InvalidTriangleListCount { count: usize },
    #[error("index {index} at element {element} is outside vertex count {vertices}")]
    IndexOutOfRange {
        element: usize,
        index: u32,
        vertices: usize,
    },
    #[error("geometry count or canonical byte size is not representable")]
    SizeOverflow,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Appearance3DError {
    #[error("appearance WGSL source must not be empty")]
    EmptySource,
    #[error("appearance source byte length is not representable")]
    SizeOverflow,
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
pub enum Texture3DError {
    #[error("texture extent must be non-zero, got {width}x{height}")]
    EmptyExtent { width: u32, height: u32 },
    #[error("texture extent byte size is not representable")]
    SizeOverflow,
    #[error("texture texel byte length mismatch: expected {expected}, got {actual}")]
    TexelByteLengthMismatch { expected: u64, actual: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TextureFormat3D {
    Rgba8Unorm,
    Rgba8UnormSrgb,
}

impl TextureFormat3D {
    pub const fn bytes_per_texel(self) -> u64 {
        match self {
            Self::Rgba8Unorm | Self::Rgba8UnormSrgb => 4,
        }
    }

    pub(crate) const fn as_wgpu(self) -> wgpu::TextureFormat {
        match self {
            Self::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
            Self::Rgba8UnormSrgb => wgpu::TextureFormat::Rgba8UnormSrgb,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Texture3D {
    width: u32,
    height: u32,
    format: TextureFormat3D,
    texels: Arc<[u8]>,
    texel_byte_len: u64,
}

impl Texture3D {
    pub fn try_new(
        width: u32,
        height: u32,
        format: TextureFormat3D,
        texels: Vec<u8>,
    ) -> Result<Self, Texture3DError> {
        if width == 0 || height == 0 {
            return Err(Texture3DError::EmptyExtent { width, height });
        }
        let expected = u64::from(width)
            .checked_mul(u64::from(height))
            .and_then(|texels| texels.checked_mul(format.bytes_per_texel()))
            .ok_or(Texture3DError::SizeOverflow)?;
        let actual = u64::try_from(texels.len()).map_err(|_| Texture3DError::SizeOverflow)?;
        if actual != expected {
            return Err(Texture3DError::TexelByteLengthMismatch { expected, actual });
        }
        Ok(Self {
            width,
            height,
            format,
            texels: texels.into(),
            texel_byte_len: actual,
        })
    }

    pub const fn width(&self) -> u32 {
        self.width
    }

    pub const fn height(&self) -> u32 {
        self.height
    }

    pub const fn format(&self) -> TextureFormat3D {
        self.format
    }

    pub fn texels(&self) -> &[u8] {
        &self.texels
    }

    pub const fn texel_byte_len(&self) -> u64 {
        self.texel_byte_len
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerAddressMode {
    ClampToEdge,
    Repeat,
    MirrorRepeat,
}

impl SamplerAddressMode {
    pub(crate) const fn as_wgpu(self) -> wgpu::AddressMode {
        match self {
            Self::ClampToEdge => wgpu::AddressMode::ClampToEdge,
            Self::Repeat => wgpu::AddressMode::Repeat,
            Self::MirrorRepeat => wgpu::AddressMode::MirrorRepeat,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SamplerFilterMode {
    Nearest,
    Linear,
}

impl SamplerFilterMode {
    pub(crate) const fn as_wgpu(self) -> wgpu::FilterMode {
        match self {
            Self::Nearest => wgpu::FilterMode::Nearest,
            Self::Linear => wgpu::FilterMode::Linear,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Sampler3D {
    address_mode_u: SamplerAddressMode,
    address_mode_v: SamplerAddressMode,
    mag_filter: SamplerFilterMode,
    min_filter: SamplerFilterMode,
}

impl Sampler3D {
    pub const LINEAR_CLAMP: Self = Self::new(
        SamplerAddressMode::ClampToEdge,
        SamplerAddressMode::ClampToEdge,
        SamplerFilterMode::Linear,
        SamplerFilterMode::Linear,
    );

    pub const fn new(
        address_mode_u: SamplerAddressMode,
        address_mode_v: SamplerAddressMode,
        mag_filter: SamplerFilterMode,
        min_filter: SamplerFilterMode,
    ) -> Self {
        Self {
            address_mode_u,
            address_mode_v,
            mag_filter,
            min_filter,
        }
    }

    pub const fn address_mode_u(self) -> SamplerAddressMode {
        self.address_mode_u
    }

    pub const fn address_mode_v(self) -> SamplerAddressMode {
        self.address_mode_v
    }

    pub const fn mag_filter(self) -> SamplerFilterMode {
        self.mag_filter
    }

    pub const fn min_filter(self) -> SamplerFilterMode {
        self.min_filter
    }
}

impl Default for Sampler3D {
    fn default() -> Self {
        Self::LINEAR_CLAMP
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceTextureBinding {
    texture: TextureHandle,
    sampler: SamplerHandle,
}

impl AppearanceTextureBinding {
    pub fn new(texture: TextureHandle, sampler: SamplerHandle) -> Self {
        Self { texture, sampler }
    }

    pub const fn texture(&self) -> &TextureHandle {
        &self.texture
    }

    pub const fn sampler(&self) -> &SamplerHandle {
        &self.sampler
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum AppearanceMode {
    #[default]
    Opaque,
    AlphaMask,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TextureCoordinate2 {
    u: f32,
    v: f32,
}

impl TextureCoordinate2 {
    pub fn try_new(u: f32, v: f32) -> Result<Self, Geometry3DError> {
        if !(u.is_finite() && v.is_finite()) {
            return Err(Geometry3DError::NonFiniteTextureCoordinate);
        }
        Ok(Self { u, v })
    }

    pub const fn u(self) -> f32 {
        self.u
    }

    pub const fn v(self) -> f32 {
        self.v
    }
}

/// Straight-linear RGBA modulation encoded as linear-quantized UNORM8 bytes.
///
/// The RGB bytes are not sRGB-encoded and alpha is not premultiplied. Construction
/// performs no transfer-function conversion or premultiplication.
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LinearVertexColorRgba8([u8; 4]);

impl LinearVertexColorRgba8 {
    pub const OPAQUE_WHITE: Self = Self::from_linear_unorm8(255, 255, 255, 255);

    /// Retains already linear-quantized UNORM8 channel bytes exactly as supplied.
    pub const fn from_linear_unorm8(red: u8, green: u8, blue: u8, alpha: u8) -> Self {
        Self([red, green, blue, alpha])
    }

    pub const fn to_array(self) -> [u8; 4] {
        self.0
    }
}

pub const DEFAULT_LINEAR_VERTEX_COLOR: LinearVertexColorRgba8 =
    LinearVertexColorRgba8::OPAQUE_WHITE;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct GeometrySemantics {
    normal: bool,
    uv: bool,
    color: bool,
}

impl GeometrySemantics {
    pub const NONE: Self = Self {
        normal: false,
        uv: false,
        color: false,
    };

    pub const fn new(normal: bool, uv: bool, color: bool) -> Self {
        Self { normal, uv, color }
    }

    pub const fn has_normal(self) -> bool {
        self.normal
    }

    pub const fn has_uv(self) -> bool {
        self.uv
    }

    pub const fn has_color(self) -> bool {
        self.color
    }

    pub const fn contains(self, required: Self) -> bool {
        (!required.normal || self.normal)
            && (!required.uv || self.uv)
            && (!required.color || self.color)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GeometryIndices {
    Unindexed,
    U16(Arc<[u16]>),
    U32(Arc<[u32]>),
}

impl GeometryIndices {
    pub fn from_u16(indices: Vec<u16>) -> Self {
        Self::U16(indices.into())
    }

    pub fn from_u32(indices: Vec<u32>) -> Self {
        Self::U32(indices.into())
    }

    pub fn len(&self) -> usize {
        match self {
            Self::Unindexed => 0,
            Self::U16(indices) => indices.len(),
            Self::U32(indices) => indices.len(),
        }
    }

    pub const fn is_indexed(&self) -> bool {
        !matches!(self, Self::Unindexed)
    }

    pub const fn element_size(&self) -> u64 {
        match self {
            Self::Unindexed => 0,
            Self::U16(_) => 2,
            Self::U32(_) => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct GeometrySourceDescriptor {
    positions: Arc<[LocalPoint3]>,
    normals: Option<Arc<[LocalVector3]>>,
    uvs: Option<Arc<[TextureCoordinate2]>>,
    colors: Option<Arc<[LinearVertexColorRgba8]>>,
    indices: GeometryIndices,
    semantics: GeometrySemantics,
    vertex_count: u64,
    index_count: u64,
    canonical_vertex_bytes: u64,
    canonical_index_bytes: u64,
    canonical_total_bytes: u64,
}

impl GeometrySourceDescriptor {
    pub fn try_new(
        positions: Vec<LocalPoint3>,
        normals: Option<Vec<LocalVector3>>,
        uvs: Option<Vec<TextureCoordinate2>>,
        colors: Option<Vec<LinearVertexColorRgba8>>,
        indices: GeometryIndices,
    ) -> Result<Self, Geometry3DError> {
        if positions.is_empty() {
            return Err(Geometry3DError::EmptyGeometry);
        }
        validate_stream_length("normal", positions.len(), normals.as_deref())?;
        validate_stream_length("uv", positions.len(), uvs.as_deref())?;
        validate_stream_length("color", positions.len(), colors.as_deref())?;

        let normals = normals
            .map(|normals| {
                normals
                    .into_iter()
                    .enumerate()
                    .map(|(vertex, normal)| normalize_normal(vertex, normal))
                    .collect::<Result<Vec<_>, _>>()
            })
            .transpose()?;

        let draw_count = if indices.is_indexed() {
            indices.len()
        } else {
            positions.len()
        };
        if draw_count == 0 || draw_count % 3 != 0 {
            return Err(Geometry3DError::InvalidTriangleListCount { count: draw_count });
        }
        validate_indices(&indices, positions.len())?;

        let vertex_count =
            u64::try_from(positions.len()).map_err(|_| Geometry3DError::SizeOverflow)?;
        let index_count =
            u64::try_from(indices.len()).map_err(|_| Geometry3DError::SizeOverflow)?;
        let canonical_vertex_bytes = vertex_count
            .checked_mul(CANONICAL_VERTEX_STRIDE)
            .ok_or(Geometry3DError::SizeOverflow)?;
        let canonical_index_bytes = index_count
            .checked_mul(indices.element_size())
            .ok_or(Geometry3DError::SizeOverflow)?;
        let canonical_total_bytes = canonical_vertex_bytes
            .checked_add(canonical_index_bytes)
            .ok_or(Geometry3DError::SizeOverflow)?;

        let semantics = GeometrySemantics::new(normals.is_some(), uvs.is_some(), colors.is_some());
        Ok(Self {
            positions: positions.into(),
            normals: normals.map(Into::into),
            uvs: uvs.map(Into::into),
            colors: colors.map(Into::into),
            indices,
            semantics,
            vertex_count,
            index_count,
            canonical_vertex_bytes,
            canonical_index_bytes,
            canonical_total_bytes,
        })
    }

    pub fn positions(&self) -> &[LocalPoint3] {
        &self.positions
    }

    pub fn normals(&self) -> Option<&[LocalVector3]> {
        self.normals.as_deref()
    }

    pub fn uvs(&self) -> Option<&[TextureCoordinate2]> {
        self.uvs.as_deref()
    }

    pub fn colors(&self) -> Option<&[LinearVertexColorRgba8]> {
        self.colors.as_deref()
    }

    pub const fn indices(&self) -> &GeometryIndices {
        &self.indices
    }

    pub const fn semantics(&self) -> GeometrySemantics {
        self.semantics
    }

    pub fn vertex_count(&self) -> u64 {
        self.vertex_count
    }

    pub fn index_count(&self) -> u64 {
        self.index_count
    }

    pub const fn canonical_vertex_bytes(&self) -> u64 {
        self.canonical_vertex_bytes
    }

    pub const fn canonical_index_bytes(&self) -> u64 {
        self.canonical_index_bytes
    }

    pub const fn canonical_total_bytes(&self) -> u64 {
        self.canonical_total_bytes
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppearanceSourceDescriptor {
    source: Arc<str>,
    source_byte_len: u64,
    required_semantics: GeometrySemantics,
    mode: AppearanceMode,
    texture_binding: Option<AppearanceTextureBinding>,
}

impl AppearanceSourceDescriptor {
    pub fn try_new(
        source: impl Into<String>,
        required_semantics: GeometrySemantics,
    ) -> Result<Self, Appearance3DError> {
        Self::try_new_with_mode(source, required_semantics, AppearanceMode::Opaque)
    }

    pub fn try_new_with_mode(
        source: impl Into<String>,
        required_semantics: GeometrySemantics,
        mode: AppearanceMode,
    ) -> Result<Self, Appearance3DError> {
        let source = source.into();
        if source.trim().is_empty() {
            return Err(Appearance3DError::EmptySource);
        }
        let source_byte_len =
            u64::try_from(source.len()).map_err(|_| Appearance3DError::SizeOverflow)?;
        Ok(Self {
            source: source.into(),
            source_byte_len,
            required_semantics,
            mode,
            texture_binding: None,
        })
    }

    /// Returns retained source only. Registration does not perform Naga admission
    /// and does not make this descriptor executable.
    pub fn source(&self) -> &str {
        &self.source
    }

    pub fn source_byte_len(&self) -> u64 {
        self.source_byte_len
    }

    pub const fn required_semantics(&self) -> GeometrySemantics {
        self.required_semantics
    }

    pub const fn mode(&self) -> AppearanceMode {
        self.mode
    }

    pub fn with_texture(mut self, texture: TextureHandle, sampler: SamplerHandle) -> Self {
        self.texture_binding = Some(AppearanceTextureBinding::new(texture, sampler));
        self
    }

    pub const fn texture_binding(&self) -> Option<&AppearanceTextureBinding> {
        self.texture_binding.as_ref()
    }
}

fn validate_stream_length<T>(
    semantic: &'static str,
    positions: usize,
    stream: Option<&[T]>,
) -> Result<(), Geometry3DError> {
    if let Some(stream) = stream
        && stream.len() != positions
    {
        return Err(Geometry3DError::SemanticLengthMismatch {
            semantic,
            positions,
            actual: stream.len(),
        });
    }
    Ok(())
}

fn normalize_normal(vertex: usize, normal: LocalVector3) -> Result<LocalVector3, Geometry3DError> {
    let length = normal
        .try_length()
        .map_err(|_| Geometry3DError::InvalidNormal { vertex })?;
    if !length.is_finite() || length <= 0.0 {
        return Err(Geometry3DError::InvalidNormal { vertex });
    }
    LocalVector3::try_new(
        normal.x() / length,
        normal.y() / length,
        normal.z() / length,
    )
    .map_err(|_| Geometry3DError::InvalidNormal { vertex })
}

fn validate_indices(indices: &GeometryIndices, vertex_count: usize) -> Result<(), Geometry3DError> {
    match indices {
        GeometryIndices::Unindexed => Ok(()),
        GeometryIndices::U16(indices) => {
            indices.iter().enumerate().try_for_each(|(element, index)| {
                validate_index(element, u32::from(*index), vertex_count)
            })
        }
        GeometryIndices::U32(indices) => indices
            .iter()
            .enumerate()
            .try_for_each(|(element, index)| validate_index(element, *index, vertex_count)),
    }
}

fn validate_index(element: usize, index: u32, vertex_count: usize) -> Result<(), Geometry3DError> {
    let in_range = usize::try_from(index)
        .ok()
        .is_some_and(|index| index < vertex_count);
    if !in_range {
        return Err(Geometry3DError::IndexOutOfRange {
            element,
            index,
            vertices: vertex_count,
        });
    }
    Ok(())
}
