//! Compact readout widgets for dense, low-interaction data surfaces.

// INV-READOUT-GEOM-1: Readout widgets preserve a compact, table-like geometry
// profile for professional dashboards: 22px data rows, 24px headers/strips,
// 34px status cells, and 1px structural hairlines. These are display-surface
// metrics, not interactive-control metrics; when readout geometry tokens exist,
// these constants should migrate to theme geometry.

use crate::core::event::{Event, EventPhase, KeyCode, PointerButton, PointerEventKind};
use crate::core::layout::{Constraints, LayoutResult};
use crate::core::math::{RectF, Size, snap_to_physical};
use crate::core::paint::{Color, ZIndex};
use crate::core::text_engine::TextEngine;
use crate::core::text_selection::{
    ReadOnlyTextSelection, TextSelectionPolicy, hit_test_run, selection_spans,
};
use crate::core::widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast};
use crate::core::widget_tree::NodeId;
use crate::model::tooltip::TooltipPayload;
use crate::utils::id::WidgetInternalId;

/// Overflow behavior for compact readout text.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReadoutTextOverflow {
    /// Clip text to the cell rect without adding a marker.
    Clip,
    /// Fit visible text and append `...` when the full text does not fit.
    Ellipsis,
}

impl Default for ReadoutTextOverflow {
    fn default() -> Self {
        Self::Ellipsis
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ReadoutTypography {
    label_size: f32,
    value_size: f32,
    label_line_height: f32,
}

impl ReadoutTypography {
    fn from_base_font_size(base_font_size: f32) -> Self {
        let label_size = (base_font_size - 2.0).max(10.0).round();
        let value_size = (base_font_size - 1.0).max(11.0).round();
        Self {
            label_size,
            value_size,
            label_line_height: (label_size * 1.2).ceil().max(label_size.ceil() + 2.0),
        }
    }

    fn from_text_engine(text_engine: &mut dyn TextEngine, base_font_size: f32) -> Self {
        let mut typography = Self::from_base_font_size(base_font_size);
        let metrics = text_engine.font_metrics(typography.label_size);
        typography.label_line_height = metrics.line_height.ceil().max(typography.label_line_height);
        typography
    }

    fn label_region_height(&self) -> f32 {
        self.label_line_height
    }
}

impl Default for ReadoutTypography {
    fn default() -> Self {
        Self::from_base_font_size(13.0)
    }
}

#[derive(Debug, Clone)]
pub struct ReadoutTextRegion {
    key: u64,
    rect: RectF,
    text: String,
    font_size: f32,
    overflow: ReadoutTextOverflow,
}

impl ReadoutTextRegion {
    fn new(
        key: u64,
        rect: RectF,
        text: impl Into<String>,
        font_size: f32,
        overflow: ReadoutTextOverflow,
    ) -> Self {
        Self {
            key,
            rect,
            text: text.into(),
            font_size,
            overflow,
        }
    }
}

struct FittedText {
    display: String,
    source_len: usize,
}

fn readout_key(kind: u64, a: usize, b: usize, c: usize) -> u64 {
    (kind << 56) | ((a as u64) << 32) | ((b as u64) << 16) | c as u64
}

fn readout_table_key(kind: usize, row: usize, column: usize) -> u64 {
    readout_key(kind as u64 + 1, row, column, 0)
}

fn status_grid_key(kind: usize, index: usize) -> u64 {
    readout_key(kind as u64 + 10, index, 0, 0)
}

fn key_value_strip_key(kind: usize, index: usize) -> u64 {
    readout_key(kind as u64 + 20, index, 0, 0)
}

fn selection_enabled(policy: TextSelectionPolicy) -> bool {
    policy != TextSelectionPolicy::Disabled
}

fn one_physical_px(scale: f32) -> f32 {
    1.0 / scale
}

fn finite_readout_width(constraints: Constraints, natural_width: f32) -> f32 {
    let width = if constraints.max_width.is_finite() {
        constraints.max_width
    } else {
        natural_width
    };
    width
        .max(constraints.min_width)
        .min(constraints.max_width)
        .max(0.0)
}

fn readout_border(cx: &mut PaintContext, rect: RectF, color: Color, z: ZIndex) {
    let px = one_physical_px(cx.scene.scale_factor());
    cx.scene.push_border_rect(rect, color, px, z, None);
}

fn readout_horizontal_rule(
    cx: &mut PaintContext,
    x: f32,
    y: f32,
    width: f32,
    color: Color,
    z: ZIndex,
) {
    let scale = cx.scene.scale_factor();
    let x0 = snap_to_physical(x, scale);
    let x1 = snap_to_physical(x + width, scale);
    let y0 = snap_to_physical(y, scale);
    cx.scene.push_rect(
        RectF::new(x0, y0, (x1 - x0).max(0.0), one_physical_px(scale)),
        color,
        z,
        None,
    );
}

fn readout_text_region(
    cx: &mut PaintContext,
    rect: RectF,
    text: &str,
    size: f32,
    color: Color,
    overflow: ReadoutTextOverflow,
    selection: Option<&ReadOnlyTextSelection>,
) {
    let fitted = fit_readout_text(cx.text_engine, text, size, rect.width, overflow);
    let run = cx.text_engine.shape(&fitted.display, size);
    let highlight_spans = selection
        .and_then(|sel| {
            if !sel.has_selection() {
                return None;
            }
            let (start, end) = sel.range();
            let visible_start = start.min(fitted.source_len);
            let visible_end = end.min(fitted.source_len);
            if visible_start >= visible_end {
                return None;
            }
            Some(selection_spans(
                &run,
                visible_start,
                visible_end,
                run.height,
                0.0,
                cx.theme.text_selection_color(),
            ))
        })
        .unwrap_or_default();
    cx.scene.begin_clip(rect, None);
    let y = rect.y + ((rect.height - run.height) * 0.5).max(0.0);
    if highlight_spans.is_empty() {
        cx.text_engine.draw(
            &run,
            rect.x,
            y,
            color,
            crate::core::paint::LAYER_CONTENT + 1,
            cx.scene,
        );
    } else {
        cx.text_engine.draw_with_backgrounds(
            &run,
            rect.x,
            y,
            color,
            highlight_spans,
            crate::core::paint::LAYER_CONTENT + 1,
            cx.scene,
        );
    }
    cx.scene.end_clip();
}

fn fit_readout_text(
    text_engine: &mut dyn TextEngine,
    text: &str,
    size: f32,
    width: f32,
    overflow: ReadoutTextOverflow,
) -> FittedText {
    if width <= 0.0 || text.is_empty() {
        return FittedText {
            display: String::new(),
            source_len: 0,
        };
    }
    if overflow == ReadoutTextOverflow::Clip {
        return FittedText {
            display: text.to_string(),
            source_len: text.len(),
        };
    }

    let full = text_engine.shape(text, size);
    if full.width <= width {
        return FittedText {
            display: text.to_string(),
            source_len: text.len(),
        };
    }

    let suffix = "...";
    if text_engine.shape(suffix, size).width > width {
        return FittedText {
            display: String::new(),
            source_len: 0,
        };
    }

    let mut boundaries: Vec<usize> = text.char_indices().map(|(idx, _)| idx).collect();
    boundaries.push(text.len());
    let mut lo = 0usize;
    let mut hi = boundaries.len() - 1;
    while lo < hi {
        let mid = (lo + hi).div_ceil(2);
        let source_len = boundaries[mid];
        let candidate = format!("{}{}", &text[..source_len], suffix);
        if text_engine.shape(&candidate, size).width <= width {
            lo = mid;
        } else {
            hi = mid - 1;
        }
    }
    let source_len = boundaries[lo];
    FittedText {
        display: format!("{}{}", &text[..source_len], suffix),
        source_len,
    }
}

fn text_offset_for_region(cx: &mut EventContext<'_>, region: &ReadoutTextRegion, x: f32) -> usize {
    let local_x = (x - region.rect.x).max(0.0);
    let fitted = fit_readout_text(
        cx.text_engine(),
        &region.text,
        region.font_size,
        region.rect.width,
        region.overflow,
    );
    let run = cx.text_engine().shape(&fitted.display, region.font_size);
    let offset = hit_test_run(&run, local_x);
    if offset >= fitted.source_len {
        region.text.len()
    } else {
        offset
    }
}

fn handle_readout_selection(
    policy: TextSelectionPolicy,
    active_key: &mut Option<u64>,
    selection: &mut ReadOnlyTextSelection,
    hit_region: Option<ReadoutTextRegion>,
    active_region: Option<ReadoutTextRegion>,
    cx: &mut EventContext<'_>,
    event: &Event,
    phase: EventPhase,
) -> bool {
    if !selection_enabled(policy) {
        return false;
    }

    match event {
        Event::Pointer(pe)
            if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                && matches!(
                    pe.kind,
                    PointerEventKind::Down
                        | PointerEventKind::DoubleClick
                        | PointerEventKind::TripleClick
                )
                && pe.button == Some(PointerButton::Primary) =>
        {
            let Some(region) = hit_region else {
                return false;
            };
            selection.set_text(region.text.clone());
            *active_key = Some(region.key);
            let offset = text_offset_for_region(cx, &region, pe.local_pos.0);
            let shift = pe.modifiers.shift;
            let click_count = match pe.kind {
                PointerEventKind::DoubleClick => 2,
                PointerEventKind::TripleClick => 3,
                _ => cx.click_count(),
            };
            if selection.pointer_down(offset, click_count, shift, cx.has_pointer_capture(), shift) {
                cx.request_focus();
                cx.capture_pointer(PointerButton::Primary);
                return true;
            }
            false
        }
        Event::Pointer(pe)
            if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                && pe.kind == PointerEventKind::Move =>
        {
            if !selection.is_dragging() {
                return false;
            }
            let Some(region) = active_region else {
                return false;
            };
            let offset = text_offset_for_region(cx, &region, pe.local_pos.0);
            selection.pointer_move(offset);
            true
        }
        Event::Pointer(pe)
            if (phase == EventPhase::Target || phase == EventPhase::Bubble)
                && pe.kind == PointerEventKind::Up
                && pe.button == Some(PointerButton::Primary) =>
        {
            if selection.is_dragging() {
                selection.pointer_up();
                cx.release_pointer();
                return true;
            }
            false
        }
        Event::Key(ke)
            if phase == EventPhase::Target && ke.is_down && ke.key_code == KeyCode::Escape =>
        {
            if selection.has_selection() {
                selection.clear();
                *active_key = None;
                return true;
            }
            false
        }
        Event::Text(te) if phase == EventPhase::Target => {
            #[cfg(target_os = "macos")]
            let shortcut_mod = te.modifiers.meta;
            #[cfg(not(target_os = "macos"))]
            let shortcut_mod = te.modifiers.ctrl;

            if !shortcut_mod {
                return false;
            }
            match te.text.as_str() {
                "c" | "C" => {
                    if let Some(text) = selection.copy_to_clipboard() {
                        cx.clipboard_set_text(&text);
                        return true;
                    }
                    false
                }
                "a" | "A" => {
                    if active_key.is_some() {
                        selection.select_all();
                        return true;
                    }
                    false
                }
                _ => false,
            }
        }
        _ => false,
    }
}

/// A proportional column definition for [`ReadoutTable`].
#[derive(Clone, Debug)]
pub struct ReadoutColumn {
    title: String,
    width: f32,
}

impl ReadoutColumn {
    /// Creates a column with a proportional base width.
    pub fn new(title: impl Into<String>, width: f32) -> Self {
        Self {
            title: title.into(),
            width,
        }
    }
}

/// A compact text cell for [`ReadoutTable`].
#[derive(Clone, Debug)]
pub struct ReadoutCell {
    text: String,
    strong: bool,
}

impl ReadoutCell {
    /// Creates a table cell. Strong cells use primary text color.
    pub fn new(text: impl Into<String>, strong: bool) -> Self {
        Self {
            text: text.into(),
            strong,
        }
    }

    /// Creates a secondary-weight cell.
    pub fn plain(text: impl Into<String>) -> Self {
        Self::new(text, false)
    }

    /// Creates a primary-weight cell.
    pub fn strong(text: impl Into<String>) -> Self {
        Self::new(text, true)
    }
}

/// Paint-only compact table for small readouts, ledgers, and state matrices.
pub struct ReadoutTable {
    id: WidgetInternalId,
    columns: Vec<ReadoutColumn>,
    rows: Vec<Vec<ReadoutCell>>,
    overflow: ReadoutTextOverflow,
    selection_policy: TextSelectionPolicy,
    active_selection_key: Option<u64>,
    text_selection: ReadOnlyTextSelection,
    layout_size: Size,
}

impl ReadoutTable {
    /// Creates a compact table with a header band and zebra rows.
    pub fn new(columns: Vec<ReadoutColumn>, rows: Vec<Vec<ReadoutCell>>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            columns,
            rows,
            overflow: ReadoutTextOverflow::default(),
            selection_policy: TextSelectionPolicy::Disabled,
            active_selection_key: None,
            text_selection: ReadOnlyTextSelection::disabled(),
            layout_size: Size::zero(),
        }
    }

    /// Sets how text that exceeds its cell should be displayed.
    pub fn with_overflow(mut self, overflow: ReadoutTextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Enables or disables read-only text selection.
    pub fn with_selection_policy(mut self, policy: TextSelectionPolicy) -> Self {
        self.selection_policy = policy;
        self.text_selection.set_policy(policy);
        self
    }

    /// Replaces the table rows while preserving the widget instance.
    pub fn set_rows(&mut self, rows: Vec<Vec<ReadoutCell>>) {
        self.rows = rows;
        self.refresh_active_selection_text();
    }

    fn natural_width(&self) -> f32 {
        let column_width: f32 = self.columns.iter().map(|column| column.width).sum();
        column_width + 16.0
    }

    /// Paints a compact table into an existing readout surface.
    pub fn paint_table(&self, cx: &mut PaintContext, rect: RectF) {
        let colors = &cx.theme.colors;
        let header_h = 24.0;
        let row_h = 22.0;
        let pad_x = 8.0;
        let total_width: f32 = self.columns.iter().map(|column| column.width).sum();
        let scale = if total_width > 0.0 {
            ((rect.width - pad_x * 2.0) / total_width).max(0.35)
        } else {
            1.0
        };

        cx.scene.push_rect(rect, colors.surfaces.base0, 1, None);
        readout_border(cx, rect, colors.borders.subtle, 4);
        cx.scene.push_rect(
            RectF::new(rect.x + 1.0, rect.y + 1.0, rect.width - 2.0, header_h),
            colors.surfaces.level2,
            2,
            None,
        );
        readout_horizontal_rule(
            cx,
            rect.x + 1.0,
            rect.y + header_h,
            rect.width - 2.0,
            colors.borders.default,
            4,
        );

        let mut x = rect.x + pad_x;
        for (column_index, column) in self.columns.iter().enumerate() {
            let width = column.width * scale;
            readout_text_region(
                cx,
                RectF::new(x, rect.y + 3.0, width - 4.0, header_h - 6.0),
                &column.title,
                10.5,
                colors.typography.primary,
                self.overflow,
                self.selection_for_key(readout_table_key(0, 0, column_index)),
            );
            x += width;
        }

        for (row_index, row) in self.rows.iter().enumerate() {
            let y = rect.y + header_h + row_index as f32 * row_h + 1.0;
            let fill = if row_index % 2 == 0 {
                colors.surfaces.level1
            } else {
                colors.surfaces.base0
            };
            cx.scene.push_rect(
                RectF::new(rect.x + 1.0, y, rect.width - 2.0, row_h),
                fill,
                1,
                None,
            );
            readout_horizontal_rule(
                cx,
                rect.x + 1.0,
                y + row_h - 1.0,
                rect.width - 2.0,
                colors.borders.subtle,
                3,
            );

            let mut x = rect.x + pad_x;
            for (column_index, (cell, column)) in row.iter().zip(self.columns.iter()).enumerate() {
                let width = column.width * scale;
                let color = if cell.strong {
                    colors.typography.primary
                } else {
                    colors.typography.secondary
                };
                readout_text_region(
                    cx,
                    RectF::new(x, y + 2.0, width - 4.0, row_h - 4.0),
                    &cell.text,
                    12.0,
                    color,
                    self.overflow,
                    self.selection_for_key(readout_table_key(1, row_index, column_index)),
                );
                x += width;
            }
        }
    }

    fn selection_for_key(&self, key: u64) -> Option<&ReadOnlyTextSelection> {
        if self.active_selection_key == Some(key) {
            Some(&self.text_selection)
        } else {
            None
        }
    }

    fn text_region_at(&self, x: f32, y: f32) -> Option<ReadoutTextRegion> {
        self.text_region_matching(|_, rect| rect.contains_point(x, y))
    }

    fn text_region_by_key(&self, key: u64) -> Option<ReadoutTextRegion> {
        self.text_region_matching(|candidate, _| candidate == key)
    }

    fn refresh_active_selection_text(&mut self) {
        let Some(key) = self.active_selection_key else {
            return;
        };
        if let Some(region) = self.text_region_by_key(key) {
            self.text_selection.set_text(region.text);
        } else {
            self.active_selection_key = None;
            self.text_selection.clear();
        }
    }

    fn text_region_matching(
        &self,
        mut matches: impl FnMut(u64, RectF) -> bool,
    ) -> Option<ReadoutTextRegion> {
        let rect = RectF::new(0.0, 0.0, self.layout_size.width, self.layout_size.height);
        let header_h = 24.0;
        let row_h = 22.0;
        let pad_x = 8.0;
        let total_width: f32 = self.columns.iter().map(|column| column.width).sum();
        let scale = if total_width > 0.0 {
            ((rect.width - pad_x * 2.0) / total_width).max(0.35)
        } else {
            1.0
        };

        let mut x = rect.x + pad_x;
        for (column_index, column) in self.columns.iter().enumerate() {
            let width = column.width * scale;
            let text_rect = RectF::new(x, rect.y + 3.0, width - 4.0, header_h - 6.0);
            let key = readout_table_key(0, 0, column_index);
            if matches(key, text_rect) {
                return Some(ReadoutTextRegion::new(
                    key,
                    text_rect,
                    column.title.clone(),
                    10.5,
                    self.overflow,
                ));
            }
            x += width;
        }

        for (row_index, row) in self.rows.iter().enumerate() {
            let y = rect.y + header_h + row_index as f32 * row_h + 1.0;
            let mut x = rect.x + pad_x;
            for (column_index, (cell, column)) in row.iter().zip(self.columns.iter()).enumerate() {
                let width = column.width * scale;
                let text_rect = RectF::new(x, y + 2.0, width - 4.0, row_h - 4.0);
                let key = readout_table_key(1, row_index, column_index);
                if matches(key, text_rect) {
                    return Some(ReadoutTextRegion::new(
                        key,
                        text_rect,
                        cell.text.clone(),
                        12.0,
                        self.overflow,
                    ));
                }
                x += width;
            }
        }
        None
    }

    fn clear_active_selection(&mut self) -> bool {
        if self.active_selection_key.is_none()
            && !self.text_selection.has_selection()
            && !self.text_selection.is_dragging()
        {
            return false;
        }
        self.active_selection_key = None;
        self.text_selection.clear();
        true
    }
}

impl WidgetDowncast for ReadoutTable {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for ReadoutTable {
    fn layout(
        &mut self,
        _cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        let height = 24.0 + self.rows.len() as f32 * 22.0 + 2.0;
        self.layout_size = Size::new(
            finite_readout_width(constraints, self.natural_width()),
            height,
        );
        LayoutResult::fit(self.layout_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.paint_table(cx, rect);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let (hit_region, active_region) = match event {
            Event::Pointer(pe) => (
                self.text_region_at(pe.local_pos.0, pe.local_pos.1),
                self.active_selection_key
                    .and_then(|key| self.text_region_by_key(key)),
            ),
            _ => (
                None,
                self.active_selection_key
                    .and_then(|key| self.text_region_by_key(key)),
            ),
        };
        handle_readout_selection(
            self.selection_policy,
            &mut self.active_selection_key,
            &mut self.text_selection,
            hit_region,
            active_region,
            cx,
            event,
            phase,
        )
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        selection_enabled(self.selection_policy)
    }

    fn focusable(&self) -> bool {
        selection_enabled(self.selection_policy)
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.clear_active_selection();
    }

    fn clear_read_only_text_selection(&mut self) -> bool {
        self.clear_active_selection()
    }

    fn debug_type_name(&self) -> &'static str {
        "ReadoutTable"
    }
}

/// A label/value field inside [`StatusGrid`].
#[derive(Clone, Debug)]
pub struct StatusGridCell {
    label: String,
    value: String,
    tooltip: Option<TooltipPayload>,
    tone: Option<StatusGridTone>,
}

impl StatusGridCell {
    /// Creates a status cell.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            tooltip: None,
            tone: None,
        }
    }

    /// Adds informational hover text for this cell.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(TooltipPayload::Text(tooltip.into()));
        self
    }

