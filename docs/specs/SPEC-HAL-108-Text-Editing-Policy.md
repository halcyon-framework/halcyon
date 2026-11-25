# SPEC-HAL-108: Text Editing Policy Layer

**Status:** Implemented
**Author:** JC
**Scope:** TextField, TextArea
**Layer:** Widget-level policy (NOT substrate)

---

## 1. Motivation

Text editing widgets require **behavioral constraints** that must not be hardcoded into:

* the editing substrate (`editing.rs`)
* the document storage (`TextDocument`)
* layout or shaping layers

Examples include:

* read-only fields
* disabled widgets
* optional maximum input length
* future constraints (numeric-only, masked input, validation hooks)

Historically, ad-hoc checks lead to:

* duplicated logic
* silent truncation
* broken undo semantics
* inconsistent widget behavior

This SPEC introduces a **Text Editing Policy Layer** — a declarative, widget-owned rule set that governs *whether* edits are allowed, without changing *how* edits work.

---

## 2. Design Goals

* **Opt-in**: no default restrictions
* **Widget-scoped**: policy is owned by `TextField` / `TextArea`
* **Non-invasive**: no changes to editing substrate APIs
* **Explicit semantics**: no silent mutation or truncation
* **Testable**: behavior must be unit-testable at widget level
* **Extensible**: future policies added without breaking ABI

Non-goals:

* validation UI
* formatting or transformation
* semantic parsing (numbers, dates, etc.)
* IME visual rendering

---

## 3. Policy Model

### 3.1 TextEditPolicy (conceptual)

```rust
pub struct TextEditPolicy {
    pub enabled: bool,
    pub read_only: bool,
    pub max_len: Option<usize>,
}
```

This structure is **pure policy**:

* no behavior
* no side effects
* no implicit defaults beyond `enabled = true`

Each widget owns an instance.

---

## 4. Policy Semantics

### 4.1 Enabled

When `enabled == false`:

Allowed:

* nothing

Forbidden:

* focus
* pointer interaction
* keyboard input
* caret blinking
* scrolling
* selection
* clipboard operations

This is a **hard off switch**.

---

### 4.2 Read-Only

When `read_only == true` and `enabled == true`:

Allowed:

* focus
* caret movement
* selection
* scrolling
* copy
* undo/redo navigation (no-op if no history)

Forbidden:

* insert
* delete
* backspace
* cut
* paste
* IME commit
* undo history mutation

Read-only **never mutates document state**.

---

### 4.3 Maximum Length (Optional)

When `max_len == Some(N)`:

* Policy is enforced **before** calling `apply_insert_op()`
* Two acceptable strategies (chosen by widget):

    * reject insertion entirely
    * clip insertion explicitly

Constraints:

* No silent truncation
* No partial UTF-8 cluster corruption
* No undo entry created for rejected edits

If `None`, behavior is unbounded.

---

## 5. Enforcement Rules

### 5.1 Enforcement Layer

Policy enforcement occurs:

* **only in widget event handling**
* **before** any mutation
* **before** undo snapshot capture

The editing substrate remains policy-agnostic.

---

### 5.2 Single Choke Point Rule

Each widget must have **one logical gate** guarding all mutation paths:

* keyboard edits
* text events
* paste
* cut
* IME commit

No scattered checks.

---

## 6. Undo / Redo Interaction

* Rejected edits → no undo entry
* Clipped edits → undo reflects clipped result
* Read-only → undo stack unchanged
* Disabled → undo stack inaccessible

UndoHistory remains unmodified.

---

## 7. Testing Requirements

For any widget adopting this policy:

Required tests:

* enabled = false

    * no focus
    * no edits
* read_only = true

    * selection works
    * copy works
    * paste does nothing
    * cut does nothing
* max_len

    * boundary behavior explicit
    * undo consistency

Tests must be widget-level, not substrate-level.

---

## 8. Migration Plan

* Stage 1: Define policy struct (this SPEC)
* Stage 2: Wire `enabled` and `read_only`
* Stage 3: Add `max_len`
* Stage 4: Extend with future policies (validation hooks, filters)

No breaking changes required.

---

## 9. Summary

This SPEC establishes a **clear separation** between:

* *editing mechanics* (substrate)
* *behavioral constraints* (policy)
* *user interaction* (widget)

It prevents future regressions and allows TextField and TextArea to evolve independently without architectural debt.

---

## 10. Geometry & Growth Policy

Widget geometry is a **policy decision**, not a substrate or layout algorithm concern.

### 10.1 Policy Fields (Conceptual)

* `initial_height`: starting height when widget is created
* `min_height`: floor for shrinking
* `max_height`: ceiling for growth (optional)
* `vertical_growth_allowed`: whether height may increase

### 10.2 Invariants

1. Policy decides **whether** geometry may change, not **how**.
2. Substrate MUST NOT know geometry exists.
3. Layout may obey or clamp geometry, but MUST NOT invent behavior.
4. Growth beyond `max_height` is forbidden unless redirected to scrolling.
5. If `vertical_growth_allowed == false`, height is fixed at `initial_height`.
6. If `max_height == None` and `vertical_growth_allowed == true`, growth is unbounded.
7. `min_height <= initial_height <= max_height` (when all defined).

