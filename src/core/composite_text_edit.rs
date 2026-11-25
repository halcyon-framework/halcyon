//! HAL-101 §§2.4–2.5: closed, engine-owned composite editor transactions.
//!
//! The handle identifies real mounts; it never grants a Widget borrow. The
//! transaction carries owned values between separate host and child scopes.

use crate::core::widget_tree::{NodeId, WidgetMountToken, WidgetNode, WidgetTree};
use crate::utils::id::TreeWidgetId;

/// Exact tree/host/child concrete-instance association. A replacement never
/// rebinds an existing handle, even when a tree ID is preserved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeEditorId {
    pub(crate) host: TreeWidgetId,
    pub(crate) child: TreeWidgetId,
    pub(crate) host_mount: WidgetMountToken,
    pub(crate) child_mount: WidgetMountToken,
    pub(crate) relation: u64,
}

impl CompositeEditorId {
    pub fn host(&self) -> TreeWidgetId {
        self.host
    }
    pub fn child(&self) -> TreeWidgetId {
        self.child
    }
}

/// Fully owned canonical editing observation, independent of shaping storage.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompositeEditSnapshot {
    pub id: CompositeEditorId,
    pub session: u64,
    pub text: String,
    pub revision: u64,
    pub cursor: usize,
    pub selection_anchor: usize,
    pub selection_focus: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompositeEditOutcome {
    Applied,
    Unchanged,
    DeniedConsume,
    DeniedBubble,
    Stale,
    Retired,
    Unavailable,
    WrongPhase,
    Reentrant,
    OwnerContextRequired,
}

impl CompositeEditOutcome {
    pub fn applied(self) -> bool {
        matches!(self, Self::Applied | Self::Unchanged)
    }
}

/// This is a synchronous result, never a queued intent or next-frame receipt.
#[derive(Debug, Clone)]
pub struct CompositeEditReply {
    pub id: Option<CompositeEditorId>,
    pub session: Option<u64>,
    pub snapshot: Option<CompositeEditSnapshot>,
    pub outcome: CompositeEditOutcome,
    pub handled: bool,
    pub stage: u8,
    pub host_action_already_emitted: bool,
}

impl CompositeEditReply {
    fn observed(snapshot: CompositeEditSnapshot) -> Self {
        Self {
            id: Some(snapshot.id.clone()),
            session: Some(snapshot.session),
            snapshot: Some(snapshot),
            outcome: CompositeEditOutcome::Unchanged,
            handled: false,
            stage: 1,
            host_action_already_emitted: false,
        }
    }

    pub(crate) fn failure(outcome: CompositeEditOutcome) -> Self {
        Self {
            id: None,
            session: None,
            snapshot: None,
            outcome,
            handled: false,
            stage: 0,
            host_action_already_emitted: false,
        }
    }

    pub(crate) fn failure_for(id: &CompositeEditorId, outcome: CompositeEditOutcome) -> Self {
        Self {
            id: Some(id.clone()),
            ..Self::failure(outcome)
        }
    }

    pub fn applied(&self) -> bool {
        self.outcome.applied()
    }
}

#[derive(Debug, Clone)]
pub(crate) enum CompositeEditOperation {
    ForwardCurrentInput,
    PlaceTouchCaret { local_x: f32 },
    #[cfg(any(target_arch = "wasm32", test))]
    BrowserText(crate::core::event_router::browser_text::BrowserTextAction),
    ObserveAndActivity,
    SeedForEntry { text: String, select_all: bool },
    SeedForEntryThenCurrentInput { text: String, select_all: bool },
    ClearAfterTagCommand,
    ResetAfterResolverCommit { text: String, select_all: bool },
    RetireDelegation,
}

pub(crate) struct CompositeEditRequest {
    pub(crate) operation: CompositeEditOperation,
    pub(crate) continuation: crate::widgets::composite_edit_host::CompositeHostContinuation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositeEditOrigin {
    CurrentInput,
    #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
    HostChoice,
    Lifecycle,
    OwnerProperty,
}

/// Separate, truthful editing delegation. The child's actual Focus/Capture
/// appearance remains unchanged; these facts qualify only its subordinate role.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct CompositeDelegationFacts {
    pub(crate) relation: u64,
    pub(crate) session: u64,
    pub(crate) host: TreeWidgetId,
    pub(crate) presentation_revision: u64,
    pub(crate) host_facts: crate::core::appearance::FrameworkAppearanceFacts,
}

pub(crate) struct CompositeEditorRelation {
    pub(crate) id: CompositeEditorId,
    pub(crate) session: u64,
    pub(crate) delegated: bool,
    pub(crate) ready: bool,
}

