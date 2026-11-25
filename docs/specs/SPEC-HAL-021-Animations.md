# SPEC-HAL-021: Generic Animation Timeline

> **Paired amendment status — Proposed (2026-10-05):** The scoped
> SPEC-HAL-363/364 text in this document is part of the proposed paired adoption
> package, not an independent adoption or implementation claim. It becomes
> effective only with joint adoption of
> [SPEC-HAL-363](SPEC-HAL-363-Transient-Presentation-Coordination-and-Lifetime.md)
> and [SPEC-HAL-364](SPEC-HAL-364-Presentation-Occurrences-Coverage-and-Execution-Admission.md)
> and their required scoped amendments. The existing declared status and
> implementation evidence apply to the pre-existing contract. All unlisted
> clauses and ordinary-owner boundaries remain unchanged.

**Author:** JC
**Original Date:** 2025-12-27
**Binding Amendment:** 2026-08-23
**Status:** Binding
**Layer:** Core runtime utility / engine-owned time-driven value interpolation
**Scope:** Timeline identity, easing, finite and repeating playback, same-frame read-only sampling, lifecycle, and frame-loop integration
**Depends on:** SPEC-HAL-001, SPEC-HAL-002, SPEC-HAL-042, SPEC-HAL-043, SPEC-HAL-147, SPEC-HAL-354
**Explicitly does NOT depend on:** SPEC-HAL-131A/B scroll/virtualization, input-fusion policy, gesture arbitration, or any renderer-specific subsystem
**Non-goals:** gesture physics, inertia, momentum, scroll behavior, structural tree mutation, layout policy, post-layout geometry-transition authority, renderer-owned sampling, a private scheduler, cross-widget animation sharing, large semantic payload storage, pause/resume, or a universal keyframe/graph system

---

## 1. Purpose and Authority Decision

Halcyon needs deterministic values that change over engine time without creating
another state owner, clock, or frame scheduler. The same mechanism must serve
ordinary retained paint, update-owned presentation state, and custom-render
consumers without creating a renderer-specific or 3D-specific animation path.

The authority chain is:

```text
EngineTicker captures one monotonic frame time
    -> SPEC-HAL-042 completes its preceding pre-routing scroll/input phases
    -> Timeline advances exactly once for that admitted frame
    -> events may start or cancel handles at that same frame time
    -> Widget update samples values needed before paint and publishes immutable
       owner state
    -> layout / post-layout projection / custom preparation consume only that
       already-authorized immutable owner state
    -> a paint-only finite consumer may instead read the same frame-stable sample
       without mutating widget/application state
    -> Halcyon alone decides whether another frame is admitted
```

Animation is a consumer of engine time. It is not application authority, a
simulation, an interaction policy, or a producer of frames.

**AUTH-021-001 — Generic Timeline Authority:** SPEC-HAL-021 is the sole generic
authority for engine-time value interpolation, playback, handle state, and
same-frame sampling. A renderer, viewport, widget family, or consumer subsystem
must not create a parallel animation clock or playback implementation.

---

## 2. Ownership Boundaries

### 2.1 Timeline ownership

The Timeline is an engine-owned registry. Widgets and engine components retain
opaque ID-based handles and their own cached or immutable presentation values.
They do not own Timeline instances, store references into Timeline storage, or
retain WidgetTree references through a handle.

### 2.2 Caller-owned state

Animation output is a value sample only. The caller remains the sole owner of:

- application and domain state;
- focus, selection, navigation, capture, and interaction state;
- scroll targets and gesture lifetimes;
- WidgetTree structure and layout inputs;
- camera/controller policy;
- custom-render model/view revisions; and
- every irreversible or semantic action.

Sampling may update the owning widget's cached presentation value during the
update phase. It must not write application meaning back from paint, layout,
custom preparation, GPU state, or readback.

### 2.3 Geometry-transition boundary

**363/364 scoped amendment:** SPEC-HAL-147 owns pure post-layout geometry projection.
Ordinary layout transitions use their actual authorized layout endpoints.
SPEC-HAL-363 may reserve a bounded Timeline parameter in Update and publish that
frame's immutable parameter with a causal request. SPEC-HAL-364 may derive a
complete presentation occurrence after canonical layout and ownership resolution
using that publication and certified presentation endpoints. Neither operation
makes Timeline a geometry owner. Canonical layout, hit geometry, constraints and
previous/current layout history SHALL remain unchanged.

---

## 3. Binding Invariants

### ANIM-INV-1 — Single Time Authority and One Advance

