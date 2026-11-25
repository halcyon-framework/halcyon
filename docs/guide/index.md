---
doc-id: halcyon-guide-index
class: first-class
authority: explanatory-non-binding
halcyon-revision: 0a633294d43dbc8ccc9df9c7bda3baf1dc600357
verified-at: 2026-11-22
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-309
---

# Halcyon Documentation Index

This file is the sole canonical entry point for `docs/guide/`
(SPEC-HAL-309 §5, INV-309-7). No other manifest or table of contents may
duplicate this registry. Everything under `docs/guide/` is
explanatory-non-binding: it explains and connects sources; it never creates
architecture and never silently resolves contradictions.

## Authority Hierarchy

1. SPEC-HAL-002 (Constitution) and its amendment process are supreme.
2. A governing SPEC carries exactly its declared status. A Draft is intent,
   not implemented truth.
3. Source, tests, compiler output, and runtime evidence establish
   implemented behavior at a particular revision.
4. `docs/guide/` explains and connects those sources.
5. Rustdoc owns item-level public API documentation beside the code.
6. Examples demonstrate intended use but do not prove general behavior.
7. Tests are executable evidence only when actually run against the cited
   revision.
8. Remaining legacy material requires per-claim reverification before reuse.

## How To Read These Documents

- Every document begins with the SPEC-HAL-309 §6 metadata block. Its
  `halcyon-revision` is the exact revision the claims were verified against.
- Material claims carry two independent axes (SPEC-HAL-309 §7) — Authority:
  `binding` / `draft-intent` / `none` / `contradictory`; Implementation:
  `implemented` / `partial` / `absent` / `unknown` / `not-applicable`. The
  axes are never collapsed.
- A document whose revision is behind HEAD is `stale`: a true statement
  about its pinned revision, not retroactively false.
- Pin convention: `halcyon-revision` names the last **code-bearing**
  revision (a documentation commit cannot contain its own hash).
  Documentation-only commits above the pin do not stale source or SPEC
  claims when the range diff proves `src/` and `docs/specs/` untouched;
  that proof is recorded in `audits/implementation-status.md` §2.
- Contradictions between authority and implementation are recorded in
  `audits/contradictions.md`, never harmonized into prose.

## Reading Paths By Task

- **Find common answers:** [FAQ](faq.md) covers getting started,
  development history, architecture, participation, licensing, security
  reporting, and project status.
- **Build a first application:** [Getting started](getting-started.md) gives
  both the source-root commands and a complete separate Cargo application.
  The widget catalog and example inventory retain the historical pins below.
- **Understand why Halcyon is designed this way:** [Design philosophy and
  architectural rationale](design-rationale.md) explains its original goals,
  constitutional invariants, ownership, extension model, and trade-offs.
- **Understand the frame and update machinery:** `overview.md` →
  `runtime-and-frame-lifecycle.md` → `data-and-update-patterns.md`.
- **Rendering, text, and theming work:** `rendering-and-compositor.md` →
  `text-and-typography.md` → `theming-and-density.md`.
- **Layout, input, and interaction work:**
  `layout-scrolling-and-geometry.md` → `input-events-focus-and-actions.md`.
- **Tabular/hierarchical views (ListView, TreeView, engines):**
  `tabular-and-hierarchical-views.md` → `data-and-update-patterns.md`.
- **Plot and market-detail composition:** `plot-and-market-detail.md`.
- **Inspect the historical agent interface:** `agent-and-remote-control.md`.
- **Look up a public signature:** `generated/api-quick-reference.md`
  (complete member inventories, zero omissions).
- **Audit documentation truth:** `audits/implementation-status.md` →
  `audits/contradictions.md`. Private historical audit sources are not distributed.

The index navigation and FAQ were originally checked for Halcyon
0.270.2-beta on 2026-11-22. Getting Started retains its verified
0.268.2-beta runtime source pin and records the separate 0.270.2-beta
source build and 0.270.3-beta release qualifications dated 2026-11-24.
The later public release documentation additions do not advance unrelated
source pins.

The other rows record freshness at their own historical verification,
not a guarantee of current implementation coverage. The 2026-07-24 review
used `b3be05fc`, and the two market-detail-adjacent guides recorded four
uncommitted files in their provenance. Those historical pins require
reverification before supporting current implementation claims.

## Document Registry

