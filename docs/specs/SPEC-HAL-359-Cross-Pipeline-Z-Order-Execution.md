# SPEC-HAL-359: Cross-Pipeline Z-Order Execution

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

## Pre-Flight

- Governing SPEC and read status: SPEC-HAL-002, SPEC-HAL-002A,
  SPEC-HAL-005, SPEC-HAL-027, SPEC-HAL-040, SPEC-HAL-064, SPEC-HAL-066,
  SPEC-HAL-081A, SPEC-HAL-084, SPEC-HAL-085A, SPEC-HAL-150,
  SPEC-HAL-163, SPEC-HAL-263, SPEC-HAL-263A, SPEC-HAL-278, SPEC-PM-006,
  SPEC-PM-007, SPEC-PM-011, and SPEC-R-021 read before writing this authority: YES
- Widget policy: N/A — renderer-owned ordering of already-admitted paint data
- Interaction added and policy authority: NO
- Applicable FORBID rules: FORBID-359-PIPELINE-ORDER,
  FORBID-359-EQUAL-Z-SEMANTICS, FORBID-359-TEXT-SCOPE,
  FORBID-359-EXECUTE-REBUILD, and FORBID-359-CROSS-SURFACE-SORT
- Existing framework mechanism inspected: `DrawBatchKey`, `PathSortKey`,
  per-surface `DrawPlan`, `SurfaceCompiledData`, RenderGraph main/overlay passes,
  text-quad lowering, path command lowering, and image registration validation
- Reinvention risk: adds no scene graph, retained ordering graph, pipeline merger,
  widget exception, application-specific path, or compositor policy
- Demo/example compliance: N/A

**Author:** JC
**Status:** Adopted / Binding
**Date:** 2026-09-13
**Layer:** 2D SceneGraph compile/execute ordering
**Owner:** Halcyon renderer
**Depends on:** SPEC-HAL-002, SPEC-HAL-027, SPEC-HAL-081A,
SPEC-HAL-084, SPEC-HAL-150, SPEC-HAL-163, SPEC-PM-006, SPEC-PM-007,
SPEC-PM-011
**Amends:** SPEC-HAL-027 Rule 3, SPEC-HAL-084 INV-RG-010 execution
clarification, SPEC-HAL-278 INV-278-A4, SPEC-PM-006, and SPEC-PM-011
**Implementation status:** Implemented with owner-local behavioral GPU proof

---

## 1. Problem and Authority Decision

Halcyon admits a `ZIndex` on ordinary scene primitives, but the former Execute
path submitted all non-text batches, then all paths, then all text. Distinct Z
values therefore stopped being authoritative when two overlapping primitives
used different pipelines. `LAYER_OVERLAY` also routed only ordinary batches;
text remained in the main pass regardless of its admitted Z value.

The binding correction is:

```text
surface-scoped immutable DrawPlan
    -> pipeline-specific Compile lowering
    -> one immutable SurfaceDrawCommand schedule
       ordered by admitted ZIndex
    -> main or overlay scope selected by each command's ZIndex
    -> Execute follows the schedule without rebuilding or reinterpreting it
```

The schedule is renderer execution data. It does not change SceneGraph,
compositor, layout, widget, or consumer authority.

---

## 2. Admitted Ordering Semantics

### 2.1 Strictly different Z values

Within one compositor base surface, every visible ordinary 2D draw item is
ordered first by ascending `ZIndex`, across all admitted pipeline families:

- rectangle and dedicated analytic batches;
- registered or target-bound images;
- tessellated paths; and
- text runs, including their text-native background spans.

A strictly larger `ZIndex` executes later and therefore composes above an
overlapping item with a smaller `ZIndex` under the item's admitted blend mode.
This rule is independent of pipeline identity and clip hash.

**INV-359-STRICT-Z:** If `a.z_index < b.z_index` on the same surface, no
pipeline-family dispatch grouping may execute `b` before `a`.

### 2.2 Equal Z values

Equal Z values do not express semantic front/back order across different batch
keys, pipelines, clip states, images, paths, or text runs. Execute remains
deterministic by retaining the existing family-local keys and this broad tie
order:

