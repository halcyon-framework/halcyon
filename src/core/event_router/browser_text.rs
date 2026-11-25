//! HAL-398. A browser observes a projection of one engine-owned edit session.

use super::EventRouter;
use crate::core::event::{
    Event, EventPhase, ImeEvent, InputPipelinePhase, InteractionDomainId, InteractionDomains,
    KeyboardAuthorityMode, Modifiers, PointerId, PointerRouteOrigin, TextInputEvent, WindowId,
};
use crate::core::{event::RawInputEvent, math::Size};
use std::rc::Rc;
use crate::core::text_engine::TextEngine;
use crate::core::widget::{EventContext, Widget};
use crate::core::widget_tree::{NodeId, WidgetMountToken, WidgetNode, WidgetTree};
use crate::utils::id::TreeWidgetId;
use crate::widgets::{combo_box::ComboBox, text_area::TextArea, text_field::TextField};
use unicode_segmentation::{GraphemeCursor, UnicodeSegmentation};

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum BrowserTextKind {
    Line,
    Multiline,
    Password,
}

/// The copy in the host is confined to the selection and its local context.
/// Password projections have the same boundaries, without the original text.
#[derive(PartialEq, Eq)]
pub(crate) struct BrowserTextProjection {
    pub(crate) value: String,
    pub(crate) selection: (u32, u32),
    pub(crate) kind: BrowserTextKind,
    pub(crate) composing: bool,
    revision: u64,
    document_selection: (usize, usize),
    // Byte offsets in the host value and in the actual document.
    boundaries: Vec<(usize, usize)>,
}

#[derive(Clone)]
pub(crate) enum BrowserTextAction {
    Select {
        anchor: usize,
        head: usize,
    },
    Replace {
        start: usize,
        end: usize,
        text: String,
    },
    Cut,
    Composition(ImeEvent),
}

impl std::fmt::Debug for BrowserTextAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("BrowserTextAction { .. }")
    }
}

impl BrowserTextProjection {
    pub(crate) fn new(
        document: &str,
        revision: u64,
        selection: (usize, usize),
        kind: BrowserTextKind,
        composing: bool,
    ) -> Option<Self> {
        let (anchor, head) = selection;
        let low = anchor.min(head);
        let high = anchor.max(head);
        if !document.is_char_boundary(low) || !document.is_char_boundary(high) {
            return None;
        }
        const CONTEXT_BYTES: usize = 128;
        let low = grapheme_floor(document, low)?;
        let high = if anchor == head {
            low
        } else {
            grapheme_ceil(document, high)?
        };
        let first = grapheme_ceil(document, low.saturating_sub(CONTEXT_BYTES))?.min(low);
        let last = grapheme_floor(document, high.saturating_add(CONTEXT_BYTES))?.max(high);
        let mut value = String::new();
        let mut boundaries = Vec::new();
        let mut units = 0_u32;
        let mut add_cluster = |source, cluster: &str, masked| {
            boundaries.push((value.len(), source));
            let text = if masked { "•" } else { cluster };
            value.push_str(text);
            units += text.encode_utf16().count() as u32;
            units
        };
        let mask = kind == BrowserTextKind::Password;
        let mut lower = 0;
        for (relative, cluster) in document[first..low].grapheme_indices(true) {
            lower = add_cluster(first + relative, cluster, mask);
        }
        let mut upper = lower;
        if high - low > CONTEXT_BYTES {
            // A large selection needs two positions, not a DOM copy of its bytes.
            upper = add_cluster(low, "\u{fffc}", mask);
        } else {
            for (relative, cluster) in document[low..high].grapheme_indices(true) {
                upper = add_cluster(low + relative, cluster, mask);
            }
        }
        for (relative, cluster) in document[high..last].grapheme_indices(true) {
            add_cluster(high + relative, cluster, mask);
        }
        boundaries.push((value.len(), last));
        Some(Self {
            value,
            selection: if anchor <= head {
                (lower, upper)
            } else {
                (upper, lower)
            },
            kind,
            composing,
            revision,
            document_selection: selection,
            boundaries,
        })
    }

