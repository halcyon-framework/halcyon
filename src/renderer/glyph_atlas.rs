//! SPEC-HAL-029 + SPEC-HAL-066: Glyph Atlas Cache
//!
//! Provides GPU-resident glyph storage to eliminate redundant rasterization
//! and enable batched text rendering.
//!
//! Per SPEC-HAL-066, the atlas supports both grayscale (R8) and subpixel (RGBA8)
//! glyph formats, keyed by (font_id, glyph_id, px_size, format).

use crate::core::math::{RectF, Size};
use crate::platform::native_font_backend::GlyphFormat;
use crate::renderer::FrameEpoch;
use crate::renderer::gpu_arena::{GpuResourceArena, GpuResourceId, GpuResourceKind};
use std::collections::{HashMap, HashSet};
use std::sync::atomic::{AtomicU64, Ordering};

/// wgpu requires bytes_per_row to be a multiple of 256 for texture uploads.
const WGPU_COPY_BYTES_PER_ROW_ALIGNMENT: u32 = 256;

// Atlas padding constants - add transparent pixels around glyph bitmaps
// to prevent bottom cut-off and texture bleeding.
const ATLAS_PAD_LEFT: u32 = 1;
const ATLAS_PAD_RIGHT: u32 = 1;
const ATLAS_PAD_TOP: u32 = 1;
const ATLAS_PAD_BOTTOM: u32 = 2; // extra descender headroom

/// Align a value to the next multiple of 256.
#[inline]
fn align_to_copy_row(x: u32) -> u32 {
    ((x + WGPU_COPY_BYTES_PER_ROW_ALIGNMENT - 1) / WGPU_COPY_BYTES_PER_ROW_ALIGNMENT)
        * WGPU_COPY_BYTES_PER_ROW_ALIGNMENT
}

// ============================================================================
// SubpixelBinX - Horizontal subpixel quantization for atlas keying
// ============================================================================

/// SPEC-HAL-066/104: Horizontal subpixel phase quantization.
///
/// Quantizes the fractional X position into 4 bins for atlas keying.
/// This ensures glyphs at different subpixel phases get separate atlas entries,
/// which is critical for pixel-stable rendering (especially for LCD-AA).
///
/// egui uses exactly this model: 4 bins at 0.125, 0.375, 0.625, 0.875 centers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum SubpixelBinX {
    /// 0.000 - 0.125 fractional X
    #[default]
    Zero = 0,
    /// 0.125 - 0.375 fractional X
    One = 1,
    /// 0.375 - 0.625 fractional X
    Two = 2,
    /// 0.625 - 0.875 fractional X (and 0.875 - 1.0 wraps to Zero)
    Three = 3,
}

impl SubpixelBinX {
    /// Compute the subpixel bin from a fractional X position.
    ///
    /// SPEC-HAL-104: This should be called with the FINAL physical X position
    /// (after block origin snapping + glyph accumulation).
    #[inline]
    pub fn from_x(x: f32) -> Self {
        let frac = x - x.floor();
        if frac < 0.125 {
            SubpixelBinX::Zero
        } else if frac < 0.375 {
            SubpixelBinX::One
        } else if frac < 0.625 {
            SubpixelBinX::Two
        } else if frac < 0.875 {
            SubpixelBinX::Three
        } else {
            SubpixelBinX::Zero // 0.875-1.0 wraps to Zero
        }
    }
}

// ============================================================================
// GlyphKey - Unique identifier for cached glyphs
// ============================================================================

/// Unique key for a glyph in the atlas.
///
/// Per SPEC-HAL-066/104, this serves as the AtlasKey:
/// (font_id, glyph_id, px_size, format, subpixel_bin_x).
///
/// Two glyphs with the same key are guaranteed to produce identical bitmaps.
/// The subpixel_bin_x ensures glyphs at different horizontal phases are cached separately.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct GlyphKey {
    /// Font face identifier (0 for default)
    pub font_id: u64,
    /// Font size in logical pixels (scaled by 100 for precision)
    pub font_size_scaled: u32,
    /// Glyph ID from the font
    pub glyph_id: u32,
    /// DPI scaling factor (scaled by 100)
    pub dpi_scaled: u16,
    /// Glyph format (grayscale or subpixel RGB)
    pub format: GlyphFormat,
    /// SPEC-HAL-104: Horizontal subpixel phase bin
    pub subpixel_bin_x: SubpixelBinX,
}

impl GlyphKey {
    /// Create a key with default subpixel bin (Zero).
    ///
    /// SPEC-HAL-104: For proper pixel stability, use `with_subpixel_bin` instead
    /// and compute the bin from the actual glyph position.
    pub fn new(font_id: u64, font_size: f32, glyph_id: u32, dpi: f32) -> Self {
        Self {
            font_id,
            font_size_scaled: (font_size * 100.0) as u32,
            glyph_id,
            dpi_scaled: (dpi * 100.0) as u16,
            format: GlyphFormat::Grayscale,
            subpixel_bin_x: SubpixelBinX::Zero,
        }
    }

    /// Create a key with explicit format (for future subpixel support).
    pub fn with_format(
        font_id: u64,
        font_size: f32,
        glyph_id: u32,
        dpi: f32,
        format: GlyphFormat,
    ) -> Self {
        Self {
            font_id,
            font_size_scaled: (font_size * 100.0) as u32,
            glyph_id,
            dpi_scaled: (dpi * 100.0) as u16,
            format,
            subpixel_bin_x: SubpixelBinX::Zero,
        }
    }