### 10.3 Forbidden

* Policy MUST NOT define pixel math.
* Policy MUST NOT reference content measurement.
* Policy MUST NOT interact with caret position.

---

## 11. Scrolling Policy

Scrolling permission is a **policy decision**, separate from scroll mechanics.

### 11.1 Policy Fields (Conceptual)

* `scrolling_allowed`: whether scrolling is permitted
* `scroll_on_overflow_only`: scroll only when content exceeds visible bounds

### 11.2 Invariants

1. Scrolling permission is policy-level.
2. Scroll offset mechanics are **out of scope** for this SPEC.
3. Caret visibility guarantees are **not defined here** (reserved for future SPEC).
4. When `enabled == false`: scrolling is forbidden.
5. When `read_only == true` and `scrolling_allowed == true`: scrolling is permitted.
6. If `scrolling_allowed == false`, overflow content is clipped (not scrollable).
7. If `scroll_on_overflow_only == true`, scroll UI is hidden when no overflow exists.

### 11.3 Interaction with Geometry Policy

* If growth is capped by `max_height` and `scrolling_allowed == true`, overflow triggers scroll.
* If growth is capped and `scrolling_allowed == false`, overflow is clipped.

### 11.4 Forbidden

* Policy MUST NOT define scroll delta math.
* Policy MUST NOT define viewport bounds.
* Policy MUST NOT define scroll bar rendering.

---

## 12. Resize Policy

User-initiated resize permission is a **policy decision**.

### 12.1 Policy Fields (Conceptual)

* `user_resize_allowed`: whether user may resize widget interactively
* `resize_direction`: vertical / horizontal / both / none

### 12.2 Invariants

1. Resize permission is policy-level.
2. Pointer mechanics are **out of scope** for this SPEC.
3. Resize MUST NOT mutate document state.
4. When `enabled == false`: resize is forbidden.
5. When `user_resize_allowed == true`, resize is bounded by `min_height` / `max_height` from geometry policy.
6. Resize beyond geometry bounds MUST be clamped, not ignored.
7. Programmatic resize (via API) is not subject to `user_resize_allowed`.

### 12.3 Forbidden

* Policy MUST NOT define resize handle hit-testing.
* Policy MUST NOT define pointer cursor changes.
* Policy MUST NOT define drag mechanics.

---

## 13. Policy Enforcement Invariants

### 13.1 Centralization

**INV-P1 — Policy decision is centralized**

There exists exactly one function that maps `(policy, event)` → decision.
Widgets MUST NOT re-encode policy logic.

### 13.2 Decision Outcomes

**INV-P2 — Allowed interaction**

If policy allows an event:

* Widget continues normal routing
* No early return is performed

**INV-P3 — Deny + Consume**

If policy denies a mutation:

* Widget MUST NOT call any mutation substrate API
* Widget MUST return `true`
* Document, cursor, selection state MUST remain unchanged

**INV-P4 — Deny + Bubble**

If policy denies interaction entirely:

* Widget MUST return `false`
* No editor state may change

Composite relation/session validation is additional authority qualification, not an alternative editing-policy function. The child applies the same centralized Allow/DeniedConsume/DeniedBubble decision immediately before mutation, preserving denied document/cursor/selection.

### 13.3 Constraints

**INV-P5 — No hidden behavior**

Policy enforcement MUST NOT:

* modify events
* inspect layout/geometry
* depend on widget type
* depend on platform beyond modifier classification already present in `event.rs`

**INV-108-PRESENTATION-SEPARATION.** The optional HAL-109 §17 TextArea presentation selector controls visual projection only. The centralized editing-policy decision remains unchanged and MUST run before the existing logical operation. Geometry and cache qualification occur separately after that decision; they cannot allow a denied operation, change Allow/DeniedConsume/DeniedBubble, alter events or postpone logical editing. Disabling presentation preserves the existing editing and rendering behavior.

**FORBID-108-PRESENTATION-POLICY.** No presentation selector, animation state or geometry result may become an editing-policy input or substitute for genuine Focus, keyboard Capture or availability.


### 13.4 Test Harness Invariants

**INV-H1 — Feature gate**

Harness APIs are available if and only if:
```
#[cfg(any(test, feature = "test-harness"))]
```

Default builds (`cargo build`) MUST NOT include harness APIs.

**INV-H2 — Side-effect free**

Harness APIs MUST NOT:

* trigger layout recomputation
* trigger paint
* trigger scroll
* modify undo history
* emit events
* call focus/blur handlers

**INV-H3 — Read-only introspection**

Getter APIs (e.g., `cursor_byte_pos_for_test`) MUST be pure reads:

* No recomputation
* No caching side effects
* No state mutation

**INV-H4 — Non-production use**

Harness APIs MUST NOT be called from non-test code paths.
The `test-harness` feature MUST NOT be enabled in production builds.