```text
ordinary batch -> path -> text
```

That tie order is an implementation determinism rule, not producer-facing
layering authority. A producer that requires one overlapping item above
another MUST assign a strictly larger Z value as required by SPEC-PM-007.

**INV-359-EQUAL-Z:** Insertion order, clip hash, pipeline enum value, texture
identity, and the deterministic family tie order MUST NOT be treated as visual
layering semantics.

### 2.3 Surface and compositor boundary

The schedule is complete only for one `SurfaceId`. Surface-to-surface order
remains the compositor's immutable `SurfaceStackSnapshot` authority under
SPEC-HAL-081A and SPEC-HAL-150. Halcyon MUST NOT merge draw commands across
surfaces into one visual Z space.

**363/364 scoped amendment:** An admitted SPEC-HAL-364 closed composition scope MAY contain an
explicit finite order of source occurrences. Within each occurrence, authored
source order—including strict primitive Z and the contiguous text background/
glyph run—SHALL be preserved. Across occurrences, explicitly admitted occurrence
order governs only inside that scope. It is normative scoped order, not
inference from equal Z, insertion order, clip hash or pipeline family.

The host paint owner SHALL allocate the scope's external placement in the
existing per-surface order. Admission SHALL prove closure: no unowned external
contribution requires interleaving with its contents, and no main/overlay/modal/
surface boundary is crossed. Without that proof the scope is refused. Isolation
into an intermediate target SHALL NOT conceal an external order change.

Outside the scope, strictly different Z ordering and SurfaceStackSnapshot
authority remain unchanged. A scope neither merges surfaces nor selects global Z
nor grants the compositor product layering policy.

---

## 3. Main and Overlay Scope

`LAYER_OVERLAY` is a command-scope threshold:

- `z_index < LAYER_OVERLAY` executes in the main pass; and
- `z_index >= LAYER_OVERLAY` executes in the overlay pass.

This applies equally to batches, paths, and text. Text is not main-only, and
paths are not appended after all ordinary batches.

The renderer-owned background prepass may prepare a derived text resource. It
does not publish visible presentation, define Z order, or authorize reordering
the main/overlay command schedule.

**INV-359-SCOPE:** Every scheduled item is visible in exactly one of the main
or overlay scopes selected from its own admitted Z value.

### 2.4 Text producer admission

Layout-owned visible text enters the same producer-owned Z space as every other
ordinary primitive. Every `TextEngine` drawing entry point MUST require an
explicit `ZIndex` and MUST forward that exact value to `PaintSink`; a text
engine MUST NOT substitute a hidden zero, content layer, or backend default.

The widget or other admitted layout owner chooses that value from its local
paint order. Text that must remain visible over an overlapping fill, image,
path, row-state tint, or other foreground class MUST use a strictly larger Z
value. The text-native background spans and glyphs retain one shared Z value
and their internal background-before-glyph ordering.

This is a producer-admission correction, not a renderer exception. It does not
authorize scene-level UI text, infer a layer from call order, or restore the
former fixed text-last dispatch sequence.

**INV-359-EXPLICIT-TEXT-Z:** No visible `TextEngine::draw*` operation admits a
text run without a producer-supplied `ZIndex`.

**363/364 scoped amendment:** The §2.3 occurrence-order scope cannot cross the main/overlay
threshold or move text into another semantic surface. Internal text-run order
under §4 and Compile/Execute ownership under §5 remain unchanged.

