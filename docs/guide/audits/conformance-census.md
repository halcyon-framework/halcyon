---
doc-id: halcyon-audit-conformance-census
class: audit
authority: explanatory-non-binding
halcyon-revision: b3be05fcbd6bbb11d6750520991a1bacd3c1ccde
verified-at: 2026-07-25
governing-specs:
  - SPEC-HAL-002
  - SPEC-HAL-309
---

# Conformance Census — Methodology, Coverage, Unknowns, Dispositions

Audit companion to the historical mechanical authority-conformance ledger.
The original ledger is retained privately, outside the public framework corpus.
This report preserves its recorded methodology and qualified findings, not a
claim that the original private evidence can be reproduced from this snapshot.
Executed at working-tree HEAD `fa9fd4511b687a86e1738b95261a9f3955404135`
whose `src/` and `docs/specs/` are byte-identical to the pin
`b3be05fcbd6bbb11d6750520991a1bacd3c1ccde` (docs-only commits between;
range-diff proof in `implementation-status.md` §2). The four dirty
market-detail files were never read as authority, cited, or staged.

## 1. Method

- **Phase 1 (authority inventory)** is fully mechanical: ledger §§1-3
  and §6 (452 files, verbatim statuses, 3272 INV + 362 FORBID
  identifiers, binding-verb line anchors, malformed/missing findings).
  Union proofs are embedded in ledger §1 and reproduce with the exact
  commands printed there.
- **Phase 2 (binding set + classification)**: the binding set is a
  mechanical status-string rule (ledger §4) that never upgrades Draft
  authority. Implementation classification (ledger §5a, 196 rows) uses
  a fixed evidence map (§2 below): `implemented`/`partial`/`absent`/
  `contradictory` appear only where the corpus already carries
  SPEC-HAL-309 §7-verified citations at this pin or tests executed at
  this pin; identifier anchors in `src/` (ledger §5) locate enforcement
  sites but NEVER imply implementation; everything else remains
  `unknown` with its anchor profile. Nothing was inferred from text
  search, filenames, comments, examples, or SPEC prose.
- Granularity: classification rows are per binding SPEC; named
  identifiers are accounted per identifier in ledger §5/§6; unnamed
  binding verbs are accounted by exact line anchors in each §6 block.

## 2. Evidence Map (reproduction)

The ledger's `ev()` map, verbatim by SPEC token — every pointer is a
corpus citation verified at the pin or an executed-test record in
`implementation-status.md` §3:

002 runtime guide §1-§8 + C3 · 002A process/N-A · 024 layout §1 +
SPEC self-declaration `:3-4` · 026 layout/runtime §5 · 027 rendering
§4 (partial per own status) · 029 text guide · 030 C4
(narrowed-binding + absent, own §10) · 065 text §1 (rustybuzz, C8) ·
081A rendering §6 + `contract.rs:142-147`/`:211-219` · 081B runtime
step 18 (`:11366`/`:11373`) · 083 step 18 (`:11200-11238`) · 084 step
18 (INV-RG-013 `:11380`) · 086 `scroll_container.rs:28-120` · 088
contradictory (4 clip tests FAIL at pin, `app.rs:3160`) · 146 guard
implemented and firing (`rect_authority.rs:703`; two tests fail
against it) · 161 C3 stage trace · 164 tabular §3 (151 members) · 167
cell cache (`core.rs:201`, `:961-996`) · 168 column-geometry
INVARIANT-1..7 · 176 input guide · 177 tabular §6 · 178 tabular §4 ·
189 tabular §6 · 192 closed-composite via 347 · 248 C3 stage · 255-257
engines (`engines/mod.rs:1-17`) · 263 image pipeline (`:11334-11337`)
· 268 `scroll_container.rs:78` · 295 texture drain (`:11334-11337`) ·
309 the corpus itself · 310 CellPainter · 312 EventLog provider
(`data_source.rs:43-244`) · 317 agent guide (`agent_server.rs:41-73`,
events `:29-90`) · 319 C3 stage + composition examples build · 321
`agent.rs:83-89` · 337 tabular §5 · 347 tabular §4 + PASS at pin
(`listview_dynamic_invalidation.rs:809`) · 354 runtime/data guides +
8/8 PASS at pin.