Every admitted frame captures one monotonic engine timestamp. Timeline advances
exactly once with that timestamp. No animation evaluator may call a private
clock, accumulate its own frame delta, spawn a thread, sleep, create an OS timer,
or advance from a renderer callback.

### ANIM-INV-2 — Frame-Stable Read-Only Sampling

After the single Timeline advance, sampling existing handles is read-only. Event
routing and Widget update may only start or cancel handles through explicit
registry operations that bind to the already-captured frame timestamp. Neither
phase may invoke a second Timeline advance, re-evaluate elapsed time, or mutate
the sampled value of an existing handle except by explicit cancellation. Layout,
post-layout projection, custom-render preparation, paint, renderer preparation,
and GPU submission cannot advance or otherwise mutate Timeline.

All reads of one handle after Widget update begins and for the remainder of that
frame observe one coherent state unless that same update explicitly starts or
cancels the handle before reading it.

### ANIM-INV-3 — Same-Frame Start Semantics

An animation started during event routing or Widget update binds to the current
captured frame timestamp. Its sample in that frame is exactly `from`; it must not
jump by the previous frame delta. An existing handle samples the value derived
from the current captured frame timestamp.

### ANIM-INV-4 — Exact Terminal Observation

A finite animation that completes on a frame exposes `Completed` with its exact
terminal value for the remainder of that frame. Timeline retirement happens only
after that observation window. Completion must not collapse into a missing
handle or leave a caller with the last pre-terminal sample.

### ANIM-INV-5 — Distinct Handle States

Read-only sampling distinguishes at least:

```text
Active(value)
Completed(exact_terminal_value)
Cancelled
Missing
TypeMismatch
```

`Cancelled` is observable for the remainder of the frame in which cancellation
occurs. `Missing` means the ID is unknown or its terminal observation window has
already retired. `TypeMismatch` neither cancels nor mutates the animation. These
states must not be represented by one ambiguous `None` result.

### ANIM-INV-6 — Determinism Across Cadence

Given identical descriptor, start timestamp, query timestamp, and handle state,
the sample is identical. A 60 Hz and 120 Hz run therefore produce equal values at
equal timestamps; cadence changes only the density of intermediate observations.
No playback mode may derive phase from accumulated `dt`.

### ANIM-INV-7 — No Structural or Semantic Effects

Timeline evaluation cannot insert, remove, replace, reparent, realize, or
virtualize widgets; mutate authoritative layout or hit-test geometry; dispatch
actions; perform GPU work; or choose application policy. A sampled owner reports
the narrowest valid invalidation through the normal update contract.

### ANIM-INV-8 — No Gesture or Physics Substitution

Timeline does not encode velocity, friction, momentum, input arbitration,
gesture start/update/end, scrolling, or controller policy. A consumer may animate
a presentation parameter after its own policy chooses endpoints, but Timeline
does not choose those endpoints or interpret input.

### ANIM-INV-9 — Explicit Playback and Lifetime

`Once` is the default. Repetition occurs only through an explicit `Loop` or
`Alternate`/`PingPong` descriptor with an explicit finite count or explicit
infinite lifetime. Infinite playback has an identifiable owner responsible for
cancellation on unmount, replacement, or the end of the intended effect.

Timers must not restart a sequence to emulate repetition. Presence of an active
handle may be reported as animation demand to Halcyon's existing scheduler, but
Timeline itself never wakes the platform or admits a frame.

### ANIM-INV-10 — Bounded Identity and Storage

Handles use non-wrapping IDs. Timeline storage is finite and queryable or bounded
by a finite engine policy. Exhaustion fails explicitly; it cannot wrap, alias a
live handle, or silently drop another animation. Advance and sample perform no
heap allocation after admitted storage exists. Completed/cancelled observation
records retire at the next legal frame boundary and do not become history.

### ANIM-INV-11 — Explicit Payload Admission

An interpolation trait does not by itself promise that arbitrary payload storage
is supported. Every payload type must be explicitly admitted by the Timeline
implementation and have deterministic interpolation. Starting an unsupported
type fails explicitly; it must not create a handle whose value remains frozen.

Type erasure is an implementation choice, not permission to hide unsupported
types or ambiguous retrieval. Large semantic snapshots, cross-widget shared
values, and application models are not Timeline payloads; owners animate bounded
parameters and derive their complete immutable state during Widget update.

