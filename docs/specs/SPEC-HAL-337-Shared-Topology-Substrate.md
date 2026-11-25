# SPEC-HAL-337: **Shared Topology Substrate — HTS Ownership and Mutation Law**

**Author:** JC
**Status:** Draft (Binding upon adoption)
**Amends:** SPEC-HAL-259 (HierarchicalTabularSubstrate) — ownership and consumer language
**Depends on:** SPEC-HAL-002 (Constitution), SPEC-HAL-082A (MWA Amendment), SPEC-HAL-161
(Cross-Window Communication), SPEC-HAL-259, SPEC-HAL-135/136 (selection law),
SPEC-HAL-260A §12 (PE intent funnel — conforming precedent), SPEC-HAL-335 §10 (temporal
intent law — conforming precedent)

---

# 1. Purpose and Principle

The row topology (`HierarchicalTabularSubstrate`) ceases to be a widget's private organ
and becomes an **application-level shared substrate**. Widgets — TreeView, PropertyEditor,
TimelineCanvas compositions, any future view — are projections over it.

This is the composition idiom the framework has already adopted (`TemporalSubstrate`:
app-owned `Rc<RefCell<…>>`, thin widget clients) promoted from private pairs
(TreeViewCore, ListViewCore) to a public N-consumer contract.

Sovereignty alignment: under SPEC-HAL-082A, windows own no widget trees, layout roots, or
semantic state. State that multiple views — eventually in multiple windows — must project
cannot lawfully live inside one window's widget tree. The application is its home.

# 2. Ownership Model

```rust
pub type SharedHts = std::rc::Rc<std::cell::RefCell<HierarchicalTabularSubstrate>>;
```

**Policy type (mandatory per RFC law):**

```rust
/// Who constructed the topology this widget projects.
pub enum HtsOwnership {
    /// Widget minted its own substrate at construction and exposes the handle.
    Internal,
    /// Substrate was injected; the application (or another composer) owns construction.
    External,
}
```

- **INV-337-O1** — Exactly one `HierarchicalTabularSubstrate` instance exists per topology.
  Two views of one hierarchy share one handle; never two substrates reconciled.
- **INV-337-O2** — Post-migration, the handle is the only ownership form. No by-value
  `HierarchicalTabularSubstrate` field exists in any widget, core, or engine
  (`CellMechanicsSubstrate` holds `SharedHts`).
- Construction freedom: `TreeView::new()` mints a substrate (`HtsOwnership::Internal`) and
  exposes it via `hts_handle() -> SharedHts`; `TreeView::with_hts(SharedHts)` accepts one
  (`External`). Single-view code keeps its one-line ergonomics; multi-view code injects.
  The substrate is always behind the handle — one code shape, no dual paths.

# 3. Access Law

- Accessors return frame-ephemeral guards: `CMS::hts() -> Ref<'_, Hts>`,
  `hts_mut() -> RefMut<'_, Hts>`. Auto-deref preserves existing call-chain syntax.
- **INV-337-A1** — No guard crosses a phase boundary or is stored in any struct
  (Constitution §3.6/§4.1). Trait-object arguments take `&*guard` at the call site.
- **FORBID-337-1** — The shared substrate never stores callbacks, observers, widget IDs it
  notifies, or references capturing tree or state (Constitution §3.4, Pillar C). It remains
  what SPEC-HAL-259 §13 made it: no rendering API, no UI events, no gesture handling.

# 4. Mutation Law

The ownership exemption is abolished. The lawful channel depends on the mutation class,
uniformly for **all** consumers including TreeView itself.

**4.1 Structural state** — topology (expand/collapse/toggle, `set_tree_data`), column
model structure (add/remove/reorder), row heights, anything that changes row count,
order, or content extent:

- **INV-337-M1** — MUST NOT be mutated during event routing (Constitution §3.2). Event
  handlers enqueue an intent; application of the intent occurs in the deferred phase
  (Phase 2) or the per-frame application hook, before layout.
- Conforming precedent: PropertyEditorCore `dispatch_intent` / INV-PE-INTENT-FUNNEL
  (SPEC-HAL-260A §12) — already routes all hts-topology mutation through one funnel.
- Non-conforming code, named: TreeViewBody's synchronous `toggle_by_id` inside the
  DoubleClick arm, the disclosure-toggle activation path, and TreeView keyboard
  expand/collapse. These migrate onto a TreeView topology-intent funnel mirroring PE's.

