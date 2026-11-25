#![cfg(target_os = "macos")]

use std::sync::Arc;
use std::time::Instant;

use winit::dpi::LogicalPosition;
use winit::window::{CursorGrabMode, Window};

use crate::core::event::{PointerId, RawInputEvent, WindowId};
use crate::core::math::Point;
use crate::core::relative_pointer::{
    RelativePointerIngressGate, RelativePointerNormalizationEpoch,
};
use crate::platform::relative_pointer::{
    RelativePointerPlatform, RelativePointerPlatformError, RelativePointerPlatformSupport,
};

pub(crate) trait MacOsRelativePointerWindowOps {
    fn set_locked(&self, locked: bool) -> Result<(), RelativePointerPlatformError>;
    fn set_visible(&self, visible: bool);
    fn restore(&self, anchor: Point) -> Result<(), RelativePointerPlatformError>;
}

impl MacOsRelativePointerWindowOps for Arc<Window> {
    fn set_locked(&self, locked: bool) -> Result<(), RelativePointerPlatformError> {
        let mode = if locked {
            CursorGrabMode::Locked
        } else {
            CursorGrabMode::None
        };
        self.set_cursor_grab(mode).map_err(|_| {
            if locked {
                RelativePointerPlatformError::Denied
            } else {
                RelativePointerPlatformError::FailedPlatformOperation
            }
        })
    }

    fn set_visible(&self, visible: bool) {
        self.set_cursor_visible(visible);
    }

    fn restore(&self, anchor: Point) -> Result<(), RelativePointerPlatformError> {
        self.set_cursor_position(LogicalPosition::new(anchor.x as f64, anchor.y as f64))
            .map_err(|_| RelativePointerPlatformError::FailedPlatformOperation)
    }
}

pub(crate) fn refresh_cursor_rects_after_focus(window: &Window) -> bool {
    use objc2_app_kit::NSView;
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};

    if !objc2_foundation::is_main_thread() {
        return false;
    }
    let Ok(handle) = window.window_handle() else {
        return false;
    };
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return false;
    };
    // The borrowed window handle keeps its NSView alive for this main-thread callback.
    let view = unsafe { &*handle.ns_view.as_ptr().cast::<NSView>() };
    let Some(ns_window) = view.window() else {
        return false;
    };
    ns_window.invalidateCursorRectsForView(view);
    true
}

pub(crate) struct MacOsRelativePointerPlatform<W: MacOsRelativePointerWindowOps> {
    window_id: WindowId,
    window: W,
    locked: bool,
    hidden: bool,
}

impl<W: MacOsRelativePointerWindowOps> MacOsRelativePointerPlatform<W> {
    pub(crate) fn new(window_id: WindowId, window: W) -> Self {
        Self {
            window_id,
            window,
            locked: false,
            hidden: false,
        }
    }

    fn owns_window(&self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        if window_id == self.window_id {
            Ok(())
        } else {
            Err(RelativePointerPlatformError::Unsupported)
        }
    }
}

impl<W: MacOsRelativePointerWindowOps> RelativePointerPlatform for MacOsRelativePointerPlatform<W> {
    fn support(&self, window_id: WindowId) -> RelativePointerPlatformSupport {
        if window_id != self.window_id {
            return RelativePointerPlatformSupport::Unsupported;
        }
        // Epoch 1 identifies the audited winit 0.30.12 NSEvent point-unit
        // identity path. A change to that provenance requires a new audit and epoch.
        RelativePointerPlatformSupport::Supported {
            normalization_epoch: RelativePointerNormalizationEpoch::new(1)
                .expect("the pinned macOS normalization epoch is nonzero"),
            restore_down_anchor: true,
        }
    }

    fn acquire(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.owns_window(window_id)?;
        if self.locked {
            return Err(RelativePointerPlatformError::Denied);
        }
        self.window.set_locked(true)?;
        self.locked = true;
        Ok(())
    }

    fn release(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.owns_window(window_id)?;
        if self.locked {
            self.window.set_locked(false)?;
            self.locked = false;
        }
        Ok(())
    }

    fn hide_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.owns_window(window_id)?;
        if !self.locked {
            return Err(RelativePointerPlatformError::Denied);
        }
        if !self.hidden {
            self.window.set_visible(false);
            self.hidden = true;
        }
        Ok(())
    }

    fn show_cursor(&mut self, window_id: WindowId) -> Result<(), RelativePointerPlatformError> {
        self.owns_window(window_id)?;
        if self.hidden {
            self.window.set_visible(true);
            self.hidden = false;
        }
        Ok(())
    }

    fn restore_cursor(
        &mut self,
        window_id: WindowId,
        anchor: Point,
    ) -> Result<(), RelativePointerPlatformError> {
        self.owns_window(window_id)?;
        if !self.locked || !self.hidden || !anchor.x.is_finite() || !anchor.y.is_finite() {
            return Err(RelativePointerPlatformError::FailedPlatformOperation);
        }
        // The pinned winit macOS warp may re-associate cursor and mouse. The
        // following release remains idempotent and runs while still hidden.
        self.window.restore(anchor)
    }
}

pub(crate) fn relative_motion_from_device_event(
    gate: Option<RelativePointerIngressGate>,
    focused_window_id: Option<WindowId>,
    bound_window_id: WindowId,
    delta: (f64, f64),
    time: Instant,
) -> Option<RawInputEvent> {
    let gate = gate?;
    if focused_window_id != Some(bound_window_id)
        || gate.window_id() != bound_window_id
        || gate.pointer_id() != PointerId::MOUSE
        || !delta.0.is_finite()
        || !delta.1.is_finite()
    {
        return None;
    }
    let logical_delta = (delta.0 as f32, delta.1 as f32);
    if !logical_delta.0.is_finite() || !logical_delta.1.is_finite() {
        return None;
    }
    Some(RawInputEvent::RelativePointerMotion {
        lease_id: gate.lease_id(),
        normalization_epoch: gate.normalization_epoch(),
        window_id: gate.window_id(),
        pointer_id: gate.pointer_id(),
        time,
        delta: logical_delta,
    })
}
