# SPEC-HAL-158: Layout Authority and Invalidation Law

**Author:** Halcyon Architect
**Status:** Draft
**Date:** 2026-01-10
**Scope:** Core Layout System, Widget Tree, Text Measurement
**Purpose:** Define non-negotiable invariants for layout correctness.
**Frame-order authority:** Read subject to binding SPEC-HAL-021 and SPEC-HAL-042.

---

## 1. Purpose

This SPEC establishes **binding invariants** for layout, invalidation, widget tree integrity, text measurement authority, and frame phase ordering.

These invariants are **law**. Violations invalidate layout correctness.

---

## 2. Layout Invalidation Law

### INV-DIRTY-1: Automatic Invalidation on State Mutation

**REQUIRED:**
Any widget that mutates internal state affecting layout size MUST automatically call `cx.invalidate_layout()` or equivalent tree invalidation.

**FORBIDDEN:**
Caller-responsible invalidation. No widget may rely on external callers to propagate layout dirtiness after internal state mutation.

**VIOLATION:**
A widget method that:
- Mutates state affecting layout (text content, padding, font size, child references)
- Does NOT call invalidation within the same method
- Documents "caller must invalidate" as the contract

---

### INV-DIRTY-2: Parent Invalidation on Child Size Change

**REQUIRED:**
When a child widget's computed size differs from its previous size, the parent MUST be marked dirty.

**FORBIDDEN:**
Silent size changes. No widget may return a different size without triggering parent invalidation.

**VIOLATION:**
A layout pass where:
- Child returns `size != cached_size`
- Parent is not marked dirty
- Includes error paths (missing node, invalid child)

---

### INV-DIRTY-3: Early Exit on Clean State

**REQUIRED:**
When a widget is not dirty AND constraints match cached constraints, layout MUST return cached size without recomputation.

**FORBIDDEN:**
Unconditional layout recomputation. No widget may ignore its dirty flag.

**VIOLATION:**
A `layout()` implementation that:
- Ignores `dirty_layout == false`
- Recomputes size unconditionally
- Performs child layout when not dirty

---

### INV-DIRTY-4: Constraint Change Detection

**REQUIRED:**
When incoming constraints differ from last-cached constraints, the widget MUST be considered dirty regardless of `dirty_layout` flag.

**FORBIDDEN:**
Ignoring constraint changes. No widget may return stale cached size when constraints changed.

**VIOLATION:**
A layout pass where:
- `constraints != last_constraints`
- Widget returns cached size without recomputation

---

### INV-DIRTY-5: Upward Propagation Termination

**REQUIRED:**
Dirty flag propagation MUST stop at an already-dirty ancestor or the root.

**FORBIDDEN:**
Unbounded propagation. No propagation loop may traverse the same node twice.

**VIOLATION:**
An infinite loop or redundant traversal during `mark_layout_dirty()`.

---

## 3. Widget Tree Authority Law

### INV-TREE-1: Persistent Node Exclusivity

**REQUIRED:**
Only widgets registered in WidgetTree via `add_child()` or equivalent API may participate in:
- Layout pass
- Paint pass
- Hit-testing

**FORBIDDEN:**
Ephemeral widget instances. No widget may create temporary widget objects during layout or paint that are not registered in the tree.

**VIOLATION:**
A widget that:
- Instantiates `Box<dyn Widget>` during `layout()` or `paint()`
- Calls methods on that instance
- Does not register it via `tree.add_child()`

---

### INV-TREE-2: Dynamic Child Registration

**REQUIRED:**
Dynamic child creation (virtualization, lazy loading) MUST use `tree.add_child()` to register children before layout.

**FORBIDDEN:**
Unregistered child layout. No widget may call `layout()` on a widget not in the tree.

**VIOLATION:**
Calling `child_widget.layout()` where `child_widget` is not reachable via tree traversal from root.

---

### INV-TREE-3: Take-Restore Symmetry

**REQUIRED:**
Every `tree.take_widget(node_id)` that succeeds MUST be followed by exactly one `tree.restore_widget(node_id, ...)` before the calling `layout()` returns.

**FORBIDDEN:**
Orphaned takes. No widget may leave a node in the "taken" state after layout completes.

