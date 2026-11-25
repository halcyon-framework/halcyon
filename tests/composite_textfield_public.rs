//! PUBLIC COMPILE QUALIFICATION ONLY.
//!
//! This external consumer uses ordinary crate exports to construct a Screen,
//! mount four tree-owned composite hosts, and type-check their scoped public
//! owner operations. The test only constructs the Screen. It does not execute
//! App::run, build, the owner hook, input routing, rendering, or native UI.

use halcyon::core::app::{Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::composite_text_edit::{
    CompositeEditOutcome, CompositeEditReply, CompositeEditorId,
};
use halcyon::core::widget::Widget;
use halcyon::engines::cell_source::ComboBoxCellEditorSpec;
use halcyon::utils::id::TreeWidgetId;
use halcyon::widgets::combo_box::ComboBox;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::slider::Slider;
use halcyon::widgets::spin_box::SpinBox;
use halcyon::widgets::tag_input::{TagInput, TagInputPolicy, TagItem};

#[derive(Default)]
struct PublicCompositeScreen {
    hosts: Option<[TreeWidgetId; 4]>,
    links: Option<[CompositeEditorId; 4]>,
    outcome: Option<Result<(), CompositeEditOutcome>>,
}

fn require_applied(reply: CompositeEditReply) -> Result<(), CompositeEditOutcome> {
    if reply.applied() {
        Ok(())
    } else {
        Err(reply.outcome)
    }
}

impl Screen for PublicCompositeScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let root = cx.mount_child(Box::new(Flex::column()));
        let combo = cx.tree.add_child(
            root,
            Box::new(ComboBox::simple_with_values(vec![(
                "Alpha".to_owned(),
                "alpha".to_owned(),
            )])),
            None,
            0,
        );
        let spin = cx.tree.add_child(root, Box::new(SpinBox::new()), None, 0);
        let slider = cx.tree.add_child(
            root,
            Box::new(Slider::new(0.0, 1.0).with_value_display()),
            None,
            0,
        );
        let tag = cx.tree.add_child(root, Box::new(TagInput::new()), None, 0);
        self.hosts = Some([combo, spin, slider, tag].map(|node| {
            cx.tree
                .widget_id_for_node(node)
                .expect("mounted public host")
        }));

        // The returned Widget remains owned by this same tree.
        cx.tree.node_mut(root).expect("mounted root").widget_mut()
    }

    fn flush_pending_actions(&mut self, cx: &mut ScreenContext<'_>) {
        if self.outcome.is_some() {
            return;
        }
        let Some(hosts) = self.hosts else {
            return;
        };
        let result = cx.with_composite_editors(|owner| {
            let combo = owner.resolve(hosts[0])?;
            let spin = owner.resolve(hosts[1])?;
            let slider = owner.resolve(hosts[2])?;
            let tag = owner.resolve(hosts[3])?;

            let snapshot = owner.snapshot(&combo)?;
            require_applied(owner.replace_text(&combo, snapshot.text))?;
            let _committed_integer = owner.force_commit(&spin)?;
            require_applied(owner.sync_combo_box(
                &combo,
                ComboBoxCellEditorSpec {
                    current_value: "alpha".to_owned(),
                    options: vec![("Alpha".to_owned(), "alpha".to_owned())],
                },
            ))?;
            require_applied(owner.replace_text(&slider, "0.5".to_owned()))?;
            require_applied(owner.set_tag_items(
                &tag,
                vec![TagItem {
                    id: 1,
                    label: "Alpha".to_owned(),
                    group: None,
                    enabled: true,
                }],
            ))?;
            require_applied(owner.set_tag_policy(&tag, TagInputPolicy::Full))?;
            Ok::<_, CompositeEditOutcome>([combo, spin, slider, tag])
        });
        match result.and_then(|result| result) {
            Ok(links) => {
                self.links = Some(links);
                self.outcome = Some(Ok(()));
            }
            Err(outcome) => self.outcome = Some(Err(outcome)),
        }
    }
}

#[test]
fn composite_editor_public_consumer_constructs_screen() {
    let screen = PublicCompositeScreen::default();
    assert!(screen.hosts.is_none());
    assert!(screen.links.is_none());
    assert!(screen.outcome.is_none());
}
