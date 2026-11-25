//! Surface stack management for the compositor.
//!
//! SPEC-HAL-081A: Ordered surface stack (frozen contract).
//! SPEC-HAL-081B: Type-level surface role separation.
//! SPEC-HAL-081C: Overlay surface support.

use super::CompositorIdentity;
use super::contract::{
    BaseContract, Contract, Contracted, MultiSurfaceContract, SceneKey,
    assert_base_contract_surface_count,
};
use super::surface::{SurfaceDescriptor, SurfaceId, SurfaceState, SurfaceVisibility};
use super::surface_role::BaseSurfaceTag;
use crate::core::math::RectF;
use std::sync::atomic::{AtomicU64, Ordering};

/// Global surface ID counter.
static NEXT_SURFACE_ID: AtomicU64 = AtomicU64::new(1);

/// Generate a unique surface ID.
fn next_surface_id() -> SurfaceId {
    SurfaceId::new(NEXT_SURFACE_ID.fetch_add(1, Ordering::Relaxed))
}

/// Ordered stack of compositor surfaces.
///
/// Maintains surfaces in z-order (lowest first) for composition.
/// Provides mutation APIs for surface management and immutable
/// snapshots for downstream consumers.
///
/// # Ordering Invariant
///
/// Surfaces are always ordered by z_index (ascending).
/// After any mutation that affects z_index, the stack is re-sorted.
#[derive(Debug, Default)]
pub struct SurfaceStack {
    /// Surfaces ordered by z_index (lowest first).
    surfaces: Vec<SurfaceState>,
    /// Flag indicating if the stack needs re-sorting.
    needs_sort: bool,
}

impl SurfaceStack {
    /// Create a new empty surface stack.
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a new base surface and add it to the stack.
    ///
    /// The stack will be re-sorted to maintain z-order.
    ///
    /// # Arguments
    ///
    /// * `z_index` - Z-order for the surface (higher = on top)
    /// * `bounds` - Surface bounds in logical coordinates
    /// * `scene_key` - Scene key identifying content (must be non-zero)
    pub fn create_surface(&mut self, z_index: i32, bounds: RectF, scene_key: u64) -> SurfaceId {
        let id = next_surface_id();
        let state = SurfaceState::new(id, z_index, bounds, scene_key);
        self.surfaces.push(state);
        self.needs_sort = true;
        self.ensure_sorted();
        id
    }

    /// SPEC-HAL-081C: Create a new overlay surface and add it to the stack.
    ///
    /// # Arguments
    ///
    /// * `z_index` - Z-order for the overlay (MUST be > parent's z_index)
    /// * `bounds` - Surface bounds in logical coordinates
    /// * `scene_key` - Scene key identifying content (must be non-zero)
    /// * `parent_surface` - ID of the parent base surface
    ///
    /// # Returns
    ///
    /// `Some(SurfaceId)` if created successfully, `None` if parent doesn't exist
    /// or parent is not a base surface.
    ///
    /// # Invariants
    ///
    /// - INV-SURF-ROLE-03: Overlay surfaces MUST have parent
    /// - INV-PROV-01: Parent surface MUST exist
    /// - INV-PROV-02: Parent surface MUST be a Base surface
    /// - INV-ZORD-02: Overlay z-index MUST be > parent's z-index
    pub(crate) fn create_overlay_surface(
        &mut self,
        z_index: i32,
        bounds: RectF,
        scene_key: u64,
        parent_surface: SurfaceId,
    ) -> Option<SurfaceId> {
        // INV-PROV-01: Parent must exist
        let parent_idx = self.find_index(parent_surface)?;
        let parent = &self.surfaces[parent_idx];

        // INV-PROV-02: Parent must be base surface
        if !parent.is_base() {
            return None;
        }

        // INV-ZORD-02: Overlay z-index must be > parent's z-index
        let parent_z = parent.z_index;
        if z_index <= parent_z {
            return None;
        }

        let id = next_surface_id();
        let state = SurfaceState::new_overlay(id, z_index, bounds, scene_key, parent_surface);
        self.surfaces.push(state);
        self.needs_sort = true;
        self.ensure_sorted();
        Some(id)
    }

    /// Remove a surface from the stack.
    ///
    /// If removing a base surface, all its overlay children are also removed
    /// (SPEC-HAL-081C INV-LIFE-04, INV-LIFE-05).
    ///
    /// Returns `true` if the surface was found and removed.
    pub fn destroy_surface(&mut self, id: SurfaceId) -> bool {
        if let Some(idx) = self.find_index(id) {
            let is_base = self.surfaces[idx].is_base();

            // SPEC-HAL-081C INV-LIFE-04/05: Cascade-destroy overlay children
            if is_base {
                // Remove all overlays that have this surface as parent
                self.surfaces
                    .retain(|s| s.parent_surface != Some(id) || s.is_base());
            }

            // Now remove the surface itself (find_index again since indices may have shifted)
            if let Some(idx) = self.find_index(id) {
                self.surfaces.remove(idx);
                return true;
            }
        }
        false
    }

    /// Update a surface's z-index.
    ///
    /// The stack will be re-sorted.
    pub fn set_z_index(&mut self, id: SurfaceId, z_index: i32) {
        if let Some(idx) = self.find_index(id) {
            self.surfaces[idx].z_index = z_index;
            self.needs_sort = true;
            self.ensure_sorted();
        }
    }

