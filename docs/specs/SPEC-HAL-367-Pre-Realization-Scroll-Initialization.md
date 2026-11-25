# SPEC-HAL-367 — Pre-Realization Scroll Initialization

Author: JC
Status: Accepted / Binding
Acceptance and materialization date: 2026-10-08
Implementation status: Implemented; R4 mechanical proof and independent source review accepted on 2026-10-08. Application integration and native proof remain separate.

This bounded extension was accepted under JC's standing delegation for
architectural review. This records delegated acceptance, not a separate explicit
JC decision on the signatures.

## Pre-Flight

- Objective: settle current scroll bounds before supporting content chooses its
  first visible window during retained-coordinate initialization.
- Mandatory startup and authority entry chain completed: live AGENTS.md,
  PRE-FLIGHT.md, docs/ENGINEERING.md, PROJECT-ENTRY.md, HAL-002/002A,
  HAL-024/131A/131B/134/141/146/313/352/354/366, applicable PM failure memory
  including PM-000/003/009/017/018, and DESIGN-INTELLIGENCE were read; exact
  LayoutContext, ScrollArea, ListViewBody/model and retained-test owners inspected.
- Governing authority and status: HAL-002 Foundational, 002A Permanent;
  131A/B Binding/Locked, 134 binding closed chain, 313 Active, 354 Binding,
  366 Accepted/Binding design; this delegated R4 accepts only this extension.
- Authority change, if any: optional synchronous metrics-to-owner-settlement
  handoff within one existing child layout lifecycle; no global phase change.
- Target layer and existing state verified: master a7b5c631, uncommitted
  0.205.0-beta retained-X candidate; R3 shrink first scene fails after stale
  pre-layout Y selects an empty ListView window before final bounds exist.
- Owned mechanisms and files: widget/mod.rs shared child dispatch, ScrollArea
  final settlement, ListViewBody shared layout, existing internal
  listview_dynamic_invalidation tests, this SPEC, scoped 366/ENTRY/changelog text.
- Named invariants: INIT-METRICS-BEFORE-WINDOW, INIT-OWNER-SETTLES-ONCE,
  INIT-OPTIONAL-DEFAULT, INIT-SNAPSHOT-COHERENCE, INIT-SINGLE-LIFECYCLE,
  INIT-PRESENTED-Y, INIT-NO-REPAIR-FRAME and retained-X/Y axis/intent independence.
- Forbidden behavior: extra final layout/window pass, late realization, mapper
  clamp, realized-cell extent inference, silent fallback after settlement,
  persistent callbacks/context, broad repaint or weakened first-scene proof.
- Existing mechanisms inspected and reuse decision: current logical model
  metrics/expansion preparation, ScrollState/smoothing clamp, body sync_scroll,
  range/hydration and existing child lifecycle are reused without a new owner.
- Relevant external or dependent boundaries: content supplies logical metrics;
  ScrollArea alone settles coordinates; realizer consumes presented Y; App's
  existing post-layout invalidation/scene suppression remains authoritative.
- Test ownership and placement: existing internal
  src/tests/listview_dynamic_invalidation.rs; preserve sixteen tests, strengthen
  shrink paint/hit proof, add the two named variable/known-expansion tests in §6.
- Exact runnable verification commands and expected proof: separately attached
  Grip `verify package halcyon test listview_dynamic_invalidation check timeout 600`
  then `verify package halcyon test treeview_scroll_area_reconstruction_clamps_retained_position_on_first_layout timeout 600`;
  expected eighteen ListView passes, one TreeView pass and successful package check.
- Output or artifact completeness boundary: bounded source/proof candidate for
  independent review, unstaged/uncommitted; no application dependency update or native acceptance.
- SemVer and changelog impact: same additive retained-scroll unit at
  0.205.0-beta; extend its existing newest entry, no second bump.
- Git, history-rewrite, destructive, and external effects: no stage/commit/ref,
  push, grant, native or application effects; preserve frozen afterimages and 239/365.
- Out of scope: virtualization algorithms, unknown-height policy, anchors,
  scroll physics/numeric migration, input, product/editor and native proof.

## Halcyon Pre-Flight

- Governing SPEC and read status: complete current 002/002A/131A/131B/134/313/366,
  affected 024/141/146/352/354 and PM owners read; this R4 supplies acceptance.
