# SPEC-HAL-266A: Per-Target Custom Render Preparation Boundary

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Status:** Adopted / Binding
**Date:** 2026-08-21
**Amendment Type:** C — Structural Amendment
**Amends with literal replacement text:** SPEC-HAL-295 INV-295-RSZ1
**Parent scope read subject to this amendment:** SPEC-HAL-266 §§2, 4, 5, and 8; SPEC-HAL-295 §§4.2–4.3, 4.8, 5, 7, and 13
**Depends on:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-021, SPEC-HAL-024, SPEC-HAL-082, SPEC-HAL-158, SPEC-HAL-266, SPEC-HAL-295, SPEC-HAL-295A, SPEC-HAL-354
**Extended by:** SPEC-HAL-355 (Adopted / Binding; 3D-viewport specialization of the per-target preparation boundary without changing its authority)
**Observation extension:** SPEC-HAL-361 (Adopted / Binding; canonical App attempt identity in the existing metrics boundary, with immutable results observed after rendering)
**Runtime scope:** Canonical `App::run` path

**Scoped App-density amendment — Accepted / Binding (2026-10-16):** JC's mandate
records technical acceptance of HAL-024 Layer E (`ec307c53`). In this contract,
`DPR`/`scale_factor` means the admitted target physical/logical ratio: `E = D × S`
for an App-owned target, or its explicitly supplied admitted ratio otherwise.
Raw OS density and App preference remain outside participant authority.
Implementation proof for App UI Scale is pending; proposed 363/364 text remains
inactive and all unrelated owner boundaries are unchanged.

This amendment is adopted and binding. It defines the target-metrics boundary
implemented by Halcyon's canonical `App::run` path, with Plot as its proving
custom-render participant.

---

**Button material contract and implementation admission.**
Contract adoption does not establish source conformance. The existing renderer
and resource owners must verify producer identity, typed request/publication and
commit transport, precise failure causes, generation protection and the actual
Button content/focus/retained integration before execution is claimed.
Unresolved implementation admission remains unresolved and requires the governing contract owner's
resolution, not executor invention. This correction selects no new source, ABI,
target, runtime, material default or consumer behavior and adopts no Proposed
SPEC-HAL-363/364 clauses.
Verification establishes conformance and evidence; it does not admit another
source, ABI, target, runtime, capability or consumer, or lift an unresolved
admission. Such extensions require a separate governing-owner decision.

## 1. Purpose

SPEC-HAL-266 correctly makes DPR a per-surface renderer authority and forbids
DPR-dependent layout. SPEC-HAL-295 nevertheless sizes custom offscreen textures
during layout from `GpuResources::scale_factor`, a single primary-window
snapshot. The live Widget contract has no mutable, target-aware boundary after
final layout and before immutable paint.

That gap makes a custom renderer unable to satisfy all three requirements at
once:

1. layout remains purely logical and target-independent;
2. offscreen resources match the current target's physical pixels;
3. paint remains immutable image emission with no allocation or GPU work.

This amendment introduces one opt-in participant-local callback at the live
post-layout, pre-paint boundary. The engine supplies an immutable per-target
metrics snapshot and retains all DPR and logical-to-physical conversion
authority. This is not a new all-Widget frame phase.

---

## 2. Ownership of This Amendment

SPEC-HAL-266A owns:

1. the timing of target-metrics delivery to custom-render participants;
2. the immutable metrics supplied at that boundary;
3. renderer-owned logical-to-physical extent and pixel-coverage conversion;
4. per-target isolation and invalidation;
5. the prohibition on layout-time or global-DPR custom resource sizing;
6. structural absence when no participant is registered.

SPEC-HAL-295A owns permission to hold and operate on GPU resources.
SPEC-HAL-263A owns the alpha contract of any front view published as an Image.
This amendment grants neither permission and redefines neither contract.

---

## 3. Normative Definitions

### 3.1 Final Logical Rect

The final logical rect is the target-local absolute paint rect held by the
`WidgetTree` after layout, absolute-position propagation, viewport propagation,
and every post-layout stabilization step have completed successfully for the
current application frame. Cached geometry remains final when `layout_tree()`
legally reuses a clean root.

If post-layout synchronization defers layout to another frame, no participant
callback runs from the incomplete frame.

### 3.2 Target Metrics Snapshot

For one custom participant and one target, the engine supplies an immutable
snapshot containing at least:

```text
SurfaceId
final logical rect
final logical viewport
target logical extent
current positive finite effective ratio E (existing DPR field)
target physical extent
physical render extent
physical composition coverage, or Inactive when no target pixel is covered
```

The snapshot is derived from the same per-surface renderer state used by that
target's `SceneBuilder`, compile, scissor conversion, and execution.

HAL-024 E.3 owns App revision/admission and effective readback. The target snapshot
must correspond to that admitted basis; participants receive the final ratio and
physical extents, not a separate App multiplier. Equal effective metrics retain
existing cache reuse; a changed App revision is not permission for resource churn
when its actual target dependencies are unchanged.

### 3.3 Physical Render Extent and Composition Coverage

The **physical render extent** is the target-density texture extent for the
participant's complete final logical width and height. It owns offscreen texture
allocation and participant-local viewport dimensions; it has no absolute target
position.

Each axis is the nearest integer to `logical extent × DPR`. This is a local-size
density projection: fractional target position, clipping, and occlusion cannot
resize the participant's complete offscreen resource.

The **physical composition coverage** is the half-open integer target-pixel
region covered by the participant's final logical rect after target bounds and
the final logical viewport are applied. It owns visibility and target-pixel
alignment. It does not silently crop or stretch the participant's full-UV Image.

These values are distinct. The current `CustomRenderPass::PhysicalRect` stores
`x` and `y`, but live offscreen texture creation consumes only `width` and
`height`; that storage shape is not proof that one rect can own both allocation
extent and target coverage.

For an unclipped rect, pixel-center phase can make composition coverage one
pixel narrower or wider than the nearest-integer render extent. That difference
is raster sampling phase, not destination scaling: the production Image quad
still spans the final logical rect with full `[0, 1]` UVs, and source texture
dimensions do not change its vertices or target coverage. Deriving allocation
from coverage would instead resize or resample the full logical content merely
because its fractional origin moved, or would crop it when visibility changed.

**363/364 scoped amendment:** Moving a presentation mask or occurrence SHALL NOT resize the
participant resource to its crop, rewrite full-UV interpretation or fabricate
another target-metrics snapshot. Full content extent and visible coverage remain
distinct under SPEC-HAL-364.

### 3.4 Custom Render Preparation

Custom render preparation is an opt-in participant-local capability callback
invoked by the engine after final layout and before retained paint. It permits
an admitted SPEC-HAL-295 participant to mutate only its GPU cache and lifecycle
state using the immutable target metrics.

It is not a general Widget update, layout, or paint callback.
It receives no Timeline or animation mutation/sampling capability. When animated
state is required, the owning widget samples SPEC-HAL-021 during the earlier
Widget update phase and publishes a complete immutable value/revision that
preparation consumes as an already-authorized input.

**HAL375 §10 sole-binding companion [S1]:** An admitted HAL375 viewport may include one opaque immutable frame-scoped presentation companion in its sole existing per-target Image publication. The outcome associates it with the same `CustomRenderAdmission` identity, `SurfaceId`, texture and current front generation, and exact renderer-owned target snapshot. The companion carries qualified data and resource custody only. Its ingress and handler are sealed to the engine for HAL375 §9. The participant receives no renderer, atlas, DrawPlan, RenderGraph, compositor, execution, or semantic authority. No additional Image, participant, surface, public extension API, or binding key is admitted.

**Scoped optional Button SDF requirements — 2026-10-25:**

Section 4's scoped material commit belongs to the engine/resource publication
owner, not the participant's preparation capability. It grants no additional
Widget Update, semantic mutation, Timeline access or ordinary Button participant
status.

**Button material resource producer.**
SPEC-HAL-295A §3.3 assigns this existing non-Widget producer to the canonical
App-runtime/renderer-resource owner, qualified by atomic K and the Button's
TreeWidgetId mount lifetime. It does not use the WidgetTree participant marker or
§3.5's node index. Preparation consumes the same immutable renderer-owned target
basis and already-published inputs under §4's staged order and material-commit
requirements.

Ordinary Button gains no custom-render participant status, second Update,
Timeline access, Paint-time resource work or all-Widget discovery. Generation
protection, explicit unregister-before-drop and certified retirement remain
mandatory. No additional producer mechanism, GPU lease/fence API, runtime or
execution admission is supplied by this ownership contract.

### 3.5 Registered Participant Set

