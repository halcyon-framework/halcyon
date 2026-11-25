# SPEC-HAL-366 — Sequential Horizontal Scroll Continuity

Author: JC
Status: Accepted / Binding
Acceptance date: 2026-10-07
Implementation status: Implemented; R4 mechanical proof and independent source review accepted on 2026-10-08. Application integration and native proof remain separate.

This bounded design was accepted under JC's standing delegation for architectural
review. This records delegated acceptance, not a separate explicit JC decision
on the method signatures.

## Pre-Flight

- Objective: preserve presented X across sequential ScrollArea reconstruction,
  independently of the existing retained-Y mechanism.
- Mandatory startup: Work Doctrine triplet, PROJECT-ENTRY, HAL-002/002A,
  HAL-011/024/131A/133/352/354 and PM-000/002/003/005/007/017/018.
- Authority change: this accepted additive contract; HAL-131A remains locked.
- Owners: ScrollArea, ScrollContainer axis settlement, the existing tree
  horizontal publisher and canonical App frame sequence.
- Invariants and forbidden behavior: sections below; no consumer offset mirror,
  new scroll owner, paint-time restoration, extra layout/update pass or input
  authority change.
- Verification: five retained-X tests in the existing internal
  listview_dynamic_invalidation owner, its existing retained-Y/live-X tests and
  the independent TreeView first-layout shrink-clamp test.
- Release: additive public capability, 0.205.0-beta; no product dependency change.
- Git boundary: candidate remains unstaged/uncommitted until independent review.

## Halcyon Pre-Flight

- Widget policy: existing ScrollArea; no new pointer/key interaction category.
- Interaction authority: explicit programmatic attachment under this SPEC.
- Existing mechanisms: HorizontalScrollLink, retained-Y attachment, ScrollState
  clamping, axis-local smoothing settlement, final tight layout and bounded
  linked-projection paint invalidation.
- Reinvention risk: no second scalar store/service, shared ScrollState,
  application repair, new frame loop or independent publication mechanism.
- Demo/native compliance: mechanical retained-frame proof only; no native or
  product acceptance is implied.

## 1. Public distinction and ownership

The additive ScrollArea methods are:

```rust
pub fn with_retained_horizontal_scroll_link(self, link: HorizontalScrollLink) -> Self
pub fn set_retained_horizontal_scroll_link(&mut self, link: HorizontalScrollLink)
```

**INV-RETAINED-X-OWNER:** One mounted ScrollArea writes a retained horizontal
context. Destroy its predecessor before mounting its successor. Header/footer
consumers remain live readers. The channel is a mechanical presented coordinate,
not shared ScrollState, component identity, authored product state, persistence,
item anchoring or simultaneous-scroll coordination. ScrollGroup keeps its owner.
An explicitly fresh visit uses fresh presentation handles.

**INV-RETAINED-X-ATTACH:** The new methods arm one pending X restore, clear
incompatible ordinary/layout anchors and preserve pending Y. Existing
`with_horizontal_scroll_link` and `set_horizontal_scroll_link` remain live-only;
replacing an attachment through either old method disarms pending X only. The
existing retained-Y attachments preserve pending X. Attachments do not write a
newborn zero, run layout or hide the caller's ordinary scheduling obligation.
No new public raw-offset API is admitted.

## 2. Measurement and final geometry

**INV-RETAINED-X-MEASURE:** Loose measurement and cached/no-op geometry passes
neither consume nor clamp pending continuity. Every existing X publication path,
including tree synchronization before updates, suppresses writes while X is
pending. Observation, paint and an early no-op automatic reveal are not intent.

**INV-RETAINED-X-FINAL:** The first tight layout reads both pending coordinates
before clearing either. For pending X, current child content_size and the
existing enabled-scrollbar/thickness occlusion rules establish the initial final
domain; predecessor or newborn extents cannot establish that domain. Child
constraints remain unchanged. Under SPEC-HAL-367, supporting content supplies
current logical metrics, the scroll owner settles pending coordinates, then the
content chooses its first window within the shared single child lifecycle.
Default content retains ordinary layout followed by settlement. ScrollState
clamps the retained X; horizontal-only settlement makes logical, target and
visual X equal before the first resulting input/thumb/paint projection.
Restoration is immediate, never an animation or a later repair frame.