- Widget policy or N/A: existing ScrollArea and optional ListViewBody layout;
  no new interaction category.
- Interaction added and policy authority: none; deterministic geometry initialization.
- Applicable FORBID rules: no retained UI references, phase leakage, unrelated
  semantic mutation, shadow scroll/window authority, paint mutation or extra realization.
- Existing framework mechanism inspected: logical metrics, expansion preparation,
  child lifecycle, final clamp, sync_scroll, retained scene/hit and suppression path.
- Reinvention risk: share the existing lifecycle/body implementation, never a
  second range, extent, cache, publisher or dispatcher.
- Demo/example compliance: no demo/native run; production retained-frame tests only.

## 1. Optional synchronous contract

The Widget hook is object-safe and defaults to `None` without invoking `settle`
or performing layout work:

```rust
fn layout_scroll_content(
    &mut self,
    cx: &mut LayoutContext,
    constraints: Constraints,
    children: &[NodeId],
    settle: &mut dyn FnMut(Size) -> f32,
) -> Option<LayoutResult>;
```

The engine-only child entry is:

```rust
pub(crate) fn layout_child_full_with_scroll_settlement(
    &mut self,
    child_id: NodeId,
    constraints: Constraints,
    settle: impl FnMut(Size) -> f32,
) -> LayoutResult;
```

**INIT-OPTIONAL-DEFAULT:** `None` means ordinary layout runs once inside the
already entered child lifecycle. `Some` means the hook completed that one layout.
**INIT-OWNER-SETTLES-ONCE:** dispatch enforces zero callback calls for `None`,
exactly one for `Some`. A second call fails before executing a second settlement.
A missing call or side-effecting decline fails a named programming invariant;
it never causes fallback/repeated layout after a settlement side effect.
**INIT-SNAPSHOT-COHERENCE:** dispatch validates finite, nonnegative Size before
settlement; the successful result returns that exact snapshot as content_size.
A contradictory result fails, rather than clamping again. Callback and snapshot
state are synchronous locals, never retained beyond the call.

## 2. One child lifecycle

**INIT-SINGLE-LIFECYCLE:** both full-child entries share one implementation of
take-widget, pre_layout, deferred wiring, current-node entry/restore, one selected
layout, widget/rect restore and size/content/constraint/dirty-layout/epoch caching.
Missing-child structural failure remains. No parent reference is passed to the
child; the closure accepts Size and returns presented Y, without tree/context access.

## 3. Metrics, settlement and realization

**INIT-METRICS-BEFORE-WINDOW:** supporting ListViewBody uses one shared layout
implementation. Existing data-source sync, factory transfer, body registration,
expansion preparation and column geometry precede the logical extent snapshot.
The snapshot comes from current models, not realized cells. Drop the core borrow,
invoke settlement once, use existing sync_scroll, then choose/hydrate the window
once. Existing Fixed/Cap/Natural child layout, editor reconciliation and later
admitted measurement refinement remain. Return the same snapshot this pass.

ScrollArea selects the handoff only for a tight pass with pending X and/or Y.
It reads both pending values before clearing either and factors its existing
final geometry/clamp/axis settlement. Supporting content triggers settlement
before its window; default content triggers it after its ordinary layout, never
twice. Current extent and existing allocation/enabled-bars/thickness determine
the initial occlusion bounds. Loose/non-restoring paths, constraints, fresh
intent, anchor/slack reconciliation and independent axis convergence remain.

**INIT-PRESENTED-Y:** settlement returns actual smooth presented Y, not target or
logical Y. Content consumes it through its existing sync path; ScrollArea remains
the sole clamp owner. A changed body paint projection dirties its retained owner
narrowly before reuse, without requesting another completing layout. X publication
remains the canonical post-layout tree publisher, never a channel write in layout.

**INIT-NO-REPAIR-FRAME:** completion and dependent child layout occur in this normal
dispatch before input/paint. Existing post-layout Layout invalidation and scene
suppression stay intact. Successful initialization leaves no new Y delta requiring
a repair frame. No second root/final layout, window pass, update or anchor resolver.

## 4. Affected clauses and dependencies