The `WidgetTree` maintains a typed `NodeId` index of admitted custom-render
participants, populated during node insertion and pruned during node removal.
Only those indexed nodes are visited. The engine reads each node's final
geometry directly and invokes its mutable participant capability directly; it
does not rediscover participants by traversing every Widget.

Index admission requires the opaque SPEC-HAL-295A `CustomRenderAdmission`
public opt-in marker. Implementing `CustomRenderParticipant` alone cannot
register a Widget. The marker records deliberate participation only; it does
not authenticate a Device or Queue or waive any SPEC-HAL-295A lifecycle
invariant.

---

## 4. Boundary Ordering

**Button material ingress and execution.**
The existing material request is published in the one ordinary Update; it cannot
activate selection post-layout. Private source/ABI admission is separate from
resource dispatch and current Ready evidence. Staged post-Paint registration,
next-Update consumption, the post-preparation/pre-Paint commitment, live-mount
checks, strict late-failure validation and independent content/focus geometry
remain unchanged. Neither source availability nor compilation proves execution.
For the retained WidgetTree target being built by canonical `App::run` (the base
surface in the live runtime):

```text
SPEC-HAL-042 captures one frame time, completes pre-routing scroll/input, and
advances Timeline once
    ↓
event routing may start/cancel animation handles
    ↓
Widget update samples Timeline read-only and publishes immutable owner state
    ↓
logical layout reaches its final result
    ↓
post-layout projections / anchors / reveal / scroll synchronization stabilize
    ↓
deferred-layout frames return without preparation
    ↓
layout-transition binding and visual-rect interpolation complete
    ↓
engine iterates the typed participant NodeId index only
    ↓
engine snapshots SurfaceId + final geometry + target metrics per participant
    ↓
participant-local preparation mutates GPU cache/lifecycle only
    ↓
front-view registrations needed by this frame are queued; the aggregate result
records whether front publication changed
    ↓
retained paint materializes the scene, reusing cached fragments where valid
    ↓
renderer drains registration state before compile
    ↓
compile / RenderGraph execute / present
```

The live insertion point is inside `build_scene_reusing()` after
`LayoutAuthority` has ended, after its deferred-layout exit, and before
`paint_tree_reusing()`. It runs even when clean layout geometry was reused.

Front publication does not require local Widget repaint when the participant's
stable `texture_id`, Image geometry, and Image semantics are unchanged. Instead,
the aggregate preparation outcome MUST:

1. force scene materialization for that frame;
2. contribute to `SceneBuildOutcome::visual_changed` (or an equivalent explicit
   presentation signal); and
3. therefore contribute to `App::run`'s `should_present` decision.

`front_publication_changed` is a presentation result, not a Widget invalidation
request or a second semantic authority. It MUST NOT dirty update or layout,
schedule a second update/layout pass, or grant the participant any update,
layout, paint, scene, or compositor mutation capability.

`paint_tree_reusing(force_scene = true)` may materialize the SceneGraph from the
clean retained root fragment; `build_fragment()` returns that cached fragment
without repainting or descending through the tree. This preserves retained
fragment reuse and avoids a second full traversal. If scene-visible image
identity, geometry, tint, or UVs change, ordinary paint invalidation is still
required.

The metrics snapshot remains immutable through callback, retained paint,
compile, and execute. A platform/App scale change admitted after the snapshot takes
effect in the next application frame and invalidates affected target-specific
custom resources there. Runtime child-window modal scenes are not additional
WidgetTree targets and are not swept by this callback.

---

**Button material publication and commitment.**

For an explicit procedural Button request, final-layout App qualification
projects F through the actual scene root's ancestor scroll scopes. Each child
clip is projected with its enclosing offsets before its owner's own ScrollNode
offset is added. The renderer derives activity and physical composition coverage
from the resulting footprint and current target bounds. K includes the composed
face and clipped logical footprint, including fractional placement. The
canonical unscrolled F still owns material allocation, expanded geometry E and
Paint. Scene traversal applies the scroll and full clip stack to the emitted
Image. An offscreen face has no current material eligibility.

Existing registration draining remains after retained Paint. Frame A's ordinary
Update supplies prospective material inputs; authorized final-target preparation
queues a front and admission request. Paint A SHALL NOT consume queued evidence
as Ready. The existing post-Paint drain registers, admits the actual generation
and notifies completion, without splicing it into that assembled scene. Frame B's
single ordinary Update consumes and validates completed evidence before requested
selection admission. Final layout and per-target preparation then verify the
complete current K/front. Changed target/revision or re-registration requires
fresh evidence. Only the scoped procedural-content qualification below may
establish fresh current-K eligibility for an already admitted unchanged generation
without producing or registering another front.

A NEW scoped engine-owned commit boundary SHALL follow that preparation and
precede retained Paint, atomically freezing K plus Ready/Unavailable into the
owning Button's paint-facing snapshot. It projects the already Update-admitted
request; it SHALL NOT activate selection post-layout, rerun Widget Update,
mutate semantic/control state, sample Timeline, prepare in Paint or assume an
ordinary Button is a CustomRenderParticipant. This bridge is not supplied by
the current preparation outcome or texture-ID table and requires explicit
implementation admission. Stable inputs reuse their valid front, not re-register
it each frame.
**Already-admitted explicit-request F-size replacement.**
Only after the existing applicable initial resource and first-target-output lifecycle has completed, when the attempt's single ordinary Update continues an already admitted explicit Button SDF request using its already-admitted Rest/Pressed pair, with unchanged owner/mount, SurfaceId, source/profile, theme/material basis and admitted effective ratio, changed final canonical face SIZE may receive a fresh complete currently admitted Rest/Pressed pair in this attempt's existing post-layout/pre-Paint resource boundary. Resulting current origin, window/viewport, clipping and physical metrics are authenticated as part of the complete current K and consuming attempt. HAL-018 section 16's initial-reveal admission/settlement remains unchanged; no visibility query or hidden-startup permission follows.
The existing producer completes every writing Queue::submit call, swaps frozen immutable fronts, and the existing registrar authenticates/registers a genuinely NEW content generation for full current K/current attempt before the existing atomic commit projects the already Update-admitted request and current endpoint. Neither endpoint is Ready before genuine complete-pair admission. No second Update/layout, selection activation, semantic/Timeline mutation or Paint resource work follows. Initial activation and every change outside this narrow exception retain their existing staged A/B semantics. No new default, state, scale, source, API, caller topology, fallback or old-generation relabelling follows.
Ordered producer writes may remain GPU-pending at that CPU publication under INV-295A-008. The same actual authenticated writer record survives its producer/reservation-to-original-G association without reset; submission return is not completion. Outstanding unsubmitted producer frames or uncertain writer submission/cancellation prevent Ready. Existing bounds and the independent writer/CPU/sampling retirement requirements below remain strict.

**Pending active-size replacement output - same coupled exception.**
Only while that already-admitted explicit request remains selected and its current target/coverage is active under the existing rules, if no complete authenticated current pair exists at the pre-Paint cutoff solely because lawful preparation/admission or certified retirement/capacity is pending, the engine SHALL defer that target's dependent retained Paint and fresh output rather than emit caption/other roles without the required replacement face. This is target-output Pending, not Ready, a third material outcome or fabricated resource failure/revocation. No stale retained scene/front or alternative material may be submitted as current.
Ordinary input/Update, preparation, certified retirement observation and existing wakes/retry ownership continue; independently eligible targets are unaffected. There is no second Update/Paint in an attempt or blocking GPU wait. A newer target snapshot invalidates the pending attempt's evidence; any successor requires fresh full-K/attempt authentication and cannot replay an old reply or splice a result into an assembled scene.
Resume dependent commitment/Paint/output only with a genuine complete fresh current pair, or end this scoped gate when the request is withdrawn/superseded, its owner is unmounted or target/coverage becomes inactive under existing rules, or an actual resource-owned failure invokes the unchanged Unavailable/late-failure law. Genuine Unavailable still exposes MATERIAL_UNAVAILABLE and its real cause and retains existing optional omission behavior. Existing inactive-target retirement, initial activation, prepared/product precedence and public API semantics remain unchanged. This is no universal required-resource API, new material default or all-surface readiness barrier.

Failure known by the commit cutoff SHALL commit Unavailable, never stale Ready.
Failure discovered after cutoff SHALL NOT rewrite the frozen snapshot, trigger
late Update or introduce Paint-time resource polling. Existing final Compile
generation/availability validation remains strict: Debug INV-IMG-005 and Release
rejection/logging are unchanged, with no substitution or visual-success claim;
owner-visible settlement occurs in the next authorized publication cycle.

