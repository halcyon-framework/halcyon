//! Compositor Core Surfaces
//!
//! This module implements the compositor subsystem with two contract modes:
//!
//! # MWA Closure Declaration
//!
//! **STATUS: CLOSED** (as of 2026-01-21)
//!
//! The Multi-Window Architecture (MWA) layer is procedurally closed.
//! This includes: Compositor, Multi-Surface, Overlay, and Interaction Domain isolation.
//!
//! **Any modification to this module requires explicit SPEC amendment.**
//!
//! See `docs/MWA-AUDIT-REPORT.md` for the binding audit and invariant list.
//!
//! # Contract Selection
//!
//! - **BaseContract (SPEC-HAL-081A)**: Single base surface only. Use `snapshot()`.
//! - **MultiSurfaceContract (SPEC-HAL-081B)**: Multiple base surfaces permitted.
//!   Use `snapshot_multi()` for multi-window rendering.
//!
//! # Authority Boundaries (SPEC-HAL-081A, SPEC-HAL-150)
//!
//! The compositor MUST NOT:
//! - Perform hit-testing at any level (INV-COMP-N11)
//! - Decide capture, dismissal, focus, or modality (INV-COMP-N12)
//! - Mutate widget geometry (INV-COMP-N01)
//! - Participate in layout computation (INV-COMP-N02)
//! - Resolve styles, themes, or visual properties (INV-COMP-N03)
//! - Own widgets (INV-COMP-N04)
//! - Own SceneGraph nodes (INV-COMP-N05)
//! - Generate visuals not backed by surfaces/layers (INV-COMP-N06)
//!
//! The compositor MAY ONLY:
//! - Order surfaces by z-index (INV-COMP-P01)
//! - Composite layers into final output (INV-COMP-P02)
//! - Expose ordered SurfaceStack descriptors (INV-COMP-P03)
//! - Hand off to RenderGraph/swapchain (INV-COMP-P04)
//! - Track surface lifetime (INV-COMP-P05)
//! - Apply surface-level occlusion (INV-COMP-P06)
//! - Expose bounds/clips/transforms (INV-COMP-P07)
//!
//! # Module Structure
//!
//! - `surface.rs`: Frozen core (SurfaceId, SurfaceDescriptor, etc.)
//! - `surface_role.rs`: Future SPEC additive contract (SurfaceRole, provenance, markers)
//! - `stack.rs`: Surface stack management

mod abi_guard;
mod authority_firewall_guard;
mod contract;
mod stack;
mod surface;
mod surface_role;

// ============================================================================
// Current Contract Exports (Frozen Core)
// ============================================================================

pub use contract::{
    AssertBaseContract, AssertMultiSurfaceContract, BaseContract, Contract, Contracted,
    MultiSurfaceContract, SceneKey, assert_base_contract_surface_count,
};
pub use stack::{
    FrameProjectionError, NavigationFrameProjection, NavigationProjectionContext,
    ProjectedNavigationContext, ProjectedSurfaceView, SurfaceStackSnapshot,
};
pub use surface::{SurfaceDescriptor, SurfaceId, SurfaceVisibility};

// SPEC-HAL-081B: Surface role type exports (type-level enforcement)
pub use surface_role::{BaseSurfaceTag, OverlaySurfaceTag, SurfaceRole};

// SPEC-HAL-081A Lockdown: SurfaceStack is pub(crate) only.
// External code must go through Compositor → snapshot() path.
// The only way to get renderable surface data is via SurfaceStackSnapshot.
pub(crate) use stack::SurfaceStack;

// ============================================================================
// Future SPEC Exports (Additive Contract - SPEC-HAL-081C)
// ============================================================================
//
// These types are scaffolding for future overlay integration.
// They are NOT embedded in current contract structs. They compile but are unused
// until future SPEC implementation begins.

pub use surface_role::{SurfaceProvenance, SurfaceRoleKind};

use crate::core::math::RectF;
use std::collections::HashSet;
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_COMPOSITOR_IDENTITY: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CompositorIdentity(u64);

fn allocate_compositor_identity() -> CompositorIdentity {
    CompositorIdentity(
        NEXT_COMPOSITOR_IDENTITY
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("compositor identity space exhausted"),
    )
}

/// One prevalidated mechanical surface mutation batch.
///
/// The batch contains no navigation or lifecycle meaning. Its operation order is
/// supplied entirely by the semantic owner above the compositor.
#[derive(Debug, Default)]
pub struct SurfaceMutationBatch {
    operations: Vec<SurfaceMutation>,
}

