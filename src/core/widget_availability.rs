use crate::core::widget::Widget;
use crate::core::widget_tree::WidgetTreeLifetimeToken;
use crate::utils::id::TreeWidgetId;
use crate::widgets::{
    button::Button, combo_box::ComboBox, text_area::TextArea, text_edit::TextEdit,
    text_field::TextField,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetAvailabilityError {
    TreeUnavailable,
    TargetUnavailable,
    UnsupportedWidget,
    ReadOnlyUnsupported,
    ObsoleteMount,
}

impl std::fmt::Display for WidgetAvailabilityError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "widget availability request rejected: {self:?}")
    }
}

impl std::error::Error for WidgetAvailabilityError {}

#[derive(Clone)]
pub(crate) struct PendingWidgetAvailability {
    pub tree_lifetime: WidgetTreeLifetimeToken,
    pub owner: TreeWidgetId,
    pub enabled: Option<bool>,
    pub read_only: Option<bool>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct EffectiveWidgetAvailability {
    pub enabled: bool,
    pub read_only: Option<bool>,
}

pub(crate) fn effective(widget: &dyn Widget) -> Option<EffectiveWidgetAvailability> {
    let any = widget.as_any();
    if let Some(button) = any.downcast_ref::<Button>() {
        return Some(EffectiveWidgetAvailability {
            enabled: button.is_enabled(),
            read_only: None,
        });
    }
    if let Some(combo) = any.downcast_ref::<ComboBox>() {
        return Some(EffectiveWidgetAvailability {
            enabled: combo.is_enabled(),
            read_only: None,
        });
    }
    if let Some(field) = any.downcast_ref::<TextField>() {
        return Some(EffectiveWidgetAvailability {
            enabled: field.is_enabled(),
            read_only: Some(field.is_read_only()),
        });
    }
    if let Some(area) = any.downcast_ref::<TextArea>() {
        return Some(EffectiveWidgetAvailability {
            enabled: area.is_enabled(),
            read_only: Some(area.is_read_only()),
        });
    }
    any.downcast_ref::<TextEdit>()
        .map(|edit| EffectiveWidgetAvailability {
            enabled: edit.is_enabled(),
            read_only: Some(edit.is_read_only()),
        })
}

pub(crate) fn publish(widget: &mut dyn Widget, policy: EffectiveWidgetAvailability) {
    if let Some(button) = widget.as_any_mut().downcast_mut::<Button>() {
        button.apply_availability_policy(policy.enabled);
    } else if let Some(combo) = widget.as_any_mut().downcast_mut::<ComboBox>() {
        combo.apply_availability_policy(policy.enabled);
    } else if let Some(field) = widget.as_any_mut().downcast_mut::<TextField>() {
        field.apply_availability_policy(policy.enabled, policy.read_only.unwrap());
    } else if let Some(area) = widget.as_any_mut().downcast_mut::<TextArea>() {
        area.apply_availability_policy(policy.enabled, policy.read_only.unwrap());
    } else if let Some(edit) = widget.as_any_mut().downcast_mut::<TextEdit>() {
        edit.apply_availability_policy(policy.enabled, policy.read_only.unwrap());
    } else {
        unreachable!("availability target prevalidated by engine");
    }
}

pub(crate) fn observe_keyboard_boundary(
    tree: &mut crate::core::widget_tree::WidgetTree,
    state: &crate::core::event::InteractionDomainState,
    event: &crate::core::event::FusedInputEvent,
) {
    if let Some(node) = state
        .focus_manager
        .focused()
        .and_then(|owner| tree.find_node_by_widget_id(owner))
        .and_then(|id| tree.node_mut(id))
    {
        if let Some(button) = node.widget.as_any_mut().downcast_mut::<Button>() {
            button.observe_availability_key_boundary(event);
        }
    }
}

#[derive(Default)]
pub(crate) struct AvailabilitySettlement {
    pub changed: Vec<TreeWidgetId>,
    pub rejected: Vec<(TreeWidgetId, WidgetAvailabilityError)>,
}

/// One frozen deferred batch. Lifecycle requests stay in the tree for the next boundary.
pub(crate) fn apply_pending_widget_availability(
    tree: &mut crate::core::widget_tree::WidgetTree,
    router: &mut crate::core::event_router::EventRouter,
    fusion: &mut crate::core::input_fusion::InputFusion,
    state: &mut crate::core::event::InteractionDomainState,
    _bindings: &mut crate::runtime::binding_registry::BindingRegistry,
    text_engine: &mut dyn crate::core::text_engine::TextEngine,
) -> AvailabilitySettlement {
    use crate::core::event::KeyboardAuthorityMode;
    let requests = tree.take_pending_widget_availability();
    let mut result = AvailabilitySettlement::default();
    let mut prepared = Vec::new();
    for request in requests {
        if !tree.is_live_mount(&request.tree_lifetime, request.owner) {
            result
                .rejected
                .push((request.owner, WidgetAvailabilityError::ObsoleteMount));
            continue;
        }
        let node = tree.find_node_by_widget_id(request.owner).unwrap();
        let Some(old) = effective(tree.node(node).unwrap().widget.as_ref()) else {
            result
                .rejected
                .push((request.owner, WidgetAvailabilityError::UnsupportedWidget));
            continue;
        };
        if request.read_only.is_some() && old.read_only.is_none() {
            result
                .rejected
                .push((request.owner, WidgetAvailabilityError::ReadOnlyUnsupported));
            continue;
        }
        let next = EffectiveWidgetAvailability {
            enabled: request.enabled.unwrap_or(old.enabled),
            read_only: request.read_only.or(old.read_only),
        };
        if next != old {
            prepared.push((request, node, old, next));
        }
    }
    let old_focus = state.focus_manager.focused();
    let loses_focus = prepared.iter().any(|(request, _, old, next)| {
        Some(request.owner) == old_focus && old.enabled && !next.enabled
    });
    for (request, node, old, next) in prepared {
        if !tree.is_live_mount(&request.tree_lifetime, request.owner) {
            result
                .rejected
                .push((request.owner, WidgetAvailabilityError::ObsoleteMount));
            continue;
        }
        if old.enabled && !next.enabled {
            router.cancel_widget_interaction_for_availability(tree, request.owner, state);
            if state.focus_manager.focused() == Some(request.owner)
                && state.keyboard_mode == KeyboardAuthorityMode::Capture
            {
                state.keyboard_mode = KeyboardAuthorityMode::Navigation;
                router.notify_capture_leave_with_availability(tree, state, text_engine, true);
            }
        }
        publish(tree.node_mut(node).unwrap().widget.as_mut(), next);
        tree.mark_paint_dirty(node);
        result.changed.push(request.owner);
    }
    if loses_focus {
        let next_focus = tree.root.and_then(|root| {
            state
                .focus_manager
                .next_after_availability_change(tree, root, old_focus.unwrap())
        });
        if let Some(next) = next_focus {
            router.request_focus_with_text_engine(tree, next, state, Some(text_engine));
        } else {
            router.clear_focus_with_text_engine(tree, state, Some(text_engine));
            state.focus_manager.retain_availability_focus_clear();
        }
        if let Some(root) = tree.root {
            state.focus_manager.rebuild_focus_chain(tree, root);
        }
    }
    let gate = crate::core::event_router::EventRouter::relative_ingress_gate(state);
    fusion.reconcile_relative_ingress(gate.as_ref());
    result
}
