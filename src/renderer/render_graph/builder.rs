//! SPEC-HAL-084: RenderGraph Builder
//!
//! Build phase entry point. Constructs graph topology from passes and resources.
//! Derives dependencies from resource reads/writes. Detects cycles.

use super::pass::{RenderPassNode, RenderPassNodeId};
use super::resources::{
    AttachmentDesc, AttachmentHandle, BufferDesc, BufferHandle, ResourceDescriptor, ResourceRef,
};
use super::topology::RenderGraphTopology;
use std::collections::{HashMap, HashSet};

/// Errors that can occur during graph construction.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GraphError {
    /// Dependency cycle detected.
    CycleDetected { path: Vec<&'static str> },
    /// Referenced resource does not exist.
    MissingResource {
        pass: &'static str,
        resource: ResourceRef,
    },
    /// Referenced pass dependency does not exist.
    MissingDependency {
        pass: &'static str,
        dep: RenderPassNodeId,
    },
    /// Duplicate pass name.
    DuplicatePassName { name: &'static str },
}

impl std::fmt::Display for GraphError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            GraphError::CycleDetected { path } => {
                write!(f, "Dependency cycle detected: {}", path.join(" -> "))
            }
            GraphError::MissingResource { pass, resource } => {
                write!(
                    f,
                    "Pass '{}' references missing resource {:?}",
                    pass, resource
                )
            }
            GraphError::MissingDependency { pass, dep } => {
                write!(f, "Pass '{}' depends on non-existent pass {:?}", pass, dep)
            }
            GraphError::DuplicatePassName { name } => {
                write!(f, "Duplicate pass name: '{}'", name)
            }
        }
    }
}

impl std::error::Error for GraphError {}

/// Builder for constructing a RenderGraph.
///
/// # Usage
///
/// ```ignore
/// let mut builder = RenderGraphBuilder::new();
///
/// let bg_texture = builder.declare_attachment(AttachmentDesc { ... });
/// let main_pass = builder.add_pass(
///     RenderPassNode::new("main", Box::new(MainPassExecutor))
///         .write(ResourceRef::Attachment(bg_texture))
/// );
///
/// let topology = builder.build(&draw_plan)?;
/// ```
pub struct RenderGraphBuilder {
    passes: Vec<RenderPassNode>,
    resources: Vec<ResourceDescriptor>,
    pass_names: HashSet<&'static str>,
    next_pass_id: u32,
    next_attachment_id: u32,
    next_buffer_id: u32,
}

impl RenderGraphBuilder {
    /// Create a new empty builder.
    pub fn new() -> Self {
        Self {
            passes: Vec::new(),
            resources: Vec::new(),
            pass_names: HashSet::new(),
            next_pass_id: 0,
            next_attachment_id: 0,
            next_buffer_id: 0,
        }
    }

    /// Declare a logical attachment and return its handle.
    pub fn declare_attachment(&mut self, desc: AttachmentDesc) -> AttachmentHandle {
        let handle = AttachmentHandle(self.next_attachment_id);
        self.next_attachment_id += 1;
        self.resources.push(ResourceDescriptor::Attachment(desc));
        handle
    }

    /// Declare a logical buffer and return its handle.
    pub fn declare_buffer(&mut self, desc: BufferDesc) -> BufferHandle {
        let handle = BufferHandle(self.next_buffer_id);
        self.next_buffer_id += 1;
        self.resources.push(ResourceDescriptor::Buffer(desc));
        handle
    }

    /// Add a render pass and return its ID.
    ///
    /// The pass ID can be used for explicit dependencies.
    pub fn add_pass(&mut self, mut pass: RenderPassNode) -> RenderPassNodeId {
        let id = RenderPassNodeId(self.next_pass_id);
        self.next_pass_id += 1;
        pass.id = id;
        self.pass_names.insert(pass.name);
        self.passes.push(pass);
        id
    }

