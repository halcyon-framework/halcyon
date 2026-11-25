# SPEC-HAL-354: Retained Value Updates and Scene Fragments

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

- SPEC: SPEC-HAL-002, SPEC-HAL-015, SPEC-HAL-021, SPEC-HAL-026, SPEC-HAL-035, SPEC-HAL-042, SPEC-HAL-083, and SPEC-HAL-083A read before writing/amending this task: YES
- Policy: N/A — core frame, invalidation, scene, and renderer machinery
- Interaction added: NO
- Forbidden behaviors checked: FORBID-354-DATA-DRIVEN-FRAME, FORBID-354-PAINT-AS-LAYOUT, FORBID-354-FRESH-WORLD, FORBID-354-STALE-FRAGMENT, FORBID-354-REFERENCE-CACHE, FORBID-354-CROSS-PHASE-MUTATION, FORBID-354-UNBOUNDED-RETENTION
- Existing infrastructure: WidgetTree layout dirtiness and cached constraints, WidgetDirty/content snapshots, EngineTicker, SceneNodeKey, SceneGraph diff, SceneCache dispatch, recycled SceneBuilder storage, DrawPlanBuilder, renderer resource caches, DataSource invalidation, ListView visible-cell realization
- Reinvention risk: Extends the existing dirty-tree, scene identity, cache-dispatch, and frame-loop machinery; it does not create a second widget tree, frame scheduler, renderer, application-state owner, or scene identity authority

**Status:** Binding
**Layer:** Core frame loop / WidgetTree invalidation / retained scene composition / renderer preparation
**Owner:** Core Architecture
**Applies To:** `App`, `WidgetTree`, `EventContext`, `UpdateContext`, read-only animation sampling, scene construction, custom-render preparation, draw-plan preparation, renderer compilation, and data-backed widgets
**Extends:** SPEC-HAL-015, SPEC-HAL-026, SPEC-HAL-035, SPEC-HAL-042, SPEC-HAL-083A
**Depends on:** SPEC-HAL-002, SPEC-HAL-021, SPEC-HAL-024, SPEC-HAL-026, SPEC-HAL-042, SPEC-HAL-083, SPEC-HAL-083A, SPEC-HAL-141, SPEC-HAL-146, SPEC-HAL-163
**Non-Goals:** Changing application data authority, allowing asynchronous WidgetTree mutation, changing widget interaction policy, partial OS damage presentation, introducing a second renderer, or weakening scene identity and cache-dispatch invariants

---

**Button material contract and implementation admission.**
Contract adoption does not establish source conformance. The existing renderer
and resource owners must verify producer identity, typed request/publication and
commit transport, precise failure causes, generation protection and the actual
Button content/focus/retained integration before execution is claimed.
Unresolved implementation admission remains unresolved and requires governing-owner
resolution, not implementation-side invention. This correction selects no new source, ABI,
target, runtime, material default or consumer behavior and adopts no Proposed
SPEC-HAL-363/364 clauses.
Verification establishes conformance and evidence; it does not admit another
source, ABI, target, runtime, capability or consumer, or lift an unresolved
admission. Such extensions require a separate governing-owner decision.

## 1. Problem and Authority Decision

Halcyon owns frame timing, layout, composition, draw order, and rendering semantics. An application may publish data at any
rate, but data arrival does not own frame cadence and does not authorize immediate layout or rendering.

The current public path cannot express the common trading-UI event:

```text
value changed
geometry and structure did not change
```

Consumers therefore use layout invalidation as a repaint signal. Under sustained data, that false geometry claim
propagates to the root and repeatedly runs layout, scene construction, and renderer preparation for stable structure.

The binding authority chain is:

```text
external data
    -> synchronized application/data-source ingress
    -> Halcyon update phase at engine cadence
    -> coalesced semantic invalidation
       -> no visual change: no scene work
       -> paint-only change: retained fragment replacement
       -> geometry/structure change: layout, then affected fragment replacement
    -> immutable presented scene snapshot
    -> SceneGraph diff
    -> SceneCache dispatch
    -> retained draw-plan / renderer preparation
    -> GPU presentation when required
```

**INV-354-AUTH-1:** Data producers own data publication. Halcyon exclusively owns when a frame is admitted and which
layout, scene, and renderer work that frame performs.

**INV-354-AUTH-2:** An invalidation records changed meaning. It is not an imperative render call.

**INV-354-AUTH-3:** Repeated equivalent invalidations before one admitted frame coalesce idempotently.

---

## 2. Invalidation Domains

Halcyon has three distinct invalidation domains:

```text
Unchanged
PaintOnly
LayoutAndPaint
```

`PaintOnly` means the widget's visual projection changed while its size, child geometry, constraints, scroll extent, and
tree structure remain valid.

`LayoutAndPaint` means geometry may have changed. Layout invalidation continues to propagate upward under SPEC-HAL-026.

### 2.1 Public paint invalidation

WidgetTree exposes node-ID and widget-ID forms of paint invalidation. Event and update contexts expose the equivalent
current-widget operation. `UpdateContext::invalidate_paint_subtree` is the explicit current-widget operation for
update-owned shared presentation state consumed by descendants.

Paint invalidation:

1. marks the target widget's local paint fragment dirty;
2. marks ancestor composition fragments dirty so the replacement remains in correct order and clip/scroll scope;
3. does not set `dirty_layout`;
4. does not dirty descendants whose own visual inputs are unchanged;
5. does not request an immediate platform redraw outside the engine scheduler.

Subtree paint invalidation is reserved for changes whose visual dependency is genuinely inherited, including theme,
display scale, debug presentation, or update-owned shared presentation state consumed by descendant painters. It does
not imply layout.

### 2.2 Layout implication

Layout invalidation always implies affected paint/composition invalidation. It must never convert paint-only
invalidation into layout invalidation.