    pub(crate) fn select(&self, anchor: u32, head: u32) -> Option<BrowserTextAction> {
        let source = |units| {
            let offset = utf16_to_byte(&self.value, units)?;
            self.boundaries
                .iter()
                .rev()
                .find(|&&(projected, _)| projected <= offset)
                .map(|&(_, source)| source)
        };
        Some(BrowserTextAction::Select {
            anchor: source(anchor)?,
            head: source(head)?,
        })
    }

    /// Reconcile an actual after-input observation. Apply whole source
    /// graphemes even if the host removed only part of a cluster.
    fn reconcile_at(
        &self,
        value: &str,
        deletion: bool,
        hint: Option<u32>,
    ) -> Option<BrowserTextAction> {
        if self.value == value {
            return None;
        }
        let prefix = self
            .value
            .chars()
            .zip(value.chars())
            .take_while(|(old, new)| old == new)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let prefix = prefix.min(
            hint.and_then(|units| utf16_to_byte(&self.value, units))
                .unwrap_or(prefix),
        );
        let suffix = self.value[prefix..]
            .chars()
            .rev()
            .zip(value[prefix..].chars().rev())
            .take_while(|(old, new)| old == new)
            .map(|(c, _)| c.len_utf8())
            .sum::<usize>();
        let (from, start) = self
            .boundaries
            .iter()
            .rev()
            .find(|&&(projected, _)| projected <= prefix)
            .copied()?;
        let (to, end) = self
            .boundaries
            .iter()
            .find(|&&(projected, _)| projected >= self.value.len() - suffix)
            .copied()?;
        let inserted = if deletion {
            String::new()
        } else {
            let new_end = value.len().checked_sub(self.value.len() - to)?;
            value.get(from..new_end)?.to_owned()
        };
        Some(BrowserTextAction::Replace {
            start,
            end,
            text: inserted,
        })
    }
}

fn grapheme_floor(document: &str, position: usize) -> Option<usize> {
    let mut position = position.min(document.len());
    while !document.is_char_boundary(position) {
        position -= 1;
    }
    let mut cursor = GraphemeCursor::new(position, document.len(), true);
    if cursor.is_boundary(document, 0).ok()? {
        Some(position)
    } else {
        cursor.prev_boundary(document, 0).ok()?.or(Some(0))
    }
}

fn grapheme_ceil(document: &str, position: usize) -> Option<usize> {
    let floor = grapheme_floor(document, position)?;
    if floor >= position || floor == document.len() {
        return Some(floor);
    }
    GraphemeCursor::new(floor, document.len(), true)
        .next_boundary(document, 0)
        .ok()?
        .or(Some(document.len()))
}

fn utf16_to_byte(value: &str, position: u32) -> Option<usize> {
    let mut units = 0;
    for (offset, scalar) in value.char_indices() {
        if units == position {
            return Some(offset);
        }
        units += scalar.len_utf16() as u32;
        if units > position {
            return None;
        }
    }
    (units == position).then_some(value.len())
}

#[derive(PartialEq, Eq)]
pub(crate) struct BrowserTextTarget {
    window: WindowId,
    domain: InteractionDomainId,
    root: WidgetMountToken,
    owner: WidgetMountToken,
    editor: WidgetMountToken,
    composite_session: Option<u64>,
    focus_epoch: u64,
}

pub(crate) struct BrowserTextSession {
    pub(crate) target: BrowserTextTarget,
    pub(crate) projection: BrowserTextProjection,
    before_input: Option<(u64, (usize, usize))>,
    composition_source: Option<(u64, (usize, usize))>,
    composition_tail: bool,
    lease: Rc<()>,
}

/// Facts from the last engine projection offered to this specific proxy lease.
pub(crate) struct BrowserTextSource {
    lease: Rc<()>,
    revision: u64,
    selection: (usize, usize),
}

/// Transport facts only. The App admits these in its ordinary input order.
pub(crate) enum BrowserTextReport {
    BeforeInput,
    Input {
        value: String,
        kind: String,
        composing: bool,
        selection: Option<(u32, u32)>,
    },
    PreEdit(String),
    Commit(String),
    Select(u32, u32),
    Key {
        key: RawInputEvent,
        shortcut_text: Option<RawInputEvent>,
    },
    Blur,
    Viewport(Option<Size>),
}