**INV-RETAINED-XY:** Either attachment order preserves both pending axes. Each
settles only its own axis. The other axis retains its logical/target/visual
separation and normal convergence, except ordinary final-bounds clamping. Joint
restoration settles both once. Existing anchor reconciliation is unchanged.

## 3. Newer intent

**INV-RETAINED-X-INTENT:** Newer accepted programmatic offset, pointer-scroll,
scrollbar, proximity, ordinary/layout-anchor or linked intent retires pending
continuity through the existing admission branches. An explicit `scroll_to(0,0)`
reset wins even when numerically idempotent against newborn zero. Pending-flag
retirement is separate from layout-anchor/slack retirement: HAL-352 no-op slack
semantics remain intact. A layout-anchor transaction supersedes continuity only
after its existing admission. Rejected/duplicate linked input, gesture dominance
and token/group admission remain unchanged.

## 4. Publication and dependency ownership

**INV-RETAINED-X-PUBLISH:** Ordinary live X publication remains before widget
updates. The explicitly pending sequential-X restore is finalized in first tight
layout and published through the same tree helper immediately after layout and
before input projections/paint. The existing bounded linked composite consumer
set invalidates together when presented X changes. Layout and scroll_offset do
not write the channel and cannot erase the publisher's change evidence. No
second update traversal, layout invalidation as repaint, paint-time publication
or late application/control semantic mutation is authorized.

| Owner | Required relationship |
| --- | --- |
| HAL-002/002A, HAL-011/024 | Existing tree, phase and logical-coordinate ownership unchanged. |
| HAL-131A L.6 | Existing retained Y and all stable relative frame ordering preserved; this separate SPEC adds explicit X. |
| HAL-133 | Existing live/group/linked admission remains authoritative. |
| HAL-352 | Existing anchor/slack admission, reconciliation and no-op semantics preserved. |
| [HAL-367](SPEC-HAL-367-Pre-Realization-Scroll-Initialization.md) | Optional current-metrics handoff settles pending X/Y before supporting content's first window; pre-layout observation is provisional only for initialization, with no extra final layout or global phase change. |
| HAL-354 §5 | Ordinary publication precedes updates; the scoped post-layout pending-X publication dirties only existing linked paint/hit projections before reuse. No Widget update rerun or anchor resolution is introduced in layout. |
| PM-002/005/007/017/018 | Presented position, loose/final geometry separation, publication and actual retained invalidation must agree. Scheduling is not invalidation. |

## 5. Required implementation proof

The existing internal `src/tests/listview_dynamic_invalidation.rs` owner uses
the real RetainedSceneTestHarness frame path. Public attachment and scroll
operations establish coordinates; tests must not inject private link storage or
repair positions after layout. Its seven tests are:

- `retained_x_reconstruction_restores_first_xy_paint_and_projections`: mounted A
  publishes nonzero XY, is destroyed, and B survives early publisher and loose
  layout without losing continuity. B's first tight retained frame proves
  logical/target/visual XY, linked header/body/footer, SceneNode::Scroll, both
  thumbs and overlay-hit ownership. No manual final publisher substitutes for
  the canonical frame.
- `retained_x_reconstruction_clamps_to_final_occlusion_bounds`: current reduced
  content and opposing bars determine the first clamp and projection.
- `retained_x_reconstruction_fresh_intent_wins`: explicit idempotent zero and
  accepted linked intent supersede continuity; later scrolling does not revive it.
- `retained_x_reconstruction_preserves_independent_vertical_convergence`:
  X-only settlement preserves a live Y target/visual separation, and either
  attachment order preserves joint restoration.
- `retained_x_reconstruction_legacy_live_alignment_stays_publish_only`: old
  builder/setter do not restore; replacing pending X disarms only X; later live
  alignment still works without layout.
- `retained_x_reconstruction_initializes_current_variable_metrics_before_realization`:
  current alternating 24/48 row heights determine the first clamp and actual
  visible final-row paint/hit/selection, under SPEC-HAL-367.