    /// SPEC-HAL-104: Create a key with explicit subpixel bin.
    ///
    /// This is the preferred constructor for pixel-stable rendering.
    /// The subpixel_bin should be computed from the glyph's final X position
    /// using `SubpixelBinX::from_x()`.
    pub fn with_subpixel_bin(
        font_id: u64,
        font_size: f32,
        glyph_id: u32,
        dpi: f32,
        subpixel_bin_x: SubpixelBinX,
    ) -> Self {
        Self {
            font_id,
            font_size_scaled: (font_size * 100.0) as u32,
            glyph_id,
            dpi_scaled: (dpi * 100.0) as u16,
            format: GlyphFormat::Grayscale,
            subpixel_bin_x,
        }
    }
}

// ============================================================================
// GlyphMetrics - Metrics from rasterization
// ============================================================================

/// Metrics for a rasterized glyph.
#[derive(Debug, Clone, Copy)]
pub struct GlyphMetrics {
    /// Horizontal advance to next glyph
    pub advance: f32,
    /// Horizontal bearing (offset from origin to left edge of bitmap)
    pub bearing_x: f32,
    /// Vertical bearing (offset from baseline to top edge of bitmap)
    pub bearing_y: f32,
    /// Width of the glyph bitmap in pixels
    pub width: u32,
    /// Height of the glyph bitmap in pixels
    pub height: u32,
    /// Font ascender (reserved for future use)
    pub ascender: f32,
    /// Font descender (reserved for future use)
    pub descender: f32,
    /// Line height (reserved for future use)
    pub line_height: f32,
}

// ============================================================================
// AtlasEntry - Cached glyph data
// ============================================================================

/// Entry for a cached glyph in the atlas.
#[derive(Debug, Clone, Copy)]
pub struct AtlasEntry {
    /// UV rectangle in atlas space (0..1)
    pub uv_rect: RectF,
    /// Horizontal advance to next glyph
    pub advance: f32,
    /// Horizontal bearing
    pub bearing_x: f32,
    /// Vertical bearing
    pub bearing_y: f32,
    /// Size of the glyph in pixels (at oversample resolution)
    pub size: Size,
    /// Font ascender (reserved for future use)
    pub ascender: f32,
    /// Font descender (reserved for future use)
    pub descender: f32,
    /// Line height (reserved for future use)
    pub line_height: f32,
    /// R-010B: Oversample factor (bitmap is this many times larger than display size)
    pub oversample: f32,
}

#[derive(Debug)]
struct CachedAtlasEntry {
    entry: AtlasEntry,
    last_used: AtomicU64,
}