impl BrowserTextSession {
    pub(crate) fn new(target: BrowserTextTarget, projection: BrowserTextProjection) -> Self {
        Self {
            target,
            projection,
            before_input: None,
            composition_source: None,
            composition_tail: false,
            lease: Rc::new(()),
        }
    }

    pub(crate) fn source(&self) -> BrowserTextSource {
        BrowserTextSource {
            lease: self.lease.clone(),
            revision: self.projection.revision,
            selection: self.projection.document_selection,
        }
    }

    pub(crate) fn owns_source(&self, source: &BrowserTextSource) -> bool {
        Rc::ptr_eq(&self.lease, &source.lease)
    }

    pub(crate) fn before_input(&mut self) {
        self.before_input = Some((self.projection.revision, self.projection.document_selection));
    }

    pub(crate) fn update_composition(&mut self, text: String) -> BrowserTextAction {
        self.composition_tail = false;
        self.before_input = None;
        self.composition_source
            .get_or_insert((self.projection.revision, self.projection.document_selection));
        BrowserTextAction::Composition(ImeEvent::PreEdit { text, cursor: None })
    }

    pub(crate) fn end_composition(&mut self, text: String) -> Option<BrowserTextAction> {
        let source = self.composition_source.take();
        if !self.projection.composing {
            return None;
        }
        self.composition_tail = true;
        self.before_input = None;
        let current = (self.projection.revision, self.projection.document_selection);
        Some(BrowserTextAction::Composition(
            if text.is_empty() || source != Some(current) {
                ImeEvent::Cancel
            } else {
                ImeEvent::Commit { text }
            },
        ))
    }

    pub(crate) fn after_input(
        &mut self,
        value: &str,
        kind: &str,
        composing: bool,
        selection: Option<(u32, u32)>,
    ) -> Option<BrowserTextAction> {
        let before = self.before_input.take();
        if before.is_some_and(|source| {
            source != (self.projection.revision, self.projection.document_selection)
        }) {
            return None;
        }
        if composing || self.projection.composing {
            return None;
        }
        let after_composition = std::mem::take(&mut self.composition_tail);
        if (after_composition && before.is_none())
            || matches!(
                kind,
                "insertCompositionText"
                    | "insertFromComposition"
                    | "insertFromPaste"
                    | "insertFromDrop"
                    | "deleteByCut"
            )
        {
            return None;
        }
        let deletion = kind.starts_with("delete");
        let hint = match kind {
            "insertText" | "insertLineBreak" | "insertParagraph" => {
                Some(self.projection.selection.0.min(self.projection.selection.1))
            }
            _ if deletion => selection.map(|(anchor, head)| anchor.min(head)),
            _ => None,
        };
        self.projection.reconcile_at(value, deletion, hint)
    }
}

impl BrowserTextSource {
    pub(crate) fn matches_projection(&self, projection: &BrowserTextProjection) -> bool {
        (self.revision, self.selection) == (projection.revision, projection.document_selection)
    }
}