#[derive(Default)]
pub(crate) struct CompositeEditorRegistry {
    pub(crate) next_relation: u64,
    pub(crate) relations: Vec<CompositeEditorRelation>,
    pub(crate) choice_owners: Vec<(TreeWidgetId, WidgetMountToken)>,
    pub(crate) manifests: std::collections::HashMap<
        TreeWidgetId,
        (
            WidgetMountToken,
            crate::engines::cell_mechanics::CompositeChoiceManifest,
        ),
    >,
}

/// Scoped post-route authority. It cannot escape the Screen closure or expose a
/// mutable peer. All operations settle before returning to its caller.
pub struct CompositeEditorOwnerContext<'a> {
    pub(crate) tree: &'a mut WidgetTree,
    pub(crate) router: &'a mut crate::core::event_router::EventRouter,
    pub(crate) domain: &'a mut crate::core::event::InteractionDomainState,
    pub(crate) text_engine: &'a mut dyn crate::core::text_engine::TextEngine,
    pub(crate) frame_time_ms: u64,
}

impl CompositeEditorOwnerContext<'_> {
    pub fn resolve(&self, host: TreeWidgetId) -> Result<CompositeEditorId, CompositeEditOutcome> {
        self.tree.composite_editor_id(host)
    }

    pub fn snapshot(
        &self,
        id: &CompositeEditorId,
    ) -> Result<CompositeEditSnapshot, CompositeEditOutcome> {
        self.tree.composite_editor_snapshot(id)
    }

    fn unchanged(&self, id: &CompositeEditorId) -> CompositeEditReply {
        match self.snapshot(id) {
            Ok(snapshot) => CompositeEditReply::observed(snapshot),
            Err(error) => CompositeEditReply::failure_for(id, error),
        }
    }

    pub fn replace_text(&mut self, id: &CompositeEditorId, text: String) -> CompositeEditReply {
        self.router.run_composite_owner_operation(
            self.tree,
            id,
            CompositeEditOperation::ResetAfterResolverCommit {
                text,
                select_all: false,
            },
            self.domain,
            self.text_engine,
            self.frame_time_ms,
        )
    }

    /// Preserve SpinBox's synchronous numeric commit and callback result while
    /// obtaining its draft only through the qualified child transaction.
    pub fn force_commit(&mut self, id: &CompositeEditorId) -> Result<i64, CompositeEditOutcome> {
        self.tree.validate_composite_editor(id)?;
        let host = self
            .tree
            .find_by_id(id.host)
            .ok_or(CompositeEditOutcome::Retired)?;
        let Some(request) = crate::widgets::composite_edit_host::force_commit_request(
            self.tree.node(host).unwrap().widget.as_ref(),
        ) else {
            return crate::widgets::composite_edit_host::committed_integer(
                self.tree.node(host).unwrap().widget.as_ref(),
            )
            .ok_or(CompositeEditOutcome::Unavailable);
        };
        let reply = self.router.run_composite_owner_request(
            self.tree,
            id,
            request,
            self.domain,
            self.text_engine,
            self.frame_time_ms,
        );
        if !reply.applied() {
            return Err(reply.outcome);
        }
        self.tree.validate_composite_editor(id)?;
        crate::widgets::composite_edit_host::committed_integer(
            self.tree.node(host).unwrap().widget.as_ref(),
        )
        .ok_or(CompositeEditOutcome::Unavailable)
    }

    pub fn sync_combo_box(
        &mut self,
        id: &CompositeEditorId,
        spec: crate::engines::cell_source::ComboBoxCellEditorSpec,
    ) -> CompositeEditReply {
        if let Err(error) = self.tree.validate_composite_editor(id) {
            return CompositeEditReply::failure_for(id, error);
        }
        let before = self.unchanged(id);
        let host = self.tree.find_by_id(id.host).unwrap();
        let (operation, options_changed) =
            match crate::widgets::composite_edit_host::prepare_combo_sync(
                self.tree.node_mut(host).unwrap().widget.as_mut(),
                &spec,
            ) {
                Ok(operation) => operation,
                Err(error) => return CompositeEditReply::failure_for(id, error),
            };
        let mut reply = if before
            .snapshot
            .as_ref()
            .is_some_and(|snapshot| snapshot.text == spec.current_value)
        {
            crate::widgets::composite_edit_host::observe(
                self.tree.node_mut(host).unwrap().widget.as_mut(),
                &before,
            );
            before
        } else {
            self.router.run_composite_owner_operation(
                self.tree,
                id,
                operation,
                self.domain,
                self.text_engine,
                self.frame_time_ms,
            )
        };
        if reply.applied() {
            if let Err(error) = self.tree.validate_composite_editor(id) {
                return CompositeEditReply::failure_for(id, error);
            }
            let input_changed = match crate::widgets::composite_edit_host::finish_combo_sync(
                self.tree.node_mut(host).unwrap().widget.as_mut(),
            ) {
                Ok(changed) => changed,
                Err(error) => return CompositeEditReply::failure_for(id, error),
            };
            if options_changed || input_changed || reply.outcome == CompositeEditOutcome::Applied {
                self.tree.mark_layout_dirty(host);
                reply.outcome = CompositeEditOutcome::Applied;
            }
        }
        reply
    }

    pub fn set_tag_items(
        &mut self,
        id: &CompositeEditorId,
        items: Vec<crate::widgets::tag_input::TagItem>,
    ) -> CompositeEditReply {
        let reply = self.unchanged(id);
        if !reply.applied() {
            return reply;
        }
        let host = self.tree.find_by_id(id.host).unwrap();
        let widget = self.tree.node_mut(host).unwrap().widget.as_mut();
        crate::widgets::composite_edit_host::observe(widget, &reply);
        if let Err(error) = crate::widgets::composite_edit_host::apply_tag_items(widget, items) {
            return CompositeEditReply::failure_for(id, error);
        }
        self.tree.mark_layout_dirty(host);
        CompositeEditReply {
            outcome: CompositeEditOutcome::Applied,
            ..reply
        }
    }

    pub fn set_tag_policy(
        &mut self,
        id: &CompositeEditorId,
        policy: crate::widgets::tag_input::TagInputPolicy,
    ) -> CompositeEditReply {
        let reply = self.router.run_composite_owner_operation(
            self.tree,
            id,
            CompositeEditOperation::RetireDelegation,
            self.domain,
            self.text_engine,
            self.frame_time_ms,
        );
        if !reply.applied() {
            return reply;
        }
        if let Err(error) = self.tree.validate_composite_editor(id) {
            return CompositeEditReply::failure_for(id, error);
        }
        let host = self.tree.find_by_id(id.host).unwrap();
        if let Err(error) = crate::widgets::composite_edit_host::apply_tag_policy(
            self.tree.node_mut(host).unwrap().widget.as_mut(),
            policy,
        ) {
            return CompositeEditReply::failure_for(id, error);
        }
        self.tree.mark_layout_dirty(host);
        CompositeEditReply {
            outcome: CompositeEditOutcome::Applied,
            ..reply
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::rect_authority::LayoutAuthorityGuard;
    use crate::widgets::{combo_box::ComboBox, flex::Flex, text_field::TextField};

    #[test]
    fn composite_editor_construction_owns_a_real_protected_child() {
        let mut tree = WidgetTree::new();
        let host = tree.create_root(
            Box::new(ComboBox::simple(vec!["one".into()]).with_smooth_caret_prototype(false)),
            None,
            0,
        );
        let id = tree
            .composite_editor_id(tree.node(host).unwrap().id())
            .unwrap();
        let child = tree.composite_child(&id).unwrap();
        assert_eq!(tree.children(host), vec![child]);
        let widget = tree.node(child).unwrap().widget();
        assert!(widget.as_any().is::<TextField>());
        assert!(
            !widget
                .as_any()
                .downcast_ref::<TextField>()
                .unwrap()
                .smooth_caret_prototype_enabled()
        );
        assert!(!widget.focusable());
        assert!(!widget.capture_eligible());
        assert!(!widget.hit_testable());
        assert!(
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = tree.node_mut(child).unwrap().widget_mut();
            }))
            .is_err()
        );
        assert!(tree.validate_composite_editor(&id).is_ok());
    }

    #[test]
    fn composite_editor_layout_lease_preserves_relation_and_child_guard() {
        let mut tree = WidgetTree::new();
        let host = tree.create_root(Box::new(ComboBox::simple(vec![])), None, 0);
        let id = tree
            .composite_editor_id(tree.node(host).unwrap().id())
            .unwrap();
        let child = tree.composite_child(&id).unwrap();
        let _layout = LayoutAuthorityGuard::enter();
        let (host_widget, _, host_rect) = tree.take_widget(host).unwrap();
        assert!(tree.validate_composite_editor(&id).is_ok());
        tree.restore_widget(host, host_widget, host_rect);
        let (child_widget, _, child_rect) = tree.take_widget(child).unwrap();
        assert!(id.child_mount.is_live());
        tree.restore_widget(child, child_widget, child_rect);
        assert!(tree.validate_composite_editor(&id).is_ok());
        assert!(tree.node(child).unwrap().composite_editor_child);
        assert_eq!(tree.composite_editor_id(id.host()).unwrap(), id);
    }

    #[test]
    fn composite_editor_replacement_and_subtree_removal_retire_original_relation() {
        let mut tree = WidgetTree::new();
        let root = tree.create_root(Box::new(Flex::column()), None, 0);
        assert!(tree.composite_editors.is_none());
        let host = tree.add_child(root, Box::new(ComboBox::simple(vec![])), None, 0);
        let host_id = tree.node(host).unwrap().id();
        let original = tree.composite_editor_id(host_id).unwrap();
        tree.replace_widget_only(host, Box::new(ComboBox::simple(vec![])));
        let successor = tree.composite_editor_id(host_id).unwrap();
        assert_ne!(original, successor);
        assert_eq!(original.host(), successor.host());
        assert_eq!(
            tree.validate_composite_editor(&original),
            Err(CompositeEditOutcome::Retired)
        );
        assert!(tree.find_by_id(original.child()).is_none());
        tree.remove_node(root);
        assert_eq!(
            tree.validate_composite_editor(&successor),
            Err(CompositeEditOutcome::Retired)
        );
        assert!(
            tree.composite_editors
                .as_ref()
                .unwrap()
                .relations
                .is_empty()
        );
    }
}

