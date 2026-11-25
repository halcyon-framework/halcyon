//! SPEC-HAL-084: Logical Resource Descriptors
//!
//! RenderGraph owns logical resources (descriptors) only.
//! Physical GPU resources are acquired from SPEC-HAL-040 pools during Compile phase.

use wgpu::TextureFormat;

/// Unique handle to an attachment declared in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct AttachmentHandle(pub(crate) u32);

/// Unique handle to a buffer declared in the graph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BufferHandle(pub(crate) u32);

/// Reference to a resource (attachment or buffer) for dependency tracking.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ResourceRef {
    Attachment(AttachmentHandle),
    Buffer(BufferHandle),
}

/// Logical attachment descriptor.
///
/// Describes size, format, and usage. Physical texture acquired during Compile.
#[derive(Debug, Clone)]
pub struct AttachmentDesc {
    /// Debug label for the attachment.
    pub label: &'static str,
    /// Size specification.
    pub size: AttachmentSize,
    /// Texture format.
    pub format: TextureFormat,
    /// Sample count (1 = no MSAA).
    pub samples: u32,
    /// Clear color if load op is Clear.
    pub clear_color: Option<wgpu::Color>,
    /// Load operation.
    pub load_op: wgpu::LoadOp<wgpu::Color>,
    /// Store operation.
    pub store_op: wgpu::StoreOp,
}

/// Attachment size specification.
#[derive(Debug, Clone, Copy)]
pub enum AttachmentSize {
    /// Fixed size in pixels.
    Absolute { width: u32, height: u32 },
    /// Relative to swapchain size.
    SwapchainRelative { factor_x: f32, factor_y: f32 },
}

impl AttachmentSize {
    /// Full swapchain size (1:1).
    pub const SWAPCHAIN_FULL: Self = Self::SwapchainRelative {
        factor_x: 1.0,
        factor_y: 1.0,
    };
}

/// Logical buffer descriptor.
///
/// Describes size and usage. Physical buffer acquired during Compile.
#[derive(Debug, Clone)]
pub struct BufferDesc {
    /// Debug label for the buffer.
    pub label: &'static str,
    /// Buffer size in bytes.
    pub size: u64,
    /// Buffer usage flags.
    pub usage: wgpu::BufferUsages,
}

/// Union type for resource descriptors.
#[derive(Debug, Clone)]
pub enum ResourceDescriptor {
    Attachment(AttachmentDesc),
    Buffer(BufferDesc),
}
