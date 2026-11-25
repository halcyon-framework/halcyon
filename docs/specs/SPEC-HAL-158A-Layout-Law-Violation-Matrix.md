# SPEC-HAL-158A: Layout Law Violation Matrix

**Parent:** SPEC-HAL-158
**Status:** Draft
**Date:** 2026-01-10
**Purpose:** Map current code violations against SPEC-HAL-158 invariants and assign dispositions.

---

## 1. Violation Inventory

### INV-DIRTY-1: Automatic Invalidation on State Mutation

```
Violation:
- src/widgets/text_block.rs:74-78
  Method: set_text()
  Type: Caller-responsible invalidation (documented at lines 92-94)
  Impact: Silent stale layout

- src/widgets/text_block.rs:86-88
  Method: invalidate()
  Type: Public invalidation without tree propagation
  Impact: Silent stale layout
```

---

### INV-DIRTY-2: Parent Invalidation on Child Size Change

```
Violation: NONE FOUND
Implementation at app.rs:1211-1215 honors this invariant.
```

---

### INV-DIRTY-3: Early Exit on Clean State

```
Violation:
- src/widgets/text_block.rs:145-195
  Method: layout()
  Type: No dirty flag check; always recomputes if cache invalid
  Impact: Redundant computation (performance, not correctness)

- src/widgets/label.rs:57-81
  Method: layout()
  Type: No dirty flag check; always measures text
  Impact: Redundant computation

- src/widgets/button.rs:100-154
  Method: layout()
  Type: No dirty flag check
  Impact: Redundant computation

- src/widgets/container.rs:68-132
  Method: layout()
  Type: No dirty flag check
  Impact: Redundant computation

- src/widgets/panel.rs:80-128
  Method: layout()
  Type: No dirty flag check
  Impact: Redundant computation

- src/widgets/legend.rs:94-167
  Method: layout()
  Type: No dirty flag check
  Impact: Redundant computation

- src/widgets/flex.rs (entire layout implementation)
  Type: No dirty flag check at widget level
  Impact: Redundant computation
```

---

### INV-DIRTY-4: Constraint Change Detection

```
Violation:
- All widgets above
  Type: No constraint caching or comparison
  Impact: Cannot skip layout when constraints unchanged
```

---

### INV-DIRTY-5: Upward Propagation Termination

```
Violation: NONE FOUND
Implementation at widget_tree.rs:876-910 honors this invariant.
```

---

### INV-TREE-1: Persistent Node Exclusivity

```
Violation: NONE FOUND
All examined widgets use persistent tree children via take_widget/restore_widget.
```

---

### INV-TREE-2: Dynamic Child Registration

```
Violation: NONE FOUND
List widget uses tree.add_child() at list.rs:2355.
```

---

### INV-TREE-3: Take-Restore Symmetry

```
Violation: NONE FOUND
All take_widget calls have matching restore_widget in same scope.
```

---

### INV-TREE-4: Single Layout Authority

```
Violation: NONE FOUND
Layout traversal ensures single layout per node per pass.
```

---

### INV-ZERO-1: Legal Zero-Size Returns

```
Status: COMPLIANT
- flex_spacer.rs: Returns zero under loose constraints (intentional)
- Initialization states return zero (intentional)
```

---

### INV-ZERO-2: Illegal Zero-Size Returns

```
Violation:
- src/widgets/button.rs:145-147
  Condition: take_widget(self.child) returns None
  Type: Silent zero without diagnostic
  Impact: Widget vanishes silently

- src/widgets/container.rs:119-121
  Condition: take_widget(self.child) returns None
  Type: Silent zero without diagnostic
  Impact: Widget vanishes silently

- src/widgets/panel.rs:120-122
  Condition: take_widget(self.child) returns None
  Type: Silent zero without diagnostic
  Impact: Widget vanishes silently

- src/core/widget.rs:88-90
  Method: layout_child()
  Condition: Node not found
  Type: Silent zero without diagnostic
  Impact: Silent layout failure

- src/core/widget.rs:139-141
  Method: relayout_child_with_constraints()
  Condition: Node not found
  Type: Silent zero without diagnostic
  Impact: Silent layout failure
```

---

### INV-ZERO-3: Zero-Size Classification

```
Violation:
- All sites listed in INV-ZERO-2
  Type: Unclassified zero returns (could be error or intentional)
  Impact: Debugging impossible
```

---

### INV-TEXT-1: Layout-Participating Text

```
Violation: NONE in core widgets
- Label: measures at label.rs:69
- TextBlock: measures via ParagraphLayoutEngine
- TextField: measures text
- Legend: measures label at legend.rs:110
```

---

### INV-TEXT-2: Decorative Text

```
Status: REQUIRES CLASSIFICATION
- src/widgets/action_palette.rs:213-224
  Type: Fixed 600x400 size, renders text without measurement
  Classification: DECORATIVE (acceptable if documented)
```

---

### INV-TEXT-3: Text Classification

