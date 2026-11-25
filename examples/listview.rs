// =============================================================================
// ListView Authoritative Demo (SPEC-HAL-179/179A Compliant)
// =============================================================================
//
// This demo is the SOLE proof that ListView is a usable UI component.
// It demonstrates all core functionality without abstractions or TreeView hooks.
//
// ## What This Demo Proves
//
// 1. Multi-column rendering (5 columns exceeding window width)
// 2. Mandatory header with column titles (scrolls horizontally with body)
// 3. Scrollable body (vertical + horizontal scrolling via ScrollArea)
// 4. Bounded row-order projection (1,000,000 rows, only the viewport window requested)
// 5. Hit-testing and selection (click selects row, selection highlighted)
// 6. Horizontal scrolling with header/body lockstep
// 7. Provider-authoritative plain and Shift multi-column sorting
// 8. Confirmed direction/priority indicators and baseline reset cycle
// 9. Durable requested-sort restoration through ViewConfigStore
//
// ## Structural Model
//
// ListView is a closed composite:
// - Header region: Fixed at top, scrolls horizontally via HorizontalScrollLink
// - Body region: Inside ScrollArea (vertical + horizontal scroll)
// - Footer child: Mounted as a 0-height structural no-op for resize completeness
//
// =============================================================================

use std::cell::Cell;
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;

use halcyon::core::event::FusedInputEvent;
use halcyon::core::layout::FlexParams;
use halcyon::core::scroll_container::HorizontalScrollLink;
use halcyon::core::widget::Widget;
use halcyon::prelude::*;
use halcyon::widgets::flex::Flex;
use halcyon::widgets::label::Label;
use halcyon::widgets::list_view::ListView;
use halcyon::widgets::scroll_area::ScrollArea;
use halcyon::widgets::tabular_view::column::ColumnId;
use halcyon::widgets::tabular_view::data_adapter::{
    AppliedSortSnapshot, BaselineOrderId, CellCapabilities, CellValue, DataInvalidationEvent,
    DataSource, Extent, OrderAccess, OrderRevision, OrderWindow, OrderWindowRequest,
    SortCapability, SortExactness, SortKey, SortKeyId, SortOperation, SortRequest,
    SortRequestOutcome, SortSpec, SortTerm, UniverseMode, ValueState,
};
use halcyon::widgets::tabular_view::row::RowId;
use halcyon::widgets::tabular_view::{
    Column, ColumnScrollDomain, SortDirection, ViewConfigKey, ViewConfigStore,
    sort_preference_from_raw_json,
};

// =============================================================================
// Configuration
// =============================================================================

/// Number of rows in the demo (proves virtualization at scale).
const ROW_COUNT: usize = 1000_000;

/// Row height (matches ListView default, theme.list_geom.row_height for Compact).
const ROW_HEIGHT: f32 = 24.0;

/// Header height.
const HEADER_HEIGHT: f32 = 24.0;

/// Footer remains mounted for structural resize completeness, but is hidden in
/// the demo because it has no content of its own.
const FOOTER_HEIGHT: f32 = 0.0;

/// Initial window dimensions.
const WINDOW_WIDTH: f32 = 800.0;
const WINDOW_HEIGHT: f32 = 800.0;

/// Column widths.
const COL_INDEX_WIDTH: f32 = 80.0;
const COL_NAME_WIDTH: f32 = 200.0;
const COL_DESC_WIDTH: f32 = 400.0;
const COL_STATUS_WIDTH: f32 = 150.0;
const COL_CATEGORY_WIDTH: f32 = 200.0;
// Total: 80 + 200 + 400 + 150 + 200 = 1030px > 800px window → horizontal scroll triggers.

// =============================================================================
// Demo Data Source
// =============================================================================

/// Column identifiers for the demo.
/// These are set after ListView configures its columns.
#[derive(Default)]
struct DemoColumns {
    index: Option<ColumnId>,
    name: Option<ColumnId>,
    description: Option<ColumnId>,
    status: Option<ColumnId>,
    category: Option<ColumnId>,
}

