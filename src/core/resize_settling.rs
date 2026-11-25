//! Resize Settling — Per-surface resize coalescing and deferred reconfiguration.
//!
//! # Problem
//!
//! Window resize generates a burst of `Resized` events. Each event currently triggers:
//! - render target resize
//! - compositor surface bounds update
//! - full redraw
//!
//! This causes jank during drag resize.
//!
//! # Solution
//!
//! Defer expensive reconfiguration until settling conditions are met:
//! - `AboutToWait` event (preferred — all pending resizes coalesced)
//! - Size delta exceeds threshold (immediate apply for large jumps)
//!
//! # Binding Constraints
//!
//! - No WindowManager authority
//! - No compositor state ownership
//! - No cross-window coupling
//! - No behavior change outside resize paths
//! - Deterministic delivery

use crate::compositor::SurfaceId;
use std::collections::HashMap;

/// Threshold for immediate resize application (pixels).
/// If size delta exceeds this in either axis, apply immediately.
pub const RESIZE_IMMEDIATE_THRESHOLD_PX: u32 = 8;

/// Per-surface resize state.
#[derive(Debug, Clone, Copy)]
pub struct SurfaceResizeState {
    /// Pending size waiting to be applied (physical pixels).
    pub pending_size: Option<(u32, u32)>,
    /// Last successfully applied size (physical pixels).
    pub last_applied_size: (u32, u32),
    /// Whether resize is currently in progress (between first Resized and apply).
    pub resize_in_progress: bool,
}

impl SurfaceResizeState {
    /// Create initial state for a surface with given size.
    pub fn new(initial_size: (u32, u32)) -> Self {
        Self {
            pending_size: None,
            last_applied_size: initial_size,
            resize_in_progress: false,
        }
    }

    /// Record a pending resize. Does NOT apply immediately.
    ///
    /// Returns `true` if immediate apply is recommended (threshold exceeded).
    pub fn record_pending(&mut self, new_size: (u32, u32)) -> bool {
        self.pending_size = Some(new_size);
        self.resize_in_progress = true;

        // Check if threshold exceeded for immediate apply
        let (last_w, last_h) = self.last_applied_size;
        let (new_w, new_h) = new_size;
        let delta_w = (new_w as i32 - last_w as i32).unsigned_abs();
        let delta_h = (new_h as i32 - last_h as i32).unsigned_abs();

        delta_w > RESIZE_IMMEDIATE_THRESHOLD_PX || delta_h > RESIZE_IMMEDIATE_THRESHOLD_PX
    }

    /// Check if there's a pending resize that differs from last applied.
    pub fn needs_apply(&self) -> bool {
        match self.pending_size {
            Some(pending) => pending != self.last_applied_size,
            None => false,
        }
    }

    /// Mark resize as applied. Call after successful reconfiguration.
    pub fn mark_applied(&mut self) {
        if let Some(size) = self.pending_size.take() {
            self.last_applied_size = size;
        }
        self.resize_in_progress = false;
    }

    /// Get the pending size if any.
    pub fn pending(&self) -> Option<(u32, u32)> {
        self.pending_size
    }

    /// Check if resize is in progress.
    pub fn in_progress(&self) -> bool {
        self.resize_in_progress
    }
}

/// Manager for per-surface resize settling.
///
/// Tracks resize state for multiple surfaces independently.
/// No cross-surface coupling — each surface settles independently.
#[derive(Debug, Default)]
pub struct ResizeSettlingManager {
    surfaces: HashMap<SurfaceId, SurfaceResizeState>,
}

impl ResizeSettlingManager {
    /// Create a new manager.
    pub fn new() -> Self {
        Self {
            surfaces: HashMap::new(),
        }
    }

    /// Register a surface with its initial size.
    pub fn register(&mut self, surface_id: SurfaceId, initial_size: (u32, u32)) {
        self.surfaces
            .insert(surface_id, SurfaceResizeState::new(initial_size));
    }

    /// Unregister a surface.
    pub fn unregister(&mut self, surface_id: SurfaceId) {
        self.surfaces.remove(&surface_id);
    }

