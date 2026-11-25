// Showcase — Layout tab content.
// One integrated composition demonstrating Flex (row/column),
// SplitPane (horizontal + vertical, nested), ScrollArea, and how layout
// primitives compose around real data widgets (TreeView, List,
// PropertyEditor v2) into an application-shaped UI.
//
// Structure:
//   Flex::column (fills tab content area)
//   ├── Toolbar         — Flex::row, natural height
//   ├── Main area       — SplitPane horizontal, flex:1
//   │   ├── Navigator   — SplitPane vertical
//   │   │   ├── Top     — TreeView (real, 10-node hierarchy)
//   │   │   └── Bottom  — List (real, 30 items)
//   │   └── Right side  — SplitPane horizontal
//   │       ├── Workspace — Flex::column with sub-toolbar + empty-state placeholder
//   │       └── Inspector — PropertyEditor v2 (3 sections × 3 editable String rows)
//   └── Status bar      — Flex::row, natural height

use crate::tip::Tip;
use halcyon::core::app::{App, AppConfig, Screen, ScreenContext, ScreenFrameContext};
use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::{Alignment, FlexParams};
use halcyon::core::ordered_selection::SelectionMode;
use halcyon::core::paint::Color;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::Widget;
use halcyon::core::widget_tree::NodeId;
use halcyon::engines::hierarchical::NodeMeta;
use halcyon::model::breadcrumbs::{BreadcrumbsSink, breadcrumbs_sink};
use halcyon::model::tab_bar::{TabBarSink, tab_bar_sink};
use halcyon::prelude::*;
use halcyon::widgets::breadcrumbs::{Breadcrumbs, CrumbId};
use halcyon::widgets::combo_box::ComboBox;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::flow_grid::FlowGrid;
use halcyon::widgets::gauge::{
    Gauge, GaugeConfig, GaugeFillStyle, GaugeMode, GaugeTickMarks, GaugeZone,
};
use halcyon::widgets::groupbox::GroupBox;
use halcyon::widgets::linear_gauge::{
    LinearGauge, LinearGaugeConfig, LinearGaugeOrientation, paint_linear_gauge,
};
use halcyon::widgets::list::List;
use halcyon::widgets::list_view::{CellPainter, DefaultCellPainter, ListView};
use halcyon::widgets::panel::Panel;
use halcyon::widgets::progress_bar::ProgressBar;
use halcyon::widgets::property_editor::{
    Cell as PeCell, CellSource, PropertyDataSource, PropertyEditor, PropertyNode, PropertyNodeKind,
    PropertyValue, RowArchetype,
};
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::sparkline::{Sparkline, SparklineConfig};
use halcyon::widgets::split_pane::SplitPane;
use halcyon::widgets::switch::Switch;
use halcyon::widgets::tab_bar::{TabBar, TabId};
use halcyon::widgets::tabular_view::Column;
use halcyon::widgets::tabular_view::column::{ColumnId, ColumnModel};
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use halcyon::widgets::text_area::TextArea;
use halcyon::widgets::tree_view::TreeView;
use std::cell::{Cell, RefCell};
use std::collections::{HashMap, VecDeque};
use std::fs;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant};

use super::*;

/// Build the Navigator TreeView hierarchy by walking the actual Halcyon
/// repository on disk. Root is the workspace baked in at compile time via
/// `CARGO_MANIFEST_DIR`. No hardcoded paths — what's on disk is what
/// shows. Build/system directories (target, .git, .idea, .claude, etc.)
/// are skipped; everything else is included.
///
/// Snapshot read at startup. A future Live `FilesystemDataSource` with
/// real invalidation events would replace this with a streaming source;
/// for the showcase demo, snapshot suffices.
fn build_navigator_tree() -> Vec<NodeMeta> {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    scan_repository(&root)
}

/// True if a directory entry should be excluded from the walk.
/// Build artifacts, VCS metadata, IDE folders, system noise.
fn should_skip_entry(name: &str) -> bool {
    matches!(
        name,
        "target"
            | ".git"
            | ".idea"
            | ".claude"
            | ".vscode"
            | "NVIDIA Corporation"
            | "node_modules"
            | ".DS_Store"
    )
}

