# SPEC-HAL-190: Authority Projection & Composite Widget Coordination

**Status:** Draft
**Layer:** Core Architecture / Widget Model
**Audience:** Engine, Widget Tree, Layout, Event Routing
**Supersedes:** Conceptual scope previously attempted in SPEC-E series
**Related:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-079, SPEC-HAL-145, SPEC-HAL-150

---

## 1. Purpose

This SPEC defines the missing architectural primitive required to build complex, composite widgets in Halcyon without:

- monolithic widgets,
- duplicated layout systems,
- scroll compensation,
- hidden UI trees,
- or parallel component architectures.

The primitive is called **Authority Projection**.

Authority Projection allows a single parent widget to own authoritative state and expose controlled, phase-bound access to that state to multiple child widgets, while preserving all Halcyon constitutional invariants.

This SPEC exists to permanently close the design gap that caused repeated compensation attempts (including the deprecated SPEC-E series).

---

## 2. Problem Statement

Halcyon enforces:

- a single widget tree,
- a single layout system,
- strict phase separation,
- single-owner state authority.

However, prior to this SPEC, Halcyon lacked a lawful mechanism for:

- a parent widget owning shared state, and
- multiple child widgets reading that state concurrently, and
- children expressing mutation intent without shared ownership.

This limitation made correct implementations of composite structures (e.g. headers + bodies + footers, synchronized views, shared filters) impossible without architectural violations.

---

## 3. Definitions

### 3.1 Authority

Authority is any state that is:

- owned by exactly one widget,
- semantically authoritative (geometry, selection, predicates, focus, etc.),
- read by other widgets,
- mutated only by the owner.

Examples:

- tabular column geometry,
- selection sets,
- search predicates,
- shared cursors or domains.

---

### 3.1.1 Authority Qualification Filter

Before introducing an authority, ALL of the following must be true:

1. **Multiple consumers**: More than one widget needs to read this state independently.

2. **Independent lifetime**: The state remains meaningful even if some consumers disappear.

3. **Semantic weight**: The state represents meaning, not presentation (selection, predicates, domains, geometry models).

4. **Single lawful owner**: There is exactly one widget that can legitimately own and mutate it.

If ANY of these is false → the state is NOT an authority.

**INV-AUTH-QUALIFY-1:**
State that fails the qualification filter MUST NOT be modeled as authority.

---

### 3.1.2 What Is Not Authority

The following MUST remain local widget state, never authority:

- hover state
- pressed state
- animation progress
- scroll offsets
- transient hit-test results
- temporary drag state
- visual interpolation
- layout bookkeeping

These are:

- mechanical (not semantic),
- ephemeral (not persistent),
- phase-local (not cross-phase).

Turning any of these into authorities is a design failure.

**INV-AUTH-NOT-MECHANICAL:**
Mechanical, ephemeral, or phase-local state MUST NOT be modeled as authority.

---

### 3.1.3 Typical Authority Axes

Real UI systems have a small, bounded number of semantic authority axes:

- selection
- structure / hierarchy
- filtering / predicates
- time / domain
- focus
- geometry models (when shared)

If you are tempted to create dozens of authorities, either:

- ownership is unclear, or
- semantics are not properly factored.

Authorities are coarse, not fine-grained.

---

### 3.2 Authority Owner

The Authority Owner is a widget that:

- owns authoritative state,
- is responsible for mutating it,
- is responsible for invalidation.

There is exactly one owner per authority.

---

### 3.3 Authority Projection

An Authority Projection is a read-only, phase-scoped view into parent-owned authority, made available to child widgets by the engine.

Projections:

- do not transfer ownership,
- do not extend lifetime,
- do not allow direct mutation.

---

### 3.4 Intent

An Intent is a structured request emitted by a child widget to express desired semantic change, without performing mutation.

Examples:

- select row,
- resize column,
- toggle flag,
- change filter.

Intents are processed only by the authority owner.

---

## 4. Core Design Principle

Composite widgets coordinate through shared authority, not shared layout or hidden UI structure.

- Layout remains hierarchical and dumb.
- State remains centralized and authoritative.
- Children cooperate by reading the same truth, not by coordinating geometry.

---

## 4A. Sovereign Widgets vs Composite Containers

### 4A.1 The Distinction

A **sovereign widget** owns authority. It:

- owns semantic state (TabularSubstrate, SelectionSet, etc.),
- processes intents,
- decides invalidation,
- defines semantics.

A **composite container** is a structural grouping that:

- has no semantic ownership,
- routes events mechanically,
- provides layout coordination only.

**INV-SOVEREIGN-1:**
ListView, TreeView, and similar complex widgets are sovereign — not composite containers.

---

### 4A.2 Sovereign Widgets Have Internal Structure

Sovereignty does NOT mean:

- one paint function,
- one event handler,
- one giant rect,
- one private layout system.

Those are implementation smells.

A sovereign widget:

- is the semantic root,
- but may materialize as a small, ordinary widget subtree.

Header, body, footer of a ListView are:

- intentionally separate widgets,
- with real layout rects,
- with clear responsibilities,
- with zero shared ownership.

They are not peers. They are organs of a single semantic entity.

---

### 4A.3 The Correct Mental Model

**INV-SOVEREIGN-2:**
A sovereign widget is a single semantic entity whose physical realization is a fixed internal widget topology.

