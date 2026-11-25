# SPEC-HAL-134: Variable Item Height Virtualization Invariants

Status: Phase 1 + Phase 2 Implemented / Binding
Layer: Virtualization Core
Applies to: List, VirtualList, any widget using heterogeneous item heights
Depends on: SPEC-HAL-131B
Supersedes: None
Closure Level: Hard (engine-grade)

---

## 1. Scope

This SPEC defines engine-grade invariants for virtualization when item heights are non-uniform and only partially known.

It formalizes:

* how item heights are measured, cached, and accumulated
* how scroll_offset → visible_range is derived with heterogeneous heights
* how realization behaves when measurements are incomplete
* determinism, purity, monotonicity, and idempotence guarantees

Correctness dominates performance.
Prediction, speculation, and heuristic guessing are forbidden.

---

## 2. Definitions

* **Measured height** — actual height obtained from a completed layout pass
* **Unknown height** — item whose height has not yet been measured
* **Height table** — indexed storage of measured heights
* **Cumulative height table** — prefix sum over heights (measured + defaults)
* **Visible range** — logical item indices intersecting the viewport
* **Realized range** — logical indices instantiated (per SPEC-HAL-131B)

---

## 3. Core Principles

* Variable height virtualization is **data-driven**, not predictive
* Scroll math must remain **pure and deterministic**
* Unknown heights must not destabilize visible_range
* Measurement may refine state, but never retroactively rewrite history

---

## 4. Height Measurement Invariants

INV-VIRT-HGT-1
An item’s height may transition only once from unknown → measured.
Once measured, height is immutable until item identity changes.

INV-VIRT-HGT-2
Measured heights MUST be cached by logical index.
They MUST NOT depend on realization order, slot reuse, or frame timing.

INV-VIRT-HGT-3
Height measurement MUST occur only during layout.
No measurement during input, scroll, hit-testing, or event routing.

---

## 5. Unknown Height Policy

INV-VIRT-HGT-4
Unknown heights MUST be represented by a deterministic fallback height.
The fallback MUST be constant for the lifetime of the list instance.

INV-VIRT-HGT-5
Fallback height MUST be used consistently in all computations:

* scroll_offset → visible_range
* cumulative height table
* realized_range derivation

INV-VIRT-HGT-6
Changing fallback height after first layout is forbidden.

---

## 6. Cumulative Height Table Invariants

INV-VIRT-HGT-7
Cumulative height table MUST be a pure prefix sum of:

* measured heights where available
* fallback height where unknown

INV-VIRT-HGT-8
Cumulative height table MUST be monotonically increasing.

INV-VIRT-HGT-9
Updates to the cumulative table MAY only occur when:

* a new height is measured
* item count changes

---

## 7. Scroll → Visible Range Mapping

INV-VIRT-HGT-10
visible_range MUST be computed exclusively from:

* scroll_offset
* viewport height
* cumulative height table

INV-VIRT-HGT-11
visible_range computation MUST be deterministic for identical inputs.

INV-VIRT-HGT-12
visible_range.start MUST be monotonic with scroll direction:

* scrolling down → start non-decreasing
* scrolling up → start non-increasing

INV-VIRT-HGT-13
Measuring a previously unknown item MUST NOT retroactively change
visible_range for the current scroll_offset.

---

## 8. Realization Invariants (Variable Height)

INV-VIRT-HGT-14
realized_range MUST depend ONLY on:

* visible_range
* prefetch policy
* height table (measured + fallback)

INV-VIRT-HGT-15
realized_range MUST NOT depend on:

* raw scroll offset
* scroll velocity or deltas
* input events
* layout pass count

INV-VIRT-HGT-16
Re-running layout with identical inputs MUST NOT change:

* visible_range
* realized_range
* widget identity ordering

---

## 9. Measurement Refinement Rules

INV-VIRT-HGT-17
When an item transitions from fallback → measured height:

* cumulative height table may be updated
* future visible_range computations may differ
* current frame visible_range MUST remain stable

INV-VIRT-HGT-18
No “scroll correction”, snap, or compensation is permitted when
measurements arrive.

---

## 10. Forbidden Behaviors

The following are explicitly forbidden:

* Predicting heights from neighbors
* Averaging previously measured items
* Retroactive scroll offset adjustment
* Re-centering viewport after measurement
* Coupling realization to measurement order
* Measuring outside layout

Violations require a new SPEC.

---

## 11. Closure Condition

INV-VIRT-HGT-COMPLETE-1
Variable height virtualization is considered complete iff:

* visible_range depends only on scroll_offset + cumulative height table
* realized_range depends only on visible_range + prefetch
* measurement refines future state only
* repeated layout with identical inputs is idempotent
* no scroll, input, or layout side effects leak across layers

Any change violating this invariant requires an explicit SPEC amendment.

---

## 12. Relationship to Other SPECs

