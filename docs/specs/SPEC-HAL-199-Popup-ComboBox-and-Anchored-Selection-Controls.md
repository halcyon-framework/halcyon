# SPEC-HAL-199: Popup, ComboBox, and Anchored Selection Controls

**Status:** Draft (Proposed)
**Layer:** Core UI Primitives / Interaction Infrastructure
**Scope:** All selection-based widgets (ListView, TreeView, GridView, Settings Panels, Context Menus)
**Depends On:** SPEC-HAL-179, SPEC-HAL-195, SPEC-HAL-196, SPEC-HAL-197, SPEC-HAL-198
**Bound By:** SPEC-HAL-002 §3.11 (No Interpretive Intent Resolution), §4.11 (Explicit Commit Authority)
**Non-Goals:** Widget-owned search logic, widget-owned async fetching, hierarchical popup structures, multi-column popup layouts

---

## 0. Purpose

This SPEC introduces a foundational popup / overlay system and a feature-complete ComboBox control built on top of it.

The goal is not to introduce a cosmetic widget, but to establish a reusable, authority-correct, composition-first infrastructure for all anchored, transient, selection-based UI interactions.

This SPEC deliberately avoids advanced features (search, async data, virtualization) while still being feature-complete for real-world use.

---

## 1. Problem Statement

Halcyon currently lacks:

- A general-purpose popup / overlay primitive
- A canonical way to anchor transient UI to a widget or rect
- A unified selection popup usable across widgets

As a result, essential UI features are blocked or risk being implemented as ad-hoc logic:

- Column visibility menus
- Sorting menus
- Pinning menus
- Context menus
- Settings pickers

ComboBox is the forcing function that makes these deficiencies explicit.

---

## 2. Non-Goals (Pandora Box Guardrails)

This SPEC explicitly does NOT include:

- Widget-owned search logic
- Widget-owned async fetching
- Hierarchical popup structures
- Multi-column popup layouts

These are deferred to future SPECs.

---

## 3. Core Architectural Insight

ComboBox is not a widget.

It is a composition of three orthogonal systems:

1. Popup / Overlay Infrastructure (foundational)
2. ListPopup (selection list)
3. Control Surface (button + optional TextField)

Each layer has distinct ownership and authority.

---

## 4. Popup / Overlay Infrastructure (Foundation)

### 4.1 Popup Definition

A Popup is a transient widget rendered in a higher z-layer, anchored to a widget or rect, participating fully in event routing.

### 4.2 Popup Invariants

### INV-POPUP-ANCHOR-1 — Anchor-Derived Position

Popup position is derived from an anchor rect or widget id.

### INV-POPUP-ZORDER-1 — Above Anchor

Popup always renders above its anchor.

### INV-POPUP-DISMISS-1 — Dismissible

Popup can be dismissed by click-away, Esc, or explicit command.

### INV-POPUP-FOCUS-1 — Focus Capture

Popup captures focus while open.

### INV-POPUP-ROUTING-1 — External Event Interception

Events outside popup content are intercepted for dismissal.

### 4.3 Anchor Model

Supported anchor sources:

- `WidgetId` (resolved to layout rect)
- Explicit `RectF`
- Screen-space point

Placement policies:

- Below
- Above
- Auto (viewport-aware)

---

## 5. ListPopup (Selection List)

### 5.1 Purpose

ListPopup is a menu-like, vertical selection list intended for transient use.

It deliberately reuses List (TabularView consumer) internally.

### 5.2 Why List Reuse Is Correct

- Proven virtualization + row semantics
- Stable selection machinery
- Keyboard navigation already aligned with ListView
- Avoids a parallel list implementation

List is used in single-column mode, without headers, resizing, or pinning.

### 5.3 ListPopup Invariants

### INV-LP-SELECTION-1 — Single Selection

Exactly one active selection (or none).

### INV-LP-KEYBOARD-1 — Keyboard Navigation

Up/Down navigates, Enter selects, Esc dismisses.

### INV-LP-FOCUS-1 — Contained Focus

Focus is fully contained while open.

### INV-LP-HEIGHT-1 — Viewport Clamping

Height clamps to viewport with scroll.

---

## 6. ComboBox Control

### 6.1 Definition

ComboBox is a control surface that:

- Displays current selection
- Opens a `Popup(ListPopup)` on interaction
- Binds selection bidirectionally

### 6.2 Internal Composition

```
ComboBox
├── Control Surface (Button-like)
│   └── Optional TextField (display-only in v1)
└── Popup
    └── ListPopup (List-backed)
```

### 6.3 TextField Reuse

The ComboBox display surface may reuse TextField in read-only mode:

- Ensures consistent text layout
- Enables future transition to editable ComboBox
- Avoids bespoke text rendering

### 6.4 ComboBox Invariants

### INV-CB-STATE-1 — Open/Close Ownership

Open/close state owned by ComboBox.

### INV-CB-SELECTION-1 — External Selection Ownership

