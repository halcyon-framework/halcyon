# SPEC-HAL-154 — Sealed After Layout Deferred Wiring Contract

**Status:** Binding
**Layer:** Core Widget Construction / Structural Integrity
**Scope:** Composite widgets that require post-mount NodeId wiring
**Depends on:** Core widget/tree APIs (NodeId), Layout pass semantics
**Constrains:** All composite widgets that accept or internally store NodeIds that may be unknown at construction time

---

## SECTION A — Problem Statement

Composite widgets sometimes cannot know all required `NodeId` references at construction time because `NodeId`s only exist after mounting into the widget tree.

If these `NodeId` references remain mutable after the first layout, the widget becomes structurally unstable:

* runtime mutations can desync layout/paint/hit-test routing
* “patch it later” wiring becomes permanent footguns
* partial wiring can silently “work” until it explodes under stress
* regressions recur because the rule is social, not mechanical

Halcyon must enforce a canonical, engine-grade construction rule:

> Structural wiring may be deferred until nodes exist, but **must become immutable before the widget participates in stable layout/paint/input**.

---

## SECTION B — Design Principles

**PRINCIPLE-1: Structural identity must become immutable**
If a widget’s behavior depends on specific child/peer `NodeId`s, those references define structural identity and must not change after initial stabilization.

**PRINCIPLE-2: Enforcement must be mechanical**
This must not rely on “developer discipline”, docs, or review. Violations must fail fast at runtime in all builds.

**PRINCIPLE-3: Unwired reads are illegal**
If wiring is required, reading an unwired reference must be a hard failure—never a default, never `NodeId(0)` fallback.

**PRINCIPLE-4: The seal boundary is the first layout pass**
The first `layout()` invocation is the canonical moment the widget transitions from “construction” to “live”. After this, structural mutation is forbidden.

---

## SECTION C — Core Contract

### C1. Deferred wiring primitive

Halcyon defines an internal reusable primitive:

`Deferred<T>`

**Required semantics:**

* Starts **unset**.
* May be set **only before seal**.
* May be read **only when set**.
* After seal, the value becomes **immutable** forever.

### C2. Seal event

Each widget that uses `Deferred<T>` MUST call:

`seal()` exactly once, automatically, on **first `layout()`** entry.

### C3. Failure mode

All violations MUST be hard failures in **release builds**, not debug-only:

* setting after seal → `panic!`
* reading before set → `panic!`
* sealing twice → `panic!` (or no-op if explicitly specified; default is panic)

---

## SECTION D — Binding Invariants (LAW)

**INV-SEAL-1 (First layout seals):**
Any widget that contains deferred structural wiring MUST seal all `Deferred<T>` fields on the first `layout()` call.

**INV-SEAL-2 (No mutation after seal):**
After sealing, any attempt to modify structural references MUST hard-fail.

**INV-SEAL-3 (No unwired reads):**
Any attempt to read an unset deferred reference MUST hard-fail. No defaults. No placeholder behavior.

**INV-SEAL-4 (Seal is internal):**
Sealing is not controlled by demos or external code. It is a widget-internal transition.

**INV-SEAL-5 (Structural = NodeId wiring):**
The following are always “structural” and must use `Deferred<T>` if deferred at all:

* child `NodeId`s that affect layout/paint/input
* scroll/content references (`ScrollArea.content`, etc.) when required for function
* header/content/footer references in composite widgets

**FORBID-SEAL-1 (Debug-only enforcement):**
Using `debug_assert!` or logging as enforcement for these invariants is forbidden.

**FORBID-SEAL-2 (Placeholder fallback):**
Using sentinel IDs like `NodeId(0)` as “okay to run” fallback after seal is forbidden.

**FORBID-SEAL-3 (Partial silent wiring):**
A widget must not enter stable operation if required wiring is missing. “It kinda works” is forbidden.

---

## SECTION E — Minimal API Shape

This SPEC does not prescribe exact file location, but the semantics are mandatory.

`Deferred<T>` MUST expose:

* `new_unset()`
* `set(value: T)` — panics if sealed or already set (unless explicitly allowed; default: single-assignment)
* `get()` — panics if unset
* `is_set()`
* `seal()` — freezes mutation; panics if called twice (default)
* `is_sealed()`

Single-assignment is the default: set once pre-seal, then immutable.

---

## SECTION F — Required Widget Integration Pattern

For any widget with deferred wiring fields:

* The widget may be constructed with unset `Deferred<NodeId>` fields.
* The owner (demo/app builder) sets them after mount when real IDs exist.
* On first `layout()`, the widget seals all deferred fields.
* All subsequent operations assume sealed and wired.

This allows expressive demo-building without permitting runtime structural mutation.

---

## SECTION G — Applicability

This SPEC applies immediately to:

* `ListView` (header/content/footer wiring)
* `ListViewContentContainer` (scroll_area wiring) if that wiring is not known at construction
* Any future composite widget that references children/peers by `NodeId` and cannot know them at `new()`

For HAL-101 section 2.4 only, engine construction/rebuild assembles the fixed TextField child role and completes its copied node relation before the first Layout seal. The host does not accept or retain the child's Widget value. Editing sessions never mutate sealed structural wiring. True composition replacement installs a new complete relation at the same lawful structural boundary; a stale registered child cannot be replaced or rebound beneath a live sealed host as if it were the old editor. This scoped clause creates no Layout-time installation permission and does not change the general Deferred invariants.

The engine completes the closed four-host assembly after host installation and before first Layout, using owned props and separate borrows; on_mount does not acquire tree authority. Root/add/true-replacement paths participate, incomplete construction cannot become live, and sealed relations cannot be lazily repaired from input or Layout. Replacing an old host retires its registered editor association before successor binding.

---

## SECTION H — Compliance Notes

If an existing widget currently uses ad-hoc setters (`set_header`, `set_scroll_area`, etc.), it MUST be migrated to `Deferred<T>` and sealing semantics, or it is considered structurally unsafe.

---

## SECTION I — Non-Goals

* This SPEC does not define selection semantics, scrolling semantics, or column behavior.
* This SPEC does not change widget tree APIs; it constrains how widgets store and mutate structural references.

---

## SECTION J — Acceptance Criteria

An implementation is compliant only if:

* All deferred structural `NodeId` references use `Deferred<NodeId>`.
* All such widgets seal on first layout automatically.
* Reading before set and writing after seal hard-fail in release.
* No placeholder fallbacks remain that allow the widget to “run” unwired.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under the supervising technical reviewer's coordinated adoption, with JC's final ownership and veto retained. The exact three source bindings and adopted scope remain in the retained adoption record. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.