**Scoped incremental-layout qualification — Author JC; delegated architectural
decision, 2026-10-10.** Every actually executed successful layout dispatch
invalidates that owner's local paint and ancestor composition after its widget
borrow is released, including same-sized results whose internal paint inputs
changed. A clean child returned from the existing layout cache is not an
executed layout owner. Final authoritative rect and viewport producers compare
their previous and completed values and invalidate actual changed owners and
genuinely inherited consumers before retained root/fragment reuse. Translation,
inherited clip/viewport, overflow and scroll wrappers, rounded clips and
paint-owned sidecars remain dependencies. Conservative subtree invalidation is
valid only for genuinely inherited changed dependencies; unchanged skipped
siblings keep their local fragments. This does not change viewport equations,
layout skipping, the single layout phase, semantic mutation authority, or the
existing theme/DPR/transition/shared-state invalidation paths, and does not adopt
363/364. Implementation of this qualification is pending source proof and
independent review.

### 2.3 Data-backed widgets

Data invalidation is classified by effect:

- cell/value/color/quantity changes with stable geometry are paint-only;
- row order, row count, column geometry, row height, wrapping width, scroll extent, child topology, and constraints are
  layout-and-paint;
- a provider invalidation that combines both classes is layout-and-paint;
- an adapter must not upgrade a known value-only invalidation merely because paint invalidation is more convenient.
- an attached synchronized receiver is update work through `needs_update`; receiver presence alone is not animation and
  does not invalidate paint when a poll drains no visible change;
- a composite whose descendant painters consume shared value state invalidates its retained paint subtree; only the
  descendant that paints a continuing visual effect declares `needs_animation`.

**INV-354-DIRTY-1:** Paint dirtiness and layout dirtiness are separately observable engine state.

**INV-354-DIRTY-2:** A clean root under identical constraints performs no widget layout calls and no full-tree geometry
restamping.

**INV-354-DIRTY-3:** A value-only update may repaint the affected widget without calling layout on that widget, its
ancestors, or unrelated siblings.

**INV-354-DIRTY-4:** Dirty classification is cleared only after the corresponding authoritative phase completes.

---

## 3. Retained Scene Fragment Model

The retained unit is an engine-owned widget scene fragment identified by stable `TreeWidgetId` plus the widget's current
mount lifetime. It is not a stored widget reference and it does not replace widget-owned `SceneNodeKey`.

One retained widget entry contains:

1. the widget's local primitives emitted before child traversal;
2. the framework focus/debug projection owned by that widget;
3. the widget's post-child overlay primitives;
4. paint-produced sidecars such as overlay hit regions and tooltip triggers;
5. a composed subtree projection that references retained child fragments through immutable engine-owned values;
6. exact dependency generations for theme, display scale, geometry, focus/capture presentation, animation, and child
   composition.

For an actually emitted SPEC-HAL-263 §5.3 admitted Image, the existing retained
scene owner SHALL retain its CPU generation dependency and consumer mapping.
Resource revocation observations SHALL be event-driven, with wake-only delivery
through the existing application frame ingress. Before retained scene reuse or
scene-choice cutoff, affected consumers SHALL receive local paint dirtiness and
ancestor recomposition; layout and unrelated siblings remain clean. The resource
owner MUST NOT acquire WidgetTree identity or mutate its authority. Repaint
replaces dependencies; consumer/tree/window teardown removes them. These CPU
records do not pin GPU resources or certify historical content. No dependency
table, observer, per-frame registry lookup, resource polling or preparation is
allocated for unused/non-Image consumers.

The implementation may use immutable shared ownership for scene values. It must not retain `&Widget`, `&WidgetTree`,
layout borrows, paint contexts, native renderer references, or other cross-frame borrows.

**363/364 scoped amendment:** A current retained widget fragment and a SPEC-HAL-364 historical generation
are distinct lifetime classes. Current fragments remain keyed by live
TreeWidgetId plus mount lifetime. A historical generation is a bounded owned
immutable paint-value closure certified by SPEC-HAL-364; it contains no
widget/tree/layout/text-run borrows and no semantic sidecars. It does not
preserve widget membership, input, focus, capture, hit regions or tooltip
registration.

### 3.1 Local versus composed dirtiness

A child paint change dirties the child's local fragment and the composition chain to the root. It does not repaint the
parent's local primitives. A parent composition rebuild reuses clean local and child fragments.

A parent local paint change does not repaint clean children.

Under the independently accepted SPEC-HAL-359 §2.5 scope, explicit member order
and main placement are composed-scene dependencies. Their change invalidates
only the host composed entry and ancestor composition and requests rendering;
child-local paint/layout, Image dependencies, focus/capture and primitive keys
remain unchanged. Engine-created optional metadata wraps the same immutable
child Arcs. An identical declaration creates no invalidation or render demand.

The independently accepted 2026-10-08 SPEC-HAL-359 §2.5.1 main-affordance addition
also treats source/destination/order declarations as composition-only dependencies.
Set/clear/reassignment invalidates the actual source-to-old/new-host paths and
host ancestors, without local repaint/layout, Image re-observation or hit-grid
invalidation. Private current exports remain bounded by live mount membership.
The source's immutable overlay Arc carries its own private current eligibility
evidence, captured from overlay scene and OVERLAY-ORIGIN sidecars before merging
with ordinary local sidecars. Cached exports validate that paired evidence at
composition; tree setters have no access to the App cache. A declaration does
not certify future output, and an empty repaint replaces both Arc and evidence.
Ordinary unexported overlays retain their existing behavior. This addition is
implemented in 0.207.0-beta and independently source-accepted on 2026-10-08;
its executor proof does not imply native or product acceptance.

### 3.2 Clips, scrolling, and order

Retained composition preserves the exact fresh-paint order:

```text
widget local primitives
    -> viewport clip
    -> widget clip
    -> scroll transform
    -> ordered child fragments
    -> close transforms/clips
    -> widget overlay primitives
```

Clip, scroll, layer, z-order, focus-indicator, tooltip, and overlay-hit-region behavior must be byte-for-byte or
structurally equivalent to a fresh scene for the same snapshots.

Fresh and retained traversal also preserve the complete SPEC-HAL-359 §2.5
effective hierarchy and closed member intervals. Main-only composition metadata
does not rewrite primitive Z, clips, scroll, overlay class, SceneNodeKey or Image
generation. Canonical and conservative accelerated hit selection project that
same explicit member list; stale spatial indexes cannot decide absence.

