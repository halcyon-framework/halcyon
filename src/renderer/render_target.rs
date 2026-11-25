//! SPEC-HAL-081B: Render Target Types for Multi-Surface Rendering
//!
//! This module provides the types needed to support rendering to multiple
//! compositor surfaces. Each surface gets its own WGPU surface and configuration.
//!
//! # Invariants (Hardened)
//!
//! - INV-RT-001: Registry owns all SurfaceBundles; no external handles
//! - INV-RT-002: SurfaceId uniqueness - no duplicate registration (returns error)
//! - INV-RT-003: Unregister removes target atomically
//! - INV-RT-004: Resize only valid for registered, non-acquired targets
//! - INV-RT-005: Acquire returns RAII guard; target locked until guard dropped
//! - INV-RT-006: Double-acquire same target is error
//! - INV-RT-007: Release via guard Drop only (no manual release)
//! - INV-RT-008: Render only through acquired targets
//! - INV-RT-009: Unregister while acquired is error
//! - INV-RT-010: Registry iteration excludes acquired targets

use crate::compositor::SurfaceId;
use std::cell::Cell;
use std::collections::HashMap;

/// Acquisition and target failures reported by the renderer.
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RendererSurfaceError {
    #[error("A timeout was encountered while trying to acquire the next frame")]
    Timeout,
    #[error("The underlying surface has changed, and therefore the swap chain must be updated")]
    Outdated,
    #[error("The swap chain has been lost and needs to be recreated")]
    Lost,
    #[error("There is no more memory left to allocate a new frame")]
    OutOfMemory,
    #[error("Acquiring a texture failed with a generic error. Check error callbacks for more information")]
    Other,
    #[error("The surface is occluded")]
    Occluded,
    #[error("Surface texture acquisition failed validation")]
    Validation,
}

// ============================================================================
// Error Types (INV-RT-002, INV-RT-004, INV-RT-006, INV-RT-009)
// ============================================================================

/// Typed errors for RenderTargetRegistry operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderTargetError {
    /// INV-RT-002: Attempted to register a SurfaceId that already exists.
    DuplicateRegistration { surface_id: SurfaceId },
    /// Target not found in registry.
    NotRegistered { surface_id: SurfaceId },
    /// INV-RT-006: Target is already acquired (double-acquire).
    AlreadyAcquired { surface_id: SurfaceId },
    /// INV-RT-009: Cannot unregister while target is acquired.
    UnregisterWhileAcquired { surface_id: SurfaceId },
    /// INV-RT-004: Cannot resize while target is acquired.
    ResizeWhileAcquired { surface_id: SurfaceId },
    /// SPEC-HAL-024 E.5: the observed target extent has no configured output.
    InactiveTarget { surface_id: SurfaceId },
    /// Headless targets have no surface and cannot be acquired for presentation.
    HeadlessCannotAcquire { surface_id: SurfaceId },
}

impl std::fmt::Display for RenderTargetError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            RenderTargetError::DuplicateRegistration { surface_id } => {
                write!(f, "Duplicate registration for SurfaceId {:?}", surface_id)
            }
            RenderTargetError::NotRegistered { surface_id } => {
                write!(f, "SurfaceId {:?} not registered", surface_id)
            }
            RenderTargetError::AlreadyAcquired { surface_id } => {
                write!(f, "SurfaceId {:?} already acquired", surface_id)
            }
            RenderTargetError::UnregisterWhileAcquired { surface_id } => {
                write!(
                    f,
                    "Cannot unregister SurfaceId {:?} while acquired",
                    surface_id
                )
            }
            RenderTargetError::ResizeWhileAcquired { surface_id } => {
                write!(f, "Cannot resize SurfaceId {:?} while acquired", surface_id)
            }
            RenderTargetError::HeadlessCannotAcquire { surface_id } => {
                write!(
                    f,
                    "Cannot acquire headless SurfaceId {:?}: no surface for presentation",
                    surface_id
                )
            }
            RenderTargetError::InactiveTarget { surface_id } => {
                write!(
                    f,
                    "SurfaceId {:?} has no active configured extent",
                    surface_id
                )
            }
        }
    }
}

impl std::error::Error for RenderTargetError {}

