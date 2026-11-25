// SPEC-HAL-312: EventLog — ListView composition.

use std::cell::RefCell;
use std::rc::Rc;

use crate::core::text_selection::TextSelectionPolicy;
use crate::model::tabular::row_expansion::{ExpandedHeightPolicy, ExpansionPolicy};
use crate::widgets::list_view::{ListView, ListViewBody, ListViewFooter};
use crate::widgets::tabular_view::column::{Column, ColumnScrollDomain};
use crate::widgets::tabular_view::data_adapter::DataSource;
use crate::widgets::tabular_view::row::RowId;
use crate::widgets::text_block::TextBlock;

use super::data_source::{EventLogDataSource, SharedEventLogSource, TimestampFormatter};
use super::severity_painter::EventLogCellPainter;
use super::types::EventLogEntry;

// ─── Policy & Config ─────────────────────────────────────

/// SPEC-HAL-312 §7: Interaction policy.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLogPolicy {
    /// Read-only display, no interaction.
    DisplayOnly,
    /// Row selection + keyboard navigation + copy. Default for Phase 1.
    Selectable,
    /// Full interaction including future actions (Phase 4).
    Interactive,
}

impl Default for EventLogPolicy {
    fn default() -> Self {
        EventLogPolicy::Selectable
    }
}

/// Column widths per SPEC-HAL-312 §5.
const SEVERITY_COLUMN_WIDTH: f32 = 24.0;
const TIMESTAMP_COLUMN_WIDTH: f32 = 110.0;
const SOURCE_COLUMN_WIDTH: f32 = 120.0;
const MESSAGE_COLUMN_WIDTH: f32 = 400.0;

pub struct EventLogConfig {
    pub policy: EventLogPolicy,
    pub row_height: f32,
    pub font_size: f32,
    pub show_source_column: bool,
    pub timestamp_formatter: Option<TimestampFormatter>,
    /// Maximum entries. Phase 1 uses unbounded Vec; capacity is stored for
    /// future ring-buffer enforcement.
    pub capacity: usize,
    /// Row expansion policy. Default: SingleExpand.
    pub expansion_policy: ExpansionPolicy,
    /// Height of the detail panel when expanded. Default: 120.0.
    pub detail_panel_height: f32,
}

impl Default for EventLogConfig {
    fn default() -> Self {
        Self {
            policy: EventLogPolicy::default(),
            row_height: 24.0,
            font_size: 13.0,
            show_source_column: true,
            timestamp_formatter: None,
            capacity: 10_000,
            expansion_policy: ExpansionPolicy::SingleExpand,
            detail_panel_height: 160.0,
        }
    }
}

// ─── EventLog Widget ─────────────────────────────────────

pub struct EventLog {
    list_view: Option<ListView<SharedEventLogSource>>,
    /// Shared reference to the data source for consumer push/clear.
    source: Rc<RefCell<EventLogDataSource>>,
    config: EventLogConfig,
    source_column_id: crate::widgets::tabular_view::column::ColumnId,
}

impl EventLog {
    pub fn new(config: EventLogConfig) -> Self {
        // Build inner data source.
        let inner = if let Some(ref fmt) = config.timestamp_formatter {
            EventLogDataSource::new().with_timestamp_formatter(fmt.clone())
        } else {
            EventLogDataSource::new()
        };
        let source = Rc::new(RefCell::new(inner));

        // Build shared wrapper for ListView.
        let shared = SharedEventLogSource::new(source.clone());
        let mut list_view = ListView::new(shared);

        // Create columns.
        let sev_col = Column::new(SEVERITY_COLUMN_WIDTH)
            .with_min_width(SEVERITY_COLUMN_WIDTH)
            .with_max_width(SEVERITY_COLUMN_WIDTH)
            .with_scroll_domain(ColumnScrollDomain::PinnedLeft)
            .with_resizable(false)
            .with_reorderable(false);
        let severity_id = list_view.add_column_raw("", sev_col);

        let ts_col = Column::new(TIMESTAMP_COLUMN_WIDTH).with_reorderable(false);
        let timestamp_id = list_view.add_column_raw("Time", ts_col);

        let source_col = Column::new(SOURCE_COLUMN_WIDTH).with_visible(config.show_source_column);
        let source_id = list_view.add_column_raw("Source", source_col);

        let msg_col = Column::new(MESSAGE_COLUMN_WIDTH).with_flex(1.0);
        let message_id = list_view.add_column_raw("Message", msg_col);

        // Register column mapping on the data source.
        source
            .borrow_mut()
            .set_columns(severity_id, timestamp_id, source_id, message_id);

        // SPEC-HAL-312A: Install expansion-aware painter with all column IDs.
        let painter = EventLogCellPainter::new(
            severity_id,
            timestamp_id,
            source_id,
            message_id,
            source.clone(),
        );
        list_view.set_cell_painter(Box::new(painter));

        // SPEC-HAL-313: Configure expansion from EventLogConfig.
        list_view.set_expansion_policy(config.expansion_policy);
        list_view.set_expanded_height_policy(ExpandedHeightPolicy::Natural);

        // SPEC-HAL-313: Default row widget factory.
        // TextBlock wraps text and handles newlines. Selection will be
        // added when SPEC-HAL-315 extends to TextBlock.
        let factory_source = source.clone();
        list_view.set_row_widget_factory(move |row_id: RowId| {
            let text = {
                let ds = factory_source.borrow();
                ds.get(row_id)
                    .map(|e| e.detail.as_deref().unwrap_or(&e.message).to_string())
                    .unwrap_or_default()
            };
            Box::new(
                TextBlock::new(text)
                    .with_size(13.0)
                    .with_selection_policy(TextSelectionPolicy::Enabled),
            )
        });

        Self {
            list_view: Some(list_view),
            source,
            config,
            source_column_id: source_id,
        }
    }

