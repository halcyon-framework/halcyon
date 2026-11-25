---
doc-id: halcyon-guide-tabular
class: first-class
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-164
  - SPEC-HAL-179
  - SPEC-HAL-188
  - SPEC-HAL-189
  - SPEC-HAL-259
  - SPEC-HAL-262
---

# Tabular and Hierarchical Views

The substrate/engine/widget stack behind ListView, TreeView, and every
cell-addressed surface. Revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`; two-axis tags per SPEC-HAL-309
§7. Nothing here was taken from the legacy `docs/WIDGET-CATALOG.md`; every
claim below is independently sourced. The dirty working-tree files
(including `src/widgets/mod.rs` and `src/widgets/aligned_order_book.rs`)
are not cited.

## 1. Governing SPEC Family

Statuses as declared at this revision:

- SPEC-HAL-164 (TabularSubstrate root abstraction): `Implemented / Closed`,
  extended by SPEC-HAL-353 (:3, :10). Family 165-178 covers columns,
  header/body domains, horizontal geometry, resize, separators, sort,
  tree projection, cell content, inline editing (174/174A), cell focus
  (175), keyboard authority (176), deferred intents (177 —
  `Implemented / Closed`), selection (178 — `Implemented / Closed`,
  amended by SPEC-HAL-347).
- SPEC-HAL-179/179A (canonical ListView): `Draft (Foundational)` (:3).
  180-187: column system, binding model, reorder, row height, focus
  entry/exit, persistence scope, header/body/footer scroll partition,
  lock-in laws (Draft-class; 186 `Draft (Binding upon adoption)`).
- SPEC-HAL-188/188A (data boundary): `Draft (Binding upon adoption)`,
  amended/extended by SPEC-HAL-353. SPEC-HAL-189 (reuse & dirty law):
  **`Binding`** (:3).
- SPEC-HAL-259 (HierarchicalTabularSubstrate): `Draft (Binding upon
  adoption)`, amended by SPEC-HAL-337 for topology ownership (§4.1) and
  consumer mutation law (§8) (:3-4).
- SPEC-HAL-262 (TreeView surface semantics & parity): `Draft (Binding upon
  adoption)`, peer of ListView, last synced 2026-07-12 owner correction
  (:5-9).

## 2. The Extracted Engines (SPEC-HAL-255 family)

`src/engines/` — "Engines own invariants and resolve logic; they have no
widget, layout, or rendering dependencies"
[implementation: implemented — `src/engines/mod.rs:1-17`].

| Engine | Owns | Forbidden | Evidence |
|---|---|---|---|
| `column_geometry` (SPEC-HAL-255/168) | Exclusive horizontal geometry resolution; epoch-versioned, once per layout pass; stale reads panic in debug | Domains resolving geometry; scroll influencing geometry | `src/engines/column_geometry/mod.rs:8-17` (INVARIANT-1..7) |
| `row_topology` (SPEC-HAL-256) | Row identity ordering, logical index resolution, sorted index storage/versioning, topology dirty gating | Column geometry; mutating other engines | `src/engines/row_topology/mod.rs:1-25` |
| `virtualization` (SPEC-HAL-257) | Visible row windowing, viewport→row mapping, realization windows | Geometry, selection, scroll ownership, ordering decisions | `src/engines/virtualization/mod.rs:1-25` |
| `hierarchical` (HTS, SPEC-HAL-259) | Orchestration only, phase order **Topology → Geometry → Virtualization → Realization**; collapsed subtree = one visible block | Any engine mutating another (INV-HTS-001) | `src/engines/hierarchical/mod.rs:1-25` (INV-HTS-001..005) |
| `cell_mechanics` (CMS) | Cell layer atop HTS: realized-cell cache, cell-paint dispatch, editor reconciliation, edit lifecycle, cell hit-test/focus, permissions, validation, width measurement — "composition over subsumption: CMS owns an HTS field" | — | `src/engines/cell_mechanics/mod.rs:1-20` |
| `cell_source` | Universal cell access: Column = schema, Row = identity+structure (widget-specific), Cell = value+editable (universal); `row_archetype` distinguishes Data vs Band rows | — | `src/engines/cell_source/mod.rs:1-20` |
| `route_geometry`, `temporal` | Route/temporal substrates (SPEC-HAL-338, 334-336 lineage) | — | `src/engines/mod.rs:10-17` |

## 3. TabularSubstrate — the Orchestrator

`src/widgets/tabular_view/mod.rs:346-475`
[implementation: implemented; 151 members total, first 40 verified]:

- Structure: `column_model`, `geometry_table`, `header: HeaderDomain`,
  `body: BodyDomain`, `row_expansion`.
- Interaction state: `sort_state` (+ `comparator` and deferred
  `pending_comparator` — the SPEC-HAL-174A shape), `focus_state`,
  `edit_state`, `selection_state`, `ordered_selection_state`.
- Enforcement: `evaluation_guard`, `column_guard`, `row_guard`,
  `sort_guard` (`EvaluationPhaseGuard` etc. — guard types live in
  `src/widgets/tabular_view/guards.rs`).
- Intents: `pending_intents: Vec<TabularIntent>`, `on_intent` observer
  (SPEC-HAL-177).
- Engines are plugged in as trait objects:
  `row_topology: Box<dyn TopologyEngine>`,
  `virtualization: Box<dyn VirtualizationStrategy>`, with
  `engine_invalidation`, `visible_range: Option<VisibleRangeToken>`,
  `scroll_epoch`, `evaluation_mode`, and an `is_listview_embedded` flag.
- Consumer construction: `TabularSubstrate::new()` /
  `with_columns(...)`, `with_header_height`, `with_row_model`,
  `with_on_intent`, `with_comparator`; column management
  (`add_column`, `set_column_visible`, `set_column_width`,
  `mark_geometry_dirty`, …) [implementation: implemented —
  `src/widgets/tabular_view/mod.rs:480-641`].

Supporting files: `data_adapter.rs` (the SPEC-HAL-188 `DataSource` trait —
see `data-and-update-patterns.md` §4), `geometry.rs`, `header.rs`,
`body.rs`, `sort.rs`, `intents.rs`, `intent_policy.rs`, `dirty.rs`,
`guards.rs`, `cell_content.rs`, `cell_lifecycle.rs`, `separators.rs`,
`tree_paint.rs`, `view_config.rs`, `column_config.rs`,
`expandable_row_host.rs` (file inventory, `src/widgets/tabular_view/`).

## 4. ListView — the Row-Centric Composite

Identity: SPEC-HAL-179 defines ListView as row-centric with TabularSubstrate
as semantic authority [authority: draft-foundational — SPEC-HAL-179:3-8].

Live composition (`src/widgets/list_view/`): `list_view.rs` (the widget),
`core.rs` (`ListViewCore` shared state), `body.rs` (cell painting inside
the scroll viewport), `footer.rs`, `cell_host.rs`, `row_host.rs`,
`cell_painter.rs` (pluggable `CellPainter`), `actions.rs`,
`column_config_overlay.rs` [implementation: implemented — file inventory].

`ListViewCore` (`src/widgets/list_view/core.rs:111-220`, 116 members at
this revision, `inspect ListViewCore facets members` complete with zero
omissions) is the composite's single shared state:

- Substrate/data: `cms: CellMechanicsSubstrate` (the §2 CMS engine),
  `adapter: DataAdapter<S>`, `row_ids`, `row_count`, `row_heights`,
  `realized_cell_values` cache keyed `(row_id, ColumnId)`.
- Selection: `selection: OrderedSelectionState<RowId>` — exactly the
  SPEC-HAL-347 amendment recorded in SPEC-HAL-178: one selection state, in
  `ListViewCore`, mutated only through the shared reducer
  [authority: SPEC-HAL-178:10 (amendment note) |
  implementation: implemented — `:135`].
- Scroll partition: `body_widget_id`,
  `horizontal_scroll_link: Option<HorizontalScrollLink>`,
  `horizontal_scroll_freeze: Option<Rc<Cell<bool>>>`,
  `requested_scroll_y`, `viewport_height` — header and footer are outside
  the ScrollArea, the body is inside; the link carries the horizontal
  offset to header/footer each frame, and the freeze flag suspends
  horizontal scroll during column resize (SPEC-HAL-186's partition law,
  `Draft (Binding upon adoption)`) [implementation: implemented —
  `:144-165`].
- Interaction: `focused_column`, `focused`, `selection_mode`,
  `hovered_separator`, `active_resize_separator`, `dragged_column_id` +
  `drag_visual_offsets` + `ghost_occlude_range` (column reorder,
  SPEC-HAL-182 family), `intent_policy: TabularIntentPolicy`.
- Persistence: `view_id`, `app_namespace`,
  `config_store: Option<Box<dyn ViewConfigStore>>`,
  `default_column_config` (SPEC-HAL-185 persistence scope).

Selection presentation invalidation (0.189.3-beta): a routed selection or
focused-row change must repaint the exact retained `ListViewBody` paint
fragment in the same event boundary, and unchanged geometry does not
authorize layout invalidation — INV-347-PRESENTATION-1
[authority: binding (`Active / Binding`, SPEC-HAL-347:3) —
SPEC-HAL-347:93; SPEC-HAL-354:354-356]. Live mechanism: intent dispatch
reports whether selection changed
(`dispatch_intents_and_focus` returns `ListViewDispatchCallbacks` with
`presentation_changed`, `src/widgets/list_view/core.rs:1002-1051`,
`:1268-1279`); pointer paths repaint via `cx.invalidate_paint()` and the
keyboard path via `invalidate_paint_by_id(body_widget_id)`
(`src/widgets/list_view/body.rs:1202-1206` hover;
`src/widgets/list_view/list_view.rs:822-826`)
[implementation: implemented]. Executable evidence:
`retained_listview_pointer_and_keyboard_selection_repaint_body_without_layout`
(`src/tests/listview_dynamic_invalidation.rs:809`) — PASS in the
clean-checkout run at this revision recorded in
`audits/implementation-status.md` §3; that file also carries two
pre-existing failures on an unrelated paint-phase path, recorded there.

Cell-read hot path (traced 2026-07-24): `DataSource::read_cell` runs
once per visible row × body-slot column per **layout** evaluation —
`ListViewBody::layout` resolves geometry, computes the visible window,
and calls `realize_visible_cells`, which clears and refills the
`realized_cell_values` cache and the CMS cell store
(`src/widgets/list_view/body.rs:391-397`,
`src/widgets/list_view/core.rs:961-985`). **Paint performs zero
`read_cell` calls** — it consumes the caches only
(`body.rs:739-751` via `cms.realized_cell_value` /
`core.realized_cell_value_state`, `core.rs:987-996`). Consequence:
paint-only invalidation (selection, hover, focus) never touches the
provider; a viewport-changing scroll re-runs layout and therefore
re-reads exactly the visible window (SPEC-HAL-354 §6: ListView
viewport change ⇒ layout+paint) [implementation: implemented].

## 5. TreeView and the Hierarchical Stack

TreeView is a peer of ListView composed over HTS + column geometry +
selection engines [authority: SPEC-HAL-262:5-8 (`Draft (Binding upon
adoption)`)]. Live shape: `src/widgets/tree_view/{tree_view,body,footer,mod}.rs`
[implementation: implemented — file inventory]. Topology mutations funnel
as intents (SPEC-HAL-337 amendment to 259 §4.1/§8: consumers do not mutate
topology directly; the shared topology substrate owns it)
[authority: SPEC-HAL-259:3-4]. Tree-over-table projection semantics are
SPEC-HAL-172; hierarchical row math lives in `engines/hierarchical`
(collapsed subtree = exactly one visible block, INV-HTS-003).

## 6. Distinctions That Must Not Blur

- **Geometry vs presentation vs data vs intent.** Column geometry: engine-
  owned, epoch-versioned (§2). Presentation state (hover, resize handles,
  drag ghosts): composite core state (§4). Consumer data: behind
  `DataSource`, provider-authoritative order revisions (§3 supporting
  files; `data-and-update-patterns.md` §4). Interaction intent: typed
  `TabularIntent` queue, drained at the deferred-mutation boundary
  (SPEC-HAL-177, `input-events-focus-and-actions.md` §7).
- **Interaction policy is per-widget.** Policies are declared by each
  widget's governing SPEC (Display/Input/Interactive); nothing here
  generalizes a policy across widgets (PRE-FLIGHT.md §B2 discipline).
- **Reuse law.** Row/cell identity, reuse, and dirty discipline for every
  substrate consumer is SPEC-HAL-189 (`Binding`); cache-eviction specifics
  are explicitly out of its scope (SPEC-HAL-189:3-8).

## 7. Related Catalog Provenance

The complete per-widget catalog exists (`widget-catalog.md`, including
`aligned_order_book` and the other market-detail composites) but is
verified against the working tree — its base commit plus the four
still-uncommitted market-detail files, on owner order; it re-pins to a
clean revision when that unit commits (see `index.md` registry). Nothing
in this guide depends on those files.