SPEC-HAL-359 §2.5.1 explicitly diverts a declared source's complete current
post-child overlay exactly once into its ancestor host's closed main-affordance
interval, after ordinary content2 and before enclosing chrome7. Source paint,
focus and children remain in place. The engine transports the same immutable
output with exactly the viewport/widget/rounded Clip and Scroll scopes enclosing
its original append, in order, plus overlay-local scopes; target/outer scopes
apply once. Source child-only scopes do not enclose that post-child append and
must not be introduced. Enclosing member prefixes survive; intervening member
prefixes below the host are replaced only for the export. Primitive Z, coordinates,
keys, Image generations and global-overlay classification are unchanged. Fresh
and retained composition use the same capture, eligibility and export path;
ineligible current output fails a programming invariant without filtering,
fallback, source repaint or semantic callbacks. No export may escape the root.
Source repaint/terminal cleanup replaces the exported interval in the same
admitted scene through the existing narrow dirty chain.

**363/364 scoped amendment:** Ordinary fresh/retained equivalence is unchanged. Historical capture
SHALL preserve the exact local → viewport clip → widget clip → scroll →
ordered children → close → overlay topology, with SPEC-HAL-364's explicit
internal/live coverage classification. A root crop, wrong-scope flattened mask
or captured sidecar is not equivalent.

### 3.3 Lifetime and eviction

Removing or replacing a widget immediately invalidates its mount lifetime and
evicts its current reusable retained entries. Current-cache size remains bounded
by live WidgetTree membership plus explicitly bounded renderer resources.

**363/364 scoped amendment:** Before legal eviction, already-certified paint values MAY enter bounded
SPEC-HAL-364 source custody through the existing lifecycle owner. This handoff
SHALL NOT repaint a removed producer or defer authoritative teardown. Only the
independently certified historical generation and required resource leases may
survive, subject to SPEC-HAL-363/364 age and capacity limits. Live-cache bounds
do not count removed identities as live; historical holdings are separately
and fully charged. Unowned historical IDs, fragments and stale generations
must not accumulate.

**INV-354-FRAGMENT-1:** A clean retained fragment is immutable.

**INV-354-FRAGMENT-2:** Reuse is permitted only when every declared dependency generation matches.

**INV-354-FRAGMENT-3:** Reused current sidecars are replayed with the current
fragment; they are not silently dropped.

**363/364 scoped amendment:** Historical occurrences SHALL neither retain nor replay sidecars.
This is lawful only for the separate paint-only value class; a reused current
fragment cannot be relabelled historical to suppress current sidecars.

**INV-354-FRAGMENT-4:** Ephemeral `SceneNodeKey::None` primitives may be retained only as part of an otherwise clean
widget fragment. Dirtying that fragment recreates them.

**INV-354-FRAGMENT-5:** Ordinary `SceneNodeKey` uniqueness, widget affinity
and SPEC-HAL-083 lifecycle guarantees remain unchanged.

**363/364 scoped amendment:** For FRAGMENT-4/5, anonymous source leaves MAY be addressed within one
immutable SPEC-HAL-364 generation/occurrence, without a cross-generation stability
claim. Known authored keys retain source affinity, while occurrence qualification
supplies distinct paint identity. SPEC-HAL-363/364 SHALL NOT fabricate WidgetIds
or reuse one SceneNodeKey for coexisting occurrences.

---

## 4. Frame Execution

SPEC-HAL-002 phase order remains immutable. A retained frame does not remove phases; it permits a phase to prove that no
work is required or to reuse a completed immutable result.

### 4.1 Update phase

**Scoped adopted execution — 2026-10-26:** the JC-delegated availability
decision in SPEC-HAL-108 §13.5 settles through the existing App deferred executor
before ordinary Widget Update and framework appearance/focus publication. Pending
availability cuts semantic intake between events, preserving original qualified
envelopes for the next legal boundary. Lifecycle-generated work is not recursively
applied. Availability is never applied during layout/Paint/SDF-prePaint or through
a second Widget Update. Changed policy invalidates only its owner, old/new focus
projections and required ancestor composition before retained reuse; equal or
rejected requests stay clean and layout/unrelated LOCAL fragments survive.

The narrowly admitted HAL-101 section 2.5 owner-property/factory collector and structural-manifest application execute at existing lawful App deferred/construction boundaries before affected Widget Update and appearance publication. They do not use Draft HAL-190/319 as authority or add a phase. Already accepted edits/results complete within their original transaction. Layout emits only a pure owned desired-role manifest; the next lawful Phase 2 requalifies and applies its exact current delta, and a stale or superseded manifest cannot install or rebind a role. Until that role is ready, gate ONLY its dependent input/publication; do not represent it as ready or replay input withheld before readiness. This gate MUST NOT defer the whole scene or block unrelated widgets, rendering, input or independent roles, including roles in the same owner/manifest. Their ordinary processing and publication continue. Ordinary child Update is exactly once; invalidation remains narrow and unrelated G1 publication/cutoff is unchanged.

Cross-thread and asynchronous data enters through synchronized queues. Widgets and screens consume bounded/coalesced
state during Halcyon's update authority. An attached receiver uses `needs_update`; `needs_animation` remains reserved for
an actually unsettled visual effect. Update code reports the narrowest correct invalidation class and emits no paint
invalidation when the final drained state is visually identical.

SPEC-HAL-021 Timeline advance is an engine operation before event routing. After
routing, `UpdateContext` exposes read-only sampling of that same frame's Timeline
state. A widget may use the sample to update its own cached presentation value or
publish a complete immutable model/view state and revision. It must compare the
derived result with its prior state and report only the narrowest actual
invalidation.

The sampled Timeline value is not custom-render or renderer authority. When an
animated value is required by layout or custom-render preparation, Widget update
publishes the complete immutable owner result before those phases. Layout,
post-layout projection, custom-render preparation, renderer preparation, and GPU
work cannot sample or mutate Timeline or revise WidgetTree/application state. A
purely paint-owned finite consumer may retain SPEC-HAL-021 read-only Paint
sampling only when no earlier phase depends on that value and Paint performs no
state mutation.

### Mounted immutable Update publication transfer