Externally: ListView behaves like one widget.
Internally: It expands into multiple child widgets.
Semantically: Authority lives only at the root.
Mechanically: Children are normal widgets — no cheats.

This avoids both:

- monolithic god-widgets (reimplementing layout, scroll, hit-testing),
- fake component containers (leaking authority, unclear ownership, implicit coordination).

---

### 4A.4 The Decisive Test

Ask: "If I replace ListView with something else, what disappears?"

Correct answer:

- selection semantics
- column model
- intent processing
- shared truth

Incorrect answer:

- header widget
- scroll area
- footer widget

Those are implementation details.

If the authority goes away, the ListView is gone — even if the child widgets remain.

---

## 5. Authority Projection Model

### 5.1 Single Ownership

**INV-AUTH-SINGLE-OWNER:**
An authority MUST have exactly one owner.

Forbidden:

- shared ownership,
- cloning,
- Rc, Arc, RefCell for authority state,
- cross-widget mutation.

---

### 5.2 Engine-Injected Access

**INV-AUTH-INJECTION:**
Authority access is never constructed by widgets.

- Projections are injected by the engine during phase execution.
- Widgets cannot fabricate or persist them.

---

### 5.3 Phase-Bound Validity

**INV-AUTH-PHASE-BOUND:**
Authority projections are valid only during the phase in which they are issued.

Allowed:

- Layout phase: read
- Event phase: read + emit intents
- Paint phase: read

Forbidden:

- storing projections,
- cross-phase usage,
- async usage.

Violations MUST fail loudly.

---

### 5.4 Read-Only Semantics

**INV-AUTH-READ-ONLY:**
Child widgets MUST NOT mutate authority directly.

All mutation flows through:

- intent emission,
- owner processing,
- deferred mutation phase.

---

### 5.5 Ancestor-Only Authority Access

**INV-AUTH-ANCESTOR-ONLY:**
Authority projection walks the ancestor chain ONLY. Siblings cannot read each other's authority.

If widget A and widget B are siblings, and both need authority X:

- They MUST share a common ancestor C that owns X.
- Both A and B access X via ancestor lookup (which finds C).
- Neither A nor B can access the other's state directly.

**INV-AUTH-NO-SIBLING-READ:**
A widget MUST NOT read authority owned by a sibling, cousin, or any non-ancestor.

**Consequence:**
If two widgets need the same authority and have no common ancestor owning it, the architecture is wrong. Fix by:

1. Introducing a common ancestor that owns the authority, OR
2. Recognizing the widgets belong to different semantic domains.

This is not a limitation. It is the law that makes complexity tractable.

---

## 6. Intent Routing

### 6.1 Intent Emission

- Intents may be emitted only during event handling.
- Children push intents into a buffer owned by the authority owner.

No bubbling.
No callbacks.
No cross-child communication.

---

### 6.2 Intent Processing

Intents are processed by the authority owner during deferred mutation.

The owner:

- validates,
- orders,
- merges,
- clamps,
- or rejects intents.

Only the owner mutates state.

---

### 6.3 Invalidation

After mutation, the owner is responsible for marking:

- layout dirty if geometry changed,
- paint dirty if visuals changed.

Children do not self-invalidate.

---

## 7. Layout & Positioning Rules

### 7.1 Layout Ownership

**INV-LAYOUT-SINGLE-SYSTEM:**
There is exactly one layout system.

- Parent widgets assign rects.
- Children stay within assigned rects.
- No child computes sibling geometry.

---

### 7.2 Composite Positioning

A composite widget:

- receives one rect,
- subdivides it into child rects using normal layout rules,
- assigns real, owned layout space to each child.

No compensation.
No transforms.
No hidden offsets.

---

### 7.3 Scroll Isolation

**INV-SCROLL-SOVEREIGNTY:**

- ScrollArea is the sole owner of scroll mechanics.
- Scroll is a viewport transform, not a layout mutation.
- Scroll state is not part of authority projection.

Children:

- never apply scroll math,
- never compensate coordinates,
- never mutate scroll state.

---

## 8. Window Resize Semantics

On window resize:

1. Layout is invalidated globally.
2. New rects are assigned top-down.
3. Authority state remains unchanged.
4. ScrollAreas clamp offsets if needed.
5. Children re-read authority during paint.

Resize affects geometry only, never semantic state.

---

## 9. Performance Model

- Rect reassignment is O(1).
- Authority reads are O(1).
- No notification graphs.
- No subscriptions.
- No recomputation unless authority actually changes.

This is the cheapest correct model.

---

### 9.1 Complexity Scaling

**INV-SCALE-1:**
Internal interaction complexity grows linearly with the number of authorities, not exponentially with widget connections.

Compare costs:

**Composite-centric design (FORBIDDEN):**

- N widgets
- N×(N−1) coordination paths
- hidden coupling
- exponential failure modes

**Authority-centric design (REQUIRED):**

- N widgets
- M authorities (M ≪ N)
- N×M read paths
- zero peer coupling

The authority model is strictly cheaper and more stable.

---

### 9.2 When Interaction Grows Complex

When internal interaction becomes more complex:

- you add authorities (if they pass the qualification filter),
- you do NOT add composites,
- you do NOT add routing,
- you do NOT add containers.