/// SPEC-HAL-081B: Render target keyed by compositor surface identity.
///
/// This is the primary handle for identifying which surface to render to.
/// It wraps the compositor's SurfaceId to establish the connection between
/// compositor surfaces and renderer targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct RenderTarget(pub SurfaceId);

impl RenderTarget {
    /// Create a render target from a compositor SurfaceId.
    #[inline]
    pub fn from_surface(id: SurfaceId) -> Self {
        Self(id)
    }

    /// Get the underlying SurfaceId.
    #[inline]
    pub fn surface_id(&self) -> SurfaceId {
        self.0
    }
}

/// SPEC-HAL-081B: Per-surface GPU state bundle.
///
/// Contains the WGPU surface needed to render to a single target.
/// Multiple SurfaceBundles can coexist, sharing the same Device and Queue.
///
/// # Configuration Authority
///
/// SurfaceBundle does NOT store SurfaceConfiguration. The renderer is the
/// single authority for swapchain configuration. The registry applies
/// configurations verbatim when instructed.
///
/// # Size Authority
///
/// SurfaceBundle does NOT store logical/physical size or scale factor.
/// Size data is compositor + renderer concern, not registry concern.
/// The registry only stores what's needed for execution-time acquisition.
pub struct SurfaceBundle {
    /// The WGPU surface for this target.
    pub surface: wgpu::Surface<'static>,
}

impl SurfaceBundle {
    /// Create a new surface bundle.
    ///
    /// # Arguments
    ///
    /// * `surface` - The WGPU surface
    pub fn new(surface: wgpu::Surface<'static>) -> Self {
        Self { surface }
    }

    /// Configure the surface with the provided configuration.
    ///
    /// The registry applies configurations verbatim. The renderer is the
    /// single authority for swapchain policy; this method just executes.
    ///
    /// # Arguments
    ///
    /// * `device` - GPU device (shared)
    /// * `config` - Surface configuration (renderer-provided)
    pub fn configure(&self, device: &wgpu::Device, config: &wgpu::SurfaceConfiguration) {
        self.surface.configure(device, config);
    }

    /// Get the current texture for this frame.
    ///
    /// Returns the acquired surface texture or a surface error.
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, RendererSurfaceError> {
        use wgpu::CurrentSurfaceTexture;
        match self.surface.get_current_texture() {
            CurrentSurfaceTexture::Success(texture)
            | CurrentSurfaceTexture::Suboptimal(texture) => Ok(texture),
            CurrentSurfaceTexture::Timeout => Err(RendererSurfaceError::Timeout),
            CurrentSurfaceTexture::Occluded => Err(RendererSurfaceError::Occluded),
            CurrentSurfaceTexture::Outdated => Err(RendererSurfaceError::Outdated),
            CurrentSurfaceTexture::Lost => Err(RendererSurfaceError::Lost),
            CurrentSurfaceTexture::Validation => Err(RendererSurfaceError::Validation),
        }
    }
}

// ============================================================================
// Registry Entry (per-target state)
// ============================================================================

/// Per-target entry in the registry.
///
/// Contains the GPU surface bundle, acquisition state, and configured size.
/// Acquisition state is per-entry, not global.
pub(crate) struct RenderTargetEntry {
    /// Surface or headless backing.
    kind: RenderTargetKind,
    /// Whether this target is currently acquired.
    /// Uses Cell for interior mutability (guard drops can release).
    acquired: Cell<bool>,
    /// INV-SCISSOR-001: Authoritative configured size in physical pixels.
    /// This is the exact (width, height) last passed to Surface::configure().
    /// Updated ONLY in configure(). Uses Cell for interior mutability.
    pub(crate) configured_size_px: Cell<(u32, u32)>,
    /// Current App-observed extent, including zero-area inactivity. This does not
    /// overwrite the last nonzero GPU configuration or change compositor visibility.
    observed_size_px: Cell<(u32, u32)>,
    /// INV-SCALE-001: Per-target scale factor (DPI scaling).
    /// Each target can have a different scale factor (multi-DPI monitors).
    /// Uses Cell for interior mutability.
    scale_factor: Cell<f32>,
}

impl RenderTargetEntry {
    fn new(bundle: SurfaceBundle, initial_size: (u32, u32), scale_factor: f32) -> Self {
        Self {
            kind: RenderTargetKind::Surface(bundle),
            acquired: Cell::new(false),
            configured_size_px: Cell::new(initial_size),
            observed_size_px: Cell::new(initial_size),
            scale_factor: Cell::new(scale_factor),
        }
    }