**Generation custody across the frame lifecycle.**
The Button material obeys SPEC-HAL-295A §3.3's dual CPU/GPU generation custody.
Retained scene/draw-plan assembly begins CPU custody; only terminal no-submit
cancellation or actual sampling submission hands it off. The post-submit callback
certificate governs eventual retirement. Compile, encoding, presentation and
frame advancement are not substitutes for that certificate. The post-Paint
registration drain SHALL NOT revoke a generation under either custody.

Except for the coupled already-admitted explicit-request F-size replacement and temporary dependent-output gate above, existing phase order and staged publication remain unchanged. No second Update, Paint-time resource poll or additional execution admission follows.

**Rounded gold Button profile: enabled continuity (HAL-012 §7.1, scoped owner amendment
2026-11-18).** Enabled is one C and one checked basis revision in K. Its
private producer renders Rest, Hover, Rest with Pressed, and Hover with Pressed
in four bounded cells of one immutable texture. Admission certifies the whole
generation before Ready. Ordinary Update publishes current hover and press
through the separate interaction revision. The existing pre-Paint cutoff uses
the currently certified K and selects its exact UV cell. Hover and press
changes require only owner-local paint invalidation and retain that generation.

Already-admitted Enabled generations use the same-basis F-size replacement and
Pending rules above for all four cells. Availability switches to and from the
single static Disabled basis use ordinary staged publication. Current
eligibility and generation custody apply to the entire texture. The rounded gold
profile uses the current K's exact finite target
ratio from 0.5 through 4.0 for its one-physical-pixel outset, SDF and per-cell
UV crop. A changed ratio requires current eligibility and a new material source
generation. Reject ratios outside that range before allocating material
resources. The prior 1x and 2x profiles keep their existing admission and
mapping.
Uncertified old-front swap, failure transport, integration or bounded shutdown
retirement cannot be reported complete.

**Prepared endpoint execution.**
The existing endpoint-pair mechanism uses atomic K with the stable material-basis
revision independently of the current checked press request revision. Both
complete endpoints are rendered in the same authorized preparation into one
bounded two-cell target. Neither cell is Ready before the whole frozen front has
genuine completed admission. Initial optional post-Paint draining and cold frame commitment remain unchanged. Only the already-admitted explicit-request F-size replacement exception above may genuinely admit a fresh complete pair before retained Paint; its pending-output gate is inseparable.

An unchanged current K/binding needs no producer, registration or GPU-completion
cycle for a press transition. Pre-Paint consumes the current ordinary Update
snapshot and freezes its exact endpoint UV plus checked request revision into
owner-local paint input. Changed UV/request projection invalidates only that
owner and necessary ancestor composition. An interaction revision alone does
not change the resource basis. Changed profile/theme, mount, surface or actual
target/geometry basis requires fresh current eligibility with no stale
continuation. Resource production and admission remain required unless the
scoped unchanged-content qualification below establishes fresh evidence for the
complete current K. Late-failure/strict Compile rules, one Update, participant
absence, Paint purity and whole-generation custody remain unchanged.

**Procedural Button content and current-target eligibility.**

These provisions apply only to the already-admitted renderer-owned procedural
Button producer and its existing supported source, ABI, material, target and
runtime paths. They admit no new producer, participant, consumer, scale, material
family, public API or generic cache.

K remains the complete current eligibility key defined by SPEC-HAL-012. All
existing members retain their meaning and equality requirements. No target
extent may be normalized, omitted or treated as equal when constructing,
comparing or committing K.

The existing resource owner records immutable content basis C with the exact
originally admitted generation and its production provenance. C identifies all
actual inputs to the finite prepared endpoint contents: admitted source/ABI,
authored material revision and resolved values, canonical face size, effective
ratio, finite effect domain and source-cell mapping, endpoint storage/format
and other actual pixel-generation dependencies. The record also retains exact
owner/mount, SurfaceId and baseline device/queue identity. C proves what was
produced; it supplies no current target eligibility.

Equality of C means exact equality of its declared pixel-generating inputs and
admitted source identity. A hash match or visually equal output alone is not
sufficient. C is neither an ImageBinding identity/generation nor K, and cannot
serve as an admission or target-eligibility credential.

Only changes confined to logical and physical window extent, translation of the
canonical face F without changing its size, and the consuming viewport or
ancestor scroll placement described above may qualify under this extension.
These are current placement and visibility changes, never equality of old and
current K. At the existing
authorized post-layout, pre-Paint preparation boundary, the engine/resource owner
obtains and validates the consuming attempt's fresh complete renderer-owned
target snapshot.

The owner must prove unchanged owner/mount, SurfaceId, admitted source/ABI,
material basis and resolved values, canonical face size, effective ratio,
physical render extent, finite LOCAL effect support, endpoint storage/format,
source-cell/UV mapping and baseline device/queue validity. All exact inputs that
actually generate the immutable texture, including complete endpoint uniform
values and source-cell alignment, must match its original C record. Absolute
face origin, consuming viewport and resulting composition coverage may be
excluded from C only where the existing admitted producer does not consume them
to generate pixels. An actual source dependency on any such value cannot be
excluded or normalized by this exception.

The current F INCLUDING ORIGIN, final viewport, target extents, activity and
physical composition coverage must instead be freshly derived and authenticated
as part of complete current-K eligibility. Current target metrics independently
pass existing finite, positive, supported-size and resource-limit checks.
Inactive targets do not qualify; their existing rejection, omission and
retirement rules remain unchanged.

The same certification must authenticate the CURRENT finite destination effect
domain derived from current F and the unchanged admitted local effect support,
together with the exact existing source-cell mapping and Update-published
endpoint selection. That destination and the current canonical scene clipping
must be used by the consuming Paint/Compile/Execute attempt. Original production
placement, a prior destination rectangle, prior viewport or prior composition
coverage cannot be reused as current placement evidence. Fractional translation
retains the renderer's current raster-phase and physical-coverage conversion;
it neither changes layout/hits nor normalizes physical edges. Existing scene
traversal remains the sole owner of canonical ancestor clipping under
SPEC-HAL-024; this qualification cannot bypass, replace or retain an old clip.

The owner must additionally authenticate the exact original admitted
ImageBinding identity/generation, current registration and availability,
immutable-front protection, and generation custody. Known or queued generation withdrawal or resource retirement
prevents qualification. Expiry of old target eligibility alone is not generation
withdrawal and cannot by itself revoke compatible immutable content. Qualification
cannot cancel a genuine resource withdrawal or retirement already in progress. Queued/unadmitted, revoked, replaced, writable or
uncertified generations cannot qualify. Pixel resemblance or a texture ID is
not a certificate.

Successful certification SHALL originate from the existing resource owner,
using the exact original authenticated generation admission and independently
current renderer/registration state. It binds the complete current K, consuming
target-snapshot provenance, immutable C, original admission and exact ImageBinding
identity/generation with its existing sampling semantics. Availability, copied
metadata, a matching texture ID or matching pixels alone do not supply authority.

At the existing pre-Paint commit cutoff, the owner SHALL authenticate that
evidence against the current live mount, SurfaceId, material basis, complete
target snapshot and protected generation. It retains original production
provenance and admission unchanged. It cannot relabel a reservation, re-register
the texture, overwrite content, manufacture a generation or treat old K as
current evidence. Only this freshly verified evidence supports current Ready.

Certification is restricted to the consuming attempt's current immutable
preparation publication. Superseded or cancelled evidence cannot be replayed,
including after resize A-to-B-to-A when K values repeat. Existing snapshot/
attempt provenance is used; no new global clock is implied. Ordinary unchanged-K
retained reuse remains unchanged. Interaction request revision remains separately
checked for current endpoint projection and does not enter material K.

A producer completion for obsolete K is not current Ready evidence and cannot
admit a generation for current K. Only a genuinely already-admitted, still-valid
immutable generation may be considered for fresh certification. Queued or
unadmitted completion cannot be retroactively authenticated by equal C. Late
producer results, eligibility evidence or failures SHALL NOT replace a newer
commitment, revive an unmounted owner or splice evidence into an assembled scene.

Qualification performs no producer GPU work, submission or GPU-completion wait.
It uses the existing bounded owner record and current publication path, not a
global registry, cross-owner resource cache or all-widget traversal. One
current eligibility publication suffices; retained uses remain protected by
exact generation custody, not an unbounded history of eligibility records.