The model does not change. Only the number of authorities does.

Widgets do not talk to other widgets. They talk to truth.

---

## 10. Explicitly Forbidden Patterns

**FORBID-AUTH-STORAGE:**
Children storing authority projections.

**FORBID-AUTH-DIRECT-MUTATION:**
Children mutating authority directly.

**FORBID-PARALLEL-UI-TREES:**
Hidden component trees, containers, panels with internal layout.

**FORBID-SCROLL-COMPENSATION:**
Manual scroll offsets in layout, paint, or hit-test.

**FORBID-WIDGET-COUPLING:**
Widgets directly accessing sibling or parent widgets.

**FORBID-CYCLIC-AUTHORITY:**
Cyclic semantic dependencies between authorities (A mutates B while B mutates A in the same frame).

If you encounter what appears to be a cyclic dependency:

- you MUST introduce a new authority that linearizes the dependency,
- you MUST NOT add callbacks or bidirectional communication.

Cyclic dependencies are unresolvable without nondeterminism.

**FORBID-AUTHORITY-INFLATION:**
Creating authorities for state that fails the qualification filter (Section 3.1.1).

If tempted to create dozens of authorities, ownership is unclear or semantics are not properly factored. Authorities are coarse, not fine-grained.

---

## 11. Relationship to Deprecated SPEC-E Series

The SPEC-E series attempted to solve composite coordination by introducing:

- containers,
- panels,
- component systems,
- parallel lifecycles.

Those approaches are now obsolete.

This SPEC provides the correct resolution point inside the widget model itself.

SPEC-E documents are superseded and should be archived as historical context only.

---

## 12. Canonical Examples (Non-Normative)

### 12.1 Single Authority: ListView

ListView with header / body / footer sharing column authority (TabularSubstrate).

- **Authority**: TabularSubstrate (columns, selection, focus)
- **Owner**: ListView
- **Readers**: HeaderWidget, BodyWidget, FooterWidget
- **Intents**: SelectRowIntent, ResizeColumnIntent, etc.

---

### 12.2 Multiple Authorities: Coordinated Editor Panel

A more complex example with query editor, result list, preview panel, timeline, status bar.

**Critical Ownership Rule:**
Cross-cutting authorities MUST be owned by a common ancestor. Sibling widgets cannot share authority directly.

**Widget Hierarchy:**

```
EditorPanel (sovereign root)
├── owns: SelectionAuthority, FilterAuthority
├── QueryEditor
│   └── owns: QueryAuthority (local to query editing)
├── ResultList
│   └── reads: SelectionAuthority, FilterAuthority (from ancestor)
├── PreviewPanel
│   └── reads: SelectionAuthority (from ancestor)
└── Timeline
    └── owns: TimelineAuthority (local to timeline)
    └── reads: SelectionAuthority (from ancestor)
```

**Authorities:**

- QueryAuthority — owned by QueryEditor; query text, parse state, errors
- SelectionAuthority — owned by EditorPanel; shared selection across panels
- FilterAuthority — owned by EditorPanel; shared filter predicate
- TimelineAuthority — owned by Timeline; time window / cursor

**Key Constraint (INV-AUTH-ANCESTOR-ONLY):**
SelectionAuthority is owned by EditorPanel (the common ancestor), not by any sibling. All children read it via ancestor projection.

**Example flow — Typing in query editor:**

1. QueryEditor child emits UpdateQueryIntent
2. QueryEditor mutates QueryAuthority
3. QueryEditor invalidates paint + layout if needed
4. ResultList reads FilterAuthority (owned by EditorPanel)
5. ResultList applies filter during its paint
6. No widget reads another sibling's authority

Each widget reads from its ancestor chain only.

**Example flow — Clicking a result:**

1. ResultList emits SelectResultIntent
2. EditorPanel (owner of SelectionAuthority) processes intent
3. EditorPanel mutates SelectionAuthority
4. EditorPanel invalidates (marks descendants dirty)
5. PreviewPanel reads SelectionAuthority (ancestor lookup)
6. Timeline reads SelectionAuthority (ancestor lookup)
7. Both repaint with current selection

No widget-to-widget calls. No sibling authority reads.

---

### 12.3 The Pattern

All are instances of the same pattern:

- authority owners control semantic truth,
- readers project authority during phases,
- intents express desired change,
- no widget-to-widget communication.

---

---

# Part II: Implementation Mechanisms

## Part Inseparability

**INV-SPEC-190-UNITY:**
Part I (invariants and principles, Sections 1-12) and Part II (mechanisms, Sections 13-20) are a SINGLE UNIT.

Partial adoption is FORBIDDEN.

- You MUST NOT implement Part II mechanisms without respecting Part I invariants.
- You MUST NOT claim Part I compliance while omitting Part II mechanisms.
- You MUST NOT implement a "simplified version" that skips Projection wrappers, intent buffers, or deferred mutation.

Either implement SPEC-HAL-190 in full, or do not implement it at all.

This is not negotiable. Partial implementations create coordination gaps that are worse than no implementation.

---

The following sections define concrete engine mechanisms required to implement Authority Projection. These are normative.

---

## 13. Authority Injection Mechanism

### 13.1 Authority Trait

A type is projectable as authority if it implements the `Authority` marker trait:

```rust
pub trait Authority: 'static {}
```

