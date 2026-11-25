# SPEC-HAL-169: TabularSubstrate — Column Resize Interaction Contract

**Status:** Implemented / Closed
**Layer:** Interaction / Intent Routing (Non-Mutating)  
**Audience:** Engine, Interaction, Layout  
**Depends on:**  
SPEC-HAL-009, SPEC-HAL-024, SPEC-HAL-037, SPEC-HAL-128, SPEC-HAL-140, SPEC-HAL-145, SPEC-HAL-158  
**Parent:** SPEC-HAL-164  
**Consumes:** SPEC-HAL-165, SPEC-HAL-166, SPEC-HAL-168

---

## 1. Purpose

This SPEC defines **how column resize interactions are expressed, routed, and authorized** without allowing interaction
code to mutate layout or geometry directly.

It exists to prevent:

- geometry mutation during input
- header-owned resizing
- paint-time width changes
- multi-source resize authority

---

## 2. Interaction Model

Column resize is modeled as a **three-phase intent lifecycle**:

1. **BeginResize**
2. **UpdateResize**
3. **CommitResize / CancelResize**

All phases emit **intent directives** only.

No geometry mutation occurs inside interaction handlers.

---

## 3. Resize Initiation

Resize initiation is allowed only from the **Header Domain**.

**INVARIANT-1:**  
Only header hit-zones may initiate resize intents.

**INVARIANT-2:**  
A resize intent must reference exactly one `column_id`.

Implicit or inferred column targets are forbidden.

---

## 4. Pointer Capture and Arbitration

On resize begin:

- pointer capture is acquired exclusively
- other header/body interactions are suppressed
- scroll gestures are suspended

**INVARIANT-3:**  
Resize capture has higher priority than scroll but lower than modal capture.

---

## 5. Resize Update Semantics

During update:

- delta is computed in **header-local coordinates**
- emitted as a proposed width delta
- no clamping or snapping is applied here

**INVARIANT-4:**  
Resize updates must not read or write column geometry.

They may only reference:

- initial column width (snapshot)
- pointer delta

---

## 6. Commit and Cancellation

On commit:

- a **structural mutation request** is emitted
- layout invalidation is triggered upstream
- geometry is recomputed via SPEC-HAL-168

On cancel:

- no mutation occurs
- no layout invalidation occurs

---

## 7. Authority Separation

**INVARIANT-5:**  
Interaction code must not:

- clamp widths
- enforce min/max
- resolve flex
- trigger partial layout

All enforcement happens during layout resolution.

---

## 8. Visual Feedback

Visual resize feedback is permitted but non-authoritative.

Allowed:

- ghost guides
- cursor changes
- overlay lines

Forbidden:

- mutating actual column geometry
- reading future geometry
- compensating offsets

---

## 9. Multi-Column Constraints

Resizing one column must not implicitly resize others.

Redistribution policies (if any) are defined elsewhere.

---

## 10. Forbidden Behaviors

Explicitly illegal:

- resizing during paint
- header-driven width mutation
- reading geometry during pointer move
- scroll-based resize updates
- per-frame width accumulation

---

## 11. Scope Boundaries

This SPEC does **not** define:

- snapping
- auto-size
- double-click behavior
- column pinning
- persistence

Those are layered later.

---

## 12. Relationship to Deprecated SPECs

This SPEC replaces resize intent from:

- SPEC-HAL-117
- SPEC-HAL-118F
- SPEC-HAL-118L

No behavior is inherited.

---

## 13. Next SPEC

SPEC-HAL-170 will define:

**Column Separators and Structural Visual Guides**

---

## 14. Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/intents.rs` (ResizeIntent enum with Begin/Update/Commit/Cancel phases), `mod.rs` (handle_intent dispatch). All invariants (INVARIANT-1 through INVARIANT-5) enforced by intent-only mutation model and pointer capture arbitration. No further work permitted under this SPEC.