    fn new_headless(initial_size: (u32, u32), scale_factor: f32) -> Self {
        Self {
            kind: RenderTargetKind::Headless,
            acquired: Cell::new(false),
            configured_size_px: Cell::new(initial_size),
            observed_size_px: Cell::new(initial_size),
            scale_factor: Cell::new(scale_factor),
        }
    }
}

/// Render target backing: either a real surface or headless (compile-phase only).
pub(crate) enum RenderTargetKind {
    /// Surface-backed target for windowed rendering.
    Surface(SurfaceBundle),
    /// Headless target (compile-phase queries only, no surface for presentation).
    Headless,
}

/// SPEC-HAL-081B: Registry of render targets (hardened).
///
/// Maps compositor SurfaceIds to their corresponding GPU surface bundles.
/// This is the central registry for all active render targets.
///
/// # Invariants (Hardened)
///
/// - INV-RT-001: Registry owns all SurfaceBundles; no external handles
/// - INV-RT-002: SurfaceId uniqueness enforced (no duplicates)
/// - INV-RT-005: Acquire returns RAII guard
/// - INV-RT-010: Iteration excludes acquired targets
///
/// # State Model
///
/// Acquisition state is per-target (stored in RenderTargetEntry), not global.
/// No allocations occur during acquire/release/iteration.
pub struct RenderTargetRegistry {
    /// Map from SurfaceId to RenderTargetEntry (unified: surface-backed or headless).
    targets: HashMap<SurfaceId, RenderTargetEntry>,
}

impl RenderTargetRegistry {
    /// Create a new empty registry.
    pub fn new() -> Self {
        Self {
            targets: HashMap::new(),
        }
    }

    /// INV-RT-002: Register a new render target.
    ///
    /// # Arguments
    ///
    /// * `id` - The compositor SurfaceId for this target
    /// * `bundle` - The GPU surface bundle
    /// * `configured_size` - Initial physical size (width, height) from Surface::configure()
    /// * `scale_factor` - INV-SCALE-001: Per-target scale factor (DPI scaling)
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Target registered successfully
    /// * `Err(DuplicateRegistration)` - SurfaceId already registered
    pub fn register(
        &mut self,
        id: SurfaceId,
        bundle: SurfaceBundle,
        configured_size: (u32, u32),
        scale_factor: f32,
    ) -> Result<(), RenderTargetError> {
        if self.targets.contains_key(&id) {
            return Err(RenderTargetError::DuplicateRegistration { surface_id: id });
        }
        self.targets.insert(
            id,
            RenderTargetEntry::new(bundle, configured_size, scale_factor),
        );
        Ok(())
    }

    /// Register a headless render target (no wgpu::Surface).
    ///
    /// Provides `configured_size_px` and `scale_factor` for compile-phase queries
    /// without requiring a real window or surface. Used for headless/test scenarios
    /// where compile_multi must run but render_multi is never called.
    pub fn register_headless(
        &mut self,
        id: SurfaceId,
        configured_size: (u32, u32),
        scale_factor: f32,
    ) -> Result<(), RenderTargetError> {
        if self.targets.contains_key(&id) {
            return Err(RenderTargetError::DuplicateRegistration { surface_id: id });
        }
        self.targets.insert(
            id,
            RenderTargetEntry::new_headless(configured_size, scale_factor),
        );
        Ok(())
    }

    /// INV-RT-003/009: Unregister a render target.
    ///
    /// # Returns
    ///
    /// * `Ok(Some(bundle))` - Surface target removed, bundle returned
    /// * `Ok(None)` - Headless target removed (no bundle to return)
    /// * `Err(NotRegistered)` - SurfaceId not found
    /// * `Err(UnregisterWhileAcquired)` - Target is currently acquired
    pub fn unregister(
        &mut self,
        id: SurfaceId,
    ) -> Result<Option<SurfaceBundle>, RenderTargetError> {
        // Check registration and acquired state in one lookup
        let is_acquired = match self.targets.get(&id) {
            None => return Err(RenderTargetError::NotRegistered { surface_id: id }),
            Some(entry) => entry.acquired.get(),
        };

        // INV-RT-009: Error if acquired
        if is_acquired {
            return Err(RenderTargetError::UnregisterWhileAcquired { surface_id: id });
        }

        // Remove atomically (INV-RT-003) - entry exists, not acquired
        let entry = self.targets.remove(&id).expect("entry exists");
        match entry.kind {
            RenderTargetKind::Surface(bundle) => Ok(Some(bundle)),
            RenderTargetKind::Headless => Ok(None),
        }
    }