    /// Record a pending resize for a surface.
    ///
    /// Returns `true` if immediate apply is recommended (threshold exceeded).
    pub fn record_pending(&mut self, surface_id: SurfaceId, new_size: (u32, u32)) -> bool {
        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            state.record_pending(new_size)
        } else {
            // Surface not registered — apply immediately (fallback)
            true
        }
    }

    /// Check if a surface needs resize applied.
    pub fn needs_apply(&self, surface_id: SurfaceId) -> bool {
        self.surfaces
            .get(&surface_id)
            .map_or(false, |s| s.needs_apply())
    }

    /// Get pending size for a surface.
    pub fn pending(&self, surface_id: SurfaceId) -> Option<(u32, u32)> {
        self.surfaces.get(&surface_id).and_then(|s| s.pending())
    }

    /// Mark resize as applied for a surface.
    pub fn mark_applied(&mut self, surface_id: SurfaceId) {
        if let Some(state) = self.surfaces.get_mut(&surface_id) {
            state.mark_applied();
        }
    }

    /// Check if any surface has resize in progress.
    pub fn any_in_progress(&self) -> bool {
        self.surfaces.values().any(|s| s.in_progress())
    }

    /// Check if a specific surface has resize in progress.
    pub fn in_progress(&self, surface_id: SurfaceId) -> bool {
        self.surfaces
            .get(&surface_id)
            .map_or(false, |s| s.in_progress())
    }

    /// Get all surfaces currently in resize-active state.
    pub fn surfaces_in_progress(&self) -> Vec<SurfaceId> {
        self.surfaces
            .iter()
            .filter(|(_, state)| state.in_progress())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Get all surfaces that need resize applied.
    pub fn surfaces_needing_apply(&self) -> Vec<SurfaceId> {
        self.surfaces
            .iter()
            .filter(|(_, state)| state.needs_apply())
            .map(|(id, _)| *id)
            .collect()
    }

    /// Apply all pending resizes. Returns list of (surface_id, new_size) that were applied.
    ///
    /// Call this from `AboutToWait` or similar settling point.
    /// The caller is responsible for actually calling resize_render_target and set_surface_bounds.
    pub fn drain_pending(&mut self) -> Vec<(SurfaceId, (u32, u32))> {
        let mut applied = Vec::new();
        for (id, state) in self.surfaces.iter_mut() {
            if state.needs_apply() {
                if let Some(size) = state.pending_size {
                    applied.push((*id, size));
                    state.mark_applied();
                }
            }
        }
        applied
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_surface_resize_state_basic() {
        let mut state = SurfaceResizeState::new((800, 600));
        assert!(!state.needs_apply());
        assert!(!state.in_progress());

        // Record pending resize (delta=4px, within RESIZE_IMMEDIATE_THRESHOLD_PX=8)
        let immediate = state.record_pending((804, 604));
        assert!(!immediate); // Within threshold
        assert!(state.needs_apply());
        assert!(state.in_progress());

        // Mark applied
        state.mark_applied();
        assert!(!state.needs_apply());
        assert!(!state.in_progress());
        assert_eq!(state.last_applied_size, (804, 604));
    }

    #[test]
    fn test_surface_resize_threshold() {
        let mut state = SurfaceResizeState::new((800, 600));

        // Small delta — no immediate apply
        let immediate = state.record_pending((805, 605));
        assert!(!immediate);

        // Large delta — immediate apply recommended
        state.mark_applied();
        let immediate = state.record_pending((900, 700));
        assert!(immediate);
    }

    #[test]
    fn test_manager_multi_surface() {
        let mut manager = ResizeSettlingManager::new();
        let surface1 = SurfaceId::test_id(1);
        let surface2 = SurfaceId::test_id(2);

        manager.register(surface1, (800, 600));
        manager.register(surface2, (400, 300));

        // Record pending for surface1 only
        manager.record_pending(surface1, (810, 610));
        assert!(manager.needs_apply(surface1));
        assert!(!manager.needs_apply(surface2));

        // Drain pending
        let applied = manager.drain_pending();
        assert_eq!(applied.len(), 1);
        assert_eq!(applied[0], (surface1, (810, 610)));

        // After drain, nothing pending
        assert!(!manager.needs_apply(surface1));
    }
}