const BASELINE_ID: &str = "demo.creation-sequence";
const STATUS_SORT_KEY: &str = "demo.workflow-status";
const COL_INDEX_ID: u64 = 10_001;
const COL_NAME_ID: u64 = 10_002;
const COL_DESCRIPTION_ID: u64 = 10_003;
const COL_STATUS_ID: u64 = 10_004;
const COL_CATEGORY_ID: u64 = 10_005;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DemoSortKind {
    Numeric,
    Status,
    Category,
}

/// Algorithmic provider for the ListView sorting proof.
///
/// It deliberately stores no RowId vector and no sorted-index vector. Any
/// requested ordinal window is mapped to stable RowIds from the confirmed
/// SortSpec. This makes the presentation-boundary scale claim observable:
/// one million rows never become one million identities in ListView memory.
pub struct DemoDataSource {
    columns: DemoColumns,
    applied: AppliedSortSnapshot,
    pending_events: Vec<DataInvalidationEvent>,
    last_window: Cell<Option<(OrderRevision, usize, usize)>>,
    max_window_len: Cell<usize>,
}

impl DemoDataSource {
    pub fn new() -> Self {
        Self {
            columns: DemoColumns::default(),
            applied: AppliedSortSnapshot {
                spec: SortSpec::baseline(BaselineOrderId::new(BASELINE_ID)),
                exactness: SortExactness::Exact,
                order_revision: OrderRevision(1),
            },
            pending_events: Vec::new(),
            last_window: Cell::new(None),
            max_window_len: Cell::new(0),
        }
    }

    /// Set column IDs (called after ListView.add_column()).
    pub fn set_columns(
        &mut self,
        index: ColumnId,
        name: ColumnId,
        description: ColumnId,
        status: ColumnId,
        category: ColumnId,
    ) {
        self.columns.index = Some(index);
        self.columns.name = Some(name);
        self.columns.description = Some(description);
        self.columns.status = Some(status);
        self.columns.category = Some(category);
    }

    fn row_index(&self, row_id: RowId) -> Option<usize> {
        let raw = row_id.raw();
        (raw > 0 && raw <= ROW_COUNT as u64).then_some(raw as usize - 1)
    }

    fn row_id(index: usize) -> RowId {
        RowId::from_raw(index as u64 + 1)
    }

    fn sort_kind(&self, key: &SortKey) -> Option<DemoSortKind> {
        match key {
            SortKey::Custom(id) if id.as_str() == STATUS_SORT_KEY => Some(DemoSortKind::Status),
            SortKey::Column(id)
                if Some(*id) == self.columns.index || Some(*id) == self.columns.name =>
            {
                Some(DemoSortKind::Numeric)
            }
            SortKey::Column(id) if Some(*id) == self.columns.category => {
                Some(DemoSortKind::Category)
            }
            _ => None,
        }
    }

    fn category_rank(residue: usize) -> u8 {
        match residue % 4 {
            0 => 0, // Engineering
            2 => 1, // Finance
            1 => 2, // Marketing
            _ => 3, // Operations
        }
    }

    fn categorical_signature(&self, residue: usize, terms: &[SortTerm]) -> Vec<u8> {
        terms
            .iter()
            .filter_map(|term| {
                let (rank, max_rank) = match self.sort_kind(&term.key)? {
                    DemoSortKind::Status => ((residue % 3) as u8, 2),
                    DemoSortKind::Category => (Self::category_rank(residue), 3),
                    DemoSortKind::Numeric => return None,
                };
                Some(match term.direction {
                    SortDirection::Ascending => rank,
                    SortDirection::Descending => max_rank - rank,
                })
            })
            .collect()
    }

