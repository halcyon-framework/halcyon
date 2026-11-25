# SPEC-PM-000: Pathology & Mistake Prevention (PM) Series — Purpose and Law

**Status:** Active / Foundational  
**Series:** SPEC-PM (Pathology & Mistake Prevention)  
**Layer:** Meta-Architecture / Engineering Process  
**Owner:** Core Architecture  
**Audience:** Engine, Widget, Interaction, Renderer  
**Applies To:** All HAL / E / R SPECs and implementations

---

## 0. Purpose

The SPEC-PM series exists to **capture, formalize, and permanently prevent known engineering failures** that repeatedly reappear despite correct local fixes.

This series is **not design** and **not feature specification**.

It is a **failure memory system**.

If a bug class already burned time, cognition, or trust once, it **must never be rediscovered by accident again**.

---

## 1. What SPEC-PM Is (and Is Not)

### SPEC-PM IS:
- A record of **real failures that actually occurred**
- A set of **hard rules derived from post-mortems**
- A **cross-cutting constraint layer** that applies to multiple subsystems
- A way to encode “this almost ruined us” into law

### SPEC-PM IS NOT:
- A replacement for SPEC-HAL / SPEC-E / SPEC-R
- A feature proposal
- A refactor plan
- A tutorial or explanation for beginners
- A place for opinions or preferences

If something belongs in “how this should work”, it is **not PM**.  
If something belongs in “we already stepped on this rake”, it **is PM**.

---

## 2. When an SPEC-PM Is Required

An SPEC-PM entry **must** be written when **any** of the following occur:

1. A bug reappears after being “fixed” previously
2. A failure spans multiple widgets (List, ListView, TreeView, etc.)
3. The root cause is **architectural**, not local
4. Virtualization, identity, or projection boundaries were violated
5. The same conceptual mistake required explanation more than once
6. A correct invariant existed mentally but not in writing

If an engineer says *“we already solved this once”*, and there is no SPEC-PM entry, that is a process failure.

---

## 3. Authority and Precedence

SPEC-PM has **binding authority**.

Rules defined in SPEC-PM:

- Override local widget assumptions
- Constrain future SPEC-HAL / SPEC-E / SPEC-R designs
- Cannot be bypassed by “special cases”
- Cannot be weakened without a **new SPEC-PM**

If an implementation technically satisfies an SPEC-HAL but violates SPEC-PM, the implementation is **invalid**.

---

## 4. Structure of the SPEC-PM Series

Each SPEC-PM document:

- Describes **one failure class**
- Names the **exact pathology**
- States **why it reappears**
- Defines **hard prevention rules**
- Lists **affected systems**
- States **forbidden patterns**

No implementation details unless required to forbid a pattern.

---

## 5. Scope of the SPEC-PM Series

SPEC-PM may address failures involving:

- Virtualization
- Identity vs visibility
- Projection vs ownership
- State mirroring
- Event ordering
- Geometry coupling
- Cached indices
- “Works until scroll”
- “Breaks after sort”
- “Fails when invisible”
- “Looks correct locally”

If the bug only exists because **human memory is unreliable**, it belongs here.

---

## 6. Series Naming and Position

The SPEC namespace is now:

- **SPEC-HAL-XXX** — Core engine architecture and law
- **SPEC-E-XXX** — Extensibility and composition
- **SPEC-R-XXX** — Renderer / backend specifics
- **SPEC-PM-XXX** — Pathology & mistake prevention (this series)

SPEC-PM documents are **orthogonal** to the others and may reference any of them.

---

## 7. Enforcement Rule

> If a future system (ListView, TreeView, Grid, PropertyEditor, etc.) reintroduces a pathology already covered by SPEC-PM, **the failure is considered preventable and unacceptable**.

The correct response is not debugging.

The correct response is:  
**“Which SPEC-PM rule did we violate?”**

---

## 8. Initial State

This SPEC establishes the series.

The **first concrete pathology document** will be:

**SPEC-PM-001 — Virtualization, Identity, and the ‘Invisible Selection’ Failure**

No other SPEC-PM entries may be written before SPEC-PM-001.

---

## 9. Closure

SPEC-PM-000 is foundational and **never closes**.

It defines *why* this series exists and *when* it must be used.

Ignoring this series means accepting repeated failure as normal.

That is not acceptable.
