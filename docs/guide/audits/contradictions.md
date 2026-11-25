---
doc-id: halcyon-audit-contradictions
class: audit
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-24
governing-specs:
  - SPEC-HAL-309
---

# Contradiction Register

Authority/implementation disagreements and record-only findings collected
while producing the guide corpus (SPEC-HAL-309 §7: contradictions are
recorded with both citations, never harmonized into prose). Base revision
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde`. "Record-only" means the fix is
outside the documentation programme's allowed writes.

## C1 — SPEC-HAL-004 frame narration vs live gated pipeline

Authority: `SPEC-HAL-004:5` (`Status: Draft`), §3.4:120-146 (every-frame
`app.handle_event` → `build_scene()` → present). Implementation: rebuild,
layout, scene, and presentation are independently gated
(`src/core/app.rs:5611-5690`, `:11193-11198`;
`runtime-and-frame-lifecycle.md` §4/§6). SPEC-HAL-030:11-24 already
records that frame timing moved to SPEC-HAL-041/042/043.
**draft-intent + superseded-in-part.**

## C2 — SPEC-HAL-018 Screen/App shapes vs live trait

Authority: `SPEC-HAL-018:102-109`, `:283-325` (Screen::build over
`ScreenContext`; `AppRoot impl App` with `build_scene()` = `unimplemented!()`;
`Status: Draft`, :6). Implementation: live `Screen` trait
(`src/core/app.rs:4785-4909`) uses `ScreenFrameContext`, adds
input/signal/update/modal hooks; `App` is a config facade (`:9060-9074`).
**draft-intent + implemented-divergent.**

## C3 — SPEC-HAL-002 §5 phase set vs live inserted stages

Authority: SPEC-HAL-002 §5 (immutable phase order). Implementation: live
sequence inserts bounded stages (signal delivery SPEC-161, navigation
projection SPEC-248, composition actions SPEC-319, deferred focus
SPEC-260) around the constitutional phases
(`src/core/app.rs:10580-11330`). SPEC-HAL-354:192-193 permits phases
proving no-op, not reordering. **binding + implemented (refinement; no
order violation observed at the cited lines).**

## C4 — SPEC-HAL-030 PresentIntent machinery absent

Authority: `SPEC-HAL-030:5` (`Design — Partially Implemented`), §5-§7
(PresentIntent, overrides). Implementation: its own §10 declares explicit
present-mode selection, PresentIntent, swapchain switching, and overrides
Not Implemented (`SPEC-HAL-030:224-230`); live renderer holds only a fixed
`default_present_mode` (`src/renderer/wgpu_renderer.rs:590`).
**narrowed-binding + absent.**

## C5 — Executable evidence not attributable to the pinned revision (RESOLVED 2026-07-24)

Original finding: `src/tests/retained_value_updates.rs` (8 named tests,
`:140-431`) and the example set were **not executed** — the working tree
carried four dirty market-detail files, so results would evidence a mixed
state (SPEC-HAL-309 §7 rule); result axis was `unknown`.

Resolution: executed 2026-07-24 against a clean scratch-worktree checkout
of `b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` (`git status --porcelain`
empty; rustc 1.97.1): `cargo test -p halcyon --lib retained_value_updates`
= **8 passed, 0 failed**; `cargo build --examples --keep-going` = 99 of
101 targets build (two named pre-existing failures). Full record and the
crate-wide census: `implementation-status.md` §3. The §9 pointers in the
runtime guide are upgraded to PASS at this revision.

## C6 — SPEC-HAL-028 status prose vs live pipeline inventory

Authority: `SPEC-HAL-028:5` (`Design – Not Implemented`), `:14-16` ("basic
inline WGSL shaders for rect and text pipelines"). Implementation: live
`WgpuRenderer` fields include shadow, gradient, color-picker SV,
radial-gauge, optional SDF-rect, and shape pipelines
(`src/renderer/wgpu_renderer.rs:592-645`) plus a pipelines module
(SPEC-R-003, `src/renderer/pipelines/mod.rs:1-9`); the modular *registry*
the SPEC designs remains absent at the cited lines.
**design-intent + (registry absent; SPEC status prose stale about the
inventory).**

## C7 — SPEC-HAL-012 runtime switching vs live mechanism

Authority: `SPEC-HAL-012:318-340` (Draft; `ThemeManager { Arc<Theme> }`,
"no widget rebuild … pure re-render"). Implementation:
`take_theme_changed()` → `theme.clone()` +
`mark_subtree_dirty(active_root)` — owned clone plus full subtree
re-layout invalidation, no ThemeManager (`src/core/app.rs:10722-10726`).
**draft-intent + implemented-divergent.**

## C8 — Legacy overview claims HarfBuzz; live engine is rustybuzz

Legacy: `docs/OVERVIEW.md:8` ("HarfBuzz shaping"). Implementation:
SPEC-HAL-065 unified shaping via rustybuzz — "the ONLY shaping engine";
native backends rasterize only (`src/core/text_shaper.rs:1-14`,
`Cargo.toml:19`). **legacy-inaccuracy; feeds the SPEC-HAL-309 §10
replacement reverification.**

## C9 — Author attribution violations in SPEC headers

The original audit found internal agent attribution in SPEC headers.
**Resolved — 2026-11-23:** public header attribution follows HAL-309's rule
that SPEC authorship belongs to JC. Document status and technical acceptance
are separate from attribution and remain unchanged.

## C10 — SPEC-HAL-012 §4 style tables vs live StyleSpec (RESOLVED to divergent 2026-07-24)

Authority: `SPEC-HAL-012:120-199` (ButtonStyle/TextFieldStyle/… tables).
Implementation, verified 2026-07-24: `styles: StyleSpec`
(`src/core/theme.rs:2013`) resolves to a four-field
focus/hover/pressed defaults holder — `focus_button`, `focus_checkbox`,
`hover`, `pressed` (`src/core/style.rs:423-431`, Default `:469`) — no
per-widget style tables exist.
**draft-intent + implemented-divergent** (was `unknown`; theming guide
§6 updated with the verified shape).

## C11 — SPEC-HAL-008/010 mixed authority within one document

Both carry a binding ScrollArea fixed-width law as the document header
(`SPEC-HAL-008:4-8`, `SPEC-HAL-010:3-7`) atop draft-era bodies. Guides
cite the header law only. **mixed authority; body = intent.**

## C12 — SPEC-HAL-031 precision intent vs coarse live path

Authority: `SPEC-HAL-031:5-7` (Draft; "precise invalidation and redraw").
Implementation: `SignalStore::set` → `pending_dirty` → loop applies
`mark_layout_dirty` (`src/model/signal_store.rs:83-111`,
`src/core/app.rs:10728-10732`) — signals invalidate layout, not precise
paint. **draft-intent + implemented-coarser.**

## C13 — Session memory vs live disk (corrected during programme)

- Memory claimed `Theme::default()` = dark (2026-05 charter flip); live:
  light (`src/core/theme.rs:2289-2293`, re-read at audit time). Memory
  corrected 2026-07-24.
- Memory claimed ListView `read_cell` per visible cell every frame with
  no cache. RESOLVED 2026-07-24 by trace: the cache exists
  (`ListViewCore.realized_cell_values`,
  `src/widgets/list_view/core.rs:201`) and `read_cell` runs only inside
  `realize_visible_cells` during `ListViewBody::layout`
  (`body.rs:391-397`, `core.rs:961-985`); paint consumes the caches
  exclusively (`body.rs:739-751`, `core.rs:987-996`) — zero provider
  reads on paint-only frames; viewport-changing scroll re-reads exactly
  the visible window. **Memory claim wrong on both counts at this
  revision; hot path documented in the tabular guide §4; session memory
  corrected 2026-07-24.**

## C14 — Stale pointer observed outside scope

The original operational instructions pointed to the absent `docs/SHOWCASE.md`.
This was recorded outside the original audit's scope. The separate legacy
`docs/WIDGET-CATALOG.md` pointer belongs to HAL-309's legacy-retirement rules;
neither historical pointer is a public guide entry point.

## C15 — SPEC-HAL-088 clip-node authority vs failing guard tests at the pin

Authority: SPEC-HAL-088 (binding-set member by verbatim status in the original
historical mechanical ledger, retained privately, §4/§6 block). Implementation
evidence: its four named clip-node tests
`core::widget_tree::tests::test_SPEC088_clip_node_{for_overflow_clip,
for_scroll_container,nesting,no_duplicate_push}` FAIL deterministically
(in-suite and focused) at clean `b3be05fc…` with
`INV-PAINT-REQUIRES-LAYOUT-EPOCH VIOLATION` panics at
`src/core/app.rs:3160` (`implementation-status.md` §3 census).
**binding + contradictory-evidence (guard machinery fires against the
SPEC's own proof tests); production-code disposition is owner-bound.**

## C16 — Constitutional absolutes vs later binding engine patterns (authority-authority family)

The 2026-07-25 constitutional consistency audit (both binding-set
halves, 195/195 non-baseline members, independent read-only auditors)
found ten authority-vs-authority findings — CC-1..CC-10, each with
both sides quoted — sharing one root cause: SPEC-HAL-002's absolute
frame/phase/mutation/sovereignty wording is narrowed by later binding
SPECs (026, 042, 081A, 082A, 024, 260A, 352, 295, 333, 354) with no
SPEC-HAL-002A amendment folding the carve-outs back into the
Constitution's text. Zero SPECs textually amend 002. Full table with
clause citations: `conformance-census.md` §5. **Record-only:
resolution requires owner-authored 002A Type-A clarifying amendments
(SPEC edits are outside documentation writes).**

## C17 — SPEC-HAL-177 deferred queue versus SPEC-HAL-174 edit authority

Recorded 2026-11-24 from the Halcyon 0.270.3-beta warning-cleanup source,
independently of the original audit's historical revision.

Authority: [HAL-177](../../specs/SPEC-HAL-177-Deferred-Intent-Queue-Semantics.md),
Design and INVARIANT-QUEUE-1 through INVARIANT-QUEUE-7, explicitly requires
a private deferred queue even without production callers. It limits queue
admission during an active edit to Edit and Resize intents, describes
execution-time refusal, and requires FIFO draining through handle_intent.
[HAL-174](../../specs/SPEC-HAL-174-TabularSubstrate-Inline-Editing-Lifecycle-and-Intent-Model.md),
INV-174-5 and sections 4, 6 and 7, requires cancellation before applying
destabilizing intents rather than silently dropping them.

Implementation: [TabularSubstrate](../../../src/widgets/tabular_view/mod.rs)
currently admits every intent in emit_intent, invokes the existing callback,
and pushes into pending_intents. The direct handle_intent route cancels
active editing before applying Sort and Focus; Tree is a no-op in the flat
substrate. process_pending_intents executes the taken queue and restores
those same intents to pending_intents for external consumption, rather than
leaving a drained queue. This creates a potential replay if that dormant
entry is activated without further changes.

**Open: authority reconciliation and implementation divergence.** No
production caller of the private entry was identified in the warning
review. This is a source-inspection finding, not evidence of an observed
production replay. Preserve the existing direct editing route and private
queue until the governing admission, execution and drain contract is
explicitly reconciled. No activation, removal, or conformance claim follows
from this record.

**Resolved (2026-11-24, Halcyon 0.270.3-beta).**
HAL-177A governs the existing queue's admission, execution and consumption.
The processor now drains one FIFO batch through the unchanged direct
handler without reinserting consumed entries. The private queue remains
dormant, and provider-authoritative ListView sorting under HAL-353 is
unchanged. Focused tests verify pure FIFO drain, admission notification,
cancel-before-sort, and absence of replay. The selected library and
edit integration tests passed (12 and 29, respectively).

## Audit-time re-verification

Five highest-risk claims were re-read from disk at audit time and
CONFIRMED verbatim: presentation gate (`src/core/app.rs:11193-11198`),
light default theme (`src/core/theme.rs:2289-2293`), invalidation triage
(`src/core/app.rs:1265-1276`), INV-REBUILD-ROOT
(`src/core/app.rs:5664-5668`), profile authorization
(`src/core/agent_server.rs:41-51`).
