use super::document::{BlockKind, CellAlign, InlineSpan, InlineStyle, MarkdownDocument};
use crate::core::{
    event::{Event, EventPhase, PointerButton, PointerEventKind},
    layout::{Constraints, LayoutResult},
    math::{RectF, Size},
    paint::{Color, LAYER_CONTENT},
    paragraph::{
        LayoutParams, Line, Paragraph, ParagraphLayoutEngine, Run, Span, TextAlign, TextStyle,
    },
    text_engine::{GlyphRun, TextBinding, TextEngine, TextInputError},
    text_selection::{ReadOnlyTextSelection, TextSelectionPolicy, hit_test_run, selection_spans},
    text_shaper::TextDirection,
    widget::{EventContext, LayoutContext, PaintContext, Widget, WidgetDowncast},
    widget_tree::NodeId,
};
use crate::{
    platform::font_manager::{FontFaceCatalog, FontWeight},
    utils::id::WidgetInternalId,
};
use std::{fmt, ops::Range, sync::Arc};

#[derive(Clone, Debug, PartialEq)]
pub struct MarkdownStyle {
    prose: String,
    code: String,
    size: Option<f32>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownStyleError {
    InvalidFamily,
    InvalidFontSize,
}
impl fmt::Display for MarkdownStyleError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Markdown style: {self:?}")
    }
}
impl std::error::Error for MarkdownStyleError {}
impl MarkdownStyle {
    pub fn new(
        prose_family: impl Into<String>,
        code_family: impl Into<String>,
    ) -> Result<Self, MarkdownStyleError> {
        let prose = prose_family.into();
        let code = code_family.into();
        if [&prose, &code]
            .iter()
            .any(|s| s.trim().is_empty() || s.chars().any(char::is_control))
        {
            return Err(MarkdownStyleError::InvalidFamily);
        }
        Ok(Self {
            prose,
            code,
            size: None,
        })
    }
    pub fn with_size(mut self, size: f32) -> Result<Self, MarkdownStyleError> {
        if !size.is_finite() || size <= 0.0 {
            return Err(MarkdownStyleError::InvalidFontSize);
        }
        self.size = Some(size);
        Ok(self)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownLayoutError {
    UnsupportedTypography,
    RequiredFaceUnavailable,
    Text(TextInputError),
    InvalidGeometry,
}
impl fmt::Display for MarkdownLayoutError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Markdown layout: {self:?}")
    }
}
impl std::error::Error for MarkdownLayoutError {}
impl From<TextInputError> for MarkdownLayoutError {
    fn from(e: TextInputError) -> Self {
        Self::Text(e)
    }
}

/// HAL390 §16: blur retention is logical selection only, never native capture.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkdownSelectionRetention {
    #[default]
    ClearOnBlur,
    RetainOnBlur,
}

/// An exact selection in the identical immutable document (HAL390 §16).
/// No geometry, widget, mount, focus or capture state is transferred.
#[derive(Clone)]
pub struct MarkdownReadHandoff {
    document: Arc<MarkdownDocument>,
    anchor: usize,
    head: usize,
}
impl fmt::Debug for MarkdownReadHandoff {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("MarkdownReadHandoff { .. }")
    }
}
impl MarkdownReadHandoff {
    pub fn document(&self) -> &Arc<MarkdownDocument> {
        &self.document
    }
    pub fn selection_anchor_byte(&self) -> usize {
        self.anchor
    }
    pub fn selection_head_byte(&self) -> usize {
        self.head
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownReadHandoffError {
    SelectionDisabled,
    SelectionInProgress,
    LayoutUnavailable,
    DocumentMismatch,
    InvalidBoundary,
    UnsupportedTarget,
}
impl fmt::Display for MarkdownReadHandoffError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Markdown read handoff: {self:?}")
    }
}
impl std::error::Error for MarkdownReadHandoffError {}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MarkdownReadHandoffStatus {
    #[default]
    NotRequested,
    Pending,
    Applied,
    Rejected(MarkdownReadHandoffError),
}

struct VisualMap {
    visual: Range<usize>,
    reading: Range<usize>,
    exact: bool,
}
struct LaidText {
    paragraph: Paragraph,
    map: Vec<VisualMap>,
    lines: Vec<Line>,
    x: f32,
    y: f32,
    width: f32,
    height: f32,
    align: TextAlign,
    strike: Vec<Range<usize>>,
    inline_code: Vec<Range<usize>>,
}
impl LaidText {
    fn shape(
        &self,
        run: &Run,
        catalog: &FontFaceCatalog,
        engine: &mut dyn TextEngine,
    ) -> Result<GlyphRun, TextInputError> {
        let face = catalog
            .get(run.font_id)
            .ok_or(TextInputError::FontUnavailable)?;
        // Paragraph detects direction over each original merged style span, not
        // over a visual substring or a later default-font reconstruction.
        let origin = self
            .paragraph
            .spans
            .iter()
            .find(|s| s.range.start <= run.range.start && s.range.end >= run.range.end)
            .ok_or(TextInputError::PositionOverflow)?;
        let direction = TextDirection::detect(&self.paragraph.text[origin.range.clone()]);
        let binding =
            TextBinding::new(face.binding.clone(), run.style.cluster_gap).with_direction(direction);
        engine.shape_bound_text(
            &self.paragraph.text[run.range.clone()],
            run.style.font_size,
            &binding,
        )
    }
    fn reading_offset(&self, offset: usize) -> usize {
        let Some(m) = self
            .map
            .iter()
            .find(|m| m.visual.start <= offset && offset < m.visual.end)
            .or_else(|| self.map.last())
        else {
            return 0;
        };
        if m.exact {
            m.reading.start + (offset.clamp(m.visual.start, m.visual.end) - m.visual.start)
        } else if offset.saturating_sub(m.visual.start) * 2 < m.visual.len() {
            m.reading.start
        } else {
            m.reading.end
        }
    }
    fn visual_selection(&self, range: (usize, usize)) -> Option<Range<usize>> {
        let mut result: Option<Range<usize>> = None;
        for m in &self.map {
            if range.0 >= m.reading.end || range.1 <= m.reading.start {
                continue;
            }
            let r = if m.exact {
                (m.visual.start + range.0.max(m.reading.start) - m.reading.start)
                    ..(m.visual.start + range.1.min(m.reading.end) - m.reading.start)
            } else {
                m.visual.clone()
            };
            if let Some(total) = &mut result {
                total.start = total.start.min(r.start);
                total.end = total.end.max(r.end);
            } else {
                result = Some(r);
            }
        }
        result
    }
}
#[derive(Clone, Copy)]
enum Role {
    Surface,
    Rule,
    Task,
}
struct Decoration {
    rect: RectF,
    role: Role,
    above: bool,
}
struct Cache {
    catalog: FontFaceCatalog,
    texts: Vec<LaidText>,
    decorations: Vec<Decoration>,
    content: Size,
    width: f32,
    key: Vec<u32>,
    boundaries: Vec<usize>,
}

