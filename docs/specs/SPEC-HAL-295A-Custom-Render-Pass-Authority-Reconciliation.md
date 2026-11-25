# SPEC-HAL-295A: Custom Render Pass Authority Reconciliation

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
**Amendment Type:** D — Emergency Correction
**Amends with literal replacement text:** SPEC-HAL-084 INV-RG-011, §K, and AUTH-084-3; SPEC-HAL-040 Invariant 1
**Parent scope read subject to this amendment:** SPEC-HAL-295 §§3–7 and §§11–13
**Depends on:** SPEC-HAL-002, SPEC-HAL-040, SPEC-HAL-084, SPEC-HAL-150, SPEC-HAL-263, SPEC-HAL-266, SPEC-HAL-295
**Extended by:** SPEC-HAL-355 (Adopted / Binding; 3D specialization of the admitted offscreen producer boundary without changing admission or publication authority)
**Runtime scope:** Canonical `App::run` ownership topology only; admitted worker threads may share its `Arc<Queue>`

This amendment reconciles the authority of the already-implemented custom-render
mechanism. It does not adopt a new feature or authorize a 3D subsystem.

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

SPEC-HAL-295 implements an offscreen custom-render mechanism in which an
explicitly admitted participant owns GPU resources, creates command encoders,
and submits work outside Halcyon's RenderGraph. Two older absolute statements
make that implemented mechanism formally contradictory:

- SPEC-HAL-084 INV-RG-011 says every GPU pass and submission is represented by
  a RenderGraph node, with no exceptions.
- SPEC-HAL-040 Invariant 1 says no GPU resource is ever stored inside a Widget
  or SceneNode.

This amendment makes the smallest correction that preserves both laws' intended
ownership boundaries. Renderer-owned GPU work remains entirely inside
RenderGraph. A SPEC-HAL-295 participant receives one bounded permission to
produce an offscreen image without gaining renderer, compositor, scene, or
semantic authority.

---

## 2. Ownership of This Amendment

SPEC-HAL-295A owns only:

1. admission to custom GPU production;
2. the offscreen resource and renderer-privacy boundary;
3. the legal handoff from an offscreen producer to an `ImageNode`;
4. baseline device-lifetime expectations in the canonical `App::run` runtime.

It does not own:

- image alpha or color-space semantics, which belong to SPEC-HAL-263 and
  SPEC-HAL-263A;
- target DPR or physical-rect timing, which belong to SPEC-HAL-266 and
  SPEC-HAL-266A;
- RenderGraph topology or pass-executor design beyond the narrow correction in
  §6;
- device/queue recreation;
- split UI/render-thread transport;
- any 3D API or renderer.

---

## 3. Normative Definitions

### 3.1 Renderer-Owned GPU Work

GPU work is renderer-owned if any of the following is true:

- it consumes `SceneFrame`, `SceneCache`, `DrawPlan`, or compositor output;
- it reads or writes a swapchain image, compositor target, RenderGraph
  attachment, or renderer-private texture or buffer;
- it uses renderer-private pipelines, bind groups, pass contexts, or resource
  pools;
- it participates in presentation or surface acquisition.

Renderer-owned GPU work is governed by SPEC-HAL-084 without exception.

### 3.2 Admitted Offscreen Producer

An admitted offscreen producer is a SPEC-HAL-295 participant explicitly created
through the custom-render admission path. It may use the baseline
`Arc<Device>`, `Arc<Queue>`, and compatible texture format only to operate on
participant-owned offscreen resources.

For a WidgetTree participant, explicit creation is represented by the opaque
`CustomRenderAdmission` opt-in marker owned by the custom-render subsystem. Its
representation is private, while its production constructor is intentionally
public and accepts `&GpuResources`. This is the extension path for application
and third-party custom-render Widget authors: canonical `Screen::build` code can
obtain the current runtime resources through the public
`ScreenFrameContext::app` and `AppContext::gpu_resources()` APIs. Ordinary
Widgets remain non-admitted by default, and implementing
`CustomRenderParticipant` or a Widget callback alone cannot place a node in the
admitted participant registry.

