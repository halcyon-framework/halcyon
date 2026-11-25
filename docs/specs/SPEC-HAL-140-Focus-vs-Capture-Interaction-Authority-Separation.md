# SPEC-HAL-140: Focus vs Capture — Interaction Authority Separation

**Status:** Binding  
**Layer:** Input & Interaction Model  
**Scope:** Separation of navigational focus and exclusive input authority  
**Depends on:** SPEC-HAL-038, SPEC-HAL-033, SPEC-HAL-131A, SPEC-HAL-128  
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Modern UI systems routinely conflate **focus** with **input authority**, leading to
catastrophic user experience failures:

* scrolling hijacked by hovered widgets
* accidental value changes via wheel
* nested scroll areas stealing intent
* focus changes mutating unrelated state
* unpredictable escape behavior

Halcyon already distinguishes **focus routing** and **gesture arbitration**, but lacks
a **formal, binding separation** between:

* *what is focused* (semantic / navigational target)
* *what has capture* (exclusive authority over input streams)

This SPEC defines that separation and makes violations **architecturally illegal**.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Focus is not authority**  
Focus identifies a target; it grants no control over input by itself.

**PRINCIPLE-2: Capture is explicit**  
Exclusive input authority is entered only through deliberate activation.

**PRINCIPLE-3: User intent dominates hierarchy**  
Input follows the user’s declared intent, not hover position or layout depth.

**PRINCIPLE-4: Escape is structural**  
Exit paths are deterministic, reversible, and non-destructive.

**PRINCIPLE-5: No ambient interaction**  
Nothing reacts “just because the pointer passed over it”.

---

## SECTION C — Core Interaction States

Halcyon recognizes the following engine-level interaction states:

1. Unfocused
2. Focused
3. Focused + Captured
4. Captured (pointer-only, transient, non-focus-granting)

Transitions outside this set are forbidden.

---

## SECTION D — Focus Contract

Focus represents **navigation and semantic targeting only**.

**INV-FOCUS-NO-AUTHORITY**  
Focus MUST NOT grant scroll, wheel, drag, or keyboard authority.

**INV-FOCUS-STABILITY**  
Focus changes MUST NOT mutate scroll, layout, or value state.

**INV-FOCUS-VISIBILITY-ONLY**  
Focus reveal may only ensure visibility; it may not reposition unnecessarily.

---

## SECTION E — Capture Contract

Capture represents **exclusive authority over one or more input streams**.

**INV-CAPTURE-EXPLICIT-ONLY**  
Capture may only be entered via explicit activation (click, Enter, Space).

**INV-CAPTURE-EXCLUSIVE**  
While captured, input MUST NOT leak to ancestors or siblings.

**INV-CAPTURE-DOMAINED**  
Capture is scoped to specific domains (scroll, pointer, keyboard).

---

## SECTION F — Scroll Authority Rules

Scroll is a **privileged input domain**.

**INV-SCROLL-NO-HOVER**  
Hover MUST NOT grant scroll authority.

**INV-SCROLL-CAPTURE-ONLY**  
Scroll input is consumed only by a captured scroll container.

**INV-SCROLL-NO-FALLTHROUGH**  
Reaching scroll bounds MUST NOT redirect scroll to parents.

---

## SECTION G — Keyboard Interaction

Keyboard navigation and activation are distinct.

* Arrow / Tab keys move focus
* Enter / Space activate and may enter capture

**INV-KEYBOARD-NO-AUTO-CAPTURE**  
Keyboard focus navigation MUST NOT implicitly capture.

---

## SECTION H — Escape Semantics

Escape is the **unwind operator**.

**INV-ESCAPE-RELEASES-CAPTURE**  
Escape exits capture but preserves focus.

**INV-ESCAPE-STABLE**  
Escape MUST NOT mutate selection, scroll, or value state.

Optional second Escape behavior must be explicitly defined per container.

---

## SECTION I — Forbidden Behaviors

The following are categorically forbidden:

* scrolling a widget on hover
* changing values via wheel without activation
* capturing input on focus entry
* leaking scroll beyond captured container
* implicit capture via layout or z-order

---

## SECTION J — Interaction with Existing SPECs

* Focus routing governed by SPEC-HAL-038
* Gesture arbitration governed by SPEC-HAL-033
* Scroll substrate governed by SPEC-HAL-131A
* Pointer capture governed by SPEC-HAL-128

SPEC-HAL-140 introduces **no new mechanics**, only binding constraints.

---

## SECTION K — Performance and Predictability

**INV-INTERACT-NO-SURPRISE**  
Interaction state transitions MUST be constant-time and deterministic.

**INV-INTERACT-NO-HIDDEN-STATE**  
All capture state must be explicit and inspectable.

---

## SECTION L — Security and Safety

* No ambient authority
* No invisible capture
* No irreversible transitions

User intent must always be recoverable.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-140 is satisfied when:

* [ ] Focus never causes scroll or value mutation
* [ ] Scroll occurs only under capture
* [ ] Hover never consumes input
* [ ] Escape reliably exits capture
* [ ] Nested scroll areas cannot hijack intent

---

## SECTION N — Authoritative Statements

**AUTH-140-1:** “Focus is a pointer, not a weapon.”  
**AUTH-140-2:** “Input authority must be explicit.”  
**AUTH-140-3:** “User intent outranks widget convenience.”