#[derive(Debug)]
enum SurfaceMutation {
    SetZIndex(SurfaceId, i32),
    SetBounds(SurfaceId, RectF),
    SetVisibility(SurfaceId, SurfaceVisibility),
    Destroy(SurfaceId),
}

impl SurfaceMutationBatch {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_z_index(&mut self, surface: SurfaceId, z_index: i32) {
        self.operations
            .push(SurfaceMutation::SetZIndex(surface, z_index));
    }

    pub fn set_bounds(&mut self, surface: SurfaceId, bounds: RectF) {
        self.operations
            .push(SurfaceMutation::SetBounds(surface, bounds));
    }

    pub fn set_visibility(&mut self, surface: SurfaceId, visibility: SurfaceVisibility) {
        self.operations
            .push(SurfaceMutation::SetVisibility(surface, visibility));
    }

    pub fn destroy(&mut self, surface: SurfaceId) {
        self.operations.push(SurfaceMutation::Destroy(surface));
    }

    pub fn is_empty(&self) -> bool {
        self.operations.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SurfaceBatchPreparationError {
    MissingSurface(SurfaceId),
    OperationAfterDestroy(SurfaceId),
    ImplicitOverlayDestruction {
        parent: SurfaceId,
        overlay: SurfaceId,
    },
}

/// Compositor state for a single window.
///
/// SPEC-HAL-081A: Core surfaces only.
/// Manages an ordered stack of surfaces for composition.
///
/// # Invariants
///
/// - Does NOT own widgets or SceneGraph nodes (INV-COMP-N04, INV-COMP-N05)
/// - Does NOT perform hit-testing (INV-COMP-N11, STOP-08)
/// - Does NOT route input or decide focus (INV-COMP-N12)
/// - Surfaces are identified by opaque SurfaceId handles
#[derive(Debug)]
pub struct Compositor {
    identity: CompositorIdentity,
    /// Ordered surface stack (lowest z-index first)
    stack: SurfaceStack,
    /// Viewport bounds for the compositor's window
    viewport: RectF,
}

impl Compositor {
    /// Create a new compositor for a single window.
    ///
    /// # Arguments
    ///
    /// * `viewport` - The window's viewport bounds in logical coordinates
    pub fn new(viewport: RectF) -> Self {
        Self {
            identity: allocate_compositor_identity(),
            stack: SurfaceStack::new(),
            viewport,
        }
    }

    /// Stable process-local identity used by semantic owners to reject a
    /// mechanical store other than the one they were prepared against.
    pub const fn identity(&self) -> CompositorIdentity {
        self.identity
    }

    /// Update the viewport bounds.
    ///
    /// Called when the window is resized.
    pub fn set_viewport(&mut self, viewport: RectF) {
        self.viewport = viewport;
    }

    /// Get the current viewport bounds.
    pub fn viewport(&self) -> RectF {
        self.viewport
    }

    /// Create a new surface and add it to the stack.
    ///
    /// Returns the SurfaceId for the new surface.
    ///
    /// # Arguments
    ///
    /// * `z_index` - Z-order for the surface (higher = on top)
    /// * `bounds` - Surface bounds in logical coordinates
    /// * `scene_key` - Scene key identifying content (use `SceneKey::BASE` for base surface)
    pub fn create_surface(
        &mut self,
        z_index: i32,
        bounds: RectF,
        scene_key: SceneKey,
    ) -> SurfaceId {
        self.stack.create_surface(z_index, bounds, scene_key.get())
    }

    /// Allocate a surface that is hidden before the call returns.
    ///
    /// This is the mechanical staging seam used by prepared semantic owners.
    pub fn create_hidden_surface(
        &mut self,
        z_index: i32,
        bounds: RectF,
        scene_key: SceneKey,
    ) -> SurfaceId {
        let surface = self.stack.create_surface(z_index, bounds, scene_key.get());
        self.stack
            .set_visibility(surface, SurfaceVisibility::Hidden);
        surface
    }

    /// Remove a surface from the stack.
    ///
    /// # Returns
    ///
    /// `true` if the surface was found and removed, `false` otherwise.
    pub fn destroy_surface(&mut self, id: SurfaceId) -> bool {
        self.stack.destroy_surface(id)
    }

    /// Update a surface's z-index.
    ///
    /// The stack will be re-sorted to maintain z-order.
    pub fn set_surface_z_index(&mut self, id: SurfaceId, z_index: i32) {
        self.stack.set_z_index(id, z_index);
    }

    /// Update a surface's bounds.
    pub fn set_surface_bounds(&mut self, id: SurfaceId, bounds: RectF) {
        self.stack.set_bounds(id, bounds);
    }

    /// Update a surface's clip rectangle.
    pub fn set_surface_clip(&mut self, id: SurfaceId, clip: Option<RectF>) {
        self.stack.set_clip(id, clip);
    }

    /// Update a surface's visibility.
    pub fn set_surface_visibility(&mut self, id: SurfaceId, visibility: SurfaceVisibility) {
        self.stack.set_visibility(id, visibility);
    }

    /// Update a surface's opacity.
    ///
    /// # Arguments
    ///
    /// * `opacity` - Opacity value in range [0.0, 1.0]
    pub fn set_surface_opacity(&mut self, id: SurfaceId, opacity: f32) {
        self.stack.set_opacity(id, opacity.clamp(0.0, 1.0));
    }

    /// Associate a scene key with a surface.
    ///
    /// This key is used by the renderer to identify which scene content
    /// belongs to which surface. The compositor does NOT own the scene;
    /// it only tracks the association via an opaque key.
    ///
    /// # INV-COMP-N05 Compliance
    ///
    /// The scene_key is a typed `SceneKey` (non-zero u64), not a reference to
    /// SceneGraph nodes. Ownership remains with the scene producer.
    pub fn set_surface_scene_key(&mut self, id: SurfaceId, scene_key: SceneKey) {
        self.stack.set_scene_key(id, scene_key);
    }

    /// Prepare an exact mechanical batch under an exclusive compositor borrow.
    ///
    /// Validation is fallible. Once returned, the prepared value reserves the
    /// compositor mutation/snapshot lane until commit, abort, or drop, so commit
    /// performs no existence checks and cannot partially fail.
    pub fn prepare_surface_batch(
        &mut self,
        batch: SurfaceMutationBatch,
    ) -> Result<PreparedSurfaceBatch<'_>, SurfaceBatchPreparationError> {
        self.validate_surface_batch(&batch)?;

        Ok(PreparedSurfaceBatch {
            compositor: self,
            operations: batch.operations,
        })
    }

    /// Validate a mechanical batch without reserving or mutating the compositor.
    pub fn validate_surface_batch(
        &self,
        batch: &SurfaceMutationBatch,
    ) -> Result<(), SurfaceBatchPreparationError> {
        let mut destroyed = HashSet::new();

        for operation in &batch.operations {
            let surface = match operation {
                SurfaceMutation::SetZIndex(surface, _)
                | SurfaceMutation::SetBounds(surface, _)
                | SurfaceMutation::SetVisibility(surface, _)
                | SurfaceMutation::Destroy(surface) => *surface,
            };

            if destroyed.contains(&surface) {
                return Err(SurfaceBatchPreparationError::OperationAfterDestroy(surface));
            }
            if !self.stack.contains(surface) {
                return Err(SurfaceBatchPreparationError::MissingSurface(surface));
            }

            if matches!(operation, SurfaceMutation::Destroy(_)) {
                for overlay in self.stack.overlays_of(surface) {
                    if !destroyed.contains(&overlay) {
                        return Err(SurfaceBatchPreparationError::ImplicitOverlayDestruction {
                            parent: surface,
                            overlay,
                        });
                    }
                }
                destroyed.insert(surface);
            }
        }

        Ok(())
    }

    pub fn apply_frame_projection(
        &self,
        projection: &NavigationFrameProjection,
    ) -> Result<ProjectedSurfaceView, FrameProjectionError> {
        if projection.compositor_identity() != self.identity {
            return Err(FrameProjectionError::WrongCompositor {
                expected: projection.compositor_identity(),
                actual: self.identity,
            });
        }
        self.stack.apply_frame_projection(projection)
    }

    /// Get a snapshot of the surface stack for rendering (BaseContract).
    ///
    /// Returns an immutable, ordered list of surface descriptors
    /// suitable for consumption by DrawPlanBuilder.
    ///
    /// # INV-COMP-P03 Compliance
    ///
    /// This is the primary interface for downstream consumers
    /// (EventFlow, RenderGraph). The compositor exposes descriptors
    /// but does not execute routing or rendering.
    ///
    /// # Contract Enforcement (SPEC-HAL-081A)
    ///
    /// Enforces single base surface constraint. Panics in debug builds
    /// if more than one base surface exists.
    /// For multi-window/multi-surface, use `snapshot_multi()`.
    pub fn snapshot(&self) -> SurfaceStackSnapshot {
        self.stack.snapshot()
    }

    /// Get a snapshot of the surface stack for multi-surface rendering (SPEC-HAL-081B).
    ///
    /// Returns `SurfaceStackSnapshot<MultiSurfaceContract>`, which permits
    /// multiple base surfaces. Overlay surfaces are still filtered out.
    ///
    /// # INV-COMP-P03 Compliance
    ///
    /// Same as `snapshot()` - compositor exposes descriptors without
    /// executing routing or rendering.
    pub fn snapshot_multi(&self) -> SurfaceStackSnapshot<MultiSurfaceContract> {
        self.stack.snapshot_multi()
    }

    /// Test-only snapshot that bypasses contract enforcement.
    ///
    /// Allows testing multi-surface ordering/visibility logic without
    /// triggering the single-surface contract assertion.
    #[cfg(test)]
    pub fn snapshot_unchecked(&self) -> SurfaceStackSnapshot {
        self.stack.snapshot_unchecked()
    }

    /// Get the number of surfaces in the stack.
    pub fn surface_count(&self) -> usize {
        self.stack.len()
    }

    /// Check if a surface exists.
    pub fn has_surface(&self, id: SurfaceId) -> bool {
        self.stack.contains(id)
    }

    /// Read a surface's mechanical visibility without assigning lifecycle meaning.
    pub fn surface_visibility(&self, id: SurfaceId) -> Option<SurfaceVisibility> {
        self.stack.get_state(id).map(|surface| surface.visibility)
    }

    /// Get a surface descriptor by ID.
    ///
    /// Returns `None` if the surface does not exist OR if it is an overlay surface.
    /// Note: This creates a temporary descriptor. For batch access, use `snapshot()`.
    ///
    /// # Overlay Isolation (SPEC-HAL-081C)
    ///
    /// This method returns ONLY base surface descriptors. Overlay surfaces
    /// are not accessible through this path - use `overlay_parent()` and
    /// `overlays_of()` for explicit overlay queries.
    pub fn get_surface(&self, id: SurfaceId) -> Option<SurfaceDescriptor> {
        // SPEC-HAL-081C: Filter to base surfaces only (matches snapshot() contract).
        // Overlay queries must use explicit overlay API (overlay_parent, overlays_of).
        self.stack
            .get_state(id)
            .filter(|s| s.is_base())
            .map(|s| s.to_descriptor())
    }

    // ========================================================================
    // SPEC-HAL-081C: Overlay Surface API
    // ========================================================================

    /// Create an overlay surface attached to a parent base surface.
    ///
    /// SPEC-HAL-081C: Overlay surfaces are compositor-internal and do not
    /// appear in renderer-facing snapshots.
    ///
    /// # Arguments
    ///
    /// * `parent` - ID of the parent base surface
    /// * `z_index` - Z-order for the overlay (MUST be > parent's z_index)
    /// * `bounds` - Surface bounds in logical coordinates
    /// * `scene_key` - Scene key identifying content
    ///
    /// # Returns
    ///
    /// `Some(SurfaceId)` on success, `None` if:
    /// - Parent does not exist
    /// - Parent is an overlay (only base surfaces can be parents)
    /// - z_index <= parent's z_index
    ///
    /// # Authority Boundary
    ///
    /// This method provides surface lifecycle only. No input/capture/focus/modal
    /// semantics (SPEC-HAL-150 boundary).
    pub fn create_overlay_surface(
        &mut self,
        parent: SurfaceId,
        z_index: i32,
        bounds: RectF,
        scene_key: SceneKey,
    ) -> Option<SurfaceId> {
        self.stack
            .create_overlay_surface(z_index, bounds, scene_key.get(), parent)
    }

    /// Get the parent surface of an overlay.
    ///
    /// SPEC-HAL-081C: Read-only query for overlay provenance.
    ///
    /// # Returns
    ///
    /// - `Some(parent_id)` if `id` is an overlay surface
    /// - `None` if `id` is a base surface or does not exist
    ///
    /// # Explicit Overlay Visibility
    ///
    /// This method (and `overlays_of`) is the ONLY public path through which
    /// overlay surfaces are visible. All other read APIs (`snapshot`, `get_surface`)
    /// filter overlays out. Upstream routing code should use these methods for
    /// overlay-aware decisions.
    pub fn overlay_parent(&self, id: SurfaceId) -> Option<SurfaceId> {
        self.stack.overlay_parent(id)
    }

    /// Get all overlay children of a base surface.
    ///
    /// SPEC-HAL-081C: Read-only query for overlay enumeration.
    ///
    /// # Returns
    ///
    /// Overlay IDs in ascending z-index order.
    /// Empty if `parent` does not exist or is not a base surface.
    ///
    /// # Explicit Overlay Visibility
    ///
    /// This method (and `overlay_parent`) is the ONLY public path through which
    /// overlay surfaces are visible. All other read APIs (`snapshot`, `get_surface`)
    /// filter overlays out. Upstream routing code should use these methods for
    /// overlay-aware decisions.
    pub fn overlays_of(&self, parent: SurfaceId) -> Vec<SurfaceId> {
        self.stack.overlays_of(parent)
    }
}

/// Exclusive reservation for one prevalidated mechanical surface batch.
#[derive(Debug)]
pub struct PreparedSurfaceBatch<'a> {
    compositor: &'a mut Compositor,
    operations: Vec<SurfaceMutation>,
}

