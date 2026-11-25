// SPEC-HAL-310 S5: ItemPainter delegate for custom item rendering.

use crate::core::math::RectF;
use crate::core::widget::PaintContext;
use crate::widgets::tabular_view::data_adapter::DataSource;
use crate::widgets::tabular_view::row::RowId;

/// Consumer-provided delegate for painting item content.
///
/// Navigator calls this for each visible item. The painter
/// owns ALL visuals: background, icon, text, borders,
/// selection highlight, hover effect, focus ring.
///
/// `rect` is the full item rect.
/// `selected` is true if this item is in the selection set.
/// `hovered` is true if the pointer is over this item.
/// `focused` is true if this item has keyboard focus AND
///   the Navigator container has focus.
pub trait ItemPainter {
    fn paint_item(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        row_id: RowId,
        selected: bool,
        hovered: bool,
        focused: bool,
        data: &dyn DataSource,
    );
}
