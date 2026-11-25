// SPEC-HAL-351: Section Surface Family Demo
// SPEC-HAL-018: Uses App + Screen pattern

use std::cell::Cell;
use std::rc::Rc;

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Alignment, Constraints, EdgeInsets, FlexParams, LayoutResult};
use halcyon::core::math::{RectF, Size};
use halcyon::core::paint::Color;
use halcyon::core::theme::Theme;
use halcyon::core::widget::{LayoutContext, PaintContext, Widget, WidgetDowncast};
use halcyon::core::widget_tree::{NodeId, WidgetTree};
use halcyon::prelude::*;
use halcyon::utils::id::WidgetInternalId;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::panel::Panel;
use halcyon::widgets::scroll_area::{LayoutScrollAnchorIntent, ScrollArea};
use halcyon::widgets::section_header::SectionHeader;
use halcyon::widgets::section_surfaces::{
    DisclosureSectionHeader, SectionBandHeader, SectionBandLevel, SectionBandMarker,
    SectionMetaHeader,
};

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    println!("=== Section Surface Family Demo ===");
    println!("Click Recovery to toggle externally owned disclosure state");
    println!("Close the window to exit");
    println!("===================================\n");

    let app = App::new(
        AppConfig::new("Section Surface Family Demo")
            .with_size(760.0, 620.0)
            .with_theme(Theme::dark()),
    );
    app.run(SectionSurfaceDemoScreen::new());
}

struct SectionSurfaceDemoScreen {
    built: bool,
    placeholder: Label,
    recovery_expanded: Rc<Cell<bool>>,
    last_recovery_expanded: bool,
    page_id: Option<NodeId>,
    recovery_section_id: Option<NodeId>,
    recovery_header_id: Option<NodeId>,
}

impl SectionSurfaceDemoScreen {
    fn new() -> Self {
        let recovery_expanded = true;
        Self {
            built: false,
            placeholder: Label::new(""),
            recovery_expanded: Rc::new(Cell::new(recovery_expanded)),
            last_recovery_expanded: recovery_expanded,
            page_id: None,
            recovery_section_id: None,
            recovery_header_id: None,
        }
    }