This trait is implemented by authority types (e.g., `TabularSubstrate`, `FilterPredicate`). It carries no methods; it exists for type discrimination.

---

### 13.2 Authority Registration

Widgets that own authorities declare this during tree construction via the `AuthorityProvider` trait:

```rust
pub trait AuthorityProvider {
    /// Return a type-erased reference to the authority this widget provides.
    /// Called by engine during tree traversal setup.
    fn provided_authority(&self) -> Option<(&dyn Any, TypeId)>;
}
```

The engine maintains a mapping from `(NodeId, TypeId)` to authority availability. This mapping is built once per tree mutation, not per frame.

**INV-AUTH-REGISTER-1:**
A widget providing authority MUST return the same `TypeId` for its lifetime.

**INV-AUTH-REGISTER-2:**
Only direct ownership is permitted. A widget cannot register authority it does not own.

---

### 13.3 Projection Type

The projection is a lifetime-bound, non-constructible wrapper:

```rust
pub struct Projection<'phase, A: Authority> {
    inner: &'phase A,
    // Private field with no public constructor
    _sealed: SealedMarker,
}

impl<'phase, A: Authority> Projection<'phase, A> {
    /// Engine-only constructor.
    pub(crate) fn new(inner: &'phase A) -> Self;

    /// Read access to the authority.
    pub fn get(&self) -> &A { self.inner }
}
```

The `'phase` lifetime is the context lifetime. When the context dies, the projection becomes invalid. Rust's borrow checker enforces this; no runtime checks needed.

**INV-AUTH-PROJECTION-1:**
`Projection` has no public constructor. Only the engine can create projections.

**INV-AUTH-PROJECTION-2:**
`Projection` does not implement `Clone`, `Copy`, or any lifetime-extending trait.

---

### 13.4 Context API

Each phase context exposes authority lookup:

```rust
impl<'a> LayoutContext<'a> {
    /// Request projection to nearest ancestor authority of type A.
    /// Returns None if no ancestor in the current subtree provides A.
    pub fn authority<A: Authority + 'static>(&self) -> Option<Projection<'a, A>>;
}

impl<'a> PaintContext<'a> {
    pub fn authority<A: Authority + 'static>(&self) -> Option<Projection<'a, A>>;
}

impl<'a> EventContext<'a> {
    pub fn authority<A: Authority + 'static>(&self) -> Option<Projection<'a, A>>;
}
```

The engine resolves the request by walking the ancestor chain from the current widget until it finds a widget providing the requested `TypeId`.

**INV-AUTH-LOOKUP-1:**
Authority lookup is O(depth). Depth is bounded by tree structure.

**INV-AUTH-LOOKUP-2:**
If multiple ancestors provide the same authority type, the nearest ancestor wins.

---

### 13.5 Failure Behavior

| Condition | Behavior |
|-----------|----------|
| No ancestor provides requested type | Returns `None` |
| Projection stored beyond phase | Compile error (lifetime) |
| Authority mutated during read phase | Impossible (no mutable access) |

No panics. No runtime guards. Rust's type system enforces phase bounds.

---

## 14. Intent Buffer Ownership & Flow

### 14.1 Engine-Owned Buffers

The engine owns all intent buffers:

```rust
// Inside engine frame state
intent_buffers: HashMap<TreeWidgetId, IntentBuffer>,

pub struct IntentBuffer {
    intents: Vec<Box<dyn Intent>>,
}
```

Buffers are keyed by the `TreeWidgetId` of the authority owner widget.

**INV-INTENT-BUFFER-1:**
Intent buffers are owned exclusively by the engine. Widgets never hold references to buffers.

---

### 14.2 Intent Trait

```rust
pub trait Intent: Any + Send {
    fn as_any(&self) -> &dyn Any;
}
```

Concrete intent types (e.g., `SelectRowIntent`, `ResizeColumnIntent`) implement this trait.

---

### 14.3 Emission API

Children emit intents via `EventContext`:

```rust
impl<'a> EventContext<'a> {
    /// Emit an intent to the nearest ancestor authority owner of type A.
    /// The intent is buffered; the owner processes it during deferred mutation.
    /// Returns false if no ancestor provides authority A.
    pub fn emit_intent<A: Authority, I: Intent>(&mut self, intent: I) -> bool;
}
```

The engine:

1. Looks up the nearest ancestor providing authority `A`.
2. Retrieves or creates the intent buffer for that ancestor's `TreeWidgetId`.
3. Pushes the intent into the buffer.

The child never touches the buffer directly. The engine mediates.

**INV-INTENT-EMIT-1:**
`emit_intent` may only be called during event handling phase.

**INV-INTENT-EMIT-2:**
The child has no visibility into the buffer contents or owner identity.

---

### 14.4 Owner Retrieval

Authority owners retrieve intents via a widget trait method called by the engine:

```rust
pub trait AuthorityOwner: Widget {
    /// Process pending intents. Called by engine during deferred mutation phase.
    /// The buffer is drained before this call; owner receives owned intents.
    fn process_intents(&mut self, intents: Vec<Box<dyn Intent>>);
}
```

The engine calls this method after event routing completes, passing the drained buffer contents.

**INV-INTENT-PROCESS-1:**
`process_intents` is called exactly once per frame if the buffer is non-empty.