Map entries whose SPEC is outside the binding set are inert (the rule
decides membership, not the map). Map keys are family-qualified
(`HAL-002`, `HAL-024`, …) — a Phase-7 fix after the bare-number key
`002` let SPEC-HAL-002's evidence bleed onto SPEC-PM-002 (the sole
cross-family numeric collision). The `HAL-268` entry now cites the
verified wiring (`set_force_animate(true)` at
`scroll_area.rs:2107/:2128` plus dead-zone logic), not only the
tagged field.

## 3. Classification Coverage (from ledger §5a)

| Bucket | Count |
|---|---|
| binding-set SPECs | 196 |
| implemented (corpus/test evidence) | 26 |
| partial (own status; verified scope) | 1 (SPEC-HAL-027) |
| narrowed-binding + absent (own §10; C4) | 1 (SPEC-HAL-030) |
| contradictory evidence (tests fail at pin) | 1 (SPEC-HAL-088) |
| unknown — production-anchored identifiers, unverified | 69 |
| unknown — no src identifier anchor | 98 |

(Counts corrected by the Phase-7 refutation pass — see §9: the
original 27th `implemented` row was SPEC-PM-002, mis-keyed onto
SPEC-HAL-002's evidence by a bare-number map collision; the map is now
family-qualified and PM-002 is `unknown`.)

Identifier-level totals (ledger §5): 2698 identifier instances across
binding SPECs — 780 production-anchored, 10 test-only, 1908
unanchored. The 166 `unknown` SPEC rows and 1908 unanchored
identifiers are the census's honest frontier: no clean conclusions
were manufactured for them.

## 4. Boundary-Case Status Dispositions (owner attention)

Mechanical rule assignments that deserve owner review — all verbatim
strings, none reinterpreted:

- `**Status:** Post-Mortem + Permanent Rule` (3 files, e.g.
  SPEC-PM-017) — excluded from the binding set by the rule (no
  include-token), yet "Permanent Rule" reads as binding intent.
  Owner disposition needed: add an include-token or restate status.
- `**Status:** Active (root cause unresolved)` — excluded via
  `unresolved`; deliberate, but the SPEC self-describes as Active.
- `**Status:** Compositor-Complete (upstream routing pending)` —
  included via `Complete`; the pending clause stays visible verbatim.
- `**Status:** Satisfied by Enforcement` — included via `Satisfied`.
- `Status: Historical. Superseded by SPEC-HAL-085A. Not binding.` —
  excluded (three exclusion tokens); self-consistent.
- 8 SPECs with **no status line at all** (ledger §3b) — mechanically
  outside the binding set; owner disposition: add status headers
  (SPEC-HAL-025, 083A, 085-Audit, 087, 193, 194, 326, 327).
- 7 malformed identifiers (ledger §3a): `INV-RT-00x` placeholder and
  six lowercase-suffix shapes (`INV-K1a`…`INV-H3a`) — identifier-
  grammar cleanup is a SPEC edit, outside this census's writes.

## 5. Constitutional Consistency (Phase 3)

Two independent read-only auditors swept the full binding set against
SPEC-HAL-002/002A (baselines read whole): 110 SPECs (001-199 half) +
85 SPECs (200+/R/PM half) = all 195 non-baseline members covered —
full/bounded reads plus targeted risk-keyword scans; integrity
statements recorded in the session transcripts. **Ten findings, both
sides quoted verbatim in the auditors' records; none harmonized.**
Zero binding SPECs textually amend SPEC-HAL-002; every `Amends:`
targets a non-002 SPEC (legitimate). The shared root cause: the
Constitution's absolute wording (exactly-six-phase frame model,
Phase-2-only mutation, no async widget-state mutation, exclusive
frame-loop sovereignty, §1 supremacy) is narrowed by later binding
engine patterns with no 002A Type-A clarifying amendment folding the
carve-outs back into 002 — exactly the "silent conflicting rules"
002A §2.2 guards against.