    fn ordered_groups(&self) -> (Vec<Vec<usize>>, Option<SortDirection>) {
        let terms = &self.applied.spec.terms;
        let numeric = terms
            .iter()
            .position(|term| self.sort_kind(&term.key) == Some(DemoSortKind::Numeric));
        let categorical_terms = &terms[..numeric.unwrap_or(terms.len())];
        let mut keyed: Vec<(Vec<u8>, usize)> = (0..12)
            .map(|residue| {
                (
                    self.categorical_signature(residue, categorical_terms),
                    residue,
                )
            })
            .collect();
        keyed.sort_by(|left, right| left.0.cmp(&right.0).then(left.1.cmp(&right.1)));

        let mut groups: Vec<Vec<usize>> = Vec::new();
        let mut last_signature: Option<Vec<u8>> = None;
        for (signature, residue) in keyed {
            if last_signature.as_ref() != Some(&signature) {
                groups.push(Vec::new());
                last_signature = Some(signature);
            }
            groups.last_mut().expect("group exists").push(residue);
        }

        let direction = numeric.map(|position| terms[position].direction);
        (groups, direction)
    }

    fn count_in_residues_through(index: usize, residues: &[usize]) -> usize {
        residues
            .iter()
            .map(|residue| {
                if index < *residue {
                    0
                } else {
                    (index - residue) / 12 + 1
                }
            })
            .sum()
    }

    fn count_in_residues(residues: &[usize]) -> usize {
        Self::count_in_residues_through(ROW_COUNT - 1, residues)
    }

    fn select_in_residues(residues: &[usize], ordinal: usize, direction: SortDirection) -> usize {
        let count = Self::count_in_residues(residues);
        let ascending_ordinal = match direction {
            SortDirection::Ascending => ordinal,
            SortDirection::Descending => count - ordinal - 1,
        };
        let mut low = 0usize;
        let mut high = ROW_COUNT - 1;
        while low < high {
            let middle = low + (high - low) / 2;
            if Self::count_in_residues_through(middle, residues) > ascending_ordinal {
                high = middle;
            } else {
                low = middle + 1;
            }
        }
        low
    }

    fn index_at_ordinal(&self, mut ordinal: usize) -> Option<usize> {
        let (groups, numeric_direction) = self.ordered_groups();
        for residues in groups {
            let count = Self::count_in_residues(&residues);
            if ordinal < count {
                return Some(Self::select_in_residues(
                    &residues,
                    ordinal,
                    numeric_direction.unwrap_or(SortDirection::Ascending),
                ));
            }
            ordinal -= count;
        }
        None
    }

    fn ordinal_of_index(&self, index: usize) -> Option<usize> {
        let residue = index % 12;
        let (groups, numeric_direction) = self.ordered_groups();
        let mut prior = 0usize;
        for residues in groups {
            let count = Self::count_in_residues(&residues);
            if residues.contains(&residue) {
                let ascending = Self::count_in_residues_through(index, &residues) - 1;
                let within = match numeric_direction.unwrap_or(SortDirection::Ascending) {
                    SortDirection::Ascending => ascending,
                    SortDirection::Descending => count - ascending - 1,
                };
                return Some(prior + within);
            }
            prior += count;
        }
        None
    }

    fn validate_spec(&self, spec: &SortSpec) -> Result<(), String> {
        if spec.baseline.as_str() != BASELINE_ID {
            return Err("unknown baseline".to_string());
        }
        let mut keys = HashSet::new();
        for term in &spec.terms {
            if !keys.insert(term.key.clone()) {
                return Err("duplicate sort key".to_string());
            }
            if !term.parameters.is_null() {
                return Err("this demo accepts no sort parameters".to_string());
            }
            if !self.sort_capability(&term.key).permits(term.direction) {
                return Err("unsupported sort key or direction".to_string());
            }
        }
        Ok(())
    }