    /// Adds an auxiliary semantic tone while retaining the textual value.
    pub fn with_tone(mut self, tone: StatusGridTone) -> Self {
        self.tone = Some(tone);
        self
    }
}

/// A full-width label/value row inside [`StatusGrid`].
#[derive(Clone, Debug)]
pub struct StatusGridWideRow {
    label: String,
    value: String,
    tooltip: Option<TooltipPayload>,
    tone: Option<StatusGridTone>,
}

impl StatusGridWideRow {
    /// Creates a full-width status row.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            tooltip: None,
            tone: None,
        }
    }

    /// Adds informational hover text for this row.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(TooltipPayload::Text(tooltip.into()));
        self
    }

    /// Adds an auxiliary semantic tone while retaining the textual value.
    pub fn with_tone(mut self, tone: StatusGridTone) -> Self {
        self.tone = Some(tone);
        self
    }
}

/// A compact repeated label/value metric inside [`StatusGrid`].
#[derive(Clone, Debug)]
pub struct StatusGridMetric {
    label: String,
    value: String,
    tooltip: Option<TooltipPayload>,
    tone: Option<StatusGridTone>,
}

impl StatusGridMetric {
    /// Creates a compact metric pair.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
            tooltip: None,
            tone: None,
        }
    }

    /// Adds informational hover text for this metric.
    pub fn with_tooltip(mut self, tooltip: impl Into<String>) -> Self {
        self.tooltip = Some(TooltipPayload::Text(tooltip.into()));
        self
    }

    /// Adds an auxiliary semantic tone while retaining the textual value.
    pub fn with_tone(mut self, tone: StatusGridTone) -> Self {
        self.tone = Some(tone);
        self
    }
}