* SPEC-HAL-131B defines base virtualization and realization purity
* SPEC-HAL-134 extends it to heterogeneous geometry
* Future SPECs may layer optimizations but MUST preserve these invariants

---

## 13. Implementation Status

### Phase 1: Measurement Substrate — IMPLEMENTED

All Phase 1 invariants (INV-VIRT-HGT-1 through INV-VIRT-HGT-9) are implemented in
`src/widgets/list.rs`. Code anchors below are authoritative.

#### State Fields (src/widgets/list.rs)

| Field                    | Line | Purpose                                      |
|--------------------------|------|----------------------------------------------|
| `fallback_height`        | 131  | Deterministic fallback for unknown heights   |
| `fallback_height_locked` | 132  | Prevents mutation after first layout         |
| `measured_heights`       | 133  | Per-item measured height cache (by index)    |
| `cumulative_y`           | 134  | Prefix sum of effective heights              |
| `pending_height_updates` | 135  | Deferred height updates from ListItem        |
| `current_phase`          | 146  | Layout/Scroll phase separation               |

#### INV-VIRT-HGT-1: Height Transition Lock

* **Sentinel**: `measured_heights[i] == 0.0` means unknown; `> 0.0` means measured
* **Reversion guard**: lines 984–993 — blocks overwriting measured height with ≤0
* **Function**: height update processing in `layout()`, Stage 0

#### INV-VIRT-HGT-2: Indexed by Logical Index

* **Field**: `measured_heights: Vec<f32>` (line 133)
* **Index usage**: `effective_height(index)` (lines 300–306)
* **Query**: `is_item_measured(index)` (lines 289–294)

#### INV-VIRT-HGT-3: Measurement Only During Layout

* **Phase enum**: `ListPhase` (lines 45–57)
* **Phase guard**: `current_phase == ListPhase::LayoutApply` assertion (lines 951–957)
* **Transitions**: `begin_layout()` (lines 640–647), `end_layout()` (lines 653–660)
* **Engine hook**: `pre_layout()` (lines 905–911)

#### INV-VIRT-HGT-4: Deterministic Fallback Height

* **Field**: `fallback_height: f32` (line 131)
* **Usage**: `effective_height()` returns `fallback_height` when `measured_heights[i] == 0.0`

#### INV-VIRT-HGT-5: Consistent Fallback Usage

* **`effective_height()`**: lines 300–306 — used by all scroll/cumulative computations
* **`item_display_height()`**: lines 327–334 — same fallback logic
* **`rebuild_cumulative_y()`**: line 356 — calls `effective_height(i)`

#### INV-VIRT-HGT-6: Fallback Immutable After First Layout

* **Lock field**: `fallback_height_locked: bool` (line 132)
* **Lock set**: lines 963–964 in `layout()` Stage 0
* **Guard**: `set_fallback_height()` debug_assert (lines 268–272)

#### INV-VIRT-HGT-7: Cumulative Table is Prefix Sum

* **Field**: `cumulative_y: Vec<f32>` (line 134)
* **Builder**: `rebuild_cumulative_y()` (lines 349–361)
* **Formula**: `cumulative_y[i] = sum(effective_height(0..i))`

#### INV-VIRT-HGT-8: Monotonically Increasing

* **Debug assertion**: lines 1002–1014 in `layout()` after height updates

#### INV-VIRT-HGT-9: Updates Only on Measurement or Count Change

Valid call sites for `rebuild_cumulative_y()`:

1. `set_item_count()` — line 207 (item count changes)
2. `set_fallback_height()` — line 277 (initial setup before lock)
3. Height update processing — line 999 (new measurement arrives)

No other call sites exist.

#### Verification

Test file: `tests/list_virtualization_phase1.rs`

---

## 14. Phase 2: Scroll → Visible Range Mapping — IMPLEMENTED

**Status: Phase 2 Implemented / Binding**

Phase 2 covers INV-VIRT-HGT-10 through INV-VIRT-HGT-13. All invariants are
**implemented and mechanically enforced** via debug assertions and unit tests.

---

### INV-VIRT-HGT-10: Exclusive Input Sources — IMPLEMENTED

**Invariant**: visible_range MUST be computed exclusively from:
- scroll_offset
- viewport height
- cumulative height table

**Responsibility**:
- **Reads**: `scroll_y` (parameter), `viewport_height` (field), `cumulative_y` (field)
- **Writes**: `visible_range` (field)
- **Owns**: The mapping from scroll position to logical item indices

**Entry Points**:
- `update_visible_range_from_scroll()` — lines 407–471
- `find_first_item_at_y()` — line 366 (helper)
- `find_last_item_at_y()` — line 383 (helper)

**Forbidden Behaviors**:
- Reading `scroll_velocity`, `scroll_delta`, or any momentum state
- Consulting `realized_range` or realization history
- Accessing frame counter, timestamp, or animation state
- Using any input event data
- Reading item content or widget properties

---

### INV-VIRT-HGT-11: Determinism — IMPLEMENTED

