//! HAL-400. A host presents choices, while the original ComboBox resolves them.

use super::{EventRouter, InputPipelinePhase, WidgetInput};
use crate::core::animation::AnimationContext;
use crate::core::event::{
    EventPhase, InteractionDomainState, PointerRouteOrigin, QualifiedTouchMotion,
    TouchPressIdentity, TouchTarget, WindowId,
};
use crate::core::math::{Point, RectF};
use crate::core::text_engine::TextEngine;
use crate::core::widget_tree::{NodeId, WidgetMountToken, WidgetTree};
use crate::model::selection_resolver::OptionId;
#[cfg(any(target_os = "ios", test))]
use crate::widgets::combo_box::ChoiceDisplayRow;
use crate::widgets::combo_box::{ChoiceGroups, ComboBox};
use std::sync::Arc;

pub(crate) struct ChoiceOptions<'a> {
    pub revision: u64,
    pub items: &'a [(OptionId, String)],
    pub groups: Option<&'a Arc<ChoiceGroups>>,
    pub selected: Option<OptionId>,
    pub popup_open: bool,
    #[cfg(any(target_arch = "wasm32", test))]
    pub affordance: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceTarget {
    pub mount: WidgetMountToken,
    pub revision: u64,
}

#[derive(Debug, Clone, PartialEq)]
#[cfg(any(target_arch = "wasm32", test))]
pub(crate) struct ChoicePlacement {
    pub target: ChoiceTarget,
    pub rect: RectF,
    pub exposed: Vec<RectF>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ChoiceLease {
    pub press: TouchPressIdentity,
    pub target: ChoiceTarget,
}

pub(crate) struct ChoiceRequest {
    pub lease: ChoiceLease,
    #[cfg(any(target_arch = "wasm32", test))]
    pub projected: ChoiceTarget,
    pub items: Vec<(OptionId, String)>,
    #[cfg_attr(
        target_arch = "wasm32",
        allow(
            dead_code,
            reason = "HAL-401 grouped snapshot; Web SELECT already projects ChoiceOptions at the qualified revision"
        )
    )]
    pub groups: Option<Arc<ChoiceGroups>>,
    pub selected: Option<OptionId>,
}

#[cfg(any(target_os = "ios", test))]
impl ChoiceRequest {
    pub(crate) fn picker_row_count(&self) -> usize {
        self.groups
            .as_ref()
            .map_or(self.items.len(), |groups| groups.row_count())
    }

    pub(crate) fn picker_row(&self, row: isize) -> Option<ChoiceDisplayRow> {
        let row = usize::try_from(row).ok()?;
        if let Some(groups) = &self.groups {
            groups.row(row)
        } else {
            (row < self.items.len()).then_some(ChoiceDisplayRow::Item(row))
        }
    }

    pub(crate) fn picker_row_title(&self, row: ChoiceDisplayRow) -> &str {
        match row {
            ChoiceDisplayRow::Header(heading) => self.groups.as_ref().unwrap().title(heading),
            ChoiceDisplayRow::Item(item) => &self.items[item].1,
        }
    }

    pub(crate) fn initial_picker_row(&self) -> usize {
        let item = self
            .selected
            .and_then(|selected| self.items.iter().position(|(id, _)| *id == selected))
            .unwrap_or(0);
        self.groups
            .as_ref()
            .map_or(item, |groups| groups.item_row(item))
    }

    pub(crate) fn picker_reply(&self, row: Option<isize>) -> ChoiceReply {
        let selection = match row.and_then(|row| self.picker_row(row)) {
            Some(ChoiceDisplayRow::Item(item)) => Some(self.items[item].0),
            Some(ChoiceDisplayRow::Header(_)) | None => None,
        };
        ChoiceReply {
            lease: self.lease.clone(),
            selection,
        }
    }
}

pub(crate) struct ChoiceReply {
    pub lease: ChoiceLease,
    pub selection: Option<OptionId>,
}

pub(crate) enum HostOffer {
    #[cfg(any(target_os = "ios", test))]
    Native(WindowId),
    #[cfg(any(target_arch = "wasm32", test))]
    RenderedSelect { contact: u64, target: ChoiceTarget },
    #[cfg(target_arch = "wasm32")]
    MissingTrustedSelect,
}

struct ActiveChoice {
    lease: ChoiceLease,
    root: NodeId,
    root_rect: RectF,
    geometry: crate::core::event::PendingTouchTap,
    at: Point,
    focus_epoch: u64,
}