A changed generated-content dependency requires fresh production/admission, never unchanged-C qualification. Only the already-admitted explicit-request F-size replacement above may perform that fresh production/admission in the same attempt; all other cases retain existing staged preparation/admission. Missing certificates, incompatible or inactive targets and genuine resource failure keep their existing rejection/failure rules, subject only to that exception's expressly scoped temporary pending-output gate. Qualification absence is rejected current evidence, not fabricated producer failure or binding revocation. No stale content, placement, clipping, last-good continuation or placeholder follows.

A later target change cannot rewrite the frozen eligibility snapshot; it is
handled in the next authorized attempt. Paint stays immutable Image emission.
Strict late Compile generation/availability checks remain. Qualification cannot
activate selection, perform a second Update, sample Timeline or mutate semantic,
input, layout or other-widget state.

**Plot active size replacement Pending.**

This rule applies only to the existing Plot CustomRenderParticipant in the canonical
App retained WidgetTree target. The first eligible replacement SHALL be caused by a
changed nonzero physical render extent from final logical Plot sizing, as in a
SplitPane resize. The exact concrete Plot mount SHALL remain live on the same
renderer-owned SurfaceId, with active nonempty composition coverage. A genuine
authenticated registered Plot front for that mount and surface SHALL have been
included in an earlier successful target submission and still have been eligible
immediately before this replacement invalidated it. An accepted
exact current async Plot producer job or its exact front registration SHALL remain
outstanding at the cutoff. A historical successful front is evidence of prior
eligibility only. A cold or first front, an unchanged-extent relayout without an
eligible replacement, and an ordinary same-key data update do not start this gate.
The size predicate is derived from existing final target metrics and needs no
SplitPane input flag.

For a still-active replacement, the qualified continuity of the same mount and
surface SHALL survive subsequent eligible resizes until release or terminal
settlement. Keep one current replacement identity and the minimum prior-presentation
eligibility fact for that mount and surface. Supersession SHALL replace the current
request and withdraw the obsolete evidence. A changed render input during that
replacement also requires a fresh current request. The request identity SHALL distinguish
the actual replacement and submission from all unresolved predecessors, even when
the target sizes repeat A → B → A. Bind the exact current target resource key,
engine snapshot and coverage, producer and series generations, and immutable render
inputs including ViewState and the optional market-reference association. Extent
equality, an equal texture ID and a stale or repeated generation cannot prove
identity. Revalidate unchanged requests without reallocation, resubmission or
duplicate registration. An ordinary same-key update with a valid coherent registered
front remains presentable through the existing path.

After final layout and before immutable retained Paint, the Plot participant SHALL
return private typed active-resize-pending evidence in its existing
CustomRenderPreparationOutcome only for the qualified outstanding request. The
indexed WidgetTree preparation owner SHALL authenticate the concrete Plot node,
admission identity, live concrete mount and tree lifetime, SurfaceId and current
canonical attempt and metrics. It SHALL pass only this authenticated target
disposition to the existing scene-build/App owner. Invalid evidence cannot defer
output. The participant receives no output, renderer or other-widget authority. The
evidence is sealed to Plot and supplies no public constructor, general participant
readiness flag or additional binding.

At the existing pre-Paint cutoff, the App owner SHALL defer fresh dependent Paint
and scene commitment for the affected SurfaceId while any such Plot remains Pending.
Build, Compile, Execute, submit and present SHALL exclude that surface for the
attempt. An image-less scene with axes, tags or siblings, an empty replacement
output, and resubmission of a retained scene are all ineligible. Prior actual OS
scanout may remain visible. It supplies no Image, binding or current output claim.
Independent surfaces proceed through their normal paths. Ordinary input, Update,
final layout, async Pool work, registrar service and existing wake continue on their
existing cadence. Pending itself schedules no extra pass, polling, blocking wait,
sleep, scheduler, worker or GPU producer.

The existing Pool and CustomRenderPass own producer execution, the same baseline
Device and Queue, and submit then swap under HAL-295A INV-008. The Plot publication
owner SHALL consume the completion in ordinary Update and stage the immutable
view/reference result with the exact request. Preparation SHALL queue the actual
produced front view for the existing renderer registrar and obtain exact-view
admission evidence through the existing
Image admission owner. Service that request via the existing requested-registration
drain when the attempt has no output to compile, or the existing drain for an
independent eligible output. A queued request remains Pending. The real registrar
result uses the existing wake and is eligible at the next ordinary attempt's
pre-Paint cutoff. The registrar and admission owner SHALL verify the exact view
and registration generation. The owners SHALL check request currency at completion
handoff, registration enqueue and drain, and the pre-Paint cutoff. A superseded
queued registration SHALL NOT replace a current entry under a reused texture ID.
The participant SHALL NOT infer registration from submission, swap or texture ID
alone.

Release Pending only when the current request's writes have been successfully
submitted on the baseline Queue, its exact swapped front is authenticated and
registered and remains available for that attempt, and its target, series,
ViewState, Y range and optional exact MarketDetailLink/reference all match the
same immutable producer result. At the cutoff, the existing Plot presentation and
scene owners SHALL commit the paint-facing Image and associated rendered view and
market presentation together. Until then neither a staged result nor a superseded
result may publish the successor view/reference. The market presentation projection
is existing output metadata. Requested view, series and link inputs stay with their
ordinary owners and phases. No Update or Layout runs again for the commitment.

The next eligible scene SHALL contain the complete Plot and its labels from the
same qualified snapshot, including the matching view and price/reference projection.
Use the existing ordinary paint invalidation for any changed local Image identity,
visibility, geometry or price/reference projection and its affected retained
dependencies. For a stable scene-visible Image with changed registered content,
use INV-013's materialization and presentation signal. A valid unchanged fragment
can be reused. Force-scene alone cannot reuse a fragment naming a withdrawn Image
or carrying an obsolete projection. Paint only reads its committed projection and
emits ordinary Image and text. The existing renderer generation checks still apply.
Actual loss discovered after cutoff SHALL preserve the frozen snapshot and use
the existing strict failure/cancellation path without reporting a complete
successful target output. It SHALL settle at the next authorized owner boundary.

Plot removal, unmount, concrete replacement, inactive or zero coverage, SurfaceId
switch, and actual source, producer, device, capacity or publication failure SHALL
end this gate for the exact affected owner. Withdrawal and failure discard its
eligibility, prevent publication of stale completions and invalidate any retained
reference to the retired front before ordinary output resumes. Preserve the
existing typed refusal or unavailability cause and ordinary omission on terminal
failure. Producer acceptance and terminal outcomes SHALL be accounted for on the
existing owner handoff, including rejected enqueue, non-producing job and
registration rejection. Mere lack of a reply is neither readiness nor a fabricated
failure. A superseded result cannot cancel a newer request. A new mount or surface
must establish its own ordinary first front.

The gate owns no extra texture or sampling lease. Existing resource owners SHALL
cancel obsolete unpublished work where terminal no-submit is known and keep
submitted or uncertain work in its actual custody. Unregister before final drop
and preserve already-admitted uses and their Queue ordering. No withdrawn Image
is eligible for new use. Bound pending state and admission requests to the current
replacement, without per-resize histories, additional texture pools or a bypass of
existing capacity or retirement requirements. The coupled Button SDF rule remains
separate and retains its exact scope.

## 5. Renderer-Owned Pixel-Coverage Conversion

Live renderer behavior is narrower than the original draft assumed:

- Image quad edges remain logical until `Globals::view_proj` maps them to the
  physical target;
- ordinary 2D passes use the full target viewport; there is no per-Widget GPU
  `set_viewport` conversion;
- `FrameConstants::clip_rect_to_scissor()` currently clamps in logical space and
  rounds origin and extent independently; and
- its test helper manually mirrors that algorithm instead of sharing one owner.

Duplicating the current `round(origin × DPR)` / `round(extent × DPR)` code in
this amendment would canonize a second conversion without proving that it covers
the same pixels as the production Image quad and clip. This amendment therefore
defines an equivalence law, not an independent arithmetic clone.

Given one immutable target snapshot and final participant geometry:

1. every logical input must be finite; DPR must be positive and finite; target
   physical dimensions must be nonzero;
2. the engine derives the full logical render size from the final logical rect
   and the visible logical coverage from intersection with the target bounds and
   final logical viewport;
3. one renderer-owned conversion implementation derives the origin-independent
   nearest-integer physical render extent, the absolute-edge physical
   composition coverage, and GPU scissor/viewport values that represent those
   logical regions;
4. the physical composition coverage MUST equal the target-pixel coverage of
   the production Image quad under the same `Globals::view_proj`, target bounds,
   and clip state; origin and extent may not be rounded independently unless
   production-raster evidence proves that policy has this equivalence;
5. the same production helper, not a copied test oracle, MUST serve
   `FrameConstants::clip_rect_to_scissor()` and custom preparation;