**Invariant**: visible_range computation MUST be deterministic for identical inputs.

**Responsibility**:
- **Guarantees**: Same (scroll_y, viewport_height, cumulative_y) → same visible_range
- **Owns**: Idempotence of scroll→range mapping

**Entry Points**:
- `update_visible_range_from_scroll()` — lines 407–471

**Forbidden Behaviors**:
- Caching intermediate results across frames
- Using random or time-based values
- Consulting global mutable state
- Depending on call count or invocation order
- Producing different results on repeated calls with identical inputs

---

### INV-VIRT-HGT-12: Monotonicity with Scroll Direction — IMPLEMENTED

**Invariant**: visible_range.start MUST be monotonic with scroll direction:
- scrolling down → start non-decreasing
- scrolling up → start non-increasing

**Responsibility**:
- **Guarantees**: No backward jumps in visible_range.start during smooth scroll
- **Owns**: Continuity of visibility progression

**Entry Points**:
- `update_visible_range_from_scroll()` — lines 407–471
- Binary search in `find_first_item_at_y()` — line 366

**Forbidden Behaviors**:
- Edge jumps: visible_range.start decreasing when scroll_y increases
- Oscillation: start flickering between values during continuous scroll
- Discontinuities: large jumps in start for small scroll_y changes
- Off-by-one visibility: item visible at scroll_y but not at scroll_y + ε

---

### INV-VIRT-HGT-13: Measurement Isolation — IMPLEMENTED

**Invariant**: Measuring a previously unknown item MUST NOT retroactively change
visible_range for the current scroll_offset.

**Responsibility**:
- **Guarantees**: Height measurement affects future frames only
- **Owns**: Stability of visible_range within a frame

**Entry Points**:
- Height update processing in `layout()` — Stage 0
- `rebuild_cumulative_y()` — lines 349–361
- `update_visible_range_from_scroll()` — lines 407–471

**Forbidden Behaviors**:
- Stale cumulative usage: using old cumulative_y after measurement
- Double realization: re-computing visible_range mid-frame after height update
- Retroactive correction: adjusting scroll_offset to "compensate" for new height
- Current-frame drift: visible_range changing within same frame due to measurement

---

### Implementation Evidence

**Pure Helper Function**:
- `compute_visible_range_from_scroll()` — lines 84–154 in `src/widgets/list.rs`
- Reads ONLY: `scroll_y`, `viewport_height`, `cumulative_y`, `item_count`
- Contains debug_assert! guards for INV-VIRT-HGT-7/8 (cumulative_y shape)

**Phase-Gated Caller**:
- `update_visible_range_from_scroll()` — lines 425–489 in `src/widgets/list.rs`
- Phase guard: `current_phase == ListPhase::ScrollCompute`
- Delegates to pure helper for variable-height mode

**Test Coverage**:
- Test file: `tests/list_phase2_scroll_mapping.rs`
- Determinism tests (INV-VIRT-HGT-11): same inputs → same outputs
- Monotonicity tests (INV-VIRT-HGT-12): scroll down → start non-decreasing
- Boundary tests: empty list, single item, exact pixel boundaries
- Negative tests (debug-only): malformed cumulative_y triggers panic

---

### Closure Note

Phase 2 is **complete**.

- Mapping contract is self-guarding via debug assertions
- No additional state fields introduced to List
- All guarantees are mechanically enforced by tests
- Violations are detectable at compile-time (debug builds), not by review

Authorization gate conditions satisfied:
1. ✓ Phase 1 remains green under full test suite
2. ✓ No additional state fields introduced
3. ✓ `update_visible_range_from_scroll()` audited against INV-VIRT-HGT-10..13
4. ✓ User authorization received

---

## 15. Authority Chain Freeze (Phase 5)

**Status: CLOSED**

The authority chain for scroll-driven virtualization is hereby frozen:

```
scroll_offset → visible_range → realized_range → projections
```

### 15.1 Closure Declaration

This chain is **closed**. Each link has exactly one authoritative source:

| Concept         | Authority                                  | SPEC            |
|-----------------|---------------------------------------------|----------------|
| scroll_offset   | ScrollState (SPEC-HAL-131A)                 | SPEC-HAL-131A   |
| visible_range   | List.update_visible_range_from_scroll()    | SPEC-HAL-134    |
| realized_range  | visible_range + prefetch policy            | SPEC-HAL-131B   |
| projections     | post_layout_populate_input_projections()   | SPEC-HAL-131C   |

### 15.2 Reinterpretation Prohibition

No intermediate layer may reinterpret upstream data:

- `visible_range` MUST NOT reinterpret, smooth, predict, or correct `scroll_offset`
- `realized_range` MUST NOT reinterpret, extend, or re-derive `visible_range`
- `projections` MUST NOT recompute `realized_range` or apply post-hoc corrections

Any deviation from this chain requires a **new SPEC**, not an extension.

### 15.3 Deviation Gate

