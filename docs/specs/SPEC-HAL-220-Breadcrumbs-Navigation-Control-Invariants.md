# SPEC-HAL-220: Breadcrumbs Navigation Control Invariants

**Status:** Binding
**Layer:** Navigation Control
**Scope:** Breadcrumbs control only
**Ground Truth:** SPEC-HAL-201, SPEC-HAL-206, SPEC-HAL-207, SPEC-HAL-208, SPEC-HAL-216, SPEC-HAL-218, SPEC-HAL-228

---

## 0. Purpose

This SPEC defines **engine-grade invariants** for the Breadcrumbs navigation control.

Breadcrumbs provide **contextual navigation visibility** and **explicit jump-back affordances**
within a resolved navigation path.

This SPEC exists to prevent Breadcrumbs from turning into:
- hidden routers
- stateful navigation controllers
- implicit history managers
- hierarchy or taxonomy representations

Breadcrumbs **expose a resolved path**, they do not **own navigation** or **represent structure**.

---

## 1. Conceptual Definition (Binding)

Breadcrumbs represent the **resolved navigation path** as produced by NavigationHost (SPEC-HAL-228).

The path is ordered by **navigation resolution**, not by taxonomy, hierarchy, or category structure.

```
Home / Electronics / Laptops
```

This path means: the user navigated Home → Electronics → Laptops.
It does **not** mean: Laptops is a child of Electronics is a child of Home.

Breadcrumbs allow the user to:
- understand where they are in the navigation sequence
- navigate explicitly to earlier points in the resolved path

Breadcrumbs do **not**:
- manage routing, history, or content
- represent hierarchical relationships
- infer or enforce structural ordering

---

## 2. Authority Source (Binding)

Breadcrumbs are a **pure projection** of NavigationHost state (INV-228-D1).

- Path order is owned by NavigationHost — Breadcrumbs never compute or reorder it
- Current crumb identity is owned by NavigationHost — Breadcrumbs never infer it
- Breadcrumbs must never inspect widgets, focus trees, or internal state to derive path information

### Absent Authority Behavior (Binding)

When Breadcrumbs receive an **empty path** (no crumbs provided), they render **nothing**: zero visual output, zero layout footprint, zero hit regions.

This is the only allowed behavior. The following are **forbidden**:
- Rendering a placeholder, skeleton, or disabled state
- Displaying cached or last-known crumbs
- Inventing crumbs from widget state, route IDs, or any other source
- Falling back to locally stored path data
- Any "best effort" or "graceful degradation" rendering

Empty input → empty output. No exceptions.

---

## Invariant Class A — Ownership & Authority

### INV-A1: No Routing Authority

Breadcrumbs must not:
- perform navigation directly
- mutate application state
- manage history stacks

They emit **explicit navigation intents only**.

---

### INV-A2: Path Is External

Breadcrumbs do not own the path.

The application provides (via NavigationHost projection):
- the ordered list of crumbs
- which crumb is current

---

## Invariant Class B — Crumb Semantics

### INV-B1: Linear Ordered Path

Breadcrumbs represent a **strictly ordered sequence**.

No branching.
No graph semantics.

The ordering is determined by navigation resolution, not by any structural relationship between nodes.

---

### INV-B2: Exactly One Current Crumb

Exactly one crumb is marked as **current**.

The current crumb represents the active location.

---

### INV-B3: Past Crumbs Are Navigable

Crumbs **before** the current one:
- may be activated
- emit navigation intent for that crumb

Crumbs **after** the current one must not exist.

---

## Invariant Class C — Interaction Semantics

### INV-C1: Explicit Activation Only

Navigation occurs only via:
- pointer activation
- keyboard activation

No hover-triggered navigation.

---

### INV-C2: Current Crumb Is Non-Navigational by Default

By default:
- the current (last) crumb is **not clickable**
- activating it performs no action

This prevents accidental self-navigation.

---

### INV-C3: Optional Dropdown on Current Crumb (Explicit)

The **current crumb may optionally expose a DropdownMenu**.

If enabled:

- the dropdown is **explicitly configured**
- it lists **valid jump targets earlier in the path**
- selecting an item emits a navigation intent