impl WidgetTree {
    pub(crate) fn assemble_composite_editor(
        &mut self,
        host_node: crate::core::widget_tree::NodeId,
    ) {
        let is_choice_owner = self
            .node_mut(host_node)
            .is_some_and(|node| node.widget.composite_choice_owner_mut().is_some());
        if is_choice_owner {
            let owner = self.node(host_node).unwrap().id;
            let mount = self.concrete_mount(owner).unwrap();
            self.composite_editors
                .get_or_insert_with(Default::default)
                .choice_owners
                .push((owner, mount));
        }
        let Some(seed) = self.node(host_node).and_then(|node| {
            crate::widgets::composite_edit_host::construction_seed(node.widget.as_ref())
        }) else {
            return;
        };
        let host = self.node(host_node).unwrap().id;
        let child_node = self.add_child(
            host_node,
            Box::new(crate::widgets::text_field::TextField::from_composite_seed(
                seed,
            )),
            None,
            0,
        );
        let child = self.node(child_node).unwrap().id;
        let host_mount = self.concrete_mount(host).expect("installed composite host");
        let child_mount = self
            .concrete_mount(child)
            .expect("installed composite child");
        let registry = self.composite_editors.get_or_insert_with(Default::default);
        registry.next_relation = registry
            .next_relation
            .checked_add(1)
            .expect("HAL-101 composite relation generation exhausted");
        let id = CompositeEditorId {
            host,
            child,
            host_mount,
            child_mount,
            relation: registry.next_relation,
        };
        registry.relations.push(CompositeEditorRelation {
            id: id.clone(),
            session: 0,
            delegated: false,
            ready: true,
        });
        self.node_mut(child_node).unwrap().composite_editor_child = true;
        let host_widget = self.node_mut(host_node).unwrap().widget.as_mut();
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        let seeded_selection = host_widget
            .as_any()
            .downcast_ref::<crate::widgets::ComboBox>()
            .filter(|combo| combo.has_seeded_display_selection())
            .map(|_| id.clone());
        let bound = crate::widgets::composite_edit_host::bind(host_widget, id, child_node);
        assert!(
            bound,
            "HAL-101 composite construction cannot publish an unbound child"
        );
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        if let Some(id) = seeded_selection {
            // Capture the new child's canonical revision before its first host projection.
            let snapshot = self
                .composite_editor_snapshot(&id)
                .expect("newly bound child");
            let reply = CompositeEditReply::observed(snapshot);
            assert!(crate::widgets::composite_edit_host::observe(
                self.node_mut(host_node).unwrap().widget.as_mut(),
                &reply,
            ));
        }
    }