**INV-354-G1-OWNER / FORBID-354-G1-INFERRED-PEER.** The canonical App/WidgetTree
Update owner MUST execute an opt-in transfer only for an application/Screen-declared
relation naming one mounted producer and one separate ordinary flat Viewport3D.
The relation MUST use typed stable tree identity qualified by current mount and
lineage. Sibling order, active viewport, geometry and payload content MUST NOT
select or reinterpret either endpoint. No Widget may own, retain or borrow a peer
Widget or WidgetTree through this capability.

**INV-354-G1-EXCLUSIVE-WRITER / FORBID-354-G1-CONCURRENT-WRITER.** An explicitly
linked Viewport3D mount MUST have exactly one engine-enforced request-writer
authority. Link activation MUST reject incompatible existing active ownership.
Only the canonical qualified cutoff transfer may supply a changed complete request
while linked; every direct public set_request attempt, including an equal value,
MUST synchronously return an explicit typed exclusive-writer refusal without
modifying requested, committed or target state. The engine-qualified transfer
MUST use the same existing semantic/structural/view/target validation with no
bypass. Direct Screen-driven requests remain legal for UNLINKED mounts and may
resume only after lawful link retirement. Activation MUST NOT reattribute a
compatible old committed front. Caller authentication, public error representation
and storage are not selected by this proposed contract.

**INV-354-G1-COMPLETE / FORBID-354-G1-PARTIAL-OR-MUTABLE.** During its single
engine-timed Widget::update(dt), the bound producer MAY supply one complete OWNED
immutable candidate containing exact qualified source/target/link identity,
correlation, checked view revision, ViewDescriptor3, RenderSnapshot, complete
ModelState and sample count. The source alone owns already-authorized semantic
derivation. Partial patches, mutable domain-state sharing, deferred semantic
callbacks, peer access and renderer-time derivation MUST NOT enter this boundary.

**INV-354-G1-CUTOFF / FORBID-354-G1-SECOND-PASS.** After the whole existing timed
widget traversal and all its mutable widget borrows end, the existing tree owner
MUST finish at most one bounded nonreentrant frame-local cutoff before Layout,
while still inside the existing Update authority. Traversal completion is not
completion of that enclosing Update authority: the cutoff completes publication
within it, after which inputs are immutable through Layout, preparation and paint.
The owner MUST resolve the declared current mounts and link, verify a separate
ordinary flat Viewport3D recipient, and invoke its existing complete checked
set_request once for a changed eligible candidate. It MUST NOT rerun update,
derive another semantic value, access Timeline, mutate structure, call consumer
code to process transfer, introduce another phase/clock/router, or write
application state after preparation.

**INV-354-G1-BOUNDED / FORBID-354-G1-LAST-WRITER-WINS.** At most one live producer
binding may address a target, and at most one complete pending publication may
occupy a live link in one frame. At application admission, the application MUST
explicitly supply a strictly finite positive aggregate maximum of simultaneously
live/unconsumed result reservations; no unlimited or guessed numeric default is
permitted. The canonical APP-level mechanical link/result owner MUST enforce that
bound. Each admitted link MUST reserve one result slot against it, and bounded
complete-request storage, BEFORE publication admission. A link MUST have at most
one pending candidate and one unconsumed result. A later candidate MUST receive
explicit refusal, with an observable reason if its reader exists, until the
previous result is actually consumed. Neither pending input nor retained result
may be overwritten. Unbound, competing, retired, wrong-type, saturated or stale
attempts MUST be explicitly refused according to their known cause. Saturation
MUST refuse new link admission rather than drop an earlier result. No unbounded
queue, persistent revision history or separate unbounded retired-link tombstone
list is permitted. Existing finite HAL-355 limits remain independently binding;
link storage cannot covertly multiply them.

**INV-354-G1-LIFETIME / FORBID-354-G1-RESURRECTION.** Endpoint qualification MUST
distinguish current mount/lineage and concrete mounted type from recycled NodeId
slots, visible IDs and concrete replacement at a surviving TreeWidgetId.
Replacement, unmount or retirement before cutoff MUST stop future admission and
settle the admitted candidate as Stale or Retired according to its actual cause,
under its original source/link/mount/correlation, without targeting a successor.
Retirement MUST prevent later publication and MUST NOT change the provenance of
an already committed or failed last-good front. The APP-level result obligation
MUST survive both Widget lifetimes and the originating Screen lifetime.
Retired links MUST retain their existing reserved result slot under the aggregate
application bound until lawful application-owner consumption or final application
shutdown. A previously issued terminal result MUST remain readable after removal;
retirement MUST NOT overwrite it. No successor may inherit the original result
or an old front as current new semantics.

**INV-354-G1-SETTLEMENT / FORBID-354-G1-LOST-PUBLICATION.** The canonical APP-level
mechanical link/result owner, independently of Widgets and Screen, MUST retain
each admitted candidate's original source/link/mount/correlation and exactly one
terminal typed result for lawful application-owner readback by that exact link
identity. The following closed categories are proposed meanings, not Rust API
spellings:

| Category | Exact mechanical meaning |
|---|---|
| AcceptedForCutoff | Admitted pending candidate; not settled and not success. |
| Transferred | Changed eligible request's checked set_request returned success; not renderer preparation or commit. |
| Unchanged | Complete equal-value/equal-revision no-op; explicitly typed consumable result with no renewed GPU work attributable to repetition. |
| Rejected | Known setter error or known invalid request, retaining the actual reason. |
| Stale | Source/link/target qualification changed. |
| Retired | Owner lifetime ended, retaining the actual retirement cause. |
| Indeterminate | Actual source-qualified inability to establish a mechanical outcome, retaining original identity and cause. |

Every AcceptedForCutoff candidate MUST reach one of the six terminal categories.
A synchronous set_request Result is definitive: success or known rejection MUST
NOT be labelled Unknown or Indeterminate. A genuinely Indeterminate result MUST
forbid blind replay and all success/display claims. An immediate refusal of a
new candidate MUST NOT erase a pending candidate or previously retained result.
Transfer admission, setter success, per-target preparation, committed publication
and actual display MUST remain separate. Failure MUST NOT fabricate a viewport
commit, silently lose an admitted publication or undo independently admitted
application Scene state.

