// SPEC-HAL-312 §8.1: Severity Column Rendering

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::math::RectF;
use crate::core::paint::Color;
use crate::core::text_engine::GlyphRun;
use crate::core::widget::PaintContext;
use crate::widgets::list_view::{CellPainter, DefaultCellPainter};
use crate::widgets::tabular_view::column::ColumnId;
use crate::widgets::tabular_view::data_adapter::{CellValue, ValueState};
use crate::widgets::tabular_view::row::RowId;

use super::data_source::{EventLogDataSource, SeverityLookup};
use super::types::EventLogLevel;

/// Paints all EventLog cells.
///
/// SPEC-HAL-312 §8.1: Severity column renders a colored indicator.
/// Other columns delegate to DefaultCellPainter.
/// SPEC-HAL-313: paint_expansion renders a vertical severity band
/// in the expansion region.
/// FORBID-312A-PAINT-TIME-STATE: Only reads from the source — never mutates.
pub struct EventLogCellPainter {
    severity_column: ColumnId,
    timestamp_column: ColumnId,
    source_column: ColumnId,
    message_column: ColumnId,
    source: Rc<RefCell<EventLogDataSource>>,
    default: DefaultCellPainter,
}

impl EventLogCellPainter {
    pub fn new(
        severity_column: ColumnId,
        timestamp_column: ColumnId,
        source_column: ColumnId,
        message_column: ColumnId,
        source: Rc<RefCell<EventLogDataSource>>,
    ) -> Self {
        Self {
            severity_column,
            timestamp_column,
            source_column,
            message_column,
            source,
            default: DefaultCellPainter,
        }
    }

    fn level_color(level: EventLogLevel) -> Color {
        match level {
            EventLogLevel::Trace => Color::from_hex(0x7F8C8D), // muted gray
            EventLogLevel::Debug => Color::from_hex(0x95A5A6), // lighter gray
            EventLogLevel::Info => Color::from_hex(0x3498DB),  // blue
            EventLogLevel::Warning => Color::from_hex(0xF39C12), // amber
            EventLogLevel::Error => Color::from_hex(0xE74C3C), // red
            EventLogLevel::Critical => Color::from_hex(0xE74C3C), // red (filled + bold in later phase)
        }
    }

    /// Indicator size (px) by severity level. Used by both paint_severity_indicator
    /// and paint_expansion to keep the square and the vertical band the same width.
    fn indicator_size(level: EventLogLevel) -> f32 {
        match level {
            EventLogLevel::Trace | EventLogLevel::Debug | EventLogLevel::Info => 6.0,
            EventLogLevel::Warning => 8.0,
            EventLogLevel::Error | EventLogLevel::Critical => 10.0,
        }
    }

    /// Paint the severity indicator (colored square) centered in the given rect.
    fn paint_severity_indicator(&self, cx: &mut PaintContext, rect: RectF, level: EventLogLevel) {
        let color = Self::level_color(level);
        let cx_center = rect.x + rect.width / 2.0;
        let cy_center = rect.y + rect.height / 2.0;
        let size = Self::indicator_size(level);
        let indicator = RectF::new(cx_center - size / 2.0, cy_center - size / 2.0, size, size);
        cx.scene.push_rect(indicator, color, 3, None);
    }
}

impl CellPainter for EventLogCellPainter {
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
    ) {
        let source = self.source.borrow();
        let level = source.level_for_row(row_id);

        if column_id == self.severity_column {
            if let Some(level) = level {
                self.paint_severity_indicator(cx, rect, level);
            }
        } else if column_id == self.timestamp_column
            || column_id == self.source_column
            || column_id == self.message_column
        {
            drop(source);
            self.default.paint_cell(
                cx,
                rect,
                row_id,
                column_id,
                value,
                shaped,
                selected,
                font_size,
                text_padding,
            );
        }
    }

    fn paint_expansion(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        row_id: RowId,
        column_id: ColumnId,
    ) {
        // Severity column: paint a vertical colored band spanning the expansion height.
        if column_id == self.severity_column {
            let source = self.source.borrow();
            if let Some(level) = source.level_for_row(row_id) {
                let color = Self::level_color(level);
                // Match indicator width from paint_severity_indicator, centered in column.
                let size = Self::indicator_size(level);
                let cx_center = rect.x + rect.width / 2.0;
                let band = RectF::new(cx_center - size / 2.0, rect.y, size, rect.height);
                cx.scene.push_rect(band, color, 5, None);
            }
        }
    }
}
