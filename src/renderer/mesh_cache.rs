//! SPEC-HAL-264: Vector Mesh Cache
//!
//! Content-addressed tessellation cache for GPU-ready vector geometry.
//! Eliminates per-frame tessellation for stable paths (icons, decorations, repeated shapes).
//!
//! INV-VMP-001: Renderer-owned. No external module may access cache entries.
//! INV-VMP-004: Color-independent. Cache stores geometry only.

use std::collections::HashMap;

use super::path::{FillRule, PathMesh};

// ============================================================================
// PixelSnapMode
// ============================================================================

/// Pixel snap strategy for cached vector meshes.
///
/// SPEC-HAL-264 §8.3: Controls how tessellated vertices align to the physical pixel grid.
/// Default: `Center` for fills, `Edge` for strokes.
#[derive(Clone, Copy, Hash, Eq, PartialEq, Debug)]
pub enum PixelSnapMode {
    /// No snapping. Continuous floating-point coordinates.
    None,
    /// Snap vertices to nearest physical pixel center.
    /// Best for filled shapes at small sizes.
    Center,
    /// Snap vertices to nearest physical pixel edge (half-pixel offset).
    /// Best for 1px strokes — centers the stroke on a full pixel.
    Edge,
}

// ============================================================================
// MeshCacheKey
// ============================================================================

/// Content-addressed key for cached tessellated meshes.
///
/// SPEC-HAL-264 §4: Captures every parameter affecting tessellation output.
/// INV-VMP-002: Equal keys produce identical tessellation output.
#[derive(Clone, Hash, Eq, PartialEq, Debug)]
pub struct MeshCacheKey {
    /// Content hash of the path commands (MoveTo, LineTo, etc.)
    pub path_hash: u64,
    /// Logical render width, quantized to 0.5px steps.
    pub render_width_q: u32,
    /// Logical render height, quantized to 0.5px steps.
    pub render_height_q: u32,
    /// Stroke width in logical pixels, quantized to 0.25px steps. Zero for fills.
    pub stroke_width_q: u32,
    /// Fill mode: true = fill, false = stroke-only.
    pub is_fill: bool,
    /// Winding rule for the combined fill (SPEC-R-004 §13).
    pub fill_rule: FillRule,
    /// Device pixel ratio, quantized to 0.125 steps.
    pub dpr_q: u32,
    /// Exact effective device pixel ratio bits used for projection.
    pub dpr_bits: u32,
    /// Pixel snap mode.
    pub snap_mode: PixelSnapMode,
}

impl MeshCacheKey {
    /// Quantize render size to 0.5 logical pixel steps.
    pub fn quantize_size(size: f32) -> u32 {
        (size * 2.0).round() as u32
    }

    /// Quantize stroke width to 0.25 logical pixel steps.
    pub fn quantize_stroke(width: f32) -> u32 {
        (width * 4.0).round() as u32
    }

    /// Quantize device pixel ratio to 0.125 steps.
    pub fn quantize_dpr(dpr: f32) -> u32 {
        (dpr * 8.0).round() as u32
    }
}

// ============================================================================
// MeshCacheEntry (internal)
// ============================================================================

/// Internal cache entry storing tessellated mesh and access metadata.
///
/// SPEC-HAL-264 §5: Stores geometry only — no color, tint, or theme data.
struct MeshCacheEntry {
    mesh: PathMesh,
    last_used_frame: u64,
}

// ============================================================================
// MeshCache
// ============================================================================

/// Centralized tessellation cache for GPU-ready vector geometry.
///
/// SPEC-HAL-264 §6: Content-addressed, DPR-aware, with LRU eviction.
/// INV-VMP-001: Renderer-owned exclusively.
/// INV-VMP-003: If a cache entry exists, `to_mesh()` must not be called.
pub struct MeshCache {
    entries: HashMap<MeshCacheKey, MeshCacheEntry>,
}

impl MeshCache {
    /// Create a new empty mesh cache.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Look up a cached mesh by key.
    ///
    /// On hit: updates `last_used_frame`, returns mesh reference.
    /// On miss: returns `None`.
    pub fn get(&mut self, key: &MeshCacheKey, current_frame: u64) -> Option<&PathMesh> {
        self.entries.get_mut(key).map(|entry| {
            entry.last_used_frame = current_frame;
            &entry.mesh
        })
    }

    /// Insert a tessellated mesh into the cache.
    ///
    /// Replaces any existing entry with the same key (idempotent).
    pub fn insert(&mut self, key: MeshCacheKey, mesh: PathMesh, current_frame: u64) {
        self.entries.insert(
            key,
            MeshCacheEntry {
                mesh,
                last_used_frame: current_frame,
            },
        );
    }

    /// Evict stale entries not accessed for `max_age_frames` frames.
    ///
    /// SPEC-HAL-264 §6.3: Bounded eviction to avoid latency spikes.
    /// Removes up to `max_evictions` entries per call.
    pub fn evict_stale(&mut self, current_frame: u64, max_age_frames: u64, max_evictions: usize) {
        let mut evicted = 0;
        self.entries.retain(|_, entry| {
            if evicted >= max_evictions {
                return true;
            }
            if current_frame.saturating_sub(entry.last_used_frame) > max_age_frames {
                evicted += 1;
                false
            } else {
                true
            }
        });
    }

    /// Drop all cache entries.
    ///
    /// INV-VMP-008: Used on device loss and DPI change.
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of entries in the cache.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}