impl BrowserTextTarget {
    fn focused_editor<'a>(
        tree: &'a WidgetTree,
        domains: &InteractionDomains,
        window: WindowId,
        root: NodeId,
    ) -> Option<(InteractionDomainId, u64, &'a WidgetNode)> {
        if domains.active_window() != Some(window) || tree.has_pending_widget_availability() {
            return None;
        }
        let domain = domains.domain_for_window(window)?;
        let state = domains.state(domain)?;
        if state.keyboard_mode != KeyboardAuthorityMode::Capture {
            return None;
        }
        let owner = state.focus_manager.focused()?;
        let node = tree.node(tree.find_by_id(owner)?)?;
        if !tree.is_descendant_of_node(owner, root)
            || !node.widget.capture_eligible()
            || node.widget.physical_keyboard_capture()
            || !tree.composite_choice_input_ready(node)
        {
            return None;
        }
        Some((domain, state.focus_manager.browser_focus_epoch(), node))
    }

    pub(crate) fn qualify(
        tree: &WidgetTree,
        domains: &InteractionDomains,
        window: WindowId,
        root: NodeId,
    ) -> Option<(Self, BrowserTextProjection)> {
        let (domain, focus_epoch, node) = Self::focused_editor(tree, domains, window, root)?;
        let owner = node.id;
        let mut editor = owner;
        let mut composite_session = None;
        let projection = if let Some(field) = node.widget.as_any().downcast_ref::<TextField>() {
            field.browser_text_projection()?
        } else if let Some(area) = node.widget.as_any().downcast_ref::<TextArea>() {
            area.browser_text_projection()?
        } else {
            let combo = node.widget.as_any().downcast_ref::<ComboBox>()?;
            if !combo.composite_active() {
                return None;
            }
            let (id, session) = tree.active_composite_text_session(owner)?;
            editor = id.child();
            composite_session = Some(session);
            tree.node(tree.composite_child(&id)?)?
                .widget
                .as_any()
                .downcast_ref::<TextField>()?
                .browser_text_projection()?
        };
        Some((
            Self {
                window,
                domain,
                root: tree.concrete_mount(tree.node(root)?.id)?,
                owner: tree.concrete_mount(owner)?,
                editor: tree.concrete_mount(editor)?,
                composite_session,
                focus_epoch,
            },
            projection,
        ))
    }

    pub(crate) fn owner(&self) -> TreeWidgetId {
        self.owner.owner()
    }

    pub(crate) fn copy_selection<'a>(
        tree: &'a WidgetTree,
        domains: &InteractionDomains,
        window: WindowId,
        root: NodeId,
        proxy_session: Option<&BrowserTextSession>,
    ) -> Option<&'a str> {
        let node = if let Some(session) = proxy_session {
            if session.target.window != window || session.target.composite_session.is_some() {
                return None;
            }
            let current = session.target.current_projection(tree, domains, root)?;
            if !session.source().matches_projection(&current) {
                return None;
            }
            tree.node(tree.find_by_id(session.target.owner())?)?
        } else {
            // Read-only fields can own selection and Copy on canvas. They
            // do not need an editable DOM projection to authorize that read.
            Self::focused_editor(tree, domains, window, root)?.2
        };
        node.widget
            .as_any()
            .downcast_ref::<TextField>()?
            .browser_clipboard_selection(false)
    }

    pub(crate) fn current_projection(
        &self,
        tree: &WidgetTree,
        domains: &InteractionDomains,
        root: NodeId,
    ) -> Option<BrowserTextProjection> {
        let (target, projection) = Self::qualify(tree, domains, self.window, root)?;
        (self == &target).then_some(projection)
    }
}

impl EventRouter {
    pub(crate) fn route_browser_cut(
        &mut self,
        tree: &mut WidgetTree,
        domains: &mut InteractionDomains,
        window: WindowId,
        root: NodeId,
        proxy_session: Option<&BrowserTextSession>,
        text_engine: &mut dyn TextEngine,
        transfer: impl FnOnce(&str) -> bool,
    ) -> bool {
        let Some((target, before)) = BrowserTextTarget::qualify(tree, domains, window, root) else {
            return false;
        };
        if proxy_session.is_some_and(|session| {
            session.target != target || !session.source().matches_projection(&before)
        }) {
            return false;
        }
        let Some(selection) = tree
            .find_by_id(target.owner())
            .and_then(|id| tree.node(id))
            .and_then(|node| node.widget.as_any().downcast_ref::<TextField>())
            .and_then(|field| field.browser_clipboard_selection(true))
        else {
            return false;
        };
        if !transfer(selection) {
            return false;
        }
        // The ordinary browser edit route requalifies the exact mount, focus
        // epoch, revision and selection before dispatching the one Cut.
        self.route_browser_text(
            tree, domains, root, &target, &before, &BrowserTextAction::Cut, text_engine,
        )
        .is_some_and(|after| after.revision != before.revision)
    }

    pub(crate) fn take_touch_keyboard_activation(
        &mut self,
    ) -> Option<(crate::core::event::QualifiedTouchMotion, TreeWidgetId)> {
        self.touch_keyboard_activation.take()
    }

