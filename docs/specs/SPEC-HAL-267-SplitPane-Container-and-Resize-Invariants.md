# SPEC-HAL-267: SplitPane Container and Resize Invariants

**Status:** Draft
**Layer:** Widget Contract
**Scope:** SplitPane widget only
**Depends on:** SPEC-HAL-155 (Resizable Separator & Gutter), SPEC-HAL-156 (Flex Slot Resize Binding), SPEC-HAL-008 (Layout Engine), SPEC-HAL-024 (Coordinate Model), SPEC-HAL-094 (Component Geometry), SPEC-HAL-096B (Geometry Tokenization), SPEC-HAL-140 (Focus vs Capture), SPEC-HAL-098 (Cursor & Pointer System), SPEC-HAL-158 (Layout Authority)
**Constrains:** SPEC-HAL-213 (Divider is visual-only — SplitPane does NOT use Divider for its handle)

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the SplitPane container widget.

A SplitPane partitions a rectangular region into **exactly two child panels** separated by a **draggable handle**. The user drags the handle to redistribute space between panels.

This SPEC exists to prevent SplitPane from becoming:
- a generic N-panel layout (use Flex with separators for that)
- a layout hack with implicit size mutation
- a widget that owns layout authority (layout authority belongs to the Flex slot model per SPEC-HAL-156)
- a source of authority bleed between interaction and layout phases

SplitPane is a **two-panel resizable container**. Nothing more.

---

## 1. Conceptual Definition

A **SplitPane** is a container that:

- holds exactly **two children** (leading and trailing panels)
- displays a **draggable handle** between them
- allows the user to **redistribute space** between panels by dragging the handle
- supports **horizontal** (side-by-side) and **vertical** (stacked) orientation

It delegates all resize math to the **Flex slot resize model** (SPEC-HAL-156).
It delegates all capture semantics to the **separator interaction model** (SPEC-HAL-155).

---

## 2. Structural Model

### 2.1 Children

**INV-SPLIT-CHILD-1:** SplitPane accepts exactly **two** children.

- `leading` — first panel (left in horizontal, top in vertical)
- `trailing` — second panel (right in horizontal, bottom in vertical)

**INV-SPLIT-CHILD-2:** Children are opaque widget subtrees. SplitPane imposes no constraints on child content.

**INV-SPLIT-CHILD-3:** If fewer than two children are provided, SplitPane treats the missing panel as zero-size. If more than two are provided, construction fails or excess children are ignored. Exact policy is implementation-defined.

### 2.2 Handle

**INV-SPLIT-HANDLE-1:** The handle is a **SplitPane-internal interactive element**, not a Divider widget (SPEC-HAL-213). Divider is visual-only and non-interactive. The handle is interactive.

**INV-SPLIT-HANDLE-2:** The handle occupies a fixed thickness along the split axis. Thickness is theme-controlled.

**INV-SPLIT-HANDLE-3:** The handle is not a child in the widget tree. It is an internal affordance of SplitPane, painted and hit-tested by SplitPane itself.

---

## 3. Orientation

**INV-SPLIT-ORIENT-1:** SplitPane has exactly one orientation, declared at construction:

- `Horizontal` — panels side by side, handle is a vertical bar, drag axis is X
- `Vertical` — panels stacked, handle is a horizontal bar, drag axis is Y

**INV-SPLIT-ORIENT-2:** Orientation is immutable after construction.

---

## 4. Size Model

### 4.1 Split Position

**INV-SPLIT-POS-1:** The split position is expressed as a **fractional ratio** in the range `[0.0, 1.0]`, representing the proportion of available space allocated to the leading panel.

**INV-SPLIT-POS-2:** Available space = SplitPane allocated size along split axis − handle thickness.

**INV-SPLIT-POS-3:** The initial split ratio is configurable. Default: `0.5`.

### 4.2 Min/Max Constraints

**INV-SPLIT-MINMAX-1:** Each panel may declare:

- `min_size: f32` — minimum size in logical pixels along the split axis
- `max_size: Option<f32>` — optional maximum size

**INV-SPLIT-MINMAX-2:** Constraints are enforced during resize. If a drag would violate a constraint, motion stops at the constraint boundary.

**INV-SPLIT-MINMAX-3:** Total allocated size is always conserved:

```
leading_size + trailing_size + handle_thickness = available_size
```

If both panels hit their min simultaneously, the handle cannot move.

### 4.3 Relationship to Flex Slot Model

**INV-SPLIT-FLEX-1:** Internally, SplitPane manages two logical Flex slots with `MinMax` size models. The handle drag mutates these size models per SPEC-HAL-156.

**INV-SPLIT-FLEX-2:** SplitPane does NOT expose raw Flex slots to the application. The Flex slot model is an internal implementation detail.

**INV-SPLIT-FLEX-3:** After a resize commit, SplitPane marks itself layout-dirty. The next layout pass recomputes child geometry from the updated slot sizes.

---

## 5. Interaction Model

### 5.1 Pointer Interaction

**INV-SPLIT-PTR-1:** The handle has a **hit zone** that may be larger than its visual thickness. The hit zone expansion is theme-controlled.