/// Walk `root` recursively and produce a pre-order `Vec<NodeMeta>`.
/// Directories are listed before files at every level (alphabetical,
/// case-insensitive within each group). `has_children` is resolved in a
/// final sweep.
fn scan_repository(root: &Path) -> Vec<NodeMeta> {
    let mut nodes: Vec<NodeMeta> = Vec::new();

    // Root node — label is the directory's own name (e.g. "halcyon").
    let root_label = root
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("workspace")
        .to_string();
    nodes.push(NodeMeta {
        node_id: 0,
        parent_index: None,
        depth: 0,
        has_children: false, // resolved below
        label: root_label,
    });

    walk_dir(root, &mut nodes, 0, 1);

    // Final sweep — set `has_children` for every node that appears as a
    // parent in the produced tree.
    let mut has_child = vec![false; nodes.len()];
    for node in &nodes {
        if let Some(parent) = node.parent_index {
            if parent < has_child.len() {
                has_child[parent] = true;
            }
        }
    }
    for (i, node) in nodes.iter_mut().enumerate() {
        node.has_children = has_child[i];
    }

    nodes
}

/// Recursive helper for `scan_repository`.
/// Reads `dir`, sorts dirs-first then files, appends each entry to
/// `nodes` with the given `depth` and `parent_idx`, recurses into
/// subdirectories. Errors from `read_dir` are treated as "no children"
/// for the affected subdirectory — no panic, no abort.
fn walk_dir(dir: &Path, nodes: &mut Vec<NodeMeta>, parent_idx: usize, depth: u32) {
    let entries = match fs::read_dir(dir) {
        Ok(e) => e,
        Err(_) => return,
    };

    let mut dirs: Vec<PathBuf> = Vec::new();
    let mut files: Vec<PathBuf> = Vec::new();

    for entry in entries.flatten() {
        let path = entry.path();
        let name = match path.file_name().and_then(|n| n.to_str()) {
            Some(n) => n,
            None => continue,
        };
        if should_skip_entry(name) {
            continue;
        }
        let is_dir = entry
            .file_type()
            .map(|t| t.is_dir())
            .unwrap_or_else(|_| path.is_dir());
        if is_dir {
            dirs.push(path);
        } else {
            files.push(path);
        }
    }

    // Alphabetical, case-insensitive, within each group.
    let key = |p: &PathBuf| {
        p.file_name()
            .and_then(|n| n.to_str())
            .map(str::to_ascii_lowercase)
            .unwrap_or_default()
    };
    dirs.sort_by_key(key);
    files.sort_by_key(key);

    for path in dirs.into_iter().chain(files.into_iter()) {
        let name = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("?")
            .to_string();
        let this_idx = nodes.len();
        nodes.push(NodeMeta {
            node_id: this_idx as u64,
            parent_index: Some(parent_idx),
            depth,
            has_children: false, // resolved by scan_repository's final sweep
            label: name,
        });
        if path.is_dir() {
            walk_dir(&path, nodes, this_idx, depth + 1);
        }
    }
}

/// In-memory PropertyDataSource backing the Layout-tab Inspector.
///
/// Three Section rows (Properties / Appearance / Behavior), three Data rows
/// per section. Each row carries a typed `PropertyValue` matching its
/// semantic kind — String, Integer, Float, Bool, Choice, Color — so PE v2
/// mounts the correct editor per cell (SPEC-HAL-260A §6.2). Mirrors the
/// pattern in `examples/property_editor_v2.rs` adapted to a single Value
/// column. Edits commit through `commit_cell`, which inserts the new
/// value and bumps `revision` so the next paint reads it back.
struct InspectorSource {
    nodes: Vec<PropertyNode>,
    cells: HashMap<(u64, ColumnId), PeCell>,
    value_col_id: ColumnId,
    revision: u64,
    next_id: u64,
}

impl InspectorSource {
    fn new(value_col_id: ColumnId) -> Self {
        let mut s = Self {
            nodes: Vec::new(),
            cells: HashMap::new(),
            value_col_id,
            revision: 1,
            next_id: 1,
        };
        s.populate();
        s
    }