#[derive(Default)]
pub(super) struct HostChoice {
    offer: Option<HostOffer>,
    active: Option<ActiveChoice>,
    request: Option<ChoiceRequest>,
}

impl EventRouter {
    /// A browser offer is scoped to the native SELECT's actual trusted Up.
    pub(crate) fn set_host_choice_offer(&mut self, offer: Option<HostOffer>) {
        self.host_choice.offer = offer;
    }

    pub(crate) fn take_host_choice_request(&mut self) -> Option<ChoiceRequest> {
        self.host_choice.request.take()
    }

    pub(crate) fn retire_host_choice(&mut self, lease: &ChoiceLease) {
        if self
            .host_choice
            .active
            .as_ref()
            .is_some_and(|active| &active.lease == lease)
        {
            self.host_choice.active = None;
            self.host_choice.request = None;
        }
    }

    pub(crate) fn retire_host_choice_window(&mut self, window: WindowId) {
        if self
            .host_choice
            .active
            .as_ref()
            .is_some_and(|active| active.lease.press.window == window)
        {
            self.host_choice.active = None;
            self.host_choice.request = None;
        }
    }

    pub(super) fn request_host_choice(
        &mut self,
        tree: &mut WidgetTree,
        root: Option<NodeId>,
        state: &mut InteractionDomainState,
        pending: &crate::core::event::PendingTouchTap,
        motion: QualifiedTouchMotion,
        text_engine: &mut dyn TextEngine,
    ) -> bool {
        let Some(root) = root.or_else(|| tree.root()) else {
            return false;
        };
        let Some(root_rect) = tree.get_absolute_paint_rect(root) else {
            return false;
        };
        if pending.target != TouchTarget::Control
            || pending.origin != PointerRouteOrigin::Regular
            || state.focus_manager.focused() != Some(pending.mount.owner())
        {
            return false;
        }
        let Some(options) = choice_options(tree, &pending.mount) else {
            return false;
        };
        #[cfg(any(target_arch = "wasm32", test))]
        let projected = ChoiceTarget {
            mount: pending.mount.clone(),
            revision: options.revision,
        };
        let admitted = match self.host_choice.offer.as_ref() {
            #[cfg(any(target_os = "ios", test))]
            Some(HostOffer::Native(window)) => *window == motion.press.window,
            #[cfg(any(target_arch = "wasm32", test))]
            Some(HostOffer::RenderedSelect {
                contact,
                target: offered,
            }) => motion.native_contact == *contact && &projected == offered,
            #[cfg(target_arch = "wasm32")]
            Some(HostOffer::MissingTrustedSelect) => false,
            None => return false,
        };
        if !admitted || !motion.first_contact {
            // The offered surface owns this touch. An unqualified release
            // cannot switch presentations or activate its native default.
            return true;
        }

        let items = options.items.to_vec();
        let groups = options.groups.cloned();
        let selected = options.selected;
        let owner = pending.mount.owner();
        if state.keyboard_mode == crate::core::event::KeyboardAuthorityMode::Capture {
            self.handle_engine_action(
                tree,
                crate::runtime::actions::ActionId::RELEASE_CAPTURE,
                Some(root),
                state,
                text_engine,
                None,
            );
        }
        tree.dismiss_nonmodal_popup(owner);
        let Some(current) = choice_options(tree, &pending.mount) else {
            return true;
        };
        if current.popup_open
            || current.items != items
            || current.groups != groups.as_ref()
            || current.selected != selected
            || state.focus_manager.focused() != Some(owner)
        {
            return true;
        }
        let target = ChoiceTarget {
            mount: pending.mount.clone(),
            revision: current.revision,
        };
        let mut geometry = pending.clone();
        geometry.popup_revision = Some(current.revision);
        let lease = ChoiceLease {
            press: motion.press,
            target,
        };
        self.host_choice.active = Some(ActiveChoice {
            lease: lease.clone(),
            root,
            root_rect,
            geometry,
            at: motion.position,
            focus_epoch: state.focus_manager.browser_focus_epoch(),
        });
        self.host_choice.request = Some(ChoiceRequest {
            lease,
            #[cfg(any(target_arch = "wasm32", test))]
            projected,
            items,
            groups,
            selected,
        });
        true
    }

