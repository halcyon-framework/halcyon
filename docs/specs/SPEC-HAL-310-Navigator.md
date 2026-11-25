# SPEC-HAL-310: **Navigator — Multi-View Content Browser**

### *Configurable Data Browser with View Modes and Hierarchical Navigation*

**Author:** JC
**Status:** Draft (v3)
**Depends on:** SPEC-HAL-002 (Constitution), SPEC-HAL-188 (DataSource),
SPEC-HAL-086 (ScrollArea), SPEC-HAL-135 (Selection),
SPEC-HAL-136 (Transactional Selection), SPEC-HAL-228 (NavigationHost),
SPEC-HAL-229 (Projection Binding), SPEC-HAL-231 (Navigation Guard),
SPEC-HAL-237 (Multiroot Routing), SPEC-HAL-247 (NavigationRootRegistry)

---

# 1. Purpose

Navigator is a configurable content browsing widget. At
minimum, it's a flat list of items in a single view mode.
At maximum, it's a multi-view data browser with hierarchical
navigation integrated through NavigationHost.

The widget scales from simple to complex via configuration,
not subclassing:

| Complexity | What |
|---|---|
| Minimal | Items in a single view mode (e.g., icon grid) |
| + view modes | Switch between Details / Icons / Tiles / etc. |
| + navigation | Double-click drills in, structural Up returns to the parent |
| + integration | NavigationHost coordination with external controls |

Navigator owns the **content projection**. Everything else —
tree sidebars, breadcrumbs, search bars, status strips — is
composed AROUND Navigator by the consumer.

---

# 2. Architectural Position

```
NavigationHost (SPEC-HAL-228 — framework core)
    ↕ application-owned projection + intent routing
Application / DataLayer location resolver
    └── NavigationEntry -> location-scoped NavigatorDataSource
Navigator (this SPEC — content projection widget)
    ├── policy: NavigatorPolicy (struct — composable)
    ├── data: Rc<RefCell<dyn NavigatorDataSource>>
    ├── painter: Rc<dyn CardPainter> (for grid-based modes)
    ├── selection: OrderedSelectionState<RowId>
    └── navigation: Option<NavigatorNavigation>

    ↕ caller-provided intent sinks (same application routing seam)
Consumer's sidebar / breadcrumbs / search (composed AROUND)
```

**Two layers of navigation:**

- **NavigationHost** — context-level. Owns committed path/history
  and resolves context-local navigation intents.
  Framework infrastructure owned by App. Navigator does
  NOT own NavigationHost.
- **Navigator** — content-level. Browse items, drill into
  containers, switch view modes. A widget that participates
  through caller-provided navigation projection and intent sinks.

Navigator emits `NavigationIntent::NavigateTo(entry)` when
the user drills in. The application drains the sink, routes the
intent to the explicitly bound navigation context, and projects
the resulting committed location and its DataLayer-resolved
`NavigatorDataSource` back into Navigator. Navigator replaces its
active content projection and requests bounded row-order windows
from that source.

External controls (tree sidebar, breadcrumbs) emit the same
intents through their own caller-bound sinks. The application
routes all of them through the same bound navigation root and its
NavigationHost. Navigator doesn't know or care who initiated the
navigation.

---

# 3. What Navigator Is NOT

- **Not NavigationHost.** Navigator holds projected location data
  and emits intents. It never holds, discovers, or calls a
  NavigationHost. NavigationHost owns navigation state.
- **Not FlowGrid.** FlowGrid is a CSS flex-wrap layout
  primitive. No virtualization, no identity, no selection.
- **Not a spreadsheet.** No cell editing. Details mode uses
  ListView internally, which has column headers.
- **Not a layout container.** Items in grid-based modes are
  delegate-painted, not widget children.
- **Not a file manager.** Navigator doesn't know what
  "files" or "folders" are. The consumer's DataSource
  defines item data and navigable containers; the application/DataLayer
  resolves each committed location to the source that owns its row universe.
- **Not opinionated about visual style.** Navigator does not
  impose card chrome (backgrounds, borders, corner radii).
  Visual item styling is entirely the CardPainter's concern.

---

# 4. View Modes

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NavigatorViewMode {
    /// Columnar table with headers: Name, Date, Type, Size.
    /// Uses ListView internally.
    Details,
    /// Compact icon + name, vertical-first multi-column flow.
    /// Uses List (basic virtualizing list) internally.
    List,
    /// Small icon centered, name below. Grid flow.
    SmallIcon,
    /// Medium icon centered, name below. Grid flow.
    Icon,
    /// Large icon/preview centered, name below. Grid flow.
    LargeIcon,
    /// Extra large icon/preview, name below. Grid flow.
    ExtraLargeIcon,
    /// Wide card: icon left, name + metadata lines right.
    Tile,
    /// Full-width row: icon left, name + type + date + size
    /// stacked right. Richest glanceable mode before Details.
    Content,
}
```

Each mode is a different layout strategy over the SAME
DataSource with the SAME selection state. The consumer
switches mode at runtime — Navigator rebuilds its internal
content widget. Selection transfers.

### 4.1 Mode Implementation

| Mode | Internal Widget | Layout |
|---|---|---|
| Details | **ListView** (existing) | Columnar rows with headers |
| List | **List** (basic) | Compact rows, column-first flow |
| SmallIcon | NavigatorBody | Grid, ~80×80 cells |
| Icon | NavigatorBody | Grid, ~100×96 cells |
| LargeIcon | NavigatorBody | Grid, ~128×120 cells |
| ExtraLargeIcon | NavigatorBody | Grid, ~256×220 cells |
| Tile | NavigatorBody | Grid, wide ~240×60 cards |
| Content | NavigatorBody | Full-width rows, ~height 64 |

Details delegates to the existing ListView widget. List
delegates to the existing List widget. All other modes use
NavigatorBody (the current grid-based body) with different
item dimensions.

### 4.2 Config Presets

```rust
impl NavigatorConfig {
    /// Columnar table — delegates to ListView.
    pub fn details() -> Self { ... }

