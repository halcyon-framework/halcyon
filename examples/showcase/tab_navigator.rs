// Showcase - Navigator tab.

use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::math::RectF;
use halcyon::core::paint::{Color, LAYER_CONTENT};
use halcyon::core::widget::PaintContext;
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

use super::*;

struct AssetGridSource {
    row_ids: Vec<RowId>,
    name_col: ColumnId,
}

impl DataSource for AssetGridSource {
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
            let n = row.raw() % 10_000;
            ValueState::Ready(CellValue::String(format!("frame_{n:04}.exr")))
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

impl NavigatorDataSource for AssetGridSource {}

struct AssetTilePainter {
    name_col: ColumnId,
}

impl ItemPainter for AssetTilePainter {
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
        let base = cx.theme.colors.surfaces.level1;
        let accent = cx.theme.colors.accents.primary;
        let bg = if selected {
            Color::from_srgb(
                base.r * 0.90 + accent.r * 0.10,
                base.g * 0.90 + accent.g * 0.10,
                base.b * 0.90 + accent.b * 0.10,
                base.a,
            )
        } else if hovered {
            cx.theme.colors.surfaces.level2
        } else {
            base
        };
        cx.scene.push_rect(rect, bg, LAYER_CONTENT, None);

        let swatch = RectF::new(rect.x + 8.0, rect.y + 26.0, rect.width - 16.0, 16.0);
        let tone = ((row_id.raw() % 5) as f32) * 0.08;
        cx.scene.push_rect(
            swatch,
            Color::from_srgb(0.18 + tone, 0.25 + tone, 0.36 + tone, 1.0),
            LAYER_CONTENT + 1,
            None,
        );

        let stripe_color = if focused || selected {
            accent
        } else {
            Color::from_srgb(0.42, 0.46, 0.50, 0.65)
        };
        cx.scene.push_rect(
            RectF::new(rect.x, rect.y, 3.0, rect.height),
            stripe_color,
            LAYER_CONTENT + 2,
            None,
        );

        let name = match data.read_cell(row_id, self.name_col) {
            ValueState::Ready(CellValue::String(s)) => s,
            _ => "asset".to_string(),
        };
        let run = cx.text_engine.shape(&name, 12.0);
        cx.text_engine.draw(
            &run,
            rect.x + 8.0,
            rect.y + 8.0,
            cx.theme.colors.typography.primary,
            LAYER_CONTENT + 3,
            cx.scene,
        );

        let meta = format!(
            "v{}  {:.1} MB",
            row_id.raw() % 7 + 1,
            12.0 + (row_id.raw() % 90) as f64
        );
        let meta_run = cx.text_engine.shape(&meta, 10.0);
        cx.text_engine.draw(
            &meta_run,
            rect.x + 8.0,
            rect.y + 46.0,
            cx.theme.colors.typography.secondary,
            LAYER_CONTENT + 3,
            cx.scene,
        );
    }
}

impl ShowcaseScreen {
    pub(super) fn build_navigator(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let name_col = ColumnId::new();
        let source = AssetGridSource {
            row_ids: (0..50_000).map(|_| RowId::new()).collect(),
            name_col,
        };
        let painter: Rc<dyn ItemPainter> = Rc::new(AssetTilePainter { name_col });

        let navigator = Navigator::new(source, painter)
            .with_policy(NavigatorPolicy::multi_select())
            .with_config(NavigatorConfig {
                fill_width: true,
                ..NavigatorConfig::for_mode(NavigatorViewMode::Tile)
            });
        let body = navigator.create_body();

        let navigator_id = cx.tree.add_child(parent, Box::new(navigator), None, 0);
        cx.tree.set_flex(navigator_id, FlexParams::flexible(1.0));

        let scroll_area = ScrollArea::new_unwired().with_scrollbars(true);
        let scroll_id = cx
            .tree
            .add_child(navigator_id, Box::new(scroll_area), None, 0);
        let body_id = cx.tree.add_child(scroll_id, Box::new(body), None, 0);

        if let Some(node) = cx.tree.node_mut(scroll_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }
    }
}
