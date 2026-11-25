// SPEC-HAL-270 Section 9: NotificationLogWidget
//
// Embeddable retained-mode widget that renders the persistent notification log.
// Not an overlay — placed in the widget tree by the screen like any other widget.
//
// INV-NLW-READ-ONLY: Reads NotificationManager during build_into() only. No mutation during layout or paint.
// INV-NLW-OWNER: Holds Rc<RefCell<NotificationManager>> for read access only.
// INV-NLW-DEFERRED: All mutations via FormBinder bindings, executed by parent screen in flush_pending_actions.
// INV-NLW-EPHEMERAL: Snapshot taken during build_into(), not stored across frames.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::layout::{Constraints, EdgeInsets, LayoutResult};
use crate::core::math::RectF;
use crate::core::paint::Color;
use crate::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::form_binder::FormBinder;
use crate::model::notification::{NotificationLevel, NotificationManager};
use crate::utils::id::WidgetInternalId;
use crate::widgets::button::Button;
use crate::widgets::flex::Flex;
use crate::widgets::flex_spacer::FlexSpacer;
use crate::widgets::label::Label;

use crate::core::app::{ScreenContext, ScreenFrameContext};

pub struct NotificationLogWidget {
    id: WidgetInternalId,
    manager: Rc<RefCell<NotificationManager>>,
    binder: FormBinder,
    dismiss_ids: Vec<u64>,
    needs_rebuild: bool,
}

impl NotificationLogWidget {
    pub fn new(manager: Rc<RefCell<NotificationManager>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            manager,
            binder: FormBinder::new(),
            dismiss_ids: Vec::new(),
            needs_rebuild: true,
        }
    }

    pub fn build_into(
        &mut self,
        parent: NodeId,
        cx: &mut ScreenFrameContext<'_>,
        slot: i16,
    ) -> NodeId {
        let writer = self.binder.writer();

        // Snapshot log
        let mgr = self.manager.borrow();
        let entries: Vec<_> = mgr.iter().cloned().collect();
        drop(mgr);
        self.dismiss_ids = entries.iter().map(|e| e.id.0).collect();

        // Root column
        let root = cx.tree.add_child(
            parent,
            Box::new(Flex::column().with_spacing(4.0)),
            None,
            slot,
        );

        let mut s: i16 = 0;

        if entries.is_empty() {
            cx.tree
                .add_child(root, Box::new(Label::new("No notifications")), None, s);
        } else {
            for entry in &entries {
                let row = cx.tree.add_child(
                    root,
                    Box::new(
                        Flex::row()
                            .with_spacing(8.0)
                            .with_padding(EdgeInsets::symmetric(8.0, 6.0)),
                    ),
                    None,
                    s,
                );
                s += 1;

                // Level indicator
                let indicator = match entry.level {
                    NotificationLevel::Error => "\u{25cf}",
                    NotificationLevel::Warning => "\u{25b3}",
                    NotificationLevel::Info => "\u{25cb}",
                };
                let level_color = match entry.level {
                    NotificationLevel::Error => Color::from_srgb(
                        0x83 as f32 / 255.0,
                        0x35 as f32 / 255.0,
                        0x35 as f32 / 255.0,
                        1.0,
                    ),
                    NotificationLevel::Warning => Color::from_srgb(
                        0x79 as f32 / 255.0,
                        0x63 as f32 / 255.0,
                        0x21 as f32 / 255.0,
                        1.0,
                    ),
                    NotificationLevel::Info => Color::from_srgb(
                        0x38 as f32 / 255.0,
                        0x58 as f32 / 255.0,
                        0x7D as f32 / 255.0,
                        1.0,
                    ),
                };
                cx.tree.add_child(
                    row,
                    Box::new(Label::new(indicator).with_color(level_color)),
                    None,
                    0,
                );

                // Message
                cx.tree
                    .add_child(row, Box::new(Label::new(&entry.message)), None, 1);

                // FlexSpacer
                cx.tree.add_child(row, Box::new(FlexSpacer::new()), None, 2);

                // [x] dismiss button
                let key = format!("dismiss_{}", entry.id.0);
                let btn = cx.tree.add_child(row, Box::new(Label::new("")), None, 3);
                let lbl = cx
                    .tree
                    .add_child(btn, Box::new(Label::new("\u{00d7}")), None, 0);
                cx.tree.replace_widget_only(
                    btn,
                    Box::new(Button::new(lbl).with_binding(&key, writer.clone())),
                );
            }

            // [Clear All]
            let btn = cx.tree.add_child(root, Box::new(Label::new("")), None, s);
            let lbl = cx
                .tree
                .add_child(btn, Box::new(Label::new("Clear All")), None, 0);
            cx.tree.replace_widget_only(
                btn,
                Box::new(Button::new(lbl).with_binding("nlw_clear_all", writer.clone())),
            );
        }

        root
    }

    pub fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) -> bool {
        let mut clear_all = false;
        let mut dismiss_list: Vec<u64> = Vec::new();
        let ids = self.dismiss_ids.clone();

        self.binder.bind(cx, |b| {
            if b.bool("nlw_clear_all").take().is_some() {
                clear_all = true;
            }
            for &id in &ids {
                let key = format!("dismiss_{}", id);
                if b.bool(&key).take().is_some() {
                    dismiss_list.push(id);
                }
            }
        });

        let mut dirty = false;

        if clear_all {
            cx.app.notification_manager.borrow_mut().clear();
            dirty = true;
        }

        for id in dismiss_list {
            cx.app
                .notification_manager
                .borrow_mut()
                .remove(crate::model::notification::NotificationId(id));
            dirty = true;
        }

        dirty
    }

    pub fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    pub fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}

impl WidgetDowncast for NotificationLogWidget {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for NotificationLogWidget {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        LayoutResult::fit(crate::core::math::Size::new(
            constraints.max_width,
            constraints.max_height,
        ))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {
        // No-op — subtree widgets handle their own paint.
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn debug_type_name(&self) -> &'static str {
        "NotificationLogWidget"
    }
}
