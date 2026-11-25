// ============================================================================
// SPEC-HAL-040 Stage 4: Staging Buffer Pool
//
// Reusable pool for CPU→GPU upload staging buffers. Replaces ad-hoc
// allocations with a single pool that recycles buffers across frames.
// ============================================================================

use super::FrameEpoch;
use super::gpu_arena::{GpuResourceArena, GpuResourceId, GpuResourceKind};

fn align_up(value: u64, alignment: u64) -> u64 {
    debug_assert!(alignment.is_power_of_two());
    (value + alignment - 1) & !(alignment - 1)
}

fn size_class(value: u64) -> u64 {
    let aligned = align_up(value.max(wgpu::MAP_ALIGNMENT), wgpu::MAP_ALIGNMENT);
    aligned.checked_next_power_of_two().unwrap_or(aligned)
}

/// A handle to a staging buffer from the pool.
///
/// Contains the arena ID for lifetime tracking, the actual wgpu buffer,
/// and the buffer's size in bytes.
#[derive(Debug)]
pub struct StagingBufferHandle {
    /// Arena ID for this buffer (used for lifetime tracking).
    pub id: GpuResourceId,
    /// The actual wgpu buffer.
    pub buffer: wgpu::Buffer,
    /// Buffer size in bytes.
    pub size: u64,
}

/// Pool of reusable staging buffers for CPU→GPU uploads.
///
/// Instead of creating and destroying staging buffers every frame,
/// this pool recycles them. Buffers are returned to the pool after
/// use and can be reused in subsequent frames.
///
/// ## Usage
///
/// 1. Call `alloc()` to get a staging buffer with at least the requested size
/// 2. Write data to the buffer and use it for uploads
/// 3. Call `recycle()` to return the buffer to the pool for reuse
///
/// ## Note
///
/// This stage does NOT implement eviction or destruction scheduling.
/// Buffers remain in the pool until explicitly removed (future stages).
pub struct StagingBufferPool {
    /// Pool of free buffers ready for reuse.
    free: Vec<StagingBufferHandle>,
    /// Total bytes allocated by the pool (for debugging/profiling).
    total_bytes: u64,
}

impl StagingBufferPool {
    /// Create a new, empty staging buffer pool.
    pub fn new() -> Self {
        Self {
            free: Vec::new(),
            total_bytes: 0,
        }
    }

    /// Get a staging buffer with capacity at least `min_size` bytes.
    ///
    /// If a suitable buffer is available in the pool, reuses it.
    /// Otherwise, creates a new buffer and registers it in the arena.
    ///
    /// ## Arguments
    ///
    /// * `device` - wgpu device for buffer creation
    /// * `arena` - GPU resource arena for lifetime tracking
    /// * `epoch` - Current frame epoch
    /// * `min_size` - Minimum required buffer size in bytes
    pub fn alloc(
        &mut self,
        device: &wgpu::Device,
        arena: &mut GpuResourceArena,
        epoch: FrameEpoch,
        min_size: u64,
    ) -> StagingBufferHandle {
        let min_size = size_class(min_size);

        // Best-fit preserves the request-size multiset across repeated frames.
        // First-fit can consume a larger class for a smaller request and force an
        // otherwise unnecessary allocation later in the same frame.
        if let Some(idx) = self
            .free
            .iter()
            .enumerate()
            .filter(|(_, handle)| handle.size >= min_size)
            .min_by_key(|(_, handle)| handle.size)
            .map(|(idx, _)| idx)
        {
            return self.free.swap_remove(idx);
        }

        // No suitable buffer found - create a new one
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("Staging Buffer"),
            size: min_size,
            usage: wgpu::BufferUsages::COPY_SRC | wgpu::BufferUsages::MAP_WRITE,
            mapped_at_creation: true,
        });

        let id = arena.register(GpuResourceKind::Buffer, epoch);
        self.total_bytes += min_size;

        StagingBufferHandle {
            id,
            buffer,
            size: min_size,
        }
    }

    /// Return a staging buffer to the pool for reuse.
    ///
    /// The buffer is NOT destroyed; it becomes available for
    /// subsequent `alloc()` calls.
    pub fn recycle(&mut self, handle: StagingBufferHandle) {
        self.free.push(handle);
    }

    /// Get the number of free buffers in the pool.
    #[cfg(test)]
    pub fn free_count(&self) -> usize {
        self.free.len()
    }

    /// Get the total bytes allocated by the pool.
    #[cfg(test)]
    pub fn total_bytes(&self) -> u64 {
        self.total_bytes
    }
}

impl Default for StagingBufferPool {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pool_new_is_empty() {
        let pool = StagingBufferPool::new();
        assert_eq!(pool.free_count(), 0);
        assert_eq!(pool.total_bytes(), 0);
    }

    // Note: Full alloc/recycle tests require a wgpu device,
    // which isn't available in unit tests without GPU initialization.
}
