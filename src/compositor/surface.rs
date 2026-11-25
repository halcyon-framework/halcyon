//! Surface types for the compositor.
//!
//! SPEC-HAL-081A: Core surface descriptors.
//! SPEC-HAL-081B: Surface role type parameter (type-level only, no runtime change).
//! SPEC-HAL-081C: Overlay surface support (compositor-internal only).
//!
//! # Type-Level Extension (SPEC-HAL-081B)
//!
//! `SurfaceDescriptor<R>` carries a surface role marker at the type level.
//! Under `BaseContract`, only `SurfaceDescriptor<BaseSurfaceTag>` is constructible.
//! The default type parameter preserves backward compatibility.
//!
//! # Overlay Extension (SPEC-HAL-081C)
//!
//! `SurfaceState` tracks `parent_surface` for overlay support.
//! Overlay-ness is inferred structurally: `parent_surface.is_some()` = overlay.
//! `SurfaceDescriptor` is UNCHANGED - no runtime fields added.
//! Overlay provenance exists only in compositor-internal state.
//!
//! # Field Freeze (SPEC-HAL-081A)
//!
//! DO NOT add new runtime fields to SurfaceDescriptor.
//! Type parameters are permitted; new data fields are not.

use crate::core::math::RectF;
use std::marker::PhantomData;

use super::surface_role::{BaseSurfaceTag, OverlaySurfaceTag, SurfaceRole};

/// Unique identifier for a compositor surface.
///
/// Surfaces are identified by opaque handles, not references to widgets
/// or scene nodes. This enforces INV-COMP-N04 and INV-COMP-N05.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct SurfaceId(u64);

impl SurfaceId {
    /// Create a new SurfaceId from a raw value.
    ///
    /// This is internal to the compositor module.
    pub(super) fn new(id: u64) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    ///
    /// Useful for debugging and logging.
    pub fn raw(&self) -> u64 {
        self.0
    }

    /// Test-only: Create a deterministic SurfaceId for unit tests.
    ///
    /// This allows tests in other modules to construct SurfaceIds without
    /// using unsafe transmute. The helper is `pub(crate)` and `#[cfg(test)]`
    /// so it's not part of the public API.
    ///
    /// # Arguments
    ///
    /// * `n` - Raw value for the SurfaceId
    #[cfg(test)]
    pub(crate) fn test_id(n: u64) -> Self {
        Self(n)
    }
}

/// Surface visibility state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum SurfaceVisibility {
    /// Surface is visible and participates in composition.
    #[default]
    Visible,
    /// Surface is hidden and skipped during composition.
    Hidden,
}

/// Immutable descriptor for a compositor surface.
///
/// This is the data exposed to downstream consumers (RenderGraph, EventFlow).
/// It contains only the information needed for composition and routing,
/// without any widget or scene ownership.
///
/// # Type Parameter (SPEC-HAL-081B)
///
/// The `R` parameter encodes the surface role at the type level:
/// - `SurfaceDescriptor<BaseSurfaceTag>` - Base surface (default)
/// - `SurfaceDescriptor<OverlaySurfaceTag>` - Overlay surface (requires MultiSurfaceContract)
///
/// Under `BaseContract`, only `BaseSurfaceTag` surfaces are visible to the renderer.
///
/// # INV-COMP-P03 Compliance
///
/// SurfaceDescriptor is the "ordered, inspectable SurfaceStack descriptor"
/// that upstream routing consumes. It exposes:
/// - bounds/clips/transforms for routing correctness (INV-COMP-P07)
/// - z-order for surface ordering (INV-COMP-P01)
/// - scene_key for renderer to identify content
///
/// # INV-COMP-N05 Compliance
///
/// The scene_key is an opaque u64, not a SceneGraph reference.
/// The compositor tracks association but does not own scene content.
#[derive(Debug, Clone)]
pub struct SurfaceDescriptor<R: SurfaceRole = BaseSurfaceTag> {
    /// Unique surface identifier.
    pub id: SurfaceId,
    /// Z-order index (higher = on top).
    pub z_index: i32,
    /// Surface bounds in logical coordinates.
    pub bounds: RectF,
    /// Optional clip rectangle (in logical coordinates).
    /// If None, the surface is not clipped.
    pub clip: Option<RectF>,
    /// Surface visibility.
    pub visibility: SurfaceVisibility,
    /// Surface opacity (0.0 = transparent, 1.0 = opaque).
    pub opacity: f32,
    /// Opaque key identifying the scene content for this surface.
    /// The compositor does NOT own the scene; this is just an identifier.
    pub scene_key: u64,
    /// Role marker (zero-size, compile-time only).
    /// pub(crate) to allow ABI guard exhaustive destructuring.
    pub(crate) _role: PhantomData<R>,
}

impl<R: SurfaceRole> SurfaceDescriptor<R> {
    /// Check if this surface is visible and should participate in composition.
    pub fn is_visible(&self) -> bool {
        self.visibility == SurfaceVisibility::Visible && self.opacity > 0.0
    }

    /// Check if this surface fully occludes surfaces below it.
    ///
    /// A surface is fully occluding if:
    /// - It is visible
    /// - It is fully opaque (opacity == 1.0)
    /// - It has no clip (or clip covers the entire bounds)
    ///
    /// This is used for surface-level occlusion culling (INV-COMP-P06).
    pub fn is_fully_occluding(&self) -> bool {
        if !self.is_visible() {
            return false;
        }
        if self.opacity < 1.0 {
            return false;
        }
        // If there's a clip, it must cover the entire bounds
        if let Some(clip) = self.clip {
            // Check if clip fully contains bounds
            clip.left() <= self.bounds.left()
                && clip.right() >= self.bounds.right()
                && clip.top() <= self.bounds.top()
                && clip.bottom() >= self.bounds.bottom()
        } else {
            true
        }
    }
}

