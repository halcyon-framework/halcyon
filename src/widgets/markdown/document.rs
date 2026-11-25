use pulldown_cmark::{Alignment, CodeBlockKind, Event, Options, Parser, Tag};
use std::{fmt, ops::Range, sync::Arc};

/// Explicit preparation bounds, independent of any application's inference limits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MarkdownLimits {
    pub max_source_bytes: usize,
    pub max_reading_bytes: usize,
    pub max_nodes: usize,
    pub max_depth: usize,
    pub max_table_columns: usize,
}
impl Default for MarkdownLimits {
    fn default() -> Self {
        Self {
            max_source_bytes: 262144,
            max_reading_bytes: 1048576,
            max_nodes: 32768,
            max_depth: 64,
            max_table_columns: 64,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MarkdownLimitKind {
    SourceBytes,
    ReadingBytes,
    Nodes,
    Depth,
    TableColumns,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MarkdownPrepareError {
    InvalidLimits,
    LimitExceeded {
        kind: MarkdownLimitKind,
        limit: usize,
        observed: usize,
    },
    InvalidSourceRange,
    UnsupportedEvent,
    Canceled,
}
impl fmt::Display for MarkdownPrepareError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Markdown preparation: {self:?}")
    }
}
impl std::error::Error for MarkdownPrepareError {}

#[derive(Clone, Copy, Default, Debug, PartialEq, Eq)]
pub(super) struct InlineStyle {
    pub bold: bool,
    pub italic: bool,
    pub strike: bool,
    pub code: bool,
}
#[derive(Clone)]
pub(super) struct InlineSpan {
    pub range: Range<usize>,
    pub style: InlineStyle,
}
#[derive(Clone, Copy)]
pub(super) enum CellAlign {
    Left,
    Center,
    Right,
}
#[derive(Clone)]
pub(super) struct Cell {
    pub range: Range<usize>,
    pub spans: Vec<InlineSpan>,
    pub header: bool,
}
pub(super) enum BlockKind {
    Text {
        heading: u8,
    },
    Code,
    Table {
        rows: Vec<Vec<Cell>>,
        align: Vec<CellAlign>,
    },
    Rule,
}
pub(super) struct Block {
    pub range: Range<usize>,
    pub spans: Vec<InlineSpan>,
    pub kind: BlockKind,
    pub list_depth: usize,
    pub quote_depth: usize,
    pub prefix: Option<Range<usize>>,
    pub task: Option<(Range<usize>, bool)>,
}
enum Kind {
    Root,
    Paragraph,
    Heading(u8),
    Quote,
    List(Option<u64>),
    Item,
    CodeBlock(
        #[allow(
            dead_code,
            reason = "Retained inert code-fence metadata; HAL-390 does not enable syntax highlighting"
        )]
        String,
    ),
    HtmlBlock,
    Table(Vec<CellAlign>),
    Row(bool),
    Cell,
    Emphasis,
    Strong,
    Strike,
    Link {
        #[allow(
            dead_code,
            reason = "Retained inert parser metadata under HAL-390 14.3"
        )]
        destination: String,
        #[allow(
            dead_code,
            reason = "Retained inert parser metadata under HAL-390 14.3"
        )]
        title: String,
    },
    Image {
        #[allow(
            dead_code,
            reason = "Retained inert parser metadata under HAL-390 14.3"
        )]
        destination: String,
        #[allow(
            dead_code,
            reason = "Retained inert parser metadata under HAL-390 14.3"
        )]
        title: String,
    },
    Text(String),
    Code(String),
    Html,
    Soft,
    Hard,
    Task(bool),
    Rule,
}
struct Node {
    kind: Kind,
    source: Range<usize>,
    children: Vec<usize>,
}
enum Origin {
    Parser(Range<usize>),
    Structural(
        #[allow(dead_code, reason = "HAL-390 14.3 requires structural node provenance")] usize,
    ),
}
struct Segment {
    #[allow(
        dead_code,
        reason = "HAL-390 14.3 requires complete reading-to-source provenance"
    )]
    reading: Range<usize>,
    #[allow(
        dead_code,
        reason = "HAL-390 14.3 requires complete reading-to-source provenance"
    )]
    origin: Origin,
    #[allow(
        dead_code,
        reason = "HAL-390 14.3 requires complete reading-to-source provenance"
    )]
    exact: bool,
}