**VIOLATION:**
A `layout()` that:
- Calls `take_widget()` successfully
- Returns without calling `restore_widget()`
- OR calls `restore_widget()` with a different node_id

---

### INV-TREE-4: Single Layout Authority

**REQUIRED:**
Each widget's `layout()` is called exactly once per layout pass, by its parent.

**FORBIDDEN:**
Multiple layout calls. No widget may be laid out more than once per frame (except explicit second-pass relayout for Flex alignment).

**VIOLATION:**
A widget receiving multiple `layout()` calls with identical constraints in the same frame (excluding documented relayout paths).

---

## 4. Zero-Size Semantics Law

### INV-ZERO-1: Legal Zero-Size Returns

**LEGAL:**
- `FlexSpacer` under loose constraints (basis computation)
- Empty container with no child (initialization state)
- Widget explicitly designed to be invisible

**REQUIRED:**
Zero-size returns for legal cases MUST NOT trigger parent invalidation (they are intentional).

---

### INV-ZERO-2: Illegal Zero-Size Returns

**ILLEGAL:**
- `take_widget()` failure due to invalid node_id
- Missing child reference
- Layout error recovery paths

**REQUIRED:**
Zero-size returns for illegal cases MUST:
- Trigger a debug assertion in debug builds
- Log a warning with node identification
- Mark parent dirty (size change from non-zero to zero)

**VIOLATION:**
Silent zero-size return from an error path without diagnostic output.

---

### INV-ZERO-3: Zero-Size Classification

**REQUIRED:**
Every `Size::zero()` return site MUST be classifiable as either:
- INTENTIONAL: Widget designed to return zero (spacers, empty containers)
- ERROR: Failure to obtain valid child/node data

**FORBIDDEN:**
Ambiguous zero returns. No `Size::zero()` may be unclassified.

**VIOLATION:**
A `Size::zero()` return that could be either intentional or error depending on runtime state, without explicit branching.

---

## 5. Text Measurement Authority Law

### INV-TEXT-1: Layout-Participating Text

**REQUIRED:**
Any widget whose layout size depends on text content MUST measure text during `layout()` using `text_engine.measure()` or equivalent.

**FORBIDDEN:**
Size assumptions. No widget may assume text dimensions without measurement.

**VIOLATION:**
A widget that:
- Returns a size
- That size should depend on text content
- No `text_engine.measure()` call occurs in `layout()`

---

### INV-TEXT-2: Decorative Text

**LEGAL:**
Widgets that render text purely for decoration (fixed-size containers with clipped text) MAY skip measurement if:
- The widget's size is independent of text content
- Text overflow is explicitly acceptable

**REQUIRED:**
Decorative text widgets MUST be explicitly marked or documented as such.

---

### INV-TEXT-3: Text Classification

**REQUIRED:**
Every widget that calls `text_engine.draw()` in `paint()` MUST be classifiable as:
- LAYOUT-PARTICIPATING: Text measured in `layout()`, size depends on text
- DECORATIVE: Text not measured, size independent of text, overflow acceptable

**FORBIDDEN:**
Unclassified text rendering. No widget may render text without explicit classification.

**VIOLATION:**
A widget that:
- Calls `text_engine.draw()` in `paint()`
- Has no corresponding `text_engine.measure()` in `layout()`
- Is not documented as decorative

---

### INV-TEXT-4: Measurement-Render Consistency

**REQUIRED:**
If text is measured in `layout()`, the same text content and font parameters MUST be used in `paint()`.

**FORBIDDEN:**
Measurement-render mismatch. No widget may measure one string and render another.

**VIOLATION:**
- `layout()` measures "Hello" at 14px
- `paint()` draws "Hello World" at 16px

---

## 6. Frame Phase Integrity Law

### INV-PHASE-1: Animation-Layout-Paint Ordering

**REQUIRED:**
SPEC-HAL-042 owns the complete frame order. Within its layout-relevant suffix,
after captured time, pre-routing scroll visual state, and input fusion:
1. Animation value advancement (Timeline.advance)
2. Event routing may start/cancel handles at the captured frame time
3. Widget update samples Timeline read-only and publishes owner state
4. Layout pass (layout_tree)
5. Authorized post-layout geometry-transition projection
6. Custom-render preparation from immutable owner state
7. Paint pass (paint_tree)

