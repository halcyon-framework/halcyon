// Showcase — Tree View tab content.
//
// Standalone showcase of TreeView at scale. Loads a synthetic ~100,000-node
// datacenter fleet hierarchy into a single TreeView, exercising
// HierarchicalTabularSubstrate's virtualization at 20x the stress baseline
// established by examples/treeview.rs (5,000 nodes).
//
// Composition mirrors examples/treeview.rs:135-219, substituting `parent`
// (the showcase's content_node) for the standalone's root Flex column:
//
//   content_node (passed in by ShowcaseScreen::build)
//     +- TreeView (flex 1.0)
//          +- ScrollArea (horizontal scrollbar enabled, scroll-link wired)
//          |    +- TreeViewBody
//          +- TreeViewFooter
//
// The tree column carries the node name; four data columns (Type, Status,
// Detail, Last Seen) carry per-depth content sourced from
// `DatacenterCellSource` — a `CellSource` implementation defined below
// that derives cell values deterministically from `node_id` and depth.
// SPEC-HAL-188 (Data Source & Mutation Contracts). SPEC-PM-016-R5
// (`cell()` is O(1) per call). Content is fully deterministic so the
// showcase is reproducible across runs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use super::ShowcaseScreen;
use halcyon::core::app::ScreenFrameContext;
use halcyon::core::layout::FlexParams;
use halcyon::core::widget_tree::NodeId;
use halcyon::engines::cell_source::{Cell, CellSource, PropertyValue, Severity};
use halcyon::engines::hierarchical::NodeMeta;
use halcyon::prelude::*;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::column::{Column, ColumnId, ColumnModel};
use halcyon::widgets::tree_view::TreeView;

// =============================================================================
// Synthetic datacenter fleet topology generator.
// =============================================================================
//
// Six-level hierarchy:
//
//   L0  Fleet (root)                                       1
//   L1  Region                                             4
//   L2  Datacenter                                        20
//   L3  Rack                                             500
//   L4  Host                                          10,000
//   L5  Service instance (leaf)                       90,000
//                                                ----------
//                                          Total:   100,525
//
// Labels are deterministic. Host serial is global (host-h00000 .. host-h09999);
// other levels number per parent. Service instance labels cycle through nine
// fixed service names per host.

const REGION_COUNT: usize = 4;
const DCS_PER_REGION: usize = 5;
const RACKS_PER_DC: usize = 25;
const HOSTS_PER_RACK: usize = 20;
const INSTANCES_PER_HOST: usize = 9;

const REGION_NAMES: [&str; REGION_COUNT] =
    ["us-east-1", "us-west-2", "eu-west-1", "ap-southeast-2"];

const SERVICE_NAMES: [&str; INSTANCES_PER_HOST] = [
    "api-gateway",
    "auth-service",
    "cache-redis",
    "ingest-pipeline",
    "metrics-collector",
    "scheduler",
    "storage-blob",
    "telemetry-emitter",
    "worker-pool",
];

const EXPECTED_NODE_COUNT: usize = 1
    + REGION_COUNT
    + REGION_COUNT * DCS_PER_REGION
    + REGION_COUNT * DCS_PER_REGION * RACKS_PER_DC
    + REGION_COUNT * DCS_PER_REGION * RACKS_PER_DC * HOSTS_PER_RACK
    + REGION_COUNT * DCS_PER_REGION * RACKS_PER_DC * HOSTS_PER_RACK * INSTANCES_PER_HOST;

/// Build the synthetic datacenter topology.
///
/// Returns a flat DFS-ordered `Vec<NodeMeta>` suitable for
/// `TreeView::set_tree_data`. Each node's `parent_index` points to an
/// earlier entry in the same Vec; the root sits at index 0.
fn build_dc_topology() -> Vec<NodeMeta> {
    let mut nodes: Vec<NodeMeta> = Vec::with_capacity(EXPECTED_NODE_COUNT);
    let mut host_serial: usize = 0;

    // L0: root
    let root_idx = nodes.len();
    nodes.push(NodeMeta {
        node_id: root_idx as u64,
        parent_index: None,
        depth: 0,
        has_children: true,
        label: "Fleet".to_string(),
    });

    for r in 0..REGION_COUNT {
        // L1: region
        let region_idx = nodes.len();
        nodes.push(NodeMeta {
            node_id: region_idx as u64,
            parent_index: Some(root_idx),
            depth: 1,
            has_children: true,
            label: REGION_NAMES[r].to_string(),
        });

        for dc in 0..DCS_PER_REGION {
            // L2: datacenter
            let dc_idx = nodes.len();
            nodes.push(NodeMeta {
                node_id: dc_idx as u64,
                parent_index: Some(region_idx),
                depth: 2,
                has_children: true,
                label: format!("{}-dc-{:02}", REGION_NAMES[r], dc + 1),
            });

            for rack in 0..RACKS_PER_DC {
                // L3: rack
                let rack_idx = nodes.len();
                nodes.push(NodeMeta {
                    node_id: rack_idx as u64,
                    parent_index: Some(dc_idx),
                    depth: 3,
                    has_children: true,
                    label: format!("rack-{:03}", rack + 1),
                });

                for _ in 0..HOSTS_PER_RACK {
                    // L4: host
                    let host_idx = nodes.len();
                    nodes.push(NodeMeta {
                        node_id: host_idx as u64,
                        parent_index: Some(rack_idx),
                        depth: 4,
                        has_children: true,
                        label: format!("host-h{:05}", host_serial),
                    });
                    host_serial += 1;

                    for inst in 0..INSTANCES_PER_HOST {
                        // L5: service instance (leaf)
                        let leaf_idx = nodes.len();
                        nodes.push(NodeMeta {
                            node_id: leaf_idx as u64,
                            parent_index: Some(host_idx),
                            depth: 5,
                            has_children: false,
                            label: SERVICE_NAMES[inst].to_string(),
                        });
                    }
                }
            }
        }
    }

    debug_assert_eq!(nodes.len(), EXPECTED_NODE_COUNT);
    nodes
}

