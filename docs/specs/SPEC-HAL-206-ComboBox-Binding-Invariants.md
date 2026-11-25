# SPEC-HAL-206: ComboBox Binding Invariants

**Status:** Binding
**Layer:** Widget Contract
**Scope:** ComboBox only
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-188A, SPEC-HAL-203, SPEC-HAL-204

---

## 0. Purpose

This SPEC defines engine-grade invariants for ComboBox.
These invariants are binding law, not documentation.
They must survive refactors, async providers, and future features.

No implementation changes in this SPEC.
Only invariant specification.

---

## Invariant Class A — State Ownership & Authority

### INV-A1: No Option Data Ownership

ComboBox does not own option data.
It only consumes projections provided externally (options list or `ProjectionOutput`).

### INV-A4: No Synthetic Options

ComboBox must never create synthetic option entries to represent empty, loading, or error states.

### INV-A2: No Async Ownership

ComboBox does not own async behavior, timing, debounce, or delays.
All timing originates in the data/provider layer.

### INV-A3: Synchronous Determinism

ComboBox internal state is synchronous and deterministic.
No timers, clocks, sleeps, or frame counters inside ComboBox.

**INV-A5: Shared Text-Entry Presentation Only.** A qualified existing ComboBox text-entry region may consume only the shared HAL-101 section 8.4.6 presentation through its admitted owner. For this scope alone, INV-A2/A3's timing prohibition does not prohibit the existing engine Timeline's finite visual projection. ComboBox still owns no timer, clock, debounce, provider timing or semantic easing. Resolver inputs, explicit commit source, decisions, option identity and synchronous behavior remain unchanged. This is no ownership exception and no independent implementation authority.

**FORBID-A5.** No animation of selected-value meaning, inferred commit, delayed resolver call, private time source or duplicated editing/presentation engine.

---

## Invariant Class B — Input & Commit Semantics

### INV-B1: Explicit Intent Only

A commit occurs only on explicit user intent:

- Enter
- Pointer release inside a valid option row
- InputConfirm path (custom value allowed)

No commit on:

- Pointer down
- Hover
- Focus
- Popup open/close
- Loading transitions

### INV-B2: Strict Mode Guard

If `allow_custom_value == false`, committing a non-matching value is forbidden.

### INV-B3: Loading Commit Passthrough

If `allow_custom_value == true`, Enter during loading commits the current text.

### INV-B4: Navigation Key Purity

Navigation keys (Up/Down/Page/Home/End) never mutate text content.

---

## Invariant Class C — Popup Lifecycle

### INV-C1: Popup Visibility Function

Popup visibility is a pure function of:

- `has_popup_content()`
- explicit user actions

### INV-C2: Popup Suppression

Popup must not render when:

- there are zero matching items
- loading is active

### INV-C3: Closure Neutrality

Popup closure must never imply error, rejection, or invalid input.

---

## Invariant Class D — Visual Signaling

### INV-D1: No Textual Messages

ComboBox must never display textual messages such as:

- "Loading..."
- "No results"
- "Invalid value"

### INV-D2: Ambient Signaling Only

All feedback is conveyed via:

- input background tint
- list highlight
- presence/absence of popup

### INV-D3: Mutually Exclusive Background States

Input background states are mutually exclusive:

- Default
- Browsing
- Custom
- NoMatch
- Loading

Exactly one may be active at any time.

---

## Invariant Class E — Loading Semantics

### INV-E1: Loading Suppression

When `loading == true`:

- popup is closed
- popup cannot open
- navigation keys are no-op

### INV-E2: Explicit Loading Clearance

Loading state must be explicitly cleared by the provider.
ComboBox never infers loading completion.

### INV-E3: Text Editing During Loading

ComboBox must remain fully responsive to text editing during loading.

---

## Invariant Class F — Matching & Highlighting

### INV-F1: Match Mode Isolation

Matching mode (Prefix / Substring) affects filtering only.
It does not affect commit semantics.

### INV-F2: Highlight Purity

Match highlighting is purely visual.
It never alters labels, values, or selection identity.

### INV-F3: Empty Match Guard

When no matches exist, highlighting must not render.

---

## Invariant Class G — Event Routing

### INV-G1: Text Forwarding

Text editing events selected by the existing ComboBox policy are delivered exactly once through the engine-owned HAL-101 section 2.4 session to the registered TextField child unless explicitly blocked by existing loading rules. A complete owned current result settles before ComboBox's next dependent text/commit action. No host holds or calls a TextField Widget directly, and no host context impersonates child identity or geometry.

### INV-G2: Multi-Click Passthrough

Pointer multi-click (double/triple) must reach TextField selection logic.

The engine-mediated route preserves the original pointer/multiclick facts, actual child-local coordinates and text-region gating; it does not replay or synthesize the event.

### INV-G3: Space Passthrough

ComboBox must never consume Space as a character input.

Preserve Space passthrough through the same qualified route without duplicate character delivery; no input-provenance inference is admitted.

The closed host adapter preserves this widget's current command, resolver/value/tag, callback and clear/reset order under HAL-101 section 2.5; ordinary standalone TextField Enter/Cancel shall not replace host policy. Runtime child-dependent API calls require the scoped owner context; no no-context peer replacement or silently queued sync is admitted.

---

## Invariant Class H — Determinism & Safety

### INV-H1: Reproducibility

Given identical input events and identical projections, ComboBox behavior is identical.

### INV-H2: Steady-State Allocation

ComboBox must not allocate per frame during steady-state interaction.

### INV-H3: Invariant Supremacy

No invariant may be violated to "improve UX" or "simplify behavior".
New behavior requires new invariants.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
