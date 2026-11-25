//! Private trusted-browser Paste intake. A request owns bounded text and exact
//! mounted/source identity; it cannot become a Key, TextInput, or another owner.

use super::*;
use crate::core::event::InteractionDomains;
use crate::core::widget::EventContext;
use crate::core::widget_tree::WidgetMountToken;
use crate::widgets::text_field::{BrowserPasteRefusal, BrowserPasteState, TextField};

/// Conversion granularity only. Every source chunk is processed; no payload quota.
pub(crate) const PASTE_SCRATCH_UNITS: usize = 4096;

#[derive(Clone, Copy)]
pub(crate) struct PasteTextPolicy {
    pub(crate) remaining_bytes: Option<usize>,
    pub(crate) normalize_newlines: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PasteRefusal {
    #[cfg(target_arch = "wasm32")]
    UntrustedSource,
    #[cfg(target_arch = "wasm32")]
    Unavailable,
    #[cfg(target_arch = "wasm32")]
    NonText,
    Empty,
    TooLarge,
    ResourceExhausted,
    InvalidUnicode,
    InvalidChunk,
    #[cfg(target_arch = "wasm32")]
    Busy,
    AvailabilityPending,
    NoCapture,
    UnsupportedTarget,
    StaleOwner,
    Editor(BrowserPasteRefusal),
}

#[derive(Clone, Copy)]
struct PasteByteCount {
    policy: PasteTextPolicy,
    bytes: usize,
    pending_cr: bool,
}

impl PasteByteCount {
    fn new(policy: PasteTextPolicy) -> Self {
        Self {
            policy,
            bytes: 0,
            pending_cr: false,
        }
    }

    fn length(&self) -> Result<usize, PasteRefusal> {
        let length = self
            .bytes
            .checked_add(usize::from(self.pending_cr))
            .filter(|length| *length <= isize::MAX as usize)
            .ok_or(PasteRefusal::TooLarge)?;
        if self.policy.remaining_bytes.is_some_and(|max| length > max) {
            return Err(PasteRefusal::TooLarge);
        }
        Ok(length)
    }

    fn append(&mut self, chunk: &str) -> Result<(), PasteRefusal> {
        for scalar in chunk.chars() {
            if self.policy.normalize_newlines {
                if self.pending_cr {
                    self.bytes = self.bytes.checked_add(1).ok_or(PasteRefusal::TooLarge)?;
                    self.pending_cr = false;
                    if scalar == '\n' {
                        self.length()?;
                        continue;
                    }
                }
                if scalar == '\r' {
                    self.pending_cr = true;
                    self.length()?;
                    continue;
                }
            }
            self.bytes = self
                .bytes
                .checked_add(scalar.len_utf8())
                .ok_or(PasteRefusal::TooLarge)?;
            self.length()?;
        }
        Ok(())
    }
}

fn checked_raw_length(current: usize, additional: usize) -> Result<usize, PasteRefusal> {
    current
        .checked_add(additional)
        .filter(|length| *length <= isize::MAX as usize)
        .ok_or(PasteRefusal::TooLarge)
}

fn allocation_refusal(_: std::collections::TryReserveError) -> PasteRefusal {
    PasteRefusal::ResourceExhausted
}

/// One fallibly allocated destination. The temporary UTF-16/UTF-8 scratch never
/// grows with total source size; downstream editor/render allocations are separate.
pub(crate) struct PasteTextBuffer {
    text: String,
    count: PasteByteCount,
}

impl PasteTextBuffer {
    pub(crate) fn new(policy: PasteTextPolicy) -> Self {
        Self {
            text: String::new(),
            count: PasteByteCount::new(policy),
        }
    }