impl CachedAtlasEntry {
    fn new(entry: AtlasEntry, last_used: u64) -> Self {
        Self {
            entry,
            last_used: AtomicU64::new(last_used),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct AtlasSlot {
    x: u32,
    y: u32,
    w: u32,
    h: u32,
}

impl AtlasSlot {
    fn area(self) -> u32 {
        self.w.saturating_mul(self.h)
    }

    fn fits(self, w: u32, h: u32) -> bool {
        self.w >= w && self.h >= h
    }
}

#[derive(Debug, Clone, Copy)]
struct GlyphLocation {
    key: GlyphKey,
    slot: AtlasSlot,
}

fn glyph_format_rank(format: GlyphFormat) -> u8 {
    match format {
        GlyphFormat::Grayscale => 0,
        GlyphFormat::SubpixelRgb => 1,
    }
}

fn glyph_eviction_sort_key(key: GlyphKey, last_used: u64) -> (u64, u64, u32, u32, u16, u8, u8) {
    (
        last_used,
        key.font_id,
        key.font_size_scaled,
        key.glyph_id,
        key.dpi_scaled,
        glyph_format_rank(key.format),
        key.subpixel_bin_x as u8,
    )
}

// ============================================================================
// SkylinePacker - Simple bin packing for atlas allocation
// ============================================================================

/// A simple skyline-based bin packer for atlas allocation.
///
/// Uses a bottom-left skyline algorithm which is simpler than guillotine
/// but still efficient for typical glyph sizes.
pub struct SkylinePacker {
    /// Width of the atlas in pixels
    width: u32,
    /// Height of the atlas in pixels
    height: u32,
    /// Skyline segments: (x, y, width) - y is the height at position x
    skyline: Vec<(u32, u32, u32)>,
}

impl SkylinePacker {
    pub fn new(width: u32, height: u32) -> Self {
        Self {
            width,
            height,
            // Start with a single segment covering the entire width at y=0
            skyline: vec![(0, 0, width)],
        }
    }

    /// Reset the packer for a new atlas size
    pub fn reset(&mut self, width: u32, height: u32) {
        self.width = width;
        self.height = height;
        self.skyline.clear();
        self.skyline.push((0, 0, width));
    }

    /// Attempt to allocate a rectangle of the given size.
    /// Returns (x, y) position if successful, None if no space.
    pub fn alloc(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        if w == 0 || h == 0 {
            return Some((0, 0));
        }

        // Add 1px padding to prevent bleeding
        let padded_w = w + 1;
        let padded_h = h + 1;

        // Find the best position (lowest y that fits)
        let mut best_idx = None;
        let mut best_y = u32::MAX;
        let mut best_x = 0;

        for (i, &(x, _y, _seg_w)) in self.skyline.iter().enumerate() {
            // Check if this glyph can start at this segment
            if x + padded_w > self.width {
                continue;
            }

            // Find the maximum y across all segments this rect would span
            let max_y = self.find_min_y(i, padded_w);

            if max_y + padded_h <= self.height && max_y < best_y {
                best_y = max_y;
                best_x = x;
                best_idx = Some(i);
            }
        }

        let _idx = best_idx?;

        // Update the skyline
        self.add_rect(best_x, best_y, padded_w, padded_h);

        Some((best_x, best_y))
    }

    /// Find the minimum Y where a rect of given width can be placed starting at segment idx
    fn find_min_y(&self, start_idx: usize, width: u32) -> u32 {
        let start_x = self.skyline[start_idx].0;
        let end_x = start_x + width;
        let mut max_y = 0;

        for &(x, y, seg_w) in &self.skyline {
            // Check if this segment overlaps with our rect
            if x < end_x && x + seg_w > start_x {
                max_y = max_y.max(y);
            }
        }

        max_y
    }

    /// Add a rectangle to the skyline
    fn add_rect(&mut self, x: u32, y: u32, w: u32, h: u32) {
        let new_y = y + h;
        let rect_right = x + w;

        // Remove segments covered by this rect and add new segment
        let mut new_skyline = Vec::new();
        let mut i = 0;

        while i < self.skyline.len() {
            let (seg_x, seg_y, seg_w) = self.skyline[i];
            let seg_right = seg_x + seg_w;

            if seg_right <= x || seg_x >= rect_right {
                // Segment doesn't overlap - keep it
                new_skyline.push((seg_x, seg_y, seg_w));
            } else {
                // Segment overlaps with rect
                if seg_x < x {
                    // Left part of segment remains
                    new_skyline.push((seg_x, seg_y, x - seg_x));
                }
                if seg_right > rect_right {
                    // Right part of segment remains
                    new_skyline.push((rect_right, seg_y, seg_right - rect_right));
                }
            }
            i += 1;
        }

        // Add the new segment for the top of our rect
        new_skyline.push((x, new_y, w));

        // Sort by x position
        new_skyline.sort_by_key(|&(x, _, _)| x);

        // Merge adjacent segments with same height
        self.skyline.clear();
        for (seg_x, seg_y, seg_w) in new_skyline {
            if let Some(last) = self.skyline.last_mut() {
                if last.0 + last.2 == seg_x && last.1 == seg_y {
                    last.2 += seg_w;
                    continue;
                }
            }
            self.skyline.push((seg_x, seg_y, seg_w));
        }
    }
}

// ============================================================================
// GlyphAtlas - GPU-resident glyph storage
// ============================================================================

/// GPU-resident glyph atlas for text rendering.
///
/// Stores rasterized glyph bitmaps in a GPU texture and provides
/// UV coordinates for rendering text as textured quads.
///
/// SPEC-HAL-064 Phase 2: Uses RGBA8 format to support LCD subpixel coverage.
/// Each pixel stores {cov_r, cov_g, cov_b, 255} for subpixel glyphs,
/// or {gray, gray, gray, 255} for grayscale glyphs.
pub struct GlyphAtlas {
    /// GPU texture holding glyph bitmaps (RGBA8 format for LCD subpixel)
    pub texture: wgpu::Texture,
    /// Texture view for shader binding
    pub view: wgpu::TextureView,
    /// Sampler for texture filtering
    pub sampler: wgpu::Sampler,
    /// SPEC-HAL-040 Stage 5: Arena ID for the current atlas texture.
    /// Used for lifetime tracking and scheduled destruction on growth.
    pub texture_id: GpuResourceId,
    /// Bin packer for allocation
    packer: SkylinePacker,
    /// Cached glyph entries
    entries: HashMap<GlyphKey, CachedAtlasEntry>,
    /// Monotonic access counter for deterministic LRU eviction
    access_tick: AtomicU64,
    /// Current atlas width
    width: u32,
    /// Current atlas height
    height: u32,
    /// CPU-side bitmap data for repacking on grow
    cpu_data: Vec<u8>,
    /// Locations of glyphs in cpu_data for growth repack and eviction reuse
    glyph_locations: Vec<GlyphLocation>,
    /// Freed atlas rectangles available for max-size eviction reuse
    free_slots: Vec<AtlasSlot>,
    max_size: u32,
}

impl GlyphAtlas {
    #[cfg(test)]
    pub(crate) fn set_max_size_for_test(&mut self, max_size: u32) {
        assert!(max_size >= self.width.max(self.height) && max_size <= self.max_size);
        self.max_size = max_size;
    }

    #[cfg(test)]
    pub(crate) fn dimensions_for_test(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Maximum atlas dimension (8192x8192)
    pub const MAX_SIZE: u32 = 8192;

    /// Create a new glyph atlas with the given initial size.
    ///
    /// SPEC-HAL-040 Stage 5: The atlas texture is registered in the GPU arena
    /// for lifetime tracking.
    pub fn new(
        device: &wgpu::Device,
        width: u32,
        height: u32,
        arena: &mut GpuResourceArena,
        epoch: FrameEpoch,
    ) -> Self {
        let (texture, view) = Self::create_texture(device, width, height);

        // SPEC-HAL-040 Stage 5: Register texture in arena for lifetime tracking
        let texture_id = arena.register(GpuResourceKind::Texture, epoch);

        // Linear filtering for smooth anti-aliased text
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Glyph Atlas Sampler"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Nearest,
            ..Default::default()
        });

        Self {
            texture,
            view,
            sampler,
            texture_id,
            packer: SkylinePacker::new(width, height),
            entries: HashMap::new(),
            access_tick: AtomicU64::new(0),
            width,
            height,
            // SPEC-HAL-064 Phase 2: 4 bytes per pixel for RGBA8
            cpu_data: vec![0; (width * height * 4) as usize],
            glyph_locations: Vec::new(),
            free_slots: Vec::new(),
            max_size: Self::MAX_SIZE.min(device.limits().max_texture_dimension_2d),
        }
    }

    fn create_texture(
        device: &wgpu::Device,
        width: u32,
        height: u32,
    ) -> (wgpu::Texture, wgpu::TextureView) {
        // SPEC-HAL-064 Phase 2: Use RGBA8 linear for LCD subpixel coverage
        // R-010: Linear format (not sRGB) for correct gamma-aware blending in shader
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("Glyph Atlas Texture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        (texture, view)
    }

    /// Get an existing glyph entry from the cache.
    pub fn get(&self, key: &GlyphKey) -> Option<AtlasEntry> {
        let cached = self.entries.get(key)?;
        cached
            .last_used
            .store(self.next_access_tick(), Ordering::Relaxed);
        Some(cached.entry)
    }

    /// Check if a glyph is already cached.
    pub fn contains(&self, key: &GlyphKey) -> bool {
        if let Some(cached) = self.entries.get(key) {
            cached
                .last_used
                .store(self.next_access_tick(), Ordering::Relaxed);
            true
        } else {
            false
        }
    }

    fn next_access_tick(&self) -> u64 {
        self.access_tick
            .fetch_add(1, Ordering::Relaxed)
            .saturating_add(1)
    }

    fn allocate_slot(&mut self, w: u32, h: u32) -> Option<(u32, u32)> {
        Self::take_reusable_slot(&mut self.free_slots, w, h).or_else(|| self.packer.alloc(w, h))
    }

    fn take_reusable_slot(free_slots: &mut Vec<AtlasSlot>, w: u32, h: u32) -> Option<(u32, u32)> {
        let (best_idx, slot) = free_slots
            .iter()
            .copied()
            .enumerate()
            .filter(|(_, slot)| slot.fits(w, h))
            .min_by_key(|(_, slot)| {
                (
                    slot.area().saturating_sub(w.saturating_mul(h)),
                    slot.area(),
                    slot.y,
                    slot.x,
                )
            })?;

        free_slots.swap_remove(best_idx);

        if slot.w > w {
            free_slots.push(AtlasSlot {
                x: slot.x + w,
                y: slot.y,
                w: slot.w - w,
                h,
            });
        }

        if slot.h > h {
            free_slots.push(AtlasSlot {
                x: slot.x,
                y: slot.y + h,
                w: slot.w,
                h: slot.h - h,
            });
        }

        Self::coalesce_free_slots(free_slots);
        Some((slot.x, slot.y))
    }

    fn coldest_glyph_key(
        entries: &HashMap<GlyphKey, CachedAtlasEntry>,
        protected: &HashSet<GlyphKey>,
    ) -> Option<GlyphKey> {
        entries
            .iter()
            .filter(|(key, _)| !protected.contains(key))
            .min_by_key(|(key, cached)| {
                glyph_eviction_sort_key(**key, cached.last_used.load(Ordering::Relaxed))
            })
            .map(|(key, _)| *key)
    }

    fn evict_lru_until_slot(
        &mut self,
        w: u32,
        h: u32,
        queue: &wgpu::Queue,
        protected: &HashSet<GlyphKey>,
    ) -> Option<(u32, u32)> {
        loop {
            if let Some(pos) = Self::take_reusable_slot(&mut self.free_slots, w, h) {
                return Some(pos);
            }

            if self.entries.is_empty() {
                self.packer.reset(self.width, self.height);
                self.glyph_locations.clear();
                self.free_slots.clear();
                self.cpu_data.fill(0);
                return self.packer.alloc(w, h);
            }

            if !self.evict_coldest_glyph(queue, protected) {
                return None;
            }
        }
    }

    fn evict_coldest_glyph(&mut self, queue: &wgpu::Queue, protected: &HashSet<GlyphKey>) -> bool {
        let Some(key) = Self::coldest_glyph_key(&self.entries, protected) else {
            return false;
        };

        let Some(location_idx) = self
            .glyph_locations
            .iter()
            .position(|location| location.key == key)
        else {
            self.entries.remove(&key);
            tracing::warn!("Glyph atlas entry had no location during eviction");
            return true;
        };

        let location = self.glyph_locations.swap_remove(location_idx);
        self.entries.remove(&key);
        self.zero_slot(location.slot, queue);
        self.free_slots.push(location.slot);
        Self::coalesce_free_slots(&mut self.free_slots);

        tracing::debug!(
            "Evicted glyph {:?} from atlas slot {}x{} at {},{}",
            key,
            location.slot.w,
            location.slot.h,
            location.slot.x,
            location.slot.y
        );

        true
    }

    fn zero_slot(&mut self, slot: AtlasSlot, queue: &wgpu::Queue) {
        if slot.w == 0 || slot.h == 0 {
            return;
        }

        for row in 0..slot.h {
            let start = (((slot.y + row) * self.width + slot.x) * 4) as usize;
            let end = start + (slot.w * 4) as usize;
            if end <= self.cpu_data.len() {
                self.cpu_data[start..end].fill(0);
            }
        }

        let row_pitch = align_to_copy_row(slot.w * 4);
        let zeroed = vec![0u8; (row_pitch * slot.h) as usize];

        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d {
                    x: slot.x,
                    y: slot.y,
                    z: 0,
                },
                aspect: wgpu::TextureAspect::All,
            },
            &zeroed,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(row_pitch),
                rows_per_image: Some(slot.h),
            },
            wgpu::Extent3d {
                width: slot.w,
                height: slot.h,
                depth_or_array_layers: 1,
            },
        );
    }