    pub(crate) fn retire_composite_editor(&mut self, node: crate::core::widget_tree::NodeId) {
        let Some(owner) = self.node(node).map(|node| node.id) else {
            return;
        };
        let Some(registry) = self.composite_editors.as_mut() else {
            return;
        };
        let children: Vec<_> = registry
            .relations
            .iter()
            .filter(|relation| relation.id.host == owner)
            .map(|relation| relation.id.child)
            .collect();
        registry
            .relations
            .retain(|relation| relation.id.host != owner && relation.id.child != owner);
        registry.choice_owners.retain(|(id, _)| *id != owner);
        registry.manifests.remove(&owner);
        for child in children {
            if let Some(child) = self.find_by_id(child) {
                self.remove_node(child);
            }
        }
    }

    pub(crate) fn composite_editor_id(
        &self,
        host: TreeWidgetId,
    ) -> Result<CompositeEditorId, CompositeEditOutcome> {
        let id = self
            .composite_editors
            .as_ref()
            .and_then(|registry| {
                registry
                    .relations
                    .iter()
                    .find(|relation| relation.id.host == host)
            })
            .map(|relation| relation.id.clone())
            .ok_or(CompositeEditOutcome::Unavailable)?;
        self.validate_composite_editor(&id)?;
        Ok(id)
    }