/// One native read-only document leaf. It owns no scroll state or parser/runtime.
pub struct MarkdownView {
    id: WidgetInternalId,
    document: Arc<MarkdownDocument>,
    style: MarkdownStyle,
    selection: ReadOnlyTextSelection,
    cache: Option<Cache>,
    failure: Option<MarkdownLayoutError>,
    size: Size,
    word_drag: Option<(usize, usize)>,
    selection_retention: MarkdownSelectionRetention,
    pending_read_handoff: Option<MarkdownReadHandoff>,
    read_handoff_status: MarkdownReadHandoffStatus,
    handoff_mounted: bool,
    handoff_used: bool,
}
impl MarkdownView {
    pub fn new(document: Arc<MarkdownDocument>, style: MarkdownStyle) -> Self {
        let selection = ReadOnlyTextSelection::new(
            TextSelectionPolicy::Disabled,
            document.reading_text().to_owned(),
        );
        Self {
            id: WidgetInternalId::new(),
            document,
            style,
            selection,
            cache: None,
            failure: None,
            size: Size::zero(),
            word_drag: None,
            selection_retention: MarkdownSelectionRetention::default(),
            pending_read_handoff: None,
            read_handoff_status: MarkdownReadHandoffStatus::NotRequested,
            handoff_mounted: false,
            handoff_used: false,
        }
    }
    pub fn with_selection_policy(mut self, policy: TextSelectionPolicy) -> Self {
        self.selection.set_policy(policy);
        self
    }
    pub fn with_selection_retention(mut self, retention: MarkdownSelectionRetention) -> Self {
        self.selection_retention = retention;
        self
    }

    /// Capture settled source selection using its current successful real layout.
    pub fn capture_read_handoff(&self) -> Result<MarkdownReadHandoff, MarkdownReadHandoffError> {
        use MarkdownReadHandoffError as Error;
        if self.selection.policy() != TextSelectionPolicy::Enabled {
            return Err(Error::SelectionDisabled);
        }
        if self.selection.is_dragging() || self.word_drag.is_some() {
            return Err(Error::SelectionInProgress);
        }
        if !matches!(
            self.read_handoff_status,
            MarkdownReadHandoffStatus::NotRequested | MarkdownReadHandoffStatus::Applied
        ) {
            return Err(Error::UnsupportedTarget);
        }
        let cache = self
            .cache
            .as_ref()
            .filter(|_| self.failure.is_none())
            .ok_or(Error::LayoutUnavailable)?;
        let selection = self.selection.selection();
        let handoff = MarkdownReadHandoff {
            document: self.document.clone(),
            anchor: selection.anchor,
            head: selection.head,
        };
        self.validate_read_handoff(&handoff)?;
        if [handoff.anchor, handoff.head]
            .iter()
            .any(|offset| cache.boundaries.binary_search(offset).is_err())
        {
            return Err(Error::InvalidBoundary);
        }
        Ok(handoff)
    }

    /// Success is inert Pending; only the tree's existing deferred cut applies it.
    pub fn try_stage_read_handoff(
        &mut self,
        handoff: &MarkdownReadHandoff,
    ) -> Result<(), MarkdownReadHandoffError> {
        use MarkdownReadHandoffError as Error;
        if self.selection.policy() != TextSelectionPolicy::Enabled {
            return Err(Error::SelectionDisabled);
        }
        if self.handoff_mounted
            || self.handoff_used
            || self.selection_retention != MarkdownSelectionRetention::RetainOnBlur
            || self.read_handoff_status != MarkdownReadHandoffStatus::NotRequested
        {
            return Err(Error::UnsupportedTarget);
        }
        self.validate_read_handoff(handoff)?;
        self.pending_read_handoff = Some(handoff.clone());
        self.read_handoff_status = MarkdownReadHandoffStatus::Pending;
        Ok(())
    }

    pub fn read_handoff_status(&self) -> MarkdownReadHandoffStatus {
        self.read_handoff_status
    }

    fn validate_read_handoff(
        &self,
        handoff: &MarkdownReadHandoff,
    ) -> Result<(), MarkdownReadHandoffError> {
        use MarkdownReadHandoffError as Error;
        if !Arc::ptr_eq(&self.document, &handoff.document)
            || self.selection.text() != handoff.document.reading_text()
        {
            return Err(Error::DocumentMismatch);
        }
        if [handoff.anchor, handoff.head]
            .iter()
            .any(|offset| !self.document.reading_text().is_char_boundary(*offset))
        {
            return Err(Error::InvalidBoundary);
        }
        Ok(())
    }

    pub(crate) fn has_pending_read_handoff(&self) -> bool {
        self.pending_read_handoff.is_some()
    }

    /// HAL383 §11/HAL390 §16: called once at the original pre-Layout owner cut.
    pub(crate) fn apply_pending_read_handoff(&mut self) -> bool {
        let Some(handoff) = self.pending_read_handoff.take() else {
            return false;
        };
        let result = if self.selection.policy() != TextSelectionPolicy::Enabled {
            Err(MarkdownReadHandoffError::SelectionDisabled)
        } else if !self.handoff_mounted
            || self.handoff_used
            || self.selection_retention != MarkdownSelectionRetention::RetainOnBlur
        {
            Err(MarkdownReadHandoffError::UnsupportedTarget)
        } else {
            self.validate_read_handoff(&handoff)
        };
        if let Err(error) = result {
            self.read_handoff_status = MarkdownReadHandoffStatus::Rejected(error);
            return false;
        }
        // Validation and publication are atomic; no partial selection is visible.
        if !self.selection.restore_markdown_read_handoff(
            self.document.reading_text(),
            handoff.anchor,
            handoff.head,
        ) {
            self.read_handoff_status =
                MarkdownReadHandoffStatus::Rejected(MarkdownReadHandoffError::InvalidBoundary);
            return false;
        }
        self.word_drag = None;
        self.read_handoff_status = MarkdownReadHandoffStatus::Applied;
        true
    }
    pub fn document(&self) -> &Arc<MarkdownDocument> {
        &self.document
    }
    pub fn layout_failure(&self) -> Option<&MarkdownLayoutError> {
        self.failure.as_ref()
    }
    fn eligible(&self) -> bool {
        self.selection.policy() != TextSelectionPolicy::Disabled
    }