impl<'a> PreparedSurfaceBatch<'a> {
    /// Diagnostic existence read while the batch reservation is live.
    pub fn has_surface(&self, surface: SurfaceId) -> bool {
        self.compositor.has_surface(surface)
    }

    /// Diagnostic visibility read while the batch reservation is live.
    pub fn surface_visibility(&self, surface: SurfaceId) -> Option<SurfaceVisibility> {
        self.compositor.surface_visibility(surface)
    }

    /// Execute the prevalidated operations without a fallible branch.
    pub fn commit(self) -> &'a mut Compositor {
        let Self {
            compositor,
            operations,
        } = self;

        for operation in operations {
            match operation {
                SurfaceMutation::SetZIndex(surface, z_index) => {
                    compositor.stack.set_z_index(surface, z_index);
                }
                SurfaceMutation::SetBounds(surface, bounds) => {
                    compositor.stack.set_bounds(surface, bounds);
                }
                SurfaceMutation::SetVisibility(surface, visibility) => {
                    compositor.stack.set_visibility(surface, visibility);
                }
                SurfaceMutation::Destroy(surface) => {
                    let removed = compositor.stack.destroy_surface(surface);
                    debug_assert!(removed, "prepared surface disappeared before batch commit");
                }
            }
        }

        compositor
    }

    /// Release the exclusive reservation without executing any operation.
    pub fn abort(self) -> &'a mut Compositor {
        self.compositor
    }
}