    pub(crate) fn validate_composite_editor(
        &self,
        id: &CompositeEditorId,
    ) -> Result<(), CompositeEditOutcome> {
        if !id.host_mount.is_live() || !id.child_mount.is_live() {
            return Err(CompositeEditOutcome::Retired);
        }
        if self.concrete_mount(id.host).as_ref() != Some(&id.host_mount)
            || self.concrete_mount(id.child).as_ref() != Some(&id.child_mount)
        {
            return Err(CompositeEditOutcome::Stale);
        }
        if !self
            .composite_editors
            .as_ref()
            .is_some_and(|registry| registry.relations.iter().any(|relation| relation.id == *id))
        {
            return Err(CompositeEditOutcome::Retired);
        }
        let host = self
            .find_by_id(id.host)
            .ok_or(CompositeEditOutcome::Retired)?;
        let host_node = self.node(host).ok_or(CompositeEditOutcome::Retired)?;
        if !host_node.widget_layout_lease
            && crate::widgets::composite_edit_host::bound_id(host_node.widget.as_ref()) != Some(id)
        {
            return Err(CompositeEditOutcome::Stale);
        }
        let child = self
            .node(
                self.find_by_id(id.child)
                    .ok_or(CompositeEditOutcome::Retired)?,
            )
            .ok_or(CompositeEditOutcome::Retired)?;
        if child.parent != Some(host)
            || !child.composite_editor_child
            || !child
                .widget
                .as_any()
                .is::<crate::widgets::text_field::TextField>()
        {
            return Err(CompositeEditOutcome::Stale);
        }
        Ok(())
    }

    pub(crate) fn composite_child(
        &self,
        id: &CompositeEditorId,
    ) -> Option<crate::core::widget_tree::NodeId> {
        self.validate_composite_editor(id).ok()?;
        self.find_by_id(id.child)
    }

    pub(crate) fn composite_editor_snapshot(
        &self,
        id: &CompositeEditorId,
    ) -> Result<CompositeEditSnapshot, CompositeEditOutcome> {
        self.validate_composite_editor(id)?;
        let relation = self
            .composite_editors
            .as_ref()
            .unwrap()
            .relations
            .iter()
            .find(|relation| relation.id == *id)
            .unwrap();
        let node = self.node(self.find_by_id(id.child).unwrap()).unwrap();
        Ok(node
            .widget
            .as_any()
            .downcast_ref::<crate::widgets::text_field::TextField>()
            .unwrap()
            .composite_snapshot(id.clone(), relation.session))
    }

    #[cfg(any(target_arch = "wasm32", test))]
    pub(crate) fn active_composite_text_session(
        &self, host: TreeWidgetId,
    ) -> Option<(CompositeEditorId, u64)> {
        let id = self.composite_editor_id(host).ok()?;
        let relation = self.composite_editors.as_ref()?.relations.iter()
            .find(|relation| relation.id == id && relation.ready && relation.delegated)?;
        Some((id, relation.session))
    }

    pub(crate) fn composite_delegation(
        &self,
        child: TreeWidgetId,
        appearance: &crate::core::appearance::FrameworkAppearanceSnapshot,
    ) -> Option<CompositeDelegationFacts> {
        let relation = self
            .composite_editors
            .as_ref()?
            .relations
            .iter()
            .find(|relation| relation.id.child == child)?;
        self.validate_composite_editor(&relation.id).ok()?;
        let host_facts = appearance.for_owner(relation.id.host);
        let host = self
            .find_by_id(relation.id.host)
            .and_then(|host| self.node(host))?;
        if !crate::widgets::composite_edit_host::editing_policy(host.widget.as_ref())?.enabled {
            return None;
        }
        if !crate::widgets::composite_edit_host::active(host.widget.as_ref()) {
            return None;
        }
        if !relation.ready
            || !relation.delegated
            || !host_facts.keyboard_captured()
            || !host_facts.window_active()
            || !host_facts.interaction_domain_active()
        {
            return None;
        }
        Some(CompositeDelegationFacts {
            relation: relation.id.relation,
            session: relation.session,
            host: relation.id.host,
            presentation_revision: crate::widgets::composite_edit_host::presentation_revision(
                host.widget.as_ref(),
            ),
            host_facts,
        })
    }