### 13.5 Atomic runtime availability — adopted execution, 2026-10-26

The architectural decision under JC's delegated approval admits this
scoped execution contract for ordinary Button, TextField, TextArea and TextEdit.
The document's existing status and all unrelated constraints remain unchanged.
This adopts no Proposed 363/364 or Draft cadence/typography clauses.

The existing App deferred executor owns one shared private
`apply_pending_widget_availability` operation with the live WidgetTree,
EventRouter, InputFusion/binding state and that tree's actual interaction domain.
The current canonical mounted tree remains PRIMARY under SPEC-HAL-151. Effective
editor policy remains solely in existing editor config; Button owns one private
enabled value. TextEdit forwards to its actual inner TextArea. There is no second
effective policy in App, appearance, renderer or a global registry.

`WidgetTree::request_widget_enabled(NodeId, bool)` and
`request_widget_read_only(NodeId, bool)` enqueue only. ScreenContext and
ScreenFrameContext delegate to that same lane when a tree exists; EventContext
requests are self-only. Resolve a live mount at enqueue and qualify the lazy
tree-owned lane by its dedicated tree-lifetime token and TreeWidgetId. Revalidate
at execution. Dead, foreign, unavailable or unsupported targets, including
read-only Button requests, are typed rejections without state changes. One
coalesced record per owner retains final authored fields before the cutoff;
equivalent effective policy is a no-op. Removal evicts pending work. A replacement
tree cannot accept a prior tree's numeric identity. Construction builders may
select initial policy before mounting; getters return effective policy, never
queued intent. Production code SHALL NOT use harness setters or an immediate
writable effective-policy bypass.

Complete the current dispatch before applying policy. If it enqueues availability,
end semantic intake at the normal between-event boundary and retain every
remaining original qualified envelope in order for the next legal intake. Settle
at the legal deferred/pre-Update cutoff before another semantic event, framework
appearance/focus snapshot or retained reuse. Screen update/build and deferred
callback requests join their legal cutoff. Lifecycle-produced requests remain
for a subsequent legal boundary, never recursive application; pending settlement
cannot be crossed by further input. No reentrant routing, second Widget Update,
layout, Paint-time or SDF-prePaint application is admitted.

Prevalidate the complete desired policy. Disabling terminates only that owner's
old pointer/gesture/keyboard/scroll/hover/IME interaction before enabled=false is
published atomically. Reuse tree-aware SystemCancel for actual captured sessions;
removing a losing candidate cannot cancel another owner's winner. Other pointers
and domains survive. Normal lifecycle role notifications occur once. A mounted
widget is not destroyed: neither `on_widgets_destroyed` nor cancel-all is legal.
The focus owner resolves the next eligible widget in the current scope, or None,
without implicit wrap, scope escape or neighbor auto-Capture; release old keyboard
Capture before transfer. Retire queued activation/IME ownership so later release
cannot activate a disabled/re-enabled owner. Re-enable never restores focus,
Capture or input. Transient IME cleanup preserves committed document and history.

An enabled read-only editor remains focusable and explicitly Capture-eligible
for selection, navigation, Copy and admitted scrolling. A read-only-only change
may retain focus/Capture and allowed selection/scroll. Settle mutating IME before
read-only takes effect; late commits cannot mutate document/history. Returning
editable manufactures no input, focus, Capture or replay. Allowed undo/redo
navigation never authorizes a document/history-mutating undo/redo operation.

Both editors SHALL call the one `decide_text_edit_policy` gate before ordinary
event-side effects, caret reset, clipboard, snapshots, history or mutation.
Disabled denies pointer/key/text/IME/scroll with DenyBubble; engine lifecycle
cleanup remains distinct. Read-only DenyConsume covers mutating Key and modifier
Text Cut/Paste/Undo/Redo, insertion/deletion/newline and mutating IME entry/commit.
Preserve Select All, Copy, navigation, harmless lifecycle cleanup and password
copy restrictions. Reuse established modifier classification; no blanket shortcut
denial, platform-policy fork or duplicated widget policy. Rejected operations
preserve document/history/cursor/selection/clipboard as applicable.

Effective eligibility, caret/scroll demand and existing appearance enabled facts
must agree with policy. Clear Button press through existing Update/Timeline
ownership without click. Only changed owner, old/new focus projections and
necessary ancestor composition invalidate before first affected scene. Layout,
unrelated LOCAL fragments, equal and rejected requests remain unchanged. No
per-frame whole-tree availability discovery or replacement scheduler is admitted.
Mounted canonical tests under both UiInteractionPolicy values SHALL use the
public requests and this same production executor, covering cutoff preservation,
capture/press cancellation, read-only commands/IME, next/None focus, fresh re-enable,
obsolete mounts and first retained output/peer reuse.

For HAL-101 section 2.5, current dispatch includes its bounded child operations and host continuations. Complete that dispatch before applying requested availability. Preserve the existing between-original-events cutoff and nonrecursive lifecycle-request rule; no later original input crosses pending policy settlement.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
