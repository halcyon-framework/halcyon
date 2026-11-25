# SPEC-HAL-208A: Labeled Controls — Configurable Label Side

**Status:** Binding (Amendment)
**Layer:** Widget Contract / Interaction
**Amends:** SPEC-HAL-208 (Labeled Controls — Ownership, Layout, and Interaction Invariants)
**Scope addition:** Switch, Toggle moved from *future* to *active* scope
**Ground Truth:** SPEC-HAL-208, SPEC-HAL-207, SPEC-HAL-000 (naming)

---

## 0. Purpose

SPEC-HAL-208 establishes that a labeled control **owns its label** as one
semantic unit and forbids external `Label` siblings (INV-A2, INV-G1) and event
forwarding (INV-G3). It is silent on **which side** of the indicator the label
occupies; the trailing (right-of-indicator) placement is a widget-implementation
detail, not an invariant.

This amendment makes label side an **explicit policy** and brings Switch and
Toggle into active scope, so a label may sit before or after the indicator while
remaining a single owned, hit-testable unit with focus presentation governed by
SPEC-HAL-208 INV-D3, including its 2026-10-24 default-selector exception.

---

## 1. Motivation

A label placed before its indicator must remain part of the control's single
owned layout and hit-testable unit. A detached sibling label is not a substitute
for owned-label semantics or a reason to forward events between widgets.
Control-owned LabelSide supplies the placement policy without changing label
ownership, default placement or the unified interaction contract.

---

## 2. Policy — `LabelSide`

A labeled control exposes label placement as an explicit enum:

```
LabelSide
  Trailing   // label after the indicator in flow order (default)
  Leading    // label before the indicator in flow order
```

- Default is `Trailing`, identical to current behavior.
- The control owns the label in **both** modes — no sibling, no synthetic node,
  no forwarding (Class A and Class G of SPEC-HAL-208 remain in full force).

*(Naming note for review: `Leading`/`Trailing` mirror the flow-relative spirit
of the existing `Alignment` vocabulary. If a plain `Left`/`Right` or `Start`/`End`
naming is preferred, that is a one-line change before sign-off.)*

---

## 3. Invariant Class H — Label Side

### INV-H1: Configurable Side
A labeled control's label may be placed `Leading` or `Trailing` relative to the
indicator. The side is a control property, set at construction or via setter.

### INV-H2: Default Preserves Behavior
Absent an explicit side, the control behaves as `Trailing`. Layout, paint, and
hit-testing are byte-for-byte identical to pre-amendment behavior. No existing
control changes appearance or behavior without opting in.

### INV-H3: Side-Invariant Layout & Interaction
SPEC-HAL-208 INV-B1 (unified layout box), INV-D1 (unified hit region), and
INV-D3 (focus presentation, including its 2026-10-24 default-selector exception)
hold **identically** for both sides. The indicator–label
gap (INV-B4) is placed on the indicator-facing edge of the label. The reported
bounding rectangle and full-area hit region cover indicator + gap + label
regardless of side. Any required focus enclosure covers that same span; the
default-selector exception applies equally to Leading and Trailing labels.

### INV-H4: Paint Order Independent of Side
SPEC-HAL-208 INV-C1 (indicator painted first, label second) governs paint
**sequence / z-order only** and is independent of horizontal position. A
`Leading` label is positioned before the indicator but still painted after it.

### INV-H5: No New Authority
Side selection grants no styling authority, introduces no synthetic widget
nodes, and adds no event forwarding. All SPEC-HAL-208 Class A and Class G
prohibitions remain binding.

---

## 4. Scope Amendment

Switch and Toggle move from *future* to **active** scope of SPEC-HAL-208.

Switch already satisfies the load-bearing invariants:
- INV-A1 / INV-A3: owns its label, renders text directly (no child nodes),
- INV-B1: reports one bounding rect covering track + gap + label,
- INV-D1: hit-tests its full bounds including the label (`switch.rs`
  `handle_event`, "full-area hit testing").

It gains Class H (label side). Trailing remains its default.

---

## 5. Migration (Non-Normative)

- No other control requires change; all default to `Trailing`.
- Checkbox / RadioButton / RadioGroup inherit Class H by scope but are unaffected
  unless a `Leading` label is explicitly requested.

---

## 6. Out of Scope

- RTL / bidirectional text-direction remapping. `Leading`/`Trailing` are defined
  in LTR flow order for now; no automatic mirroring under RTL locales.
- Vertical (above/below) label placement. This amendment governs horizontal
  side only.
