# SPEC-HAL-214: SectionHeader Primitive Invariants

**Status:** Binding
**Layer:** Visual Primitive / Layout Structure
**Scope:** SectionHeader widget only
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-213

**Amended by:** SPEC-HAL-349, which adds theme-owned primary and subtle emphasis presentations without changing SectionHeader ownership or interaction semantics.

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the SectionHeader primitive.

A SectionHeader provides **visual structure and labeling**
for grouped content without introducing interaction,
navigation, or hierarchy semantics.

This SPEC exists to prevent SectionHeaders from turning into:
- collapsible containers
- navigational elements
- implicit grouping authorities

SectionHeader is **descriptive, not operative**.

---

## 1. Conceptual Definition

A **SectionHeader** is a visual label that:
- names a section of content
- optionally includes secondary text
- improves readability and scanability

It must **never**:
- control layout of children
- manage expand/collapse state
- participate in navigation

---

## Invariant Class A — Ownership & Authority

### INV-A1: No Content Ownership

SectionHeader does not own or manage content beneath it.

It is **not a container**.

---

### INV-A2: No Structural Authority

SectionHeader does not define:
- grouping
- hierarchy
- containment
- ordering

It is purely visual context.

---

## Invariant Class B — Interaction Semantics

### INV-B1: Non-Interactive by Default

SectionHeader must not:
- accept pointer events
- accept keyboard events
- be focusable

---

### INV-B2: No Implicit Actions

Clicking or hovering a SectionHeader must never:
- toggle visibility
- scroll
- navigate
- emit events

---

## Invariant Class C — Text & Label Semantics

### INV-C1: Primary Label Is Mandatory

A SectionHeader always has:
- a primary text label

Empty headers are forbidden.

---

### INV-C2: Secondary Text Is Optional

Optional secondary text:
- is visual only
- uses secondary typography
- carries no semantics

---

### INV-C3: Label Ownership

All text is **owned directly** by the SectionHeader.

No child Label widgets.
No external composition.

---

## Invariant Class D — Layout Contract

### INV-D1: Self-Contained Geometry

SectionHeader layout accounts for:
- primary text
- optional secondary text
- theme-defined spacing

It does not depend on surrounding widgets.

---

### INV-D2: No Child Layout

SectionHeader does not measure or position other widgets.

---

### INV-D3: Associated Divider Position

A SectionHeader's visual divider is positioned **above** the header text,
never inline, trailing, or between primary and secondary text.

The divider is **visually bound to the header**, not to surrounding content.
It belongs to the header's visual affordance even when implemented
as a separate Divider widget (SPEC-HAL-213).

---

### INV-D4: Spacing Hierarchy

Spacing between a SectionHeader's divider and its primary text
must be **smaller** than spacing between consecutive sections.

This ensures headers visually attach to their divider,
not to the preceding section's content.

All spacing values must be:
- theme-derived
- proportional to typography metrics (e.g. line_height)
- never hardcoded pixel literals

---

## Invariant Class E — Visual Rendering

### INV-E1: Typography-Driven

Rendering uses:
- theme typography styles
- theme colors

No hardcoded fonts or sizes.

---

### INV-E2: No Decoration Semantics

SectionHeader must not:
- look like a button
- use focus rings
- use hover affordances

---

### INV-E3: Typography Weight Distinction

The primary label uses **emphasized weight** (theme-defined).
Secondary text uses **regular / secondary weight** (theme-defined).

This distinction is **visual only** — it carries no semantic meaning.
Weight values must come from the theme, never hardcoded.

---

## Invariant Class F — Determinism & Safety

### INV-F1: Deterministic Rendering

Rendering depends only on:
- provided text
- theme
- layout constraints

---

### INV-F2: Zero Allocation Steady State

No per-frame allocation during layout or paint.

---

### INV-F3: Invariant Supremacy

No invariant may be violated for:
- UX polish
- platform imitation
- convenience

Extensions require a new SPEC.

---

## 2. Non-Goals (Explicit)

This SPEC does **not** define:
- collapsible sections
- navigation headers
- anchors or IDs
- interactive disclosure patterns

Those are separate concerns.

---

## 3. Summary (Non-Normative)

- SectionHeader is **visual context only**
- No interaction, no hierarchy, no control
- Divider belongs above the header, visually bound to it
- Primary text is emphasized, secondary text is not
- Spacing is invariant-bound and theme-derived
- Improves structure without owning it

This SPEC locks SectionHeader as a **safe structural primitive**.
