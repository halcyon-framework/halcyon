# SPEC-HAL-124

**Keyboard Type-Ahead Search Contract**

**Status:** Design — Approved
**Scope:** Incremental keyboard-driven item navigation via prefix matching
**Applies to:** List, Tree, VirtualizedListView, VirtualizedTreeView
**Layer:** Interaction / Navigation
**Depends on:**

* SPEC-HAL-114 (List & ListItem Selection Model)
* SPEC-HAL-116 (Tree & TreeItem Interaction Model)
* SPEC-HAL-118 (Virtualized ListView & TreeView Contract)

**Non-Goals:**

* Text input fields
* Full-text search UI
* IME composition
* Accessibility narration (handled by SPEC-HAL-125)

---

## 1. Purpose

Type-ahead search enables **fast, keyboard-only navigation** by typing character sequences that match item labels.

This SPEC defines a **shared, deterministic contract** for type-ahead behavior across list- and tree-based widgets.

If type-ahead changes selection, expands nodes, or activates items implicitly, the implementation is **incorrect by definition**.

---

## 2. Core Definition

### 2.1 Type-Ahead Search

A transient interaction mode where:

* sequential character keypresses form a **prefix buffer**
* the buffer is matched against visible item labels
* focus moves to the next matching item

Type-ahead is **navigation**, not editing.

---

## 3. Eligibility Rules

### INV-TA1 — Explicit Enablement

Type-ahead search:

* MUST be explicitly enabled per widget
* default state is **disabled**

No widget gains type-ahead implicitly.

---

### INV-TA2 — Eligible Widgets Only

Type-ahead MAY be enabled only for widgets that:

* have a linear or hierarchical item sequence
* expose stable item identity
* support keyboard focus

---

## 4. Prefix Buffer Semantics

### INV-TA3 — Prefix Accumulation

On character input:

* characters append to a prefix buffer
* buffer is case-insensitive
* buffer is matched from its start

---

### INV-TA4 — Timeout Reset

The prefix buffer:

* resets after a configurable timeout
* default timeout: **500 ms**
* timeout restarts on each valid character

---

### INV-TA5 — Non-Characters Ignored

Non-character keys:

* MUST NOT affect prefix buffer
* MUST NOT trigger search

Examples:

* arrows
* modifiers
* function keys
* Enter / Space

---

## 5. Matching Rules

### INV-TA6 — Visible Items Only

Matching applies only to:

* visible list items
* visible tree nodes

Collapsed tree descendants are excluded.

---

### INV-TA7 — Forward Search with Wrap

Search behavior:

* starts **after** the currently focused item
* proceeds forward
* wraps once from end → beginning
* stops after full cycle

---

### INV-TA8 — First Match Wins

* first matching item becomes the target
* partial matches beyond prefix are irrelevant
* no scoring, no ranking, no fuzzy logic

---

## 6. Focus and Selection Effects

### INV-TA9 — Focus Movement Only

Type-ahead search:

* moves **focus**
* MUST NOT change selection
* MUST NOT activate items
* MUST NOT expand or collapse nodes

Selection changes require explicit user action.

---

### INV-TA10 — No Auto-Activation

Type-ahead MUST NOT:

* trigger click behavior
* emit activation events
* commit values (e.g. in Dropdown)

---

## 7. Tree-Specific Rules

### INV-TA11 — Hierarchical Order

In trees:

* matching order follows flattened **visible traversal order**
* indentation depth does not affect priority

---

### INV-TA12 — Expansion Prohibition

Type-ahead:

* MUST NOT expand collapsed nodes
* MUST NOT alter tree structure

---

## 8. Virtualization Rules

### INV-TA13 — Logical Matching

In virtualized views:

* matching operates on **logical items**, not realized widgets
* virtualization MUST materialize target item if needed
* scroll-to-reveal MUST occur after focus move

---

## 9. Programmatic Control

### INV-TA14 — Explicit API

Widgets MAY expose:

* enable / disable toggle
* timeout configuration

Programmatic triggering of type-ahead is forbidden.

---

## 10. Responsibility Boundaries

### Widget Owns:

* prefix buffer state
* matching logic
* focus relocation

### Widget MUST NOT Own:

* selection mutation
* activation logic
* text editing
* accessibility announcements

### Delegated:

* focus changes → focus system (SPEC-HAL-126)
* scrolling → scroll overlay (SPEC-HAL-110)

---

## 11. Forbidden Behavior

Type-ahead search MUST NOT:

* select items
* activate items
* expand trees
* depend on rendered geometry
* interfere with text input widgets
* persist prefix across focus loss

---

## 12. Test Requirements

Implementations MUST test:

* prefix accumulation and timeout reset
* wraparound search
* focus-only movement
* tree visibility constraints
* virtualization materialization
* disabled behavior when not enabled

---

## 13. Reference Implementation

Planned integration points:

* `List`
* `Tree`
* `VirtualizedListView`
* `VirtualizedTreeView`

Shared utility:

* `type_ahead::matcher`

---

## 14. Final Note

Type-ahead search exists to make **large structures feel small**.

If it surprises the user even once,
it has already failed.