6. empty logical render size or empty physical composition coverage yields
   `Inactive`; the engine does not allocate a synthetic 1×1 custom target; and
7. participants receive the final extent and coverage values and cannot repeat,
   reinterpret, or override the conversion.

The shared helper is the sole renderer coordinate-conversion owner and operates
only on immutable value inputs. `FrameConstants::clip_rect_to_scissor()` and
custom preparation are sibling consumers of its value outputs; custom
preparation must not receive or depend on `FrameConstants`, `PassContext`, a
RenderGraph node or attachment, or any other renderer-private graph state.

Until the shared renderer helper and fractional-edge production proof exist,
the current scissor function is live behavior but not sufficient adoption
evidence for this amendment.

---

## 6. Binding Invariants

### INV-266A-001 — Post-Layout, Pre-Paint Timing

The participant-local callback runs only after final logical layout, all
post-layout stabilization, and the deferred-layout exit, and before retained
paint for the same target and application frame. It must not run from inside
layout or paint.

### INV-266A-002 — Renderer-Owned Target Authority

`SurfaceId`, DPR, logical target extent, and physical target extent come from
the renderer's per-target authority. A Window, OS query, cached primary-window
value, or participant-chosen value is not a legal source.

### INV-266A-003 — One Immutable Snapshot

Preparation, SceneBuilder/PaintSink, compile, and execute for a target use one
coherent target-metrics snapshot. DPR, physical render extent, and composition
coverage cannot change midway through those operations.

For the procedural Button placement/viewport changes covered by section 4, an
earlier-produced immutable generation requires fresh eligibility evidence for
this exact current snapshot AND its current destination/visibility projection.
Ordinary unchanged-K reuse remains unchanged. Original production provenance
never replaces current frame authority.

### INV-266A-004 — Layout Remains Logical

Layout receives and produces logical coordinates only. It cannot allocate,
resize, or choose physical custom-render resources from DPR. Final layout may
record logical geometry; it does not consume target metrics.

### INV-266A-005 — GPU Cache and Lifecycle Mutation Only

During preparation, a participant may:

- create, resize, retire, or reuse participant-owned GPU resources;
- update render-local GPU buffers, uniforms, and rebuildable caches from
  already-authorized inputs;
- encode or submit participant-owned offscreen work only as permitted by
  SPEC-HAL-295A;
- queue front-view registration or unregistration through the declared image
  lifecycle.

Already-authorized inputs may include an immutable widget-owned value/revision
published during same-frame Update under SPEC-HAL-021 and SPEC-HAL-354. The
participant cannot receive a Timeline handle/capability, sample or advance
Timeline, or derive and write a new semantic value back to widget/application
state.

It must not mutate layout results, WidgetTree structure, application/domain
state, interaction semantics, navigation, compositor intent, or another
participant's state.

**363/364 scoped amendment:** Preparation MAY cooperate with a SPEC-HAL-364 resource lease only
through existing GPU-cache/lifecycle authority and already-published immutable
inputs. It SHALL NOT decide choreography, mutate Timeline, defer semantic
teardown or introduce a second preparation callback/pass.

### INV-266A-006 — Paint Is Image Emission Only

Paint emits an `ImageNode` containing either a target-independent registered
texture identity or an opaque admitted target-bound presentation identity.
Paint never reads or chooses `SurfaceId` or an actual target front. During
per-surface `DrawPlan` materialization, the engine resolves a target-bound
identity through the immutable preparation bindings for the same target
snapshot; an absent binding omits that Image. Resolution performs no GPU or
lifecycle mutation and lowers to the unchanged ordinary Image primitive.
The ordinary absent-binding omission does not authorize successful dependent output for section 4's coupled Button SDF active-size replacement Pending or its scoped Plot active size replacement Pending. Each gate defers only the affected target's dependent commitment/Paint/output. Genuine Unavailable and every contribution outside these exact gates retain their existing rules. No proposed SPEC-HAL-363/364 provision or new Image capability is activated.

Paint performs no device query, DPR conversion, GPU allocation, resize, buffer
write, encoding, submission, registration, or lifecycle mutation.

**HAL375 §10 sole-binding companion [S1]:** Paint continues to emit the ordinary target-bound Image. The binding and prepared payload are immutable during Paint. After Paint, the renderer-owned DrawPlan materializer may record one occurrence receipt in that same binding record when the ordinary Image is actually inserted. The receipt identifies the exact inserted primitive in the current surface and build, including its final effective order, inherited clip and scale. The materializer writes occurrence evidence only. It never changes the binding, resource, placement, or target authority. An absent Image supplies no receipt. A second inserted occurrence for the same binding makes it ambiguous for the rest of the build. Freeze all receipts before Compile. Compile may use a unique receipt only while its exact Image survives ordinary Compile and registration checks for the current target.

**363/364 scoped amendment:** Ordinary absent-target omission remains unchanged. For a required
SPEC-HAL-363/364 contribution, missing current or historical binding instead
follows SPEC-HAL-364 B-9.8/B-9.9 complete settlement/unavailability; it cannot
silently omit a required Image.

### INV-266A-007 — Per-Target Resource Isolation

Target-dependent custom resources are keyed by participant identity and
`SurfaceId`. Concurrent targets with different DPR or physical extent cannot
share one mutable size authority or publish a front rendered for the wrong
target metrics.

Device-compatible immutable assets may be shared only when they contain no
target-dependent extent, transform, clip, or presentation state.

The scoped procedural-content qualification is confined to the same owner/mount
and SurfaceId. It admits neither cross-surface sharing nor one target's evidence
satisfying another target.

### INV-266A-007A — Target-Bound Presentation Isolation

A preparation outcome may publish at most one binding from its node's
`CustomRenderAdmission` identity to the already-published front for the
callback's renderer-owned `SurfaceId`. The engine verifies that admission
identity and owns and reuses the exact `(identity, SurfaceId)` binding table. A
binding for one `SurfaceId` can never satisfy another.

**HAL375 §10 sole-binding companion [S1]:** The engine authenticates the existing `(presentation identity, SurfaceId)` key and keeps the optional companion and occurrence receipt in the same frame-local record as the one Image binding. Invalid, inactive, absent, ambiguous, or stale companion evidence is ineligible for optional output. Ordinary Image eligibility retains its own contract. Compile receives read-only records. Release the frame records when Compile consumes them or the frame is abandoned. Only admitted queue-safe execution custody may outlive that frame's Compile. Ordinary non-companion publication and the existing `Copy` Image binding retain their meaning.

**363/364 scoped amendment:** The current per-target presentation table continues to contain
at most one current binding per admitted participant and SurfaceId.
SPEC-HAL-364 historical leases SHALL NOT append old fronts to that table, change
lookup semantics or let one target's binding satisfy another. Historical
occurrence binding uses a separate renderer-owned generation reference with
its own admitted target validity.

### INV-266A-008 — Canonical Conversion Is Engine-Owned

The physical render extent and composition coverage supplied by the engine are
final. A participant uses them for their declared purposes and target-specific
uniforms without recomputing them from logical values or DPR.

### INV-266A-009 — Idempotent Stable Metrics

Repeated preparation with unchanged participant inputs and identical target
metrics must reuse compatible GPU resources. It must not recreate textures,
buffers, bind groups, pipelines, or registrations merely because another
application frame began. A changed immutable owner revision is an explicit input
change; a new frame with an equal revision/value is not.

Window extent, face translation and consuming-viewport changes are not identical
target metrics. For the expressly scoped unchanged procedural-content case,
complete current-K eligibility and current placement/visibility are freshly
qualified while the immutable generation is retained without recreation,
re-registration, producer GPU work or producer-completion wait.

### INV-266A-010 — Explicit Invalidation

A change in `SurfaceId`, DPR, final logical rect, final logical viewport,
physical render extent, physical composition coverage, target physical extent,
texture format, or baseline device validity explicitly invalidates every
affected target-dependent resource or visibility decision. Rebuild and
publication must complete before paint exposes a front for incompatible render
metrics.

If no valid front exists, ordinary optional paint omits the Image or uses a separately authorized non-GPU placeholder. It must not sample a mismatched front. Only section 4's coupled Button SDF active-size replacement Pending and scoped Plot active size replacement Pending instead defer dependent target commitment/Paint/output under their respective predicates. Genuine Unavailable retains its actual failure/omission law. No new placeholder is authorized.

