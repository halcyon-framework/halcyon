// ============================================================================
// SPEC-HAL-040 Stage 2-3: Unified GpuResourceArena
//
// Central registry for all GPU resources. Stage 2 implements the bookkeeping
// structure. Stage 3 adds deferred destruction scheduling (metadata only).
// ============================================================================

use std::collections::HashMap;

use super::FrameEpoch;

/// SPEC-HAL-040: Unique identifier for a GPU resource in the arena.
///
/// This is a newtype wrapper around `u64`, assigned sequentially by the arena.
/// The ID is stable for the lifetime of the resource and is never reused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GpuResourceId(u64);

impl GpuResourceId {
    /// Get the raw ID value.
    #[inline]
    pub const fn value(self) -> u64 {
        self.0
    }
}

/// SPEC-HAL-040: Classification of GPU resource types.
///
/// Used to categorize resources for debugging, profiling, and future
/// destruction policy decisions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum GpuResourceKind {
    Buffer,
    Texture,
    TextureView,
    Sampler,
    BindGroup,
    Pipeline,
}

/// SPEC-HAL-040: Metadata for a single GPU resource.
///
/// Stores lifecycle information used for resource management decisions.
/// The arena does NOT hold actual GPU objects - only this metadata.
#[derive(Debug, Clone, Copy)]
pub struct GpuResourceMeta {
    /// The type of GPU resource.
    pub kind: GpuResourceKind,
    /// The frame epoch when this resource was created.
    pub epoch_created: FrameEpoch,
    /// The frame epoch when this resource was last used.
    pub epoch_last_used: FrameEpoch,
    /// SPEC-HAL-040 Stage 3: Destruction scheduling.
    /// None = not scheduled; Some(e) = eligible for removal when current_epoch >= e.
    pub destroy_after: Option<FrameEpoch>,
}

/// SPEC-HAL-040 Stage 2: Central registry for GPU resource metadata.
///
/// This is the bookkeeping layer for GPU resource lifetimes. It does NOT
/// hold actual GPU handles (wgpu::Buffer, wgpu::Texture, etc.) - those remain
/// in their original locations. The arena only tracks metadata for lifetime
/// decisions in later stages.
///
/// ## Stage 2 Scope
///
/// - Registration of new resources
/// - Updating last-used epoch
/// - Read-only metadata queries
///
/// ## Not Yet Implemented (Stage 3+)
///
/// - Resource destruction
/// - Pooling
/// - Safety rules
/// - Integration with actual GPU object creation
pub struct GpuResourceArena {
    /// Next ID to assign (monotonically increasing).
    next_id: u64,
    /// Metadata for all registered resources.
    resources: HashMap<GpuResourceId, GpuResourceMeta>,
}

impl GpuResourceArena {
    /// Create a new, empty arena.
    pub fn new() -> Self {
        Self {
            next_id: 0,
            resources: HashMap::new(),
        }
    }

    /// Register a new resource and return its unique ID.
    ///
    /// The resource is tagged with `epoch` as both its creation and last-used epoch.
    /// Returns a unique `GpuResourceId` that can be used to reference this resource.
    ///
    /// ## Arguments
    ///
    /// * `kind` - The type of GPU resource being registered
    /// * `epoch` - The current frame epoch (used for both created and last_used)
    pub fn register(&mut self, kind: GpuResourceKind, epoch: FrameEpoch) -> GpuResourceId {
        let id = GpuResourceId(self.next_id);
        self.next_id += 1;

        let meta = GpuResourceMeta {
            kind,
            epoch_created: epoch,
            epoch_last_used: epoch,
            destroy_after: None,
        };

        self.resources.insert(id, meta);
        id
    }

    /// Update the last-used epoch for a resource.
    ///
    /// Call this when a resource is used during rendering to track its activity.
    /// Does nothing if the resource ID is not found (defensive behavior).
    ///
    /// ## Arguments
    ///
    /// * `id` - The resource ID to update
    /// * `epoch` - The current frame epoch
    pub fn touch(&mut self, id: GpuResourceId, epoch: FrameEpoch) {
        if let Some(meta) = self.resources.get_mut(&id) {
            meta.epoch_last_used = epoch;
        }
    }

    /// Get read-only metadata for a resource.
    ///
    /// Returns `None` if the resource ID is not found.
    pub fn meta(&self, id: GpuResourceId) -> Option<&GpuResourceMeta> {
        self.resources.get(&id)
    }

    /// SPEC-HAL-040 Stage 3: Schedule a resource for destruction.
    ///
    /// Marks the resource as eligible for removal after `epoch_last_used.next()`.
    /// If the resource is already scheduled, this will only bring the destruction
    /// earlier (never delay it).
    ///
    /// Does nothing if the resource ID is not found.
    pub fn schedule_destroy(&mut self, id: GpuResourceId) {
        if let Some(meta) = self.resources.get_mut(&id) {
            let safe_epoch = meta.epoch_last_used.next();
            meta.destroy_after = Some(match meta.destroy_after {
                None => safe_epoch,
                Some(existing) => {
                    if safe_epoch < existing {
                        safe_epoch
                    } else {
                        existing
                    }
                }
            });
        }
    }