| Path | Class | Freshness at verification | Verified revision | Verified at | Governing SPECs | Purpose |
|------|-------|---------------------------|-------------------|-------------|-----------------|---------|
| `index.md` | first-class | current | `0a633294d43dbc8ccc9df9c7bda3baf1dc600357` | 2026-11-22 | SPEC-HAL-002, SPEC-HAL-309 | Canonical entry point and registry |
| `overview.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-001/002/003/018/042/354 | Architecture at a glance |
| `design-rationale.md` | first-class | rationale verified against governing sources; no full implementation requalification | `51beb5c3e5a1c86b3cc7b75e1c94cb4b4c9f6c71` | 2026-11-24 | SPEC-HAL-000/001/002/002A/010/018/266A/295/295A/309/354, SPEC-PM-000/001/027 | Original goals, design rationale, Constitution, extension boundaries, engineering lessons, and trade-offs |
| `getting-started.md` | first-class | historical runtime pin; separately qualified source build and release runtime scopes | `eec0b3045b653195d3b9d43c761d2b3170fe8dc9`; newer qualification bases recorded in the guide | 2026-11-22; further qualification 2026-11-24 | SPEC-HAL-002/011/018/309/398/400 | First native and Web application with a separate Cargo root |
| `faq.md` | first-class | source pin retained; public release guidance updated | `0a633294d43dbc8ccc9df9c7bda3baf1dc600357` | 2026-11-22; public guidance 2026-11-24 | SPEC-HAL-002, SPEC-HAL-002A, SPEC-HAL-309; additional topic sources cited in FAQ | Common questions, project history, design rationale, founder and AI workflow, participation, sponsorship, licensing, security reporting, limitations and verification boundaries |
| `runtime-and-frame-lifecycle.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-001/002/018/026/030/042/043/354 | App/Screen/WidgetTree; update, rebuild, layout, scene, render, presentation boundaries |
| `rendering-and-compositor.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-002/005/027/028/030/354 | Scene-to-GPU path, compositor boundary, render graph |
| `layout-scrolling-and-geometry.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-002/008/024/026 | Constraints, finite geometry, scroll state and links |
| `input-events-focus-and-actions.md` | first-class | stale §§1–8; current §9 | §§1–8: `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`; §9: `15040b309cbcb425f970e3e440e88c08362bbdfd` | §§1–8: 2026-07-24; §9: 2026-10-21 | SPEC-HAL-002/009/020/033/112/128/151/176 | Event routing, focus/capture, actions, input fusion; separately verified native-contact continuation |
| `text-and-typography.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-002/006/029/034 | Shaping, rasterization, metrics, caches |
| `theming-and-density.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-002/012 | Theme tokens, density, runtime propagation |
| `data-and-update-patterns.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-002/007/031/188/354 | Channels, polling, update-only invalidation |
| `widget-catalog.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` +4 dirty files | 2026-07-24 | SPEC-HAL-011/309 | Public widgets in coherent families; 81-module inventory (working-tree provenance noted in doc) |
| `tabular-and-hierarchical-views.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-164/179/188/189/259/262 | TabularSubstrate, engines, ListView, TreeView |
| `plot-and-market-detail.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` +4 dirty files | 2026-07-24 | SPEC-HAL-296/303 | Plot subsystem and market-detail composition (working-tree provenance noted in doc) |
| `agent-and-remote-control.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-317/318 | Agent server, capability profiles, bounded querying, fused input |
| `examples-index.md` | first-class | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-000/309 | Example inventory and navigation; complete per-file purposes, APIs, SPEC refs, and verified build status |
| `generated/api-quick-reference.md` | generated | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-309 | Mechanical signature extract via internal rust-analyzer-backed semantic inspection; member inventory complete, zero omissions |
| `generated/authority-conformance-ledger.md` | generated | private historical record; not distributed | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-25 | SPEC-HAL-002, SPEC-HAL-309 | Original mechanical authority census retained privately; public audits describe its qualified results |
| `generated/rustdoc-inventory.md` | generated | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-25 | SPEC-HAL-309 | Mechanical public-API census: 330 exported modules, 6187 items with rustdoc-presence per item; quality sample in the census audit |
| `audits/implementation-status.md` | audit | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-309 | Coverage, evidence ledger, known unknowns |
| `audits/contradictions.md` | audit | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-309 | Contradiction register C1-C14 |
| `audits/legacy-claim-accounting.md` | audit | private historical record; not distributed | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-309 | Original per-claim accounting of the four legacy documents retained privately |
| `audits/conformance-census.md` | audit | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-25 | SPEC-HAL-002, SPEC-HAL-309 | Conformance-census methodology, evidence map, classification coverage, unknowns, owner dispositions |
| `audits/spec-hal-355-conformance.md` | audit | stale | Candidate based on `7985f1ddb945108248d17c32efb4268bc4684a4d` | 2026-08-23 | SPEC-HAL-355, SPEC-HAL-295A, SPEC-PM-007 | Historical candidate and backend conformance record |
| `audits/spec-hal-362-macos-conformance.md` | audit | stale | Not recorded in document metadata | Not recorded in document metadata | SPEC-HAL-362 | Historical macOS capture and admission record; current conformance unverified |
| `drafts/readme.md` | draft | current | `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` | 2026-07-24 | SPEC-HAL-309 | Draft-area rules (drafts are never canonical reading) |

The two historical conformance audit rows provide navigation to existing
records. HAL-355 identifies a candidate base; HAL-362 does not declare a
document verification revision or date. These entries do not establish
current implementation or backend conformance.

## Governing SPECs Of The Documentation System

- **SPEC-HAL-002** — Constitution; supreme over all SPECs and documentation.
- **SPEC-HAL-309** — Documentation System; Adopted 2026-07-24; owns this
  corpus, its classes, metadata, claim classification, and the legacy
  replacement plan.
- **SPEC-HAL-000 §11** — Documentation Standards (code comments, SPEC
  format, examples).
- **SPEC-HAL-003** — Project Structure; repository layout the guides
  describe.

## Legacy And Research Register

Pre-contract material outside `docs/guide/`. None of it is canonical
reading; reuse requires per-claim reverification (SPEC-HAL-309 §10).

| Path | Designation | State |
|------|-------------|-------|
| `docs/DESIGN-INTELLIGENCE.md` | legacy | Composition guidance; advice layer retained-unique pending owner promotion; accounted for in the private historical legacy review |

## Freshness Legend

- `planned` — registered, not yet written.
- `current` — verified revision equals the last code-bearing revision at
  last check (documentation-only commits above it do not stale source
  claims; see the pin convention in How To Read These Documents).
- `stale` — a code-bearing commit has advanced past the verified
  revision; content remains true of its pinned revision.
- `legacy` — pre-contract material awaiting reverification or retirement.