    /// INV-RT-004: Configure a registered render target.
    ///
    /// The registry applies configurations verbatim. The renderer is the
    /// single authority for swapchain policy.
    ///
    /// INV-SCISSOR-001: Updates the stored configured_size_px to match the
    /// config dimensions. This is the ONLY place where configured_size_px
    /// is updated after registration.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Target configured successfully
    /// * `Err(NotRegistered)` - SurfaceId not found
    /// * `Err(ResizeWhileAcquired)` - Target is currently acquired
    pub fn configure(
        &self,
        id: SurfaceId,
        device: &wgpu::Device,
        config: &wgpu::SurfaceConfiguration,
    ) -> Result<(), RenderTargetError> {
        let entry = self
            .targets
            .get(&id)
            .ok_or(RenderTargetError::NotRegistered { surface_id: id })?;

        // INV-RT-004: Check if acquired
        if entry.acquired.get() {
            return Err(RenderTargetError::ResizeWhileAcquired { surface_id: id });
        }

        // INV-SCISSOR-001: Update authoritative configured size BEFORE calling configure.
        // This ensures the stored size always matches what Surface::configure() receives.
        entry.configured_size_px.set((config.width, config.height));
        entry.observed_size_px.set((config.width, config.height));

        if let RenderTargetKind::Surface(bundle) = &entry.kind {
            bundle.configure(device, config);
        }
        Ok(())
    }

    pub(crate) fn observe_size_px(
        &self,
        id: SurfaceId,
        size: (u32, u32),
    ) -> Result<(), RenderTargetError> {
        let entry = self
            .targets
            .get(&id)
            .ok_or(RenderTargetError::NotRegistered { surface_id: id })?;
        if entry.acquired.get() {
            return Err(RenderTargetError::ResizeWhileAcquired { surface_id: id });
        }
        entry.observed_size_px.set(size);
        Ok(())
    }

    pub(crate) fn has_active_extent(&self, id: SurfaceId) -> bool {
        self.targets.get(&id).is_some_and(|entry| {
            let size = entry.observed_size_px.get();
            size.0 > 0 && size.1 > 0 && size == entry.configured_size_px.get()
        })
    }

    /// INV-RT-005/006: Acquire a target for rendering.
    ///
    /// Returns an RAII guard that releases the target when dropped.
    ///
    /// # Returns
    ///
    /// * `Ok(guard)` - Target acquired, guard provides access
    /// * `Err(NotRegistered)` - SurfaceId not found
    /// * `Err(AlreadyAcquired)` - Target is already acquired (double-acquire)
    pub fn acquire(&self, id: SurfaceId) -> Result<AcquiredTargetGuard<'_>, RenderTargetError> {
        let entry = self
            .targets
            .get(&id)
            .ok_or(RenderTargetError::NotRegistered { surface_id: id })?;

        // Headless targets have no surface — acquisition is structurally invalid.
        if matches!(entry.kind, RenderTargetKind::Headless) {
            return Err(RenderTargetError::HeadlessCannotAcquire { surface_id: id });
        }

        // INV-RT-006: Check for double-acquire
        if entry.acquired.get() {
            return Err(RenderTargetError::AlreadyAcquired { surface_id: id });
        }
        if !self.has_active_extent(id) {
            return Err(RenderTargetError::InactiveTarget { surface_id: id });
        }

        // Mark as acquired
        entry.acquired.set(true);