    /// Build the graph topology.
    ///
    /// # Arguments
    ///
    /// * `_draw_plan` - The DrawPlan (INV-RG-002: sole input, no SceneCache/SceneGraph).
    ///
    /// # Errors
    ///
    /// Returns `GraphError` if:
    /// - Dependency cycle detected
    /// - Missing resource reference
    /// - Missing pass dependency
    /// - Duplicate pass name
    pub fn build<P: crate::compositor::Contract>(
        self,
        _draw_plan: &super::super::draw_plan::DrawPlan<P>,
    ) -> Result<RenderGraphTopology, GraphError> {
        // Validate pass names are unique
        // (Already enforced by HashSet, but check for duplicates in passes vec)
        let mut seen_names = HashSet::new();
        for pass in &self.passes {
            if !seen_names.insert(pass.name) {
                return Err(GraphError::DuplicatePassName { name: pass.name });
            }
        }

        // Validate resource references exist
        let attachment_count = self.next_attachment_id;
        let buffer_count = self.next_buffer_id;
        for pass in &self.passes {
            for res in pass.reads.iter().chain(pass.writes.iter()) {
                match res {
                    ResourceRef::Attachment(h) if h.0 >= attachment_count => {
                        return Err(GraphError::MissingResource {
                            pass: pass.name,
                            resource: res.clone(),
                        });
                    }
                    ResourceRef::Buffer(h) if h.0 >= buffer_count => {
                        return Err(GraphError::MissingResource {
                            pass: pass.name,
                            resource: res.clone(),
                        });
                    }
                    _ => {}
                }
            }
        }

        // Validate explicit dependencies exist
        let pass_count = self.passes.len() as u32;
        for pass in &self.passes {
            for dep in &pass.explicit_deps {
                if dep.0 >= pass_count {
                    return Err(GraphError::MissingDependency {
                        pass: pass.name,
                        dep: *dep,
                    });
                }
            }
        }

        // Build dependency graph from resource reads/writes
        let sorted_order = self.topological_sort()?;

        Ok(RenderGraphTopology {
            passes: self.passes,
            sorted_order,
            resources: self.resources,
        })
    }

    /// Perform topological sort with cycle detection.
    fn topological_sort(&self) -> Result<Vec<RenderPassNodeId>, GraphError> {
        // Build adjacency list from:
        // 1. Resource dependencies (write -> read)
        // 2. Explicit dependencies
        let mut adjacency: HashMap<u32, Vec<u32>> = HashMap::new();
        let mut in_degree: HashMap<u32, usize> = HashMap::new();

        // Initialize
        for pass in &self.passes {
            adjacency.entry(pass.id.0).or_default();
            in_degree.entry(pass.id.0).or_insert(0);
        }

        // Build resource -> writer map
        let mut resource_writers: HashMap<ResourceRef, u32> = HashMap::new();
        for pass in &self.passes {
            for res in &pass.writes {
                resource_writers.insert(*res, pass.id.0);
            }
        }

        // Add edges: if pass A writes resource R and pass B reads R, then A -> B
        for pass in &self.passes {
            for res in &pass.reads {
                if let Some(&writer_id) = resource_writers.get(res) {
                    if writer_id != pass.id.0 {
                        adjacency.entry(writer_id).or_default().push(pass.id.0);
                        *in_degree.entry(pass.id.0).or_insert(0) += 1;
                    }
                }
            }
        }

        // Add explicit dependency edges
        for pass in &self.passes {
            for dep in &pass.explicit_deps {
                adjacency.entry(dep.0).or_default().push(pass.id.0);
                *in_degree.entry(pass.id.0).or_insert(0) += 1;
            }
        }

        // Kahn's algorithm for topological sort
        let mut queue: Vec<u32> = in_degree
            .iter()
            .filter(|(_, deg)| **deg == 0)
            .map(|(id, _)| *id)
            .collect();
        // Sort for determinism
        queue.sort();

        let mut result = Vec::new();
        let mut remaining = in_degree.clone();

        while let Some(node) = queue.pop() {
            result.push(RenderPassNodeId(node));

            if let Some(neighbors) = adjacency.get(&node) {
                for &neighbor in neighbors {
                    if let Some(deg) = remaining.get_mut(&neighbor) {
                        *deg -= 1;
                        if *deg == 0 {
                            queue.push(neighbor);
                            queue.sort(); // Maintain deterministic order
                        }
                    }
                }
            }
        }

        // Check for cycle
        if result.len() != self.passes.len() {
            // Find cycle path for error message
            let in_cycle: Vec<&'static str> = self
                .passes
                .iter()
                .filter(|p| remaining.get(&p.id.0).copied().unwrap_or(0) > 0)
                .map(|p| p.name)
                .collect();
            return Err(GraphError::CycleDetected { path: in_cycle });
        }

        Ok(result)
    }
}

impl Default for RenderGraphBuilder {
    fn default() -> Self {
        Self::new()
    }
}