    fn text_style(
        &self,
        format: InlineStyle,
        heading: u8,
        header: bool,
        em: f32,
        color: Color,
    ) -> TextStyle {
        // INV-MD-GEOMETRY: all multipliers are adopted HAL390 §14.4.
        let multiple = if heading == 0 {
            1.0
        } else {
            [1.75, 1.5, 1.25, 1.125, 1.0, 1.0][usize::from(heading - 1)]
        };
        TextStyle::default()
            .with_family(if format.code {
                self.style.code.as_str()
            } else {
                self.style.prose.as_str()
            })
            .with_size(em * multiple)
            .with_color(color)
            .with_weight(if format.bold || heading != 0 || header {
                FontWeight::Bold
            } else {
                FontWeight::Regular
            })
            .with_italic(format.italic)
    }
    fn qualify(
        style: &TextStyle,
        catalog: &FontFaceCatalog,
    ) -> Result<TextBinding, MarkdownLayoutError> {
        let face = catalog
            .find(style.font_family.name(), style.weight, style.italic)
            .filter(|f| {
                f.family_name == style.font_family.name()
                    && f.weight == style.weight
                    && f.italic == style.italic
            })
            .and_then(|f| catalog.get(f.font_id))
            .ok_or(MarkdownLayoutError::RequiredFaceUnavailable)?;
        Ok(TextBinding::new(face.binding.clone(), style.cluster_gap))
    }
    fn layout_text(
        &self,
        range: Range<usize>,
        spans: &[InlineSpan],
        task: Option<&(Range<usize>, bool)>,
        heading: u8,
        header: bool,
        width: f32,
        em: f32,
        color: Color,
        catalog: &FontFaceCatalog,
        engine: &mut dyn TextEngine,
    ) -> Result<LaidText, MarkdownLayoutError> {
        let mut text = String::new();
        let mut map = Vec::new();
        let mut styles: Vec<Span> = Vec::new();
        let mut strike = Vec::new();
        let mut inline_code = Vec::new();
        let mut column = 0usize;
        for span in spans {
            let a = span.range.start.max(range.start);
            let b = span.range.end.min(range.end);
            if a >= b {
                continue;
            }
            let style = self.text_style(span.style, heading, header, em, color);
            let binding = Self::qualify(&style, catalog)?;
            if span.style.code {
                let parsed = ttf_parser::Face::parse(binding.face().data(), 0)
                    .map_err(|_| MarkdownLayoutError::RequiredFaceUnavailable)?;
                if !parsed.is_monospaced() {
                    return Err(MarkdownLayoutError::RequiredFaceUnavailable);
                }
                // The actual admitted space, rather than a family-name heuristic,
                // supplies tab-cell advance. Expanded spaces keep that binding.
                let space = engine.shape_bound_text(" ", style.font_size, &binding)?;
                if !space.width.is_finite() || space.width <= 0.0 {
                    return Err(MarkdownLayoutError::InvalidGeometry);
                }
            }
            let visual_start = text.len();
            let mut current = a;
            while current < b {
                let ch = self.document.reading_text()[current..b]
                    .chars()
                    .next()
                    .unwrap();
                if let Some((r, _)) = task.filter(|(r, _)| r.start == current) {
                    let start = text.len();
                    text.push_str("    ");
                    map.push(VisualMap {
                        visual: start..text.len(),
                        reading: r.clone(),
                        exact: false,
                    });
                    current = r.end;
                    column += 4;
                    continue;
                }
                if ch == '\t' && span.style.code {
                    let count = 4 - column % 4;
                    let start = text.len();
                    text.extend(std::iter::repeat_n(' ', count));
                    map.push(VisualMap {
                        visual: start..text.len(),
                        reading: current..current + 1,
                        exact: false,
                    });
                    column += count;
                    current += 1;
                } else {
                    let begin = current;
                    while current < b {
                        let c = self.document.reading_text()[current..b]
                            .chars()
                            .next()
                            .unwrap();
                        if (c == '\t' && span.style.code)
                            || task.is_some_and(|(r, _)| r.start == current)
                        {
                            break;
                        }
                        current += c.len_utf8();
                        if c == '\n' {
                            column = 0;
                        } else {
                            column = column.saturating_add(1);
                        }
                    }
                    let start = text.len();
                    text.push_str(&self.document.reading_text()[begin..current]);
                    map.push(VisualMap {
                        visual: start..text.len(),
                        reading: begin..current,
                        exact: true,
                    });
                }
            }
            let r = visual_start..text.len();
            if span.style.strike {
                strike.push(r.clone());
            }
            if span.style.code {
                inline_code.push(r.clone());
            }
            if let Some(previous) = styles
                .last_mut()
                .filter(|s| s.range.end == r.start && s.style.same_as(&style))
            {
                previous.range.end = r.end;
            } else {
                styles.push(Span::new(r, style));
            }
        }
        if styles.is_empty() {
            styles.push(Span::new(
                0..0,
                self.text_style(InlineStyle::default(), heading, header, em, color),
            ));
        }
        let primary = Self::qualify(&styles[0].style, catalog)?;
        let mut canonical = engine.metrics_bound_text(styles[0].style.font_size, &primary)?;
        for span in &styles {
            let binding = Self::qualify(&span.style, catalog)?;
            let metrics = engine.metrics_bound_text(span.style.font_size, &binding)?;
            canonical.ascent = canonical.ascent.max(metrics.ascent);
            canonical.descent = canonical.descent.max(metrics.descent);
            canonical.line_gap = canonical.line_gap.max(metrics.line_gap);
        }
        canonical.line_height = canonical.ascent + canonical.descent + canonical.line_gap;
        let paragraph = Paragraph::with_spans(text, styles);
        let params = LayoutParams {
            max_width: width,
            scale: 1.0,
            canonical_metrics: canonical,
        };
        let mut lines = ParagraphLayoutEngine::try_layout_with_catalog(
            &paragraph,
            catalog,
            primary.face().font_id(),
            &params,
        )?;
        for line in &mut lines {
            // HAL390 line boxes include every actual fallback/styled run while
            // preserving the canonical ascent as their minimum baseline.
            for run in &line.runs {
                line.ascent = line.ascent.max(run.metrics.ascent);
                line.descent = line.descent.max(run.metrics.descent);
            }
            line.baseline = line.ascent;
            line.height = line.ascent + line.descent;
        }
        let natural = lines.iter().map(|l| l.width).fold(0.0, f32::max);
        let height = lines.iter().map(|l| l.height).sum::<f32>();
        if !natural.is_finite() || !height.is_finite() {
            return Err(MarkdownLayoutError::InvalidGeometry);
        }
        let laid = LaidText {
            paragraph,
            map,
            lines,
            x: 0.0,
            y: 0.0,
            width: if width.is_finite() { width } else { natural },
            height,
            align: TextAlign::Left,
            strike,
            inline_code,
        };
        for line in &laid.lines {
            for run in &line.runs {
                laid.shape(run, catalog, engine)?;
            }
        }
        Ok(laid)
    }
    fn build_cache(
        &self,
        width: f32,
        em: f32,
        key: Vec<u32>,
        catalog: FontFaceCatalog,
        cx: &mut LayoutContext,
    ) -> Result<Cache, MarkdownLayoutError> {
        let color = cx.theme.colors.typography.primary;
        let border = cx.theme.borders.b1;
        let mut texts = Vec::new();
        let mut decorations = Vec::new();
        let mut y = 0.0_f32;
        let mut extent = width;
        for (index, block) in self.document.blocks.iter().enumerate() {
            let heading = if let BlockKind::Text { heading } = block.kind {
                heading
            } else {
                0
            };
            if index != 0 {
                y += if heading != 0 { em } else { 0.75 * em };
            }
            let x = block.list_depth as f32 * 1.5 * em + block.quote_depth as f32 * em;
            let available = (width - x).max(0.0);
            let top = y;
            if let Some(prefix) = &block.prefix {
                let text = &self.document.reading_text()[prefix.clone()];
                let skip = text.len() - text.trim_start_matches(' ').len();
                let r = prefix.start + skip..prefix.end;
                let spans = [InlineSpan {
                    range: r.clone(),
                    style: InlineStyle::default(),
                }];
                let mut laid = self.layout_text(
                    r,
                    &spans,
                    None,
                    0,
                    false,
                    f32::INFINITY,
                    em,
                    color,
                    &catalog,
                    cx.text_engine,
                )?;
                laid.x = (x - laid.width).max(0.0);
                laid.y = y;
                texts.push(laid);
            }
            match &block.kind {
                BlockKind::Rule => {
                    decorations.push(Decoration {
                        rect: RectF::new(x, y + em * 0.5, available, border),
                        role: Role::Rule,
                        above: false,
                    });
                    y += em;
                }
                BlockKind::Table { rows, align } => {
                    let n = rows.iter().map(Vec::len).max().unwrap_or(0);
                    if n == 0 {
                        continue;
                    }
                    let mut preferred = vec![0.0_f32; n];
                    let mut cluster = vec![0.0_f32; n];
                    for row in rows {
                        for (i, cell) in row.iter().enumerate() {
                            let probe = self.layout_text(
                                cell.range.clone(),
                                &cell.spans,
                                None,
                                0,
                                cell.header,
                                f32::INFINITY,
                                em,
                                color,
                                &catalog,
                                cx.text_engine,
                            )?;
                            preferred[i] = preferred[i].max(probe.width + em);
                            for line in &probe.lines {
                                for run in &line.runs {
                                    let glyph = probe.shape(run, &catalog, cx.text_engine)?;
                                    for c in &glyph.clusters {
                                        cluster[i] = cluster[i].max(c.x_end - c.x_start);
                                    }
                                }
                            }
                        }
                    }
                    let minimum: Vec<_> = preferred
                        .iter()
                        .zip(&cluster)
                        .map(|(p, c)| p.min((6.0 * em).max(c + em)))
                        .collect();
                    let columns = table_columns(&preferred, &minimum, available)?;
                    let total = columns.iter().sum::<f32>();
                    extent = extent.max(x + total);
                    for row in rows {
                        let mut cells = Vec::new();
                        let mut cell_x = x;
                        let mut row_h = 0.0_f32;
                        for (i, cell) in row.iter().enumerate() {
                            let mut laid = self.layout_text(
                                cell.range.clone(),
                                &cell.spans,
                                None,
                                0,
                                cell.header,
                                (columns[i] - em).max(0.0),
                                em,
                                color,
                                &catalog,
                                cx.text_engine,
                            )?;
                            laid.x = cell_x + em * 0.5;
                            laid.y = y + em * 0.25;
                            laid.align = match align.get(i).copied().unwrap_or(CellAlign::Left) {
                                CellAlign::Left => TextAlign::Left,
                                CellAlign::Center => TextAlign::Center,
                                CellAlign::Right => TextAlign::Right,
                            };
                            row_h = row_h.max(laid.height + em * 0.5);
                            cells.push((laid, cell.header, cell_x, columns[i]));
                            cell_x += columns[i];
                        }
                        for (laid, header, cell_x, cell_w) in cells {
                            if header {
                                decorations.push(Decoration {
                                    rect: RectF::new(cell_x, y, cell_w, row_h),
                                    role: Role::Surface,
                                    above: false,
                                });
                            }
                            for r in [
                                RectF::new(cell_x, y, cell_w, border),
                                RectF::new(cell_x, y, border, row_h),
                                RectF::new(cell_x, y + row_h - border, cell_w, border),
                                RectF::new(cell_x + cell_w - border, y, border, row_h),
                            ] {
                                decorations.push(Decoration {
                                    rect: r,
                                    role: Role::Rule,
                                    above: false,
                                });
                            }
                            texts.push(laid);
                        }
                        y += row_h;
                    }
                }
                BlockKind::Code | BlockKind::Text { .. } => {
                    let code = matches!(block.kind, BlockKind::Code);
                    let padding = if code { em * 0.5 } else { 0.0 };
                    let mut laid = self.layout_text(
                        block.range.clone(),
                        &block.spans,
                        block.task.as_ref(),
                        heading,
                        false,
                        if code {
                            f32::INFINITY
                        } else {
                            (available - 2.0 * padding).max(0.0)
                        },
                        em,
                        color,
                        &catalog,
                        cx.text_engine,
                    )?;
                    laid.x = x + padding;
                    laid.y = y + padding;
                    let block_w = if code {
                        available.max(laid.width + 2.0 * padding)
                    } else {
                        available
                    };
                    let block_h = laid.height + 2.0 * padding;
                    if code {
                        decorations.push(Decoration {
                            rect: RectF::new(x, y, block_w, block_h),
                            role: Role::Surface,
                            above: false,
                        });
                        laid.inline_code.clear();
                    }
                    if let Some((task, checked)) = &block.task {
                        if let Some(m) = laid.map.iter().find(|m| m.reading == *task) {
                            // The task state is geometry, never a fake glyph or
                            // an interactive Checkbox. Text remains selectable.
                            for line in &laid.lines {
                                for run in &line.runs {
                                    if run.range.start <= m.visual.start
                                        && m.visual.start < run.range.end
                                    {
                                        let glyph = laid.shape(run, &catalog, cx.text_engine)?;
                                        let advance = crate::core::text_selection::x_for_offset(
                                            &glyph,
                                            m.visual.start - run.range.start,
                                        );
                                        let tx = laid.x + advance;
                                        let ty = laid.y;
                                        let side = em * 0.75;
                                        for r in [
                                            RectF::new(tx, ty, side, border),
                                            RectF::new(tx, ty, border, side),
                                            RectF::new(tx, ty + side - border, side, border),
                                            RectF::new(tx + side - border, ty, border, side),
                                        ] {
                                            decorations.push(Decoration {
                                                rect: r,
                                                role: Role::Task,
                                                above: false,
                                            });
                                        }
                                        if *checked {
                                            decorations.push(Decoration {
                                                rect: RectF::new(
                                                    tx + side * 0.25,
                                                    ty + side * 0.25,
                                                    side * 0.5,
                                                    side * 0.5,
                                                ),
                                                role: Role::Task,
                                                above: false,
                                            });
                                        }
                                    }
                                }
                            }
                        }
                    }
                    extent = extent.max(
                        laid.x + laid.lines.iter().map(|l| l.width).fold(0.0, f32::max) + padding,
                    );
                    y += block_h;
                    texts.push(laid);
                }
            }
            for depth in 0..block.quote_depth {
                decorations.push(Decoration {
                    rect: RectF::new(
                        block.list_depth as f32 * 1.5 * em + depth as f32 * em,
                        top,
                        border,
                        (y - top).max(0.0),
                    ),
                    role: Role::Rule,
                    above: false,
                });
            }
        }
        if !extent.is_finite() || !y.is_finite() {
            return Err(MarkdownLayoutError::InvalidGeometry);
        }
        let mut boundaries = vec![0, self.document.reading_text().len()];
        for text in &texts {
            for line in &text.lines {
                for run in &line.runs {
                    let glyph = text.shape(run, &catalog, cx.text_engine)?;
                    for cluster in &glyph.clusters {
                        boundaries
                            .push(text.reading_offset(run.range.start + cluster.source.start));
                        boundaries.push(text.reading_offset(run.range.start + cluster.source.end));
                    }
                }
            }
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        if boundaries
            .iter()
            .any(|offset| !self.document.reading_text().is_char_boundary(*offset))
        {
            return Err(MarkdownLayoutError::InvalidGeometry);
        }
        Ok(Cache {
            catalog,
            texts,
            decorations,
            content: Size::new(extent, y),
            width,
            key,
            boundaries,
        })
    }

    fn safe_word(&self, offset: usize) -> Option<(usize, usize)> {
        let cache = self.cache.as_ref()?;
        let (start, end) =
            crate::core::text_selection::find_word_boundaries(self.document.reading_text(), offset);
        let a = cache
            .boundaries
            .partition_point(|v| *v <= start)
            .saturating_sub(1);
        let b = cache
            .boundaries
            .partition_point(|v| *v < end)
            .min(cache.boundaries.len() - 1);
        Some((cache.boundaries[a], cache.boundaries[b]))
    }

    fn hit(&self, x: f32, y: f32, engine: &mut dyn TextEngine) -> Option<usize> {
        let cache = self.cache.as_ref()?;
        let mut nearest = None;
        let mut distance = f32::INFINITY;
        for text in &cache.texts {
            let mut top = text.y;
            for line in &text.lines {
                let dy = if y < top {
                    top - y
                } else if y > top + line.height {
                    y - top - line.height
                } else {
                    0.0
                };
                let left = text.x + text.align.offset(line.width, text.width);
                let dx = if x < left {
                    left - x
                } else if x > left + line.width {
                    x - left - line.width
                } else {
                    0.0
                };
                let d = dy * dy + dx * dx;
                if d <= distance {
                    let mut run_x = left;
                    for (i, run) in line.runs.iter().enumerate() {
                        if x <= run_x + run.width || i + 1 == line.runs.len() {
                            let glyph = text.shape(run, &cache.catalog, engine).ok()?;
                            let visual = run.range.start + hit_test_run(&glyph, x - run_x);
                            let offset = text.reading_offset(visual);
                            if self.document.reading_text().is_char_boundary(offset) {
                                nearest = Some(offset);
                                distance = d;
                            }
                            break;
                        }
                        run_x += run.width;
                    }
                }
                top += line.height;
            }
        }
        nearest
    }
}

fn table_columns(
    preferred: &[f32],
    minimum: &[f32],
    width: f32,
) -> Result<Vec<f32>, MarkdownLayoutError> {
    let p = preferred.iter().sum::<f32>();
    let m = minimum.iter().sum::<f32>();
    if preferred.is_empty()
        || preferred.len() != minimum.len()
        || !p.is_finite()
        || !m.is_finite()
        || !width.is_finite()
    {
        return Err(MarkdownLayoutError::InvalidGeometry);
    }
    Ok(if p <= width {
        preferred
            .iter()
            .map(|v| v + (width - p) / preferred.len() as f32)
            .collect()
    } else if m <= width && p > m {
        minimum
            .iter()
            .zip(preferred)
            .map(|(a, b)| a + (width - m) * (b - a) / (p - m))
            .collect()
    } else {
        minimum.to_vec()
    })
}
impl WidgetDowncast for MarkdownView {
    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
    fn as_any_mut(&mut self) -> &mut dyn std::any::Any {
        self
    }
}
impl Widget for MarkdownView {
    fn id(&self) -> WidgetInternalId {
        self.id
    }
    fn debug_type_name(&self) -> &'static str {
        "MarkdownView"
    }
    fn layout(
        &mut self,
        cx: &mut LayoutContext,
        c: Constraints,
        _children: &[NodeId],
    ) -> LayoutResult {
        self.handoff_used = true;
        let em = self.style.size.unwrap_or(cx.theme.typography.font_size);
        let valid = c.max_width.is_finite()
            && c.max_width >= 0.0
            && c.min_width.is_finite()
            && c.min_width >= 0.0
            && c.min_width <= c.max_width
            && c.min_height.is_finite()
            && c.min_height >= 0.0
            && !c.max_height.is_nan()
            && c.max_height >= c.min_height
            && em.is_finite()
            && em > 0.0;
        let result = (|| {
            if !valid {
                return Err(MarkdownLayoutError::InvalidGeometry);
            }
            let catalog = cx
                .text_engine
                .font_face_catalog()
                .ok_or(MarkdownLayoutError::UnsupportedTypography)?;
            let metrics = cx.text_engine.metrics("", em);
            // Layout is logical scale 1, exactly as Paragraph/TextBlock. Native
            // metric changes, catalog generation and theme inputs invalidate it;
            // Paint/Compile own actual DPR sampling, not the document.
            let mut key = vec![
                em.to_bits(),
                1.0_f32.to_bits(),
                metrics.ascent.to_bits(),
                metrics.descent.to_bits(),
                metrics.line_height.to_bits(),
                cx.theme.borders.b1.to_bits(),
            ];
            for color in [
                cx.theme.colors.typography.primary,
                cx.theme.colors.surfaces.level1,
                cx.theme.colors.borders.subtle,
            ] {
                key.extend([
                    color.r.to_bits(),
                    color.g.to_bits(),
                    color.b.to_bits(),
                    color.a.to_bits(),
                ]);
            }
            if !self.cache.as_ref().is_some_and(|cache| {
                cache.width == c.max_width
                    && cache.key == key
                    && cache.catalog.same_generation(&catalog)
            }) {
                let cache = self.build_cache(c.max_width, em, key, catalog, cx)?;
                self.cache = Some(cache);
            }
            Ok(())
        })();
        self.failure = result.err();
        let content = if self.failure.is_none() {
            self.cache.as_ref().unwrap().content
        } else {
            self.cache = None;
            let run = cx
                .text_engine
                .shape("Markdown layout unavailable", cx.theme.typography.font_size);
            Size::new(run.width, run.height)
        };
        let size = if valid {
            c.constrain(Size::new(c.max_width, content.height))
        } else {
            Size::zero()
        };
        self.size = size;
        LayoutResult::new(size, content)
    }
    fn paint(&self, cx: &mut PaintContext, rect: RectF, _children: &[NodeId]) {
        let Some(cache) = &self.cache else {
            if self.failure.is_some() {
                let run = cx
                    .text_engine
                    .shape("Markdown layout unavailable", cx.theme.typography.font_size);
                cx.text_engine.draw(
                    &run,
                    rect.x,
                    rect.y,
                    cx.theme.colors.typography.primary,
                    LAYER_CONTENT,
                    cx.scene,
                );
            }
            return;
        };
        for d in &cache.decorations {
            let color = match d.role {
                Role::Surface => cx.theme.colors.surfaces.level1,
                Role::Rule => cx.theme.colors.borders.subtle,
                Role::Task => cx.theme.colors.typography.primary,
            };
            let r = RectF::new(
                rect.x + d.rect.x,
                rect.y + d.rect.y,
                d.rect.width,
                d.rect.height,
            );
            cx.scene.push_rect(
                r,
                color,
                if d.above {
                    LAYER_CONTENT + 1
                } else {
                    LAYER_CONTENT - 1
                },
                None,
            );
        }
        let selection = self
            .selection
            .has_selection()
            .then(|| self.selection.range());
        let highlight = cx.theme.text_selection_color();
        for text in &cache.texts {
            let visual_selection = selection.and_then(|r| text.visual_selection(r));
            let mut y = rect.y + text.y;
            for line in &text.lines {
                let mut x = rect.x + text.x + text.align.offset(line.width, text.width);
                for face_run in &line.runs {
                    let glyph = match text.shape(face_run, &cache.catalog, cx.text_engine) {
                        Ok(glyph) => glyph,
                        Err(_) => {
                            let diagnostic = cx.text_engine.shape(
                                "Markdown layout unavailable",
                                cx.theme.typography.font_size,
                            );
                            cx.text_engine.draw(
                                &diagnostic,
                                rect.x,
                                rect.y,
                                cx.theme.colors.typography.primary,
                                LAYER_CONTENT,
                                cx.scene,
                            );
                            return;
                        }
                    };
                    let top = y + line.baseline - glyph.ascent;
                    let mut backgrounds = Vec::new();
                    for code in &text.inline_code {
                        let a = code.start.max(face_run.range.start);
                        let b = code.end.min(face_run.range.end);
                        if a < b {
                            backgrounds.extend(selection_spans(
                                &glyph,
                                a - face_run.range.start,
                                b - face_run.range.start,
                                line.height,
                                glyph.ascent - line.baseline,
                                cx.theme.colors.surfaces.level1,
                            ));
                        }
                    }
                    if let Some(r) = &visual_selection {
                        let a = r.start.max(face_run.range.start);
                        let b = r.end.min(face_run.range.end);
                        if a < b {
                            backgrounds.extend(selection_spans(
                                &glyph,
                                a - face_run.range.start,
                                b - face_run.range.start,
                                line.height,
                                glyph.ascent - line.baseline,
                                highlight,
                            ));
                        }
                    }
                    cx.text_engine.draw_with_backgrounds(
                        &glyph,
                        x,
                        top,
                        face_run.style.color,
                        backgrounds,
                        LAYER_CONTENT,
                        cx.scene,
                    );
                    for strike in &text.strike {
                        let a = strike.start.max(face_run.range.start);
                        let b = strike.end.min(face_run.range.end);
                        if a < b {
                            for span in selection_spans(
                                &glyph,
                                a - face_run.range.start,
                                b - face_run.range.start,
                                line.height,
                                0.0,
                                face_run.style.color,
                            ) {
                                cx.scene.push_rect(
                                    RectF::new(
                                        x + span.x_offset,
                                        y + line.baseline - glyph.ascent * 0.5,
                                        span.width,
                                        cx.theme.borders.b1,
                                    ),
                                    face_run.style.color,
                                    LAYER_CONTENT + 1,
                                    None,
                                );
                            }
                        }
                    }
                    x += face_run.width;
                }
                y += line.height;
            }
        }
    }
    fn handle_event(&mut self, cx: &mut EventContext, event: &Event, phase: EventPhase) -> bool {
        if matches!(event, Event::Pointer(_) | Event::Text(_) | Event::Key(_)) {
            self.handoff_used = true;
        }
        if !self.eligible() {
            return false;
        }
        match event {
            Event::Pointer(p) if matches!(phase, EventPhase::Target | EventPhase::Bubble) => {
                match p.kind {
                    PointerEventKind::Down
                    | PointerEventKind::DoubleClick
                    | PointerEventKind::TripleClick
                        if p.button == Some(PointerButton::Primary) =>
                    {
                        let Some(offset) = self.hit(p.local_pos.0, p.local_pos.1, cx.text_engine())
                        else {
                            return false;
                        };
                        let count = match p.kind {
                            PointerEventKind::DoubleClick => 2,
                            PointerEventKind::TripleClick => 3,
                            _ => cx.click_count(),
                        };
                        if self.selection.pointer_down(
                            offset,
                            count,
                            p.modifiers.shift,
                            cx.has_pointer_capture(),
                            p.modifiers.shift,
                        ) {
                            self.word_drag = None;
                            if count == 2 {
                                if let Some((a, b)) = self.safe_word(offset) {
                                    self.selection.pointer_down(
                                        a,
                                        1,
                                        false,
                                        cx.has_pointer_capture(),
                                        p.modifiers.shift,
                                    );
                                    self.selection.pointer_move(b);
                                    self.word_drag = Some((a, b));
                                }
                            }
                            cx.capture_pointer(PointerButton::Primary);
                            cx.invalidate_paint();
                            return true;
                        }
                    }
                    PointerEventKind::Move if self.selection.is_dragging() => {
                        if let Some(offset) =
                            self.hit(p.local_pos.0, p.local_pos.1, cx.text_engine())
                        {
                            if let Some((anchor_start, anchor_end)) = self.word_drag {
                                if let Some((a, b)) = self.safe_word(offset) {
                                    let (anchor, head) = if offset < anchor_start {
                                        (anchor_end, a)
                                    } else {
                                        (anchor_start, b)
                                    };
                                    self.selection.pointer_down(
                                        anchor,
                                        1,
                                        false,
                                        cx.has_pointer_capture(),
                                        p.modifiers.shift,
                                    );
                                    self.selection.pointer_move(head);
                                }
                            } else {
                                self.selection.pointer_move(offset);
                            }
                            cx.invalidate_paint();
                            return true;
                        }
                    }
                    PointerEventKind::Up
                        if self.selection.is_dragging()
                            && p.button == Some(PointerButton::Primary) =>
                    {
                        self.selection.pointer_up();
                        self.word_drag = None;
                        cx.release_pointer();
                        cx.invalidate_paint();
                        return true;
                    }
                    _ => {}
                }
                false
            }
            Event::Text(t) if phase == EventPhase::Target => {
                #[cfg(target_os = "macos")]
                let shortcut = t.modifiers.meta;
                #[cfg(not(target_os = "macos"))]
                let shortcut = t.modifiers.ctrl;
                if !shortcut {
                    return false;
                }
                match t.text.as_str() {
                    "c" | "C" if t.modifiers.shift => {
                        cx.clipboard_set_text(self.document.source());
                        true
                    }
                    "c" | "C" => {
                        if let Some(text) = self.selection.copy_to_clipboard() {
                            cx.clipboard_set_text(&text);
                            true
                        } else {
                            false
                        }
                    }
                    "a" | "A" => {
                        self.selection.select_all();
                        cx.invalidate_paint();
                        true
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }
    fn hit_testable(&self) -> bool {
        self.eligible()
    }
    fn focusable(&self) -> bool {
        self.eligible()
    }
    fn capture_eligible(&self) -> bool {
        self.eligible()
    }
    fn on_mount(&mut self) {
        self.handoff_mounted = true;
        if self.read_handoff_status != MarkdownReadHandoffStatus::Applied {
            self.selection.clear();
        } else {
            self.selection.pointer_up();
        }
        self.word_drag = None;
    }
    fn on_blur(&mut self, _cx: &mut EventContext) {
        if self.selection.policy() == TextSelectionPolicy::Enabled
            && self.selection_retention == MarkdownSelectionRetention::RetainOnBlur
        {
            self.selection.pointer_up();
        } else {
            self.selection.clear();
        }
        self.word_drag = None;
    }
    fn on_capture_leave(&mut self, _cx: &mut EventContext) {
        if self.selection.policy() == TextSelectionPolicy::OnCapture {
            self.selection.clear();
            self.word_drag = None;
        } else if self.selection.policy() == TextSelectionPolicy::Enabled
            && self.selection_retention == MarkdownSelectionRetention::RetainOnBlur
        {
            self.selection.pointer_up();
            self.word_drag = None;
        }
    }
    fn clear_read_only_text_selection(&mut self) -> bool {
        if !self.selection.has_selection() && !self.selection.is_dragging() {
            return false;
        }
        self.selection.clear();
        self.word_drag = None;
        true
    }
    fn clear_read_only_text_selection_on_outside_down(&mut self) -> bool {
        // INV-MD-CLEAR-REASON: retention changes only implicit outside-down.
        if self.selection.policy() == TextSelectionPolicy::Enabled
            && self.selection_retention == MarkdownSelectionRetention::RetainOnBlur
        {
            false
        } else {
            self.clear_read_only_text_selection()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn handoff_target(document: Arc<MarkdownDocument>) -> MarkdownView {
        MarkdownView::new(
            document,
            MarkdownStyle::new("Arial", "Courier New").unwrap(),
        )
        .with_selection_policy(TextSelectionPolicy::Enabled)
        .with_selection_retention(MarkdownSelectionRetention::RetainOnBlur)
    }

    #[test]
    fn markdown_read_handoff_rejects_invalid_targets_and_private_snapshots_atomically() {
        use MarkdownReadHandoffError as Error;
        use MarkdownReadHandoffStatus as Status;
        let doc =
            MarkdownDocument::parse("看alpha".into(), super::super::MarkdownLimits::default())
                .unwrap();
        // Private construction here supplies negative cases only. Positive
        // capture/application proof uses real routed selection in markdown_view.
        let saved = MarkdownReadHandoff {
            document: doc.clone(),
            anchor: 3,
            head: 0,
        };
        let mut target = handoff_target(doc.clone());
        assert_eq!(
            target.capture_read_handoff().unwrap_err(),
            Error::LayoutUnavailable
        );
        let empty = handoff_target(
            MarkdownDocument::parse("".into(), super::super::MarkdownLimits::default()).unwrap(),
        );
        assert_eq!(
            empty.capture_read_handoff().unwrap_err(),
            Error::LayoutUnavailable
        );
        let mut disabled = MarkdownView::new(
            doc.clone(),
            MarkdownStyle::new("Arial", "Courier New").unwrap(),
        );
        assert_eq!(
            disabled.capture_read_handoff().unwrap_err(),
            Error::SelectionDisabled
        );
        assert_eq!(
            disabled.try_stage_read_handoff(&saved),
            Err(Error::SelectionDisabled)
        );
        let mut default_retention = MarkdownView::new(
            doc.clone(),
            MarkdownStyle::new("Arial", "Courier New").unwrap(),
        )
        .with_selection_policy(TextSelectionPolicy::Enabled);
        assert_eq!(
            default_retention.try_stage_read_handoff(&saved),
            Err(Error::UnsupportedTarget)
        );
        let other =
            MarkdownDocument::parse("看alpha".into(), super::super::MarkdownLimits::default())
                .unwrap();
        assert!(!Arc::ptr_eq(&doc, &other));
        assert_eq!(
            handoff_target(other).try_stage_read_handoff(&saved),
            Err(Error::DocumentMismatch)
        );
        for (anchor, head) in [(1, 0), (0, 2), (usize::MAX, 0)] {
            let invalid = MarkdownReadHandoff {
                document: doc.clone(),
                anchor,
                head,
            };
            assert_eq!(
                target.try_stage_read_handoff(&invalid),
                Err(Error::InvalidBoundary)
            );
            assert_eq!(target.read_handoff_status(), Status::NotRequested);
            assert_eq!(target.selection.range(), (0, 0));
        }
        target.try_stage_read_handoff(&saved).unwrap();
        assert_eq!(target.read_handoff_status(), Status::Pending);
        assert_eq!(
            target.capture_read_handoff().unwrap_err(),
            Error::UnsupportedTarget
        );
        assert_eq!(
            target.try_stage_read_handoff(&saved),
            Err(Error::UnsupportedTarget)
        );
        assert!(Arc::ptr_eq(
            target.pending_read_handoff.as_ref().unwrap().document(),
            &doc
        ));
        assert_eq!(target.pending_read_handoff.as_ref().unwrap().anchor, 3);
        assert_eq!(target.selection.range(), (0, 0));
        let mut mounted = handoff_target(doc.clone());
        mounted.on_mount();
        assert_eq!(
            mounted.try_stage_read_handoff(&saved),
            Err(Error::UnsupportedTarget)
        );
        let mut used = handoff_target(doc.clone());
        used.handoff_used = true;
        assert_eq!(
            used.try_stage_read_handoff(&saved),
            Err(Error::UnsupportedTarget)
        );
        let mut dragging = handoff_target(doc.clone());
        assert!(dragging.selection.pointer_down(3, 2, false, true, false));
        assert_eq!(
            dragging.capture_read_handoff().unwrap_err(),
            Error::SelectionInProgress
        );
        // Revalidation refuses changed policy, document, backing string or an
        // invalid pending interval. None publishes a partial logical range.
        for mode in 0..5 {
            let mut view = handoff_target(doc.clone());
            view.try_stage_read_handoff(&saved).unwrap();
            view.on_mount();
            let expected = match mode {
                0 => {
                    view.selection.set_policy(TextSelectionPolicy::Disabled);
                    Error::SelectionDisabled
                }
                1 => {
                    view.selection_retention = MarkdownSelectionRetention::ClearOnBlur;
                    Error::UnsupportedTarget
                }
                2 => {
                    view.document = MarkdownDocument::parse(
                        "看alpha".into(),
                        super::super::MarkdownLimits::default(),
                    )
                    .unwrap();
                    Error::DocumentMismatch
                }
                3 => {
                    view.selection.set_text("other".into());
                    Error::DocumentMismatch
                }
                _ => {
                    view.pending_read_handoff.as_mut().unwrap().head = 1;
                    Error::InvalidBoundary
                }
            };
            assert!(!view.apply_pending_read_handoff());
            assert_eq!(view.read_handoff_status(), Status::Rejected(expected));
            assert_eq!(view.selection.range(), (0, 0));
            assert!(!view.has_pending_read_handoff());
            assert!(!view.apply_pending_read_handoff());
            assert_eq!(view.read_handoff_status(), Status::Rejected(expected));
        }
    }
    #[test]
    fn markdown_exact_column_distribution() {
        assert_eq!(
            table_columns(&[100.0, 200.0], &[60.0, 80.0], 340.0).unwrap(),
            [120.0, 220.0]
        );
        let mid = table_columns(&[100.0, 200.0], &[60.0, 80.0], 220.0).unwrap();
        assert_eq!(mid, [80.0, 140.0]);
        assert_eq!(
            table_columns(&[100.0, 200.0], &[60.0, 80.0], 100.0).unwrap(),
            [60.0, 80.0]
        );
        assert_eq!(
            table_columns(&[0.0, 0.0], &[0.0, 0.0], 20.0).unwrap(),
            [10.0, 10.0]
        );
    }

    #[cfg(target_os = "macos")]
    fn engine() -> crate::platform::font_manager_text_engine::FontManagerTextEngine {
        let mut fonts = crate::platform::font_manager::FontManager::new();
        let mut first = None;
        for asset in crate::tests::_support::markdown_fonts() {
            let id = fonts.register_font_asset(asset).unwrap();
            first.get_or_insert(id);
        }
        crate::platform::font_manager_text_engine::FontManagerTextEngine::with_face_catalog(
            fonts.face_catalog(),
            first.unwrap(),
        )
        .unwrap()
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn text_selection_palette_markdown_keeps_inline_code_decoration() {
        use crate::core::app::RetainedSceneTestHarness;
        use crate::core::icons::builtin_registry;
        use crate::core::widget_tree::WidgetTree;
        use crate::tests::_support::{assert_painted_text_backgrounds, text_selection_palette_cases};

        let doc = MarkdownDocument::parse("`selected`".into(), super::super::MarkdownLimits::default())
            .unwrap();
        let mut text_engine = engine();
        for (theme, expected) in text_selection_palette_cases() {
            let mut tree = WidgetTree::new();
            let root = tree.create_root(Box::new(handoff_target(doc.clone())), None, 0);
            let mut harness = RetainedSceneTestHarness::new();
            let constraints = Constraints::loose(Size::new(360.0, 100.0));
            let icons = builtin_registry();
            harness.frame(&mut tree, root, constraints, &theme, &mut text_engine, &icons);
            let view = tree
                .node_mut(root)
                .unwrap()
                .widget
                .as_any_mut()
                .downcast_mut::<MarkdownView>()
                .unwrap();
            view.selection.select_all();
            tree.mark_paint_dirty(root);
            let frame = harness.frame(&mut tree, root, constraints, &theme, &mut text_engine, &icons);
            assert_painted_text_backgrounds(
                &frame.scene_root,
                "selected",
                &[theme.colors.surfaces.level1, expected],
            );
        }
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn markdown_bound_selection_tabs_copy_resize_and_cache() {
        use crate::core::{
            event::{KeyboardAuthorityMode, Modifiers, TextInputEvent},
            theme::Theme,
            widget_tree::WidgetTree,
        };
        let source = "# **Heading**\n\nA *styled* é and אבג 看龍.\n\n```\na\tb\n```";
        let doc = MarkdownDocument::parse(source.into(), super::super::MarkdownLimits::default())
            .unwrap();
        let mut view = MarkdownView::new(
            doc.clone(),
            MarkdownStyle::new("Arial", "Courier New").unwrap(),
        )
        .with_selection_policy(TextSelectionPolicy::Enabled);
        let mut tree = WidgetTree::new();
        let mut engine = engine();
        let theme = Theme::default();
        let mut layout = |view: &mut MarkdownView, width: f32, engine: &mut dyn TextEngine| {
            let mut cx = LayoutContext::new(&mut tree, engine, &theme);
            view.layout(&mut cx, Constraints::loose(Size::new(width, 800.0)), &[])
        };
        let initial = layout(&mut view, 360.0, &mut engine);
        assert!(view.layout_failure().is_none());
        assert!(initial.content_size.height > 0.0);
        let identity = view.cache.as_ref().unwrap().texts.as_ptr();
        layout(&mut view, 360.0, &mut engine);
        assert_eq!(
            identity,
            view.cache.as_ref().unwrap().texts.as_ptr(),
            "same cache: no repeated paragraph layout"
        );
        let code = view
            .cache
            .as_ref()
            .unwrap()
            .texts
            .iter()
            .find(|t| t.paragraph.text.contains("a   b"))
            .unwrap();
        let tab = doc.reading_text().find('\t').unwrap();
        let mapping = code
            .map
            .iter()
            .find(|m| m.reading == (tab..tab + 1))
            .unwrap();
        assert!(!mapping.exact);
        assert_eq!(&code.paragraph.text[mapping.visual.clone()], "   ");
        assert_eq!(
            code.visual_selection((tab, tab + 1)),
            Some(mapping.visual.clone())
        );
        let combined = doc.reading_text().find("é").unwrap();
        let (a, b) = view.safe_word(combined).unwrap();
        assert!(
            a <= combined && b >= combined + "é".len(),
            "word cannot bisect the shaped combining cluster"
        );
        for text in &view.cache.as_ref().unwrap().texts {
            let mut y = text.y;
            for line in &text.lines {
                let mut x = text.x;
                for run in &line.runs {
                    let glyph = text
                        .shape(run, &view.cache.as_ref().unwrap().catalog, &mut engine)
                        .unwrap();
                    for cluster in &glyph.clusters {
                        let offset = view
                            .hit(
                                x + (cluster.x_start + cluster.x_end) * 0.5,
                                y + line.height * 0.5,
                                &mut engine,
                            )
                            .unwrap();
                        assert!(doc.reading_text().is_char_boundary(offset));
                        assert!(view.cache.as_ref().unwrap().boundaries.contains(&offset));
                    }
                    x += run.width;
                }
                y += line.height;
            }
        }
        view.selection.pointer_down(a, 1, false, true, false);
        view.selection.pointer_move(b);
        view.selection.pointer_up();
        let selected = view.selection.range();
        let exact = view.selection.selected_text().to_owned();
        layout(&mut view, 180.0, &mut engine);
        engine.set_scale(2.0);
        layout(&mut view, 180.0, &mut engine);
        assert_eq!(view.selection.range(), selected);
        assert_eq!(view.selection.selected_text(), exact);
        assert!(Arc::ptr_eq(view.document(), &doc));
        // Copy uses the existing Event::Text and clipboard write boundary. The
        // original test transport prevents reads/writes of the user's clipboard.
        let mut clipboard = crate::platform::clipboard::Clipboard::new().unwrap();
        clipboard.use_memory_transport_for_test("untouched");
        let mut tree = WidgetTree::new();
        let tree_ptr = &mut tree as *mut WidgetTree;
        let _phase2 = crate::core::rect_authority::UpdateAuthorityGuard::enter();
        for (text, shift, expected) in [
            ("a", false, None),
            ("c", false, Some(doc.reading_text())),
            ("C", true, Some(source)),
        ] {
            let mut cx = EventContext::new(
                crate::utils::id::TreeWidgetId(1),
                tree_ptr,
                0,
                &mut clipboard,
                Some(&mut engine),
                None,
                true,
                false,
                KeyboardAuthorityMode::Capture,
            );
            let event = Event::Text(TextInputEvent {
                text: text.into(),
                modifiers: Modifiers {
                    meta: true,
                    ctrl: true,
                    shift,
                    ..Modifiers::default()
                },
            });
            assert!(view.handle_event(&mut cx, &event, EventPhase::Target));
            drop(cx);
            if let Some(expected) = expected {
                assert_eq!(clipboard.get_text().as_deref(), Some(expected));
            }
        }
        view.on_mount();
        assert!(
            !view.selection.has_selection(),
            "a fresh concrete mount cannot retain old selection"
        );
    }

    #[cfg(target_os = "macos")]
    #[test]
    fn markdown_required_real_styles_and_fixed_pitch_refuse_explicitly() {
        use crate::core::{theme::Theme, widget_tree::WidgetTree};
        let doc = MarkdownDocument::parse(
            "**bold** `code`".into(),
            super::super::MarkdownLimits::default(),
        )
        .unwrap();
        let mut tree = WidgetTree::new();
        let theme = Theme::default();
        let mut native = engine();
        for (family, code, expected) in [
            (
                "Absent",
                "Courier New",
                MarkdownLayoutError::RequiredFaceUnavailable,
            ),
            (
                "Arial",
                "Arial",
                MarkdownLayoutError::RequiredFaceUnavailable,
            ),
        ] {
            let mut view =
                MarkdownView::new(doc.clone(), MarkdownStyle::new(family, code).unwrap());
            view.layout(
                &mut LayoutContext::new(&mut tree, &mut native, &theme),
                Constraints::loose(Size::new(200.0, 300.0)),
                &[],
            );
            assert_eq!(view.layout_failure(), Some(&expected));
            assert!(view.cache.is_none());
        }
        let mut view = MarkdownView::new(doc, MarkdownStyle::new("Arial", "Courier New").unwrap());
        view.layout(
            &mut LayoutContext::new(&mut tree, &mut native, &theme),
            Constraints::unbounded(),
            &[],
        );
        assert_eq!(
            view.layout_failure(),
            Some(&MarkdownLayoutError::InvalidGeometry)
        );
    }
}