// =============================================================================
// DatacenterCellSource — per-depth cell content for non-tree columns.
// =============================================================================
//
// SPEC-HAL-188 (Data Source & Mutation Contracts). Implements `CellSource`
// against the datacenter topology produced by `build_dc_topology`. Cell
// content is computed on demand from `node_id` and the node's cached
// `depth`; SPEC-PM-016-R5 — every `cell()` invocation is O(1) (hashmap
// lookup + match + `format!`).
//
// Read-only source. `commit_cell` returns `false` unconditionally; the
// showcase carries no edit surface for these cells.
//
// Variety is keyed off `node_id`:
//   - L4 Host status distributes 80% Online / 10% Degraded / 6% Offline /
//     4% Maintenance via `node_id % 100`.
//   - L4 Host class cycles through four canonical instance types via
//     `node_id % 4`.
//   - L5 Service status distributes the same enum, offset by `+7` to
//     decorrelate from parent host status.
//   - L5 Service version cycles through five tags via `node_id % 5`.
//   - L4/L5 Last Seen cycles through four recency labels via `node_id % 4`.
//
// Topology variety (non-uniform region sizes, maintenance racks, varied
// instances-per-host) is explicitly deferred to a future task; here the
// topology stays uniform and variety lives in cell content.

struct DatacenterCellSource {
    type_col_id: ColumnId,
    status_col_id: ColumnId,
    detail_col_id: ColumnId,
    last_seen_col_id: ColumnId,
    owner_col_id: ColumnId,
    /// `node_id` → `depth`. Built once at construction; constant for the
    /// life of the source.
    node_depths: HashMap<u64, u32>,
    /// Per-service Owner edits, keyed by `node_id`. Empty at construction;
    /// `commit_cell` writes here, `cell` reads here (override beats the
    /// derived default). Identity-keyed so an edit survives virtualization
    /// and scroll — the same `node_id` re-realizes the stored value.
    owner_overrides: HashMap<u64, String>,
    /// Bumped on every accepted `commit_cell`. Drives the substrate's
    /// content-width re-measurement (SPEC-HAL-180); the per-cell display
    /// value re-realizes every layout pass regardless (SPEC-PM-016).
    revision: u64,
}

impl DatacenterCellSource {
    fn new(
        type_col_id: ColumnId,
        status_col_id: ColumnId,
        detail_col_id: ColumnId,
        last_seen_col_id: ColumnId,
        owner_col_id: ColumnId,
        topology: &[NodeMeta],
    ) -> Self {
        let node_depths = topology.iter().map(|n| (n.node_id, n.depth)).collect();
        Self {
            type_col_id,
            status_col_id,
            detail_col_id,
            last_seen_col_id,
            owner_col_id,
            node_depths,
            owner_overrides: HashMap::new(),
            revision: 0,
        }
    }
}