    fn alloc_id(&mut self) -> u64 {
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    fn add_section(&mut self, label: &str) -> u64 {
        let id = self.alloc_id();
        self.nodes.push(PropertyNode {
            node_id: id,
            label: label.into(),
            parent_id: None,
            kind: PropertyNodeKind::Section,
        });
        id
    }

    /// Add a Data row carrying a typed `PropertyValue`. PE v2 dispatches
    /// to the correct editor per variant (SPEC-HAL-260A §6.2): String →
    /// TextField, Integer/Float → SpinBox, Bool → always-mounted Checkbox,
    /// Choice → ComboBox, Color → ColorPickerField. Mirrors the pattern
    /// in `examples/property_editor_v2.rs` `DemoSource::add_property`,
    /// adapted to the showcase's single Value column.
    fn add_property(&mut self, label: &str, parent_id: u64, value: PropertyValue) {
        let id = self.alloc_id();
        self.nodes.push(PropertyNode {
            node_id: id,
            label: label.into(),
            parent_id: Some(parent_id),
            kind: PropertyNodeKind::Data,
        });
        let editable = value.is_editable();
        self.cells
            .insert((id, self.value_col_id), PeCell::new(value, editable));
    }

    fn populate(&mut self) {
        let properties = self.add_section("Properties");
        self.add_property("Name", properties, PropertyValue::String("Document".into()));
        self.add_property("Size", properties, PropertyValue::Integer(24));
        self.add_property(
            "Format",
            properties,
            PropertyValue::Choice {
                selected: 1,
                options: vec!["RGB".into(), "RGBA".into(), "HSL".into(), "CMYK".into()],
            },
        );

        let appearance = self.add_section("Appearance");
        self.add_property(
            "Theme",
            appearance,
            PropertyValue::Choice {
                selected: 1,
                options: vec!["Light".into(), "Dark".into(), "System".into()],
            },
        );
        self.add_property(
            "Tint",
            appearance,
            PropertyValue::Color(Color::from_hex(0x4888C8)),
        );
        self.add_property("Opacity", appearance, PropertyValue::Float(0.95));

        let behavior = self.add_section("Behavior");
        self.add_property("Editable", behavior, PropertyValue::Bool(true));
        self.add_property("Visible", behavior, PropertyValue::Bool(true));
        self.add_property("Locked", behavior, PropertyValue::Bool(false));
    }
}

impl CellSource for InspectorSource {
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<PeCell> {
        self.cells.get(&(row_id, column_id)).cloned()
    }

    fn row_archetype(&self, row_id: u64) -> RowArchetype {
        self.nodes
            .iter()
            .find(|n| n.node_id == row_id)
            .map_or(RowArchetype::Data, |n| match n.kind {
                PropertyNodeKind::Data => RowArchetype::Data,
                PropertyNodeKind::Section | PropertyNodeKind::SubSection => RowArchetype::Band,
            })
    }

    fn commit_cell(&mut self, row_id: u64, column_id: ColumnId, new_value: PropertyValue) -> bool {
        let editable = new_value.is_editable();
        self.cells
            .insert((row_id, column_id), PeCell::new(new_value, editable));
        self.revision += 1;
        true
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}

impl PropertyDataSource for InspectorSource {
    fn nodes(&self) -> Vec<PropertyNode> {
        self.nodes
            .iter()
            .map(|n| PropertyNode {
                node_id: n.node_id,
                label: n.label.clone(),
                parent_id: n.parent_id,
                kind: n.kind,
            })
            .collect()
    }
}

impl ShowcaseScreen {
    pub(super) fn build_layout(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        // Outer column fills the tab content area. No outer ScrollArea —
        // Layout tab is fixed-layout so SplitPane children get a real
        // bounded height from above.
        let outer = cx
            .tree
            .add_child(parent, Box::new(Flex::column().with_spacing(0.0)), None, 0);
        cx.tree.set_flex(outer, FlexParams::flexible(1.0));

        // ═══════════════════════════════════════════════════════════
        // TOOLBAR (slot 0) — natural height
        // ═══════════════════════════════════════════════════════════
        let toolbar = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(8.0))
                    .with_align_items(Alignment::Center),
            ),
            None,
            0,
        );

        for (i, &(label, tip)) in [
            ("Action", "Application actions"),
            ("Edit", "Editing commands"),
            ("View", "View options"),
            ("Tools", "Utilities and settings"),
        ]
        .iter()
        .enumerate()
        {
            let b_tip = cx
                .tree
                .add_child(toolbar, Box::new(Tip::new(tip)), None, i as i16);
            let b = cx.tree.add_child(
                b_tip,
                Box::new(Button::new(NodeId(0)).with_padding(6.0)),
                None,
                0,
            );
            let lbl = cx.tree.add_child(b, Box::new(Label::new(label)), None, 0);
            halcyon::replace_widget!(
                cx.tree,
                b,
                Button::new(lbl).with_padding(6.0).on_click(|| {})
            );
        }

        let tb_spacer = cx
            .tree
            .add_child(toolbar, Box::new(FlexSpacer::new()), None, 4);
        cx.tree.set_flex(tb_spacer, FlexParams::flexible(1.0));