    pub(crate) fn append_utf16_chunk(&mut self, units: &[u16]) -> Result<(), PasteRefusal> {
        if units.len() > PASTE_SCRATCH_UNITS {
            return Err(PasteRefusal::InvalidChunk);
        }
        let mut scratch = [0_u8; PASTE_SCRATCH_UNITS * 3];
        let mut used = 0;
        for scalar in char::decode_utf16(units.iter().copied()) {
            let scalar = scalar.map_err(|_| PasteRefusal::InvalidUnicode)?;
            let end = used + scalar.len_utf8();
            let space = scratch
                .get_mut(used..end)
                .ok_or(PasteRefusal::InvalidChunk)?;
            scalar.encode_utf8(space);
            used = end;
        }
        let chunk =
            std::str::from_utf8(&scratch[..used]).map_err(|_| PasteRefusal::InvalidUnicode)?;
        let mut next_count = self.count;
        next_count.append(chunk)?;
        checked_raw_length(self.text.len(), chunk.len())?;
        self.text
            .try_reserve(chunk.len())
            .map_err(allocation_refusal)?;
        self.text.push_str(chunk);
        self.count = next_count;
        Ok(())
    }

    pub(crate) fn finish(self) -> Result<String, PasteRefusal> {
        self.count.length()?;
        if self.text.is_empty() {
            Err(PasteRefusal::Empty)
        } else {
            Ok(self.text)
        }
    }
}

pub(crate) fn paste_chunk_end(start: u32, total: u32, unit_at: impl FnOnce(u32) -> u16) -> u32 {
    let end = start.saturating_add(PASTE_SCRATCH_UNITS as u32).min(total);
    if end > start && end < total && (0xd800..=0xdbff).contains(&unit_at(end - 1)) {
        end - 1
    } else {
        end
    }
}

pub(crate) struct BrowserPasteTarget {
    owner: WidgetMountToken,
    input_root: WidgetMountToken,
    root: NodeId,
    domain: InteractionDomainId,
    window: WindowId,
    source: BrowserPasteState,
}

// Deliberately neither Clone nor Debug: one delivery, no clipboard contents in logs.
pub(crate) struct BrowserPaste {
    target: BrowserPasteTarget,
    text: String,
}

impl BrowserPasteTarget {
    pub(crate) fn qualify(
        tree: &WidgetTree,
        domains: &InteractionDomains,
        window: WindowId,
        root: NodeId,
    ) -> Result<Self, PasteRefusal> {
        // The original App intake also defers all keyboard input at this barrier.
        // Clipboard data cannot be read later outside its genuine gesture.
        if tree.has_pending_widget_availability() {
            return Err(PasteRefusal::AvailabilityPending);
        }
        if domains.active_window() != Some(window) {
            return Err(PasteRefusal::NoCapture);
        }
        let domain = domains
            .domain_for_window(window)
            .ok_or(PasteRefusal::NoCapture)?;
        let state = domains.state(domain).ok_or(PasteRefusal::NoCapture)?;
        if state.keyboard_mode != KeyboardAuthorityMode::Capture
            || state.focus_manager.interaction_domain_id() != domain
        {
            return Err(PasteRefusal::NoCapture);
        }
        let owner = state
            .focus_manager
            .focused()
            .ok_or(PasteRefusal::NoCapture)?;
        let node = tree
            .find_by_id(owner)
            .and_then(|id| tree.node(id))
            .ok_or(PasteRefusal::StaleOwner)?;
        let field = node
            .widget
            .as_any()
            .downcast_ref::<TextField>()
            .ok_or(PasteRefusal::UnsupportedTarget)?;
        let source = field.browser_paste_state().map_err(PasteRefusal::Editor)?;
        if !tree.is_descendant_of_node(owner, root)
            || !node.widget.capture_eligible()
            || node.widget.physical_keyboard_capture()
        {
            return Err(PasteRefusal::NoCapture);
        }
        Ok(Self {
            owner: tree.concrete_mount(owner).ok_or(PasteRefusal::StaleOwner)?,
            input_root: tree
                .node(root)
                .and_then(|node| tree.concrete_mount(node.id))
                .ok_or(PasteRefusal::StaleOwner)?,
            root,
            domain,
            window,
            source,
        })
    }

    fn validate(
        &self,
        tree: &WidgetTree,
        domains: &InteractionDomains,
        root: NodeId,
    ) -> Result<(), PasteRefusal> {
        let current = Self::qualify(tree, domains, self.window, root)?;
        if self.root != current.root
            || self.domain != current.domain
            || self.owner != current.owner
            || self.input_root != current.input_root
            || !self.owner.is_live()
            || !self.input_root.is_live()
        {
            return Err(PasteRefusal::StaleOwner);
        }
        if self.source != current.source {
            return Err(PasteRefusal::Editor(BrowserPasteRefusal::StaleSource));
        }
        Ok(())
    }

