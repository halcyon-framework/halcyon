// Showcase — Workbench tab: dense batch-processing / configuration console.
//
// SCAFFOLD SHELL ONLY. Establishes the region skeleton:
//   profile strip  ->  SplitPane(Navigator | Workspace)  ->  console status strip
// Region interiors (source tree, presets, pipeline table, transform-panel
// wall, inspector) are filled by subsequent tasks. Placeholders mark each region.

use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::math::RectF;
use halcyon::core::paint::Color;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::text_engine::GlyphRun;
use halcyon::core::widget::PaintContext;
use halcyon::core::widget_tree::NodeId;
use halcyon::engines::hierarchical::NodeMeta;
use halcyon::prelude::*;
use halcyon::widgets::divider::Divider;
use halcyon::widgets::groupbox::GroupBox;
use halcyon::widgets::list_view::{CellPainter, DefaultCellPainter, ListView};
use halcyon::widgets::property_editor::{
    Cell as PeCell, CellSource, PropertyDataSource, PropertyEditor, PropertyNode, PropertyNodeKind,
    PropertyValue, RowArchetype,
};
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::split_pane::{
    SplitPane, SplitPaneDividerVisibility, SplitPaneInteractionPolicy,
};
use halcyon::widgets::tabular_view::Column;
use halcyon::widgets::tabular_view::column::ColumnId;
use halcyon::widgets::tabular_view::column::ColumnModel;
use halcyon::widgets::tabular_view::data_adapter::{
    CellCapabilities, CellValue, DataInvalidationEvent, DataSource, Extent, UniverseMode,
    ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use halcyon::widgets::tree_view::TreeView;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::rc::Rc;

use super::*;

/// Mock asset / data-pipeline source hierarchy for the Workbench Navigator.
/// Flat pre-order `Vec<NodeMeta>` (node_id == index) — the same shape TreeView
/// consumes in the Layout tab. Snapshot only; no live filesystem source.
fn workbench_source_tree() -> Vec<NodeMeta> {
    let n =
        |node_id: u64, parent_index: Option<usize>, depth: u32, has_children: bool, label: &str| {
            NodeMeta {
                node_id,
                parent_index,
                depth,
                has_children,
                label: label.to_string(),
            }
        };
    vec![
        n(0, None, 0, true, "project"),
        n(1, Some(0), 1, true, "inbox"),
        n(2, Some(1), 2, true, "batch_2026-06-03"),
        n(3, Some(2), 3, false, "frame_0001.exr"),
        n(4, Some(2), 3, false, "frame_0002.exr"),
        n(5, Some(2), 3, false, "frame_0003.exr"),
        n(6, Some(1), 2, true, "batch_2026-06-02"),
        n(7, Some(6), 3, false, "scan_a.tiff"),
        n(8, Some(6), 3, false, "scan_b.tiff"),
        n(9, Some(0), 1, true, "staging"),
        n(10, Some(9), 2, false, "normalized"),
        n(11, Some(9), 2, false, "proxies"),
        n(12, Some(0), 1, true, "output"),
        n(13, Some(12), 2, false, "delivery"),
        n(14, Some(12), 2, false, "archive"),
    ]
}

/// Saved transform-chain presets for the Workbench Navigator. Single-column,
/// in-memory, snapshot DataSource (SPEC-HAL-188) — the same shape the List View
/// tab uses, narrowed to one "Preset" column.
struct PresetSource {
    rows: Vec<RowId>,
    index_by_id: HashMap<RowId, usize>,
    names: Vec<&'static str>,
    name_col: Option<ColumnId>,
    pending_events: Vec<DataInvalidationEvent>,
}

impl PresetSource {
    fn new() -> Self {
        let names = vec![
            "Resize — 2048px longest edge",
            "Strip EXIF + GPS",
            "Transcode — H.265 / CRF 18",
            "Color — Rec.709 to sRGB",
            "Denoise — NLM (strong)",
            "Sharpen — unsharp 0.8",
            "Watermark — bottom-right",
            "Proxy — quarter-res ProRes",
            "Rename — sequence ####",
            "Archive — tar + zstd-19",
        ];
        let rows: Vec<RowId> = (0..names.len()).map(|_| RowId::new()).collect();
        let index_by_id: HashMap<RowId, usize> =
            rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
        Self {
            rows,
            index_by_id,
            names,
            name_col: None,
            pending_events: Vec::new(),
        }
    }

    fn set_name_col(&mut self, col: ColumnId) {
        self.name_col = Some(col);
    }
}

impl DataSource for PresetSource {
    fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Snapshot
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some(idx) = self.index_by_id.get(&row).copied() else {
            return ValueState::Missing;
        };
        if Some(column) == self.name_col {
            ValueState::Ready(CellValue::String(self.names[idx].to_string()))
        } else {
            ValueState::Missing
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// One row of the Workbench pipeline table.
struct PipelineRow {
    source: &'static str,
    stage: &'static str,
    progress: f64,
    status: &'static str,
    size: &'static str,
    eta: &'static str,
}

/// Mock asset-pipeline job table. Six columns, in-memory, snapshot
/// DataSource (SPEC-HAL-188). Progress is a fraction in [0,1]; the painter
/// renders it as a bar. Status is one of Running/Done/Queued/Failed/Paused;
/// the painter renders it as a colored pill.
struct PipelineSource {
    rows: Vec<RowId>,
    index_by_id: HashMap<RowId, usize>,
    data: Vec<PipelineRow>,
    col_source: Option<ColumnId>,
    col_stage: Option<ColumnId>,
    col_progress: Option<ColumnId>,
    col_status: Option<ColumnId>,
    col_size: Option<ColumnId>,
    col_eta: Option<ColumnId>,
    pending_events: Vec<DataInvalidationEvent>,
}

impl PipelineSource {
    fn new() -> Self {
        let data = vec![
            PipelineRow {
                source: "plate_A_0001.exr",
                stage: "Color",
                progress: 1.00,
                status: "Done",
                size: "61.4 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "plate_A_0002.exr",
                stage: "Color",
                progress: 1.00,
                status: "Done",
                size: "60.9 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "plate_A_0003.exr",
                stage: "Transcode",
                progress: 0.82,
                status: "Running",
                size: "58.2 MB",
                eta: "00:07",
            },
            PipelineRow {
                source: "plate_A_0004.exr",
                stage: "Transcode",
                progress: 0.64,
                status: "Running",
                size: "59.1 MB",
                eta: "00:11",
            },
            PipelineRow {
                source: "plate_A_0005.exr",
                stage: "Denoise",
                progress: 0.41,
                status: "Running",
                size: "62.0 MB",
                eta: "00:18",
            },
            PipelineRow {
                source: "plate_A_0006.exr",
                stage: "Denoise",
                progress: 0.00,
                status: "Queued",
                size: "61.7 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "scan_014.tiff",
                stage: "Normalize",
                progress: 0.00,
                status: "Queued",
                size: "12.8 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "scan_015.tiff",
                stage: "Normalize",
                progress: 0.00,
                status: "Queued",
                size: "12.6 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "scan_016.tiff",
                stage: "Ingest",
                progress: 0.97,
                status: "Running",
                size: "12.9 MB",
                eta: "00:01",
            },
            PipelineRow {
                source: "frame_0001.exr",
                stage: "Color",
                progress: 1.00,
                status: "Done",
                size: "47.3 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "frame_0002.exr",
                stage: "Color",
                progress: 1.00,
                status: "Done",
                size: "47.1 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "frame_0003.exr",
                stage: "Proxy",
                progress: 0.55,
                status: "Running",
                size: "47.6 MB",
                eta: "00:09",
            },
            PipelineRow {
                source: "frame_0004.exr",
                stage: "Proxy",
                progress: 0.00,
                status: "Paused",
                size: "47.4 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "frame_0005.exr",
                stage: "Watermark",
                progress: 0.00,
                status: "Queued",
                size: "47.8 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "take_22_master.mov",
                stage: "Transcode",
                progress: 0.33,
                status: "Running",
                size: "1.21 GB",
                eta: "02:14",
            },
            PipelineRow {
                source: "take_23_master.mov",
                stage: "Transcode",
                progress: 0.00,
                status: "Queued",
                size: "1.18 GB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "take_19_master.mov",
                stage: "Transcode",
                progress: 0.00,
                status: "Failed",
                size: "1.24 GB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "hdr_pano_01.exr",
                stage: "Normalize",
                progress: 0.00,
                status: "Failed",
                size: "184 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "hdr_pano_02.exr",
                stage: "Normalize",
                progress: 0.71,
                status: "Running",
                size: "181 MB",
                eta: "00:05",
            },
            PipelineRow {
                source: "delivery_v3.mp4",
                stage: "Archive",
                progress: 0.88,
                status: "Running",
                size: "742 MB",
                eta: "00:04",
            },
            PipelineRow {
                source: "delivery_v2.mp4",
                stage: "Archive",
                progress: 1.00,
                status: "Done",
                size: "731 MB",
                eta: "\u{2014}",
            },
            PipelineRow {
                source: "proxy_set_0007.mov",
                stage: "Proxy",
                progress: 0.00,
                status: "Queued",
                size: "96.0 MB",
                eta: "\u{2014}",
            },
        ];
        let rows: Vec<RowId> = (0..data.len()).map(|_| RowId::new()).collect();
        let index_by_id: HashMap<RowId, usize> =
            rows.iter().enumerate().map(|(i, r)| (*r, i)).collect();
        Self {
            rows,
            index_by_id,
            data,
            col_source: None,
            col_stage: None,
            col_progress: None,
            col_status: None,
            col_size: None,
            col_eta: None,
            pending_events: Vec::new(),
        }
    }

    fn set_columns(
        &mut self,
        source: ColumnId,
        stage: ColumnId,
        progress: ColumnId,
        status: ColumnId,
        size: ColumnId,
        eta: ColumnId,
    ) {
        self.col_source = Some(source);
        self.col_stage = Some(stage);
        self.col_progress = Some(progress);
        self.col_status = Some(status);
        self.col_size = Some(size);
        self.col_eta = Some(eta);
    }

    /// Aggregate status counts for the toolbar summary, computed from the
    /// in-memory rows so the line can never drift from the table contents.
    fn summary_line(&self) -> String {
        let total = self.data.len();
        let mut running = 0usize;
        let mut queued = 0usize;
        let mut failed = 0usize;
        for r in &self.data {
            match r.status {
                "Running" => running += 1,
                "Queued" => queued += 1,
                "Failed" => failed += 1,
                _ => {}
            }
        }
        format!(
            "{} jobs \u{00B7} {} running \u{00B7} {} queued \u{00B7} {} failed",
            total, running, queued, failed
        )
    }
}

impl DataSource for PipelineSource {
    fn row_ids(&self) -> &[RowId] {
        &self.rows
    }

    fn extent(&self) -> Extent {
        Extent::Exact(self.rows.len())
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Snapshot
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some(idx) = self.index_by_id.get(&row).copied() else {
            return ValueState::Missing;
        };
        let r = &self.data[idx];
        if Some(column) == self.col_source {
            ValueState::Ready(CellValue::String(r.source.to_string()))
        } else if Some(column) == self.col_stage {
            ValueState::Ready(CellValue::String(r.stage.to_string()))
        } else if Some(column) == self.col_progress {
            ValueState::Ready(CellValue::Float(r.progress))
        } else if Some(column) == self.col_status {
            ValueState::Ready(CellValue::String(r.status.to_string()))
        } else if Some(column) == self.col_size {
            ValueState::Ready(CellValue::String(r.size.to_string()))
        } else if Some(column) == self.col_eta {
            ValueState::Ready(CellValue::String(r.eta.to_string()))
        } else {
            ValueState::Missing
        }
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.pending_events)
    }
}

/// Pipeline-table cell painter. Progress column -> inline bar + percent label.
/// Status column -> tinted rounded pill with a same-hue label. Every other
/// column falls through to `DefaultCellPainter` (text). All colors are
/// theme-resolved (semantic / accents / surfaces).
struct WorkbenchPipelinePainter {
    progress_col: ColumnId,
    status_col: ColumnId,
}

impl CellPainter for WorkbenchPipelinePainter {
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
        // Progress — inline bar + right-aligned percent.
        if column_id == self.progress_col {
            let frac = match &value {
                ValueState::Ready(CellValue::Float(f)) => (*f as f32).clamp(0.0, 1.0),
                ValueState::Ready(CellValue::Int(n)) => (*n as f32 / 100.0).clamp(0.0, 1.0),
                _ => 0.0,
            };
            let label = format!("{}%", (frac * 100.0).round() as i32);
            let run = cx.text_engine.shape(&label, font_size);
            let label_gap = 8.0;
            let bar_h = 6.0;
            let bar_x = rect.x + text_padding;
            let bar_y = rect.y + (rect.height - bar_h) / 2.0;
            let bar_w = (rect.width - text_padding * 2.0 - run.width - label_gap).max(0.0);
            cx.scene.push_rect(
                RectF {
                    x: bar_x,
                    y: bar_y,
                    width: bar_w,
                    height: bar_h,
                },
                cx.theme.colors.surfaces.level2,
                3,
                None,
            );
            if frac > 0.0 && bar_w > 0.0 {
                cx.scene.push_rect(
                    RectF {
                        x: bar_x,
                        y: bar_y,
                        width: bar_w * frac,
                        height: bar_h,
                    },
                    cx.theme.colors.accents.primary,
                    4,
                    None,
                );
            }
            let text_color = if selected {
                cx.theme.colors.list.text_selected
            } else {
                cx.theme.colors.list.text
            };
            let tx = rect.x + rect.width - text_padding - run.width;
            let ty = rect.y + (rect.height - run.height) / 2.0;
            cx.scene
                .push_text_with_backgrounds(&run, tx, ty, text_color, Vec::new(), 5, None);
            return;
        }

        // Status — colored pill (tinted background + same-hue label).
        if column_id == self.status_col {
            let label = match &value {
                ValueState::Ready(CellValue::String(s)) => s.clone(),
                _ => String::new(),
            };
            let hue = match label.as_str() {
                "Running" => cx.theme.colors.semantic.info,
                "Done" => cx.theme.colors.semantic.success,
                "Failed" => cx.theme.colors.semantic.error,
                "Queued" => cx.theme.colors.semantic.warning,
                "Paused" => cx.theme.colors.typography.secondary,
                _ => cx.theme.colors.typography.muted,
            };
            let run = cx.text_engine.shape(&label, font_size);
            let pill_h = (run.height + 6.0).min((rect.height - 4.0).max(0.0));
            let pill_w = run.width + 16.0;
            let pill_x = rect.x + text_padding;
            let pill_y = rect.y + (rect.height - pill_h) / 2.0;
            cx.scene.push_rect(
                RectF {
                    x: pill_x,
                    y: pill_y,
                    width: pill_w,
                    height: pill_h,
                },
                hue.with_alpha(0.18),
                3,
                None,
            );
            let tx = pill_x + 8.0;
            let ty = rect.y + (rect.height - run.height) / 2.0;
            cx.scene
                .push_text_with_backgrounds(&run, tx, ty, hue, Vec::new(), 4, None);
            return;
        }

        // All other columns — default text rendering.
        DefaultCellPainter.paint_cell(
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

/// In-memory PropertyDataSource backing the Workbench Inspector. Four Section
/// rows (Source / Pipeline / Image / Output) with typed `PropertyValue` data
/// rows. Each variant mounts the correct PE v2 editor (SPEC-HAL-260A §6.2):
/// String → TextField, Integer/Float → SpinBox, Bool → Checkbox, Choice →
/// ComboBox, Color → ColorPickerField.
struct WorkbenchInspectorSource {
    nodes: Vec<PropertyNode>,
    cells: HashMap<(u64, ColumnId), PeCell>,
    value_col_id: ColumnId,
    revision: u64,
    next_id: u64,
}

impl WorkbenchInspectorSource {
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
        let source = self.add_section("Source");
        self.add_property(
            "Name",
            source,
            PropertyValue::String("frame_0003.exr".into()),
        );
        self.add_property("Size", source, PropertyValue::String("47.6 MB".into()));
        self.add_property("Frames", source, PropertyValue::Integer(240));

        let pipeline = self.add_section("Pipeline");
        self.add_property(
            "Stage",
            pipeline,
            PropertyValue::Choice {
                selected: 4,
                options: vec![
                    "Ingest".into(),
                    "Normalize".into(),
                    "Denoise".into(),
                    "Color".into(),
                    "Proxy".into(),
                    "Transcode".into(),
                    "Watermark".into(),
                    "Archive".into(),
                ],
            },
        );
        self.add_property(
            "Status",
            pipeline,
            PropertyValue::Choice {
                selected: 1,
                options: vec![
                    "Queued".into(),
                    "Running".into(),
                    "Done".into(),
                    "Failed".into(),
                    "Paused".into(),
                ],
            },
        );
        self.add_property("Progress", pipeline, PropertyValue::Float(0.55));

        let image = self.add_section("Image");
        self.add_property(
            "Color space",
            image,
            PropertyValue::Choice {
                selected: 0,
                options: vec![
                    "Rec.709".into(),
                    "sRGB".into(),
                    "Linear".into(),
                    "ACEScg".into(),
                ],
            },
        );
        self.add_property(
            "Dimensions",
            image,
            PropertyValue::String("4096x2160".into()),
        );
        self.add_property(
            "Tint",
            image,
            PropertyValue::Color(Color::from_hex(0x4888C8)),
        );

        let output = self.add_section("Output");
        self.add_property(
            "Codec",
            output,
            PropertyValue::Choice {
                selected: 2,
                options: vec![
                    "H.265".into(),
                    "H.264".into(),
                    "ProRes".into(),
                    "PNG".into(),
                    "EXR".into(),
                ],
            },
        );
        self.add_property("CRF", output, PropertyValue::Integer(18));
        self.add_property("Faststart", output, PropertyValue::Bool(true));
    }
}

impl CellSource for WorkbenchInspectorSource {
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

impl PropertyDataSource for WorkbenchInspectorSource {
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
    pub(super) fn build_workbench(&mut self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let divider_policy = SplitPaneInteractionPolicy {
            divider_visibility: SplitPaneDividerVisibility::Always,
        };

        // Outer column fills the tab content area. Fixed layout (no outer
        // ScrollArea) so the SplitPane gets a bounded height from above.
        let outer = cx
            .tree
            .add_child(parent, Box::new(Flex::column().with_spacing(0.0)), None, 0);
        cx.tree.set_flex(outer, FlexParams::flexible(1.0));

        // -- Profile strip (slot 0) — natural height ------------------
        let profile_bar = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(8.0)),
            ),
            None,
            0,
        );
        let profile_spacer = cx
            .tree
            .add_child(profile_bar, Box::new(FlexSpacer::new()), None, 0);
        cx.tree.set_flex(profile_spacer, FlexParams::flexible(1.0));
        cx.tree.add_child(
            profile_bar,
            Box::new(Label::new("Profile: Default").with_size(11.0)),
            None,
            1,
        );

        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 1);

        // -- Main area (slot 2) — SplitPane: Navigator | Workspace --
        let main_split = cx.tree.add_child(
            outer,
            Box::new(
                SplitPane::horizontal(NodeId(0), NodeId(0))
                    .initial_ratio(0.20)
                    .leading_min(180.0)
                    .trailing_min(360.0),
            ),
            None,
            2,
        );
        cx.tree.set_flex(main_split, FlexParams::flexible(1.0));

        // ── LEFT: Navigator — SplitPane vertical (source tree / presets) ──
        let nav_split = cx.tree.add_child(
            main_split,
            Box::new(
                SplitPane::vertical(NodeId(0), NodeId(0))
                    .initial_ratio(0.6)
                    .leading_min(120.0)
                    .trailing_min(80.0),
            ),
            None,
            0,
        );

        // Navigator top — real TreeView over the mock source hierarchy.
        // Capture body + footer + scroll-link BEFORE moving TreeView into a
        // Box. TreeView mounts directly under nav_split — a single-child Panel
        // wrap breaks the SPEC-HAL-154
        // seal path.
        let mut tree_view = TreeView::new();
        tree_view.set_tree_data(workbench_source_tree());

        let mut col_model = ColumnModel::new();
        let tree_col = Column::new(180.0).with_flex(1.0).with_min_width(180.0);
        let tree_col_id = tree_col.id;
        col_model.push(tree_col);
        tree_view.set_columns(col_model);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Source");
        tree_view.set_footer_height(0.0);

        let hscroll_link = tree_view.horizontal_scroll_link();
        let hscroll_freeze = tree_view.horizontal_scroll_freeze();
        let tree_body = tree_view.create_body_widget();
        let tree_footer = tree_view.create_footer_widget();

        let tv_id = cx.tree.add_child(nav_split, Box::new(tree_view), None, 0);

        let tv_scroll = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze)
            .with_content_owned_vertical_translation();
        let tv_scroll_id = cx.tree.add_child(tv_id, Box::new(tv_scroll), None, 0);

        let tv_body_id = cx
            .tree
            .add_child(tv_scroll_id, Box::new(tree_body), None, 0);

        if let Some(node) = cx.tree.node_mut(tv_scroll_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(tv_body_id);
            }
        }

        cx.tree.add_child(tv_id, Box::new(tree_footer), None, 1);

        // Navigator bottom — real ListView of saved presets (single "Preset"
        // column, virtualized, selectable). Same mount pattern as the List View
        // tab: ListView -> ScrollArea(body) -> footer, content wired explicitly.
        let mut presets = ListView::new(PresetSource::new());
        let preset_col = presets.add_column_raw(
            "Preset",
            Column::new(180.0).with_flex(1.0).with_min_width(160.0),
        );
        presets.adapter_mut().source_mut().set_name_col(preset_col);
        presets.set_footer_height(0.0);

        let preset_hscroll_link = HorizontalScrollLink::new();
        let preset_hscroll_freeze = Rc::new(Cell::new(false));
        presets
            .set_horizontal_scroll_link(preset_hscroll_link.clone(), preset_hscroll_freeze.clone());

        let preset_body = presets.create_body_widget();
        let preset_footer = presets.create_footer_widget();

        let presets_id = cx.tree.add_child(nav_split, Box::new(presets), None, 1);

        let preset_scroll = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(preset_hscroll_link)
            .with_horizontal_freeze(preset_hscroll_freeze);
        let preset_scroll_id = cx
            .tree
            .add_child(presets_id, Box::new(preset_scroll), None, 0);

        let preset_body_id = cx
            .tree
            .add_child(preset_scroll_id, Box::new(preset_body), None, 0);

        if let Some(node) = cx.tree.node_mut(preset_scroll_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(preset_body_id);
            }
        }

        cx.tree
            .add_child(presets_id, Box::new(preset_footer), None, 1);

        // Close the vertical Navigator split now that both panes exist.
        halcyon::replace_widget!(
            cx.tree,
            nav_split,
            SplitPane::vertical(tv_id, presets_id)
                .with_interaction_policy(divider_policy)
                .initial_ratio(0.6)
                .leading_min(120.0)
                .trailing_min(80.0)
        );

        // ── RIGHT: Workspace — SplitPane horizontal (center | inspector) ──
        let work_region = cx.tree.add_child(
            main_split,
            Box::new(
                SplitPane::horizontal(NodeId(0), NodeId(0))
                    .initial_ratio(0.74)
                    .leading_min(420.0)
                    .trailing_min(220.0),
            ),
            None,
            1,
        );

        // Center — SplitPane vertical (pipeline table / transform-panel wall).
        let center_split = cx.tree.add_child(
            work_region,
            Box::new(
                SplitPane::vertical(NodeId(0), NodeId(0))
                    .initial_ratio(0.62)
                    .leading_min(160.0)
                    .trailing_min(120.0),
            ),
            None,
            0,
        );

        // Center top — DENSE pipeline table. Six columns; Progress renders an
        // inline bar, Status a colored pill (WorkbenchPipelinePainter). Same
        // mount pattern as the presets list: ListView -> ScrollArea(body) ->
        // footer, content wired explicitly.
        let pipe_source = PipelineSource::new();
        let pipe_summary = pipe_source.summary_line();
        let mut pipeline = ListView::new(pipe_source);
        let col_source = pipeline.add_column_raw(
            "Source",
            Column::new(240.0).with_flex(1.0).with_min_width(200.0),
        );
        let col_stage = pipeline.add_column_raw("Stage", Column::new(150.0).with_min_width(120.0));
        let col_progress =
            pipeline.add_column_raw("Progress", Column::new(160.0).with_min_width(140.0));
        let col_status =
            pipeline.add_column_raw("Status", Column::new(120.0).with_min_width(110.0));
        let col_size = pipeline.add_column_raw("Size", Column::new(96.0).with_min_width(80.0));
        let col_eta = pipeline.add_column_raw("ETA", Column::new(80.0).with_min_width(72.0));
        pipeline.adapter_mut().source_mut().set_columns(
            col_source,
            col_stage,
            col_progress,
            col_status,
            col_size,
            col_eta,
        );
        pipeline.set_cell_painter(Box::new(WorkbenchPipelinePainter {
            progress_col: col_progress,
            status_col: col_status,
        }));
        pipeline.set_footer_height(0.0);

        let pipe_hscroll_link = HorizontalScrollLink::new();
        let pipe_hscroll_freeze = Rc::new(Cell::new(false));
        pipeline.set_horizontal_scroll_link(pipe_hscroll_link.clone(), pipe_hscroll_freeze.clone());

        let pipe_body = pipeline.create_body_widget();
        let pipe_footer = pipeline.create_footer_widget();

        let pipe_pane = cx.tree.add_child(
            center_split,
            Box::new(Flex::column().with_spacing(0.0)),
            None,
            0,
        );
        // Filter + summary toolbar over the table, so the pane reads as an
        // operable job console rather than a bare grid. The toolbar is natural
        // height; a divider separates it; the table takes the remaining space.
        let pipe_toolbar = cx.tree.add_child(
            pipe_pane,
            Box::new(
                Flex::row()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(6.0)),
            ),
            None,
            0,
        );
        let filter_tip = cx.tree.add_child(
            pipe_toolbar,
            Box::new(super::tip::Tip::new(
                "Filter the job list by status: all, active, queued, or failed",
            )),
            None,
            0,
        );
        cx.tree.add_child(
            filter_tip,
            Box::new(halcyon::widgets::segmented_control::SegmentedControl::new(
                vec![
                    halcyon::widgets::segmented_control::Segment::new("All"),
                    halcyon::widgets::segmented_control::Segment::new("Active"),
                    halcyon::widgets::segmented_control::Segment::new("Queued"),
                    halcyon::widgets::segmented_control::Segment::new("Failed"),
                ],
                0,
            )),
            None,
            0,
        );
        let pipe_toolbar_spacer =
            cx.tree
                .add_child(pipe_toolbar, Box::new(FlexSpacer::new()), None, 1);
        cx.tree
            .set_flex(pipe_toolbar_spacer, FlexParams::flexible(1.0));
        cx.tree.add_child(
            pipe_toolbar,
            Box::new(Label::new(pipe_summary).with_size(11.0)),
            None,
            2,
        );
        cx.tree
            .add_child(pipe_pane, Box::new(Divider::horizontal()), None, 1);
        let pipe_id = cx.tree.add_child(pipe_pane, Box::new(pipeline), None, 2);
        cx.tree.set_flex(pipe_id, FlexParams::flexible(1.0));

        let pipe_scroll = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(pipe_hscroll_link)
            .with_horizontal_freeze(pipe_hscroll_freeze);
        let pipe_scroll_id = cx.tree.add_child(pipe_id, Box::new(pipe_scroll), None, 0);

        let pipe_body_id = cx
            .tree
            .add_child(pipe_scroll_id, Box::new(pipe_body), None, 0);

        if let Some(node) = cx.tree.node_mut(pipe_scroll_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(pipe_body_id);
            }
        }

        cx.tree.add_child(pipe_id, Box::new(pipe_footer), None, 1);

        // Center bottom — transform-panel wall: a dense grid of image-pipeline
        // stage panels. Vertical ScrollArea over a column of control rows; each
        // row holds three equal-width GroupBoxes (proven Inputs-tab pattern).
        let panel_wall =
            cx.tree
                .add_child(center_split, Box::new(ScrollArea::new(NodeId(0))), None, 1);
        let wall_col = cx.tree.add_child(
            panel_wall,
            Box::new(
                Flex::column()
                    .with_spacing(8.0)
                    .with_padding(EdgeInsets::all(8.0)),
            ),
            None,
            0,
        );
        halcyon::replace_widget!(cx.tree, panel_wall, ScrollArea::new(wall_col));

        // ── Row 1 ──
        let wall_row1 =
            cx.tree
                .add_child(wall_col, Box::new(Flex::row().with_spacing(8.0)), None, 0);

        // GroupBox: Resize
        let resize_box = cx.tree.add_child(
            wall_row1,
            Box::new(GroupBox::new(NodeId(0), "Resize")),
            None,
            0,
        );
        cx.tree.set_flex(resize_box, FlexParams::flexible(1.0));
        let resize_col = cx.tree.add_child(
            resize_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            resize_box,
            GroupBox::new(resize_col, "Resize")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        let resize_fit = cx.tree.add_child(
            resize_col,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            0,
        );
        cx.tree.add_child(
            resize_fit,
            Box::new(Label::new("Fit mode").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            resize_fit,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "Longest edge".into(),
                "Width".into(),
                "Height".into(),
                "Exact".into(),
            ])),
            None,
            1,
        );
        cx.tree.add_child(
            resize_col,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_label("Target px")
                    .with_value(2048)
                    .with_min(16)
                    .with_max(16384)
                    .with_step(1),
            ),
            None,
            1,
        );
        cx.tree.add_child(
            resize_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 200.0)
                    .with_label("Scale %")
                    .with_value(100.0),
            ),
            None,
            2,
        );
        let upscale_tip = cx.tree.add_child(
            resize_col,
            Box::new(super::tip::Tip::new(
                "Permit enlarging images past their source resolution",
            )),
            None,
            3,
        );
        cx.tree.add_child(
            upscale_tip,
            Box::new(halcyon::widgets::checkbox::Checkbox::new().with_label("Allow upscaling")),
            None,
            0,
        );
        let aspect_tip = cx.tree.add_child(
            resize_col,
            Box::new(super::tip::Tip::new(
                "Keep the original width-to-height ratio when resizing",
            )),
            None,
            4,
        );
        cx.tree.add_child(
            aspect_tip,
            Box::new(
                halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Preserve aspect"),
            ),
            None,
            0,
        );

        // GroupBox: Color
        let color_box = cx.tree.add_child(
            wall_row1,
            Box::new(GroupBox::new(NodeId(0), "Color")),
            None,
            1,
        );
        cx.tree.set_flex(color_box, FlexParams::flexible(1.0));
        let color_col = cx.tree.add_child(
            color_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            color_box,
            GroupBox::new(color_col, "Color")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        let color_in = cx.tree.add_child(
            color_col,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            0,
        );
        cx.tree.add_child(
            color_in,
            Box::new(Label::new("Input space").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            color_in,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "Rec.709".into(),
                "sRGB".into(),
                "Linear".into(),
                "ACEScg".into(),
            ])),
            None,
            1,
        );
        let color_out = cx.tree.add_child(
            color_col,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            1,
        );
        cx.tree.add_child(
            color_out,
            Box::new(Label::new("Output space").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            color_out,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "sRGB".into(),
                "Rec.709".into(),
                "Display P3".into(),
            ])),
            None,
            1,
        );
        cx.tree.add_child(
            color_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(-2.0, 2.0)
                    .with_label("Exposure")
                    .with_value(0.0),
            ),
            None,
            2,
        );
        cx.tree.add_child(
            color_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 2.0)
                    .with_label("Saturation")
                    .with_value(1.0),
            ),
            None,
            3,
        );
        cx.tree.add_child(
            color_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Tone map")),
            None,
            4,
        );

        // GroupBox: Denoise
        let denoise_box = cx.tree.add_child(
            wall_row1,
            Box::new(GroupBox::new(NodeId(0), "Denoise")),
            None,
            2,
        );
        cx.tree.set_flex(denoise_box, FlexParams::flexible(1.0));
        let denoise_col = cx.tree.add_child(
            denoise_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            denoise_box,
            GroupBox::new(denoise_col, "Denoise")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        cx.tree.add_child(
            denoise_col,
            Box::new(halcyon::widgets::segmented_control::SegmentedControl::new(
                vec![
                    halcyon::widgets::segmented_control::Segment::new("NLM"),
                    halcyon::widgets::segmented_control::Segment::new("Bilateral"),
                    halcyon::widgets::segmented_control::Segment::new("Wavelet"),
                ],
                0,
            )),
            None,
            0,
        );
        cx.tree.add_child(
            denoise_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 1.0)
                    .with_label("Strength")
                    .with_value(0.5),
            ),
            None,
            1,
        );
        cx.tree.add_child(
            denoise_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 1.0)
                    .with_label("Detail")
                    .with_value(0.7),
            ),
            None,
            2,
        );
        cx.tree.add_child(
            denoise_col,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_label("Radius")
                    .with_value(4)
                    .with_min(1)
                    .with_max(16)
                    .with_step(1),
            ),
            None,
            3,
        );
        cx.tree.add_child(
            denoise_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::new().with_label("Chroma only")),
            None,
            4,
        );

        // ── Full-width band — Output naming. A single GroupBox spanning the
        // whole wall width with a horizontal control row, for layout
        // diversity against the three-column stage panels above and below.
        let wall_band =
            cx.tree
                .add_child(wall_col, Box::new(Flex::row().with_spacing(8.0)), None, 1);
        let naming_box = cx.tree.add_child(
            wall_band,
            Box::new(GroupBox::new(NodeId(0), "Output naming")),
            None,
            0,
        );
        cx.tree.set_flex(naming_box, FlexParams::flexible(1.0));
        let naming_row = cx.tree.add_child(
            naming_box,
            Box::new(Flex::row().with_spacing(16.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            naming_box,
            GroupBox::new(naming_row, "Output naming")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        // Five equal-width fields, each a label above its control, every field
        // taking an equal share (flex 1.0) of the full band width. Uniform
        // structure, evenly distributed, no trailing void.
        let nf_pattern = cx.tree.add_child(
            naming_row,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            0,
        );
        cx.tree.set_flex(nf_pattern, FlexParams::flexible(1.0));
        cx.tree.add_child(
            nf_pattern,
            Box::new(Label::new("Pattern").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            nf_pattern,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "{name}_{seq}".into(),
                "{date}_{name}".into(),
                "{project}_{seq}".into(),
                "{name}".into(),
            ])),
            None,
            1,
        );

        let nf_start = cx.tree.add_child(
            naming_row,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            1,
        );
        cx.tree.set_flex(nf_start, FlexParams::flexible(1.0));
        cx.tree.add_child(
            nf_start,
            Box::new(Label::new("Start #").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            nf_start,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_value(1)
                    .with_min(0)
                    .with_max(9999)
                    .with_step(1),
            ),
            None,
            1,
        );

        let nf_pad = cx.tree.add_child(
            naming_row,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            2,
        );
        cx.tree.set_flex(nf_pad, FlexParams::flexible(1.0));
        cx.tree.add_child(
            nf_pad,
            Box::new(Label::new("Padding").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            nf_pad,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_value(4)
                    .with_min(1)
                    .with_max(8)
                    .with_step(1),
            ),
            None,
            1,
        );

        let nf_case = cx.tree.add_child(
            naming_row,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            3,
        );
        cx.tree.set_flex(nf_case, FlexParams::flexible(1.0));
        cx.tree.add_child(
            nf_case,
            Box::new(Label::new("Case").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            nf_case,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "Keep".into(),
                "lower".into(),
                "UPPER".into(),
                "Title".into(),
            ])),
            None,
            1,
        );

        let nf_over = cx.tree.add_child(
            naming_row,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            4,
        );
        cx.tree.set_flex(nf_over, FlexParams::flexible(1.0));
        cx.tree.add_child(
            nf_over,
            Box::new(Label::new("Overwrite").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            nf_over,
            Box::new(halcyon::widgets::checkbox::Checkbox::new()),
            None,
            1,
        );

        // ── Row 2 ──
        let wall_row2 =
            cx.tree
                .add_child(wall_col, Box::new(Flex::row().with_spacing(8.0)), None, 2);

        // GroupBox: Sharpen
        let sharpen_box = cx.tree.add_child(
            wall_row2,
            Box::new(GroupBox::new(NodeId(0), "Sharpen")),
            None,
            0,
        );
        cx.tree.set_flex(sharpen_box, FlexParams::flexible(1.0));
        let sharpen_col = cx.tree.add_child(
            sharpen_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            sharpen_box,
            GroupBox::new(sharpen_col, "Sharpen")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        cx.tree.add_child(
            sharpen_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 2.0)
                    .with_label("Amount")
                    .with_value(0.8),
            ),
            None,
            0,
        );
        cx.tree.add_child(
            sharpen_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 5.0)
                    .with_label("Radius")
                    .with_value(1.2),
            ),
            None,
            1,
        );
        cx.tree.add_child(
            sharpen_col,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_label("Threshold")
                    .with_value(3)
                    .with_min(0)
                    .with_max(255)
                    .with_step(1),
            ),
            None,
            2,
        );
        cx.tree.add_child(
            sharpen_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Edge-aware")),
            None,
            3,
        );

        // GroupBox: Watermark
        let watermark_box = cx.tree.add_child(
            wall_row2,
            Box::new(GroupBox::new(NodeId(0), "Watermark")),
            None,
            1,
        );
        cx.tree.set_flex(watermark_box, FlexParams::flexible(1.0));
        let watermark_col = cx.tree.add_child(
            watermark_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            watermark_box,
            GroupBox::new(watermark_col, "Watermark")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        let watermark_pos = cx.tree.add_child(
            watermark_col,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            0,
        );
        cx.tree.add_child(
            watermark_pos,
            Box::new(Label::new("Position").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            watermark_pos,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "Bottom-right".into(),
                "Bottom-left".into(),
                "Top-right".into(),
                "Top-left".into(),
                "Center".into(),
            ])),
            None,
            1,
        );
        cx.tree.add_child(
            watermark_col,
            Box::new(
                halcyon::widgets::slider::Slider::new(0.0, 1.0)
                    .with_label("Opacity")
                    .with_value(0.35),
            ),
            None,
            1,
        );
        cx.tree.add_child(
            watermark_col,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_label("Margin px")
                    .with_value(24)
                    .with_min(0)
                    .with_max(256)
                    .with_step(1),
            ),
            None,
            2,
        );
        cx.tree.add_child(
            watermark_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::new().with_label("Tile")),
            None,
            3,
        );

        // GroupBox: Output
        let output_box = cx.tree.add_child(
            wall_row2,
            Box::new(GroupBox::new(NodeId(0), "Output")),
            None,
            2,
        );
        cx.tree.set_flex(output_box, FlexParams::flexible(1.0));
        let output_col = cx.tree.add_child(
            output_box,
            Box::new(Flex::column().with_spacing(8.0)),
            None,
            0,
        );
        halcyon::replace_widget!(
            cx.tree,
            output_box,
            GroupBox::new(output_col, "Output")
                .with_background(Color::from_srgb(0.0, 0.0, 0.0, 0.015))
        );
        let output_codec = cx.tree.add_child(
            output_col,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            0,
        );
        cx.tree.add_child(
            output_codec,
            Box::new(Label::new("Codec").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            output_codec,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "H.265".into(),
                "H.264".into(),
                "ProRes".into(),
                "PNG".into(),
                "EXR".into(),
            ])),
            None,
            1,
        );
        cx.tree.add_child(
            output_col,
            Box::new(
                halcyon::widgets::spin_box::SpinBox::new()
                    .with_label("CRF")
                    .with_value(18)
                    .with_min(0)
                    .with_max(51)
                    .with_step(1),
            ),
            None,
            1,
        );
        let output_container = cx.tree.add_child(
            output_col,
            Box::new(Flex::column().with_spacing(2.0)),
            None,
            2,
        );
        cx.tree.add_child(
            output_container,
            Box::new(Label::new("Container").with_size(11.0)),
            None,
            0,
        );
        cx.tree.add_child(
            output_container,
            Box::new(halcyon::widgets::combo_box::ComboBox::simple(vec![
                "mp4".into(),
                "mov".into(),
                "mkv".into(),
            ])),
            None,
            1,
        );
        cx.tree.add_child(
            output_col,
            Box::new(halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Faststart")),
            None,
            3,
        );
        cx.tree.add_child(
            output_col,
            Box::new(
                halcyon::widgets::checkbox::Checkbox::checked(true).with_label("Strip metadata"),
            ),
            None,
            4,
        );

        // Close the center vertical split now that both panes exist.
        halcyon::replace_widget!(
            cx.tree,
            center_split,
            SplitPane::vertical(pipe_pane, panel_wall)
                .with_interaction_policy(divider_policy)
                .initial_ratio(0.62)
                .leading_min(160.0)
                .trailing_min(120.0)
        );

        // Inspector — real PropertyEditor v2 (SPEC-HAL-260A) over the asset/job
        // record. PE mounts DIRECTLY as the workspace split's trailing slot
        // child so it gets a bounded max_height; an outer ScrollArea would
        // starve the body. PE owns its own inner ScrollArea over the body.
        let mut editor = PropertyEditor::new();
        editor.add_name_column("Property");
        let pe_value_col = editor.add_value_column("Value");

        let pe_source = Rc::new(RefCell::new(WorkbenchInspectorSource::new(pe_value_col)));
        editor.set_data_source(pe_source);

        let pe_body = editor.create_body_widget();

        let inspector = cx.tree.add_child(work_region, Box::new(editor), None, 1);

        let pe_scroll = ScrollArea::new_unwired();
        let pe_scroll_id = cx.tree.add_child(inspector, Box::new(pe_scroll), None, 0);

        let pe_body_id = cx.tree.add_child(pe_scroll_id, Box::new(pe_body), None, 0);

        if let Some(node) = cx.tree.node_mut(pe_scroll_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(pe_body_id);
            }
        }

        // Close the workspace horizontal split.
        halcyon::replace_widget!(
            cx.tree,
            work_region,
            SplitPane::horizontal(center_split, inspector)
                .with_interaction_policy(divider_policy)
                .initial_ratio(0.74)
                .leading_min(420.0)
                .trailing_min(220.0)
        );

        // Bind the real child node ids into the split.
        halcyon::replace_widget!(
            cx.tree,
            main_split,
            SplitPane::horizontal(nav_split, work_region)
                .with_interaction_policy(divider_policy)
                .initial_ratio(0.20)
                .leading_min(180.0)
                .trailing_min(360.0)
        );

        cx.tree
            .add_child(outer, Box::new(Divider::horizontal()), None, 3);

        // -- Console status strip (slot 4) — natural height --------
        let status = cx.tree.add_child(
            outer,
            Box::new(
                Flex::row()
                    .with_spacing(16.0)
                    .with_padding(EdgeInsets::all(6.0)),
            ),
            None,
            4,
        );
        cx.tree
            .add_child(status, Box::new(Label::new("0 objects")), None, 0);
        cx.tree
            .add_child(status, Box::new(Divider::vertical()), None, 1);
        cx.tree
            .add_child(status, Box::new(Label::new("Ready")), None, 2);
        let status_spacer = cx
            .tree
            .add_child(status, Box::new(FlexSpacer::new()), None, 3);
        cx.tree.set_flex(status_spacer, FlexParams::flexible(1.0));
    }
}
