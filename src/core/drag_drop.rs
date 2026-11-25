//! SPEC-HAL-241: Drag and Drop session management.
//!
//! Centralized drag state, owned by the interaction domain.
//! One active session at a time per INV-241.1.

use std::any::{Any, TypeId};

use crate::core::math::{Point, Size};
use crate::utils::id::TreeWidgetId;

/// Active drag session. One at a time per interaction domain.
pub struct DragSession {
    pub source_widget: TreeWidgetId,
    pub payload: Box<dyn Any>,
    pub payload_type: TypeId,
    pub current_target: Option<TreeWidgetId>,
    pub origin: Point,
    pub current_pos: Point,
    /// Layout size of the source widget at drag start
    pub source_size: Size,
    /// Smoothed display position for ghost rendering (trails cursor)
    pub display_pos: Point,
    /// Ghost alpha [0.0, 1.0] — fades in on start, fades out on end
    pub ghost_alpha: f32,
    /// Whether the session is in the fade-out phase
    pub fading_out: bool,
}

impl DragSession {
    pub fn new(
        source: TreeWidgetId,
        payload: Box<dyn Any>,
        origin: Point,
        source_size: Size,
    ) -> Self {
        let payload_type = (*payload).type_id();
        Self {
            source_widget: source,
            payload,
            payload_type,
            current_target: None,
            origin,
            current_pos: origin,
            source_size,
            display_pos: origin,
            ghost_alpha: 0.0,
            fading_out: false,
        }
    }

    /// Per-frame update. Call from frame loop with frame dt.
    /// Smooths display_pos toward current_pos and ramps alpha.
    pub fn update(&mut self, dt: f32) {
        // Exponential smoothing: display_pos trails current_pos
        // Speed 18.0 gives ~60ms effective lag — fast enough to feel responsive,
        // slow enough to see trailing on fast moves.
        let factor = 1.0 - (-dt * 18.0_f32).exp();
        self.display_pos.x += (self.current_pos.x - self.display_pos.x) * factor;
        self.display_pos.y += (self.current_pos.y - self.display_pos.y) * factor;

        // Alpha ramp
        if self.fading_out {
            // Fade out: 80ms
            self.ghost_alpha = (self.ghost_alpha - dt / 0.08).max(0.0);
        } else {
            // Fade in: 80ms
            self.ghost_alpha = (self.ghost_alpha + dt / 0.08).min(1.0);
        }
    }

    /// Returns true when fade-out is complete (session can be fully removed)
    pub fn is_fade_complete(&self) -> bool {
        self.fading_out && self.ghost_alpha <= 0.001
    }
}

/// Drag/drop state manager. Lives in the interaction domain.
pub struct DragDropState {
    session: Option<DragSession>,
}

impl DragDropState {
    pub fn new() -> Self {
        Self { session: None }
    }

    pub fn start(&mut self, session: DragSession) {
        debug_assert!(
            self.session.is_none(),
            "Cannot start drag while another is active"
        );
        self.session = Some(session);
    }

    pub fn active(&self) -> bool {
        self.session.is_some()
    }

    pub fn session(&self) -> Option<&DragSession> {
        self.session.as_ref()
    }

    pub fn session_mut(&mut self) -> Option<&mut DragSession> {
        self.session.as_mut()
    }

    /// End the drag session, returning the session for payload delivery.
    pub fn end(&mut self) -> Option<DragSession> {
        self.session.take()
    }

    /// Cancel without delivering payload. Returns the current target (if any)
    /// so the caller can fire on_drag_leave.
    pub fn cancel(&mut self) -> Option<TreeWidgetId> {
        let target = self.session.as_ref().and_then(|s| s.current_target);
        self.session = None;
        target
    }

    /// Begin fade-out after drop/cancel. Payload is consumed, but session stays for ghost.
    pub fn begin_fade_out(&mut self) -> Option<Box<dyn Any>> {
        if let Some(session) = &mut self.session {
            session.fading_out = true;
            session.current_target = None;
            // Take payload so it's not held during fade-out
            let payload = std::mem::replace(&mut session.payload, Box::new(()));
            Some(payload)
        } else {
            None
        }
    }

    /// Check if fade-out is complete and remove session if so.
    /// Call from frame loop.
    pub fn cleanup_if_faded(&mut self) {
        if self
            .session
            .as_ref()
            .map_or(false, |s| s.is_fade_complete())
        {
            self.session = None;
        }
    }
}

impl Default for DragDropState {
    fn default() -> Self {
        Self::new()
    }
}