    pub(crate) fn current_host_choice(
        &mut self,
        tree: &WidgetTree,
        root: NodeId,
        state: &InteractionDomainState,
        window: WindowId,
    ) -> Option<&ChoiceLease> {
        let active = self.host_choice.active.as_ref()?;
        if active.lease.press.window != window {
            return None;
        }
        let owner = active.lease.target.mount.owner();
        let current = root == active.root
            && tree.get_absolute_paint_rect(root) == Some(active.root_rect)
            && active.lease.press.domain == state.focus_manager.interaction_domain_id()
            && state.focus_manager.focused() == Some(owner)
            && state.focus_manager.browser_focus_epoch() == active.focus_epoch
            && choice_options(tree, &active.lease.target.mount).is_some_and(|options| {
                !options.popup_open && options.revision == active.lease.target.revision
            })
            && tree
                .touch_dismissible_popup(active.at, Some(owner))
                .is_none()
            && tree
                .hit_test_for_routing(active.at.x, active.at.y)
                .and_then(|(hit, origin)| {
                    Self::touch_tap_at(tree, &hit, origin, active.at, Some(root))
                })
                .as_ref()
                == Some(&active.geometry);
        if !current {
            self.host_choice.active = None;
            self.host_choice.request = None;
        }
        self.host_choice.active.as_ref().map(|active| &active.lease)
    }

    pub(crate) fn route_host_choice(
        &mut self,
        tree: &mut WidgetTree,
        root: NodeId,
        state: &mut InteractionDomainState,
        reply: ChoiceReply,
        text_engine: &mut dyn TextEngine,
        animation: Option<AnimationContext<'_>>,
    ) -> bool {
        if self.current_host_choice(tree, root, state, reply.lease.press.window)
            != Some(&reply.lease)
        {
            return false;
        }
        self.retire_host_choice(&reply.lease);
        let Some(option) = reply.selection else {
            return false;
        };
        let owner = reply.lease.target.mount.owner();
        let pointer = reply.lease.press.pointer;
        let kind = "HostChoice";
        self.enter_phase(InputPipelinePhase::PreHitTest, pointer, kind);
        self.exit_phase(InputPipelinePhase::PreHitTest, pointer, kind);
        self.enter_phase(InputPipelinePhase::HitTest, pointer, kind);
        self.exit_phase(InputPipelinePhase::HitTest, pointer, kind);
        self.enter_phase(InputPipelinePhase::Routing, pointer, kind);
        let handled = self.dispatch_widget_input(
            tree,
            owner,
            None,
            EventPhase::Target,
            PointerRouteOrigin::Regular,
            Some(text_engine as *mut _),
            state,
            animation,
            WidgetInput::HostChoice(option),
        );
        self.exit_phase(InputPipelinePhase::Routing, pointer, kind);
        self.enter_phase(InputPipelinePhase::PostProcess, pointer, kind);
        self.exit_phase(InputPipelinePhase::PostProcess, pointer, kind);
        handled
    }
}

pub(crate) fn choice_options<'a>(
    tree: &'a WidgetTree,
    mount: &WidgetMountToken,
) -> Option<ChoiceOptions<'a>> {
    let node_id = tree.find_by_id(mount.owner())?;
    let node = tree.node(node_id)?;
    if !mount.is_live()
        || tree.concrete_mount(node.id).as_ref() != Some(mount)
        || !tree.composite_choice_input_ready(node)
        || !tree.composite_presentation_visible(node_id)
    {
        return None;
    }
    node.widget
        .as_any()
        .downcast_ref::<ComboBox>()?
        .host_choice_options()
}

#[cfg(any(target_arch = "wasm32", test))]
pub(crate) fn subtract_cover(rect: RectF, cover: RectF) -> Vec<RectF> {
    let Some(overlap) = rect.intersection(&cover) else {
        return vec![rect];
    };
    let right = overlap.x + overlap.width;
    let bottom = overlap.y + overlap.height;
    [
        RectF::new(rect.x, rect.y, rect.width, overlap.y - rect.y),
        RectF::new(rect.x, bottom, rect.width, rect.y + rect.height - bottom),
        RectF::new(rect.x, overlap.y, overlap.x - rect.x, overlap.height),
        RectF::new(
            right,
            overlap.y,
            rect.x + rect.width - right,
            overlap.height,
        ),
    ]
    .into_iter()
    .filter(|piece| piece.width > 0.0 && piece.height > 0.0)
    .collect()
}