        Ok(AcquiredTargetGuard {
            registry: self,
            surface_id: id,
        })
    }

    /// INV-RT-007: Release a target (internal, called by guard Drop).
    fn release(&self, id: SurfaceId) {
        let entry = self
            .targets
            .get(&id)
            .expect("INV-RT-007: release on missing target");
        entry.acquired.set(false);
    }

    /// INV-SCISSOR-001: Get the authoritative configured size for a surface.
    ///
    /// Returns the exact (width, height) in physical pixels that was last
    /// passed to `wgpu::Surface::configure()` for this target. This is the
    /// ONLY source of truth for scissor clamp bounds.
    ///
    /// # Returns
    ///
    /// * `Some((width, height))` - The configured physical size
    /// * `None` - Surface not registered
    pub fn configured_size_px(&self, id: SurfaceId) -> Option<(u32, u32)> {
        self.targets.get(&id).map(|e| e.configured_size_px.get())
    }

    /// INV-SCALE-001: Get the per-target scale factor for a surface.
    ///
    /// # Returns
    ///
    /// * `Some(scale_factor)` - The per-target scale factor
    /// * `None` - Surface not registered
    pub fn scale_factor(&self, id: SurfaceId) -> Option<f32> {
        self.targets.get(&id).map(|e| e.scale_factor.get())
    }

    /// INV-SCALE-001: Set the per-target scale factor for a surface.
    ///
    /// # Returns
    ///
    /// * `Ok(())` - Scale factor updated successfully
    /// * `Err(NotRegistered)` - SurfaceId not found
    /// * `Err(ResizeWhileAcquired)` - Target is currently acquired (INV-RT-004)
    pub fn set_scale_factor(&self, id: SurfaceId, scale: f32) -> Result<(), RenderTargetError> {
        let entry = self
            .targets
            .get(&id)
            .ok_or(RenderTargetError::NotRegistered { surface_id: id })?;

        // INV-RT-004: Check if acquired (same guard as resize)
        if entry.acquired.get() {
            return Err(RenderTargetError::ResizeWhileAcquired { surface_id: id });
        }

        entry.scale_factor.set(scale);
        Ok(())
    }

    /// Check if a target is currently acquired (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should not query acquisition state; the ownership
    /// model enforces correctness via RAII guards.
    #[cfg(debug_assertions)]
    pub fn is_acquired(&self, id: SurfaceId) -> bool {
        self.targets
            .get(&id)
            .map(|e| e.acquired.get())
            .unwrap_or(false)
    }

    /// Get a reference to a target's bundle (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should use `acquire()` which returns an RAII guard
    /// and properly enforces the ownership model.
    #[cfg(debug_assertions)]
    pub fn get(&self, id: SurfaceId) -> Option<&SurfaceBundle> {
        self.targets.get(&id).and_then(|e| match &e.kind {
            RenderTargetKind::Surface(bundle) => Some(bundle),
            RenderTargetKind::Headless => None,
        })
    }

    /// Get a mutable reference to a target's bundle (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should use `acquire()` which returns an RAII guard
    /// and properly enforces the ownership model.
    #[cfg(debug_assertions)]
    pub fn get_mut(&mut self, id: SurfaceId) -> Option<&mut SurfaceBundle> {
        self.targets.get_mut(&id).and_then(|e| match &mut e.kind {
            RenderTargetKind::Surface(bundle) => Some(bundle),
            RenderTargetKind::Headless => None,
        })
    }

    /// Check if a target is registered (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should not query registration state; use `acquire()`
    /// which returns an error if the target is not registered.
    #[cfg(debug_assertions)]
    pub fn contains(&self, id: SurfaceId) -> bool {
        self.targets.contains_key(&id)
    }

    /// Get the number of registered targets (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should not reason about registry cardinality.
    #[cfg(debug_assertions)]
    pub fn len(&self) -> usize {
        self.targets.len()
    }

    /// Check if the registry is empty (debug-only).
    ///
    /// This is a diagnostic helper for debug assertions only.
    /// Production code should not reason about registry cardinality.
    #[cfg(debug_assertions)]
    pub fn is_empty(&self) -> bool {
        self.targets.is_empty()
    }

    /// INV-RT-010: Iterate over all registered targets (excludes acquired).
    ///
    /// No allocation during iteration.
    pub fn iter(&self) -> impl Iterator<Item = (&SurfaceId, &SurfaceBundle)> {
        self.targets
            .iter()
            .filter(|(_, e)| !e.acquired.get())
            .filter_map(|(id, e)| match &e.kind {
                RenderTargetKind::Surface(bundle) => Some((id, bundle)),
                RenderTargetKind::Headless => None,
            })
    }

    /// Iterate over ALL registered targets (includes acquired).
    ///
    /// Use this when you need to see all targets regardless of acquisition state.
    pub fn iter_all(&self) -> impl Iterator<Item = (&SurfaceId, &SurfaceBundle)> {
        self.targets.iter().filter_map(|(id, e)| match &e.kind {
            RenderTargetKind::Surface(bundle) => Some((id, bundle)),
            RenderTargetKind::Headless => None,
        })
    }

    /// Iterate mutably over all registered targets (excludes acquired).
    ///
    /// No allocation during iteration.
    pub fn iter_mut(&mut self) -> impl Iterator<Item = (&SurfaceId, &mut SurfaceBundle)> {
        self.targets
            .iter_mut()
            .filter(|(_, e)| !e.acquired.get())
            .filter_map(|(id, e)| match &mut e.kind {
                RenderTargetKind::Surface(bundle) => Some((id, bundle)),
                RenderTargetKind::Headless => None,
            })
    }
}

