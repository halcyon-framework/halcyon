# SPEC-HAL-241: Drag and Drop Interaction Authority

**Status:** Draft
**Layer:** Interaction Model / Input Routing / Event System
**Scope:** Drag initiation, routing, target resolution, and drop action semantics
**Owner:** Core Interaction Substrate Team
**Applies To:** All draggable/droppable controls across Halcyon

---

## 0. Purpose

This SPEC defines the architectural rules, authority model, and system interactions for drag and drop within the Halcyon UI framework. It establishes:
- Who owns drag state
- How targets are resolved
- What data contracts are permitted
- What behaviors are forbidden

---

## 1. Invariants

### INV-241.1: Input Substrate Owns Drag State

Drag initiation, movement, and termination are governed by the input system.

- Drag state is global and exclusive
- Only one active drag at a time
- Cancels on escape, input loss, or programmatic abort

### INV-241.2: Drag Sources Must Emit Intents

A control initiating a drag must define an intent — no implicit data exposure.

- Intent type defines allowed target contracts
- Examples: `Intent::DragAsset(AssetId)`, `Intent::MoveItem(RawItemRef)`

### INV-241.3: Drop Targets Must Declare Eligibility

Drop eligibility is resolved through authority check, not hit-only detection.

- Controls must declare accepted drag types via trait or registration
- Hit alone is not enough — routing consults capability

### INV-241.4: Visual Feedback Is Decoupled

The input system manages ghost rendering; visuals may be overridden via delegate.

- Drag image is optional, may be provided by source control
- Targets may alter their appearance, but must not intercept routing

### INV-241.5: Drops Dispatch Intents, Not Data

A successful drop results in a structured intent, not raw data delivery.

- Drop triggers `Intent::DropAssetHere(AssetId, DropTargetId)` or similar
- Drop target must not mutate data inline

---

## 2. Drag Lifecycle

```
[User Mousedown + Move] -> [DragStart Intent] -> [Drag State Captured]
-> [Move Events + Hit Detection] -> [Eligible Target?] -> [Drop Hover Feedback]
-> [Mouse Up] -> [Dispatch Drop Intent] -> [Release Drag State]
```

---

## 3. System Responsibilities

- Central input dispatcher manages drag state
- Routing layer queries drop targets
- Visual system renders ghost/feedback overlays

---

## 4. Constraints

- No overlapping drags
- Drag termination must be atomic
- Input system must expose cancel API

---

## 5. Out of Scope

- OS-level drag/drop (e.g., dragging files from desktop)
- Data transfer between apps
- Clipboard / copy-paste semantics (separate SPEC)
