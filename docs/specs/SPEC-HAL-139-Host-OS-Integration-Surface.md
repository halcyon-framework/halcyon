# SPEC-HAL-139: OS-Level Integration and System Mediation Contract

**Status:** Design / Binding
**Layer:** Platform Mediation (engine ↔ operating system)
**Scope:** Safe, explicit integration with host OS capabilities
**Depends on:** SPEC-HAL-127, SPEC-HAL-128, SPEC-HAL-137, SPEC-HAL-138
**Constrains:** Engine boundary only (no widget-level OS access)

---

## SECTION A — Problem Statement

Modern operating systems treat applications as **composited surfaces** governed by system-level services:

* window management
* input methods
* accessibility
* clipboard
* drag-and-drop
* system dialogs
* permissions
* native confirmations

Halcyon currently operates as a **self-contained UI engine**, which is correct — but without a **formal OS mediation layer**, integration risks:

* ad-hoc platform code
* security footguns
* performance regressions
* inconsistent behavior across platforms
* confirmation logic duplication

This SPEC defines **how Halcyon speaks to the OS** — and, critically, what it is **forbidden** to do.

---

## SECTION B — Design Principles

**PRINCIPLE-1: The OS sees one surface**
Halcyon exposes a single composited surface unless explicitly delegated.

**PRINCIPLE-2: Mediation, not embedding**
The OS is never “inside” Halcyon; Halcyon negotiates with it.

**PRINCIPLE-3: Capability-based access**
All OS interactions are explicit capabilities, never ambient.

**PRINCIPLE-4: Engine-owned boundary**
Widgets never talk to the OS.

**PRINCIPLE-5: Replaceable backends**
Platform specifics are pluggable, not structural.

---

## SECTION C — OS Interaction Domains

Halcyon may integrate with the OS only through these domains:

1. Windowing & lifecycle
2. Input methods (IME, dead keys)
3. Clipboard
4. Drag-and-drop
5. Accessibility
6. Native dialogs
7. Permissions & secure prompts
8. System theming & metrics
9. System menu projection

Anything else is forbidden unless SPEC’d.

---

## SECTION D — Capability Model

All OS integration is gated by **explicit capabilities**.

Examples:

* ClipboardRead
* ClipboardWrite
* NativeDialog
* FilePicker
* DragSource
* DragTarget
* AccessibilityBridge
* SystemMenuProjection

**INV-OS-CAP-1**
Capabilities must be requested explicitly by the engine.

**FORBID-OS-AMBIENT-ACCESS**
No implicit OS access is allowed.

---

## SECTION E — Native Dialog Integration

Native dialogs (file picker, confirmation, permission prompts) integrate via SPEC-HAL-138.

Pipeline:

1. Engine emits confirmation intent
2. OS backend may satisfy confirmation using native UI
3. Result feeds back into confirmation pipeline

**INV-OS-DIALOG-1**
Native dialogs are substitutes for confirmation UI, not shortcuts.

**INV-OS-DIALOG-2**
Dialog dismissal without explicit choice equals Cancel.

---

## SECTION E2 — System Menu Projection

Native system menu bars integrate through SPEC-HAL-333.

Pipeline:

1. Engine owns a platform-neutral menu model
2. OS backend may project that model into native system chrome
3. Native activation returns an engine-owned command identity
4. Engine dispatches through the Action System in its mutation-legal phase

**INV-OS-MENU-1**
Native menus are projections, not authorities.

**INV-OS-MENU-2**
OS menu APIs may not own focus, routing, validation, edit semantics, lifecycle policy, window/domain state, clipboard state, or application state.

**INV-OS-MENU-3**
Menu services that require selection or clipboard access must use explicit Halcyon capabilities. Ambient OS access is forbidden.

**INV-OS-MENU-4**
The first permitted native system menu backend is macOS main-menu projection. Windows `HMENU`, Linux dbusmenu, Wayland appmenu, tray menus, dock menus, and status-item menus require separate SPEC authority.

---

## SECTION F — Input Method Editors (IME)

IME is treated as **system-owned text composition**.

Rules:

* IME state is opaque to widgets
* engine mediates composition events
* widgets receive committed text only

**INV-OS-IME-1**
IME must not bypass Halcyon’s text input pipeline.

**FORBID-OS-IME-WIDGET**
Widgets must not handle IME directly.

---

## SECTION G — Clipboard Contract

Clipboard access is transactional:

* read → snapshot
* write → explicit intent

**INV-OS-CLIP-1**
Clipboard reads never mutate selection.

**INV-OS-CLIP-2**
Clipboard writes require explicit user intent.

---

## SECTION H — Drag and Drop

OS drag-and-drop integrates with existing pointer capture rules.

* OS drag start → translated to engine drag intent
* engine validates acceptance
* OS drop resolves to engine intent

**INV-OS-DND-1**
OS drag does not bypass gesture arbitration.

**INV-OS-DND-2**
Drop resolution must pass through validation (SPEC-HAL-138).

---

## SECTION I — Accessibility Bridge

Accessibility is **observational**, not interactive by default.

* expose tree structure
* expose roles and labels
* expose focus and selection

**INV-OS-A11Y-1**
Accessibility queries must not trigger layout or realization.

**FORBID-OS-A11Y-MUTATION**
Accessibility must not mutate UI state.

---

## SECTION J — Window and Lifecycle

OS lifecycle events:

* suspend
* resume
* minimize
* resize
* DPI change

Rules:

* engine receives notifications
* widgets react indirectly via layout and state invalidation

**INV-OS-WINDOW-1**
Lifecycle events must not inject input.

---

## SECTION K — Performance and Isolation

**INV-OS-PERF-1**
OS callbacks must not block the render loop.

**INV-OS-PERF-2**
No synchronous OS calls during paint.

**INV-OS-PERF-3**
All OS mediation is edge-triggered, never polled.

---

## SECTION L — Security Model

* OS access is capability-scoped
* confirmation pipeline governs sensitive actions
* platform code cannot bypass engine invariants

**FORBID-OS-BACKDOOR**
Platform backends must not mutate engine state directly.

---

## SECTION M — Acceptance Criteria

SPEC-HAL-139 is satisfied when:

* [ ] All OS access is capability-gated
* [ ] Widgets have zero OS imports
* [ ] Native dialogs integrate via SPEC-HAL-138
* [ ] IME works without widget awareness
* [ ] Accessibility is query-only
* [ ] Performance is unaffected at scale

---

## SECTION N — Authoritative Statements

**AUTH-139-1:** “The OS sees one surface.”
**AUTH-139-2:** “Capabilities are explicit or nonexistent.”
**AUTH-139-3:** “System integration must never weaken engine invariants.”