Future changes that would:

- Add a new link to the chain
- Inject intermediate processing
- Reinterpret data between links
- Add parallel derivation paths

are **forbidden** without explicit SPEC justification and approval.

---

## 16. Prefetch Policy Formalization (Phase 6)

**Status: Binding**

Phase 6 formalizes how `realized_range` expands from `visible_range`, making prefetch
behavior deterministic, bounded, and non-reinterpretable.

### 16.1 Policy Parameters

| Parameter       | Value | Description                                    |
|-----------------|-------|------------------------------------------------|
| PREFETCH_ITEMS  | 2     | Symmetric expansion in items (±2)              |

### 16.2 Invariants (Binding)

**INV-PREFETCH-1 — Single Authority**
`realized_range` MUST be derived ONLY from:
- `visible_range`
- fixed prefetch parameters (PREFETCH_ITEMS)

No other inputs (scroll velocity, frame history, adaptive behavior) are permitted.

**INV-PREFETCH-2 — Symmetric Expansion**
Prefetch expansion MUST be symmetric:
```
realized_range = (visible.start - P) .. (visible.end + P)
```
where `P = PREFETCH_ITEMS`. Expansion is equal on both sides.

**INV-PREFETCH-3 — Hard Clamping**
`realized_range` MUST be clamped to `[0, item_count]`.
No overflow, no wrap, no compensation.

**INV-PREFETCH-4 — Determinism**
Identical `visible_range` + identical policy ⇒ identical `realized_range`.
No history, no hysteresis, no adaptive behavior.

**INV-PREFETCH-5 — No Shadow Logic**
No consumer may re-expand, shrink, or reinterpret `realized_range`.
All prefetch logic lives in ONE function: `compute_realized_range_from_visible()`.

### 16.3 Implementation

**Pure Helper Function**: `compute_realized_range_from_visible()`
- Location: `src/widgets/list.rs`
- Inputs: `visible_range`, `item_count`, `prefetch`
- Output: `Range<usize>`
- Contains debug assertions for all invariants

**Call Sites** (exhaustive):
1. Layout Stage 4 — instantiation window computation
2. `realized_widget_for_item()` — item realization check
3. `realized_range()` — trait implementation

### 16.4 Forbidden Behaviors

The following are explicitly forbidden:

- Adaptive or dynamic prefetch
- Scroll-velocity-based expansion
- Frame-history-based behavior
- Any "just in case" widening
- Pixel-based overscan (removed in Phase 6)
- Secondary expansion by consumers

### 16.5 Test Coverage

Test file: `tests/list_prefetch_phase6.rs`

Coverage:
- Zero prefetch
- Small/large prefetch
- Near-start and near-end clamping
- Symmetry verification
- Determinism (same input → same output)
- No negative indices, no overflow
- Range well-formedness (exhaustive)

---

## 17. Slot–Index Mapping Freeze (Phase 7)

**Status: Binding**

Phase 7 freezes and formalizes the mapping between **logical indices** and **slot indices**
so widget reuse, focus, and visual state are mechanically stable under virtualization + prefetch.

Phase 6 froze *what* is realized.
Phase 7 freezes *how realized items map to slots*.

### 17.1 Core Formula

The slot–index mapping is governed by a single, canonical formula:

```
slot_index = logical_index - realized_range.start
logical_index = realized_range.start + slot_index
```

No alternative formulas are permitted.

### 17.2 Invariants (Binding)

**INV-SLOT-1 — Single Mapping Authority**
Slot index MUST be computed as:
```
slot_index = logical_index - realized_range.start
```
No alternative formulas permitted. All slot mapping logic lives in:
- `slot_index_from_logical()` for logical → slot
- `logical_index_from_slot()` for slot → logical

**INV-SLOT-2 — Contiguity**
Slots MUST represent a contiguous window:
```
slots = realized_range.len()
```
No gaps, no sparsity, no reordering.

**INV-SLOT-3 — Stability Across Frames**
If `realized_range` is unchanged:
- slot_index ↔ logical_index mapping MUST remain identical
- widgets MUST NOT migrate between slots

**INV-SLOT-4 — Bounds Safety**
For every realized item:
```
0 <= slot_index < children.len()
```
Out-of-range access is forbidden.

**INV-SLOT-5 — No Shadow Slot Logic**
No consumer may:
- recompute slot_index differently
- cache alternative mappings
- infer mapping from widget order

All slot mapping logic lives in ONE place.

### 17.3 Implementation

**Pure Helper Functions**: `slot_index_from_logical()`, `logical_index_from_slot()`
- Location: `src/widgets/list.rs`
- Inputs: `logical_index` or `slot_index`, `realized_range`
- Output: `Option<usize>` (None if out of range)
- Contains debug assertions for INV-SLOT-1 and INV-SLOT-4

**Call Sites** (exhaustive):
1. Layout loop — slot → logical mapping for widget wiring
2. `realized_widget_for_item()` — logical → slot lookup

