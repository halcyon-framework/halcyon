# SPEC-HAL-160 — High-Throughput Interactive Text Log Substrate

**Status:** Draft (Exploratory / Future)
**Layer:** Core Architecture / Text & Data Presentation
**Severity:** Structural (Prospective)
**Enforcement:** Mandatory if implemented
**Related SPECs:** SPEC-HAL-010, SPEC-HAL-104, SPEC-HAL-118, SPEC-HAL-141, SPEC-HAL-146, SPEC-HAL-159

---

## 1. Problem Statement

Halcyon currently treats text primarily as **document-like content** (labels, paragraphs, editors) or **diagnostic output**.
There is no architectural substrate for **extreme-rate, append-heavy, interactive textual streams** that must remain:

* responsive under sustained high throughput
* fully selectable (multi-range, discontinuous)
* layout-governed
* visually stable under scrolling and filtering
* safe from unbounded memory or layout thrashing

Professional domains (trading, medical monitoring, observability, IDE-like consoles) routinely require **real-time textual logs** that are not merely “text widgets”, but **data streams with interaction semantics**.

Without a dedicated architectural model, such features devolve into:

* ad-hoc text buffers
* unbounded string growth
* renderer-level hacks
* broken selection semantics
* UI stalls during bursts

This SPEC defines a **future-facing architectural contract** for such a subsystem.

---

## 2. Scope

This SPEC governs:

* High-throughput append-only or append-dominant text streams
* Interactive selection over large, mutable text histories
* Integration with layout, scrolling, and clipping
* Memory and lifecycle constraints for unbounded data

This SPEC does **not** define:

* Typography, shaping, or glyph rasterization
* Concrete widgets or APIs
* Storage backends or persistence formats
* Logging frameworks or diagnostics tooling

---

## 3. Definitions

**Text Log Substrate**  
A structural layer responsible for managing high-volume textual data independently of rendering.

**Append-Dominant Stream**  
A data source where writes are primarily appends, possibly with truncation or windowing.

**Logical Text Span**  
A stable, identity-bearing range of text independent of physical storage or layout.

**Interactive Selection**  
User-driven selection that may include:
* multiple disjoint ranges
* non-contiguous spans
* semantic grouping (e.g. by log entry)

---

## 4. Core Principle

> **Text flow is data first, layout second, rendering last.**

High-throughput text must never be modeled as a single mutable string or as renderer-owned state.

---

## 5. Binding Invariants

### INV-LOG-160-1 — Data/Layout Separation

**The text log substrate MUST own textual data independently of layout and rendering.**

Layout consumes immutable snapshots or views, never the live mutation source.

---

### INV-LOG-160-2 — Append Isolation

**Append operations MUST NOT trigger full re-layout or re-shaping of existing text.**

Only newly visible or invalidated spans may participate in layout work.

---

### INV-LOG-160-3 — Stable Span Identity

**Each logical text span MUST have a stable identity independent of storage compaction or eviction.**

Selection and interaction operate on span identity, not byte offsets.

---

### INV-LOG-160-4 — Bounded Memory Model

**The substrate MUST define explicit retention and eviction rules.**

Unbounded growth without policy is architecturally illegal.

---

### INV-LOG-160-5 — Layout Participation

**All visible text produced by the log MUST be rendered via layout-owned widgets.**

No renderer-level text emission is permitted.

---

### INV-LOG-160-6 — Selection Independence

**Selection state MUST be orthogonal to storage layout and rendering order.**

Selection survives scrolling, truncation, and incremental updates.

---

## 6. Allowed but Restricted Behavior

### ALLOW-LOG-160-1 — Incremental Materialization

The system MAY materialize only visible or near-visible spans into layout primitives.

Non-visible spans remain data-only.

---

### ALLOW-LOG-160-2 — Structural Grouping

The substrate MAY expose higher-level groupings (entries, records, blocks) without forcing visual separation.

---

## 7. Prohibited Patterns (Explicit)

The following are **forbidden**:

* Representing the log as a single `String` or `Vec<String>`
* Rebuilding the entire text layout on each append
* Binding selection to byte offsets in a mutable buffer
* Renderer-driven text streaming
* Layout invalidation proportional to total log size

---

## 8. Architectural Consequences

If implemented, this SPEC implies:

* A dedicated text data model distinct from text widgets
* Snapshot or epoch-based views for layout consumption
* Selection as a first-class, data-level concept
* Clear memory and eviction semantics
* Predictable performance under burst load

---

## 9. Enforcement Strategy

If adopted:

* Code review rejects append-heavy text without substrate usage
* Renderer APIs remain inaccessible for such text
* Debug assertions MAY enforce append/layout separation
* Performance audits treat violations as structural bugs

---

## 10. Rationale

This SPEC exists to **reserve architectural ground**.

High-performance professional software repeatedly reinvents this subsystem poorly.
Halcyon should not accidentally lock itself into a model that makes such features impossible or fragile.

By defining constraints early, future implementation remains viable without retrofits.

---

## 11. Non-Goals

This SPEC does NOT:

* Mandate implementation
* Specify APIs or widgets
* Choose storage structures
* Optimize prematurely
* Replace existing text widgets

It defines **what must be true if this ever exists**.

---

## 12. Final Statement

> High-throughput text is not “just text”.

If Halcyon ever supports it, it must be treated as a **data system with UI consequences**, not a rendering trick.