// ============================================================================
// RAII Guard (INV-RT-005, INV-RT-007)
// ============================================================================

/// INV-RT-005/007: RAII guard for an acquired render target.
///
/// The guard provides access to the acquired target and automatically
/// releases it when dropped. This ensures targets cannot be leaked
/// in an acquired state.
///
/// # Invariants
///
/// - INV-RT-007: Release happens ONLY via Drop (no manual release API)
/// - The guard holds a reference to the registry, preventing concurrent modification
pub struct AcquiredTargetGuard<'a> {
    registry: &'a RenderTargetRegistry,
    surface_id: SurfaceId,
}

impl<'a> AcquiredTargetGuard<'a> {
    /// Get the SurfaceId of the acquired target.
    pub fn surface_id(&self) -> SurfaceId {
        self.surface_id
    }

    /// Get a reference to the acquired target's bundle.
    pub fn bundle(&self) -> &SurfaceBundle {
        // SAFETY: We verified the target exists in acquire(), and we hold a reference
        // to the registry preventing removal.
        let entry = self
            .registry
            .targets
            .get(&self.surface_id)
            .expect("INV-RT-005: acquired target must exist");
        match &entry.kind {
            RenderTargetKind::Surface(bundle) => bundle,
            RenderTargetKind::Headless => unreachable!("headless targets cannot be acquired"),
        }
    }

    /// Get the current texture for rendering.
    ///
    /// INV-RT-008: This is the ONLY way to get a texture for rendering.
    pub fn get_current_texture(&self) -> Result<wgpu::SurfaceTexture, RendererSurfaceError> {
        self.bundle().get_current_texture()
    }
}

impl Drop for AcquiredTargetGuard<'_> {
    fn drop(&mut self) {
        // INV-RT-007: Release via Drop only
        self.registry.release(self.surface_id);
    }
}

impl Default for RenderTargetRegistry {
    fn default() -> Self {
        Self::new()
    }
}

