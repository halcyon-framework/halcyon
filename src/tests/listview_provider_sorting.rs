use std::cell::Cell;
use std::rc::Rc;

use crate::widgets::list_view::ListView;
use crate::widgets::tabular_view::column::{Column, ColumnId};
use crate::widgets::tabular_view::data_adapter::{
    AppliedSortSnapshot, BaselineOrderId, CellCapabilities, CellValue, DataInvalidationEvent,
    DataSource, Extent, OrderAccess, OrderRevision, OrderWindow, OrderWindowRequest,
    SortCapability, SortExactness, SortKey, SortOperation, SortRequest, SortRequestOutcome,
    SortSpec, SortTerm, UniverseMode, ValueState,
};
use crate::widgets::tabular_view::intents::SortDirection;
use crate::widgets::tabular_view::row::RowId;

const ROWS: usize = 1_000_000;

struct WindowedSortSource {
    column: ColumnId,
    snapshot: AppliedSortSnapshot,
    events: Vec<DataInvalidationEvent>,
    max_requested: Rc<Cell<usize>>,
}

impl WindowedSortSource {
    fn new(column: ColumnId, max_requested: Rc<Cell<usize>>) -> Self {
        Self {
            column,
            snapshot: AppliedSortSnapshot {
                spec: SortSpec::baseline(BaselineOrderId::new("test.baseline")),
                exactness: SortExactness::Exact,
                order_revision: OrderRevision(1),
            },
            events: Vec::new(),
            max_requested,
        }
    }

    fn resolve(&self, operation: SortOperation) -> SortSpec {
        let mut spec = self.snapshot.spec.clone();
        match operation {
            SortOperation::CycleReplace(key) => {
                spec.terms = match spec.terms.first() {
                    Some(term) if term.key == key && term.direction == SortDirection::Ascending => {
                        vec![SortTerm::new(key, SortDirection::Descending)]
                    }
                    Some(term)
                        if term.key == key && term.direction == SortDirection::Descending =>
                    {
                        Vec::new()
                    }
                    _ => vec![SortTerm::new(key, SortDirection::Ascending)],
                };
            }
            SortOperation::CycleAdditive(key) => {
                if let Some(term) = spec.terms.iter_mut().find(|term| term.key == key) {
                    term.direction = term.direction.toggle();
                } else {
                    spec.terms
                        .push(SortTerm::new(key, SortDirection::Ascending));
                }
            }
            SortOperation::ClearToBaseline(baseline) => spec = SortSpec::baseline(baseline),
            SortOperation::ApplySpec(restored) => spec = restored,
            SortOperation::SetDirection {
                key,
                direction,
                additive,
            } => {
                if !additive {
                    spec.terms.clear();
                }
                spec.terms.push(SortTerm::new(key, direction));
            }
            SortOperation::RemoveTerm(key) => spec.terms.retain(|term| term.key != key),
            SortOperation::CancelPending(_) => {}
        }
        spec
    }
}

impl DataSource for WindowedSortSource {
    fn order_access(&self) -> OrderAccess {
        OrderAccess::Windowed
    }

    fn order_revision(&self) -> OrderRevision {
        self.snapshot.order_revision
    }

    fn order_window(&self, request: OrderWindowRequest) -> ValueState<OrderWindow> {
        if request.revision != self.snapshot.order_revision {
            return ValueState::Error {
                error: Some("stale".to_string()),
            };
        }
        self.max_requested
            .set(self.max_requested.get().max(request.length));
        let descending = self
            .snapshot
            .spec
            .terms
            .first()
            .is_some_and(|term| term.direction == SortDirection::Descending);
        let end = request.start.saturating_add(request.length).min(ROWS);
        let row_ids = (request.start..end)
            .map(|ordinal| {
                let index = if descending {
                    ROWS - ordinal - 1
                } else {
                    ordinal
                };
                RowId::from_raw(index as u64 + 1)
            })
            .collect();
        ValueState::Ready(OrderWindow {
            revision: request.revision,
            start: request.start,
            row_ids,
        })
    }