- `retained_x_reconstruction_preserves_known_expansion_metrics_before_realization`:
  mounted Fixed(64) expansion survives retained-X attachment with coherent full
  extent and expanded child/row identity and hit ownership.

Nested retained fragments must be traversed by scene assertions. The first
resulting scene must be asserted directly, including transformed/clipped successor
end-region row text and exact pointer hit/RowId, without a repair-frame loop. Preserve
all existing ListView retained-Y/live-X and TreeView tests. Exact proof commands:

```text
verify package halcyon test listview_dynamic_invalidation check timeout 600
verify package halcyon test treeview_scroll_area_reconstruction_clamps_retained_position_on_first_layout timeout 600
```

## 6. Non-goals

No consumer offset compensation, generic persistence, new scroll handle/service,
shared ScrollState, item anchoring extension, numeric extent ceiling, f64
migration, physics/virtualization rewrite, new input class, extra update/layout
loop, application implementation, native/client acceptance, file chooser or timing work.
HAL-239/365 and proposed HAL-363/364 are not adopted by this SPEC.

## 7. Candidate evidence and review boundary

Candidate base: `a7b5c631` (accepted absolute-drag predecessor); candidate version:
`0.205.0-beta`. This is not implementation acceptance or native/product proof.

### Original P2 receipt (historical)

The prescribed attached verification block was run, then rerun only after
correcting an owned stale-domain shadowing defect exposed by its first failure.
The corrected run's package check passed (0 errors, 75 warnings). The ListView
filter executed 16 tests: 12 passed, 4 failed. New test outcomes:

| Test suffix after `retained_x_reconstruction_` | Actual result |
| --- | --- |
| `restores_first_xy_paint_and_projections` | First restored scene/projection assertions passed; subsequent unchanged-source `!idle.visual_changed` assertion failed. |
| `clamps_to_final_occlusion_bounds` | Failed because the first retained frame contained no scene. |
| `fresh_intent_wins` | Passed. |
| `preserves_independent_vertical_convergence` | Passed. |
| `legacy_live_alignment_stays_publish_only` | Passed. |

The unchanged `listview_scrollable_cells_remain_inside_content_clip_after_scroll`
and `listview_paints_fixed_header_chrome_above_hydrated_body` tests failed at
`AllowPaintMutationGuard requires Paint phase to be active`. Their bodies and
assertions were preserved; no baseline replay was run.

The block stopped at the failed ListView command, so the TreeView command did
not execute. The shrink proof retains its first-scene assertion: existing
post-layout scroll-to-content synchronization marks changed ListViewBody scroll
projection layout-dirty, and `build_scene_reusing` returns no scene when that
sync reports `layout_deferred`. No repair frame, manual final publisher, weakened
assertion or ListView production change was added. The missing first scene and
unchanged-source visual-change result require independent owner disposition.

### R3 proof-premise correction and observed results

The retained-XY fixture now keeps one WidgetTree/root and one retained harness.
Each predecessor subtree is removed through the tree lifecycle before its
successor mounts. The fixture drains removal records, checks that every retired
TreeWidgetId no longer resolves, checks fresh successor identities, and permits
exactly one current ListView/ScrollArea subtree. No cache reset or priming frame
is used.

The unchanged-content frame now proves stable widget identities, layout epoch,
logical/target/presented XY and published X. Any emitted scene must preserve its
actual Scroll/header/footer/thumb and overlay-hit projections. It does not claim
global idleness: this harness uses frame_dt zero while ScrollArea's existing
scrollbar timer/alpha can still request animation repaint. The two historical
direct-paint tests received only legal layout/paint guard scopes; their names and
behavioral assertions remain unchanged.

One exact-leaf formatting preview was inspected and the needed leaf formatting
was applied. The two prescribed verification commands then ran separately:

```text
verify package halcyon test listview_dynamic_invalidation timeout 600
verify package halcyon test treeview_scroll_area_reconstruction_clamps_retained_position_on_first_layout timeout 600
```

The ListView command failed (Cargo exit 101): 16 tests executed, 15 passed and
1 failed, with 2203 filtered out. New test outcomes:

| Test suffix after `retained_x_reconstruction_` | R3 result |
| --- | --- |
| `restores_first_xy_paint_and_projections` | Passed, including the first canonical restored scene and unchanged-content continuity checks. |
| `clamps_to_final_occlusion_bounds` | Failed at the preserved first-scene assertion: `the first restored frame must contain its scene`. |
| `fresh_intent_wins` | Passed. |
| `preserves_independent_vertical_convergence` | Passed. |
| `legacy_live_alignment_stays_publish_only` | Passed. |

Both named historical direct-paint tests passed with their existing assertions.
All eleven pre-existing tests in this module passed; no predecessor/baseline
replay was run. The independent TreeView command executed after the failed
ListView command and passed (Cargo exit 0): 1 passed, 0 failed, 4008 filtered out.
No package check or equivalent rerun was added in R3.

The first-scene shrink defect remains substantive: the new ListView can realize
an empty window from the predecessor's out-of-bounds Y before ScrollArea obtains
the current logical extent and clamps it. Post-layout sync correctly dirties the
obsolete realization and suppresses that scene. No production bytes, suppression
guard, expected clamp or first-scene assertion were changed. The initialization
ordering correction requires a separately reviewed owner boundary; R3 proposes
that boundary directly without adopting or implementing it here. The candidate
remains unaccepted, unstaged and uncommitted; applications must not adopt that candidate.

### R4 initialization correction and observed results

The delegated R4 acceptance is materialized in
[SPEC-HAL-367](SPEC-HAL-367-Pre-Realization-Scroll-Initialization.md). Its optional
shared child lifecycle lets ListViewBody report current logical metrics after
source/expansion preparation. ScrollArea settles final occlusion-aware bounds
and pending axes once, then the body consumes presented Y before its first window.
Default content retains its ordinary layout. Existing post-layout synchronization,
Layout invalidation and scene suppression are unchanged.

The persistent fixture retains successor RowIds and strengthens the first shrink
scene with actual final-row text after cumulative Scroll/clip transforms, current
body hit ownership and ordinary pointer selection of that exact RowId. Two added
tests cover alternating 24/48 heights through RowHeightsChanged (360-unit extent)
and mounted Fixed(64) expansion (304-unit extent, stable child/row identity/hit).

All verification used attached Grip invocations, sequentially. Initial test
compilation rejected the probe's wrong Button constructor; the separately invoked
TreeView command reported the same compile failure. After the constructor fix,
ListView executed 18 tests: 11 passed, 7 failed. Six failures exposed the extracted
finalizer's stale local domain used by the existing formula assertion; the seventh
was the probe's incorrect expectation that ListViewBody's Down handler returns
handled. Both were corrected without weakening geometry or RowId assertions.
That revision's independent TreeView command passed (exit 0, 1/1).

The next combined ListView/check invocation passed package check (0 errors,
75 warnings), and ran 18 tests: 17 passed, 1 failed. The first canonical shrink
scene and variable-height scene passed. Only the new expansion probe's requirement
for a current global child layout epoch failed: existing layout_child legally
returns cached layout for clean unchanged constraints. The independent TreeView
command then passed on the corrected runtime (exit 0, 1 passed, 4010 filtered out;
138 suites). The probe now asserts the preserved cached child epoch, clean layout,
unchanged 64-unit geometry/identity and actual hit ownership.

Only that test premise changed after the successful package check/TreeView proof.
One causal ListView test-only invocation then passed (exit 0): 18 passed, 0 failed,
3993 filtered out; 138 suites. All seven retained-X tests and all eleven existing
module tests passed. No green check or TreeView rerun followed this fixture-only
correction. Final proof commands were:

```text
verify package halcyon test listview_dynamic_invalidation check timeout 600
verify package halcyon test treeview_scroll_area_reconstruction_clamps_retained_position_on_first_layout timeout 600
verify package halcyon test listview_dynamic_invalidation timeout 600
```

The first two apply to the unchanged final runtime; the last verifies the corrected
fixture. This closes the required mechanical first-scene proof, not independent
implementation acceptance, application adoption, release or native/product proof. The full
0.205.0-beta candidate remains unstaged and uncommitted for review. All earlier
failed receipts above remain historical evidence.