/// SPEC-HAL-081B: Neutral target descriptor for compiled plans.
///
/// Contains only render-relevant metadata for a target. No acquisition state,
/// no ownership, no GPU resources. Texture acquisition happens exclusively
/// in `render_multi()` via `RenderTargetRegistry::acquire()`.
///
/// This is a purely declarative description of what to render, not how or when.
#[derive(Debug, Clone)]
pub struct TargetDescriptor {
    /// The SurfaceId identifying this target.
    pub surface_id: SurfaceId,
    /// Physical size (width, height) in physical pixels.
    pub physical_size: (u32, u32),
    /// Logical size (width, height) in logical pixels.
    pub logical_size: (f32, f32),
    /// Scale factor for this target.
    pub scale_factor: f32,
    /// Offset into globals buffer for this target (bytes).
    /// Used for dynamic uniform binding during render.
    pub globals_offset: u32,
    /// Offset into path clips buffer for this target (bytes).
    /// SPEC-HAL-081B: Per-target path clips for multi-surface rendering.
    pub path_clips_offset: u32,
}

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Basic Registry Tests (debug-only, use len/is_empty)
    // ========================================================================

    #[test]
    #[cfg(debug_assertions)]
    fn test_registry_empty_initially() {
        let registry = RenderTargetRegistry::new();
        assert!(registry.is_empty());
        assert_eq!(registry.len(), 0);
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_registry_default() {
        let registry = RenderTargetRegistry::default();
        assert!(registry.is_empty());
    }

    // ========================================================================
    // Error Type Tests
    // ========================================================================

    #[test]
    fn test_error_display() {
        use crate::compositor::SurfaceId;

        // Create a test SurfaceId using the test-only helper
        let id = SurfaceId::test_id(42);

        let err = RenderTargetError::DuplicateRegistration { surface_id: id };
        assert!(err.to_string().contains("Duplicate registration"));
        assert!(err.to_string().contains("42"));

        let err = RenderTargetError::NotRegistered { surface_id: id };
        assert!(err.to_string().contains("not registered"));

        let err = RenderTargetError::AlreadyAcquired { surface_id: id };
        assert!(err.to_string().contains("already acquired"));

        let err = RenderTargetError::UnregisterWhileAcquired { surface_id: id };
        assert!(err.to_string().contains("Cannot unregister"));

        let err = RenderTargetError::ResizeWhileAcquired { surface_id: id };
        assert!(err.to_string().contains("Cannot resize"));

        let err = RenderTargetError::HeadlessCannotAcquire { surface_id: id };
        assert!(err.to_string().contains("Cannot acquire headless"));
    }

    #[test]
    fn test_error_is_error_trait() {
        fn assert_error<T: std::error::Error>() {}
        assert_error::<RenderTargetError>();
    }

    // ========================================================================
    // Lifecycle Tests (require GPU context - documented behavior only)
    // ========================================================================
    //
    // The following invariants are tested at compile-time via the type system:
    //
    // - INV-RT-001: Registry owns SurfaceBundles (no Clone on SurfaceBundle)
    // - INV-RT-007: Release via Drop only (no public release method)
    //
    // The following invariants require GPU context for full integration testing
    // and are verified in wgpu_renderer integration tests:
    //
    // - INV-RT-002: Duplicate registration returns error
    // - INV-RT-003: Unregister removes atomically
    // - INV-RT-004: Resize requires non-acquired state
    // - INV-RT-005: Acquire returns RAII guard
    // - INV-RT-006: Double-acquire returns error
    // - INV-RT-009: Unregister while acquired returns error
    // - INV-RT-010: Iteration excludes acquired targets

    #[test]
    fn test_acquire_not_registered() {
        use crate::compositor::SurfaceId;

        let registry = RenderTargetRegistry::new();
        let id = SurfaceId::test_id(1);

        let result = registry.acquire(id);
        assert!(matches!(
            result,
            Err(RenderTargetError::NotRegistered { .. })
        ));
    }

    #[test]
    fn test_unregister_not_registered() {
        use crate::compositor::SurfaceId;

        let mut registry = RenderTargetRegistry::new();
        let id = SurfaceId::test_id(1);

        let result = registry.unregister(id);
        assert!(matches!(
            result,
            Err(RenderTargetError::NotRegistered { .. })
        ));
    }

    #[test]
    #[cfg(debug_assertions)]
    fn test_is_acquired_false_when_not_registered() {
        use crate::compositor::SurfaceId;

        let registry = RenderTargetRegistry::new();
        let id = SurfaceId::test_id(1);

        assert!(!registry.is_acquired(id));
    }

    #[test]
    fn test_headless_cannot_acquire() {
        use crate::compositor::SurfaceId;

        let mut registry = RenderTargetRegistry::new();
        let id = SurfaceId::test_id(99);
        registry.register_headless(id, (100, 100), 1.0).unwrap();

        let result = registry.acquire(id);
        assert!(matches!(
            result,
            Err(RenderTargetError::HeadlessCannotAcquire { .. })
        ));
    }

    #[test]
    fn test_headless_unregister_returns_none() {
        use crate::compositor::SurfaceId;

        let mut registry = RenderTargetRegistry::new();
        let id = SurfaceId::test_id(99);
        registry.register_headless(id, (100, 100), 1.0).unwrap();

        let result = registry.unregister(id);
        assert!(matches!(result, Ok(None)));
    }

    // Note: Full integration tests require GPU context and are in wgpu_renderer tests.
    // Tests requiring SurfaceBundle creation must use a real GPU context.
    //
    // Lifecycle invariants that require GPU context:
    // - register() + unregister() roundtrip
    // - register() duplicate detection (INV-RT-002)
    // - acquire() + guard drop release (INV-RT-005, INV-RT-007)
    // - acquire() double-acquire detection (INV-RT-006)
    // - unregister() while acquired (INV-RT-009)
    // - resize() while acquired (INV-RT-004)
    // - iter() excludes acquired (INV-RT-010)
}