/// One exact source revision. Debug deliberately omits all document content.
pub struct MarkdownDocument {
    source: Arc<str>,
    reading: String,
    nodes: Vec<Node>,
    segments: Vec<Segment>,
    pub(super) blocks: Vec<Block>,
    code: Vec<String>,
}
impl fmt::Debug for MarkdownDocument {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("MarkdownDocument")
            .field("source_bytes", &self.source.len())
            .field("reading_bytes", &self.reading.len())
            .field("nodes", &self.nodes.len())
            .field("code_blocks", &self.code.len())
            .finish_non_exhaustive()
    }
}
struct Budget {
    limits: MarkdownLimits,
    nodes: usize,
}
impl Budget {
    fn check(
        kind: MarkdownLimitKind,
        limit: usize,
        observed: usize,
    ) -> Result<(), MarkdownPrepareError> {
        if observed > limit {
            Err(MarkdownPrepareError::LimitExceeded {
                kind,
                limit,
                observed,
            })
        } else {
            Ok(())
        }
    }
    fn node(&mut self) -> Result<(), MarkdownPrepareError> {
        let next = self.nodes.checked_add(1).unwrap_or(usize::MAX);
        Self::check(MarkdownLimitKind::Nodes, self.limits.max_nodes, next)?;
        self.nodes = next;
        Ok(())
    }
}
impl MarkdownDocument {
    pub fn parse(
        source: Arc<str>,
        limits: MarkdownLimits,
    ) -> Result<Arc<Self>, MarkdownPrepareError> {
        Self::parse_with_cancel(source, limits, || false)
    }
    pub fn parse_with_cancel(
        source: Arc<str>,
        limits: MarkdownLimits,
        mut canceled: impl FnMut() -> bool,
    ) -> Result<Arc<Self>, MarkdownPrepareError> {
        if [
            limits.max_source_bytes,
            limits.max_reading_bytes,
            limits.max_nodes,
            limits.max_depth,
            limits.max_table_columns,
        ]
        .contains(&0)
        {
            return Err(MarkdownPrepareError::InvalidLimits);
        }
        Budget::check(
            MarkdownLimitKind::SourceBytes,
            limits.max_source_bytes,
            source.len(),
        )?;
        if canceled() {
            return Err(MarkdownPrepareError::Canceled);
        }
        let options =
            Options::ENABLE_TABLES | Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TASKLISTS;
        let parser = Parser::new_ext(&source, options);
        if canceled() {
            return Err(MarkdownPrepareError::Canceled);
        }
        let mut budget = Budget { limits, nodes: 0 };
        budget.node()?;
        let mut nodes = vec![Node {
            kind: Kind::Root,
            source: 0..source.len(),
            children: Vec::new(),
        }];
        let mut stack = vec![0];
        for (event, range) in parser.into_offset_iter() {
            if canceled() {
                return Err(MarkdownPrepareError::Canceled);
            }
            if source.get(range.clone()).is_none() {
                return Err(MarkdownPrepareError::InvalidSourceRange);
            }
            let parent = *stack.last().ok_or(MarkdownPrepareError::UnsupportedEvent)?;
            if matches!(event, Event::End(_)) {
                if stack.len() == 1 {
                    return Err(MarkdownPrepareError::UnsupportedEvent);
                }
                stack.pop();
                continue;
            }
            let opens = matches!(event, Event::Start(_));
            if opens {
                Budget::check(MarkdownLimitKind::Depth, limits.max_depth, stack.len())?;
            }
            let kind = match event {
                Event::Start(tag) => match tag {
                    Tag::Paragraph => Kind::Paragraph,
                    Tag::Heading { level, .. } => Kind::Heading(level as u8),
                    Tag::BlockQuote(_) => Kind::Quote,
                    Tag::List(start) => Kind::List(start),
                    Tag::Item => Kind::Item,
                    Tag::CodeBlock(kind) => Kind::CodeBlock(match kind {
                        CodeBlockKind::Indented => String::new(),
                        CodeBlockKind::Fenced(s) => s.into_string(),
                    }),
                    Tag::HtmlBlock => Kind::HtmlBlock,
                    Tag::Table(align) => {
                        Budget::check(
                            MarkdownLimitKind::TableColumns,
                            limits.max_table_columns,
                            align.len(),
                        )?;
                        Kind::Table(
                            align
                                .into_iter()
                                .map(|a| match a {
                                    Alignment::Center => CellAlign::Center,
                                    Alignment::Right => CellAlign::Right,
                                    _ => CellAlign::Left,
                                })
                                .collect(),
                        )
                    }
                    Tag::TableHead => Kind::Row(true),
                    Tag::TableRow => Kind::Row(false),
                    Tag::TableCell => {
                        Budget::check(
                            MarkdownLimitKind::TableColumns,
                            limits.max_table_columns,
                            nodes[parent].children.len().saturating_add(1),
                        )?;
                        Kind::Cell
                    }
                    Tag::Emphasis => Kind::Emphasis,
                    Tag::Strong => Kind::Strong,
                    Tag::Strikethrough => Kind::Strike,
                    Tag::Link {
                        dest_url, title, ..
                    } => Kind::Link {
                        destination: dest_url.into_string(),
                        title: title.into_string(),
                    },
                    Tag::Image {
                        dest_url, title, ..
                    } => Kind::Image {
                        destination: dest_url.into_string(),
                        title: title.into_string(),
                    },
                    _ => return Err(MarkdownPrepareError::UnsupportedEvent),
                },
                Event::Text(s) => Kind::Text(s.into_string()),
                Event::Code(s) => Kind::Code(s.into_string()),
                Event::Html(_) | Event::InlineHtml(_) => Kind::Html,
                Event::SoftBreak => Kind::Soft,
                Event::HardBreak => Kind::Hard,
                Event::TaskListMarker(checked) => Kind::Task(checked),
                Event::Rule => Kind::Rule,
                _ => return Err(MarkdownPrepareError::UnsupportedEvent),
            };
            budget.node()?;
            let id = nodes.len();
            nodes.push(Node {
                kind,
                source: range,
                children: Vec::new(),
            });
            nodes[parent].children.push(id);
            if opens {
                stack.push(id);
            }
        }
        if stack.len() != 1 {
            return Err(MarkdownPrepareError::UnsupportedEvent);
        }
        // Tight list items have inline children without a Paragraph event. Group
        // them iteratively; untrusted nesting never consumes the Rust call stack.
        let count = nodes.len();
        for id in 0..count {
            if !matches!(nodes[id].kind, Kind::Root | Kind::Quote | Kind::Item) {
                continue;
            }
            let children = std::mem::take(&mut nodes[id].children);
            let mut group: Vec<usize> = Vec::new();
            for child in children.into_iter().chain(std::iter::once(usize::MAX)) {
                let inline = child != usize::MAX && is_inline(&nodes[child].kind);
                if !inline && !group.is_empty() {
                    budget.node()?;
                    let first = group[0];
                    let last = *group.last().unwrap();
                    let source = nodes[first].source.start..nodes[last].source.end;
                    let next = nodes.len();
                    nodes.push(Node {
                        kind: Kind::Paragraph,
                        source,
                        children: std::mem::take(&mut group),
                    });
                    nodes[id].children.push(next);
                }
                if inline {
                    group.push(child);
                } else if child != usize::MAX {
                    nodes[id].children.push(child);
                }
            }
        }
        let mut document = Self {
            source,
            reading: String::new(),
            nodes,
            segments: Vec::new(),
            blocks: Vec::new(),
            code: Vec::new(),
        };
        document.project(&mut budget, &mut canceled)?;
        if canceled() {
            return Err(MarkdownPrepareError::Canceled);
        }
        Ok(Arc::new(document))
    }
    pub fn source(&self) -> &str {
        &self.source
    }
    pub fn reading_text(&self) -> &str {
        &self.reading
    }
    pub fn code_block_count(&self) -> usize {
        self.code.len()
    }
    pub fn code_literal(&self, ordinal: usize) -> Option<&str> {
        self.code.get(ordinal).map(String::as_str)
    }