**INV-354-G1-RESULT-OWNER / FORBID-354-G1-SCREEN-BOUND-RECEIPT.** Transfer-result
readback MUST be available to the lawful application owner after either mount or
the originating Screen disappears. No originating Widget, Screen, renderer,
shared global or best-effort UI intent queue may own this guarantee in place of
the canonical APP-level mechanical owner. A result MUST remain unconsumed until
actual lawful application-owner consumption; its slot MUST NOT be treated as
free merely because a mount or Screen was removed. Final application shutdown
ends the in-process guarantee: unsettled admitted candidates MUST be classified
Retired with shutdown cause as far as that running application can observe;
already terminal results retain their original meaning. No persistence,
post-process delivery or external observation is implied, and no silent close
or indefinitely accumulating application memory is permitted. This mechanical
obligation does not settle application-owned semantic command acceptance.

**INV-354-G1-RETURN / FORBID-354-G1-CROSS-PHASE-STATUS-BORROW.** Application/Screen
MUST observe prepared target status at its next ordinary lawful tree-access
boundary through scoped lookup/downcast, clone the owned status and copy the
surface/target-metrics/tuple evidence, then end the tree/viewport borrow.
Observation MUST retain original source/link correlation and distinguish requested
from committed state. It remains historical until the application verifies the
source, mount, target and current semantic association at use. No direct
post-preparation callback or same-frame post-preparation Screen hook is added.
This prepared-status observation MUST NOT substitute for APP-owned transfer-result
readback, whose original identity and bounded lifetime survive Screen/mount removal.

**INV-354-G1-ZERO-USE / FORBID-354-G1-GENERAL-BUS.** Without an active explicit
link, ordinary core 2D and flat 3D behavior/structure MUST remain unchanged. Generic
2D rendering MUST acquire no 3D branch or resource work. Existing direct Screen-set
requests remain legal for UNLINKED mounts and resume only after lawful retirement;
linked attempts obey INV-354-G1-EXCLUSIVE-WRITER even when equal. Any generic
phase-limited immutable transfer machinery
used for this contract admits ONLY the mechanically checked ordinary flat
Viewport3D request as its initial consumer; it confers no general semantic-event
bus, arbitrary cross-widget mutation or application-domain authority.

**363/364 scoped amendment:** A group parameter needed after layout SHALL be sampled/published in
Update. Post-layout admission and retained composition may derive immutable
values from it; they SHALL NOT sample/mutate Timeline or revise semantic/control
state. A post-layout outcome is an immutable observation reconciled by the next
legal Update, not a late write to the group controller.

**HAL-128 geometry retirement qualification (2026-10-08, delegated
architectural acceptance):** The qualified production scene
entry runs one bounded terminal checkpoint after the single normal Widget Update,
with widget borrows released and before Layout. Existing tree-aware cancellation
retires exact opted-in press lineages; actually changed live opted-in capture
owners receive final owner-qualified appearance facts through HAL-128's narrow
synchronous cache-settlement hook. Recapture/publish final facts and prepare
remaining capture-dependent presentation once. This extends §4.1's consuming
coherence, not the global frame order: no second Update, Timeline/timer advance,
late semantic callback or repair frame. Implemented in 0.208.0-beta and
independently source-accepted on 2026-10-08; HAL-128's R4/RV1 subsection records the
completed owner-target proof. Native and product integration remain separate.

**Scoped optional Button SDF requirements — 2026-10-25:**

The scoped 266A SDF material commit qualifies phase publication only for a
separate engine-owned immutable resource-status snapshot after preparation and
before retained Paint. It does not revise WidgetTree/application semantic state,
repeat Update/layout, mutate focus/capture facts or advance/sample Timeline.
A changed committed identity, status, mapping or coverage invalidates only the
consuming owner's local Paint and required ancestor composition before reuse,
retiring old E and consumed Image dependencies. Equal commitments create no
continuing work. Unavailable suppresses only the stale material contribution;
content and independent focus output persist. Ordinary non-material paths and
engine cadence remain unchanged.

### 4.2 Layout phase

If the root is layout-clean and root constraints are unchanged, layout is a no-op that reuses the last completed geometry
generation. Paint is permitted against that valid generation. Halcyon must not require an O(tree) per-frame epoch stamp
merely to prove unchanged geometry remains valid.

### 4.3 Scene phase

The scene phase replaces dirty local fragments, recomposes only dirty ancestor paths, replays retained sidecars, and
publishes one immutable presented scene snapshot. If no visual dependency changed, it reuses the previous presented
snapshot.

**363/364 scoped amendment:** One complete current presented scene containing admitted occurrences
SHALL be selected before SceneGraph diff. A fallback changing that scene MUST
be selected before this final scene-choice cutoff.

### 4.4 Renderer preparation and presentation

The existing mandatory order remains:

```text
presented scene
    -> SceneGraph diff
    -> SceneCache dispatch
    -> render preparation
```

Unchanged retained fragments preserve their prepared renderer resources. A small fragment change must not require
recreating or resending unrelated scene geometry or text.

GPU presentation may redraw the target for an admitted visual frame. This SPEC does not require partial OS damage,
swapchain preservation, or skipping a necessary GPU draw. It requires that CPU layout, scene emission, text work, draw
planning, and buffer preparation remain proportional to the changed visual projection.

**INV-354-FRAME-1:** One admitted engine frame performs at most one update/layout/scene/presentation sequence per
surface.

**INV-354-FRAME-2:** Data ingress frequency cannot increase presentation attempts above engine cadence.

**INV-354-FRAME-3:** A frame with no visual, geometry, animation, focus, overlay, theme, or surface change performs no
layout, scene replacement, draw-plan rebuild, GPU compilation, or presentation.

**INV-354-FRAME-4:** A paint-only frame preserves the last completed geometry generation.

**INV-354-FRAME-5:** An animation sample consumed by Widget update and needed by
same-frame custom preparation is published as immutable widget-owned state before
layout. Custom preparation observes that exact state/revision in the same frame;
it cannot defer the observation to paint or pull it from Timeline itself.