    fn resolve_operation(&self, operation: SortOperation) -> Result<SortSpec, String> {
        let mut spec = self.applied.spec.clone();
        match operation {
            SortOperation::CycleReplace(key) => {
                spec.terms = match spec.terms.iter().find(|term| term.key == key) {
                    None => vec![SortTerm::new(key, SortDirection::Ascending)],
                    Some(term) if term.direction == SortDirection::Ascending => {
                        vec![SortTerm::new(key, SortDirection::Descending)]
                    }
                    Some(_) => Vec::new(),
                };
            }
            SortOperation::CycleAdditive(key) => {
                if let Some(position) = spec.terms.iter().position(|term| term.key == key) {
                    if spec.terms[position].direction == SortDirection::Ascending {
                        spec.terms[position].direction = SortDirection::Descending;
                    } else {
                        spec.terms.remove(position);
                    }
                } else {
                    spec.terms
                        .push(SortTerm::new(key, SortDirection::Ascending));
                }
            }
            SortOperation::SetDirection {
                key,
                direction,
                additive,
            } => {
                if !additive {
                    spec.terms.clear();
                }
                if let Some(term) = spec.terms.iter_mut().find(|term| term.key == key) {
                    term.direction = direction;
                } else {
                    spec.terms.push(SortTerm::new(key, direction));
                }
            }
            SortOperation::RemoveTerm(key) => {
                spec.terms.retain(|term| term.key != key);
            }
            SortOperation::ClearToBaseline(baseline) => {
                spec = SortSpec::baseline(baseline);
            }
            SortOperation::ApplySpec(restored) => spec = restored,
            SortOperation::CancelPending(_) => {
                return Err("the synchronous demo has no pending request".to_string());
            }
        }
        self.validate_spec(&spec)?;
        Ok(spec)
    }
}

impl DataSource for DemoDataSource {
    fn order_access(&self) -> OrderAccess {
        OrderAccess::Windowed
    }

    fn order_revision(&self) -> OrderRevision {
        self.applied.order_revision
    }

    fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow> {
        if request.revision != self.applied.order_revision {
            return ValueState::Error {
                error: Some("stale demo order revision".to_string()),
            };
        }
        let end = request.start.saturating_add(request.length).min(ROW_COUNT);
        let row_ids = (request.start..end)
            .filter_map(|ordinal| self.index_at_ordinal(ordinal).map(Self::row_id))
            .collect::<Vec<_>>();
        self.max_window_len
            .set(self.max_window_len.get().max(row_ids.len()));
        let window_key = (request.revision, request.start, row_ids.len());
        if self.last_window.get() != Some(window_key) {
            println!(
                "[bounded-order proof] revision={} window={}..{} (max cached={})",
                request.revision.0,
                request.start,
                request.start + row_ids.len(),
                self.max_window_len.get()
            );
            self.last_window.set(Some(window_key));
        }
        ValueState::Ready(OrderWindow {
            revision: request.revision,
            start: request.start,
            row_ids,
        })
    }

    fn locate_row(&self, revision: OrderRevision, row_id: RowId) -> ValueState<Option<usize>> {
        if revision != self.applied.order_revision {
            return ValueState::Error {
                error: Some("stale demo order revision".to_string()),
            };
        }
        ValueState::Ready(
            self.row_index(row_id)
                .and_then(|index| self.ordinal_of_index(index)),
        )
    }

    fn sort_key_for_column(&self, column: ColumnId) -> Option<SortKey> {
        if Some(column) == self.columns.status {
            Some(SortKey::Custom(SortKeyId::new(STATUS_SORT_KEY)))
        } else if Some(column) == self.columns.description {
            None
        } else if Some(column) == self.columns.index
            || Some(column) == self.columns.name
            || Some(column) == self.columns.category
        {
            Some(SortKey::Column(column))
        } else {
            None
        }
    }

    fn sort_capability(&self, key: &SortKey) -> SortCapability {
        if self.sort_kind(key).is_some() {
            SortCapability::exact()
        } else {
            SortCapability::Unsupported
        }
    }

    fn applied_sort(&self) -> Option<AppliedSortSnapshot> {
        Some(self.applied.clone())
    }