        cx.tree.add_child(
            toolbar,
            Box::new(Label::new("Default Profile").with_size(11.0)),
            None,
            5,
        );

        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 1);

        // ═══════════════════════════════════════════════════════════
        // MAIN AREA (slot 2) — SplitPane horizontal, flex:1
        // Outer split: Navigator | (Workspace | Inspector)
        // ═══════════════════════════════════════════════════════════
        let main_split = cx.tree.add_child(
            outer,
            Box::new(
                SplitPane::horizontal(NodeId(0), NodeId(0))
                    .initial_ratio(0.22)
                    .leading_min(180.0)
                    .trailing_min(360.0),
            ),
            None,
            2,
        );
        cx.tree.set_flex(main_split, FlexParams::flexible(1.0));

        // ── LEFT: Navigator (SplitPane vertical) ──────────────────
        let nav_split = cx.tree.add_child(
            main_split,
            Box::new(
                SplitPane::vertical(NodeId(0), NodeId(0))
                    .initial_ratio(0.55)
                    .leading_min(80.0)
                    .trailing_min(80.0),
            ),
            None,
            0,
        );

        // Navigator top — real TreeView with mock 10-node hierarchy.
        // Build TreeView via the standard pattern from examples/treeview.rs:
        // capture body + footer + scroll-link BEFORE moving TreeView into a Box.
        let mut tree_view = TreeView::new();
        tree_view.set_tree_data(build_navigator_tree());

        let mut col_model = ColumnModel::new();
        let tree_col = Column::new(180.0).with_flex(1.0).with_min_width(180.0);
        let tree_col_id = tree_col.id;
        col_model.push(tree_col);
        tree_view.set_columns(col_model);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Name");
        tree_view.set_footer_height(0.0); // no footer in this compact form

        let hscroll_link = tree_view.horizontal_scroll_link();
        let tree_body = tree_view.create_body_widget();
        let tree_footer = tree_view.create_footer_widget();

        // TreeView mounted directly under nav_split. SplitPane.layout routes
        // child layout via cx.layout_child(children[0]) which invokes the
        // dispatcher that seals ScrollArea's DeferredWiring (SPEC-HAL-154).
        // Wrapping TreeView in a single-child Panel (B1D) broke that call path.
        let tv_id = cx.tree.add_child(nav_split, Box::new(tree_view), None, 0);

        let tv_scroll = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_content_owned_vertical_translation();
        let tv_scroll_id = cx.tree.add_child(tv_id, Box::new(tv_scroll), None, 0);

        let tv_body_id = cx
            .tree
            .add_child(tv_scroll_id, Box::new(tree_body), None, 0);

        if let Some(node) = cx.tree.node_mut(tv_scroll_id) {
            if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(tv_body_id);
            }
        }

        cx.tree.add_child(tv_id, Box::new(tree_footer), None, 1);

        // Navigator bottom — real List with 30 auto-labeled items.
        // List paints "Item N" rows internally (SPEC-HAL-094); no custom
        // label hook without ListView+DataSource machinery, which is
        // heavier than this layout demo needs.
        let mut list = List::new(SelectionMode::Multi);
        list.set_item_count(30);

        // List + ScrollArea mounted directly under nav_split (same reasoning
        // as TreeView above — Panel-wrap broke the SPEC-HAL-154 seal path).
        let list_scroll = ScrollArea::new_unwired().with_content_size(180.0, 30.0 * 24.0);
        let list_scroll_id = cx.tree.add_child(nav_split, Box::new(list_scroll), None, 1);

        let list_id = cx.tree.add_child(list_scroll_id, Box::new(list), None, 0);

        if let Some(node) = cx.tree.node_mut(list_scroll_id) {
            if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(list_id);
            }
        }

        // Close the vertical Navigator split now that both panes exist.
        halcyon::replace_widget!(
            cx.tree,
            nav_split,
            SplitPane::vertical(tv_id, list_scroll_id)
                .initial_ratio(0.55)
                .leading_min(80.0)
                .trailing_min(80.0)
        );

        // ── RIGHT: nested SplitPane (Workspace | Inspector) ───────
        let right_split = cx.tree.add_child(
            main_split,
            Box::new(
                SplitPane::horizontal(NodeId(0), NodeId(0))
                    .initial_ratio(0.70)
                    .leading_min(240.0)
                    .trailing_min(200.0),
            ),
            None,
            1,
        );

        // ── CENTER: Workspace ─────────────────────────────────────
        let ws_col = cx.tree.add_child(
            right_split,
            Box::new(Flex::column().with_spacing(0.0)),
            None,
            0,
        );

        // Workspace sub-toolbar
        let ws_subtoolbar = cx.tree.add_child(
            ws_col,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(8.0))
                    .with_align_items(Alignment::Center),
            ),
            None,
            0,
        );
        for (i, &(label, tip)) in [
            ("Apply", "Apply pending edits to the document"),
            ("Reset", "Discard pending edits"),
        ]
        .iter()
        .enumerate()
        {
            let b_tip = cx
                .tree
                .add_child(ws_subtoolbar, Box::new(Tip::new(tip)), None, i as i16);
            let b = cx.tree.add_child(
                b_tip,
                Box::new(Button::new(NodeId(0)).with_padding(6.0)),
                None,
                0,
            );
            let lbl = cx.tree.add_child(b, Box::new(Label::new(label)), None, 0);
            halcyon::replace_widget!(
                cx.tree,
                b,
                Button::new(lbl).with_padding(6.0).on_click(|| {})
            );
        }
        // Document mode dropdown
        let mut mode_combo = ComboBox::simple(vec![
            "Markdown".into(),
            "Plain Text".into(),
            "Rust".into(),
            "JSON".into(),
            "TOML".into(),
        ]);
        mode_combo.set_input_text("Markdown".into());
        let mode_tip = cx.tree.add_child(
            ws_subtoolbar,
            Box::new(Tip::new("Document syntax mode")),
            None,
            2,
        );
        cx.tree.set_flex(mode_tip, FlexParams::fixed(150.0));
        cx.tree.add_child(mode_tip, Box::new(mode_combo), None, 0);

        // Editor toggles
        Self::add_tipped(
            cx,
            ws_subtoolbar,
            3,
            "Wrap long lines to the pane width",
            Box::new(Label::new("Wrap").with_size(11.0)),
        );
        Self::add_tipped(
            cx,
            ws_subtoolbar,
            4,
            "Wrap long lines to the pane width",
            Box::new(Switch::on(true)),
        );
        Self::add_tipped(
            cx,
            ws_subtoolbar,
            5,
            "Open the document without allowing edits",
            Box::new(Label::new("Read-only").with_size(11.0)),
        );
        Self::add_tipped(
            cx,
            ws_subtoolbar,
            6,
            "Open the document without allowing edits",
            Box::new(Switch::on(false)),
        );

        cx.tree
            .add_child(ws_col, Box::new(Divider::horizontal()), None, 1);

        // Workspace document — a real TextArea editor filling the pane. TextArea
        // is self-scrolling (owns its scrollbars), so it mounts directly with no
        // enclosing ScrollArea.
        let doc = "# Halcyon Showcase\n\
            \n\
            A GPU-first, cross-platform desktop UI framework in Rust.\n\
            \n\
            ## Layout\n\
            \n\
            This pane is a live TextArea. Type into it — the word and character\n\
            counts in the status strip below update every frame.\n\
            \n\
            The left Navigator is a real TreeView over this repository, with a\n\
            List beneath it. The right Inspector is a PropertyEditor. Every\n\
            divider between the panes is a draggable SplitPane.\n\
            \n\
            ## Editing\n\
            \n\
            - Select with drag, or Shift+Arrow\n\
            - Undo / redo with Cmd/Ctrl+Z and Cmd/Ctrl+Shift+Z\n\
            - Scrolling engages once content exceeds the viewport\n";

        let ws_editor = cx.tree.add_child(
            ws_col,
            Box::new(
                TextArea::new()
                    .with_text(doc)
                    .with_placeholder("Type here…"),
            ),
            None,
            2,
        );
        cx.tree.set_flex(ws_editor, FlexParams::flexible(1.0));
        self.ws_textarea_id = Some(ws_editor);

        // Divider above the editor status strip.
        cx.tree
            .add_child(ws_col, Box::new(Divider::horizontal()), None, 3);

        // Editor status strip — filename (left); live word/char counts and
        // encoding (right). Counts are updated each frame in ShowcaseScreen::update.
        let ws_status = cx.tree.add_child(
            ws_col,
            Box::new(
                Flex::row()
                    .with_spacing(12.0)
                    .with_padding(EdgeInsets::all(6.0)),
            ),
            None,
            4,
        );
        cx.tree.add_child(
            ws_status,
            Box::new(Label::new("untitled.md").with_size(11.0)),
            None,
            0,
        );
        let ws_status_spacer = cx
            .tree
            .add_child(ws_status, Box::new(FlexSpacer::new()), None, 1);
        cx.tree
            .set_flex(ws_status_spacer, FlexParams::flexible(1.0));
        let words_id = cx.tree.add_child(
            ws_status,
            Box::new(Label::new("Words: 0").with_size(11.0)),
            None,
            2,
        );
        self.ws_words_label_id = Some(words_id);
        cx.tree
            .add_child(ws_status, Box::new(Divider::vertical()), None, 3);
        let chars_id = cx.tree.add_child(
            ws_status,
            Box::new(Label::new("Chars: 0").with_size(11.0)),
            None,
            4,
        );
        self.ws_chars_label_id = Some(chars_id);
        cx.tree
            .add_child(ws_status, Box::new(Divider::vertical()), None, 5);
        cx.tree.add_child(
            ws_status,
            Box::new(Label::new("UTF-8 · LF").with_size(11.0)),
            None,
            6,
        );

        // ── RIGHT: Inspector ──────────────────────────────────────
        // Inspector is a real PropertyEditor v2 (SPEC-HAL-260A). Three
        // Section rows (Properties / Appearance / Behavior) with three
        // editable String rows each. PE is mounted directly under
        // right_split (matching TreeView's mount under nav_split at line
        // 375 of this file). The canonical PE mount per
        // examples/property_editor_v2.rs:669-685 is
        //   PropertyEditor → ScrollArea → body
        // with PE as the direct child of a parent that supplies a
        // bounded max_height. SplitPane supplies that bounded constraint
        // by slot. Wrapping PE in an outer ScrollArea (B1F's prior
        // shape) gave PE an unbounded max_height; PE::layout then
        // computed body_h against ∞ and the body never received a
        // paintable allocation — Inspector painted only the column
        // header strip, body was empty.
        let mut editor = PropertyEditor::new();
        editor.add_name_column("Property");
        let value_col_id = editor.add_value_column("Value");

        let source = Rc::new(RefCell::new(InspectorSource::new(value_col_id)));
        editor.set_data_source(source);

        let pe_body = editor.create_body_widget();

        let pe_id = cx.tree.add_child(right_split, Box::new(editor), None, 1);

        let pe_scroll = ScrollArea::new_unwired();
        let pe_scroll_id = cx.tree.add_child(pe_id, Box::new(pe_scroll), None, 0);

        let pe_body_id = cx.tree.add_child(pe_scroll_id, Box::new(pe_body), None, 0);

        if let Some(node) = cx.tree.node_mut(pe_scroll_id) {
            if let Some(sa) = node.widget.as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(pe_body_id);
            }
        }

        // Close the right horizontal split (Workspace | Inspector).
        halcyon::replace_widget!(
            cx.tree,
            right_split,
            SplitPane::horizontal(ws_col, pe_id)
                .initial_ratio(0.70)
                .leading_min(240.0)
                .trailing_min(200.0)
        );

        // Close the outer main split (Navigator | right_split).
        halcyon::replace_widget!(
            cx.tree,
            main_split,
            SplitPane::horizontal(nav_split, right_split)
                .initial_ratio(0.22)
                .leading_min(180.0)
                .trailing_min(360.0)
        );

        // ═══════════════════════════════════════════════════════════
        // Divider above status bar + STATUS BAR (slots 3-4)
        // ═══════════════════════════════════════════════════════════
        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 3);

        let status_bar = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(12.0)
                    .with_padding(EdgeInsets::all(6.0)),
            ),
            None,
            4,
        );
        cx.tree.add_child(
            status_bar,
            Box::new(Label::new("Ready").with_size(11.0)),
            None,
            0,
        );
        let sb_spacer = cx
            .tree
            .add_child(status_bar, Box::new(FlexSpacer::new()), None, 1);
        cx.tree.set_flex(sb_spacer, FlexParams::flexible(1.0));
        cx.tree.add_child(
            status_bar,
            Box::new(Label::new("Items: 30").with_size(11.0)),
            None,
            2,
        );
        cx.tree
            .add_child(status_bar, Box::new(Divider::vertical()), None, 3);
        cx.tree.add_child(
            status_bar,
            Box::new(Label::new("Mode: A").with_size(11.0)),
            None,
            4,
        );
        cx.tree
            .add_child(status_bar, Box::new(Divider::vertical()), None, 5);
        cx.tree.add_child(
            status_bar,
            Box::new(Label::new("v1.0").with_size(11.0)),
            None,
            6,
        );
    }
}