    pub(crate) fn with_text(self, text: String) -> Result<BrowserPaste, PasteRefusal> {
        if text.is_empty() {
            return Err(PasteRefusal::Empty);
        }
        let mut count = PasteByteCount::new(self.text_policy());
        count.append(&text)?;
        Ok(BrowserPaste { target: self, text })
    }

    pub(crate) fn text_policy(&self) -> PasteTextPolicy {
        let (remaining_bytes, normalize_newlines) = self.source.text_policy();
        PasteTextPolicy {
            remaining_bytes,
            normalize_newlines,
        }
    }
}

impl BrowserPaste {
    /// Check between every original routed input, including early-exit paths.
    pub(crate) fn is_current(
        &self,
        tree: &WidgetTree,
        domains: &InteractionDomains,
        root: NodeId,
    ) -> bool {
        self.target.validate(tree, domains, root).is_ok()
    }
}

impl EventRouter {
    #[cfg(test)]
    pub(crate) fn use_browser_clipboard_transport_for_test(&mut self) {
        // The ordinary wasm Clipboard::new returns None; never touch the OS clipboard.
        self.clipboard = None;
    }

    pub(crate) fn route_browser_paste(
        &mut self,
        tree: &mut WidgetTree,
        domains: &mut InteractionDomains,
        root: NodeId,
        paste: BrowserPaste,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        animation: Option<crate::core::animation::AnimationContext<'_>>,
    ) -> Result<(), PasteRefusal> {
        let pointer = PointerId::MOUSE;
        let kind = "BrowserPaste";
        self.enter_phase(InputPipelinePhase::PreHitTest, pointer, kind);
        let admission = paste.target.validate(tree, domains, root);
        self.exit_phase(InputPipelinePhase::PreHitTest, pointer, kind);
        self.enter_phase(InputPipelinePhase::HitTest, pointer, kind);
        self.exit_phase(InputPipelinePhase::HitTest, pointer, kind);
        self.enter_phase(InputPipelinePhase::Routing, pointer, kind);
        let result = (|| {
            admission?;
            let owner = paste.target.owner.owner();
            let node_id = tree.find_by_id(owner).ok_or(PasteRefusal::StaleOwner)?;
            let tree_ptr = tree as *mut WidgetTree;
            let state = domains
                .state(paste.target.domain)
                .ok_or(PasteRefusal::StaleOwner)?;
            let node = tree.node_mut(node_id).ok_or(PasteRefusal::StaleOwner)?;
            crate::core::deferred_wiring::assert_sealed_for_phase(node.widget.as_ref(), "event");
            let field = node
                .widget
                .as_any_mut()
                .downcast_mut::<TextField>()
                .ok_or(PasteRefusal::UnsupportedTarget)?;
            let mut cx = EventContext::new(
                owner,
                tree_ptr,
                self.frame_time_ms,
                self.clipboard_ptr(),
                Some(text_engine),
                animation,
                false,
                false,
                state.keyboard_mode,
            );
            // The shared Paste operation has no focus/capture/structural effects.
            field
                .apply_browser_paste(&mut cx, paste.target.source, paste.text)
                .map_err(PasteRefusal::Editor)?;
            tree.mark_paint_dirty(node_id);
            Ok(())
        })();
        self.exit_phase(InputPipelinePhase::Routing, pointer, kind);
        self.enter_phase(InputPipelinePhase::PostProcess, pointer, kind);
        self.exit_phase(InputPipelinePhase::PostProcess, pointer, kind);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn decode_source(text: &str, policy: PasteTextPolicy) -> Result<String, PasteRefusal> {
        let units: Vec<_> = text.encode_utf16().collect();
        let mut output = PasteTextBuffer::new(policy);
        let mut start = 0;
        while start < units.len() as u32 {
            let end = paste_chunk_end(start, units.len() as u32, |i| units[i as usize]);
            output.append_utf16_chunk(&units[start as usize..end as usize])?;
            start = end;
        }
        output.finish()
    }

    #[test]
    fn browser_paste_scratch_boundaries_preserve_crlf_and_supplementary_source() {
        let crlf = format!("{}\r\nz", "a".repeat(PASTE_SCRATCH_UNITS - 1));
        let policy = PasteTextPolicy {
            remaining_bytes: Some(PASTE_SCRATCH_UNITS + 1),
            normalize_newlines: true,
        };
        assert_eq!(decode_source(&crlf, policy).unwrap(), crlf);
        assert_eq!(
            crate::core::text_edit_delegate::normalize_paste_newlines(crlf.clone()).len(),
            PASTE_SCRATCH_UNITS + 1
        );
        assert_eq!(
            decode_source(
                &crlf,
                PasteTextPolicy {
                    normalize_newlines: false,
                    ..policy
                }
            ),
            Err(PasteRefusal::TooLarge)
        );
        let supplementary = format!("{}😀z", "a".repeat(PASTE_SCRATCH_UNITS - 1));
        let units: Vec<_> = supplementary.encode_utf16().collect();
        assert_eq!(
            paste_chunk_end(0, units.len() as u32, |i| units[i as usize]),
            (PASTE_SCRATCH_UNITS - 1) as u32
        );
        assert_eq!(
            decode_source(
                &supplementary,
                PasteTextPolicy {
                    remaining_bytes: Some(supplementary.len()),
                    normalize_newlines: true
                }
            )
            .unwrap(),
            supplementary
        );
        assert_eq!(
            decode_source(
                "\r\n\r\n",
                PasteTextPolicy {
                    remaining_bytes: Some(2),
                    normalize_newlines: true
                }
            )
            .unwrap(),
            "\r\n\r\n"
        );
    }

    #[test]
    fn browser_paste_unlimited_editor_accepts_long_source_with_fallible_intake() {
        let mut output = PasteTextBuffer::new(PasteTextPolicy {
            remaining_bytes: None,
            normalize_newlines: true,
        });
        let units = [b'x' as u16; PASTE_SCRATCH_UNITS];
        for _ in 0..80 {
            output.append_utf16_chunk(&units).unwrap();
        }
        let text = output.finish().unwrap();
        assert_eq!(text.len(), 80 * PASTE_SCRATCH_UNITS);
        assert!(text.len() > 256 * 1024);
        assert!(text.bytes().all(|byte| byte == b'x'));
    }

    #[test]
    fn browser_paste_invalid_utf16_and_length_overflow_are_typed_and_atomic() {
        let policy = PasteTextPolicy {
            remaining_bytes: None,
            normalize_newlines: true,
        };
        for invalid in [[0xd800], [0xdc00]] {
            let mut output = PasteTextBuffer::new(policy);
            assert_eq!(
                output.append_utf16_chunk(&invalid),
                Err(PasteRefusal::InvalidUnicode)
            );
            assert!(output.text.is_empty());
        }
        assert_eq!(
            checked_raw_length(usize::MAX, 1),
            Err(PasteRefusal::TooLarge)
        );
        assert_eq!(
            checked_raw_length(isize::MAX as usize, 1),
            Err(PasteRefusal::TooLarge)
        );
        let mut count = PasteByteCount {
            policy,
            bytes: usize::MAX,
            pending_cr: false,
        };
        assert_eq!(count.append("x"), Err(PasteRefusal::TooLarge));
        let mut output = PasteTextBuffer::new(PasteTextPolicy {
            remaining_bytes: Some(1),
            ..policy
        });
        output.append_utf16_chunk(&[b'a' as u16]).unwrap();
        assert_eq!(
            output.append_utf16_chunk(&[b'b' as u16]),
            Err(PasteRefusal::TooLarge)
        );
        assert_eq!(output.text, "a", "no partial rejected chunk is appended");
    }

    #[test]
    fn browser_paste_allocation_error_has_resource_exhausted_type() {
        // Exercise the error mapping without attempting real heap exhaustion.
        // Production separately rejects representability before its reservation.
        let error = String::new().try_reserve(usize::MAX).unwrap_err();
        assert_eq!(allocation_refusal(error), PasteRefusal::ResourceExhausted);
    }
}