The marker records explicit opt-in only. It stores no `GpuResources` reference,
device/queue identity, epoch, or lifetime, grants no resource access, and does
not authenticate the resources later used by the participant. Constructing or
possessing it therefore does not waive any invariant in this amendment. The
participant's actual GPU resources and publication lifecycle remain separately
subject to this SPEC and its proof obligations.

Possession of `Device` or `Queue` is not renderer ownership and grants no access
to renderer-private state.

### 3.3 Publication Boundary

**Button material source admission.**
The existing private static material source and ABI remain renderer-owned.
Their admission grants no generic shader API, Widget GPU ownership or ordinary
Button CustomRenderParticipant status. Reservation, authenticated generation,
frozen-front, dual custody and failure rules remain mandatory. Source admission
and compilation alone do not establish current resource or execution evidence.

The only legal renderer-visible result of an offscreen producer is a registered
sampled texture view referenced by `texture_id` from an ordinary `ImageNode`.
Registration establishes sampling availability; the `ImageNode` establishes
scene geometry, clip, z-order, tint, and compositor intent.

Registration is a CPU publication boundary. It does not assert that producing
commands have already completed on the GPU; execution visibility is provided by
the per-publication ordering in INV-295A-008 on Halcyon's one baseline `Queue`.

**Button material resource contract.**

The concrete SDF producer SHALL be explicitly admitted under the existing
renderer/custom-render resource infrastructure. Button owns no GPU device,
queue, CustomRenderPass, global registry or historical lease. Its immutable
Ready/Unavailable publication is CPU resource metadata; renderer-visible GPU
output remains an admitted registered Image. Producer writes precede swap and
publication on the same baseline queue; a registered sampled front SHALL NOT
be overwritten. The resource owner protects sampling lifetime and unregisters
before final drop. ImageBinding evidence neither supplies that protection nor
grants recovery. Exact generation revocation is monotonic; re-registration never
revives an old binding. Producer failure cause remains resource-owned.

Generation protection SHALL cover the post-Paint registration drain as well as
Compile and sampling: queued replacement SHALL NOT invalidate a generation
already emitted by the assembled scene. A planned withdrawal known before the
commit cutoff cannot be ignored merely because availability is still currently
true. The concrete producer identity/protection policy requires owner admission;
no new identity allocator, registry or lease is inferred.

**Button material producer ownership.**

The existing producer capability belongs to the canonical App-runtime/renderer-
resource authority, not a Widget, WidgetTree CustomRenderParticipant or global
singleton. Its immutable generation record retains the original authenticated
admission, content basis C, exact ImageBinding identity/generation and owner/mount,
SurfaceId and baseline resource ownership.

Current Ready/Unavailable publications remain keyed by the complete current K.
The existing resource owner may issue fresh eligibility evidence referring to
an unchanged admitted generation only under SPEC-HAL-266A's scoped qualification.
For its admitted placement/viewport changes, that evidence also authenticates
the consuming attempt's current finite destination and visibility projection;
Paint and scene traversal retain the current canonical clip. Original placement
is not current evidence. Eligibility remains distinct from original generation
admission and exact generation custody. It cannot rekey or relabel a reservation
or producer admission, authenticate different content, revive a revoked
generation or substitute C for K.

The record owns its existing front/back lifetime and immutable inputs using
baseline GpuResources and the existing queue. WgpuRenderer remains registrar;
Button consumes only admitted immutable CPU publication. No universal material
registry, shader graph, shared-resource architecture or Button-owned GPU follows.
Eligibility publication is not a new front swap or registration; original
producer enqueue/publication ordering under INV-295A-008 remains unchanged.

Replacement SHALL NOT overwrite or re-register a sampled texture ID with live
emitted Image uses. The owner retains the prior front and generation until all
assembled Compile/sampling uses settle under existing queue ownership, or
postpones replacement to a legal later frame without treating elapsed frames as
settlement evidence. Retirement explicitly unregisters before drop; a Drop
assertion is not cleanup proof. No GPU-completion prerequisite is added to
publication. If the current owner cannot certify safe retirement, execution
SHALL NOT proceed. This ownership contract admits no invented transport,
failure mapping, producer, runtime or consumer integration.