**FORBIDDEN:**
Phase interleaving. No animation advancement during layout or paint.

**VIOLATION:**
- `Timeline.advance()` called between `layout_tree()` and `paint_tree()`
- Animation values modified during `layout()` or `paint()`

---

### INV-PHASE-2: No Size Mutation During Paint

**REQUIRED:**
All widget sizes and positions MUST be finalized before paint begins.

**FORBIDDEN:**
Layout mutation during paint. No widget may modify its rect, size, or position during `paint()`.

**VIOLATION:**
A `paint()` implementation that:
- Calls `tree.set_rect()`
- Modifies `self.bounds` or equivalent size state
- Triggers layout invalidation

---

### INV-PHASE-3: Input Before Layout

**REQUIRED:**
All input events MUST be processed before layout begins.

**FORBIDDEN:**
Mid-layout input processing. No event routing during layout pass.

**VIOLATION:**
An event handler executing between the start and end of `layout_tree()`.

---

### INV-PHASE-4: Layout Authority Scope

**REQUIRED:**
Rect mutations via `set_rect()` or equivalent MUST occur only within layout phase, under layout authority.

**FORBIDDEN:**
Out-of-phase rect mutation. No rect changes outside layout phase.

**VIOLATION:**
- `set_rect()` called during paint
- `set_rect()` called during event handling
- `set_rect()` called during animation advancement

---

## 7. Enforcement Expectations

### 7.1 Invariant Status

These invariants are **enforceable**. They define correctness.

### 7.2 Violation Consequences

Any code that violates these invariants:
- Produces undefined layout behavior
- May cause visual corruption
- May cause infinite loops
- Is considered **incorrect** regardless of apparent functionality

### 7.3 Conformance Requirement

Future code MUST conform to these invariants.

Existing code that violates these invariants:
- MUST be identified
- MUST be scheduled for remediation or removal
- MAY NOT be extended or copied

### 7.4 No Grandfathering by Default

Legacy code does not receive automatic exemption. Exemptions require explicit SPEC amendment with justification.

---

## 8. Invariant Index

| ID | Domain | Summary |
|----|--------|---------|
| INV-DIRTY-1 | Invalidation | Automatic invalidation on state mutation |
| INV-DIRTY-2 | Invalidation | Parent dirty on child size change |
| INV-DIRTY-3 | Invalidation | Early exit on clean state |
| INV-DIRTY-4 | Invalidation | Constraint change detection |
| INV-DIRTY-5 | Invalidation | Upward propagation termination |
| INV-TREE-1 | Widget Tree | Persistent node exclusivity |
| INV-TREE-2 | Widget Tree | Dynamic child registration |
| INV-TREE-3 | Widget Tree | Take-restore symmetry |
| INV-TREE-4 | Widget Tree | Single layout authority |
| INV-ZERO-1 | Zero-Size | Legal zero-size returns |
| INV-ZERO-2 | Zero-Size | Illegal zero-size returns |
| INV-ZERO-3 | Zero-Size | Zero-size classification |
| INV-TEXT-1 | Text Measurement | Layout-participating text |
| INV-TEXT-2 | Text Measurement | Decorative text |
| INV-TEXT-3 | Text Measurement | Text classification |
| INV-TEXT-4 | Text Measurement | Measurement-render consistency |
| INV-PHASE-1 | Frame Phase | Animation-Layout-Paint ordering |
| INV-PHASE-2 | Frame Phase | No size mutation during paint |
| INV-PHASE-3 | Frame Phase | Input before layout |
| INV-PHASE-4 | Frame Phase | Layout authority scope |

---

## 9. Cross-References

| Invariant | Related SPEC | Status |
|-----------|-------------|--------|
| INV-DIRTY-1..5 | SPEC-HAL-026 | Extends (adds enforcement) |
| INV-TREE-1..4 | SPEC-HAL-087 | Extends (adds exclusivity) |
| INV-TEXT-1..4 | SPEC-HAL-011 | Extends (adds authority) |
| INV-PHASE-1..4 | SPEC-HAL-141 | Formalizes (adds invariants) |

---

## 10. Changelog

| Version | Date | Changes |
|---------|------|---------|
| 1.0 | 2026-01-10 | Initial invariant definition |
