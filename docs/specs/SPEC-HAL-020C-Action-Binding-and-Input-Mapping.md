# SPEC-HAL-020C: Action Binding and Input Mapping

**Status:** Design / Binding
**Layer:** Input interpretation (device → action intent)
**Scope:** Key / pointer / gesture → ActionId mapping, binding tables, conflict rules
**Depends on:** SPEC-HAL-020A (Action Core Model), SPEC-HAL-020B (Resolution & Dispatch)
**Constrains:** Input system, event router, binding registry
**Explicitly Excludes:** Action resolution, execution, widget logic

---

## SECTION A — Purpose

This SPEC defines **how raw input becomes an ActionId**.

It answers:

* How keys, mouse buttons, gestures, and chords map to actions
* Where bindings live
* How conflicts are resolved
* How rebinding works
* How “contextual” bindings differ from resolution

This layer **does not know** which action will fire.
It only produces *candidates*.

---

## SECTION B — Separation of Concerns

**INV-ACT-BIND-SEP-1**
Input binding MUST be independent from action resolution.

**INV-ACT-BIND-SEP-2**
Binding MUST NOT consult widget state, focus, overlays, or selection.

**INV-ACT-BIND-SEP-3**
Binding output is `ActionId + BindingMetadata`, nothing else.

---

## SECTION C — Input Sources

Bindings MAY originate from:

* Keyboard (keys, modifiers, chords, sequences)
* Pointer (button, click count)
* Scroll (wheel, trackpad)
* Gesture (high-level, already recognized)
* Programmatic triggers (tests, automation)

**INV-ACT-BIND-IN-1**
All input sources MUST normalize into a common binding key format.

---

## SECTION D — Binding Units

A **Binding** is defined as:

* Input Pattern (e.g. `Ctrl+K`, `Escape`, `MouseLeft`)
* Target `ActionId`
* Optional conditions (device class, platform, layout)
* Priority class (see Section F)

Bindings are **data**, not code.

---

## SECTION E — Binding Tables

Bindings are stored in layered tables:

1. **System Defaults**
2. **Application Defaults**
3. **User Overrides**
4. **Runtime Overrides** (temporary, e.g. modal rebinding)

**INV-ACT-BIND-TBL-1**
Later tables override earlier ones.

**INV-ACT-BIND-TBL-2**
Removal is explicit (tombstone), not implicit.

---

## SECTION F — Binding Priority Classes

Bindings belong to one of these classes:

* **Hard** – cannot be overridden (engine safety only)
* **Normal** – default bindings
* **Soft** – user-preferred, lowest priority

**INV-ACT-BIND-PRI-1**
Hard bindings override all others.

**INV-ACT-BIND-PRI-2**
Soft bindings MUST NOT override Hard or Normal.

---

## SECTION G — Conflict Resolution (Binding-Time)

Conflicts are resolved **before** action resolution.

**INV-ACT-BIND-CONF-1**
For the same input pattern, exactly one binding may survive.

Resolution order:

1. Higher priority class
2. Later binding table
3. Deterministic tie-break (stable order)

**FORBID-ACT-BIND-MULTI**
A single input MUST NOT emit multiple ActionIds.

---

## SECTION H — Context Independence Rule

Bindings are **context-agnostic**.

**INV-ACT-BIND-CTX-1**
Bindings MUST NOT depend on widget type, focus, or overlays.

Contextual behavior is handled **later** by SPEC-HAL-020B.

Example:

* `Escape → ActionId::escape` (always)
* Whether Escape closes a dialog, clears selection, or does nothing is not decided here.

---

## SECTION I — Chords and Sequences

Bindings MAY define:

* Chords (simultaneous modifiers)
* Sequences (e.g. `Ctrl+K, Ctrl+C`)

**INV-ACT-BIND-SEQ-1**
Sequence state MUST be time-bounded.

**INV-ACT-BIND-SEQ-2**
Partial sequences MUST NOT emit actions.

---

## SECTION J — Platform and Layout Awareness

Bindings MAY declare applicability constraints:

* OS
* Keyboard layout
* Input device class

**INV-ACT-BIND-PLAT-1**
Platform-specific bindings MUST fall back cleanly.