    /// Update a surface's bounds.
    pub fn set_bounds(&mut self, id: SurfaceId, bounds: RectF) {
        if let Some(idx) = self.find_index(id) {
            self.surfaces[idx].bounds = bounds;
        }
    }

    /// Update a surface's clip rectangle.
    pub fn set_clip(&mut self, id: SurfaceId, clip: Option<RectF>) {
        if let Some(idx) = self.find_index(id) {
            self.surfaces[idx].clip = clip;
        }
    }

    /// Update a surface's visibility.
    pub fn set_visibility(&mut self, id: SurfaceId, visibility: SurfaceVisibility) {
        if let Some(idx) = self.find_index(id) {
            self.surfaces[idx].visibility = visibility;
        }
    }

    /// Update a surface's opacity.
    pub fn set_opacity(&mut self, id: SurfaceId, opacity: f32) {
        if let Some(idx) = self.find_index(id) {
            self.surfaces[idx].opacity = opacity;
        }
    }

    /// Set the scene key for a surface.
    pub fn set_scene_key(&mut self, id: SurfaceId, scene_key: SceneKey) {
        if let Some(idx) = self.find_index(id) {
            self.surfaces[idx].scene_key = scene_key.get();
        }
    }

    pub fn apply_frame_projection(
        &self,
        projection: &NavigationFrameProjection,
    ) -> Result<ProjectedSurfaceView, FrameProjectionError> {
        let mut navigation_contexts = Vec::with_capacity(projection.navigation_contexts.len());
        for (context_index, context) in projection.navigation_contexts.iter().enumerate() {
            let mut navigation_surfaces = Vec::with_capacity(context.navigation_surfaces.len());
            for surface in &context.navigation_surfaces {
                let state = self.get_state(*surface).ok_or(
                    FrameProjectionError::MissingNavigationSurface {
                        context_index,
                        surface: *surface,
                    },
                )?;
                if !state.is_base() {
                    return Err(FrameProjectionError::NavigationSurfaceRoleMismatch {
                        context_index,
                        surface: *surface,
                    });
                }
                navigation_surfaces.push(state.to_descriptor());
            }
            if let Some(active_surface) = context.active_surface {
                let state = self.get_state(active_surface).ok_or(
                    FrameProjectionError::MissingActiveSurface {
                        context_index,
                        surface: active_surface,
                    },
                )?;
                if !state.is_base() || !context.navigation_surfaces.contains(&active_surface) {
                    return Err(FrameProjectionError::ActiveSurfaceOutsideContext {
                        context_index,
                        surface: active_surface,
                    });
                }
            }
            navigation_contexts.push(ProjectedNavigationContext {
                navigation_surfaces,
                active_surface: context.active_surface,
            });
        }

        for surface in &projection.overlay_surfaces {
            let state = self
                .get_state(*surface)
                .ok_or(FrameProjectionError::MissingOverlaySurface(*surface))?;
            if !state.is_overlay() {
                return Err(FrameProjectionError::OverlaySurfaceRoleMismatch(*surface));
            }
        }
        if let Some(surface) = projection.focused_surface
            && !self.contains(surface)
        {
            return Err(FrameProjectionError::MissingFocusedSurface(surface));
        }

        Ok(ProjectedSurfaceView {
            navigation_contexts,
            overlay_surfaces: projection.overlay_surfaces.clone(),
            focused_surface: projection.focused_surface,
        })
    }

    /// Get a snapshot of the surface stack for rendering (BaseContract).
    ///
    /// The snapshot is an immutable, ordered list of BASE surface descriptors only.
    /// Overlay surfaces are filtered out (SPEC-HAL-081C: renderer invisibility).
    ///
    /// # Contract Enforcement (SPEC-HAL-081A)
    ///
    /// Returns `SurfaceStackSnapshot<BaseContract>`, which enforces the single-surface
    /// constraint. For multi-surface (multi-window), use `snapshot_multi()`.
    ///
    /// # Renderer Isolation (SPEC-HAL-081C)
    ///
    /// This method returns ONLY base surfaces. Overlay surfaces are compositor-internal
    /// and MUST NOT reach the renderer. This is the enforcement point for
    /// renderer invisibility.
    pub fn snapshot(&self) -> SurfaceStackSnapshot<BaseContract> {
        // SPEC-HAL-081A: Enforce single-surface constraint (on base surfaces only)
        let base_count = self.base_surface_count();
        assert_base_contract_surface_count(base_count);

        self.build_snapshot()
    }

    /// Get a snapshot of the surface stack for multi-surface rendering (SPEC-HAL-081B).
    ///
    /// Returns `SurfaceStackSnapshot<MultiSurfaceContract>`, which permits multiple
    /// base surfaces. Overlay surfaces are still filtered out.
    ///
    /// # Renderer Isolation (SPEC-HAL-081C)
    ///
    /// This method returns ONLY base surfaces. Overlay surfaces are compositor-internal
    /// and MUST NOT reach the renderer.
    pub fn snapshot_multi(&self) -> SurfaceStackSnapshot<MultiSurfaceContract> {
        // MultiSurfaceContract: no count constraint (MAX_SURFACES = usize::MAX)
        self.build_snapshot()
    }