// ============================================================================
// Authority Firewall (STOP-08 enforcement)
// ============================================================================

/// Compile-time enforcement: compositor module MUST NOT import these modules.
///
/// This is a documentation marker. Actual enforcement is via code review
/// and the module structure itself (no `use` statements for forbidden modules).
///
/// Forbidden imports (SPEC-HAL-081A STOP-08):
/// - crate::core::widget (widget ownership)
/// - crate::core::event_router (input routing)
/// - crate::core::layout (layout participation)
/// - crate::core::style (style resolution)
///
/// The compositor module has NO imports from these modules.
/// Any future import would be a STOP-08 violation.
///
/// # SPEC-HAL-082A Override (Binding)
///
/// SPEC-HAL-082 described per-window compositors, renderers, widget trees.
/// SPEC-HAL-082A **overrides** this: windows are transport-only, non-authoritative.
/// The compositor is surface-scoped, not window-scoped. This is intentional.
/// Do NOT add window-ownership semantics to the compositor.
#[doc(hidden)]
mod _authority_firewall {
    // This module exists solely to document the authority boundary.
    // It contains no code.
    //
    // If you need to add an import to the compositor module, check
    // SPEC-HAL-081A first. Imports from widget/event_router/layout/style
    // are FORBIDDEN and require SPEC amendment.
}

