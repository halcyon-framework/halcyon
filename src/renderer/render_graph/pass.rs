//! SPEC-HAL-084: Render Pass Nodes and Executors
//!
//! Every GPU pass corresponds to a RenderPassNode in the graph.
//! Pass executors receive constrained context (no Device/Queue/pools).

use super::executor::PassContext;
use super::resources::{AttachmentHandle, BufferHandle, ResourceRef};

/// Unique identifier for a render pass node.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderPassNodeId(pub(crate) u32);

/// A node in the render graph representing a GPU pass.
///
/// Dependencies are derived from resource reads/writes.
/// `explicit_deps` is for ordering constraints not expressible as resource edges.
pub struct RenderPassNode {
    /// Unique identifier assigned by the builder.
    pub id: RenderPassNodeId,
    /// Debug name for the pass.
    pub name: &'static str,
    /// Resources this pass reads from.
    pub reads: Vec<ResourceRef>,
    /// Resources this pass writes to.
    pub writes: Vec<ResourceRef>,
    /// Explicit dependencies (for non-resource ordering only).
    pub explicit_deps: Vec<RenderPassNodeId>,
    /// The executor that encodes GPU commands for this pass.
    pub executor: Box<dyn PassExecutor>,
}

impl RenderPassNode {
    /// Create a new render pass node.
    pub fn new(name: &'static str, executor: Box<dyn PassExecutor>) -> Self {
        Self {
            id: RenderPassNodeId(0), // Assigned by builder
            name,
            reads: Vec::new(),
            writes: Vec::new(),
            explicit_deps: Vec::new(),
            executor,
        }
    }

    /// Add a resource read dependency.
    pub fn read(mut self, resource: ResourceRef) -> Self {
        self.reads.push(resource);
        self
    }

    /// Add a resource write dependency.
    pub fn write(mut self, resource: ResourceRef) -> Self {
        self.writes.push(resource);
        self
    }

    /// Add an explicit ordering dependency (for non-resource constraints).
    pub fn depends_on(mut self, pass: RenderPassNodeId) -> Self {
        self.explicit_deps.push(pass);
        self
    }
}

/// Error emitted by a render graph pass during command encoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PassExecutionError {
    /// A pass referenced an attachment that was not bound during Compile.
    MissingAttachment {
        pass: &'static str,
        attachment: AttachmentHandle,
    },
    /// A pass referenced a buffer that was not bound during Compile.
    MissingBuffer {
        pass: &'static str,
        buffer: BufferHandle,
    },
}

impl PassExecutionError {
    pub const fn missing_attachment(pass: &'static str, attachment: AttachmentHandle) -> Self {
        Self::MissingAttachment { pass, attachment }
    }

    pub const fn missing_buffer(pass: &'static str, buffer: BufferHandle) -> Self {
        Self::MissingBuffer { pass, buffer }
    }
}

impl std::fmt::Display for PassExecutionError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PassExecutionError::MissingAttachment { pass, attachment } => {
                write!(
                    f,
                    "Pass '{pass}' references missing attachment {attachment:?}"
                )
            }
            PassExecutionError::MissingBuffer { pass, buffer } => {
                write!(f, "Pass '{pass}' references missing buffer {buffer:?}")
            }
        }
    }
}

impl std::error::Error for PassExecutionError {}

/// Trait for pass executors.
///
/// # Invariant (INV-RG-015)
///
/// Executors receive only:
/// - `&mut CommandEncoder` (via PassContext)
/// - Read-only `BoundResources` view
/// - Immutable per-frame constants (viewport, scale, etc.)
///
/// Executors MUST NOT have access to:
/// - Device
/// - Queue
/// - Surface
/// - Resource pools
/// - SceneCache
/// - Graph structure
pub trait PassExecutor: Send + Sync {
    /// Encode GPU commands for this pass.
    ///
    /// # Arguments
    ///
    /// * `ctx` - Constrained context with encoder and bound resources only.
    fn execute(&self, ctx: &mut PassContext) -> Result<(), PassExecutionError>;
}