Changed window extent, face origin or consuming viewport invalidates prior
procedural Button target eligibility and every affected placement/visibility
decision even when immutable content remains compatible. Only fresh
qualification under section 4 may establish current eligibility and current
placement without rebuilding that content. Changed generated-content
dependencies still require fresh production/admission. Generic Image and other
participant paths, existing omission and separately authorized placeholder rules
are unchanged; this scoped qualification does not expand them.

**363/364 scoped amendment:** The ordinary omission/authorized-placeholder alternative above
does not admit partial occurrence success. Required SPEC-HAL-363/364 content
uses its explicit complete scene settlement or surface-unavailability outcome;
no mismatched front or implicit placeholder is permitted.

### INV-266A-011 — No All-Widget Traversal

The engine invokes preparation only for the registered participant set. When
that set is empty, it performs no custom participant traversal, metrics record
construction, GPU cache work, registration work, or submission.

### INV-266A-012 — Global Scale Metadata Is Non-Authoritative

`GpuResources::scale_factor` or any equivalent single scalar cannot size,
resize, transform, or validate target-dependent custom resources. If retained
for compatibility during migration, it is explicitly non-authoritative and
must not participate in conforming custom-render behavior.

### INV-266A-013 — Retained Reuse and Presentation Are Explicit

A changed front under a stable target-bound presentation identity does not dirty
local retained paint. The callback outcome instead forces cached-scene
materialization and presentation for that application frame. The engine must
neither skip the frame because the root fragment is clean nor repaint/descend
through the whole WidgetTree merely to observe the new registered view.

**363/364 scoped amendment:** Historical-generation validity changes invalidate only dependent
occurrences and necessary ancestor composition. Current-front publication still
uses the existing materialization/presentation signal without repainting
unchanged widgets. INV-266A-003/007/014 coherent target isolation and same-frame
Update inputs, and INV-266A-011 no-global-scan behavior, are unchanged.

### INV-266A-014 — Same-Frame Update-Owned Input

When Widget update publishes a changed immutable owner value/revision from a
SPEC-HAL-021 sample, preparation for that application frame consumes exactly that
revision after layout and before paint. It must not consume the previous frame's
revision, wait for paint, or query Timeline directly.

**INV-266A-G1-RECIPIENT-CUTOFF / FORBID-266A-G1-PREPARATION-FETCH.** For a changed
eligible complete flat request supplied by a separately mounted producer during
the same frame's single timed Widget update, the canonical App/WidgetTree owner
MUST complete the HAL-354 mounted immutable publication cutoff before Layout and
before this target's preparation. Its checked setter transfer MUST occur within
the existing Update authority after producer borrows end, with exclusive qualified
writer admission and requalified current concrete type/mount, even at a surviving
tree ID. Only the canonical linked cutoff may change the linked request; a direct
public setter attempt while linked MUST receive synchronous typed exclusive-writer
refusal with no requested/committed/target mutation. The linked transfer MUST use
the same full validation without bypass. Preparation MUST consume that exact
accepted immutable request and revision in the same admitted frame, rather than
the previous frame's value. Preparation MUST NOT fetch a producer, poll a shared
semantic model, invoke a semantic callback, sample Timeline, derive a replacement
view or write another widget/application state.

**INV-266A-G1-STAGE-TRUTH / FORBID-266A-G1-FALSE-SAME-FRAME.** Same-frame transfer
does not guarantee that preparation or display occurred. Deferred layout, inactive
or zero-area targets, admission failure, retirement and compatible last-good
failure MUST retain their actual stage and original qualified provenance.
Requested admission, per-target preparation, committed publication and display
MUST remain distinct. When ordinary preparation proceeds, its final immutable
metrics and existing target-local failure/retirement rules remain unchanged.
A missing same-frame post-preparation Screen callback MUST NOT cause a second
update or phase; owned prepared-target observation returns at the next ordinary
Screen tree boundary. Transfer results instead remain with the bounded canonical
APP-level link/result owner and remain readable by exact link identity after
Screen or mount removal. Its application-supplied finite positive aggregate cap,
reserved slot, consumption, retirement and shutdown rules in HAL-354 remain
mandatory. AcceptedForCutoff is pending; Transferred, Unchanged, Rejected, Stale,
Retired and narrowly qualified Indeterminate are terminal transfer categories,
not renderer stages. The synchronous setter Result MUST remain definitive;
preparation failure cannot turn it into Unknown or Indeterminate or rewrite it.

---

## 7. Forbidden Behaviors

The following are non-conformant:

1. reading DPR in `Widget::layout`;
2. creating or resizing offscreen textures from layout;
3. using `GpuResources::scale_factor` as current target authority;
4. reading the OS, Window, or a global renderer singleton from a participant;
5. recomputing physical extent or composition coverage inside a participant;
6. mutating semantic, interaction, navigation, or layout state during
   preparation;
7. allocating, resizing, encoding, submitting, registering, or unregistering
   during paint;
8. traversing all widgets to find custom participants each frame;
9. sharing one target-sized mutable front across surfaces with different target
   metrics;
10. painting a front whose recorded target metrics do not match the current
    snapshot, other than an unchanged admitted procedural Button generation with
    fresh complete-current-K eligibility under section 4 scoped qualification.
    This exception leaves existing target-independent registered Image and other
    participant paths unchanged;
11. treating preparation as permission to access renderer-private targets;
12. claiming this boundary for a non-`App::run` runtime without separate
    topology authority;
13. discovering participants through a post-layout all-Widget walk;
14. treating child-window modal scenes as retained WidgetTree targets;
15. copying the current scissor rounding formula into a second owner without
    production pixel-coverage equivalence; or
16. publishing a new front while allowing retained-scene and present gates to
    classify the frame as visually unchanged; or
17. supplying Timeline, an animation sampler, or a private clock to preparation,
    or deriving/writing animation-owned widget/application state there.

---

## 8. Literal Replacement and Parent Integration

### 8.1 SPEC-HAL-266

SPEC-HAL-266 FORBID-DPR-003 remains binding: layout has no DPR. Upon adoption,
the proposal backlink anchored at FORBID-DPR-003 becomes this normative parent
clause:

> SPEC-HAL-266 §§2, 4, 5, and 8 are read subject to SPEC-HAL-266A for an
> admitted custom-render participant. SPEC-HAL-266A owns the sole mutable
> post-layout consumer of full target metrics before paint. FORBID-DPR-003
> remains binding, and the existing PaintSink DPR projection remains read-only
> paint-time snapping authority, not custom GPU resource-sizing authority.

No existing SPEC-HAL-266 prose is mechanically replaced by this subsection.

### 8.2 SPEC-HAL-295

Upon adoption:

- INV-295-RSZ1 becomes:

  > Custom target-dependent resources are created or resized only during the
  > SPEC-HAL-266A post-layout preparation boundary from the engine-supplied
   > physical render extent. Layout records logical geometry only; paint emits an
   > Image only.

The proposal backlink in SPEC-HAL-295 then becomes this normative parent clause:

> SPEC-HAL-295 §§4.2–4.3, 4.8, 5, 7, and 13 are read subject to
> SPEC-HAL-266A for target-dependent custom-render preparation. Target sizing
> and resize follow SPEC-HAL-266A §4; `GpuResources::scale_factor` is not target
> authority; texture allocation consumes the engine-supplied physical render
> extent; target alignment and visibility consume the engine-supplied physical
> composition coverage; and resize acceptance requires the distinct-surface and
> scale-change proof in SPEC-HAL-266A §9.

No existing SPEC-HAL-295 prose other than INV-295-RSZ1 is mechanically replaced
by this amendment.

No custom GPU permission is added here; SPEC-HAL-295A remains its sole owner.

---

## 9. Contract and Proof Obligations

Adoption and implementation require source-grounded evidence for all of the
following:

1. a registered participant receives its capability callback after final
   layout/post-layout stabilization and before retained paint, including when
   clean root geometry is reused;
2. layout has no DPR input and performs no custom GPU allocation or resize;
3. paint emits an Image without GPU or lifecycle mutation;
4. DPR values `1.0`, `1.5`, and `2.0`, fractional origins/extents, negative
   origins, and right/bottom target edges produce physical coverage exactly
   equal to production Image raster + scissor behavior;
5. the production scissor path and preparation path call one renderer-owned
   conversion helper; a copied test helper is rejected;
6. two independent surface scenes containing the same retained target-bound
   Image identity, with distinct registered `SurfaceId`/DPR snapshots, resolve
   only their own distinct render extents and target-specific fronts when
   prepared in both A → B and B → A order, without claiming that current
   child-window modal scenes own WidgetTrees;
7. a scale-factor change invalidates and rebuilds only resources for the
   affected surface before publication;
8. unchanged metrics reuse existing resources and registrations without churn;
9. an empty or fully clipped logical rect produces `Inactive` and no synthetic
   custom texture;