### 17.4 Forbidden Behaviors

The following are explicitly forbidden:

- Adaptive slot allocation
- Sparse slot reuse
- Slot reordering
- Any logic depending on widget pool history
- Inline `index - something` math outside helpers

### 17.5 Test Coverage

Test file: `tests/list_slot_mapping_phase7.rs`

Coverage (24 tests):
- Formula correctness at start/middle/end
- Round-trip consistency (slot → logical → slot)
- Contiguity verification
- Stability across frames
- Rejection of out-of-range indices
- Integration with Phase 6 prefetch
- Edge cases (empty range, single item, large range)

---

## 18. Widget Identity & State Stability (Phase 8)

**Status**: Binding

Phase 7 froze index mapping. Phase 8 freezes **identity and state ownership**.

### 18.1 Problem Statement

Slot reuse is structurally correct, but identity semantics were implicit.
Phase 8 answers one question:

> *When is a widget allowed to keep its internal state across frames?*

### 18.2 Core Invariants

**INV-ID-1 — Logical Identity Authority**

A widget's semantic identity is **logical index**, not:
- slot index
- widget pointer
- pool position
- frame order

**INV-ID-2 — State Preservation Rule**

A widget MAY preserve internal state **if and only if**:
```
previous_logical_index == current_logical_index
```

Otherwise:
- state MUST be reset
- visual flags MUST clear
- transient state MUST drop

**INV-ID-3 — Mandatory Rebind on Index Change**

When a widget is reused for a different logical index:
- `on_rebind(new_logical_index)` MUST be invoked
- no old state may survive
- silent reuse is forbidden

**INV-ID-4 — No Slot-Based Identity**

Widget identity MUST NOT depend on:
- slot index
- children array position
- creation order
- reuse history

**INV-ID-5 — Deterministic Rebinding**

Given identical sequences of `(realized_range, logical_index)`:
- widget identity evolution MUST be identical
- no frame-dependent behavior permitted

### 18.3 Implementation

**ListItem Changes**:
- Added `last_logical_index: Option<usize>` field for rebind detection
- Added `on_rebind(&mut self, new_logical_index: usize)` method
- `on_rebind()` clears `previous_height` (transient measurement state)
- `on_rebind()` updates `last_logical_index` for next detection

**List Layout Loop**:
```rust
let needs_rebind = list_item.last_logical_index() != Some(logical_index);
list_item.set_index(logical_index);
if needs_rebind {
    list_item.on_rebind(logical_index);
}
```

### 18.4 Transient State Cleared on Rebind

The following state is cleared when `on_rebind()` is called:
- `previous_height` — relates to old item's measurement history

The following state is NOT cleared (re-wired each layout):
- callbacks (on_click, on_height_change)
- visual_state_provider

### 18.5 Forbidden Behaviors

- Implicit state carryover across logical index changes
- Slot-based heuristics for state preservation
- Pool-history–based behavior
- "Optimizations" that skip rebinds
- Silent widget reuse without on_rebind()

### 18.6 Test Coverage

Test file: `tests/list_widget_identity_phase8.rs`

Coverage (18 tests):
- Logical identity authority (identity is index, not slot)
- State preserved when logical index unchanged
- State reset when logical index changes
- Mandatory rebind on first use and on change
- Slot reuse with different indices resets state
- Scroll away and back behavior
- Partial scroll preserves overlapping items
- Deterministic rebinding across identical sequences
- Edge cases (rebind to zero, repeated frames)

---

## 19. Selection & Focus Authority (Phase 9)

**Status**: Binding

Phase 8 froze widget identity and transient state. Phase 9 freezes **selection and focus authority**.

### 19.1 Problem Statement

Selection and focus state could theoretically be stored in two places:
1. **Widget-internal** (each ListItem stores `is_selected: bool`)
2. **External authority** (ListState stores `selected_ids: HashSet<LogicalItemId>`)

Only option 2 is correct for virtualization. Widget-internal storage would be lost on rebind.

### 19.2 Core Invariants

**INV-SEL-EXTERN-1 — No Internal Selection State**

ListItem MUST NOT contain fields for:
- selection (`is_selected`)
- focus (`is_focused`)
- hover (`is_hovered`)
- pressed/active state

**INV-SEL-EXTERN-2 — External Authority**

Selection and focus authority MUST live in ListState:
- `selected_ids: HashSet<LogicalItemId>`
- `focused_id: Option<LogicalItemId>`

These are the ONLY authoritative sources.

**INV-SEL-EXTERN-3 — Pure Projection**

`ListItem.visual_state()` MUST derive its result from:
- `visual_state_provider` closure (set by parent List)
- which queries `ListState` by logical index

ListItem.visual_state() never returns cached or internal values.

**INV-SEL-EXTERN-4 — Selection Survives Reuse**

Selection MUST survive:
- scroll away (widget unrealized)
- scroll back (widget re-realized)
- slot reuse for different logical index
- on_rebind() invocation

