# SPEC-HAL-203: ComboBox Projection Model & Professional Semantics

**Status:** Draft (Binding upon adoption)
**Layer:** Widget Semantics / Projection Consumption
**Owner:** Widget Architecture
**Applies To:** ComboBox, future Selector widgets
**Depends On:** SPEC-HAL-164, SPEC-HAL-179, SPEC-HAL-188, SPEC-HAL-188A

---

## 0. Purpose

This SPEC defines the non-negotiable professional semantics of ComboBox in Halcyon.

It formalizes ComboBox as:

- a projection consumer
- an intent emitter
- never a data owner
- never a data coordinator

The goal is to make ComboBox suitable for large, dynamic, remote, and inter-dependent datasets, not toy examples.

---

## 1. Foundational Law

### 1.1 ComboBox owns no data

ComboBox MUST NOT:

- store full datasets
- fetch data
- assume data size
- assume data locality (sync / async)
- assume completeness or finality

ComboBox operates exclusively on OptionProjection.

---

### 1.2 ComboBox is not a filter, matcher, or resolver

ComboBox MUST NOT:

- decide what items "match"
- perform fuzzy search
- perform filtering
- infer backend logic

All semantic decisions belong to the projection source.

---

## 2. OptionProjection Contract

ComboBox consumes a bounded projection with the following semantics.

### 2.1 Required fields

`items: &[OptionView]`

Where OptionView is a read-only representation (label, id, optional metadata).

---

### 2.2 Optional but first-class fields

```
count_hint: Option<CountHint>
state: ProjectionState
min_visible: usize
max_visible: usize
```

CountHint:

- `Exact(n)`
- `AtLeast(n)`
- `Unknown`

ProjectionState:

- `Stable`
- `Loading`
- `EmptyButValid`
- `Error` (future-compatible)

---

## 3. Dimensional Semantics (Overlay Geometry)

### 3.1 Overlay sizing law

Overlay height MUST be derived as:

```
visible_rows = clamp(items.len(), min_visible, max_visible)
overlay_height = visible_rows * ROW_HEIGHT
```

### 3.2 Mandatory behavior

- If `items.len() < min_visible` — popup MUST NOT expand meaningfully
- If `items.len() < max_visible` — popup MUST shrink to content
- If `items.len() > max_visible` — popup MUST clamp, never grow further

No half-empty overlays.
No visually "dead" space.

---

## 4. Searching & Typing Semantics (Strict Separation)

ComboBox supports three independent behaviors.

### 4.1 Type-to-Search (Bias Only)

- Highlights a candidate
- Does NOT filter
- Does NOT modify projection
- Already implemented correctly

Optional. Off by default.

---

### 4.2 Search Intent Emission

On text changes, ComboBox MAY emit:

`SearchIntent { text }`

This intent:

- does NOT imply filtering
- does NOT imply completion
- is advisory only

The projection source decides how to respond.

---

### 4.3 Auto-Completion (Niche, Explicit)

Auto-completion:

- MUST be opt-in
- MUST be projection-driven
- MUST NOT be guessed by ComboBox

ComboBox only applies completion when explicitly instructed via projection metadata.

---

## 5. Large / Remote Dataset Semantics

ComboBox MUST operate correctly when:

- total dataset is unknown or huge
- projection contains a small window
- backend is async or incremental

Behavioral requirements:

- display available items only
- remain responsive during Loading
- allow navigation within projection
- emit intents when user reaches boundaries (future extension)

ComboBox never blocks, never spins, never assumes.

---

## 6. Dependency Between Widgets (Critical)

### 6.1 Widgets NEVER depend on each other

ComboBox MUST NOT:

- observe another widget
- subscribe to another widget
- coordinate state with another widget

---

### 6.2 Shared model → shared projections

Correct mental model:

```
[ Model / Store / State ]
        |
  derives projections
        |
[ ComboBox A ]   [ ComboBox B ]
```

- Widget A emits intent
- Model updates
- New projections are derived
- All widgets update independently

This applies to:

- chained dropdowns
- incremental selectors
- conditional option sets

---

## 7. Projection Definition (Clarified)

### 7.1 What a projection IS

A projection is:

- read-only
- bounded
- partial
- contextual

Example: "10 visible items out of 1,000,000 possible"

---

### 7.2 What a projection is NOT

A projection is NOT:

- ownership
- storage
- mutation
- a cursor
- a cache

ComboBox never escapes this boundary.

---

## 8. Empty, Loading, and Error States

ComboBox MUST:

- distinguish between:
  - Empty (valid result)
  - Loading (transient)
- avoid expanding empty overlays
- keep dimensions stable when loading
- allow visual affordances later (spinner, placeholder)

---

## 9. Non-Goals (Explicit)

This SPEC intentionally does NOT define:

- rendering style
- animations
- virtualization strategy
- accessibility semantics
- backend protocol

Those belong to separate SPECs.

---

## 10. Enforcement Summary (Hard Rules)

- ComboBox is a projection consumer
- Overlay size is projection-driven
- Searching is intent-based
- Filtering is external
- Widget dependencies are forbidden
- Projection is read-only, bounded, partial

Violation of any rule above is a constitutional break, not a bug.
