# SPEC-HAL-164: TabularSubstrate — Root Abstraction and Invariants

**Status:** Implemented / Closed
**Layer:** Widget / Structural Composition  
**Audience:** Engine, Widget, Layout, Virtualization  
**Depends on:**  
SPEC-HAL-008, SPEC-HAL-024, SPEC-HAL-037, SPEC-HAL-079, SPEC-HAL-086, SPEC-HAL-131A/B/C, SPEC-HAL-141, SPEC-HAL-145,
SPEC-HAL-146, SPEC-HAL-158, SPEC-HAL-159  
**Supersedes:** SPEC-HAL-117, SPEC-HAL-118*, SPEC-HAL-114 (as substrate)
**Extended by:** SPEC-HAL-353 for the data-layer/substrate order-authority split in SPEC-HAL-188-backed consumers

---

## 1. Purpose

This SPEC introduces **TabularSubstrate** as a **root structural abstraction** for all column-based views.

TabularSubstrate is **not** a List with columns.  
It is a **first-class columnar structure** with explicit geometry domains.

ListView, TableView, and TreeView are **projections over TabularSubstrate**, not extensions of List.

---

## 2. Core Failure Being Corrected

The deprecated SPEC-HAL-117/118 series attempted to:

* treat List as a substrate
* layer columns, headers, and interactions on top
* retrofit geometry after layout

This violated:

* Rect Mutation Authority
* Temporal Geometry Ownership
* Single-Layout-Per-Frame
* Stable Widget Identity
* Explicit domain separation

TabularSubstrate fixes this by making **columns structural**, not decorative.

---

## 3. Semantic Engine Authority

TabularSubstrate, as defined by the SPEC series HAL-164 through HAL-178, is the **authoritative mechanics engine** for
row-based tabular interaction over its current RowId projection. For SPEC-HAL-188-backed consumers, the data layer owns
row-universe identity testimony, sort semantics, and confirmed applied order under SPEC-HAL-353.

**INVARIANT-A1:**
TabularSubstrate owns the following semantic authorities, distributed across its component SPECs:

* **RowId projection mechanics and effective traversal order** — including tree-visible traversal over the confirmed
  provider projection. For SPEC-HAL-188-backed consumers, the data layer owns base/applied order and the substrate must
  not compute a second sort projection (SPEC-HAL-172, SPEC-HAL-171, SPEC-HAL-188, SPEC-HAL-353)
* **Column identity and resolved horizontal geometry** — including header/body alignment, single-resolution-per-frame enforcement (SPEC-HAL-165, SPEC-HAL-168)
* **Selection semantics** — row-based, reducer-driven, identity-stable selection with anchor and range computation (SPEC-HAL-178)
* **Cell focus and navigation semantics** — virtualization-safe, identity-based focus with navigation rules (SPEC-HAL-175)
* **Inline edit arbitration** — exclusive edit authority, destabilization rules, cancellation semantics (SPEC-HAL-174, SPEC-HAL-174A)
* **Intent execution authority** — `handle_intent` entry point and internal deferral infrastructure for substrate-owned
  mechanics. Data-backed sort operations cross the adapter to the data layer for execution (SPEC-HAL-177,
  SPEC-HAL-353)

**INVARIANT-A2:**
No embedding component (ListView, TreeView, PropertyView, or future tabular constructs) may redefine, shadow, or
duplicate substrate mechanics authority or data-layer order authority.

Violations of semantic engine authority are architectural failures.

---

## 4. Embedding Component Responsibilities (Non-Authority)

Embedding components (ListView, TreeView, PropertyView) integrate TabularSubstrate into concrete widgets. Their responsibilities are strictly bounded.

**Permitted:**

* **Intent mapping** — Translate raw gestures, keyboard input, and interaction events into semantic intents. Substrate-owned intents enter TabularSubstrate; data-backed sort operations cross DataAdapter to the data layer. This is policy, not authority.
* **Column configuration inputs** — Provide column definitions (min/max/preferred/flex/visibility) as inputs to the Column Model. Embedding components do not own or mutate the Column Model directly.
* **Data/provider wiring** — Connect external data sources and providers through the adapter to the substrate's confirmed RowId projection. Data ownership, order semantics, and mutation authority remain external.
* **Persistence** — Manage lawful column presentation and requested query preferences under SPEC-HAL-185/353. Persistence is external input, not semantic authority; selection, focus, applied order, and scroll are not restored by ListView.
* **Visual presentation** — Define styling, theming, and rendering behavior. Presentation must not alter semantics.

**Forbidden:**

* Duplicating or shadowing selection, focus, edit, data-layer order, or column-geometry authority
* Maintaining parallel state for any substrate-owned semantic
* Applying sorting, resizing, or reorder mutations directly during input dispatch (intent/request-only path required;
  data-backed sorting executes in the data layer)
* Defining navigation, selection mode, or edit arbitration rules that contradict substrate SPECs
* Claiming ownership of "column interaction policy" or "row ordering contract" beyond intent mapping

---

## 5. Canonical Presentation Contract

A TabularSubstrate instance defines a canonical tabular presentation consisting of:

* A **non-scrolling header domain** (horizontal scroll coupled, vertical scroll independent)
* A **vertically scrollable body domain** (receives virtualized row content)
* A **shared horizontal coordinate space** (single Column Geometry Table)

**INVARIANT-A3:**
Any component embedding TabularSubstrate must respect this structure and may not rearrange, duplicate, or independently manage these domains.

Header and body domains must not be independently managed as separate tabular authorities; both must consume the same authoritative column geometry and share the single horizontal coordinate space.

---

## 6. Root Structural Model

TabularSubstrate is composed of **three immutable geometry domains**:

1. **Header Domain**
2. **Body Domain**
3. **(Optional) Footer Domain**

These domains:

* are siblings, not overlays
* participate in layout independently
* share column geometry via a single authority

There is **exactly one column model** per TabularSubstrate instance.

---

## 7. TabularSubstrate Is Not a Widget Tree

TabularSubstrate is a **structural coordinator**, not a content container.

It does **not**:

* own arbitrary child widgets
* perform rendering
* implement interaction semantics directly

It **does**:

* define column geometry
* resolve horizontal extents
* coordinate vertical projection
* expose stable identity hooks for projections

---

## 8. Column Geometry Authority (Binding Law)

**INVARIANT-1:**  
Column geometry is resolved **once per frame** by TabularSubstrate.

**INVARIANT-2:**  
Header, Body, Footer **must not compute column widths independently**.

**INVARIANT-3:**  
No projection may mutate column geometry during paint or input handling.

Violations are fatal.

---

## 9. Projection Model

TabularSubstrate exposes **projection slots**, not children:

* Header Projection
* Row Projection
* Cell Projection

Projections:

* receive resolved column geometry
* may not alter it
* may cache only frame-local derived data

Tree indentation, row expansion, inline editors, etc. live **entirely inside projections**.

---

## 10. Virtualization Boundary

TabularSubstrate defines **horizontal structure** only.

Vertical concerns are delegated:

* row realization
* variable height
* scroll windowing

Virtualization is layered **on top**, not baked in.

TabularSubstrate must remain functional with:

* zero virtualization
* full virtualization
* mixed strategies

---

## 11. Interaction Neutrality (Root Abstraction Scope)

TabularSubstrate itself:

* does not handle pointer events
* does not own focus
* does not route actions

It exposes **geometry and hit regions** for projections to bind against.

**Note:** Interaction semantics (selection, focus, edit, navigation) are defined in child SPECs within the TabularSubstrate series (SPEC-HAL-174 through SPEC-HAL-178). Those SPECs are authoritative for their respective domains. This section describes only the scope of SPEC-HAL-164 itself.

---

## 12. Explicit Non-Goals (Root Abstraction Scope)

This SPEC (HAL-164) does **not** define:

* selection semantics (see SPEC-HAL-178)
* sorting semantics (see SPEC-HAL-171 for header intent and SPEC-HAL-353 for data-backed execution/order authority)
* resize interaction (see SPEC-HAL-169)
* focus/navigation (see SPEC-HAL-175)
* edit arbitration (see SPEC-HAL-174)
* scrolling behavior
* tree semantics (see SPEC-HAL-172)
* window or surface identity

Those belong to child SPECs within the TabularSubstrate series.

---

## 13. Non-Responsibilities (Full Series Scope)

The TabularSubstrate series (SPEC-HAL-164 through SPEC-HAL-178) explicitly excludes ownership of:

* **Data fetching/mutation** — The substrate does not fetch, store, or mutate business data. Data ownership is external.
* **Permission interpretation** — Access control, authorization, and permission checks belong to the external data/policy layer; embedding components project capability testimony only.
* **Persistence formats** — Storage formats, serialization, and external state management belong to embedding components.
* **Gesture decoding** — Raw input interpretation (mouse vs touch, modifier keys, gesture recognition) belongs to embedding components. Already-decoded intents enter the owning substrate or data-adapter request boundary.
* **Theming/styling** — Visual appearance, colors, fonts, and decorative elements are presentation concerns.

These responsibilities belong to the explicitly owning external data/policy layer or embedding component, not the substrate engine.

---

## 14. Survivors This SPEC Relies On

This SPEC explicitly relies on the continued validity of:

* SPEC-HAL-008 — Layout Engine
* SPEC-HAL-024 — Coordinate Model
* SPEC-HAL-037 — Widget Identity Lifecycle
* SPEC-HAL-079 — Unified Event Flow v3
* SPEC-HAL-086 — Scroll and Viewport System
* SPEC-HAL-131A/B/C — Virtualization Substrate
* SPEC-HAL-141 — Rect Mutation Authority
* SPEC-HAL-145 — Layout Transitions and Reflow Authority
* SPEC-HAL-146 — Temporal Ownership of Geometry
* SPEC-HAL-158 — Layout Authority and Invalidation Law
* SPEC-HAL-159 — Text Ownership and Layout Authority

None of these are extended or modified here.

---

## 15. What Comes Next

This SPEC defines **only the root abstraction**.

Follow-up SPECs will define:

* column model
* header domain
* body/row projection
* resize interaction
* sorting affordances
* tree projection

No implementation work is permitted until **SPEC-HAL-165** is written and accepted.

---

## 16. Closure

**This SPEC is fully implemented and closed.** Implementation: `src/widgets/tabular_view/mod.rs` (TabularSubstrate struct, domain coordination), with Header, Body, and Column domains as sibling modules. All invariants enforced by structural separation and geometry authority wiring. No further work permitted under this SPEC.

**Proposed relationship to SPEC-HAL-363/364 (2026-10-05):** The paired
documents remain Proposed pending joint adoption. This annotation changes no
declared status, implemented scope or substantive law in this document.

SPEC-HAL-363 §10 and SPEC-HAL-364 §§2–3/12 govern only a new owned immutable
paint-output handoff from an already-realized producer. They do not extend the
lifetime of RowProjection, frame-local derived data, layout borrows or a row
model, and do not reopen this closed semantic scope. Source provenance uses
existing substrate/RowId/ColumnId identities and their owner lifetime, never
indices or recycled slots. No per-row WidgetId, offscreen geometry query,
new realization policy or column/header/body/footer authority is introduced.