**Dual generation custody.**
For each exact ImageBinding identity/generation under its exact admitted resource
owner and immutable content generation, the frame owner SHALL acquire CPU custody
when a scene/draw plan containing that Image is
assembled. Each CPU token ends only on terminal cancellation with no future
submission, or actual successful Queue::submit of the command buffer containing
that generation's sampling commands. The submit handoff SHALL establish GPU
pending custody without a retirement-eligible gap. The submission owner SHALL
register the terminal observation after that exact submission on the same
baseline queue, using the transport below. Native maintenance uses nonblocking
Device::poll(Poll).
Explicit unregister-before-drop is eligible only when no CPU queued, assembled
or compiled use tokens remain AND all submitted sampling completion tokens through
the generation's last use have been observed. Multiple in-flight generations
SHALL retain independent custody and resources. No texture ID with active CPU/GPU
custody may be re-registered or overwritten. Frame counters, elapsed epochs,
swaps, per-frame blocking Wait and ImageBinding itself are not retirement
certificates. Error/device loss SHALL fail closed and retain uncertified resources,
not assert retirement; safe bounded shutdown retirement requires separate source
proof. This selects lifetime requirements, not a new GPU lease/fence API or
production implementation. Uncertified integration cannot proceed. Publication
ordering under INV-295A-008 is unchanged and acquires no GPU-completion
prerequisite.

**Observation-bounded terminality and same-queue transport.**
A genuine authenticated post-submit terminal observation settles only the exact
covered submitted-use obligation. For browser WebGPU, the renderer uses
wgpu 30.0.2's public `Queue::as_webgpu()` to reach the original WebGPU queue.
It calls that queue's `onSubmittedWorkDone()` synchronously after the covered
submit and defers only awaiting the returned Promise. Other backends retain
`Queue::on_submitted_work_done`. Rejection retains captured custody, invokes no
completion callback and propagates the original error. Registration, elapsed
time, other commands or queues, and dependency changes supply no terminal fact.

WebGPU may fulfill the Promise on device loss. Fulfillment certifies settlement
of the covered submitted use only. Successful execution, physical GPU idleness,
device health, valid image contents, successful presentation, Ready and
eligibility for another owner each require their own authority. Observed device
loss leaves an authenticated terminal observation intact. Loss supplies no
missing observation, settles no submission-uncertain or exported use, and
satisfies no independent retirement predicate.