    /// Internal snapshot builder, generic over contract.
    ///
    /// # OVERLAY ISOLATION BOUNDARY (SPEC-HAL-081C)
    ///
    /// The filter below is the ONLY barrier preventing overlay surfaces from
    /// reaching the renderer. If you remove or weaken this filter, overlays
    /// will leak into SurfaceStackSnapshot and violate renderer invisibility.
    /// Test: invariant_snapshot_contains_only_base_surfaces
    ///
    /// # Z-ORDER INVARIANT
    ///
    /// Surfaces in the snapshot are in ascending z-index order (lowest first).
    /// This is enforced by `ensure_sorted()` after any mutation, and verified
    /// here with a debug assertion.
    fn build_snapshot<C: Contract>(&self) -> SurfaceStackSnapshot<C> {
        // INV-ZORD: Verify z-order invariant (surfaces must be sorted by z_index)
        debug_assert!(
            self.surfaces
                .windows(2)
                .all(|w| w[0].z_index <= w[1].z_index),
            "Z-order invariant violation: surfaces not sorted by z_index"
        );

        SurfaceStackSnapshot {
            surfaces: self
                .surfaces
                .iter()
                .filter(|s| s.is_base())
                .map(|s| s.to_descriptor())
                .collect(),
            _phase: Contracted::new(),
        }
    }

    /// Test-only snapshot that bypasses BaseContract enforcement.
    ///
    /// This allows testing the SurfaceStack's internal ordering and visibility
    /// logic with multiple surfaces using BaseContract type signature.
    ///
    /// Note: Still filters to base surfaces only for type safety.
    #[cfg(test)]
    pub fn snapshot_unchecked(&self) -> SurfaceStackSnapshot<BaseContract> {
        // Bypass BaseContract count assertion, but keep overlay filter
        self.build_snapshot()
    }

    /// Get the total number of surfaces (base + overlay).
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// SPEC-HAL-081C: Get the number of base surfaces.
    pub fn base_surface_count(&self) -> usize {
        self.surfaces.iter().filter(|s| s.is_base()).count()
    }

    /// SPEC-HAL-081C: Get the number of overlay surfaces.
    #[allow(dead_code)] // Will be used in future 081C tasks
    pub(super) fn overlay_count(&self) -> usize {
        self.surfaces.iter().filter(|s| s.is_overlay()).count()
    }