/// Theme-backed semantic tone for a [`StatusGrid`] item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StatusGridTone {
    Info,
    Success,
    Warning,
    Error,
}

/// Visual treatment used to present a [`StatusGridTone`].
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusGridTonePresentation {
    /// A compact square immediately before the value text.
    #[default]
    InlineMarker,
    /// A narrow semantic rail along the owning item's left edge.
    EdgeRail,
}

const STATUS_GRID_MARKER_SIZE: f32 = 5.0;
const STATUS_GRID_MARKER_GAP: f32 = 5.0;
const STATUS_GRID_RAIL_WIDTH: f32 = 3.0;

fn status_grid_value_text_rect(
    rect: RectF,
    tone: Option<StatusGridTone>,
    presentation: StatusGridTonePresentation,
) -> RectF {
    if tone.is_none() || presentation == StatusGridTonePresentation::EdgeRail {
        return rect;
    }
    let reserve = STATUS_GRID_MARKER_SIZE + STATUS_GRID_MARKER_GAP;
    RectF::new(
        rect.x + reserve,
        rect.y,
        (rect.width - reserve).max(0.0),
        rect.height,
    )
}

fn status_grid_marker_rect(rect: RectF) -> RectF {
    RectF::new(
        rect.x,
        rect.y + ((rect.height - STATUS_GRID_MARKER_SIZE) * 0.5).max(0.0),
        STATUS_GRID_MARKER_SIZE,
        STATUS_GRID_MARKER_SIZE,
    )
}