    fn lv(&self) -> &ListView<SharedEventLogSource> {
        self.list_view.as_ref().expect("ListView already taken")
    }

    fn lv_mut(&mut self) -> &mut ListView<SharedEventLogSource> {
        self.list_view.as_mut().expect("ListView already taken")
    }

    // ── Consumer API ─────────────────────────────────────

    /// Append an entry.
    pub fn push(&self, entry: EventLogEntry) {
        self.source.borrow_mut().push(entry);
    }

    /// Clear all entries.
    pub fn clear(&self) {
        self.source.borrow_mut().clear();
    }

    /// Read-only access to config.
    pub fn config(&self) -> &EventLogConfig {
        &self.config
    }

    /// Access the underlying ListView (before `take_list_view`).
    pub fn list_view(&self) -> &ListView<SharedEventLogSource> {
        self.lv()
    }

    /// Mutable access to the underlying ListView (before `take_list_view`).
    pub fn list_view_mut(&mut self) -> &mut ListView<SharedEventLogSource> {
        self.lv_mut()
    }

    /// Take ownership of the ListView for widget tree insertion.
    ///
    /// After this call, `list_view()` / `list_view_mut()` will panic.
    /// Use `push()` / `clear()` / `format_rows_for_copy()` on EventLog directly.
    pub fn take_list_view(&mut self) -> ListView<SharedEventLogSource> {
        self.list_view.take().expect("ListView already taken")
    }

    /// Create the body widget for tree construction.
    pub fn create_body_widget(&self) -> ListViewBody<SharedEventLogSource> {
        self.lv().create_body_widget()
    }

    /// Create the footer widget for tree construction.
    pub fn create_footer_widget(&mut self) -> ListViewFooter<SharedEventLogSource> {
        self.lv_mut().create_footer_widget()
    }

    /// Set source column visibility at runtime.
    pub fn set_source_visible(&mut self, visible: bool) {
        let col_id = self.source_column_id;
        self.lv_mut().set_column_visible(col_id, visible);
    }

    /// Build structured copy text for the given row IDs.
    ///
    /// Format: `[HH:MM:SS.mmm] [LEVEL] [source] message` per row, joined by `\n`.
    /// Callers should pass selected row IDs from the ListView's selection state.
    pub fn format_rows_for_copy(
        &self,
        selected_row_ids: &[crate::widgets::tabular_view::row::RowId],
    ) -> String {
        let source = self.source.borrow();
        let fmt = source.timestamp_formatter().clone();
        let row_ids = source.row_ids();

        // Intersect with row_ids to preserve display order.
        let selected_set: std::collections::HashSet<_> = selected_row_ids.iter().copied().collect();
        let mut lines: Vec<String> = Vec::new();
        for &rid in row_ids {
            if selected_set.contains(&rid) {
                if let Some(entry) = source.get(rid) {
                    let ts = fmt(entry.timestamp_ms);
                    let line = format!(
                        "[{}] [{}] [{}] {}",
                        ts,
                        entry.level.tag().trim(),
                        entry.source,
                        entry.message,
                    );
                    lines.push(line);
                }
            }
        }
        lines.join("\n")
    }
}
