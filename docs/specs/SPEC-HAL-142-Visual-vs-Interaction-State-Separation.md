# SPEC-HAL-142: Visual State vs Interaction State — Explicit Separation

**Status:** Binding
**Layer:** Interaction & Rendering Model
**Scope:** Separation of visual presentation state from interaction logic state
**Depends on:** SPEC-HAL-038, SPEC-HAL-052, SPEC-HAL-107, SPEC-HAL-135, SPEC-HAL-140, SPEC-HAL-141
**Constrains:** Engine-level semantics only (no widget policy invention)

---

## SECTION A — Problem Statement

Halcyon currently allows **visual state** and **interaction state** to partially overlap:

* focus affects rendering
* selection affects hit-testing
* hover influences layout timing
* internal interaction flags leak into paint decisions

This coupling causes:

* tests breaking when visuals change
* interaction regressions after refactors
* visual artifacts masking logic bugs
* “ghost state” where visuals lie about behavior
* impossible-to-reason-about invariants

Visuals are a **projection**.
Interaction is **truth**.

This SPEC enforces a **hard architectural separation** between:

* **what the system is doing**
* **what the user sees**

---

## SECTION B — Design Principles

**PRINCIPLE-1: Interaction is authoritative**
Logic determines truth; visuals reflect it.

**PRINCIPLE-2: Visuals never drive behavior**
Rendering MUST NOT affect interaction outcomes.

**PRINCIPLE-3: One-way dependency**
Interaction → Visuals is allowed.
Visuals → Interaction is forbidden.

**PRINCIPLE-4: Replace implication with declaration**
No state is inferred from visuals.

**PRINCIPLE-5: Test logic without pixels**
Interaction must be fully testable without rendering.

---

## SECTION C — Interaction State (Authoritative)

Interaction state consists of **engine-truth variables**, including but not limited to:

* focus target
* navigation cursor
* selection set
* anchor / range session state
* capture state
* transaction state
* activation state

**INV-INTERACT-AUTH-ONLY**
Interaction state MUST NOT depend on visual state.

**INV-INTERACT-EXPLICIT**
All interaction state transitions must be explicit reducer actions.

---

## SECTION D — Visual State (Derived)

Visual state consists of **render-only projections**, including:

* focused highlight
* selection highlight
* hover affordances
* disabled appearance
* animation interpolation state

**INV-VISUAL-DERIVED**
Visual state MUST be derived from interaction state.

**INV-VISUAL-READONLY**
Visual state MUST NOT mutate interaction state.

---

## SECTION E — Focus vs Visual Focus

Visual indication of focus is **not focus itself**.

**INV-VISUAL-FOCUS-NOT-AUTH**
Rendering a focused style MUST NOT imply focus exists.

**INV-FOCUS-NO-VISUAL-INFERENCE**
Focus existence MUST NOT be inferred from paint state.

---

## SECTION F — Selection vs Highlight

Selection is logical; highlight is visual.

**INV-SELECTION-INDEPENDENT**
Selection MUST exist even if not rendered.

**INV-HIGHLIGHT-NO-LOGIC**
Highlight presence MUST NOT affect selection behavior.

---

## SECTION G — Hover State

Hover is a visual affordance only.

**INV-HOVER-NO-INTERACT**
Hover MUST NOT alter interaction state.

**INV-HOVER-NO-AUTHORITY**
Hover MUST NOT grant focus, capture, or selection power.

---

## SECTION H — Animation Interaction Boundary

Animations are visual-only unless explicitly authorized.

**INV-ANIM-NO-LOGIC**
Animation state MUST NOT change interaction outcomes.

**INV-ANIM-CONSUMES-VISUALS**
Animations consume visual projections only.

---

## SECTION I — Testing Implications

**INV-LOGIC-TESTABLE-HEADLESS**
All interaction logic MUST be testable without renderer involvement.

**INV-VISUAL-NO-ASSERTIONS**
Logic tests MUST NOT assert visual properties.

---

## SECTION J — Forbidden Behaviors

The following are categorically forbidden:

* reading visual flags in reducers
* mutating interaction state during paint
* inferring focus/selection from visuals
* coupling animation state to interaction rules
* widget-local hacks that blur the boundary

---

## SECTION K — Interaction with Existing SPECs

* Focus routing — SPEC-HAL-038
* Editing substrate — SPEC-HAL-052
* Selection invariants — SPEC-HAL-107, SPEC-HAL-135
* Focus vs capture — SPEC-HAL-140
* Geometry authority — SPEC-HAL-141

SPEC-HAL-142 introduces **no new state**, only separation.

---

## SECTION L — Performance and Predictability

**INV-STATE-NO-FEEDBACK**
No feedback loops between rendering and interaction.

**INV-STATE-DETERMINISTIC**
Given identical inputs, interaction state is identical regardless of visuals.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-142 is satisfied when:

* [ ] Interaction logic never reads visual state
* [ ] Visuals are fully derived
* [ ] Logic tests run headless
* [ ] Visual refactors don’t break interaction
* [ ] Ghost state is eliminated

---

## SECTION N — Authoritative Statements

**AUTH-142-1:** “Interaction is truth; visuals are commentary.”
**AUTH-142-2:** “Pixels do not decide behavior.”
**AUTH-142-3:** “If logic needs paint, the design is broken.”