fn status_grid_tone_rect(
    item_rect: RectF,
    value_rect: RectF,
    presentation: StatusGridTonePresentation,
) -> RectF {
    match presentation {
        StatusGridTonePresentation::InlineMarker => status_grid_marker_rect(value_rect),
        StatusGridTonePresentation::EdgeRail => RectF::new(
            item_rect.x,
            item_rect.y,
            STATUS_GRID_RAIL_WIDTH.min(item_rect.width),
            item_rect.height,
        ),
    }
}

fn status_grid_tone_color(cx: &PaintContext, tone: StatusGridTone) -> Color {
    match tone {
        StatusGridTone::Info => cx.theme.colors.semantic.info,
        StatusGridTone::Success => cx.theme.colors.semantic.success,
        StatusGridTone::Warning => cx.theme.colors.semantic.warning,
        StatusGridTone::Error => cx.theme.colors.semantic.error,
    }
}

fn paint_status_grid_value(
    cx: &mut PaintContext,
    item_rect: RectF,
    rect: RectF,
    value: &str,
    size: f32,
    overflow: ReadoutTextOverflow,
    tone: Option<StatusGridTone>,
    presentation: StatusGridTonePresentation,
    selection: Option<&ReadOnlyTextSelection>,
) {
    if let Some(tone) = tone {
        cx.scene.push_rect(
            status_grid_tone_rect(item_rect, rect, presentation),
            status_grid_tone_color(cx, tone),
            5,
            None,
        );
    }
    readout_text_region(
        cx,
        status_grid_value_text_rect(rect, tone, presentation),
        value,
        size,
        cx.theme.colors.typography.primary,
        overflow,
        selection,
    );
}

fn status_grid_cell_label_rect(cell: RectF, typography: ReadoutTypography) -> RectF {
    let height = typography.label_region_height();
    RectF::new(
        cell.x + 7.0,
        cell.y + 10.0 - height * 0.5,
        cell.width - 14.0,
        height,
    )
}