| # | SPEC vs constitutional clause | Character |
|---|---|---|
| CC-1 | SPEC-HAL-026 §7/§8 (`node.parent` dirty-marking pseudocode, child→parent propagation during layout) vs 002 §3.1 (no parent pointers), §3.2/§4.7 (no mutation during layout) | scope-ambiguity with direct-conflict aspect |
| CC-2 | SPEC-HAL-042 §3.2 ("authoritative frame loop": events→fusion→routing→animation→layout→scene→GPU; no Phase 2 Deferred Mutations, no Phase 3 Rebuild) vs 002 §5 "exactly this order" | scope-ambiguity (two rival authoritative orderings) |
| CC-3 | SPEC-HAL-081A INV-MW-04 ("Window focus is managed by platform, not compositor") vs 002 INV-SOV-SEMANTIC-OWNERSHIP (Focus = Halcyon-exclusive; "No 'let OS handle focus' paths") | direct-conflict (reconcilable only as an unstated advisory-signal reading) |
| CC-4 | SPEC-HAL-082A self-labels "constitutional amendment" while amending 082 and skipping 002A §4 Step-1 declaration/type/matrix | amendment-needed (terminological/process) |
| CC-5 | SPEC-HAL-024:32-34 ("override *all* other documentation… absolute truth") vs 002 §1 supremacy ("this document wins") | latent scope-ambiguity (content itself consistent) |
| CC-6 | SPEC-HAL-260A INV-PE-TREE-DEFERRED + §14.2 (editor child insert/remove **during Body's layout pass**) vs 002 §3.8 (structural ops only between frames) / §3.2 | superseded-tension / amendment-needed |
| CC-7 | SPEC-HAL-352 INV-352-PIPELINE-1/-3 (post-layout, pre-paint engine phase mutating ScrollArea scroll state) vs 002 §5 fixed phases / §4.2 Phase-2-only mutation | superseded-tension (engine-owned state, dedicated phase — mitigating) |
| CC-8 | SPEC-HAL-295 (widget-owned offscreen GPU loop "decoupled… 120fps", optional dedicated thread; INV-295-2) vs 002 §3.3 (no thread mutates Widget state) — hinges on whether a widget's GPU cache is constitutional "widget state" | scope-ambiguity (295 §11 claims Pillar B preserved) |
| CC-9 | SPEC-HAL-333 INV-C10 (native macOS menu tracking "may… stall Halcyon's frame loop"; accepted) vs 002 Pillar F (Frame Loop Semantics non-delegable; "OS MUST NOT be treated as authoritative") | superseded-tension |
| CC-10 | SPEC-HAL-354 INV-354-FRAME-3 (no-change frame performs no layout/scene/GPU/present) vs 002 §5 "exactly this order" — 354:192 self-reconciles ("permits a phase to prove that no work is required") | scope-ambiguity, self-reconciled (weakest) |

Auditors' minor observations (hygiene, not constitutional):
`SPEC-HAL-328A` file is internally titled SPEC-HAL-329 (no 329 file
exists); `SPEC-HAL-285` filename says "Canvas-Item" for the deprecated
concept its own INV-285-A4 forbids; SPEC-HAL-328 §11.2 self-declares a
transitional violation of its own INV-KBD-MODE-3 and its §16 index
contradicts its rewritten §12.6 (internal to 328); SPEC-HAL-232 uses
"constitutional amendment" wording loosely; SPEC-HAL-268 is Active
with open questions ("draft-shaped yet binding by the mechanical
rule"). Owner dispositions in §8.

Register linkage: recorded as C16 in `contradictions.md` (this table
is the authoritative detail; both-sides verbatim quotes preserved in
the audit transcripts and reproducible from the cited lines).

## 6. Executable Evidence Map (Phase 4)

Executed evidence at the pin (clean scratch-worktree runs recorded in
`implementation-status.md` §3; not re-run here — re-running equivalent
green checks is banned) plus this census's own Grip run. Every
executed claim carries its exact command, result, and revision.

| Requirement family | Symbol(s) | Tests | Command | Result @ revision |
|---|---|---|---|---|
| SPEC-HAL-354 retained pipeline (§8.1-8.12) | invalidation split, retained fragments, cadence (`src/core/app.rs`, widget tree) | 8 named `retained_value_updates` tests (`:140-431`) | `cargo test -p halcyon --lib retained_value_updates` (clean worktree) | 8 passed / 0 failed @ `b3be05fc` |
| SPEC-HAL-347 §8.9 / 354 §8.13 selection repaint | `dispatch_intents_and_focus` → `ListViewDispatchCallbacks.presentation_changed` (`core.rs:1002-1051`) | `retained_listview_pointer_and_keyboard_selection_repaint_body_without_layout` (`:809`) | `cargo test -p halcyon --lib listview_dynamic_invalidation` (clean worktree) | PASS; file family 9 passed / 2 pre-existing failures @ `b3be05fc` |
| SPEC-HAL-088 clip nodes | widget-tree clip machinery | 4 × `test_SPEC088_clip_node_*` | `cargo test -p halcyon --lib test_SPEC088` (clean worktree) | 4 FAIL deterministic @ `b3be05fc` (C15) |
| SPEC-HAL-146 paint-phase guard | `AllowPaintMutationGuard` (`src/core/rect_authority.rs:703`) | firing evidenced by the two listview failures above | same family run | guard fires @ `b3be05fc`; no dedicated guard-test enumeration — that mapping is `unknown` |
| Whole lib census | crate | 1817 lib tests | `cargo test -p halcyon --lib -- --test-threads=1` (clean worktree) | 1808 passed / 9 failed (named) @ `b3be05fc`; `renderer_staging` failure is suite-order-dependent (passes focused 2/2) |
| market-detail family | plot/market_detail seam | 30 tests | `cargo test -p halcyon --lib tests::market_detail` (clean worktree) | 28 passed / 2 failed @ `b3be05fc` (area of the uncommitted owner unit) |
| Examples build | all example targets | n/a | `cargo build --examples --keep-going` (clean worktree) | 99/101 @ `b3be05fc` (`navigation_host` E0382, `shape_primitives` E0004) |
| Crate documentation build | rustdoc over the exported API | n/a | `verify package halcyon doc timeout 900` (Grip, live tree: pin + the four dirty files — mixed-state caveat) | first run hit the runner's 300 s cargo cap while documenting dependencies (recorded verbatim); immediate rerun with cached dependency docs: **exit 0 in 18.11 s** — `halcyon (lib doc) generated 23 warnings`, zero errors |

Missing/insufficient proof (explicit): the 166 `unknown` binding
SPECs have no requirement-mapped executed evidence beyond the blanket
lib census; identifier-level test mapping exists only for the 10
test-anchored identifiers (ledger §5). No production code was fixed
or touched during this census.

## 6a. Rustdoc Census (Phase 5)

Mechanical layer: `generated/rustdoc-inventory.md` — 330 exported
module files (pub-mod chain from `lib.rs`), 6187 exported items, 4054
with a rustdoc line, 2133 missing (every one listed there); module
`//!` docs present in 84 of 330 files. Restricted-visibility items are
excluded; container members are cross-verified in
`generated/api-quick-reference.md`.

Quality axes (independent 40-item sample, ten families, doc read
against implementation): **29 complete-and-source-faithful ·
9 present-but-incomplete · 2 misleading-or-stale ·
0 broken-link-risk.** The two stale docs, both sides read:

- `src/core/widget_tree.rs:1410` `mark_subtree_dirty` — doc says
  "(downward only)" but the body's first call is
  `mark_layout_dirty(node_id)`, which propagates dirty upward through
  ancestors. Doc contradicts code.
- `src/core/scroll_container.rs:221` `update_visual` — doc says "call
  this each frame (typically in paint)" but the body asserts
  Update-phase mutation authority (SPEC-HAL-146); calling in paint
  panics in debug. Doc predates the phase-guard.

Representative incompleteness (full list in the sample record):
`WidgetTree::add_child` omits sibling-append ordering;
`Theme::set_density` also rebuilds spacing/list_geom;
`WgpuRenderer::gpu_resources` returns a placeholder scale_factor 1.0;
`AppConfig::with_tower` and `start_agent_server_with_profile` have
undocumented debug panics; `visible_rows_for_viewport` can be
transiently empty under windowed async order.

Broken-link-risk census (from the executed doc build — complete list,
8 `rustdoc::broken_intra_doc_links` + 15 `rustdoc::invalid_html_tags`
= 23):

- Unresolved intra-doc links: `src/agent/orchestrator/id.rs:12` (×2,
  `[Task:8]`/`[Instruction:8]` bit-layout brackets),
  `src/core/editing.rs:1262`,
  `src/engines/temporal/time_geometry.rs:169` (×2, `tiers[0]`/`[1]`),
  `src/widgets/list_view/list_view.rs:529`/`:530` (`children[0]`/`[1]`),
  `src/engines/hierarchical/mod.rs:829` (`sorted_indices[index]`).
- Invalid HTML tags (unescaped generics in docs):
  `src/platform/backend_text_engine.rs:1`, `src/plot/mod.rs:483`,
  `src/renderer/vertex.rs:192,193,206,207,208`,
  `src/runtime/binding_registry.rs:206`,
  `src/widgets/radio_group_widget.rs:152`,
  `src/widgets/scroll_area.rs:296`,
  `src/widgets/tabular_view/intent_policy.rs:232`,
  `src/widgets/tabular_view/row.rs:160`,
  `src/widgets/tabular_view/sort.rs:174`,
  `src/widgets/tabular_view/mod.rs:451`,
  `src/engines/hierarchical/mod.rs:822`.

All are warning-class (docs still generate); fixes are Rust edits —
owner-bound, outside this census's writes. The same run reproduced the
52 known lib dead-code warnings verbatim.

Classification policy applied corpus-wide: sampled items carry their
judged class above; every other documented item is **unverified**;
every undocumented item is **missing** (inventory §3). No Rustdoc was
edited during this census.

## 7. Unknowns Register

- 166 binding SPECs classified `unknown` (69 anchored / 97
  unanchored) — no semantic verification exists for them in the
  corpus at this pin; they are candidates for future targeted
  verification units, not silent claims.
- 1908 binding-spec identifiers without src anchors; 10 test-only.
- Anything inside the four dirty working-tree files.

## 7a. Cross-Corpus Reconciliation (Phase 6)

- **First-class guides**: no census finding contradicts a guide claim.
  The two stale Rustdoc comments (§6a) do not appear in any guide —
  the theming guide/C7 describe `mark_subtree_dirty(active_root)` as
  full-subtree re-layout invalidation, which remains true; the stale
  part is the comment's "(downward only)" clause, not the guide.
- **`generated/api-quick-reference.md`**: granularities reconcile —
  it holds complete member inventories (including private members) for
  its named types; the Rustdoc inventory counts exported items only.
  No conflicting counts are asserted between them.
- **`audits/implementation-status.md`**: §1 table extended with the
  three census artifacts; §3 ledger unchanged (this census re-executed
  nothing it records; the one new execution — the doc build — is
  §6 here).
- **`audits/contradictions.md`**: C15 added (SPEC-HAL-088); no
  existing entry proved stale by the census.
- **Historical legacy-claim audit** (retained privately): untouched by census
  findings; its dispositions stand.
- **`examples-index.md`**: build-status rows agree with the §6
  evidence map (99/101, same two named failures).
- **Root `CHANGELOG.md`**: the 0.189.3-beta entry matches the
  SPEC-347/354 closure the corpus documents; used as historical
  evidence only.
- **Registry**: all three census artifacts registered exactly once in
  `index.md` (union check re-run clean at batch time).

## 8. Owner Dispositions Summary

1. Status-line hygiene: 8 missing status headers; boundary strings in
   §4 above.
2. Identifier-grammar hygiene: 7 malformed identifiers (ledger §3a).
3. SPEC-HAL-088: four failing clip tests at the pin (census truth;
   production-code work, outside documentation writes).
4. SPEC-HAL-146 paint-phase guard: two listview tests fail against
   the live guard (recorded in `implementation-status.md` §3).
5. Constitutional carve-outs CC-1..CC-10 (§5; C16): owner-authored
   002A Type-A clarifying amendments to §3.3/§3.8/§5/Pillar F and the
   024 precedence clause would close the family.
6. SPEC hygiene from the constitutional pass: 328A file/title drift
   (internally SPEC-HAL-329), 285 filename carries the deprecated
   "Item" concept, 328's §16 index vs its §12.6, 232's loose
   "constitutional amendment" wording.
7. Rustdoc warnings: 23 doc-build warnings (§6a) and the 2 stale doc
   comments plus 9 material omissions from the quality sample — Rust
   edits, owner-bound.

## 9. Independent Adversarial Refutation (Phase 7)

A separate read-only session attempted to refute the finished census
(~70 checks; every authority family HAL/R/PM/MC covered):

- **Every mechanical union count reproduced exactly** from source:
  452 files/blocks; 3272 INV + 362 FORBID distinct; 4062/383 citation
  pairs; 2994+278 / 345+17 single/multi owners; 2131 binding-verb
  lines; binding set 196 as an exact set match; join 2698/780/10/1908
  re-derived; 196 classification rows; rustdoc 330 modules and
  6187/4054/2133 with the missing list summing to its own total; the
  constitutional split 110 + 85 + the 002 baseline = 196; no
  truncation anywhere (per-block identifier sums equal the §1 totals);
  §6a doc-warning list = exactly 23.
- **Five adversarial challenges failed** (census upheld): 081B
  `:11366/:11373` exact; 146 guard anchor; 167 `core.rs:201`;
  test-anchor prod-classification semantics; ten Draft/upon-adoption
  evidence-map SPECs all correctly excluded — no Draft was upgraded,
  and 100 %-anchored `unknown` rows were never promoted.
- **One confirmed defect, fixed same-pass**: SPEC-PM-002 was
  classified `implemented` with SPEC-HAL-002's evidence string via the
  bare-number map-key collision. Fixed: family-qualified keys,
  PM-002 → `unknown (no src identifier anchor; 3 identifiers)`,
  coverage corrected to 26 implemented / 98 unanchored-unknown.
- **Four wording imprecisions, fixed same-pass**: the §4 rule now
  states inclusion tokens match case-sensitively (case-insensitive
  inclusion would wrongly admit all-caps `IMPLEMENTED` (151) and
  lowercase `adopted` (339) — both verified sound exclusions); the §5
  preamble now defines the displayed anchor as lexicographically-first
  under `LC_ALL=C` (possibly a test path) with prod/test
  classification by existence of a production occurrence; the
  `HAL-268` evidence entry now cites its verified wiring; the
  `app.rs:3160` panic anchor was confirmed as the runtime-reported
  panic site (string literal at `:3162`) — encompassing, kept.
- Executed-test truth: every §6 row matches `implementation-status.md`
  §3; nothing claims an execution without a ledger record; the doc
  build is the census's only new run and is declared as such.