---

## SECTION K — Rebinding Guarantees

**INV-ACT-BIND-REB-1**
All non-Hard bindings MUST be rebindable.

**INV-ACT-BIND-REB-2**
Rebinding MUST NOT require widget changes.

**INV-ACT-BIND-REB-3**
Rebinding MUST be serializable.

---

## SECTION L — Output Contract

Binding produces:

* `ActionId`
* `BindingSource` (which table)
* `BindingPriority`
* `InputOrigin` (for diagnostics)

Nothing else.

**INV-ACT-BIND-OUT-1**
Binding MUST NOT produce side effects.

---

## SECTION M — Error Handling

**INV-ACT-BIND-ERR-1**
Invalid bindings are ignored with diagnostics.

**INV-ACT-BIND-ERR-2**
Conflicts MUST be detectable in tooling.

---

## SECTION N — Performance Invariants

**INV-ACT-BIND-PERF-1**
Binding lookup MUST be O(1) or amortized constant.

**INV-ACT-BIND-PERF-2**
No allocation per key event in steady state.

---

## SECTION O — Forbidden Behaviors

**FORBID-ACT-BIND-CTX-LEAK**
Bindings MUST NOT encode context.

**FORBID-ACT-BIND-EXEC**
Bindings MUST NOT execute actions.

**FORBID-ACT-BIND-SPECIAL**
No hard-coded special keys (including ESC).

---

## SECTION P — Acceptance Criteria

SPEC-HAL-020C is satisfied when:

* [ ] Input → ActionId mapping is deterministic
* [ ] Rebinding works without widget changes
* [ ] Conflicts are resolved before resolution
* [ ] ESC is treated as a normal action
* [ ] Binding layer has zero knowledge of context

---

## SECTION Q — Relationship to Other SPECs

* 020A — defines actions
* 020B — decides *which* action fires
* 020C — maps input to action intent
* 020D — executes actions and manages lifecycle

## Scoped physical-key admission — 2026-10-10

**Author:** JC. **Decision:** the accepted physical-input prerequisite for
applications that record physical-key responses. Implementation is a 0.211.0-beta source
candidate with retained focused implementation proof. Independent
source acceptance remains pending; this addition does not promote any other
Draft scope.

The canonical binding key explicitly selects logical `KeyCode` or framework-owned
`PhysicalKeyCode`. Physical identity originates only from native physical-key
transport, never from a character, keyboard layout, or text. Unknown identity is
explicit and cannot match or be recorded as a physical binding. Existing logical
constructors and UI defaults remain logical. Ctrl and Meta remain distinct.

Both candidates from one key event resolve to at most one action using the
existing priority, layer, and stable last-entry rules. The binding owner remains
context-free; physical response values do not require a fabricated action.

**Observable-boundary decision (2026-10-10; Author JC):** A checked
physical-response binding is admitted only from an observed initial nonrepeat
nonmodifier Down with known physical identity. The existing shared event/binding
owners expose typed reserved/unsupported classification. No observation is
`Unobserved`, not an inferred key, wrong answer, or trainable binding. Observed
Escape, Tab, debug and fullscreen reservations are not training values; actual
App consumption is reported by its existing owner. Clipboard keys remain
admitted inside physical Capture under the separately accepted conditional law.
Ordinary logical constructors and generic binding tables keep their behavior;
they do not certify a physical response's observability. This is not an OS
shortcut blacklist, layout inference, global hook, or polling contract.

**Original-key reservation correction (2026-10-10; Author JC):** The
existing Fusion registration's selected Global shortcut declares an App input
intent for its original nonrepeat Down. Qualified normalization retains that
selected identity before emitting Down and the derived Shortcut. App settles
physical Capture before delivering the original Down; a one-shot recorder cannot
commit that reserved input. This does not make Global scope `BindingPriority::Hard`
or change binding conflict resolution. Existing BindingRegistry action admission
and the original shortcut execution owners remain in place. No Screen callback
is invoked to predict consumption. Ordinary non-opted-in and logical delivery
retain their existing KeyDown/Shortcut order. Focused implementation proof is retained; the candidate remains pending independent re-review.