10. non-finite or non-positive target metrics fail explicitly before participant
   invocation;
11. a participant cannot query Window/OS DPR or override engine-supplied extent
    or coverage through the preparation API;
12. participant membership requires the opaque SPEC-HAL-295A public admission
    marker, is maintained on tree insertion/removal, and callback dispatch
    touches only that typed `NodeId` index; implementing the callback trait
    without explicit opt-in does not enter the index;
13. with no registered participants, no custom traversal, metrics records, GPU
    resources, jobs, registrations, or submissions occur;
14. front publication forces scene materialization and presentation for the
    exact target while the retained target-bound Image node remains reusable;
    inactive preparation or participant removal omits only that target's Image,
    without local repaint or a full tree descent;
15. all published fronts satisfy the authoritative Image alpha contract (and
    SPEC-HAL-263A if adopted), while all GPU work satisfies SPEC-HAL-295A.
16. an event-started animation samples exact `from` during same-frame Widget
    update, publishes one immutable participant input revision, and preparation
    consumes that exact revision before paint with no Timeline capability and no
    one-frame lag.

The proof consumer must be generic. Plot migration may validate the contract,
but Plot is an implementation consequence, not the owner of this phase or its
acceptance law.

Proof of this scoped extension after contract adoption. Contract adoption does not grant implementation admission or establish source conformance. The following new transition proofs gate implementation acceptance and execution claims for this scoped extension. Previously binding adoption and proof conditions remain unchanged. Implementation work remains subject to explicit governing-owner authorization.

The scoped procedural-content extension must prove window-only resize and real
ScrollArea viewport resize, including a fixed-size face translated by ordinary
reflow, fractional face translation within existing admitted ratios, current
partial clipping and repeated K values after A-to-B-to-A. Every qualified
attempt must carry the fresh complete K and renderer snapshot, current F/origin,
viewport, physical composition coverage and destination effect domain.

With exact unchanged generated contents, it must show the identical originally
admitted ImageBinding identity/generation, continuous actual rendered face within
the current visible clip, and zero new producer preparation, texture/pass
allocation, registration, producer encoding or submission. Current placement
must be compared with a freshly produced equivalent at the SAME current
position, scale, viewport and raster phase; comparing different screen positions
or relying only on source equivalence is not sufficient. No material output may
bypass the current canonical clip or remain at the old position.

Existing changed-material, scale, face-size, incompatible mapping, mount/surface,
revocation, late-failure and target-isolation proofs remain mandatory. Actual
generated-content changes and changes outside this scope must not reuse a
generation. Inactive targets retain their existing rejection/omission behavior.

The proof must preserve exact CPU/GPU custody through held, cancelled and
submitted plans, reject obsolete producer completions and stale or cancelled
eligibility/placement evidence, including repeated K, and preserve the existing
terminal no-submit cancellation, submission handoff and completion requirements.
Device loss before and after commit cutoff must show truthful failure, no revived
Ready or fallback, unchanged frozen-snapshot rules and retention of uncertified
resources without fabricated retirement or recovery. No device recreation or
additional runtime/scale/state admission follows. Source equivalence or
color/pixel goldens do not substitute for these invariants.
**Scoped active-size replacement proof after contract adoption.**
Contract adoption is not implementation admission or source conformance. The following new proofs gate post-adoption implementation acceptance/execution claims for this exact extension. Existing proof obligations and historical results keep their original status; no inactive universal-default packet is adopted.
1. Establish the existing initial optional A/B pair and an actual initial target output before size changes; preserve initial request rejection/activation, public legacy selection/withdrawal, prepared/product precedence and HAL-018 initial reveal. Headless proof does not establish native reveal.
2. For both themes and existing admitted ratios/profiles only, exercise actual rapid F-size changes and resulting current placement/viewport/clip at the real post-layout boundary. Each fresh pair has new C/G and genuine complete full-current-K/attempt admission before dependent Paint; no second Update/layout, partial pair, stale/stretch/fallback or unchanged-C size relabelling.
3. Hold two owner/mount material records through real writer/CPU/sampling custody and request another size. Preserve the bound with no tail allocation. Every capacity-pending attempt reports target Pending and produces no new label-only dependent output. Release only certified obligations; the latest current F then produces a complete actual output, with independent target eligibility preserved.
4. Authenticate same-queue submit-return/swap/admit/sample while writer completion remains unobserved: ordered Pending may Ready; an outstanding unsubmitted frame or unknown submit/cancel may not. Missing completion alone cannot fabricate failure/revocation. Actual failure uses its exact owner cause and unchanged genuine Unavailable/late-failure behavior.
5. Withdraw/cancel a submitted-pending generation before any sampling, and separately cancel an unsubmitted writer frame and a consuming CPU plan. The same exact writer record survives admission; no cancellation settles another or already submitted obligation. Unregister/drop occurs once only after the independent complete retirement predicates.
6. Independently hold writer completion, an actual CPU plan and a sampling completion. Each prevents retirement. After observing writer completion, submit a previously held later sampling plan and prove the earlier writer witness cannot settle that later use. Reject foreign/replayed/old-G witnesses, repeated-K replay, obsolete/cancelled attempt evidence and stale completion.
7. Preserve press/release/cancel/capture/focus and exact callbacks while resizing; unchanged-C window/origin/viewport changes still retain G with zero extra producer work, registration or submit. Record actual preparation/pass/texture/registration/submission counts and CPU cost rather than infer performance.
8. Prove finite real failure, genuine Unavailable versus temporary replacement Pending, device-loss retention, strict frozen/late Compile law, withdrawal/unmount/inactivity release of only this output gate, progress/wakes and bounded certified cleanup. Use real per-attempt GPU readbacks and source-bound receipts, not settled-only images or aesthetic goldens; native continuity requires fresh executable provenance and actual native observation.

---

## 10. Consistency and Reduction Audit

| Amendment | Sole owner | Must not be used as |
|---|---|---|
| SPEC-HAL-295A | Permission, offscreen resource scope, renderer privacy, Image publication boundary | A source of target metrics or alpha semantics |
| SPEC-HAL-263A | Image tint/texture association, upload normalization, shader equation, alpha proof | Permission to own GPU resources or a target-sizing phase |
| SPEC-HAL-266A | Participant-local post-layout timing, per-target metrics, physical extent/coverage conversion, target invalidation | Permission to encode/submit or a definition of Image alpha |

A conforming implementation must satisfy all applicable amendments
independently. Compliance with one cannot be cited as evidence for another.

| Law | Result |
|---|---|
| SPEC-HAL-002 phase separation | Strengthened. Logical layout, GPU preparation, immutable paint, and renderer execution have distinct mutation rights. |
| SPEC-HAL-021 generic animation | Preserved. Timeline advances once and is sampled read-only during Update; preparation receives only immutable owner state. |
| SPEC-HAL-024 coordinate model | Preserved. Logical coordinates remain authoritative until one engine-owned target conversion. |
| SPEC-HAL-082 per-window state | Preserved through explicit `SurfaceId` and target-keyed resources. |
| SPEC-HAL-158 layout authority | Preserved. Preparation cannot change layout outputs. |
| SPEC-HAL-266 DPR authority | Strengthened. No global or layout-time DPR path remains. |
| SPEC-HAL-295 custom GPU permission | Imported from SPEC-HAL-295A; not widened. |
| SPEC-HAL-263 image semantics | Not redefined; the currently authoritative SPEC-HAL-263 contract, as amended by SPEC-HAL-263A if adopted, is imported. |
| SPEC-HAL-354 retained update | Preserved. Update owns animation-derived widget/application mutation and preparation remains a later GPU-only consumer. |

The boundary is an opt-in participant capability rather than an all-Widget
phase, uses the existing post-layout/pre-paint insertion point and final geometry
rather than adding a second layout authority, and uses existing per-target
renderer state rather than introducing a global DPR service.

---

## 11. Adoption and Backlinks

Adopted on 2026-08-21 after the live topology confirmed the boundary defined by
this document. Adoption performed exactly these authority changes:

1. SPEC-HAL-266's SPEC-HAL-266A proposal metadata became “Amended by,” and the
   FORBID-DPR-003 anchored proposal note became the normative parent clause
   quoted in §8.1, without mechanically rewriting §§2, 4, 5, or 8;
2. the sole literal parent replacement in §8.2 was applied to SPEC-HAL-295
   INV-295-RSZ1;
3. both the SPEC-HAL-266A portion of SPEC-HAL-295's proposal metadata and its
   §4.8 anchored proposal note became the normative parent clause quoted in
   §8.2, without mechanically rewriting the other named sections;