fn status_grid_wide_label_rect(row_rect: RectF, typography: ReadoutTypography) -> RectF {
    let height = typography.label_region_height();
    RectF::new(
        row_rect.x + 7.0,
        row_rect.y + 13.0 - height * 0.5,
        82.0,
        height,
    )
}

fn register_status_grid_tooltip(
    cx: &mut PaintContext,
    rect: RectF,
    tooltip: Option<&TooltipPayload>,
) {
    if let Some(tooltip) = tooltip {
        cx.register_tooltip(rect, tooltip.clone());
    }
}

/// Paint-only status dashboard for small state readouts.
pub struct StatusGrid {
    id: WidgetInternalId,
    columns: usize,
    cells: Vec<StatusGridCell>,
    wide_rows: Vec<StatusGridWideRow>,
    metrics: Vec<StatusGridMetric>,
    tone_presentation: StatusGridTonePresentation,
    overflow: ReadoutTextOverflow,
    selection_policy: TextSelectionPolicy,
    active_selection_key: Option<u64>,
    text_selection: ReadOnlyTextSelection,
    layout_size: Size,
    typography: ReadoutTypography,
}

impl StatusGrid {
    /// Creates a status grid with three columns.
    pub fn new(cells: Vec<StatusGridCell>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            columns: 3,
            cells,
            wide_rows: Vec::new(),
            metrics: Vec::new(),
            tone_presentation: StatusGridTonePresentation::default(),
            overflow: ReadoutTextOverflow::default(),
            selection_policy: TextSelectionPolicy::Disabled,
            active_selection_key: None,
            text_selection: ReadOnlyTextSelection::disabled(),
            layout_size: Size::zero(),
            typography: ReadoutTypography::default(),
        }
    }

    /// Sets the number of status-cell columns.
    pub fn with_columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// Sets full-width rows.
    pub fn with_wide_rows(mut self, rows: Vec<StatusGridWideRow>) -> Self {
        self.wide_rows = rows;
        self
    }

    /// Sets compact metrics.
    pub fn with_metrics(mut self, metrics: Vec<StatusGridMetric>) -> Self {
        self.metrics = metrics;
        self
    }

    /// Sets how semantic tones are presented across this grid.
    pub fn with_tone_presentation(mut self, presentation: StatusGridTonePresentation) -> Self {
        self.tone_presentation = presentation;
        self
    }

    /// Sets how text that exceeds its readout rect should be displayed.
    pub fn with_overflow(mut self, overflow: ReadoutTextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Enables or disables read-only text selection.
    pub fn with_selection_policy(mut self, policy: TextSelectionPolicy) -> Self {
        self.selection_policy = policy;
        self.text_selection.set_policy(policy);
        self
    }

    /// Replaces the status cells while preserving the readout widget instance.
    pub fn set_cells(&mut self, cells: Vec<StatusGridCell>) {
        self.cells = cells;
        self.refresh_active_selection_text();
    }

    /// Replaces the full-width status rows while preserving the widget instance.
    pub fn set_wide_rows(&mut self, rows: Vec<StatusGridWideRow>) {
        self.wide_rows = rows;
        self.refresh_active_selection_text();
    }

    /// Replaces the compact metric row while preserving the widget instance.
    pub fn set_metrics(&mut self, metrics: Vec<StatusGridMetric>) {
        self.metrics = metrics;
        self.refresh_active_selection_text();
    }

    /// Replaces all projected readout data in one pass.
    pub fn set_readout(
        &mut self,
        cells: Vec<StatusGridCell>,
        wide_rows: Vec<StatusGridWideRow>,
        metrics: Vec<StatusGridMetric>,
    ) {
        self.cells = cells;
        self.wide_rows = wide_rows;
        self.metrics = metrics;
        self.refresh_active_selection_text();
    }

    /// Computes the natural height for the configured readout.
    pub fn natural_height(&self) -> f32 {
        status_grid_height(
            self.columns,
            self.cells.len(),
            self.wide_rows.len(),
            !self.metrics.is_empty(),
        )
    }

    fn natural_width(&self) -> f32 {
        let columns = self.columns.max(1);
        let pad = 8.0;
        let gap = 6.0;
        pad * 2.0 + columns as f32 * 80.0 + gap * columns.saturating_sub(1) as f32
    }

    /// Paints a status grid into an existing readout surface.
    pub fn paint_readout(&self, cx: &mut PaintContext, rect: RectF) {
        Self::paint_snapshot_with_selection(
            cx,
            rect,
            self.columns,
            &self.cells,
            &self.wide_rows,
            &self.metrics,
            self.tone_presentation,
            self.overflow,
            self.active_selection_key,
            Some(&self.text_selection),
        );
    }

    /// Paints status-grid data without requiring ownership by a widget node.
    pub fn paint_snapshot(
        cx: &mut PaintContext,
        rect: RectF,
        columns: usize,
        cells: &[StatusGridCell],
        wide_rows: &[StatusGridWideRow],
        metrics: &[StatusGridMetric],
    ) {
        Self::paint_snapshot_with_selection(
            cx,
            rect,
            columns,
            cells,
            wide_rows,
            metrics,
            StatusGridTonePresentation::default(),
            ReadoutTextOverflow::default(),
            None,
            None,
        );
    }

    fn paint_snapshot_with_selection(
        cx: &mut PaintContext,
        rect: RectF,
        columns: usize,
        cells: &[StatusGridCell],
        wide_rows: &[StatusGridWideRow],
        metrics: &[StatusGridMetric],
        tone_presentation: StatusGridTonePresentation,
        overflow: ReadoutTextOverflow,
        active_key: Option<u64>,
        selection: Option<&ReadOnlyTextSelection>,
    ) {
        let columns = columns.max(1);
        let typography =
            ReadoutTypography::from_text_engine(cx.text_engine, cx.theme.typography.font_size);
        let colors = &cx.theme.colors;
        cx.scene.push_rect(rect, colors.surfaces.base0, 1, None);
        readout_border(cx, rect, colors.borders.subtle, 3);

        let pad = 8.0;
        let gap = 6.0;
        let cell_h = 34.0;
        let cell_w = ((rect.width - pad * 2.0 - gap * (columns.saturating_sub(1) as f32))
            / columns as f32)
            .max(80.0);

        for (index, cell_data) in cells.iter().enumerate() {
            let col = index % columns;
            let row = index / columns;
            let x = rect.x + pad + col as f32 * (cell_w + gap);
            let y = rect.y + pad + row as f32 * (cell_h + gap);
            let cell = RectF::new(x, y, cell_w, cell_h);
            cx.scene.push_rect(cell, colors.surfaces.level1, 2, None);
            readout_border(cx, cell, colors.borders.default, 4);
            register_status_grid_tooltip(cx, cell, cell_data.tooltip.as_ref());
            let label_key = status_grid_key(0, index);
            let label_selection = if active_key == Some(label_key) {
                selection
            } else {
                None
            };
            readout_text_region(
                cx,
                status_grid_cell_label_rect(cell, typography),
                &cell_data.label,
                typography.label_size,
                colors.typography.muted,
                overflow,
                label_selection,
            );
            let value_key = status_grid_key(1, index);
            let value_selection = if active_key == Some(value_key) {
                selection
            } else {
                None
            };
            paint_status_grid_value(
                cx,
                cell,
                RectF::new(cell.x + 7.0, cell.y + 15.0, cell.width - 14.0, 17.0),
                &cell_data.value,
                typography.value_size,
                overflow,
                cell_data.tone,
                tone_presentation,
                value_selection,
            );
        }

        let cell_rows = cells.len().div_ceil(columns);
        let mut y =
            rect.y + pad + cell_rows as f32 * cell_h + cell_rows.saturating_sub(1) as f32 * gap;
        for (row_index, row) in wide_rows.iter().enumerate() {
            y += gap;
            let row_rect = RectF::new(rect.x + pad, y, rect.width - pad * 2.0, 26.0);
            cx.scene
                .push_rect(row_rect, colors.surfaces.level1, 2, None);
            readout_border(cx, row_rect, colors.borders.default, 4);
            register_status_grid_tooltip(cx, row_rect, row.tooltip.as_ref());
            let label_key = status_grid_key(2, row_index);
            let label_selection = if active_key == Some(label_key) {
                selection
            } else {
                None
            };
            readout_text_region(
                cx,
                status_grid_wide_label_rect(row_rect, typography),
                &row.label,
                typography.label_size,
                colors.typography.muted,
                overflow,
                label_selection,
            );
            let value_key = status_grid_key(3, row_index);
            let value_selection = if active_key == Some(value_key) {
                selection
            } else {
                None
            };
            paint_status_grid_value(
                cx,
                row_rect,
                RectF::new(
                    row_rect.x + 94.0,
                    row_rect.y + 2.0,
                    row_rect.width - 101.0,
                    22.0,
                ),
                &row.value,
                typography.value_size,
                overflow,
                row.tone,
                tone_presentation,
                value_selection,
            );
            y = row_rect.bottom();
        }

        if !metrics.is_empty() {
            y += gap;
            let metric_rect = RectF::new(rect.x + pad, y, rect.width - pad * 2.0, 24.0);
            cx.scene
                .push_rect(metric_rect, colors.surfaces.level1, 2, None);
            readout_border(cx, metric_rect, colors.borders.default, 4);
            let metric_w = (metric_rect.width - 12.0) / metrics.len() as f32;
            for (index, metric) in metrics.iter().enumerate() {
                let x = metric_rect.x + 6.0 + index as f32 * metric_w;
                let item_rect = RectF::new(x, metric_rect.y, metric_w, metric_rect.height);
                register_status_grid_tooltip(cx, item_rect, metric.tooltip.as_ref());
                let label_key = status_grid_key(4, index);
                let label_selection = if active_key == Some(label_key) {
                    selection
                } else {
                    None
                };
                readout_text_region(
                    cx,
                    RectF::new(x, metric_rect.y + 3.0, metric_w * 0.58, 18.0),
                    &metric.label,
                    typography.label_size,
                    colors.typography.muted,
                    overflow,
                    label_selection,
                );
                let value_key = status_grid_key(5, index);
                let value_selection = if active_key == Some(value_key) {
                    selection
                } else {
                    None
                };
                paint_status_grid_value(
                    cx,
                    item_rect,
                    RectF::new(
                        x + metric_w * 0.58,
                        metric_rect.y + 3.0,
                        metric_w * 0.38,
                        18.0,
                    ),
                    &metric.value,
                    typography.value_size,
                    overflow,
                    metric.tone,
                    tone_presentation,
                    value_selection,
                );
            }
        }
    }

    fn text_region_at(&self, x: f32, y: f32) -> Option<ReadoutTextRegion> {
        self.text_region_matching(|_, rect| rect.contains_point(x, y))
    }

    fn text_region_by_key(&self, key: u64) -> Option<ReadoutTextRegion> {
        self.text_region_matching(|candidate, _| candidate == key)
    }

    fn refresh_active_selection_text(&mut self) {
        let Some(key) = self.active_selection_key else {
            return;
        };
        if let Some(region) = self.text_region_by_key(key) {
            self.text_selection.set_text(region.text);
        } else {
            self.active_selection_key = None;
            self.text_selection.clear();
        }
    }

    fn clear_active_selection(&mut self) -> bool {
        if self.active_selection_key.is_none()
            && !self.text_selection.has_selection()
            && !self.text_selection.is_dragging()
        {
            return false;
        }
        self.active_selection_key = None;
        self.text_selection.clear();
        true
    }

    fn text_region_matching(
        &self,
        mut matches: impl FnMut(u64, RectF) -> bool,
    ) -> Option<ReadoutTextRegion> {
        status_grid_region_matching(
            self.layout_size,
            self.columns,
            &self.cells,
            &self.wide_rows,
            &self.metrics,
            self.tone_presentation,
            self.overflow,
            self.typography,
            &mut matches,
        )
    }
}