This is structurally guaranteed by external authority.

**INV-SEL-EXTERN-5 — Focus Survives Reuse**

Focus MUST survive all the same conditions as selection.

### 19.3 ListItem Field Audit (Phase 9)

**Structural fields** (persist across frames):
- `id: WidgetInternalId` — widget's own identity
- `child: Option<NodeId>` — child reference

**Dynamic fields** (set each layout by parent):
- `index: usize` — current logical index

**Transient fields** (cleared on rebind per Phase 8):
- `bounds: Size` — layout cache
- `previous_height: f32` — measurement history

**Rebind tracking** (Phase 8):
- `last_logical_index: Option<usize>` — rebind detection

**Callbacks** (re-wired each layout):
- `on_click` — click handler
- `on_height_change` — height change callback

**External state query** (pure projection):
- `visual_state_provider` — queries ListState

**NONE of the above are selection/focus/hover fields.**

### 19.4 Implementation Evidence

**ListState authority** (`src/widgets/list_state.rs`):
```rust
pub struct ListState {
    pub focused_id: Option<LogicalItemId>,
    pub selected_ids: HashSet<LogicalItemId>,
    ...
}
```

**ListItem projection** (`src/widgets/list_item.rs`):
```rust
pub fn visual_state(&self) -> ListItemVisualState {
    self.visual_state_provider
        .as_ref()
        .map(|provider| provider(self.index))
        .unwrap_or_default()
}
```

**List wiring** (`src/widgets/list.rs`):
```rust
let snapshot = Rc::clone(&self.visual_snapshot);
list_item.set_visual_state_provider(move |index| {
    snapshot.borrow().item_visual_state(index)
});
```

### 19.5 Forbidden Behaviors

- ListItem storing `is_selected`, `is_focused`, `is_hovered`, `is_pressed`
- Selection surviving via widget memory (must survive via ListState)
- Focus derived from widget rather than queried from ListState
- Caching visual state within ListItem
- Any selection/focus state cleared by on_rebind()

### 19.6 Test Coverage

Test file: `tests/selection_survives_reuse_phase9.rs`

Coverage:
- Selection survives scroll away and back (INV-SEL-EXTERN-4)
- Selection survives slot reuse for different item (INV-SEL-EXTERN-5)
- Multi-selection survives widget reuse (INV-SEL-EXTERN-2)
- ListItem has no internal selection state (INV-SEL-EXTERN-1)
- Focus survives widget reuse (INV-SEL-EXTERN-5)

Additional coverage in: `tests/selection_focus_projection_tests.rs`
- Selection binds to LogicalItemId (INV-STATE-IDENTITY-1)
- State survives visible_range changes
- Multi-selection survives realization churn
- State changes do not trigger realization

---

## 20. Input Projection Stability (Phase 10)

**Status**: Locked
**Locked**: 2025-12-22

Phase 9 froze selection/focus authority. Phase 10 freezes **input projection routing**.

### 20.1 Core Invariants

**INV-INPUT-PROJ-1**: Input Always Targets LogicalItemId
```
pointer event → hit_test_projected() → ProjectedHitResult
              → dispatch_projected_input() → handle_projected_action()
              → container handles via logical_index
```

Widget identity (TreeWidgetId) is an OPTIONAL optimization hint, not a routing key.

**INV-INPUT-PROJ-2**: Slot Reuse Does Not Affect Targeting

When slots are reused after scroll:
- Slot B rendered item 1 → now renders item 3
- Click at same viewport position targets item 3

No "whatever was in this slot" memory.

**INV-INPUT-PROJ-3**: Hit Testing Is Stateless

`InputProjection::hit_test(x, y)` is pure:
- Same inputs → same outputs
- No cached target
- No "last hit" memory
- No sticky binding

**INV-INPUT-PROJ-4**: Focus Survives Widget Reuse

Focus is stored in `ListState.focused_id` (LogicalItemId).
Scroll causing widget reuse does not change focus index.

**INV-INPUT-PROJ-5**: No Pointer Capture on Item Widgets

ListItem handles clicks as immediate Down→Up sequences.
Container (List) may capture for scroll, but never for item targeting.

**INV-INPUT-PROJ-6**: Selection Action Routes to Container

Selection changes go through:
```
handle_projected_action(logical_id, logical_index, ProjectedAction::Select)
```

Never through widget lookup. ListState is the only authority.

### 20.2 Audit Summary

**Input Projection Flow (verified 2025-12-22)**:
1. Raw event → InputFusion → FusedInputEvent
2. EventRouter calls `tree.hit_test_projected(x, y)`
3. Returns ProjectedHitResult with LogicalItemId, container_id
4. `dispatch_projected_input()` routes to container
5. Container handles via `handle_projected_action(logical_index, action)`
6. State changes stored in ListState by LogicalItemId