// ============================================================================
// MWA Deferred Items — Explicitly Sealed (SPEC-HAL-082A Non-Goals)
// ============================================================================
//
// The following features are EXPLICITLY OUT OF SCOPE for the MWA layer.
// They are not "left for later" — they are forbidden without SPEC amendment.
//
// SEALED-MWA-001: Multi-monitor placement policy
//   - NO monitor enumeration in compositor
//   - NO placement hints or preferred-monitor fields
//   - Placement is OS-level only (transport)
//
// SEALED-MWA-002: Window persistence and restore
//   - NO serialization of window state in compositor
//   - NO save/restore lifecycle hooks
//   - Persistence is application-level if needed
//
// SEALED-MWA-003: Window type taxonomy (tool, dialog, inspector)
//   - NO WindowType enum in compositor
//   - NO semantic window classification
//   - OS window hints are transport-level only
//
// SEALED-MWA-004: Cross-window drag-and-drop
//   - NO drag context migration in compositor
//   - NO cross-surface pointer handoff semantics
//   - Drag is interaction-domain scoped (SPEC-HAL-151)
//
// SEALED-MWA-005: Per-window independent widget trees
//   - NO WidgetTree ownership in compositor or window
//   - Current: single shared tree, multiple surfaces
//   - Per-window trees require explicit SPEC
//
// Any attempt to implement these features in the compositor module
// violates the MWA closure and requires SPEC amendment.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_compositor_create_surface() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let id = compositor.create_surface(0, RectF::new(0.0, 0.0, 400.0, 300.0), SceneKey::BASE);
        assert!(compositor.has_surface(id));
        assert_eq!(compositor.surface_count(), 1);
    }

    #[test]
    fn test_compositor_destroy_surface() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));
        let id = compositor.create_surface(0, RectF::new(0.0, 0.0, 400.0, 300.0), SceneKey::BASE);
        assert!(compositor.destroy_surface(id));
        assert!(!compositor.has_surface(id));
        assert_eq!(compositor.surface_count(), 0);
    }

    #[test]
    fn test_compositor_snapshot_ordering() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create surfaces with different z-indices (all share same scene_key for single-surface test)
        // Note: Uses snapshot_unchecked() to test multi-surface ordering logic
        // without triggering single-surface contract enforcement
        let id1 = compositor.create_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let id2 = compositor.create_surface(5, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let id3 = compositor.create_surface(15, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        let snapshot = compositor.snapshot_unchecked();
        let surfaces: Vec<_> = snapshot.iter().collect();

        // Should be ordered by z-index: id2 (5), id1 (10), id3 (15)
        assert_eq!(surfaces.len(), 3);
        assert_eq!(surfaces[0].id, id2);
        assert_eq!(surfaces[1].id, id1);
        assert_eq!(surfaces[2].id, id3);
    }

    #[test]
    fn test_compositor_z_index_update() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Note: Uses snapshot_unchecked() to test multi-surface z-index logic
        // without triggering single-surface contract enforcement
        let id1 = compositor.create_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        let id2 = compositor.create_surface(5, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);

        // Change id2's z-index to be higher than id1
        compositor.set_surface_z_index(id2, 20);

        let snapshot = compositor.snapshot_unchecked();
        let surfaces: Vec<_> = snapshot.iter().collect();

        // Now id1 (10) should come before id2 (20)
        assert_eq!(surfaces[0].id, id1);
        assert_eq!(surfaces[1].id, id2);
    }

    // ========================================================================
    // SPEC-HAL-081C: Compositor Overlay API Tests
    // ========================================================================

    /// SPEC-HAL-081C: Overlay creation succeeds with valid parent.
    #[test]
    fn compositor_create_overlay_surface_success() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base surface
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // Create overlay via new API
        let overlay_id = compositor.create_overlay_surface(
            base_id,
            100,
            RectF::new(10.0, 10.0, 200.0, 100.0),
            SceneKey::new(2),
        );

        assert!(overlay_id.is_some(), "Overlay creation should succeed");
        let overlay_id = overlay_id.unwrap();

        // Verify overlay exists and has correct parent
        assert!(compositor.has_surface(overlay_id));
        assert_eq!(compositor.overlay_parent(overlay_id), Some(base_id));

        // Verify base has no parent (it's a base surface)
        assert_eq!(compositor.overlay_parent(base_id), None);
    }

    /// SPEC-HAL-081C: Overlay creation rejects nonexistent parent.
    #[test]
    fn compositor_create_overlay_surface_rejects_nonexistent_parent() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create and destroy a surface to get a stale ID
        let temp_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), SceneKey::BASE);
        compositor.destroy_surface(temp_id);

        // Try to create overlay with nonexistent parent
        let result = compositor.create_overlay_surface(
            temp_id,
            100,
            RectF::new(0.0, 0.0, 100.0, 100.0),
            SceneKey::new(2),
        );

        assert!(
            result.is_none(),
            "Overlay with nonexistent parent must return None"
        );
    }

    /// SPEC-HAL-081C: Overlay creation rejects overlay as parent.
    #[test]
    fn compositor_create_overlay_surface_rejects_overlay_parent() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base and overlay
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        let overlay_id = compositor
            .create_overlay_surface(
                base_id,
                100,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(2),
            )
            .unwrap();

        // Try to create overlay with overlay as parent
        let result = compositor.create_overlay_surface(
            overlay_id,
            200,
            RectF::new(0.0, 0.0, 50.0, 50.0),
            SceneKey::new(3),
        );

        assert!(
            result.is_none(),
            "Overlay with overlay parent must return None"
        );
    }

    /// SPEC-HAL-081C: Overlay creation rejects z_index <= parent.
    #[test]
    fn compositor_create_overlay_surface_rejects_low_z() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base with z=100
        let base_id =
            compositor.create_surface(100, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // Try overlay with z=100 (equal)
        let result1 = compositor.create_overlay_surface(
            base_id,
            100,
            RectF::new(0.0, 0.0, 100.0, 100.0),
            SceneKey::new(2),
        );
        assert!(
            result1.is_none(),
            "Overlay with z_index == parent must return None"
        );

        // Try overlay with z=50 (less)
        let result2 = compositor.create_overlay_surface(
            base_id,
            50,
            RectF::new(0.0, 0.0, 100.0, 100.0),
            SceneKey::new(3),
        );
        assert!(
            result2.is_none(),
            "Overlay with z_index < parent must return None"
        );
    }

    /// SPEC-HAL-081C: overlays_of returns children sorted by z-index.
    #[test]
    fn compositor_overlays_of_returns_sorted_by_z() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base with z=0
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // Create overlays in non-sorted order: z=30, z=10, z=20
        let overlay_z30 = compositor
            .create_overlay_surface(
                base_id,
                30,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(2),
            )
            .unwrap();
        let overlay_z10 = compositor
            .create_overlay_surface(
                base_id,
                10,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(3),
            )
            .unwrap();
        let overlay_z20 = compositor
            .create_overlay_surface(
                base_id,
                20,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(4),
            )
            .unwrap();

        // overlays_of must return in z-index order: z=10, z=20, z=30
        let overlays = compositor.overlays_of(base_id);
        assert_eq!(overlays.len(), 3);
        assert_eq!(overlays[0], overlay_z10, "First overlay should be z=10");
        assert_eq!(overlays[1], overlay_z20, "Second overlay should be z=20");
        assert_eq!(overlays[2], overlay_z30, "Third overlay should be z=30");
    }

    /// SPEC-HAL-081C: Destroying base cascades to all overlays.
    #[test]
    fn compositor_destroy_base_cascades_to_overlays() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base with overlays
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);
        let overlay1 = compositor
            .create_overlay_surface(
                base_id,
                10,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(2),
            )
            .unwrap();
        let overlay2 = compositor
            .create_overlay_surface(
                base_id,
                20,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(3),
            )
            .unwrap();

        assert_eq!(compositor.surface_count(), 3);

        // Destroy base
        let destroyed = compositor.destroy_surface(base_id);
        assert!(destroyed);

        // All surfaces should be gone
        assert!(!compositor.has_surface(base_id), "Base should be destroyed");
        assert!(
            !compositor.has_surface(overlay1),
            "Overlay1 should be cascade-destroyed"
        );
        assert!(
            !compositor.has_surface(overlay2),
            "Overlay2 should be cascade-destroyed"
        );
        assert_eq!(compositor.surface_count(), 0);

        // overlay_parent returns None for destroyed surfaces
        assert_eq!(compositor.overlay_parent(overlay1), None);
        assert_eq!(compositor.overlay_parent(overlay2), None);
    }

    // ========================================================================
    // SPEC-HAL-081C: Snapshot & Query Hardening Tests
    // ========================================================================

    /// INVARIANT: No snapshot path ever returns overlay surfaces.
    ///
    /// This test creates base + overlays via Compositor API and verifies
    /// that ALL snapshot-producing APIs exclude overlays.
    ///
    /// Fails loudly if someone accidentally removes overlay filtering.
    #[test]
    fn compositor_snapshot_never_contains_overlays() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base surface
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // Create multiple overlays
        let overlay1 = compositor
            .create_overlay_surface(
                base_id,
                10,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(2),
            )
            .unwrap();
        let overlay2 = compositor
            .create_overlay_surface(
                base_id,
                20,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(3),
            )
            .unwrap();
        let overlay3 = compositor
            .create_overlay_surface(
                base_id,
                30,
                RectF::new(0.0, 0.0, 100.0, 100.0),
                SceneKey::new(4),
            )
            .unwrap();

        // Verify stack has all 4 surfaces
        assert_eq!(
            compositor.surface_count(),
            4,
            "Expected 1 base + 3 overlays"
        );

        // --- Test snapshot_unchecked() ---
        let snapshot = compositor.snapshot_unchecked();
        let snapshot_ids: Vec<_> = snapshot.iter().map(|s| s.id).collect();
        assert_eq!(
            snapshot_ids.len(),
            1,
            "INVARIANT VIOLATION: snapshot_unchecked() must contain only base surface"
        );
        assert!(
            snapshot_ids.contains(&base_id),
            "INVARIANT VIOLATION: snapshot must contain base"
        );
        assert!(
            !snapshot_ids.contains(&overlay1),
            "INVARIANT VIOLATION: snapshot contains overlay1"
        );
        assert!(
            !snapshot_ids.contains(&overlay2),
            "INVARIANT VIOLATION: snapshot contains overlay2"
        );
        assert!(
            !snapshot_ids.contains(&overlay3),
            "INVARIANT VIOLATION: snapshot contains overlay3"
        );

        // --- Test snapshot() via visible() ---
        let visible_ids: Vec<_> = snapshot.visible().map(|s| s.id).collect();
        assert!(
            !visible_ids.contains(&overlay1),
            "INVARIANT VIOLATION: visible() contains overlay"
        );

        // --- Test snapshot() via iter_top_down() ---
        let top_down_ids: Vec<_> = snapshot.iter_top_down().map(|s| s.id).collect();
        assert!(
            !top_down_ids.contains(&overlay1),
            "INVARIANT VIOLATION: iter_top_down() contains overlay"
        );

        // --- Test snapshot() via get() ---
        assert!(
            snapshot.get(overlay1).is_none(),
            "INVARIANT VIOLATION: snapshot.get() returned overlay"
        );
        assert!(
            snapshot.get(overlay2).is_none(),
            "INVARIANT VIOLATION: snapshot.get() returned overlay"
        );
    }

    /// INVARIANT: get_surface() returns None for overlay surfaces.
    ///
    /// Overlays are not accessible through get_surface() - the returned
    /// SurfaceDescriptor has no parent_surface field (ABI frozen), so
    /// returning an overlay descriptor would misrepresent its role.
    ///
    /// Explicit overlay queries must use overlay_parent() and overlays_of().
    #[test]
    fn compositor_get_surface_returns_descriptor_without_parent() {
        let mut compositor = Compositor::new(RectF::new(0.0, 0.0, 800.0, 600.0));

        // Create base surface
        let base_id =
            compositor.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), SceneKey::BASE);

        // Create overlay
        let overlay_id = compositor
            .create_overlay_surface(
                base_id,
                100,
                RectF::new(10.0, 10.0, 200.0, 100.0),
                SceneKey::new(2),
            )
            .unwrap();

        // Verify overlay exists
        assert!(compositor.has_surface(overlay_id), "Overlay must exist");

        // --- Test: get_surface() returns base descriptor ---
        let base_desc = compositor.get_surface(base_id);
        assert!(
            base_desc.is_some(),
            "get_surface() must return base surface descriptor"
        );
        let base_desc = base_desc.unwrap();
        assert_eq!(base_desc.id, base_id);
        // SurfaceDescriptor has no parent_surface field (ABI frozen)
        // The type SurfaceDescriptor<BaseSurfaceTag> proves it's a base surface

        // --- Test: get_surface() returns None for overlay ---
        let overlay_desc = compositor.get_surface(overlay_id);
        assert!(
            overlay_desc.is_none(),
            "INVARIANT VIOLATION: get_surface() must return None for overlay surfaces"
        );

        // --- Test: overlay is still accessible via explicit API ---
        assert_eq!(
            compositor.overlay_parent(overlay_id),
            Some(base_id),
            "Overlay must be accessible via overlay_parent()"
        );
    }
}