    pub(crate) fn run_composite_child(
        &mut self,
        id: &CompositeEditorId,
        operation: CompositeEditOperation,
        cx: &mut crate::core::widget::EventContext,
        event: Option<&crate::core::event::Event>,
    ) -> CompositeEditReply {
        if let Err(error) = self.validate_composite_editor(id) {
            return CompositeEditReply::failure_for(id, error);
        }
        if cx.widget_id != id.child {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::Stale);
        }
        if !cx
            .composite_edit_authority()
            .is_some_and(|(authority, _, _)| authority == id)
        {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::WrongPhase);
        }
        if cx.composite_edit_origin() == Some(CompositeEditOrigin::CurrentInput)
            && (!cx.composite_edit_has_capture() || !self.composite_editor_ready(id))
        {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::Unavailable);
        }
        #[cfg(any(target_arch = "wasm32", target_os = "ios", test))]
        if cx.composite_edit_origin() == Some(CompositeEditOrigin::HostChoice)
            && (!self.composite_editor_ready(id)
                || !matches!(operation, CompositeEditOperation::ObserveAndActivity
                    | CompositeEditOperation::ResetAfterResolverCommit { .. }))
        {
            return CompositeEditReply::failure_for(id, CompositeEditOutcome::Unavailable);
        }
        let policy = self
            .node(self.find_by_id(id.host).unwrap())
            .and_then(|node| {
                crate::widgets::composite_edit_host::editing_policy(node.widget.as_ref())
            });
        let before = self.composite_editor_snapshot(id).unwrap();
        let relation = self
            .composite_editors
            .as_mut()
            .unwrap()
            .relations
            .iter_mut()
            .find(|relation| relation.id == *id)
            .unwrap();
        match &operation {
            CompositeEditOperation::SeedForEntry { .. }
            | CompositeEditOperation::SeedForEntryThenCurrentInput { .. } => {
                relation.session = relation
                    .session
                    .checked_add(1)
                    .expect("HAL-101 session generation exhausted");
                relation.delegated = true;
            }
            CompositeEditOperation::RetireDelegation => {
                relation.session = relation
                    .session
                    .checked_add(1)
                    .expect("HAL-101 session generation exhausted");
                relation.delegated = false;
            }
            CompositeEditOperation::ForwardCurrentInput | CompositeEditOperation::PlaceTouchCaret { .. }
                if cx.composite_edit_origin() == Some(CompositeEditOrigin::CurrentInput) =>
            {
                relation.delegated = true;
            }
            _ => {}
        }
        let session = relation.session;
        let child = self.find_by_id(id.child).unwrap();
        let field = self
            .node_mut(child)
            .unwrap()
            .widget
            .as_any_mut()
            .downcast_mut::<crate::widgets::text_field::TextField>()
            .unwrap();
        if let Some(policy) = policy {
            if field.is_enabled() != policy.enabled || field.is_read_only() != policy.read_only {
                field.apply_availability_policy(policy.enabled, policy.read_only);
            }
        }
        let (outcome, handled) = field.run_composite_operation(operation, cx, event);
        let snapshot = field.composite_snapshot(id.clone(), session);
        let changed = before.text != snapshot.text
            || before.cursor != snapshot.cursor
            || before.selection_anchor != snapshot.selection_anchor
            || before.selection_focus != snapshot.selection_focus;
        if changed {
            self.mark_layout_dirty(child);
        }
        CompositeEditReply {
            id: Some(id.clone()),
            session: Some(session),
            snapshot: Some(snapshot),
            outcome: if outcome == CompositeEditOutcome::Applied && !changed {
                CompositeEditOutcome::Unchanged
            } else {
                outcome
            },
            handled,
            stage: 1,
            host_action_already_emitted: false,
        }
    }

    pub(crate) fn composite_editor_ready(&self, id: &CompositeEditorId) -> bool {
        self.validate_composite_editor(id).is_ok()
            && self.composite_editors.as_ref().is_some_and(|registry| {
                registry
                    .relations
                    .iter()
                    .any(|relation| relation.id == *id && relation.ready)
            })
    }

    pub(crate) fn set_composite_editor_ready(
        &mut self,
        id: &CompositeEditorId,
        ready: bool,
    ) -> Result<(), CompositeEditOutcome> {
        self.validate_composite_editor(id)?;
        let relation = self
            .composite_editors
            .as_mut()
            .unwrap()
            .relations
            .iter_mut()
            .find(|relation| relation.id == *id)
            .unwrap();
        let changed = relation.ready != ready;
        relation.ready = ready;
        if !ready {
            relation.delegated = false;
            if changed {
                relation.session = relation
                    .session
                    .checked_add(1)
                    .expect("HAL-101 composite session generation exhausted");
            }
        }
        if changed {
            if let Some(host) = self.find_by_id(id.host) {
                if !ready
                    && let Some(node) = self.node_mut(host)
                    && node.composite_choice_host
                    && let Some(combo) = node
                        .widget
                        .as_any_mut()
                        .downcast_mut::<crate::widgets::combo_box::ComboBox>()
                {
                    combo.retire_cell_choice_popup_input();
                }
                self.mark_layout_dirty(host);
                self.mark_subtree_paint_dirty(host);
            }
        }
        Ok(())
    }

    /// Only registered cell Choice mounts have an ordinary dispatch readiness gate.
    pub(crate) fn composite_choice_input_ready(&self, node: &WidgetNode) -> bool {
        let host = if node.composite_editor_child {
            node.parent.and_then(|parent| self.node(parent))
        } else {
            Some(node)
        };
        let Some(host) = host.filter(|host| host.composite_choice_host) else {
            return true;
        };
        crate::widgets::composite_edit_host::bound_id(host.widget.as_ref())
            .is_some_and(|id| id.host == host.id && self.composite_editor_ready(id))
    }

    pub(crate) fn composite_presentation_visible(&self, node_id: NodeId) -> bool {
        let Some(node) = self.node(node_id) else {
            return false;
        };
        if node.composite_choice_host {
            // A retained offscreen editor has no current cell placement. Its
            // interaction pin by itself confers no inline or overlay output.
            return self.composite_choice_input_ready(node)
                && self
                    .get_rect(node_id)
                    .is_some_and(|rect| rect.width > 0.0 && rect.height > 0.0);
        }
        !node.composite_editor_child || self.composite_child_presentation_visible(node.id)
    }

    pub(crate) fn composite_child_presentation_visible(&self, child: TreeWidgetId) -> bool {
        let Some(node) = self.find_by_id(child).and_then(|node| self.node(node)) else {
            return false;
        };
        if !node.composite_editor_child {
            return true;
        }
        let Some(relation) = self.composite_editors.as_ref().and_then(|registry| {
            registry
                .relations
                .iter()
                .find(|relation| relation.id.child == child)
        }) else {
            return false;
        };
        self.composite_editor_ready(&relation.id)
            && self
                .find_by_id(relation.id.host)
                .and_then(|host| self.node(host))
                .is_some_and(|host| {
                    crate::widgets::composite_edit_host::visible(host.widget.as_ref())
                })
    }

    pub(crate) fn record_composite_choice_manifest(
        &mut self,
        owner_node: crate::core::widget_tree::NodeId,
        manifest: crate::engines::cell_mechanics::CompositeChoiceManifest,
    ) {
        let Some(owner) = self.node(owner_node).map(|node| node.id) else {
            return;
        };
        let Some(mount) = self.concrete_mount(owner) else {
            return;
        };
        let Some(registry) = self.composite_editors.as_mut() else {
            return;
        };
        if registry
            .choice_owners
            .iter()
            .any(|(id, token)| *id == owner && *token == mount)
        {
            registry.manifests.insert(owner, (mount, manifest));
        }
    }

    pub(crate) fn process_composite_choice_work(
        &mut self,
        router: &mut crate::core::event_router::EventRouter,
        domain: &mut crate::core::event::InteractionDomainState,
        text_engine: &mut dyn crate::core::text_engine::TextEngine,
        frame_time_ms: u64,
    ) {
        use crate::engines::cell_mechanics::{
            CompositeChoiceMountFacts, CompositeChoiceSettlement as Settlement,
            CompositeChoiceWork as Work,
        };
        let Some(registry) = self.composite_editors.as_ref() else {
            return;
        };
        if registry.choice_owners.is_empty() {
            return;
        }
        let owners = registry.choice_owners.clone();
        // Keyboard Capture belongs to the focused owner. Both it and plain
        // Focus protect the exact subtree, together with current pointer Capture.
        let interaction_owners: Vec<_> = domain
            .pointer_capture_manager
            .active_pointers()
            .into_iter()
            .filter_map(|pointer| domain.pointer_capture_manager.current_capture(pointer))
            .chain(domain.focus_manager.focused())
            .collect();
        for (owner, mount) in owners {
            if self.concrete_mount(owner).as_ref() != Some(&mount) {
                continue;
            }
            let Some(owner_node) = self.find_by_id(owner) else {
                continue;
            };
            let manifest = self
                .composite_editors
                .as_mut()
                .unwrap()
                .manifests
                .remove(&owner)
                .and_then(|(token, manifest)| (token == mount).then_some(manifest));
            let targets = self
                .node_mut(owner_node)
                .and_then(|node| node.widget.composite_choice_owner_mut())
                .map(|mut source| source.owner_mut().mounted_composite_choice_targets())
                .unwrap_or_default();
            let mounts = targets
                .into_iter()
                .filter_map(|(role, target)| {
                    let host = self.find_by_id(target.host)?;
                    let node = self.node(host)?;
                    if node.parent != Some(owner_node)
                        || !node.composite_choice_host
                        || self.validate_composite_editor(&target).is_err()
                    {
                        return None;
                    }
                    let pinned = interaction_owners
                        .iter()
                        .any(|owner| self.is_descendant_of_node(*owner, host))
                        || self.composite_choice_has_active_editor(&target, node);
                    Some((role, CompositeChoiceMountFacts { target, pinned }))
                })
                .collect();
            let work = self
                .node_mut(owner_node)
                .unwrap()
                .widget
                .composite_choice_owner_mut()
                .map(|mut source| {
                    source.owner_mut().collect_composite_choice_work(manifest, &mounts)
                })
                .unwrap_or_default();
            // The source Widget/provider borrow has ended. Each work item is an
            // owned value and each structural/data operation requalifies owner.
            for work in work {
                if self.concrete_mount(owner).as_ref() != Some(&mount) {
                    break;
                }
                let settlement = match work {
                    Work::Mount { role, key, widget } => {
                        let node = self.add_child(owner_node, widget, Some(key), 0);
                        let host = self.node(node).unwrap().id;
                        if let Err(refusal) = self.qualify_cell_editor(owner_node, node, role.row_id, role.column_id) {
                            tracing::warn!(?refusal, ?role, "HAL391 cell choice remains standalone");
                        }
                        match self.composite_editor_id(host) {
                            Ok(target) => {
                                self.node_mut(node).unwrap().composite_choice_host = true;
                                Settlement::Mounted {
                                    role,
                                    node,
                                    owner: host,
                                    target,
                                }
                            }
                            Err(outcome) => {
                                self.remove_node(node);
                                Settlement::Rejected { role, outcome }
                            }
                        }
                    }
                    Work::Sync {
                        role,
                        target,
                        spec,
                        factory,
                    } => {
                        let mut cx = CompositeEditorOwnerContext {
                            tree: self,
                            router,
                            domain,
                            text_engine,
                            frame_time_ms,
                        };
                        let reply = factory.sync_combo_box_editor(&mut cx, &target, spec);
                        if let Some(node) = cx.tree.find_by_id(target.host) {
                            if let Err(refusal) = cx.tree.qualify_cell_editor(owner_node, node, role.row_id, role.column_id) {
                                tracing::warn!(?refusal, ?role, "HAL391 cell choice remains standalone");
                            }
                        }
                        let _ = cx.tree.set_composite_editor_ready(&target, reply.applied());
                        Settlement::Synced {
                            role,
                            outcome: reply.outcome,
                        }
                    }
                    Work::Remove {
                        role,
                        node,
                        owner: target_owner,
                        target,
                    } => {
                        if self.validate_composite_editor(&target).is_ok()
                            && self.node(node).is_some_and(|node| {
                                node.id == target_owner && node.parent == Some(owner_node)
                            })
                        {
                            self.remove_node(node);
                            Settlement::Removed { role }
                        } else {
                            Settlement::Rejected {
                                role,
                                outcome: CompositeEditOutcome::Stale,
                            }
                        }
                    }
                    Work::NonReady {
                        role,
                        target,
                        reason,
                    } => {
                        if let Some(target) = target {
                            let _ = self.set_composite_editor_ready(&target, false);
                        }
                        Settlement::NonReady { role, reason }
                    }
                };
                if self.concrete_mount(owner).as_ref() != Some(&mount) {
                    break;
                }
                if let Some(mut source) = self
                    .node_mut(owner_node)
                    .and_then(|node| node.widget.composite_choice_owner_mut())
                {
                    source.owner_mut().settle_composite_choice_work(settlement);
                }
            }
        }
    }

    fn composite_choice_has_active_editor(
        &self,
        target: &CompositeEditorId,
        node: &WidgetNode,
    ) -> bool {
        use crate::widgets::{combo_box::ComboBox, composite_edit_host};
        self.composite_editors.as_ref().is_some_and(|registry| {
            registry.relations.iter().any(|relation| {
                relation.id == *target
                    && relation.ready
                    // Text editing and IME are subordinate to the live
                    // delegation. A visible input configuration alone is inert.
                    && ((relation.delegated && composite_edit_host::active(node.widget.as_ref()))
                        || node
                            .widget
                            .as_any()
                            .downcast_ref::<ComboBox>()
                            .is_some_and(|combo| combo.popup_input_revision().is_some()))
            })
        })
    }
}