    /// SPEC-HAL-040 Stage 3: Remove metadata entries whose destruction epoch has matured.
    ///
    /// A resource is removed if `destroy_after == Some(e)` and `e <= current_epoch`.
    /// This only removes metadata - actual GPU destruction happens in Stage 4+.
    pub fn collect_garbage(&mut self, current_epoch: FrameEpoch) {
        self.resources.retain(|_, meta| match meta.destroy_after {
            None => true,
            Some(e) => e > current_epoch,
        });
    }

    /// Get the number of registered resources.
    #[cfg(test)]
    pub fn len(&self) -> usize {
        self.resources.len()
    }

    /// Check if the arena is empty.
    #[cfg(test)]
    pub fn is_empty(&self) -> bool {
        self.resources.is_empty()
    }
}

impl Default for GpuResourceArena {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_arena_new_is_empty() {
        let arena = GpuResourceArena::new();
        assert!(arena.is_empty());
        assert_eq!(arena.len(), 0);
    }

    #[test]
    fn test_register_returns_unique_ids() {
        let mut arena = GpuResourceArena::new();
        let epoch = FrameEpoch::zero();

        let id1 = arena.register(GpuResourceKind::Buffer, epoch);
        let id2 = arena.register(GpuResourceKind::Texture, epoch);
        let id3 = arena.register(GpuResourceKind::Pipeline, epoch);

        assert_ne!(id1, id2);
        assert_ne!(id2, id3);
        assert_ne!(id1, id3);

        // IDs should be sequential
        assert_eq!(id1.value(), 0);
        assert_eq!(id2.value(), 1);
        assert_eq!(id3.value(), 2);

        assert_eq!(arena.len(), 3);
    }

    #[test]
    fn test_register_stores_correct_metadata() {
        let mut arena = GpuResourceArena::new();
        let epoch = FrameEpoch(42);

        let id = arena.register(GpuResourceKind::BindGroup, epoch);
        let meta = arena.meta(id).expect("resource should exist");

        assert_eq!(meta.kind, GpuResourceKind::BindGroup);
        assert_eq!(meta.epoch_created, epoch);
        assert_eq!(meta.epoch_last_used, epoch);
    }

    #[test]
    fn test_touch_updates_last_used_epoch() {
        let mut arena = GpuResourceArena::new();
        let create_epoch = FrameEpoch(10);
        let use_epoch = FrameEpoch(50);

        let id = arena.register(GpuResourceKind::Sampler, create_epoch);

        // Touch with a later epoch
        arena.touch(id, use_epoch);

        let meta = arena.meta(id).expect("resource should exist");
        assert_eq!(
            meta.epoch_created, create_epoch,
            "created epoch should not change"
        );
        assert_eq!(
            meta.epoch_last_used, use_epoch,
            "last_used epoch should be updated"
        );
    }

    #[test]
    fn test_touch_nonexistent_does_nothing() {
        let mut arena = GpuResourceArena::new();
        let fake_id = GpuResourceId(9999);

        // Should not panic
        arena.touch(fake_id, FrameEpoch(100));

        // Arena should still be empty
        assert!(arena.is_empty());
    }

    #[test]
    fn test_meta_returns_none_for_nonexistent() {
        let arena = GpuResourceArena::new();
        let fake_id = GpuResourceId(42);

        assert!(arena.meta(fake_id).is_none());
    }

    #[test]
    fn test_gpu_resource_id_copy_and_hash() {
        use std::collections::HashSet;

        let id1 = GpuResourceId(42);
        let id2 = id1; // Copy
        let id3 = GpuResourceId(42);

        assert_eq!(id1, id2);
        assert_eq!(id1, id3);

        let mut set = HashSet::new();
        set.insert(id1);
        assert!(set.contains(&id2));
        assert!(set.contains(&id3));
    }

    #[test]
    fn test_all_resource_kinds() {
        let mut arena = GpuResourceArena::new();
        let epoch = FrameEpoch::zero();

        let kinds = [
            GpuResourceKind::Buffer,
            GpuResourceKind::Texture,
            GpuResourceKind::TextureView,
            GpuResourceKind::Sampler,
            GpuResourceKind::BindGroup,
            GpuResourceKind::Pipeline,
        ];

        for kind in kinds {
            let id = arena.register(kind, epoch);
            let meta = arena.meta(id).unwrap();
            assert_eq!(meta.kind, kind);
        }

        assert_eq!(arena.len(), 6);
    }
}