**4.2 Interaction state** — vertical scroll, viewport dimensions, hover, selection, and
active column resize (JC ruling, 2026-06-12: the resize drag is a gesture — live width
streaming preserves the established preview behavior; the gesture ends at
`finalize_resize` / `cancel_resize`):

- **INV-337-M2** — Event-phase writes remain lawful through their existing single channels
  (SPEC-HAL-135 commit-on-press for selection; viewport sync at layout for layout-domain
  inputs, per the established TreeViewBody / TimelineCanvas pattern; live width streaming
  during an active resize drag through the existing resize-intent channel, with
  `finalize_resize` / `cancel_resize` ending the gesture — Esc rolls back to the
  pre-drag width).
- **INV-337-S1** — The substrate's `scroll_y` remains the single vertical-scroll authority
  (INV-TV-5, generalized). Exactly one application path writes it per pane — the existing
  ScrollArea sync. **FORBID-337-4** — no second concurrent scroll writer; additional
  gesture sources (e.g. wheel over a timeline pane) forward requests into that path.

# 5. Read Surface for Multi-Consumer Projection

The virtualization already computes the visible window's y-offset and discards it. With N
consumers deriving row geometry, the substrate publishes it:

```rust
pub struct VisibleWindow {
    pub rows: Vec<VisibleRow>,
    /// Content-space y of rows[0] (the value compute_visible_range already produces).
    pub first_y: f32,
}
pub fn visible_window(&mut self) -> VisibleWindow;
```

- **INV-337-R1** — Consumers derive per-row geometry exclusively from substrate-published
  values (`visible_window`, `row_height`, `scroll_offset`). No consumer re-implements the
  virtualization arithmetic (SPEC-PM-009).
- **FORBID-337-2** — Timeline widgets never hold `SharedHts`. The TimelineCanvas
  row-geometry seam (`set_row_geometry`, SPEC-HAL-335 §7 as implemented) remains the
  projection boundary; an adapter (screen-level or `engines::temporal::hts_rows`) derives
  `RowGeometry` from the shared substrate per frame. The canvas stays provider-agnostic —
  free mode, fixed lists, and HTS all feed one seam.

# 6. Invalidation and Propagation

- **INV-337-P1** — Intra-window propagation is pull: consumers compare `revision()` per
  frame. No push, no observers.
- **INV-337-X1** — Cross-window propagation, when per-window widget trees land, uses
  SPEC-HAL-161 signals exclusively: pure-data envelopes (e.g. topology revision), explicit
  addressing, delivered in the dedicated signal phase. Never a shared guard held across a
  window boundary, never an OS-callback write (082A §3).

# 7. Cross-Window Scope

Single engine instance, single UI thread — `Rc` being `!Send` enforces this at compile
time. Sharing one `SharedHts` across multiple windows' trees is lawful under 082A (windows
own nothing) once the per-window-widget-tree App slice exists; until then this SPEC's law
applies within one window's tree. This SPEC defines the law that slice lands on; it does
not implement the slice.

# 8. Migration Mandates (ordered; each stage independently gated)

1. **CMS handle-ization** — `hts: SharedHts`; `new(hts: SharedHts)`; guard-returning
   accessors; `&*` touches at trait-object call sites (selection oracle et al.).
2. **TreeView ownership surface** — `HtsOwnership`, `hts_handle()`, `with_hts()`;
   TreeViewCore/Body choreography.
3. **TreeView topology-intent funnel** — all structural mutation entry points enumerated
   and routed per §4.1; deferred application point established.
4. **PropertyEditor v2 choreography** — handle plumbing only; its funnel already conforms.
5. **`visible_window()` accessor** on HTS per §5.
6. **Consumers attach** — the bound Gantt composition lands directly on `SharedHts` via
   the §5 surface and the §FORBID-337-2 adapter. No bridge delegates, no migration debt.

# 9. Out of Scope

Per-window widget-tree implementation (MWA App slice); ScrollArea gesture plumbing
redesign; unification of row selection across views (open question — ruled separately
before stage 6 composes selection-bearing views).

# 10. Amendments Mandated Upon Adoption

- SPEC-HAL-259: ownership language ("local" substrate, widget-constructed) → handle-owned
  per §2; consumer section gains §4 mutation law reference.
- SPEC-HAL-335 §7: bound-mode row text cites this SPEC as the row-geometry source feeding
  the seam.
- SPEC-HAL-336 §8: bound Gantt composition rewritten to the §8.6 shape.