**INV-SPLIT-PTR-2:** On pointer-down within the hit zone, SplitPane acquires **exclusive pointer capture** (SPEC-HAL-140, SPEC-HAL-155 §F1).

**INV-SPLIT-PTR-3:** During capture, pointer-move events produce a **proposed delta** along the split axis.

**INV-SPLIT-PTR-4:** On pointer-up, capture is released and the resize transaction commits.

### 5.2 Transactional Resize

**INV-SPLIT-TX-1:** Resize follows the transaction model from SPEC-HAL-155 §F2:

1. **Begin** — snapshot current split ratio / slot sizes
2. **Update** — compute proposed sizes from drag delta, clamp to min/max
3. **Commit** — apply new split ratio on pointer-up
4. **Cancel** — on Escape during drag, restore snapshot

**INV-SPLIT-TX-2:** During the Update phase, layout is recomputed each frame with the proposed sizes. This provides **live preview** of the resize.

**INV-SPLIT-TX-3:** Cancel (Escape) restores the exact pre-drag state.

### 5.3 Cursor Feedback

**INV-SPLIT-CURSOR-1:** When the pointer hovers over the handle hit zone, the cursor changes to the appropriate resize cursor (SPEC-HAL-098):

- Horizontal split → `ColResize` (↔ or ‖)
- Vertical split → `RowResize` (↕ or ═)

**INV-SPLIT-CURSOR-2:** During capture, the resize cursor persists regardless of pointer position.

### 5.4 Keyboard Interaction

**INV-SPLIT-KB-1:** When the handle has focus, arrow keys along the split axis move the handle by a **step increment** (theme-controlled, default: 1 grid unit).

**INV-SPLIT-KB-2:** Home/End move the handle to min/max positions respectively.

**INV-SPLIT-KB-3:** The handle is focusable and participates in tab order between the two panels.

Tab order: `leading panel` → `handle` → `trailing panel`.

### 5.5 Double-Click Reset

**INV-SPLIT-DBLCLICK-1:** Double-clicking the handle resets the split ratio to its initial value.

---

## 6. Layout Contract

**INV-SPLIT-LAYOUT-1:** SplitPane performs layout as a two-slot Flex container along the split axis:

1. Subtract handle thickness from available size
2. Distribute remaining space per current split ratio, clamped to min/max
3. Position leading panel at origin
4. Position handle after leading panel
5. Position trailing panel after handle

**INV-SPLIT-LAYOUT-2:** Cross-axis sizing: each panel receives the full SplitPane extent on the cross axis (no cross-axis splitting).

**INV-SPLIT-LAYOUT-3:** SplitPane respects incoming layout constraints. If the parent constrains SplitPane to a size smaller than both panels' min sizes + handle, the panels are clamped proportionally.

---

## 7. Visual Rendering

### 7.1 Handle Rendering

**INV-SPLIT-VIS-1:** The handle visual is entirely theme-controlled:

- idle appearance (line, gutter, grip dots — theme decides)
- hovered appearance
- pressed/dragging appearance
- focused appearance

**INV-SPLIT-VIS-2:** The handle MUST provide a visible affordance in at least the hovered state. A completely invisible handle with no hover feedback is forbidden — the user must be able to discover it.

### 7.2 Panel Rendering

**INV-SPLIT-VIS-3:** SplitPane does not render panel backgrounds. Children are responsible for their own visual appearance.

**INV-SPLIT-VIS-4:** SplitPane clips each panel to its allocated region.

---

## 8. State Model

**INV-SPLIT-STATE-1:** SplitPane internal state consists of:

- `split_ratio: f32` — current ratio (persistent across rebuilds)
- `orientation: Orientation` — immutable after construction
- `drag_state: Option<DragState>` — ephemeral, exists only during active drag

**INV-SPLIT-STATE-2:** `split_ratio` is widget-local state. It survives rebuilds per the identity model (SPEC-HAL-087, SPEC-HAL-148).

**INV-SPLIT-STATE-3:** `drag_state` is ephemeral. It is never persisted, never serialized, and is dropped when capture ends.

**INV-SPLIT-STATE-4:** SplitPane MAY expose a callback for split ratio changes so the application can persist the ratio externally. This callback is advisory — SplitPane remains the authoritative owner of `split_ratio` during its lifetime.

---

## 9. Accessibility

**INV-SPLIT-A11Y-1:** The handle exposes an accessibility role of `Splitter` (or platform equivalent).

**INV-SPLIT-A11Y-2:** The handle exposes the current split ratio as a value (e.g., percentage).

**INV-SPLIT-A11Y-3:** The handle supports accessible increment/decrement actions mapped to arrow key behavior (§5.4).

---

## 10. Forbidden Behavior

**FORBID-SPLIT-1:** SplitPane must NOT support more than two children. N-way splitting requires nested SplitPanes or a dedicated multi-split container (future SPEC).

**FORBID-SPLIT-2:** SplitPane must NOT collapse a panel to zero size unless the panel's `min_size` is explicitly `0.0`. Implicit collapse is forbidden.