| Owner | Scoped relationship |
| --- | --- |
| HAL-002/002A, HAL-024/141/146 | Existing tree, logical coordinates, phase and geometry authority unchanged; no UI references escape. |
| HAL-131A L.2/L.4/L.6 | Pre-layout sync still runs first; its initial retained coordinate is provisional until current bounds exist. Supporting body receives final presented Y before its first window. No mapper or global ordering change. |
| HAL-131B | Existing range/realization/projection finality retained; no late realization. |
| HAL-134 §§15/21/22 | One shared engine-controlled child lifecycle and one completed final realization; optional handoff does not bypass pre_layout/wiring/epoch bookkeeping. |
| HAL-313 | Known/default/refined expansion metrics and existing preparation preserved; unknown measurements are not fabricated or retroactively realized again. |
| HAL-352 | Existing intent/anchor/slack admission and reconciliation preserved; no anchor resolution in LayoutContext. |
| HAL-354, HAL-366 | Existing narrow retained invalidation and canonical post-layout X publication; no scheduling substitute, extra update or suppressed invalid scene. |
| PM-003/009/017/018 | Real first-scene content/hit proof, one coherent geometry snapshot, existing numeric limitations and actual paint dirtiness. No numeric migration. |

Locked owners are not rewritten. This separate accepted contract carries only
the initial synchronization qualification above; ordinary scroll/range algorithms
and variable-height/expansion authority remain unchanged.

## 5. Non-goals and hard prohibitions

No mapper/RowHeights/HTS formula change, type-specific ScrollArea downcast,
predecessor metrics, realized-cell extent, persistent prepared cache, consumer
offset injection, silent fallback, hidden priming/repair frame, whole-tree repaint,
late application semantics, new scroll/input/resource service or product code.
HAL-239/365 and proposed 363/364 remain inactive and untouched.

## 6. Proof and review boundary

Preserve all sixteen R3 tests and their behavioral assertions. Strengthen the
first canonical shrink scene to prove actual successor final-row text after
cumulative Scroll/clip transforms, current body hit ownership and ordinary pointer
selection of that exact RowId. Keep XY/thumb/header/footer/overlay assertions.
Add exactly:

- `retained_x_reconstruction_initializes_current_variable_metrics_before_realization`:
  ten alternating 24/48 rows via RowHeightsChanged yield current 360-unit extent,
  first-scene clamp, visible final row and exact hit/selection ownership.
- `retained_x_reconstruction_preserves_known_expansion_metrics_before_realization`:
  mounted expansion intent with Fixed(64), then retained-X setter and normal layout
  invalidation; next canonical frame preserves full extent and expanded row/child
  identity/hit with no added final realization.

The persistent tree/root/harness remains. No private offset/link writes, manual
final publisher, fallback scene or second-frame loop. Flex loose measurement may
invoke ordinary layout; cardinality is claimed at the supporting dispatch only.
Run the two separate commands declared in Pre-Flight. Record actual outcomes
after execution; green proof does not constitute independent acceptance, native
proof, product adoption or publication.

## 7. Candidate evidence

The shared dispatcher enforces the optional/default, single-settlement and exact
content_size snapshot invariants. Supporting ListViewBody uses one layout
implementation and its existing sync_scroll after dropping the model borrow.
ScrollArea uses the factored final geometry owner once; non-supporting content
uses ordinary layout. No mapping formula, post-layout safeguard or global phase
was changed.

R4's final runtime passed package check (0 errors, 75 warnings) and the independent
TreeView test (exit 0, 1 passed, 4010 filtered out). After the sole remaining test
premise was corrected to allow existing cached child layout, the ListView owner
passed (exit 0, 18 passed, 0 failed, 3993 filtered out). No production source
changed between those proofs. Detailed causal failed receipts and commands remain
in SPEC-HAL-366 §7.

The first canonical ten-row shrink scene proves the 240-unit extent and final
occlusion clamp, visible successor final-row text and exact body hit/RowId. The
variable test proves current 360-unit extent; the mounted expansion test proves
304 units including Fixed(64), stable child/row identity and child hit ownership.
No repair frame, private offset injection or manual final publisher is used.
This remains an unstaged/uncommitted candidate at 0.205.0-beta for independent
source review; no native, client integration or release acceptance is claimed.