    fn append(
        &mut self,
        value: &str,
        origin: Origin,
        budget: &mut Budget,
    ) -> Result<Range<usize>, MarkdownPrepareError> {
        let end = self
            .reading
            .len()
            .checked_add(value.len())
            .unwrap_or(usize::MAX);
        Budget::check(
            MarkdownLimitKind::ReadingBytes,
            budget.limits.max_reading_bytes,
            end,
        )?;
        let range = self.reading.len()..end;
        if !value.is_empty() {
            budget.node()?;
            let exact = match &origin {
                Origin::Parser(source) => self.source.get(source.clone()) == Some(value),
                _ => false,
            };
            self.segments.push(Segment {
                reading: range.clone(),
                origin,
                exact,
            });
            self.reading.push_str(value);
        }
        Ok(range)
    }
    fn inline(
        &mut self,
        root: usize,
        budget: &mut Budget,
        canceled: &mut impl FnMut() -> bool,
    ) -> Result<(Range<usize>, Vec<InlineSpan>, Option<(Range<usize>, bool)>), MarkdownPrepareError>
    {
        enum Action {
            Node(usize, InlineStyle),
            CloseImage(usize, InlineStyle),
        }
        let mut work: Vec<_> = self.nodes[root]
            .children
            .iter()
            .rev()
            .map(|id| Action::Node(*id, InlineStyle::default()))
            .collect();
        let start = self.reading.len();
        let mut spans: Vec<InlineSpan> = Vec::new();
        let mut task = None;
        while let Some(action) = work.pop() {
            if canceled() {
                return Err(MarkdownPrepareError::Canceled);
            }
            let (id, mut style) = match action {
                Action::Node(id, s) => (id, s),
                Action::CloseImage(id, s) => {
                    let range = self.append("]", Origin::Structural(id), budget)?;
                    budget.node()?;
                    spans.push(InlineSpan { range, style: s });
                    continue;
                }
            };
            let mut suffix = false;
            let value = match &self.nodes[id].kind {
                Kind::Text(s) => Some((s.clone(), false)),
                Kind::Code(s) => {
                    style.code = true;
                    Some((s.clone(), false))
                }
                Kind::Html => Some((self.source[self.nodes[id].source.clone()].to_owned(), false)),
                Kind::Soft => Some((" ".into(), false)),
                Kind::Hard => Some(("\n".into(), false)),
                Kind::Task(checked) => Some((if *checked { "[x] " } else { "[ ] " }.into(), true)),
                Kind::Image { .. } => {
                    suffix = true;
                    Some(("[image not loaded: ".into(), true))
                }
                Kind::Strong => {
                    style.bold = true;
                    None
                }
                Kind::Emphasis => {
                    style.italic = true;
                    None
                }
                Kind::Strike => {
                    style.strike = true;
                    None
                }
                _ => None,
            };
            if let Some((value, structural)) = value {
                let origin = if structural {
                    Origin::Structural(id)
                } else {
                    Origin::Parser(self.nodes[id].source.clone())
                };
                let range = self.append(&value, origin, budget)?;
                if let Kind::Task(checked) = self.nodes[id].kind {
                    task = Some((range.clone(), checked));
                }
                if !range.is_empty() {
                    budget.node()?;
                    spans.push(InlineSpan { range, style });
                }
            }
            if suffix {
                work.push(Action::CloseImage(id, style));
            }
            for child in self.nodes[id].children.iter().rev() {
                work.push(Action::Node(*child, style));
            }
        }
        Ok((start..self.reading.len(), spans, task))
    }
    fn project(
        &mut self,
        budget: &mut Budget,
        canceled: &mut impl FnMut() -> bool,
    ) -> Result<(), MarkdownPrepareError> {
        #[derive(Clone, Default)]
        struct Context {
            list: usize,
            quote: usize,
            prefix: Option<Range<usize>>,
        }
        enum Work {
            Visit(usize, Context),
            Separate(usize, &'static str),
            Item(usize, Context, String),
        }
        // Determine nonempty projections without recursion. A Rule creates only
        // geometry; it cannot fabricate a final reading separator.
        let mut visible = vec![false; self.nodes.len()];
        let mut stack = vec![(0, false)];
        while let Some((id, done)) = stack.pop() {
            if !done {
                stack.push((id, true));
                for child in &self.nodes[id].children {
                    stack.push((*child, false));
                }
                continue;
            }
            visible[id] = match &self.nodes[id].kind {
                Kind::Rule => false,
                Kind::Text(s) | Kind::Code(s) => !s.is_empty(),
                Kind::Html => !self.nodes[id].source.is_empty(),
                Kind::Item | Kind::Image { .. } | Kind::Task(_) | Kind::Soft | Kind::Hard => true,
                _ => self.nodes[id].children.iter().any(|id| visible[*id]),
            };
        }
        let mut work = vec![Work::Visit(0, Context::default())];
        while let Some(action) = work.pop() {
            if canceled() {
                return Err(MarkdownPrepareError::Canceled);
            }
            let (id, mut context) = match action {
                Work::Separate(id, s) => {
                    self.append(s, Origin::Structural(id), budget)?;
                    continue;
                }
                Work::Item(id, mut c, prefix) => {
                    c.prefix = Some(self.append(&prefix, Origin::Structural(id), budget)?);
                    work.push(Work::Visit(id, c));
                    continue;
                }
                Work::Visit(id, c) => (id, c),
            };
            match &self.nodes[id].kind {
                Kind::Root | Kind::Quote | Kind::Item => {
                    if matches!(self.nodes[id].kind, Kind::Quote) {
                        context.quote += 1;
                    }
                    if matches!(self.nodes[id].kind, Kind::Item)
                        && (self.nodes[id].children.is_empty()
                            || matches!(self.nodes[self.nodes[id].children[0]].kind, Kind::List(_)))
                    {
                        budget.node()?;
                        let offset = self.reading.len();
                        self.blocks.push(Block {
                            range: offset..offset,
                            spans: Vec::new(),
                            kind: BlockKind::Text { heading: 0 },
                            list_depth: context.list,
                            quote_depth: context.quote,
                            prefix: context.prefix.take(),
                            task: None,
                        });
                    }
                    let mut actions = Vec::new();
                    let mut prior = false;
                    for child in &self.nodes[id].children {
                        let mut c = context.clone();
                        if prior {
                            c.prefix = None;
                        }
                        if visible[*child] {
                            if prior {
                                actions.push(Work::Separate(id, "\n\n"));
                            }
                            prior = true;
                        }
                        actions.push(Work::Visit(*child, c));
                    }
                    work.extend(actions.into_iter().rev());
                }
                Kind::List(start) => {
                    let mut actions = Vec::new();
                    for (index, child) in self.nodes[id].children.iter().enumerate() {
                        if index != 0 {
                            actions.push(Work::Separate(id, "\n"));
                        }
                        let marker = if let Some(n) = start {
                            format!(
                                "{}. ",
                                n.checked_add(index as u64)
                                    .ok_or(MarkdownPrepareError::InvalidSourceRange)?
                            )
                        } else {
                            "- ".into()
                        };
                        let prefix = format!("{}{}", "  ".repeat(context.list), marker);
                        let mut c = context.clone();
                        c.list += 1;
                        actions.push(Work::Item(*child, c, prefix));
                    }
                    work.extend(actions.into_iter().rev());
                }
                Kind::Table(align) => {
                    let align = align.clone();
                    let mut rows = Vec::new();
                    let start = self.reading.len();
                    let row_ids = self.nodes[id].children.clone();
                    for (ri, row_id) in row_ids.into_iter().enumerate() {
                        if ri != 0 {
                            self.append("\n", Origin::Structural(row_id), budget)?;
                        }
                        let header = matches!(self.nodes[row_id].kind, Kind::Row(true));
                        let cell_ids = self.nodes[row_id].children.clone();
                        let mut cells = Vec::new();
                        for (ci, cell_id) in cell_ids.into_iter().enumerate() {
                            if ci != 0 {
                                self.append("\t", Origin::Structural(row_id), budget)?;
                            }
                            let (range, spans, _) = self.inline(cell_id, budget, canceled)?;
                            budget.node()?;
                            cells.push(Cell {
                                range,
                                spans,
                                header,
                            });
                        }
                        budget.node()?;
                        rows.push(cells);
                    }
                    budget.node()?;
                    self.blocks.push(Block {
                        range: start..self.reading.len(),
                        spans: Vec::new(),
                        kind: BlockKind::Table { rows, align },
                        list_depth: context.list,
                        quote_depth: context.quote,
                        prefix: context.prefix,
                        task: None,
                    });
                }
                Kind::Rule => {
                    budget.node()?;
                    let offset = self.reading.len();
                    self.blocks.push(Block {
                        range: offset..offset,
                        spans: Vec::new(),
                        kind: BlockKind::Rule,
                        list_depth: context.list,
                        quote_depth: context.quote,
                        prefix: context.prefix,
                        task: None,
                    });
                }
                Kind::Paragraph | Kind::Heading(_) | Kind::CodeBlock(_) | Kind::HtmlBlock => {
                    let heading = if let Kind::Heading(h) = self.nodes[id].kind {
                        h
                    } else {
                        0
                    };
                    let code = matches!(self.nodes[id].kind, Kind::CodeBlock(_));
                    let (range, mut spans, task) = if matches!(self.nodes[id].kind, Kind::HtmlBlock)
                    {
                        // HTML event chunks can omit the CR of CRLF. Literal
                        // HTML uses the complete parser block source range, not
                        // concatenated normalized chunks or an HTML renderer.
                        let source = self.nodes[id].source.clone();
                        let literal = self.source[source.clone()].to_owned();
                        let range = self.append(&literal, Origin::Parser(source), budget)?;
                        budget.node()?;
                        (
                            range.clone(),
                            vec![InlineSpan {
                                range,
                                style: InlineStyle::default(),
                            }],
                            None,
                        )
                    } else {
                        self.inline(id, budget, canceled)?
                    };
                    if code {
                        budget.node()?;
                        self.code.push(self.reading[range.clone()].to_owned());
                        for span in &mut spans {
                            span.style = InlineStyle {
                                code: true,
                                ..InlineStyle::default()
                            };
                        }
                    }
                    budget.node()?;
                    self.blocks.push(Block {
                        range,
                        spans,
                        kind: if code {
                            BlockKind::Code
                        } else {
                            BlockKind::Text { heading }
                        },
                        list_depth: context.list,
                        quote_depth: context.quote,
                        prefix: context.prefix,
                        task,
                    });
                }
                _ => return Err(MarkdownPrepareError::UnsupportedEvent),
            }
        }
        Ok(())
    }
}
fn is_inline(kind: &Kind) -> bool {
    matches!(
        kind,
        Kind::Text(_)
            | Kind::Code(_)
            | Kind::Html
            | Kind::Soft
            | Kind::Hard
            | Kind::Task(_)
            | Kind::Image { .. }
            | Kind::Link { .. }
            | Kind::Strong
            | Kind::Emphasis
            | Kind::Strike
    )
}

#[cfg(test)]
mod provenance_tests {
    use super::*;
    #[test]
    fn markdown_provenance_uses_whole_decoded_ranges_and_structural_origins() {
        let doc = MarkdownDocument::parse(
            "A &amp; \\* `two\nwords`\n\n![alt](file:inert)".into(),
            MarkdownLimits::default(),
        )
        .unwrap();
        let mut end = 0;
        let mut decoded = false;
        let mut structural = false;
        for segment in &doc.segments {
            assert_eq!(segment.reading.start, end);
            end = segment.reading.end;
            match &segment.origin {
                Origin::Parser(r) => {
                    assert!(doc.source.get(r.clone()).is_some());
                    if segment.exact {
                        assert_eq!(
                            doc.source[r.clone()].as_bytes(),
                            doc.reading[segment.reading.clone()].as_bytes()
                        );
                    } else {
                        decoded = true;
                    }
                }
                Origin::Structural(id) => {
                    assert!(*id < doc.nodes.len());
                    assert!(!segment.exact);
                    structural = true;
                }
            }
        }
        assert_eq!(end, doc.reading.len());
        assert!(decoded && structural);
        assert!(doc.nodes.iter().any(|node|matches!(&node.kind,Kind::Image {destination,title} if destination=="file:inert" && title.is_empty())));
    }
}