**No widget identity in routing path** — verified by code inspection.
**No slot index in routing path** — verified by code inspection.
**No realization order dependency** — dispatch works for unrealized items.

### 20.3 Rebind Safety

**Pointer capture + reuse concern**: DOES NOT APPLY

ListItem does not capture pointer. Click is immediate (Down→Up).
Even if widgets are reused mid-interaction, the click completes
before rebind occurs.

Container (List) may capture for scroll operations, but scroll
targets the container, not individual items.

### 20.4 Forbidden Behaviors

- Routing input by widget identity instead of logical identity
- Caching hit-test results across frames
- "Sticky" click targets that persist after scroll
- ListItem capturing pointer
- Selection/focus dispatch targeting widget instead of container
- Input decisions based on slot index or realization order

### 20.5 Test Coverage

Test file: `tests/input_projection_stability_phase10.rs`

Coverage (14 tests):
- `test_projected_hit_result_prioritizes_logical_id` — INV-INPUT-PROJ-1
- `test_unrealized_item_has_logical_id` — INV-INPUT-PROJ-1
- `test_click_targets_logical_id_not_slot` — INV-INPUT-PROJ-2
- `test_hover_tracking_by_logical_index` — INV-INPUT-PROJ-3
- `test_no_phantom_clicks_after_scroll` — INV-INPUT-PROJ-3
- `test_hit_test_is_stateless` — INV-INPUT-PROJ-3
- `test_focus_survives_widget_reuse` — INV-INPUT-PROJ-4
- `test_arrow_navigation_targets_logical_index` — INV-INPUT-PROJ-4
- `test_listitem_no_pointer_capture` — INV-INPUT-PROJ-5
- `test_selection_action_targets_list_state` — INV-INPUT-PROJ-6
- `test_multi_selection_stable_across_reuse` — INV-INPUT-PROJ-6
- `test_projection_provider_stable_ids` — LogicalItemId stability
- `test_focus_to_does_not_change_selection` — FocusTo semantics
- `test_focus_navigation_updates_selection_in_single_mode` — Navigation semantics

### 20.6 SPEC Update

SPEC-HAL-131B Section T.8 added with:
- INV-INPUT-PROJ-1 through INV-INPUT-PROJ-5
- Audit triggers for capture and dispatch routing
- Forbidden patterns

---

## 21. Pre-Layout Synchronization Invariants

**Status**: Binding
**Added**: 2025-12-23

This section formalizes invariants discovered during the virtualization black gap fix.
These invariants prevent 1-frame lag between scroll offset and layout.

### 21.1 Pre-Layout Scroll Sync

**INV-VIS-PRE-SYNC-1 — Scroll Observation Before Layout**

Any widget whose layout depends on scroll offset MUST observe the current
scroll offset BEFORE `layout_tree()` executes.

Applies to:
- `List.visible_range`
- Any future virtualized container
- Any widget performing realization windows

See: SPEC-HAL-131A Section L.2

### 21.2 Layout Dirty on Geometry-Relevant State Change

**INV-VIS-DIRTY-1 — Mandatory Dirty Marking**

If a widget mutates state that affects child geometry, it MUST be marked
layout-dirty in the same frame.

Applies to:
- `List.visible_range`
- Any future virtualized container
- Any widget performing realization windows

**INV-VIS-DIRTY-2 — Forbidden Patterns**

Forbidden:
- Updating visible_range without triggering layout
- Relying on paint-time transforms to compensate for stale geometry
- Caching layout when visible_range changed

**INV-VIS-DIRTY-3 — Enforcement**

`update_visible_range_from_scroll()` MUST return a change flag.
Caller MUST call `tree.mark_layout_dirty(widget_id)` when changed.

### 21.3 Scroll Sync Scope Isolation

**INV-VIS-SCOPE-1 — Nested ScrollArea Isolation**

Scroll synchronization applies ONLY to descendants of a ScrollArea's content
subtree and MUST NOT cross into nested ScrollAreas.

Each ScrollArea owns its own scroll offset. A child ScrollArea has independent
scroll state that MUST NOT be overwritten by parent sync.

Traversal explicitly stops at nested ScrollAreas. This is intentional.

See: SPEC-HAL-131A Section L.5, INV-SCROLL-SYNC-SCOPE-1

### 21.4 Relationship to Frame Pipeline

The full frame pipeline ordering is documented in SPEC-HAL-131A Section L.4:

```
0. pre_layout_sync_scroll_to_lists()       — sync scroll BEFORE layout
1. layout_tree()                           — compute widget geometry
2. post_layout_populate_input_projections()— build input projections
3. post_layout_resolve_scroll_anchors()    — resolve scroll anchors
4. post_layout_sync_scroll_to_lists()      — sync for anchor changes
5. paint_tree()                            — build scene graph
```

Reordering is forbidden. Each step depends on outputs from previous steps.

### 21.5 Guaranteed Layout for Virtualized Children (Added 2025-12-23)

