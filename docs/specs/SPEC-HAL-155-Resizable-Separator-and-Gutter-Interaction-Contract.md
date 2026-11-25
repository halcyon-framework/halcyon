# SPEC-HAL-155: Resizable Separator & Gutter Interaction Contract

**Status:** Draft
**Layer:** Layout + Interaction Primitives
**Scope:** Engine-level layout interaction, not a widget
**Depends on:** SPEC-HAL-024 (Coordinate Model), SPEC-HAL-038 (Layout), SPEC-HAL-140 (Focus vs Capture)
**Constrains:** No widget-specific behavior, no rendering policy

---

## SECTION A — Problem Statement

Modern UI layouts require **interactive partitioning** of space:

* split panes
* resizable columns / rows
* adjustable gutters inside flex layouts

Historically this is implemented as ad-hoc widgets.
That approach fails because:

* resizing is **layout authority**, not widget policy
* resizing must interact with **flex resolution**
* resizing must respect **min/max constraints**
* resizing must be **transactional and reversible**

This SPEC defines a **Resizable Separator** as a **layout interaction primitive**, not a widget.

---

## SECTION B — Definitions

### B1. Separator

A **Separator** is a **logical boundary** between two adjacent layout regions.

It has:

* orientation
* position
* interaction zone (gutter / thumb)
* no intrinsic visual identity

### B2. Gutter

A **Gutter** is the interactive region associated with a separator.

* receives pointer interaction
* owns resize capture
* does not own layout

### B3. Target Region

A **Target Region** is a layout child whose **allocated size** is affected by a separator.

---

## SECTION C — Orientation Model

**INV-SEP-ORIENT-1:** A separator has exactly one orientation:

* `VerticalSeparator` → resizes **width**
* `HorizontalSeparator` → resizes **height**

Orientation determines:

* pointer axis used
* layout axis affected

---

## SECTION D — Layout Attachment Model

**INV-SEP-ATTACH-1:** A separator attaches to exactly one layout container.

**INV-SEP-ATTACH-2:** A separator references **two adjacent layout slots**:

* `before`
* `after`

**INV-SEP-ATTACH-3:** Separator never owns children.

---

## SECTION E — Size Authority Model

### E1. Resizable Slot

A layout slot may declare itself **resizable**.

Each resizable slot exposes:

* `initial_size`
* `min_size`
* `max_size` (optional)
* `flex_weight` (optional)

**INV-SEP-SIZE-1:** Separator modifies **slot size inputs**, not final layout output.

---

### E2. Resize Semantics

**INV-SEP-RESIZE-1:** Dragging a separator produces a **proposed size delta**.

**INV-SEP-RESIZE-2:** Proposed size is clamped by:

* min_size
* max_size (if present)

**INV-SEP-RESIZE-3:** Resize does NOT implicitly redistribute size to other slots.

Redistribution, if desired, is explicit policy (out of scope here).

---

## SECTION F — Interaction Model

### F1. Capture

**INV-SEP-CAP-1:** Separator gutter acquires **exclusive pointer capture** during drag.

**INV-SEP-CAP-2:** Focus is irrelevant (SPEC-HAL-140).

---

### F2. Transactional Resize

**INV-SEP-TX-1:** Resize is transactional.

Phases:

1. **Begin** — snapshot slot sizes
2. **Update** — compute proposed sizes
3. **Commit** — apply to layout state
4. **Cancel** — discard snapshot

**INV-SEP-TX-2:** No layout mutation occurs before commit.

---

## SECTION G — Flex Interaction Rules

**INV-SEP-FLEX-1:** Separator operates on **resolved slot size inputs**, not raw flex weights.

**INV-SEP-FLEX-2:** After commit:

* slot may become fixed-size
* or flex-participating with updated base size

Exact flex reconciliation is container policy.

---

## SECTION H — Forbidden Behavior

**FORBID-SEP-1:** Separator must not:

* paint itself
* create widgets
* own layout children

**FORBID-SEP-2:** Separator must not:

* directly mutate layout during drag
* bypass min/max constraints

**FORBID-SEP-3:** Separator must not redistribute size implicitly.

---

## SECTION I — Relationship to Widgets

A widget **may visually represent** a separator or gutter, but:

**INV-SEP-WIDGET-1:** Widgets are **views**, not authorities.

All resize logic lives at:

* layout + interaction layer

---

## SECTION J — Non-Goals

This SPEC does **not** define:

* visual appearance
* snapping
* animation
* persistence
* multi-separator coupling

Those belong to higher-level components.

---

## SECTION K — Motivation (Why This Exists)

This SPEC allows:

* ListView column resizing
* Split panes
* Adjustable dashboards
* IDE-style layouts

…without turning layout into widget spaghetti.

---

## SECTION L — Status

Draft.
No implementation required yet.