**FORBID-SPLIT-3:** SplitPane must NOT mutate child widget state. It only controls geometry allocation.

**FORBID-SPLIT-4:** SplitPane must NOT paint into child regions. Each panel is an independent paint subtree.

**FORBID-SPLIT-5:** SplitPane must NOT cache or store child layout results. All layout products are ephemeral per SPEC-HAL-002.

**FORBID-SPLIT-6:** SplitPane must NOT act as a focus scope. It is a transparent container for focus traversal, with the handle as an additional tab stop.

---

## 11. Nesting

**INV-SPLIT-NEST-1:** SplitPanes may be nested. A panel of one SplitPane may contain another SplitPane as its child.

**INV-SPLIT-NEST-2:** Nested SplitPanes are independent. No coordinated resize, no coupled constraints.

**INV-SPLIT-NEST-3:** Orientation of nested SplitPanes is unrestricted. Horizontal inside vertical (and vice versa) is valid.

---

## 12. Collapsible Panels (Optional Extension)

SplitPane MAY support an optional **collapsible** mode where one panel can be collapsed to zero (or near-zero) size, with a toggle affordance on the handle.

If supported:

**INV-SPLIT-COLLAPSE-1:** Collapse is an explicit user action (click on collapse affordance or keyboard shortcut). Never automatic.

**INV-SPLIT-COLLAPSE-2:** Collapse stores the pre-collapse ratio so it can be restored on expand.

**INV-SPLIT-COLLAPSE-3:** A collapsed panel receives zero layout space but remains in the widget tree (not removed).

If not supported in initial implementation, this section is deferred.

---

## 13. Non-Goals (Explicit)

This SPEC does NOT define:

- N-way split containers (three or more panels)
- Tabbed panel switching within a split region
- Drag-to-rearrange panel ordering
- Serialization of split state to disk
- Animated resize transitions (may be layered later per SPEC-HAL-097)

---

## 14. Construction API (Non-Normative)

Indicative API shape:

```rust
SplitPane::horizontal(leading, trailing)
    .initial_ratio(0.3)
    .leading_min(200.0)
    .trailing_min(150.0)
    .on_ratio_changed(|ratio| { /* persist */ })
```

Or vertical:

```rust
SplitPane::vertical(top, bottom)
    .initial_ratio(0.5)
```

Exact API is implementation-defined. This section is non-normative.

---

## 15. Summary

- SplitPane is a **two-panel resizable container**
- Handle is an internal interactive affordance, not a Divider
- Resize operates on Flex slot size models (SPEC-HAL-156)
- Capture and transaction semantics from SPEC-HAL-155
- Split ratio is widget-local state, persistent across rebuilds
- Theme controls all visual appearance
- Exactly two children, one orientation, one handle
- Nesting is supported and uncoupled

---

## 16. Divider visibility interaction policy (2026-11-22)

**Status: Adopted / Binding for this section only.** JC requires configurable
divider visibility and selects a visible resting divider for Showcase Workbench.
The remainder of this document and HAL-155 retain their declared Draft status.
HAL-002/002A, HAL-012 and existing input, layout and retained owners are unchanged.

**INV-SPLIT-VIS-POLICY:** SplitPane owns the explicit construction-time
`SplitPaneInteractionPolicy`, whose `divider_visibility` is a
`SplitPaneDividerVisibility` value:

- `OnInteraction`: preserve the current hidden idle state; show the existing
  line while hovered, dragging or displaying its existing focus presentation.
- `Always`: show that same line at rest as well as during those states.

The framework constructor default is `OnInteraction`, preserving existing
consumers. `SplitPane::with_interaction_policy` selects the value before mounting.
The policy is per instance; nesting creates no inheritance or shared state.
This section adds no mounted mutation API or application-wide policy switch.

**INV-SPLIT-VIS-INPUT-INDEPENDENT:** Visibility changes only the admission of
existing handle paint. Both choices preserve the current 3-logical-unit gutter,
1-unit centered visible line and 9-unit hit band, both orientations, allocations,
ratios, minimum sizes, clipping, cursor, drag Capture, cancellation, double-click
reset, keyboard actions and Focus presentation. A hidden line remains hit-testable
under the existing input contract. No synthetic hover or new interactive child
is allowed.

HAL-012 retains appearance authority. The existing state colors remain:
`theme.colors.borders.default` at rest, `borders.strong` while hovered, and
`accents.primary` while dragging, with the existing independent focus decoration.
No visibility policy may change metrics, invent Focus/Capture facts, paint panel
backgrounds or enter the application's keyboard `UiInteractionPolicy`.

Showcase Workbench selects `Always` for its four actual final SplitPane
constructions: navigator/workspace, center/inspector, source/presets and
pipeline/transforms. Placeholder construction alone is not completion.

Verification uses the existing SplitPane owner tests to retain hidden-mode
coverage and prove visible idle output in both orientations, unchanged geometry
and hit eligibility, and existing hover/drag/keyboard behavior. The ordinary
Workbench must visibly retain its dividers after the pointer leaves them.
