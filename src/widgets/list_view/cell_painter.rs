// SPEC-HAL-310 prerequisite: CellPainter delegate for custom cell rendering.
//
// Default: text rendering (current behavior).
// Consumer overrides for specific columns or all columns.

use crate::core::math::RectF;
use crate::core::text_engine::GlyphRun;
use crate::core::widget::PaintContext;
use crate::widgets::tabular_view::column::ColumnId;
use crate::widgets::tabular_view::data_adapter::{CellValue, ValueState};
use crate::widgets::tabular_view::row::RowId;

/// Canonical cell value -> display text mapping. Single source of truth shared
/// by realization-time shaping (ListViewBody) and the paint-time fallback.
pub(crate) fn cell_value_text(value: &ValueState<CellValue>) -> String {
    match value {
        ValueState::Ready(CellValue::String(s)) => s.clone(),
        ValueState::Ready(CellValue::Int(n)) => format!("{}", n),
        ValueState::Ready(CellValue::Float(f)) => format!("{:.2}", f),
        ValueState::Ready(CellValue::Bool(b)) => format!("{}", b),
        ValueState::Ready(CellValue::Null) => "\u{2014}".to_string(),
        ValueState::Pending => "...".to_string(),
        ValueState::Missing => String::new(),
        ValueState::Denied { .. } => "[denied]".to_string(),
        ValueState::Error { .. } => "[error]".to_string(),
    }
}

/// Delegate for custom cell rendering in ListView.
///
/// Implement this trait to control how individual cells are painted.
/// The default implementation renders text (identical to the current
/// hardcoded behavior).
pub trait CellPainter {
    /// Paint a single cell.
    ///
    /// `cx` — paint context (scene, text_engine, theme)
    /// `rect` — the cell's clip rect (already positioned)
    /// `row_id` — which row
    /// `column_id` — which column
    /// `value` — the cell's data value from DataSource
    /// `selected` — whether this row is in the selection set
    /// `font_size` — current theme font size
    /// `text_padding` — current theme text padding
    fn paint_cell(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        row_id: RowId,
        column_id: ColumnId,
        value: ValueState<CellValue>,
        shaped: Option<&GlyphRun>,
        selected: bool,
        font_size: f32,
        text_padding: f32,
    );

    /// Paint decoration in the expansion region for a specific column.
    ///
    /// Called by ListViewBody for each column of each expanded row.
    /// `rect` is the expansion region rect for this column (below the collapsed strip).
    /// Default: no-op. Override for column-specific expansion decorations
    /// (e.g., stretched severity indicators, background bands).
    fn paint_expansion(
        &self,
        _cx: &mut PaintContext,
        _rect: RectF,
        _row_id: RowId,
        _column_id: ColumnId,
    ) {
    }
}

/// Default cell painter — renders cell values as text.
/// This preserves the exact current ListView behavior.
pub struct DefaultCellPainter;

impl CellPainter for DefaultCellPainter {
    fn paint_cell(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        _row_id: RowId,
        _column_id: ColumnId,
        value: ValueState<CellValue>,
        shaped: Option<&GlyphRun>,
        selected: bool,
        font_size: f32,
        text_padding: f32,
    ) {
        let list_colors = &cx.theme.colors.list;
        let text_color = if selected {
            list_colors.text_selected
        } else {
            list_colors.text
        };

        // SPEC-PM-016 R1/R2: paint the run resolved at realization. Shape only
        // as a fallback, when no realized run was supplied (non-ListView callers).
        if let Some(run) = shaped {
            let text_y = rect.y + (rect.height - run.height) / 2.0;
            cx.scene.push_text_with_backgrounds(
                run,
                rect.x + text_padding,
                text_y,
                text_color,
                Vec::new(),
                3,
                None,
            );
        } else {
            let text = cell_value_text(&value);
            let run = cx.text_engine.shape(&text, font_size);
            let text_y = rect.y + (rect.height - run.height) / 2.0;
            cx.scene.push_text_with_backgrounds(
                &run,
                rect.x + text_padding,
                text_y,
                text_color,
                Vec::new(),
                3,
                None,
            );
        }
    }
}
