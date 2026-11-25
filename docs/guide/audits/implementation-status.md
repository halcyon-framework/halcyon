---
doc-id: halcyon-audit-implementation-status
class: audit
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-309
---

# Implementation Status and Coverage Audit

State of the documentation programme at revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` (halcyon `0.189.3-beta`),
2026-07-24. Working-tree caveat active throughout: four dirty market-detail
files (`examples/showcase/tab_plot.rs`, `src/tests/market_detail.rs`,
`src/widgets/aligned_order_book.rs`, `src/widgets/mod.rs`) — cited only by
the two working-tree-provenance guides on owner order, never staged.

## 1. Corpus State

| Document | State | Notes |
|---|---|---|
| `index.md` | current | sole registry; 20 rows |
| `overview.md` | current | module map from live `src/lib.rs` |
| `getting-started.md` | current | pattern from `examples/checkbox.rs`, source-cited, not executed |
| `runtime-and-frame-lifecycle.md` | current | production frame sequence line-cited |
| `rendering-and-compositor.md` | current | |
| `text-and-typography.md` | current | rustybuzz correction vs legacy |
| `theming-and-density.md` | current | light default verified twice |
| `layout-scrolling-and-geometry.md` | current | |
| `input-events-focus-and-actions.md` | current | |
| `data-and-update-patterns.md` | current | |
| `tabular-and-hierarchical-views.md` | current | no legacy-catalog reuse |
| `agent-and-remote-control.md` | current | |
| `examples-index.md` | current | 99+2 inventory with complete per-file purposes/APIs/SPEC refs/build status (mechanical pass 2026-07-24); union proven against the live listing; fixed its own prior `time_picker` omission |
| `generated/api-quick-reference.md` | current (generated) | method + omission table recorded |
| `audits/contradictions.md` | current | C1-C14; C5/C10 resolved, C13 traced |
| Historical legacy-claim audit (private) | current at the original pin; not distributed | §10 per-claim accounting of all four legacy documents; deviating claims evidenced |
| Historical authority-conformance ledger (private) | current at the original pin; not distributed | 2026-07-25 census: 452 SPECs, 3272+362 identifiers, binding set 196, 196 classification rows; original union proofs retained privately |
| `audits/conformance-census.md` | current | census methodology, evidence map, coverage, boundary dispositions, unknowns register |
| `generated/rustdoc-inventory.md` | current | public-API census: 330 exported modules, 6187 items, per-item rustdoc presence, missing list |
| `audits/implementation-status.md` | current | this file |
| `drafts/readme.md` | current | rules only |
| `widget-catalog.md` | current | working-tree provenance: base `b3be05fc` + the four still-uncommitted market-detail files, on owner order; re-verified 2026-07-24; final clean re-pin at unit commit |
| `plot-and-market-detail.md` | current | same provenance; cites `MarketDetailLink`/`AlignedOrderBook`/showcase tab + 12-test proof set |

## 2. Verification Method and Its Limits

- Every material claim in the original corpus carries a citation produced from a
  same-session read of the cited lines via internal semantic inspection and
  source reads at the pinned revision; SPEC statuses are quoted as declared
  (SPEC-HAL-309 §7).
- 2026-07-24 re-verification to `b3be05fc…` (SPEC-HAL-309 §8, no blind
  replacement): the range `68216f25..b3be05fc` touches only
  `CHANGELOG.md`, `Cargo.toml`/`Cargo.lock`, SPEC-HAL-347, SPEC-HAL-354,
  `src/tests/listview_dynamic_invalidation.rs`, and
  `src/widgets/list_view/{body,core,list_view}.rs` (plus the corpus
  itself); citations into all other files carry over mechanically, and
  every citation into the touched files was re-read at HEAD. Shifted
  SPEC-HAL-354 status-section anchors were updated, the ListViewCore
  member count re-inspected (116), and the new INV-347-PRESENTATION-1
  behavior documented in the tabular guide with fresh citations.
- Audit-time adversarial spot-check: five highest-risk claims re-read
  fresh from disk and CONFIRMED (list in `contradictions.md`
  §Audit-time re-verification).
- INV-309-1/§9.2 compile rule, applied 2026-07-24: the corpus contains
  exactly one usage-example rust fence (`getting-started.md` §1) — made
  complete and compile-verified at this revision (dropped into
  `examples/` in the clean scratch worktree;
  `cargo build --example guide_getting_started` succeeded, zero
  errors). The 14 rust fences in `generated/api-quick-reference.md` are
  mechanical signature inventories, not usage examples: they are
  governed by INV-309-8 (recorded revision + method), not by the
  §9.2 drop-into-a-file rule, and are not claimed compilable.
- Independent adversarial second pass: performed 2026-07-24 by a
  separate read-only auditor session over the finished corpus — 48
  material claims checked across 13 corpus files (≈40 source-line
  citations opened at their anchors + 26 SPEC status lines read from
  the SPECs themselves): **44 confirmed, 4 imprecise, 0 refuted**. The
  auditor independently established that every commit above the pin is
  documentation-only (`git diff --name-only` over the range excluding
  `docs/guide/` is empty), so all source/SPEC citations resolve against
  the live tree. The four imprecisions — the unstated docs-only pin
  convention presented as "HEAD", a `core.rs:111-201` struct end-anchor
  (actual `:111-220`), a "19 rows" registry count (20 after the
  accounting file), and a merged "Stable/Complete" status rendering for
  SPEC-HAL-024 — were each fixed same-day; the pin convention is now
  stated in `index.md`.
- Prior limitation, retained for the record: the original programme was
  a single-writer self-audit (writer and auditor the same session)
  until the independent pass above.

## 3. Executable Evidence Ledger

All clean-checkout rows below share one environment: scratch
`git worktree` checkout of `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`
(`git status --porcelain` empty), rustc 1.97.1 / cargo 1.97.1, macOS,
2026-07-24. Direct `cargo` was used because Grip `verify` binds to the
active workspace, which carries the four dirty market-detail files and
therefore cannot produce clean-revision attribution.

| Evidence | Status | Reason |
|---|---|---|
| `src/tests/retained_value_updates.rs` (8 tests, names pinned in `runtime-and-frame-lifecycle.md` §9) | **executed at clean `b3be05fc…`: 8 passed, 0 failed** — `cargo test -p halcyon --lib retained_value_updates` ("8 passed; 0 failed; 1809 filtered out; finished in 0.02s"). Supersedes the earlier owner-ordered dirty-tree run (same 8/8 result on `ad632311` + four dirty files). C5 resolved. | clean-checkout attribution |
| `src/tests/listview_dynamic_invalidation.rs` (11 tests) | **executed at clean `b3be05fc…`: 9 passed, 2 failed — deterministic, in-suite and isolated** — failing: `listview_paints_fixed_header_chrome_above_hydrated_body` (`:963`) and `listview_scrollable_cells_remain_inside_content_clip_after_scroll` (`:597`), both panicking `SPEC-HAL-146 VIOLATION: AllowPaintMutationGuard requires Paint phase to be active` at `src/core/rect_authority.rs:703`. Both pre-date `b3be05fc` — that commit added only two dispatch helpers and `retained_listview_pointer_and_keyboard_selection_repaint_body_without_layout` (`:809`), which **passes**. Record-only: test fixes are outside documentation writes. | proof file for SPEC-HAL-347 §8.9 / SPEC-HAL-354 §8.13 |
| Full lib test census | **executed at clean `b3be05fc…`: 1817 tests — 1808 passed, 9 failed** (single-threaded run; a parallel run varied 8-9, see renderer_staging row detail). Deterministic failures: the two listview rows above; `core::widget_tree::tests::test_SPEC088_clip_node_{for_overflow_clip,for_scroll_container,nesting,no_duplicate_push}` (panic `INV-PAINT-REQUIRES-LAYOUT-EPOCH VIOLATION` at `src/core/app.rs:3160`; also 4 failed when run focused); `tests::market_detail::{aligned_book_repaints_when_completed_reference_publication_changes,completed_plot_frame_publishes_reference_and_price_transform_together}` (assertions at `src/tests/market_detail.rs:640`, `:585`; focused run 28 passed / 2 failed — the failing pair sits in the area the uncommitted market-detail unit is changing). `tests::renderer_staging::repeated_path_compile_with_frame_advance_bounds_in_flight_staging` fails only in full-suite context (capacity assertion at `src/tests/renderer_staging.rs:112`, differing counts per run) and passes focused (2/2) — suite-order dependent. Record-only. | crate-wide truth at the pinned revision |
| Example builds | **executed at clean `b3be05fc…`: 99 of 101 example targets build** — `cargo build --examples --keep-going`; failures: `navigation_host` (E0382, use of partially moved value `intent`) and `shape_primitives` (E0004, non-exhaustive match missing `SceneNode::RetainedFragment(_)`), both pre-existing at HEAD. Record-only. | clean-checkout attribution |
| Benchmarks / runtime captures | **none exist in corpus** | no measurement taken; all performance axes `unknown` |
| Compile health of the crate | **lib and lib-test targets compile with 0 errors at `b3be05fc…`** (52 lib warnings / 47 lib-test warnings reported by the runs above) | evidenced by the same runs |

## 4. Subsystem Coverage Map

| Subsystem | Guide | Backing authority spine | Weakest verified point |
|---|---|---|---|
| Engine assembly / frame | runtime guide | 002 §5 · 026 Final · 030 narrowed · 041/042/043 · 354 Binding | closed 2026-07-24: live tail traced `src/core/app.rs:11337-11419` (texture drain → build_multi → compile_multi → render_multi → submit_and_present → advance_frame → FrameRendered) |
| Scene→GPU | rendering guide | 005 Draft · 027 Partially Implemented · 083/084 code-tagged · 040 epoch | closed 2026-07-24: batcher per-function surface anchored (`src/renderer/batcher.rs:924-2367`) in the rendering guide §4 |
| Compositor | rendering guide §6 | 081A/B/C in-code, MWA CLOSED | closed 2026-07-24: contract internals read (`src/compositor/contract.rs:55-269`) and anchored in the rendering guide §6 |
| Text | text guide | 065 in-code · 029 Implemented · 006 Draft · 034 absent | closed 2026-07-24: GlyphRun (7 fields) + TextMetrics (4 fields) shapes verified (`src/core/text_engine.rs:20-34`, `:208-216`) |
| Theme | theming guide | 012 Draft + live Theme | closed 2026-07-24: StyleSpec shape verified (`src/core/style.rs:423-431`); C10 resolved to implemented-divergent |
| Layout/scroll | layout guide | 024 (`Status: Stable`, `Implementation: Complete` — its own `:3-4`) · 026 Final · 086 in-code | closed 2026-07-24: full ScrollContainer struct read (`src/core/scroll_container.rs:28-120`) and recorded in the generated reference |
| Input/actions | input guide | 020 family Design/Binding · 176 · 033 Draft | closed 2026-07-24: EventRouter 76-member inventory complete; routing pipeline anchored in the input guide |
| Data | data guide | 354 Binding · 188/188A Draft-binding · 189 Binding · 031 Draft | closed 2026-07-24: live provider impl read (`EventLogDataSource`, `src/widgets/event_log/data_source.rs:43-244`) and cited in the data guide §4 |
| Tabular/HTS | tabular guide | 164 Implemented/Closed · 177/178 Closed · 259/262 Draft-amended | closed 2026-07-24: TabularSubstrate (151) and ListViewCore (116) member inventories complete in the generated reference |
| Agent | agent guide | 317 in-code · 318 Draft | closed 2026-07-24: complete AgentEventKind variant set + bounded-query constants verified (`src/core/agent.rs:29-105`) and listed in the agent guide §3 |
| Examples | examples index | file listing (complete) | closed 2026-07-24: per-file purposes/APIs/SPEC refs extracted from every file's own header (agent-assisted, spot-verified), build status attached from the clean-checkout run, union proven mechanically |

## 5. Known Unknowns (explicit)

Anything in the four dirty files. Closed 2026-07-24: per-example
purposes (mechanical pass complete, union proven); truncated member sets (all pagination
cursors followed to exhaustion), StyleSpec shape (C10 → divergent),
ListView cell-read hot path (C13 → traced, tabular guide §4), the
render/present tail, batcher per-function surface, compositor contract
internals, GlyphRun/TextMetrics shapes, EventRouter inventory, live
provider implementation, AgentEventKind tail.

## 6. Corrections Made to Session Memory During the Programme

Theme default (dark→light reversal observed and memory corrected);
ListView cell-cache memory flagged suspect (C13) and then proven wrong
by trace 2026-07-24 — the realized-cell cache exists and paint performs
zero `read_cell` calls (tabular guide §4); memory corrected. Recorded
here because external readers may hold the same stale beliefs from
earlier reports.

## 7. Stale-Document Detection

Mechanical rule: any registry row whose revision ≠ current HEAD is stale
(SPEC-HAL-309 §5/§8). At the 2026-07-24 re-verification HEAD is
`b3be05fc…` and every row is re-pinned to it (method in §2); the two
working-tree-provenance guides additionally depend on the four
uncommitted files and take a final clean re-pin when that unit commits.
The legacy register (`index.md`) holds the pre-contract documents;
`docs/OVERVIEW.md` + `docs/WIDGET-CATALOG.md` remain duplicated
(387/666 lines) and all four legacy files are now per-claim accounted
(the historical legacy-claim audit, retained privately); their retirement stays
an owner-ordered §10.4 unit in this historical report.
