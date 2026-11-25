//! Grouped choices and model-owned selection across a locale rebuild.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::appearance::ButtonMaterialPolicy;
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{EdgeInsets, FlexParams};
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::model::default_selection_resolver::DefaultSelectionResolver;
use halcyon::model::selection_resolver::{CommittedValue, OptionId, ResolverHandle};
use halcyon::prelude::{Button, Flex, Label, ScrollArea, TextField};
use halcyon::widgets::{ComboBox, OptionGroupBoundary, text_area::TextArea};
use std::{cell::Cell, rc::Rc};

const ZIMBABWE: OptionId = OptionId::from_raw(1447254615);

#[cfg_attr(target_arch = "wasm32", allow(dead_code))]
fn main() {
    run_grouped_combo_box();
}

pub(crate) fn run_grouped_combo_box() {
    let mut config = AppConfig::new("Grouped ComboBox").with_size(280.0, 500.0);
    config.theme.button_material_policy = ButtonMaterialPolicy::SdfV3;
    App::new(config).run(GroupedForm {
        placeholder: Label::new(""),
        rebuild: true,
        french: Rc::new(Cell::new(false)),
        built_in_french: false,
        selected: Rc::new(Cell::new(None)),
        commit_count: Rc::new(Cell::new(0)),
        displayed_count: 0,
        commit_status: None,
    });
}

struct GroupedForm {
    placeholder: Label,
    rebuild: bool,
    french: Rc<Cell<bool>>,
    built_in_french: bool,
    selected: Rc<Cell<Option<OptionId>>>,
    commit_count: Rc<Cell<usize>>,
    displayed_count: usize,
    commit_status: Option<NodeId>,
}

impl Screen for GroupedForm {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let scroll = cx.mount_child(Box::new(ScrollArea::new_unwired()));
        let body = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_padding(EdgeInsets::all(20.0))
                    .with_spacing(12.0),
            ),
            None,
            0,
        );
        cx.tree
            .node_mut(scroll)
            .unwrap()
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .unwrap()
            .wire_content(body);

        let french = self.french.get();
        self.built_in_french = french;
        let labels: Vec<_> = (1..=249)
            .map(|index| match index {
                249 => "Zimbabwe".to_owned(),
                1..=30 if french => format!("Choix {index:03}"),
                _ => format!("Option {index:03}"),
            })
            .collect();
        let options: Vec<_> = labels
            .iter()
            .enumerate()
            .map(|(index, label)| {
                (
                    if index == 248 {
                        ZIMBABWE
                    } else {
                        OptionId::from_raw(10_000 + index as u64 * 17)
                    },
                    label.as_str(),
                )
            })
            .collect();
        let groups = [
            OptionGroupBoundary {
                start_index: 0,
                title: "EU / EEA",
            },
            OptionGroupBoundary {
                start_index: 30,
                title: "Outside EU / EEA",
            },
        ];
        let mut countries = ComboBox::new(ResolverHandle::new(DefaultSelectionResolver::strict()));
        countries.set_text_input_enabled(false);
        countries
            .set_grouped_projection(&options, &groups, None, 0, 11)
            .expect("valid example projection");
        let selected = self.selected.get();
        let caption = options
            .iter()
            .find(|(id, _)| Some(*id) == selected)
            .map_or("", |(_, label)| *label);
        countries
            .set_display_selection(selected, caption.to_owned())
            .expect("current example selection");
        let model_selection = Rc::clone(&self.selected);
        let commit_count = Rc::clone(&self.commit_count);
        countries.set_on_committed(move |value| {
            if let CommittedValue::ExistingOption(id) = value {
                model_selection.set(Some(*id));
                commit_count.set(
                    commit_count
                        .get()
                        .checked_add(1)
                        .expect("commit count overflow"),
                );
            }
        });
        let locale = Rc::clone(&self.french);
        let switch_locale = Button::with_text(if french {
            "Locale: French"
        } else {
            "Locale: English"
        })
        .on_click(move || locale.set(!locale.get()));

        let controls: [(Box<dyn Widget>, f32); 5] = [
            (Box::new(Label::new("Name")), 23.0),
            (
                Box::new(TextField::new().with_placeholder("Type your name")),
                38.0,
            ),
            (Box::new(Label::new("Country")), 23.0),
            (Box::new(countries), 38.0),
            (Box::new(switch_locale), 38.0),
        ];
        for (widget, height) in controls {
            let node = cx.tree.add_child(body, widget, None, 0);
            cx.tree.set_flex(node, FlexParams::fixed(height));
        }
        self.displayed_count = self.commit_count.get();
        let status = cx.tree.add_child(
            body,
            Box::new(Label::new(format!(
                "Selection commits: {}",
                self.displayed_count
            ))),
            None,
            0,
        );
        cx.tree.set_flex(status, FlexParams::fixed(23.0));
        self.commit_status = Some(status);
        let notes = cx
            .tree
            .add_child(body, Box::new(Label::new("Notes")), None, 0);
        cx.tree.set_flex(notes, FlexParams::fixed(23.0));
        let editor = cx.tree.add_child(body, Box::new(TextArea::new()), None, 0);
        cx.tree.set_flex(editor, FlexParams::fixed(300.0));
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild) || self.french.get() != self.built_in_french
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let count = self.commit_count.get();
        if count == self.displayed_count {
            return;
        }
        if let Some(status) = self.commit_status
            && let Some(label) = tree
                .node_mut(status)
                .and_then(|node| node.widget_mut().as_any_mut().downcast_mut::<Label>())
        {
            label.set_text(format!("Selection commits: {count}"));
            tree.mark_layout_dirty(status);
            self.displayed_count = count;
        }
    }
}