    fn coalesce_free_slots(free_slots: &mut Vec<AtlasSlot>) {
        free_slots.retain(|slot| slot.w > 0 && slot.h > 0);

        loop {
            let mut merged = false;

            'outer: for i in 0..free_slots.len() {
                for j in (i + 1)..free_slots.len() {
                    if let Some(slot) = Self::merge_slots(free_slots[i], free_slots[j]) {
                        free_slots[i] = slot;
                        free_slots.swap_remove(j);
                        merged = true;
                        break 'outer;
                    }
                }
            }

            if !merged {
                break;
            }
        }
    }

    fn merge_slots(a: AtlasSlot, b: AtlasSlot) -> Option<AtlasSlot> {
        if a.y == b.y && a.h == b.h {
            if a.x + a.w == b.x {
                return Some(AtlasSlot {
                    x: a.x,
                    y: a.y,
                    w: a.w + b.w,
                    h: a.h,
                });
            }
            if b.x + b.w == a.x {
                return Some(AtlasSlot {
                    x: b.x,
                    y: b.y,
                    w: a.w + b.w,
                    h: a.h,
                });
            }
        }

        if a.x == b.x && a.w == b.w {
            if a.y + a.h == b.y {
                return Some(AtlasSlot {
                    x: a.x,
                    y: a.y,
                    w: a.w,
                    h: a.h + b.h,
                });
            }
            if b.y + b.h == a.y {
                return Some(AtlasSlot {
                    x: b.x,
                    y: b.y,
                    w: b.w,
                    h: a.h + b.h,
                });
            }
        }

        None
    }

    /// Insert a glyph into the atlas.
    ///
    /// Returns the atlas entry with UV coordinates.
    /// If the atlas is full, it will attempt to grow, then evict LRU entries at max size.
    ///
    /// SPEC-HAL-040 Stage 5: Arena and epoch are passed through for texture
    /// lifetime tracking on growth.
    ///
    /// SPEC-HAL-064 Phase 2: Handles both grayscale and SubpixelRgb formats.
    /// Grayscale glyphs are converted to RGBA {g,g,g,255}.
    /// SubpixelRgb glyphs are stored as {R,G,B,255}.
    ///
    /// `bitmap_format` specifies the actual format of the bitmap pixels,
    /// which may differ from the key's format field.
    ///
    /// `oversample` is the R-010B oversample factor (default 2.0). The bitmap
    /// is this many times larger than the display size.
    pub fn insert_glyph(
        &mut self,
        key: GlyphKey,
        bitmap: &[u8],
        bitmap_format: GlyphFormat,
        metrics: GlyphMetrics,
        oversample: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        arena: &mut GpuResourceArena,
        epoch: FrameEpoch,
    ) -> Option<AtlasEntry> {
        self.insert_glyph_protected(
            key,
            bitmap,
            bitmap_format,
            metrics,
            oversample,
            device,
            queue,
            arena,
            epoch,
            &HashSet::new(),
        )
    }

    pub(crate) fn insert_glyph_protected(
        &mut self,
        key: GlyphKey,
        bitmap: &[u8],
        bitmap_format: GlyphFormat,
        metrics: GlyphMetrics,
        oversample: f32,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        arena: &mut GpuResourceArena,
        epoch: FrameEpoch,
        protected: &HashSet<GlyphKey>,
    ) -> Option<AtlasEntry> {
        // Check if already cached
        if let Some(cached) = self.entries.get(&key) {
            cached
                .last_used
                .store(self.next_access_tick(), Ordering::Relaxed);
            return Some(cached.entry);
        }

        // Source bitmap dimensions (from rasterization)
        let src_w = metrics.width;
        let src_h = metrics.height;

        // Padded dimensions for atlas allocation
        let dst_w = src_w.checked_add(ATLAS_PAD_LEFT + ATLAS_PAD_RIGHT)?;
        let dst_h = src_h.checked_add(ATLAS_PAD_TOP + ATLAS_PAD_BOTTOM)?;
        if dst_w > self.max_size || dst_h > self.max_size {
            return None;
        }

        // Try to allocate space using padded dimensions
        let (x, y) = loop {
            if let Some(pos) = self.allocate_slot(dst_w, dst_h) {
                break pos;
            }

            if self.grow(device, queue, arena, epoch) {
                continue;
            }

            if let Some(pos) = self.evict_lru_until_slot(dst_w, dst_h, queue, protected) {
                break pos;
            }

            tracing::error!("Glyph atlas full and cannot evict enough space");
            return None;
        };

        // Upload bitmap to GPU with padding offset
        // SPEC-HAL-064 Phase 2: All pixels are now 4 bytes (RGBA)
        if src_w > 0 && src_h > 0 {
            // GPU row pitch must be 256-byte aligned for write_texture
            // Each pixel is 4 bytes (RGBA)
            let row_pitch = align_to_copy_row(dst_w * 4);
            let mut padded_buf = vec![0u8; (row_pitch * dst_h) as usize];

            // Determine if input is grayscale (1 byte/pixel) or SubpixelRgb (4 bytes/pixel)
            let is_subpixel = bitmap_format == GlyphFormat::SubpixelRgb;

            // Copy source bitmap into padded buffer at (ATLAS_PAD_LEFT, ATLAS_PAD_TOP)
            for row in 0..src_h {
                // Destination row in padded buffer (offset by ATLAS_PAD_TOP rows)
                let dst_row = row + ATLAS_PAD_TOP;

                for col in 0..src_w {
                    let dst_px = (dst_row * row_pitch + (ATLAS_PAD_LEFT + col) * 4) as usize;

                    if is_subpixel {
                        // SubpixelRgb: 4 bytes per pixel (R, G, B, X)
                        let src_px = ((row * src_w + col) * 4) as usize;
                        if src_px + 3 < bitmap.len() && dst_px + 3 < padded_buf.len() {
                            padded_buf[dst_px] = bitmap[src_px]; // R
                            padded_buf[dst_px + 1] = bitmap[src_px + 1]; // G
                            padded_buf[dst_px + 2] = bitmap[src_px + 2]; // B
                            padded_buf[dst_px + 3] = 255; // A = 255
                        }
                    } else {
                        // Grayscale: 1 byte per pixel, convert to RGBA {g,g,g,255}
                        let src_px = (row * src_w + col) as usize;
                        if src_px < bitmap.len() && dst_px + 3 < padded_buf.len() {
                            let g = bitmap[src_px];
                            padded_buf[dst_px] = g; // R
                            padded_buf[dst_px + 1] = g; // G
                            padded_buf[dst_px + 2] = g; // B
                            padded_buf[dst_px + 3] = 255; // A = 255
                        }
                    }
                }
            }

            queue.write_texture(
                wgpu::TexelCopyTextureInfo {
                    texture: &self.texture,
                    mip_level: 0,
                    origin: wgpu::Origin3d { x, y, z: 0 },
                    aspect: wgpu::TextureAspect::All,
                },
                &padded_buf,
                wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(row_pitch),
                    rows_per_image: Some(dst_h),
                },
                wgpu::Extent3d {
                    width: dst_w,
                    height: dst_h,
                    depth_or_array_layers: 1,
                },
            );

            // Store in CPU data for potential repacking (4 bytes per pixel)
            for row in 0..dst_h {
                for col in 0..dst_w {
                    let dst_atlas_px = (((y + row) * self.width + x + col) * 4) as usize;
                    let src_buf_px = ((row * row_pitch) + col * 4) as usize;
                    if dst_atlas_px + 3 < self.cpu_data.len() && src_buf_px + 3 < padded_buf.len() {
                        self.cpu_data[dst_atlas_px] = padded_buf[src_buf_px];
                        self.cpu_data[dst_atlas_px + 1] = padded_buf[src_buf_px + 1];
                        self.cpu_data[dst_atlas_px + 2] = padded_buf[src_buf_px + 2];
                        self.cpu_data[dst_atlas_px + 3] = padded_buf[src_buf_px + 3];
                    }
                }
            }

            // Store padded dimensions for growth repack and eviction reuse
            self.glyph_locations.push(GlyphLocation {
                key,
                slot: AtlasSlot {
                    x,
                    y,
                    w: dst_w,
                    h: dst_h,
                },
            });
        }

        // Compute UV coordinates (normalized 0..1)
        // UV rect covers the full padded rectangle
        // Shrink UVs inward by half a texel to prevent bleeding from neighbor pixels
        let inset_px = 0.5_f32;
        let tex_w = self.width as f32;
        let tex_h = self.height as f32;

        let u0 = (x as f32 + inset_px) / tex_w;
        let v0 = (y as f32 + inset_px) / tex_h;
        let u1 = (x as f32 + dst_w as f32 - inset_px) / tex_w;
        let v1 = (y as f32 + dst_h as f32 - inset_px) / tex_h;

        let uv_rect = RectF::new(u0, v0, u1 - u0, v1 - v0);

        // AtlasEntry.size is the PADDED size (so quad covers full padded area)
        // bearing_x and bearing_y remain unchanged - they describe offset from pen
        // to the real glyph bitmap, which now sits at (PAD_LEFT, PAD_TOP) within the padded rect
        let entry = AtlasEntry {
            uv_rect,
            advance: metrics.advance,
            bearing_x: metrics.bearing_x,
            bearing_y: metrics.bearing_y,
            size: Size::new(dst_w as f32, dst_h as f32),
            ascender: metrics.ascender,
            descender: metrics.descender,
            line_height: metrics.line_height,
            oversample,
        };

        let access_tick = self.next_access_tick();
        self.entries
            .insert(key, CachedAtlasEntry::new(entry, access_tick));
        Some(entry)
    }

    /// Attempt to grow the atlas by doubling dimensions.
    ///
    /// Returns true if growth succeeded, false if already at max size.
    ///
    /// SPEC-HAL-040 Stage 5: On growth, the old texture is scheduled for destruction
    /// and the new texture is registered in the arena.
    fn grow(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        arena: &mut GpuResourceArena,
        epoch: FrameEpoch,
    ) -> bool {
        let new_width = (self.width * 2).min(self.max_size);
        let new_height = (self.height * 2).min(self.max_size);

        if new_width == self.width && new_height == self.height {
            // Already at max size
            return false;
        }

        tracing::info!(
            "Growing glyph atlas from {}x{} to {}x{}",
            self.width,
            self.height,
            new_width,
            new_height
        );

        // Create new texture
        let (new_texture, new_view) = Self::create_texture(device, new_width, new_height);

        // Create new CPU buffer (4 bytes per pixel for RGBA)
        let mut new_cpu_data = vec![0u8; (new_width * new_height * 4) as usize];

        // Reset packer and re-insert all glyphs
        let mut new_packer = SkylinePacker::new(new_width, new_height);
        let mut new_entries = HashMap::new();
        let mut new_locations = Vec::new();

        // Re-pack all glyphs
        for location in &self.glyph_locations {
            let key = location.key;
            let AtlasSlot {
                x: old_x,
                y: old_y,
                w,
                h,
            } = location.slot;

            if let Some((new_x, new_y)) = new_packer.alloc(w, h) {
                // Copy from old CPU data to new CPU data (4 bytes per pixel)
                for row in 0..h {
                    for col in 0..w {
                        let src_px = (((old_y + row) * self.width + old_x + col) * 4) as usize;
                        let dst_px = (((new_y + row) * new_width + new_x + col) * 4) as usize;
                        if src_px + 3 < self.cpu_data.len() && dst_px + 3 < new_cpu_data.len() {
                            new_cpu_data[dst_px] = self.cpu_data[src_px];
                            new_cpu_data[dst_px + 1] = self.cpu_data[src_px + 1];
                            new_cpu_data[dst_px + 2] = self.cpu_data[src_px + 2];
                            new_cpu_data[dst_px + 3] = self.cpu_data[src_px + 3];
                        }
                    }
                }

                // Update entry with new UV coords
                if let Some(old_entry) = self.entries.get(&key) {
                    // Apply same UV inset as insert_glyph
                    let inset_px = 0.5_f32;
                    let tex_w = new_width as f32;
                    let tex_h = new_height as f32;

                    let u0 = (new_x as f32 + inset_px) / tex_w;
                    let v0 = (new_y as f32 + inset_px) / tex_h;
                    let u1 = (new_x as f32 + w as f32 - inset_px) / tex_w;
                    let v1 = (new_y as f32 + h as f32 - inset_px) / tex_h;

                    let new_uv_rect = RectF::new(u0, v0, u1 - u0, v1 - v0);
                    let new_entry = AtlasEntry {
                        uv_rect: new_uv_rect,
                        ..old_entry.entry
                    };
                    new_entries.insert(
                        key,
                        CachedAtlasEntry::new(
                            new_entry,
                            old_entry.last_used.load(Ordering::Relaxed),
                        ),
                    );
                    new_locations.push(GlyphLocation {
                        key,
                        slot: AtlasSlot {
                            x: new_x,
                            y: new_y,
                            w,
                            h,
                        },
                    });
                }
            } else {
                return false;
            }
        }

        // Commit the repack only after every retained glyph fits.
        arena.schedule_destroy(self.texture_id);
        let new_texture_id = arena.register(GpuResourceKind::Texture, epoch);

        // Upload entire new CPU data to GPU
        // Note: atlas dimensions are always powers of 2 >= 1024, so new_width * 4 is always
        // a multiple of 256 and no padding is needed here.
        debug_assert!(
            (new_width * 4) % WGPU_COPY_BYTES_PER_ROW_ALIGNMENT == 0,
            "Atlas width * 4 must be aligned to 256 bytes"
        );
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &new_texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &new_cpu_data,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(new_width * 4), // 4 bytes per pixel
                rows_per_image: Some(new_height),
            },
            wgpu::Extent3d {
                width: new_width,
                height: new_height,
                depth_or_array_layers: 1,
            },
        );

        // Update state
        self.texture = new_texture;
        self.view = new_view;
        self.texture_id = new_texture_id;
        self.width = new_width;
        self.height = new_height;
        self.cpu_data = new_cpu_data;
        self.packer = new_packer;
        self.entries = new_entries;
        self.glyph_locations = new_locations;
        self.free_slots.clear();

        true
    }

    /// Get the current atlas dimensions.
    pub fn size(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get the number of cached glyphs.
    pub fn glyph_count(&self) -> usize {
        self.entries.len()
    }

    /// Clear all cached glyphs from the atlas.
    ///
    /// Resets the packer and entry map while keeping the GPU texture allocated.
    /// The texture is cleared to zeros.
    pub fn clear(&mut self, queue: &wgpu::Queue) {
        // Reset packer to initial state
        self.packer.reset(self.width, self.height);

        // Clear entry map
        self.entries.clear();

        // Clear glyph locations
        self.glyph_locations.clear();

        // Clear reusable slots and access counters
        self.free_slots.clear();
        self.access_tick.store(0, Ordering::Relaxed);

        // Clear CPU data
        self.cpu_data.fill(0);

        // Clear GPU texture by uploading zeroed data (4 bytes per pixel)
        let zeroed = vec![0u8; (self.width * self.height * 4) as usize];
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &self.texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &zeroed,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(self.width * 4), // 4 bytes per pixel
                rows_per_image: Some(self.height),
            },
            wgpu::Extent3d {
                width: self.width,
                height: self.height,
                depth_or_array_layers: 1,
            },
        );

        tracing::info!("Glyph atlas cleared ({}x{})", self.width, self.height);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_glyph_key_equality() {
        let k1 = GlyphKey::new(0, 16.0, 65, 1.0);
        let k2 = GlyphKey::new(0, 16.0, 65, 1.0);
        let k3 = GlyphKey::new(0, 16.0, 66, 1.0);

        assert_eq!(k1, k2);
        assert_ne!(k1, k3);
        // Default format is Grayscale
        assert_eq!(k1.format, GlyphFormat::Grayscale);
        // Default subpixel bin is Zero
        assert_eq!(k1.subpixel_bin_x, SubpixelBinX::Zero);
    }

    #[test]
    fn test_glyph_key_format() {
        let k1 = GlyphKey::new(0, 16.0, 65, 1.0);
        let k2 = GlyphKey::with_format(0, 16.0, 65, 1.0, GlyphFormat::SubpixelRgb);

        // Same glyph, different format = different key
        assert_ne!(k1, k2);
        assert_eq!(k1.format, GlyphFormat::Grayscale);
        assert_eq!(k2.format, GlyphFormat::SubpixelRgb);
    }

    #[test]
    fn test_glyph_key_subpixel_bin() {
        let k1 = GlyphKey::new(0, 16.0, 65, 1.0);
        let k2 = GlyphKey::with_subpixel_bin(0, 16.0, 65, 1.0, SubpixelBinX::One);
        let k3 = GlyphKey::with_subpixel_bin(0, 16.0, 65, 1.0, SubpixelBinX::Two);

        // Same glyph, different subpixel bin = different key
        assert_ne!(k1, k2);
        assert_ne!(k2, k3);
        assert_eq!(k1.subpixel_bin_x, SubpixelBinX::Zero);
        assert_eq!(k2.subpixel_bin_x, SubpixelBinX::One);
        assert_eq!(k3.subpixel_bin_x, SubpixelBinX::Two);
    }

    #[test]
    fn test_subpixel_bin_from_x() {
        // Test bin boundaries
        assert_eq!(SubpixelBinX::from_x(0.0), SubpixelBinX::Zero);
        assert_eq!(SubpixelBinX::from_x(0.1), SubpixelBinX::Zero);
        assert_eq!(SubpixelBinX::from_x(0.125), SubpixelBinX::One);
        assert_eq!(SubpixelBinX::from_x(0.3), SubpixelBinX::One);
        assert_eq!(SubpixelBinX::from_x(0.375), SubpixelBinX::Two);
        assert_eq!(SubpixelBinX::from_x(0.5), SubpixelBinX::Two);
        assert_eq!(SubpixelBinX::from_x(0.625), SubpixelBinX::Three);
        assert_eq!(SubpixelBinX::from_x(0.8), SubpixelBinX::Three);
        assert_eq!(SubpixelBinX::from_x(0.875), SubpixelBinX::Zero); // wraps
        assert_eq!(SubpixelBinX::from_x(0.99), SubpixelBinX::Zero);

        // Test with integer + fraction
        assert_eq!(SubpixelBinX::from_x(100.0), SubpixelBinX::Zero);
        assert_eq!(SubpixelBinX::from_x(100.3), SubpixelBinX::One);
        assert_eq!(SubpixelBinX::from_x(100.5), SubpixelBinX::Two);
        assert_eq!(SubpixelBinX::from_x(100.7), SubpixelBinX::Three);
    }

    #[test]
    fn test_skyline_packer_basic() {
        let mut packer = SkylinePacker::new(256, 256);

        // Allocate some rects
        let pos1 = packer.alloc(32, 32);
        assert!(pos1.is_some());
        let (x1, y1) = pos1.unwrap();
        assert_eq!((x1, y1), (0, 0));

        let pos2 = packer.alloc(32, 32);
        assert!(pos2.is_some());
        let (x2, y2) = pos2.unwrap();
        // Should be next to or below first rect
        assert!(x2 == 33 || y2 == 33);
    }

    #[test]
    fn test_skyline_packer_fills() {
        let mut packer = SkylinePacker::new(64, 64);

        // Fill with small rects
        let mut count = 0;
        while packer.alloc(8, 8).is_some() {
            count += 1;
            if count > 100 {
                break; // Prevent infinite loop in case of bug
            }
        }

        // With 1px padding, 9x9 effective size
        // 64/9 = 7 per row, 7 rows = 49 glyphs approximately
        assert!(
            count >= 30,
            "Should fit at least 30 8x8 glyphs, got {}",
            count
        );
    }

    fn dummy_entry() -> AtlasEntry {
        AtlasEntry {
            uv_rect: RectF::new(0.0, 0.0, 1.0, 1.0),
            advance: 0.0,
            bearing_x: 0.0,
            bearing_y: 0.0,
            size: Size::new(1.0, 1.0),
            ascender: 0.0,
            descender: 0.0,
            line_height: 0.0,
            oversample: 1.0,
        }
    }

    #[test]
    fn test_coldest_glyph_key_uses_lru_then_key_order() {
        let mut entries = HashMap::new();
        let older = GlyphKey::new(0, 12.0, 65, 1.0);
        let newer = GlyphKey::new(0, 12.0, 66, 1.0);
        let tie_lower_key = GlyphKey::new(0, 12.0, 64, 1.0);

        entries.insert(older, CachedAtlasEntry::new(dummy_entry(), 10));
        entries.insert(newer, CachedAtlasEntry::new(dummy_entry(), 20));
        entries.insert(tie_lower_key, CachedAtlasEntry::new(dummy_entry(), 10));

        assert_eq!(
            GlyphAtlas::coldest_glyph_key(&entries, &HashSet::new()),
            Some(tie_lower_key)
        );
        let mut protected = HashSet::from([tie_lower_key, older]);
        assert_eq!(
            GlyphAtlas::coldest_glyph_key(&entries, &protected),
            Some(newer)
        );
        protected.insert(newer);
        assert_eq!(GlyphAtlas::coldest_glyph_key(&entries, &protected), None);
    }

    #[test]
    fn test_take_reusable_slot_best_fit_and_split() {
        let mut free_slots = vec![
            AtlasSlot {
                x: 0,
                y: 0,
                w: 64,
                h: 64,
            },
            AtlasSlot {
                x: 128,
                y: 0,
                w: 20,
                h: 20,
            },
        ];

        let pos = GlyphAtlas::take_reusable_slot(&mut free_slots, 16, 16);

        assert_eq!(pos, Some((128, 0)));
        assert!(free_slots.iter().any(|slot| *slot
            == AtlasSlot {
                x: 144,
                y: 0,
                w: 4,
                h: 16,
            }));
        assert!(free_slots.iter().any(|slot| *slot
            == AtlasSlot {
                x: 128,
                y: 16,
                w: 20,
                h: 4,
            }));
    }

    #[test]
    fn test_coalesce_free_slots_merges_adjacent_rectangles() {
        let mut free_slots = vec![
            AtlasSlot {
                x: 10,
                y: 0,
                w: 10,
                h: 10,
            },
            AtlasSlot {
                x: 0,
                y: 0,
                w: 10,
                h: 10,
            },
            AtlasSlot {
                x: 0,
                y: 10,
                w: 20,
                h: 10,
            },
        ];

        GlyphAtlas::coalesce_free_slots(&mut free_slots);

        assert_eq!(
            free_slots,
            vec![AtlasSlot {
                x: 0,
                y: 0,
                w: 20,
                h: 20,
            }]
        );
    }
}