**363/364 scoped amendment:** For SPEC-HAL-363/364, the final paragraph's requirement to derive all
complete state during Widget update is replaced within this scope: Timeline
payloads SHALL remain explicitly admitted bounded parameters. A group may
distribute one published parameter to multiple participants; source generations,
participant state, semantic snapshots and composition graphs SHALL NOT be
Timeline payloads. The owning Update SHALL publish all mutable control decisions
and the coherent parameter. Post-layout work MAY derive immutable geometry,
coverage and execution values from those inputs, but SHALL NOT mutate control
state or sample Timeline. Outside this scope the original requirement remains.

### ANIM-INV-12 — Explicit Initial State

An owner initializes every cached value and optional handle before its first
frame. There is no implicit mount animation and no universal assumption that the
neutral value is numeric zero. Mount, replacement, and unmount clear or cancel
owned handles according to their explicit lifetime.

---

## 4. Animation Descriptor

One animation descriptor contains:

```text
admitted payload type
from
to
positive finite leg duration
deterministic easing
playback mode
```

Start time is assigned by the engine from the current captured frame timestamp;
it is not caller-supplied wall time. Descriptors with zero, negative, non-finite,
or unrepresentable duration/count products fail before handle admission.

### 4.1 Interpolation

Interpolation is pure:

```text
value = interpolate(leg_start, leg_end, eased_phase)
```

`phase` is in `[0, 1]`. Easing is deterministic, finite for every admitted phase,
and constant-time. Color interpolation uses the color space declared by the
admitted color type; the Timeline does not reinterpret storage encoding.

The baseline admitted payload set must be explicit and tested. Existing small
value roles such as `f32`, `f64`, `Point`, and `Color` are suitable; adding a type
requires its interpolation and storage admission to be part of the same verified
change.

### 4.2 Playback modes

The generic modes are:

- **Once:** one forward leg from `from` to `to`;
- **Loop:** repeated forward legs from `from` to `to`, with a positive finite
  cycle count or explicit infinite count; and
- **Alternate / PingPong:** legs alternate `from -> to`, then `to -> from`, with
  a positive finite leg count or explicit infinite count.

No implicit repetition, auto-reverse, pause, resume, delay, keyframe graph, or
timer-driven restart is part of this contract.

### 4.3 Absolute-time evaluation

Let `D` be one positive finite leg duration and `E = max(0, now - start_time)`.
Evaluation derives the cycle/leg quotient and local remainder directly from
`E / D`. It never increments phase by frame delta.

For `Once`:

- `E = 0` samples `from`;
- `0 < E < D` samples the eased forward leg; and
- `E >= D` samples `Completed(to)`.

For `Loop`:

- each nonterminal cycle is a forward leg;
- at an exact interior boundary `E = kD`, the new cycle starts at `from`;
- a finite count of `N` completes at `E >= ND` with exact terminal `to`; and
- an infinite count never completes without explicit cancellation.

For `Alternate` / `PingPong`:

- leg zero is `from -> to`, leg one is `to -> from`, and direction continues by
  leg parity;
- at an exact interior boundary `E = kD`, the sample is the shared endpoint:
  `to` for odd `k`, `from` for even `k`;
- a reverse leg applies the same easing to interpolation from `to` to `from`;
- a finite count of `N` legs completes at `E >= ND`, with terminal `to` for odd
  `N` and `from` for even `N`; and
- an infinite count never completes without explicit cancellation.

Cycle/leg arithmetic is checked and non-wrapping. A finite count whose total
duration is not representable fails admission. An infinite elapsed quotient that
cannot be represented fails explicitly rather than wrapping parity or producing
an unspecified value.

---

## 5. Handle Lifecycle and Sampling

### 5.1 Start and replacement

Starting allocates a fresh non-wrapping handle and records the current engine
frame time. Replacing an effect is an owner operation: the owner samples or uses
its current cached value as the new `from`, cancels the previous handle, starts a
new descriptor, and stores the new handle. Timeline does not infer replacement
from matching endpoints or owners.

### 5.2 Cancellation and retirement

Cancellation is idempotent for the same live handle and produces the observable
`Cancelled` state for the current frame. It does not synthesize a terminal value.
Completion and cancellation retire only after their frame-wide observation
window; subsequent reads are `Missing`.

An unmounted or replaced owner cannot leave an infinite animation as anonymous
permanent frame demand. The owner or engine lifecycle boundary cancels and
releases every owned handle.

### 5.3 Read capabilities

Event and update contexts may receive explicit start/cancel capability. Update
context additionally exposes generic read-only same-frame sampling. Paint may
retain read-only sampling for paint-owned finite consumers, but a non-paint
consumer cannot wait until paint to publish state required earlier in the frame.