**INV-VIRT-LAYOUT-EPOCH-1 — Direct Layout Path Cache Update**

List uses direct `child_widget.layout()` calls instead of `cx.layout_child()` to
enable custom child positioning. When using direct layout, List MUST manually
update the child's cache state:

```rust
if let Some(node) = cx.tree.node_mut(child_id) {
    node.cached_size = result.size;
    node.last_constraints = Some(constraints);
    node.dirty_layout = false;
    #[cfg(debug_assertions)]
    node.layout_epoch = current_layout_epoch();
}
```

This ensures:
1. Layout skip optimization works correctly on subsequent frames
2. Paint-time epoch assertion passes (INV-PAINT-REQUIRES-LAYOUT-EPOCH)
3. Child geometry is properly tracked

**INV-VIRT-LAYOUT-EPOCH-2 — Paint Requires Current Epoch**

Virtualized children may NOT be painted unless laid out in the current frame.

- Violation: paint_node assertion fires for stale layout_epoch
- Symptom: "Item 0 pile-up" — all slots compute same logical_index from stale Y position
- Root cause: List not stamping child layout_epoch

**INV-VIRT-LAYOUT-EPOCH-3 — Custom Positioning Requires Cache Update**

Any widget that:
1. Uses direct `child_widget.layout()` instead of `cx.layout_child()`
2. Sets custom child positions via `restore_widget(child, widget, custom_rect)`

MUST also update the child's cache state including `layout_epoch`.

See: SPEC-HAL-042 Section 11.4, SPEC-HAL-037 Section 11.4

### 21.6 Logical Index Source Invariants (Added 2025-12-23)

**INV-VIRT-INDEX-SOURCE-1 — Logical Index Is Data-Derived**

A virtualized child's logical index MUST be derived from virtualization state,
not geometry.

Required formula:
```
logical_index = base_index + slot_index
```
Where:
- `base_index` = `realized_range.start` (NOT `visible_range.start`!)
- `slot_index` = physical slot number (0..slot_count)

**INV-VIRT-BASE-INDEX-AUTHORITY-1 — Base Index Source**

`base_index` MUST equal `realized_range.start`, NOT `visible_range.start`.

Reason: Slots are positioned at `realized_range.start + widget_idx` during layout.
For paint to compute the same logical_index, base_index must match this positioning.

`visible_range` and `realized_range` differ by `PREFETCH_ITEMS` — using the wrong
one causes a systematic offset between layout positioning and paint labeling.

**FORBIDDEN**: Deriving logical_index from `rect.y`, `rect`, or any geometry.
Geometry is viewport-relative and does NOT represent data identity.

**INV-VIRT-INDEX-SOURCE-2 — Geometry Is View-Relative**

Widget geometry (rect.x, rect.y, size) is viewport-relative.
It MUST NOT be used to infer data identity or logical indices.

Reasons:
- Scroll offsets rebase geometry
- Clipping truncates visible regions
- Multiple logical items can have `rect.y ≈ 0` at different times
- Geometry ≠ data identity (fundamental virtualization principle)

See: SPEC-HAL-042 Section 11.5

---

## 22. Virtualization Closure

**Status**: CLOSED
**Closed**: 2025-12-22

This section formally marks the end of virtualization as an open concern.

### 22.1 Closure Declaration

The following subsystems are **closed concerns**:

1. **Realization** — visible_range → realized_range mapping (SPEC-HAL-131B)
2. **Slot Reuse** — widget pool management, slot↔index mapping (Phase 7)
3. **Selection** — external authority in ListState (Phase 9)
4. **Focus** — external authority in ListState (Phase 9)
5. **Input Projection** — LogicalItemId-based routing (Phase 10)

These subsystems have:
- Frozen invariants with mechanical enforcement
- Comprehensive test coverage mapping to named invariants
- No open questions or pending design decisions

### 22.2 Future Change Gate

Any future change touching:
- Realization logic
- Slot mapping formulas
- Selection/focus authority
- Input projection routing
- Widget identity semantics

**MUST open a new SPEC**. Amendments to this SPEC are forbidden.

### 22.3 Gesture / Drag Isolation

The upcoming gesture and drag work:
- **MUST NOT** weaken any virtualization invariant
- **MUST NOT** introduce widget-identity-based routing
- **MUST NOT** couple input dispatch to slot indices
- **MUST NOT** store interaction state in ListItem

Gesture work operates on top of the closed virtualization substrate.
Violations require explicit SPEC justification and user approval.

### 22.4 What Phase 10 Does NOT Cover

The following are explicitly **out of scope** for virtualization closure
and belong to future gesture/interaction work:

- **Pointer capture** — multi-frame capture for drag operations
- **Drag lifetimes** — press→move→release gesture sequences
- **Gesture arbitration** — scroll vs drag vs select disambiguation
- **Multi-frame pointer state** — tracking pointer across frames

These are preparation markers for the next phase, not implementation targets.

---

End of SPEC-HAL-134
