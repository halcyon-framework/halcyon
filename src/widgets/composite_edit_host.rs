//! Closed HAL-101 §2.5 adapters for the four admitted composite text owners.
//!
//! These values contain configuration and qualified observations, never a
//! TextField, a widget borrow, an event, or an executable continuation.

use crate::core::composite_text_edit::{
    CompositeEditOperation, CompositeEditOutcome, CompositeEditReply, CompositeEditRequest,
    CompositeEditSnapshot, CompositeEditorId,
};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::RectF;
use crate::core::paragraph::TextAlign;
use crate::core::widget::{EventContext, LayoutContext, Widget};
use crate::core::widget_tree::NodeId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositeEditorFilter {
    Any,
    Integer,
    Decimal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompositeEditorPalette {
    InputChrome,
    EmbeddedAccent,
}

#[derive(Debug, Clone)]
pub(crate) struct CompositeEditorSeed {
    pub(crate) text: String,
    pub(crate) align: TextAlign,
    pub(crate) filter: CompositeEditorFilter,
    pub(crate) placeholder: Option<String>,
    pub(crate) smooth_caret_enabled: bool,
    pub(crate) palette: CompositeEditorPalette,
}

/// Initial props become immutable applied observations only through the owner.
pub(crate) struct CompositeEditorState {
    pub(crate) id: Option<CompositeEditorId>,
    pub(crate) child: Option<NodeId>,
    pub(crate) snapshot: Option<CompositeEditSnapshot>,
    projection_revision: u64,
    seed: CompositeEditorSeed,
}

impl CompositeEditorState {
    pub(crate) fn new(align: TextAlign, filter: CompositeEditorFilter) -> Self {
        Self {
            id: None,
            child: None,
            snapshot: None,
            projection_revision: 0,
            seed: CompositeEditorSeed {
                text: String::new(),
                align,
                filter,
                placeholder: None,
                smooth_caret_enabled: true,
                palette: CompositeEditorPalette::EmbeddedAccent,
            },
        }
    }

    pub(crate) fn text(&self) -> &str {
        self.snapshot
            .as_ref()
            .map_or(self.seed.text.as_str(), |s| s.text.as_str())
    }

    pub(crate) fn construction_seed(&self) -> CompositeEditorSeed {
        self.seed.clone()
    }

    pub(crate) fn presentation_revision(&self) -> u64 {
        self.projection_revision
    }

    pub(crate) fn change_presentation_source(&mut self) {
        self.projection_revision = self
            .projection_revision
            .checked_add(1)
            .expect("composite presentation source revision exhausted");
    }

    pub(crate) fn set_initial_text(&mut self, text: String) -> Result<(), CompositeEditOutcome> {
        if self.id.is_some() {
            return Err(CompositeEditOutcome::OwnerContextRequired);
        }
        self.seed.text = text;
        Ok(())
    }

    pub(crate) fn set_initial_motion(&mut self, enabled: bool) -> Result<(), CompositeEditOutcome> {
        if self.id.is_some() {
            return Err(CompositeEditOutcome::OwnerContextRequired);
        }
        self.seed.smooth_caret_enabled = enabled;
        Ok(())
    }

    pub(crate) fn bind(&mut self, id: CompositeEditorId, child: NodeId) -> bool {
        if self.id.is_some() || self.child.is_some() {
            return false;
        }
        self.id = Some(id);
        self.child = Some(child);
        true
    }

    pub(crate) fn absorb(&mut self, reply: &CompositeEditReply) -> bool {
        if !reply.applied() {
            return false;
        }
        let Some(snapshot) = reply.snapshot.as_ref() else {
            return false;
        };
        if self.id.as_ref() != Some(&snapshot.id) {
            return false;
        }
        self.snapshot = Some(snapshot.clone());
        true
    }

    pub(crate) fn layout(
        &self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        x: f32,
        y: f32,
    ) -> LayoutResult {
        if self
            .id
            .as_ref()
            .is_none_or(|id| cx.tree.validate_composite_editor(id).is_err())
        {
            return LayoutResult::fit(crate::core::math::Size::zero());
        }
        let child = self
            .child
            .expect("HAL-101 composite child must be installed before Layout");
        let size = cx.layout_child(child, constraints);
        cx.set_child_rect(child, RectF::new(x, y, size.width, size.height));
        LayoutResult::fit(size)
    }
}

pub(crate) enum CompositeHostContinuation {
    Combo(super::combo_box::CompositeComboContinuation),
    Spin(super::spin_box::CompositeSpinContinuation),
    Slider(super::slider::CompositeSliderContinuation),
    Tag(super::tag_input::CompositeTagContinuation),
}

pub(crate) enum CompositeHostStep {
    Done {
        handled: bool,
    },
    Final {
        operation: CompositeEditOperation,
        continuation: CompositeHostContinuation,
    },
}

pub(crate) fn construction_seed(widget: &dyn Widget) -> Option<CompositeEditorSeed> {
    if let Some(host) = widget.as_any().downcast_ref::<super::combo_box::ComboBox>() {
        Some(host.composite_seed())
    } else if let Some(host) = widget.as_any().downcast_ref::<super::spin_box::SpinBox>() {
        Some(host.composite_seed())
    } else if let Some(host) = widget.as_any().downcast_ref::<super::slider::Slider>() {
        Some(host.composite_seed())
    } else {
        widget
            .as_any()
            .downcast_ref::<super::tag_input::TagInput>()
            .map(|host| host.composite_seed())
    }
}

pub(crate) fn bind(widget: &mut dyn Widget, id: CompositeEditorId, child: NodeId) -> bool {
    if let Some(host) = widget
        .as_any_mut()
        .downcast_mut::<super::combo_box::ComboBox>()
    {
        host.composite_bind(id, child)
    } else if let Some(host) = widget
        .as_any_mut()
        .downcast_mut::<super::spin_box::SpinBox>()
    {
        host.composite_bind(id, child)
    } else if let Some(host) = widget.as_any_mut().downcast_mut::<super::slider::Slider>() {
        host.composite_bind(id, child)
    } else if let Some(host) = widget
        .as_any_mut()
        .downcast_mut::<super::tag_input::TagInput>()
    {
        host.composite_bind(id, child)
    } else {
        false
    }
}

pub(crate) fn resume(
    widget: &mut dyn Widget,
    cx: &mut EventContext,
    continuation: CompositeHostContinuation,
    reply: CompositeEditReply,
) -> CompositeHostStep {
    match continuation {
        CompositeHostContinuation::Combo(tag) => widget
            .as_any_mut()
            .downcast_mut::<super::combo_box::ComboBox>()
            .map_or(CompositeHostStep::Done { handled: false }, |host| {
                host.resume_composite(cx, tag, reply)
            }),
        CompositeHostContinuation::Spin(tag) => widget
            .as_any_mut()
            .downcast_mut::<super::spin_box::SpinBox>()
            .map_or(CompositeHostStep::Done { handled: false }, |host| {
                host.resume_composite(cx, tag, reply)
            }),
        CompositeHostContinuation::Slider(tag) => widget
            .as_any_mut()
            .downcast_mut::<super::slider::Slider>()
            .map_or(CompositeHostStep::Done { handled: false }, |host| {
                host.resume_composite(cx, tag, reply)
            }),
        CompositeHostContinuation::Tag(tag) => widget
            .as_any_mut()
            .downcast_mut::<super::tag_input::TagInput>()
            .map_or(CompositeHostStep::Done { handled: false }, |host| {
                host.resume_composite(cx, tag, reply)
            }),
    }
}

pub(crate) fn active(widget: &dyn Widget) -> bool {
    if let Some(h) = widget.as_any().downcast_ref::<super::combo_box::ComboBox>() {
        h.composite_active()
    } else if let Some(h) = widget.as_any().downcast_ref::<super::spin_box::SpinBox>() {
        h.composite_active()
    } else if let Some(h) = widget.as_any().downcast_ref::<super::slider::Slider>() {
        h.composite_active()
    } else {
        widget
            .as_any()
            .downcast_ref::<super::tag_input::TagInput>()
            .is_some_and(|h| h.composite_active())
    }
}

pub(crate) fn bound_id(widget: &dyn Widget) -> Option<&CompositeEditorId> {
    if let Some(h) = widget.as_any().downcast_ref::<super::combo_box::ComboBox>() {
        h.composite_id()
    } else if let Some(h) = widget.as_any().downcast_ref::<super::spin_box::SpinBox>() {
        h.composite_id()
    } else if let Some(h) = widget.as_any().downcast_ref::<super::slider::Slider>() {
        h.composite_id()
    } else {
        widget
            .as_any()
            .downcast_ref::<super::tag_input::TagInput>()
            .and_then(|h| h.composite_id())
    }
}

pub(crate) fn presentation_revision(widget: &dyn Widget) -> u64 {
    if let Some(host) = widget.as_any().downcast_ref::<super::combo_box::ComboBox>() {
        host.composite_presentation_revision()
    } else if let Some(host) = widget.as_any().downcast_ref::<super::tag_input::TagInput>() {
        host.composite_presentation_revision()
    } else {
        0
    }
}

pub(crate) fn visible(widget: &dyn Widget) -> bool {
    if let Some(h) = widget.as_any().downcast_ref::<super::combo_box::ComboBox>() {
        h.composite_visible()
    } else if let Some(h) = widget.as_any().downcast_ref::<super::spin_box::SpinBox>() {
        h.composite_visible()
    } else if let Some(h) = widget.as_any().downcast_ref::<super::slider::Slider>() {
        h.composite_visible()
    } else {
        widget
            .as_any()
            .downcast_ref::<super::tag_input::TagInput>()
            .is_some_and(|h| h.composite_visible())
    }
}

pub(crate) fn editing_policy(
    widget: &dyn Widget,
) -> Option<crate::core::text_edit_policy::TextEditPolicy> {
    if let Some(h) = widget.as_any().downcast_ref::<super::combo_box::ComboBox>() {
        Some(h.composite_policy())
    } else if let Some(h) = widget.as_any().downcast_ref::<super::spin_box::SpinBox>() {
        Some(h.composite_policy())
    } else if let Some(h) = widget.as_any().downcast_ref::<super::slider::Slider>() {
        Some(h.composite_policy())
    } else {
        widget
            .as_any()
            .downcast_ref::<super::tag_input::TagInput>()
            .map(|h| h.composite_policy())
    }
}

pub(crate) fn observe(widget: &mut dyn Widget, reply: &CompositeEditReply) -> bool {
    if let Some(h) = widget
        .as_any_mut()
        .downcast_mut::<super::combo_box::ComboBox>()
    {
        h.composite_observe(reply)
    } else if let Some(h) = widget
        .as_any_mut()
        .downcast_mut::<super::spin_box::SpinBox>()
    {
        h.composite_observe(reply)
    } else if let Some(h) = widget.as_any_mut().downcast_mut::<super::slider::Slider>() {
        h.composite_observe(reply)
    } else {
        widget
            .as_any_mut()
            .downcast_mut::<super::tag_input::TagInput>()
            .is_some_and(|h| h.composite_observe(reply))
    }
}

pub(crate) fn combine_entry_request(
    entry: CompositeEditRequest,
    mut current: CompositeEditRequest,
) -> Result<CompositeEditRequest, (CompositeEditOutcome, CompositeEditRequest)> {
    match (entry.operation, entry.continuation, &current.operation) {
        (
            CompositeEditOperation::SeedForEntry { text, select_all },
            CompositeHostContinuation::Combo(super::combo_box::CompositeComboContinuation::Entry)
            | CompositeHostContinuation::Slider(super::slider::CompositeSliderContinuation::Entry),
            CompositeEditOperation::ForwardCurrentInput,
        ) => {
            current.operation =
                CompositeEditOperation::SeedForEntryThenCurrentInput { text, select_all };
            Ok(current)
        }
        _ => Err((CompositeEditOutcome::Reentrant, current)),
    }
}

pub(crate) fn force_commit_request(widget: &dyn Widget) -> Option<CompositeEditRequest> {
    widget
        .as_any()
        .downcast_ref::<super::spin_box::SpinBox>()
        .filter(|host| host.composite_visible())
        .map(|_| CompositeEditRequest {
            operation: CompositeEditOperation::ObserveAndActivity,
            continuation: CompositeHostContinuation::Spin(
                super::spin_box::CompositeSpinContinuation::Commit { release: false },
            ),
        })
}

pub(crate) fn committed_integer(widget: &dyn Widget) -> Option<i64> {
    widget
        .as_any()
        .downcast_ref::<super::spin_box::SpinBox>()
        .map(|host| host.value())
}

pub(crate) fn prepare_combo_sync(
    widget: &mut dyn Widget,
    spec: &crate::engines::cell_source::ComboBoxCellEditorSpec,
) -> Result<(CompositeEditOperation, bool), CompositeEditOutcome> {
    let host = widget
        .as_any_mut()
        .downcast_mut::<super::combo_box::ComboBox>()
        .ok_or(CompositeEditOutcome::Unavailable)?;
    let changed = host.composite_sync_options(spec);
    Ok((
        CompositeEditOperation::ResetAfterResolverCommit {
            text: spec.current_value.clone(),
            select_all: false,
        },
        changed,
    ))
}

pub(crate) fn finish_combo_sync(widget: &mut dyn Widget) -> Result<bool, CompositeEditOutcome> {
    let host = widget
        .as_any_mut()
        .downcast_mut::<super::combo_box::ComboBox>()
        .ok_or(CompositeEditOutcome::Unavailable)?;
    Ok(host.composite_disable_input())
}

pub(crate) fn apply_tag_items(
    widget: &mut dyn Widget,
    items: Vec<super::tag_input::TagItem>,
) -> Result<(), CompositeEditOutcome> {
    let host = widget
        .as_any_mut()
        .downcast_mut::<super::tag_input::TagInput>()
        .ok_or(CompositeEditOutcome::Unavailable)?;
    host.composite_set_items(items);
    Ok(())
}

pub(crate) fn apply_tag_policy(
    widget: &mut dyn Widget,
    policy: super::tag_input::TagInputPolicy,
) -> Result<(), CompositeEditOutcome> {
    let host = widget
        .as_any_mut()
        .downcast_mut::<super::tag_input::TagInput>()
        .ok_or(CompositeEditOutcome::Unavailable)?;
    host.composite_set_policy(policy);
    Ok(())
}

pub(crate) fn continuation_has_emitted_action(continuation: &CompositeHostContinuation) -> bool {
    matches!(
        continuation,
        CompositeHostContinuation::Tag(super::tag_input::CompositeTagContinuation::Clear {
            emitted: true,
            ..
        })
    )
}