The Timeline and its handles are never exposed to custom-render preparation,
renderer preparation, RenderGraph execution, shaders, or GPU submission.

---

## 6. Canonical Frame Integration

SPEC-HAL-042 is the sole authority for the complete frame order, including
engine-owned pre-routing scroll visual advance and raw/input-fusion completion.
SPEC-HAL-021 adds only these Timeline-relative constraints to that order:

1. EngineTicker captures the frame timestamp once.
2. After the preceding phases owned by SPEC-HAL-042, Timeline advances existing
   handles exactly once at that timestamp and before event routing.
3. Fused events route; handlers may start/cancel handles at the same timestamp
   but cannot advance Timeline.
4. Widget update runs after routing, samples Timeline read-only, updates owner
   state, and emits the narrowest invalidation without a second advance.
5. SPEC-HAL-266A custom preparation runs only after that owner publication and
   consumes immutable widget-owned state without Timeline access.

Layout, SPEC-HAL-147 post-layout geometry-transition projection, retained paint,
renderer preparation, GPU submission, and all unrelated phases retain their
literal placement and authority from SPEC-HAL-042 and their owning SPECs.

The advance-before-routing order is deliberate. If routing preceded advance,
an event-started handle whose start time came from the previous Timeline update
could jump by one frame delta. Binding to the already-captured current frame time
instead makes the first same-frame sample exactly `from`.

After Widget update completes, the state consumed by layout, custom preparation,
paint, and rendering is immutable for that frame. Custom preparation does not
sample Timeline directly because SPEC-HAL-354 forbids it from mutating
WidgetTree/application state and SPEC-HAL-266A grants only GPU cache/lifecycle
mutation from already-authorized inputs.

**363/364 scoped amendment:** Causal reservation and prior-outcome reconciliation occur within the
existing Update boundary. Post-layout admission and pure projection occur at the
existing SPEC-HAL-147 boundary, while source completeness may be established
during the ordinary retained scene pass. No group receives a post-layout start,
restart, cancellation, sampling capability or second advance. A failed same-frame
first admission expires; it cannot join later at advanced progress.

---

## 7. Invalidation and Scheduling

A sampled owner compares the new presentation value/status with its previous
state and reports the narrowest legal effect:

- identical sample: no invalidation;
- visual value with stable geometry: paint-only or the consumer's explicit
  custom-presentation path;
- geometry projection: only through the separate SPEC-HAL-147 transition
  authority; and
- semantic or structural change: not authorized by Timeline output.

`needs_animation` is reserved for an actually unsettled visual effect under
SPEC-HAL-354. Active Timeline state may keep the engine's existing animation
demand active, including at 120 Hz where the platform/display path admits it.
The engine may throttle or skip frames under its own policy. On the next admitted
frame, absolute-time evaluation catches up without accumulated-delta drift.

An infinite animation is therefore intentionally continuous engine demand until
its owner cancels it. Merely keeping a receiver, timer, context, viewport, or
inactive widget alive is not animation demand.

---

## 8. Consumer Patterns

### 8.1 Paint-owned finite transition

The widget stores a current presentation value and optional handle. An event
starts or replaces `Once`. A read-only sample applies `Active(value)` or the exact
`Completed(value)`; completion then clears the handle. Cancelled, missing, and
type mismatch follow distinct owner policy rather than preserving an accidental
last pre-terminal sample.

### 8.2 Update-owned continuous value

The widget stores a repeating handle plus its own complete immutable state and
revision. Each admitted frame, Widget update samples the bounded animation value,
derives the complete owner state, advances its semantic revision only when the
derived value changed, and publishes the narrowest invalidation. Custom-render
preparation later in that same frame consumes the immutable owner state and
revision without Timeline access.

### 8.3 Screen transitions

An engine transition layer may use one or more Timeline handles for fade,
translation, or scale while navigation and screen-stack mutation remain owned by
their existing deferred mutation authority. Timeline changes only the visual
projection; it does not decide push/pop policy or retain navigation state.

---

## 9. Forbidden Behaviors

**FORBID-021-PRIVATE-TIME:** Calling a private clock, accumulating independent
phase, sleeping, creating timers, or scheduling frames from animation code.

**FORBID-021-AMBIGUOUS-TERMINAL:** Removing a completed handle before its exact
terminal value/status can be observed for that frame, or representing completion,
cancellation, missing identity, and type mismatch as one undifferentiated result.

**FORBID-021-DT-PLAYBACK:** Implementing `Loop` or `Alternate` by adding frame
delta to stored phase or by restarting finite animations from a timer.