**INV-INTENT-PROCESS-2:**
The buffer is cleared before the call. The owner receives owned data, not references.

---

### 14.5 Buffer Lifecycle

| Phase | Buffer State |
|-------|--------------|
| Frame start | Empty (cleared at previous frame end) |
| Event routing | Intents accumulate |
| Deferred mutation | Drained, passed to owner |
| Layout | Empty |
| Paint | Empty |
| Frame end | Empty |

---

## 15. Mutation Timing & Phase Timeline

### 15.1 Frame Phase Sequence

```
┌─────────────────────────────────────────────────────────────────┐
│                         FRAME N                                 │
├─────────────────────────────────────────────────────────────────┤
│                                                                 │
│  1. EVENT ROUTING                                               │
│     ├─ Input events dispatched to widgets                       │
│     ├─ Widgets read authority (immutable)                       │
│     ├─ Widgets emit intents → engine buffers                    │
│     └─ Event handlers return                                    │
│                                                                 │
│  2. DEFERRED MUTATION  ← NEW PHASE                              │
│     ├─ Engine drains intent buffers                             │
│     ├─ Engine calls owner.process_intents(intents)              │
│     ├─ Owner mutates authority state                            │
│     └─ Owner marks dirty flags (layout and/or paint)            │
│                                                                 │
│  3. LAYOUT (if dirty)                                           │
│     ├─ Top-down constraint propagation                          │
│     ├─ Widgets read authority (immutable)                       │
│     └─ Rects assigned                                           │
│                                                                 │
│  4. POST-LAYOUT SYNC                                            │
│     ├─ sync_scroll called on virtualizing widgets               │
│     └─ Realized ranges updated                                  │
│                                                                 │
│  5. PAINT                                                       │
│     ├─ Widgets read authority (immutable)                       │
│     └─ Primitives emitted                                       │
│                                                                 │
└─────────────────────────────────────────────────────────────────┘
```

---

### 15.2 Geometry-Changing Intents

If an intent causes geometry change (e.g., column resize):

1. Owner processes intent in deferred mutation phase.
2. Owner updates authority state.
3. Owner calls `cx.invalidate_layout()`.
4. Layout phase runs in the SAME frame.
5. All children receive updated geometry during their layout.

**INV-MUTATION-TIMING-1:**
Geometry-changing mutations take effect in the same frame, not the next frame.

**INV-MUTATION-TIMING-2:**
Layout always runs after deferred mutation if dirty.

---

### 15.3 Visual-Only Intents

If an intent causes visual change only (e.g., selection change):

1. Owner processes intent in deferred mutation phase.
2. Owner updates authority state.
3. Owner calls `cx.invalidate_paint()`.
4. Layout is skipped (not dirty).
5. Paint runs; children read updated state.

---

### 15.4 Cross-Authority Independence

**INV-INTENT-INDEPENDENCE:**
Authorities processed in the same deferred mutation phase MUST be semantically independent.

This means:

- Authority A's mutation MUST NOT depend on reading Authority B's post-mutation state.
- Authority B's mutation MUST NOT depend on reading Authority A's post-mutation state.
- Each authority owner processes intents using only its own pre-mutation state.

If two authorities have a causal dependency (A must change before B reads the result):

1. A single higher-level authority MUST own both, OR
2. The mutation MUST be split across frames.

**FORBID-CROSS-AUTHORITY-FRAME-DEPENDENCY:**
An authority owner MUST NOT read another authority during `process_intents()` and expect to see that authority's post-mutation state from the same frame.

Ordering within a single authority is the owner's responsibility.
Ordering across authorities within a frame is FORBIDDEN.

---

### 15.5 Tree Mutation During Event Phase

**INV-TREE-MUTATION-LEGAL:**
Tree mutations (add/remove widgets) during event phase are legal.

If an authority owner is removed from the tree before deferred mutation:

- Pending intents targeting that owner are silently dropped.
- This is NOT an error.
- Widgets emitting intents MUST NOT assume delivery is guaranteed.

**INV-INTENT-BEST-EFFORT:**
Intent delivery is best-effort. If the target authority owner ceases to exist, the intent disappears.

This is acceptable because:

1. Intents express desire, not obligation.
2. The semantic effect would be meaningless if the owner is gone.
3. Error reporting would require tracking intent origins, which adds complexity for no benefit.

---

## 16. Relationship to sync_scroll

### 16.1 sync_scroll Is Not Authority Projection

`sync_scroll` is a pre-existing, engine-orchestrated synchronization mechanism for scroll offset propagation. It is NOT an authority projection mechanism.

| Property | Authority Projection | sync_scroll |
|----------|---------------------|-------------|
| Direction | Child reads parent | Engine pushes to child |
| Timing | On-demand during phase | Dedicated post-layout pass |
| Data | Arbitrary authority types | Single f32 (scroll_y) |
| Initiation | Child requests | Engine initiates |
| Mutation | Via intent queue | Direct method call |

---

### 16.2 Why sync_scroll Does Not Violate SPEC-190

sync_scroll is lawful because:

1. **Engine-orchestrated**: The child does not choose when sync_scroll is called. The engine calls it.

2. **Unidirectional**: Data flows from ScrollArea → child. The child does not write back to ScrollArea.

3. **Narrow scope**: sync_scroll propagates exactly one value (scroll_y) for exactly one purpose (virtualization range update).