This behavior is **opt-in** and **never implicit**.

---

## Invariant Class D — Dropdown Integration (If Enabled)

### INV-D1: Dropdown Is a Separate Control

The dropdown attached to a crumb:
- follows SPEC-HAL-218 (DropdownMenu)
- follows SPEC-HAL-216 (PopupMenu semantics)

Breadcrumbs do not reimplement menu behavior.

---

### INV-D2: No Hidden Navigation Logic

The dropdown:
- does not infer destinations
- does not auto-populate
- reflects only explicitly provided targets

---

### INV-D3: Dropdown Does Not Change Breadcrumb State

Opening or using the dropdown:
- does not mutate the breadcrumb path
- does not mark crumbs active/inactive

Only emitted intents may cause external updates.

---

## Invariant Class E — Visual & Layout Contract

### INV-E1: Crumbs Are Visual Labels

Each crumb consists of:
- a label
- an optional separator

No crumb is a container.

---

### INV-E2: Current Crumb Is Visually Distinct

The current crumb must be visually differentiated.

Visual distinction does not imply interactivity.

---

### INV-E3: Geometry Is Self-Contained

Breadcrumbs layout accounts only for:
- crumbs
- separators
- optional dropdown affordance

It does not depend on content size or routing depth.

---

## Invariant Class F — Disabled & Availability Semantics

### INV-F1: Disabled Breadcrumbs Are Absolute

When disabled:
- no crumb is activatable
- no dropdown may open

---

### INV-F2: Disabled Crumb Is Inert

A disabled crumb:
- emits no navigation intent
- opens no dropdown

---

## Invariant Class G — Binding & Integration

### INV-G1: Intent Emission Only

Breadcrumbs emit:
- explicit navigation intents (crumb index or key)

They do not:
- bind to models
- perform routing
- update state directly

---

### INV-G2: No Form Binding

Breadcrumbs must not integrate with FormBinder.

Navigation ≠ form input.

---

## Invariant Class H — Determinism & Safety

### INV-H1: Deterministic Interaction

Given the same path and inputs,
the same navigation intent is emitted.

---

### INV-H2: No Per-Frame Allocation

Idle and steady interaction must not allocate.

---

### INV-H3: Invariant Supremacy

No invariant may be violated for:
- UX convenience
- platform imitation
- feature parity

New behavior requires a new SPEC.

---

## 3. Semantic Boundary: Breadcrumbs Are Not Hierarchy

Breadcrumbs are **not** and must never be interpreted as:
- category paths
- taxonomy indicators
- hierarchy locators
- tree projections
- parent/child representations

Breadcrumbs reflect **where the user navigated**, not **where something is located** in a structural sense.

### Canonical Example

Consider this navigation sequence:

1. User starts at Home
2. User navigates to Electronics
3. User navigates to Laptops
4. User navigates to Library

Breadcrumbs show: `Home / Electronics / Laptops / Library`

This path is **valid**. Library is not a child of Laptops. There is no parent/child relationship implied. The path records navigation resolution order — nothing more.

A hierarchical UI would show Library under a different parent entirely. That is a different concept, a different control, and a different SPEC.

### Terminology Lock-In

Any UI component that represents canonical hierarchy, taxonomy, or category location is **not Breadcrumbs** and requires a separate model and SPEC.

This boundary is permanent.

---

## 4. Non-Goals (Explicit)

This SPEC does **not** define:
- routing implementation
- history stacks
- breadcrumb auto-generation
- deep-linking or URLs
- hierarchical structure representation

Those belong to higher-level navigation systems or separate SPECs.

---

## 5. Summary (Non-Normative)

- Breadcrumbs expose **where you navigated**
- Path order is **navigation resolution order**, not structural order
- Navigation is **explicit and external**
- Authority is **NavigationHost** — Breadcrumbs are a pure projection
- Optional dropdown on the current crumb is allowed — **only if declared**
- Breadcrumbs are permanently **not hierarchy**

This SPEC locks Breadcrumbs as a **transparent navigation surface**, not a controller, and not a structural indicator.