**HAL375 scoped adoption, 2026-11-07.** Only the [first passive PBR HUD](SPEC-HAL-375-3D-Anchored-Spatial-HUD-and-Information-Connectors.md#9-adopted-first-passive-pbr-hud-profile--2026-11-07-s1) may add its fully admitted optional group to the same surface's immutable draw schedule. Its exact current viewport depth result gates the whole leader/label pair. Order the viewport Image, leaders and text by strict local Z, keeping text backgrounds contiguous and inherited clips and external order intact. Unavailable or unadmitted candidates contribute no draw commands. This does not grant another surface, an out-of-order pass or a general GPU-driven ordering API.

---

### 2.5 Closed ordinary-control-subtree order — Accepted / Binding

**Decision:** This bounded contract was accepted on 2026-10-08 under
JC's standing delegation.
**Implementation:** Implemented in 0.206.0-beta; independent source review accepted on 2026-10-08. Routed activation and headless GPU proof are complete; native and application integration remain separate.
This subsection is independent of the Proposed 363/364 occurrence amendments.

`core::widget_tree` exposes `ControlSubtreePlacement::Main { z_index: ZIndex }`,
`WidgetTree::set_control_subtree_order(&mut self, host: NodeId,
back_to_front: &[NodeId], placement: ControlSubtreePlacement)
-> Result<(), ControlSubtreeOrderError>` and
`clear_control_subtree_order(&mut self, host: NodeId)
-> Result<(), ControlSubtreeOrderError>`. Construction and ordinary deferred UI
mutation are legal; routing, layout and scene traversal are not admission phases.

Admission validates the live host, unique complete current direct-child set and
main-range placement before mutation. The five rejection meanings are
`MissingHost`, `DuplicateMember`, `NotDirectChild` (including a missing member),
`IncompleteMemberSet`, and `InvalidMainPlacement`. Empty membership requires a
childless host. Rejection preserves the previous declaration. An identical valid
declaration and clearing a live undeclared host are no-ops. Owned member IDs are
qualified by current mount lifetime; removal retires that membership. Addition
or reparenting requires a new complete declaration before scene publication or
routing; stale membership is a named programming-invariant failure.

The scope orders the MAIN paint of the listed direct-child subtrees, not the
host's paint. The host paint owner explicitly chooses its external main anchor
and closes the interval against surrounding contributions. Strictly lower
external placement precedes the whole interval; strictly higher follows it.
Equal external placement grants no interleaving or front/back guarantee.

Inside the scope, explicit back-to-front member order precedes every member's
unchanged local Z. Inside a member, existing strict local-Z and non-semantic
family ties remain. Nested scopes retain their enclosing member prefix and
evaluate their anchor at that local level; they cannot escape above a later
outer member. Scope identity distinguishes closed batching intervals but grants
no semantic precedence at equal placement. Full hierarchy and interval identity
participate in batch/key comparison, equality, hashing and partitioning. No
scalar rank packing, local-Z rewrite, insertion/hash/pipeline/texture/identity
ranking, or per-member render target is permitted.

Main/overlay classification uses each primitive's ORIGINAL admitted Z. Overlay
primitives and sidecars retain their ordinary global owner and order without a
main-scope prefix. Canonical inherited clip/scroll structure and surface remain
unchanged. An explicitly ordered noninteractive decorative member may remain
workspace-clipped below unrelated menus; this grants no live-gesture ownership
or automatic elevation of an earlier member's `paint_overlay`.

The engine wraps immutable retained child Arcs with optional composition
metadata. Reorder/placement invalidates only host/ancestor composition with
render demand, not child-local paint, layout, keys, Image generations or
focus/capture. Fresh and retained traversal must produce equivalent effective
order with unchanged child Arcs. Public record-shape additions must be disclosed;
old struct-literal source compatibility is not implied.

Canonical front-to-back hit traversal and accelerated selection use the SAME
explicit members. Ordinary node Z retains its existing role outside declarations.
Reorder immediately invalidates the old grid without layout; canonical traversal
remains valid until an ordinary rebuild. Grid coverage includes EVERY intersected
half-open cell of the candidate's current presented/clipped viewport intersection,
including visible rectangles with off-viewport origins. Final acceptance and
local coordinates reuse canonical ancestor clip/scroll containment under
SPEC-HAL-024. Empty buckets prove absence only in a fresh conservative index.

Compile receives batch, path and contiguous text-run streams nondecreasing under
the SAME effective comparator and merges them into the immutable schedule.
Execute neither sorts nor reads tree state. All ordinary scalar-Z callers and
same-member strict-Z laws remain unchanged; PM-007 requires STRICT effective
order for every required overlap, never an equal-key exemption.

Required proof uses mounted real Button/Label/focus output, unchanged retained
child Arcs, public atomic/no-op/clear/reorder admission, nested scopes and strict
external placement; an actual >128-node grid with large/scrolled/clipped controls
and canonical winner/local-coordinate agreement before/after rebuild; and real
Compositor -> DrawPlan -> Compile -> encode/readback execution covering earlier
text/focus while preserving later local content, path/text-run integrity,
workspace clips, overlays and exact Image generation dependencies. Adapter
absence is unsatisfied visibility proof, not a passing execution witness.

#### 2.5.1 Current workspace main affordances — Accepted / Binding design

**Decision:** The independently reviewed contract and its correction were
adopted on 2026-10-08 under JC's standing delegation.
**Implementation:** Implemented in 0.207.0-beta; independent source review accepted
on 2026-10-08. Executor package check, two preview tests and four control-order
tests passed, including required-adapter GPU readback. Native and product
integration remain separate. This addition does not adopt 236/363/364 or the
separately proposed geometry-retirement contract.

`WidgetTree::set_control_subtree_main_affordances(&mut self, host: NodeId,
back_to_front: &[NodeId]) -> Result<(), ControlSubtreeAffordanceError>` and
`clear_control_subtree_main_affordances(&mut self, host: NodeId)
-> Result<(), ControlSubtreeAffordanceError>` admit the current post-child
`paint_overlay` output of explicitly ordered proper descendants. The host must
have the complete current ordinary member declaration at
`ControlSubtreePlacement::Main { z_index: LAYER_CONTENT }`. Construction and
deferred mutation are the only admission phases. Sources may be inactive and
retain sole ownership of all gesture and preview state.

The five structural rejection meanings are `MissingHost`, `DuplicateSource`,
`NotDescendant` (including a missing source or the host itself),
`SourceAlreadyAssigned`, and `InvalidContentPlacement`. Admission is atomic,
mount-qualified and stores only owned IDs. One source has one destination
ancestor. Identical valid sets and empty clears are no-ops; an empty set clears.
A nonempty declaration prevents incompatible ordinary placement or ordinary
clear through `ControlSubtreeOrderError::InvalidMainPlacement`. Removal retires
exact source/host mounts; current membership is checked before composition/cache
reuse. Recycled NodeIds cannot inherit a declaration.

**INV-359-MAIN-AFFORDANCE-OUTPUT:** The composition owner, not the public tree
setter, validates the complete CURRENT overlay output. Its immutable overlay Arc
is paired with private overlay-origin eligibility evidence captured before
overlay hit/tooltip sidecars merge with ordinary local sidecars. An exported
overlay must contain only decorative main output: no overlay-origin input or
tooltip sidecars, global-overlay/surface escape, or fabricated scene key. Newly
painted and cached outputs follow the same named programming invariant. An
ineligible export cannot be split, filtered, repainted to discover eligibility,
or silently restored in place. Clear restores ordinary output without imposing
export eligibility on an unexported menu/tooltip. Source Image registration and
generation observations remain unchanged.

**INV-359-MAIN-AFFORDANCE-ONCE:** Capture `paint_overlay` once at the source and
emit its complete current output once. Divert it from its ordinary append into
the declared host's closed engine-owned `LAYER_MAIN_AFFORDANCE` interval (6):
ordinary member content (2) precedes the interval; enclosing scrollbar chrome
(7) follows. Original primitive Z still controls main/overlay classification and
is never rewritten. Existing `ControlSubtreeMember` wrappers express host mount
and source ordinal without new renderer/scene fields or caller rank arithmetic.
Enclosing member prefixes above the host remain; only intervening prefixes below
the host are replaced for this decorative channel.

The export preserves, in order, every viewport/widget/rounded Clip and Scroll
that enclosed the original append, and all overlay-local scopes. Target and
outer scopes apply exactly once. The source's post-child overlay is outside its
own child-only scopes; export must not introduce them. Primitive coordinates,
keys and resource generations stay unchanged. Intermediate clipping still
clips. No unconsumed export may leave the root. Fresh and retained paths share
these rules; no semantic callback or second source paint occurs at the host.

**INV-359-MAIN-AFFORDANCE-DEPENDENCY:** Private composed entries retain only
bounded current unconsumed exports. Source repaint replaces output and evidence
and propagates normal ancestor composition dirtiness. Declaration set/clear/
reassignment dirties only the actual source-to-old/new-host composition paths
and host ancestors, with render demand. It does not dirty local paint/layout,
reobserve Images, invalidate the hit grid, change real member order or affect
focus/capture. A cleared/cancelled outline is absent from the first affected
admitted scene without another Move, repair frame or cache reset.

Proof uses real mounted controls and a conforming sole-owner Absolute handle.
Actual Button Down/Up activation is checked before and after legal gesture
termination. During handle capture, underlying canonical/accelerated hit queries
retain their ordinary winner while captured delivery remains with the handle.
Retained proof covers cached declaration changes, no-op/clear/reassignment,
same-Arc exactly-once output, provenance, Image/key identity, mount retirement,
nested closure and clip/scroll scopes. Required-adapter Compile/encode/readback
must show the lower handle's live outline above later opaque Button fill/text/
focus, below enclosing chrome/menu, workspace-clipped, and absent in the first
cancelled scene. No native/product acceptance is implied.

## 4. Text Run Integrity

One admitted text primitive lowers to one contiguous text run. Its zero or more
`TextBackgroundSpan` quads execute before its glyph quads inside that run. The
run as a whole occupies the text primitive's Z value in the cross-pipeline
schedule.

This preserves text-native selection/caret composition without making
`TextBackgroundSpan` a workaround for unrelated popup or shape ordering.
Empty text runs emit no visible command.

---

## 5. Compile and Execute Lifecycle

Compile owns the schedule because only Compile knows the final executable
batch set and the frame-global text/path buffer ranges.

For each surface, Compile MUST:

1. lower pipeline-specific GPU data;
2. complete same-frame resource and pipeline revalidation;
3. remove any rejected non-executable batch under its owning failure contract;
4. construct commands referencing only retained batches, non-empty path draws,
   and successfully emitted text quads; and
5. sort the commands by the rules in §2.

Path offsets count executable non-empty path draws, not source entries that
lowered to no GPU command. Text offsets count emitted quads.

Execute consumes the immutable schedule and pre-created resources only. It
MUST NOT read SceneGraph, inspect widget state, rebuild ordering, create GPU
resources, perform buffer writes, or consult compositor mutable state.

**INV-359-FINAL-PLAN:** Every command index and range refers to the final
executable per-surface data after Compile validation.

**363/364 scoped amendment:** Step 3 retains ordinary batch rejection under its existing owner.
A required SPEC-HAL-363/364 member cannot be removed independently; a known
failure invalidates the affected complete surface plan under SPEC-HAL-364
before schedule execution. Unforeseen later failure follows B-9.18. Compile
remains sole owner of the final immutable schedule; Execute cannot rebuild it.

---

## 6. Invariants and Prohibitions

- **INV-359-OWNER:** Halcyon renderer owns mechanical cross-pipeline execution;
  producers own intentional Z allocation.
- **INV-359-DETERMINISM:** Identical admitted per-surface input produces the
  same command schedule.
- **INV-359-TEXT-RUN:** Text backgrounds and glyphs remain one ordered run.
- **INV-359-EXPLICIT-TEXT-Z:** `TextEngine` forwards the producer's exact
  required `ZIndex`; no backend chooses an implicit text layer.
- **INV-359-PHASE:** The schedule is immutable Compile output consumed by
  RenderGraph Execute.
- **FORBID-359-PIPELINE-ORDER:** A fixed pipeline-family dispatch sequence MUST
  NOT override strictly different Z values.
- **FORBID-359-EQUAL-Z-SEMANTICS:** Halcyon and consumers MUST NOT infer visual
  order from equal-Z tie mechanics.
- **FORBID-359-TEXT-SCOPE:** Text MUST NOT be forced into the main pass when its
  admitted Z belongs to overlay scope.
- **FORBID-359-EXECUTE-REBUILD:** Execute MUST NOT reconstruct or mutate the
  schedule.
- **FORBID-359-CROSS-SURFACE-SORT:** The renderer MUST NOT flatten compositor
  surfaces into a shared primitive Z ordering.

---

## 7. Required Proof

Acceptance requires actual compile/encode/GPU-readback behavior proving:

1. text below an opaque rectangle is fully covered;
2. text above that rectangle remains visibly present;
3. the same two directions hold in overlay scope;
4. rectangle/rectangle strict-Z behavior remains unchanged;
5. image below rectangle and image above rectangle both follow strict Z;
6. text and image registration survive same-compile pipeline revalidation;
7. path offsets and scheduled indices refer only to executable path commands;
   and
8. one package check plus renderer-owner diagnostics are green.

Producer-boundary acceptance additionally requires:

9. every in-repository `TextEngine` implementation forwards an arbitrary
   caller-supplied Z value unchanged;
10. representative composite paint graphs prove text strictly above their
    overlapping owner-local backgrounds or fills, including Property Editor,
    Order Book, and Console; and
11. native inspection of representative integrations covers the affected
    producer compositions without reverting strict cross-pipeline execution.

Proof MUST exercise the real DrawPlan -> Compile -> RenderGraph Execute path.
Source-string guards and consumer-side rearrangement are insufficient.

---

## 8. Deliberately Excluded Authority

This SPEC does not add:

- semantic ordering for equal Z values;
- insertion-order layering across batch or clip boundaries;
- cross-surface or cross-window primitive sorting;
- a retained scene/ordering graph or arbitrary render-pass graph;
- a pipeline merger or producer-selected GPU pipeline;
- a new blend mode, transparency model, LCD-text algorithm, or glyph layout;
- depth ordering between 2D primitives and contents of an offscreen 3D image;
- automatic Z allocation for widgets; or
- any application-specific rendering rule.

**363/364 scoped amendment:** SPEC-HAL-364 supplies only the explicit closed occurrence scope
in §2.3, not general producer control of a RenderGraph. The exclusions above
remain unchanged for ordinary callers and outside that scope.

## Adopted Frosted Glass prefix barrier (HAL-385 section8, 2026-11-13)

A FrostedGlass marker is an explicit same-SurfaceId operation in the immutable effective paint order. It consumes the complete current prefix and excludes its own result and all later commands; later markers include earlier valid replacements. The marker requires exclusive effective order in its own Main/Overlay scope. Ambiguous equal order is refused, never inferred from pipeline identity or insertion order.

SurfaceDrawCommand::FrostedGlass and the bounded range-aware encoder preserve every existing batch/path/text index and keep coalescing inside the selected range. A marker cannot be crossed by batching or text/path grouping. Main stays before Overlay, closed control-subtree order remains intact, and SurfaceStackSnapshot still alone governs distinct surfaces. The declared HAL-385 graph segments do not choose a new global Z or make the swapchain a source. No-frost schedules and their equal-Z determinism remain unchanged.
## 2026-11-18 scoped material-Bloom command-order adoption: HAL-395 [S1 Binding]

The first accepted Bloom source is an optically selected attribute of the ALREADY admitted AnalyticSurface Batch or FrostedGlass marker in the final immutable per-SurfaceId command schedule. Its complete 4-pass graph segment is attached at the existing original source command AFTER that material's actual draw/replacement and BEFORE every strictly higher-Z normal command. It is not a new producer Z, synthetic overlapping widget or semantic draw order. Existing strict different-Z and Main/Overlay scope remain exact. Where an overlapping unrelated command has equal Z and no provable disjointness, HAL-395 requires an explicit BloomBuildRefusal::AmbiguousOrder, not reliance on insertion or pipeline ties. Compile must prove the original command/ordinal retained identity, and no shader or graph pass may reorder text/focus, mix SurfaceIds, or repair erroneous upstream geometry. No-Bloom command scheduling remains unchanged. Source/GPU implementation and native evidence are not claimed.