**363/364 scoped amendment:** From the §4.3 scene-choice cutoff through diff, dispatch, Build,
Compile and Execute, a new canonical scene SHALL NOT be spliced in and the
sequence SHALL NOT restart. Unavailability discovered later follows
SPEC-HAL-364 B-9.9's complete surface-failure contract; unforeseeable failure
after execution begins follows B-9.18. Ordinary unchanged-scene reuse remains
legal only when exact dependencies remain valid.

---

## 5. Required Dependency Invalidation

HAL-128's accepted opt-in geometry-terminal path clears its actual preview and
invalidates changed owner paint/ancestor composition before the first affected
reuse. Final snapshot publication alone does not settle a widget's preexisting
capture cache; the narrow terminal hook must consume the final real facts.
Unchanged owners need no new resolution. Geometry invalidity is established at
legal pre-Layout checkpoints; Layout/Scene/Paint remain free of semantic teardown.

The following changes invalidate paint even when application values are unchanged:

- active theme generation;
- display scale or pixel-snapping generation;
- focus/capture presentation for the old and new focused widgets;
- active layout-transition visual rects;
- animation timeline values consumed by a widget;
- hover/selection/caret state used by paint;
- clip, scroll, layer, or overlay projection changes;
- debug-outline or other engine-owned diagnostic presentation.

When a presented scroll coordinate is shared across a closed composite, publication precedes widget updates and the
entire bounded set of retained fragments that consume that coordinate invalidates together. Reusing a body, header,
footer, clip, hit, or affordance fragment from the previous scroll coordinate is forbidden. This is paint-only when
completed layout geometry is unchanged.

SPEC-HAL-366 adds one scoped finalization boundary: ordinary live-coordinate publication remains before widget
updates; an explicitly opted-in pending sequential-X restore is finalized against first tight layout and published
through the same owner after that layout, before input projections and paint. Its consumers are the existing linked
paint/hit projections. This does not rerun Widget update, resolve anchors in layout, or authorize late mutation of
application/control semantic state. The existing bounded composite invalidation applies before retained reuse.

`Widget::needs_animation()` opts that widget into per-frame local paint invalidation while true. It does not authorize
unrelated subtree repaint or layout.

Generic finite or repeating Timeline playback follows SPEC-HAL-021. Active
playback may keep the existing engine animation demand alive, but repeated
sampling of an unchanged derived value emits no new invalidation. An infinite
handle is explicit owner-held work; an idle receiver, context, or custom-render
participant is not animation demand.

`Widget::needs_update()` keeps Halcyon's update phase scheduled without implying visual change. It is the correct
contract for synchronized data-channel polling and asynchronous completion. An update-only widget must explicitly call
`UpdateContext::invalidate_paint()` or `UpdateContext::invalidate_layout()` when consumed work changes its projection.
`needs_update()` does not dirty scene fragments and does not authorize presentation by itself.

**INV-354-CADENCE-1:** Update cadence and paint cadence are distinct. Polling an empty channel or incomplete asynchronous
producer may keep update ticks alive while every visual phase remains a no-op.

**INV-354-CADENCE-2:** Continuous engine demand is admitted through a stable engine-owned deadline. Its interval follows
the current display refresh rate when the platform reports one and otherwise falls back to 120 Hz. Repeated platform
wakeups or `AboutToWait` callbacks before that deadline do not invoke another update phase. An ordinary late wake advances
from the prior deadline phase; it must not redefine the next deadline as `observed now + interval` and thereby accumulate
scheduler latency. A real stall skips every elapsed period in one step and emits no catch-up burst. A no-change
`needs_update` widget therefore remains bounded by Halcyon's active engine cadence and cannot spin the platform event loop
merely because retained presentation suppresses visual work.

Text content changes invalidate the owning paint fragment. Text shaping and layout remain governed by the Constitution's
text ephemerality law: no glyph/layout references survive across frames, and any cache uses stable value keys.

The scoped HAL-048 UI typography addition (Author JC, delegated architectural
acceptance, 2026-10-10) makes exact admitted face/configuration and cluster gap
dependencies of text measurement and retained logical output. A lawful changed
control request invalidates LayoutAndPaint and ancestor composition before the
first successor scene; equivalent requests are no-ops and unchanged siblings
remain reusable. Retained values hold immutable font backing, never shaped runs
or native handles. Atlas bitmaps are face-bound; spacing changes only positions.
This addition is pending source proof and independent review.

**363/364 scoped amendment:** Dependencies SHALL include the source/configuration/cue,
capture/content generation, live coverage/order/host, target metric, execution
profile, resource/pipeline/font and group-projection values actually consumed.
Changed occurrences and required ancestor composition invalidate; unrelated
clean siblings do not. Projection/focus/capture alone SHALL NOT dirty layout.
Current-style metrics enter layout through their existing owner.

---

**Scoped optional Button SDF requirements — 2026-10-25:**

Section 4.1's admitted SDF material commitment and removal use the same bounded
owner-local dependency rule, including retirement of old E and Image dependencies;
neither status publication alone nor new frame admission permits global dirtiness.

## 6. Public Consumer Contract

Consumers follow this rule:

```text
data changed
    -> update model/widget/data source
    -> classify effect
    -> mark paint only, or mark layout and paint
    -> return control to Halcyon
```

Consumers must not call the platform renderer, request one frame per data event, or use layout invalidation as a generic
"show new values" notification.

Specific owner examples:

- `StatusGrid::set_readout`: actual natural-height/flex change is layout-and-paint; unchanged row structure and height are
  paint-only.
- `ListView`: visible cell replacement is paint-only; row topology, row/column extent, or viewport geometry change is
  layout-and-paint.
- Plot and aligned-order-book channel updates: final coalesced visible state is paint-only; idle receivers and identical
  final snapshots do not animate or invalidate paint.
- Standalone `OrderBook`: same-topology values invalidate the shared-state row paint subtree; row-count/scroll-extent
  changes are layout-and-paint; row flash animation is owned by `OrderBookBody`, which paints it.