    /// Compact icon + name, column-first flow — delegates to List.
    pub fn list() -> Self { ... }

    /// Small icons ~80×80, 8px gaps.
    pub fn small_icon() -> Self { ... }

    /// Medium icons ~100×96, 12px gaps.
    pub fn icon() -> Self { ... }

    /// Large icons ~128×120, 12px gaps.
    pub fn large_icon() -> Self { ... }

    /// Extra large icons/previews ~256×220, 16px gaps.
    pub fn extra_large_icon() -> Self { ... }

    /// Wide tile — ~240×60, icon left, text right, 8px gaps.
    pub fn tile() -> Self { ... }

    /// Content row — full width, ~64px height, icon left,
    /// name + type + date + size stacked right.
    pub fn content() -> Self { ... }
}
```

---

# 5. Policy

Navigator's interaction surface is governed by a composable
policy struct, following the `TabularIntentPolicy` pattern.
Each axis is independently configurable.

```rust
pub struct NavigatorPolicy {
    /// Which view modes are available. Consumer controls this.
    /// Navigator only allows switching to modes in this set.
    /// If only one mode is listed, mode switching is disabled.
    pub allowed_modes: Vec<NavigatorViewMode>,

    /// Default view mode. Must be in allowed_modes.
    pub default_mode: NavigatorViewMode,

    /// Selection cardinality.
    pub selection_mode: SelectionMode,

    /// Immediate (mutate on input) vs Transactional (working
    /// set, requires explicit commit).
    pub interaction_model: SelectionInteractionModel,

    /// When activation callbacks fire.
    pub activation_mode: ActivationMode,

    /// Whether Shift+click/Shift+nav performs range selection.
    pub range_selection: RangePolicy,

    /// Whether hierarchical navigation is enabled.
    /// When true: double-click/Enter on navigable items drills
    /// in, Escape with empty selection navigates up.
    /// When false: double-click/Enter only emits ItemActivated.
    pub navigation_enabled: bool,
}
```

### 5.1 Policy Presets

```rust
impl NavigatorPolicy {
    /// Display only. No selection, no keyboard nav.
    pub fn display_only() -> Self {
        Self {
            allowed_modes: vec![NavigatorViewMode::Icon],
            default_mode: NavigatorViewMode::Icon,
            selection_mode: SelectionMode::None,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::ExplicitOnly,
            range_selection: RangePolicy::Disabled,
            navigation_enabled: false,
        }
    }

    /// Single-select browser. Click selects, double-click activates.
    pub fn single_select() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::Icon,
                NavigatorViewMode::Tile,
            ],
            default_mode: NavigatorViewMode::Details,
            selection_mode: SelectionMode::Single,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::Disabled,
            navigation_enabled: false,
        }
    }

    /// Multi-select browser. Ctrl/Shift for multi-select,
    /// double-click activates.
    pub fn multi_select() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::Icon,
                NavigatorViewMode::Tile,
                NavigatorViewMode::LargeIcon,
            ],
            default_mode: NavigatorViewMode::Details,
            selection_mode: SelectionMode::Multi,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::ShiftRange,
            navigation_enabled: false,
        }
    }

    /// File browser. Multi-select, hierarchical navigation,
    /// all view modes.
    pub fn file_browser() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::List,
                NavigatorViewMode::SmallIcon,
                NavigatorViewMode::Icon,
                NavigatorViewMode::LargeIcon,
                NavigatorViewMode::ExtraLargeIcon,
                NavigatorViewMode::Tile,
                NavigatorViewMode::Content,
            ],
            default_mode: NavigatorViewMode::Details,
            selection_mode: SelectionMode::Multi,
            interaction_model: SelectionInteractionModel::Immediate,
            activation_mode: ActivationMode::OnDoubleClick,
            range_selection: RangePolicy::ShiftRange,
            navigation_enabled: true,
        }
    }

    /// Asset picker dialog. Transactional selection — user
    /// browses and selects, then commits via OK button.
    pub fn asset_picker() -> Self {
        Self {
            allowed_modes: vec![
                NavigatorViewMode::Details,
                NavigatorViewMode::Icon,
                NavigatorViewMode::Tile,
            ],
            default_mode: NavigatorViewMode::Icon,
            selection_mode: SelectionMode::Multi,
            interaction_model: SelectionInteractionModel::Transactional,
            activation_mode: ActivationMode::ExplicitOnly,
            range_selection: RangePolicy::ShiftRange,
            navigation_enabled: true,
        }
    }
}
```

---

# 6. Data Model

```rust
pub struct NavigatorConfig {
    /// Item width in logical pixels (grid-based modes).
    pub item_width: f32,
    /// Item height in logical pixels (grid-based modes).
    pub item_height: f32,
    /// Horizontal gap between items.
    pub gap_x: f32,
    /// Vertical gap between item rows.
    pub gap_y: f32,
    /// Horizontal alignment when items don't fill the row.
    pub alignment: GridAlignment,
    /// Opt-in fluid fill (grid-based modes). When true, item
    /// width stretches to fill the row content width per
    /// INV-310-13; supersedes `alignment`. Default false.
    pub fill_width: bool,
    /// Active view mode.
    pub view_mode: NavigatorViewMode,
}
```

Navigator does NOT specify visual chrome (backgrounds, borders,
corner radii). The CardPainter is responsible for all item
visuals. Navigator provides the item rect and interaction
state (selected, hovered, focused). The painter decides how
to render.

---

# 7. NavigatorDataSource

Extends the FROZEN `DataSource` trait with one concept: an
item can be a navigable container. Location-to-source resolution
does not add a second row-universe API to this trait.

```rust
pub trait NavigatorDataSource: DataSource {
    /// If this item is a navigable container, return its
    /// semantic NavigationEntry. Items with children can be drilled
    /// into via double-click or Enter.
    ///
    /// Default: None (flat collection, no hierarchy).
    fn navigation_entry(&self, row: RowId) -> Option<NavigationEntry> {
        None
    }
}