/// Internal mutable surface state used by SurfaceStack.
///
/// This is separate from SurfaceDescriptor to allow mutation
/// while keeping the public API immutable.
///
/// # SPEC-HAL-081C Extension
///
/// Added `parent_surface` field for overlay support.
/// Overlay-ness is structurally inferred: `parent_surface.is_some()` = overlay.
/// No runtime role enum - role is determined by construction path.
///
/// ABI FREEZE (SPEC-HAL-081A)
/// Any structural change requires a new SPEC and a new module.
/// Note: SurfaceState is compositor-internal, not part of frozen SurfaceDescriptor ABI.
#[derive(Debug, Clone)]
pub(super) struct SurfaceState {
    pub id: SurfaceId,
    pub z_index: i32,
    pub bounds: RectF,
    pub clip: Option<RectF>,
    pub visibility: SurfaceVisibility,
    pub opacity: f32,
    pub scene_key: u64,
    /// SPEC-HAL-081C: Parent surface for overlays. None for base surfaces.
    /// Overlay-ness is structurally inferred: Some(_) = overlay, None = base.
    pub parent_surface: Option<SurfaceId>,
}

impl SurfaceState {
    /// Create a new base SurfaceState with explicit scene_key.
    ///
    /// # Contract
    ///
    /// `scene_key` must be non-zero. Use `SceneKey::get()` to obtain the raw value.
    pub fn new(id: SurfaceId, z_index: i32, bounds: RectF, scene_key: u64) -> Self {
        debug_assert!(
            scene_key != 0,
            "SurfaceState scene_key must be non-zero. Use SceneKey type to enforce this."
        );
        Self {
            id,
            z_index,
            bounds,
            clip: None,
            visibility: SurfaceVisibility::Visible,
            opacity: 1.0,
            scene_key,
            parent_surface: None,
        }
    }

    /// SPEC-HAL-081C: Create a new overlay SurfaceState.
    ///
    /// # Contract
    ///
    /// - `scene_key` must be non-zero
    /// - `parent_surface` must reference a valid base surface (caller responsibility)
    /// - `z_index` must be greater than parent's z_index (caller responsibility)
    ///
    /// # Invariants
    ///
    /// - INV-SURF-ROLE-03: Overlay surfaces MUST have parent
    /// - INV-PROV-02: Parent surface MUST be a Base surface (enforced by caller)
    pub(super) fn new_overlay(
        id: SurfaceId,
        z_index: i32,
        bounds: RectF,
        scene_key: u64,
        parent_surface: SurfaceId,
    ) -> Self {
        debug_assert!(
            scene_key != 0,
            "SurfaceState scene_key must be non-zero. Use SceneKey type to enforce this."
        );
        Self {
            id,
            z_index,
            bounds,
            clip: None,
            visibility: SurfaceVisibility::Visible,
            opacity: 1.0,
            scene_key,
            parent_surface: Some(parent_surface),
        }
    }

    /// Check if this is a base surface.
    ///
    /// Structurally inferred: base surfaces have no parent.
    #[inline]
    pub fn is_base(&self) -> bool {
        self.parent_surface.is_none()
    }

    /// Check if this is an overlay surface.
    ///
    /// Structurally inferred: overlay surfaces have a parent.
    #[inline]
    pub fn is_overlay(&self) -> bool {
        self.parent_surface.is_some()
    }

    /// Convert to a base surface descriptor.
    ///
    /// # Panics (debug builds)
    ///
    /// Panics if called on an overlay surface. Use `to_overlay_descriptor()` instead.
    pub fn to_descriptor(&self) -> SurfaceDescriptor<BaseSurfaceTag> {
        debug_assert!(
            self.is_base(),
            "to_descriptor() called on overlay surface. Use to_overlay_descriptor() instead."
        );
        SurfaceDescriptor {
            id: self.id,
            z_index: self.z_index,
            bounds: self.bounds,
            clip: self.clip,
            visibility: self.visibility,
            opacity: self.opacity,
            scene_key: self.scene_key,
            _role: PhantomData,
        }
    }

    /// SPEC-HAL-081C: Convert to an overlay surface descriptor.
    ///
    /// This method is compositor-internal only. Overlay descriptors
    /// MUST NOT reach renderer code.
    ///
    /// # Panics (debug builds)
    ///
    /// Panics if called on a base surface. Use `to_descriptor()` instead.
    #[allow(
        dead_code,
        reason = "Retained role-typed compositor-only overlay projection; HAL-081C"
    )]
    pub(super) fn to_overlay_descriptor(&self) -> SurfaceDescriptor<OverlaySurfaceTag> {
        debug_assert!(
            self.is_overlay(),
            "to_overlay_descriptor() called on base surface. Use to_descriptor() instead."
        );
        SurfaceDescriptor {
            id: self.id,
            z_index: self.z_index,
            bounds: self.bounds,
            clip: self.clip,
            visibility: self.visibility,
            opacity: self.opacity,
            scene_key: self.scene_key,
            _role: PhantomData,
        }
    }
}