    fn request_sort(&mut self, request: SortRequest) -> SortRequestOutcome {
        let spec = match self.resolve_operation(request.operation) {
            Ok(spec) => spec,
            Err(reason) => {
                return SortRequestOutcome::Rejected {
                    request_id: request.request_id,
                    reason,
                    current_snapshot: Some(self.applied.clone()),
                };
            }
        };
        if spec != self.applied.spec {
            self.applied = AppliedSortSnapshot {
                spec,
                exactness: SortExactness::Exact,
                order_revision: OrderRevision(self.applied.order_revision.0 + 1),
            };
            self.pending_events
                .push(DataInvalidationEvent::BaseOrderReplaced);
        }
        println!(
            "[sort proof] applied revision={} terms={:?}",
            self.applied.order_revision.0, self.applied.spec.terms
        );
        SortRequestOutcome::Applied {
            request_id: request.request_id,
            snapshot: self.applied.clone(),
        }
    }

    fn extent(&self) -> Extent {
        Extent::Exact(ROW_COUNT)
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, column: ColumnId) -> ValueState<CellValue> {
        let Some(idx) = self.row_index(row) else {
            return ValueState::Missing;
        };

        // Return different data per column.
        if Some(column) == self.columns.index {
            ValueState::Ready(CellValue::Int(idx as i64))
        } else if Some(column) == self.columns.name {
            ValueState::Ready(CellValue::String(format!("Item {}", idx)))
        } else if Some(column) == self.columns.description {
            ValueState::Ready(CellValue::String(format!(
                "Description for row {} - demo data with some extra text",
                idx
            )))
        } else if Some(column) == self.columns.status {
            let status = match idx % 3 {
                0 => "Active",
                1 => "Pending",
                _ => "Archived",
            };
            ValueState::Ready(CellValue::String(status.to_string()))
        } else if Some(column) == self.columns.category {
            let category = match idx % 4 {
                0 => "Engineering",
                1 => "Marketing",
                2 => "Finance",
                _ => "Operations",
            };
            ValueState::Ready(CellValue::String(category.to_string()))
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

// =============================================================================
// Durable demo preference store
// =============================================================================

struct JsonFileViewConfigStore {
    path: PathBuf,
}

impl JsonFileViewConfigStore {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }

    fn read_all(&self) -> HashMap<String, serde_json::Value> {
        fs::read_to_string(&self.path)
            .ok()
            .and_then(|contents| serde_json::from_str(&contents).ok())
            .unwrap_or_default()
    }

    fn write_all(&self, values: &HashMap<String, serde_json::Value>) {
        if let Some(parent) = self.path.parent() {
            let _ = fs::create_dir_all(parent);
        }
        let Ok(json) = serde_json::to_string_pretty(values) else {
            return;
        };
        let temporary = self.path.with_extension("json.tmp");
        if fs::write(&temporary, json).is_ok() {
            let _ = fs::rename(temporary, &self.path);
        }
    }
}

impl ViewConfigStore for JsonFileViewConfigStore {
    fn load(&self, key: &ViewConfigKey) -> Option<serde_json::Value> {
        self.read_all().remove(&key.storage_key())
    }

    fn save(&self, key: &ViewConfigKey, value: &serde_json::Value) {
        let mut values = self.read_all();
        values.insert(key.storage_key(), value.clone());
        self.write_all(&values);
    }

    fn clear(&self, key: &ViewConfigKey) {
        let mut values = self.read_all();
        values.remove(&key.storage_key());
        self.write_all(&values);
    }
}

fn demo_state_path() -> PathBuf {
    std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("target/demo-state/listview-sorting.json")
}

fn configured_list_view(state_path: PathBuf) -> ListView<DemoDataSource> {
    let mut list_view = ListView::new(DemoDataSource::new());
    let col_index = list_view.add_column_raw(
        "Index",
        Column::with_id(ColumnId::from_raw(COL_INDEX_ID), COL_INDEX_WIDTH)
            .with_scroll_domain(ColumnScrollDomain::PinnedLeft),
    );
    let col_name = list_view.add_column_raw(
        "Name",
        Column::with_id(ColumnId::from_raw(COL_NAME_ID), COL_NAME_WIDTH),
    );
    let col_desc = list_view.add_column_raw(
        "Description",
        Column::with_id(ColumnId::from_raw(COL_DESCRIPTION_ID), COL_DESC_WIDTH)
            .with_sortable(false),
    );
    let col_status = list_view.add_column_raw(
        "Status",
        Column::with_id(ColumnId::from_raw(COL_STATUS_ID), COL_STATUS_WIDTH),
    );
    let col_category = list_view.add_column_raw(
        "Category",
        Column::with_id(ColumnId::from_raw(COL_CATEGORY_ID), COL_CATEGORY_WIDTH),
    );
    list_view.adapter_mut().source_mut().set_columns(
        col_index,
        col_name,
        col_desc,
        col_status,
        col_category,
    );
    list_view = list_view
        .with_app_namespace("halcyon.examples")
        .with_view_id("provider-authoritative-sorting")
        .with_config_store(Box::new(JsonFileViewConfigStore::new(state_path)));
    list_view.finalize_config();
    list_view
}

fn run_headless_proof() {
    let proof_path = std::env::current_dir()
        .unwrap_or_else(|_| std::env::temp_dir())
        .join("target/demo-state/listview-sorting-proof.json");
    let _ = fs::remove_file(&proof_path);

    let mut list = configured_list_view(proof_path.clone());
    assert_eq!(list.row_count(), ROW_COUNT);
    assert!(list.adapter_mut().cached_row_ids().is_empty());
    let initial = list.adapter_mut().fetch_order_window(500_000, 41).unwrap();
    assert_eq!(initial.row_ids.len(), 41);

    let status = SortKey::Custom(SortKeyId::new(STATUS_SORT_KEY));
    let category = SortKey::Column(ColumnId::from_raw(COL_CATEGORY_ID));
    list.request_sort(SortOperation::CycleReplace(status.clone()));
    let status_rows = list.adapter_mut().fetch_order_window(0, 4).unwrap();
    assert_eq!(
        status_rows.row_ids,
        vec![
            RowId::from_raw(1),
            RowId::from_raw(4),
            RowId::from_raw(7),
            RowId::from_raw(10),
        ]
    );

    list.request_sort(SortOperation::CycleAdditive(category));
    let multi_rows = list.adapter_mut().fetch_order_window(0, 4).unwrap();
    assert_eq!(
        multi_rows.row_ids,
        vec![
            RowId::from_raw(1),
            RowId::from_raw(13),
            RowId::from_raw(25),
            RowId::from_raw(37),
        ]
    );
    let persisted_spec = list.applied_sort().expect("applied sort").spec;
    assert_eq!(persisted_spec.terms.len(), 2);
    drop(list);

    let mut restored = configured_list_view(proof_path.clone());
    assert_eq!(
        restored.applied_sort().expect("restored sort").spec,
        persisted_spec
    );
    restored.clear_sort().expect("baseline request");
    assert!(
        restored
            .applied_sort()
            .expect("baseline applied")
            .spec
            .terms
            .is_empty()
    );
    drop(restored);

    let mut baseline = configured_list_view(proof_path.clone());
    assert!(
        baseline
            .applied_sort()
            .expect("persisted baseline")
            .spec
            .terms
            .is_empty()
    );
    assert!(baseline.adapter_mut().cached_row_ids().is_empty());
    baseline.request_sort(SortOperation::CycleReplace(status));
    baseline
        .reset_sort_preference()
        .expect("reset preference baseline request");
    let proof_key = ViewConfigKey {
        app_namespace: "halcyon.examples".to_string(),
        widget_type: "list_view".to_string(),
        view_id: "provider-authoritative-sorting".to_string(),
    };
    let saved = JsonFileViewConfigStore::new(proof_path.clone())
        .load(&proof_key)
        .expect("column configuration remains persisted");
    assert!(sort_preference_from_raw_json(&proof_key, &saved).is_none());
    drop(baseline);

    let reset = configured_list_view(proof_path.clone());
    assert!(
        reset
            .applied_sort()
            .expect("reset starts at provider baseline")
            .spec
            .terms
            .is_empty()
    );
    let _ = fs::remove_file(proof_path);
    println!(
        "[proof PASS] bounded windows, multi-sort, clear/reset, and restart restoration verified"
    );
}

// =============================================================================
// ListView Demo Screen
// =============================================================================

/// Demo screen that owns and manages a ListView.
struct ListViewDemoScreen {
    /// Placeholder widget for returning from build().
    placeholder: Label,
    /// Whether we've built the tree.
    built: bool,
}

impl ListViewDemoScreen {
    fn new() -> Self {
        Self {
            placeholder: Label::new(""),
            built: false,
        }
    }
}

impl Screen for ListViewDemoScreen {
    fn build<'a>(&'a mut self, cx: &'a mut ScreenFrameContext<'a>) -> &'a mut dyn Widget {
        if self.built {
            return &mut self.placeholder;
        }
        self.built = true;

        let mut list_view = configured_list_view(demo_state_path());

        // Keep footer mounted for resize completeness, but hidden by default.
        list_view.set_footer_height(FOOTER_HEIGHT);

        // Wire horizontal scroll link for header/footer alignment with body scroll.
        let hscroll_link = HorizontalScrollLink::new();
        let hscroll_freeze = Rc::new(Cell::new(false));
        list_view.set_horizontal_scroll_link(hscroll_link.clone(), hscroll_freeze.clone());

        // Create body and footer widgets BEFORE moving list_view (shares Rc state).
        let body_widget = list_view.create_body_widget();
        let footer_widget = list_view.create_footer_widget();

        // Build widget tree:
        // Root (Flex column)
        //   └─ ListView (flexible, fills remaining space)
        //        ├─ ScrollArea (body scroll, h-scroll enabled) [children[0]]
        //        │    └─ ListViewBody (row content)
        //        └─ ListViewFooter [children[1]]
        let tree_root = cx.tree.root().expect("Tree must have root");
        let root = cx
            .tree
            .add_child(tree_root, Box::new(Flex::column()), None, 0);

        // Add ListView as child of Flex.
        let list_view_id = cx.tree.add_child(root, Box::new(list_view), None, 0);
        cx.tree.set_flex(list_view_id, FlexParams::flexible(1.0));

        // Add ScrollArea (unwired) as child of ListView.
        // Disable vertical inset so body content touches header directly.
        // Enable horizontal scrollbar and wire scroll link + freeze.
        let scroll_area = ScrollArea::new_unwired()
            .enable_horizontal_scrollbar(true)
            .with_horizontal_scroll_link(hscroll_link)
            .with_horizontal_freeze(hscroll_freeze);
        let scroll_area_id = cx
            .tree
            .add_child(list_view_id, Box::new(scroll_area), None, 0);

        // Add body widget as child of ScrollArea.
        let body_id = cx
            .tree
            .add_child(scroll_area_id, Box::new(body_widget), None, 0);

        // Wire ScrollArea to its content.
        if let Some(node) = cx.tree.node_mut(scroll_area_id) {
            if let Some(scroll_area_widget) =
                node.widget_mut().as_any_mut().downcast_mut::<ScrollArea>()
            {
                scroll_area_widget.wire_content(body_id);
            }
        }

        // Add footer widget as child of ListView (children[1]).
        // Footer is positioned below body, fixed at bottom.
        let _footer_id = cx
            .tree
            .add_child(list_view_id, Box::new(footer_widget), None, 0);

        &mut self.placeholder
    }