impl WidgetDowncast for StatusGrid {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for StatusGrid {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.typography =
            ReadoutTypography::from_text_engine(cx.text_engine, cx.theme.typography.font_size);
        self.layout_size = Size::new(
            finite_readout_width(constraints, self.natural_width()),
            self.natural_height(),
        );
        LayoutResult::fit(self.layout_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.paint_readout(cx, rect);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let (hit_region, active_region) = match event {
            Event::Pointer(pe) => (
                self.text_region_at(pe.local_pos.0, pe.local_pos.1),
                self.active_selection_key
                    .and_then(|key| self.text_region_by_key(key)),
            ),
            _ => (
                None,
                self.active_selection_key
                    .and_then(|key| self.text_region_by_key(key)),
            ),
        };
        handle_readout_selection(
            self.selection_policy,
            &mut self.active_selection_key,
            &mut self.text_selection,
            hit_region,
            active_region,
            cx,
            event,
            phase,
        )
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        selection_enabled(self.selection_policy)
    }

    fn focusable(&self) -> bool {
        selection_enabled(self.selection_policy)
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.clear_active_selection();
    }

    fn clear_read_only_text_selection(&mut self) -> bool {
        self.clear_active_selection()
    }

    fn debug_type_name(&self) -> &'static str {
        "StatusGrid"
    }
}