**FORBID-021-RENDERER-SAMPLING:** Giving Timeline access to custom-render
preparation, renderer preparation, RenderGraph, shader, encoder, or submission
code.

**FORBID-021-SEMANTIC-AUTHORITY:** Using an interpolated value as permission to
mutate application meaning, WidgetTree structure, canonical layout/hit-test
geometry, focus, selection, navigation, or interaction policy.

**FORBID-021-UNADMITTED-PAYLOAD:** Accepting an arbitrary interpolation type that
the Timeline evaluator cannot actually update and then returning a frozen or
silently missing value.

**FORBID-021-ORPHANED-INFINITE:** Leaving explicit infinite playback alive after
its owner is unmounted, replaced, retired, or no longer intends the effect.

**FORBID-021-PARALLEL-SYSTEM:** Creating a widget-, viewport-, renderer-, or
3D-specific animation timeline, playback mode, clock, or scheduling loop.

---

## 10. Performance and Storage Requirements

- Advance is `O(active + current-frame terminal records)`.
- Sample is bounded and does not allocate.
- Easing and interpolation are constant-time per sampled value.
- No history of prior values, cycles, or completed frames is retained.
- Start may consume pre-admitted storage; exhaustion is explicit and atomic.
- Repeated finite start/complete and infinite start/cancel cycles stabilize at a
  finite storage high-water mark.
- Payload storage is private and may be typed, enum-backed, or type-erased only
  if ANIM-INV-5, ANIM-INV-10, and ANIM-INV-11 remain mechanically true.

This SPEC does not prescribe one Rust container. It prescribes observable type,
lifetime, allocation, and failure behavior.

---

## 11. Required Proof

Conformance requires all of:

1. one engine timestamp and exactly one Timeline advance per admitted frame;
2. a handle started by an event samples exact `from` during same-frame Widget
   update, while an older handle samples the current frame time;
3. `Once` exposes exact `Completed(to)` for the completion frame before
   retirement;
4. active, completed, cancelled, missing, and type-mismatch observations are
   distinct and do not mutate one another;
5. table-driven `Once`, finite/infinite `Loop`, and finite/infinite
   `Alternate` tests cover start, interior, exact boundaries, terminal parity,
   large elapsed time, invalid duration/count, and cancellation;
6. 60 Hz and 120 Hz evaluations are exactly equal at every shared timestamp;
7. an update-owned sample publishes immutable state consumed by same-frame
   custom preparation with no one-frame lag and no Timeline access in that
   preparation or renderer path;
8. existing paint-owned finite consumers receive the exact terminal value rather
   than retaining a last pre-terminal approximation;
9. unmount/replacement cancels owned infinite playback and frame demand stops;
10. unsupported payload admission and mismatched retrieval fail explicitly;
11. repeated lifecycle and sustained playback keep update/sample allocation-free
    after warm-up and storage bounded; and
12. SPEC-HAL-147 post-layout geometry projection remains separate and preserves
    canonical layout/hit-test authority.

---

## 12. Relationship to Other Animation Documents

- SPEC-HAL-042 owns literal frame order and the EngineTicker.
- SPEC-HAL-043 owns frame admission, timers, throttling, and idle policy; timers
  do not emulate Timeline repetition.
- SPEC-HAL-147 owns post-layout authorized geometry interpolation only.
- SPEC-HAL-354 owns update cadence, invalidation, retained reuse, and the ban on
  cross-phase WidgetTree/application mutation.
- SPEC-HAL-070 remains a Draft design exploration. Its keyframe graph, direct
  renderer writes, and universal system are not promoted or imported. Only the
  generic need for explicit repeating playback is resolved here.

---

## 13. Implementation Status and Gate

This binding amendment ratifies the generic contract; it does not claim the live
implementation already conforms. At amendment time, the live Timeline provides
engine-owned finite interpolation and paint-time lookup, but it does not yet
provide same-frame UpdateContext sampling, distinct terminal/cancel/missing/type
status, exact completion-frame retention, or generic Loop/Alternate playback.

Implementation may proceed only as one coherent generic Timeline change with the
proof in §11. It must not first add a 3D-only animation path, timer restart loop,
or renderer-side sampling workaround.

---

## 14. Authoritative Statement

**AUTH-021-002:** Halcyon animation is deterministic, bounded value
interpolation over one engine-owned monotonic frame time. It produces frame-stable
read-only samples for owner-controlled presentation state, preserves exact finite
completion, admits repetition explicitly, and owns no semantic state, renderer
state, interaction policy, clock, or scheduler.