    fn handle_input(&mut self, _event: &FusedInputEvent, _cx: &mut ScreenContext<'_>) -> bool {
        // Let all input pass through to the widgets.
        // ScrollArea handles wheel scroll, ListView handles clicks.
        false
    }

    fn on_enter(&mut self, _cx: &mut ScreenContext<'_>) {
        println!("=== ListView Provider-Authoritative Sorting Proof (SPEC-HAL-353) ===");
        println!();
        println!("Configuration:");
        println!(
            "  - {} rows; provider stores no RowId/sorted-index vector",
            ROW_COUNT
        );
        println!("  - ListView requests one bounded revision-scoped viewport window");
        println!(
            "  - 5 columns: Index ({}px), Name ({}px), Description ({}px), Status ({}px), Category ({}px)",
            COL_INDEX_WIDTH, COL_NAME_WIDTH, COL_DESC_WIDTH, COL_STATUS_WIDTH, COL_CATEGORY_WIDTH
        );
        println!(
            "  - Total column width: {}px > {}px window → horizontal scroll",
            COL_INDEX_WIDTH
                + COL_NAME_WIDTH
                + COL_DESC_WIDTH
                + COL_STATUS_WIDTH
                + COL_CATEGORY_WIDTH,
            WINDOW_WIDTH
        );
        println!("  - Row height: {}px", ROW_HEIGHT);
        println!("  - Header height: {}px", HEADER_HEIGHT);
        println!();
        println!("Structural model (INV-LV-DOMAIN-1):");
        println!("  - Header: FIXED at top, scrolls horizontally with body");
        println!("  - Body: Scrollable region (vertical + horizontal)");
        println!(
            "  - Footer: mounted structural no-op, hidden by default ({}px)",
            FOOTER_HEIGHT
        );
        println!();
        println!("Controls:");
        println!("  - Mouse wheel: scroll body vertically and horizontally");
        println!("  - Click: select row");
        println!("  - Click sortable header: ascending → descending → provider baseline");
        println!("  - Shift+click sortable header: append/toggle/remove multi-sort term");
        println!("  - Sortable headers show a quiet ↑↓ affordance that strengthens on hover");
        println!("  - Header cells animate hover and button-like press/release feedback");
        println!("  - Active arrows and numbers are provider-confirmed direction/priority");
        println!("  - Description is intentionally non-sortable (view + provider policy)");
        println!(
            "  - Status uses custom provider strategy '{}'",
            STATUS_SORT_KEY
        );
        println!("  - Drag separator: resize column");
        println!("  - Double-click separator: auto-size column");
        println!("  - Drag header: reorder column (click-vs-drag arbitration remains active)");
        println!(
            "  - Sort preference survives restart at {}",
            demo_state_path().display()
        );
        println!();
        println!("Verification checklist:");
        println!("  [ ] 5 columns visible (total > window width)");
        println!("  [ ] Horizontal scrollbar appears");
        println!("  [ ] Scroll horizontally: header and body move in lockstep");
        println!("  [ ] Hover separator while scrolled: correct separator highlights");
        println!("  [ ] Drag separator while scrolled: correct column resizes");
        println!("  [ ] Double-click separator while scrolled: auto-size works");
        println!("  [ ] Hover sortable headers: quiet ↑↓ and cell feedback become clearer");
        println!("  [ ] Press a header: button-like press/release feedback is visible");
        println!("  [ ] Click Status: rows group Active, Pending, Archived; header shows ↑1");
        println!("  [ ] Shift+click Category: alphabetical tie-break groups; headers show ↑1/↑2");
        println!("  [ ] Click Status twice more: descending, then reset to baseline");
        println!("  [ ] Restart after sorting: saved spec is re-requested and revalidated");
        println!("  [ ] Console windows stay bounded while scrolling one million rows");
        println!();
        println!("Close window to exit.");
        println!("=================================================");
    }

    fn mark_rebuild(&mut self) {
        // No rebuild needed; widget tree is static.
    }

    fn take_rebuild_flag(&mut self) -> bool {
        false
    }
}

// =============================================================================
// Main Entry Point
// =============================================================================

fn main() {
    if std::env::args().any(|argument| argument == "--proof") {
        run_headless_proof();
        return;
    }
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::INFO)
        .init();

    let app = App::new(
        AppConfig::new("ListView Multi-Column Demo").with_size(WINDOW_WIDTH, WINDOW_HEIGHT),
    );
    app.run(ListViewDemoScreen::new());
}