```
Violation:
- src/widgets/action_palette.rs
  Type: Unclassified text rendering
  Impact: Contract ambiguity
```

---

### INV-TEXT-4: Measurement-Render Consistency

```
Violation: NONE FOUND
Examined widgets use same parameters in layout and paint.
```

---

### INV-PHASE-1: Animation-Layout-Paint Ordering

```
Violation: NONE FOUND
Frame loop at app.rs:1685-1879 enforces correct ordering.
```

---

### INV-PHASE-2: No Size Mutation During Paint

```
Violation: NONE FOUND
LayoutAuthorityGuard at app.rs:984-1005 enforces this.
```

---

### INV-PHASE-3: Input Before Layout

```
Violation: NONE FOUND
Event routing occurs before build_scene() call.
```

---

### INV-PHASE-4: Layout Authority Scope

```
Violation: NONE FOUND
LayoutAuthorityGuard enforcement exists.
```

---

## 2. Disposition Decisions

### INV-DIRTY-1 Violations

| File | Disposition | Decision |
|------|-------------|----------|
| text_block.rs:74-78 (set_text) | **FIX REQUIRED (CORE)** | — |
| text_block.rs:86-88 (invalidate) | **FIX REQUIRED (CORE)** | — |

---

### INV-DIRTY-3 Violations (Early Exit)

| File | Disposition | Decision |
|------|-------------|----------|
| All widget layout() methods | **GRANDFATHER (TEMPORARY)** | Performance issue, not correctness |

---

### INV-DIRTY-4 Violations (Constraint Detection)

| File | Disposition | Decision |
|------|-------------|----------|
| All widget layout() methods | **GRANDFATHER (TEMPORARY)** | Performance issue, not correctness |

---

### INV-ZERO-2 Violations (Silent Zero)

| File | Disposition | Decision |
|------|-------------|----------|
| button.rs:145-147 | **FIX REQUIRED (CORE)** | — |
| container.rs:119-121 | **FIX REQUIRED (CORE)** | — |
| panel.rs:120-122 | **FIX REQUIRED (CORE)** | — |
| widget.rs:88-90 | **FIX REQUIRED (CORE)** | — |
| widget.rs:139-141 | **FIX REQUIRED (CORE)** | — |

---

### INV-ZERO-3 Violations (Classification)

| File | Disposition | Decision |
|------|-------------|----------|
| All INV-ZERO-2 sites | **FIX REQUIRED (CORE)** | Same as INV-ZERO-2 |

---

### INV-TEXT-3 Violations (Classification)

| File | Disposition | Decision |
|------|-------------|----------|
| action_palette.rs | **GRANDFATHER (TEMPORARY)** | Non-critical widget, fixed size acceptable |

---

## 3. Justifications

### GRANDFATHER (TEMPORARY) Decisions

**INV-DIRTY-3 / INV-DIRTY-4 (all widget layout methods):**
Layout always produces correct results. Missing early-exit is a performance issue, not a correctness issue. Widgets will be optimized later. Cannot be extended without adding dirty flag support.

**INV-TEXT-3 (action_palette.rs):**
ActionPalette is a modal overlay with intentionally fixed dimensions. Text overflow is clipped by design. This is decorative text. Cannot be copied as a pattern for new widgets.

---

## 4. Red Line List — Absolute Blockers

The following violations **MUST** be resolved before:
- New widget development
- ListView resurrection
- Layout-dependent polish work

### Priority 1: Core Invalidation

| Invariant | File | Issue |
|-----------|------|-------|
| INV-DIRTY-1 | text_block.rs | set_text() requires caller invalidation |

**Blocking:** Any widget that contains TextBlock with dynamic content.

---

### Priority 2: Silent Zero Failures

| Invariant | Files | Issue |
|-----------|-------|-------|
| INV-ZERO-2 | button.rs, container.rs, panel.rs, widget.rs | Silent zero on missing child |

**Blocking:** Debugging any "widget disappeared" issue. All new container widgets.

---

### Priority 3: LayoutContext Helper Failures

| Invariant | File | Issue |
|-----------|------|-------|
| INV-ZERO-2 | widget.rs (layout_child, relayout_child_with_constraints) | Silent zero on missing node |

**Blocking:** Any widget using LayoutContext helpers with potentially invalid NodeIds.

---

## 5. Summary

| Category | Count | Disposition |
|----------|-------|-------------|
| No Violation | 12 invariants | — |
| FIX REQUIRED (CORE) | 7 sites | Must resolve before new work |
| GRANDFATHER (TEMPORARY) | Many sites | Performance-only, isolated |
| NUKE | 0 | — |

---

## 6. Resolution Order

1. **INV-DIRTY-1:** TextBlock automatic invalidation
2. **INV-ZERO-2:** Silent zero → debug assert + logging
3. **INV-ZERO-3:** Classification enforcement (follows from #2)

Only after these three:
- New widget work permitted
- ListView resurrection permitted
- Layout polish permitted