An unresolved `device.lost` Promise supplies neither health nor failure evidence.
Normal work on a healthy device does not await that Promise. Delayed loss delivery
alone leaves authenticated queue observations valid. Observed loss denies current
binding availability and operations requiring it at their existing owner cutoff.
Later loss follows the existing frozen-snapshot, strict Compile and next
authorized publication rules. See [WebGPU Promise Ordering](https://gpuweb.github.io/gpuweb/#promise-ordering),
[device loss and timeline listeners](https://gpuweb.github.io/gpuweb/#lose-the-device)
and [HAL-385 §9.3](SPEC-HAL-385-True-Frosted-Backdrop-Glass.md#93-actual-submission-and-failure-ordering).

**Producer-write custody for the scoped active-size replacement.**
Before admission, the existing resource owner retains authenticated writer-submission and pending/complete/uncertain evidence for the actual producer/reservation, written resources and baseline Device/Queue. Genuine admission associates that SAME record with its original actual ImageBinding generation. It survives cancellation, replacement, current-K certification and retirement without reset, duplication, relabelling or a custody gap, including registered generations never sampled. This is distinct from original admission, target eligibility, CPU-use custody and sampling-use custody, not a new public material status or API.
Successful return of all relevant writer submits establishes submitted-pending/order evidence only. Immutable swapped fronts, complete genuine pair admission and no outstanding unsubmitted producer frame are required before Ready. Known ordered pending writes may publish and be sampled under INV-295A-008 without waiting for GPU completion. Unknown writer submission/cancellation cannot authenticate Ready or retirement.
Writer Complete requires an authenticated resource-owner completion observation registered after the actual relevant writer submissions on the same baseline queue and covering all such writes to the resources being retired. Registering a callback is not observing completion. Submission return, swap, registration, current K/attempt, elapsed frames, an unrelated/replayed witness, terminal CPU cancellation or a sampling-use certificate cannot supply missing writer completion.
Final unregister/destruction requires all submitted writes certified complete, no outstanding writer frame able to submit, terminal no-submit proof for cancelled unsubmitted work, no CPU use able to submit, and all actual sampling submissions settled under their existing exact post-submit certificates. These predicates are independent. Empty sampling custody cannot settle never-sampled writer work; earlier writer completion cannot settle a later sampling submission. Cancellation releases only the exact obligation whose future submission is terminally excluded, never submitted or submission-uncertain work.
The final admission/unregister boundary also requires the exact current registered
generation and its binding availability. Observed loss refuses this retirement.
Queue terminal evidence overrides none of these predicates. Ordered pending
writes retain the original INV-295A-008 publication path without a GPU-completion
prerequisite.
Missing/delayed completion alone remains unsettled and fabricates neither producer failure nor binding revocation. Actual errors keep their genuine owner-supplied causes; submission/cancellation or completion-observation uncertainty and device loss retain uncertified resources under existing failure rules. Late discovery preserves frozen-snapshot/strict Compile behavior. No timeout, forced drop or absent callback converts uncertainty into completion. The existing producer/resource owner carries this protection through the actual admission/retirement path; merely removing its staging wait is nonconformant. Existing initial optional behavior and all other custody protections remain unchanged.

Fresh current-K eligibility evidence neither ends nor duplicates generation
custody. Every current or retained use of the same exact generation participates
in that generation's existing lifetime protection. Withdrawal of one current
eligibility record does not permit retirement while any CPU or GPU use remains.
Terminal cancellation, submission handoff without a gap, completion observation,
unregister-before-drop and failure requirements remain unchanged.

Certification is CPU eligibility metadata, never a generation-custody token or
GPU-completion certificate. Cancellation releases an exact CPU-use token only
after terminal proof that its plan cannot submit. It neither settles an already
submitted or submission-uncertain GPU use nor cancels another plan's custody.
Eligibility expiry, encoding completion and elapsed frames cannot certify retirement.

Known baseline device/queue invalidity or device loss before commit cutoff
prevents certification and Ready. Settlement uses the genuine failure cause
supplied by the existing resource owner; no cause is invented from missing
eligibility alone. Loss discovered after cutoff does not rewrite the frozen
snapshot. Existing strict Compile/execution failure handling applies, with
owner-visible settlement in the next authorized publication cycle. No late
Update, Paint polling, fallback or successful-output claim follows.

Device loss or unobserved completion SHALL retain uncertified generation
resources under existing failure custody. Cancellation, eligibility expiry,
failed polling and elapsed frames are not retirement evidence. This extension
authorizes no device recreation, registration replay, cross-device certification,
generation revival or recovery. INV-295A-009 and its separate SPEC-HAL-040
amendment requirement remain unchanged; no automatic epoch mechanism is inferred.

**Prepared endpoint resource.**

The existing material producer may own one immutable two-cell Rest/Pressed
texture under its authenticated content basis and original generation admission.
Both complete endpoints must be produced on the baseline queue before genuine
admission certifies the pair. One exact ImageBinding identity/generation and
existing CPU/GPU custody cover both cells. Current consumption requires evidence
for the complete current K. Only SPEC-HAL-266A's scoped unchanged-content
qualification may supply that evidence without another production or registration
cycle; it cannot alter original admission.

Endpoint selection from the separately checked interaction request revision
remains distinct from target qualification and cannot create Ready evidence.
The engine commits current UV before retained Paint, preserving warm immediate
press/release/cancel behavior. Any incompatible content or target change,
withdrawal or revocation prevents stale projection and follows existing admission
and certified whole-texture retirement, protecting old compiled uses and bounded
overlap. Registrar protection, exact writer/CPU/sampling settlement, failure/late-Compile checks and resource ownership remain unchanged. Only SPEC-HAL-266A section 4's already-admitted explicit-request F-size replacement and coupled temporary pending-output gate qualify phase order; initial optional A/B admission is unchanged. No generic shader/cache API, new default/state/scale/caller capability or native visual/latency claim follows.

**Rounded gold Button profile (HAL-012 §7.1).** The same private producer and
registrar admit one immutable texture for the current basis. Enabled bases contain
the four Rest, Hover, Rest with Pressed, and Hover with Pressed cells specified
by HAL-012 §7.1. Disabled uses a single static face. Both follow the same original
admission, current eligibility and exact generation custody.

### 3.4 Rebuildable GPU State

Rebuildable GPU state includes pipelines, bind groups, buffers, textures,
texture views, command encoders, and render-local caches derivable from
upstream data. It excludes application/domain authority, serialized view state,
cross-widget coordination state, and policy decisions.

---

## 4. Binding Invariants

### INV-295A-001 — Canonical Runtime Scope

This permission applies only to the canonical `App::run` runtime in which the
renderer supplies the baseline device, queue, format, and deferred image
registration channels.

This is an ownership-topology restriction, not a single-thread restriction.
Admitted producers may run on worker threads and may share the renderer-supplied
`Arc<Queue>` while remaining subject to the per-publication ordering in
INV-295A-008.

No claim is made for the draft `SceneSwapchain` / split UI-render-thread
topology. Support for that topology requires separate authority for request,
publication, retirement, and failure transport.

### INV-295A-002 — Explicit Admission

GPU ownership is not a general Widget capability. Only a participant admitted
through the SPEC-HAL-295 custom-render path may retain GPU resources. Ordinary
widgets remain PaintSink consumers and cannot obtain this permission by holding
a texture ID or emitting an `ImageNode`.

WidgetTree registration MUST require the opaque `CustomRenderAdmission` opt-in
marker in addition to the preparation callback. A callback trait implementation
without that marker is inert. The public marker distinguishes deliberate
custom-render participation from an ordinary Widget; it is not proof of
device/queue identity or lifecycle conformance.

### INV-295A-003 — Offscreen-Only Ownership

An admitted participant may create, retain, encode, and submit work only for
resources it owns. Its render attachments must be offscreen textures created by
that participant for later sampling through the Image pipeline.

The participant must never encode against a surface texture, compositor target,
RenderGraph attachment, renderer background texture, or any other
renderer-owned target.

### INV-295A-004 — Registered Image Composition

Custom output enters Halcyon's renderer only as a registered sampled texture
view and an `ImageNode`. Target-independent images carry the registered texture
identity directly. Per-target images carry only an opaque admitted presentation
identity that the engine resolves for the current `SurfaceId` before ordinary
Image batching. Both use the unchanged Image shader and composition path. All
display composition remains in the normal SceneGraph → Batcher → RenderGraph
path.

The custom producer cannot decide scene placement, clip, z-order, blend state,
surface selection, or presentation timing.

### INV-295A-005 — Renderer Privacy

An admitted participant receives no `PassContext`, graph attachment handle,
surface handle, renderer pipeline, renderer bind group, renderer buffer, or
mutable `WgpuRenderer` reference for encoding.

Registration and retirement APIs may mutate the renderer's image registry at
their declared lifecycle boundary. They do not expose renderer internals.

### INV-295A-006 — No DrawPlan or Graph Authority

Custom GPU work must not read, mutate, append to, reorder, or reinterpret
`DrawPlan`, RenderGraph topology, pass dependencies, compositor output, or
scene state. The producer's only scene contribution is the same immutable image
primitive available to any PaintSink consumer.

The producer cannot inspect or mutate the target-bound presentation table,
`DrawPlan`, surface selection, or Image lowering. Its only additional output is
the participant-local binding returned from the authorized per-target callback.

**HAL375 §10 sole-binding companion [S1]:** A HAL266A companion for HAL375 is private data within the sole existing binding. The producer's sole scene contribution remains the registered, target-bound Image. Only the renderer may admit the complete HAL375 §9 text, backgrounds, leaders, and visibility into the normal surface schedule. INV-004/006 retain their prohibitions on producer access to placement, lowering, DrawPlan, and graph authority.

**363/364 scoped amendment:** For INV-295A-004/006 the producer still contributes only its ordinary
current Image publication. Private historical binding/certification is an
engine/resource-owner capability; it grants no producer placement, mask, order,
surface-selection, presentation-timing or graph-topology authority.
Capturability is separate from CustomRenderAdmission opt-in; that marker
authenticates neither device nor immutable content. INV-295A-009 retains
baseline-only validity; no public device epoch or recovery promise is added.

### INV-295A-007 — GPU State Is Never Semantic Authority

A custom-render participant may retain only rebuildable GPU state and
render-local mechanics. Application/session authorities own all domain data,
serialized view state, interaction meaning, and cross-widget coordination.

GPU readback cannot silently become a semantic data source. Any separately
authorized readback must publish pure data through an explicit application
boundary before that data can affect UI semantics.

### INV-295A-008 — Submission, Swap, and Publication Ordering

For one candidate front, every command buffer that writes that front MUST be
successfully enqueued by `Queue::submit()` on the same baseline queue that later
submits Image composition, and every such submit call MUST return before the
completion/publication state for that front is exposed. Front/back swap occurs
after the final relevant submit returns. Registration publishes the resulting
front view only after the swap.

When producer and renderer run on different threads, the transfer of that
completion/publication state to UI registration MUST establish a CPU
happens-before edge. The UI registration boundary—direct registration or a
deferred registration drain—then completes before the later renderer submission
that samples the front. wgpu's single-queue ordering therefore makes the
front's writer submissions execute before its sampling submission. Publication
does not wait for GPU completion and does not require `Device::poll`, a fence
wait, or `Queue::on_submitted_work_done()`.

Resource-disjoint producers may call `Queue::submit()` concurrently through the
same `Arc<Queue>`, as the live `PlotRenderPool` does. This amendment neither
requires nor claims a global CPU total order among unrelated producer submits
and adds no serializer. It does forbid publishing a front when any submission
that writes that front is unordered with respect to that front's publication or
later sampling submission. A second queue or second device is not admitted.

No texture may be written while it is the registered front sampled by Halcyon.
Retirement must unregister every published view before the participant drops
the final resource that keeps that view valid.

**363/364 scoped amendment:** An admitted producer MAY support SPEC-HAL-364 capture only by
supplying an exact content-generation read lease at its existing resource
publication/lifecycle boundary. While that generation may be sampled by history
or submitted work, the producer SHALL NOT write, clear, resize in place, recycle
it as a back buffer or republish different content under that generation.
Removing current registration is not permission to overwrite a historical lease.

Lease acquisition and withdrawal/reuse SHALL be ordered by the actual producer/
resource owner so no capture/write race exists. An uncooperative producer or
unverifiable generation refuses historical capture; ordinary current Image
publication remains possible.

An independent copy, when separately execution-admitted, SHALL be an explicit
renderer-owned SPEC-HAL-084 operation with a protected source and a newly owned
destination generation. It SHALL NOT use this participant exception to inspect
renderer-private output, SceneCache, DrawPlan or a swapchain. Existing same-queue
enqueue-before-publication and unregister-before-final-drop laws remain.

### INV-295A-009 — Baseline Device Lifetime Only

The actual GPU resources retained by an admitted producer are valid only for the
lifetime of their baseline device and queue. `CustomRenderAdmission` does not
track that lifetime or authenticate either handle; as a marker it may outlive or
coexist with different resources without making those resources conformant.
This amendment does not promise device-loss recovery, device epoch tracking,
automatic cache recreation, or registration replay.

If Halcyon later gains device/queue recreation, a separate SPEC-HAL-040
amendment must define epoch ownership, stale-publication rejection, and
participant rebuild sequencing before custom resources can claim recovery.

### INV-295A-010 — No-Participant Structural Absence

When no custom-render participant is admitted, the runtime creates no custom
textures, buffers, pipelines, command encoders, command buffers, registrations,
workers, or submissions. Ordinary 2D scene traversal and RenderGraph topology
remain unchanged.

---

## 5. Forbidden Behaviors

The following are non-conformant:

1. exposing `PassContext` or renderer-private attachments to a custom producer;
2. rendering directly into a swapchain or compositor target;
3. adding an unrepresented renderer-owned pass or submission outside
   RenderGraph;
4. allowing arbitrary widgets to retain GPU objects without custom-render
   admission;
5. storing any GPU object in a `SceneNode`;
6. mutating `DrawPlan`, scene geometry, z-order, clip, or compositor policy from
   custom GPU code;
7. treating device/queue sharing as a device-recovery guarantee;
8. publishing a front view before its writes are submitted;
9. reusing a registered front as a writable back texture;
10. dropping participant-owned texture storage before unregistration is
    drained;
11. claiming this contract for `SceneSwapchain` or another runtime topology by
    analogy;
12. treating GPU completion as a prerequisite for same-queue publication; or
13. publishing a front when a submit that writes that front lacks the required
    CPU happens-before path to publication and later sampling.

---

## 6. Literal Replacements and Parent Integration

### 6.1 SPEC-HAL-084 INV-RG-011

At adoption, INV-RG-011 was replaced by:

> Every renderer-owned GPU pass and every GPU submission that consumes or
> mutates renderer/compositor resources MUST correspond to a RenderGraph node.
> Ad-hoc encoders, helper passes, direct encoder access, and hidden submissions
> in that domain remain forbidden.
>
> A producer explicitly admitted by SPEC-HAL-295 is outside the RenderGraph
> domain only while it operates exclusively on participant-owned offscreen
> resources and obeys SPEC-HAL-295A. Its display result is represented in graph
> topology by the ordinary `ImageNode` composition pass. This exception grants
> no access to renderer-owned resources and cannot be extended by analogy.

No other INV-RG-* invariant is modified.

### 6.2 SPEC-HAL-084 §K and AUTH-084-3

The §K criterion “All GPU work flows through the graph” becomes:

> All renderer-owned GPU work flows through the graph; admitted SPEC-HAL-295
> offscreen production satisfies the isolated producer contract in
> SPEC-HAL-295A and enters renderer execution only through Image composition.

AUTH-084-3 becomes:

> Every renderer-owned GPU command has a graph node. An admitted offscreen
> producer has no renderer-owned command and is represented at composition by
> its `ImageNode`.

### 6.3 SPEC-HAL-040 Invariant 1

At adoption, Invariant 1 was replaced by:

> No GPU resource is ever stored in a `SceneNode` or ordinary Widget state. The
> sole bounded exception is a SPEC-HAL-295 custom-render participant, which may
> be widget-associated and may retain participant-owned offscreen resources and
> rebuildable GPU caches under SPEC-HAL-295A. That exception never grants
> semantic authority or renderer-private resource access.

No lifetime tier, pool rule, resize rule, or recovery promise in SPEC-HAL-040 is
otherwise changed by this amendment.

### 6.4 SPEC-HAL-295

At adoption, the proposal backlink in SPEC-HAL-295 became this normative parent
clause:

> SPEC-HAL-295 §§3–7 and §§11–13 are read subject to SPEC-HAL-295A. “External to
> the render graph” means only the admitted offscreen domain defined there.
> “Independent” means independently encoded and submitted; it does not mean
> independent authority over scene or presentation.

No existing SPEC-HAL-295 prose is mechanically replaced by this subsection.

Target metrics and alpha semantics are not defined here. If SPEC-HAL-266A or
SPEC-HAL-263A is adopted, its domain-specific rules govern the corresponding
boundary; until then the current parent SPEC remains authoritative.

---

## 7. Contract and Proof Obligations

Adoption was conditioned on source-grounded evidence for all of the following:

1. the canonical `App::run` path is the only runtime claimed by the contract;
2. custom encoding cannot obtain `PassContext`, surface views, graph
   attachments, renderer pipelines, or `DrawPlan` mutation access;
3. a custom participant can render to its own back texture, submit, swap,
   register the front, return its admitted target-bound binding, and display it
   through the ordinary `ImageNode` composition path;
4. clipping, z-order, tint, and surrounding 2D content remain controlled by the
   normal Image/RenderGraph composition path;
5. front/back non-aliasing, enqueue-before-swap, and the per-front CPU
   happens-before path from every writer submit through completion publication,
   direct or deferred UI registration, and later renderer sampling submit are
   mechanically tested, including a worker-thread producer;
6. immediate and deferred destruction keep published views alive until
   unregistration is drained;
7. an ordinary widget without `CustomRenderAdmission` cannot enter the typed
   participant registry or use the custom GPU lifecycle, even if it implements
   the preparation callback trait;
8. a runtime with no admitted producer creates none of the resources enumerated
   by INV-295A-010 and preserves its ordinary graph/pass/submission topology;
9. same-queue sampling observes producer output without a GPU completion wait,
   while resource-disjoint producer submits may be concurrent without a global
   serializer;
10. no acceptance evidence relies on device recreation or the draft
    `SceneSwapchain` topology.

The framework adoption proof is retained in `src/tests/custom_render_pass.rs`
and `src/tests/custom_render_preparation.rs`. It covers explicit admission,
worker-thread direct and deferred publication ordering, same-Queue sampling
without an intervening GPU-completion wait, resource-disjoint concurrent
producers, front/back retirement, the ordinary 2D RenderGraph topology and
lazy producer admission. Proof-only counters are thread-local and compiled under
`cfg(test)`; they cover only activity on the asserting thread and do not claim
process-global absence. No proof instrumentation adds release-path state or work.

---

## 8. Consistency Matrix

| Law | Result |
|---|---|
| SPEC-HAL-002 retained ownership | Preserved. WidgetTree retains widget ownership; the admitted participant owns only rebuildable GPU state. |
| SPEC-HAL-002 phase separation | Preserved. Paint emits an image only; renderer phases remain unchanged. |
| SPEC-HAL-002 pure data / no semantic GPU authority | Strengthened by INV-295A-007. |
| SPEC-HAL-040 lifetime ownership | Clarified for one external, device-bound participant class; no renderer pool rule is relaxed. |
| SPEC-HAL-084 RenderGraph authority | Preserved for every renderer/compositor resource and command. The exception is defined by resource ownership, not pass name. |
| SPEC-HAL-150 compositor authority | Preserved. Placement, clip, z-order, and presentation stay downstream of `ImageNode`. |
| SPEC-HAL-263 Image contract | Imported without reinterpretation. |
| SPEC-HAL-266 target metrics | Imported without reinterpretation. |

The amendment introduces no cross-widget reference, no hidden scene mutation,
no navigation path, no new renderer phase, and no new semantic authority.

---

## 9. Adoption and Backlinks

Before adoption, backlinks in SPEC-HAL-040, SPEC-HAL-084, and SPEC-HAL-295
identified this document as a proposed amendment with no semantic effect.

At adoption:

1. apply only the literal replacement text in §§6.1–6.3 at SPEC-HAL-084
   INV-RG-011, §K, and AUTH-084-3 and SPEC-HAL-040 Invariant 1;
2. change the SPEC-HAL-295A proposal metadata in SPEC-HAL-040 and SPEC-HAL-084
   to “Amended by” metadata and promote each current local proposal note to an
   adopted amendment backlink;
3. replace the SPEC-HAL-295A portion of SPEC-HAL-295's proposal metadata with
   the normative parent clause quoted in §6.4, without mechanically rewriting
   its existing sections;
4. retain this document as the audit trail for the correction; and
5. do not change the status of any future 3D or split-runtime work.

---

## 10. Non-Goals

- defining a 3D renderer, viewport, camera, scene, or material API;
- moving custom work into RenderGraph;
- generalizing command submission batching;
- adding device-loss recovery or a device epoch;
- supporting draft split-runtime architecture;
- defining per-target DPR conversion;
- defining image alpha conversion;
- modifying ordinary Widget or SceneNode ownership beyond the exact exception
  above.

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

## Theme-selected optional Button input (2026-11-22)

**Status: Adopted / Binding, scoped qualification of section 3.3.** HAL-012's
2026-11-22 explicit Theme policy feeds the existing authenticated Button material
request through ordinary Update. The existing App/renderer producer owns all
preparation, registration, immutable generation publication and retirement.

This authoring extension grants no widget GPU ownership, shared cache, new
producer, shader, scale, target or caller. Original command/queue, writer, CPU
and sampling custody and certified retirement remain unchanged. It neither
adopts nor activates the held universal-default Button proposal.
