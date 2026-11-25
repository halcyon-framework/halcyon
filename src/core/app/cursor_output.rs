use crate::core::event::CursorIcon;
use std::sync::mpsc::{self, Receiver, Sender, TryRecvError};

#[derive(Clone, Copy, Debug)]
enum CursorOutput {
    Apply(Option<bool>, Option<CursorIcon>),
    Restore,
}

#[cfg(test)]
mod tests {
    use super::*;
    use winit::window::CursorIcon as NativeCursor;

    #[test]
    fn dedicated_cursor_host_lock_guard_defers_shutdown_restoration() {
        let (sender, mut output) = channel();
        let worker = std::thread::spawn(move || {
            sender.publish(None, Some(CursorIcon::Text));
            sender.publish(None, Some(CursorIcon::ColResize));
            sender.publish(Some(false), None);
        });
        worker.join().unwrap();
        let mut events = Vec::new();
        output.drain_with(true, |visible, icon| events.push((visible, icon)));
        assert!(events.is_empty());
        output.drain_with(false, |visible, icon| events.push((visible, icon)));
        assert_eq!(events, [(Some(true), Some(NativeCursor::Default))]);
        events.clear();
        output.drain_with(false, |visible, icon| events.push((visible, icon)));
        assert!(events.is_empty());
    }
}

pub(super) struct CursorOutputSender(Sender<CursorOutput>);

impl CursorOutputSender {
    pub(super) fn publish(&self, visible: Option<bool>, icon: Option<CursorIcon>) {
        // A disconnected host has no remaining native window to update.
        if self.0.send(CursorOutput::Apply(visible, icon)).is_err() {
            return;
        }
    }
}

impl Drop for CursorOutputSender {
    fn drop(&mut self) {
        // Also covers unwinding: a stopped UI owner cannot retain a hidden cursor.
        if self.0.send(CursorOutput::Restore).is_err() {
            return;
        }
    }
}

/// Ordered UI-thread cursor feedback, independent of scene publication.
///
/// The native event-loop owner drains this output even when no scene is built.
/// It supplies the actual relative-lock fact; this output never owns that lease.
pub struct UiThreadCursorOutput {
    receiver: Receiver<CursorOutput>,
    restore_pending: bool,
    disconnected: bool,
}

pub(super) fn channel() -> (CursorOutputSender, UiThreadCursorOutput) {
    let (sender, receiver) = mpsc::channel();
    (
        CursorOutputSender(sender),
        UiThreadCursorOutput {
            receiver,
            restore_pending: false,
            disconnected: false,
        },
    )
}

impl UiThreadCursorOutput {
    /// Apply admitted feedback on the native window's event-loop thread.
    ///
    /// `relative_locked` must come from the existing relative-pointer owner.
    /// While locked, ordinary feedback is discarded without touching native state.
    pub fn apply_to_window(&mut self, window: &winit::window::Window, relative_locked: bool) {
        self.drain_with(relative_locked, |visible, icon| {
            if let Some(visible) = visible {
                window.set_cursor_visible(visible);
            }
            if let Some(icon) = icon {
                window.set_cursor(icon);
            }
        });
    }

    /// Restore before destroying the host window, after ending any relative lease.
    pub fn restore_window(&mut self, window: &winit::window::Window) {
        self.restore_pending = true;
        self.drain_with(false, |visible, icon| {
            if let Some(visible) = visible {
                window.set_cursor_visible(visible);
            }
            if let Some(icon) = icon {
                window.set_cursor(icon);
            }
        });
    }

    pub(super) fn drain_with(
        &mut self,
        relative_locked: bool,
        mut apply: impl FnMut(Option<bool>, Option<winit::window::CursorIcon>),
    ) {
        if !self.disconnected {
            loop {
                match self.receiver.try_recv() {
                    Ok(CursorOutput::Apply(visible, icon)) => {
                        if !relative_locked {
                            apply(visible, icon.map(crate::core::event::cursor_to_winit));
                        }
                    }
                    Ok(CursorOutput::Restore) => self.restore_pending = true,
                    Err(TryRecvError::Empty) => break,
                    Err(TryRecvError::Disconnected) => {
                        self.disconnected = true;
                        self.restore_pending = true;
                        break;
                    }
                }
            }
        }
        if self.restore_pending && !relative_locked {
            self.restore_pending = false;
            apply(Some(true), Some(winit::window::CursorIcon::Default));
        }
    }
}
