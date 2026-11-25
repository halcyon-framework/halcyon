//! SPEC-HAL-084: RenderGraph v2 — GPU Pass Orchestration
//!
//! This module implements the RenderGraph system for orchestrating GPU work
//! with explicit phase separation (Build/Compile/Execute) and pass dependencies.
//!
//! # Architecture
//!
//! ```text
//! DrawPlan (from Batcher)
//!     ↓
//! RenderGraphBuilder::build(&DrawPlan)
//!     ↓
//! RenderGraphTopology (sorted passes + resource descriptors)
//!     ↓
//! RenderGraph::compile(&Topology, &mut ResourcePools)
//!     ↓
//! CompiledGraph (bound physical resources)
//!     ↓
//! RenderGraph::execute(&CompiledGraph, &mut CommandEncoder)
//!     ↓
//! Vec<CommandBuffer> (returned to platform for submission)
//! ```
//!
//! # Invariants (SPEC-HAL-084 Section B)
//!
//! - INV-RG-001: Orchestration only — no geometry generation
//! - INV-RG-002: DrawPlan is sole input; SceneCache never reaches RenderGraph
//! - INV-RG-003: Phase separation is absolute (Build/Compile/Execute)
//! - INV-RG-005: Dependencies derived from resource reads/writes
//! - INV-RG-007: Logical resources only; physical from pools during Compile
//! - INV-RG-011: All GPU passes are explicit graph nodes
//! - INV-RG-013: No submission/presentation — returns command buffers
//! - INV-RG-015: Pass executors have constrained access (no Device/Queue/pools)

pub mod builder;
pub mod executor;
mod forbidden;
pub mod graph;
mod guard;
pub mod pass;
pub mod passes;
pub mod resources;
pub mod topology;

pub use builder::{GraphError, RenderGraphBuilder};
pub use executor::{
    BoundResources, CompiledGraph, FrameConstants, FrameDrawData, PassContext, PassDrawScope,
    PipelineBuffers, RenderResources, TargetDrawContext,
};
pub use graph::RenderGraph;
pub use pass::{PassExecutionError, PassExecutor, RenderPassNode, RenderPassNodeId};
pub use passes::{MainPassExecutor, OverlayPassExecutor};
pub use resources::{
    AttachmentDesc, AttachmentHandle, AttachmentSize, BufferDesc, BufferHandle, ResourceDescriptor,
    ResourceRef,
};
pub use topology::RenderGraphTopology;