    pub(crate) fn route_browser_text(
        &mut self,
        tree: &mut WidgetTree,
        domains: &mut InteractionDomains,
        root: NodeId,
        target: &BrowserTextTarget,
        before: &BrowserTextProjection,
        action: &BrowserTextAction,
        text_engine: &mut dyn TextEngine,
    ) -> Option<BrowserTextProjection> {
        let current = target.current_projection(tree, domains, root)?;
        if current != *before {
            return Some(current);
        }
        let state = domains.state_mut(target.domain)?;
        let pointer = PointerId::MOUSE;
        let kind = "BrowserText";
        self.enter_phase(InputPipelinePhase::PreHitTest, pointer, kind);
        self.exit_phase(InputPipelinePhase::PreHitTest, pointer, kind);
        self.enter_phase(InputPipelinePhase::HitTest, pointer, kind);
        self.exit_phase(InputPipelinePhase::HitTest, pointer, kind);
        self.enter_phase(InputPipelinePhase::Routing, pointer, kind);
        let event = match action {
            BrowserTextAction::Composition(ime) => Event::Ime(ime.clone()),
            _ => Event::Text(TextInputEvent {
                text: String::new(),
                modifiers: Modifiers::default(),
            }),
        };
        self.dispatch_widget_input(
            tree,
            target.owner(),
            Some(&event),
            EventPhase::Target,
            PointerRouteOrigin::Regular,
            Some(text_engine as *mut _),
            state,
            None,
            super::WidgetInput::BrowserText(action.clone()),
        );
        self.exit_phase(InputPipelinePhase::Routing, pointer, kind);
        self.enter_phase(InputPipelinePhase::PostProcess, pointer, kind);
        self.exit_phase(InputPipelinePhase::PostProcess, pointer, kind);
        target.current_projection(tree, domains, root)
    }

    pub(crate) fn request_browser_caret_reveal(&mut self, target: &BrowserTextTarget) {
        self.pending_focus_reveal = Some(target.owner());
    }
}

pub(super) fn dispatch_text(
    widget: &mut dyn Widget,
    cx: &mut EventContext,
    action: BrowserTextAction,
) -> bool {
    if let Some(field) = widget.as_any_mut().downcast_mut::<TextField>() {
        return field.handle_browser_text(cx, action);
    }
    if let Some(area) = widget.as_any_mut().downcast_mut::<TextArea>() {
        return area.handle_browser_text(cx, action);
    }
    widget
        .as_any_mut()
        .downcast_mut::<ComboBox>()
        .is_some_and(|combo| combo.handle_browser_text(cx, action))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn hal398_after_input_preserves_graphemes_and_password_boundaries() {
        let source = "a👩‍🚀e\u{301}z";
        let projection =
            BrowserTextProjection::new(source, 0, (12, 12), BrowserTextKind::Line, false).unwrap();
        assert!(matches!(projection.reconcile_at("ae\u{301}z", true, None),
            Some(BrowserTextAction::Replace { start: 1, end: 12, text }) if text.is_empty()));
        assert!(matches!(projection.reconcile_at("a👩‍🚀éz", false, None),
            Some(BrowserTextAction::Replace { start: 12, end: 15, text }) if text == "é"));
        assert!(
            projection.select(2, 2).is_none(),
            "a surrogate pair is indivisible"
        );
        let password =
            BrowserTextProjection::new(source, 0, (12, 12), BrowserTextKind::Password, false)
                .unwrap();
        assert_eq!(password.value, "••••");
        assert_eq!(password.selection, (2, 2));
        assert!(matches!(
            password.select(1, 2),
            Some(BrowserTextAction::Select {
                anchor: 1,
                head: 12
            })
        ));
        let document = "a".repeat(2048);
        let projection = BrowserTextProjection::new(
            &document,
            0,
            (0, document.len()),
            BrowserTextKind::Multiline,
            false,
        )
        .unwrap();
        assert_eq!(projection.value, "\u{fffc}");
        assert!(
            matches!(projection.reconcile_at("replacement", false, Some(0)),
            Some(BrowserTextAction::Replace { start: 0, end: 2048, text }) if text == "replacement")
        );
    }
}
