// Navigator Demo — 1M mock items in a content browser

use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::math::RectF;
use halcyon::core::paint::{Color, LAYER_CONTENT, RoundedCorners, ShadowMode};
use halcyon::core::widget::{PaintContext, Widget};
use halcyon::prelude::*;
use halcyon::widgets::navigator::data_source::NavigatorDataSource;
use halcyon::widgets::navigator::item_painter::ItemPainter;
use halcyon::widgets::navigator::navigator::{
    Navigator, NavigatorConfig, NavigatorPolicy, NavigatorViewMode,
};
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use halcyon::widgets::tabular_view::{ColumnId, RowId};
use std::rc::Rc;

fn main() {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(AppConfig::new("Navigator Demo").with_size(900.0, 700.0));
    app.run(NavigatorScreen::new());
}

// ─── Mock DataSource ────────────────────────────────────────────────────────

struct MockGridSource {
    row_ids: Vec<RowId>,
    name_col: ColumnId,
}

impl DataSource for MockGridSource {
    fn row_ids(&self) -> &[RowId] {
        &self.row_ids
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.row_ids.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Snapshot
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        if column == self.name_col {
            ValueState::Ready(CellValue::String(format!("Card {}", row.raw())))
        } else {
            ValueState::Missing
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        Vec::new()
    }
}

impl NavigatorDataSource for MockGridSource {}

// ─── Mock ItemPainter ───────────────────────────────────────────────────────

struct MockPainter {
    name_col: ColumnId,
}

impl ItemPainter for MockPainter {
    fn paint_item(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        row_id: RowId,
        selected: bool,
        hovered: bool,
        focused: bool,
        data: &dyn DataSource,
    ) {
        let radius = 6.0;
        let corners = RoundedCorners::uniform(radius);

        // Hover — subtle drop shadow lift
        if hovered && !selected {
            cx.scene.push_shadow(
                rect,
                radius,
                ShadowMode::Drop,
                (0.0, 2.0),
                0.0,
                6.0,
                Color::from_srgb(0.0, 0.0, 0.0, 0.15),
                LAYER_CONTENT - 1,
                None,
            );
        }

        // Background — very subtle tint on selection
        let base_bg = cx.theme.colors.surfaces.level1;
        let bg = if selected {
            let accent = cx.theme.colors.accents.primary;
            Color::from_srgb(
                base_bg.r * 0.98 + accent.r * 0.02,
                base_bg.g * 0.98 + accent.g * 0.02,
                base_bg.b * 0.98 + accent.b * 0.02,
                base_bg.a,
            )
        } else {
            base_bg
        };
        cx.scene
            .push_rounded_rect(rect, bg, corners, LAYER_CONTENT, None);

        // Card border — accent when selected, focus ring, or subtle default
        let (border_color, border_width) = if selected {
            (cx.theme.colors.accents.primary, 1.5)
        } else if focused {
            (cx.theme.colors.borders.focus, 1.0)
        } else {
            (Color::from_srgb(0.3, 0.3, 0.4, 0.5), 1.0)
        };
        cx.scene.push_border_rounded_rect(
            rect,
            border_color,
            border_width,
            corners,
            LAYER_CONTENT + 1,
            None,
        );

        // Text label
        let name = match data.read_cell(row_id, self.name_col) {
            ValueState::Ready(CellValue::String(s)) => s,
            _ => "Item".to_string(),
        };
        let run = cx.text_engine.shape(&name, 13.0);
        cx.text_engine.draw(
            &run,
            rect.x + 8.0,
            rect.y + 8.0,
            cx.theme.colors.typography.primary,
            LAYER_CONTENT + 2,
            cx.scene,
        );

        // Colored placeholder content
        let color_rect = RectF::new(
            rect.x + 8.0,
            rect.y + 28.0,
            rect.width - 16.0,
            rect.height - 36.0,
        );
        let hue = (row_id.raw() % 7) as f32 / 7.0;
        let color = Color::from_srgb(hue, 0.4, 0.6, 0.3);
        cx.scene.push_rounded_rect(
            color_rect,
            color,
            RoundedCorners::uniform(2.0),
            LAYER_CONTENT + 2,
            None,
        );
    }
}

// ─── Screen ─────────────────────────────────────────────────────────────────

struct NavigatorScreen {
    built: bool,
    needs_rebuild: bool,
    placeholder: Label,
    view_mode: NavigatorViewMode,
}

impl NavigatorScreen {
    fn new() -> Self {
        Self {
            built: false,
            needs_rebuild: true,
            placeholder: Label::new(""),
            view_mode: NavigatorViewMode::Icon,
        }
    }
}

impl Screen for NavigatorScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        let name_col = ColumnId::new();
        let source = MockGridSource {
            row_ids: (0..1_000_000).map(|_| RowId::new()).collect(),
            name_col,
        };
        let painter: Rc<dyn ItemPainter> = Rc::new(MockPainter { name_col });

        let config = NavigatorConfig::for_mode(self.view_mode);

        let navigator = Navigator::new(source, painter)
            .with_policy(NavigatorPolicy::multi_select())
            .with_config(config);
        let body = navigator.create_body();

        // Wire tree: Navigator -> ScrollArea -> NavigatorBody
        let gv_id = cx.mount_child(Box::new(navigator));

        let scroll_area = ScrollArea::new_unwired().with_scrollbars(true);
        let sa_id = cx.tree.add_child(gv_id, Box::new(scroll_area), None, 0);

        let body_id = cx.tree.add_child(sa_id, Box::new(body), None, 0);

        // Wire ScrollArea content
        if let Some(node) = cx.tree.node_mut(sa_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        &mut self.placeholder
    }

    fn handle_input(&mut self, event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // View mode switching via number keys (1-6)
        if let FusedInputEvent::TextInput { text, .. } = event {
            let new_mode = match text.as_str() {
                "1" => Some(NavigatorViewMode::SmallIcon),
                "2" => Some(NavigatorViewMode::Icon),
                "3" => Some(NavigatorViewMode::LargeIcon),
                "4" => Some(NavigatorViewMode::ExtraLargeIcon),
                "5" => Some(NavigatorViewMode::Tile),
                "6" => Some(NavigatorViewMode::Content),
                _ => None,
            };
            if let Some(mode) = new_mode {
                if mode != self.view_mode {
                    self.view_mode = mode;
                    self.built = false;
                    self.needs_rebuild = true;
                    return true;
                }
            }
        }
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {}

    fn mark_rebuild(&mut self) {
        self.needs_rebuild = true;
    }

    fn take_rebuild_flag(&mut self) -> bool {
        std::mem::take(&mut self.needs_rebuild)
    }
}