pub type NavigatorDataSourceHandle = Rc<RefCell<dyn NavigatorDataSource>>;
```

**Flat usage:** Do not override `navigation_entry`. Navigator
behaves as a flat item browser over the source supplied at construction.

**Hierarchical usage:** `navigation_entry` maps container items to
semantic locations. After NavigationHost commits one of those locations,
the application/DataLayer resolves that `NavigationEntry` to a
location-scoped `NavigatorDataSourceHandle` and supplies both as one
`NavigatorLocationProjection`. The source exposes its provider-authoritative
order through SPEC-HAL-188 `order_revision`, bounded
`order_window(OrderWindowRequest) -> ValueState<OrderWindow>`, `locate_row`,
and `extent` operations. A source
may be dedicated to one location or may represent a provider query whose
location scope is managed by the DataLayer; Navigator does not decide that
storage strategy.

Source resolution is total for every committed `NavigationEntry`: it returns a
handle immediately, while pending, denied, missing, and failed data remain
ordinary source testimony under SPEC-HAL-188. If an application cannot map an
entry to any source, that destination is not commit-ready and must be rejected
by route/guard policy. After a commit, retaining the prior location's source as
a fallback is forbidden.

All view modes consume the same active source and order revision. Details
and List use adapters over that source; grid-based modes use the same bounded
row-window transport directly. No mode may treat the compatibility
full-RowId snapshot path as canonical or accumulate a complete RowId universe.

`DataSource` is FROZEN. `NavigatorDataSource` is a new trait
that requires `DataSource`. Existing DataSource impls work
unchanged (just add an empty `impl NavigatorDataSource for
MySource {}`).

---

# 8. Navigation Integration

### 8.1 NavigatorNavigation

Optional. Navigator works without it (flat mode).
Only active when `policy.navigation_enabled == true`.

```rust
pub type NavigatorNavigationSink = Rc<RefCell<VecDeque<NavigationIntent>>>;

#[derive(Clone)]
pub struct NavigatorLocationProjection {
    /// NavigationHost-committed semantic location.
    current: NavigationEntry,
    /// DataLayer-resolved row universe for that location.
    source: NavigatorDataSourceHandle,
}