    /// Check if the stack is empty.
    #[allow(
        dead_code,
        reason = "Retained stack emptiness query at the compositor owner boundary"
    )]
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    /// Check if a surface exists in the stack.
    pub fn contains(&self, id: SurfaceId) -> bool {
        self.find_index(id).is_some()
    }

    /// SPEC-HAL-081C: Check if a surface is a base surface.
    #[allow(dead_code)] // Will be used in future 081C tasks
    pub(super) fn is_base_surface(&self, id: SurfaceId) -> bool {
        self.find_index(id)
            .map(|idx| self.surfaces[idx].is_base())
            .unwrap_or(false)
    }

    /// SPEC-HAL-081C: Get the parent surface of an overlay.
    ///
    /// Returns `Some(parent_id)` if `id` is an overlay surface.
    /// Returns `None` if `id` is a base surface or does not exist.
    ///
    /// EXPLICIT OVERLAY VISIBILITY: This method (and overlays_of) is the ONLY
    /// path through which overlay provenance is exposed. All other read APIs
    /// (snapshot, get_surface) filter overlays out. Do not add new paths.
    pub(super) fn overlay_parent(&self, id: SurfaceId) -> Option<SurfaceId> {
        self.get_state(id).and_then(|s| s.parent_surface)
    }

    /// SPEC-HAL-081C: Get all overlay children of a base surface.
    ///
    /// Returns overlay IDs in ascending z-index order (matching stack ordering).
    /// Returns empty if `parent` does not exist or is not a base surface.
    ///
    /// EXPLICIT OVERLAY VISIBILITY: This method (and overlay_parent) is the ONLY
    /// path through which overlay enumeration is exposed. All other read APIs
    /// (snapshot, get_surface) filter overlays out. Do not add new paths.
    pub(super) fn overlays_of(&self, parent: SurfaceId) -> Vec<SurfaceId> {
        // Verify parent exists and is a base surface
        let parent_state = match self.get_state(parent) {
            Some(s) if s.is_base() => s,
            _ => return Vec::new(),
        };
        let _ = parent_state; // Used only for validation

        // Collect overlays with this parent, already sorted by z_index
        self.surfaces
            .iter()
            .filter(|s| s.parent_surface == Some(parent))
            .map(|s| s.id)
            .collect()
    }

    /// Get a surface state by ID (internal use).
    ///
    /// For external consumers, use `snapshot()` to get immutable descriptors.
    pub(super) fn get_state(&self, id: SurfaceId) -> Option<&SurfaceState> {
        self.find_index(id).map(|idx| &self.surfaces[idx])
    }

    /// Find the index of a surface by ID.
    fn find_index(&self, id: SurfaceId) -> Option<usize> {
        self.surfaces.iter().position(|s| s.id == id)
    }

    /// Ensure the stack is sorted by z_index.
    ///
    /// Ordering: surfaces are sorted by z_index ascending.
    /// Base surfaces naturally come before overlays due to INV-ZORD-02
    /// (overlay z-index > parent z-index).
    fn ensure_sorted(&mut self) {
        if self.needs_sort {
            self.surfaces.sort_by_key(|s| s.z_index);
            self.needs_sort = false;
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationProjectionContext {
    pub navigation_surfaces: Vec<SurfaceId>,
    pub active_surface: Option<SurfaceId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NavigationFrameProjection {
    compositor_identity: CompositorIdentity,
    pub navigation_contexts: Vec<NavigationProjectionContext>,
    pub overlay_surfaces: Vec<SurfaceId>,
    pub focused_surface: Option<SurfaceId>,
}

impl NavigationFrameProjection {
    pub fn new(
        compositor_identity: CompositorIdentity,
        navigation_contexts: Vec<NavigationProjectionContext>,
        overlay_surfaces: Vec<SurfaceId>,
        focused_surface: Option<SurfaceId>,
    ) -> Self {
        Self {
            compositor_identity,
            navigation_contexts,
            overlay_surfaces,
            focused_surface,
        }
    }

    pub const fn compositor_identity(&self) -> CompositorIdentity {
        self.compositor_identity
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FrameProjectionError {
    WrongCompositor {
        expected: CompositorIdentity,
        actual: CompositorIdentity,
    },
    MissingNavigationSurface {
        context_index: usize,
        surface: SurfaceId,
    },
    NavigationSurfaceRoleMismatch {
        context_index: usize,
        surface: SurfaceId,
    },
    MissingActiveSurface {
        context_index: usize,
        surface: SurfaceId,
    },
    ActiveSurfaceOutsideContext {
        context_index: usize,
        surface: SurfaceId,
    },
    MissingOverlaySurface(SurfaceId),
    OverlaySurfaceRoleMismatch(SurfaceId),
    MissingFocusedSurface(SurfaceId),
}

#[derive(Debug, Clone)]
pub struct ProjectedNavigationContext {
    pub navigation_surfaces: Vec<SurfaceDescriptor<BaseSurfaceTag>>,
    pub active_surface: Option<SurfaceId>,
}

/// Navigation-projected view of compositor surfaces.
///
/// Produced by `SurfaceStack::apply_frame_projection()`. Resolves surface IDs
/// without reordering, inference, retention, or silent omission.
#[derive(Debug, Clone)]
pub struct ProjectedSurfaceView {
    pub navigation_contexts: Vec<ProjectedNavigationContext>,
    pub overlay_surfaces: Vec<SurfaceId>,
    pub focused_surface: Option<SurfaceId>,
}

/// Immutable snapshot of the surface stack.
///
/// This is the primary interface for downstream consumers
/// (DrawPlanBuilder, EventFlow router).
///
/// # Contract Parameterization
///
/// The `P` type parameter encodes the compositor contract at the type level:
/// - `SurfaceStackSnapshot<BaseContract>`: Single-surface constraint enforced
///
/// Multi-surface composition would require a different contract marker type.
/// This prevents unsupported multi-surface code from compiling without explicit
/// introduction of new contract marker types.
///
/// # Surface Role (SPEC-HAL-081B)
///
/// Under `BaseContract`, all surfaces are `SurfaceDescriptor<BaseSurfaceTag>`.
/// Overlay surfaces (`OverlaySurfaceTag`) require `MultiSurfaceContract`.
///
/// # INV-COMP-P03 Compliance
///
/// The snapshot provides "ordered, inspectable SurfaceStack descriptors"
/// that upstream systems consume without compositor involvement in
/// the actual routing or rendering execution.
#[derive(Debug, Clone)]
pub struct SurfaceStackSnapshot<P: Contract = BaseContract> {
    /// Base surfaces (always present under any contract).
    surfaces: Vec<SurfaceDescriptor<BaseSurfaceTag>>,
    /// Phase marker for compile-time enforcement.
    _phase: Contracted<P>,
}

impl<P: Contract> SurfaceStackSnapshot<P> {
    /// Iterate over base surfaces in z-order (lowest first).
    pub fn iter(&self) -> impl Iterator<Item = &SurfaceDescriptor<BaseSurfaceTag>> {
        self.surfaces.iter()
    }

    /// Iterate over base surfaces in reverse z-order (highest first).
    ///
    /// Useful for hit-testing (by upstream systems, not compositor).
    pub fn iter_top_down(&self) -> impl Iterator<Item = &SurfaceDescriptor<BaseSurfaceTag>> {
        self.surfaces.iter().rev()
    }

    /// Get the number of base surfaces.
    pub fn len(&self) -> usize {
        self.surfaces.len()
    }

    /// Check if the snapshot is empty.
    pub fn is_empty(&self) -> bool {
        self.surfaces.is_empty()
    }

    /// Get visible base surfaces only.
    pub fn visible(&self) -> impl Iterator<Item = &SurfaceDescriptor<BaseSurfaceTag>> {
        self.surfaces.iter().filter(|s| s.is_visible())
    }

    /// Get a base surface by ID.
    pub fn get(&self, id: SurfaceId) -> Option<&SurfaceDescriptor<BaseSurfaceTag>> {
        self.surfaces.iter().find(|s| s.id == id)
    }

    /// Get a slice of all base surfaces.
    pub fn as_slice(&self) -> &[SurfaceDescriptor<BaseSurfaceTag>] {
        &self.surfaces
    }

    /// Get the phase name for debugging.
    pub fn phase_name(&self) -> &'static str {
        P::NAME
    }
}

impl<'a, P: Contract> IntoIterator for &'a SurfaceStackSnapshot<P> {
    type Item = &'a SurfaceDescriptor<BaseSurfaceTag>;
    type IntoIter = std::slice::Iter<'a, SurfaceDescriptor<BaseSurfaceTag>>;

    fn into_iter(self) -> Self::IntoIter {
        self.surfaces.iter()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_stack_ordering() {
        let mut stack = SurfaceStack::new();

        // scene_key=1 is the base surface key
        // Note: Uses snapshot_unchecked() to test multi-surface ordering logic
        // without triggering single-surface contract enforcement
        let id1 = stack.create_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let id2 = stack.create_surface(5, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let id3 = stack.create_surface(15, RectF::new(0.0, 0.0, 100.0, 100.0), 1);

        let snapshot = stack.snapshot_unchecked();
        let surfaces: Vec<_> = snapshot.iter().collect();

        // Should be in z-order: 5, 10, 15
        assert_eq!(surfaces[0].z_index, 5);
        assert_eq!(surfaces[0].id, id2);
        assert_eq!(surfaces[1].z_index, 10);
        assert_eq!(surfaces[1].id, id1);
        assert_eq!(surfaces[2].z_index, 15);
        assert_eq!(surfaces[2].id, id3);
    }

    #[test]
    fn test_surface_visibility_filter() {
        let mut stack = SurfaceStack::new();

        // Note: Uses snapshot_unchecked() to test multi-surface visibility logic
        // without triggering single-surface contract enforcement
        let id1 = stack.create_surface(1, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let id2 = stack.create_surface(2, RectF::new(0.0, 0.0, 100.0, 100.0), 1);

        stack.set_visibility(id1, SurfaceVisibility::Hidden);

        let snapshot = stack.snapshot_unchecked();
        let visible: Vec<_> = snapshot.visible().collect();

        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, id2);
    }

    #[test]
    fn test_surface_opacity() {
        let mut stack = SurfaceStack::new();
        let id = stack.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), 1);

        stack.set_opacity(id, 0.5);

        let snapshot = stack.snapshot();
        let surface = snapshot.get(id).unwrap();
        assert_eq!(surface.opacity, 0.5);
    }

    #[test]
    fn test_surface_occlusion() {
        let mut stack = SurfaceStack::new();
        let id = stack.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), 1);

        let snapshot = stack.snapshot();
        let surface = snapshot.get(id).unwrap();

        // Fully opaque, no clip = fully occluding
        assert!(surface.is_fully_occluding());

        // Make it semi-transparent
        stack.set_opacity(id, 0.5);
        let snapshot = stack.snapshot();
        let surface = snapshot.get(id).unwrap();
        assert!(!surface.is_fully_occluding());
    }

    // ========================================================================
    // SPEC-HAL-081A Guards
    // ========================================================================

    /// Guard: snapshot_unchecked() is test-only (#[cfg(test)]).
    ///
    /// This method exists solely to allow testing multi-surface ordering logic
    /// without triggering contract enforcement. It is gated by #[cfg(test)]
    /// and cannot be called from production code.
    ///
    /// Evidence: The method is defined with #[cfg(test)] attribute.
    /// If this test compiles, snapshot_unchecked exists in test builds.
    /// Production builds will not have this method available.
    #[test]
    fn guard_snapshot_unchecked_is_test_only() {
        let stack = SurfaceStack::new();
        // This call compiles only because we're in #[cfg(test)]
        let _snapshot = stack.snapshot_unchecked();
        // In non-test builds, this method doesn't exist
    }

    // ========================================================================
    // SPEC-HAL-081C Overlay Tests
    // ========================================================================

    /// SPEC-HAL-081C: Overlay surface creation.
    ///
    /// Verifies overlay surfaces can be created with valid parent.
    #[test]
    fn test_overlay_surface_creation() {
        let mut stack = SurfaceStack::new();

        // Create base surface
        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);

        // Create overlay with valid parent and higher z-index
        let overlay_id =
            stack.create_overlay_surface(100, RectF::new(10.0, 10.0, 200.0, 100.0), 2, base_id);

        assert!(
            overlay_id.is_some(),
            "Overlay creation should succeed with valid parent"
        );
        let overlay_id = overlay_id.unwrap();

        // Verify counts
        assert_eq!(stack.len(), 2); // Total surfaces
        assert_eq!(stack.base_surface_count(), 1);
        assert_eq!(stack.overlay_count(), 1);

        // Verify overlay state
        let overlay_state = stack.get_state(overlay_id).unwrap();
        assert!(overlay_state.is_overlay());
        assert_eq!(overlay_state.parent_surface, Some(base_id));
    }

    /// SPEC-HAL-081C: Overlay surfaces are invisible to renderer snapshots.
    ///
    /// Verifies snapshot() returns only base surfaces, not overlays.
    #[test]
    fn test_overlay_invisible_to_snapshot() {
        let mut stack = SurfaceStack::new();

        // Create base surface
        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);

        // Create overlay
        let _overlay_id =
            stack.create_overlay_surface(100, RectF::new(10.0, 10.0, 200.0, 100.0), 2, base_id);

        // Snapshot should only contain base surface
        let snapshot = stack.snapshot();
        assert_eq!(
            snapshot.len(),
            1,
            "Snapshot should contain only base surfaces"
        );
        assert_eq!(snapshot.iter().next().unwrap().id, base_id);
    }

    /// SPEC-HAL-081C INV-PROV-02: Cannot create overlay with non-base parent.
    #[test]
    fn test_overlay_rejects_non_base_parent() {
        let mut stack = SurfaceStack::new();

        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);
        let overlay1_id =
            stack.create_overlay_surface(100, RectF::new(0.0, 0.0, 100.0, 100.0), 2, base_id);
        let overlay1_id = overlay1_id.unwrap();

        // Try to create overlay with overlay parent (should fail)
        let result =
            stack.create_overlay_surface(200, RectF::new(0.0, 0.0, 50.0, 50.0), 3, overlay1_id);

        assert!(
            result.is_none(),
            "Creating overlay with overlay parent should fail"
        );
    }

    /// SPEC-HAL-081C INV-ZORD-02: Overlay z-index must be > parent z-index.
    #[test]
    fn test_overlay_rejects_low_z_index() {
        let mut stack = SurfaceStack::new();

        let base_id = stack.create_surface(100, RectF::new(0.0, 0.0, 800.0, 600.0), 1);

        // Try to create overlay with z-index <= parent (should fail)
        let result =
            stack.create_overlay_surface(50, RectF::new(0.0, 0.0, 100.0, 100.0), 2, base_id);

        assert!(
            result.is_none(),
            "Creating overlay with z-index <= parent should fail"
        );
    }

    /// SPEC-HAL-081C INV-PROV-01: Overlay requires existing parent.
    #[test]
    fn test_overlay_rejects_nonexistent_parent() {
        let mut stack = SurfaceStack::new();

        // Create and destroy a surface to get a stale ID
        let temp_id = stack.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        stack.destroy_surface(temp_id);

        // Try to create overlay with non-existent parent
        let result =
            stack.create_overlay_surface(100, RectF::new(0.0, 0.0, 100.0, 100.0), 2, temp_id);

        assert!(
            result.is_none(),
            "Creating overlay with non-existent parent should fail"
        );
    }

    /// SPEC-HAL-081C INV-LIFE-04/05: Destroying base cascades to overlays.
    #[test]
    fn test_destroy_base_cascades_to_overlays() {
        let mut stack = SurfaceStack::new();

        // Create base with two overlays
        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);
        let overlay1_id =
            stack.create_overlay_surface(100, RectF::new(0.0, 0.0, 100.0, 100.0), 2, base_id);
        let overlay2_id =
            stack.create_overlay_surface(200, RectF::new(0.0, 0.0, 100.0, 100.0), 3, base_id);

        assert!(overlay1_id.is_some());
        assert!(overlay2_id.is_some());
        assert_eq!(stack.len(), 3);

        // Destroy base surface
        let destroyed = stack.destroy_surface(base_id);
        assert!(destroyed);

        // All surfaces should be gone
        assert_eq!(
            stack.len(),
            0,
            "Destroying base should cascade to all overlays"
        );
        assert!(!stack.contains(base_id));
        assert!(!stack.contains(overlay1_id.unwrap()));
        assert!(!stack.contains(overlay2_id.unwrap()));
    }

    /// SPEC-HAL-081C: Destroying overlay does not affect base.
    #[test]
    fn test_destroy_overlay_preserves_base() {
        let mut stack = SurfaceStack::new();

        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);
        let overlay_id = stack
            .create_overlay_surface(100, RectF::new(0.0, 0.0, 100.0, 100.0), 2, base_id)
            .unwrap();

        // Destroy only overlay
        let destroyed = stack.destroy_surface(overlay_id);
        assert!(destroyed);

        // Base should remain
        assert_eq!(stack.len(), 1);
        assert!(stack.contains(base_id));
        assert!(!stack.contains(overlay_id));
    }

    /// SPEC-HAL-081C: Base surface state has parent_surface=None.
    ///
    /// Note: SurfaceDescriptor does NOT have parent_surface (ABI frozen).
    /// Overlay provenance is internal to compositor only.
    #[test]
    fn test_base_state_has_no_parent() {
        let mut stack = SurfaceStack::new();
        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);

        let state = stack.get_state(base_id).unwrap();

        assert!(
            state.is_base(),
            "Base surface state should have parent_surface=None"
        );
        assert!(
            state.parent_surface.is_none(),
            "Base surface should have no parent"
        );
    }

    // ========================================================================
    // SPEC-HAL-081C Invariant Guards
    // ========================================================================

    /// INVARIANT: Overlay z-index is always greater than parent's z-index.
    ///
    /// For any base surface B and its overlays O₁…Oₙ in the sorted stack:
    /// z_index(B) < z_index(Oᵢ) must hold.
    ///
    /// This test fails if ordering logic is accidentally changed.
    #[test]
    fn invariant_overlay_z_order_greater_than_parent() {
        let mut stack = SurfaceStack::new();

        // Create base surface at z=50
        let base_id = stack.create_surface(50, RectF::new(0.0, 0.0, 800.0, 600.0), 1);

        // Create multiple overlays with z > 50
        let overlay1 = stack
            .create_overlay_surface(100, RectF::new(0.0, 0.0, 100.0, 100.0), 2, base_id)
            .expect("overlay1 creation must succeed");
        let overlay2 = stack
            .create_overlay_surface(150, RectF::new(0.0, 0.0, 100.0, 100.0), 3, base_id)
            .expect("overlay2 creation must succeed");
        let overlay3 = stack
            .create_overlay_surface(75, RectF::new(0.0, 0.0, 100.0, 100.0), 4, base_id)
            .expect("overlay3 creation must succeed");

        // Get base z-index
        let base_z = stack.get_state(base_id).unwrap().z_index;

        // INVARIANT: Every overlay's z-index must be > base's z-index
        for overlay_id in [overlay1, overlay2, overlay3] {
            let overlay_state = stack.get_state(overlay_id).unwrap();
            assert!(
                overlay_state.is_overlay(),
                "Surface {:?} must be an overlay",
                overlay_id
            );
            assert!(
                overlay_state.z_index > base_z,
                "INVARIANT VIOLATION: Overlay {:?} z_index {} must be > parent z_index {}",
                overlay_id,
                overlay_state.z_index,
                base_z
            );
        }
    }

    /// INVARIANT: Snapshot contains only base surfaces, never overlays.
    ///
    /// SurfaceStackSnapshot<BaseContract> must filter out all overlay surfaces,
    /// regardless of how many exist in the stack.
    ///
    /// This is independent verification of snapshot isolation.
    #[test]
    fn invariant_snapshot_contains_only_base_surfaces() {
        let mut stack = SurfaceStack::new();

        // Create base surface
        let base_id = stack.create_surface(0, RectF::new(0.0, 0.0, 800.0, 600.0), 1);

        // Create multiple overlays
        let overlay1 = stack
            .create_overlay_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), 2, base_id)
            .unwrap();
        let overlay2 = stack
            .create_overlay_surface(20, RectF::new(0.0, 0.0, 100.0, 100.0), 3, base_id)
            .unwrap();
        let overlay3 = stack
            .create_overlay_surface(30, RectF::new(0.0, 0.0, 100.0, 100.0), 4, base_id)
            .unwrap();

        // Stack has 4 surfaces total
        assert_eq!(stack.len(), 4, "Stack should have 1 base + 3 overlays");

        // Take snapshot
        let snapshot = stack.snapshot();

        // INVARIANT: Snapshot must contain exactly 1 surface (the base)
        assert_eq!(
            snapshot.len(),
            1,
            "INVARIANT VIOLATION: Snapshot must contain only base surfaces, found {}",
            snapshot.len()
        );

        // INVARIANT: The single surface must be the base, not any overlay
        let snapshot_ids: Vec<_> = snapshot.iter().map(|s| s.id).collect();
        assert!(
            snapshot_ids.contains(&base_id),
            "INVARIANT VIOLATION: Snapshot must contain base surface"
        );
        assert!(
            !snapshot_ids.contains(&overlay1),
            "INVARIANT VIOLATION: Snapshot must not contain overlay1"
        );
        assert!(
            !snapshot_ids.contains(&overlay2),
            "INVARIANT VIOLATION: Snapshot must not contain overlay2"
        );
        assert!(
            !snapshot_ids.contains(&overlay3),
            "INVARIANT VIOLATION: Snapshot must not contain overlay3"
        );
    }

    /// INVARIANT: Destruction does not reorder unrelated surfaces.
    ///
    /// - Destroying an overlay must not reorder remaining base surfaces.
    /// - Destroying a base must remove its overlays without reordering unrelated bases.
    #[test]
    fn invariant_destruction_preserves_unrelated_order() {
        let mut stack = SurfaceStack::new();

        // Create two independent base surfaces with different z-indices
        // Using snapshot_unchecked to bypass single-surface contract for this test
        let base1 = stack.create_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let base2 = stack.create_surface(50, RectF::new(0.0, 0.0, 100.0, 100.0), 2);

        // Create overlays for base1
        let overlay1a = stack
            .create_overlay_surface(20, RectF::new(0.0, 0.0, 50.0, 50.0), 3, base1)
            .unwrap();
        let overlay1b = stack
            .create_overlay_surface(30, RectF::new(0.0, 0.0, 50.0, 50.0), 4, base1)
            .unwrap();

        // Create overlay for base2
        let overlay2a = stack
            .create_overlay_surface(60, RectF::new(0.0, 0.0, 50.0, 50.0), 5, base2)
            .unwrap();

        // Verify initial state: 5 surfaces total
        assert_eq!(stack.len(), 5);

        // Record base2's z-index before any destruction
        let base2_z_before = stack.get_state(base2).unwrap().z_index;

        // --- Test 1: Destroying overlay does not reorder bases ---
        stack.destroy_surface(overlay1a);
        assert_eq!(stack.len(), 4, "One overlay removed");

        // base2 must retain its z-index
        let base2_z_after = stack.get_state(base2).unwrap().z_index;
        assert_eq!(
            base2_z_before, base2_z_after,
            "INVARIANT VIOLATION: Destroying overlay1a changed base2's z-index"
        );

        // base2's overlay must still exist
        assert!(
            stack.contains(overlay2a),
            "INVARIANT VIOLATION: Destroying overlay1a removed unrelated overlay2a"
        );

        // --- Test 2: Destroying base removes its overlays, not unrelated surfaces ---
        stack.destroy_surface(base1);

        // base1 and its remaining overlay (overlay1b) should be gone
        assert!(!stack.contains(base1), "base1 should be destroyed");
        assert!(
            !stack.contains(overlay1b),
            "overlay1b should be cascade-destroyed"
        );

        // base2 and its overlay must remain
        assert!(
            stack.contains(base2),
            "INVARIANT VIOLATION: Destroying base1 removed unrelated base2"
        );
        assert!(
            stack.contains(overlay2a),
            "INVARIANT VIOLATION: Destroying base1 removed unrelated overlay2a"
        );

        // base2's z-index must be unchanged
        let base2_z_final = stack.get_state(base2).unwrap().z_index;
        assert_eq!(
            base2_z_before, base2_z_final,
            "INVARIANT VIOLATION: Destroying base1 changed base2's z-index"
        );

        // Final state: 2 surfaces (base2 + overlay2a)
        assert_eq!(stack.len(), 2);
    }

    // ========================================================================
    // SPEC-HAL-081B Multi-Surface Contract Tests
    // ========================================================================

    /// SPEC-HAL-081A: BaseContract rejects multiple base surfaces.
    ///
    /// Under BaseContract, creating a second base surface causes snapshot()
    /// to panic (in BOTH debug and release builds). Use snapshot_multi() for multi-window.
    #[test]
    #[should_panic(expected = "SPEC-HAL-081A violation")]
    fn contract_base_rejects_multiple_surfaces() {
        let mut stack = SurfaceStack::new();

        // Create first base surface - OK
        let _id1 = stack.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), 1);

        // Create second base surface - still OK at creation time
        let _id2 = stack.create_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), 2);

        // snapshot() enforces BaseContract - should panic
        let _snapshot = stack.snapshot();
    }

    /// SPEC-HAL-081B: MultiSurfaceContract permits multiple base surfaces.
    ///
    /// Under MultiSurfaceContract, creating multiple base surfaces is valid
    /// and snapshot_multi() returns all of them in z-order.
    #[test]
    fn contract_multi_surface_permits_multiple() {
        let mut stack = SurfaceStack::new();

        // Create multiple base surfaces
        let id1 = stack.create_surface(10, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let id2 = stack.create_surface(5, RectF::new(0.0, 0.0, 100.0, 100.0), 2);
        let id3 = stack.create_surface(15, RectF::new(0.0, 0.0, 100.0, 100.0), 3);

        // snapshot_multi() should succeed without panic
        let snapshot = stack.snapshot_multi();

        // Verify all 3 base surfaces are present in z-order
        assert_eq!(
            snapshot.len(),
            3,
            "MultiSurfaceContract should permit 3 base surfaces"
        );

        let surfaces: Vec<_> = snapshot.iter().collect();
        assert_eq!(surfaces[0].id, id2, "First should be z=5");
        assert_eq!(surfaces[0].z_index, 5);
        assert_eq!(surfaces[1].id, id1, "Second should be z=10");
        assert_eq!(surfaces[1].z_index, 10);
        assert_eq!(surfaces[2].id, id3, "Third should be z=15");
        assert_eq!(surfaces[2].z_index, 15);
    }

    /// SPEC-HAL-081B: Overlays excluded from MultiSurfaceContract snapshot.
    ///
    /// Even under MultiSurfaceContract, overlay surfaces are still filtered
    /// out of snapshots (SPEC-HAL-081C renderer invisibility).
    #[test]
    fn contract_multi_surface_excludes_overlays() {
        let mut stack = SurfaceStack::new();

        // Create two base surfaces
        let base1 = stack.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let base2 = stack.create_surface(50, RectF::new(0.0, 0.0, 100.0, 100.0), 2);

        // Create overlays for each base
        let overlay1 = stack
            .create_overlay_surface(10, RectF::new(0.0, 0.0, 50.0, 50.0), 3, base1)
            .unwrap();
        let overlay2 = stack
            .create_overlay_surface(60, RectF::new(0.0, 0.0, 50.0, 50.0), 4, base2)
            .unwrap();

        // Stack has 4 surfaces total
        assert_eq!(stack.len(), 4, "Stack should have 2 bases + 2 overlays");

        // snapshot_multi() should only contain base surfaces
        let snapshot = stack.snapshot_multi();
        assert_eq!(
            snapshot.len(),
            2,
            "MultiSurfaceContract snapshot should contain only base surfaces"
        );

        let snapshot_ids: Vec<_> = snapshot.iter().map(|s| s.id).collect();
        assert!(snapshot_ids.contains(&base1), "Snapshot must contain base1");
        assert!(snapshot_ids.contains(&base2), "Snapshot must contain base2");
        assert!(
            !snapshot_ids.contains(&overlay1),
            "Snapshot must NOT contain overlay1"
        );
        assert!(
            !snapshot_ids.contains(&overlay2),
            "Snapshot must NOT contain overlay2"
        );
    }

    /// SPEC-HAL-081B: Both contracts filter overlays identically.
    ///
    /// Verifies that overlay filtering is contract-independent.
    #[test]
    fn contract_both_filter_overlays_identically() {
        let mut stack = SurfaceStack::new();

        // Create base with overlays (single base for BaseContract compatibility)
        let base = stack.create_surface(0, RectF::new(0.0, 0.0, 100.0, 100.0), 1);
        let _overlay1 = stack
            .create_overlay_surface(10, RectF::new(0.0, 0.0, 50.0, 50.0), 2, base)
            .unwrap();
        let _overlay2 = stack
            .create_overlay_surface(20, RectF::new(0.0, 0.0, 50.0, 50.0), 3, base)
            .unwrap();

        // BaseContract snapshot (via snapshot_unchecked to avoid panic with overlays)
        let base_snapshot = stack.snapshot();
        let multi_snapshot = stack.snapshot_multi();

        // Both should have exactly 1 surface (the base)
        assert_eq!(
            base_snapshot.len(),
            1,
            "BaseContract snapshot should have 1 surface"
        );
        assert_eq!(
            multi_snapshot.len(),
            1,
            "MultiSurfaceContract snapshot should have 1 surface"
        );

        // Both should have the same base surface
        assert_eq!(base_snapshot.iter().next().unwrap().id, base);
        assert_eq!(multi_snapshot.iter().next().unwrap().id, base);
    }
}