- Internal GPU texture/render generations do not by themselves dirty surrounding layout.
- An animated custom-render owner samples Timeline during Widget update, publishes
  one complete immutable state/revision, and lets the existing participant-local
  preparation consume it later in that frame without Timeline access.

---

## 7. Forbidden Behaviors

**FORBID-354-DATA-DRIVEN-FRAME:** A data producer or consumer directly requests one render attempt per data event.

**FORBID-354-PAINT-AS-LAYOUT:** A known content-only change calls layout invalidation as its repaint mechanism.

**FORBID-354-FRESH-WORLD:** A localized paint change rebuilds every widget scene fragment or resends unrelated renderer
geometry merely because the output surface is presented.

**FORBID-354-STALE-FRAGMENT:** A fragment is reused after any undeclared or changed dependency.

**FORBID-354-REFERENCE-CACHE:** A retained cache stores widget, tree, paint-context, layout, text-run, glyph-cluster, or
renderer borrows across frames.

**FORBID-354-CROSS-PHASE-MUTATION:** Layout, scene construction, renderer preparation, or GPU submission mutates
WidgetTree or application data.

This includes custom-render preparation sampling or advancing Timeline, deriving
a new semantic model/view value, or writing that value back into the WidgetTree.

**FORBID-354-UNBOUNDED-RETENTION:** Removed widget lifetimes, old fragment generations, draw plans, or staging resources
accumulate without a live bounded owner.

---

## 8. Required Proof

Completion requires focused public-path proof:

1. A burst of at least 440 value publications before one engine tick produces one coalesced paint generation, zero
   widget layout calls, and at most one presentation attempt.
2. Repeated paint invalidation is idempotent.
3. A changed widget repaints while an unchanged sibling's local paint function is not called.
4. A child repaint recomposes ancestors without repainting their clean local primitives.
5. A layout-affecting change still performs layout and produces the same geometry as a fresh frame.
6. Retained and forced-fresh scene outputs are structurally equivalent for clips, scrolling, overlays, focus indicators,
   tooltip triggers, and draw order.
7. Theme, scale, focus/capture, animation, and structural changes invalidate all required dependencies.
8. Removing and rebuilding widgets cannot reuse stale mount fragments; cache cardinality stabilizes under repetition.
9. SceneGraph diff and SceneCache dispatch remain ordered before renderer preparation.
10. A faithful live-data StatusGrid/ListView/Plot path demonstrates bounded layout and scene work under repeated updates.
11. Plot and order-book widgets with attached idle receivers declare update work but no animation, a drained burst
    produces one narrow invalidation, and an identical final snapshot produces none.
12. Under arbitrarily frequent platform scheduling opportunities, an unchanged update-only widget receives no more than
    the configured active engine cadence and produces no scene or presentation work. Repeated bounded late wakeups retain
    the configured display-rate phase, while a multi-period stall admits one frame and advances directly to a future
    deadline.
13. A closed composite whose reducer state is painted by a retained child invalidates that exact child fragment for
    pointer, keyboard, focus, hover, and selection changes without using layout as a repaint mechanism; an unchanged
    reducer result remains paint-clean.
14. A Timeline handle started by an event samples exact `from` in same-frame
    Widget update; the resulting immutable owner revision is consumed by
    same-frame custom preparation before paint, while an unchanged derived sample
    produces no extra invalidation and custom preparation/renderer phases cannot
    access Timeline.

Performance proof reports separate counts or timings for update, widget layout, local paint, scene recomposition,
draw-plan preparation, renderer compilation, GPU render, and presentation. A lower FPS or consumer-side throttle is not
proof of completion.

---

## 9. Implementation Status

Halcyon 0.187.1-beta implements the public paint/layout invalidation split, bounded update-only cadence, explicit shared-state
paint-subtree invalidation, clean-root layout reuse, immutable retained widget scene fragments, retained sidecars,
topology-bounded eviction, and unchanged-frame suppression through the production App path. Focused proof covers
coalesced 440-update ingress, zero-layout paint replacement, changed-child/clean-sibling fragment reuse, unchanged-frame
scene suppression, bounded topology eviction, retained-node renderer flattening, and Plot/OrderBook/AlignedOrderBook
channel polling without unconditional animation paint. Non-Linux platform redraw scheduling retains one stable 120 Hz
deadline across event-loop callbacks, so empty update-only polling cannot bypass `WaitUntil` and spin `Screen::update`.

Halcyon 0.195.0-beta derives that non-Linux active interval from the current monitor when available, retains the scheduled
phase across ordinary `WaitUntil` latency, skips whole missed periods without a redraw burst, and falls back to 120 Hz only
when the platform cannot report a refresh rate.

Halcyon 0.189.3-beta closes the retained ListView selection dependency: pointer, keyboard, focus, and hover changes
invalidate the exact `ListViewBody` paint fragment, while unchanged selection remains paint-clean and the completed
layout epoch is reused.

The remaining completion proof is consumer integration and measurement of the real StatusGrid/ListView/Plot workload
under §8.10. A visually changed surface still traverses the full composed scene during draw-plan construction; existing
renderer caches retain resources, but fragment-granular prepared draw-plan reuse is not claimed by this version.

The 2026-08-23 same-frame Timeline-sampling additions are binding but are not
claimed implemented by the versions above. At amendment time, live
`UpdateContext` does not yet expose the SPEC-HAL-021 read-only sampling capability
or the §8.14 same-frame custom-preparation proof.

Scoped composite-editor adoption — 2026-10-29: the composite-editor additions and replacements in this document are Binding under coordinated architectural adoption, with JC's final ownership and veto retained. The retained adoption record identifies the exact three source bindings and adopted scope. No implementation or conformance result is asserted; all unrelated declared statuses remain unchanged.


## 10. App-owned F12 outlines and change-only retained invalidation (2026-11-14)

**Status: ADOPTED scoped correction under the Binding HAL354 sections 3.1 and 5 and Foundational HAL002.** This neither activates Proposed HAL093/HAL365 nor changes keyboard policy.

### 10.1 Demonstrated failure