    fn locate_row(&self, revision: OrderRevision, row_id: RowId) -> ValueState<Option<usize>> {
        if revision != self.snapshot.order_revision {
            return ValueState::Error {
                error: Some("stale".to_string()),
            };
        }
        let index = row_id.raw().checked_sub(1).map(|raw| raw as usize);
        let descending = self
            .snapshot
            .spec
            .terms
            .first()
            .is_some_and(|term| term.direction == SortDirection::Descending);
        ValueState::Ready(
            index
                .filter(|index| *index < ROWS)
                .map(|index| if descending { ROWS - index - 1 } else { index }),
        )
    }

    fn sort_capability(&self, key: &SortKey) -> SortCapability {
        if *key == SortKey::Column(self.column) {
            SortCapability::exact()
        } else {
            SortCapability::Unsupported
        }
    }

    fn applied_sort(&self) -> Option<AppliedSortSnapshot> {
        Some(self.snapshot.clone())
    }

    fn request_sort(&mut self, request: SortRequest) -> SortRequestOutcome {
        let spec = self.resolve(request.operation);
        if spec != self.snapshot.spec {
            self.snapshot.spec = spec;
            self.snapshot.order_revision = OrderRevision(self.snapshot.order_revision.0 + 1);
            self.events.push(DataInvalidationEvent::BaseOrderReplaced);
        }
        SortRequestOutcome::Applied {
            request_id: request.request_id,
            snapshot: self.snapshot.clone(),
        }
    }

    fn extent(&self) -> Extent {
        Extent::Exact(ROWS)
    }

    fn mode(&self) -> UniverseMode {
        UniverseMode::Live
    }

    fn read_cell(&self, row: RowId, _column: ColumnId) -> ValueState<CellValue> {
        ValueState::Ready(CellValue::Int(row.raw() as i64 - 1))
    }

    fn capabilities(&self, _row: RowId, _column: ColumnId) -> CellCapabilities {
        CellCapabilities::default()
    }

    fn drain_invalidation_events(&mut self) -> Vec<DataInvalidationEvent> {
        std::mem::take(&mut self.events)
    }
}

#[test]
fn million_row_listview_keeps_only_bounded_order_window() {
    let column = ColumnId::from_raw(91_001);
    let max_requested = Rc::new(Cell::new(0));
    let source = WindowedSortSource::new(column, max_requested.clone());
    let mut list = ListView::new(source);
    list.add_column_raw("Index", Column::with_id(column, 100.0));

    assert_eq!(list.row_count(), ROWS);
    let mut adapter = list.adapter_mut();
    assert!(adapter.cached_row_ids().is_empty());
    let window = adapter.fetch_order_window(500_000, 41).unwrap();
    assert_eq!(window.row_ids.len(), 41);
    assert_eq!(adapter.cached_order_window().unwrap().row_ids.len(), 41);
    assert_eq!(max_requested.get(), 41);
}

#[test]
fn provider_cycle_replaces_order_and_preserves_row_identity_selection() {
    let column = ColumnId::from_raw(91_002);
    let source = WindowedSortSource::new(column, Rc::new(Cell::new(0)));
    let mut list = ListView::new(source);
    list.add_column_raw("Index", Column::with_id(column, 100.0));
    let core = list.test_core_weak().upgrade().expect("core remains alive");
    let selected = RowId::from_raw(123_457);
    core.borrow_mut().action_select_row(selected);
    let initial_epoch = core.borrow().cms.hts().base_row_order_epoch();

    list.request_sort(SortOperation::CycleReplace(SortKey::Column(column)));
    assert_eq!(
        list.applied_sort().unwrap().spec.terms[0].direction,
        SortDirection::Ascending
    );

    list.request_sort(SortOperation::CycleReplace(SortKey::Column(column)));
    assert_eq!(
        list.applied_sort().unwrap().spec.terms[0].direction,
        SortDirection::Descending
    );
    let first = list.adapter_mut().fetch_order_window(0, 1).unwrap();
    assert_eq!(first.row_ids, vec![RowId::from_raw(ROWS as u64)]);

    list.request_sort(SortOperation::CycleReplace(SortKey::Column(column)));
    assert!(list.applied_sort().unwrap().spec.terms.is_empty());
    assert!(core.borrow().selection.is_selected(selected));
    assert_eq!(
        core.borrow().cms.hts().base_row_order_epoch(),
        initial_epoch + 3
    );
}