4. **Post-layout timing**: It runs after layout, before paint, in a dedicated engine pass.

5. **Not semantic authority**: Scroll offset is a viewport coordinate, not semantic state like selection or column geometry.

**INV-SYNC-SCROLL-1:**
sync_scroll remains as a parallel, engine-owned channel for scroll offset propagation.

**INV-SYNC-SCROLL-2:**
sync_scroll is not used for authority state. Authority uses the projection mechanism.

---

### 16.3 Coexistence

A composite widget like ListView uses BOTH mechanisms:

- **Authority Projection**: Header, body, footer read column geometry, selection, focus from TabularSubstrate.
- **sync_scroll**: Body receives scroll_y from ScrollArea for virtualization.

These mechanisms are orthogonal and do not conflict.

---

## 17. Rc/RefCell Prohibition Scope

### 17.1 What Is Forbidden

**FORBID-SHARED-SEMANTIC-AUTHORITY:**
Using `Rc`, `Arc`, `RefCell`, or `Mutex` to share semantic authority state between widgets.

Forbidden examples:

- `Rc<RefCell<SelectionState>>` passed to header and body
- `Arc<Mutex<ColumnGeometry>>` shared across children
- `Rc<TabularSubstrate>` held by multiple widgets

These patterns violate single ownership and enable uncoordinated mutation.

---

### 17.2 What Is Permitted

**PERMIT-MECHANICAL-LINKS:**
Engine-orchestrated, unidirectional, single-value coordination channels.

Permitted examples:

- `HorizontalScrollLink` (Rc<Cell<f32>>)
- `ScrollGroupId` for linked scrolling

These are permitted because:

1. **Single writer**: One widget writes, others read.
2. **Engine timing**: Updates occur at engine-defined points.
3. **Not semantic**: They carry mechanical coordinates, not domain state.
4. **Narrow**: Single scalar values, not complex state graphs.

---

### 17.3 HorizontalScrollLink Justification

`HorizontalScrollLink` is `Rc<Cell<f32>>`. Why is it legal?

| Criterion | HorizontalScrollLink | Forbidden Pattern |
|-----------|---------------------|-------------------|
| Writer count | 1 (ScrollArea only) | Multiple |
| Data type | f32 coordinate | Complex state |
| Semantic meaning | Viewport offset | Domain authority |
| Mutation timing | Engine-controlled | Arbitrary |

`HorizontalScrollLink` is a mechanical coordination primitive, not semantic authority. It does not require replacement.

**INV-MECHANICAL-LINK-1:**
Mechanical links are permitted only for single-writer, single-scalar, engine-timed coordination.

**INV-MECHANICAL-LINK-2:**
Mechanical links MUST NOT carry semantic authority (selection, focus, predicates, etc.).

### 17.4 VerticalScrollLink Justification

`VerticalScrollLink` retains one presented Y coordinate while one `ScrollArea`
instance is replaced by another. It remains a permitted mechanical link because:

1. **Sequential single writer**: the outgoing and replacement `ScrollArea` do not
   write concurrently.
2. **Single scalar**: only the mechanical vertical coordinate is retained.
3. **Engine timing**: the replacement consumes it once at the final tight-layout
   boundary before paint.
4. **No semantic authority**: selection, focused row, data identity, and content
   interpretation remain owned elsewhere.

It MUST NOT be used to synchronize simultaneously mounted vertical scroll areas;
that contract belongs to `ScrollGroupId`.

---

## 18. Invalidation Granularity

### 18.1 Whole-Composite Invalidation

When authority changes, the owner marks itself dirty. The engine's standard dirty propagation marks descendants.

There is no fine-grained "only repaint the body" mechanism.

**INV-INVALIDATION-1:**
Authority change invalidates the owner. Descendants inherit dirty status via tree propagation.

---

### 18.2 Why Fine-Grained Invalidation Is Not Needed

1. **Authority changes are rare**: Most frames have no authority mutation.

2. **Paint is cheap**: Repainting header + body + footer is bounded by child count.

3. **Observer graphs are forbidden**: Fine-grained invalidation requires tracking which children depend on which authority fields. This is an observer graph.

4. **Simplicity**: Whole-composite invalidation is O(children) with no bookkeeping.

---

### 18.3 How Children Learn of Changes

Children do not receive notifications. They simply read authority during paint.

If authority changed → owner is dirty → children are dirty → children repaint → children read current state.

If authority did not change → children may or may not repaint → if they do, they read same state.

No subscriptions. No callbacks. No observer registration.

**INV-INVALIDATION-2:**
Children never self-invalidate based on authority changes. They read current state when painted.

---

## 19. Summary

SPEC-HAL-190 introduces Authority Projection as a first-class architectural primitive.

It allows Halcyon to support arbitrarily complex composite widgets while preserving:

- a single widget tree,
- a single layout system,
- strict phase separation,
- and constitutional invariants.

With this SPEC, no parallel UI architectures are required.

---

## 20. Engine Kickstart Checklist (Strict Implementation Order)

Implementation MUST proceed in the order below. Each step is:

- **Atomic**: Completes one mechanism fully.
- **Testable**: Has clear verification criteria.
- **Non-overlapping**: Does not depend on future steps.

No speculative optimizations. No caching. No shortcuts.