The retained 2026-11-14 investigation executed one owner-context App suite with 129 selected, 93 passing and 36 failing cases. Two original ui_wake tests share one witnessed first cause: an earlier passing F12 test leaves DEBUG_WIDGET_OUTLINES true globally. Original paint_tree_reusing subsequently calls mark_subtree_paint_dirty(root) on every frame solely because the flag is true, despite unchanged inputs, clean retained entries, and no appearance/scale/structure change. This repaints clean locals and fabricates another scene after a settled Timeline. The observation code was removed and the original source restored. The remaining 34 App failure cases and the separately failing Router case retain their unresolved status; this finding does not explain all 14 context-dependent App failures.

### 10.2 Owned value and exact invalidation

INV-354-OUTLINE-OWNER: The outline presentation value belongs to a single live AppContext, default false upon its construction. AppRoot's existing F12 input route toggles that exact owning AppContext, not a shared process flag. Two independent AppRoots cannot change each other's outline state, including when tests or applications run concurrently or sequentially. Programmatic read/set operations use explicit AppContext receiver methods. The zero-argument module-level exported toggle_debug_outlines and debug_outlines_enabled cannot express lawful instance ownership: replace their unscoped interface with the receiver methods and classify this unavoidable public surface correction under the current zero-major release policy. No process static, TLS, implicit registry, hidden compatibility shim, environment switch, OS decision, or second screen/widget owner is permitted. Retain F12 behavior, existing reservation/physical-capture/Escape precedence, and exact visual outline primitives, colors, geometry, depth and ordering.

INV-354-OUTLINE-SNAPSHOT: Propagate the owning value as an explicit immutable frame input to the original fresh/retained paint path in both run_ui_loop and App::run. In ordinary standalone rendering without an AppContext, supply an explicit false input, preserving existing public standalone entry signatures. PaintContext and RetainedPaintEnvironment may consume the frame value but may not become authorities for it. A retained cache may record only the last consumed value for equality comparison, scoped to the owning App lifetime.

INV-354-OUTLINE-DELTA: Before retained scene choice, compare the current outline value with the cached last-consumed value. A real false-to-true or true-to-false change invalidates the affected local paint fragments and ancestor composition exactly once using existing legal pre-paint dirty propagation. The first uncached paint emits the actual current value normally. With the value unchanged, INCLUDING sustained true, outlines remain present through retained fragment reuse without new local paint, another scene, or frame demand. F12 or an explicit programmatic change reaches the next ordinary eligible frame without a forced Screen rebuild, layout invalidation, input-phase WidgetTree mutation, or a new rendering route.

FORBID-354-OUTLINE-LEAK: No global read/write or test cleanup of diagnostic state; no test-run serialization to hide leakage; no changed asserts/tolerances, suppressed real visual updates, new feature activation flag, duplicate widget traversal, or change to unrelated renderer or input behavior. The old debug flag is not a reason to repaint unchanged frames. This follows HAL354 FRAGMENT-1/2 and FORBID-354-FRESH-WORLD, PM018's change-owner invalidation and the Constitution's explicit-phase and no-hidden-global rules.

### 10.3 Observable acceptance

Under the SAME production App and retained paths, prove: (1) two simultaneously live AppRoots start with independent false values; F12 on one changes only its own actual outlines; (2) the first false-to-true scene paints the original outlines, a settled unchanged successor reuses the same local Arc and emits no new scene, and true-to-false removes the outlines once; (3) explicit AppContext setters have identical changed-versus-no-op invalidation behavior; (4) the two original ui_wake_test assertions and their sequences remain byte-identical and pass inside the owner-context run; (5) fresh/retained output and original F12 input precedence agree. Reuse the original owner suite and record the remaining exact failures without concealing them or claiming that all 36/37 were fixed. Do not replay unrelated previously qualified suites.

Implement in the current existing App, retained-paint and adjacent owner-test scopes with only necessary own-package release metadata. Do not touch product projects, optical material/shaders, GPU custody, accepted keyboard routing, external runtime or other authority documents. Preserve pre-existing foreign document edits and old diagnostic receipts. This scoped correction is a complete framework bug-fix unit, not a generic diagnostic subsystem.


### 10.4 Independent source and receipt disposition (2026-11-14)

**ACCEPTED within HAL354 section10's scoped implementation and original App owner-context proof. The complete App suite remains RED.** Independent review reread live app.rs, the bound section10 contract and the retained original execution receipts. Live revision 94e2d67d and Halcyon0.237.0-beta match the delivered candidate. The process-global DEBUG_WIDGET_OUTLINES control is absent. AppContext owns the default-false value; the public receiver methods replace unscoped module functions, F12 changes that AppContext, and UiWakeHandle is used only on a real changed set. The two ordinary loops supply explicit immutable snapshots. The original pre-Layout retained comparison invalidates on a bit delta; the former steady-true subtree-repaint branch is removed. Original outline source paint, colors, depth and physical-pixel sizing remain present.

The scoped native Grip receipts show changed-check child0 and package build child0. The corrected App owner test execution was real Cargo child101 with 131 selected, 109 passed, 22 failed, zero ignored; do not relabel its terminal status PASS. Two added owner/scene regressions and both original byte-identical ui_wake representatives passed. Comparing the previous 129-case owner receipt shows 14 formerly failing selectors now passing, zero newly failed selectors. That comparison does not prove the causes of every recovered selector. The initial 25 E0061 diagnostics prevented test execution until four existing private test fixture modules received ten explicit standalone false arguments; original assertion/other bytes were preserved. The afterward-only F12 comment edit did not change the verified runtime behavior.

The 22 remaining App failures are distinct: two cursor/input-typography owners and twenty transient_presentation owners. The transient presentation pair HAL363/364 remains Proposed, not activated by this debugging acceptance. The separately recorded EventRouter baseline failure was not rerun or closed. JC's native visual judgement, OS physical input, Present/scanout and dependent-product acceptance are not asserted. No additional verification was repeated for this independent review.

### 10.5 Built-in F12 shortcut removal (2026-11-25)

JC directed removal of the built-in outline shortcut on native and Web. This supersedes the F12-specific activation, reservation and acceptance requirements in §§10.2–10.3. When the host delivers F12, ordinary key routing and explicit application bindings apply. The existing AppContext setter remains the outline control, with the same ownership, frame snapshot and change-only invalidation.