fn status_grid_region_matching(
    layout_size: Size,
    columns: usize,
    cells: &[StatusGridCell],
    wide_rows: &[StatusGridWideRow],
    metrics: &[StatusGridMetric],
    tone_presentation: StatusGridTonePresentation,
    overflow: ReadoutTextOverflow,
    typography: ReadoutTypography,
    matches: &mut impl FnMut(u64, RectF) -> bool,
) -> Option<ReadoutTextRegion> {
    let columns = columns.max(1);
    let rect = RectF::new(0.0, 0.0, layout_size.width, layout_size.height);
    let pad = 8.0;
    let gap = 6.0;
    let cell_h = 34.0;
    let cell_w = ((rect.width - pad * 2.0 - gap * (columns.saturating_sub(1) as f32))
        / columns as f32)
        .max(80.0);

    for (index, cell_data) in cells.iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let x = rect.x + pad + col as f32 * (cell_w + gap);
        let y = rect.y + pad + row as f32 * (cell_h + gap);
        let cell = RectF::new(x, y, cell_w, cell_h);

        let label_rect = status_grid_cell_label_rect(cell, typography);
        let label_key = status_grid_key(0, index);
        if matches(label_key, label_rect) {
            return Some(ReadoutTextRegion::new(
                label_key,
                label_rect,
                cell_data.label.clone(),
                typography.label_size,
                overflow,
            ));
        }

        let value_rect = status_grid_value_text_rect(
            RectF::new(cell.x + 7.0, cell.y + 15.0, cell.width - 14.0, 17.0),
            cell_data.tone,
            tone_presentation,
        );
        let value_key = status_grid_key(1, index);
        if matches(value_key, value_rect) {
            return Some(ReadoutTextRegion::new(
                value_key,
                value_rect,
                cell_data.value.clone(),
                typography.value_size,
                overflow,
            ));
        }
    }

    let cell_rows = cells.len().div_ceil(columns);
    let mut y = rect.y + pad + cell_rows as f32 * cell_h + cell_rows.saturating_sub(1) as f32 * gap;
    for (row_index, row) in wide_rows.iter().enumerate() {
        y += gap;
        let row_rect = RectF::new(rect.x + pad, y, rect.width - pad * 2.0, 26.0);

        let label_rect = status_grid_wide_label_rect(row_rect, typography);
        let label_key = status_grid_key(2, row_index);
        if matches(label_key, label_rect) {
            return Some(ReadoutTextRegion::new(
                label_key,
                label_rect,
                row.label.clone(),
                typography.label_size,
                overflow,
            ));
        }

        let value_rect = status_grid_value_text_rect(
            RectF::new(
                row_rect.x + 94.0,
                row_rect.y + 2.0,
                row_rect.width - 101.0,
                22.0,
            ),
            row.tone,
            tone_presentation,
        );
        let value_key = status_grid_key(3, row_index);
        if matches(value_key, value_rect) {
            return Some(ReadoutTextRegion::new(
                value_key,
                value_rect,
                row.value.clone(),
                typography.value_size,
                overflow,
            ));
        }

        y = row_rect.bottom();
    }

    if !metrics.is_empty() {
        y += gap;
        let metric_rect = RectF::new(rect.x + pad, y, rect.width - pad * 2.0, 24.0);
        let metric_w = (metric_rect.width - 12.0) / metrics.len() as f32;
        for (index, metric) in metrics.iter().enumerate() {
            let x = metric_rect.x + 6.0 + index as f32 * metric_w;

            let label_rect = RectF::new(x, metric_rect.y + 3.0, metric_w * 0.58, 18.0);
            let label_key = status_grid_key(4, index);
            if matches(label_key, label_rect) {
                return Some(ReadoutTextRegion::new(
                    label_key,
                    label_rect,
                    metric.label.clone(),
                    typography.label_size,
                    overflow,
                ));
            }

            let value_rect = status_grid_value_text_rect(
                RectF::new(
                    x + metric_w * 0.58,
                    metric_rect.y + 3.0,
                    metric_w * 0.38,
                    18.0,
                ),
                metric.tone,
                tone_presentation,
            );
            let value_key = status_grid_key(5, index);
            if matches(value_key, value_rect) {
                return Some(ReadoutTextRegion::new(
                    value_key,
                    value_rect,
                    metric.value.clone(),
                    typography.value_size,
                    overflow,
                ));
            }
        }
    }

    None
}

fn status_grid_height(
    columns: usize,
    cell_count: usize,
    wide_row_count: usize,
    has_metrics: bool,
) -> f32 {
    let columns = columns.max(1);
    let cell_rows = cell_count.div_ceil(columns);
    let pad_top = 8.0;
    let pad_bottom = 8.0;
    let gap = 6.0;
    let cell_h = 34.0;
    let mut height = pad_top + cell_rows as f32 * cell_h;
    if cell_rows > 1 {
        height += (cell_rows - 1) as f32 * gap;
    }
    height += wide_row_count as f32 * (gap + 26.0);
    if has_metrics {
        height += gap + 24.0;
    }
    height + pad_bottom
}

/// A compact label/value pair for [`KeyValueStrip`].
#[derive(Clone, Debug)]
pub struct KeyValueItem {
    label: String,
    value: String,
}

impl KeyValueItem {
    /// Creates a label/value item.
    pub fn new(label: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            label: label.into(),
            value: value.into(),
        }
    }
}

/// Paint-only label/value strip for footer policy and compact readouts.
pub struct KeyValueStrip {
    id: WidgetInternalId,
    columns: usize,
    items: Vec<KeyValueItem>,
    overflow: ReadoutTextOverflow,
    selection_policy: TextSelectionPolicy,
    active_selection_key: Option<u64>,
    text_selection: ReadOnlyTextSelection,
    layout_size: Size,
    typography: ReadoutTypography,
}

impl KeyValueStrip {
    /// Creates a two-column key/value strip.
    pub fn new(items: Vec<KeyValueItem>) -> Self {
        Self {
            id: WidgetInternalId::new(),
            columns: 2,
            items,
            overflow: ReadoutTextOverflow::default(),
            selection_policy: TextSelectionPolicy::Disabled,
            active_selection_key: None,
            text_selection: ReadOnlyTextSelection::disabled(),
            layout_size: Size::zero(),
            typography: ReadoutTypography::default(),
        }
    }

    /// Sets the number of columns.
    pub fn with_columns(mut self, columns: usize) -> Self {
        self.columns = columns.max(1);
        self
    }