    fn add_field(
        cx: &mut ScreenFrameContext<'_>,
        parent: NodeId,
        index: i16,
        label: &str,
        value: &str,
    ) -> NodeId {
        let border = cx.app.theme.colors.borders.subtle;
        let background = cx.app.theme.colors.surfaces.content;
        let secondary = cx.app.theme.colors.typography.secondary;

        let field = cx.tree.add_child(
            parent,
            Box::new(
                Panel::new(NodeId(0))
                    .with_padding(6.0)
                    .with_border(border, 1.0)
                    .with_background(background),
            ),
            None,
            index,
        );
        let column = cx.tree.add_child(
            field,
            Box::new(
                Flex::column()
                    .with_spacing(2.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            0,
        );
        cx.tree.add_child(
            column,
            Box::new(Label::new(label).with_size(11.0).with_color(secondary)),
            None,
            0,
        );
        cx.tree
            .add_child(column, Box::new(Label::new(value)), None, 1);
        halcyon::replace_widget!(
            cx.tree,
            field,
            Panel::new(column)
                .with_padding(6.0)
                .with_border(border, 1.0)
                .with_background(background)
        );
        field
    }

    fn add_field_row(
        cx: &mut ScreenFrameContext<'_>,
        parent: NodeId,
        index: i16,
        left: (&str, &str),
        right: Option<(&str, &str)>,
    ) -> NodeId {
        let row = cx.tree.add_child(
            parent,
            Box::new(
                Flex::row()
                    .with_spacing(6.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            index,
        );
        let left_field = Self::add_field(cx, row, 0, left.0, left.1);
        cx.tree.set_flex(left_field, FlexParams::flexible(1.0));

        if let Some(right) = right {
            let right_field = Self::add_field(cx, row, 1, right.0, right.1);
            cx.tree.set_flex(right_field, FlexParams::flexible(1.0));
        }

        row
    }

    fn sync_recovery_expanded(&self, tree: &mut WidgetTree, expanded: bool) {
        if let (Some(header_id), Some(section_id)) =
            (self.recovery_header_id, self.recovery_section_id)
        {
            let intent = if expanded {
                LayoutScrollAnchorIntent::ExpandRevealRegion
            } else {
                LayoutScrollAnchorIntent::CollapsePreserveHeader
            };
            tree.begin_layout_scroll_anchor(header_id, section_id, intent);
        }

        if let Some(header_id) = self.recovery_header_id {
            if let Some(node) = tree.node_mut(header_id) {
                if let Some(header) = node
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<DisclosureSectionHeader>()
                {
                    header.set_expanded(expanded);
                    header.set_marker(if expanded {
                        SectionBandMarker::Selection
                    } else {
                        SectionBandMarker::None
                    });
                }
            }
            tree.mark_layout_dirty(header_id);
        }

        if let Some(section_id) = self.recovery_section_id {
            let mut changed = false;
            if let Some(node) = tree.node_mut(section_id) {
                if let Some(section) = node
                    .widget_mut()
                    .as_any_mut()
                    .downcast_mut::<DemoCollapsibleSection>()
                {
                    changed = section.set_expanded(expanded);
                }
            }
            if changed {
                tree.mark_layout_dirty(section_id);
            }
        }

        if let Some(page_id) = self.page_id {
            tree.mark_layout_dirty(page_id);
        }
    }
}

struct DemoCollapsibleSection {
    id: WidgetInternalId,
    expanded: bool,
    spacing: f32,
}

impl DemoCollapsibleSection {
    fn new(expanded: bool, spacing: f32) -> Self {
        Self {
            id: WidgetInternalId::new(),
            expanded,
            spacing,
        }
    }

    fn set_expanded(&mut self, expanded: bool) -> bool {
        let changed = self.expanded != expanded;
        self.expanded = expanded;
        changed
    }

    fn available_width(constraints: Constraints) -> f32 {
        if constraints.max_width.is_finite() {
            constraints.max_width
        } else {
            constraints.min_width
        }
        .max(0.0)
    }
}

impl WidgetDowncast for DemoCollapsibleSection {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for DemoCollapsibleSection {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        children: &[NodeId],
    ) -> LayoutResult {
        let width = Self::available_width(constraints);
        let Some(&header_id) = children.first() else {
            return LayoutResult::fit(constraints.constrain(Size::new(width, 0.0)));
        };

        let header_constraints = Constraints::new(width, width, 0.0, constraints.max_height);
        let header_size = cx.layout_child(header_id, header_constraints);
        cx.set_child_rect(header_id, RectF::new(0.0, 0.0, width, header_size.height));

        let mut height = header_size.height;
        if let Some(&content_id) = children.get(1) {
            if self.expanded {
                let content_max_height = if constraints.max_height.is_finite() {
                    (constraints.max_height - header_size.height - self.spacing).max(0.0)
                } else {
                    f32::INFINITY
                };
                let content_constraints = Constraints::new(width, width, 0.0, content_max_height);
                let content_size = cx.layout_child(content_id, content_constraints);
                let content_y = header_size.height + self.spacing;
                cx.set_child_rect(
                    content_id,
                    RectF::new(0.0, content_y, width, content_size.height),
                );
                height = content_y + content_size.height;
            } else {
                cx.layout_child(content_id, Constraints::tight(Size::zero()));
                cx.set_child_rect(content_id, RectF::zero());
            }
        }

        LayoutResult::fit(constraints.constrain(Size::new(width, height)))
    }

    fn paint(&self, _cx: &mut PaintContext, _rect: RectF, _children: &[NodeId]) {}

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        false
    }

    fn debug_type_name(&self) -> &'static str {
        "DemoCollapsibleSection"
    }
}

impl Screen for SectionSurfaceDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;
        self.page_id = None;
        self.recovery_section_id = None;
        self.recovery_header_id = None;

        let root_panel = cx.mount_child(Box::new(Label::new("")));
        let outer = cx
            .tree
            .add_child(root_panel, Box::new(Flex::column()), None, 0);
        halcyon::replace_widget!(
            cx.tree,
            root_panel,
            Panel::new(outer)
                .with_padding(0.0)
                .with_border(Color::TRANSPARENT, 0.0)
        );

        let header = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_align_items(Alignment::Center)
                    .with_padding(EdgeInsets::new(8.0, 10.0, 4.0, 10.0)),
            ),
            None,
            0,
        );
        cx.tree.add_child(
            header,
            Box::new(Label::new("Section Surface Family").with_size(16.0)),
            None,
            0,
        );
        let header_spacer = cx
            .tree
            .add_child(header, Box::new(FlexSpacer::new()), None, 1);
        cx.tree.set_flex(header_spacer, FlexParams::flexible(1.0));
        cx.tree.add_child(
            header,
            Box::new(Label::new("SPEC-HAL-351").with_size(12.0)),
            None,
            2,
        );

        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 1);

        let scroll = cx
            .tree
            .add_child(outer, Box::new(ScrollArea::new(NodeId(0))), None, 2);
        cx.tree.set_flex(scroll, FlexParams::flexible(1.0));
        let page = cx.tree.add_child(
            scroll,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_align_items(Alignment::Stretch)
                    .with_padding(EdgeInsets::new(10.0, 10.0, 28.0, 10.0)),
            ),
            None,
            0,
        );
        self.page_id = Some(page);
        halcyon::replace_widget!(cx.tree, scroll, ScrollArea::new(page));

        let mut index = 0;

        cx.tree.add_child(
            page,
            Box::new(SectionHeader::new("Market").with_secondary("OperatorWorkstationService")),
            None,
            index,
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Entity kind", "Market"),
            Some(("Owner", "OperatorWorkstationService")),
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Stable id", "market-quote:10:binance-local-1-1"),
            None,
        );
        index += 1;
        cx.tree
            .add_child(page, Box::new(FixedSpacer::vertical(4.0)), None, index);
        index += 1;

        cx.tree.add_child(
            page,
            Box::new(
                SectionBandHeader::new("Identity")
                    .with_level(SectionBandLevel::Section)
                    .with_marker(SectionBandMarker::Focus),
            ),
            None,
            index,
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Entity kind", "Market"),
            Some(("Owner", "OperatorWorkstationService")),
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Stable id", "market-quote:10:binance-local-1-1"),
            None,
        );
        index += 1;

        cx.tree.add_child(
            page,
            Box::new(SectionBandHeader::new("Identity").with_level(SectionBandLevel::Section)),
            None,
            index,
        );
        index += 1;
        cx.tree.add_child(
            page,
            Box::new(
                SectionBandHeader::new("Read model")
                    .with_level(SectionBandLevel::SubSection)
                    .with_marker(SectionBandMarker::None),
            ),
            None,
            index,
        );
        index += 1;
        Self::add_field_row(cx, page, index, ("Lane", "workstation.market_quotes"), None);
        index += 1;

        cx.tree.add_child(
            page,
            Box::new(
                SectionMetaHeader::new("Projection", "4 fields")
                    .with_level(SectionBandLevel::Section)
                    .with_marker(SectionBandMarker::Selection),
            ),
            None,
            index,
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Revision", "r3773"),
            Some(("Inspector feed", "Normal")),
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Status", "Resolved"),
            Some(("Audit refs", "3")),
        );
        index += 1;

        let expanded = self.recovery_expanded.get();
        let recovery_state = self.recovery_expanded.clone();
        let recovery_section = cx.tree.add_child(
            page,
            Box::new(DemoCollapsibleSection::new(expanded, 8.0)),
            None,
            index,
        );
        self.recovery_section_id = Some(recovery_section);

        let recovery_header = cx.tree.add_child(
            recovery_section,
            Box::new(
                DisclosureSectionHeader::new("Recovery", expanded)
                    .with_marker(if expanded {
                        SectionBandMarker::Selection
                    } else {
                        SectionBandMarker::None
                    })
                    .on_toggle(move || {
                        recovery_state.set(!recovery_state.get());
                    }),
            ),
            None,
            0,
        );
        self.recovery_header_id = Some(recovery_header);

        let recovery_content = cx.tree.add_child(
            recovery_section,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_align_items(Alignment::Stretch),
            ),
            None,
            1,
        );
        Self::add_field_row(
            cx,
            recovery_content,
            0,
            ("Replay window", "12 events"),
            Some(("Checkpoint", "stable")),
        );
        Self::add_field_row(
            cx,
            recovery_content,
            1,
            ("Recovery lane", "read-model replay"),
            Some(("Authority", "external controller")),
        );
        index += 1;

        cx.tree.add_child(
            page,
            Box::new(
                SectionMetaHeader::new("Audit", "r3773 - resolved")
                    .with_level(SectionBandLevel::SubSection),
            ),
            None,
            index,
        );
        index += 1;
        Self::add_field_row(
            cx,
            page,
            index,
            ("Reviewer", "local verifier"),
            Some(("Refs", "3")),
        );
        self.last_recovery_expanded = expanded;

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        // This demo keeps the ScrollArea node stable so toggles retain scroll offset.
    }

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }

    fn update(&mut self, tree: &mut WidgetTree) {
        let expanded = self.recovery_expanded.get();
        if expanded != self.last_recovery_expanded {
            self.sync_recovery_expanded(tree, expanded);
            self.last_recovery_expanded = expanded;
        }
    }
}