impl CellSource for DatacenterCellSource {
    fn cell(&self, row_id: u64, column_id: ColumnId) -> Option<Cell> {
        let depth = *self.node_depths.get(&row_id)?;

        // Status column renders as a severity-colored Badge (SPEC-HAL-332 §3:
        // success / warning / error are functional, not decorative). Badge is
        // non-editable by construction, so the read-only data-source contract
        // is unchanged. All other data columns render as plain text.
        if column_id == self.status_col_id {
            let status = status_string(depth, row_id);
            let severity = status_severity(&status);
            return Some(Cell::read_only(PropertyValue::Badge(status, severity)));
        }

        // Owner column: editable text on Service (leaf, depth 5) rows only.
        // The double-click edit gesture is reachable only on leaf rows —
        // rows with children consume double-click for expand/collapse
        // (tree_view/body.rs) — and "owner" is a per-service attribute, not
        // an aggregate, so Owner carries an editable cell at depth 5 and no
        // cell elsewhere. A stored override (a prior edit) beats the derived
        // default. SPEC-HAL-174 (edit lifecycle), SPEC-HAL-188 (commit).
        if column_id == self.owner_col_id {
            if depth != 5 {
                return None;
            }
            let owner = self
                .owner_overrides
                .get(&row_id)
                .cloned()
                .unwrap_or_else(|| default_owner(row_id).to_string());
            return Some(Cell::new(PropertyValue::String(owner), true));
        }

        let text = if column_id == self.type_col_id {
            type_string(depth).to_string()
        } else if column_id == self.detail_col_id {
            detail_string(depth, row_id)
        } else if column_id == self.last_seen_col_id {
            last_seen_string(depth, row_id)
        } else {
            return None;
        };
        Some(Cell::read_only(PropertyValue::String(text)))
    }

    fn commit_cell(&mut self, row_id: u64, column_id: ColumnId, new_value: PropertyValue) -> bool {
        if column_id == self.owner_col_id {
            self.owner_overrides
                .insert(row_id, new_value.display_text());
            self.revision = self.revision.wrapping_add(1);
            return true;
        }
        false
    }

    fn revision(&self) -> u64 {
        self.revision
    }
}

fn type_string(depth: u32) -> &'static str {
    match depth {
        0 => "Fleet",
        1 => "Region",
        2 => "Datacenter",
        3 => "Rack",
        4 => "Host",
        5 => "Service",
        _ => "—",
    }
}

fn status_string(depth: u32, node_id: u64) -> String {
    match depth {
        0 => "—".to_string(),
        1 | 2 | 3 => "Mixed".to_string(),
        4 => host_status(node_id).to_string(),
        5 => host_status(node_id.wrapping_add(7)).to_string(),
        _ => "—".to_string(),
    }
}

fn host_status(node_id: u64) -> &'static str {
    match node_id % 100 {
        0..=79 => "ONLINE",
        80..=89 => "DEGRADED",
        90..=95 => "OFFLINE",
        _ => "MAINTENANCE",
    }
}

/// Derived default Owner for a Service (leaf) row, cycled deterministically
/// by `node_id`. Shown until the user edits the cell; a committed edit
/// overrides it via `owner_overrides`.
fn default_owner(node_id: u64) -> &'static str {
    match node_id % 5 {
        0 => "platform-team",
        1 => "sre-team",
        2 => "data-team",
        3 => "identity-team",
        _ => "observability-team",
    }
}

/// Maps a status label to its functional severity for Badge rendering
/// (SPEC-HAL-332 §3 functional palette). Keep in lockstep with the strings
/// produced by `host_status` / `status_string`:
///   ONLINE → Success, DEGRADED → Warning, OFFLINE → Error,
///   MAINTENANCE → Info. Aggregate / placeholder labels ("Mixed", "—")
///   carry no health verdict and fall to Neutral (universal text color).
fn status_severity(status: &str) -> Severity {
    match status {
        "ONLINE" => Severity::Success,
        "DEGRADED" => Severity::Warning,
        "OFFLINE" => Severity::Error,
        "MAINTENANCE" => Severity::Info,
        _ => Severity::Neutral,
    }
}

fn detail_string(depth: u32, node_id: u64) -> String {
    match depth {
        0 => format!(
            "{} nodes across {} regions",
            EXPECTED_NODE_COUNT, REGION_COUNT
        ),
        1 => format!(
            "{} datacenters, {} hosts",
            DCS_PER_REGION,
            DCS_PER_REGION * RACKS_PER_DC * HOSTS_PER_RACK
        ),
        2 => format!(
            "{} racks, {} hosts",
            RACKS_PER_DC,
            RACKS_PER_DC * HOSTS_PER_RACK
        ),
        3 => format!("{} hosts", HOSTS_PER_RACK),
        4 => format!("{}, {} services", host_class(node_id), INSTANCES_PER_HOST),
        5 => format!(
            "{}, {} req/s",
            service_version(node_id),
            service_rps(node_id)
        ),
        _ => String::new(),
    }
}

fn host_class(node_id: u64) -> &'static str {
    match node_id % 4 {
        0 => "c5.xlarge",
        1 => "m5.2xlarge",
        2 => "r5.4xlarge",
        _ => "i3.large",
    }
}

fn service_version(node_id: u64) -> &'static str {
    match node_id % 5 {
        0 => "v2.3.1",
        1 => "v2.3.2",
        2 => "v2.2.8",
        3 => "v2.4.0",
        _ => "v2.3.0",
    }
}