impl NavigatorLocationProjection {
    pub fn new(current: NavigationEntry, source: NavigatorDataSourceHandle) -> Self {
        Self { current, source }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum NavigatorContentGate {
    Open,
    Closed,
}

/// Stable state shared by every Details, List, and grid adapter.
struct NavigatorSharedSourceBinding {
    active_source: NavigatorDataSourceHandle,
    gate: NavigatorContentGate,
}

impl NavigatorSharedSourceBinding {
    fn close(&mut self) {
        self.gate = NavigatorContentGate::Closed;
    }

    fn replace_while_closed(&mut self, source: NavigatorDataSourceHandle) {
        assert_eq!(self.gate, NavigatorContentGate::Closed);
        self.active_source = source;
    }

    fn open(&mut self) {
        self.gate = NavigatorContentGate::Open;
    }

    fn is_open(&self) -> bool {
        self.gate == NavigatorContentGate::Open
    }
}

pub struct NavigatorNavigation {
    /// Caller-projected committed location and its resolved row universe.
    projection: NavigatorLocationProjection,
    /// Caller-provided sink bound outside the widget to one navigation context.
    intent_sink: NavigatorNavigationSink,
    /// True until update() has installed this projection in every view mode.
    /// While true, the shared content gate is closed to painting and input.
    projection_pending: bool,
}

impl NavigatorNavigation {
    pub fn new(
        projection: NavigatorLocationProjection,
        intent_sink: NavigatorNavigationSink,
    ) -> Self {
        Self {
            projection,
            intent_sink,
            projection_pending: true,
        }
    }
}

pub fn navigator_navigation_sink() -> NavigatorNavigationSink {
    Rc::new(RefCell::new(VecDeque::new()))
}

impl Navigator {
    pub fn set_navigation_projection(&mut self, projection: NavigatorLocationProjection) {
        if let Some(navigation) = self.navigation.as_ref() {
            let changed = navigation.projection.current != projection.current
                || !Rc::ptr_eq(&navigation.projection.source, &projection.source);
            if changed {
                self.shared_source.borrow_mut().close();
                let navigation = self.navigation.as_mut().unwrap();
                navigation.projection = projection;
                navigation.projection_pending = true;
            }
        }
    }

    pub fn replace_navigation_binding(&mut self, mut binding: NavigatorNavigation) {
        if let Some(old) = self.navigation.as_ref() {
            old.intent_sink.borrow_mut().clear();
        }
        self.shared_source.borrow_mut().close();
        binding.projection_pending = true;
        self.navigation = Some(binding);
    }

    pub fn clear_navigation_binding(&mut self) {
        if let Some(old) = self.navigation.take() {
            old.intent_sink.borrow_mut().clear();
        }
        self.shared_source.borrow_mut().open();
    }
}
```

`Navigator` owns exactly one `Rc<RefCell<NavigatorSharedSourceBinding>>`; every
Details, List, and grid adapter holds that same handle. Construction installs the
flat source with an open gate. No adapter may retain a source handle outside this
binding or read rows while the gate is closed. `with_navigation(binding)`
delegates to `replace_navigation_binding(binding)` before the Navigator is
exposed, so initial pending state cannot exist with an open shared gate.

Navigator receives only the committed semantic location, its DataLayer-resolved source, and an intent sink. The caller invokes `set_navigation_projection` only after the bound NavigationHost commits a different location or the DataLayer resolves a different source. Provider invalidation handles order changes within an unchanged projection. `set_navigation_projection` updates content within the existing context binding; it is not a context rebind. A context change must use `replace_navigation_binding`, which atomically replaces the projected location, source, and sink and forces hydration even when the new location and source identities equal the old values. `NavigatorNavigation::new` also starts pending, so initial installation cannot be skipped. Closing `shared_source.gate` occurs before the pending projection is stored and before the setter or binding replacement returns: every view mode stops painting prior-source rows and rejects row-targeted pointer, keyboard, selection, activation, and navigation-intent production until `update()` installs the new projection. `clear_navigation_binding` takes and discards the pending projection plus every undrained intent, then opens the gate on the last source that completed installation. If clear occurs while a new projection is pending, that never-installed source is discarded and the previously active source intentionally resumes as flat content; no adapter can observe the pending handle.

Navigation commit delivery and `set_navigation_projection`/`replace_navigation_binding` execute in the same serialized UI mutation transaction. The application must resolve the total location-to-source projection and close Navigator's content gate before event routing, layout, paint, or another frame can observe the committed host location. A caller that cannot provide that immediate handle must reject the candidate through route/guard policy before commit; it must not commit first and defer quarantine to a later task.

Each sink is dedicated to one Navigator binding instance and must not be shared with another control, binding, or context. Replacement/clear discards every undrained old intent so no stale command can reach the prior context. The sink binding identifies the target context outside the widget; Navigator holds no host, root registry, or location resolver reference.

### 8.2 Internal Navigation (double-click / Enter)

When the user double-clicks or presses Enter on a navigable
item (and `policy.navigation_enabled == true`):

```rust
if let Some(entry) = data.navigation_entry(row_id) {
    nav.intent_sink
        .borrow_mut()
        .push_back(NavigationIntent::NavigateTo(entry));
}
```

### 8.3 External Navigation

An external control emits `NavigationIntent::NavigateTo(...)`
through its own caller-provided sink. The application drains the
sink. A standalone single-root consumer may deliver the intent to
its explicitly bound host/orchestrator directly. A multiroot consumer
submits the bound interaction-domain/root identity plus intent through
SPEC-HAL-237 routing validation, after which NavigationRootRegistry
dispatches only the resulting explicit root target. After the owning
NavigationHost commits, the caller updates Navigator's projected
location/source pair and Navigator replaces its content projection. There
is no coupling between Navigator and the external control.

### 8.4 Structural Up Navigation

Escape (when nothing is selected and navigation enabled)
emits `NavigationIntent::NavigateUp`. NavigationHost resolves
the structural parent as a new committed state. This is distinct
from `HistoryBack`; Navigator does not reinterpret Up as history
traversal. After the host commits, the caller resolves and supplies the new
location/source projection.

### 8.5 Pending Projection Application

During `update()`, Navigator consumes a pending projection exactly once and
performs one atomic content-projection replacement:

1. verify that `shared_source.gate` is closed and invalidate every prior-source realized row,
2. call `replace_while_closed(projected_source)` so the shared binding names the projected `NavigatorDataSourceHandle` as the sole active source for every view mode while access remains closed,
3. discard all bounded order-window and revision caches from the previous source,
4. rebind the shared source indirection observed by the Details, List, and grid adapters so they all observe the new source and its current `order_revision`, without mutating widget-tree structure,
5. clear selected RowIds, focus, persistent/range anchors, range-session state, and transactional working/committed selection while preserving configured selection policy,
6. clear hover, press/double-click, drag, reveal, and other item-local transient state, then reset scroll to the start,
7. request only the bounded SPEC-HAL-188 order windows needed by the active viewport and set `projection_pending = false`,
8. call `shared_source.open()` only after steps 1–7 complete; pending, denied, error, or missing window testimony renders the new projection's normal non-row state, and the prior source is no longer reachable through the shared binding.

No row window from the prior source or revision may survive the replacement.
The semantic `NavigationEntry` is retained as projection testimony; it is never
passed into a second `items_at`-style enumeration path. A failed or pending
provider window leaves the new projection authoritative and renders the normal
pending/empty state; Navigator must not fall back to the prior location's rows.

### 8.6 Runtime Migration Note (0.160.0-beta)

The live `NavigatorNavigation` still stores `Rc<RefCell<NavigationHost>>`, and Navigator reads and invokes that host directly. Live navigation locations are `NavigationNodeId`, and the runtime `NavigatorEvent::Navigated` is emitted only after direct host application. Live construction also snapshots the complete `source.row_ids()`, and layout/invalidation replace Navigator's complete `Vec<RowId>` from that compatibility path. The runtime therefore cannot represent a location-scoped, remote, unknown, streaming, or million-row provider through bounded order windows. An observed-location comparison initialized to the current value would also skip initial hydration; target construction and whole-binding replacement must explicitly set `projection_pending = true`.

That implementation does not satisfy INV-310-7/8/14/15 or FORBID-310-4/5/7. Phase 11 must migrate locations end-to-end to semantic `NavigationEntry`, replace the retained host reference with caller-projected location/source data plus a caller-provided intent sink, remove `Navigated` as a second command/result seam, replace full RowId materialization with the SPEC-HAL-188 revisioned bounded-window path in every mode, and provide whole-binding replace/clear operations that discard stale undrained intents. Standalone single-root consumers retain SPEC-HAL-247's direct bound-host/orchestrator path; multiroot consumers route drained intents through the SPEC-HAL-237/247 domain/root seam before host execution.

---

# 9. CardPainter

Navigator does not paint item visuals. A consumer-provided
painter handles all item content and styling:

```rust
pub trait CardPainter {
    fn paint_card(
        &self,
        cx: &mut PaintContext,
        rect: RectF,
        row_id: RowId,
        selected: bool,
        hovered: bool,
        data: &dyn DataSource,
    );
}
```

Navigator paints interaction overlays only: selection tint,
hover highlight, focus ring. The CardPainter paints everything
else — background, icon, text, borders, whatever the consumer
wants.

Active in all grid-based modes (SmallIcon, Icon, LargeIcon,
ExtraLargeIcon, Tile, Content). Ignored in Details mode
(ListView has its own CellPainter) and List mode (List has
its own rendering).

### 9.1 Built-In IconCardPainter (Future)

Standard icon + label layout for the common case:

```rust
pub struct IconCardPainter {
    icon_fn: Box<dyn Fn(RowId, &dyn DataSource) -> Option<(IconId, Color)>>,
    name_column: ColumnId,
    detail_column: Option<ColumnId>,
    icon_size: f32,
    max_label_lines: u8,
}
```

Paints icon centered at top, name centered below, optional
detail line. Future addition — not in initial implementation.

---

# 10. Selection

Reuses `OrderedSelectionState` from `src/core/ordered_selection/`.

Selection behavior is governed by `NavigatorPolicy`:
- `selection_mode` controls cardinality (None / Single / Multi)
- `interaction_model` controls commit semantics (Immediate / Transactional)
- `range_selection` controls Shift+click/Shift+nav behavior
- `activation_mode` controls when items are "activated"

Selection is shared across view modes via the same
`OrderedSelectionState<RowId>`. Switching modes preserves
selection. Applying a location/source projection clears selected and
committed RowIds, focus, anchors, and any range/transaction session before
the new source becomes interactive.

For mode switching with an external widget (e.g., consumer
has a separate ListView showing the same data), selection
can be shared via `Rc<RefCell<OrderedSelectionState>>`.

---

# 11. Keyboard Navigation

| Key | Action |
|-----|--------|
| ← | Move cursor left (clamp at start) |
| → | Move cursor right (clamp at end) |
| ↑ | Move cursor up (same column, previous row) |
| ↓ | Move cursor down (same column, next row) |
| h/j/k/l | Vim: left/down/up/right (clamp, no wrap) |
| Home | First item |
| End | Last item only when `Extent::Exact(n)` proves a non-empty finite end; otherwise unavailable |
| PageUp/Down | Scroll by viewport height |
| Enter | Activate item per policy. If navigable + navigation enabled → drill in |
| Space | Toggle selection on cursor item |
| Ctrl+A | Select all when Multi mode has an explicit finite enumerable selection oracle; unavailable for windowed, unknown, or streaming sources without one |
| Escape | Clear selection. If navigation enabled and selection already clear → navigate up |

Home requests ordinal zero in the current `OrderRevision`; it moves only after
that bounded order window is `Ready`. End is available only for
`Extent::Exact(n)` with `n > 0`, in which case it requests the window containing
ordinal `n - 1`. End is unavailable for `LowerBound`, `UpperBound`, `Bounded`,
`Unknown`, and `Streaming` extents because none proves a last row. An exact
empty extent has no movement target.

Arrow, Vim, PageUp/PageDown, Home, End, auto-reveal, and range-extension
operations may request `locate_row` and bounded order windows. If any required
result is `Pending`, `Denied`, `Error`, `Missing`, stale, or otherwise not
`Ready` for the current revision, the intent is not accepted: focus, selection,
anchors, scroll target, and all corresponding versions remain unchanged. The
intent may be retried after provider invalidation. PageDown may discover later
windows for unknown or streaming extents, but it must not synthesize a final
ordinal or retain a partial movement while discovery is pending.

Cursor movement auto-reveals with breathing room
(`reveal_margin = gap_y`).

Keyboard navigation is disabled when
`policy.selection_mode == SelectionMode::None`.

---

# 12. Events

```rust
pub enum NavigatorEvent {
    /// Single click on an item.
    ItemClicked(RowId),
    /// Double click on an item.
    ItemDoubleClicked(RowId),
    /// Item activated (per activation_mode policy).
    ItemActivated(RowId),
    /// Selection set changed.
    SelectionChanged,
    /// View mode changed.
    ViewModeChanged(NavigatorViewMode),
}
```

Events are observations only; `NavigatorNavigationSink` is the sole navigation-command output. For one navigation-enabled Enter or double-click activation, Navigator emits exactly one `NavigationIntent` into the sink. It may also emit the ordinary `ItemActivated` or `ItemDoubleClicked` observation, but consumers must not translate that observation into a second navigation command.

---

# 13. Architecture (Internal)

```
Navigator (container — owns config, policy, navigation, mode)
  ├─ shared source binding + revision-scoped bounded-window adapters
  └─ ScrollArea (vertical only)
       └─ NavigatorBody (grid-based modes)
       └─ List (List mode — basic virtualizing list)
       └─ ListView (Details mode — columnar table)
```

Same Container + ScrollArea + Body pattern as OrderBook and
the current implementation. `Rc<RefCell<NavigatorState>>`
shared between container and body.

The shared source binding and its content gate are stable widget state. Applying a navigation
projection closes the gate, swaps the handle behind that binding while closed, invalidates adapter caches, and reopens only after atomic installation;
it does not replace widget-tree structure. Details, List, and NavigatorBody
therefore cannot retain different location sources. View-mode changes remain
the only reason this SPEC rebuilds the body subtree.

NavigatorBody handles grid-based modes directly (SmallIcon,
Icon, LargeIcon, ExtraLargeIcon, Tile, Content). For Details
mode, Navigator swaps the body for a ListView child. For
List mode, Navigator swaps for a List child. The ScrollArea
+ body subtree is rebuilt on mode change during the rebuild
  phase (Phase 3 — Constitution §5).

---

# 14. Drag-to-Reorder (Deferred)

Drag-to-reorder is architecturally designed but deferred from
initial implementation. DataSource owns item order. Reorder
requires a round-trip: Navigator emits reorder intent →
consumer requests a DataLayer mutation → the provider publishes a new order
revision → Navigator invalidates old windows and requests bounded windows
for that revision. The interaction model and event
definitions will be specified when this feature is
implemented.

The current implementation retains the drag infrastructure
from the GridView phases (DragState, shift_offsets, ghost
card rendering) but this is provisional and will be revisited
when the reorder feature is formally specified.

---

# 15. Invariants

**INV-310-0:** Navigator uses `NavigatorDataSource` which
extends the FROZEN `DataSource` trait. No modifications to
DataSource.

**INV-310-1:** Virtualized. Only visible items painted.

**INV-310-2:** Item *height* is fixed per config preset. Item
*width* is fixed by default and equals `config.item_width`
exactly. An opt-in fluid mode (`config.fill_width == true`)
stretches item width to fill the row per INV-310-13. When
`fill_width == false` (the default), the fixed-width behavior
is preserved verbatim.

**INV-310-3:** Column count adapts to available width
(grid-based modes).

**INV-310-4:** Selection uses `OrderedSelectionState`.
Governed by policy fields (selection_mode,
interaction_model, range_selection). Shared across view
modes and with external widgets.

**INV-310-5:** CardPainter receives read-only DataSource
during paint. No mutation. Constitution §4.7.

**INV-310-6:** Grid-based modes do NOT use TabularSubstrate.
Details mode delegates to ListView which uses it.

**INV-310-7:** Navigator does NOT own NavigationHost.
Navigator receives projected location data and emits navigation
intents through a caller-provided sink. The consumer (Screen, App,
or shared context) owns context binding, interaction-domain routing,
and committed-state projection.

**INV-310-8:** Initial, changed-location, changed-source, and whole-context
projections are applied in `update()` through an explicit pending flag.
Projection installation clears all location-local selection, focus, anchor,
range, transaction, hover, drag, reveal, and scroll state before new rows are
made interactive. The setter closes the one gate stored in `NavigatorSharedSourceBinding` synchronously before recording the pending projection,
so rows from the prior committed location can neither paint nor receive input
between host commit and projection installation. No data mutation occurs during
paint or layout.

**INV-310-9:** View mode changes rebuild the body subtree
during rebuild phase (Phase 3). Selection persists.
Scroll position resets.

**INV-310-10:** Navigator does NOT impose visual item styling.
No backgrounds, borders, or corner radii in NavigatorConfig.
CardPainter owns all item visuals.

**INV-310-11:** Policy fields are composable and independent.
Each axis (selection mode, interaction model, activation,
range, navigation, allowed modes) is configured separately.

**INV-310-12:** Allowed modes list is enforced. Navigator
rejects `set_view_mode()` calls for modes not in
`policy.allowed_modes`. Single-mode policies disable mode
switching entirely.

**INV-310-13:** Fluid row fill (opt-in). When
`config.fill_width == true` in a grid-based mode, items fill
the available *content* width — the layout width minus any
reserved vertical-scrollbar space. Column count is the maximum
number of `item_width`-minimum items that fit the content
width (INV-310-3); items never render narrower than
`item_width`. Stretched width =
`(content_width − (cols − 1) · gap_x) / cols`, applied
uniformly. Column count and stretched width MUST derive from
the SAME content width. No residual right gutter beyond
`gap_x`. `fill_width` supersedes `GridAlignment` (no slack
remains to align). When content width is narrower than two
minimum-width items, one item fills the row. Item height is
unaffected.

**INV-310-14:** Every view mode consumes the same location-scoped
`NavigatorDataSource` and provider-confirmed `OrderRevision`. Row enumeration
uses `ValueState<OrderWindow>` from bounded SPEC-HAL-188 `order_window`
requests, and identity-to-position recovery uses `locate_row`; total
presentation memory must not scale with total provider extent. A movement or
range intent whose required locate/window testimony is not ready accepts no
partial state change.

**INV-310-15:** `NavigatorNavigation::new` and
`replace_navigation_binding` always force one pending projection application.
An equal semantic location must not suppress initial hydration or context
rebinding.

**INV-310-16:** `clear_navigation_binding` discards the pending projection and
its sink before reopening the shared gate on the last completely installed
source. It never installs the pending handle, leaves the gate closed, or permits
different adapters to retain different sources.

---

# 16. Forbidden Behaviors

**FORBID-310-0:** Navigator must NOT create child widgets
for items in grid-based modes. Items are delegate-painted.
Widget tree cost is constant regardless of item count.

**FORBID-310-1:** No horizontal scroll in grid-based modes.
Column count adapts to width.

**FORBID-310-2:** No DataSource mutation during paint/layout.

**FORBID-310-3:** No sorting or filtering by Navigator.
Consumer controls row order via DataSource.

**FORBID-310-4:** Navigator must NOT own or create a
NavigationHost. It must not receive or retain a host reference.

**FORBID-310-5:** Navigator must NOT call NavigationHost.
All navigation leaves the widget through `NavigatorNavigationSink`;
the consumer owns context binding, routing submission, and projection.

**FORBID-310-6:** Navigator must NOT paint item chrome
(backgrounds, borders, corner radii). This is the
CardPainter's responsibility.

**FORBID-310-7:** Navigator must NOT add a location-specific full-row
enumeration API such as `items_at`, call the compatibility no-window
`row_ids()` path as canonical behavior, or accumulate a complete RowId
universe. Navigation location resolution must not bypass the provider's
revisioned bounded order contract.

---

# 17. Consumer API

### 17.1 Single-mode flat browser

```rust
let nav = Navigator::new(source, painter)
    .with_policy(NavigatorPolicy::single_select());
```

### 17.2 Multi-mode browser

```rust
let nav = Navigator::new(source, painter)
    .with_policy(NavigatorPolicy::multi_select());

// Consumer's toolbar has a SegmentedControl:
nav.set_view_mode(NavigatorViewMode::Details);
nav.set_view_mode(NavigatorViewMode::Icon);
```

### 17.3 Asset picker with transactional selection

```rust
let navigation_sink = navigator_navigation_sink();
let current = host.borrow().control_projection().current.clone();
let projection = NavigatorLocationProjection::new(current, source.clone());
let nav = Navigator::new(source, painter)
    .with_policy(NavigatorPolicy::asset_picker())
    .with_navigation(NavigatorNavigation::new(
        projection,
        navigation_sink.clone(),
    ));

// User browses and selects items.
// Consumer routes navigation_sink through the bound domain/root,
// resolves the committed location to a source, then projects both.
// Consumer commits selection via OK button:
// let selected = nav.committed_selection();
```

### 17.4 Full file browser composition (consumer-side)

```rust
// Consumer's Screen::build():
let split = SplitPane::horizontal();
    // Left: TreeView projection/sink bound to the same navigation context
    // Right: Navigator
let breadcrumbs = Breadcrumbs::from_nav_path(host.borrow().path());
let mode_switcher = SegmentedControl::new(["Details", "Icons", "Tiles"]);
let navigation_sink = navigator_navigation_sink();
let current = host.borrow().control_projection().current.clone();
let projection = NavigatorLocationProjection::new(current, source.clone());
let navigator = Navigator::new(source, painter)
    .with_policy(NavigatorPolicy::file_browser())
    .with_navigation(NavigatorNavigation::new(
        projection,
        navigation_sink.clone(),
    ));
```

Navigator owns the content projection. Everything around
it is the consumer's composition.

### 17.5 Custom policy

```rust
let policy = NavigatorPolicy {
    allowed_modes: vec![NavigatorViewMode::Icon, NavigatorViewMode::Details],
    default_mode: NavigatorViewMode::Icon,
    selection_mode: SelectionMode::Single,
    interaction_model: SelectionInteractionModel::Immediate,
    activation_mode: ActivationMode::OnClick,
    range_selection: RangePolicy::Disabled,
    navigation_enabled: false,
};
let nav = Navigator::new(source, painter)
    .with_policy(policy);
```

---

# 18. Implementation Phases

Current code (Phases 1-7) is preserved. The evolution path:

**Phase 7:** ✅ Rename GridView → Navigator. Completed.

**Phase 8:** NavigatorPolicy struct. Replace the
`NavigatorPolicy` enum with the composable struct. Wire
`selection_mode`, `interaction_model`, `activation_mode`,
`range_selection` to `OrderedSelectionState`. Add preset
constructors.

**Phase 9:** NavigatorViewMode enum and config presets.
Add all 8 view modes. Details mode creates ListView
internally. List mode creates List internally. Grid-based
modes use NavigatorBody with different dimensions. Remove
card chrome from NavigatorConfig.

**Phase 10:** `NavigatorDataSource` trait. Its default
`navigation_entry` returns flat behavior. Add `NavigatorNavigation`,
`NavigatorLocationProjection`, and the location-scoped source binding.
Wire forced pending projection installation in `update()`.

**Phase 11:** NavigationHost integration through caller-owned
projection and intent sinks. Double-click on a navigable item
emits `NavigateTo` (when navigation_enabled). Escape with empty
selection emits `NavigateUp`. Caller-projected location changes
replace the active DataLayer-resolved source, reset location-local state,
and consume only revisioned bounded order windows in every view mode.
Navigator retains no NavigationHost or resolver reference.

---

# 19. Acceptance Criteria

All original GridView criteria (1-13 from SPEC-HAL-310 v1)
remain valid, plus:

14. Policy struct controls all interaction axes independently
15. Allowed modes enforced — set_view_mode rejects unlisted modes
16. Single-mode policy disables mode switching
17. SelectionInteractionModel::Transactional requires explicit commit
18. View mode switching preserves selection
19. NavigatorDataSource with flat defaults = flat browser
20. Double-click on navigable item emits NavigateTo; caller routes it through the explicit navigation-context binding
21. External navigation (tree sidebar) updates Navigator content through a caller-resolved location/source projection
22. Escape with no selection navigates up to the structural parent (if navigation enabled)
23. Location/source projection replacement clears selection, focus, all anchors/range and transactional selection state, item-local transient state, and scroll
24. Config presets produce correct item sizes
25. Navigator paints NO item chrome — painter owns visuals
26. Details mode uses ListView with column headers
27. List mode uses basic List (compact, column-first flow)
28. Fluid fill is opt-in (`fill_width`), disabled by default;
    when enabled, item width fills the row with no residual
    gutter and never renders below `item_width`
29. Navigation locations use semantic `NavigationEntry` values end-to-end
30. Context rebinding atomically replaces projected location and its dedicated sink, discarding undrained old intents
31. One navigation-enabled activation emits exactly one navigation intent; observational events are not a second command seam
32. Initial binding and whole-context rebinding hydrate exactly once even when location/source identity compares equal
33. Details, List, and grid modes consume one provider order revision through bounded row windows; a million-row source requires no million-RowId widget snapshot
34. The one shared content gate closes synchronously for every pending projection; clearing a pending binding discards it and reopens only the last completely installed flat source

---

# 20. Consistency Matrix (SPEC-HAL-002A)

| Pillar | Status |
|--------|--------|
| A — Retained-Mode Ownership | Preserved. Navigator in tree. Items delegate-painted. |
| B — Phase Separation | Preserved. Navigation in update(). Mode change in rebuild. Paint pure. |
| C — ID-based Identity | Preserved. Items by RowId. Locations by semantic NavigationEntry identity. |
| D — Pure Data | Preserved. DataSource is pure. CardPainter is pure rendering. |
| E — GPU-First | Preserved. Standard scene graph primitives. SDF borders (SPEC-R-022). |
| F — Sovereignty | Preserved. NavigationHost is Halcyon-owned, not OS. |

---

# 21. Relationship to Existing Code

| Component | Relationship |
|-----------|-------------|
| `DataSource` (SPEC-188) | **Extended.** `NavigatorDataSource` adds only row-to-navigation-entry mapping. The application/DataLayer resolves locations to sources; the FROZEN revisioned bounded-order contract remains canonical. |
| `NavigationHost` (SPEC-228) | **Integrated through caller-owned projection and intent routing.** Navigator neither owns nor references it. |
| `NavigationGuard` (SPEC-231) | **Respected.** Explicit single-root binding or validated multiroot dispatch reaches guard-gated NavigationHost execution. |
| `Multiroot routing` (SPEC-237/247) | **Respected when present.** The consumer supplies the domain/root binding; Navigator never selects or dispatches a root. |
| `OrderedSelectionState` (SPEC-135) | **Reused.** Governed by policy. Shared across view modes. |
| `SelectionInteractionModel` (SPEC-136) | **Reused.** Policy selects Immediate vs Transactional. |
| `TabularIntentPolicy` (SPEC-200) | **Pattern reference.** Navigator policy follows same composable struct pattern. |
| `ScrollArea` (SPEC-086) | **Reused.** Vertical scroll container. |
| `ListView` | **Used internally** for Details view mode. |
| `List` | **Used internally** for List view mode (compact). |
| `TabularSubstrate` | **NOT used** by grid-based modes. Used by ListView in Details mode. |
| `FlowGrid` | **NOT related.** Layout primitive, not a data browser. |
| `RoundedRect rendering` (SPEC-R-022) | **Used.** SDF pipeline for selection/focus borders. |
| `Breadcrumbs` (SPEC-220) | **Consumer composes.** Projects NavigationHost path. |
| `TreeView` (SPEC-262) | **Consumer composes.** Sidebar projection/sink bound to the same navigation context. |
| `SegmentedControl` (SPEC-274) | **Consumer composes.** View mode switcher. |