---

### Step 1: Authority Marker Trait

**Implement:**
```rust
pub trait Authority: 'static {}
```

**Test:**
- Define a test authority type implementing `Authority`.
- Verify trait bound works in generic contexts.

**Verification:** Compiles. No runtime behavior yet.

---

### Step 2: Projection Type

**Implement:**
```rust
pub struct Projection<'phase, A: Authority> {
    inner: &'phase A,
    _sealed: SealedMarker,
}

impl<'phase, A: Authority> Projection<'phase, A> {
    pub(crate) fn new(inner: &'phase A) -> Self;
    pub fn get(&self) -> &A;
}
```

**Test:**
- Verify `Projection` cannot be constructed outside crate.
- Verify `Projection` does not implement `Clone` or `Copy`.
- Verify lifetime bounds prevent storage.

**Verification:** Attempting to store `Projection` in a struct field fails to compile.

---

### Step 3: AuthorityProvider Trait

**Implement:**
```rust
pub trait AuthorityProvider {
    fn provided_authority(&self) -> Option<(&dyn Any, TypeId)>;
}
```

**Provide default impl returning `None` for Widget.**

**Test:**
- Widget that does not provide authority returns `None`.
- Widget that provides authority returns correct `TypeId`.

**Verification:** Unit tests pass.

---

### Step 4: Authority Registry in Engine

**Implement:**
- Engine maintains mapping from `(NodeId, TypeId)` to authority availability.
- Mapping rebuilt on tree mutation only (not per frame).

**Test:**
- Add widget providing authority → registry updated.
- Remove widget → registry updated.
- No registry update during frame without tree mutation.

**Verification:** Unit tests with mock tree mutations.

---

### Step 5: Ancestor Lookup in Contexts

**Implement:**
Add to `LayoutContext`, `PaintContext`, `EventContext`:
```rust
pub fn authority<A: Authority + 'static>(&self) -> Option<Projection<'a, A>>;
```

**Implementation:**
Walk ancestor chain using registry. Return first match or `None`.

**Test:**
- Child widget calls `authority<TestAuthority>()`.
- Returns `Some` if ancestor provides it.
- Returns `None` if no ancestor provides it.
- Returns nearest ancestor if multiple provide same type.

**Verification:** Unit tests with mock widget tree.

---

### Step 6: Intent Trait

**Implement:**
```rust
pub trait Intent: Any + Send {
    fn as_any(&self) -> &dyn Any;
}
```

**Test:**
- Define test intent type.
- Verify downcast works via `as_any()`.

**Verification:** Compiles, downcast succeeds in unit test.

---

### Step 7: Intent Buffer in Engine

**Implement:**
```rust
struct IntentBuffer {
    intents: Vec<Box<dyn Intent>>,
}

// In engine frame state:
intent_buffers: HashMap<TreeWidgetId, IntentBuffer>,
```

**Test:**
- Buffer created empty.
- Intents pushed into buffer.
- Buffer drained returns all intents.

**Verification:** Unit tests for buffer operations.

---

### Step 8: emit_intent in EventContext

**Implement:**
Add to `EventContext`:
```rust
pub fn emit_intent<A: Authority, I: Intent>(&mut self, intent: I) -> bool;
```

**Implementation:**
1. Lookup ancestor providing `A`.
2. If found, push intent to that ancestor's buffer.
3. Return `true` if pushed, `false` if no ancestor found.

**Test:**
- Emit intent with ancestor present → returns `true`, buffer contains intent.
- Emit intent with no ancestor → returns `false`, no buffer created.

**Verification:** Unit tests with mock tree.

---

### Step 9: AuthorityOwner Trait

**Implement:**
```rust
pub trait AuthorityOwner: Widget {
    fn process_intents(&mut self, intents: Vec<Box<dyn Intent>>);
}
```

**Test:**
- Trait implementable on test widget.
- `process_intents` receives drained buffer contents.

**Verification:** Unit test calling trait method.

---

### Step 10: Deferred Mutation Phase

**Implement:**
Insert new phase between event routing and layout:

1. For each non-empty intent buffer:
   a. Lookup widget by `TreeWidgetId`.
   b. If widget exists and implements `AuthorityOwner`:
      - Drain buffer.
      - Call `widget.process_intents(intents)`.
   c. If widget removed: drop intents silently.
2. Clear all buffers.

**Test:**
- Intent emitted → owner's `process_intents` called after event phase.
- Owner removed before deferred mutation → intent dropped, no panic.
- Multiple owners → each receives only their intents.

**Verification:** Integration test with full frame cycle.

---

### Step 11: Verify sync_scroll Unchanged

**Test:**
- Existing sync_scroll tests pass.
- ScrollArea + ListView virtualization still works.
- No changes to sync_scroll API.

**Verification:** All existing sync_scroll tests green.

---

### Step 12: Verify HorizontalScrollLink Unchanged

**Test:**
- Existing horizontal scroll behavior unchanged.
- No API changes to HorizontalScrollLink.

**Verification:** All existing horizontal scroll tests green.

---

### Step 13: End-to-End Verification

**Test:**
- Build ListView using Authority Projection (TabularSubstrate as authority).
- Verify header/body/footer read column geometry via projection.
- Verify selection intent flow: click → emit intent → owner mutates → children repaint.
- Verify no SPEC-190 invariant violations.