4. this document remains the audit trail;
5. the generic boundary and its production pixel-coverage equivalence were
   implemented and verified before claiming a dependent custom renderer is
   DPR-correct;
6. Plot's layout-time global-DPR sizing moved to the generic boundary as a
   separate implementation consequence without transferring SPEC ownership to
   Plot.

Implementation evidence is maintained by the opaque SPEC-HAL-295A admission
marker, the typed participant registry in `WidgetTree`, the canonical
post-layout callback in `App`, the renderer-owned target metrics and physical
coverage modules, the Plot participant, and focused coverage in
`src/tests/custom_render_preparation.rs`.

The 2026-08-23 same-frame animation-input amendment is binding but is not claimed
implemented by that evidence. The existing callback remains Timeline-free;
UpdateContext sampling and the exact same-frame immutable owner-revision proof
remain implementation work under SPEC-HAL-021 and §9.16.

---

## 12. Non-Goals

- granting custom GPU resource ownership;
- defining Image alpha semantics;
- defining a 3D subsystem;
- defining animation playback or exposing Timeline to custom renderers;
- changing logical layout or adding DPR to `LayoutContext`;
- traversing every Widget after layout;
- defining device-loss recovery;
- supporting draft split UI/render-thread topology;
- generalizing submission batching;
- making Plot the architectural owner of target metrics.

## Scoped contract adoption - 2026-10-28

**Authority status:** Adopted / Binding scoped amendment.
**Owner decision:** JC approved exact coordinated proposal SHA-256
`1e06d45b3ae2f21c576acf2fea8557287ae58e1d2ef6ff5727cba452d72ed169`
(38,130 bytes). The literal amendments incorporated in SPEC-HAL-012,
SPEC-HAL-266A and SPEC-HAL-295A are adopted together for the same-owner,
same-SurfaceId, window-extent-only immutable-content C/current-full-K
eligibility extension.

**Implementation status:** Contract adoption only. This records no source
conformance, implementation admission or runtime acceptance. Implementation
requires separate explicit governing-owner authorization and the scoped proof
obligations before implementation acceptance or execution claims.

**Preserved history and status:** Existing adoption dates, historical records
and all unrelated rules retain their status. Proposed SPEC-HAL-363/364 clauses
remain inactive. Later proposal revisions and the proposal's narrative or
experimental evidence are not adopted by this record.

## Final scoped contract adoption - 2026-10-28

**Authority status:** Adopted / Binding final scoped amendment.
**Final owner decision:** JC approved exact current coordinated proposal SHA-256
`76c044622fe37d7c9e6222e177342907268ddf5947bb77a35201357b31921248`
(38,084 bytes). Its literal SPEC-HAL-012, SPEC-HAL-266A and SPEC-HAL-295A
amendments define the current adopted same-owner, same-SurfaceId,
window-extent-only immutable-content C/current-full-K eligibility extension.

**Supersession and history:** This final adoption supersedes all earlier
proposal-hash instructions and the earlier
`1e06d45b3ae2f21c576acf2fea8557287ae58e1d2ef6ff5727cba452d72ed169`
scoped selection and wording. The earlier record and historical proof remain
preserved as history; they are not alternate current authority for this scoped
extension. No other proposal revision or experimental narrative is adopted.

**Implementation status:** Normative contract adoption only. This does not grant
implementation admission or establish source conformance, runtime acceptance
or a resize-fix claim. Implementation remains subject to separate explicit
governing-owner authorization and all applicable proof obligations.

**Unaffected authority:** Original dates and unrelated binding requirements,
existing proof conditions and historical records retain their status. Proposed
SPEC-HAL-363/364 provisions remain inactive.


## Current-placement eligibility contract adoption - 2026-10-28

**Authority status:** Adopted / Binding scoped coordinated owner amendment.
**Owner decision:** JC's owner direction on 2026-10-28, authorizes adoption
after independent consistency validation of the exact proposal
`Button SDF current-placement eligibility amendment`,
SHA-256 `5d92d6d33288f0e4e01414076dc6803d21f7aef5fe50cdbaffc6ccdea3749878`
(15,585 bytes). That validation passed against the current Constitution and
SPEC-HAL-012, SPEC-HAL-024, SPEC-HAL-266A and SPEC-HAL-295A.
Only the packet's eight literal coordinated replacement blocks are adopted
into SPEC-HAL-012, SPEC-HAL-266A and SPEC-HAL-295A. Its narrative, pre-flight,
measurements and draft status do not become independent framework authority.

**Scope:** The existing same-owner, same-SurfaceId, active-target procedural
Button generation may receive fresh complete-current-K eligibility across
window-extent changes, fixed-size F translation and consuming-viewport changes
only under the unchanged-content and current-placement qualification above.
Current destination, viewport, physical coverage, clipping and attempt
freshness remain distinct from immutable C, original admission and exact G.
Original generation custody, failure, cancellation and retirement remain strict.

**Historical qualification:** The earlier `76c044622fe37d7c9e6222e177342907268ddf5947bb77a35201357b31921248`
and `1e06d45b3ae2f21c576acf2fea8557287ae58e1d2ef6ff5727cba452d72ed169`
adoption records remain verbatim as history. This later coordinated adoption
extends their window-extent-only limitation only through the eight literal
replacements named above. All unrelated binding clauses, existing proof
conditions, original dates and historical evidence retain their status.

**Implementation status:** Normative contract adoption only. This record
grants no Rust implementation admission and establishes no source conformance,
native executable provenance, rendered-continuity acceptance or resize-fix
claim. Production implementation requires separate explicit governing-owner
authorization and the scoped post-adoption implementation proofs. Existing
baseline fixture evidence is not proof of implementation of this extension.

**Unaffected authority:** SPEC-HAL-002 and SPEC-HAL-024 are unchanged. No new
source, shader, scale, state, runtime, consumer, public API, default material,
fallback, shared-resource cache or inactive-target retention policy is adopted.
Proposed SPEC-HAL-363/364 provisions remain inactive.

## Active Button SDF size-replacement contract adoption - 2026-10-28

**Status: Adopted / Binding, scoped owner amendment.** JC's explicit owner decision, adopts the coordinated literal changes in `Active explicit Button SDF size-replacement amendment`, exact proposal SHA-256 `f6bc9795425cdb8a92e1a1c7eebc88a3d78cbb08fa7a881b242db30b82781e5c`.

This scope is only the existing canonical already-admitted Rest/Pressed pair's active F-size replacement after its initial resource and first-target-output lifecycle: fresh current-C/G/full-K admission before Paint, temporary target-local replacement Pending, and independently authenticated producer-write/CPU/sampling custody. Initial optional A/B activation, HAL-018 section 16 startup, existing source/profile/state/scale and public API/caller limits, genuine failure and every unrelated rule remain unchanged. Universal default replacement remains on hold; no inactive universal-default or HAL-363/364 provision is adopted.

Contract adoption does not establish source conformance or native visual/performance acceptance. The new post-adoption obligations gate implementation acceptance and execution claims; prior proof/history retains its original status. Source-bound before/after hashes, exact literal preview and independent review belong to this adoption's receipt. Implementation requires separate governing-owner authorization.

## Plot active size replacement Pending adoption - 2026-11-10

**Status** Adopted / Binding, scoped technical amendment. The supervising technical reviewer adopted the exact §3.1 and §3.2 text of the
`Plot active resize Pending proposal`
in HAL-PLOT-ACTIVE-RESIZE-PENDING-S1-ADOPTION-AND-S2-EXECUTION-01. The historical
proposal SHA-256 is `a3a3c6f10190e36b2c8f5ef342000ca5e29fcf59ff6a519d8ce24a960cb79e00`.
The adoption covers only Plot active same-SurfaceId physical size replacement.
The coupled Button exception retains its existing scope. JC retains final product
and constitutional authority. Adoption supplies no source or native proof.

## Theme-selected optional Button input (2026-11-22)

**Status: Adopted / Binding, scoped qualification of section 4.** The explicit
Theme selection adopted in HAL-012's 2026-11-22 Button-material section may
produce the same optional procedural request during each Button's one ordinary
Update. It is an admitted authoring source for the existing explicit-request
path, including its already-admitted active-size replacement.

Canonical App/base-target scope, final-layout qualification, initial staged
admission, complete current-key checks, target-local Pending, pre-Paint
publication and existing wake/custody boundaries are unchanged. Theme, density
or Button population does not certify readiness or admit another target, scale
or caller. No late Update, preparation in Paint, or universal-default activation
is authorized. The remaining default-family proposal and its hold are unchanged.