    /// Sets how text that exceeds its readout rect should be displayed.
    pub fn with_overflow(mut self, overflow: ReadoutTextOverflow) -> Self {
        self.overflow = overflow;
        self
    }

    /// Enables or disables read-only text selection.
    pub fn with_selection_policy(mut self, policy: TextSelectionPolicy) -> Self {
        self.selection_policy = policy;
        self.text_selection.set_policy(policy);
        self
    }

    /// Computes the natural height for the configured strip.
    pub fn natural_height(&self) -> f32 {
        let rows = self.items.len().div_ceil(self.columns.max(1));
        10.0 + rows as f32 * 24.0
    }

    fn natural_width(&self) -> f32 {
        let columns = self.columns.max(1);
        16.0 + columns as f32 * 160.0 + 6.0 * columns.saturating_sub(1) as f32
    }

    /// Paints the strip into an existing readout surface.
    pub fn paint_strip(&self, cx: &mut PaintContext, rect: RectF) {
        let colors = &cx.theme.colors;
        let typography =
            ReadoutTypography::from_text_engine(cx.text_engine, cx.theme.typography.font_size);
        cx.scene.push_rect(rect, colors.surfaces.base0, 1, None);
        readout_border(cx, rect, colors.borders.subtle, 3);
        let columns = self.columns.max(1);
        let item_w = (rect.width - 16.0 - 6.0 * columns.saturating_sub(1) as f32) / columns as f32;
        let item_h = 24.0;
        for (index, item) in self.items.iter().enumerate() {
            let col = index % columns;
            let row = index / columns;
            let x = rect.x + 8.0 + col as f32 * (item_w + 6.0);
            let y = rect.y + 6.0 + row as f32 * item_h;
            let label_key = key_value_strip_key(0, index);
            let label_selection = if self.active_selection_key == Some(label_key) {
                Some(&self.text_selection)
            } else {
                None
            };
            readout_text_region(
                cx,
                RectF::new(x, y, item_w * 0.34, 18.0),
                &item.label,
                typography.label_size,
                colors.typography.muted,
                self.overflow,
                label_selection,
            );
            let value_key = key_value_strip_key(1, index);
            let value_selection = if self.active_selection_key == Some(value_key) {
                Some(&self.text_selection)
            } else {
                None
            };
            readout_text_region(
                cx,
                RectF::new(x + item_w * 0.34, y, item_w * 0.62, 18.0),
                &item.value,
                typography.value_size,
                colors.typography.primary,
                self.overflow,
                value_selection,
            );
        }
    }

    fn text_region_at(&self, x: f32, y: f32) -> Option<ReadoutTextRegion> {
        self.text_region_matching(|_, rect| rect.contains_point(x, y))
    }

    fn text_region_by_key(&self, key: u64) -> Option<ReadoutTextRegion> {
        self.text_region_matching(|candidate, _| candidate == key)
    }

    fn text_region_matching(
        &self,
        mut matches: impl FnMut(u64, RectF) -> bool,
    ) -> Option<ReadoutTextRegion> {
        key_value_strip_region_matching(
            self.layout_size,
            self.columns,
            &self.items,
            self.overflow,
            self.typography,
            &mut matches,
        )
    }

    fn clear_active_selection(&mut self) -> bool {
        if self.active_selection_key.is_none()
            && !self.text_selection.has_selection()
            && !self.text_selection.is_dragging()
        {
            return false;
        }
        self.active_selection_key = None;
        self.text_selection.clear();
        true
    }
}

fn key_value_strip_region_matching(
    layout_size: Size,
    columns: usize,
    items: &[KeyValueItem],
    overflow: ReadoutTextOverflow,
    typography: ReadoutTypography,
    matches: &mut impl FnMut(u64, RectF) -> bool,
) -> Option<ReadoutTextRegion> {
    let rect = RectF::new(0.0, 0.0, layout_size.width, layout_size.height);
    let columns = columns.max(1);
    let item_w = (rect.width - 16.0 - 6.0 * columns.saturating_sub(1) as f32) / columns as f32;
    let item_h = 24.0;

    for (index, item) in items.iter().enumerate() {
        let col = index % columns;
        let row = index / columns;
        let x = rect.x + 8.0 + col as f32 * (item_w + 6.0);
        let y = rect.y + 6.0 + row as f32 * item_h;

        let label_rect = RectF::new(x, y, item_w * 0.34, 18.0);
        let label_key = key_value_strip_key(0, index);
        if matches(label_key, label_rect) {
            return Some(ReadoutTextRegion::new(
                label_key,
                label_rect,
                item.label.clone(),
                typography.label_size,
                overflow,
            ));
        }

        let value_rect = RectF::new(x + item_w * 0.34, y, item_w * 0.62, 18.0);
        let value_key = key_value_strip_key(1, index);
        if matches(value_key, value_rect) {
            return Some(ReadoutTextRegion::new(
                value_key,
                value_rect,
                item.value.clone(),
                typography.value_size,
                overflow,
            ));
        }
    }

    None
}

impl WidgetDowncast for KeyValueStrip {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}

impl Widget for KeyValueStrip {
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        constraints: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.typography =
            ReadoutTypography::from_text_engine(cx.text_engine, cx.theme.typography.font_size);
        self.layout_size = Size::new(
            finite_readout_width(constraints, self.natural_width()),
            self.natural_height(),
        );
        LayoutResult::fit(self.layout_size)
    }

    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        self.paint_strip(cx, rect);
    }

    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        let (hit_region, active_region) = match event {
            Event::Pointer(pe) => (
                self.text_region_at(pe.local_pos.0, pe.local_pos.1),
                self.active_selection_key
                    .and_then(|key| self.text_region_by_key(key)),
            ),
            _ => (
                None,
                self.active_selection_key
                    .and_then(|key| self.text_region_by_key(key)),
            ),
        };
        handle_readout_selection(
            self.selection_policy,
            &mut self.active_selection_key,
            &mut self.text_selection,
            hit_region,
            active_region,
            cx,
            event,
            phase,
        )
    }

    fn id(&self) -> WidgetInternalId {
        self.id
    }

    fn hit_testable(&self) -> bool {
        selection_enabled(self.selection_policy)
    }

    fn focusable(&self) -> bool {
        selection_enabled(self.selection_policy)
    }

    fn on_blur(&mut self, _cx: &mut EventContext) {
        self.clear_active_selection();
    }

    fn clear_read_only_text_selection(&mut self) -> bool {
        self.clear_active_selection()
    }

    fn debug_type_name(&self) -> &'static str {
        "KeyValueStrip"
    }
}