**Verification:** `listview_demo` runs correctly with authority-based implementation.

---

## 21. Post-Implementation Audit

After implementation, verify:

1. No `Rc<RefCell<...>>` for semantic authority anywhere in codebase.
2. All authority types implement `Authority` marker.
3. All authority owners implement `AuthorityOwner`.
4. All children access authority via `cx.authority<A>()`.
5. All mutations flow through `emit_intent` + `process_intents`.
6. No examples in documentation suggest sibling authority access.
7. No examples suggest direct mutation of authority by non-owners.

---

## Appendix A: When SPEC-HAL-190 Is Required (and When It Is Not)

This appendix clarifies the boundary between closed composites (which do NOT need SPEC-HAL-190 runtime machinery) and open composites (which DO).

---

### A.1 Closed Composites: Direct Substrate Passing

A **closed composite** is a widget where:

- All internal structure is fixed at implementation time
- No external widgets are plugged in as children
- Internal organs (header, body, footer, etc.) are implementation details, not extensibility points
- The composite owns and directly passes its substrate to internal phases

**Example: ListView**

ListView is the canonical closed composite:

- Owns a `TabularSubstrate` directly
- Header/body/footer are internal organs, not widget children
- Layout/paint/event phases access substrate via `&self.substrate`
- No `cx.authority<T>()` lookups
- No `cx.emit_intent()` calls
- No `AuthorityOwner` implementation

This is **correct and preferred** for closed composites.

**INV-CLOSED-COMPOSITE-1:**
Closed composites SHOULD use direct substrate ownership and method calls, not SPEC-HAL-190 projection mechanisms.

**INV-CLOSED-COMPOSITE-2:**
ListView, TreeView, and Grid — when implemented as closed composites — are exempt from SPEC-HAL-190 runtime machinery.

---

### A.2 When SPEC-HAL-190 Is REQUIRED

SPEC-HAL-190 projection and intent machinery is **required** only when at least ONE of the following is true:

| Condition | Example |
|-----------|---------|
| **Children are externally pluggable** | A dock panel accepting arbitrary user-defined panes |
| **Multiple heterogeneous widgets must coordinate** | Search panel coordinating list, tree, and grid |
| **Ownership crosses widget boundaries** | Filter bar owned by panel A, consumed by list B |
| **Deferred multi-step interaction semantics** | Drag range spanning tree and list simultaneously |
| **Siblings need shared truth** | Two independent lists sharing selection authority |

In these cases, direct substrate passing is impossible because:

- The substrate owner and consumers are in different widget subtrees
- External code cannot access private `&self.substrate`
- Coordination requires ancestor-mediated authority lookup

**INV-SPEC-190-TRIGGER-1:**
SPEC-HAL-190 machinery is required ONLY when coordination crosses widget ownership boundaries.

---

### A.3 Non-Goal: SPEC-HAL-190 Is Not Internal Plumbing

**SPEC-HAL-190 is infrastructure for cross-composite coordination, not a replacement for direct method calls inside a widget.**

Forbidden reasoning:

> "ListView should use `cx.authority<TabularSubstrate>()` because SPEC-HAL-190 says authority access goes through projections."

This is wrong. SPEC-HAL-190 defines how **children** access **ancestor-owned** authority. ListView owns its substrate — there is no ancestor lookup.

**INV-SPEC-190-SCOPE-1:**
SPEC-HAL-190 mechanisms are for parent→child authority sharing, not for a widget accessing its own fields.

**INV-SPEC-190-SCOPE-2:**
Cargo-culting SPEC-HAL-190 into closed composites adds complexity without benefit and violates the principle of simplest correct implementation.

---

### A.4 Decision Flowchart

```
Does the widget own all semantic state internally?
│
├─ YES → Does it have externally pluggable children?
│        │
│        ├─ NO  → CLOSED COMPOSITE: Direct substrate passing. No SPEC-HAL-190.
│        │
│        └─ YES → OPEN COMPOSITE: SPEC-HAL-190 required for child coordination.
│
└─ NO  → Is the semantic state owned by an ancestor?
         │
         ├─ YES → AUTHORITY CONSUMER: Use cx.authority<T>() per SPEC-HAL-190.
         │
         └─ NO  → ARCHITECTURE ERROR: Clarify ownership before proceeding.
```

---

### A.5 Canonical Classification

| Widget | Type | SPEC-HAL-190 |
|--------|------|-------------|
| ListView | Closed composite | NOT REQUIRED |
| TreeView | Closed composite | NOT REQUIRED |
| Grid | Closed composite | NOT REQUIRED |
| EditorPanel with pluggable panes | Open composite | REQUIRED |
| SearchCoordinator (list + tree + grid) | Coordination root | REQUIRED |
| Filter bar consumed by sibling lists | Authority consumer | REQUIRED |

---

### A.6 Why This Matters

Without this distinction:

1. Future implementers cargo-cult SPEC-HAL-190 into simple widgets
2. Simple composites acquire unnecessary machinery
3. Performance degrades from indirect authority lookups
4. Codebase becomes harder to understand
5. The actual use case for SPEC-HAL-190 (cross-widget coordination) gets diluted

ListView remains the canonical example of "closed composite done right" — sovereign, self-contained, zero external dependencies, maximum simplicity.
