//! Mount a read-only ListView backed by a small, immutable file catalog.
//! Run with `cargo run --example listview_basic`.

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::Widget;
use halcyon::prelude::{Flex, Label};
use halcyon::widgets::list_view::ListView;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::column::{Column, ColumnId};
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, OrderAccess,
    OrderRevision, OrderWindow, OrderWindowRequest, UniverseMode, ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use std::cell::Cell;
use std::rc::Rc;

const NAME: ColumnId = ColumnId::from_raw(1);
const KIND: ColumnId = ColumnId::from_raw(2);
const SIZE: ColumnId = ColumnId::from_raw(3);
const FILE_ORDER: OrderRevision = OrderRevision(1);

// Each record contains its identity, name, kind, and display size.
const FILES: [(RowId, &str, &str, &str); 12] = [
    (RowId::from_raw(101), "README.md", "Markdown", "4 KB"),
    (RowId::from_raw(204), "notes.txt", "Text", "2 KB"),
    (RowId::from_raw(132), "outline.md", "Markdown", "8 KB"),
    (RowId::from_raw(228), "logo.svg", "Vector image", "6 KB"),
    (RowId::from_raw(146), "mockup.png", "Image", "128 KB"),
    (RowId::from_raw(303), "sample.csv", "Spreadsheet", "12 KB"),
    (RowId::from_raw(177), "palette.json", "JSON", "1 KB"),
    (RowId::from_raw(265), "draft.pdf", "PDF", "240 KB"),
    (RowId::from_raw(187), "preview.jpg", "Image", "95 KB"),
    (
        RowId::from_raw(318),
        "settings.toml",
        "Configuration",
        "3 KB",
    ),
    (RowId::from_raw(195), "index.html", "Web page", "5 KB"),
    (RowId::from_raw(321), "archive.zip", "Archive", "1 MB"),
];

struct FileSource;

impl DataSource for FileSource {
    fn order_access(&self) -> OrderAccess {
        OrderAccess::Windowed
    }

    fn order_revision(&self) -> OrderRevision {
        FILE_ORDER
    }

    fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow> {
        if request.revision != self.order_revision() {
            return ValueState::Error {
                error: Some("stale order revision".into()),
            };
        }
        let start = request.start.min(FILES.len());
        let end = start.saturating_add(request.length).min(FILES.len());
        ValueState::Ready(OrderWindow {
            revision: request.revision,
            start,
            row_ids: FILES[start..end].iter().map(|(id, ..)| *id).collect(),
        })
    }

    fn locate_row(&self, revision: OrderRevision, row: RowId) -> ValueState<Option<usize>> {
        if revision != self.order_revision() {
            return ValueState::Error {
                error: Some("stale order revision".into()),
            };
        }
        ValueState::Ready(FILES.iter().position(|(id, ..)| *id == row))
    }

    fn extent(&self) -> Extent {
        Extent::Exact(FILES.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Snapshot
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some((_, name, kind, size)) = FILES.iter().find(|(id, ..)| *id == row) else {
            return ValueState::Missing;
        };
        let value = match column {
            NAME => name,
            KIND => kind,
            SIZE => size,
            _ => return ValueState::Missing,
        };
        ValueState::Ready(CellValue::String((*value).to_owned()))
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        Vec::new()
    }
}

fn main() {
    App::new(AppConfig::new("Halcyon - ListView Basics").with_size(640.0, 400.0)).run(FileList {
        placeholder: Label::new(""),
        rebuild: true,
    });
}

struct FileList {
    placeholder: Label,
    rebuild: bool,
}

impl Screen for FileList {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let mut list = ListView::new(FileSource);
        for (title, id, width) in [
            ("Name", NAME, 320.0),
            ("Kind", KIND, 150.0),
            ("Size", SIZE, 100.0),
        ] {
            list.add_column_raw(title, Column::with_id(id, width).with_sortable(false));
        }
        list.finalize_config();

        // The header and footer follow the body's horizontal scroll offset.
        let link = HorizontalScrollLink::new();
        let freeze = Rc::new(Cell::new(false));
        list.set_horizontal_scroll_link(link.clone(), Rc::clone(&freeze));
        let scroll = list
            .take_body_scroll_area()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(link)
            .with_horizontal_freeze(freeze);
        let body = list.create_body_widget();
        let footer = list.create_footer_widget();

        let root = cx.mount_child(Box::new(Flex::column()));
        let list = cx.tree.add_child(root, Box::new(list), None, 0);
        cx.tree.set_flex(list, FlexParams::flexible(1.0));
        let scroll = cx.tree.add_child(list, Box::new(scroll), None, 0);
        let body = cx.tree.add_child(scroll, Box::new(body), None, 0);
        cx.tree
            .node_mut(scroll)
            .expect("mounted list body ScrollArea")
            .widget_mut()
            .as_any_mut()
            .downcast_mut::<ScrollArea>()
            .expect("ListView body is a ScrollArea")
            .wire_content(body);
        // The footer is a structural child, even at its default zero height.
        cx.tree.add_child(list, Box::new(footer), None, 0);
        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        false
    }

    fn mark_rebuild(&mut self) {
        self.rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.rebuild)
    }
}