Selection owned externally.

### INV-CB-DISMISS-1 — Atomic Commit

Selection commits and popup closes atomically.

### INV-CB-FOCUS-1 — Focus Return

Focus returns to control on close.

### 6.5 Input-Driven Selection Semantics

ComboBox MAY include an optional text input surface.

Typing emits input and search signals routed to the data adapter. Search semantics — filtering, matching, ranking — are exclusively adapter-owned.

The widget never filters, normalizes, ranks, or matches items itself. The widget renders adapter-provided results and forwards user actions.

### INV-CB-INPUT-1 — Input Is Intent, Not Search

Text input is treated as intent signaling. ComboBox never performs filtering or matching internally.

### INV-CB-DATA-1 — Data Layer Ownership

All search, filtering, ranking, and fetching logic lives in the data adapter / model layer.

### INV-CB-CUSTOM-1 — Custom Value Policy

Acceptance of values not present in the list is governed by an explicit policy flag.

If disallowed, Enter on unmatched input is rejected. If allowed, Enter commits the custom value atomically.

### INV-CB-AUTOCOMPLETE-OPTIONAL — Auto-Completion (Optional)

Auto-completion is optional and adapter-driven. The widget may display suggestions but never generates them.

### 6.6 Commit Source Rules

Only two commit sources exist:

1. **Explicit selection** — clicking a list item, or keyboard navigation (Up/Down) followed by confirm (Enter)
2. **Explicit confirm of raw input** — Enter pressed with typed text

Clicking a list item is a direct, final commit of that item. The widget forwards the committed value immediately.

Reopening the popup after a commit begins a new interaction. It is not retroactive modification of the previous commit.

### INV-CB-COMMIT-1 — Explicit Commit Only

All commits MUST originate from an explicit user action (pointer activation or keyboard confirm). No implicit, timed, or focus-loss commits.

### INV-CB-COMMIT-2 — Click Is Final

Clicking a list item commits that item immediately and atomically. No post-click matching, normalization, or substitution.

### 6.7 Ambiguity Resolution

If typed input and list selection diverge, the widget MUST NOT infer which the user intended.

The adapter decides what Enter means when input text does not match the highlighted selection. The widget only forwards the raw input and the current selection state to the adapter.

### INV-CB-AMBIGUITY-1 — No Widget-Side Disambiguation

When typed input conflicts with list selection, the widget forwards both signals to the adapter without choosing between them. The adapter's resolution policy governs the outcome.

### 6.8 Default Resolver

Halcyon MAY ship a default adapter-level resolver providing common matching strategies (case-insensitive exact match, prefix match, etc.).

The default resolver is replaceable. Applications may substitute any resolver that satisfies the adapter interface.

Widgets remain oblivious to resolver implementation. No widget behavior changes based on which resolver is active.

### 6.9 Explicit Prohibitions

The following widget-side behaviors are permanently forbidden:

- Auto-matching typed input to existing values at commit time
- Case folding or normalization for commit decisions
- Choosing between typed value and selected value
- "Helpful" substitution or smart defaults at commit
- Inferring commit intent from partial input

Widgets MAY assist intent formation (highlighting matches, displaying suggestions, scrolling to candidates), but all assistance MUST remain reversible. Only explicit user actions produce commits.

This section enforces SPEC-HAL-002 §3.11 (No Interpretive Intent Resolution) and §4.11 (Explicit Commit Authority) within the ComboBox domain.

### 6.10 Terminology

| Term | Definition |
|------|------------|
| Resolver / Adapter | Owns matching, search, filtering, ranking, and commit resolution rules |
| Widget | Renders UI, routes input, forwards intents to adapter |
| Commit | Irreversible action originating from explicit user command only |

---

## 7. Authority Model

Clear separation is mandatory:

| Concern | Owner |
|---------|-------|
| Selection Value | Model / Adapter |
| Matching / Resolution | Resolver / Adapter |
| Commit Policy | Resolver / Adapter |
| Popup Lifetime | Popup System |
| Open State | ComboBox |
| Rendering | Widget |
| Input Forwarding | Widget |
| Keyboard Routing | Popup |

Violations are forbidden.

---

## 8. Generalization Across Widgets

This system directly enables:

- Column visibility menus (checkbox ListPopup)
- Sorting menus (single/multi)
- Pinning menus
- TreeView node menus
- Settings pickers

Without ListView-specific logic.

---

## 9. Future Extensions (Explicitly Deferred)

- Multi-column popup lists
- Hierarchical menus
- Async data adapters

Each requires a separate SPEC.

Note: Input-driven selection and search-as-you-type are now architecturally permitted via §6.5-6.8. They are adapter-driven features, not widget features, and do not require a separate SPEC.

---

## 10. Summary

This SPEC establishes:

- Popup as a first-class primitive
- ListPopup as a reusable selection surface
- ComboBox as a thin composition layer

It closes a foundational gap and reduces future architectural entropy.