fn service_rps(node_id: u64) -> String {
    let n = (node_id.wrapping_mul(13) % 2400) + 100;
    if n >= 1000 {
        format!("{}.{}k", n / 1000, (n % 1000) / 100)
    } else {
        format!("{}", n)
    }
}

fn last_seen_string(depth: u32, node_id: u64) -> String {
    match depth {
        0..=3 => "now".to_string(),
        4 | 5 => match node_id % 4 {
            0 => "now".to_string(),
            1 => "5s ago".to_string(),
            2 => "30s ago".to_string(),
            _ => "2m ago".to_string(),
        },
        _ => String::new(),
    }
}

impl ShowcaseScreen {
    /// Build the Tree View tab content.
    ///
    /// Mirrors examples/treeview.rs:135-219, with `parent` (the showcase's
    /// content_node) substituted for the standalone's root Flex column.
    /// Single tree column, no data columns — see file header for why.
    pub(super) fn build_tree_view(&self, cx: &mut ScreenFrameContext<'_>, parent: NodeId) {
        let mut tree_view = TreeView::new();

        let nodes = build_dc_topology();

        // Six columns: tree column (Name), four read-only data columns
        // (Type, Status, Detail, Last Seen), and one editable column
        // (Owner). Detail is the widest because L0/L1 descriptions are
        // verbose; Status, Last Seen, and Type are narrow. Owner sits third
        // so it is visible without horizontal scrolling. Total content width
        // 1140px exceeds typical showcase tab content area on a
        // default-sized window, exercising the horizontal scrollbar and the
        // header↔body scroll link wired below.
        let mut col_model = ColumnModel::new();
        let tree_col = Column::new(320.0);
        let type_col = Column::new(120.0);
        let owner_col = Column::new(160.0);
        let status_col = Column::new(120.0);
        let detail_col = Column::new(300.0);
        let last_seen_col = Column::new(120.0);
        let tree_col_id = tree_col.id;
        let type_col_id = type_col.id;
        let owner_col_id = owner_col.id;
        let status_col_id = status_col.id;
        let detail_col_id = detail_col.id;
        let last_seen_col_id = last_seen_col.id;
        col_model.push(tree_col);
        col_model.push(type_col);
        col_model.push(owner_col);
        col_model.push(status_col);
        col_model.push(detail_col);
        col_model.push(last_seen_col);
        tree_view.set_columns(col_model);
        tree_view.set_tree_column(tree_col_id);
        tree_view.set_column_title(tree_col_id, "Name");
        tree_view.set_column_title(type_col_id, "Type");
        tree_view.set_column_title(owner_col_id, "Owner");
        tree_view.set_column_title(status_col_id, "Status");
        tree_view.set_column_title(detail_col_id, "Detail");
        tree_view.set_column_title(last_seen_col_id, "Last Seen");

        // Wire the cell source BEFORE moving `nodes` into `set_tree_data`.
        // Source reads `depth` from a borrowed `&nodes` slice and caches
        // a `HashMap<u64, u32>` internally — the topology itself moves
        // into the substrate.
        let cell_source = Rc::new(RefCell::new(DatacenterCellSource::new(
            type_col_id,
            status_col_id,
            detail_col_id,
            last_seen_col_id,
            owner_col_id,
            &nodes,
        )));
        tree_view.set_cell_source(Some(cell_source));

        tree_view.set_tree_data(nodes);

        tree_view.set_footer_height(24.0);

        // Pull the horizontal scroll link before moving tree_view.
        let hscroll_link = tree_view.horizontal_scroll_link();
        let hscroll_freeze = tree_view.horizontal_scroll_freeze();

        // Body and footer share Rc state with TreeView — create both
        // before tree_view is moved into the widget tree.
        let body_widget = tree_view.create_body_widget();
        let footer_widget = tree_view.create_footer_widget();

        // TreeView fills the tab's content area.
        let tree_view_id = cx.tree.add_child(parent, Box::new(tree_view), None, 0);
        cx.tree.set_flex(tree_view_id, FlexParams::flexible(1.0));

        // ScrollArea is the first child of TreeView. Horizontal scrollbar
        // is enabled and the scroll link is wired so the (built-in) header
        // tracks body horizontal scroll.
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze)
            .with_content_owned_vertical_translation();
        let scroll_area_id = cx
            .tree
            .add_child(tree_view_id, Box::new(scroll_area), None, 0);

        // TreeViewBody is the content of the ScrollArea.
        let body_id = cx
            .tree
            .add_child(scroll_area_id, Box::new(body_widget), None, 0);

        // Wire ScrollArea to its content.
        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(sa) = node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>() {
                sa.wire_content(body_id);
            }
        }

        // Footer is the second child of TreeView.
        cx.tree
            .add_child(tree_view_id, Box::new(footer_widget), None, 0);
    }
}
