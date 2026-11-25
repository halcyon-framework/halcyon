//! SPEC-HAL-056: Platform Clipboard Abstraction
//!
//! Provides cross-platform clipboard access for copy/paste operations.
//! Uses arboard on desktop and UIKit on iOS.

#[cfg(all(not(target_os = "ios"), not(target_arch = "wasm32")))]
use arboard::Clipboard as ArboardClipboard;

/// Platform clipboard handle.
///
/// Wraps the OS clipboard API for text copy/paste operations.
/// Thread-local due to arboard requirements on some platforms.
pub struct Clipboard {
    #[cfg(test)]
    memory_text: Option<String>,
    #[cfg(all(not(target_os = "ios"), not(target_arch = "wasm32")))]
    inner: ArboardClipboard,
    #[cfg(target_os = "ios")]
    inner: objc2::rc::Retained<objc2_ui_kit::UIPasteboard>,
    #[cfg(target_os = "ios")]
    _main_thread: objc2_foundation::MainThreadMarker,
}

impl Clipboard {
    /// Create a new clipboard handle.
    ///
    /// Returns None if clipboard access is unavailable (e.g., no display server).
    pub fn new() -> Option<Self> {
        #[cfg(target_arch = "wasm32")]
        {
            tracing::warn!("Synchronous platform clipboard is unavailable in the browser");
            None
        }
        #[cfg(all(not(target_os = "ios"), not(target_arch = "wasm32")))]
        {
            ArboardClipboard::new().ok().map(|inner| Self {
                inner,
                #[cfg(test)]
                memory_text: None,
            })
        }
        #[cfg(target_os = "ios")]
        {
            let main_thread = objc2_foundation::MainThreadMarker::new()?;
            Some(Self {
                #[cfg(test)]
                memory_text: None,
                // UIKit access is confined to the main thread by the retained marker.
                inner: unsafe { objc2_ui_kit::UIPasteboard::generalPasteboard() },
                _main_thread: main_thread,
            })
        }
    }

    /// Get text from the clipboard.
    ///
    /// Returns None if clipboard is empty or contains non-text data.
    pub fn get_text(&mut self) -> Option<String> {
        #[cfg(test)]
        if let Some(text) = self.memory_text.as_ref() {
            return Some(text.clone());
        }
        #[cfg(target_arch = "wasm32")]
        {
            None
        }
        #[cfg(all(not(target_os = "ios"), not(target_arch = "wasm32")))]
        {
            self.inner.get_text().ok()
        }
        #[cfg(target_os = "ios")]
        {
            unsafe { self.inner.string() }.map(|text| text.to_string())
        }
    }

    /// Set text to the clipboard.
    ///
    /// Returns true if successful, false otherwise.
    pub fn set_text(&mut self, text: &str) -> bool {
        #[cfg(test)]
        if let Some(stored) = self.memory_text.as_mut() {
            *stored = text.to_owned();
            return true;
        }
        #[cfg(target_arch = "wasm32")]
        {
            let _ = text;
            false
        }
        #[cfg(all(not(target_os = "ios"), not(target_arch = "wasm32")))]
        {
            self.inner.set_text(text.to_string()).is_ok()
        }
        #[cfg(target_os = "ios")]
        {
            let text = objc2_foundation::NSString::from_str(text);
            unsafe { self.inner.setString(Some(&text)) };
            true
        }
    }

    #[cfg(test)]
    pub(crate) fn use_memory_transport_for_test(&mut self, initial: &str) {
        self.memory_text = Some(initial.to_owned());
    }
}

impl std::fmt::Debug for Clipboard {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Clipboard").finish_non_exhaustive()
    }
}
